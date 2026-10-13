//! API data: PageSpeed Insights, Search Console, GA4, AI prompts,
//! embeddings and link metrics, stored as (url, source, metric, value) rows.
//!
//! Rows keep the URL the provider reported. `address` is the crawled URL it
//! matches (after the provider's trailing-slash and case options), filled in
//! by [`Store::resolve_api_urls`]; rows with no match are orphans.

use crate::tabs::{FilterDef, IssueType, Priority, TabDef};
use crate::Store;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use toad_core::config::CrawlConfig;

pub const PSI: &str = "psi";
pub const GSC: &str = "gsc";
pub const INSPECT: &str = "gsc_inspect";
pub const GA4: &str = "ga4";
pub const AI: &str = "ai";
pub const EMBED: &str = "embed";
pub const AHREFS: &str = "ahrefs";
pub const MAJESTIC: &str = "majestic";
pub const MOZ: &str = "moz";
/// Errors from any source: metric = source name.
pub const ERROR: &str = "error";

/// One value from a provider.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiValue {
    pub url: String,
    pub metric: String,
    pub value: String,
}

impl ApiValue {
    pub fn new(url: impl Into<String>, metric: impl Into<String>, value: impl ToString) -> Self {
        Self { url: url.into(), metric: metric.into(), value: value.to_string() }
    }
}

/// The key a provider URL and a crawled URL are matched on.
pub fn match_key(url: &str, ignore_trailing_slash: bool, case_sensitive: bool) -> String {
    let mut k = url.trim().to_string();
    if ignore_trailing_slash && k.len() > 1 {
        // Keep the slash of a bare origin ("https://x.com/").
        let bare_origin = k.matches('/').count() <= 3;
        if !bare_origin {
            while k.ends_with('/') {
                k.pop();
            }
        }
    }
    if !case_sensitive {
        k = k.to_lowercase();
    }
    k
}

