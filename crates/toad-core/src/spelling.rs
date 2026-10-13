//! Content > Spelling & Grammar.
//!
//! Spelling uses Hunspell dictionaries through `spellbook` (pure Rust). A
//! dictionary is downloaded the first time a language is needed, from the
//! wooorm/dictionaries collection, into the configured dictionaries folder.
//! Grammar is checked by a LanguageTool server (`/v2/check`) whose address
//! the user configures; nothing is sent anywhere unless one is set.

use crate::config::{CrawlConfig, LanguageMode, SpellingConfig};
use serde::{Deserialize, Serialize};
use spellbook::Dictionary;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Languages with a Hunspell dictionary: (BCP 47 code, name, dictionary id).
pub const LANGUAGES: &[(&str, &str, &str)] = &[
    ("en-US", "English (USA)", "en"),
    ("en-GB", "English (UK)", "en-GB"),
    ("en-AU", "English (Australia)", "en-AU"),
    ("en-CA", "English (Canada)", "en-CA"),
    ("bg-BG", "Bulgarian", "bg"),
    ("ca-ES", "Catalan", "ca"),
    ("hr-HR", "Croatian", "hr"),
    ("cs-CZ", "Czech", "cs"),
    ("da-DK", "Danish", "da"),
    ("nl-NL", "Dutch", "nl"),
    ("et-EE", "Estonian", "et"),
    ("fr-FR", "French", "fr"),
    ("de-DE", "German", "de"),
    ("el-GR", "Greek", "el"),
    ("hu-HU", "Hungarian", "hu"),
    ("ga-IE", "Irish", "ga"),
    ("it-IT", "Italian", "it"),
    ("lv-LV", "Latvian", "lv"),
    ("lt-LT", "Lithuanian", "lt"),
    ("nb-NO", "Norwegian (Bokmål)", "nb"),
    ("pl-PL", "Polish", "pl"),
    ("pt-BR", "Portuguese (Brazil)", "pt"),
    ("pt-PT", "Portuguese (Portugal)", "pt-PT"),
    ("ro-RO", "Romanian", "ro"),
    ("ru-RU", "Russian", "ru"),
    ("sk-SK", "Slovak", "sk"),
    ("sl-SI", "Slovenian", "sl"),
    ("es-ES", "Spanish", "es"),
    ("sv-SE", "Swedish", "sv"),
    ("tr-TR", "Turkish", "tr"),
    ("uk-UA", "Ukrainian", "uk"),
];

const DICTIONARY_BASE: &str = "https://raw.githubusercontent.com/wooorm/dictionaries/main/dictionaries";
const MAX_ERRORS_PER_PAGE: usize = 200;
const MAX_SUGGESTIONS_PER_PAGE: usize = 50;

/// One spelling or grammar error on a page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanguageIssue {
    /// "Spelling" or "Grammar".
    pub kind: String,
    /// The misspelt word, or the flagged text.
    pub text: String,
    pub suggestion: Option<String>,
    /// Grammar rule id (LanguageTool), empty for spelling.
    pub rule: String,
    pub message: String,
    /// The language code used, e.g. en-GB.
    pub language: String,
}

/// The language to check a page in: Manual forces the configured one; Auto
/// uses the page's `lang`, taking the configured dialect for a bare primary
/// language (e.g. `en` → the chosen English) and US English otherwise.
pub fn resolve_language(cfg: &SpellingConfig, page_lang: Option<&str>) -> Option<&'static str> {
    let configured = LANGUAGES.iter().find(|(c, ..)| c.eq_ignore_ascii_case(&cfg.language)).map(|(c, ..)| *c);
    if cfg.language_mode == LanguageMode::Manual {
        return configured;
    }
    let Some(lang) = page_lang.map(|l| l.trim().replace('_', "-")).filter(|l| !l.is_empty()) else { return configured };
    if let Some((c, ..)) = LANGUAGES.iter().find(|(c, ..)| c.eq_ignore_ascii_case(&lang)) {
        return Some(c);
    }
    let primary = lang.split('-').next().unwrap_or("").to_ascii_lowercase();
    if let Some(c) = configured.filter(|c| c.to_ascii_lowercase().starts_with(&format!("{primary}-"))) {
        return Some(c);
    }
    LANGUAGES.iter().find(|(c, ..)| c.to_ascii_lowercase().starts_with(&format!("{primary}-"))).map(|(c, ..)| *c)
}

fn dictionary_id(code: &str) -> Option<&'static str> {
    LANGUAGES.iter().find(|(c, ..)| c.eq_ignore_ascii_case(code)).map(|(_, _, id)| *id)
}

