//! The crawl loop: a breadth-first frontier, robots.txt, scope rules,
//! limits and a fixed number of concurrent fetches.

use crate::config::{CookieStorage, CrawlConfig, RobotsMode};
use crate::custom::CustomRules;
use crate::extract::extract;
use crate::fetch::Fetcher;
use crate::model::{Link, LinkKind, PageRecord, Progress};
use crate::robots::{Robots, Verdict};
use crate::sitemap::{self, SitemapKind};
use crate::urlutil::{list_input_allowed, folder_depth, normalize, query_param_count, Scope};
use regex::Regex;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use url::Url;

pub enum CrawlEvent {
    Page(Box<PageRecord>),
    /// The page URLs listed in one XML sitemap.
    Sitemap { sitemap: String, urls: Vec<String> },
    Progress(Progress),
    Finished(Progress),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchKind {
    /// Fetch and, if HTML, parse and follow links.
    Page,
    /// Fetch to record the response only (resources, external URLs,
    /// links outside the start folder).
    CheckOnly,
}

#[derive(Debug, Clone)]
struct QueueItem {
    url: Url,
    depth: u32,
    kind: FetchKind,
    redirect_hops: u32,
    /// Session Only cookies carried along a redirect chain ("a=1; b=2").
    cookies: Option<String>,
    /// "Ignore robots.txt but report status": the line that would block it.
    robots_line: Option<String>,
}

#[derive(Default)]
struct LimitCounters {
    per_host: HashMap<String, u64>,
    per_depth: HashMap<u32, u64>,
    per_path: Vec<u64>,
}

/// Lets a UI pause or stop a running crawl.
#[derive(Clone, Default)]
pub struct CrawlControl {
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    /// Configuration > Exclude edited while the crawl runs.
    exclude: Arc<std::sync::RwLock<Option<Vec<Regex>>>>,
}

impl CrawlControl {
    /// Replace the Exclude list of a running crawl. URLs already queued are
    /// checked again when they are about to be requested.
    pub fn set_exclude(&self, patterns: &[String]) -> anyhow::Result<()> {
        let compiled = patterns.iter().map(|p| Regex::new(p)).collect::<Result<Vec<_>, _>>()?;
        if let Ok(mut w) = self.exclude.write() {
            *w = Some(compiled);
        }
        Ok(())
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }
    fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
}

pub struct Crawler {
    cfg: Arc<CrawlConfig>,
    fetcher: Fetcher,
    scope: Scope,
    include: Vec<Regex>,
    exclude: Vec<Regex>,
    robots: HashMap<String, Arc<Robots>>,
    rules: Arc<CustomRules>,
    /// Proxy, trusted certificates and authentication.
    network: Arc<crate::auth::Network>,
    /// Rendering: JavaScript (launched when the crawl starts).
    renderer: Option<Arc<crate::render::Renderer>>,
    /// Content > Spelling & Grammar (dictionaries shared by every page).
    language: Arc<crate::spelling::LanguageChecker>,
    /// Exclude as edited during the crawl (shared with CrawlControl).
    live_exclude: Arc<std::sync::RwLock<Option<Vec<Regex>>>>,
    /// Limit by URL Path, compiled.
    path_limits: Vec<(Regex, u64)>,
    /// Hosts that sent Strict-Transport-Security (Respect HSTS Policy).
    hsts: Arc<std::sync::Mutex<HashSet<String>>>,
}

/// A link recorded in an earlier crawl session whose target was never
/// crawled (input to [`Crawler::resume`]).
#[derive(Debug, Clone)]
pub struct PendingLink {
    pub link: Link,
    /// Crawl depth of the page the link was found on.
    pub source_depth: u32,
    /// The source page had a nofollow directive.
    pub source_nofollow: bool,
}

/// Upper bound on sitemap files fetched from indexes, to stop runaway nesting.
const MAX_SITEMAP_FILES: usize = 1_000;

impl Crawler {
    pub fn new(cfg: CrawlConfig, start: &Url) -> anyhow::Result<Self> {
        let compile = |v: &[String]| v.iter().map(|p| Regex::new(p)).collect::<Result<Vec<_>, _>>();
        Ok(Self {
            fetcher: Fetcher::new(&cfg)?,
            scope: Scope::new(start, &cfg),
            include: compile(&cfg.include)?,
            exclude: compile(&cfg.exclude)?,
            robots: HashMap::new(),
            rules: Arc::new(CustomRules::compile(&cfg)?),
            hsts: Arc::default(),
            live_exclude: Arc::default(),
            language: Arc::new(crate::spelling::LanguageChecker::new(&cfg)),
            renderer: None,
            network: Arc::new(crate::auth::Network::default()),
            path_limits: cfg
                .path_limits
                .iter()
                .filter(|l| !l.pattern.is_empty())
                .map(|l| Ok((Regex::new(&l.pattern)?, l.max)))
                .collect::<Result<Vec<_>, regex::Error>>()?,
            cfg: Arc::new(cfg),
        })
    }

    /// Use a proxy, trusted certificates and authentication
    /// (File > Settings and Configuration > Authentication).
    pub fn with_network(mut self, network: Arc<crate::auth::Network>) -> anyhow::Result<Self> {
        self.fetcher = Fetcher::with_network(&self.cfg, network.clone())?;
        self.network = network;
        Ok(self)
    }

