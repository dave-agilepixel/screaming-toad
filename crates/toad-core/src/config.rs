//! Crawl configuration. Field names follow the Screaming Frog menus; defaults
//! follow CONFIG_REFERENCE.md. Profiles are saved and loaded as JSON (`.toadconfig`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Spider,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RobotsMode {
    #[default]
    Respect,
    Ignore,
    IgnoreButReport,
}

/// robots.txt > Custom Robots: one subdomain's override.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomRobots {
    /// Host, e.g. `www.example.com` (a scheme or path is ignored).
    pub host: String,
    /// Override Enabled: use `txt` instead of downloading robots.txt.
    pub enabled: bool,
    pub txt: String,
}

impl CustomRobots {
    pub fn host_name(&self) -> String {
        let h = self.host.trim();
        let h = h.split_once("://").map_or(h, |(_, rest)| rest);
        h.split('/').next().unwrap_or("").to_ascii_lowercase()
    }
}

/// URL Rewriting > Regex Replace: one rule.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RegexReplace {
    pub regex: String,
    pub replace: String,
}

/// URL Rewriting > percent-encoding case: `%c3%a9` vs `%C3%A9`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PercentCase {
    #[default]
    Uppercase,
    Lowercase,
    /// Leave encodings as found.
    None,
}

/// Spider > Limits > Limit by URL Path: one row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PathLimit {
    pub pattern: String,
    pub max: u64,
}

impl Default for PathLimit {
    fn default() -> Self {
        Self { pattern: String::new(), max: 1_000 }
    }
}

