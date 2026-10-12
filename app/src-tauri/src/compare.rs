//! Compare mode commands: pick two crawls, build or open a comparison, and
//! read its overview, issues, grid rows and detail panes.

use crate::{err, AppState, CmdResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};
use toad_store::compare::{
    test_mapping, ChangeElement, CompareConfig, CompareIssueRow, CompareOverviewRow, CompareState, CompareSummary, LinkState,
    UrlMapping,
};
use toad_store::{Comparison, FilterDef, RowQuery, Store, TabDef, Table};

fn with_cmp<T>(state: &AppState, f: impl FnOnce(&Comparison) -> anyhow::Result<T>) -> CmdResult<T> {
    let guard = state.compare.lock().map_err(err)?;
    let cmp = guard.as_ref().ok_or("No comparison is open.")?;
    f(cmp).map_err(err)
}

fn crawls_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    Ok(app.path().app_data_dir().map_err(err)?.join("crawls"))
}

/// One saved crawl, for the Current/Previous pickers (a first cut of File > Crawls).
#[derive(Serialize)]
pub struct CrawlInfo {
    path: String,
    name: String,
    start_url: Option<String>,
    urls: u64,
    /// Seconds since the Unix epoch.
    modified: u64,
}

fn crawl_info(path: &Path) -> Option<CrawlInfo> {
    let store = Store::open(path).ok()?;
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    Some(CrawlInfo {
        path: path.to_string_lossy().into_owned(),
        name: path.file_stem()?.to_string_lossy().into_owned(),
        start_url: store.meta("start_url").ok().flatten(),
        urls: store.url_count().ok()?,
        modified,
    })
}

/// Crawls saved by the app, newest first.
#[tauri::command]
pub fn list_crawls(app: AppHandle) -> CmdResult<Vec<CrawlInfo>> {
    let dir = crawls_dir(&app)?;
    let Ok(entries) = std::fs::read_dir(&dir) else { return Ok(vec![]) };
    let mut out: Vec<CrawlInfo> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toad"))
        .filter_map(|p| crawl_info(&p))
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(out)
}

#[tauri::command]
pub fn crawl_info_for(path: String) -> CmdResult<CrawlInfo> {
    crawl_info(Path::new(&path)).ok_or_else(|| format!("{path} is not a Screaming Toad crawl"))
}

#[tauri::command]
pub fn get_compare_config(state: State<'_, AppState>) -> CmdResult<CompareConfig> {
    Ok(state.compare_config.lock().map_err(err)?.clone())
}

#[tauri::command]
pub fn set_compare_config(state: State<'_, AppState>, config: CompareConfig) -> CmdResult<()> {
    toad_store::compare::UrlMapper::new(&config.url_mapping).map_err(err)?;
    *state.compare_config.lock().map_err(err)? = config;
    Ok(())
}

#[tauri::command]
pub fn test_url_mapping(rules: Vec<UrlMapping>, url: String) -> CmdResult<String> {
    test_mapping(&rules, &url).map_err(err)
}

/// Compare two crawls with the current Compare configuration. The result is
/// saved under the app's `comparisons` folder and becomes the open comparison.
#[tauri::command]
pub async fn run_compare(app: AppHandle, state: State<'_, AppState>, current: String, previous: String) -> CmdResult<CompareSummary> {
    let cfg = state.compare_config.lock().map_err(err)?.clone();
    let dir = app.path().app_data_dir().map_err(err)?.join("comparisons");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(err)?.as_secs();
    let out = dir.join(format!("comparison-{stamp}.toadcmp"));
    // Drop the old comparison first so its file handles close.
    *state.compare.lock().map_err(err)? = None;
    let cmp = tauri::async_runtime::spawn_blocking(move || Comparison::build(Path::new(&current), Path::new(&previous), &out, &cfg))
        .await
        .map_err(err)?
        .map_err(err)?;
    let summary = cmp.summary().map_err(err)?;
    *state.compare.lock().map_err(err)? = Some(cmp);
    Ok(summary)
}

#[tauri::command]
pub async fn open_comparison(state: State<'_, AppState>, path: String) -> CmdResult<CompareSummary> {
    *state.compare.lock().map_err(err)? = None;
    let cmp = tauri::async_runtime::spawn_blocking(move || Comparison::open(Path::new(&path))).await.map_err(err)?.map_err(err)?;
    let summary = cmp.summary().map_err(err)?;
    *state.compare_config.lock().map_err(err)? = cmp.config().clone();
    *state.compare.lock().map_err(err)? = Some(cmp);
    Ok(summary)
}