    pub async fn run(mut self, seeds: Vec<Url>, tx: mpsc::Sender<CrawlEvent>, control: CrawlControl) -> anyhow::Result<Progress> {
        self.live_exclude = control.exclude.clone();
        self.start_renderer().await?;
        let started = Instant::now();
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<QueueItem> = VecDeque::new();
        for mut url in seeds {
            // URL Rewriting applies to discovered URLs only; start and List
            // URLs are crawled as given (minus any fragment).
            url.set_fragment(None);
            if !list_input_allowed(&self.cfg, &url, &self.include, &self.exclude) {
                continue;
            }
            if seen.insert(url.to_string()) {
                queue.push_back(QueueItem { url, depth: 0, kind: FetchKind::Page, redirect_hops: 0, cookies: None, robots_line: None });
            }
        }

        let mut progress = Progress::default();
        if self.cfg.crawl_linked_sitemaps {
            if let Some(start) = queue.front().map(|i| i.url.clone()) {
                self.crawl_sitemaps(&start, &tx, &mut seen, &mut queue, &mut progress).await;
            }
        }

        self.crawl_loop(seen, queue, progress, 0, started, tx, control).await
    }

    /// Continue a crawl that was stopped or interrupted. `crawled` are the
    /// URLs already in the crawl file; `pending` are links found from them
    /// whose targets were never crawled. Each link goes through the same
    /// follow rules as a fresh crawl, so a changed configuration applies.
    pub async fn resume(
        mut self,
        crawled: Vec<String>,
        pending: Vec<PendingLink>,
        tx: mpsc::Sender<CrawlEvent>,
        control: CrawlControl,
    ) -> anyhow::Result<Progress> {
        self.live_exclude = control.exclude.clone();
        self.start_renderer().await?;
        let started = Instant::now();
        let already = crawled.len() as u64;
        let mut seen: HashSet<String> = crawled.into_iter().collect();
        let mut queue: VecDeque<QueueItem> = VecDeque::new();
        let mut progress = Progress { crawled: already, ..Default::default() };
        let mut pending = pending;
        // Shallowest first, as a breadth-first crawl would have reached them.
        pending.sort_by_key(|p| p.source_depth);
        for p in pending {
            let depth = p.source_depth + 1;
            if self.cfg.max_depth.is_some_and(|max| depth > max) {
                continue;
            }
            let item = if p.link.kind == LinkKind::Redirect {
                let Ok(target) = Url::parse(&p.link.to) else { continue };
                let kind = if self.scope.is_internal(&target) { FetchKind::Page } else { FetchKind::CheckOnly };
                self.allowed(&target, kind == FetchKind::Page).then_some(QueueItem { url: target, depth, kind, redirect_hops: 1, cookies: None, robots_line: None })
            } else {
                let nofollow = p.source_nofollow && !self.cfg.follow_internal_nofollow;
                self.link_item(&p.link, depth, nofollow)
            };
            if let Some(item) = item {
                if seen.insert(item.url.to_string()) {
                    progress.discovered += 1;
                    queue.push_back(item);
                }
            }
        }
        self.crawl_loop(seen, queue, progress, already, started, tx, control).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn crawl_loop(
        mut self,
        mut seen: HashSet<String>,
        mut queue: VecDeque<QueueItem>,
        mut progress: Progress,
        already_issued: u64,
        started: Instant,
        tx: mpsc::Sender<CrawlEvent>,
        control: CrawlControl,
    ) -> anyhow::Result<Progress> {
        let mut tasks: JoinSet<(QueueItem, PageRecord)> = JoinSet::new();
        let mut issued: u64 = already_issued;
        let mut limits = LimitCounters::default();
        let mut ticker = self
            .cfg
            .max_urls_per_sec
            .filter(|r| *r > 0.0)
            .map(|r| tokio::time::interval(Duration::from_secs_f64(1.0 / r)));

        loop {
            if control.is_stopped() {
                tasks.abort_all();
                break;
            }
            while !control.is_paused() && tasks.len() < self.cfg.threads.max(1) && issued < self.cfg.max_urls {
                let Some(mut item) = queue.pop_front() else { break };

                if let Some(record) = self.robots_block(&mut item).await {
                    progress.discovered += 1;
                    let show = if record.is_internal {
                        self.cfg.show_internal_blocked_by_robots
                    } else {
                        self.cfg.show_external_blocked_by_robots
                    };
                    if show {
                        tx.send(CrawlEvent::Page(Box::new(record))).await.ok();
                    }
                    continue;
                }

                // Exclude may have been edited since the URL was queued.
                if item.depth > 0 && self.is_excluded(item.url.as_str()) {
                    continue;
                }
                if !self.within_limits(&item, &mut limits) {
                    continue;
                }
                if let Some(t) = ticker.as_mut() {
                    t.tick().await;
                }
                issued += 1;
                let fetcher = self.fetcher.clone();
                let cfg = self.cfg.clone();
                let rules = self.rules.clone();
                let hsts = self.hsts.clone();
                let language = self.language.clone();
                let renderer = self.renderer.clone();
                // List mode audits every uploaded URL fully, whatever its host.
                let is_internal = self.scope.is_internal(&item.url) || (self.cfg.mode == crate::config::Mode::List && item.depth == 0);
                tasks.spawn(async move {
                    let record = process(&fetcher, &cfg, &rules, &item, is_internal, &hsts, &language, renderer.as_deref()).await;
                    (item, record)
                });
            }

            if tasks.is_empty() {
                if queue.is_empty() || issued >= self.cfg.max_urls {
                    break;
                }
                // Paused with nothing in flight.
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }

            let Some(joined) = tasks.join_next().await else { continue };
            let (item, record) = match joined {
                Ok(r) => r,
                Err(_) => continue,
            };

            let mut record = record;
            if self.cfg.robots_mode == RobotsMode::Respect {
                if let Some(info) = record.rendered.as_mut() {
                    let requests = std::mem::take(&mut info.requests);
                    for u in requests.iter().filter_map(|u| Url::parse(u).ok()) {
                        let robots = self.robots_for(&u.origin().ascii_serialization()).await;
                        let path = match u.query() {
                            Some(q) => format!("{}?{q}", u.path()),
                            None => u.path().to_string(),
                        };
                        if matches!(robots.check(&self.cfg.robots_user_agent, &path), Verdict::Blocked(_)) {
                            info.blocked_resources.push(u.to_string());
                        }
                    }
                }
            }
            if item.kind == FetchKind::Page && record.is_internal {
                for next in self.discover(&item, &record) {
                    if seen.insert(next.url.to_string()) {
                        progress.discovered += 1;
                        queue.push_back(next);
                    }
                }
            }

            progress.crawled += 1;
            progress.queued = queue.len() as u64 + tasks.len() as u64;
            progress.elapsed_ms = started.elapsed().as_millis() as u64;
            if tx.send(CrawlEvent::Page(Box::new(record))).await.is_err() {
                break;
            }
            if progress.crawled % 10 == 0 {
                tx.send(CrawlEvent::Progress(progress.clone())).await.ok();
            }
        }

        progress.queued = queue.len() as u64;
        progress.elapsed_ms = started.elapsed().as_millis() as u64;
        tx.send(CrawlEvent::Finished(progress.clone())).await.ok();
        if let Some(r) = self.renderer.take().and_then(|r| Arc::try_unwrap(r).ok()) {
            r.close().await;
        }
        Ok(progress)
    }

    /// Rendering: JavaScript needs a browser for the whole crawl.
    async fn start_renderer(&mut self) -> anyhow::Result<()> {
        if self.cfg.rendering.javascript() && self.renderer.is_none() {
            self.renderer = Some(Arc::new(crate::render::Renderer::launch(&self.cfg, self.network.clone()).await?));
        }
        Ok(())
    }

    /// robots.txt for an origin, fetched once and cached.
    async fn robots_for(&mut self, origin: &str) -> Arc<Robots> {
        if let Some(r) = self.robots.get(origin) {
            return r.clone();
        }
        // A custom entry can name the host alone, or host:port.
        let parsed = Url::parse(origin).ok();
        let host = parsed.as_ref().and_then(|u| u.host_str()).unwrap_or("").to_ascii_lowercase();
        let authority = match parsed.as_ref().and_then(|u| u.port()) {
            Some(p) => format!("{host}:{p}"),
            None => host.clone(),
        };
        if let Some(custom) = self.cfg.custom_robots.iter().find(|c| c.enabled && (c.host_name() == host || c.host_name() == authority)) {
            let robots = Arc::new(Robots::parse(&custom.txt));
            self.robots.insert(origin.to_string(), robots.clone());
            return robots;
        }
        let robots = match Url::parse(&format!("{origin}/robots.txt")) {
            // Exclude applies to robots.txt fetches too.
            Ok(u) if self.is_excluded(u.as_str()) => Robots::allow_all(),
            Ok(u) => {
                let res = self.fetcher.fetch(&u).await;
                if res.status == 200 {
                    Robots::parse(&String::from_utf8_lossy(&res.body))
                } else {
                    Robots::allow_all()
                }
            }
            Err(_) => Robots::allow_all(),
        };
        let robots = Arc::new(robots);
        self.robots.insert(origin.to_string(), robots.clone());
        robots
    }

    /// Returns a "Blocked by Robots.txt" record if robots.txt disallows the
    /// URL. With "Ignore but report status" the URL is crawled, and the line
    /// that would have blocked it is noted on the item instead.
    async fn robots_block(&mut self, item: &mut QueueItem) -> Option<PageRecord> {
        if self.cfg.robots_mode == RobotsMode::Ignore {
            return None;
        }
        let origin = item.url.origin().ascii_serialization();
        let robots = self.robots_for(&origin).await;
        let path = match item.url.query() {
            Some(q) => format!("{}?{}", item.url.path(), q),
            None => item.url.path().to_string(),
        };
        match robots.check(&self.cfg.robots_user_agent, &path) {
            Verdict::Allowed => None,
            Verdict::Blocked(line) if self.cfg.robots_mode == RobotsMode::IgnoreButReport => {
                item.robots_line = Some(line);
                None
            }
            Verdict::Blocked(line) => {
                let mut r = PageRecord {
                    url: item.url.to_string(),
                    depth: item.depth,
                    is_internal: self.scope.is_internal(&item.url),
                    status: "Blocked by Robots.txt".into(),
                    blocked_by_robots_line: Some(line),
                    ..Default::default()
                };
                r.compute_indexability();
                Some(r)
            }
        }
    }

    /// Configuration > Spider > Crawl > Crawl Linked XML Sitemaps: fetch the
    /// sitemaps (from robots.txt and/or the configured list), follow
    /// indexes, record each sitemap file, and queue the page URLs they list.
    async fn crawl_sitemaps(
        &mut self,
        start: &Url,
        tx: &mpsc::Sender<CrawlEvent>,
        seen: &mut HashSet<String>,
        queue: &mut VecDeque<QueueItem>,
        progress: &mut Progress,
    ) {
        let mut pending: VecDeque<String> = self.cfg.sitemap_urls.iter().cloned().collect();
        if self.cfg.auto_discover_sitemaps_via_robots {
            let robots = self.robots_for(&start.origin().ascii_serialization()).await;
            pending.extend(robots.sitemaps.iter().cloned());
        }
        let mut fetched: HashSet<String> = HashSet::new();
        while let Some(sitemap_url) = pending.pop_front() {
            if fetched.len() >= MAX_SITEMAP_FILES || !fetched.insert(sitemap_url.clone()) {
                continue;
            }
            let Ok(url) = Url::parse(&sitemap_url) else { continue };
            let res = self.fetcher.fetch(&url).await;
            let parsed = (res.status == 200).then(|| sitemap::parse(&res.body));

            let mut record = PageRecord {
                url: url.to_string(),
                depth: 0,
                is_internal: self.scope.is_internal(&url),
                status_code: res.status,
                status: res.status_text.clone(),
                size: res.body.len() as u64,
                transferred: res.transferred,
                response_time_ms: res.elapsed_ms,
                http_version: res.http_version.clone(),
                error: res.error.clone(),
                sitemap_url_count: parsed.as_ref().map(|p| p.locs.len() as u64),
                ..Default::default()
            };
            record.headers = res.headers;
            record.content_type = record.header("content-type").unwrap_or("application/xml").to_string();
            record.compute_indexability();
            seen.insert(record.url.clone());
            progress.crawled += 1;
            tx.send(CrawlEvent::Page(Box::new(record))).await.ok();

            let Some(parsed) = parsed else { continue };
            match parsed.kind {
                SitemapKind::Index => pending.extend(parsed.locs),
                SitemapKind::UrlSet => {
                    for loc in &parsed.locs {
                        let Ok(page) = Url::parse(loc) else { continue };
                        let page = normalize(page, &self.cfg);
                        if !self.scope.is_internal(&page) || !self.allowed(&page, true) {
                            continue;
                        }
                        if seen.insert(page.to_string()) {
                            progress.discovered += 1;
                            queue.push_back(QueueItem { url: page, depth: 1, kind: FetchKind::Page, redirect_hops: 0, cookies: None, robots_line: None });
                        }
                    }
                    tx.send(CrawlEvent::Sitemap { sitemap: url.to_string(), urls: parsed.locs }).await.ok();
                }
            }
        }
    }

    /// Decide which links from a crawled internal page join the queue.
    fn discover(&self, item: &QueueItem, record: &PageRecord) -> Vec<QueueItem> {
        let cfg = &self.cfg;
        let mut out = Vec::new();
        let depth = item.depth + 1;
        let over_depth = cfg.max_depth.is_some_and(|max| depth > max);

        // Redirects (HTTP or meta refresh) count as one level of depth.
        // Always Follow Redirects ignores the depth limit for them.
        let is_meta_refresh = record.redirect_type.as_deref() == Some("Meta Refresh Redirect");
        if let Some(target) = record.redirect_url.as_deref().and_then(|u| Url::parse(u).ok()) {
            if (!over_depth || cfg.always_follow_redirects)
                && item.redirect_hops < cfg.max_redirects
                && (cfg.crawl_meta_refresh || !is_meta_refresh)
            {
                let kind = if self.scope.is_internal(&target) { FetchKind::Page } else { FetchKind::CheckOnly };
                if self.allowed(&target, kind == FetchKind::Page) {
                    out.push(QueueItem { url: target, depth, kind, redirect_hops: item.redirect_hops + 1, cookies: chain_cookies(cfg, item, record), robots_line: None });
                }
            }
        }

        // Always Follow Canonicals: canonical targets past the depth limit,
        // capped like a redirect chain.
        if over_depth {
            if cfg.always_follow_canonicals && item.redirect_hops < cfg.max_redirects {
                if let Some(target) = record.canonical().filter(|c| *c != record.url).and_then(|c| Url::parse(c).ok()) {
                    let kind = if self.scope.is_internal(&target) { FetchKind::Page } else { FetchKind::CheckOnly };
                    if self.allowed(&target, kind == FetchKind::Page) {
                        out.push(QueueItem { url: target, depth, kind, redirect_hops: item.redirect_hops + 1, cookies: None, robots_line: None });
                    }
                }
            }
            return out;
        }

        let Some(page) = &record.page else { return out };
        let page_nofollow = record.is_nofollow() && !cfg.follow_internal_nofollow;

        out.extend(page.links.iter().filter_map(|link| self.link_item(link, depth, page_nofollow)));
        out
    }

    /// Whether a link found on a page (at `depth` for its target) is queued,
    /// and how: fetched as a page, or only checked.
    fn link_item(&self, link: &Link, depth: u32, page_nofollow: bool) -> Option<QueueItem> {
        let cfg = &self.cfg;
        let url = Url::parse(&link.to).ok()?;
        let internal = self.scope.is_internal(&url);
        if !self.should_crawl(link, internal, page_nofollow) {
            return None;
        }
        let mut kind = match link.kind {
            LinkKind::Hyperlink | LinkKind::Canonical | LinkKind::HttpCanonical | LinkKind::RelNext
            | LinkKind::RelPrev | LinkKind::Hreflang | LinkKind::Iframe | LinkKind::MetaRefresh
            | LinkKind::Redirect | LinkKind::Amp | LinkKind::MobileAlternate => FetchKind::Page,
            LinkKind::Image | LinkKind::Css | LinkKind::JavaScript | LinkKind::Media | LinkKind::Swf => FetchKind::CheckOnly,
            LinkKind::Uncrawlable => return None,
        };
        if !internal {
            kind = FetchKind::CheckOnly;
        } else if kind == FetchKind::Page && !cfg.crawl_outside_start_folder && !self.scope.in_start_folder(&url) {
            if !cfg.check_links_outside_start_folder {
                return None;
            }
            kind = FetchKind::CheckOnly;
        }
        if !self.allowed(&url, internal) {
            return None;
        }
        Some(QueueItem { url, depth, kind, redirect_hops: 0, cookies: None, robots_line: None })
    }

    fn should_crawl(&self, link: &Link, internal: bool, page_nofollow: bool) -> bool {
        let cfg = &self.cfg;
        if !internal && !cfg.crawl_external_links {
            return false;
        }
        match link.kind {
            LinkKind::Hyperlink => {
                if internal {
                    cfg.crawl_internal_links && !page_nofollow && (link.follow || cfg.follow_internal_nofollow)
                } else {
                    link.follow || cfg.follow_external_nofollow
                }
            }
            LinkKind::Image => cfg.crawl_images,
            LinkKind::Css => cfg.crawl_css,
            LinkKind::JavaScript => cfg.crawl_js,
            LinkKind::Canonical | LinkKind::HttpCanonical => cfg.crawl_canonicals || cfg.always_follow_canonicals,
            LinkKind::RelNext | LinkKind::RelPrev => cfg.crawl_pagination,
            LinkKind::Hreflang => cfg.crawl_hreflang,
            LinkKind::Iframe => cfg.crawl_iframes,
            LinkKind::MetaRefresh => cfg.crawl_meta_refresh,
            LinkKind::Redirect => true,
            LinkKind::Media => cfg.crawl_media,
            LinkKind::Swf => cfg.crawl_swf,
            LinkKind::Amp => cfg.crawl_amp,
            LinkKind::MobileAlternate => cfg.crawl_mobile_alternate,
            LinkKind::Uncrawlable => false,
        }
    }

    /// Spider > Limits that count URLs as they are requested: per subdomain,
    /// per crawl depth and by URL path. Counts the URL when it is allowed.
    fn within_limits(&self, item: &QueueItem, c: &mut LimitCounters) -> bool {
        let cfg = &self.cfg;
        let host = item.url.host_str().unwrap_or("").to_ascii_lowercase();
        if cfg.max_urls_per_subdomain.is_some_and(|m| c.per_host.get(&host).copied().unwrap_or(0) >= m) {
            return false;
        }
        if cfg.max_urls_per_depth.is_some_and(|m| c.per_depth.get(&item.depth).copied().unwrap_or(0) >= m) {
            return false;
        }
        let s = item.url.as_str();
        let matched: Vec<usize> = self.path_limits.iter().enumerate().filter(|(_, (re, _))| re.is_match(s)).map(|(i, _)| i).collect();
        if c.per_path.len() < self.path_limits.len() {
            c.per_path.resize(self.path_limits.len(), 0);
        }
        if matched.iter().any(|&i| c.per_path[i] >= self.path_limits[i].1) {
            return false;
        }
        *c.per_host.entry(host).or_insert(0) += 1;
        *c.per_depth.entry(item.depth).or_insert(0) += 1;
        for i in matched {
            c.per_path[i] += 1;
        }
        true
    }

    /// Exclude, as edited mid-crawl (CrawlControl::set_exclude) or as configured.
    fn is_excluded(&self, url: &str) -> bool {
        match self.live_exclude.read().ok().as_deref() {
            Some(Some(live)) => live.iter().any(|r| r.is_match(url)),
            _ => self.exclude.iter().any(|r| r.is_match(url)),
        }
    }

    /// Include/Exclude rules and URL limits.
    fn allowed(&self, url: &Url, internal: bool) -> bool {
        let cfg = &self.cfg;
        let s = url.as_str();
        if s.len() > cfg.max_url_length {
            return false;
        }
        if self.is_excluded(s) {
            return false;
        }
        if internal {
            if !self.include.is_empty() && !self.include.iter().any(|r| r.is_match(s)) {
                return false;
            }
            if cfg.max_folder_depth.is_some_and(|m| folder_depth(url) > m) {
                return false;
            }
            if cfg.max_query_params.is_some_and(|m| query_param_count(url) > m) {
                return false;
            }
        }
        true
    }
}

async fn process(
    fetcher: &Fetcher,
    cfg: &CrawlConfig,
    rules: &CustomRules,
    item: &QueueItem,
    is_internal: bool,
    hsts: &std::sync::Mutex<HashSet<String>>,
    language: &crate::spelling::LanguageChecker,
    renderer: Option<&crate::render::Renderer>,
) -> PageRecord {
    // Respect HSTS Policy: an http:// URL on a host that sent HSTS is
    // reported as a 307 to https://, without a request.
    if cfg.respect_hsts && item.url.scheme() == "http" {
        let host = item.url.host_str().unwrap_or("").to_ascii_lowercase();
        if hsts.lock().is_ok_and(|h| h.contains(&host)) {
            let mut target = item.url.clone();
            let _ = target.set_scheme("https");
            let mut r = PageRecord {
                url: item.url.to_string(),
                depth: item.depth,
                is_internal,
                status_code: 307,
                status: "HSTS Policy".into(),
                redirect_url: Some(target.to_string()),
                redirect_type: Some("HSTS Policy".into()),
                ..Default::default()
            };
            r.compute_indexability();
            return r;
        }
    }
    if !matches!(item.url.scheme(), "http" | "https") {
        let mut r = PageRecord {
            url: item.url.to_string(),
            depth: item.depth,
            is_internal,
            status_code: 0,
            status: "Malformed URL".into(),
            error: Some(format!("Malformed URL: unknown scheme '{}'", item.url.scheme())),
            ..Default::default()
        };
        r.compute_indexability();
        return r;
    }
    let cookie = (cfg.cookie_storage == CookieStorage::SessionOnly).then_some(item.cookies.as_deref()).flatten();
    let res = fetcher.fetch_with_cookies(&item.url, cookie).await;
    if item.url.scheme() == "https" {
        let sts = res.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("strict-transport-security")).map(|(_, v)| v.to_ascii_lowercase());
        if sts.is_some_and(|v| !v.replace(' ', "").contains("max-age=0")) {
            if let (Ok(mut h), Some(host)) = (hsts.lock(), item.url.host_str()) {
                h.insert(host.to_ascii_lowercase());
            }
        }
    }
    let mut r = PageRecord {
        url: item.url.to_string(),
        depth: item.depth,
        is_internal,
        status_code: res.status,
        status: res.status_text.clone(),
        size: res.body.len() as u64,
        transferred: res.transferred,
        response_time_ms: res.elapsed_ms,
        http_version: res.http_version.clone(),
        error: res.error.clone(),
        ..Default::default()
    };
    r.headers = res.headers;
    r.content_type = r.header("content-type").unwrap_or("").to_string();
    r.last_modified = r.header("last-modified").map(str::to_string);
    r.x_robots_tag = r
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("x-robots-tag"))
        .map(|(_, v)| v.clone())
        .collect();
    r.http_canonical = r.header("link").and_then(http_canonical).and_then(|c| item.url.join(&c).ok()).map(|u| u.to_string());