/// Spider > Advanced > Cookie Storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CookieStorage {
    /// Cookies are kept along a redirect chain only.
    #[default]
    SessionOnly,
    /// One cookie jar shared by the whole crawl.
    Persistent,
    DoNotStore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CrawlConfig {
    pub mode: Mode,

    // Configuration > Spider > Crawl (resource and link types)
    pub store_images: bool,
    pub crawl_images: bool,
    pub store_css: bool,
    pub crawl_css: bool,
    pub store_js: bool,
    pub crawl_js: bool,
    pub store_internal_links: bool,
    pub crawl_internal_links: bool,
    pub store_media: bool,
    pub crawl_media: bool,
    pub store_swf: bool,
    pub crawl_swf: bool,
    pub store_amp: bool,
    pub crawl_amp: bool,
    pub store_meta_refresh: bool,
    pub store_iframes: bool,
    pub store_mobile_alternate: bool,
    pub crawl_mobile_alternate: bool,
    /// Store only: there is no Crawl option for uncrawlable links.
    pub store_uncrawlable_links: bool,
    /// Report malformed hrefs such as `hppts://` as No Response.
    pub crawl_invalid_links: bool,
    pub store_external_links: bool,
    pub crawl_external_links: bool,
    pub store_canonicals: bool,
    pub crawl_canonicals: bool,
    pub store_pagination: bool,
    pub crawl_pagination: bool,
    pub store_hreflang: bool,
    pub crawl_hreflang: bool,
    pub crawl_meta_refresh: bool,
    pub crawl_iframes: bool,

    // Configuration > Spider > Crawl (behaviour)
    pub check_links_outside_start_folder: bool,
    pub crawl_outside_start_folder: bool,
    pub crawl_all_subdomains: bool,
    pub follow_internal_nofollow: bool,
    pub follow_external_nofollow: bool,
    pub crawl_fragment_identifiers: bool,
    pub crawl_linked_sitemaps: bool,
    pub auto_discover_sitemaps_via_robots: bool,
    pub sitemap_urls: Vec<String>,

    // Configuration > Spider > Limits
    pub max_urls: u64,
    pub max_depth: Option<u32>,
    pub max_folder_depth: Option<u32>,
    pub max_query_params: Option<u32>,
    pub max_redirects: u32,
    pub max_url_length: usize,
    pub max_links_per_url: usize,
    pub max_page_size_kb: u64,
    /// Limit Crawl Total Per Subdomain.
    pub max_urls_per_subdomain: Option<u64>,
    /// Limit URLs Per Crawl Depth.
    pub max_urls_per_depth: Option<u64>,
    /// Limit by URL Path: at most `max` URLs matching each regex.
    pub path_limits: Vec<PathLimit>,

    // Configuration > Spider > Advanced
    pub cookie_storage: CookieStorage,
    pub ignore_non_indexable_for_issues: bool,
    pub ignore_paginated_for_duplicates: bool,
    pub always_follow_redirects: bool,
    pub always_follow_canonicals: bool,
    /// Crawl noindex pages but hide them from every tab.
    pub respect_noindex: bool,
    /// Crawl canonicalised pages but hide them from every tab.
    pub respect_canonical: bool,
    /// Crawl rel=prev pages but hide them from every tab.
    pub respect_next_prev: bool,
    /// Report http:// links to an HSTS host as a 307 "HSTS Policy" redirect.
    pub respect_hsts: bool,
    /// A page that meta-refreshes to itself is Non-Indexable.
    pub respect_self_referencing_meta_refresh: bool,
    pub extract_images_from_srcset: bool,
    pub perform_html_validation: bool,
    pub response_timeout_secs: u64,
    pub retries_5xx: u32,
    pub assume_pages_are_html: bool,

    // Configuration > Speed
    pub threads: usize,
    pub max_urls_per_sec: Option<f64>,

    // Configuration > robots.txt
    pub robots_mode: RobotsMode,
    pub show_internal_blocked_by_robots: bool,
    pub show_external_blocked_by_robots: bool,
    /// Custom Robots: robots.txt text used instead of the live file.
    pub custom_robots: Vec<CustomRobots>,

    // Configuration > User-Agent / HTTP Header
    pub user_agent: String,
    pub robots_user_agent: String,
    pub custom_headers: Vec<(String, String)>,

    // Configuration > Include / Exclude (partial regex match, case-sensitive)
    pub include: Vec<String>,
    pub exclude: Vec<String>,

    // Configuration > URL Rewriting (discovered URLs only)
    pub remove_all_parameters: bool,
    pub remove_parameters: Vec<String>,
    /// Regex Replace, applied in order (`$1` back-references).
    pub url_rewrite_rules: Vec<RegexReplace>,
    pub lowercase_urls: bool,
    pub percent_encoding_case: PercentCase,

    // Configuration > CDNs: hosts (or host/path prefixes) treated as Internal.
    pub cdns: Vec<String>,

    // Configuration > Content > Area
    pub content_area: ContentArea,

    // Configuration > Spider > Extraction > Structured Data. JSON-LD and
    // Microdata with validation are on by default here; Screaming Frog has
    // them off (a deliberate deviation).
    pub structured_data: StructuredDataConfig,

    // Configuration > Spider > Extraction: page details, URL details, directives.
    pub extraction: Extraction,

    // Configuration > Spider > Extraction > Store HTML: keep the HTML of
    // internal pages (View Source, page-source exports, Compare "Content").
    pub store_html: bool,

    // Configuration > Spider > Rendering
    pub rendering: RenderingConfig,

    // Configuration > Spider > Extraction > PDF
    /// Parse PDF text: word count, readability, Custom Search, spelling,
    /// and Bulk Export > All PDF Content.
    pub store_pdf: bool,
    /// Subject, Author, Creation/Modification Date and Page Count columns.
    pub extract_pdf_properties: bool,
    /// Record links found in PDFs (with their page) for discovery and inlinks.
    pub extract_pdf_links: bool,

    // Crawl Analysis > Configure
    pub crawl_analysis: CrawlAnalysisConfig,

    // Configuration > Content > Duplicates
    pub near_duplicates: bool,
    pub near_duplicate_threshold: f64,
    pub near_duplicates_indexable_only: bool,

    // Configuration > Content > Spelling & Grammar
    pub spelling: SpellingConfig,

    // Configuration > Custom > Search / Extraction
    pub custom_search: Vec<CustomSearch>,
    pub custom_extraction: Vec<CustomExtractor>,
    /// Custom > Custom JavaScript (needs Rendering: JavaScript).
    pub custom_javascript: Vec<CustomJs>,

    // Configuration > Custom > Link Positions
    /// Off: link paths and positions are not recorded (saves memory).
    pub link_positions_enabled: bool,
    /// Matched top-down against each link's path; the first match wins.
    pub link_positions: Vec<LinkPosition>,

    // Configuration > Spider > Preferences
    pub preferences: Preferences,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            mode: Mode::Spider,
            store_images: true,
            crawl_images: true,
            store_css: true,
            crawl_css: true,
            store_js: true,
            crawl_js: true,
            store_internal_links: true,
            crawl_internal_links: true,
            store_media: false,
            crawl_media: false,
            store_swf: true,
            crawl_swf: true,
            store_amp: false,
            crawl_amp: false,
            store_meta_refresh: true,
            store_iframes: true,
            store_mobile_alternate: false,
            crawl_mobile_alternate: false,
            store_uncrawlable_links: false,
            crawl_invalid_links: false,
            store_external_links: true,
            crawl_external_links: true,
            store_canonicals: true,
            crawl_canonicals: true,
            store_pagination: true,
            crawl_pagination: false,
            store_hreflang: true,
            crawl_hreflang: false,
            crawl_meta_refresh: true,
            crawl_iframes: true,
            check_links_outside_start_folder: true,
            crawl_outside_start_folder: false,
            crawl_all_subdomains: false,
            follow_internal_nofollow: false,
            follow_external_nofollow: false,
            crawl_fragment_identifiers: false,
            crawl_linked_sitemaps: false,
            auto_discover_sitemaps_via_robots: true,
            sitemap_urls: vec![],
            max_urls: 5_000_000,
            max_depth: None,
            max_folder_depth: None,
            max_query_params: None,
            max_redirects: 10,
            max_url_length: 10_000,
            max_links_per_url: 10_000,
            max_page_size_kb: 50_000,
            max_urls_per_subdomain: None,
            max_urls_per_depth: None,
            path_limits: vec![],
            cookie_storage: CookieStorage::SessionOnly,
            ignore_non_indexable_for_issues: true,
            ignore_paginated_for_duplicates: true,
            always_follow_redirects: false,
            always_follow_canonicals: false,
            respect_noindex: false,
            respect_canonical: false,
            respect_next_prev: false,
            respect_hsts: true,
            respect_self_referencing_meta_refresh: true,
            extract_images_from_srcset: false,
            perform_html_validation: true,
            response_timeout_secs: 20,
            retries_5xx: 0,
            assume_pages_are_html: false,
            threads: 5,
            max_urls_per_sec: None,
            robots_mode: RobotsMode::Respect,
            show_internal_blocked_by_robots: true,
            show_external_blocked_by_robots: false,
            custom_robots: vec![],
            user_agent: format!("Screaming Toad SEO Spider/{}", env!("CARGO_PKG_VERSION")),
            robots_user_agent: "Screaming Toad".to_string(),
            custom_headers: vec![
                (
                    "Accept".into(),
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".into(),
                ),
                ("Accept-Encoding".into(), "gzip".into()),
                ("Cache-Control".into(), "no-cache".into()),
                ("Pragma".into(), "no-cache".into()),
            ],
            include: vec![],
            exclude: vec![],
            remove_all_parameters: false,
            remove_parameters: vec![],
            url_rewrite_rules: vec![],
            lowercase_urls: false,
            percent_encoding_case: PercentCase::Uppercase,
            cdns: vec![],
            content_area: ContentArea::default(),
            structured_data: StructuredDataConfig::default(),
            store_html: false,
            rendering: RenderingConfig::default(),
            crawl_analysis: CrawlAnalysisConfig::default(),
            store_pdf: false,
            extract_pdf_properties: false,
            extract_pdf_links: false,
            extraction: Extraction::default(),
            near_duplicates: false,
            near_duplicate_threshold: 0.90,
            near_duplicates_indexable_only: true,
            spelling: SpellingConfig::default(),
            custom_search: vec![],
            custom_extraction: vec![],
            custom_javascript: vec![],
            link_positions_enabled: true,
            link_positions: LinkPosition::defaults(),
            preferences: Preferences::default(),
        }
    }
}