#[tauri::command]
pub fn close_comparison(state: State<'_, AppState>) -> CmdResult<()> {
    *state.compare.lock().map_err(err)? = None;
    Ok(())
}

#[tauri::command]
pub fn compare_summary(state: State<'_, AppState>) -> CmdResult<Option<CompareSummary>> {
    let guard = state.compare.lock().map_err(err)?;
    guard.as_ref().map(|c| c.summary()).transpose().map_err(err)
}

/// The current crawl's tabs, plus the Change Detection tab.
#[tauri::command]
pub fn compare_tabs(state: State<'_, AppState>) -> CmdResult<Vec<TabDef>> {
    with_cmp(&state, |c| {
        let mut tabs = c.current().tabs();
        let mut filters = vec![FilterDef { name: "All".into(), sql: String::new(), issue: None, description: "Pages in both crawls that were compared.".into() }];
        for e in ChangeElement::ALL.into_iter().filter(|e| c.config().elements.contains(e)) {
            filters.push(FilterDef { name: e.name().into(), sql: String::new(), issue: None, description: e.description() });
        }
        tabs.push(TabDef {
            name: "Change Detection",
            base: String::new(),
            columns: vec![],
            extra_columns: vec![],
            filters,
        });
        Ok(tabs)
    })
}

#[tauri::command]
pub fn compare_overview(state: State<'_, AppState>) -> CmdResult<Vec<CompareOverviewRow>> {
    with_cmp(&state, |c| c.overview())
}

#[tauri::command]
pub fn compare_issues(state: State<'_, AppState>) -> CmdResult<Vec<CompareIssueRow>> {
    with_cmp(&state, |c| c.issues())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareRowsRequest {
    tab: String,
    filter: String,
    /// Current, Previous, Added, New, Removed or Missing; for Change
    /// Detection, Changed or Not Changed.
    state: String,
    search: Option<String>,
    sort_column: Option<String>,
    sort_desc: Option<bool>,
    offset: u64,
    limit: Option<u64>,
}

impl CompareRowsRequest {
    fn query(&self) -> RowQuery {
        RowQuery {
            tab: self.tab.clone(),
            filter: self.filter.clone(),
            search: self.search.clone(),
            sort: self.sort_column.clone().map(|c| (c, self.sort_desc.unwrap_or(false))),
            offset: self.offset,
            limit: self.limit,
        }
    }
}

fn compare_table(c: &Comparison, req: &CompareRowsRequest) -> anyhow::Result<Table> {
    let q = req.query();
    if req.tab == "Change Detection" {
        let changed = req.state != "Not Changed";
        return c.change_rows(ChangeElement::from_name(&req.filter), changed, &q);
    }
    let state = CompareState::from_name(&req.state).unwrap_or(CompareState::Current);
    c.rows(&q, state)
}

#[tauri::command]
pub fn compare_rows(state: State<'_, AppState>, req: CompareRowsRequest) -> CmdResult<Table> {
    with_cmp(&state, |c| compare_table(c, &req))
}

#[tauri::command]
pub fn compare_export(state: State<'_, AppState>, req: CompareRowsRequest, path: String) -> CmdResult<u64> {
    with_cmp(&state, |c| {
        let t = compare_table(c, &CompareRowsRequest { offset: 0, limit: None, ..req })?;
        toad_store::write_csv(&t, Path::new(&path))?;
        Ok(t.rows.len() as u64)
    })
}

#[tauri::command]
pub fn compare_export_overview(state: State<'_, AppState>, path: String) -> CmdResult<u64> {
    with_cmp(&state, |c| {
        let t = c.overview_table()?;
        toad_store::write_csv(&t, Path::new(&path))?;
        Ok(t.rows.len() as u64)
    })
}

#[tauri::command]
pub fn compare_url_details(state: State<'_, AppState>, url: String) -> CmdResult<Table> {
    with_cmp(&state, |c| c.url_details(&url))
}

#[tauri::command]
pub fn compare_links(state: State<'_, AppState>, url: String, outlinks: bool, link_state: LinkState) -> CmdResult<Table> {
    with_cmp(&state, |c| c.links(&url, outlinks, link_state))
}

#[tauri::command]
pub fn compare_site_structure(state: State<'_, AppState>, depth: usize) -> CmdResult<Table> {
    with_cmp(&state, |c| c.site_structure(depth))
}
