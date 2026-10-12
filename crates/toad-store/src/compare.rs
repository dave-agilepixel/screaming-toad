//! Compare mode: diff two crawls filter by filter, plus Change Detection.
//!
//! A comparison is its own small SQLite file (`.toadcmp`) holding the two
//! source paths, the compare configuration and the materialised results.
//! Filter SQL uses unqualified table names, so each filter is evaluated
//! inside its own crawl's database; the comparison file is ATTACHed to both
//! crawl connections as `cmp` and the results are joined there on a URL
//! *key*: the current crawl's address, or the previous crawl's address after
//! URL Mapping. See docs/COMPARE_MODE.md.

use crate::query::{IssueRow, RowQuery, Table};
use crate::tabs::{IssueType, Priority};
use crate::Store;
use anyhow::{anyhow, bail, Context};
use regex::Regex;
use rusqlite::types::Value;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use toad_core::minhash;

/// One URL Mapping rule: a regex over previous-crawl URLs and its replacement
/// (`$1` style groups allowed).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UrlMapping {
    pub regex: String,
    pub replace: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChangeElement {
    Indexability,
    PageTitles,
    MetaDescription,
    H1,
    WordCount,
    CrawlDepth,
    Inlinks,
    UniqueInlinks,
    InternalOutlinks,
    UniqueInternalOutlinks,
    ExternalOutlinks,
    UniqueExternalOutlinks,
    StructuredDataUniqueTypes,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ElementKind {
    Text,
    Numeric,
    Content,
}

impl ChangeElement {
    pub const ALL: [ChangeElement; 14] = [
        Self::Indexability,
        Self::PageTitles,
        Self::MetaDescription,
        Self::H1,
        Self::WordCount,
        Self::CrawlDepth,
        Self::Inlinks,
        Self::UniqueInlinks,
        Self::InternalOutlinks,
        Self::UniqueInternalOutlinks,
        Self::ExternalOutlinks,
        Self::UniqueExternalOutlinks,
        Self::StructuredDataUniqueTypes,
        Self::Content,
    ];

    /// Filter name in the Change Detection tab.
    pub fn name(self) -> &'static str {
        match self {
            Self::Indexability => "Indexability",
            Self::PageTitles => "Page Titles",
            Self::MetaDescription => "Meta Description",
            Self::H1 => "H1",
            Self::WordCount => "Word Count",
            Self::CrawlDepth => "Crawl Depth",
            Self::Inlinks => "Inlinks",
            Self::UniqueInlinks => "Unique Inlinks",
            Self::InternalOutlinks => "Internal Outlinks",
            Self::UniqueInternalOutlinks => "Unique Internal Outlinks",
            Self::ExternalOutlinks => "External Outlinks",
            Self::UniqueExternalOutlinks => "Unique External Outlinks",
            Self::StructuredDataUniqueTypes => "Structured Data Unique Types",
            Self::Content => "Content",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.name().eq_ignore_ascii_case(name))
    }

    /// Column label used in "Current X" / "Previous X".
    fn label(self) -> &'static str {
        match self {
            Self::PageTitles => "Page Title",
            other => other.name(),
        }
    }

    /// Column in `cmp_changes` (prefixed `cur_` / `prev_`).
    fn column(self) -> &'static str {
        match self {
            Self::Indexability => "indexable",
            Self::PageTitles => "title",
            Self::MetaDescription => "meta_description",
            Self::H1 => "h1",
            Self::WordCount => "word_count",
            Self::CrawlDepth => "depth",
            Self::Inlinks => "inlinks",
            Self::UniqueInlinks => "unique_inlinks",
            Self::InternalOutlinks => "outlinks",
            Self::UniqueInternalOutlinks => "unique_outlinks",
            Self::ExternalOutlinks => "external_outlinks",
            Self::UniqueExternalOutlinks => "unique_external_outlinks",
            Self::StructuredDataUniqueTypes => "sd_types",
            Self::Content => "word_count",
        }
    }

    fn kind(self) -> ElementKind {
        match self {
            Self::Indexability | Self::PageTitles | Self::MetaDescription | Self::H1 | Self::StructuredDataUniqueTypes => {
                ElementKind::Text
            }
            Self::Content => ElementKind::Content,
            _ => ElementKind::Numeric,
        }
    }

    pub fn description(self) -> String {
        match self {
            Self::Content => "Pages where the content has changed by more than the configured similarity change.".into(),
            other => format!("Pages that have changed {}.", other.name().to_lowercase()),
        }
    }
}

/// Config > Compare.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompareConfig {
    /// Change Detection elements that are switched on (all by default).
    pub elements: Vec<ChangeElement>,
    /// Content: flag pages whose MinHash similarity dropped by more than this %.
    pub similarity_change_pct: f64,
    pub ignore_non_indexable: bool,
    pub url_mapping: Vec<UrlMapping>,
}

impl Default for CompareConfig {
    fn default() -> Self {
        Self {
            elements: ChangeElement::ALL.to_vec(),
            similarity_change_pct: 10.0,
            ignore_non_indexable: false,
            url_mapping: Vec::new(),
        }
    }
}

/// Which URLs of a filter the grid shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareState {
    Current,
    Previous,
    Added,
    New,
    Removed,
    Missing,
}

impl CompareState {
    pub const ALL: [CompareState; 6] = [Self::Current, Self::Previous, Self::Added, Self::New, Self::Removed, Self::Missing];

    pub fn name(self) -> &'static str {
        match self {
            Self::Current => "Current",
            Self::Previous => "Previous",
            Self::Added => "Added",
            Self::New => "New",
            Self::Removed => "Removed",
            Self::Missing => "Missing",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name().eq_ignore_ascii_case(name))
    }

    /// Status-bar text, as Screaming Frog words it.
    pub fn description(self) -> &'static str {
        match self {
            Self::Current => "URLs in current crawl",
            Self::Previous => "URLs in previous crawl",
            Self::Added => "URLs in previous crawl that moved to filter of current crawl",
            Self::New => "New URLs found in current crawl and filter",
            Self::Removed => "URLs in filter for previous crawl, but not in filter for current crawl",
            Self::Missing => "URLs not found in the current crawl, that previously were in filter",
        }
    }
}

