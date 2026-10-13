//! Spider > Rendering > JavaScript: pages are rendered in a headless
//! Chrome-family browser (Chrome, Edge or Chromium) over the DevTools
//! protocol, as Screaming Frog renders in its embedded Chromium.
//!
//! One browser is launched per crawl; at most `threads` tabs render at once.
//! Subresources (scripts, styles, images) are fetched by the browser itself,
//! so robots.txt and Exclude are not applied to them; a page's blocked
//! resources are reported by checking each requested URL against robots.txt
//! afterwards (JavaScript > Pages With Blocked Resources).

use crate::config::{CrawlConfig, CustomJs, CustomJsKind, RenderingConfig};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::{SetDeviceMetricsOverrideParams, SetTouchEmulationEnabledParams};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::cdp::js_protocol::runtime::{EventConsoleApiCalled, EventExceptionThrown};
use chromiumoxide::page::ScreenshotParams;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;
use url::Url;

/// One Chrome console message (Enable JavaScript Error Reporting).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsoleMessage {
    /// "error", "warning", "info", "log"...
    pub level: String,
    pub message: String,
    /// "console" or "exception".
    pub source: String,
}

/// One axe-core rule a page fails (Spider > Extraction > Accessibility).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct A11yViolation {
    pub rule: String,
    /// "critical", "serious", "moderate" or "minor".
    pub impact: String,
    pub tags: Vec<String>,
    pub help: String,
    pub help_url: String,
    pub nodes: Vec<A11yNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct A11yNode {
    /// CSS selector of the failing element.
    pub target: String,
    pub html: String,
}

impl A11yViolation {
    /// The guideline Screaming Frog groups the rule under: the earliest WCAG
    /// version and level in its tags, else Best Practice.
    pub fn guideline(&self) -> &'static str {
        const ORDER: [(&str, &str); 7] = [
            ("wcag2a", "WCAG 2.0 A"),
            ("wcag2aa", "WCAG 2.0 AA"),
            ("wcag2aaa", "WCAG 2.0 AAA"),
            ("wcag21a", "WCAG 2.1 A"),
            ("wcag21aa", "WCAG 2.1 AA"),
            ("wcag22aa", "WCAG 2.2 AA"),
            ("best-practice", "Best Practice"),
        ];
        ORDER.iter().find(|(tag, _)| self.tags.iter().any(|t| t == tag)).map_or("Other", |(_, name)| name)
    }
}

/// axe-core (MPL-2.0, Deque Systems), refreshed by `scripts/fetch_axe.sh`.
const AXE_JS: &str = include_str!("../data/axe.min.js");

const AXE_RUN: &str = r#"(async () => {
  const r = await axe.run(document, { resultTypes: ['violations'] });
  return r.violations.map(v => ({
    rule: v.id,
    impact: v.impact || '',
    tags: v.tags,
    help: v.help,
    help_url: v.helpUrl,
    nodes: v.nodes.slice(0, 100).map(n => ({ target: n.target.flat(Infinity).join(' '), html: (n.html || '').slice(0, 1000) })),
  }));
})()"#;

/// One Custom JavaScript snippet's output on a page, in configuration order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CustomJsResult {
    pub values: Vec<String>,
    pub error: Option<String>,
}

/// The `seoSpider` API given to Custom JavaScript snippets.
const SEO_SPIDER_SHIM: &str = r#"(async () => {
  const __out = { data: null, error: null, urls: [], texts: [] };
  const seoSpider = {
    data: (d) => { __out.data = d; return __out; },
    error: (e) => { __out.error = String(e); return __out; },
    saveText: (text, file, append) => { __out.texts.push({ text: String(text), file: String(file), append: !!append }); },
    saveUrls: (urls) => { [].concat(urls).forEach(u => __out.urls.push(String(u))); },
    loadScript: (src) => new Promise((resolve, reject) => {
      const s = document.createElement('script');
      s.src = src; s.onload = resolve; s.onerror = () => reject(new Error('could not load ' + src));
      document.head.appendChild(s);
    }),
  };
  try {
    const r = await (async () => {
/*SNIPPET*/
    })();
    if (r !== undefined && r !== __out && __out.data === null && __out.error === null) __out.data = r;
  } catch (e) {
    __out.error = String((e && e.message) || e);
  }
  try { __out.data = JSON.parse(JSON.stringify(__out.data)); } catch (e) { __out.data = String(__out.data); }
  return __out;
})()"#;