    if (300..400).contains(&r.status_code) {
        if let Some(loc) = r.header("location").and_then(|l| item.url.join(l).ok()) {
            r.redirect_url = Some(normalize(loc, cfg).to_string());
            r.redirect_type = Some("HTTP Redirect".into());
        }
    }

    let ct = r.content_type.to_ascii_lowercase();
    r.is_html = ct.contains("text/html") || ct.contains("application/xhtml") || (ct.is_empty() && cfg.assume_pages_are_html);
    if r.status_code == 200 && !res.body.is_empty() {
        r.hash = Some(format!("{:x}", md5::compute(&res.body)));
    }

    r.robots_report_line = item.robots_line.clone();
    let mut self_refresh = false;
    if res.truncated {
        r.error = Some(format!("Not processed: larger than the Max Page Size ({} KB)", cfg.max_page_size_kb));
    } else if is_internal
        && r.status_code == 200
        && r.content_type.to_ascii_lowercase().contains("pdf")
        && (cfg.store_pdf || cfg.extract_pdf_properties || cfg.extract_pdf_links)
    {
        match crate::pdf::parse(&res.body, &item.url) {
            Ok(doc) => {
                let mut page = crate::model::PageData::default();
                // Title and Keywords are always read when a PDF is parsed.
                page.titles = doc.info.title.clone().into_iter().collect();
                page.meta_keywords = doc.info.keywords.clone().into_iter().collect();
                if cfg.store_pdf {
                    let text = doc.text.split_whitespace().collect::<Vec<_>>().join(" ");
                    crate::extract::text_metrics(&mut page, &text);
                    if !rules.is_empty() {
                        let targets = crate::custom::SearchTargets {
                            html: &text,
                            head_html: "",
                            body_html: &text,
                            page_text: &text,
                            page_text_no_anchors: &text,
                            content_text: &text,
                            xpath: None,
                        };
                        page.custom_search = crate::custom::run_searches(rules, &targets);
                    }
                    if language.enabled() {
                        page.language_issues = language.check(&text, None).await;
                    }
                    page.content_text = text.clone();
                    r.pdf_text = Some(text);
                }
                if cfg.extract_pdf_links {
                    page.links = doc.links;
                }
                if cfg.extract_pdf_properties {
                    r.pdf = Some(doc.info);
                }
                r.page = Some(page);
            }
            Err(e) => r.error = Some(format!("PDF could not be read: {e}")),
        }
    } else if r.is_html && is_internal && item.kind == FetchKind::Page && r.status_code == 200 {
        let html = String::from_utf8_lossy(&res.body);
        let mut page = extract(&html, &item.url, cfg, rules);
        if let Some(renderer) = renderer {
            let (rendered_page, info) = render_page(renderer, cfg, rules, &item.url, &r.content_type, page).await;
            page = rendered_page;
            r.rendered = Some(info);
        }
        if language.enabled() {
            // Titles, meta descriptions and the content area.
            let text = page.titles.iter().chain(page.meta_descriptions.iter()).chain(std::iter::once(&page.content_text)).cloned().collect::<Vec<_>>().join(". ");
            page.language_issues = language.check(&text, page.language.as_deref()).await;
        }
        if cfg.store_html {
            r.html = Some(html.clone().into_owned());
        }
        if let Some(target) = page.meta_refresh_url.clone() {
            if target != r.url {
                r.redirect_url = Some(target);
                r.redirect_type = Some("Meta Refresh Redirect".into());
            } else if cfg.respect_self_referencing_meta_refresh {
                self_refresh = true;
            }
        }
        r.page = Some(page);
    }
    // Spider > Extraction > Directives: a directive that isn't extracted
    // doesn't affect indexability or nofollow either.
    if !cfg.extraction.meta_robots {
        if let Some(p) = r.page.as_mut() {
            p.meta_robots.clear();
        }
    }
    if !cfg.extraction.x_robots_tag {
        r.x_robots_tag.clear();
    }
    r.compute_indexability();
    if self_refresh && r.indexable {
        r.indexable = false;
        r.indexability_status = "Self Referencing Meta Refresh".into();
    }
    r
}

