//! Crawl store: one SQLite file per crawl (`.toad`), holding every URL,
//! every link, and the configuration the crawl ran with.

mod analysis;
mod query;
mod reports;
mod schema;
mod sitemap_gen;
pub mod tabs;

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
            let values = url_values(rec, order);
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
                "INSERT INTO links (source_id, target, internal, kind, anchor, alt, follow, rel, target_attr, path_type, link_path, position, hreflang)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )?;
            let internal = |target: &str| {
                Url::parse(target).ok().zip(self.scope.as_ref()).is_some_and(|(u, s)| s.is_internal(&u))
            };
            if let Some(page) = &rec.page {
                for l in &page.links {
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
                    ])?;
                }
            }
            if let (Some(target), Some("HTTP Redirect")) = (&rec.redirect_url, rec.redirect_type.as_deref()) {
                link_stmt.execute(params![
                    source_id, target, internal(target), LinkKind::Redirect.as_str(),
                    "", Option::<String>::None, true, "", "", "Absolute", "", "", Option::<String>::None,
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
                        "INSERT INTO sd_issues (url_id, item_type, severity, property, message) VALUES (?1, ?2, ?3, ?4, ?5)",
                    )?;
                    for e in &sd.parse_errors {
                        issue_stmt.execute(params![source_id, "", "Parse Error", "", e])?;
                    }
                    for i in &sd.issues {
                        let severity = match i.severity {
                            Severity::Error => "Error",
                            Severity::Warning => "Warning",
                        };
                        issue_stmt.execute(params![source_id, i.item_type, severity, i.property, i.message])?;
                    }
                }
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

    pub fn url_count(&self) -> anyhow::Result<u64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM urls", [], |r| r.get::<_, i64>(0))? as u64)
    }
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
fn url_values(rec: &PageRecord, order: i64) -> Vec<(&'static str, Value)> {
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
    let empty: Vec<String> = Vec::new();
    let titles = page.map(|p| &p.titles).unwrap_or(&empty);
    let descs = page.map(|p| &p.meta_descriptions).unwrap_or(&empty);
    let keywords = page.map(|p| &p.meta_keywords).unwrap_or(&empty);
    let h1 = page.map(|p| &p.h1).unwrap_or(&empty);
    let h2 = page.map(|p| &p.h2).unwrap_or(&empty);
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

    let forms = page.map(|p| p.form_actions.as_slice()).unwrap_or(&[]);
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
        ("size", int(rec.size as i64)),
        ("transferred", int(rec.transferred as i64)),
        ("response_time", Value::Real(rec.response_time_ms as f64 / 1000.0)),
        ("http_version", text(&rec.http_version)),
        ("last_modified", opt(rec.last_modified.as_deref())),
        ("redirect_url", opt(rec.redirect_url.as_deref())),
        ("redirect_type", opt(rec.redirect_type.as_deref())),
        ("hash", opt(rec.hash.as_deref())),
        ("blocked_robots_line", opt(rec.blocked_by_robots_line.as_deref())),
        ("error", opt(rec.error.as_deref())),
        ("headers_json", text(&serde_json::to_string(&rec.headers).unwrap_or_default())),
        ("title_1", nth(titles, 0)),
        ("title_1_length", nth_len(titles, 0)),
        ("title_1_px", titles.first().map(|t| int(pixel::title_px(t))).unwrap_or(Value::Null)),
        ("title_2", nth(titles, 1)),
        ("title_count", int(titles.len() as i64)),
        ("title_outside_head", flag(page.is_some_and(|p| p.title_outside_head))),
        ("meta_description_1", nth(descs, 0)),
        ("meta_description_1_length", nth_len(descs, 0)),
        ("meta_description_1_px", descs.first().map(|d| int(pixel::description_px(d))).unwrap_or(Value::Null)),
        ("meta_description_2", nth(descs, 1)),
        ("meta_description_count", int(descs.len() as i64)),
        ("description_outside_head", flag(page.is_some_and(|p| p.description_outside_head))),
        ("meta_keywords_1", nth(keywords, 0)),
        ("meta_keywords_count", int(keywords.len() as i64)),
        ("h1_1", nth(h1, 0)),
        ("h1_1_length", nth_len(h1, 0)),
        ("h1_2", nth(h1, 1)),
        ("h1_2_length", nth_len(h1, 1)),
        ("h1_count", int(h1.len() as i64)),
        ("h1_alt_text", flag(page.is_some_and(|p| p.h1_has_alt_text))),
        ("h1_non_sequential", flag(page.is_some_and(|p| p.h1_non_sequential))),
        ("h2_1", nth(h2, 0)),
        ("h2_1_length", nth_len(h2, 0)),
        ("h2_2", nth(h2, 1)),
        ("h2_2_length", nth_len(h2, 1)),
        ("h2_count", int(h2.len() as i64)),
        ("h2_non_sequential", flag(page.is_some_and(|p| p.h2_non_sequential))),
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
        ("word_count", page.map(|p| int(p.word_count)).unwrap_or(Value::Null)),
        ("sentence_count", page.map(|p| int(p.sentence_count)).unwrap_or(Value::Null)),
        ("avg_words_per_sentence", real(avg_wps)),
        ("flesch", real(page.and_then(|p| p.flesch_reading_ease))),
        ("text_ratio", real(page.map(|p| p.text_ratio))),
        ("lorem_ipsum", flag(page.is_some_and(|p| p.lorem_ipsum))),
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
        ("sd_errors", int(sd.map_or(0, |s| s.issues.iter().filter(|i| i.severity == Severity::Error).count() as i64))),
        ("sd_warnings", int(sd.map_or(0, |s| s.issues.iter().filter(|i| i.severity == Severity::Warning).count() as i64))),
        ("sd_parse_errors", int(sd.map_or(0, |s| s.parse_errors.len() as i64))),
        ("minhash", match page {
            Some(p) if !p.minhash.is_empty() => Value::Blob(minhash::to_bytes(&p.minhash)),
            _ => Value::Null,
        }),
    ]
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
    fn decodes_percent() {
        assert_eq!(percent_decode("/caf%C3%A9/a%20b"), "/café/a b");
        assert_eq!(percent_decode("/100%"), "/100%");
    }
}