#[derive(Deserialize, Default)]
struct ShimOut {
    data: serde_json::Value,
    error: Option<String>,
    urls: Vec<String>,
    texts: Vec<SavedText>,
}

#[derive(Deserialize)]
struct SavedText {
    text: String,
    file: String,
    append: bool,
}

/// seoSpider.data(...) as column values: an array gives one value per item.
fn js_values(v: serde_json::Value) -> Vec<String> {
    let one = |v: serde_json::Value| match v {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    };
    match v {
        serde_json::Value::Null => vec![],
        serde_json::Value::Array(items) => items.into_iter().filter(|i| !i.is_null()).map(one).collect(),
        other => vec![one(other)],
    }
}

/// seoSpider.saveText: append or write a local file. Serialised so
/// concurrent pages appending to one file don't interleave.
fn save_text(t: &SavedText) -> std::io::Result<()> {
    use std::io::Write;
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = std::fs::OpenOptions::new().create(true).write(true).append(t.append).truncate(!t.append).open(&t.file)?;
    writeln!(f, "{}", t.text)
}

/// What rendering a page produced.
#[derive(Debug, Clone, Default)]
pub struct Rendered {
    pub html: String,
    pub console: Vec<ConsoleMessage>,
    pub screenshot: Option<Vec<u8>>,
    /// Every URL the page requested while rendering (resources, XHR).
    pub requests: Vec<String>,
    /// Bytes transferred for the page and its resources (Performance API).
    pub transferred_bytes: u64,
    /// axe-core violations, when Accessibility is on.
    pub accessibility: Option<Vec<A11yViolation>>,
    /// Custom JavaScript results, one per configured snippet.
    pub custom_js: Vec<CustomJsResult>,
    /// URLs passed to seoSpider.saveUrls, to be crawled.
    pub saved_urls: Vec<String>,
}

/// Browsers to try when no path is configured.
pub fn candidate_browsers() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    #[cfg(windows)]
    {
        for base in [std::env::var_os("ProgramFiles"), std::env::var_os("ProgramFiles(x86)"), std::env::var_os("LOCALAPPDATA")].into_iter().flatten() {
            let base = PathBuf::from(base);
            v.push(base.join(r"Google\Chrome\Application\chrome.exe"));
            v.push(base.join(r"Microsoft\Edge\Application\msedge.exe"));
            v.push(base.join(r"Chromium\Application\chrome.exe"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        v.push("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into());
        v.push("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".into());
        v.push("/Applications/Chromium.app/Contents/MacOS/Chromium".into());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for p in ["/usr/bin/google-chrome", "/usr/bin/google-chrome-stable", "/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/microsoft-edge", "/snap/bin/chromium"] {
            v.push(p.into());
        }
    }
    v
}

/// The browser that will be used: the configured path, or the first one found.
pub fn find_browser(cfg: &RenderingConfig) -> Option<PathBuf> {
    match cfg.browser_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => Some(PathBuf::from(p)).filter(|p| p.exists()),
        None => candidate_browsers().into_iter().find(|p| p.exists()),
    }
}

pub struct Renderer {
    browser: Browser,
    handler: tokio::task::JoinHandle<()>,
    slots: Semaphore,
    cfg: RenderingConfig,
    user_agent: String,
    profile_dir: PathBuf,
    custom_js: Vec<CustomJs>,
    network: Arc<crate::auth::Network>,
}

/// Flattens shadow roots (Flatten Shadow DOM) and same-origin iframes
/// (Flatten iframes) into the light DOM, then reports transfer size.
const SNAPSHOT_JS: &str = r#"(function(flattenShadow, flattenFrames) {
  if (flattenShadow) {
    const walk = (root) => {
      root.querySelectorAll('*').forEach(el => {
        if (el.shadowRoot) {
          walk(el.shadowRoot);
          const holder = document.createElement('div');
          holder.setAttribute('data-toad-shadow-root', '');
          holder.innerHTML = el.shadowRoot.innerHTML;
          el.appendChild(holder);
        }
      });
    };
    walk(document);
  }
  if (flattenFrames) {
    document.querySelectorAll('iframe').forEach(f => {
      try {
        const body = f.contentDocument && f.contentDocument.body;
        if (body) {
          const holder = document.createElement('div');
          holder.setAttribute('data-toad-iframe', f.src || '');
          holder.innerHTML = body.innerHTML;
          f.replaceWith(holder);
        }
      } catch (e) { /* cross-origin: left as an iframe */ }
    });
  }
  let bytes = 0;
  const nav = performance.getEntriesByType('navigation')[0];
  if (nav) bytes += nav.transferSize || nav.encodedBodySize || 0;
  const urls = [];
  performance.getEntriesByType('resource').forEach(r => { bytes += r.transferSize || r.encodedBodySize || 0; urls.push(r.name); });
  return { bytes, urls };
})"#;

