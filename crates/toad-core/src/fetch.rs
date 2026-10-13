//! HTTP fetching. Redirects are not followed automatically: each hop is
//! recorded as its own URL, as Screaming Frog does.

use crate::auth::Network;
use crate::config::{CookieStorage, CrawlConfig};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::sync::Arc;
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
    network: Arc<Network>,
}

impl Fetcher {
    pub fn new(cfg: &CrawlConfig) -> anyhow::Result<Self> {
        Self::with_network(cfg, Arc::new(Network::default()))
    }

    /// A fetcher using the proxy, trusted certificates and authentication in
    /// `network`.
    pub fn with_network(cfg: &CrawlConfig, network: Arc<Network>) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();
        for (k, v) in &cfg.custom_headers {
            if k.eq_ignore_ascii_case("user-agent") {
                continue;
            }
            headers.insert(HeaderName::from_bytes(k.as_bytes())?, HeaderValue::from_str(v)?);
        }
        let mut builder = reqwest::Client::builder();
        let settings = &network.settings;
        if let Some(proxy_url) = settings.proxy.url() {
            let mut proxy = reqwest::Proxy::all(&proxy_url)?;
            if !settings.proxy.username.is_empty() {
                proxy = proxy.basic_auth(&settings.proxy.username, &settings.proxy.password);
            }
            let bypass: Vec<&str> = settings.proxy.no_proxy.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if !bypass.is_empty() {
                proxy = proxy.no_proxy(reqwest::NoProxy::from_string(&bypass.join(",")));
            }
            builder = builder.proxy(proxy);
        }
        for path in &settings.trusted_certificates {
            builder = builder.add_root_certificate(load_certificate(path)?);
        }
        let client = builder
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
        Ok(Self { client, max_bytes: cfg.max_page_size_kb * 1024, retries_5xx: cfg.retries_5xx, network })
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

    /// One request, answering up to two authentication challenges (the
    /// second covers a stale Digest nonce).
    async fn send(&self, url: &Url, cookie: Option<&str>) -> reqwest::Result<reqwest::Response> {
        // Forms-based login cookies go with the crawl's own cookies.
        let forms = self.network.cookie_header(url);
        let cookie = match (cookie.filter(|c| !c.is_empty()), forms) {
            (Some(a), Some(b)) => Some(format!("{a}; {b}")),
            (a, b) => a.map(str::to_string).or(b),
        };
        let mut authorization = self.network.preemptive(url, "GET");
        let mut attempts = 0;
        loop {
            let mut req = self.client.get(url.clone());
            if let Some(c) = &cookie {
                req = req.header(reqwest::header::COOKIE, c);
            }
            if let Some(a) = &authorization {
                req = req.header(reqwest::header::AUTHORIZATION, a);
            }
            let resp = req.send().await?;
            if resp.status() != reqwest::StatusCode::UNAUTHORIZED || attempts >= 2 {
                return Ok(resp);
            }
            let challenges: Vec<String> =
                resp.headers().get_all(reqwest::header::WWW_AUTHENTICATE).iter().map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned()).collect();
            match self.network.respond(url, "GET", &challenges).await {
                Some(a) if Some(&a) != authorization.as_ref() || attempts == 0 => authorization = Some(a),
                _ => return Ok(resp),
            }
            attempts += 1;
        }
    }

    async fn fetch_once(&self, url: &Url, cookie: Option<&str>) -> FetchResult {
        let start = Instant::now();
        let mut resp = match self.send(url, cookie).await {
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

/// A trusted certificate file: PEM (one or more certificates) or DER.
pub fn load_certificate(path: &str) -> anyhow::Result<reqwest::Certificate> {
    let bytes = std::fs::read(path).map_err(|e| anyhow::anyhow!("trusted certificate {path}: {e}"))?;
    let cert = if bytes.starts_with(b"-----BEGIN") || String::from_utf8_lossy(&bytes).contains("-----BEGIN CERTIFICATE") {
        reqwest::Certificate::from_pem(&bytes)
    } else {
        reqwest::Certificate::from_der(&bytes)
    };
    cert.map_err(|e| anyhow::anyhow!("trusted certificate {path} is not a PEM or DER certificate: {e}"))
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
