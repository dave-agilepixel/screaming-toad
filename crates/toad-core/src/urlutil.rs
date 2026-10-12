//! URL resolution, normalisation, rewriting and crawl scope.

use crate::config::{CrawlConfig, Mode, PercentCase};
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use url::Url;

/// Compiled regexes for URL Rewriting, cached by pattern (normalize runs
/// for every discovered link).
fn cached_regex(pattern: &str) -> Option<Regex> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Regex>>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().ok()?;
    cache.entry(pattern.to_string()).or_insert_with(|| Regex::new(pattern).ok()).clone()
}

/// Resolve an href found on a page. Returns None for non-HTTP schemes
/// (mailto:, tel:, javascript:, data:) and unparseable values.
pub fn resolve(base: &Url, href: &str) -> Option<Url> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    let lower = href.to_ascii_lowercase();
    if ["mailto:", "tel:", "javascript:", "data:", "sms:", "ftp:", "file:"]
        .iter()
        .any(|s| lower.starts_with(s))
    {
        return None;
    }
    let url = base.join(href).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(url)
}

/// Crawl Invalid Links: an absolute href whose scheme looks like a typo of
/// http(s), such as `hppts://` or `htp://`. Returned as parsed so it can be
/// reported as a malformed URL.
pub fn malformed(href: &str) -> Option<Url> {
    let url = Url::parse(href.trim()).ok()?;
    let scheme = url.scheme();
    let typo = scheme.starts_with('h') && (3..=6).contains(&scheme.len()) && !matches!(scheme, "http" | "https");
    (typo && url.host_str().is_some()).then_some(url)
}

/// Apply URL Rewriting settings and fragment handling.
pub fn normalize(mut url: Url, cfg: &CrawlConfig) -> Url {
    if !cfg.crawl_fragment_identifiers {
        url.set_fragment(None);
    }
    if cfg.remove_all_parameters {
        url.set_query(None);
    } else if !cfg.remove_parameters.is_empty() && url.query().is_some() {
        let kept: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| !cfg.remove_parameters.iter().any(|p| p == k.as_ref()))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if kept.is_empty() {
            url.set_query(None);
        } else {
            url.query_pairs_mut().clear().extend_pairs(kept);
        }
    }
    for rule in cfg.url_rewrite_rules.iter().filter(|r| !r.regex.is_empty()) {
        if let Some(re) = cached_regex(&rule.regex) {
            let rewritten = re.replace_all(url.as_str(), rule.replace.as_str());
            if rewritten != url.as_str() {
                if let Ok(u) = Url::parse(&rewritten) {
                    url = u;
                }
            }
        }
    }
    if cfg.lowercase_urls {
        if let Ok(lower) = Url::parse(&url.as_str().to_lowercase()) {
            url = lower;
        }
    }
    if cfg.percent_encoding_case != PercentCase::None && url.as_str().contains('%') {
        let upper = cfg.percent_encoding_case == PercentCase::Uppercase;
        let s = url.as_str();
        let mut out = String::with_capacity(s.len());
        let mut chars = s.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            let hex = s.get(i + 1..i + 3).filter(|h| h.chars().all(|c| c.is_ascii_hexdigit()));
            match (c, hex) {
                ('%', Some(h)) => {
                    out.push('%');
                    out.push_str(&if upper { h.to_ascii_uppercase() } else { h.to_ascii_lowercase() });
                    chars.next();
                    chars.next();
                }
                _ => out.push(c),
            }
        }
        if let Ok(u) = Url::parse(&out) {
            url = u;
        }
    }
    url
}

/// The Test panes of URL Rewriting, CDNs, Include and Exclude: how one URL
/// is treated under a configuration.
#[derive(Debug, Clone, Serialize)]
pub struct UrlTest {
    /// The URL after URL Rewriting (as a discovered URL).
    pub rewritten: String,
    /// The URL-encoded form Include/Exclude regexes run against.
    pub encoded: String,
    /// Internal or External, given the start URL and the CDN list.
    pub internal: Option<bool>,
    pub include_match: Option<String>,
    /// Include allows it (true when the Include list is empty).
    pub included: bool,
    pub exclude_match: Option<String>,
    pub error: Option<String>,
}