impl CrawlConfig {
    /// List mode defaults, as Screaming Frog applies them: crawl depth 0,
    /// robots.txt ignored and no sitemap auto-discovery. Sitemaps listed under
    /// "Crawl These Sitemaps" are still crawled.
    pub fn for_list_mode(mut self) -> Self {
        self.mode = Mode::List;
        self.max_depth = Some(0);
        self.robots_mode = RobotsMode::Ignore;
        self.auto_discover_sitemaps_via_robots = false;
        self
    }

    /// Spider > Crawl: whether a link of this kind is recorded. Crawl implies
    /// Store; external links also need External Links. HTTP redirects are
    /// always recorded.
    pub fn stores_link(&self, kind: crate::model::LinkKind, internal: bool) -> bool {
        use crate::model::LinkKind::*;
        let (store, crawl) = match kind {
            Redirect => return true,
            Uncrawlable => return self.store_uncrawlable_links,
            Hyperlink => (self.store_internal_links, self.crawl_internal_links),
            Image => (self.store_images, self.crawl_images),
            Css => (self.store_css, self.crawl_css),
            JavaScript => (self.store_js, self.crawl_js),
            Canonical | HttpCanonical => (self.store_canonicals, self.crawl_canonicals),
            RelNext | RelPrev => (self.store_pagination, self.crawl_pagination),
            Hreflang => (self.store_hreflang, self.crawl_hreflang),
            Iframe => (self.store_iframes, self.crawl_iframes),
            MetaRefresh => (self.store_meta_refresh, self.crawl_meta_refresh),
            Media => (self.store_media, self.crawl_media),
            Swf => (self.store_swf, self.crawl_swf),
            Amp => (self.store_amp, self.crawl_amp),
            MobileAlternate => (self.store_mobile_alternate, self.crawl_mobile_alternate),
        };
        if internal {
            store || crawl
        } else {
            let external = self.store_external_links || self.crawl_external_links;
            external && (kind == Hyperlink || store || crawl)
        }
    }

