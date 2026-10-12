//! Records the crawler produces for the store.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkKind {
    Hyperlink,
    Image,
    Css,
    JavaScript,
    Canonical,
    HttpCanonical,
    Hreflang,
    RelNext,
    RelPrev,
    Iframe,
    MetaRefresh,
    Redirect,
}

impl LinkKind {
    pub const ALL: [LinkKind; 12] = [
        LinkKind::Hyperlink,
        LinkKind::Image,
        LinkKind::Css,
        LinkKind::JavaScript,
        LinkKind::Canonical,
        LinkKind::HttpCanonical,
        LinkKind::Hreflang,
        LinkKind::RelNext,
        LinkKind::RelPrev,
        LinkKind::Iframe,
        LinkKind::MetaRefresh,
        LinkKind::Redirect,
    ];

    /// The inverse of [`LinkKind::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LinkKind::Hyperlink => "Hyperlink",
            LinkKind::Image => "Image",
            LinkKind::Css => "CSS",
            LinkKind::JavaScript => "JavaScript",
            LinkKind::Canonical => "Canonical",
            LinkKind::HttpCanonical => "HTTP Canonical",
            LinkKind::Hreflang => "Hreflang",
            LinkKind::RelNext => "Rel Next",
            LinkKind::RelPrev => "Rel Prev",
            LinkKind::Iframe => "Iframe",
            LinkKind::MetaRefresh => "Meta Refresh",
            LinkKind::Redirect => "Redirect",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub kind: LinkKind,
    /// Resolved, normalised target URL.
    pub to: String,
    pub href: String,
    pub anchor: String,
    /// None when the attribute is absent (different from an empty alt).
    pub alt: Option<String>,
    pub follow: bool,
    pub rel: String,
    pub target: String,
    pub path_type: String,
    pub link_path: String,
    pub position: String,
    pub hreflang: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageData {
    pub titles: Vec<String>,
    pub title_outside_head: bool,
    pub meta_descriptions: Vec<String>,
    pub description_outside_head: bool,
    pub meta_keywords: Vec<String>,
    pub h1: Vec<String>,
    pub h2: Vec<String>,
    pub h1_has_alt_text: bool,
    pub h1_non_sequential: bool,
    pub h2_non_sequential: bool,
    pub meta_robots: Vec<String>,
    pub robots_outside_head: bool,
    pub canonicals: Vec<String>,
    pub canonical_outside_head: bool,
    pub canonical_is_relative: bool,
    pub canonical_has_fragment: bool,
    pub canonical_invalid_attribute: bool,
    pub rel_next: Vec<String>,
    pub rel_prev: Vec<String>,
    pub hreflang: Vec<(String, String)>,
    pub hreflang_outside_head: bool,
    pub meta_refresh: Option<String>,
    pub meta_refresh_url: Option<String>,
    pub language: Option<String>,
    pub has_viewport: bool,
    pub word_count: u32,
    pub sentence_count: u32,
    pub flesch_reading_ease: Option<f64>,
    pub text_ratio: f64,
    pub content_text: String,
    pub form_actions: Vec<String>,
    pub head_tags: u32,
    pub body_tags: u32,
    pub lorem_ipsum: bool,
    pub links: Vec<Link>,
    /// Match count per Custom Search, in configuration order.
    pub custom_search: Vec<u32>,
    /// Values per Custom Extractor, in configuration order.
    pub custom_extraction: Vec<Vec<String>>,
    /// MinHash signature of the content area (empty unless near duplicates are on).
    pub minhash: Vec<u32>,
    pub structured_data: Option<crate::structured::StructuredData>,
}

/// One fetched (or blocked, or failed) URL.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageRecord {
    pub url: String,
    pub depth: u32,
    pub is_internal: bool,
    pub status_code: u16,
    pub status: String,
    pub content_type: String,
    pub is_html: bool,
    pub size: u64,
    pub transferred: u64,
    pub response_time_ms: u64,
    pub http_version: String,
    pub last_modified: Option<String>,
    pub redirect_url: Option<String>,
    pub redirect_type: Option<String>,
    pub headers: Vec<(String, String)>,
    pub x_robots_tag: Vec<String>,
    pub http_canonical: Option<String>,
    pub hash: Option<String>,
    pub blocked_by_robots_line: Option<String>,
    pub error: Option<String>,
    pub page: Option<PageData>,
    /// For XML sitemap files: the number of `<loc>` entries.
    pub sitemap_url_count: Option<u64>,
    pub indexable: bool,
    pub indexability_status: String,
    /// The page's HTML, when Store HTML is on (internal HTML pages only).
    #[serde(skip)]
    pub html: Option<String>,
}

impl PageRecord {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Indexable / Non-Indexable and the reason, as shown in every tab.
    pub fn compute_indexability(&mut self) {
        let reason = if self.blocked_by_robots_line.is_some() {
            Some("Blocked by Robots.txt")
        } else if self.status_code == 0 {
            Some("No Response")
        } else if (300..400).contains(&self.status_code) {
            Some("Redirected")
        } else if (400..500).contains(&self.status_code) {
            Some("Client Error")
        } else if self.status_code >= 500 {
            Some("Server Error")
        } else if self.status_code != 200 {
            Some("Non-200")
        } else if self.is_noindex() {
            Some("noindex")
        } else if self.is_canonicalised() {
            Some("Canonicalised")
        } else {
            None
        };
        self.indexable = reason.is_none();
        self.indexability_status = reason.unwrap_or("").to_string();
    }

    pub fn directives(&self) -> impl Iterator<Item = String> + '_ {
        let meta = self.page.iter().flat_map(|p| p.meta_robots.iter());
        meta.chain(self.x_robots_tag.iter())
            .flat_map(|v| v.split(',').map(|d| d.trim().to_ascii_lowercase()).collect::<Vec<_>>())
    }

    pub fn is_noindex(&self) -> bool {
        self.directives().any(|d| d == "noindex" || d == "none")
    }

    pub fn is_nofollow(&self) -> bool {
        self.directives().any(|d| d == "nofollow" || d == "none")
    }

    pub fn canonical(&self) -> Option<&str> {
        self.page
            .as_ref()
            .and_then(|p| p.canonicals.first())
            .map(|s| s.as_str())
            .or(self.http_canonical.as_deref())
    }

    pub fn is_canonicalised(&self) -> bool {
        self.canonical().is_some_and(|c| c != self.url)
    }
}

/// Progress reported while crawling.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub crawled: u64,
    pub queued: u64,
    pub discovered: u64,
    pub elapsed_ms: u64,
}
