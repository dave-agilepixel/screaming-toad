//! Configuration > API Access: fetching PageSpeed Insights, Search Console,
//! GA4, AI prompt answers, embeddings and link metrics into a crawl.
//!
//! Screaming Frog requests API data while it crawls. Toad splits it in two:
//! - Bulk sources (Search Console Search Analytics, GA4) are fetched before
//!   the crawl, so URLs only they know about can be crawled (orphans).
//! - Per-URL sources (PSI, URL Inspection, AI, embeddings, link metrics) run
//!   when the crawl ends, over the crawled pages, or later with Request API
//!   Data. Pages that already have a source's data are skipped.

pub mod ai;
pub mod google;
pub mod keystore;
pub mod links;
pub mod notify;
pub mod psi;
mod throttle;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use toad_core::config::CrawlConfig;
use toad_store::api::{self as store_api, ApiValue};
use toad_store::Store;
pub use reqwest;
pub use throttle::Throttle;

/// Access to the crawl store: a plain `Mutex<Store>` (CLI, tests) or the
/// desktop app's `Mutex<Option<Store>>` (the open crawl, which the UI keeps
/// querying while API data arrives).
pub trait StoreLock: Sync {
    fn lock_store(&self) -> anyhow::Result<StoreGuard<'_>>;
}

pub enum StoreGuard<'a> {
    Plain(MutexGuard<'a, Store>),
    Open(MutexGuard<'a, Option<Store>>),
}

impl std::ops::Deref for StoreGuard<'_> {
    type Target = Store;
    fn deref(&self) -> &Store {
        match self {
            Self::Plain(g) => g,
            Self::Open(g) => g.as_ref().expect("checked when locked"),
        }
    }
}

impl std::ops::DerefMut for StoreGuard<'_> {
    fn deref_mut(&mut self) -> &mut Store {
        match self {
            Self::Plain(g) => g,
            Self::Open(g) => g.as_mut().expect("checked when locked"),
        }
    }
}

impl StoreLock for Mutex<Store> {
    fn lock_store(&self) -> anyhow::Result<StoreGuard<'_>> {
        Ok(StoreGuard::Plain(self.lock().map_err(|_| anyhow::anyhow!("store lock"))?))
    }
}

impl StoreLock for Mutex<Option<Store>> {
    fn lock_store(&self) -> anyhow::Result<StoreGuard<'_>> {
        let g = self.lock().map_err(|_| anyhow::anyhow!("store lock"))?;
        anyhow::ensure!(g.is_some(), "the crawl was closed");
        Ok(StoreGuard::Open(g))
    }
}

/// Names of secrets (OS credential store entries / environment variables).
pub mod keys {
    pub const PSI: &str = "psi_key";
    pub const GOOGLE_CLIENT_ID: &str = "google_client_id";
    pub const GOOGLE_CLIENT_SECRET: &str = "google_client_secret";
    pub const GOOGLE_REFRESH_TOKEN: &str = "google_refresh_token";
    pub const OPENAI: &str = "openai_key";
    pub const ANTHROPIC: &str = "anthropic_key";
    pub const GEMINI: &str = "gemini_key";
    pub const COMPATIBLE: &str = "compatible_key";
    pub const AHREFS: &str = "ahrefs_key";
    pub const MAJESTIC: &str = "majestic_key";
    /// Moz: "access_id:secret_key".
    pub const MOZ: &str = "moz_key";
    /// Notifications: the SMTP account's password.
    pub const SMTP: &str = "smtp_password";
    pub const ALL: &[&str] =
        &[PSI, GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, GOOGLE_REFRESH_TOKEN, OPENAI, ANTHROPIC, GEMINI, COMPATIBLE, AHREFS, MAJESTIC, MOZ, SMTP];
}

/// API keys and tokens, held in memory only.
#[derive(Debug, Clone, Default)]
pub struct Secrets(HashMap<String, String>);

impl Secrets {
    pub fn new(map: HashMap<String, String>) -> Self {
        Self(map.into_iter().filter(|(_, v)| !v.trim().is_empty()).collect())
    }