    /// Problems that would stop a crawl starting, or make a setting
    /// meaningless, as readable messages. Empty means the configuration is valid.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for r in &self.url_rewrite_rules {
            if let Err(e) = regex::Regex::new(&r.regex) {
                problems.push(format!("URL Rewriting: '{}' is not a valid regex ({e})", r.regex));
            }
        }
        for l in &self.path_limits {
            if let Err(e) = regex::Regex::new(&l.pattern) {
                problems.push(format!("Limits: '{}' is not a valid regex for Limit by URL Path ({e})", l.pattern));
            }
        }
        for (label, patterns) in [("Include", &self.include), ("Exclude", &self.exclude)] {
            for p in patterns {
                if let Err(e) = regex::Regex::new(p) {
                    problems.push(format!("{label}: '{p}' is not a valid regex ({e})"));
                }
            }
        }
        if let Err(e) = crate::custom::CustomRules::compile(self) {
            problems.push(format!("Custom: {e}"));
        }
        if self.custom_search.len() > 100 {
            problems.push(format!("Custom Search: at most 100 searches ({} configured)", self.custom_search.len()));
        }
        if self.custom_extraction.len() > 100 {
            problems.push(format!("Custom Extraction: at most 100 extractors ({} configured)", self.custom_extraction.len()));
        }
        let mut names = std::collections::HashSet::new();
        for name in self.custom_search.iter().map(|s| &s.name).chain(self.custom_extraction.iter().map(|x| &x.name)) {
            if name.trim().is_empty() {
                problems.push("Custom: every search and extractor needs a name".into());
            } else if !names.insert(name.trim().to_lowercase()) {
                problems.push(format!("Custom: the name '{name}' is used more than once"));
            }
        }
        for (name, value) in &self.custom_headers {
            if reqwest::header::HeaderName::from_bytes(name.as_bytes()).is_err() {
                problems.push(format!("HTTP Header: '{name}' is not a valid header name"));
            } else if reqwest::header::HeaderValue::from_str(value).is_err() {
                problems.push(format!("HTTP Header: the value for '{name}' is not valid"));
            }
        }
        if self.user_agent.trim().is_empty() {
            problems.push("User-Agent: the HTTP user-agent is empty".into());
        }
        if self.threads == 0 {
            problems.push("Speed: Max Threads must be at least 1".into());
        }
        if self.max_urls_per_sec.is_some_and(|r| r <= 0.0) {
            problems.push("Speed: Max URI/s must be above 0".into());
        }
        if !(0.0..=1.0).contains(&self.near_duplicate_threshold) {
            problems.push("Duplicates: the near-duplicate threshold must be between 0% and 100%".into());
        }
        if self.max_urls == 0 {
            problems.push("Limits: Limit Crawl Total must be at least 1".into());
        }
        if self.response_timeout_secs == 0 {
            problems.push("Advanced: Response Timeout must be at least 1 second".into());
        }
        let p = &self.preferences;
        if p.title_min_chars > p.title_max_chars || p.title_min_px > p.title_max_px {
            problems.push("Preferences: the page title minimum is above the maximum".into());
        }
        if p.description_min_chars > p.description_max_chars || p.description_min_px > p.description_max_px {
            problems.push("Preferences: the meta description minimum is above the maximum".into());
        }
        problems
    }

    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