impl Store {
    /// Replace one source's values for these URLs (the metrics given).
    pub fn put_api_values(&mut self, source: &str, values: &[ApiValue]) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO api_values (url, address, source, metric, value) VALUES (?1, ?1, ?2, ?3, ?4)
                 ON CONFLICT(url, source, metric) DO UPDATE SET value = excluded.value",
            )?;
            for v in values {
                stmt.execute(params![v.url, source, v.metric, v.value])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Match a source's URLs to crawled URLs. Exact matches win; otherwise
    /// the provider's trailing-slash and case options apply.
    pub fn resolve_api_urls(&mut self, source: &str, ignore_trailing_slash: bool, case_sensitive: bool) -> anyhow::Result<()> {
        let mut by_key: HashMap<String, String> = HashMap::new();
        {
            let mut stmt = self.conn.prepare("SELECT address FROM urls")?;
            for a in stmt.query_map([], |r| r.get::<_, String>(0))? {
                let a = a?;
                by_key.entry(match_key(&a, ignore_trailing_slash, case_sensitive)).or_insert(a);
            }
        }
        let urls: Vec<String> = {
            let mut stmt = self.conn.prepare("SELECT DISTINCT url FROM api_values WHERE source = ?1")?;
            let rows = stmt.query_map(params![source], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached("UPDATE api_values SET address = ?3 WHERE source = ?1 AND url = ?2")?;
            for u in urls {
                let address = by_key.get(&match_key(&u, ignore_trailing_slash, case_sensitive)).cloned();
                stmt.execute(params![source, u, address])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// URLs a per-page API runs on: internal HTML pages that returned 200.
    pub fn api_targets(&self, indexable_only: bool) -> anyhow::Result<Vec<String>> {
        let sql = format!(
            "SELECT address FROM urls WHERE is_internal = 1 AND is_html = 1 AND status_code = 200{} ORDER BY crawl_order",
            if indexable_only { " AND indexable = 1" } else { "" }
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// URLs a source already has data for (so Request API Data can skip them).
    pub fn api_done(&self, source: &str) -> anyhow::Result<std::collections::HashSet<String>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT url FROM api_values WHERE source = ?1")?;
        let rows = stmt.query_map(params![source], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Provider URLs with no crawled match (orphans), most-valuable first by `metric`.
    pub fn api_orphans(&self, source: &str, metric: &str) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT url FROM api_values WHERE source = ?1 AND metric = ?2 AND address IS NULL ORDER BY CAST(value AS REAL) DESC",
        )?;
        let rows = stmt.query_map(params![source, metric], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn api_value(&self, address: &str, source: &str, metric: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM api_values WHERE address = ?1 AND source = ?2 AND metric = ?3", params![address, source, metric], |r| r.get(0))
            .optional()?)
    }

    /// The page text an AI prompt or embedding is given.
    pub fn content_for(&self, address: &str, content: toad_core::api_config::AiContent) -> anyhow::Result<Option<String>> {
        use toad_core::api_config::AiContent;
        match content {
            AiContent::UrlOnly => Ok(Some(String::new())),
            AiContent::Html => self.page_source(address),
            AiContent::PageText | AiContent::ContentArea => {
                let kind = if content == AiContent::PageText { "page" } else { "content" };
                Ok(self
                    .conn
                    .query_row(
                        "SELECT t.body FROM page_text t JOIN urls u ON u.id = t.url_id WHERE u.address = ?1 AND t.kind = ?2",
                        params![address, kind],
                        |r| r.get::<_, Vec<u8>>(0),
                    )
                    .optional()?
                    .map(|b| crate::decompress(&b))
                    .transpose()?)
            }
        }
    }

    /// Store an embedding vector for a page.
    pub fn put_embedding(&mut self, address: &str, vector: &[f32]) -> anyhow::Result<()> {
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.conn.execute("INSERT OR REPLACE INTO embeddings (address, vector) VALUES (?1, ?2)", params![address, bytes])?;
        Ok(())
    }

    pub fn embeddings(&self) -> anyhow::Result<Vec<(String, Vec<f32>)>> {
        let mut stmt = self.conn.prepare("SELECT e.address, e.vector FROM embeddings e JOIN urls u ON u.address = e.address ORDER BY u.crawl_order")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (a, b) = row?;
            out.push((a, b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()));
        }
        Ok(out)
    }

    /// Whether an address is indexable (for embeddings' indexable-only options).
    pub fn is_indexable(&self, address: &str) -> anyhow::Result<bool> {
        Ok(self
            .conn
            .query_row("SELECT indexable FROM urls WHERE address = ?1", params![address], |r| r.get::<_, bool>(0))
            .optional()?
            .unwrap_or(false))
    }
}

/// A value of `source`/`metric` for the row's URL.
fn val(source: &str, metric: &str) -> String {
    format!(
        "(SELECT a.value FROM api_values a WHERE a.address = u.address AND a.source = '{source}' AND a.metric = '{}')",
        metric.replace('\'', "''")
    )
}

fn num(source: &str, metric: &str) -> String {
    format!("CAST({} AS REAL)", val(source, metric))
}

fn has(source: &str) -> String {
    format!("EXISTS (SELECT 1 FROM api_values a WHERE a.address = u.address AND a.source = '{source}' AND a.metric <> '{ERROR}')")
}

fn errored(source: &str) -> String {
    format!("EXISTS (SELECT 1 FROM api_values a WHERE a.address = u.address AND a.source = '{ERROR}' AND a.metric = '{source}')")
}

fn f(name: &str, sql: String, issue: Option<(IssueType, Priority)>, description: &str) -> FilterDef {
    FilterDef { name: name.to_string(), sql, issue, description: description.to_string() }
}

fn all() -> FilterDef {
    f("All", "1 = 1".into(), None, "Every HTML page.")
}

/// Lighthouse audits shown as PageSpeed filters: Lighthouse 13 insights and
/// the classic audits it kept.
pub const PSI_AUDITS: &[(&str, &str)] = &[
    ("render-blocking-insight", "Render Blocking Requests"),
    ("document-latency-insight", "Document Request Latency"),
    ("image-delivery-insight", "Improve Image Delivery"),
    ("lcp-discovery-insight", "LCP Request Discovery"),
    ("lcp-phases-insight", "LCP Breakdown"),
    ("cls-culprits-insight", "Layout Shift Culprits"),
    ("dom-size-insight", "Optimize DOM Size"),
    ("duplicated-javascript-insight", "Duplicated JavaScript"),
    ("font-display-insight", "Font Display"),
    ("interaction-to-next-paint-insight", "INP Breakdown"),
    ("legacy-javascript-insight", "Legacy JavaScript"),
    ("modern-http-insight", "Modern HTTP"),
    ("network-dependency-tree-insight", "Network Dependency Tree"),
    ("third-parties-insight", "3rd Parties"),
    ("use-cache-insight", "Use Efficient Cache Lifetimes"),
    ("viewport-insight", "Optimize Viewport for Mobile"),
    ("forced-reflow-insight", "Forced Reflow"),
    ("unused-css-rules", "Reduce Unused CSS"),
    ("unused-javascript", "Reduce Unused JavaScript"),
    ("unminified-css", "Minify CSS"),
    ("unminified-javascript", "Minify JavaScript"),
    ("mainthread-work-breakdown", "Minimize Main-Thread Work"),
    ("bootup-time", "Reduce JavaScript Execution Time"),
    ("long-tasks", "Avoid Long Main-Thread Tasks"),
    ("non-composited-animations", "Avoid Non-Composited Animations"),
    ("unsized-images", "Image Elements Do Not Have Explicit Width & Height"),
    ("total-byte-weight", "Avoid Enormous Network Payloads"),
];

/// The API tabs, in Screaming Frog's order: PageSpeed, Search Console,
/// Analytics, Link Metrics, AI.
pub fn api_tabs(cfg: &CrawlConfig, html: &str, start_url_sql: &str) -> Vec<TabDef> {
    use IssueType::*;
    use Priority::*;
    let mut tabs = Vec::new();

    // ---------------------------------------------------------------- PageSpeed
    let mut psi_cols: Vec<(String, String)> = vec![
        ("Performance Score".into(), val(PSI, "performance_score")),
        ("Core Web Vitals Assessment".into(), val(PSI, "cwv_assessment")),
        ("CrUX Largest Contentful Paint (ms)".into(), val(PSI, "crux_lcp_ms")),
        ("CrUX Interaction to Next Paint (ms)".into(), val(PSI, "crux_inp_ms")),
        ("CrUX Cumulative Layout Shift".into(), val(PSI, "crux_cls")),
        ("CrUX First Contentful Paint (ms)".into(), val(PSI, "crux_fcp_ms")),
        ("CrUX Time to First Byte (ms)".into(), val(PSI, "crux_ttfb_ms")),
        ("First Contentful Paint (ms)".into(), val(PSI, "lh_fcp_ms")),
        ("Largest Contentful Paint (ms)".into(), val(PSI, "lh_lcp_ms")),
        ("Total Blocking Time (ms)".into(), val(PSI, "lh_tbt_ms")),
        ("Cumulative Layout Shift".into(), val(PSI, "lh_cls")),
        ("Speed Index (ms)".into(), val(PSI, "lh_si_ms")),
        ("Total Size Transferred (bytes)".into(), val(PSI, "total_bytes")),
        ("PSI Error".into(), val(ERROR, PSI)),
    ];
    let mut psi_filters = vec![
        all(),
        f("Failed Core Web Vitals Assessment", format!("{} = 'Failed'", val(PSI, "cwv_assessment")), Some((Issue, High)), "Real-user (CrUX) LCP, INP or CLS is not good at the 75th percentile."),
        f("Passed Core Web Vitals Assessment", format!("{} = 'Passed'", val(PSI, "cwv_assessment")), None, "Real-user LCP, INP and CLS are all good."),
        f("No CrUX Data", format!("{} = 'No Data'", val(PSI, "cwv_assessment")), None, "Too little real-user traffic for Chrome UX Report data."),
        f("Performance Score Below 50", format!("{} < 50", num(PSI, "performance_score")), Some((Warning, Medium)), "Lighthouse lab performance score under 50 (poor)."),
        f("Request Error", errored(PSI), Some((Warning, Low)), "PageSpeed Insights couldn't analyse the page."),
    ];
    for (id, name) in PSI_AUDITS {
        psi_filters.push(f(
            name,
            format!("{} < 0.9", num(PSI, &format!("audit:{id}"))),
            Some((Opportunity, Medium)),
            &format!("Lighthouse flags this page for '{name}' ({id})."),
        ));
        psi_cols.push((format!("{name} Savings (ms)"), val(PSI, &format!("savings_ms:{id}"))));
    }
    tabs.push(TabDef { name: "PageSpeed", base: html.into(), columns: vec!["Address", "Indexability"], extra_columns: psi_cols, filters: psi_filters });

    // ---------------------------------------------------------------- Search Console
    let gsc_cols: Vec<(String, String)> = vec![
        ("Clicks".into(), val(GSC, "clicks")),
        ("Impressions".into(), val(GSC, "impressions")),
        ("CTR".into(), val(GSC, "ctr")),
        ("Position".into(), val(GSC, "position")),
        ("Summary".into(), val(INSPECT, "verdict")),
        ("Coverage".into(), val(INSPECT, "coverage")),
        ("Last Crawl".into(), val(INSPECT, "last_crawl")),
        ("Crawled As".into(), val(INSPECT, "crawled_as")),
        ("Crawl Allowed".into(), val(INSPECT, "robots")),
        ("Page Fetch".into(), val(INSPECT, "page_fetch")),
        ("Indexing Allowed".into(), val(INSPECT, "indexing")),
        ("User-Declared Canonical".into(), val(INSPECT, "user_canonical")),
        ("Google-Selected Canonical".into(), val(INSPECT, "google_canonical")),
        ("Rich Results".into(), val(INSPECT, "rich_results")),
        ("Search Console Error".into(), format!("coalesce({}, {})", val(ERROR, GSC), val(ERROR, INSPECT))),
    ];
    let orphan = |source: &str| format!("{} AND coalesce(u.unique_inlinks, 0) = 0 AND u.address <> {start_url_sql}", has(source));
    let verdict = val(INSPECT, "verdict");
    tabs.push(TabDef {
        name: "Search Console",
        base: html.into(),
        columns: vec!["Address", "Indexability", "Indexability Status"],
        extra_columns: gsc_cols,
        filters: vec![
            all(),
            f("Clicks Above 0", format!("{} > 0", num(GSC, "clicks")), None, "Pages with clicks from Google search in the date range."),
            f("No Search Analytics Data", format!("NOT {}", has(GSC)), None, "Pages with no impressions in Search Console in the date range."),
            f("Non-Indexable with Search Analytics Data", format!("u.indexable = 0 AND {}", has(GSC)), Some((Issue, High)), "Non-indexable pages that still appear in search results."),
            f("Orphan URLs", orphan(GSC), Some((Warning, High)), "Pages in Search Console that no crawled page links to (needs Crawl New URLs Discovered in Search Console)."),
            f("URL Is Not on Google", format!("{verdict} IN ('FAIL', 'NEUTRAL')"), Some((Issue, High)), "URL Inspection: Google hasn't indexed this URL."),
            f("Indexable URL Not Indexed", format!("u.indexable = 1 AND {verdict} IN ('FAIL', 'NEUTRAL')"), Some((Issue, High)), "Indexable pages Google hasn't indexed."),
            f("URL Is on Google, But Has Issues", format!("{verdict} = 'PARTIAL'"), Some((Warning, Medium)), "Indexed, with problems in enhancements such as rich results."),
            f(
                "User-Declared Canonical Not Selected",
                format!("{} IS NOT NULL AND {} IS NOT NULL AND {} <> {}", val(INSPECT, "user_canonical"), val(INSPECT, "google_canonical"), val(INSPECT, "user_canonical"), val(INSPECT, "google_canonical")),
                Some((Warning, High)),
                "Google chose a different canonical from the one the page declares.",
            ),
            f("Rich Result Invalid", format!("{} = 'FAIL'", val(INSPECT, "rich_results")), Some((Issue, Medium)), "URL Inspection reports invalid rich result markup."),
            f("Request Error", format!("{} OR {}", errored(GSC), errored(INSPECT)), Some((Warning, Low)), "A Search Console request failed."),
        ],
    });

    // ---------------------------------------------------------------- Analytics
    let ga = &cfg.apis.ga4;
    let ga_cols: Vec<(String, String)> =
        ga.metrics.iter().map(|m| (format!("GA4 {m}"), val(GA4, m))).chain([("GA4 Error".into(), val(ERROR, GA4))]).collect();
    let sessions = ga.metrics.iter().find(|m| *m == "sessions").cloned().unwrap_or_else(|| ga.metrics.first().cloned().unwrap_or_default());
    tabs.push(TabDef {
        name: "Analytics",
        base: html.into(),
        columns: vec!["Address", "Indexability", "Indexability Status"],
        extra_columns: ga_cols,
        filters: vec![
            all(),
            f("Sessions Above 0", format!("{} > 0", num(GA4, &sessions)), None, "Pages with sessions in the date range."),
            f("Bounce Rate Above 70%", format!("{} > 0.7", num(GA4, "bounceRate")), Some((Warning, Low)), "Pages where most sessions were not engaged (needs the bounceRate metric)."),
            f("No GA Data", format!("NOT {}", has(GA4)), None, "Pages with no GA4 data in the date range."),
            f("Non-Indexable with GA Data", format!("u.indexable = 0 AND {}", has(GA4)), Some((Warning, Medium)), "Non-indexable pages that still get visits."),
            f("Orphan URLs", orphan(GA4), Some((Warning, High)), "Pages in GA4 that no crawled page links to (needs Crawl New URLs Discovered in Google Analytics)."),
        ],
    });

    // ---------------------------------------------------------------- Link Metrics
    let link_cols: Vec<(String, String)> = vec![
        ("Ahrefs Domain Rating".into(), val(AHREFS, "domain_rating")),
        ("Ahrefs Backlinks".into(), val(AHREFS, "backlinks")),
        ("Ahrefs Referring Domains".into(), val(AHREFS, "refdomains")),
        ("Ahrefs Organic Traffic".into(), val(AHREFS, "org_traffic")),
        ("Ahrefs Organic Keywords".into(), val(AHREFS, "org_keywords")),
        ("Majestic Trust Flow".into(), val(MAJESTIC, "trust_flow")),
        ("Majestic Citation Flow".into(), val(MAJESTIC, "citation_flow")),
        ("Majestic External Backlinks".into(), val(MAJESTIC, "backlinks")),
        ("Majestic Referring Domains".into(), val(MAJESTIC, "refdomains")),
        ("Moz Page Authority".into(), val(MOZ, "page_authority")),
        ("Moz Domain Authority".into(), val(MOZ, "domain_authority")),
        ("Moz Spam Score".into(), val(MOZ, "spam_score")),
        ("Moz Linking Root Domains".into(), val(MOZ, "refdomains")),
        ("Moz External Links".into(), val(MOZ, "backlinks")),
    ];
    tabs.push(TabDef {
        name: "Link Metrics",
        base: html.into(),
        columns: vec!["Address", "Indexability"],
        extra_columns: link_cols,
        filters: vec![
            all(),
            f("Ahrefs", has(AHREFS), None, "Pages with Ahrefs data."),
            f("Majestic", has(MAJESTIC), None, "Pages with Majestic data."),
            f("Moz", has(MOZ), None, "Pages with Moz data."),
            f("Request Error", format!("{} OR {} OR {}", errored(AHREFS), errored(MAJESTIC), errored(MOZ)), Some((Warning, Low)), "A link metrics request failed."),
        ],
    });

    // ---------------------------------------------------------------- AI
    let prompts: Vec<&toad_core::api_config::AiPrompt> = cfg.apis.ai.prompts.iter().filter(|p| !p.name.trim().is_empty()).collect();
    let mut ai_cols: Vec<(String, String)> = prompts.iter().map(|p| (p.name.clone(), val(AI, &p.name))).collect();
    ai_cols.push(("AI Error".into(), val(ERROR, AI)));
    let mut ai_filters = vec![all()];
    for p in &prompts {
        ai_filters.push(f(&p.name, format!("{} IS NOT NULL", val(AI, &p.name)), None, &format!("Pages with a response to '{}'.", p.name)));
    }
    ai_filters.push(f("Request Error", errored(AI), Some((Warning, Low)), "An AI request failed."));
    tabs.push(TabDef { name: "AI", base: html.into(), columns: vec!["Address", "Indexability"], extra_columns: ai_cols, filters: ai_filters });

    tabs
}

/// Content > Embeddings filters and columns, added to the Content tab.
pub fn embedding_filters(cfg: &CrawlConfig) -> (Vec<FilterDef>, Vec<(String, String)>) {
    use IssueType::*;
    use Priority::*;
    let e = &cfg.apis.embeddings;
    let mut filters = Vec::new();
    if e.semantic_similarity {
        let ix = if e.similarity_indexable_only { " AND u.indexable = 1" } else { "" };
        filters.push(f(
            "Semantically Similar",
            format!("{} >= {}{ix}", num(EMBED, "closest_similarity"), e.similarity_threshold),
            Some((Warning, Medium)),
            "Pages whose embeddings are at least as similar as the Semantic Similarity threshold to another page: possible duplicate topics.",
        ));
    }
    if e.low_relevance {
        let ix = if e.low_relevance_indexable_only { " AND u.indexable = 1" } else { "" };
        filters.push(f(
            "Low Relevance Content",
            format!("{} < {}{ix}", num(EMBED, "relevance"), e.low_relevance_threshold),
            Some((Opportunity, Low)),
            "Pages whose embedding is far from the site's average topic.",
        ));
    }
    let cols = vec![
        ("Closest Semantically Similar Address".into(), val(EMBED, "closest_address")),
        ("Semantic Similarity Score".into(), val(EMBED, "closest_similarity")),
        ("No. Semantically Similar".into(), val(EMBED, "similar_count")),
        ("Semantic Relevance Score".into(), val(EMBED, "relevance")),
    ];
    (filters, cols)
}

/// Cosine similarity of two vectors (0 when either is empty or zero).
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0f64, 0f64, 0f64);
    for (x, y) in a.iter().zip(b) {
        dot += *x as f64 * *y as f64;
        na += (*x as f64).powi(2);
        nb += (*y as f64).powi(2);
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

impl Store {
    /// Semantic Similarity and Low Relevance from stored embeddings.
    /// Similarity: each page's closest other page (cosine) and how many are
    /// above the threshold. Relevance: cosine to the mean of all pages.
    pub fn analyse_embeddings(&mut self) -> anyhow::Result<()> {
        let e = self.cfg.apis.embeddings.clone();
        let mut vecs = self.embeddings()?;
        if e.similarity_indexable_only {
            let mut keep = Vec::new();
            for (a, v) in vecs {
                if self.is_indexable(&a)? {
                    keep.push((a, v));
                }
            }
            vecs = keep;
        }
        let mut out = Vec::new();
        let dim = vecs.first().map_or(0, |(_, v)| v.len());
        let mut centroid = vec![0f32; dim];
        for (_, v) in vecs.iter().filter(|(_, v)| v.len() == dim) {
            for (c, x) in centroid.iter_mut().zip(v) {
                *c += *x;
            }
        }
        for (i, (a, v)) in vecs.iter().enumerate() {
            let mut best: Option<(f64, &str)> = None;
            let mut count = 0u32;
            for (j, (b, w)) in vecs.iter().enumerate() {
                if i == j {
                    continue;
                }
                let s = cosine(v, w);
                if s >= e.similarity_threshold {
                    count += 1;
                }
                if best.is_none_or(|(bs, _)| s > bs) {
                    best = Some((s, b));
                }
            }
            if let Some((s, b)) = best {
                out.push(crate::api::ApiValue::new(a.clone(), "closest_similarity", format!("{:.4}", s)));
                out.push(crate::api::ApiValue::new(a.clone(), "closest_address", b));
            }
            out.push(crate::api::ApiValue::new(a.clone(), "similar_count", count));
            out.push(crate::api::ApiValue::new(a.clone(), "relevance", format!("{:.4}", cosine(v, &centroid))));
        }
        self.conn.execute("DELETE FROM api_values WHERE source = ?1", params![EMBED])?;
        self.put_api_values(EMBED, &out)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_keys() {
        assert_eq!(match_key("https://x.com/a/", true, true), "https://x.com/a");
        assert_eq!(match_key("https://x.com/", true, true), "https://x.com/");
        assert_eq!(match_key("https://x.com/A", false, false), "https://x.com/a");
    }

    #[test]
    fn cosine_similarity() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-9);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-9);
        assert_eq!(cosine(&[], &[]), 0.0);
    }
}