    /// The CLI: TOAD_PSI_KEY, TOAD_OPENAI_KEY, TOAD_GOOGLE_REFRESH_TOKEN...
    pub fn from_env() -> Self {
        Self::new(keys::ALL.iter().filter_map(|k| std::env::var(format!("TOAD_{}", k.to_uppercase())).ok().map(|v| (k.to_string(), v))).collect())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn require(&self, key: &str, what: &str) -> anyhow::Result<&str> {
        self.get(key).ok_or_else(|| anyhow::anyhow!("{what}: no key set. Add it in Configuration > API Access (CLI: TOAD_{}).", key.to_uppercase()))
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.0.insert(key.to_string(), value.to_string());
    }
}

/// Provider base URLs. The defaults are the real services; tests point
/// them at local mock servers.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub psi: String,
    pub google_auth: String,
    pub google_token: String,
    pub gsc: String,
    pub inspection: String,
    pub ga4_admin: String,
    pub ga4_data: String,
    pub anthropic: String,
    pub gemini: String,
    pub ahrefs: String,
    pub majestic: String,
    pub moz: String,
    /// Overrides the configured OpenAI / OpenAI-compatible / Ollama base URLs.
    pub openai: Option<String>,
    pub ollama: Option<String>,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            psi: "https://www.googleapis.com/pagespeedonline/v5/runPagespeed".into(),
            google_auth: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            google_token: "https://oauth2.googleapis.com/token".into(),
            gsc: "https://www.googleapis.com/webmasters/v3".into(),
            inspection: "https://searchconsole.googleapis.com/v1/urlInspection/index:inspect".into(),
            ga4_admin: "https://analyticsadmin.googleapis.com/v1beta".into(),
            ga4_data: "https://analyticsdata.googleapis.com/v1beta".into(),
            anthropic: "https://api.anthropic.com/v1".into(),
            gemini: "https://generativelanguage.googleapis.com/v1beta".into(),
            ahrefs: "https://api.ahrefs.com/v3".into(),
            majestic: "https://api.majestic.com/api/json".into(),
            moz: "https://lsapi.seomoz.com/v2".into(),
            openai: None,
            ollama: None,
        }
    }
}

/// Progress of one source, for the status bar.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiProgress {
    pub source: String,
    pub done: u64,
    pub total: u64,
}

pub type ProgressFn = Arc<dyn Fn(ApiProgress) + Send + Sync>;

pub struct ApiRunner {
    pub cfg: CrawlConfig,
    pub secrets: Secrets,
    pub endpoints: Endpoints,
    pub client: reqwest::Client,
    progress: Option<ProgressFn>,
    google: tokio::sync::Mutex<Option<google::AccessToken>>,
    /// The crawl's start URL: GA4 rows are limited to its site and scheme.
    site: Option<url::Url>,
}

