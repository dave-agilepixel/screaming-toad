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
    /// By crawl depth, from `change_freq_by_depth`.
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
    /// Pages > Response Codes: 2xx are always included; these add other
    /// status classes (pages that return them are still crawled URLs).
    pub include_3xx: bool,
    pub include_4xx: bool,
    pub include_5xx: bool,
    pub last_mod: LastMod,
    /// Priority by crawl depth (`priority_by_depth`, depth 0–5+).
    pub priority: bool,
    pub priority_by_depth: [f64; 6],
    pub change_freq: ChangeFreq,
    /// Change Frequency by crawl depth 0–5+ (always, hourly, daily, weekly,
    /// monthly, yearly or never).
    pub change_freq_by_depth: [String; 6],
    pub max_urls_per_file: usize,
    /// One sitemap per segment (Configuration > Segments), plus an index.
    pub by_segment: bool,
    /// Images: list each page's images in the sitemap (`image:image`).
    pub include_images: bool,
    pub images: ImageSitemapOptions,
    /// Hreflang: each page's alternates (`xhtml:link rel="alternate"`).
    pub include_hreflang: bool,
}

impl Default for SitemapOptions {
    fn default() -> Self {
        Self {
            include_noindex: false,
            include_canonicalised: false,
            include_paginated: false,
            include_pdfs: false,
            include_3xx: false,
            include_4xx: false,
            include_5xx: false,
            last_mod: LastMod::ServerResponse,
            priority: true,
            priority_by_depth: [1.0, 0.9, 0.8, 0.7, 0.6, 0.5],
            change_freq: ChangeFreq::FromLastModified,
            change_freq_by_depth: ["daily", "daily", "weekly", "monthly", "monthly", "monthly"].map(String::from),
            max_urls_per_file: 49_999,
            by_segment: false,
            include_images: false,
            images: ImageSitemapOptions::default(),
            include_hreflang: false,
        }
    }
}

struct Entry {
    id: i64,
    loc: String,
    depth: u32,
    last_modified: Option<String>,
    images: Vec<String>,
    /// (hreflang, href)
    alternates: Vec<(String, String)>,
}