/// Download a language's dictionary into `dir` (`<id>.aff` / `<id>.dic`).
pub async fn download_dictionary(dir: &Path, code: &str) -> anyhow::Result<()> {
    let id = dictionary_id(code).ok_or_else(|| anyhow::anyhow!("no dictionary for {code}"))?;
    std::fs::create_dir_all(dir)?;
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(60)).build()?;
    for ext in ["aff", "dic"] {
        let url = format!("{DICTIONARY_BASE}/{id}/index.{ext}");
        let res = client.get(&url).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("{url} returned {}", res.status());
        }
        let bytes = res.bytes().await?;
        std::fs::write(dir.join(format!("{id}.{ext}")), &bytes)?;
    }
    Ok(())
}

pub fn dictionary_installed(dir: &Path, code: &str) -> bool {
    dictionary_id(code).is_some_and(|id| dir.join(format!("{id}.aff")).exists() && dir.join(format!("{id}.dic")).exists())
}

/// Spell and grammar checking for one crawl: dictionaries are loaded (and,
/// if missing, downloaded) once per language and shared by every page.
pub struct LanguageChecker {
    cfg: SpellingConfig,
    dir: Option<PathBuf>,
    dictionaries: Mutex<HashMap<String, Option<Arc<Dictionary>>>>,
    ignore: HashSet<String>,
    client: reqwest::Client,
}

impl LanguageChecker {
    pub fn new(cfg: &CrawlConfig) -> Self {
        let s = &cfg.spelling;
        let ignore = s.ignore_words.iter().chain(s.dictionary_words.iter()).map(|w| w.trim().to_lowercase()).filter(|w| !w.is_empty()).collect();
        Self {
            cfg: s.clone(),
            dir: s.dictionaries_dir.clone(),
            dictionaries: Mutex::new(HashMap::new()),
            ignore,
            client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().unwrap_or_default(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.cfg.spell_check || (self.cfg.grammar_check && !self.cfg.grammar_server.trim().is_empty())
    }

    /// Use an already-parsed dictionary for a language (tests, custom setups).
    pub fn insert_dictionary(&self, code: &str, dict: Dictionary) {
        if let Ok(mut d) = self.dictionaries.lock() {
            d.insert(code.to_string(), Some(Arc::new(dict)));
        }
    }

    async fn dictionary(&self, code: &str) -> Option<Arc<Dictionary>> {
        if let Some(cached) = self.dictionaries.lock().ok()?.get(code).cloned() {
            return cached;
        }
        let loaded = match &self.dir {
            Some(dir) => {
                if !dictionary_installed(dir, code) {
                    if let Err(e) = download_dictionary(dir, code).await {
                        eprintln!("spelling: could not download the {code} dictionary: {e}");
                    }
                }
                let id = dictionary_id(code)?;
                let read = |ext: &str| std::fs::read_to_string(dir.join(format!("{id}.{ext}"))).ok();
                match (read("aff"), read("dic")) {
                    (Some(aff), Some(dic)) => Dictionary::new(&aff, &dic).ok().map(Arc::new),
                    _ => None,
                }
            }
            None => None,
        };
        if let Ok(mut d) = self.dictionaries.lock() {
            d.insert(code.to_string(), loaded.clone());
        }
        loaded
    }

    /// Check a page's text (title, meta description and content area).
    pub async fn check(&self, text: &str, page_lang: Option<&str>) -> Vec<LanguageIssue> {
        let Some(code) = resolve_language(&self.cfg, page_lang) else { return vec![] };
        let mut out = Vec::new();
        if self.cfg.spell_check {
            if let Some(dict) = self.dictionary(code).await {
                out.extend(self.spelling(&dict, text, code));
            }
        }
        if self.cfg.grammar_check && !self.cfg.grammar_server.trim().is_empty() {
            match self.grammar(text, code).await {
                Ok(g) => out.extend(g),
                Err(e) => eprintln!("grammar: {e}"),
            }
        }
        out.truncate(MAX_ERRORS_PER_PAGE);
        out
    }

    fn spelling(&self, dict: &Dictionary, text: &str, code: &str) -> Vec<LanguageIssue> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for word in words(text) {
            let key = word.to_lowercase();
            if self.ignore.contains(&key) || !seen.insert(key) || dict.check(word) {
                continue;
            }
            let suggestion = (out.len() < MAX_SUGGESTIONS_PER_PAGE)
                .then(|| {
                    let mut s = Vec::new();
                    dict.suggest(word, &mut s);
                    s.into_iter().next()
                })
                .flatten();
            out.push(LanguageIssue {
                kind: "Spelling".into(),
                text: word.to_string(),
                suggestion,
                rule: String::new(),
                message: "Possible spelling mistake".into(),
                language: code.to_string(),
            });
        }
        out
    }

    async fn grammar(&self, text: &str, code: &str) -> anyhow::Result<Vec<LanguageIssue>> {
        #[derive(Deserialize)]
        struct Resp {
            matches: Vec<Match>,
        }
        #[derive(Deserialize)]
        struct Match {
            message: String,
            offset: usize,
            length: usize,
            #[serde(default)]
            replacements: Vec<Replacement>,
            rule: Rule,
        }
        #[derive(Deserialize)]
        struct Replacement {
            value: String,
        }
        #[derive(Deserialize)]
        struct Rule {
            id: String,
            #[serde(default)]
            category: Option<Category>,
        }
        #[derive(Deserialize)]
        struct Category {
            id: String,
        }
        let url = format!("{}/v2/check", self.cfg.grammar_server.trim_end_matches('/'));
        let mut form = vec![("text", text.to_string()), ("language", code.to_string())];
        // Spelling is checked separately (with the user's dictionary).
        let mut disabled = self.cfg.disabled_grammar_rules.clone();
        disabled.push("MORFOLOGIK_RULE_EN_US".into());
        form.push(("disabledRules", disabled.join(",")));
        form.push(("disabledCategories", "TYPOS".into()));
        let res = self.client.post(&url).form(&form).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("{url} returned {}", res.status());
        }
        let body: Resp = serde_json::from_slice(&res.bytes().await?)?;
        Ok(body
            .matches
            .into_iter()
            .filter(|m| m.rule.category.as_ref().is_none_or(|c| c.id != "TYPOS"))
            .map(|m| LanguageIssue {
                kind: "Grammar".into(),
                text: text.chars().skip(m.offset).take(m.length).collect(),
                suggestion: m.replacements.into_iter().next().map(|r| r.value),
                rule: m.rule.id,
                message: m.message,
                language: code.to_string(),
            })
            .collect())
    }
}