/// Crawl Analysis > Configure. Link counts, redirect chains and Link Origin
/// are always worked out when a crawl ends; these are the heavier areas.
/// Sitemaps, pagination, hreflang and canonical filters are computed live
/// in Toad, so they need no analysis step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrawlAnalysisConfig {
    /// Auto-analyse at End of Crawl. On by default in Toad (deliberate
    /// deviation: SF leaves it off).
    pub auto_analyse: bool,
    /// Link Score (internal PageRank).
    pub link_score: bool,
    /// Content: near duplicates (also needs Content > Duplicates > Enable Near Duplicates).
    pub content: bool,
}

impl Default for CrawlAnalysisConfig {
    fn default() -> Self {
        Self { auto_analyse: true, link_score: true, content: true }
    }
}

/// Custom > Custom JavaScript: Extraction snippets return data with
/// `seoSpider.data(...)`; Action snippets (scrolling, clicking) run first
/// and return nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CustomJsKind {
    #[default]
    Extraction,
    Action,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomJs {
    pub name: String,
    pub kind: CustomJsKind,
    /// The body of an async function. `seoSpider` is in scope.
    pub code: String,
    pub timeout_secs: f64,
    /// Comma-separated content types the snippet runs on.
    pub content_types: String,
}

impl Default for CustomJs {
    fn default() -> Self {
        Self { name: String::new(), kind: CustomJsKind::Extraction, code: String::new(), timeout_secs: 10.0, content_types: "text/html".into() }
    }
}

impl CustomJs {
    pub fn applies_to(&self, content_type: &str) -> bool {
        let ct = content_type.to_ascii_lowercase();
        self.content_types.split(',').map(|t| t.trim().to_ascii_lowercase()).filter(|t| !t.is_empty()).any(|t| ct.contains(&t))
    }
}

/// Spider > Rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RenderMode {
    #[default]
    TextOnly,
    #[serde(rename = "javascript")]
    JavaScript,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderingConfig {
    pub mode: RenderMode,
    pub screenshots: bool,
    pub js_error_reporting: bool,
    pub flatten_shadow_dom: bool,
    pub flatten_iframes: bool,
    /// Wait after the load event before the snapshot.
    pub ajax_timeout_secs: f64,
    /// Window Size preset name (the UI fills the fields below from it).
    pub window_preset: String,
    pub width: u32,
    pub height: u32,
    pub scaling_factor: f64,
    pub mobile: bool,
    pub touch: bool,
    /// Full-page screenshots.
    pub resize_to_content: bool,
    pub resize_time_secs: f64,
    pub rotate: bool,
    /// Spider > Extraction > Store Rendered HTML.
    pub store_rendered_html: bool,
    /// Spider > Extraction > Accessibility: run axe-core on rendered pages.
    pub accessibility: bool,
    /// Chrome / Edge / Chromium executable. Empty: auto-detect.
    pub browser_path: Option<String>,
}

impl Default for RenderingConfig {
    fn default() -> Self {
        Self {
            mode: RenderMode::TextOnly,
            screenshots: true,
            js_error_reporting: false,
            flatten_shadow_dom: true,
            flatten_iframes: true,
            ajax_timeout_secs: 5.0,
            window_preset: "Googlebot Mobile: Smartphone".into(),
            width: 411,
            height: 731,
            scaling_factor: 1.0,
            mobile: true,
            touch: true,
            resize_to_content: true,
            resize_time_secs: 1.0,
            rotate: false,
            store_rendered_html: false,
            accessibility: false,
            browser_path: None,
        }
    }
}