impl Renderer {
    pub async fn launch(cfg: &CrawlConfig, network: Arc<crate::auth::Network>) -> anyhow::Result<Self> {
        let r = &cfg.rendering;
        let exe = find_browser(r).ok_or_else(|| {
            anyhow::anyhow!("JavaScript rendering needs Chrome, Edge or Chromium. None was found; set its path in Configuration > Spider > Rendering.")
        })?;
        // A private profile per crawl, so crawls don't share cookies or caches.
        let profile_dir = std::env::temp_dir().join(format!("toad-render-{}-{}", std::process::id(), unique_suffix()));
        let (w, h) = window(r);
        let mut builder = BrowserConfig::builder();
        // File > Settings > Proxy. Proxy credentials are answered per page.
        if let Some(proxy) = network.settings.proxy.url() {
            builder = builder.arg(format!("--proxy-server={}", proxy.replace("socks5h://", "socks5://")));
            let bypass: Vec<&str> = network.settings.proxy.no_proxy.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if !bypass.is_empty() {
                builder = builder.arg(format!("--proxy-bypass-list={}", bypass.join(";")));
            }
        }
        let config = builder
            .chrome_executable(&exe)
            .new_headless_mode()
            .user_data_dir(&profile_dir)
            .window_size(w, h)
            .request_timeout(Duration::from_secs(cfg.response_timeout_secs.max(10) + r.ajax_timeout_secs.ceil() as u64))
            .arg("--disable-gpu")
            .arg("--mute-audio")
            .arg("--no-first-run")
            .arg("--disable-extensions")
            .build()
            .map_err(|e| anyhow::anyhow!("browser configuration: {e}"))?;
        let (browser, mut handler) = Browser::launch(config).await.map_err(|e| anyhow::anyhow!("could not start {}: {e}", exe.display()))?;
        let handler = tokio::spawn(async move { while handler.next().await.is_some() {} });
        Ok(Self {
            browser,
            handler,
            slots: Semaphore::new(cfg.threads.max(1)),
            cfg: r.clone(),
            user_agent: cfg.user_agent.clone(),
            profile_dir,
            custom_js: cfg.custom_javascript.iter().filter(|s| !s.code.trim().is_empty()).cloned().collect(),
            network,
        })
    }

    /// Render one URL: load, wait the AJAX timeout, flatten, snapshot.
    pub async fn render(&self, url: &Url, content_type: &str) -> anyhow::Result<Rendered> {
        let _slot = self.slots.acquire().await?;
        let page = self.browser.new_page("about:blank").await?;
        let result = self.render_on(&page, url, content_type).await;
        let _ = page.close().await;
        result
    }

