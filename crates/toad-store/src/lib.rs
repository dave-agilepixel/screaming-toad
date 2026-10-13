//! Crawl store: one SQLite file per crawl (`.toad`), holding every URL,
//! every link, and the configuration the crawl ran with.

mod analysis;
pub mod compare;
pub mod crawls;
mod query;
mod reports;
mod schema;
mod sitemap_gen;
pub mod tabs;

pub use crawls::{CrawlEntry, CrawlStatus};
pub use compare::{ChangeElement, CompareConfig, CompareState, Comparison, LinkState, UrlMapping};
pub use query::{write_csv, IssueRow, OverviewRow, RowQuery, Table};
pub use reports::{ReportDef, ReportKind};
pub use sitemap_gen::{ChangeFreq, LastMod, SitemapOptions};
pub use tabs::{FilterDef, IssueType, Priority, TabDef};

use anyhow::Context;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::path::Path;
use toad_core::model::{LinkKind, PageRecord};
use toad_core::urlutil::{folder_depth, normalize, query_param_count, Scope};
use toad_core::minhash;
use toad_core::structured::Severity;
use toad_core::{pixel, CrawlConfig};
use url::Url;

pub struct Store {
    conn: Connection,
    cfg: CrawlConfig,
    scope: Option<Scope>,
    next_order: i64,
    insert_sql: Option<String>,
}

impl Store {
    /// Create a new crawl file, replacing any existing file at `path`.
    pub fn create(path: &Path, start_url: &Url, cfg: &CrawlConfig) -> anyhow::Result<Self> {
        if path.exists() {
            std::fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(schema::SCHEMA)?;
        let store = Self {
            conn,
            cfg: cfg.clone(),
            scope: Some(Scope::new(start_url, cfg)),
            next_order: 0,
            insert_sql: None,
        };
        store.set_meta("start_url", start_url.as_str())?;
        store.set_meta("config", &serde_json::to_string(cfg)?)?;
        store.set_meta("toad_version", env!("CARGO_PKG_VERSION"))?;
        Ok(store)
    }

    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(schema::SCHEMA)?;
        schema::migrate(&conn)?;
        let get = |k: &str| -> anyhow::Result<Option<String>> {
            Ok(conn.query_row("SELECT value FROM meta WHERE key = ?1", params![k], |r| r.get(0)).optional()?)
        };
        let cfg: CrawlConfig = match get("config")? {
            Some(json) => serde_json::from_str(&json)?,
            None => CrawlConfig::default(),
        };
        let scope = get("start_url")?.and_then(|u| Url::parse(&u).ok()).map(|u| Scope::new(&u, &cfg));
        let next_order: i64 = conn.query_row("SELECT coalesce(max(crawl_order), 0) + 1 FROM urls", [], |r| r.get(0))?;
        Ok(Self { conn, cfg, scope, next_order, insert_sql: None })
    }

    pub fn config(&self) -> &CrawlConfig {
        &self.cfg
    }

