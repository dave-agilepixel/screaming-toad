//! Configuration > Segments, compiled to SQL. A URL's segment is a `CASE`
//! over the segments in order, so the first match wins. Rules can test any
//! grid column or any filter ("Issue"), whose predicate is reused as is.

use crate::tabs::{self, column_sql, TabDef};
use crate::{Store, Table};
use rusqlite::functions::FunctionFlags;
use rusqlite::Connection;
use std::cell::RefCell;
use std::collections::HashMap;
use toad_core::config::{CrawlConfig, Segment, SegmentOp, SegmentRule};

/// The field name for rules that match a filter.
pub const ISSUE: &str = "Issue";

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_").replace('\'', "''")
}

/// One rule as a predicate on `urls u`, or None when it can't apply.
fn rule_sql(rule: &SegmentRule, tabs: &[TabDef]) -> Option<String> {
    if rule.field == ISSUE {
        let (tab, filter) = rule.value.split_once(':')?;
        let t = tabs.iter().find(|t| t.name.eq_ignore_ascii_case(tab.trim()))?;
        let f = t.filter(filter.trim())?;
        let p = format!("(({}) AND ({}))", t.base, f.sql);
        return Some(match rule.op {
            SegmentOp::DoesNotEqual | SegmentOp::DoesNotContain => format!("NOT {p}"),
            _ => p,
        });
    }
    let expr = column_sql(&rule.field)?;
    let text = format!("coalesce(CAST({expr} AS TEXT), '')");
    let v = &rule.value;
    Some(match rule.op {
        SegmentOp::Contains => format!("{text} LIKE '%{}%' ESCAPE '\\'", like(v)),
        SegmentOp::DoesNotContain => format!("{text} NOT LIKE '%{}%' ESCAPE '\\'", like(v)),
        SegmentOp::Equals => format!("{text} = {}", quote(v)),
        SegmentOp::DoesNotEqual => format!("{text} <> {}", quote(v)),
        SegmentOp::StartsWith => format!("{text} LIKE '{}%' ESCAPE '\\'", like(v)),
        SegmentOp::EndsWith => format!("{text} LIKE '%{}' ESCAPE '\\'", like(v)),
        SegmentOp::Regex => format!("regexp({}, {text})", quote(v)),
        SegmentOp::NotRegex => format!("NOT regexp({}, {text})", quote(v)),
        SegmentOp::GreaterThan => format!("({expr}) IS NOT NULL AND CAST({expr} AS REAL) > {}", v.trim().parse::<f64>().ok()?),
        SegmentOp::LessThan => format!("({expr}) IS NOT NULL AND CAST({expr} AS REAL) < {}", v.trim().parse::<f64>().ok()?),
    })
}

fn segment_sql(s: &Segment, tabs: &[TabDef]) -> Option<String> {
    let parts: Vec<String> = s.rules.iter().filter_map(|r| rule_sql(r, tabs)).collect();
    if parts.is_empty() {
        return None;
    }
    let joiner = if s.match_all { " AND " } else { " OR " };
    Some(format!("({})", parts.join(joiner)))
}

/// The Segment column: the name of the first matching segment, or NULL.
pub fn case_sql(cfg: &CrawlConfig) -> Option<String> {
    if cfg.segments.is_empty() {
        return None;
    }
    let tabs = tabs::tabs_without_segments(cfg);
    let whens: Vec<String> = cfg
        .segments
        .iter()
        .filter_map(|s| segment_sql(s, &tabs).map(|p| format!("WHEN {p} THEN {}", quote(&s.name))))
        .collect();
    (!whens.is_empty()).then(|| format!("(CASE {} END)", whens.join(" ")))
}

/// Rows in segment `name` ("No Segment" for rows in none).
pub fn predicate(cfg: &CrawlConfig, name: &str) -> String {
    match case_sql(cfg) {
        Some(case) if name == NO_SEGMENT => format!("{case} IS NULL"),
        Some(case) => format!("{case} = {}", quote(name)),
        None => if name == NO_SEGMENT { "1 = 1".into() } else { "0 = 1".into() },
    }
}

pub const NO_SEGMENT: &str = "No Segment";

thread_local! {
    static REGEX_CACHE: RefCell<HashMap<String, Option<regex::Regex>>> = RefCell::new(HashMap::new());
}

/// `regexp(pattern, text)` for segment rules. Invalid patterns match nothing.
pub fn register_regexp(conn: &Connection) -> rusqlite::Result<()> {
    conn.create_scalar_function("regexp", 2, FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC, |ctx| {
        let pattern: String = ctx.get(0)?;
        let text: String = ctx.get::<Option<String>>(1)?.unwrap_or_default();
        Ok(REGEX_CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() > 256 {
                c.clear();
            }
            c.entry(pattern.clone()).or_insert_with(|| regex::Regex::new(&pattern).ok()).as_ref().is_some_and(|re| re.is_match(&text))
        }))
    })
}

impl Store {
    /// Right-hand Segments tab and Reports > Segments Overview: per segment,
    /// URL counts, indexability and how many URLs have issues of each type.
    pub fn segments_overview(&self) -> anyhow::Result<Table> {
        let cfg = self.config();
        let columns: Vec<String> =
            ["Segment", "URLs", "Indexable", "Non-Indexable", "Issues", "Warnings", "Opportunities"].iter().map(|s| s.to_string()).collect();
        let Some(case) = case_sql(cfg) else {
            return Ok(Table { columns, rows: vec![], total: 0 });
        };
        let tabs = tabs::tabs_without_segments(cfg);
        let any_of = |kind: tabs::IssueType| -> String {
            let preds: Vec<String> = tabs
                .iter()
                .flat_map(|t| t.filters.iter().filter(move |f| f.issue.is_some_and(|(k, _)| k == kind)).map(move |f| format!("(({}) AND ({}))", t.base, f.sql)))
                .collect();
            if preds.is_empty() { "0".into() } else { format!("CASE WHEN {} THEN 1 ELSE 0 END", preds.join(" OR ")) }
        };
        let sql = format!(
            "SELECT coalesce({case}, '{NO_SEGMENT}') AS seg, COUNT(*),
                    SUM(CASE WHEN u.indexable = 1 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN u.indexable = 1 THEN 0 ELSE 1 END),
                    SUM({}), SUM({}), SUM({})
             FROM urls u WHERE u.is_internal = 1 GROUP BY seg",
            any_of(tabs::IssueType::Issue),
            any_of(tabs::IssueType::Warning),
            any_of(tabs::IssueType::Opportunity),
        );
        let found = self.query_strings(&sql, &[])?;
        // In configuration order, then No Segment.
        let mut rows = Vec::new();
        for name in cfg.segments.iter().map(|s| s.name.as_str()).chain([NO_SEGMENT]) {
            match found.iter().find(|r| r[0] == name) {
                Some(r) => rows.push(r.clone()),
                None if name != NO_SEGMENT => rows.push(vec![name.to_string(), "0".into(), "0".into(), "0".into(), "0".into(), "0".into(), "0".into()]),
                None => {}
            }
        }
        Ok(Table { columns, total: rows.len() as u64, rows })
    }

    /// Fields a segment rule can test: every grid column, plus Issue.
    pub fn segment_fields() -> Vec<String> {
        std::iter::once(ISSUE.to_string()).chain(tabs::column_names().into_iter().map(str::to_string)).collect()
    }
}