/// The words of a text worth checking: tokens with digits, @ or / (codes,
/// emails, URLs) and all-caps acronyms are skipped; apostrophes are kept
/// inside words.
pub fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split_whitespace()
        .filter(|t| !t.chars().any(|c| c.is_ascii_digit() || c == '@' || c == '/' || c == '\\' || c == '_'))
        .flat_map(|t| t.split(|c: char| !(c.is_alphabetic() || c == '\'' || c == '’')))
        .map(|w| w.trim_matches(|c| c == '\'' || c == '’'))
        .filter(|w| w.chars().count() > 1)
        .filter(|w| !(w.chars().all(|c| !c.is_lowercase())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SpellingConfig;

    const AFF: &str = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzESIANRTOLCDUGMPHBYFVKWZ'\n";
    const DIC: &str = "6\nthe\ntoad\nlives\nin\na\npond\n";

    #[test]
    fn tokenises_words() {
        let w: Vec<&str> = words("The toad's pond, 2024 NASA a@b.com https://x.com tadpole-season").collect();
        assert_eq!(w, vec!["The", "toad's", "pond", "tadpole", "season"]);
    }

    #[test]
    fn languages_auto_and_manual() {
        let auto = SpellingConfig { language: "en-GB".into(), ..SpellingConfig::default() };
        assert_eq!(resolve_language(&auto, Some("en")), Some("en-GB"), "bare en takes the chosen dialect");
        assert_eq!(resolve_language(&auto, Some("en-US")), Some("en-US"));
        assert_eq!(resolve_language(&auto, Some("fr")), Some("fr-FR"));
        assert_eq!(resolve_language(&auto, Some("xx")), None);
        assert_eq!(resolve_language(&auto, None), Some("en-GB"));
        let manual = SpellingConfig { language_mode: LanguageMode::Manual, language: "de-DE".into(), ..SpellingConfig::default() };
        assert_eq!(resolve_language(&manual, Some("fr")), Some("de-DE"));
    }

    #[tokio::test]
    async fn finds_spelling_errors_and_respects_ignore() {
        let cfg = CrawlConfig {
            spelling: SpellingConfig { spell_check: true, language: "en-US".into(), ignore_words: vec!["Toadz".into()], ..SpellingConfig::default() },
            ..CrawlConfig::default()
        };
        let checker = LanguageChecker::new(&cfg);
        checker.insert_dictionary("en-US", Dictionary::new(AFF, DIC).unwrap());
        let issues = checker.check("The toad livse in a pnd. Toadz too. The toad livse.", Some("en")).await;
        let words: Vec<&str> = issues.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(words, vec!["livse", "pnd", "too"], "each word once; ignored words skipped");
        assert_eq!(issues[0].suggestion.as_deref(), Some("lives"));
        assert!(issues.iter().all(|i| i.kind == "Spelling" && i.language == "en-US"));
    }
}