    /// Configuration > Authentication in the browser: forms-based login
    /// cookies, and Basic/Digest (or proxy) credentials for challenges.
    async fn authenticate(&self, page: &chromiumoxide::Page, url: &Url) -> anyhow::Result<()> {
        use chromiumoxide::cdp::browser_protocol::network::{CookieParam, SetCookiesParams};
        let auth = self.network.auth();
        let cookies: Vec<CookieParam> = auth
            .forms
            .iter()
            .flat_map(|f| f.cookies.iter())
            .filter_map(|c| {
                let mut b = CookieParam::builder().name(&c.name).value(&c.value).path(if c.path.is_empty() { "/" } else { c.path.as_str() }).secure(c.secure).http_only(c.http_only);
                b = if c.host_only {
                    let scheme = if c.secure { "https" } else { "http" };
                    b.url(format!("{scheme}://{}{}", c.domain.trim_start_matches('.'), if c.path.is_empty() { "/" } else { c.path.as_str() }))
                } else {
                    b.domain(&c.domain)
                };
                b.build().ok()
            })
            .collect();
        if !cookies.is_empty() {
            page.execute(SetCookiesParams::new(cookies)).await?;
        }
        let site = auth.standards.iter().find(|c| c.matches(url)).map(|c| (c.username.clone(), c.password.clone()));
        let proxy = &self.network.settings.proxy;
        let proxy = (proxy.enabled && !proxy.username.is_empty()).then(|| (proxy.username.clone(), proxy.password.clone()));
        if let Some((username, password)) = site.or(proxy) {
            page.authenticate(chromiumoxide::auth::Credentials { username, password }).await?;
        }
        Ok(())
    }

    /// Custom JavaScript: Actions first, then Extractions, each with its own timeout.
    async fn run_custom_js(&self, page: &chromiumoxide::Page, content_type: &str) -> (Vec<CustomJsResult>, Vec<String>) {
        let mut results = vec![CustomJsResult::default(); self.custom_js.len()];
        let mut urls = Vec::new();
        let order = self.custom_js.iter().enumerate().filter(|(_, s)| s.kind == CustomJsKind::Action).chain(self.custom_js.iter().enumerate().filter(|(_, s)| s.kind == CustomJsKind::Extraction));
        for (i, snippet) in order {
            if !snippet.applies_to(content_type) {
                continue;
            }
            let script = SEO_SPIDER_SHIM.replace("/*SNIPPET*/", &snippet.code);
            let timeout = Duration::from_secs_f64(snippet.timeout_secs.max(0.1));
            let out: anyhow::Result<ShimOut> = match tokio::time::timeout(timeout, page.evaluate_expression(script)).await {
                Err(_) => Err(anyhow::anyhow!("timed out after {}s", snippet.timeout_secs)),
                Ok(r) => r.map_err(anyhow::Error::from).and_then(|v| v.into_value().map_err(anyhow::Error::from)),
            };
            match out {
                Ok(out) => {
                    for t in &out.texts {
                        if let Err(e) = save_text(t) {
                            results[i].error = Some(format!("saveText {}: {e}", t.file));
                        }
                    }
                    urls.extend(out.urls);
                    if snippet.kind == CustomJsKind::Extraction {
                        results[i].values = js_values(out.data);
                    }
                    if out.error.is_some() {
                        results[i].error = out.error;
                    }
                }
                Err(e) => results[i].error = Some(e.to_string()),
            }
        }
        (results, urls)
    }

