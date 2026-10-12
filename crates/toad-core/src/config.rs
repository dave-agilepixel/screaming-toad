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
    pub crawl_internal_links: bool,
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

    // Configuration > Spider > Advanced
    pub ignore_non_indexable_for_issues: bool,
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

    // Configuration > User-Agent / HTTP Header
    pub user_agent: String,
    pub robots_user_agent: String,
    pub custom_headers: Vec<(String, String)>,

    // Configuration > Include / Exclude (partial regex match, case-sensitive)
    pub include: Vec<String>,
    pub exclude: Vec<String>,

    // Configuration > URL Rewriting
    pub remove_parameters: Vec<String>,
    pub lowercase_urls: bool,

    // Configuration > Content > Area
    pub content_area: ContentArea,

    // Configuration > Spider > Extraction > Structured Data (JSON-LD and
    // Microdata, with Google rich result validation). On by default here;
    // Screaming Frog has it off.
    pub extract_structured_data: bool,

    // Configuration > Content > Duplicates
    pub near_duplicates: bool,
    pub near_duplicate_threshold: f64,
    pub near_duplicates_indexable_only: bool,

    // Configuration > Custom > Search / Extraction
    pub custom_search: Vec<CustomSearch>,
    pub custom_extraction: Vec<CustomExtractor>,

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
            crawl_internal_links: true,
            store_external_links: true,
            crawl_external_links: true,
            store_canonicals: true,
            crawl_canonicals: true,
            store_pagination: false,
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
            max_url_length: 2_000,
            max_links_per_url: 10_000,
            max_page_size_kb: 50_000,
            ignore_non_indexable_for_issues: true,
            response_timeout_secs: 20,
            retries_5xx: 0,
            assume_pages_are_html: false,
            threads: 5,
            max_urls_per_sec: None,
            robots_mode: RobotsMode::Respect,
            show_internal_blocked_by_robots: true,
            show_external_blocked_by_robots: false,
            user_agent: format!("Screaming Toad SEO Spider/{}", env!("CARGO_PKG_VERSION")),
            robots_user_agent: "Screaming Toad".to_string(),
            custom_headers: vec![
                (
                    "Accept".into(),
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".into(),
                ),
                ("Cache-Control".into(), "no-cache".into()),
                ("Pragma".into(), "no-cache".into()),
            ],
            include: vec![],
            exclude: vec![],
            remove_parameters: vec![],
            lowercase_urls: false,
            content_area: ContentArea::default(),
            extract_structured_data: true,
            near_duplicates: false,
            near_duplicate_threshold: 0.90,
            near_duplicates_indexable_only: true,
            custom_search: vec![],
            custom_extraction: vec![],
            preferences: Preferences::default(),
        }
    }
}

impl CrawlConfig {
    /// List mode forces crawl depth 0 and ignores robots.txt, as Screaming Frog does.
    pub fn for_list_mode(mut self) -> Self {
        self.mode = Mode::List;
        self.max_depth = Some(0);
        self.robots_mode = RobotsMode::Ignore;
        self
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
    ContentArea,
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
            kind: ExtractorKind::Css,
            expression: String::new(),
            attribute: None,
            extract: ExtractWhat::Text,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExtractorKind {
    #[default]
    Css,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExtractWhat {
    Element,
    InnerHtml,
    #[default]
    Text,
}

/// Text used for word count, readability and duplicate checks.
/// Default: `<body>` minus `<nav>` and `<footer>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentArea {
    pub include_elements: Vec<String>,
    pub exclude_elements: Vec<String>,
    pub include_classes: Vec<String>,
    pub exclude_classes: Vec<String>,
    pub include_ids: Vec<String>,
    pub exclude_ids: Vec<String>,
}

impl Default for ContentArea {
    fn default() -> Self {
        Self {
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
            non_descriptive_anchor_text: [
                "click here", "click this", "find out more", "go", "here", "learn more", "more",
                "over here", "read here", "read more", "right here", "start", "this",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            soft_404_phrases: ["page not found", "404 error", "not found"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}
