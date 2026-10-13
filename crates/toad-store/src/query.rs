//! Reading a crawl: grid rows, counts, issues, link details and exports.

use crate::tabs::{self, column_sql, IssueType, Priority, TabDef};
use crate::Store;
use anyhow::{anyhow, Context};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Matching rows before offset/limit.
    pub total: u64,
}

#[derive(Debug, Clone, Default)]
pub struct RowQuery {
    pub tab: String,
    pub filter: String,
    /// Case-insensitive substring match on Address.
    pub search: Option<String>,
    /// Column header to sort by, and whether descending.
    pub sort: Option<(String, bool)>,
    pub offset: u64,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IssueRow {
    pub tab: String,
    pub filter: String,
    pub issue_type: IssueType,
    pub priority: Priority,
    pub urls: u64,
    pub pct_of_total: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewRow {
    pub tab: String,
    pub filter: String,
    pub urls: u64,
}

impl Store {
    /// Tabs, narrowed to the global segment filter when one is set.
    pub fn tabs(&self) -> Vec<TabDef> {
        let mut tabs = tabs::tabs(&self.cfg);
        if let Some(seg) = &self.segment_filter {
            let p = crate::segments::predicate(&self.cfg, seg);
            for t in &mut tabs {
                t.base = format!("({}) AND ({p})", t.base);
            }
        }
        tabs
    }

    pub(crate) fn resolve(&self, tab: &str, filter: &str) -> anyhow::Result<(TabDef, String)> {
        let tab_def = self
            .tabs()
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(tab))
            .ok_or_else(|| anyhow!("unknown tab '{tab}'"))?;
        let filter_name = if filter.is_empty() { "All" } else { filter };
        let f = tab_def
            .filter(filter_name)
            .ok_or_else(|| anyhow!("unknown filter '{filter_name}' in tab '{}'", tab_def.name))?;
        let predicate = format!("({}) AND ({})", tab_def.base, f.sql);
        Ok((tab_def, predicate))
    }

    pub fn count(&self, tab: &str, filter: &str) -> anyhow::Result<u64> {
        let (_, predicate) = self.resolve(tab, filter)?;
        self.count_where(&predicate)
    }

    fn count_where(&self, predicate: &str) -> anyhow::Result<u64> {
        let sql = format!("SELECT COUNT(*) FROM urls u WHERE {predicate}");
        let n: i64 = self.conn.query_row(&sql, [], |r| r.get(0)).with_context(|| sql.clone())?;
        Ok(n as u64)
    }

    pub fn rows(&self, q: &RowQuery) -> anyhow::Result<Table> {
        self.rows_where(q, None)
    }

    /// Grid rows, optionally restricted by an extra predicate on `urls u`
    /// (Compare mode uses this to show only Added, New, Removed or Missing URLs).
    pub(crate) fn rows_where(&self, q: &RowQuery, extra: Option<&str>) -> anyhow::Result<Table> {
        let (tab, mut predicate) = self.resolve(&q.tab, &q.filter)?;
        if let Some(extra) = extra {
            predicate = format!("{predicate} AND ({extra})");
        }
        let mut args: Vec<Value> = Vec::new();
        if let Some(s) = q.search.as_deref().filter(|s| !s.is_empty()) {
            predicate = format!("{predicate} AND u.address LIKE ?1 ESCAPE '\\'");
            let escaped = s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            args.push(Value::Text(format!("%{escaped}%")));
        }
        // Configured columns placed among the fixed ones (Segment) take their
        // SQL from extra_columns; the rest of extra_columns go at the end.
        let mut columns: Vec<(String, String)> = tab
            .columns
            .iter()
            .map(|c| {
                let sql = tab.extra_columns.iter().find(|(n, _)| n == c).map(|(_, s)| s.clone());
                (c.to_string(), sql.unwrap_or_else(|| column_sql(c).unwrap_or("NULL").to_string()))
            })
            .collect();
        columns.extend(tab.extra_columns.iter().filter(|(n, _)| !tab.columns.contains(&n.as_str())).cloned());
        let select: Vec<&str> = columns.iter().map(|(_, sql)| sql.as_str()).collect();
        let order = match &q.sort {
            Some((col, desc)) => match columns.iter().find(|(name, _)| name == col) {
                Some((_, expr)) => format!("{expr} {}", if *desc { "DESC" } else { "ASC" }),
                None => "u.crawl_order".into(),
            },
            None => "u.crawl_order".into(),
        };
        let limit = q.limit.map(|l| l as i64).unwrap_or(-1);
        let sql = format!(
            "SELECT {} FROM urls u WHERE {predicate} ORDER BY {order} LIMIT {limit} OFFSET {}",
            select.join(", "),
            q.offset
        );
        let count_sql = format!("SELECT COUNT(*) FROM urls u WHERE {predicate}");
        let total: i64 = self.conn.query_row(&count_sql, params_from_iter(args.iter()), |r| r.get(0))?;
        let rows = self.query_strings(&sql, &args)?;
        Ok(Table {
            columns: columns.into_iter().map(|(name, _)| name).collect(),
            rows,
            total: total as u64,
        })
    }

    pub(crate) fn query_strings(&self, sql: &str, args: &[Value]) -> anyhow::Result<Vec<Vec<String>>> {
        let mut stmt = self.conn.prepare(sql).with_context(|| sql.to_string())?;
        let width = stmt.column_count();
        let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
            (0..width).map(|i| r.get::<_, Value>(i).map(value_string)).collect::<Result<Vec<_>, _>>()
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Every tab and filter with its URL count (the right-hand Overview).
    pub fn overview(&self) -> anyhow::Result<Vec<OverviewRow>> {
        let mut out = Vec::new();
        for tab in self.tabs() {
            for f in &tab.filters {
                let urls = self.count_where(&format!("({}) AND ({})", tab.base, f.sql))?;
                out.push(OverviewRow { tab: tab.name.to_string(), filter: f.name.clone(), urls });
            }
        }
        Ok(out)
    }

    /// Filters that are issues, warnings or opportunities and have at least
    /// one URL, sorted by type then priority then count.
    pub fn issues(&self) -> anyhow::Result<Vec<IssueRow>> {
        let mut out = Vec::new();
        for tab in self.tabs() {
            let total = self.count_where(&tab.base)?;
            for f in &tab.filters {
                let Some((issue_type, priority)) = f.issue else { continue };
                let urls = self.count_where(&format!("({}) AND ({})", tab.base, f.sql))?;
                if urls == 0 {
                    continue;
                }
                out.push(IssueRow {
                    tab: tab.name.to_string(),
                    filter: f.name.clone(),
                    issue_type,
                    priority,
                    urls,
                    pct_of_total: if total > 0 { (urls as f64 * 1000.0 / total as f64).round() / 10.0 } else { 0.0 },
                    description: f.description.clone(),
                });
            }
        }
        let rank = |t: IssueType| match t {
            IssueType::Issue => 0,
            IssueType::Warning => 1,
            IssueType::Opportunity => 2,
        };
        out.sort_by(|a, b| {
            rank(a.issue_type)
                .cmp(&rank(b.issue_type))
                .then(a.priority.cmp(&b.priority))
                .then(b.urls.cmp(&a.urls))
        });
        Ok(out)
    }

    pub fn inlinks(&self, address: &str) -> anyhow::Result<Table> {
        self.link_table(
            "SELECT l.kind, s.address, l.target, l.anchor, l.alt, CASE WHEN l.follow = 1 THEN 'true' ELSE 'false' END,
                    l.target_attr, l.rel, t.status_code, t.status, l.path_type, l.link_path, l.position, l.origin
             FROM links l JOIN urls s ON s.id = l.source_id LEFT JOIN urls t ON t.id = l.target_id
             WHERE l.target = ?1 ORDER BY s.crawl_order",
            address,
        )
    }

    pub fn outlinks(&self, address: &str) -> anyhow::Result<Table> {
        self.link_table(
            "SELECT l.kind, s.address, l.target, l.anchor, l.alt, CASE WHEN l.follow = 1 THEN 'true' ELSE 'false' END,
                    l.target_attr, l.rel, t.status_code, t.status, l.path_type, l.link_path, l.position, l.origin
             FROM links l JOIN urls s ON s.id = l.source_id LEFT JOIN urls t ON t.id = l.target_id
             WHERE s.address = ?1 ORDER BY l.id",
            address,
        )
    }

    fn link_table(&self, sql: &str, address: &str) -> anyhow::Result<Table> {
        let rows = self.query_strings(sql, &[Value::Text(address.to_string())])?;
        Ok(Table {
            columns: ["Type", "From", "To", "Anchor Text", "Alt Text", "Follow", "Target", "Rel", "Status Code", "Status", "Path Type", "Link Path", "Link Position", "Link Origin"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            total: rows.len() as u64,
            rows,
        })
    }

    /// Near duplicates of one URL, most similar first (Duplicate Details pane).
    pub fn near_duplicates(&self, address: &str) -> anyhow::Result<Table> {
        let rows = self.query_strings(
            "SELECT o.address, round(d.similarity * 100, 1) || '%'
             FROM urls u
             JOIN near_duplicates d ON d.a_id = u.id OR d.b_id = u.id
             JOIN urls o ON o.id = CASE WHEN d.a_id = u.id THEN d.b_id ELSE d.a_id END
             WHERE u.address = ?1
             ORDER BY d.similarity DESC",
            &[Value::Text(address.to_string())],
        )?;
        Ok(Table { columns: vec!["Address".into(), "Similarity".into()], total: rows.len() as u64, rows })
    }

    /// Structured data items and validation issues for one URL
    /// (Structured Data Details pane).
    pub fn structured_details(&self, address: &str) -> anyhow::Result<Table> {
        let rows = self.query_strings(
            "SELECT 'Item', '', i.format, i.types, '', i.properties FROM sd_items i JOIN urls u ON u.id = i.url_id WHERE u.address = ?1
             UNION ALL
             SELECT s.severity, s.source, '', s.item_type, s.property, s.message FROM sd_issues s JOIN urls u ON u.id = s.url_id WHERE u.address = ?1",
            &[Value::Text(address.to_string())],
        )?;
        Ok(Table {
            columns: ["Kind", "Validation", "Format", "Type", "Property", "Detail"].iter().map(|s| s.to_string()).collect(),
            total: rows.len() as u64,
            rows,
        })
    }

    /// Every stored field for one URL (the lower URL Details pane).
    pub fn url_details(&self, address: &str) -> anyhow::Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare("SELECT * FROM urls WHERE address = ?1")?;
        let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows = stmt.query(params![address])?;
        let Some(row) = rows.next()? else { return Ok(vec![]) };
        let mut out = Vec::new();
        for (i, name) in names.into_iter().enumerate() {
            out.push((name, value_string(row.get::<_, Value>(i)?)));
        }
        Ok(out)
    }

    /// Export one tab/filter to CSV, honouring the search.
    pub fn export_csv(&self, q: &RowQuery, path: &Path) -> anyhow::Result<u64> {
        let table = self.rows(&RowQuery { offset: 0, limit: None, ..q.clone() })?;
        crate::export::write_table(&table, path, &format!("{} {}", q.tab, q.filter))?;
        Ok(table.rows.len() as u64)
    }
}

pub fn write_csv(table: &Table, path: &Path) -> anyhow::Result<()> {
    let mut w = csv::Writer::from_path(path).with_context(|| format!("writing {}", path.display()))?;
    w.write_record(&table.columns)?;
    for row in &table.rows {
        w.write_record(row)?;
    }
    w.flush()?;
    Ok(())
}

pub(crate) fn value_string(v: Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => {
            let s = format!("{f:.3}");
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
        Value::Text(s) => s,
        Value::Blob(_) => String::new(),
    }
}