impl RenderingConfig {
    pub fn javascript(&self) -> bool {
        self.mode == RenderMode::JavaScript
    }
}

/// Spider > Extraction > Structured Data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StructuredDataConfig {
    pub json_ld: bool,
    pub microdata: bool,
    pub rdfa: bool,
    /// Types and properties checked against the schema.org vocabulary.
    pub schema_org_validation: bool,
    /// Google Rich Result Feature Validation.
    pub google_rich_results: bool,
    /// Schema.org names are case-sensitive; off flags wrong case as a warning.
    pub case_sensitive: bool,
}

impl Default for StructuredDataConfig {
    fn default() -> Self {
        Self { json_ld: true, microdata: true, rdfa: false, schema_org_validation: true, google_rich_results: true, case_sensitive: false }
    }
}

impl StructuredDataConfig {
    pub fn any_format(&self) -> bool {
        self.json_ld || self.microdata || self.rdfa
    }
}

/// Content > Spelling & Grammar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpellingConfig {
    pub spell_check: bool,
    pub grammar_check: bool,
    pub language_mode: LanguageMode,
    /// BCP 47 code: the Manual language, and the dialect for a bare page
    /// `lang` in Auto mode (e.g. `en` -> en-GB).
    pub language: String,
    /// LanguageTool server for grammar, e.g. http://localhost:8081. Empty: no grammar check.
    pub grammar_server: String,
    /// Grammar Rules switched off (LanguageTool rule ids).
    pub disabled_grammar_rules: Vec<String>,
    /// Ignore: words skipped in this crawl.
    pub ignore_words: Vec<String>,
    /// Dictionary: the user's global word list (kept outside profiles; set by
    /// the app or CLI at crawl start).
    #[serde(skip)]
    pub dictionary_words: Vec<String>,
    /// Where Hunspell dictionaries are stored and downloaded to.
    #[serde(skip)]
    pub dictionaries_dir: Option<std::path::PathBuf>,
}

impl Default for SpellingConfig {
    fn default() -> Self {
        Self {
            spell_check: false,
            grammar_check: false,
            language_mode: LanguageMode::Auto,
            language: "en-US".into(),
            grammar_server: String::new(),
            disabled_grammar_rules: vec![],
            ignore_words: vec![],
            dictionary_words: vec![],
            dictionaries_dir: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LanguageMode {
    /// Each page's `lang` attribute.
    #[default]
    Auto,
    Manual,
}

/// Custom > Link Positions: one row (name, link-path substring).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkPosition {
    pub name: String,
    pub contains: String,
}

impl LinkPosition {
    /// Content (`/`) matches every path, so it stays last.
    pub fn defaults() -> Vec<Self> {
        [("Head", "/head/"), ("Navigation", "nav"), ("Header", "header"), ("Aside", "aside"), ("Footer", "footer"), ("Content", "/")]
            .iter()
            .map(|(n, c)| Self { name: n.to_string(), contains: c.to_string() })
            .collect()
    }

    /// The first position whose substring the link path contains.
    pub fn classify<'a>(positions: &'a [LinkPosition], link_path: &str) -> Option<&'a str> {
        positions.iter().find(|p| !p.contains.is_empty() && link_path.contains(p.contains.as_str())).map(|p| p.name.as_str())
    }
}

/// Spider > Extraction: which page and URL details are extracted and stored.
/// Indexability is always computed (every tab's filters depend on it).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Extraction {
    pub page_title: bool,
    pub meta_description: bool,
    pub meta_keywords: bool,
    pub h1: bool,
    pub h2: bool,
    pub word_count: bool,
    pub readability: bool,
    pub text_to_code_ratio: bool,
    pub hash: bool,
    pub page_size: bool,
    pub forms: bool,
    pub response_time: bool,
    pub last_modified: bool,
    pub http_headers: bool,
    pub cookies: bool,
    pub meta_robots: bool,
    pub x_robots_tag: bool,
}

