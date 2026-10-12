//! Tabs, columns and filters. Every filter is a SQL predicate over the
//! `urls` table (aliased `u`), so the same definition drives the grid,
//! the Overview counts, the Issues panel and exports.

use serde::Serialize;
use toad_core::config::{CrawlConfig, SearchMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum IssueType {
    Issue,
    Warning,
    Opportunity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Priority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilterDef {
    pub name: String,
    /// SQL predicate on `urls u`, ANDed with the tab's base predicate.
    #[serde(skip)]
    pub sql: String,
    pub issue: Option<(IssueType, Priority)>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TabDef {
    pub name: &'static str,
    #[serde(skip)]
    pub base: String,
    pub columns: Vec<&'static str>,
    /// Columns built from the configuration (Custom Search/Extraction):
    /// header and SQL expression.
    #[serde(skip)]
    pub extra_columns: Vec<(String, String)>,
    pub filters: Vec<FilterDef>,
}

impl TabDef {
    pub fn filter(&self, name: &str) -> Option<&FilterDef> {
        self.filters.iter().find(|f| f.name.eq_ignore_ascii_case(name))
    }
}

/// Column header → SQL expression.
pub fn column_sql(name: &str) -> Option<&'static str> {
    COLUMNS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

const COLUMNS: &[(&str, &str)] = &[
    ("Address", "u.address"),
    ("Content Type", "u.content_type"),
    ("Status Code", "u.status_code"),
    ("Status", "u.status"),
    ("Indexability", "CASE WHEN u.indexable = 1 THEN 'Indexable' ELSE 'Non-Indexable' END"),
    ("Indexability Status", "u.indexability_status"),
    ("Title 1", "u.title_1"),
    ("Title 1 Length", "u.title_1_length"),
    ("Title 1 Pixel Width", "u.title_1_px"),
    ("Title 2", "u.title_2"),
    ("Title Occurrences", "u.title_count"),
    ("Meta Description 1", "u.meta_description_1"),
    ("Meta Description 1 Length", "u.meta_description_1_length"),
    ("Meta Description 1 Pixel Width", "u.meta_description_1_px"),
    ("Meta Description 2", "u.meta_description_2"),
    ("Meta Description Occurrences", "u.meta_description_count"),
    ("Meta Keywords 1", "u.meta_keywords_1"),
    ("Meta Keywords 1 Length", "length(u.meta_keywords_1)"),
    ("Meta Keywords Occurrences", "u.meta_keywords_count"),
    ("H1-1", "u.h1_1"),
    ("H1-1 Length", "u.h1_1_length"),
    ("H1-2", "u.h1_2"),
    ("H1-2 Length", "u.h1_2_length"),
    ("H1 Occurrences", "u.h1_count"),
    ("H2-1", "u.h2_1"),
    ("H2-1 Length", "u.h2_1_length"),
    ("H2-2", "u.h2_2"),
    ("H2-2 Length", "u.h2_2_length"),
    ("H2 Occurrences", "u.h2_count"),
    ("Meta Robots 1", "u.meta_robots_1"),
    ("Meta Robots 2", "u.meta_robots_2"),
    ("X-Robots-Tag 1", "u.x_robots_tag_1"),
    ("X-Robots-Tag 2", "u.x_robots_tag_2"),
    ("Meta Refresh 1", "u.meta_refresh"),
    ("Canonical Link Element 1", "u.canonical_1"),
    ("Canonical Occurrences", "u.canonical_count"),
    ("HTTP Canonical", "u.http_canonical"),
    ("rel=\"next\" 1", "u.rel_next"),
    ("rel=\"prev\" 1", "u.rel_prev"),
    ("Hreflang Occurrences", "u.hreflang_count"),
    ("Size (bytes)", "u.size"),
    ("Transferred (bytes)", "u.transferred"),
    ("Word Count", "u.word_count"),
    ("Sentence Count", "u.sentence_count"),
    ("Average Words Per Sentence", "u.avg_words_per_sentence"),
    ("Flesch Reading Ease Score", "u.flesch"),
    ("Readability", "CASE WHEN u.flesch IS NULL THEN NULL WHEN u.flesch >= 90 THEN 'Very Easy' WHEN u.flesch >= 80 THEN 'Easy' WHEN u.flesch >= 70 THEN 'Fairly Easy' WHEN u.flesch >= 60 THEN 'Normal' WHEN u.flesch >= 50 THEN 'Fairly Difficult' WHEN u.flesch >= 30 THEN 'Difficult' ELSE 'Very Difficult' END"),
    ("Text Ratio", "u.text_ratio"),
    ("Crawl Depth", "u.depth"),
    ("Folder Depth", "u.folder_depth"),
    ("Link Score", "u.link_score"),
    ("Inlinks", "u.inlinks"),
    ("Unique Inlinks", "u.unique_inlinks"),
    ("% of Total", "u.pct_of_total"),
    ("Outlinks", "u.outlinks"),
    ("Unique Outlinks", "u.unique_outlinks"),
    ("External Outlinks", "u.external_outlinks"),
    ("Unique External Outlinks", "u.unique_external_outlinks"),
    ("Hash", "u.hash"),
    ("Response Time", "u.response_time"),
    ("Last Modified", "u.last_modified"),
    ("Redirect URL", "u.redirect_url"),
    ("Redirect Type", "u.redirect_type"),
    ("Redirect Chain Length", "u.redirect_chain_length"),
    ("Final Destination", "u.final_destination"),
    ("HTTP Version", "u.http_version"),
    ("Language", "u.language"),
    ("Length", "u.url_length"),
    ("Matched Robots.txt Line", "u.blocked_robots_line"),
    ("Error", "u.error"),
    ("Closest Similarity Match", "CASE WHEN u.closest_similarity IS NULL THEN NULL ELSE round(u.closest_similarity * 100, 1) || '%' END"),
    ("No. Near Duplicates", "u.near_duplicate_count"),
    ("Sitemap URLs", "u.sitemap_url_count"),
    ("In Sitemaps", "(SELECT COUNT(DISTINCT s.sitemap) FROM sitemap_entries s WHERE s.url = u.address)"),
    ("Structured Data Types", "u.sd_types"),
    ("Structured Data Formats", "u.sd_formats"),
    ("Total Items", "u.sd_count"),
    ("Errors", "u.sd_errors"),
    ("Warnings", "u.sd_warnings"),
    ("Parse Errors", "u.sd_parse_errors"),
    ("Cookies", "(SELECT COUNT(*) FROM cookies c WHERE c.url_id = u.id)"),
];

/// Values kept per extractor as separate columns (Name 1, Name 2, ...).
const EXTRACTION_COLUMNS: usize = 3;

pub fn tabs(cfg: &CrawlConfig) -> Vec<TabDef> {
    let mut tabs = build_tabs(cfg);
    // Spider > Extraction: a tab or filter whose element isn't extracted
    // shows nothing (rather than flagging every page as "missing" it).
    let x = &cfg.extraction;
    for t in &mut tabs {
        let off = match t.name {
            "Page Titles" => !x.page_title,
            "Meta Description" => !x.meta_description,
            "Meta Keywords" => !x.meta_keywords,
            "H1" => !x.h1,
            "H2" => !x.h2,
            _ => false,
        };
        if off || (t.name == "Validation" && !cfg.perform_html_validation) {
            t.base = "0 = 1".into();
        }
        // Advanced > Respect noindex / Canonical / Next-Prev: crawled, but
        // hidden from every tab.
        let mut hide = Vec::new();
        if cfg.respect_noindex {
            hide.push("coalesce(u.directives, '') LIKE '%,noindex,%' OR coalesce(u.directives, '') LIKE '%,none,%'");
        }
        if cfg.respect_canonical {
            hide.push("coalesce(u.canonical_1, u.http_canonical, u.address) <> u.address");
        }
        if cfg.respect_next_prev {
            hide.push("u.rel_prev IS NOT NULL");
        }
        if !hide.is_empty() {
            t.base = format!("({}) AND NOT ({})", t.base, hide.join(" OR "));
        }
        if t.name == "Internal" && x.cookies {
            t.columns.push("Cookies");
        }
        for f in &mut t.filters {
            let off = (f.name.starts_with("Low Content") && !x.word_count)
                || (f.name.starts_with("Readability") && !x.readability)
                || (f.name == "Exact Duplicates" && !x.hash)
                || (f.name.starts_with("Form") && !x.forms);
            if off {
                f.sql = "0 = 1".into();
            }
        }
    }
    tabs
}

fn build_tabs(cfg: &CrawlConfig) -> Vec<TabDef> {
    let prefs = &cfg.preferences;
    let ignore_non_indexable = cfg.ignore_non_indexable_for_issues;
    // Internal HTML pages that returned 200: the rows most on-page tabs show.
    let html = "u.is_internal = 1 AND u.is_html = 1 AND u.status_code = 200";
    // Restrict on-page issue filters to indexable pages when configured.
    let ix = if ignore_non_indexable { " AND u.indexable = 1" } else { "" };
    // Advanced > Ignore Paginated URLs for Duplicate Filters: rel=prev pages are left out.
    let paginated = if cfg.ignore_paginated_for_duplicates { " AND u.rel_prev IS NULL" } else { "" };
    let dup_scoped = |col: &str, ix: &str| {
        let ix = format!("{ix}{paginated}");
        format!(
            "u.{col} IS NOT NULL AND u.{col} <> '' AND u.{col} IN (SELECT d.{col} FROM urls d WHERE d.is_internal = 1 AND d.is_html = 1 AND d.status_code = 200{dix} AND d.{col} IS NOT NULL AND d.{col} <> '' GROUP BY d.{col} HAVING COUNT(*) > 1){ix}",
            dix = ix.replace("u.", "d.")
        )
    };
    let dup = |col: &str| dup_scoped(col, ix);
    // Exact duplicates follow Content > Duplicates > "Only Check Indexable Pages".
    let dup_ix = if cfg.near_duplicates_indexable_only { " AND u.indexable = 1" } else { "" };
    let has_link = |cond: &str| format!("EXISTS (SELECT 1 FROM links l WHERE l.source_id = u.id AND {cond})");
    let directive = |d: &str| format!("u.directives LIKE '%,{d},%'");
    let directive_prefix = |d: &str| format!("u.directives LIKE '%,{d}%'");
    let in_sitemap = "EXISTS (SELECT 1 FROM sitemap_entries s WHERE s.url = u.address)".to_string();

    use IssueType::*;
    use Priority::*;
    let f = |name: &str, sql: String, issue: Option<(IssueType, Priority)>, description: &str| FilterDef {
        name: name.to_string(),
        sql,
        issue,
        description: description.to_string(),
    };
    let all = || f("All", "1 = 1".into(), None, "Every URL in this tab.");

    let content_type_filters = || {
        vec![
            all(),
            f("HTML", "u.is_html = 1".into(), None, "HTML pages."),
            f("JavaScript", "u.content_type LIKE '%javascript%'".into(), None, "JavaScript files."),
            f("CSS", "u.content_type LIKE '%css%'".into(), None, "Stylesheets."),
            f("Images", "u.content_type LIKE 'image/%'".into(), None, "Images."),
            f("PDF", "u.content_type LIKE '%pdf%'".into(), None, "PDF documents."),
            f("Media", "(u.content_type LIKE 'audio/%' OR u.content_type LIKE 'video/%')".into(), None, "Audio and video files."),
            f(
                "Other",
                "u.is_html = 0 AND u.content_type <> '' AND u.content_type NOT LIKE '%javascript%' AND u.content_type NOT LIKE '%css%' AND u.content_type NOT LIKE 'image/%' AND u.content_type NOT LIKE '%pdf%' AND u.content_type NOT LIKE 'audio/%' AND u.content_type NOT LIKE 'video/%'".into(),
                None,
                "Any other content type.",
            ),
            f("Unknown", "(u.content_type IS NULL OR u.content_type = '')".into(), None, "No content type, or not crawled (for example blocked by robots.txt)."),
        ]
    };

    let p = prefs;
    let mut response_filters = vec![all()];
    let codes: [(&str, &str, (IssueType, Priority), &str); 9] = [
        ("Blocked by Robots.txt", "u.blocked_robots_line IS NOT NULL", (Warning, High), "Blocked by robots.txt."),
        ("No Response", "u.status_code = 0 AND u.blocked_robots_line IS NULL", (Issue, High), "No response: malformed URL, timeout, refusal or connection error."),
        ("Success (2XX)", "u.status_code BETWEEN 200 AND 299", (Warning, Low), "Successful responses."),
        ("Redirection (3XX)", "u.status_code BETWEEN 300 AND 399", (Warning, Low), "Server-side redirects."),
        ("Redirection (Meta Refresh)", "u.redirect_type = 'Meta Refresh Redirect'", (Warning, Low), "Meta refresh redirects."),
        ("Redirect Chain", "u.redirect_chain_length > 1", (Warning, Medium), "Redirects that take more than one hop to reach the final URL."),
        ("Redirect Loop", "u.redirect_loop = 1", (Issue, High), "Redirect chains that return to a URL already in the chain."),
        ("Client Error (4XX)", "u.status_code BETWEEN 400 AND 499", (Issue, High), "Client errors such as 404 and 410."),
        ("Server Error (5XX)", "u.status_code >= 500", (Issue, High), "Server errors such as 500 and 503."),
    ];
    for (name, sql, issue, desc) in codes {
        let is_success = name.starts_with("Success");
        response_filters.push(f(name, sql.into(), None, desc));
        response_filters.push(f(&format!("Internal {name}"), format!("u.is_internal = 1 AND {sql}"), (!is_success).then_some(issue), desc));
        let ext_issue = (!is_success && name != "Redirect Loop" && name != "Redirect Chain").then_some((Warning, Low));
        response_filters.push(f(&format!("External {name}"), format!("u.is_internal = 0 AND {sql}"), ext_issue, desc));
    }

    vec![
        TabDef {
            name: "Internal",
            base: "u.is_internal = 1".into(),
            extra_columns: vec![],
            columns: vec![
                "Address", "Content Type", "Status Code", "Status", "Indexability", "Indexability Status",
                "Title 1", "Title 1 Length", "Title 1 Pixel Width", "Meta Description 1", "Meta Description 1 Length",
                "Meta Description 1 Pixel Width", "Meta Keywords 1", "H1-1", "H1-1 Length", "H2-1", "H2-1 Length",
                "Meta Robots 1", "X-Robots-Tag 1", "Meta Refresh 1", "Canonical Link Element 1", "rel=\"next\" 1",
                "rel=\"prev\" 1", "Size (bytes)", "Transferred (bytes)", "Word Count", "Flesch Reading Ease Score",
                "Readability", "Text Ratio", "Crawl Depth", "Folder Depth", "Link Score", "Inlinks", "Unique Inlinks",
                "% of Total", "Outlinks", "Unique Outlinks", "External Outlinks", "Unique External Outlinks", "Hash",
                "Response Time", "Last Modified", "Redirect URL", "Redirect Type", "HTTP Version", "Language",
            ],
            filters: content_type_filters(),
        },
        TabDef {
            name: "External",
            base: "u.is_internal = 0".into(),
            extra_columns: vec![],
            columns: vec!["Address", "Content Type", "Status Code", "Status", "Crawl Depth", "Inlinks", "Response Time"],
            filters: content_type_filters(),
        },
        TabDef {
            name: "Security",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Content Type", "Status Code", "Status", "Indexability", "Indexability Status"],
            filters: vec![
                all(),
                f("HTTP URLs", "u.address LIKE 'http://%'".into(), Some((Issue, High)), "Pages served over insecure HTTP."),
                f("HTTPS URLs", "u.address LIKE 'https://%'".into(), None, "Pages served over HTTPS."),
                f("Mixed Content", format!("u.address LIKE 'https://%' AND {}", has_link("l.kind IN ('Image','CSS','JavaScript','Iframe') AND l.target LIKE 'http://%'")), Some((Issue, High)), "HTTPS pages loading resources over HTTP."),
                f("Form URL Insecure", "u.form_action_insecure = 1".into(), Some((Issue, High)), "A form posts to an HTTP URL."),
                f("Form on HTTP URL", "u.form_on_http = 1".into(), Some((Issue, High)), "A form sits on an HTTP page."),
                f("Unsafe Cross-Origin Links", has_link("l.internal = 0 AND l.target_attr = '_blank' AND lower(l.rel) NOT LIKE '%noopener%' AND lower(l.rel) NOT LIKE '%noreferrer%'"), Some((Warning, Low)), "External links opening in a new tab without rel=\"noopener\" or \"noreferrer\"."),
                f("Protocol-Relative Resource Links", has_link("l.path_type = 'Protocol-Relative' AND l.kind IN ('Image','CSS','JavaScript','Iframe')"), Some((Warning, Low)), "Resources loaded with protocol-relative (//) URLs."),
                f("Missing HSTS Header", "u.address LIKE 'https://%' AND u.hsts IS NULL".into(), Some((Warning, Low)), "No Strict-Transport-Security header."),
                f("Missing Content-Security-Policy Header", "u.csp IS NULL".into(), Some((Warning, Low)), "No Content-Security-Policy header."),
                f("Missing X-Content-Type-Options Header", "(u.x_content_type_options IS NULL OR lower(u.x_content_type_options) <> 'nosniff')".into(), Some((Warning, Low)), "X-Content-Type-Options is not 'nosniff'."),
                f("Missing X-Frame-Options Header", "(u.x_frame_options IS NULL OR upper(u.x_frame_options) NOT IN ('DENY','SAMEORIGIN'))".into(), Some((Warning, Low)), "X-Frame-Options is not DENY or SAMEORIGIN."),
                f("Missing Secure Referrer-Policy Header", "(u.referrer_policy IS NULL OR lower(u.referrer_policy) NOT IN ('no-referrer','strict-origin','strict-origin-when-cross-origin','same-origin'))".into(), Some((Warning, Low)), "No secure Referrer-Policy header."),
            ],
        },
        TabDef {
            name: "Response Codes",
            base: "1 = 1".into(),
            extra_columns: vec![],
            columns: vec!["Address", "Content Type", "Status Code", "Status", "Indexability", "Indexability Status", "Inlinks", "Response Time", "Redirect URL", "Redirect Type", "Redirect Chain Length", "Final Destination", "Matched Robots.txt Line", "Error"],
            filters: response_filters,
        },
        TabDef {
            name: "URL",
            base: "u.is_internal = 1 AND u.is_html = 1".into(),
            extra_columns: vec![],
            columns: vec!["Address", "Content Type", "Status Code", "Status", "Indexability", "Indexability Status", "Hash", "Length", "Canonical Link Element 1"],
            filters: vec![
                all(),
                f("Non ASCII Characters", "u.url_non_ascii = 1".into(), Some((Warning, Low)), "URL contains characters outside ASCII."),
                f("Underscores", "u.url_underscores = 1".into(), Some((Opportunity, Low)), "URL contains underscores."),
                f("Uppercase", "u.url_uppercase = 1".into(), Some((Warning, Low)), "URL contains uppercase characters."),
                f("Multiple Slashes", "u.url_multiple_slashes = 1".into(), Some((Issue, Low)), "Consecutive slashes in the path."),
                f("Repetitive Path", "u.url_repetitive_path = 1".into(), Some((Warning, Low)), "A path segment repeats."),
                f("Contains A Space", "u.url_space = 1".into(), Some((Issue, Low)), "URL contains a space."),
                f("Internal Search", "u.url_internal_search = 1".into(), Some((Warning, Low)), "URL looks like an internal search results page."),
                f("Parameters", "u.query_params > 0".into(), Some((Warning, Low)), "URL contains query parameters."),
                f("GA Tracking Parameters", "u.url_ga_params = 1".into(), Some((Warning, Low)), "URL contains utm_, _ga or _gl parameters."),
                f(&format!("Over {} Characters", p.url_max_chars), format!("u.url_length > {}", p.url_max_chars), Some((Opportunity, Low)), "URL is longer than the preference."),
            ],
        },
        TabDef {
            name: "Page Titles",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "Title Occurrences", "Title 1", "Title 1 Length", "Title 1 Pixel Width", "Title 2"],
            filters: vec![
                all(),
                f("Missing", format!("(u.title_count = 0 OR u.title_1 = ''){ix}"), Some((Issue, High)), "No title, or an empty one."),
                f("Duplicate", dup("title_1"), Some((Opportunity, Medium)), "The same title appears on more than one page."),
                f(&format!("Over {} Characters", p.title_max_chars), format!("u.title_1_length > {}{ix}", p.title_max_chars), Some((Opportunity, Medium)), "Title may be truncated in search results."),
                f(&format!("Below {} Characters", p.title_min_chars), format!("u.title_1_length > 0 AND u.title_1_length < {}{ix}", p.title_min_chars), Some((Opportunity, Medium)), "Title may be too short to describe the page."),
                f(&format!("Over {} Pixels", p.title_max_px), format!("u.title_1_px > {}{ix}", p.title_max_px), Some((Opportunity, Medium)), "Title is wider than Google displays."),
                f(&format!("Below {} Pixels", p.title_min_px), format!("u.title_1_px > 0 AND u.title_1_px < {}{ix}", p.title_min_px), Some((Opportunity, Medium)), "Title uses little of the available width."),
                f("Same as H1", format!("u.title_1 <> '' AND lower(u.title_1) = lower(u.h1_1){ix}"), Some((Opportunity, Low)), "Title is identical to the H1."),
                f("Multiple", format!("u.title_count > 1{ix}"), Some((Issue, High)), "More than one title element."),
                f("Outside <head>", format!("u.title_outside_head = 1{ix}"), Some((Issue, High)), "Title element outside the head."),
            ],
        },
        TabDef {
            name: "Meta Description",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "Meta Description Occurrences", "Meta Description 1", "Meta Description 1 Length", "Meta Description 1 Pixel Width", "Meta Description 2"],
            filters: vec![
                all(),
                f("Missing", format!("(u.meta_description_count = 0 OR u.meta_description_1 = ''){ix}"), Some((Opportunity, Low)), "No meta description, or an empty one."),
                f("Duplicate", dup("meta_description_1"), Some((Opportunity, Low)), "The same description appears on more than one page."),
                f(&format!("Over {} Characters", p.description_max_chars), format!("u.meta_description_1_length > {}{ix}", p.description_max_chars), Some((Opportunity, Low)), "Description may be truncated."),
                f(&format!("Below {} Characters", p.description_min_chars), format!("u.meta_description_1_length > 0 AND u.meta_description_1_length < {}{ix}", p.description_min_chars), Some((Opportunity, Low)), "Description may be too short."),
                f(&format!("Over {} Pixels", p.description_max_px), format!("u.meta_description_1_px > {}{ix}", p.description_max_px), Some((Opportunity, Low)), "Description is wider than Google displays."),
                f(&format!("Below {} Pixels", p.description_min_px), format!("u.meta_description_1_px > 0 AND u.meta_description_1_px < {}{ix}", p.description_min_px), Some((Opportunity, Low)), "Description uses little of the available width."),
                f("Multiple", format!("u.meta_description_count > 1{ix}"), Some((Issue, Medium)), "More than one meta description."),
                f("Outside <head>", format!("u.description_outside_head = 1{ix}"), Some((Issue, Medium)), "Meta description outside the head."),
            ],
        },
        TabDef {
            name: "Meta Keywords",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "Meta Keywords Occurrences", "Meta Keywords 1", "Meta Keywords 1 Length"],
            filters: vec![
                all(),
                f("Missing", "(u.meta_keywords_count = 0 OR u.meta_keywords_1 = '')".into(), None, "No meta keywords."),
                f("Duplicate", dup("meta_keywords_1"), None, "The same keywords on more than one page."),
                f("Multiple", "u.meta_keywords_count > 1".into(), None, "More than one meta keywords element."),
            ],
        },
        TabDef {
            name: "H1",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "H1 Occurrences", "H1-1", "H1-1 Length", "H1-2", "H1-2 Length"],
            filters: vec![
                all(),
                f("Missing", format!("(u.h1_count = 0 OR u.h1_1 = ''){ix}"), Some((Issue, Medium)), "No H1, or an empty one."),
                f("Duplicate", dup("h1_1"), Some((Opportunity, Low)), "The same H1 on more than one page."),
                f(&format!("Over {} Characters", p.h1_max_chars), format!("u.h1_1_length > {}{ix}", p.h1_max_chars), Some((Opportunity, Low)), "H1 is longer than the preference."),
                f("Multiple", format!("u.h1_count > 1{ix}"), Some((Warning, Medium)), "More than one H1."),
                f("Alt Text in H1", format!("u.h1_alt_text = 1{ix}"), Some((Warning, Low)), "An image's alt text forms part of the H1."),
                f("Non-sequential", format!("u.h1_non_sequential = 1{ix}"), Some((Warning, Low)), "The H1 is not the first heading on the page."),
            ],
        },
        TabDef {
            name: "H2",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "H2 Occurrences", "H2-1", "H2-1 Length", "H2-2", "H2-2 Length"],
            filters: vec![
                all(),
                f("Missing", format!("(u.h2_count = 0 OR u.h2_1 = ''){ix}"), Some((Warning, Low)), "No H2, or an empty one."),
                f("Duplicate", dup("h2_1"), Some((Opportunity, Low)), "The same first H2 on more than one page."),
                f(&format!("Over {} Characters", p.h2_max_chars), format!("u.h2_1_length > {}{ix}", p.h2_max_chars), Some((Opportunity, Low)), "H2 is longer than the preference."),
                f("Multiple", format!("u.h2_count > 1{ix}"), Some((Warning, Low)), "More than one H2."),
                f("Non-sequential", format!("u.h2_non_sequential = 1{ix}"), Some((Warning, Low)), "An H2 appears before any H1."),
            ],
        },
        TabDef {
            name: "Content",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "Word Count", "Sentence Count", "Average Words Per Sentence", "Flesch Reading Ease Score", "Readability", "Text Ratio", "Closest Similarity Match", "No. Near Duplicates", "Language", "Hash"],
            filters: vec![
                all(),
                f("Exact Duplicates", dup_scoped("hash", dup_ix), Some((Issue, High)), "Pages with identical HTML (matching MD5 hash)."),
                f("Near Duplicates", "u.near_duplicate_count > 0".into(), Some((Warning, Medium)), "Content-area text at least as similar as the near-duplicate threshold (MinHash). Needs Near Duplicates enabled and crawl analysis."),
                f(&format!("Low Content Pages (under {} words)", p.low_content_words), format!("u.word_count < {}{ix}", p.low_content_words), Some((Opportunity, Medium)), "Little text in the content area."),
                f("Soft 404 Pages", {
                    let phrases: Vec<String> = p.soft_404_phrases.iter().map(|ph| {
                        let ph = ph.replace('\'', "''").to_lowercase();
                        format!("lower(coalesce(u.title_1,'')) LIKE '%{ph}%' OR lower(coalesce(u.h1_1,'')) LIKE '%{ph}%'")
                    }).collect();
                    if phrases.is_empty() { "0 = 1".into() } else { format!("({})", phrases.join(" OR ")) }
                }, Some((Warning, High)), "200 pages whose title or H1 reads like an error page."),
                f("Readability Difficult", format!("u.flesch >= 30 AND u.flesch < 50{ix}"), Some((Opportunity, Low)), "Flesch reading ease 30–49."),
                f("Readability Very Difficult", format!("u.flesch < 30{ix}"), Some((Opportunity, Low)), "Flesch reading ease 0–29."),
                f("Lorem Ipsum Placeholder", "u.lorem_ipsum = 1".into(), Some((Warning, High)), "Page contains lorem ipsum placeholder text."),
            ],
        },
        TabDef {
            name: "Images",
            base: "u.is_internal = 1 AND u.content_type LIKE 'image/%'".into(),
            extra_columns: vec![],
            columns: vec!["Address", "Content Type", "Status Code", "Status", "Size (bytes)", "Inlinks"],
            filters: vec![
                all(),
                f(&format!("Over {} KB", p.image_max_kb), format!("u.size > {}", p.image_max_kb as u64 * 1024), Some((Opportunity, Medium)), "Large images slow pages down."),
                f("Missing Alt Text", "EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.kind = 'Image' AND l.alt = '')".into(), Some((Issue, Low)), "Images with an empty alt attribute."),
                f("Missing Alt Attribute", "EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.kind = 'Image' AND l.alt IS NULL)".into(), Some((Issue, Low)), "Images with no alt attribute at all."),
                f(&format!("Alt Text Over {} Characters", p.alt_text_max_chars), format!("EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.kind = 'Image' AND length(l.alt) > {})", p.alt_text_max_chars), Some((Opportunity, Low)), "Alt text longer than the preference."),
            ],
        },
        TabDef {
            name: "Canonicals",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "Canonical Occurrences", "Canonical Link Element 1", "HTTP Canonical", "Meta Robots 1", "X-Robots-Tag 1"],
            filters: vec![
                all(),
                f("Contains Canonical", "(u.canonical_count > 0 OR u.http_canonical IS NOT NULL)".into(), None, "Has a canonical link element or HTTP canonical."),
                f("Self Referencing", "coalesce(u.canonical_1, u.http_canonical) = u.address".into(), None, "Canonical points at the page itself."),
                f("Canonicalised", "coalesce(u.canonical_1, u.http_canonical) <> u.address".into(), Some((Warning, High)), "Canonical points at a different URL."),
                f("Missing", "u.canonical_count = 0 AND u.http_canonical IS NULL".into(), Some((Warning, Medium)), "No canonical."),
                f("Multiple", "u.canonical_count > 1".into(), Some((Warning, Low)), "More than one canonical."),
                f("Multiple Conflicting", "u.canonical_distinct > 1".into(), Some((Issue, High)), "Multiple canonicals pointing at different URLs."),
                f("Non-Indexable Canonical", "coalesce(u.canonical_1, u.http_canonical) <> u.address AND EXISTS (SELECT 1 FROM urls t WHERE t.address = coalesce(u.canonical_1, u.http_canonical) AND t.indexable = 0)".into(), Some((Issue, High)), "Canonical target is blocked, errors, redirects or is noindex."),
                f("Canonical Is Relative", "u.canonical_relative = 1".into(), Some((Warning, Low)), "Canonical uses a relative URL."),
                f("Canonical Chain", "coalesce(u.canonical_1, u.http_canonical) <> u.address AND EXISTS (SELECT 1 FROM urls t WHERE t.address = coalesce(u.canonical_1, u.http_canonical) AND (t.redirect_url IS NOT NULL OR coalesce(t.canonical_1, t.http_canonical, t.address) <> t.address))".into(), Some((Warning, Medium)), "The canonical target itself redirects or is canonicalised elsewhere."),
                f("Unlinked", "EXISTS (SELECT 1 FROM links c WHERE c.target_id = u.id AND c.kind = 'Canonical') AND NOT EXISTS (SELECT 1 FROM links h WHERE h.target_id = u.id AND h.kind = 'Hyperlink')".into(), Some((Warning, Medium)), "Found only through a canonical, never a hyperlink."),
                f("Invalid Attribute In Annotation", "u.canonical_invalid_attr = 1".into(), Some((Issue, High)), "Canonical carries hreflang, lang, media or type."),
                f("Contains Fragment URL", "u.canonical_fragment = 1".into(), Some((Issue, High)), "Canonical href contains a #fragment."),
                f("Outside <head>", "u.canonical_outside_head = 1".into(), Some((Issue, High)), "Canonical element outside the head."),
            ],
        },
        TabDef {
            name: "Pagination",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "rel=\"next\" 1", "rel=\"prev\" 1", "Canonical Link Element 1", "Meta Robots 1"],
            filters: vec![
                all(),
                f("Contains Pagination", "(u.rel_next IS NOT NULL OR u.rel_prev IS NOT NULL)".into(), None, "Has rel=next or rel=prev."),
                f("First Page", "u.rel_next IS NOT NULL AND u.rel_prev IS NULL".into(), None, "Has next only."),
                f("Paginated 2+ Pages", "u.rel_prev IS NOT NULL".into(), None, "Has prev."),
                f("Non-200 Pagination URL", has_link("l.kind IN ('Rel Next','Rel Prev') AND EXISTS (SELECT 1 FROM urls t WHERE t.id = l.target_id AND t.status_code <> 200)"), Some((Issue, High)), "A pagination URL does not return 200."),
                f("Non-Indexable", "(u.rel_next IS NOT NULL OR u.rel_prev IS NOT NULL) AND u.indexable = 0".into(), Some((Warning, High)), "Paginated page is non-indexable."),
                f("Multiple Pagination URLs", "(u.rel_next_count > 1 OR u.rel_prev_count > 1)".into(), Some((Issue, Low)), "More than one next or prev."),
                f("Unlinked Pagination URL", has_link("l.kind IN ('Rel Next','Rel Prev') AND NOT EXISTS (SELECT 1 FROM links h WHERE h.target = l.target AND h.kind = 'Hyperlink')"), Some((Issue, Medium)), "A pagination URL is not linked with an ordinary hyperlink anywhere."),
                f("Pagination Loop", "(u.rel_next = u.address OR u.rel_prev = u.address OR (u.rel_next IS NOT NULL AND u.rel_next = u.rel_prev))".into(), Some((Issue, Low)), "rel=next or rel=prev points back at the page itself, or both point at the same URL."),
                f("Sequence Error", "u.rel_next IS NOT NULL AND EXISTS (SELECT 1 FROM urls t WHERE t.address = u.rel_next AND t.status_code = 200 AND t.is_html = 1 AND coalesce(t.rel_prev, '') <> u.address)".into(), Some((Issue, Low)), "The next page's rel=prev does not point back to this page."),
            ],
        },
        TabDef {
            name: "Directives",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "Meta Robots 1", "Meta Robots 2", "X-Robots-Tag 1", "X-Robots-Tag 2", "Meta Refresh 1"],
            filters: vec![
                all(),
                f("Index", directive("index"), None, "Explicit index directive."),
                f("Noindex", directive("noindex"), Some((Warning, High)), "noindex: the page will be dropped from search results."),
                f("Follow", directive("follow"), None, "Explicit follow directive."),
                f("Nofollow", directive("nofollow"), Some((Warning, High)), "nofollow: links on the page are not followed."),
                f("None", directive("none"), Some((Warning, High)), "Equivalent to noindex, nofollow."),
                f("NoArchive", directive("noarchive"), None, "No cached copy."),
                f("NoSnippet", directive("nosnippet"), Some((Warning, Low)), "No text snippet in results."),
                f("Max-Snippet", directive_prefix("max-snippet"), None, "Snippet length limit."),
                f("Max-Image-Preview", directive_prefix("max-image-preview"), None, "Image preview size limit."),
                f("Max-Video-Preview", directive_prefix("max-video-preview"), None, "Video preview length limit."),
                f("NoODP", directive("noodp"), Some((Warning, Low)), "Obsolete directive."),
                f("NoYDIR", directive("noydir"), Some((Warning, Low)), "Obsolete directive."),
                f("NoImageIndex", directive("noimageindex"), Some((Issue, Low)), "Images on the page are not indexed."),
                f("NoTranslate", directive("notranslate"), Some((Warning, Low)), "No translation offered in results."),
                f("Unavailable_After", directive_prefix("unavailable_after"), Some((Warning, Medium)), "Page drops out of results after a date."),
                f("Refresh", "u.meta_refresh IS NOT NULL".into(), None, "Has a meta refresh."),
                f("Outside <head>", "u.robots_outside_head = 1".into(), Some((Issue, High)), "Meta robots outside the head."),
            ],
        },
        TabDef {
            name: "Hreflang",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "Hreflang Occurrences", "Language"],
            filters: vec![
                all(),
                f("Contains Hreflang", "u.hreflang_count > 0".into(), None, "Has hreflang annotations."),
                f("Non-200 Hreflang URLs", has_link("l.kind = 'Hreflang' AND EXISTS (SELECT 1 FROM urls t WHERE t.id = l.target_id AND t.status_code <> 200)"), Some((Issue, High)), "An alternate URL does not return 200."),
                f("Missing Return Links", has_link("l.kind = 'Hreflang' AND l.target_id <> u.id AND EXISTS (SELECT 1 FROM urls t WHERE t.id = l.target_id AND t.status_code = 200 AND t.is_html = 1) AND NOT EXISTS (SELECT 1 FROM links r WHERE r.source_id = l.target_id AND r.kind = 'Hreflang' AND r.target_id = u.id)"), Some((Issue, High)), "An alternate page does not link back."),
                f("Inconsistent Language & Region Return Links", has_link("l.kind = 'Hreflang' AND l.target_id <> u.id AND EXISTS (SELECT 1 FROM links r WHERE r.source_id = l.target_id AND r.kind = 'Hreflang' AND r.target_id = u.id AND EXISTS (SELECT 1 FROM links me WHERE me.source_id = u.id AND me.kind = 'Hreflang' AND me.target_id = u.id AND lower(me.hreflang) <> lower(r.hreflang)))"), Some((Issue, High)), "An alternate links back with a different language or region code from the one this page uses for itself."),
                f("Non-Canonical Return Links", has_link("l.kind = 'Hreflang' AND EXISTS (SELECT 1 FROM urls t WHERE t.id = l.target_id AND coalesce(t.canonical_1, t.http_canonical, t.address) <> t.address)"), Some((Issue, High)), "An hreflang alternate is canonicalised to a different URL."),
                f("Noindex Return Links", has_link("l.kind = 'Hreflang' AND EXISTS (SELECT 1 FROM urls t WHERE t.id = l.target_id AND (t.directives LIKE '%,noindex,%' OR t.directives LIKE '%,none,%'))"), Some((Issue, High)), "An hreflang alternate is noindex."),
                f("Unlinked Hreflang URLs", has_link("l.kind = 'Hreflang' AND l.target_id <> u.id AND NOT EXISTS (SELECT 1 FROM links h WHERE h.target = l.target AND h.kind = 'Hyperlink')"), Some((Issue, Medium)), "An alternate URL is only reachable through hreflang, never a hyperlink."),
                f("Multiple Entries", "EXISTS (SELECT 1 FROM links l WHERE l.source_id = u.id AND l.kind = 'Hreflang' GROUP BY lower(l.hreflang) HAVING COUNT(DISTINCT l.target) > 1)".into(), Some((Issue, High)), "The same language/region code points at more than one URL."),
                f("Not Using Canonical", "u.hreflang_count > 0 AND coalesce(u.canonical_1, u.http_canonical, u.address) <> u.address".into(), Some((Issue, High)), "Page has hreflang but is canonicalised elsewhere; annotations should sit on canonical URLs."),
                f("Incorrect Language & Region Codes", "u.hreflang_invalid = 1".into(), Some((Issue, High)), "Codes are not valid ISO 639-1 / ISO 3166-1."),
                f("Missing Self Reference", "u.hreflang_count > 0 AND NOT EXISTS (SELECT 1 FROM links l WHERE l.source_id = u.id AND l.kind = 'Hreflang' AND l.target_id = u.id)".into(), Some((Warning, Low)), "No hreflang pointing at the page itself."),
                f("Missing X-Default", "u.hreflang_count > 0 AND NOT EXISTS (SELECT 1 FROM links l WHERE l.source_id = u.id AND l.kind = 'Hreflang' AND lower(l.hreflang) = 'x-default')".into(), Some((Warning, Low)), "No x-default annotation."),
                f("Missing", "u.hreflang_count = 0".into(), None, "No hreflang."),
                f("Outside <head>", "u.hreflang_outside_head = 1".into(), Some((Issue, High)), "Hreflang link element outside the head."),
            ],
        },
        TabDef {
            name: "Links",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "Crawl Depth", "Link Score", "Inlinks", "Unique Inlinks", "% of Total", "Outlinks", "Unique Outlinks", "External Outlinks", "Unique External Outlinks"],
            filters: vec![
                all(),
                f("Pages With High Crawl Depth", format!("u.depth > {}{ix}", p.high_crawl_depth), Some((Opportunity, Medium)), "Pages many clicks from the start page."),
                f("Pages Without Internal Outlinks", "u.outlinks = 0".into(), Some((Warning, High)), "No internal hyperlinks on the page."),
                f("Pages With Uncrawlable Internal Outlinks", has_link("l.kind = 'Uncrawlable'"), Some((Warning, High)), "Links the spider can't follow: span/div hrefs, onclick navigation or javascript: hrefs (needs Spider > Crawl > Uncrawlable Links: Store)."),
                f("Internal Nofollow Outlinks", has_link("l.internal = 1 AND l.kind = 'Hyperlink' AND l.follow = 0"), Some((Warning, Low)), "Internal links marked nofollow."),
                f("Internal Outlinks With No Anchor Text", has_link("l.internal = 1 AND l.kind = 'Hyperlink' AND l.anchor = '' AND coalesce(l.alt, '') = ''"), Some((Opportunity, Low)), "Internal links with no anchor text or image alt."),
                f("Non-Descriptive Anchor Text In Internal Outlinks", {
                    let list: Vec<String> = p.non_descriptive_anchor_text.iter().map(|a| format!("'{}'", a.replace('\'', "''").to_lowercase())).collect();
                    has_link(&format!("l.internal = 1 AND l.kind = 'Hyperlink' AND lower(trim(l.anchor)) IN ({})", if list.is_empty() { "''".into() } else { list.join(",") }))
                }, Some((Opportunity, Low)), "Anchor text such as 'click here' or 'read more'."),
                f("Pages With High External Outlinks", format!("u.external_outlinks > {}", p.high_external_outlinks), Some((Warning, Low)), "More external links than the preference."),
                f("Pages With High Internal Outlinks", format!("u.outlinks > {}", p.high_internal_outlinks), Some((Warning, Low)), "More internal links than the preference."),
                f("Follow & Nofollow Internal Inlinks To Page", "EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.internal = 1 AND l.kind = 'Hyperlink' AND l.follow = 1) AND EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.internal = 1 AND l.kind = 'Hyperlink' AND l.follow = 0)".into(), Some((Warning, Low)), "Linked both with and without nofollow."),
                f("Internal Nofollow Inlinks Only", "EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.kind = 'Hyperlink') AND NOT EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.kind = 'Hyperlink' AND l.follow = 1)".into(), Some((Warning, Low)), "Every internal link to the page is nofollow."),
                f("Outlinks To Localhost", has_link("(l.target LIKE 'http://localhost%' OR l.target LIKE 'https://localhost%' OR l.target LIKE 'http://127.0.0.1%' OR l.target LIKE 'https://127.0.0.1%')"), Some((Issue, High)), "Links to localhost or 127.0.0.1."),
                f("Non-Indexable Page Inlinks Only", "u.indexable = 1 AND EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.kind = 'Hyperlink') AND NOT EXISTS (SELECT 1 FROM links l JOIN urls s ON s.id = l.source_id WHERE l.target_id = u.id AND l.kind = 'Hyperlink' AND s.indexable = 1)".into(), Some((Warning, High)), "Indexable page linked only from non-indexable pages."),
            ],
        },
        TabDef {
            name: "Validation",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Content Type", "Status Code", "Status", "Indexability", "Indexability Status", "Size (bytes)"],
            filters: vec![
                all(),
                f("Missing <head> Tag", "u.head_tags = 0".into(), Some((Issue, High)), "No head element in the source."),
                f("Multiple <head> Tags", "u.head_tags > 1".into(), Some((Issue, High)), "More than one head element."),
                f("Missing <body> Tag", "u.body_tags = 0".into(), Some((Issue, High)), "No body element in the source."),
                f("Multiple <body> Tags", "u.body_tags > 1".into(), Some((Issue, High)), "More than one body element."),
                f(&format!("HTML Document Over {} MB", p.html_max_mb), format!("u.size > {}", p.html_max_mb as u64 * 1024 * 1024), Some((Issue, High)), "HTML larger than the preference."),
            ],
        },
        TabDef {
            name: "Sitemaps",
            base: "u.is_internal = 1".into(),
            extra_columns: vec![],
            columns: vec!["Address", "Content Type", "Status Code", "Status", "Indexability", "Indexability Status", "In Sitemaps", "Sitemap URLs", "Size (bytes)"],
            filters: vec![
                all(),
                f("URLs In Sitemap", in_sitemap.clone(), None, "URLs listed in a crawled XML sitemap."),
                f("URLs Not In Sitemap", format!("u.is_html = 1 AND u.status_code = 200 AND u.indexable = 1 AND u.sitemap_url_count IS NULL AND EXISTS (SELECT 1 FROM sitemap_entries) AND NOT ({in_sitemap})"), Some((Issue, Medium)), "Indexable pages found by the crawl but missing from the XML sitemaps."),
                f("Orphan URLs", format!("{in_sitemap} AND NOT EXISTS (SELECT 1 FROM links l WHERE l.target_id = u.id AND l.kind = 'Hyperlink')"), Some((Issue, Medium)), "In a sitemap, but no internal link points to them."),
                f("Non-Indexable URLs in Sitemap", format!("{in_sitemap} AND u.indexable = 0"), Some((Issue, Medium)), "Sitemaps should list only indexable, canonical URLs."),
                f("URLs In Multiple Sitemaps", "(SELECT COUNT(DISTINCT s.sitemap) FROM sitemap_entries s WHERE s.url = u.address) > 1".into(), Some((Warning, Low)), "Listed in more than one sitemap."),
                f("XML Sitemap With Over 50k URLs", "u.sitemap_url_count > 50000".into(), Some((Issue, High)), "Sitemaps may list at most 50,000 URLs."),
                f("XML Sitemap With Over 50MB", "u.sitemap_url_count IS NOT NULL AND u.size > 52428800".into(), Some((Issue, High)), "Sitemaps may be at most 50 MB uncompressed."),
            ],
        },
        TabDef {
            name: "Structured Data",
            base: html.into(),
            extra_columns: vec![],
            columns: vec!["Address", "Indexability", "Indexability Status", "Structured Data Types", "Structured Data Formats", "Total Items", "Errors", "Warnings", "Parse Errors"],
            filters: vec![
                all(),
                f("Contains Structured Data", "u.sd_count > 0".into(), None, "Has JSON-LD or Microdata."),
                f("Missing", format!("u.sd_count = 0 AND u.sd_parse_errors = 0{ix}"), Some((Opportunity, Low)), "No structured data."),
                f("Validation Errors", "u.sd_errors > 0".into(), Some((Issue, High)), "Missing properties Google requires for a rich result."),
                f("Validation Warnings", "u.sd_warnings > 0".into(), Some((Opportunity, Low)), "Missing properties Google recommends."),
                f("Parse Errors", "u.sd_parse_errors > 0".into(), Some((Issue, High)), "JSON-LD that is not valid JSON."),
                f("JSON-LD URLs", "u.sd_formats LIKE '%JSON-LD%'".into(), None, "Pages with JSON-LD."),
                f("Microdata URLs", "u.sd_formats LIKE '%Microdata%'".into(), None, "Pages with Microdata."),
            ],
        },
        custom_search_tab(cfg, html),
        custom_extraction_tab(cfg, html),
    ]
}