pub fn test_url(cfg: &CrawlConfig, start: Option<&str>, url: &str) -> UrlTest {
    let mut t = UrlTest {
        rewritten: String::new(),
        encoded: String::new(),
        internal: None,
        include_match: None,
        included: cfg.include.is_empty(),
        exclude_match: None,
        error: None,
    };
    let parsed = match Url::parse(url.trim()) {
        Ok(u) => u,
        Err(e) => {
            t.error = Some(format!("not a valid URL: {e}"));
            return t;
        }
    };
    t.encoded = parsed.to_string();
    let rewritten = normalize(parsed, cfg);
    t.rewritten = rewritten.to_string();
    if let Some(start) = start.and_then(|s| Url::parse(s.trim()).ok()) {
        t.internal = Some(Scope::new(&start, cfg).is_internal(&rewritten));
    }
    let s = rewritten.as_str();
    t.include_match = cfg.include.iter().find(|p| Regex::new(p).is_ok_and(|r| r.is_match(s))).cloned();
    t.included |= t.include_match.is_some();
    t.exclude_match = cfg.exclude.iter().find(|p| Regex::new(p).is_ok_and(|r| r.is_match(s))).cloned();
    t
}

/// Whether a List mode input is crawled: Include and Exclude apply to every
/// URL in List mode (but never to the Spider start URL).
pub fn list_input_allowed(cfg: &CrawlConfig, url: &Url, include: &[Regex], exclude: &[Regex]) -> bool {
    if cfg.mode != Mode::List {
        return true;
    }
    let s = url.as_str();
    (include.is_empty() || include.iter().any(|r| r.is_match(s))) && !exclude.iter().any(|r| r.is_match(s))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathType {
    Absolute,
    ProtocolRelative,
    RootRelative,
    PathRelative,
}

impl PathType {
    pub fn of(href: &str) -> Self {
        let h = href.trim();
        if h.starts_with("//") {
            PathType::ProtocolRelative
        } else if h.starts_with('/') {
            PathType::RootRelative
        } else if h.contains("://") {
            PathType::Absolute
        } else {
            PathType::PathRelative
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PathType::Absolute => "Absolute",
            PathType::ProtocolRelative => "Protocol-Relative",
            PathType::RootRelative => "Root-Relative",
            PathType::PathRelative => "Path-Relative",
        }
    }
}

/// `/` = 0, `/a/` = 1, `/a/page.html` = 1, `/a/b/` = 2.
pub fn folder_depth(url: &Url) -> u32 {
    url.path().matches('/').count().saturating_sub(1) as u32
}

pub fn query_param_count(url: &Url) -> u32 {
    url.query()
        .map(|q| q.split('&').filter(|p| !p.is_empty()).count() as u32)
        .unwrap_or(0)
}

/// Decides what is Internal, and whether the crawl may go there.
#[derive(Debug, Clone)]
pub struct Scope {
    host: String,
    root_domain: String,
    include_subdomains: bool,
    start_folder: String,
    /// Configuration > CDNs: (host, path prefix) pairs treated as Internal.
    cdns: Vec<(String, String)>,
}

impl Scope {
    pub fn new(start: &Url, cfg: &CrawlConfig) -> Self {
        let host = start.host_str().unwrap_or_default().to_ascii_lowercase();
        let root_domain = host.strip_prefix("www.").unwrap_or(&host).to_string();
        // A start URL with no subdomain crawls every subdomain, like a site: query.
        let include_subdomains = cfg.crawl_all_subdomains || !has_subdomain(&host);
        let path = start.path();
        let start_folder = match path.rfind('/') {
            Some(i) => path[..=i].to_string(),
            None => "/".to_string(),
        };
        let cdns = cfg
            .cdns
            .iter()
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .map(|c| {
                let c = c.split_once("://").map_or(c, |(_, rest)| rest);
                let (h, p) = c.split_once('/').map_or((c, String::from("/")), |(h, p)| (h, format!("/{p}")));
                (h.to_ascii_lowercase(), p)
            })
            .collect();
        Self { host, root_domain, include_subdomains, start_folder, cdns }
    }

    pub fn is_internal(&self, url: &Url) -> bool {
        let host = match url.host_str() {
            Some(h) => h.to_ascii_lowercase(),
            None => return false,
        };
        if host == self.host {
            return true;
        }
        if self.cdns.iter().any(|(h, p)| *h == host && url.path().starts_with(p.as_str())) {
            return true;
        }
        self.include_subdomains
            && (host == self.root_domain || host.ends_with(&format!(".{}", self.root_domain)))
    }

    pub fn in_start_folder(&self, url: &Url) -> bool {
        url.path().starts_with(&self.start_folder)
    }
}

/// Rough check for a subdomain. A public-suffix list would be exact; this
/// handles the common two-part suffixes (co.uk, com.au and so on).
fn has_subdomain(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    if host.parse::<std::net::IpAddr>().is_ok() || labels.len() < 3 {
        return false;
    }
    let n = labels.len();
    let two_part_suffix = labels[n - 1].len() == 2
        && ["co", "com", "org", "net", "ac", "gov", "edu", "ltd", "plc", "me"]
            .contains(&labels[n - 2]);
    if two_part_suffix {
        labels.len() > 3
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{PercentCase, RegexReplace};

    #[test]
    fn url_rewriting_steps() {
        let cfg = CrawlConfig {
            remove_parameters: vec!["sid".into()],
            url_rewrite_rules: vec![
                RegexReplace { regex: r"staging\.example\.com".into(), replace: "www.example.com".into() },
                RegexReplace { regex: "/old/(.*)".into(), replace: "/new/$1".into() },
            ],
            ..CrawlConfig::default()
        };
        let n = |u: &str| normalize(Url::parse(u).unwrap(), &cfg).to_string();
        assert_eq!(n("https://staging.example.com/old/p?sid=1&x=2#f"), "https://www.example.com/new/p?x=2");
        // Percent-encoding case: Uppercase by default.
        assert_eq!(n("https://www.example.com/caf%c3%a9"), "https://www.example.com/caf%C3%A9");
        let lower = CrawlConfig { percent_encoding_case: PercentCase::Lowercase, ..CrawlConfig::default() };
        assert_eq!(normalize(Url::parse("https://a.com/caf%C3%A9").unwrap(), &lower).as_str(), "https://a.com/caf%c3%a9");
        let none = CrawlConfig { percent_encoding_case: PercentCase::None, ..CrawlConfig::default() };
        assert_eq!(normalize(Url::parse("https://a.com/caf%c3%A9").unwrap(), &none).as_str(), "https://a.com/caf%c3%A9");
        let all = CrawlConfig { remove_all_parameters: true, ..CrawlConfig::default() };
        assert_eq!(normalize(Url::parse("https://a.com/p?a=1&b=2").unwrap(), &all).as_str(), "https://a.com/p");
    }

    #[test]
    fn cdns_are_internal() {
        let cfg = CrawlConfig { cdns: vec!["cdn.example.net".into(), "https://assets.other.com/example/".into()], ..CrawlConfig::default() };
        let scope = Scope::new(&Url::parse("https://www.example.com/").unwrap(), &cfg);
        let internal = |u: &str| scope.is_internal(&Url::parse(u).unwrap());
        assert!(internal("https://cdn.example.net/img/a.png"));
        assert!(internal("https://assets.other.com/example/a.css"));
        assert!(!internal("https://assets.other.com/someone-else/a.css"));
        assert!(!internal("https://elsewhere.com/"));
    }

    #[test]
    fn test_pane_explains_a_url() {
        let cfg = CrawlConfig { include: vec!["/blog/".into()], exclude: vec![r"\?replytocom".into()], ..CrawlConfig::default() };
        let t = test_url(&cfg, Some("https://www.example.com/"), "https://www.example.com/blog/post?replytocom=5");
        assert_eq!(t.internal, Some(true));
        assert_eq!(t.include_match.as_deref(), Some("/blog/"));
        assert!(t.included);
        assert_eq!(t.exclude_match.as_deref(), Some(r"\?replytocom"));
        assert!(test_url(&cfg, None, "not a url").error.is_some());
    }

    use super::*;

    #[test]
    fn folder_depth_matches_screaming_frog() {
        let d = |s: &str| folder_depth(&Url::parse(s).unwrap());
        assert_eq!(d("https://a.com/"), 0);
        assert_eq!(d("https://a.com/a/"), 1);
        assert_eq!(d("https://a.com/a/page.html"), 1);
        assert_eq!(d("https://a.com/a/b/"), 2);
    }

    #[test]
    fn scope_subdomains() {
        let cfg = CrawlConfig::default();
        let s = Scope::new(&Url::parse("https://example.com/").unwrap(), &cfg);
        assert!(s.is_internal(&Url::parse("https://blog.example.com/x").unwrap()));
        let s = Scope::new(&Url::parse("https://www.example.co.uk/").unwrap(), &cfg);
        assert!(!s.is_internal(&Url::parse("https://blog.example.co.uk/x").unwrap()));
        assert!(s.is_internal(&Url::parse("https://www.example.co.uk/x").unwrap()));
    }

    #[test]
    fn removes_parameters() {
        let mut cfg = CrawlConfig::default();
        cfg.remove_parameters = vec!["utm_source".into()];
        let u = normalize(Url::parse("https://a.com/p?utm_source=x&id=2#top").unwrap(), &cfg);
        assert_eq!(u.as_str(), "https://a.com/p?id=2");
    }
}