/// Inlinks/Outlinks view in Compare mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkState {
    Current,
    Previous,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompareOverviewRow {
    pub tab: String,
    pub filter: String,
    pub current: u64,
    pub previous: u64,
    pub change: i64,
    /// Change / Previous × 100; None when Previous is 0.
    pub change_pct: Option<f64>,
    /// None for rows where the state does not apply (Change Detection).
    pub added: Option<u64>,
    pub new: Option<u64>,
    pub removed: Option<u64>,
    pub missing: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompareIssueRow {
    pub tab: String,
    pub filter: String,
    pub issue_type: IssueType,
    pub priority: Priority,
    pub current: u64,
    pub previous: u64,
    pub change: i64,
    pub change_pct: Option<f64>,
    pub added: u64,
    pub new: u64,
    pub removed: u64,
    pub missing: u64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompareSummary {
    pub current_path: String,
    pub previous_path: String,
    pub current_start_url: Option<String>,
    pub previous_start_url: Option<String>,
    pub current_urls: u64,
    pub previous_urls: u64,
    /// URLs present in both crawls (after URL Mapping).
    pub in_both: u64,
    /// Content change detection needs a content signature (Near Duplicates,
    /// later Store HTML) in both crawls.
    pub content_available: bool,
    pub config: CompareConfig,
}

/// Applies URL Mapping rules, in order, to previous-crawl URLs.
#[derive(Debug, Clone, Default)]
pub struct UrlMapper {
    rules: Vec<(Regex, String)>,
}

impl UrlMapper {
    pub fn new(rules: &[UrlMapping]) -> anyhow::Result<Self> {
        let rules = rules
            .iter()
            .filter(|r| !r.regex.is_empty())
            .map(|r| Ok((Regex::new(&r.regex).with_context(|| format!("URL Mapping regex '{}'", r.regex))?, r.replace.clone())))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Self { rules })
    }

    pub fn map(&self, url: &str) -> String {
        let mut out = url.to_string();
        for (re, rep) in &self.rules {
            out = re.replace_all(&out, rep.as_str()).into_owned();
        }
        out
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS cmp.cmp_meta (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS cmp.cmp_cur (address TEXT PRIMARY KEY) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS cmp.cmp_prev (address TEXT PRIMARY KEY, key TEXT NOT NULL) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS cmp.cmp_prev_key ON cmp_prev(key);
CREATE TABLE IF NOT EXISTS cmp.cmp_filters (
    id INTEGER PRIMARY KEY, tab TEXT NOT NULL, filter TEXT NOT NULL,
    issue_type TEXT, priority TEXT, description TEXT,
    current INTEGER NOT NULL DEFAULT 0, previous INTEGER NOT NULL DEFAULT 0,
    added INTEGER NOT NULL DEFAULT 0, new INTEGER NOT NULL DEFAULT 0,
    removed INTEGER NOT NULL DEFAULT 0, missing INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS cmp.cmp_members (
    filter_id INTEGER NOT NULL, side INTEGER NOT NULL, key TEXT NOT NULL,
    PRIMARY KEY (filter_id, side, key)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS cmp.cmp_changes (
    key TEXT PRIMARY KEY, cur_address TEXT NOT NULL, prev_address TEXT NOT NULL,
    cur_indexable INTEGER, prev_indexable INTEGER,
    cur_indexability_status TEXT, prev_indexability_status TEXT,
    cur_title TEXT, prev_title TEXT,
    cur_meta_description TEXT, prev_meta_description TEXT,
    cur_h1 TEXT, prev_h1 TEXT,
    cur_word_count INTEGER, prev_word_count INTEGER,
    cur_depth INTEGER, prev_depth INTEGER,
    cur_inlinks INTEGER, prev_inlinks INTEGER,
    cur_unique_inlinks INTEGER, prev_unique_inlinks INTEGER,
    cur_outlinks INTEGER, prev_outlinks INTEGER,
    cur_unique_outlinks INTEGER, prev_unique_outlinks INTEGER,
    cur_external_outlinks INTEGER, prev_external_outlinks INTEGER,
    cur_unique_external_outlinks INTEGER, prev_unique_external_outlinks INTEGER,
    cur_sd_types TEXT, prev_sd_types TEXT,
    similarity REAL
) WITHOUT ROWID;
";

/// Columns read from each crawl for Change Detection, in `cmp_changes` order
/// (after the address).
const CHANGE_COLUMNS: &[&str] = &[
    "indexable",
    "indexability_status",
    "title_1",
    "meta_description_1",
    "h1_1",
    "word_count",
    "depth",
    "inlinks",
    "unique_inlinks",
    "outlinks",
    "unique_outlinks",
    "external_outlinks",
    "unique_external_outlinks",
    "sd_types",
];

const CHANGE_TARGETS: &[&str] = &[
    "indexable",
    "indexability_status",
    "title",
    "meta_description",
    "h1",
    "word_count",
    "depth",
    "inlinks",
    "unique_inlinks",
    "outlinks",
    "unique_outlinks",
    "external_outlinks",
    "unique_external_outlinks",
    "sd_types",
];

pub struct Comparison {
    current: Store,
    previous: Store,
    current_path: PathBuf,
    previous_path: PathBuf,
    path: PathBuf,
    cfg: CompareConfig,
    mapper: UrlMapper,
}

fn attach(store: &Store, cmp_path: &Path) -> anyhow::Result<()> {
    store
        .conn
        .execute("ATTACH DATABASE ?1 AS cmp", params![cmp_path.to_string_lossy()])
        .with_context(|| format!("attaching {}", cmp_path.display()))?;
    Ok(())
}

fn absolute(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

impl Comparison {
    /// Compare two crawls and save the result to `out` (replacing any file there).
    pub fn build(current_path: &Path, previous_path: &Path, out: &Path, cfg: &CompareConfig) -> anyhow::Result<Self> {
        let mapper = UrlMapper::new(&cfg.url_mapping)?;
        if absolute(current_path) == absolute(previous_path) {
            bail!("the current and previous crawls are the same file");
        }
        if out.exists() {
            std::fs::remove_file(out).with_context(|| format!("removing {}", out.display()))?;
        }
        let current = Store::open(current_path).with_context(|| format!("opening {}", current_path.display()))?;
        let previous = Store::open(previous_path).with_context(|| format!("opening {}", previous_path.display()))?;
        attach(&current, out)?;
        current.conn.execute_batch(SCHEMA)?;
        attach(&previous, out)?;

        let mut cmp = Self {
            current,
            previous,
            current_path: absolute(current_path),
            previous_path: absolute(previous_path),
            path: out.to_path_buf(),
            cfg: cfg.clone(),
            mapper,
        };
        cmp.set_meta("current_path", &cmp.current_path.to_string_lossy())?;
        cmp.set_meta("previous_path", &cmp.previous_path.to_string_lossy())?;
        cmp.set_meta("config", &serde_json::to_string(cfg)?)?;
        cmp.set_meta("toad_version", env!("CARGO_PKG_VERSION"))?;
        cmp.load_keys()?;
        cmp.load_members()?;
        cmp.load_changes()?;
        cmp.update_counts()?;
        Ok(cmp)
    }

    /// Open a saved comparison. Both source crawls must still exist.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let (current_path, previous_path, cfg) = {
            let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
            let get = |k: &str| -> anyhow::Result<Option<String>> {
                Ok(conn.query_row("SELECT value FROM cmp_meta WHERE key = ?1", params![k], |r| r.get(0)).optional()?)
            };
            let cur = get("current_path")?.ok_or_else(|| anyhow!("{} is not a comparison file", path.display()))?;
            let prev = get("previous_path")?.ok_or_else(|| anyhow!("{} is not a comparison file", path.display()))?;
            let cfg: CompareConfig = match get("config")? {
                Some(json) => serde_json::from_str(&json)?,
                None => CompareConfig::default(),
            };
            (PathBuf::from(cur), PathBuf::from(prev), cfg)
        };
        for p in [&current_path, &previous_path] {
            if !p.exists() {
                bail!("the comparison's source crawl {} no longer exists", p.display());
            }
        }
        let current = Store::open(&current_path)?;
        let previous = Store::open(&previous_path)?;
        attach(&current, path)?;
        attach(&previous, path)?;
        let mapper = UrlMapper::new(&cfg.url_mapping)?;
        Ok(Self { current, previous, current_path, previous_path, path: path.to_path_buf(), cfg, mapper })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config(&self) -> &CompareConfig {
        &self.cfg
    }

    pub fn current(&self) -> &Store {
        &self.current
    }

    pub fn previous(&self) -> &Store {
        &self.previous
    }

    pub fn current_path(&self) -> &Path {
        &self.current_path
    }

    pub fn previous_path(&self) -> &Path {
        &self.previous_path
    }

    fn set_meta(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.current.conn.execute(
            "INSERT INTO cmp.cmp_meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Every URL of each crawl, with the previous crawl's URLs mapped to keys.
    fn load_keys(&mut self) -> anyhow::Result<()> {
        self.current.conn.execute("INSERT OR IGNORE INTO cmp.cmp_cur (address) SELECT address FROM urls", [])?;
        let prev: Vec<String> = {
            let mut stmt = self.previous.conn.prepare("SELECT address FROM urls")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let tx = self.previous.conn.transaction()?;
        {
            let mut stmt = tx.prepare("INSERT OR IGNORE INTO cmp.cmp_prev (address, key) VALUES (?1, ?2)")?;
            for a in &prev {
                stmt.execute(params![a, self.mapper.map(a)])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Evaluate every filter in both crawls and record which keys match.
    fn load_members(&mut self) -> anyhow::Result<()> {
        let prev_tabs = self.previous.tabs();
        let cur_tabs = self.current.tabs();
        let tx = self.current.conn.transaction()?;
        let mut prev_jobs: Vec<(i64, String)> = Vec::new();
        {
            let mut id = 0i64;
            for tab in &cur_tabs {
                for f in &tab.filters {
                    id += 1;
                    let (issue_type, priority) = match f.issue {
                        Some((t, p)) => (Some(format!("{t:?}")), Some(format!("{p:?}"))),
                        None => (None, None),
                    };
                    tx.execute(
                        "INSERT INTO cmp.cmp_filters (id, tab, filter, issue_type, priority, description) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![id, tab.name, f.name, issue_type, priority, f.description],
                    )?;
                    let pred = format!("({}) AND ({})", tab.base, f.sql);
                    let sql = format!("INSERT OR IGNORE INTO cmp.cmp_members (filter_id, side, key) SELECT {id}, 0, u.address FROM urls u WHERE {pred}");
                    tx.execute(&sql, []).with_context(|| sql.clone())?;
                    // The same tab and filter in the previous crawl (its own
                    // configuration may differ, e.g. custom searches).
                    if let Some(pt) = prev_tabs.iter().find(|t| t.name == tab.name) {
                        if let Some(pf) = pt.filter(&f.name) {
                            prev_jobs.push((id, format!("({}) AND ({})", pt.base, pf.sql)));
                        }
                    }
                }
            }
        }
        tx.commit()?;

        let tx = self.previous.conn.transaction()?;
        for (id, pred) in prev_jobs {
            let sql = format!(
                "INSERT OR IGNORE INTO cmp.cmp_members (filter_id, side, key)
                 SELECT {id}, 1, cmpk.key FROM urls u JOIN cmp.cmp_prev cmpk ON cmpk.address = u.address WHERE {pred}"
            );
            tx.execute(&sql, []).with_context(|| sql.clone())?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Change Detection: internal HTML 200 pages present in both crawls.
    fn load_changes(&mut self) -> anyhow::Result<()> {
        let select = format!(
            "SELECT address, {}, minhash FROM urls WHERE is_internal = 1 AND is_html = 1 AND status_code = 200",
            CHANGE_COLUMNS.join(", ")
        );
        let read = |store: &Store| -> anyhow::Result<Vec<(String, Vec<Value>, Option<Vec<u8>>)>> {
            let mut stmt = store.conn.prepare(&select)?;
            let n = CHANGE_COLUMNS.len();
            let rows = stmt.query_map([], |r| {
                let address: String = r.get(0)?;
                let values = (1..=n).map(|i| r.get::<_, Value>(i)).collect::<Result<Vec<_>, _>>()?;
                let hash: Option<Vec<u8>> = r.get(n + 1)?;
                Ok((address, values, hash))
            })?;
            Ok(rows.collect::<Result<_, _>>()?)
        };
        let mut prev_by_key: HashMap<String, (String, Vec<Value>, Option<Vec<u8>>)> = HashMap::new();
        for (address, values, hash) in read(&self.previous)? {
            prev_by_key.entry(self.mapper.map(&address)).or_insert((address, values, hash));
        }
        let current_rows = read(&self.current)?;

        let mut cols = vec!["key".to_string(), "cur_address".into(), "prev_address".into()];
        for c in CHANGE_TARGETS {
            cols.push(format!("cur_{c}"));
            cols.push(format!("prev_{c}"));
        }
        cols.push("similarity".into());
        let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
        let insert = format!("INSERT OR IGNORE INTO cmp.cmp_changes ({}) VALUES ({})", cols.join(", "), placeholders.join(", "));

        let tx = self.current.conn.transaction()?;
        {
            let mut stmt = tx.prepare(&insert)?;
            for (address, values, hash) in current_rows {
                let Some((prev_address, prev_values, prev_hash)) = prev_by_key.get(&address) else { continue };
                let mut row: Vec<Value> = vec![Value::Text(address.clone()), Value::Text(address.clone()), Value::Text(prev_address.clone())];
                for (c, p) in values.iter().zip(prev_values) {
                    row.push(c.clone());
                    row.push(p.clone());
                }
                let similarity = match (&hash, prev_hash) {
                    (Some(a), Some(b)) => {
                        let (a, b) = (minhash::from_bytes(a), minhash::from_bytes(b));
                        (a.len() == b.len() && !a.is_empty()).then(|| minhash::similarity(&a, &b))
                    }
                    _ => None,
                };
                row.push(similarity.map(Value::Real).unwrap_or(Value::Null));
                stmt.execute(rusqlite::params_from_iter(row))?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn update_counts(&self) -> anyhow::Result<()> {
        self.current.conn.execute_batch(
            "UPDATE cmp.cmp_filters SET
                current = (SELECT COUNT(*) FROM cmp.cmp_members m WHERE m.filter_id = cmp_filters.id AND m.side = 0),
                previous = (SELECT COUNT(*) FROM cmp.cmp_members m WHERE m.filter_id = cmp_filters.id AND m.side = 1),
                added = (SELECT COUNT(*) FROM cmp.cmp_members m WHERE m.filter_id = cmp_filters.id AND m.side = 0
                         AND EXISTS (SELECT 1 FROM cmp.cmp_prev p WHERE p.key = m.key)
                         AND NOT EXISTS (SELECT 1 FROM cmp.cmp_members o WHERE o.filter_id = m.filter_id AND o.side = 1 AND o.key = m.key)),
                new = (SELECT COUNT(*) FROM cmp.cmp_members m WHERE m.filter_id = cmp_filters.id AND m.side = 0
                       AND NOT EXISTS (SELECT 1 FROM cmp.cmp_prev p WHERE p.key = m.key)),
                removed = (SELECT COUNT(*) FROM cmp.cmp_members m WHERE m.filter_id = cmp_filters.id AND m.side = 1
                           AND EXISTS (SELECT 1 FROM cmp.cmp_cur c WHERE c.address = m.key)
                           AND NOT EXISTS (SELECT 1 FROM cmp.cmp_members o WHERE o.filter_id = m.filter_id AND o.side = 0 AND o.key = m.key)),
                missing = (SELECT COUNT(*) FROM cmp.cmp_members m WHERE m.filter_id = cmp_filters.id AND m.side = 1
                           AND NOT EXISTS (SELECT 1 FROM cmp.cmp_cur c WHERE c.address = m.key));",
        )?;
        Ok(())
    }

    fn scalar(&self, sql: &str) -> anyhow::Result<i64> {
        Ok(self.current.conn.query_row(sql, [], |r| r.get(0)).with_context(|| sql.to_string())?)
    }

    pub fn summary(&self) -> anyhow::Result<CompareSummary> {
        Ok(CompareSummary {
            current_path: self.current_path.to_string_lossy().into_owned(),
            previous_path: self.previous_path.to_string_lossy().into_owned(),
            current_start_url: self.current.meta("start_url")?,
            previous_start_url: self.previous.meta("start_url")?,
            current_urls: self.scalar("SELECT COUNT(*) FROM cmp.cmp_cur")? as u64,
            previous_urls: self.scalar("SELECT COUNT(*) FROM cmp.cmp_prev")? as u64,
            in_both: self.scalar("SELECT COUNT(*) FROM cmp.cmp_cur c WHERE EXISTS (SELECT 1 FROM cmp.cmp_prev p WHERE p.key = c.address)")? as u64,
            content_available: self.scalar("SELECT EXISTS (SELECT 1 FROM cmp.cmp_changes WHERE similarity IS NOT NULL)")? == 1,
            config: self.cfg.clone(),
        })
    }

    fn filter_id(&self, tab: &str, filter: &str) -> anyhow::Result<i64> {
        let filter = if filter.is_empty() { "All" } else { filter };
        self.current
            .conn
            .query_row(
                "SELECT id FROM cmp.cmp_filters WHERE tab = ?1 COLLATE NOCASE AND filter = ?2 COLLATE NOCASE",
                params![tab, filter],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| anyhow!("unknown filter '{filter}' in tab '{tab}'"))
    }

    /// Every filter with its compare counts, then the Change Detection tab.
    pub fn overview(&self) -> anyhow::Result<Vec<CompareOverviewRow>> {
        let mut stmt = self.current.conn.prepare(
            "SELECT tab, filter, current, previous, added, new, removed, missing FROM cmp.cmp_filters ORDER BY id",
        )?;
        let mut out = stmt
            .query_map([], |r| {
                let current: i64 = r.get(2)?;
                let previous: i64 = r.get(3)?;
                Ok(CompareOverviewRow {
                    tab: r.get(0)?,
                    filter: r.get(1)?,
                    current: current as u64,
                    previous: previous as u64,
                    change: current - previous,
                    change_pct: pct(current - previous, previous),
                    added: Some(r.get::<_, i64>(4)? as u64),
                    new: Some(r.get::<_, i64>(5)? as u64),
                    removed: Some(r.get::<_, i64>(6)? as u64),
                    missing: Some(r.get::<_, i64>(7)? as u64),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        out.extend(self.change_detection_overview()?);
        Ok(out)
    }

    /// Change Detection rows for the Overview: "All" (URLs compared), then the
    /// number of changed URLs for each enabled element. Change % is of All.
    pub fn change_detection_overview(&self) -> anyhow::Result<Vec<CompareOverviewRow>> {
        let all = self.scalar(&format!("SELECT COUNT(*) FROM cmp.cmp_changes WHERE {}", self.scope_sql(None)))?;
        let row = |filter: &str, n: i64| CompareOverviewRow {
            tab: "Change Detection".into(),
            filter: filter.into(),
            current: n as u64,
            previous: n as u64,
            change: n,
            change_pct: pct(n, all),
            added: None,
            new: None,
            removed: None,
            missing: None,
        };
        let mut out = vec![CompareOverviewRow { change: 0, change_pct: None, ..row("All", all) }];
        for e in self.enabled_elements() {
            let n = self.scalar(&format!(
                "SELECT COUNT(*) FROM cmp.cmp_changes WHERE {} AND {}",
                self.scope_sql(Some(e)),
                self.changed_sql(e)
            ))?;
            out.push(row(e.name(), n));
        }
        Ok(out)
    }

    fn enabled_elements(&self) -> Vec<ChangeElement> {
        ChangeElement::ALL.into_iter().filter(|e| self.cfg.elements.contains(e)).collect()
    }

    /// Which `cmp_changes` rows an element is judged on.
    fn scope_sql(&self, element: Option<ChangeElement>) -> String {
        let mut conds = vec!["1 = 1".to_string()];
        if self.cfg.ignore_non_indexable && element != Some(ChangeElement::Indexability) {
            conds.push("cur_indexable = 1 AND prev_indexable = 1".into());
        }
        if element == Some(ChangeElement::Content) {
            conds.push("similarity IS NOT NULL".into());
        }
        conds.join(" AND ")
    }

    fn changed_sql(&self, e: ChangeElement) -> String {
        let c = e.column();
        match e.kind() {
            ElementKind::Text => format!("coalesce(cur_{c}, '') <> coalesce(prev_{c}, '')"),
            ElementKind::Numeric => format!("coalesce(cur_{c}, 0) <> coalesce(prev_{c}, 0)"),
            ElementKind::Content => format!("(1 - similarity) * 100 > {}", self.cfg.similarity_change_pct),
        }
    }

    /// Issues with their compare counts: every issue filter with URLs in
    /// either crawl, sorted like the single-crawl Issues panel.
    pub fn issues(&self) -> anyhow::Result<Vec<CompareIssueRow>> {
        let mut stmt = self.current.conn.prepare(
            "SELECT tab, filter, issue_type, priority, current, previous, added, new, removed, missing, description
             FROM cmp.cmp_filters WHERE issue_type IS NOT NULL AND (current > 0 OR previous > 0)",
        )?;
        let mut out = stmt
            .query_map([], |r| {
                let current: i64 = r.get(4)?;
                let previous: i64 = r.get(5)?;
                let issue_type: String = r.get(2)?;
                let priority: String = r.get(3)?;
                Ok(CompareIssueRow {
                    tab: r.get(0)?,
                    filter: r.get(1)?,
                    issue_type: match issue_type.as_str() {
                        "Issue" => IssueType::Issue,
                        "Warning" => IssueType::Warning,
                        _ => IssueType::Opportunity,
                    },
                    priority: match priority.as_str() {
                        "High" => Priority::High,
                        "Medium" => Priority::Medium,
                        _ => Priority::Low,
                    },
                    current: current as u64,
                    previous: previous as u64,
                    change: current - previous,
                    change_pct: pct(current - previous, previous),
                    added: r.get::<_, i64>(6)? as u64,
                    new: r.get::<_, i64>(7)? as u64,
                    removed: r.get::<_, i64>(8)? as u64,
                    missing: r.get::<_, i64>(9)? as u64,
                    description: r.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let rank = |t: IssueType| match t {
            IssueType::Issue => 0,
            IssueType::Warning => 1,
            IssueType::Opportunity => 2,
        };
        out.sort_by(|a, b| {
            rank(a.issue_type)
                .cmp(&rank(b.issue_type))
                .then(a.priority.cmp(&b.priority))
                .then(b.current.max(b.previous).cmp(&a.current.max(a.previous)))
        });
        Ok(out)
    }

    /// The single-crawl Issues rows for one side, e.g. for the "Previous" view.
    pub fn side_issues(&self, previous: bool) -> anyhow::Result<Vec<IssueRow>> {
        if previous { self.previous.issues() } else { self.current.issues() }
    }

    /// Grid rows for a tab/filter in one compare state. Current, Added and
    /// New rows come from the current crawl; Previous, Removed and Missing
    /// rows from the previous crawl, with their original (unmapped) URLs.
    pub fn rows(&self, q: &RowQuery, state: CompareState) -> anyhow::Result<Table> {
        if q.tab.eq_ignore_ascii_case("Change Detection") {
            let element = ChangeElement::from_name(&q.filter);
            return self.change_rows(element, true, q);
        }
        let id = self.filter_id(&q.tab, &q.filter)?;
        let in_prev = "EXISTS (SELECT 1 FROM cmp.cmp_prev p WHERE p.key = m.key)";
        let in_cur = "EXISTS (SELECT 1 FROM cmp.cmp_cur c WHERE c.address = m.key)";
        let other = |side: i64| format!("EXISTS (SELECT 1 FROM cmp.cmp_members o WHERE o.filter_id = {id} AND o.side = {side} AND o.key = m.key)");
        let cur_set = |cond: String| {
            format!("u.address IN (SELECT m.key FROM cmp.cmp_members m WHERE m.filter_id = {id} AND m.side = 0 AND {cond})")
        };
        let prev_set = |cond: String| {
            format!(
                "u.address IN (SELECT pa.address FROM cmp.cmp_members m JOIN cmp.cmp_prev pa ON pa.key = m.key
                               WHERE m.filter_id = {id} AND m.side = 1 AND {cond})"
            )
        };
        match state {
            CompareState::Current => self.current.rows(q),
            CompareState::Previous => self.previous_rows(q, None),
            CompareState::Added => self.current.rows_where(q, Some(&cur_set(format!("{in_prev} AND NOT {}", other(1))))),
            CompareState::New => self.current.rows_where(q, Some(&cur_set(format!("NOT {in_prev}")))),
            CompareState::Removed => self.previous_rows(q, Some(&prev_set(format!("{in_cur} AND NOT {}", other(0))))),
            CompareState::Missing => self.previous_rows(q, Some(&prev_set(format!("NOT {in_cur}")))),
        }
    }

    /// Previous-crawl rows; if that crawl lacks the filter, its tab's "All"
    /// rows are used (membership already restricts them) or nothing is shown.
    fn previous_rows(&self, q: &RowQuery, extra: Option<&str>) -> anyhow::Result<Table> {
        match self.previous.resolve(&q.tab, &q.filter) {
            Ok(_) => self.previous.rows_where(q, extra),
            Err(_) if extra.is_some() => self.previous.rows_where(&RowQuery { filter: "All".into(), ..q.clone() }, extra),
            Err(_) => {
                let mut t = self.current.rows(&RowQuery { limit: Some(0), ..q.clone() })?;
                t.total = 0;
                Ok(t)
            }
        }
    }

    /// Change Detection grid. `element` None means "All" (every compared URL).
    pub fn change_rows(&self, element: Option<ChangeElement>, changed: bool, q: &RowQuery) -> anyhow::Result<Table> {
        let mut columns: Vec<(String, String)> = vec![("Address".into(), "cur_address".into())];
        let mut default_sort = "cur_address".to_string();
        match element {
            None => {
                columns.push(("Previous Address".into(), "prev_address".into()));
                columns.push(("Current Indexability".into(), indexability_sql("cur")));
                columns.push(("Previous Indexability".into(), indexability_sql("prev")));
            }
            Some(e) => match e.kind() {
                ElementKind::Text if e == ChangeElement::Indexability => {
                    columns.push(("Current Indexability".into(), indexability_sql("cur")));
                    columns.push(("Previous Indexability".into(), indexability_sql("prev")));
                    columns.push(("Current Indexability Status".into(), "cur_indexability_status".into()));
                    columns.push(("Previous Indexability Status".into(), "prev_indexability_status".into()));
                }
                ElementKind::Text => {
                    let c = e.column();
                    columns.push((format!("Current {}", e.label()), format!("cur_{c}")));
                    columns.push((format!("Previous {}", e.label()), format!("prev_{c}")));
                }
                ElementKind::Numeric => {
                    let c = e.column();
                    columns.push((format!("Current {}", e.label()), format!("cur_{c}")));
                    columns.push((format!("Previous {}", e.label()), format!("prev_{c}")));
                    let change = format!("(coalesce(cur_{c}, 0) - coalesce(prev_{c}, 0))");
                    columns.push(("Change".into(), change.clone()));
                    columns.push((
                        "Change %".into(),
                        format!("CASE WHEN coalesce(prev_{c}, 0) = 0 THEN NULL ELSE round({change} * 100.0 / prev_{c}, 2) END"),
                    ));
                    default_sort = format!("abs({change}) DESC, cur_address");
                }
                ElementKind::Content => {
                    columns.push(("Similarity Match %".into(), "round(similarity * 100, 1)".into()));
                    columns.push(("Change %".into(), "round(100 - similarity * 100, 1)".into()));
                    columns.push(("Current Word Count".into(), "cur_word_count".into()));
                    columns.push(("Previous Word Count".into(), "prev_word_count".into()));
                    default_sort = "similarity ASC, cur_address".into();
                }
            },
        }
        if element.is_some_and(|e| !self.cfg.elements.contains(&e)) {
            bail!("Change Detection for {} is switched off in Config > Compare", element.unwrap().name());
        }
        let mut predicate = self.scope_sql(element);
        if let Some(e) = element {
            let c = self.changed_sql(e);
            predicate = if changed { format!("{predicate} AND {c}") } else { format!("{predicate} AND NOT ({c})") };
        }
        let mut args: Vec<Value> = Vec::new();
        if let Some(s) = q.search.as_deref().filter(|s| !s.is_empty()) {
            predicate = format!("{predicate} AND (cur_address LIKE ?1 ESCAPE '\\' OR prev_address LIKE ?1 ESCAPE '\\')");
            let escaped = s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            args.push(Value::Text(format!("%{escaped}%")));
        }
        let order = match &q.sort {
            Some((col, desc)) => match columns.iter().find(|(n, _)| n == col) {
                Some((_, expr)) => format!("{expr} {}", if *desc { "DESC" } else { "ASC" }),
                None => default_sort,
            },
            None => default_sort,
        };
        let select: Vec<&str> = columns.iter().map(|(_, s)| s.as_str()).collect();
        let limit = q.limit.map(|l| l as i64).unwrap_or(-1);
        let sql = format!(
            "SELECT {} FROM cmp.cmp_changes WHERE {predicate} ORDER BY {order} LIMIT {limit} OFFSET {}",
            select.join(", "),
            q.offset
        );
        let total: i64 = self.current.conn.query_row(
            &format!("SELECT COUNT(*) FROM cmp.cmp_changes WHERE {predicate}"),
            rusqlite::params_from_iter(args.iter()),
            |r| r.get(0),
        )?;
        let rows = self.current.query_strings(&sql, &args)?;
        Ok(Table { columns: columns.into_iter().map(|(n, _)| n).collect(), rows, total: total as u64 })
    }

    /// The current and previous addresses for a URL shown in any compare view
    /// (the address may be from either crawl).
    fn pair(&self, address: &str) -> anyhow::Result<(Option<String>, Option<String>)> {
        let conn = &self.current.conn;
        let in_cur: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM cmp.cmp_cur WHERE address = ?1)", params![address], |r| r.get(0))?;
        if in_cur {
            let prev: Option<String> = conn
                .query_row("SELECT address FROM cmp.cmp_prev WHERE key = ?1 ORDER BY address LIMIT 1", params![address], |r| r.get(0))
                .optional()?;
            return Ok((Some(address.to_string()), prev));
        }
        let key: Option<String> =
            conn.query_row("SELECT key FROM cmp.cmp_prev WHERE address = ?1", params![address], |r| r.get(0)).optional()?;
        match key {
            Some(k) => {
                let cur: Option<String> =
                    conn.query_row("SELECT address FROM cmp.cmp_cur WHERE address = ?1", params![k], |r| r.get(0)).optional()?;
                Ok((cur, Some(address.to_string())))
            }
            None => Ok((None, None)),
        }
    }

    /// URL Details: Name | Current Value | Previous Value.
    pub fn url_details(&self, address: &str) -> anyhow::Result<Table> {
        let (cur, prev) = self.pair(address)?;
        let cur_fields = match &cur {
            Some(a) => self.current.url_details(a)?,
            None => vec![],
        };
        let prev_fields = match &prev {
            Some(a) => self.previous.url_details(a)?,
            None => vec![],
        };
        let prev_map: HashMap<&str, &str> = prev_fields.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let names: Vec<&str> = if cur_fields.is_empty() {
            prev_fields.iter().map(|(k, _)| k.as_str()).collect()
        } else {
            cur_fields.iter().map(|(k, _)| k.as_str()).collect()
        };
        let cur_map: HashMap<&str, &str> = cur_fields.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let rows: Vec<Vec<String>> = names
            .into_iter()
            .filter(|n| !matches!(*n, "id" | "minhash" | "headers_json"))
            .map(|n| vec![n.to_string(), cur_map.get(n).unwrap_or(&"").to_string(), prev_map.get(n).unwrap_or(&"").to_string()])
            .collect();
        Ok(Table { columns: vec!["Name".into(), "Current Value".into(), "Previous Value".into()], total: rows.len() as u64, rows })
    }

    /// Inlinks or outlinks for a URL in one link state. Previous-crawl links
    /// have URL Mapping applied to From and To, so they line up with the
    /// current crawl; Added and Removed compare Type, From, To and Anchor Text.
    pub fn links(&self, address: &str, outlinks: bool, state: LinkState) -> anyhow::Result<Table> {
        let (cur, prev) = self.pair(address)?;
        let fetch = |store: &Store, a: &Option<String>| -> anyhow::Result<Option<Table>> {
            match a {
                Some(a) => Ok(Some(if outlinks { store.outlinks(a)? } else { store.inlinks(a)? })),
                None => Ok(None),
            }
        };
        let cur_t = fetch(&self.current, &cur)?;
        let mut prev_t = fetch(&self.previous, &prev)?;
        if let Some(t) = prev_t.as_mut() {
            for row in &mut t.rows {
                row[1] = self.mapper.map(&row[1]);
                row[2] = self.mapper.map(&row[2]);
            }
        }
        let template = cur_t.clone().or_else(|| prev_t.clone()).unwrap_or_else(|| {
            if outlinks { self.current.outlinks("").unwrap_or_default() } else { self.current.inlinks("").unwrap_or_default() }
        });
        let key = |r: &Vec<String>| (r[0].clone(), r[1].clone(), r[2].clone(), r[3].clone());
        let rows = match state {
            LinkState::Current => cur_t.map(|t| t.rows).unwrap_or_default(),
            LinkState::Previous => prev_t.map(|t| t.rows).unwrap_or_default(),
            LinkState::Added => {
                let old: HashSet<_> = prev_t.iter().flat_map(|t| t.rows.iter().map(key)).collect();
                cur_t.map(|t| t.rows.into_iter().filter(|r| !old.contains(&key(r))).collect()).unwrap_or_default()
            }
            LinkState::Removed => {
                let new: HashSet<_> = cur_t.iter().flat_map(|t| t.rows.iter().map(key)).collect();
                prev_t.map(|t| t.rows.into_iter().filter(|r| !new.contains(&key(r))).collect()).unwrap_or_default()
            }
        };
        Ok(Table { columns: template.columns, total: rows.len() as u64, rows })
    }

    /// Site Structure compare: URL counts per top-level path segment (to
    /// `depth` segments), Path | Current URLs | Previous URLs | Change | Change %.
    pub fn site_structure(&self, depth: usize) -> anyhow::Result<Table> {
        let count = |store: &Store, map: bool| -> anyhow::Result<HashMap<String, i64>> {
            let mut stmt = store.conn.prepare("SELECT address FROM urls WHERE is_internal = 1 AND is_html = 1")?;
            let addrs: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
            let mut out = HashMap::new();
            for a in addrs {
                let a = if map { self.mapper.map(&a) } else { a };
                let Ok(u) = url::Url::parse(&a) else { continue };
                // A page counts towards "/" and each folder above it, up to `depth` folders.
                let path = u.path();
                let mut folders: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
                if !path.ends_with('/') {
                    folders.pop();
                }
                let mut prefix = String::from("/");
                *out.entry(prefix.clone()).or_insert(0) += 1;
                for f in folders.into_iter().take(depth) {
                    prefix.push_str(f);
                    prefix.push('/');
                    *out.entry(prefix.clone()).or_insert(0) += 1;
                }
            }
            Ok(out)
        };
        let cur = count(&self.current, false)?;
        let prev = count(&self.previous, true)?;
        let mut paths: Vec<&String> = cur.keys().chain(prev.keys()).collect::<HashSet<_>>().into_iter().collect();
        paths.sort();
        let rows: Vec<Vec<String>> = paths
            .into_iter()
            .map(|p| {
                let c = *cur.get(p).unwrap_or(&0);
                let v = *prev.get(p).unwrap_or(&0);
                vec![
                    p.clone(),
                    c.to_string(),
                    v.to_string(),
                    (c - v).to_string(),
                    pct(c - v, v).map(|x| format!("{x}")).unwrap_or_default(),
                ]
            })
            .collect();
        Ok(Table {
            columns: ["Path", "Current URLs", "Previous URLs", "Change", "Change %"].iter().map(|s| s.to_string()).collect(),
            total: rows.len() as u64,
            rows,
        })
    }

    /// The Overview as a CSV-ready table (Reports > Crawl Overview in compare mode).
    pub fn overview_table(&self) -> anyhow::Result<Table> {
        let opt = |v: Option<u64>| v.map(|n| n.to_string()).unwrap_or_else(|| "-".into());
        let rows: Vec<Vec<String>> = self
            .overview()?
            .into_iter()
            .map(|r| {
                vec![
                    r.tab,
                    r.filter,
                    r.current.to_string(),
                    r.previous.to_string(),
                    r.change.to_string(),
                    r.change_pct.map(|p| format!("{p}")).unwrap_or_default(),
                    opt(r.added),
                    opt(r.new),
                    opt(r.removed),
                    opt(r.missing),
                ]
            })
            .collect();
        Ok(Table {
            columns: ["Tab", "Filter", "Current URLs", "Previous URLs", "Change", "Change %", "Added", "New", "Removed", "Missing"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            total: rows.len() as u64,
            rows,
        })
    }

    /// Export one tab/filter/state to CSV.
    pub fn export_csv(&self, q: &RowQuery, state: CompareState, path: &Path) -> anyhow::Result<u64> {
        let t = self.rows(&RowQuery { offset: 0, limit: None, ..q.clone() }, state)?;
        crate::write_csv(&t, path)?;
        Ok(t.rows.len() as u64)
    }
}

fn indexability_sql(side: &str) -> String {
    format!("CASE WHEN {side}_indexable = 1 THEN 'Indexable' ELSE 'Non-Indexable' END")
}

/// Change as a percentage of `base`, to 2 decimal places.
fn pct(change: i64, base: i64) -> Option<f64> {
    (base != 0).then(|| (change as f64 * 10000.0 / base as f64).round() / 100.0)
}

/// Apply URL Mapping rules to one URL (the Test tab).
pub fn test_mapping(rules: &[UrlMapping], url: &str) -> anyhow::Result<String> {
    Ok(UrlMapper::new(rules)?.map(url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_applies_rules_in_order() {
        let rules = vec![
            UrlMapping { regex: "staging\\.example\\.com".into(), replace: "www.example.com".into() },
            UrlMapping { regex: "/$".into(), replace: "".into() },
        ];
        assert_eq!(test_mapping(&rules, "https://staging.example.com/a/").unwrap(), "https://www.example.com/a");
        let groups = vec![UrlMapping { regex: "/m/(.*)".into(), replace: "/$1".into() }];
        assert_eq!(test_mapping(&groups, "https://x.com/m/page").unwrap(), "https://x.com/page");
        assert!(UrlMapper::new(&[UrlMapping { regex: "(".into(), replace: "".into() }]).is_err());
    }

    #[test]
    fn percentages() {
        assert_eq!(pct(390, 12924), Some(3.02));
        assert_eq!(pct(0, 7), Some(0.0));
        assert_eq!(pct(-1, 4), Some(-25.0));
        assert_eq!(pct(5, 0), None);
    }

    #[test]
    fn element_names_round_trip() {
        for e in ChangeElement::ALL {
            assert_eq!(ChangeElement::from_name(e.name()), Some(e));
        }
        for s in CompareState::ALL {
            assert_eq!(CompareState::from_name(s.name()), Some(s));
        }
    }
}
