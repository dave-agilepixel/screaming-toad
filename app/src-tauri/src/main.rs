//! Screaming Toad desktop app: Tauri commands over the crawl engine and store.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler, PageRecord, Progress};
use toad_store::{IssueRow, OverviewRow, ReportDef, RowQuery, SitemapOptions, Store, Table, TabDef};
use url::Url;

#[derive(Default)]
struct AppState {
    store: Arc<Mutex<Option<Store>>>,
    crawl_path: Arc<Mutex<Option<PathBuf>>>,
    control: Mutex<Option<CrawlControl>>,
    config: Mutex<CrawlConfig>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn with_store<T>(state: &AppState, f: impl FnOnce(&Store) -> anyhow::Result<T>) -> CmdResult<T> {
    let guard = state.store.lock().map_err(err)?;
    let store = guard.as_ref().ok_or("No crawl is open.")?;
    f(store).map_err(err)
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    crawled: u64,
    queued: u64,
    discovered: u64,
    elapsed_ms: u64,
}

impl From<&Progress> for ProgressPayload {
    fn from(p: &Progress) -> Self {
        Self { crawled: p.crawled, queued: p.queued, discovered: p.discovered, elapsed_ms: p.elapsed_ms }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartCrawl {
    url: Option<String>,
    list: Option<Vec<String>>,
}

#[tauri::command]
async fn start_crawl(app: AppHandle, state: State<'_, AppState>, req: StartCrawl) -> CmdResult<String> {
    let mut cfg = state.config.lock().map_err(err)?.clone();
    let seeds: Vec<Url> = match (req.url, req.list) {
        (Some(u), _) if !u.trim().is_empty() => {
            let u = u.trim();
            let with_scheme = if u.contains("://") { u.to_string() } else { format!("https://{u}") };
            vec![Url::parse(&with_scheme).map_err(err)?]
        }
        (_, Some(list)) => {
            cfg = cfg.for_list_mode();
            list.iter().filter_map(|l| Url::parse(l.trim()).ok()).collect()
        }
        _ => return Err("Enter a URL to crawl.".into()),
    };
    if seeds.is_empty() {
        return Err("No valid URLs to crawl.".into());
    }
    if let Some(c) = state.control.lock().map_err(err)?.as_ref() {
        c.stop();
    }

    let dir = app.path().app_data_dir().map_err(err)?.join("crawls");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(err)?.as_secs();
    let host = seeds[0].host_str().unwrap_or("crawl").replace('.', "_");
    let path = dir.join(format!("{host}-{stamp}.toad"));

    let start = seeds[0].clone();
    let store = Store::create(&path, &start, &cfg).map_err(err)?;
    *state.store.lock().map_err(err)? = Some(store);
    *state.crawl_path.lock().map_err(err)? = Some(path.clone());

    let crawler = Crawler::new(cfg, &start).map_err(err)?;
    let control = CrawlControl::default();
    *state.control.lock().map_err(err)? = Some(control.clone());
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    let store_ref = state.store.clone();

    tauri::async_runtime::spawn(async move {
        let crawl = tauri::async_runtime::spawn(crawler.run(seeds, tx, control));
        let mut batch: Vec<PageRecord> = Vec::with_capacity(100);
        let flush = |batch: &mut Vec<PageRecord>| {
            if let Ok(mut guard) = store_ref.lock() {
                if let Some(store) = guard.as_mut() {
                    if let Err(e) = store.insert_batch(batch) {
                        eprintln!("store error: {e}");
                    }
                }
            }
            batch.clear();
        };
        let mut last_flush = std::time::Instant::now();
        while let Some(event) = rx.recv().await {
            match event {
                CrawlEvent::Page(rec) => {
                    batch.push(*rec);
                    if batch.len() >= 100 || last_flush.elapsed().as_millis() > 500 {
                        flush(&mut batch);
                        last_flush = std::time::Instant::now();
                    }
                }
                CrawlEvent::Sitemap { sitemap, urls } => {
                    flush(&mut batch);
                    if let Ok(mut guard) = store_ref.lock() {
                        if let Some(store) = guard.as_mut() {
                            if let Err(e) = store.insert_sitemap(&sitemap, &urls) {
                                eprintln!("store error: {e}");
                            }
                        }
                    }
                }
                CrawlEvent::Progress(p) => {
                    flush(&mut batch);
                    last_flush = std::time::Instant::now();
                    app.emit("crawl-progress", ProgressPayload::from(&p)).ok();
                }
                CrawlEvent::Finished(p) => {
                    flush(&mut batch);
                    app.emit("crawl-progress", ProgressPayload::from(&p)).ok();
                    break;
                }
            }
        }
        flush(&mut batch);
        let _ = crawl.await;
        app.emit("crawl-analysing", ()).ok();
        if let Ok(mut guard) = store_ref.lock() {
            if let Some(store) = guard.as_mut() {
                if let Err(e) = store.analyse() {
                    eprintln!("analysis error: {e}");
                }
            }
        }
        app.emit("crawl-finished", ()).ok();
    });

    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
fn pause_crawl(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(c) = state.control.lock().map_err(err)?.as_ref() {
        c.pause();
    }
    Ok(())
}

#[tauri::command]
fn resume_crawl(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(c) = state.control.lock().map_err(err)?.as_ref() {
        c.resume();
    }
    Ok(())
}

#[tauri::command]
fn stop_crawl(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(c) = state.control.lock().map_err(err)?.as_ref() {
        c.stop();
    }
    Ok(())
}

#[tauri::command]
fn get_tabs(state: State<'_, AppState>) -> CmdResult<Vec<TabDef>> {
    let guard = state.store.lock().map_err(err)?;
    Ok(match guard.as_ref() {
        Some(store) => store.tabs(),
        None => {
            let cfg = state.config.lock().map_err(err)?;
            toad_store::tabs::tabs(&cfg)
        }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RowsRequest {
    tab: String,
    filter: String,
    search: Option<String>,
    sort_column: Option<String>,
    sort_desc: Option<bool>,
    offset: u64,
    limit: u64,
}

#[tauri::command]
fn get_rows(state: State<'_, AppState>, req: RowsRequest) -> CmdResult<Table> {
    let has_store = state.store.lock().map_err(err)?.is_some();
    if !has_store {
        return Ok(Table::default());
    }
    with_store(&state, |s| {
        s.rows(&RowQuery {
            tab: req.tab,
            filter: req.filter,
            search: req.search,
            sort: req.sort_column.map(|c| (c, req.sort_desc.unwrap_or(false))),
            offset: req.offset,
            limit: Some(req.limit),
        })
    })
}

#[tauri::command]
fn get_overview(state: State<'_, AppState>) -> CmdResult<Vec<OverviewRow>> {
    if state.store.lock().map_err(err)?.is_none() {
        return Ok(vec![]);
    }
    with_store(&state, |s| s.overview())
}

#[tauri::command]
fn get_issues(state: State<'_, AppState>) -> CmdResult<Vec<IssueRow>> {
    if state.store.lock().map_err(err)?.is_none() {
        return Ok(vec![]);
    }
    with_store(&state, |s| s.issues())
}

#[tauri::command]
fn get_inlinks(state: State<'_, AppState>, url: String) -> CmdResult<Table> {
    with_store(&state, |s| s.inlinks(&url))
}

#[tauri::command]
fn get_outlinks(state: State<'_, AppState>, url: String) -> CmdResult<Table> {
    with_store(&state, |s| s.outlinks(&url))
}

#[tauri::command]
fn get_reports(state: State<'_, AppState>) -> CmdResult<Vec<ReportDef>> {
    with_store(&state, |s| Ok(s.reports()))
}

#[tauri::command]
fn export_report(state: State<'_, AppState>, name: String, path: String) -> CmdResult<u64> {
    with_store(&state, |s| {
        let table = s.report(&name)?;
        toad_store::write_csv(&table, std::path::Path::new(&path))?;
        Ok(table.rows.len() as u64)
    })
}

#[tauri::command]
fn get_structured_details(state: State<'_, AppState>, url: String) -> CmdResult<Table> {
    with_store(&state, |s| s.structured_details(&url))
}

#[tauri::command]
fn get_near_duplicates(state: State<'_, AppState>, url: String) -> CmdResult<Table> {
    with_store(&state, |s| s.near_duplicates(&url))
}

#[tauri::command]
fn generate_sitemap(state: State<'_, AppState>, dir: String, options: SitemapOptions) -> CmdResult<Vec<String>> {
    with_store(&state, |s| {
        Ok(s.generate_sitemap(std::path::Path::new(&dir), &options)?
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    })
}

#[tauri::command]
fn get_url_details(state: State<'_, AppState>, url: String) -> CmdResult<Vec<(String, String)>> {
    with_store(&state, |s| s.url_details(&url))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportRequest {
    tab: String,
    filter: String,
    search: Option<String>,
    path: String,
}

#[tauri::command]
fn export_csv(state: State<'_, AppState>, req: ExportRequest) -> CmdResult<u64> {
    with_store(&state, |s| {
        s.export_csv(
            &RowQuery { tab: req.tab, filter: req.filter, search: req.search, ..Default::default() },
            std::path::Path::new(&req.path),
        )
    })
}

#[tauri::command]
fn open_crawl(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let store = Store::open(std::path::Path::new(&path)).map_err(err)?;
    *state.config.lock().map_err(err)? = store.config().clone();
    *state.store.lock().map_err(err)? = Some(store);
    *state.crawl_path.lock().map_err(err)? = Some(PathBuf::from(path));
    Ok(())
}

#[tauri::command]
fn save_crawl_as(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    with_store(&state, |s| s.save_copy(std::path::Path::new(&path)))
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> CmdResult<CrawlConfig> {
    Ok(state.config.lock().map_err(err)?.clone())
}

#[tauri::command]
fn set_config(state: State<'_, AppState>, config: CrawlConfig) -> CmdResult<()> {
    *state.config.lock().map_err(err)? = config;
    Ok(())
}

#[tauri::command]
fn reset_config(state: State<'_, AppState>) -> CmdResult<CrawlConfig> {
    let cfg = CrawlConfig::default();
    *state.config.lock().map_err(err)? = cfg.clone();
    Ok(cfg)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            start_crawl,
            pause_crawl,
            resume_crawl,
            stop_crawl,
            get_tabs,
            get_rows,
            get_overview,
            get_issues,
            get_inlinks,
            get_outlinks,
            get_url_details,
            get_near_duplicates,
            get_reports,
            export_report,
            get_structured_details,
            generate_sitemap,
            export_csv,
            open_crawl,
            save_crawl_as,
            get_config,
            set_config,
            reset_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Screaming Toad");
}