fn custom_search_tab(cfg: &CrawlConfig, base: &str) -> TabDef {
    let mut extra_columns = Vec::new();
    let mut filters = vec![FilterDef { name: "All".into(), sql: "1 = 1".into(), issue: None, description: "Every HTML page.".into() }];
    for (idx, s) in cfg.custom_search.iter().filter(|s| !s.query.is_empty()).enumerate() {
        let value = format!("(SELECT CAST(c.value AS INTEGER) FROM custom_values c WHERE c.url_id = u.id AND c.kind = 'search' AND c.idx = {idx})");
        let label = match s.mode {
            SearchMode::Contains => format!("Contains: {}", s.name),
            SearchMode::DoesNotContain => format!("Does Not Contain: {}", s.name),
        };
        let sql = match s.mode {
            SearchMode::Contains => format!("coalesce({value}, 0) > 0"),
            SearchMode::DoesNotContain => format!("{value} = 0"),
        };
        extra_columns.push((label.clone(), value));
        filters.push(FilterDef { name: s.name.clone(), sql, issue: None, description: label });
    }
    TabDef {
        name: "Custom Search",
        base: base.into(),
        columns: vec!["Address", "Content Type", "Status Code", "Status"],
        extra_columns,
        filters,
    }
}

fn custom_extraction_tab(cfg: &CrawlConfig, base: &str) -> TabDef {
    let mut extra_columns = Vec::new();
    let mut filters = vec![FilterDef { name: "All".into(), sql: "1 = 1".into(), issue: None, description: "Every HTML page.".into() }];
    for (idx, x) in cfg.custom_extraction.iter().filter(|x| !x.expression.is_empty()).enumerate() {
        for ord in 0..EXTRACTION_COLUMNS {
            extra_columns.push((
                format!("{} {}", x.name, ord + 1),
                format!("(SELECT c.value FROM custom_values c WHERE c.url_id = u.id AND c.kind = 'extract' AND c.idx = {idx} AND c.ord = {ord})"),
            ));
        }
        filters.push(FilterDef {
            name: x.name.clone(),
            sql: format!("EXISTS (SELECT 1 FROM custom_values c WHERE c.url_id = u.id AND c.kind = 'extract' AND c.idx = {idx})"),
            issue: None,
            description: format!("Pages where '{}' extracted a value.", x.name),
        });
    }
    TabDef {
        name: "Custom Extraction",
        base: base.into(),
        columns: vec!["Address", "Content Type", "Status Code", "Status"],
        extra_columns,
        filters,
    }
}
