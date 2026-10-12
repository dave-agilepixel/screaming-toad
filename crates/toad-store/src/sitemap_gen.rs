//! Sitemaps > XML Sitemap: generate sitemaps from a crawl.

use crate::Store;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LastMod {
    #[default]
    None,
    /// From the Last-Modified response header.
    ServerResponse,
    /// The same W3C date (YYYY-MM-DD) for every URL.
    Custom(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeFreq {
    #[default]
    None,
    /// Daily if Last-Modified is within 24 hours, otherwise monthly.
    FromLastModified,
    /// By crawl depth: 0–1 daily, 2 weekly, 3+ monthly.
    ByDepth,
}

/// Defaults match Screaming Frog: 200-status HTML pages only, with noindex,
/// canonicalised and paginated pages and PDFs excluded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SitemapOptions {
    pub include_noindex: bool,
    pub include_canonicalised: bool,
    pub include_paginated: bool,
    pub include_pdfs: bool,
    pub last_mod: LastMod,
    /// Priority by crawl depth: 1.0, 0.9, 0.8, 0.7, 0.6, then 0.5.
    pub priority: bool,
    pub change_freq: ChangeFreq,
    pub max_urls_per_file: usize,
}

impl Default for SitemapOptions {
    fn default() -> Self {
        Self {
            include_noindex: false,
            include_canonicalised: false,
            include_paginated: false,
            include_pdfs: false,
            last_mod: LastMod::None,
            priority: false,
            change_freq: ChangeFreq::None,
            max_urls_per_file: 49_999,
        }
    }
}

struct Entry {
    loc: String,
    depth: u32,
    last_modified: Option<String>,
}

impl Store {
    /// Write `sitemap.xml` into `dir`, or `sitemap1.xml`… plus
    /// `sitemap_index.xml` when there are more URLs than fit in one file.
    /// Returns the files written.
    pub fn generate_sitemap(&self, dir: &Path, opts: &SitemapOptions) -> anyhow::Result<Vec<PathBuf>> {
        let mut conditions = vec![
            "is_internal = 1".to_string(),
            "status_code = 200".to_string(),
            "blocked_robots_line IS NULL".to_string(),
        ];
        conditions.push(if opts.include_pdfs {
            "(is_html = 1 OR content_type LIKE '%pdf%')".into()
        } else {
            "is_html = 1".into()
        });
        if !opts.include_noindex {
            conditions.push("coalesce(directives, '') NOT LIKE '%,noindex,%' AND coalesce(directives, '') NOT LIKE '%,none,%'".into());
        }
        if !opts.include_canonicalised {
            conditions.push("(coalesce(canonical_1, http_canonical) IS NULL OR coalesce(canonical_1, http_canonical) = address)".into());
        }
        if !opts.include_paginated {
            conditions.push("rel_prev IS NULL".into());
        }
        let sql = format!(
            "SELECT address, depth, last_modified FROM urls WHERE {} ORDER BY depth, crawl_order",
            conditions.join(" AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let entries: Vec<Entry> = stmt
            .query_map([], |r| {
                Ok(Entry { loc: r.get(0)?, depth: r.get::<_, Option<i64>>(1)?.unwrap_or(0) as u32, last_modified: r.get(2)? })
            })?
            .collect::<Result<_, _>>()?;

        std::fs::create_dir_all(dir)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let per_file = opts.max_urls_per_file.clamp(1, 50_000);
        let chunks: Vec<&[Entry]> = entries.chunks(per_file).collect();
        let mut written = Vec::new();

        if chunks.len() <= 1 {
            let path = dir.join("sitemap.xml");
            std::fs::write(&path, urlset(chunks.first().copied().unwrap_or(&[]), opts, now))?;
            written.push(path);
            return Ok(written);
        }

        // The index refers to the child files where the sitemap will be hosted:
        // the root of the crawled site.
        let base = self
            .meta("start_url")?
            .and_then(|u| Url::parse(&u).ok())
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_default();
        let mut index = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        for (i, chunk) in chunks.iter().enumerate() {
            let name = format!("sitemap{}.xml", i + 1);
            let path = dir.join(&name);
            std::fs::write(&path, urlset(chunk, opts, now))?;
            written.push(path);
            let _ = writeln!(index, "  <sitemap>\n    <loc>{}</loc>\n  </sitemap>", escape(&format!("{base}/{name}")));
        }
        index.push_str("</sitemapindex>\n");
        let path = dir.join("sitemap_index.xml");
        std::fs::write(&path, index)?;
        written.push(path);
        Ok(written)
    }
}

fn urlset(entries: &[Entry], opts: &SitemapOptions, now: i64) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    for e in entries {
        out.push_str("  <url>\n");
        let _ = writeln!(out, "    <loc>{}</loc>", escape(&e.loc));
        let modified = e.last_modified.as_deref().and_then(parse_http_date);
        match &opts.last_mod {
            LastMod::None => {}
            LastMod::ServerResponse => {
                if let Some(secs) = modified {
                    let _ = writeln!(out, "    <lastmod>{}</lastmod>", w3c_date(secs));
                }
            }
            LastMod::Custom(date) => {
                let _ = writeln!(out, "    <lastmod>{}</lastmod>", escape(date));
            }
        }
        let freq = match opts.change_freq {
            ChangeFreq::None => None,
            ChangeFreq::FromLastModified => Some(match modified {
                Some(secs) if now - secs < 86_400 => "daily",
                _ => "monthly",
            }),
            ChangeFreq::ByDepth => Some(match e.depth {
                0 | 1 => "daily",
                2 => "weekly",
                _ => "monthly",
            }),
        };
        if let Some(f) = freq {
            let _ = writeln!(out, "    <changefreq>{f}</changefreq>");
        }
        if opts.priority {
            let p = 1.0 - 0.1 * e.depth.min(5) as f64;
            let _ = writeln!(out, "    <priority>{p:.1}</priority>");
        }
        out.push_str("  </url>\n");
    }
    out.push_str("</urlset>\n");
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// `Tue, 15 Nov 1994 08:12:31 GMT` → Unix seconds.
fn parse_http_date(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 5 {
        return None;
    }
    let day: i64 = parts[1].parse().ok()?;
    let month = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"]
        .iter()
        .position(|m| parts[2].to_ascii_lowercase().starts_with(m))? as i64
        + 1;
    let year: i64 = parts[3].parse().ok()?;
    let hms: Vec<i64> = parts[4].split(':').filter_map(|p| p.parse().ok()).collect();
    if hms.len() != 3 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hms[0] * 3600 + hms[1] * 60 + hms[2])
}

/// Days since 1970-01-01 (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn w3c_date(secs: i64) -> String {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_dates() {
        let secs = parse_http_date("Tue, 15 Nov 1994 08:12:31 GMT").unwrap();
        assert_eq!(secs, 784_887_151);
        assert_eq!(w3c_date(secs), "1994-11-15");
        assert_eq!(w3c_date(0), "1970-01-01");
        assert_eq!(parse_http_date("garbage"), None);
    }
}