impl Store {
    /// Write `sitemap.xml` into `dir`, or `sitemap1.xml`… plus
    /// `sitemap_index.xml` when there are more URLs than fit in one file.
    /// Returns the files written.
    pub fn generate_sitemap(&self, dir: &Path, opts: &SitemapOptions) -> anyhow::Result<Vec<PathBuf>> {
        let mut classes = vec!["status_code BETWEEN 200 AND 299".to_string()];
        if opts.include_3xx {
            classes.push("status_code BETWEEN 300 AND 399".into());
        }
        if opts.include_4xx {
            classes.push("status_code BETWEEN 400 AND 499".into());
        }
        if opts.include_5xx {
            classes.push("status_code >= 500".into());
        }
        let mut conditions = vec!["is_internal = 1".to_string(), format!("({})", classes.join(" OR ")), "blocked_robots_line IS NULL".to_string()];
        // Non-2xx responses aren't HTML pages as such; keep them by status.
        let html_or_status = if opts.include_3xx || opts.include_4xx || opts.include_5xx { " OR status_code >= 300" } else { "" };
        conditions.push(if opts.include_pdfs {
            format!("(is_html = 1 OR content_type LIKE '%pdf%'{html_or_status})")
        } else {
            format!("(is_html = 1{html_or_status})")
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
        let segment = match crate::segments::case_sql(self.config()) {
            Some(case) if opts.by_segment => case,
            _ => "NULL".into(),
        };
        let sql = format!(
            "SELECT address, depth, last_modified, {segment}, id FROM urls u WHERE {} ORDER BY depth, crawl_order",
            conditions.join(" AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows: Vec<(Entry, Option<String>)> = stmt
            .query_map([], |r| {
                Ok((
                    Entry {
                        id: r.get(4)?,
                        loc: r.get(0)?,
                        depth: r.get::<_, Option<i64>>(1)?.unwrap_or(0) as u32,
                        last_modified: r.get(2)?,
                        images: vec![],
                        alternates: vec![],
                    },
                    r.get(3)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        if opts.include_images {
            let mut by_page = self.page_images(&opts.images, None)?;
            for (e, _) in &mut rows {
                e.images = by_page.remove(&e.id).unwrap_or_default();
            }
        }
        if opts.include_hreflang {
            let mut by_page = self.page_alternates()?;
            for (e, _) in &mut rows {
                e.alternates = by_page.remove(&e.id).unwrap_or_default();
            }
        }
        if opts.by_segment && rows.iter().any(|(_, s)| s.is_some()) {
            return self.sitemaps_by_segment(dir, opts, rows);
        }
        let entries: Vec<Entry> = rows.into_iter().map(|(e, _)| e).collect();

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

    /// One sitemap (or several, past the per-file limit) per segment, in
    /// configuration order, with URLs in no segment in sitemap-other.xml,
    /// and an index listing them all.
    fn sitemaps_by_segment(&self, dir: &Path, opts: &SitemapOptions, rows: Vec<(Entry, Option<String>)>) -> anyhow::Result<Vec<PathBuf>> {
        std::fs::create_dir_all(dir)?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let base = self
            .meta("start_url")?
            .and_then(|u| Url::parse(&u).ok())
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_default();
        let per_file = opts.max_urls_per_file.clamp(1, 50_000);
        let mut groups: Vec<(String, Vec<Entry>)> = self.config().segments.iter().map(|s| (s.name.clone(), Vec::new())).collect();
        groups.push((String::new(), Vec::new()));
        for (entry, seg) in rows {
            let key = seg.unwrap_or_default();
            if let Some(g) = groups.iter_mut().find(|(n, _)| *n == key) {
                g.1.push(entry);
            }
        }
        let mut written = Vec::new();
        let mut index = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        let mut used = std::collections::HashSet::new();
        for (name, entries) in groups.iter().filter(|(_, e)| !e.is_empty()) {
            let mut slug = if name.is_empty() { "other".to_string() } else { slugify(name) };
            while !used.insert(slug.clone()) {
                slug.push('-');
            }
            let chunks: Vec<&[Entry]> = entries.chunks(per_file).collect();
            for (i, chunk) in chunks.iter().enumerate() {
                let file = if chunks.len() == 1 { format!("sitemap-{slug}.xml") } else { format!("sitemap-{slug}-{}.xml", i + 1) };
                let path = dir.join(&file);
                std::fs::write(&path, urlset(chunk, opts, now))?;
                written.push(path);
                let _ = writeln!(index, "  <sitemap>\n    <loc>{}</loc>\n  </sitemap>", escape(&format!("{base}/{file}")));
            }
        }
        index.push_str("</sitemapindex>\n");
        let path = dir.join("sitemap_index.xml");
        std::fs::write(&path, index)?;
        written.push(path);
        Ok(written)
    }
}

impl Store {
    /// Each page's images that pass the image options, by page id.
    fn page_images(&self, opts: &ImageSitemapOptions, _limit: Option<usize>) -> anyhow::Result<std::collections::HashMap<i64, Vec<String>>> {
        let cdn = if opts.cdn_regex.trim().is_empty() { None } else { Some(regex::Regex::new(opts.cdn_regex.trim())?) };
        let mut stmt = self.conn.prepare(
            "SELECT l.source_id, l.target, l.internal, coalesce(i.status_code, 0),
                    (SELECT COUNT(DISTINCT l2.source_id) FROM links l2 WHERE l2.target = l.target AND l2.kind = 'Image'),
                    coalesce(i.directives, '')
             FROM links l LEFT JOIN urls i ON i.address = l.target
             WHERE l.kind = 'Image' ORDER BY l.source_id, l.id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, bool>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?))
        })?;
        let mut out: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
        for row in rows {
            let (page, image, internal, status, inlinks, directives) = row?;
            if !internal && !cdn.as_ref().is_some_and(|re| re.is_match(&image)) {
                continue;
            }
            if !opts.include_non_200 && status != 200 {
                continue;
            }
            if !opts.include_noindex && (directives.contains(",noindex,") || directives.contains(",none,")) {
                continue;
            }
            if opts.max_inlinks.is_some_and(|m| inlinks > m as i64) {
                continue;
            }
            let v = out.entry(page).or_default();
            // The protocol allows 1,000 images per page.
            if v.len() < 1000 && !v.contains(&image) {
                v.push(image);
            }
        }
        Ok(out)
    }

    /// Each page's hreflang alternates (hreflang, href), by page id.
    fn page_alternates(&self) -> anyhow::Result<std::collections::HashMap<i64, Vec<(String, String)>>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_id, hreflang, target FROM links WHERE kind = 'Hreflang' AND hreflang IS NOT NULL AND hreflang <> '' ORDER BY source_id, id",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        let mut out: std::collections::HashMap<i64, Vec<(String, String)>> = std::collections::HashMap::new();
        for row in rows {
            let (page, lang, href) = row?;
            let v = out.entry(page).or_default();
            if !v.iter().any(|(l, h)| *l == lang && *h == href) {
                v.push((lang, href));
            }
        }
        Ok(out)
    }
}

/// Sitemaps > Images Sitemap options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageSitemapOptions {
    /// Leave out images linked from more than this many pages (logos,
    /// icons). None: keep all.
    pub max_inlinks: Option<u32>,
    /// Images on other hosts (CDNs) are included when their URL matches
    /// this regex; internal images are always included.
    pub cdn_regex: String,
    /// Include images that didn't return 200 (or weren't crawled).
    pub include_non_200: bool,
    /// Include images that are noindex (X-Robots-Tag).
    pub include_noindex: bool,
    pub max_urls_per_file: usize,
}

impl Default for ImageSitemapOptions {
    fn default() -> Self {
        Self { max_inlinks: Some(10), cdn_regex: String::new(), include_non_200: false, include_noindex: false, max_urls_per_file: 49_999 }
    }
}

impl Store {
    /// Sitemaps > Images Sitemap: the pages a page sitemap would list, each
    /// with its images (`image:image`), skipping pages without images.
    pub fn generate_images_sitemap(&self, dir: &Path, page_opts: &SitemapOptions, opts: &ImageSitemapOptions) -> anyhow::Result<Vec<PathBuf>> {
        let mut conditions = vec!["p.is_internal = 1".to_string(), "p.status_code = 200".to_string(), "p.is_html = 1".to_string()];
        if !page_opts.include_noindex {
            conditions.push("coalesce(p.directives, '') NOT LIKE '%,noindex,%' AND coalesce(p.directives, '') NOT LIKE '%,none,%'".into());
        }
        if !page_opts.include_canonicalised {
            conditions.push("(coalesce(p.canonical_1, p.http_canonical) IS NULL OR coalesce(p.canonical_1, p.http_canonical) = p.address)".into());
        }
        let cdn = if opts.cdn_regex.trim().is_empty() { None } else { Some(regex::Regex::new(opts.cdn_regex.trim())?) };
        let sql = format!(
            "SELECT p.address, l.target, l.internal, coalesce(i.status_code, 0),
                    (SELECT COUNT(DISTINCT l2.source_id) FROM links l2 WHERE l2.target = l.target AND l2.kind = 'Image')
             FROM urls p JOIN links l ON l.source_id = p.id AND l.kind = 'Image'
             LEFT JOIN urls i ON i.address = l.target
             WHERE {} ORDER BY p.depth, p.crawl_order, l.id",
            conditions.join(" AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, bool>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?))
        })?;
        let mut pages: Vec<(String, Vec<String>)> = Vec::new();
        for row in rows {
            let (page, image, internal, status, inlinks) = row?;
            if !internal && !cdn.as_ref().is_some_and(|re| re.is_match(&image)) {
                continue;
            }
            if !opts.include_non_200 && status != 200 {
                continue;
            }
            if opts.max_inlinks.is_some_and(|m| inlinks > m as i64) {
                continue;
            }
            if pages.last().is_none_or(|(p, _)| *p != page) {
                pages.push((page, Vec::new()));
            }
            let images = &mut pages.last_mut().expect("pushed").1;
            // The protocol allows 1,000 images per page.
            if images.len() < 1000 && !images.contains(&image) {
                images.push(image);
            }
        }
        std::fs::create_dir_all(dir)?;
        let per_file = opts.max_urls_per_file.clamp(1, 50_000);
        let chunks: Vec<&[(String, Vec<String>)]> = pages.chunks(per_file).collect();
        let body = |chunk: &[(String, Vec<String>)]| {
            let mut out = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:image=\"http://www.google.com/schemas/sitemap-image/1.1\">\n",
            );
            for (page, images) in chunk {
                let _ = writeln!(out, "  <url>\n    <loc>{}</loc>", escape(page));
                for img in images {
                    let _ = writeln!(out, "    <image:image>\n      <image:loc>{}</image:loc>\n    </image:image>", escape(img));
                }
                out.push_str("  </url>\n");
            }
            out.push_str("</urlset>\n");
            out
        };
        let mut written = Vec::new();
        if chunks.len() <= 1 {
            let path = dir.join("image_sitemap.xml");
            std::fs::write(&path, body(chunks.first().copied().unwrap_or(&[])))?;
            written.push(path);
            return Ok(written);
        }
        let base = self.meta("start_url")?.and_then(|u| Url::parse(&u).ok()).map(|u| u.origin().ascii_serialization()).unwrap_or_default();
        let mut index = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        for (i, chunk) in chunks.iter().enumerate() {
            let name = format!("image_sitemap{}.xml", i + 1);
            let path = dir.join(&name);
            std::fs::write(&path, body(chunk))?;
            written.push(path);
            let _ = writeln!(index, "  <sitemap>\n    <loc>{}</loc>\n  </sitemap>", escape(&format!("{base}/{name}")));
        }
        index.push_str("</sitemapindex>\n");
        let path = dir.join("image_sitemap_index.xml");
        std::fs::write(&path, index)?;
        written.push(path);
        Ok(written)
    }
}

fn slugify(name: &str) -> String {
    let s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "segment".into() } else { s }
}

fn urlset(entries: &[Entry], opts: &SitemapOptions, now: i64) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\"");
    if opts.include_images {
        out.push_str(" xmlns:image=\"http://www.google.com/schemas/sitemap-image/1.1\"");
    }
    if opts.include_hreflang {
        out.push_str(" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\"");
    }
    out.push_str(">\n");
    for e in entries {
        out.push_str("  <url>\n");
        let _ = writeln!(out, "    <loc>{}</loc>", escape(&e.loc));
        for (lang, href) in &e.alternates {
            let _ = writeln!(out, "    <xhtml:link rel=\"alternate\" hreflang=\"{}\" href=\"{}\"/>", escape(lang), escape(href));
        }
        for img in &e.images {
            let _ = writeln!(out, "    <image:image>\n      <image:loc>{}</image:loc>\n    </image:image>", escape(img));
        }
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
            ChangeFreq::ByDepth => Some(opts.change_freq_by_depth[e.depth.min(5) as usize].as_str()).filter(|f| !f.is_empty()),
        };
        if let Some(f) = freq {
            let _ = writeln!(out, "    <changefreq>{f}</changefreq>");
        }
        if opts.priority {
            let p = opts.priority_by_depth[e.depth.min(5) as usize].clamp(0.0, 1.0);
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