impl Default for Extraction {
    fn default() -> Self {
        Self {
            page_title: true,
            meta_description: true,
            meta_keywords: true,
            h1: true,
            h2: true,
            word_count: true,
            readability: true,
            text_to_code_ratio: true,
            hash: true,
            page_size: true,
            forms: true,
            response_time: true,
            last_modified: true,
            http_headers: false,
            cookies: false,
            meta_robots: true,
            x_robots_tag: true,
        }
    }
}

/// Configuration > Custom > Search: count matches of text or a regex.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomSearch {
    pub name: String,
    pub mode: SearchMode,
    pub kind: QueryKind,
    pub query: String,
    /// Text searches only; regex is case-sensitive unless it uses `(?i)`.
    pub case_sensitive: bool,
    pub search_in: SearchIn,
    /// Search In: XPath — the expression whose matches are searched.
    pub xpath: String,
}

impl Default for CustomSearch {
    fn default() -> Self {
        Self {
            name: String::new(),
            mode: SearchMode::Contains,
            kind: QueryKind::Text,
            query: String::new(),
            case_sensitive: false,
            search_in: SearchIn::Html,
            xpath: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    #[default]
    Contains,
    DoesNotContain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QueryKind {
    #[default]
    Text,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchIn {
    #[default]
    Html,
    HtmlHead,
    HtmlBody,
    PageText,
    /// Page text without the text of links.
    PageTextNoAnchors,
    ContentArea,
    /// The HTML of the elements an XPath expression selects.
    Xpath,
}

/// Configuration > Custom > Extraction. XPath is not supported yet; CSS
/// selectors cover most of the same ground.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomExtractor {
    pub name: String,
    pub kind: ExtractorKind,
    pub expression: String,
    /// CSS only: read this attribute instead of the element.
    pub attribute: Option<String>,
    pub extract: ExtractWhat,
}

impl Default for CustomExtractor {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: ExtractorKind::Xpath,
            expression: String::new(),
            attribute: None,
            extract: ExtractWhat::InnerHtml,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExtractorKind {
    #[default]
    Xpath,
    Css,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExtractWhat {
    Element,
    #[default]
    InnerHtml,
    Text,
    /// XPath functions such as count() or string-length(): the result as text.
    FunctionValue,
}

/// Text used for word count, readability and duplicate checks.
/// Default: `<body>` minus `<nav>` and `<footer>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentArea {
    /// Which lists apply: Exclude (default), Include, or Both (Toad's superset).
    pub mode: AreaMode,
    /// Count image alt text as content.
    pub check_alt_text: bool,
    pub include_elements: Vec<String>,
    pub exclude_elements: Vec<String>,
    pub include_classes: Vec<String>,
    pub exclude_classes: Vec<String>,
    pub include_ids: Vec<String>,
    pub exclude_ids: Vec<String>,
}

/// Content > Area > Mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AreaMode {
    #[default]
    Exclude,
    Include,
    /// Apply the include lists and the exclude lists together.
    Both,
}

impl ContentArea {
    /// The lists that apply under the chosen mode.
    pub fn effective(&self) -> ContentArea {
        let mut a = self.clone();
        match self.mode {
            AreaMode::Exclude => {
                a.include_elements.clear();
                a.include_classes.clear();
                a.include_ids.clear();
            }
            AreaMode::Include => {
                a.exclude_elements.clear();
                a.exclude_classes.clear();
                a.exclude_ids.clear();
            }
            AreaMode::Both => {}
        }
        a
    }
}

impl Default for ContentArea {
    fn default() -> Self {
        Self {
            mode: AreaMode::Exclude,
            check_alt_text: false,
            include_elements: vec![],
            exclude_elements: vec!["nav".into(), "footer".into()],
            include_classes: vec![],
            exclude_classes: vec![],
            include_ids: vec![],
            exclude_ids: vec![],
        }
    }
}

/// Thresholds behind the filters (Configuration > Spider > Preferences).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub title_min_chars: u32,
    pub title_max_chars: u32,
    pub title_min_px: u32,
    pub title_max_px: u32,
    pub description_min_chars: u32,
    pub description_max_chars: u32,
    pub description_min_px: u32,
    pub description_max_px: u32,
    pub url_max_chars: u32,
    pub h1_max_chars: u32,
    pub h2_max_chars: u32,
    pub alt_text_max_chars: u32,
    pub image_max_kb: u32,
    pub low_content_words: u32,
    pub high_crawl_depth: u32,
    pub high_internal_outlinks: u32,
    pub high_external_outlinks: u32,
    pub html_max_mb: u32,
    /// Validation > Resource Over X MB (CSS, JavaScript, images and other files).
    pub resource_max_mb: u32,
    pub non_descriptive_anchor_text: Vec<String>,
    pub soft_404_phrases: Vec<String>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            title_min_chars: 30,
            title_max_chars: 60,
            title_min_px: 200,
            title_max_px: 561,
            description_min_chars: 70,
            description_max_chars: 155,
            description_min_px: 400,
            description_max_px: 985,
            url_max_chars: 115,
            h1_max_chars: 70,
            h2_max_chars: 70,
            alt_text_max_chars: 100,
            image_max_kb: 100,
            low_content_words: 200,
            high_crawl_depth: 3,
            high_internal_outlinks: 1000,
            high_external_outlinks: 10,
            html_max_mb: 2,
            resource_max_mb: 2,
            non_descriptive_anchor_text: [
                "click here", "click this", "find out more", "go", "here", "learn more", "more",
                "over here", "read here", "read more", "right here", "start", "this",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            soft_404_phrases: ["Page Not Found", "404 Page Can't Be Found"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_follows_crawl_and_external() {
        use crate::model::LinkKind::*;
        let cfg = CrawlConfig::default();
        assert!(cfg.stores_link(Image, true));
        assert!(!cfg.stores_link(Media, true), "Media is off by default");
        assert!(!cfg.stores_link(Uncrawlable, true));
        assert!(cfg.stores_link(Redirect, false));
        let no_store = CrawlConfig { store_images: false, ..CrawlConfig::default() };
        assert!(no_store.stores_link(Image, true), "Crawl implies Store");
        let neither = CrawlConfig { store_images: false, crawl_images: false, ..CrawlConfig::default() };
        assert!(!neither.stores_link(Image, true));
        let no_external = CrawlConfig { store_external_links: false, crawl_external_links: false, ..CrawlConfig::default() };
        assert!(!no_external.stores_link(Hyperlink, false) && !no_external.stores_link(Image, false));
        assert!(no_external.stores_link(Hyperlink, true));
    }

    #[test]
    fn default_config_is_valid() {
        assert_eq!(CrawlConfig::default().validate(), Vec::<String>::new());
    }

    #[test]
    fn validate_reports_problems() {
        let cfg = CrawlConfig {
            include: vec!["(".into()],
            threads: 0,
            custom_headers: vec![("Bad Header".into(), "x".into())],
            custom_search: vec![CustomSearch { name: "A".into(), query: "x".into(), ..Default::default() }],
            custom_extraction: vec![CustomExtractor { name: "a".into(), expression: "h1".into(), ..Default::default() }],
            ..CrawlConfig::default()
        };
        let problems = cfg.validate();
        assert!(problems.iter().any(|p| p.starts_with("Include:")), "{problems:?}");
        assert!(problems.iter().any(|p| p.starts_with("Speed:")));
        assert!(problems.iter().any(|p| p.starts_with("HTTP Header:")));
        assert!(problems.iter().any(|p| p.contains("used more than once")));
    }

    #[test]
    fn profiles_round_trip_and_tolerate_missing_fields() {
        let dir = std::env::temp_dir().join(format!("toad-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.toadconfig");
        let cfg = CrawlConfig { threads: 9, lowercase_urls: true, ..CrawlConfig::default() };
        cfg.save(&path).unwrap();
        let back = CrawlConfig::load(&path).unwrap();
        assert_eq!((back.threads, back.lowercase_urls), (9, true));
        // Profiles from older versions lack newer fields; they take defaults.
        std::fs::write(&path, r#"{"threads": 3}"#).unwrap();
        let old = CrawlConfig::load(&path).unwrap();
        assert_eq!((old.threads, old.max_url_length), (3, 10_000));
    }
}
