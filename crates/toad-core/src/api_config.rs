//! Configuration > API Access and Content > Embeddings: the non-secret
//! settings of every data provider. Keys, client secrets and OAuth tokens are
//! never stored here (this config is saved into crawl files and profiles);
//! the desktop app keeps them in the OS credential store and the CLI reads
//! them from environment variables.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiConfig {
    pub psi: PsiConfig,
    pub gsc: GscConfig,
    pub ga4: Ga4Config,
    pub ai: AiConfig,
    pub embeddings: EmbeddingsConfig,
    pub ahrefs: AhrefsConfig,
    pub majestic: MajesticConfig,
    pub moz: MozConfig,
}

impl ApiConfig {
    /// Whether page text must be kept for AI prompts or embeddings:
    /// (full page text, content area text).
    pub fn needs_text(&self) -> (bool, bool) {
        let mut wants: Vec<AiContent> = self.ai.prompts.iter().filter(|p| p.enabled).map(|p| p.content).collect();
        if self.embeddings.enabled {
            wants.push(self.embeddings.content);
        }
        (wants.contains(&AiContent::PageText), wants.contains(&AiContent::ContentArea))
    }

    pub fn any_enabled(&self) -> bool {
        self.psi.enabled
            || self.gsc.enabled
            || self.ga4.enabled
            || self.ai.prompts.iter().any(|p| p.enabled)
            || self.embeddings.enabled
            || self.ahrefs.enabled
            || self.majestic.enabled
            || self.moz.enabled
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PsiDevice {
    #[default]
    Mobile,
    Desktop,
}

/// API Access > PageSpeed Insights.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PsiConfig {
    pub enabled: bool,
    pub device: PsiDevice,
    /// Requests in flight at once (PSI allows about 4 per second per key).
    pub max_outstanding: usize,
    /// Metric groups.
    pub summary: bool,
    pub crux: bool,
    pub lighthouse_metrics: bool,
    pub opportunities: bool,
    pub diagnostics: bool,
    /// Only request indexable pages.
    pub indexable_only: bool,
}

impl Default for PsiConfig {
    fn default() -> Self {
        Self { enabled: false, device: PsiDevice::Mobile, max_outstanding: 4, summary: true, crux: true, lighthouse_metrics: true, opportunities: true, diagnostics: true, indexable_only: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GscDevice {
    #[default]
    All,
    Desktop,
    Mobile,
    Tablet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GscSearchType {
    #[default]
    Web,
    Image,
    Video,
    News,
    Discover,
    GoogleNews,
}

impl GscSearchType {
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Image => "image",
            Self::Video => "video",
            Self::News => "news",
            Self::Discover => "discover",
            Self::GoogleNews => "googleNews",
        }
    }
}

/// API Access > Google Search Console.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GscConfig {
    pub enabled: bool,
    /// The Search Console property, e.g. `sc-domain:example.com` or `https://www.example.com/`.
    pub property: String,
    /// Search Analytics date range: the last N days (data lags about 2 days).
    pub days: u32,
    pub device: GscDevice,
    /// ISO 3166-1 alpha-3 country code, empty for all countries.
    pub country: String,
    pub search_type: GscSearchType,
    /// Dimension filter on queries: only rows whose query contains (or, with
    /// `query_exclude`, does not contain) this text. Empty: no filter.
    pub query_filter: String,
    pub query_exclude: bool,
    /// Match Search Console URLs to crawled URLs ignoring a trailing slash / case.
    pub ignore_trailing_slash: bool,
    pub case_sensitive: bool,
    /// Crawl New URLs Discovered in Google Search Console (orphans).
    pub crawl_new_urls: bool,
    /// URL Inspection API.
    pub url_inspection: bool,
    pub inspection_indexable_only: bool,
    /// Requests a day per property (Google allows 2,000).
    pub inspection_daily_limit: u32,
}

impl Default for GscConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            property: String::new(),
            days: 90,
            device: GscDevice::All,
            country: String::new(),
            search_type: GscSearchType::Web,
            query_filter: String::new(),
            query_exclude: false,
            ignore_trailing_slash: true,
            case_sensitive: true,
            crawl_new_urls: false,
            url_inspection: false,
            inspection_indexable_only: true,
            inspection_daily_limit: 2000,
        }
    }
}