/// What the bulk sources found before the crawl.
#[derive(Debug, Default)]
pub struct BulkData {
    pub values: Vec<(&'static str, Vec<ApiValue>)>,
    /// URLs from Search Console / GA4 to crawl too (Crawl New URLs Discovered).
    pub new_urls: Vec<url::Url>,
    pub errors: Vec<String>,
}

impl ApiRunner {
    pub fn new(cfg: CrawlConfig, secrets: Secrets) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .user_agent(format!("Screaming Toad/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self { cfg, secrets, endpoints: Endpoints::default(), client, progress: None, google: tokio::sync::Mutex::new(None), site: None }
    }

    pub fn with_site(mut self, start: url::Url) -> Self {
        self.site = Some(start);
        self
    }

    pub fn with_endpoints(mut self, endpoints: Endpoints) -> Self {
        self.endpoints = endpoints;
        self
    }

    pub fn with_progress(mut self, f: ProgressFn) -> Self {
        self.progress = Some(f);
        self
    }

    fn report(&self, source: &str, done: u64, total: u64) {
        if let Some(f) = &self.progress {
            f(ApiProgress { source: source.into(), done, total });
        }
    }

    /// Search Console Search Analytics and GA4, before (or without) a crawl.
    pub async fn fetch_bulk(&self) -> BulkData {
        let mut out = BulkData::default();
        let apis = &self.cfg.apis;
        if apis.gsc.enabled {
            match google::search_analytics(self).await {
                Ok(values) => {
                    if apis.gsc.crawl_new_urls {
                        out.new_urls.extend(values.iter().filter(|v| v.metric == "clicks").filter_map(|v| url::Url::parse(&v.url).ok()));
                    }
                    out.values.push((store_api::GSC, values));
                }
                Err(e) => out.errors.push(format!("Search Console: {e:#}")),
            }
        }
        if apis.ga4.enabled {
            match google::ga4_report(self).await {
                Ok(mut values) => {
                    if let Some(site) = &self.site {
                        // GA4 properties can span several sites: keep this one's
                        // host and its subdomains, with the crawl's scheme.
                        let host = site.host_str().unwrap_or("").to_string();
                        let root = host.strip_prefix("www.").unwrap_or(&host).to_string();
                        values.retain(|v| {
                            url::Url::parse(&v.url).ok().and_then(|u| u.host_str().map(str::to_string)).is_some_and(|h| h == root || h.ends_with(&format!(".{root}")))
                        });
                        google::rescheme(&mut values, site.scheme());
                    }
                    if apis.ga4.crawl_new_urls {
                        let first = apis.ga4.metrics.first().cloned().unwrap_or_default();
                        out.new_urls.extend(values.iter().filter(|v| v.metric == first).filter_map(|v| url::Url::parse(&v.url).ok()));
                    }
                    out.values.push((store_api::GA4, values));
                }
                Err(e) => out.errors.push(format!("GA4: {e:#}")),
            }
        }
        out.new_urls.sort();
        out.new_urls.dedup();
        out
    }

    /// Save bulk data into a crawl (before or after crawling).
    pub fn store_bulk(&self, store: &dyn StoreLock, bulk: &BulkData) -> anyhow::Result<()> {
        let mut s = store.lock_store()?;
        for (source, values) in &bulk.values {
            s.put_api_values(source, values)?;
        }
        Ok(())
    }

    /// Per-URL sources over the crawled pages, then URL matching and
    /// embeddings analysis. Returns the errors met (requests that failed
    /// are also recorded per URL).
    pub async fn fetch_per_url(&self, store: &dyn StoreLock) -> Vec<String> {
        let mut errors = Vec::new();
        let apis = &self.cfg.apis;
        if apis.psi.enabled {
            if let Err(e) = psi::run(self, store).await {
                errors.push(format!("PageSpeed Insights: {e:#}"));
            }
        }
        if apis.gsc.enabled && apis.gsc.url_inspection {
            if let Err(e) = google::run_inspection(self, store).await {
                errors.push(format!("URL Inspection: {e:#}"));
            }
        }
        if apis.ai.prompts.iter().any(|p| p.enabled) {
            if let Err(e) = ai::run_prompts(self, store).await {
                errors.push(format!("AI: {e:#}"));
            }
        }
        if apis.embeddings.enabled {
            if let Err(e) = ai::run_embeddings(self, store).await {
                errors.push(format!("Embeddings: {e:#}"));
            }
        }
        if apis.ahrefs.enabled || apis.majestic.enabled || apis.moz.enabled {
            errors.extend(links::run(self, store).await);
        }
        if let Err(e) = self.finish(store) {
            errors.push(format!("{e:#}"));
        }
        errors
    }

    /// Match provider URLs to crawled URLs and analyse embeddings.
    pub fn finish(&self, store: &dyn StoreLock) -> anyhow::Result<()> {
        let mut s = store.lock_store()?;
        let (gsc, ga) = (&self.cfg.apis.gsc, &self.cfg.apis.ga4);
        s.resolve_api_urls(store_api::GSC, gsc.ignore_trailing_slash, gsc.case_sensitive)?;
        s.resolve_api_urls(store_api::GA4, ga.ignore_trailing_slash, ga.case_sensitive)?;
        if self.cfg.apis.embeddings.enabled {
            s.analyse_embeddings()?;
        }
        Ok(())
    }

    /// Request API Data for a finished crawl: bulk sources, then per-URL.
    pub async fn request_all(&self, store: &dyn StoreLock) -> Vec<String> {
        let bulk = self.fetch_bulk().await;
        let mut errors = bulk.errors.clone();
        if let Err(e) = self.store_bulk(store, &bulk) {
            errors.push(format!("{e:#}"));
        }
        errors.extend(self.fetch_per_url(store).await);
        errors
    }
}

/// Send a request, retrying 429 and 5xx responses with backoff, and return
/// the JSON body. Error responses come back as errors with Google-style
/// `error.message` text when present.
pub(crate) async fn send_json(build: impl Fn() -> reqwest::RequestBuilder) -> anyhow::Result<serde_json::Value> {
    let mut wait = std::time::Duration::from_secs(2);
    for attempt in 0..4 {
        let resp = build().send().await?;
        let status = resp.status();
        if (status.as_u16() == 429 || status.is_server_error()) && attempt < 3 {
            let retry_after = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
            tokio::time::sleep(retry_after.map(std::time::Duration::from_secs).unwrap_or(wait).min(std::time::Duration::from_secs(60))).await;
            wait *= 2;
            continue;
        }
        let text = resp.text().await?;
        if !status.is_success() {
            let msg = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| {
                    v.pointer("/error/message")
                        .or_else(|| v.pointer("/error"))
                        .or_else(|| v.pointer("/message"))
                        .map(|m| m.as_str().map(str::to_string).unwrap_or_else(|| m.to_string()))
                })
                .unwrap_or_else(|| text.chars().take(300).collect());
            anyhow::bail!("HTTP {status}: {msg}");
        }
        return serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("unexpected response ({e}): {}", text.chars().take(200).collect::<String>()));
    }
    anyhow::bail!("gave up after retries")
}

