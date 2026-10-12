//! XML sitemap parsing: urlsets, sitemap indexes and gzipped files.

use regex::Regex;
use std::io::Read;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SitemapKind {
    UrlSet,
    Index,
}

#[derive(Debug, Clone)]
pub struct ParsedSitemap {
    pub kind: SitemapKind,
    /// `<loc>` values: page URLs for a urlset, child sitemaps for an index.
    pub locs: Vec<String>,
}

/// Parse a sitemap body. Gzipped bodies (by magic bytes) are inflated first.
pub fn parse(body: &[u8]) -> ParsedSitemap {
    let text = if body.starts_with(&[0x1f, 0x8b]) {
        let mut out = String::new();
        let mut decoder = flate2::read::GzDecoder::new(body);
        if decoder.read_to_string(&mut out).is_err() {
            out.clear();
        }
        out
    } else {
        String::from_utf8_lossy(body).into_owned()
    };

    let kind = if text.contains("<sitemapindex") { SitemapKind::Index } else { SitemapKind::UrlSet };
    let locs = loc_regex()
        .captures_iter(&text)
        .filter_map(|c| c.get(1))
        .map(|m| unescape(m.as_str().trim()))
        .filter(|s| !s.is_empty())
        .collect();
    ParsedSitemap { kind, locs }
}

fn loc_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<loc>\s*(?:<!\[CDATA\[)?(.*?)(?:\]\]>)?\s*</loc>").unwrap())
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlset_and_index() {
        let urlset = br#"<?xml version="1.0"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url><loc>https://x.com/a?b=1&amp;c=2</loc><lastmod>2026-01-01</lastmod></url>
            <url><loc> <![CDATA[https://x.com/b]]> </loc></url></urlset>"#;
        let p = parse(urlset);
        assert_eq!(p.kind, SitemapKind::UrlSet);
        assert_eq!(p.locs, vec!["https://x.com/a?b=1&c=2", "https://x.com/b"]);

        let index = br#"<sitemapindex><sitemap><loc>https://x.com/s1.xml</loc></sitemap></sitemapindex>"#;
        let p = parse(index);
        assert_eq!(p.kind, SitemapKind::Index);
        assert_eq!(p.locs, vec!["https://x.com/s1.xml"]);
    }
}