/// API Access > Google Analytics 4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ga4Config {
    pub enabled: bool,
    /// The numeric GA4 property ID.
    pub property: String,
    pub days: u32,
    /// GA4 Data API metric names.
    pub metrics: Vec<String>,
    /// Only sessions whose default channel group is this (e.g. "Organic
    /// Search"); empty for all.
    pub channel: String,
    pub ignore_trailing_slash: bool,
    pub case_sensitive: bool,
    /// Cap on report rows.
    pub max_rows: u32,
    /// Crawl New URLs Discovered in Google Analytics (orphans).
    pub crawl_new_urls: bool,
}

impl Default for Ga4Config {
    fn default() -> Self {
        Self {
            enabled: false,
            property: String::new(),
            days: 30,
            metrics: ["sessions", "engagedSessions", "engagementRate", "screenPageViews", "averageSessionDuration", "bounceRate", "keyEvents"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            channel: String::new(),
            ignore_trailing_slash: true,
            case_sensitive: true,
            max_rows: 100_000,
            crawl_new_urls: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    #[default]
    OpenAi,
    Anthropic,
    Gemini,
    Ollama,
    /// Any OpenAI-compatible endpoint (Azure, DeepSeek, Grok, OpenRouter, LM Studio...).
    OpenAiCompatible,
}

impl AiProvider {
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Gemini",
            Self::Ollama => "Ollama",
            Self::OpenAiCompatible => "OpenAI-compatible",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiContent {
    /// The page's visible text.
    #[default]
    PageText,
    /// The Content Area text (Content > Area).
    ContentArea,
    /// The stored HTML (needs Store HTML).
    Html,
    /// Only the URL: the prompt itself says what to do.
    UrlOnly,
}

/// One row of API Access > AI > Prompt Configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiPrompt {
    pub enabled: bool,
    pub name: String,
    pub provider: AiProvider,
    pub model: String,
    /// The instruction. The page content follows it.
    pub prompt: String,
    pub content: AiContent,
}

impl Default for AiPrompt {
    fn default() -> Self {
        Self { enabled: true, name: String::new(), provider: AiProvider::OpenAi, model: "gpt-4o-mini".into(), prompt: String::new(), content: AiContent::PageText }
    }
}

/// API Access > AI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub prompts: Vec<AiPrompt>,
    pub openai_base_url: String,
    pub compatible_base_url: String,
    pub ollama_base_url: String,
    pub temperature: f64,
    pub max_output_tokens: u32,
    /// Content sent is cut to this many characters.
    pub max_input_chars: usize,
    pub requests_per_minute: u32,
    pub indexable_only: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            prompts: vec![],
            openai_base_url: "https://api.openai.com/v1".into(),
            compatible_base_url: String::new(),
            ollama_base_url: "http://localhost:11434".into(),
            temperature: 0.2,
            max_output_tokens: 500,
            max_input_chars: 20_000,
            requests_per_minute: 60,
            indexable_only: true,
        }
    }
}

/// Content > Embeddings: Semantic Similarity and Low Relevance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingsConfig {
    pub enabled: bool,
    pub provider: AiProvider,
    pub model: String,
    pub content: AiContent,
    /// Embedding Filter Rules: only pages whose URL matches this regex (empty: all).
    pub include_regex: String,
    pub semantic_similarity: bool,
    pub similarity_threshold: f64,
    pub similarity_indexable_only: bool,
    pub low_relevance: bool,
    pub low_relevance_threshold: f64,
    pub low_relevance_indexable_only: bool,
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: AiProvider::OpenAi,
            model: "text-embedding-3-small".into(),
            content: AiContent::ContentArea,
            include_regex: String::new(),
            semantic_similarity: true,
            similarity_threshold: 0.95,
            similarity_indexable_only: true,
            low_relevance: true,
            low_relevance_threshold: 0.4,
            low_relevance_indexable_only: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LinkTarget {
    /// The exact URL.
    #[default]
    Url,
    /// Everything under the URL's subdomain.
    Subdomain,
    /// The whole registrable domain.
    RootDomain,
}

/// API Access > Ahrefs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AhrefsConfig {
    pub enabled: bool,
    pub target: LinkTarget,
    /// Organic traffic and keywords (site-explorer/metrics), as well as backlinks.
    pub organic: bool,
}

/// API Access > Majestic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MajesticConfig {
    pub enabled: bool,
    pub target: LinkTarget,
    /// Fresh (90 days) or Historic index.
    pub historic: bool,
}

impl Default for MajesticConfig {
    fn default() -> Self {
        Self { enabled: false, target: LinkTarget::Url, historic: false }
    }
}

/// API Access > Moz.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MozConfig {
    pub enabled: bool,
    pub target: LinkTarget,
}