    async fn render_on(&self, page: &chromiumoxide::Page, url: &Url, content_type: &str) -> anyhow::Result<Rendered> {
        let r = &self.cfg;
        let (w, h) = window(r);
        page.set_user_agent(self.user_agent.as_str()).await?;
        page.execute(SetDeviceMetricsOverrideParams::new(w as i64, h as i64, r.scaling_factor, r.mobile)).await?;
        if r.touch {
            page.execute(SetTouchEmulationEnabledParams::new(true)).await?;
        }

        let console: Arc<Mutex<Vec<ConsoleMessage>>> = Arc::default();
        let mut listeners = Vec::new();
        if r.js_error_reporting {
            let mut calls = page.event_listener::<EventConsoleApiCalled>().await?;
            let sink = console.clone();
            listeners.push(tokio::spawn(async move {
                while let Some(ev) = calls.next().await {
                    let text = ev
                        .args
                        .iter()
                        .map(|a| a.value.as_ref().map(|v| v.to_string().trim_matches('"').to_string()).or_else(|| a.description.clone()).unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let level = format!("{:?}", ev.r#type).to_lowercase();
                    if let Ok(mut c) = sink.lock() {
                        c.push(ConsoleMessage { level, message: text, source: "console".into() });
                    }
                }
            }));
            let mut errors = page.event_listener::<EventExceptionThrown>().await?;
            let sink = console.clone();
            listeners.push(tokio::spawn(async move {
                while let Some(ev) = errors.next().await {
                    let d = &ev.exception_details;
                    let text = d.exception.as_ref().and_then(|e| e.description.clone()).unwrap_or_else(|| d.text.clone());
                    if let Ok(mut c) = sink.lock() {
                        c.push(ConsoleMessage { level: "error".into(), message: text, source: "exception".into() });
                    }
                }
            }));
        }

        self.authenticate(page, url).await?;
        page.goto(url.as_str()).await?;
        if r.ajax_timeout_secs > 0.0 {
            tokio::time::sleep(Duration::from_secs_f64(r.ajax_timeout_secs)).await;
        }
        let (custom_js, saved_urls) = self.run_custom_js(page, content_type).await;
        // Accessibility runs on the live DOM, before flattening changes it.
        let accessibility = if r.accessibility {
            page.evaluate_expression(AXE_JS).await?;
            let v: Vec<A11yViolation> = page.evaluate_expression(AXE_RUN).await?.into_value()?;
            Some(v)
        } else {
            None
        };
        #[derive(Deserialize, Default)]
        struct Snapshot {
            bytes: f64,
            urls: Vec<String>,
        }
        let snap: Snapshot = page
            .evaluate(format!("{SNAPSHOT_JS}({}, {})", r.flatten_shadow_dom, r.flatten_iframes))
            .await
            .ok()
            .and_then(|v| v.into_value().ok())
            .unwrap_or_default();
        let html = page.content().await?;

        let screenshot = if r.screenshots {
            if r.resize_to_content && r.resize_time_secs > 0.0 {
                tokio::time::sleep(Duration::from_secs_f64(r.resize_time_secs)).await;
            }
            let params = ScreenshotParams::builder().format(CaptureScreenshotFormat::Png).full_page(r.resize_to_content).build();
            page.screenshot(params).await.ok()
        } else {
            None
        };
        for l in listeners {
            l.abort();
        }
        let console = console.lock().map(|c| c.clone()).unwrap_or_default();
        Ok(Rendered { html, console, screenshot, requests: snap.urls, transferred_bytes: snap.bytes.max(0.0) as u64, accessibility, custom_js, saved_urls })
    }

    pub async fn close(mut self) {
        let _ = self.browser.close().await;
        let _ = self.browser.wait().await;
        self.handler.abort();
        let _ = std::fs::remove_dir_all(&self.profile_dir);
    }
}

/// Configuration > Authentication > Forms Based: a visible browser window
/// the user logs in with. Its cookies are captured when they finish.
pub struct LoginWindow {
    browser: Browser,
    handler: tokio::task::JoinHandle<()>,
    profile_dir: PathBuf,
}

impl LoginWindow {
    pub async fn open(rendering: &RenderingConfig, network: &crate::auth::Network, url: &Url) -> anyhow::Result<Self> {
        let exe = find_browser(rendering).ok_or_else(|| {
            anyhow::anyhow!("Forms-based login needs Chrome, Edge or Chromium. None was found; set its path in Configuration > Spider > Rendering.")
        })?;
        let profile_dir = std::env::temp_dir().join(format!("toad-login-{}-{}", std::process::id(), unique_suffix()));
        let mut builder = BrowserConfig::builder()
            .chrome_executable(&exe)
            .with_head()
            .user_data_dir(&profile_dir)
            .window_size(1100, 850)
            .arg("--no-first-run")
            .arg("--disable-extensions");
        if let Some(proxy) = network.settings.proxy.url() {
            builder = builder.arg(format!("--proxy-server={}", proxy.replace("socks5h://", "socks5://")));
        }
        let config = builder.build().map_err(|e| anyhow::anyhow!("browser configuration: {e}"))?;
        let (browser, mut handler) = Browser::launch(config).await.map_err(|e| anyhow::anyhow!("could not start {}: {e}", exe.display()))?;
        let handler = tokio::spawn(async move { while handler.next().await.is_some() {} });
        let page = browser.new_page("about:blank").await?;
        let proxy = &network.settings.proxy;
        if proxy.enabled && !proxy.username.is_empty() {
            page.authenticate(chromiumoxide::auth::Credentials { username: proxy.username.clone(), password: proxy.password.clone() }).await?;
        }
        page.goto(url.as_str()).await?;
        Ok(Self { browser, handler, profile_dir })
    }

    /// Every cookie the browser holds now.
    pub async fn cookies(&self) -> anyhow::Result<Vec<crate::auth::AuthCookie>> {
        Ok(self
            .browser
            .get_cookies()
            .await?
            .into_iter()
            .map(|c| crate::auth::AuthCookie {
                host_only: !c.domain.starts_with('.'),
                name: c.name,
                value: c.value,
                domain: c.domain,
                path: c.path,
                secure: c.secure,
                http_only: c.http_only,
            })
            .collect())
    }

    pub async fn close(mut self) {
        let _ = self.browser.close().await;
        let _ = self.browser.wait().await;
        self.handler.abort();
        let _ = std::fs::remove_dir_all(&self.profile_dir);
    }
}

/// A suffix unique within this process (profile folder names).
fn unique_suffix() -> String {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("{nanos}-{}", N.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
}

/// Width and height after Rotate.
fn window(r: &RenderingConfig) -> (u32, u32) {
    if r.rotate { (r.height, r.width) } else { (r.width, r.height) }
}

/// CO2 per page view (grams) from bytes transferred, using the Sustainable
/// Web Design model's headline figures (0.81 kWh/GB, 442 g CO2e/kWh).
pub fn co2_grams(bytes: u64) -> f64 {
    let gb = bytes as f64 / 1_000_000_000.0;
    ((gb * 0.81 * 442.0) * 1000.0).round() / 1000.0
}

/// Sustainable Web Design digital carbon rating for grams per page view.
pub fn carbon_rating(grams: f64) -> &'static str {
    match grams {
        g if g <= 0.040 => "A+",
        g if g <= 0.079 => "A",
        g if g <= 0.145 => "B",
        g if g <= 0.209 => "C",
        g if g <= 0.278 => "D",
        g if g <= 0.359 => "E",
        _ => "F",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carbon_model() {
        assert_eq!(co2_grams(0), 0.0);
        let g = co2_grams(2_000_000);
        assert!((g - 0.716).abs() < 0.001, "{g}");
        assert_eq!(carbon_rating(0.03), "A+");
        assert_eq!(carbon_rating(0.2), "C");
        assert_eq!(carbon_rating(g), "F");
    }

    #[test]
    fn custom_js_values() {
        assert_eq!(js_values(serde_json::json!(["a", 2, null])), vec!["a", "2"]);
        assert_eq!(js_values(serde_json::json!("x")), vec!["x"]);
        assert_eq!(js_values(serde_json::json!({"k": 1})), vec![r#"{"k":1}"#]);
        assert!(js_values(serde_json::Value::Null).is_empty());
        let s = CustomJs { content_types: "text/html, application/xhtml".into(), ..Default::default() };
        assert!(s.applies_to("text/html; charset=utf-8"));
        assert!(!s.applies_to("application/pdf"));
    }

    #[test]
    fn rotate_swaps_the_window() {
        let r = RenderingConfig { rotate: true, ..RenderingConfig::default() };
        assert_eq!(window(&r), (731, 411));
    }
}