/// Write values for one source, plus an error row per failed URL.
pub(crate) fn save(store: &dyn StoreLock, source: &str, values: &[ApiValue], failed: &[(String, String)]) -> anyhow::Result<()> {
    let mut s = store.lock_store()?;
    if !values.is_empty() {
        s.put_api_values(source, values)?;
    }
    if !failed.is_empty() {
        let errs: Vec<ApiValue> = failed.iter().map(|(u, e)| ApiValue::new(u.clone(), source, e)).collect();
        s.put_api_values(store_api::ERROR, &errs)?;
    }
    Ok(())
}

/// Run `f` over `urls` with the throttle, saving results every 25 URLs.
pub(crate) async fn for_each_url<F, Fut>(runner: &ApiRunner, store: &dyn StoreLock, source: &'static str, urls: Vec<String>, throttle: Throttle, f: F) -> anyhow::Result<()>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Vec<ApiValue>>>,
{
    use futures::StreamExt;
    let total = urls.len() as u64;
    let throttle = Arc::new(throttle);
    let mut done = 0u64;
    let mut values = Vec::new();
    let mut failed = Vec::new();
    let mut stream = futures::stream::iter(urls.into_iter().map(|u| {
        let t = throttle.clone();
        let fut = f(u.clone());
        async move {
            let _permit = t.acquire().await;
            (u, fut.await)
        }
    }))
    .buffer_unordered(throttle.concurrency());
    runner.report(source, 0, total);
    while let Some((u, r)) = stream.next().await {
        done += 1;
        match r {
            Ok(v) => values.extend(v),
            Err(e) => failed.push((u, format!("{e:#}"))),
        }
        if done % 25 == 0 || done == total {
            save(store, source, &std::mem::take(&mut values), &std::mem::take(&mut failed))?;
            runner.report(source, done, total);
        }
    }
    Ok(())
}