    pub fn set_meta(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn meta(&self, key: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| r.get(0))
            .optional()?)
    }

    /// Write a batch of crawled URLs and their links in one transaction.
    pub fn insert_batch(&mut self, records: &[PageRecord]) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        for rec in records {
            let order = self.next_order;
            self.next_order += 1;
            let values = url_values(rec, order, &self.cfg);
            let sql = self.insert_sql.get_or_insert_with(|| {
                let cols: Vec<&str> = values.iter().map(|(c, _)| *c).collect();
                let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
                format!("INSERT OR IGNORE INTO urls ({}) VALUES ({})", cols.join(", "), placeholders.join(", "))
            });
            let inserted = tx.prepare_cached(sql)?.execute(params_from_iter(values.into_iter().map(|(_, v)| v)))?;
            if inserted == 0 {
                continue;
            }
            let source_id = tx.last_insert_rowid();
            let mut link_stmt = tx.prepare_cached(
                "INSERT INTO links (source_id, target, internal, kind, anchor, alt, follow, rel, target_attr, path_type, link_path, position, hreflang, origin)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            )?;
            let internal = |target: &str| {
                Url::parse(target).ok().zip(self.scope.as_ref()).is_some_and(|(u, s)| s.is_internal(&u))
            };
            if let Some(page) = &rec.page {
                for l in page.links.iter().filter(|l| self.cfg.stores_link(l.kind, internal(&l.to))) {
                    link_stmt.execute(params![
                        source_id,
                        l.to,
                        internal(&l.to),
                        l.kind.as_str(),
                        l.anchor,
                        l.alt,
                        l.follow,
                        l.rel,
                        l.target,
                        l.path_type,
                        l.link_path,
                        l.position,
                        l.hreflang,
                        Some(l.origin.as_str()).filter(|o| !o.is_empty()),
                    ])?;
                }
            }
            if let (Some(target), Some("HTTP Redirect" | "HSTS Policy")) = (&rec.redirect_url, rec.redirect_type.as_deref()) {
                link_stmt.execute(params![
                    source_id, target, internal(target), LinkKind::Redirect.as_str(),
                    "", Option::<String>::None, true, "", "", "Absolute", "", "", Option::<String>::None, Option::<String>::None,
                ])?;
            }
            if let Some(page) = &rec.page {
                let mut custom_stmt = tx.prepare_cached(
                    "INSERT INTO custom_values (url_id, kind, idx, ord, value) VALUES (?1, ?2, ?3, ?4, ?5)",
                )?;
                for (idx, count) in page.custom_search.iter().enumerate() {
                    custom_stmt.execute(params![source_id, "search", idx as i64, 0, count.to_string()])?;
                }
                for (idx, values) in page.custom_extraction.iter().enumerate() {
                    for (ord, value) in values.iter().enumerate() {
                        custom_stmt.execute(params![source_id, "extract", idx as i64, ord as i64, value])?;
                    }
                }
                if let Some(sd) = &page.structured_data {
                    let mut item_stmt = tx.prepare_cached(
                        "INSERT INTO sd_items (url_id, format, types, properties) VALUES (?1, ?2, ?3, ?4)",
                    )?;
                    for item in &sd.items {
                        item_stmt.execute(params![source_id, item.format, item.types.join(", "), item.properties.join(", ")])?;
                    }
                    let mut issue_stmt = tx.prepare_cached(
                        "INSERT INTO sd_issues (url_id, item_type, severity, property, message, source) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    )?;
                    for e in &sd.parse_errors {
                        issue_stmt.execute(params![source_id, "", "Parse Error", "", e, toad_core::structured::SCHEMA_ORG])?;
                    }
                    for i in &sd.issues {
                        let severity = match i.severity {
                            Severity::Error => "Error",
                            Severity::Warning => "Warning",
                        };
                        issue_stmt.execute(params![source_id, i.item_type, severity, i.property, i.message, i.source])?;
                    }
                }
            }
            if let Some(page) = &rec.page {
                if !page.language_issues.is_empty() {
                    let mut stmt = tx.prepare_cached(
                        "INSERT INTO language_errors (url_id, kind, text, suggestion, rule, message, language) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    )?;
                    for i in &page.language_issues {
                        stmt.execute(params![source_id, i.kind, i.text, i.suggestion, i.rule, i.message, i.language])?;
                    }
                }
            }
            if self.cfg.extraction.cookies {
                let mut cookie_stmt = tx.prepare_cached(
                    "INSERT INTO cookies (url_id, name, value, domain, path, expires, secure, http_only, same_site) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )?;
                for (_, raw) in rec.headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie")) {
                    let c = parse_set_cookie(raw);
                    cookie_stmt.execute(params![source_id, c.name, c.value, c.domain, c.path, c.expires, c.secure, c.http_only, c.same_site])?;
                }
            }
            if let Some(text) = &rec.pdf_text {
                tx.prepare_cached("INSERT OR REPLACE INTO pdf_text (url_id, body) VALUES (?1, ?2)")?
                    .execute(params![source_id, compress(text.as_bytes())?])?;
            }
            if let Some(info) = &rec.rendered {
                let mut console_stmt =
                    tx.prepare_cached("INSERT INTO console_log (url_id, ord, level, source, message) VALUES (?1, ?2, ?3, ?4, ?5)")?;
                for (ord, m) in info.console.iter().enumerate() {
                    console_stmt.execute(params![source_id, ord as i64, m.level, m.source, m.message])?;
                }
                let mut blocked_stmt = tx.prepare_cached("INSERT INTO blocked_resources (url_id, resource) VALUES (?1, ?2)")?;
                for b in &info.blocked_resources {
                    blocked_stmt.execute(params![source_id, b])?;
                }
                let mut js_stmt = tx.prepare_cached("INSERT INTO custom_values (url_id, kind, idx, ord, value) VALUES (?1, ?2, ?3, ?4, ?5)")?;
                for (idx, res) in info.custom_js.iter().enumerate() {
                    for (ord, value) in res.values.iter().enumerate() {
                        js_stmt.execute(params![source_id, "js", idx as i64, ord as i64, value])?;
                    }
                    if let Some(e) = &res.error {
                        js_stmt.execute(params![source_id, "js_error", idx as i64, 0, e])?;
                    }
                }
                let mut a11y_stmt = tx.prepare_cached(
                    "INSERT INTO accessibility (url_id, rule, impact, guideline, help, help_url, target, html) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )?;
                for v in info.accessibility.iter().flatten() {
                    for n in &v.nodes {
                        a11y_stmt.execute(params![source_id, v.rule, v.impact, v.guideline(), v.help, v.help_url, n.target, n.html])?;
                    }
                }
                if let Some(png) = &info.screenshot {
                    tx.prepare_cached("INSERT OR REPLACE INTO screenshots (url_id, png) VALUES (?1, ?2)")?.execute(params![source_id, png])?;
                }
                if let Some(html) = &info.html {
                    tx.prepare_cached("INSERT OR REPLACE INTO rendered_html (url_id, body) VALUES (?1, ?2)")?
                        .execute(params![source_id, compress(html.as_bytes())?])?;
                }
            }
            if let Some(html) = &rec.html {
                tx.prepare_cached("INSERT OR REPLACE INTO html (url_id, body) VALUES (?1, ?2)")?
                    .execute(params![source_id, compress(html.as_bytes())?])?;
            }
            if let Some(c) = &rec.http_canonical {
                link_stmt.execute(params![
                    source_id, c, internal(c), LinkKind::HttpCanonical.as_str(),
                    "", Option::<String>::None, true, "", "", "Absolute", "", "Head", Option::<String>::None,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Record the page URLs listed in one XML sitemap.
    pub fn insert_sitemap(&mut self, sitemap: &str, urls: &[String]) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached("INSERT INTO sitemap_entries (sitemap, url) VALUES (?1, ?2)")?;
            for u in urls {
                // Normalise the same way crawled URLs are, so they join.
                let url = Url::parse(u).map(|p| normalize(p, &self.cfg).to_string()).unwrap_or_else(|_| u.clone());
                stmt.execute(params![sitemap, url])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Post-crawl analysis: link counts, Link Score, redirect chains, near duplicates.
    pub fn analyse(&mut self) -> anyhow::Result<()> {
        analysis::run(&mut self.conn, &self.cfg)?;
        self.set_meta("analysed", "true")?;
        Ok(())
    }

    /// Write a compacted copy of the crawl to `path` (File > Save As).
    pub fn save_copy(&self, path: &Path) -> anyhow::Result<()> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        self.conn.execute("VACUUM INTO ?1", params![path.to_string_lossy()])?;
        Ok(())
    }

    /// The stored HTML of a URL (View Source), if Store HTML was on.
    pub fn page_source(&self, address: &str) -> anyhow::Result<Option<String>> {
        let blob: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT h.body FROM html h JOIN urls u ON u.id = h.url_id WHERE u.address = ?1",
                params![address],
                |r| r.get(0),
            )
            .optional()?;
        blob.map(|b| decompress(&b)).transpose()
    }

    /// View Source > Rendered HTML (Store Rendered HTML).
    pub fn rendered_source(&self, address: &str) -> anyhow::Result<Option<String>> {
        let blob: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT h.body FROM rendered_html h JOIN urls u ON u.id = h.url_id WHERE u.address = ?1",
                params![address],
                |r| r.get(0),
            )
            .optional()?;
        blob.map(|b| decompress(&b)).transpose()
    }

    /// Rendered Page lower tab: the PNG screenshot.
    pub fn screenshot(&self, address: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row(
                "SELECT s.png FROM screenshots s JOIN urls u ON u.id = s.url_id WHERE u.address = ?1",
                params![address],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Chrome Console Log lower tab.
    pub fn console_log(&self, address: &str) -> anyhow::Result<Table> {
        let rows = self.query_strings(
            "SELECT c.level, c.source, c.message FROM console_log c JOIN urls u ON u.id = c.url_id WHERE u.address = ?1 ORDER BY c.ord",
            &[Value::Text(address.to_string())],
        )?;
        Ok(Table { columns: ["Level", "Source", "Message"].iter().map(|s| s.to_string()).collect(), total: rows.len() as u64, rows })
    }

    /// Accessibility Details lower tab: each failing element.
    pub fn accessibility_details(&self, address: &str) -> anyhow::Result<Table> {
        let rows = self.query_strings(
            "SELECT a.guideline, a.impact, a.rule, a.help, a.target, a.html, a.help_url
             FROM accessibility a JOIN urls u ON u.id = a.url_id WHERE u.address = ?1 ORDER BY a.rowid",
            &[Value::Text(address.to_string())],
        )?;
        Ok(Table {
            columns: ["Guideline", "Impact", "Rule", "Description", "Location", "HTML", "Help"].iter().map(|s| s.to_string()).collect(),
            total: rows.len() as u64,
            rows,
        })
    }

    /// Resources the page requested that robots.txt blocks.
    pub fn blocked_resources(&self, address: &str) -> anyhow::Result<Table> {
        let rows = self.query_strings(
            "SELECT b.resource FROM blocked_resources b JOIN urls u ON u.id = b.url_id WHERE u.address = ?1 ORDER BY b.rowid",
            &[Value::Text(address.to_string())],
        )?;
        Ok(Table { columns: vec!["Blocked Resource".to_string()], total: rows.len() as u64, rows })
    }

    /// HTTP Headers lower tab: the response headers of one URL.
    pub fn http_headers(&self, address: &str) -> anyhow::Result<Table> {
        if !self.cfg.extraction.http_headers {
            return Ok(Table { columns: vec!["Header Name".into(), "Header Value".into()], ..Default::default() });
        }
        let json: Option<String> =
            self.conn.query_row("SELECT headers_json FROM urls WHERE address = ?1", params![address], |r| r.get(0)).optional()?.flatten();
        let headers: Vec<(String, String)> = json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
        let rows: Vec<Vec<String>> = headers.into_iter().map(|(k, v)| vec![k, v]).collect();
        Ok(Table { columns: vec!["Header Name".into(), "Header Value".into()], total: rows.len() as u64, rows })
    }

    /// Cookies lower tab: the cookies one URL set.
    pub fn cookies(&self, address: &str) -> anyhow::Result<Table> {
        let rows = self.query_strings(
            "SELECT c.name, c.value, c.domain, c.path, c.expires,
                    CASE WHEN c.secure = 1 THEN 'true' ELSE 'false' END,
                    CASE WHEN c.http_only = 1 THEN 'true' ELSE 'false' END, c.same_site
             FROM cookies c JOIN urls u ON u.id = c.url_id WHERE u.address = ?1 ORDER BY c.id",
            &[Value::Text(address.to_string())],
        )?;
        Ok(Table {
            columns: ["Cookie Name", "Cookie Value", "Domain", "Path", "Expiry", "Secure", "HttpOnly", "SameSite"].iter().map(|s| s.to_string()).collect(),
            total: rows.len() as u64,
            rows,
        })
    }

    /// Spelling & Grammar Details lower tab.
    pub fn language_details(&self, address: &str) -> anyhow::Result<Table> {
        let rows = self.query_strings(
            "SELECT e.kind, e.text, e.suggestion, e.message, e.rule, e.language
             FROM language_errors e JOIN urls u ON u.id = e.url_id WHERE u.address = ?1 ORDER BY e.id",
            &[Value::Text(address.to_string())],
        )?;
        Ok(Table {
            columns: ["Type", "Error", "Suggestion", "Message", "Rule", "Language"].iter().map(|s| s.to_string()).collect(),
            total: rows.len() as u64,
            rows,
        })
    }

    /// The right-hand Spelling & Grammar panel: the top 100 errors by URLs.
    pub fn top_language_errors(&self) -> anyhow::Result<Table> {
        let rows = self.query_strings(
            "SELECT kind, text, MAX(suggestion), COUNT(DISTINCT url_id) AS n, MAX(rule)
             FROM language_errors GROUP BY kind, lower(text) ORDER BY n DESC, text LIMIT 100",
            &[],
        )?;
        Ok(Table {
            columns: ["Type", "Error", "Suggestion", "URLs", "Rule"].iter().map(|s| s.to_string()).collect(),
            total: rows.len() as u64,
            rows,
        })
    }

    /// Ignore All / Add to Dictionary: drop a word's spelling errors (or a
    /// grammar rule's errors) from this crawl and recount, without re-crawling.
    pub fn dismiss_language_error(&mut self, kind: &str, text_or_rule: &str) -> anyhow::Result<u64> {
        let tx = self.conn.transaction()?;
        let n = if kind == "Grammar" {
            tx.execute("DELETE FROM language_errors WHERE kind = 'Grammar' AND rule = ?1", params![text_or_rule])?
        } else {
            tx.execute("DELETE FROM language_errors WHERE kind = 'Spelling' AND lower(text) = lower(?1)", params![text_or_rule])?
        };
        tx.execute_batch(
            "UPDATE urls SET
                spelling_errors = CASE WHEN spelling_errors IS NULL THEN NULL ELSE (SELECT COUNT(*) FROM language_errors e WHERE e.url_id = urls.id AND e.kind = 'Spelling') END,
                grammar_errors = CASE WHEN grammar_errors IS NULL THEN NULL ELSE (SELECT COUNT(*) FROM language_errors e WHERE e.url_id = urls.id AND e.kind = 'Grammar') END;",
        )?;
        tx.commit()?;
        Ok(n as u64)
    }

    /// Bulk Export > Web > All PDF Content: one `.txt` per stored PDF, plus
    /// `index.csv`. Returns the count.
    pub fn export_pdf_text(&self, dir: &Path) -> anyhow::Result<u64> {
        std::fs::create_dir_all(dir)?;
        let mut stmt = self.conn.prepare("SELECT u.address, t.body FROM pdf_text t JOIN urls u ON u.id = t.url_id ORDER BY u.crawl_order")?;
        let mut rows = stmt.query([])?;
        let mut index = csv::Writer::from_path(dir.join("index.csv"))?;
        index.write_record(["File", "Address"])?;
        let mut n = 0u64;
        while let Some(row) = rows.next()? {
            let address: String = row.get(0)?;
            let body: Vec<u8> = row.get(1)?;
            n += 1;
            let file = format!("{n:06}.txt");
            std::fs::write(dir.join(&file), decompress(&body)?)?;
            index.write_record([file.as_str(), address.as_str()])?;
        }
        index.flush()?;
        Ok(n)
    }

    /// Whether any page source was stored in this crawl.
    pub fn has_html(&self) -> anyhow::Result<bool> {
        Ok(self.conn.query_row("SELECT EXISTS (SELECT 1 FROM html)", [], |r| r.get(0))?)
    }

    /// Bulk Export > Web > All Page Source: one `.html` file per stored page
    /// in `dir`, plus `index.csv` mapping files to addresses. Returns the count.
    pub fn export_page_source(&self, dir: &Path) -> anyhow::Result<u64> {
        std::fs::create_dir_all(dir)?;
        let mut stmt = self.conn.prepare("SELECT u.address, h.body FROM html h JOIN urls u ON u.id = h.url_id ORDER BY u.crawl_order")?;
        let mut rows = stmt.query([])?;
        let mut index = csv::Writer::from_path(dir.join("index.csv"))?;
        index.write_record(["File", "Address"])?;
        let mut n = 0u64;
        while let Some(row) = rows.next()? {
            let address: String = row.get(0)?;
            let body: Vec<u8> = row.get(1)?;
            n += 1;
            let file = format!("{n:06}.html");
            std::fs::write(dir.join(&file), decompress(&body)?)?;
            index.write_record([file.as_str(), address.as_str()])?;
        }
        index.flush()?;
        Ok(n)
    }

    pub fn url_count(&self) -> anyhow::Result<u64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM urls", [], |r| r.get::<_, i64>(0))? as u64)
    }
}

#[derive(Debug, Default, PartialEq)]
struct SetCookie {
    name: String,
    value: String,
    domain: Option<String>,
    path: Option<String>,
    expires: Option<String>,
    secure: bool,
    http_only: bool,
    same_site: Option<String>,
}

/// `name=value; Domain=x; Path=/; Expires=...; Max-Age=...; Secure; HttpOnly; SameSite=Lax`
fn parse_set_cookie(raw: &str) -> SetCookie {
    let mut parts = raw.split(';');
    let (name, value) = parts.next().unwrap_or("").split_once('=').unwrap_or((raw, ""));
    let mut c = SetCookie { name: name.trim().to_string(), value: value.trim().to_string(), ..Default::default() };
    for attr in parts {
        let (k, v) = attr.split_once('=').unwrap_or((attr, ""));
        let v = Some(v.trim().to_string()).filter(|v| !v.is_empty());
        match k.trim().to_ascii_lowercase().as_str() {
            "domain" => c.domain = v,
            "path" => c.path = v,
            "expires" => c.expires = c.expires.take().or(v),
            "max-age" => c.expires = v.map(|s| format!("Max-Age={s}")),
            "secure" => c.secure = true,
            "httponly" => c.http_only = true,
            "samesite" => c.same_site = v,
            _ => {}
        }
    }
    c
}

fn compress(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    use std::io::Write;
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(bytes)?;
    Ok(enc.finish()?)
}

fn decompress(bytes: &[u8]) -> anyhow::Result<String> {
    use std::io::Read;
    let mut out = String::new();
    flate2::read::DeflateDecoder::new(bytes).read_to_string(&mut out)?;
    Ok(out)
}

fn text(s: &str) -> Value {
    Value::Text(s.to_string())
}
fn opt(s: Option<&str>) -> Value {
    s.map(text).unwrap_or(Value::Null)
}
fn int(i: impl Into<i64>) -> Value {
    Value::Integer(i.into())
}
fn flag(b: bool) -> Value {
    Value::Integer(b as i64)
}
fn real(f: Option<f64>) -> Value {
    f.map(Value::Real).unwrap_or(Value::Null)
}
fn nth(v: &[String], i: usize) -> Value {
    opt(v.get(i).map(|s| s.as_str()))
}
fn nth_len(v: &[String], i: usize) -> Value {
    v.get(i).map(|s| int(s.chars().count() as i64)).unwrap_or(Value::Null)
}

/// Column values for one URL, always in the same order (the insert
/// statement is built from the first call and cached).
fn url_values(rec: &PageRecord, order: i64, cfg: &toad_core::CrawlConfig) -> Vec<(&'static str, Value)> {
    let x = &cfg.extraction;
    let parsed = Url::parse(&rec.url).ok();
    let path = parsed.as_ref().map(|u| u.path().to_string()).unwrap_or_default();
    let decoded_path = percent_decode(&path);
    let query_keys: Vec<String> = parsed
        .as_ref()
        .map(|u| u.query_pairs().map(|(k, _)| k.to_ascii_lowercase()).collect())
        .unwrap_or_default();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let repetitive = segments.iter().enumerate().any(|(i, s)| segments[..i].contains(s));
    let is_https = rec.url.starts_with("https://");
    let page = rec.page.as_ref();
    let rendered = rec.rendered.as_ref();
    let original = rendered.map(|r| &r.original);
    // CO2 per page view: everything the browser transferred when rendered,
    // otherwise the page's own transfer size.
    let co2 = match rendered {
        Some(r) if r.error.is_none() => Some(toad_core::render::co2_grams(r.transferred_bytes.max(rec.transferred))),
        _ if rec.is_html && rec.status_code == 200 => Some(toad_core::render::co2_grams(rec.transferred)),
        _ => None,
    };
    let empty: Vec<String> = Vec::new();
    // Spider > Extraction: elements that are switched off are stored empty.
    let titles = page.filter(|_| x.page_title).map(|p| &p.titles).unwrap_or(&empty);
    let descs = page.filter(|_| x.meta_description).map(|p| &p.meta_descriptions).unwrap_or(&empty);
    let keywords = page.filter(|_| x.meta_keywords).map(|p| &p.meta_keywords).unwrap_or(&empty);
    let h1 = page.filter(|_| x.h1).map(|p| &p.h1).unwrap_or(&empty);
    let h2 = page.filter(|_| x.h2).map(|p| &p.h2).unwrap_or(&empty);
    let robots = page.map(|p| &p.meta_robots).unwrap_or(&empty);
    let canonicals = page.map(|p| &p.canonicals).unwrap_or(&empty);
    let mut distinct_canonicals = canonicals.clone();
    distinct_canonicals.sort();
    distinct_canonicals.dedup();

    let directives: Vec<String> = robots
        .iter()
        .chain(rec.x_robots_tag.iter())
        .flat_map(|v| v.split(','))
        .map(|d| {
            let d = d.trim().to_ascii_lowercase();
            // X-Robots-Tag may be scoped: "googlebot: noindex".
            let scoped = match d.split_once(':') {
                Some((agent, rest))
                    if !agent.contains('-') && !rest.trim().is_empty() && !agent.starts_with("max") && agent != "unavailable_after" =>
                {
                    Some(rest.trim().to_string())
                }
                _ => None,
            };
            scoped.unwrap_or(d)
        })
        .map(|d| d.replace(' ', ""))
        .filter(|d| !d.is_empty())
        .collect();

    let forms = if x.forms { page.map(|p| p.form_actions.as_slice()).unwrap_or(&[]) } else { &[] };
    let hreflang_invalid = page.is_some_and(|p| p.hreflang.iter().any(|(code, _)| !valid_hreflang(code)));
    let sd = page.and_then(|p| p.structured_data.as_ref());
    let mut sd_types: Vec<String> = sd.map(|s| s.items.iter().flat_map(|i| i.types.clone()).collect()).unwrap_or_default();
    sd_types.sort();
    sd_types.dedup();
    let mut sd_formats: Vec<String> = sd.map(|s| s.items.iter().map(|i| i.format.clone()).collect()).unwrap_or_default();
    sd_formats.sort();
    sd_formats.dedup();
    let avg_wps = page.and_then(|p| (p.sentence_count > 0).then(|| (p.word_count as f64 / p.sentence_count as f64 * 10.0).round() / 10.0));

    vec![
        ("address", text(&rec.url)),
        ("crawl_order", int(order)),
        ("is_internal", flag(rec.is_internal)),
        ("depth", int(rec.depth)),
        ("status_code", int(rec.status_code)),
        ("status", text(&rec.status)),
        ("content_type", text(&rec.content_type)),
        ("is_html", flag(rec.is_html)),
        ("indexable", flag(rec.indexable)),
        ("indexability_status", text(&rec.indexability_status)),
        ("size", if x.page_size { int(rec.size as i64) } else { Value::Null }),
        ("transferred", if x.page_size { int(rec.transferred as i64) } else { Value::Null }),
        ("response_time", if x.response_time { Value::Real(rec.response_time_ms as f64 / 1000.0) } else { Value::Null }),
        ("http_version", text(&rec.http_version)),
        ("last_modified", if x.last_modified { opt(rec.last_modified.as_deref()) } else { Value::Null }),
        ("redirect_url", opt(rec.redirect_url.as_deref())),
        ("redirect_type", opt(rec.redirect_type.as_deref())),
        ("hash", if x.hash { opt(rec.hash.as_deref()) } else { Value::Null }),
        // Blocked, or (Ignore but report status) would have been blocked.
        ("blocked_robots_line", opt(rec.blocked_by_robots_line.as_deref().or(rec.robots_report_line.as_deref()))),
        ("error", opt(rec.error.as_deref())),
        // Headers are always kept: the Security columns read them. The HTTP
        // Headers toggle controls the lower tab and the header exports.
        ("headers_json", text(&serde_json::to_string(&rec.headers).unwrap_or_default())),
        ("title_1", nth(titles, 0)),
        ("title_1_length", nth_len(titles, 0)),
        ("title_1_px", titles.first().map(|t| int(pixel::title_px(t))).unwrap_or(Value::Null)),
        ("title_2", nth(titles, 1)),
        ("title_count", int(titles.len() as i64)),
        ("title_outside_head", flag(x.page_title && page.is_some_and(|p| p.title_outside_head))),
        ("meta_description_1", nth(descs, 0)),
        ("meta_description_1_length", nth_len(descs, 0)),
        ("meta_description_1_px", descs.first().map(|d| int(pixel::description_px(d))).unwrap_or(Value::Null)),
        ("meta_description_2", nth(descs, 1)),
        ("meta_description_count", int(descs.len() as i64)),
        ("description_outside_head", flag(x.meta_description && page.is_some_and(|p| p.description_outside_head))),
        ("meta_keywords_1", nth(keywords, 0)),
        ("meta_keywords_count", int(keywords.len() as i64)),
        ("h1_1", nth(h1, 0)),
        ("h1_1_length", nth_len(h1, 0)),
        ("h1_2", nth(h1, 1)),
        ("h1_2_length", nth_len(h1, 1)),
        ("h1_count", int(h1.len() as i64)),
        ("h1_alt_text", flag(x.h1 && page.is_some_and(|p| p.h1_has_alt_text))),
        ("h1_non_sequential", flag(x.h1 && page.is_some_and(|p| p.h1_non_sequential))),
        ("h2_1", nth(h2, 0)),
        ("h2_1_length", nth_len(h2, 0)),
        ("h2_2", nth(h2, 1)),
        ("h2_2_length", nth_len(h2, 1)),
        ("h2_count", int(h2.len() as i64)),
        ("h2_non_sequential", flag(x.h2 && page.is_some_and(|p| p.h2_non_sequential))),
        ("meta_robots_1", nth(robots, 0)),
        ("meta_robots_2", nth(robots, 1)),
        ("x_robots_tag_1", nth(&rec.x_robots_tag, 0)),
        ("x_robots_tag_2", nth(&rec.x_robots_tag, 1)),
        ("directives", if directives.is_empty() { Value::Null } else { text(&format!(",{},", directives.join(","))) }),
        ("robots_outside_head", flag(page.is_some_and(|p| p.robots_outside_head))),
        ("meta_refresh", opt(page.and_then(|p| p.meta_refresh.as_deref()))),
        ("canonical_1", nth(canonicals, 0)),
        ("canonical_count", int(canonicals.len() as i64)),
        ("canonical_distinct", int(distinct_canonicals.len() as i64)),
        ("http_canonical", opt(rec.http_canonical.as_deref())),
        ("canonical_outside_head", flag(page.is_some_and(|p| p.canonical_outside_head))),
        ("canonical_relative", flag(page.is_some_and(|p| p.canonical_is_relative))),
        ("canonical_fragment", flag(page.is_some_and(|p| p.canonical_has_fragment))),
        ("canonical_invalid_attr", flag(page.is_some_and(|p| p.canonical_invalid_attribute))),
        ("rel_next", page.map(|p| nth(&p.rel_next, 0)).unwrap_or(Value::Null)),
        ("rel_prev", page.map(|p| nth(&p.rel_prev, 0)).unwrap_or(Value::Null)),
        ("rel_next_count", int(page.map_or(0, |p| p.rel_next.len() as i64))),
        ("rel_prev_count", int(page.map_or(0, |p| p.rel_prev.len() as i64))),
        ("hreflang_count", int(page.map_or(0, |p| p.hreflang.len() as i64))),
        ("hreflang_outside_head", flag(page.is_some_and(|p| p.hreflang_outside_head))),
        ("hreflang_invalid", flag(hreflang_invalid)),
        ("language", opt(page.and_then(|p| p.language.as_deref()))),
        ("has_viewport", flag(page.is_some_and(|p| p.has_viewport))),
        ("word_count", page.filter(|_| x.word_count).map(|p| int(p.word_count)).unwrap_or(Value::Null)),
        ("sentence_count", page.filter(|_| x.word_count).map(|p| int(p.sentence_count)).unwrap_or(Value::Null)),
        ("avg_words_per_sentence", real(avg_wps.filter(|_| x.word_count))),
        ("flesch", real(page.and_then(|p| p.flesch_reading_ease).filter(|_| x.readability))),
        ("text_ratio", real(page.map(|p| p.text_ratio).filter(|_| x.text_to_code_ratio))),
        ("lorem_ipsum", flag(page.is_some_and(|p| p.lorem_ipsum))),
        ("ajax_fragment_meta", flag(page.is_some_and(|p| p.ajax_fragment_meta))),
        ("a11y_checked", flag(rendered.is_some_and(|r| r.accessibility.is_some()))),
        ("head_tags", page.map(|p| int(p.head_tags)).unwrap_or(Value::Null)),
        ("body_tags", page.map(|p| int(p.body_tags)).unwrap_or(Value::Null)),
        ("hsts", opt(rec.header("strict-transport-security"))),
        ("csp", opt(rec.header("content-security-policy"))),
        ("x_content_type_options", opt(rec.header("x-content-type-options"))),
        ("x_frame_options", opt(rec.header("x-frame-options"))),
        ("referrer_policy", opt(rec.header("referrer-policy"))),
        ("form_on_http", flag(!is_https && !forms.is_empty())),
        ("form_action_insecure", flag(forms.iter().any(|a| a.starts_with("http://")))),
        ("url_length", int(rec.url.chars().count() as i64)),
        ("folder_depth", parsed.as_ref().map(|u| int(folder_depth(u))).unwrap_or(Value::Null)),
        ("query_params", parsed.as_ref().map(|u| int(query_param_count(u))).unwrap_or(Value::Null)),
        ("url_non_ascii", flag(!decoded_path.is_ascii())),
        ("url_underscores", flag(path.contains('_'))),
        ("url_uppercase", flag(path.chars().any(|c| c.is_ascii_uppercase()))),
        ("url_multiple_slashes", flag(path.contains("//"))),
        ("url_repetitive_path", flag(repetitive)),
        ("url_space", flag(decoded_path.contains(' ') || rec.url.contains("%20"))),
        ("url_internal_search", flag(path.to_ascii_lowercase().contains("/search") || query_keys.iter().any(|k| ["q", "s", "search", "query", "keyword"].contains(&k.as_str())))),
        ("url_ga_params", flag(query_keys.iter().any(|k| k.starts_with("utm_") || k == "_ga" || k == "_gl"))),
        ("sitemap_url_count", rec.sitemap_url_count.map(|n| int(n as i64)).unwrap_or(Value::Null)),
        ("sd_count", int(sd.map_or(0, |s| s.items.len() as i64))),
        ("sd_types", if sd_types.is_empty() { Value::Null } else { text(&sd_types.join(", ")) }),
        ("sd_formats", if sd_formats.is_empty() { Value::Null } else { text(&sd_formats.join(", ")) }),
        ("sd_errors", int(sd_count(sd, Severity::Error, toad_core::structured::SCHEMA_ORG))),
        ("sd_warnings", int(sd_count(sd, Severity::Warning, toad_core::structured::SCHEMA_ORG))),
        ("sd_rich_errors", int(sd_count(sd, Severity::Error, toad_core::structured::GOOGLE))),
        ("sd_rich_warnings", int(sd_count(sd, Severity::Warning, toad_core::structured::GOOGLE))),
        ("sd_parse_errors", int(sd.map_or(0, |s| s.parse_errors.len() as i64))),
        ("pdf_title", opt(rec.pdf.as_ref().and_then(|p| p.title.as_deref()))),
        ("pdf_subject", opt(rec.pdf.as_ref().and_then(|p| p.subject.as_deref()))),
        ("pdf_author", opt(rec.pdf.as_ref().and_then(|p| p.author.as_deref()))),
        ("pdf_keywords", opt(rec.pdf.as_ref().and_then(|p| p.keywords.as_deref()))),
        ("pdf_created", opt(rec.pdf.as_ref().and_then(|p| p.created.as_deref()))),
        ("pdf_modified", opt(rec.pdf.as_ref().and_then(|p| p.modified.as_deref()))),
        ("pdf_pages", rec.pdf.as_ref().map(|p| int(p.pages as i64)).unwrap_or(Value::Null)),
        ("rendered", flag(rendered.is_some_and(|r| r.error.is_none()))),
        ("render_error", opt(rendered.and_then(|r| r.error.as_deref()))),
        ("orig_title", opt(original.and_then(|o| o.title.as_deref()))),
        ("orig_meta_description", opt(original.and_then(|o| o.meta_description.as_deref()))),
        ("orig_h1", opt(original.and_then(|o| o.h1.as_deref()))),
        ("orig_canonical", opt(original.and_then(|o| o.canonical.as_deref()))),
        ("orig_directives", opt(original.map(|o| o.directives.as_str()))),
        ("orig_word_count", original.map(|o| int(o.word_count)).unwrap_or(Value::Null)),
        ("orig_link_count", original.map(|o| int(o.link_count)).unwrap_or(Value::Null)),
        ("js_link_count", if rendered.is_some() {
            int(page.map_or(0, |p| p.links.iter().filter(|l| l.origin == "Rendered HTML").count() as i64))
        } else {
            Value::Null
        }),
        ("js_errors", if rendered.is_some() && cfg.rendering.js_error_reporting {
            int(rendered.map_or(0, |r| r.console.iter().filter(|m| m.level == "error").count() as i64))
        } else {
            Value::Null
        }),
        ("js_warnings", if rendered.is_some() && cfg.rendering.js_error_reporting {
            int(rendered.map_or(0, |r| r.console.iter().filter(|m| m.level == "warning").count() as i64))
        } else {
            Value::Null
        }),
        ("blocked_resources", rendered.map(|r| int(r.blocked_resources.len() as i64)).unwrap_or(Value::Null)),
        ("co2_mg", co2.map(|g| real(Some((g * 1000.0 * 100.0).round() / 100.0))).unwrap_or(Value::Null)),
        ("carbon_rating", opt(co2.map(toad_core::render::carbon_rating))),
        ("spelling_errors", if cfg.spelling.spell_check && page.is_some() {
            int(page.map_or(0, |p| p.language_issues.iter().filter(|i| i.kind == "Spelling").count() as i64))
        } else {
            Value::Null
        }),
        ("grammar_errors", if cfg.spelling.grammar_check && page.is_some() {
            int(page.map_or(0, |p| p.language_issues.iter().filter(|i| i.kind == "Grammar").count() as i64))
        } else {
            Value::Null
        }),
        ("minhash", match page {
            Some(p) if !p.minhash.is_empty() => Value::Blob(minhash::to_bytes(&p.minhash)),
            _ => Value::Null,
        }),
    ]
}

fn sd_count(sd: Option<&toad_core::structured::StructuredData>, severity: Severity, source: &str) -> i64 {
    sd.map_or(0, |s| s.issues.iter().filter(|i| i.severity == severity && i.source == source).count() as i64)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// ISO 639-1 language (2 letters), optional script (4 letters), optional
/// ISO 3166-1 alpha-2 region; or `x-default`.
fn valid_hreflang(code: &str) -> bool {
    let code = code.trim().to_ascii_lowercase();
    if code == "x-default" {
        return true;
    }
    let parts: Vec<&str> = code.split('-').collect();
    let alpha = |s: &str, n: usize| s.len() == n && s.chars().all(|c| c.is_ascii_alphabetic());
    match parts.as_slice() {
        [lang] => alpha(lang, 2),
        [lang, region] => alpha(lang, 2) && (alpha(region, 2) || alpha(region, 4)),
        [lang, script, region] => alpha(lang, 2) && alpha(script, 4) && alpha(region, 2),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hreflang_codes() {
        assert!(valid_hreflang("en"));
        assert!(valid_hreflang("en-GB"));
        assert!(valid_hreflang("x-default"));
        assert!(valid_hreflang("zh-Hant-TW"));
        assert!(!valid_hreflang("en-UK1"));
        assert!(!valid_hreflang("english"));
    }

    #[test]
    fn parses_set_cookie() {
        let c = parse_set_cookie("sid=abc123; Path=/; Domain=.example.com; Max-Age=3600; Secure; HttpOnly; SameSite=Lax");
        assert_eq!((c.name.as_str(), c.value.as_str()), ("sid", "abc123"));
        assert_eq!((c.domain.as_deref(), c.path.as_deref()), (Some(".example.com"), Some("/")));
        assert_eq!(c.expires.as_deref(), Some("Max-Age=3600"));
        assert!(c.secure && c.http_only);
        assert_eq!(c.same_site.as_deref(), Some("Lax"));
        assert_eq!(parse_set_cookie("flag").name, "flag");
    }

    #[test]
    fn old_crawl_files_gain_new_columns() {
        let path = std::env::temp_dir().join(format!("toad-migrate-{}.toad", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(schema::SCHEMA).unwrap();
            for (name, _) in schema::ADDED_URL_COLUMNS {
                conn.execute_batch(&format!("ALTER TABLE urls DROP COLUMN {name}")).unwrap();
            }
        }
        let store = Store::open(&path).unwrap();
        let cols: Vec<String> = store.conn.prepare("SELECT name FROM pragma_table_info('urls')").unwrap().query_map([], |r| r.get(0)).unwrap().map(|c| c.unwrap()).collect();
        assert!(cols.iter().any(|c| c == "spelling_errors"));
    }

    #[test]
    fn decodes_percent() {
        assert_eq!(percent_decode("/caf%C3%A9/a%20b"), "/café/a b");
        assert_eq!(percent_decode("/100%"), "/100%");
    }
}