/// Session Only cookies for the next hop of a redirect chain: those already
/// carried, plus any the redirect response set (later values win).
fn chain_cookies(cfg: &CrawlConfig, item: &QueueItem, record: &PageRecord) -> Option<String> {
    if cfg.cookie_storage != CookieStorage::SessionOnly {
        return None;
    }
    let mut pairs: Vec<(String, String)> = item
        .cookies
        .as_deref()
        .unwrap_or("")
        .split(';')
        .filter_map(|p| p.trim().split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect();
    for (_, raw) in record.headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie")) {
        if let Some((k, v)) = raw.split(';').next().and_then(|nv| nv.trim().split_once('=')) {
            pairs.retain(|(n, _)| n != k);
            pairs.push((k.to_string(), v.to_string()));
        }
    }
    (!pairs.is_empty()).then(|| pairs.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; "))
}

/// `<https://x.com/a>; rel="canonical"` → `https://x.com/a`.
fn http_canonical(link_header: &str) -> Option<String> {
    link_header.split(',').find_map(|part| {
        let lower = part.to_ascii_lowercase();
        if !lower.contains("rel=\"canonical\"") && !lower.contains("rel=canonical") {
            return None;
        }
        let start = part.find('<')? + 1;
        let end = part.find('>')?;
        Some(part[start..end].trim().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta_refresh_record() -> PageRecord {
        PageRecord {
            url: "https://example.com/old".into(),
            is_internal: true,
            status_code: 200,
            redirect_url: Some("https://example.com/new".into()),
            redirect_type: Some("Meta Refresh Redirect".into()),
            ..Default::default()
        }
    }

    #[test]
    fn meta_refresh_follows_the_crawl_setting() {
        let start = Url::parse("https://example.com/").unwrap();
        let item = QueueItem { url: Url::parse("https://example.com/old").unwrap(), depth: 0, kind: FetchKind::Page, redirect_hops: 0, cookies: None, robots_line: None };
        let followed = |crawl_meta_refresh: bool| {
            let cfg = CrawlConfig { crawl_meta_refresh, ..CrawlConfig::default() };
            let crawler = Crawler::new(cfg, &start).unwrap();
            crawler.discover(&item, &meta_refresh_record()).iter().any(|q| q.url.as_str() == "https://example.com/new")
        };
        assert!(followed(true));
        assert!(!followed(false), "Crawl > Meta Refresh off must not follow refresh targets");
    }
}

#[cfg(test)]
mod hsts_tests {
    use super::*;

    #[tokio::test]
    async fn http_links_to_an_hsts_host_become_307() {
        let cfg = CrawlConfig::default();
        let fetcher = Fetcher::new(&cfg).unwrap();
        let rules = CustomRules::default();
        let hsts = std::sync::Mutex::new(HashSet::from(["secure.example".to_string()]));
        let item = QueueItem { url: Url::parse("http://secure.example/page").unwrap(), depth: 1, kind: FetchKind::Page, redirect_hops: 0, cookies: None, robots_line: None };
        let r = process(&fetcher, &cfg, &rules, &item, true, &hsts, &crate::spelling::LanguageChecker::new(&cfg), None).await;
        assert_eq!((r.status_code, r.status.as_str()), (307, "HSTS Policy"));
        assert_eq!(r.redirect_url.as_deref(), Some("https://secure.example/page"));
        assert_eq!(r.redirect_type.as_deref(), Some("HSTS Policy"));
        assert!(!r.indexable);
    }

    #[test]
    fn session_cookies_follow_the_redirect_chain() {
        let cfg = CrawlConfig::default();
        let item = QueueItem { url: Url::parse("https://x.com/a").unwrap(), depth: 0, kind: FetchKind::Page, redirect_hops: 0, cookies: Some("a=1; b=2".into()), robots_line: None };
        let rec = PageRecord { headers: vec![("Set-Cookie".into(), "b=3; Path=/".into()), ("set-cookie".into(), "c=4".into())], ..Default::default() };
        assert_eq!(chain_cookies(&cfg, &item, &rec).as_deref(), Some("a=1; b=3; c=4"));
        let off = CrawlConfig { cookie_storage: CookieStorage::Persistent, ..CrawlConfig::default() };
        assert_eq!(chain_cookies(&off, &item, &rec), None);
    }
}


/// Render a page and extract from the rendered HTML. Links from both the
/// original and the rendered HTML are kept, with their Link Origin. If
/// rendering fails the original HTML's data is used and the error noted.
async fn render_page(
    renderer: &crate::render::Renderer,
    cfg: &CrawlConfig,
    rules: &CustomRules,
    url: &Url,
    content_type: &str,
    raw: crate::model::PageData,
) -> (crate::model::PageData, crate::model::RenderInfo) {
    let original = crate::model::OriginalHtml {
        title: raw.titles.first().cloned(),
        meta_description: raw.meta_descriptions.first().cloned(),
        h1: raw.h1.first().cloned(),
        canonical: raw.canonicals.first().cloned(),
        directives: raw.meta_robots.join(","),
        word_count: raw.word_count,
        link_count: raw.links.len() as u32,
    };
    let mut info = crate::model::RenderInfo { original, ..Default::default() };
    match renderer.render(url, content_type).await {
        Ok(out) => {
            let mut page = extract(&out.html, url, cfg, rules);
            let key = |l: &Link| (l.kind, l.to.clone());
            let raw_keys: HashSet<(LinkKind, String)> = raw.links.iter().map(key).collect();
            let rendered_keys: HashSet<(LinkKind, String)> = page.links.iter().map(key).collect();
            for l in &mut page.links {
                l.origin = if raw_keys.contains(&key(l)) { "HTML & Rendered HTML".into() } else { "Rendered HTML".into() };
            }
            for mut l in raw.links.into_iter().filter(|l| !rendered_keys.contains(&key(l))) {
                l.origin = "HTML".into();
                page.links.push(l);
            }
            info.console = out.console;
            info.screenshot = out.screenshot;
            info.requests = out.requests;
            info.transferred_bytes = out.transferred_bytes;
            info.accessibility = out.accessibility;
            info.accessibility_score = out.accessibility_score;
            info.custom_js = out.custom_js;
            info.images = out.images;
            // CSS background images: crawled as images, marked by rel.
            for bg in out.backgrounds {
                if page.links.iter().any(|l| l.kind == LinkKind::Image && l.to == bg) {
                    continue;
                }
                if let Ok(to) = Url::parse(&bg) {
                    page.links.push(Link {
                        kind: LinkKind::Image,
                        to: to.to_string(),
                        href: bg,
                        anchor: String::new(),
                        alt: None,
                        follow: true,
                        rel: "background-image".into(),
                        target: String::new(),
                        path_type: "Absolute".into(),
                        link_path: String::new(),
                        position: String::new(),
                        hreflang: None,
                        origin: "Rendered HTML".into(),
                    });
                }
            }
            // seoSpider.saveUrls: crawled like links found on the page.
            for href in out.saved_urls {
                if let Ok(to) = url.join(href.trim()) {
                    if matches!(to.scheme(), "http" | "https") {
                        page.links.push(Link {
                            kind: LinkKind::Hyperlink,
                            to: to.to_string(),
                            href,
                            anchor: String::new(),
                            alt: None,
                            follow: true,
                            rel: String::new(),
                            target: String::new(),
                            path_type: "Absolute".into(),
                            link_path: String::new(),
                            position: "Custom JavaScript".into(),
                            hreflang: None,
                            origin: "Custom JavaScript".into(),
                        });
                    }
                }
            }
            if cfg.rendering.store_rendered_html {
                info.html = Some(out.html);
            }
            (page, info)
        }
        Err(e) => {
            info.error = Some(format!("Rendering failed: {e}"));
            (raw, info)
        }
    }
}
