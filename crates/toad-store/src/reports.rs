//! Reports and Bulk Export menus: named tables built from the whole crawl.

use crate::query::Table;
use crate::Store;
use anyhow::anyhow;
use rusqlite::types::Value;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ReportKind {
    Report,
    BulkExport,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportDef {
    pub name: &'static str,
    pub kind: ReportKind,
    pub description: &'static str,
}

/// Columns every link export shares.
const LINK_COLUMNS: &str = "l.kind AS \"Type\", s.address AS \"Source\", l.target AS \"Destination\",
    l.anchor AS \"Anchor\", l.alt AS \"Alt Text\", CASE WHEN l.follow = 1 THEN 'true' ELSE 'false' END AS \"Follow\",
    l.rel AS \"Rel\", l.target_attr AS \"Target\", t.status_code AS \"Status Code\", t.status AS \"Status\",
    l.path_type AS \"Path Type\", l.link_path AS \"Link Path\", l.position AS \"Link Position\"";
const LINK_FROM: &str = "FROM links l JOIN urls s ON s.id = l.source_id LEFT JOIN urls t ON t.id = l.target_id";

/// Name, kind, description, SQL. Redirect Chains is built in Rust instead.
const SQL_REPORTS: &[(&str, ReportKind, &str, &str)] = &[
    ("All Redirects", ReportKind::Report, "Every redirecting URL with its final destination.",
     "SELECT u.address AS \"Address\", u.status_code AS \"Status Code\", u.redirect_type AS \"Redirect Type\",
             u.redirect_url AS \"Redirect URL\", u.final_destination AS \"Final Destination\",
             f.status_code AS \"Final Status Code\", u.redirect_chain_length AS \"Number of Redirects\",
             CASE WHEN u.redirect_loop = 1 THEN 'true' ELSE 'false' END AS \"Loop\", u.inlinks AS \"Inlinks\"
      FROM urls u LEFT JOIN urls f ON f.address = u.final_destination
      WHERE u.redirect_url IS NOT NULL ORDER BY u.crawl_order"),
    ("Redirects to Error", ReportKind::Report, "Redirects whose final destination is blocked, has no response, or returns 4XX/5XX.",
     "SELECT u.address AS \"Address\", u.final_destination AS \"Final Destination\", f.status_code AS \"Final Status Code\",
             coalesce(f.indexability_status, 'Not crawled') AS \"Final Status\", u.redirect_chain_length AS \"Number of Redirects\", u.inlinks AS \"Inlinks\"
      FROM urls u LEFT JOIN urls f ON f.address = u.final_destination
      WHERE u.redirect_url IS NOT NULL AND (f.id IS NULL OR f.status_code = 0 OR f.status_code >= 400)
      ORDER BY u.crawl_order"),
    ("Canonical Chains", ReportKind::Report, "Canonicals pointing at URLs that redirect or are canonicalised again.",
     "SELECT u.address AS \"Address\", coalesce(u.canonical_1, u.http_canonical) AS \"Canonical\",
             t.status_code AS \"Canonical Status Code\", t.redirect_url AS \"Canonical Redirects To\",
             coalesce(t.canonical_1, t.http_canonical) AS \"Canonical's Canonical\"
      FROM urls u JOIN urls t ON t.address = coalesce(u.canonical_1, u.http_canonical)
      WHERE coalesce(u.canonical_1, u.http_canonical) <> u.address
        AND (t.redirect_url IS NOT NULL OR coalesce(t.canonical_1, t.http_canonical, t.address) <> t.address)
      ORDER BY u.crawl_order"),
    ("Non-Indexable Canonicals", ReportKind::Report, "Canonical targets that are blocked, error, redirect or are noindex.",
     "SELECT u.address AS \"Address\", coalesce(u.canonical_1, u.http_canonical) AS \"Canonical\",
             t.status_code AS \"Canonical Status Code\", t.indexability_status AS \"Canonical Indexability Status\",
             CASE WHEN EXISTS (SELECT 1 FROM links h WHERE h.target = t.address AND h.kind = 'Hyperlink') THEN 'false' ELSE 'true' END AS \"Unlinked\"
      FROM urls u JOIN urls t ON t.address = coalesce(u.canonical_1, u.http_canonical)
      WHERE coalesce(u.canonical_1, u.http_canonical) <> u.address AND t.indexable = 0
      ORDER BY u.crawl_order"),
    ("Insecure Content", ReportKind::Report, "HTTPS pages loading resources or linking over HTTP.",
     "SELECT s.address AS \"Source\", l.target AS \"Insecure URL\", l.kind AS \"Type\", l.link_path AS \"Link Path\"
      FROM links l JOIN urls s ON s.id = l.source_id
      WHERE s.address LIKE 'https://%' AND l.target LIKE 'http://%'
      ORDER BY s.crawl_order"),
    ("Orphan Pages", ReportKind::Report, "URLs listed in an XML sitemap that no internal hyperlink points to.",
     "SELECT u.address AS \"Address\", u.status_code AS \"Status Code\", u.indexability_status AS \"Indexability Status\",
             'Sitemap' AS \"Source\", (SELECT group_concat(DISTINCT s.sitemap) FROM sitemap_entries s WHERE s.url = u.address) AS \"Sitemaps\"
      FROM urls u
      WHERE EXISTS (SELECT 1 FROM sitemap_entries s WHERE s.url = u.address)
        AND NOT EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.kind = 'Hyperlink')
      ORDER BY u.crawl_order"),
    ("SERP Summary", ReportKind::Report, "Titles and descriptions with their lengths and pixel widths.",
     "SELECT u.address AS \"URL\", u.title_1 AS \"Title\", u.title_1_length AS \"Title Characters\", u.title_1_px AS \"Title Pixels\",
             u.meta_description_1 AS \"Description\", u.meta_description_1_length AS \"Description Characters\",
             u.meta_description_1_px AS \"Description Pixels\"
      FROM urls u WHERE u.is_internal = 1 AND u.is_html = 1 AND u.status_code = 200 ORDER BY u.crawl_order"),
    ("Hreflang: All Hreflang URLs", ReportKind::Report, "Every hreflang annotation with the alternate's response.",
     "SELECT s.address AS \"Source\", l.hreflang AS \"Language\", l.target AS \"Alternate URL\",
             t.status_code AS \"Status Code\", t.indexability_status AS \"Indexability Status\"
      FROM links l JOIN urls s ON s.id = l.source_id LEFT JOIN urls t ON t.id = l.target_id
      WHERE l.kind = 'Hreflang' ORDER BY s.crawl_order, l.id"),
    ("Hreflang: Missing Return Links", ReportKind::Report, "Alternates that do not link back to the page that references them.",
     "SELECT s.address AS \"Source\", l.hreflang AS \"Language\", l.target AS \"Alternate Without Return Link\"
      FROM links l JOIN urls s ON s.id = l.source_id JOIN urls t ON t.id = l.target_id
      WHERE l.kind = 'Hreflang' AND t.id <> s.id AND t.status_code = 200 AND t.is_html = 1
        AND NOT EXISTS (SELECT 1 FROM links r WHERE r.source_id = t.id AND r.kind = 'Hreflang' AND r.target_id = s.id)
      ORDER BY s.crawl_order"),
    ("Structured Data: Validation Errors & Warnings", ReportKind::Report, "Every structured data issue, per URL.",
     "SELECT u.address AS \"Address\", i.severity AS \"Severity\", i.item_type AS \"Type\", i.property AS \"Property\", i.message AS \"Issue\"
      FROM sd_issues i JOIN urls u ON u.id = i.url_id ORDER BY u.crawl_order, i.severity"),
    ("Structured Data: Validation Summary", ReportKind::Report, "Unique structured data issues with the number of URLs affected.",
     "SELECT g.severity AS \"Severity\", g.item_type AS \"Type\", g.message AS \"Issue\", g.urls AS \"URLs\", u.address AS \"Sample URL\"
      FROM (SELECT severity, item_type, message, COUNT(DISTINCT url_id) AS urls, MIN(url_id) AS sample_id
            FROM sd_issues GROUP BY severity, item_type, message) g
      JOIN urls u ON u.id = g.sample_id
      ORDER BY g.severity, g.urls DESC"),
    ("All Inlinks", ReportKind::BulkExport, "Every link found in the crawl, of every type.", ""),
    ("All Anchor Text", ReportKind::BulkExport, "Every hyperlink's anchor text.",
     "SELECT s.address AS \"Source\", l.target AS \"Destination\", l.anchor AS \"Anchor\", l.alt AS \"Alt Text\", l.position AS \"Link Position\"
      FROM links l JOIN urls s ON s.id = l.source_id WHERE l.kind = 'Hyperlink' ORDER BY s.crawl_order, l.id"),
    ("Redirection (3XX) Inlinks", ReportKind::BulkExport, "Links pointing at redirects.", "t.status_code BETWEEN 300 AND 399"),
    ("Client Error (4XX) Inlinks", ReportKind::BulkExport, "Links pointing at 4XX URLs.", "t.status_code BETWEEN 400 AND 499"),
    ("Server Error (5XX) Inlinks", ReportKind::BulkExport, "Links pointing at 5XX URLs.", "t.status_code >= 500"),
    ("No Response Inlinks", ReportKind::BulkExport, "Links pointing at URLs that gave no response.", "t.status_code = 0 AND t.blocked_robots_line IS NULL"),
    ("All Error Inlinks", ReportKind::BulkExport, "Links pointing at URLs with no response, 4XX or 5XX.", "(t.status_code >= 400 OR (t.status_code = 0 AND t.blocked_robots_line IS NULL))"),
    ("Images Missing Alt Text", ReportKind::BulkExport, "Image links with an empty or missing alt attribute.", "l.kind = 'Image' AND coalesce(l.alt, '') = ''"),
    ("Internal Nofollow Outlinks", ReportKind::BulkExport, "Internal hyperlinks marked nofollow, ugc or sponsored.", "l.kind = 'Hyperlink' AND l.internal = 1 AND l.follow = 0"),
    ("Non-Descriptive Anchor Text", ReportKind::BulkExport, "Internal links whose anchor text is on the non-descriptive list.", "NON_DESCRIPTIVE"),
];

const REDIRECT_CHAINS: &str = "Redirect Chains";

impl Store {
    pub fn reports(&self) -> Vec<ReportDef> {
        let mut out = vec![ReportDef {
            name: REDIRECT_CHAINS,
            kind: ReportKind::Report,
            description: "Redirects with two or more hops, or loops, with every hop listed.",
        }];
        out.extend(SQL_REPORTS.iter().map(|(name, kind, description, _)| ReportDef { name: *name, kind: *kind, description: *description }));
        out
    }

    pub fn report(&self, name: &str) -> anyhow::Result<Table> {
        if name.eq_ignore_ascii_case(REDIRECT_CHAINS) {
            return self.redirect_chains();
        }
        let (_, kind, _, sql) = SQL_REPORTS
            .iter()
            .find(|(n, ..)| n.eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow!("unknown report '{name}'. Run `toad report <crawl> --list` to see them."))?;
        let sql = match (*kind, *sql) {
            (ReportKind::BulkExport, "") => format!("SELECT {LINK_COLUMNS} {LINK_FROM} ORDER BY s.crawl_order, l.id"),
            (ReportKind::BulkExport, "NON_DESCRIPTIVE") => {
                let list: Vec<String> = self
                    .cfg
                    .preferences
                    .non_descriptive_anchor_text
                    .iter()
                    .map(|a| format!("'{}'", a.replace('\'', "''").to_lowercase()))
                    .collect();
                format!(
                    "SELECT {LINK_COLUMNS} {LINK_FROM} WHERE l.kind = 'Hyperlink' AND l.internal = 1 AND lower(trim(l.anchor)) IN ({}) ORDER BY s.crawl_order, l.id",
                    if list.is_empty() { "''".into() } else { list.join(",") }
                )
            }
            (ReportKind::BulkExport, cond) if !cond.trim_start().to_uppercase().starts_with("SELECT") => {
                format!("SELECT {LINK_COLUMNS} {LINK_FROM} WHERE {cond} ORDER BY s.crawl_order, l.id")
            }
            (_, full) => full.to_string(),
        };
        self.table(&sql, &[])
    }

    /// Run SQL and keep the column names it declares.
    pub(crate) fn table(&self, sql: &str, args: &[Value]) -> anyhow::Result<Table> {
        let mut stmt = self.conn.prepare(sql)?;
        let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        drop(stmt);
        let rows = self.query_strings(sql, args)?;
        Ok(Table { columns, total: rows.len() as u64, rows })
    }

    fn redirect_chains(&self) -> anyhow::Result<Table> {
        let mut next: HashMap<String, (String, i64, String)> = HashMap::new();
        let mut status: HashMap<String, i64> = HashMap::new();
        let mut starts: Vec<(String, i64)> = Vec::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT address, status_code, redirect_url, coalesce(redirect_type, ''), redirect_chain_length, inlinks FROM urls ORDER BY crawl_order",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })?;
            for row in rows {
                let (address, code, redirect, rtype, chain_len, inlinks) = row?;
                status.insert(address.clone(), code);
                if let Some(target) = redirect {
                    next.insert(address.clone(), (target, code, rtype));
                    // Chains are reported from URLs that something links to.
                    if chain_len > 1 && inlinks > 0 {
                        starts.push((address, chain_len));
                    }
                }
            }
        }
        let mut rows = Vec::new();
        for (start, hops) in starts {
            let mut path = vec![start.clone()];
            let mut types = Vec::new();
            let mut current = start.clone();
            let mut looped = false;
            while let Some((target, code, rtype)) = next.get(&current) {
                types.push(format!("{code} {rtype}").trim().to_string());
                if path.contains(target) {
                    looped = true;
                    path.push(target.clone());
                    break;
                }
                path.push(target.clone());
                current = target.clone();
                if path.len() > 100 {
                    break;
                }
            }
            let last = path.last().cloned().unwrap_or_default();
            rows.push(vec![
                start,
                last.clone(),
                status.get(&last).map(|c| c.to_string()).unwrap_or_else(|| "Not crawled".into()),
                hops.to_string(),
                looped.to_string(),
                path.join(" → "),
                types.join(" → "),
            ]);
        }
        Ok(Table {
            columns: ["Address", "Final Address", "Final Status Code", "Number of Redirects", "Loop", "Chain", "Redirect Types"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            total: rows.len() as u64,
            rows,
        })
    }
}
