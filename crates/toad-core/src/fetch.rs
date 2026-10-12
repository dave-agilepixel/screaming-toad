//! HTTP fetching. Redirects are not followed automatically: each hop is
//! recorded as its own URL, as Screaming Frog does.

use crate::config::{CookieStorage, CrawlConfig};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::time::{Duration, Instant};
use url::Url;

#[derive(Debug, Default)]
pub struct FetchResult {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub truncated: bool,
    pub elapsed_ms: u64,
    pub http_version: String,
    pub transferred: u64,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct Fetcher {
    client: reqwest::Client,
    max_bytes: u64,
    retries_5xx: u32,
}

impl Fetcher {
    pub fn new(cfg: &CrawlConfig) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();
        for (k, v) in &cfg.custom_headers {
            if k.eq_ignore_ascii_case("user-agent") {
                continue;
            }
            headers.insert(HeaderName::from_bytes(k.as_bytes())?, HeaderValue::from_str(v)?);
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            // Persistent: one jar shared by every request in the crawl.
            .cookie_store(cfg.cookie_storage == CookieStorage::Persistent)
            .timeout(Duration::from_secs(cfg.response_timeout_secs))
            .connect_timeout(Duration::from_secs(cfg.response_timeout_secs))
            .user_agent(cfg.user_agent.clone())
            .default_headers(headers)
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .pool_max_idle_per_host(cfg.threads.max(1))
            .build()?;
        Ok(Self { client, max_bytes: cfg.max_page_size_kb * 1024, retries_5xx: cfg.retries_5xx })
    }

    pub async fn fetch(&self, url: &Url) -> FetchResult {
        self.fetch_with_cookies(url, None).await
    }

    /// Fetch, sending `cookie` as the Cookie header (Session Only cookies
    /// carried along a redirect chain).
    pub async fn fetch_with_cookies(&self, url: &Url, cookie: Option<&str>) -> FetchResult {
        let mut attempt = 0;
        loop {
            let res = self.fetch_once(url, cookie).await;
            if res.status >= 500 && attempt < self.retries_5xx {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
                continue;
            }
            return res;
        }
    }

    async fn fetch_once(&self, url: &Url, cookie: Option<&str>) -> FetchResult {
        let start = Instant::now();
        let mut req = self.client.get(url.clone());
        if let Some(c) = cookie.filter(|c| !c.is_empty()) {
            req = req.header(reqwest::header::COOKIE, c);
        }
        let mut resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                return FetchResult {
                    status_text: no_response_reason(&e),
                    error: Some(e.to_string()),
                    elapsed_ms: start.elapsed().as_millis() as u64,
                    ..Default::default()
                }
            }
        };
        let status = resp.status();
        let headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned()))
            .collect();
        let content_length = resp.content_length();
        let http_version = format!("{:?}", resp.version());

        let mut body = Vec::new();
        let mut truncated = false;
        let mut error = None;
        loop {
            match resp.chunk().await {
                Ok(Some(chunk)) => {
                    if body.len() as u64 + chunk.len() as u64 > self.max_bytes {
                        truncated = true;
                        break;
                    }
                    body.extend_from_slice(&chunk);
                }
                Ok(None) => break,
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            }
        }
        FetchResult {
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or("").to_string(),
            transferred: content_length.unwrap_or(body.len() as u64),
            headers,
            body,
            truncated,
            elapsed_ms: start.elapsed().as_millis() as u64,
            http_version,
            error,
        }
    }
}

fn no_response_reason(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "Timeout".into()
    } else if e.is_connect() {
        "Connection Refused".into()
    } else {
        "Connection Error".into()
    }
}
