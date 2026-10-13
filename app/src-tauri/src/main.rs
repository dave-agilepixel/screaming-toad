//! Screaming Toad desktop app: Tauri commands over the crawl engine and store.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod compare;
mod profiles;
mod settings;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler, PageRecord, Progress};
use toad_store::{CompareConfig, Comparison, IssueRow, OverviewRow, ReportDef, RowQuery, SitemapOptions, Store, Table, TabDef};
use url::Url;

#[derive(Default)]
struct AppState {
    store: Arc<Mutex<Option<Store>>>,
    crawl_path: Arc<Mutex<Option<PathBuf>>>,
    control: Mutex<Option<CrawlControl>>,
    config: Mutex<CrawlConfig>,
    /// The open comparison (Compare mode) and Config > Compare.
    compare: Arc<Mutex<Option<Comparison>>>,
    compare_config: Mutex<CompareConfig>,
    /// File > Settings (storage folder, retention, last mode).
    settings: Mutex<settings::AppSettings>,
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
    settings::prepare_spelling(&app, &mut cfg);
    let mut list_inputs: Option<Vec<String>> = None;
    let seeds: Vec<Url> = match (req.url, req.list) {
        (Some(u), _) if !u.trim().is_empty() => {
            let u = u.trim();
            let with_scheme = if u.contains("://") { u.to_string() } else { format!("https://{u}") };
            vec![Url::parse(&with_scheme).map_err(err)?]
        }
        (_, Some(list)) => {
            cfg = cfg.for_list_mode();
            let inputs: Vec<String> = list.iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
            let seeds = inputs.iter().filter_map(|l| Url::parse(l).ok()).collect();
            list_inputs = Some(inputs);
            seeds
        }
        _ => return Err("Enter a URL to crawl.".into()),
    };
    if seeds.is_empty() {
        return Err("No valid URLs to crawl.".into());
    }
    if let Some(c) = state.control.lock().map_err(err)?.as_ref() {
        c.stop();
    }

    let dir = settings::crawls_dir(&app, &state)?;
    let stamp = toad_store::crawls::now();
    let host = seeds[0].host_str().unwrap_or("crawl").to_string();
    let path = dir.join(format!("{}-{stamp}.toad", host.replace('.', "_")));

    let start = seeds[0].clone();
    let store = Store::create(&path, &start, &cfg).map_err(err)?;
    let mode = if list_inputs.is_some() { "list" } else { "spider" };
    store.mark_started(&host, mode).map_err(err)?;
    if let Some(inputs) = &list_inputs {
        store.set_list_inputs(inputs).map_err(err)?;
    }
    *state.store.lock().map_err(err)? = Some(store);
    *state.crawl_path.lock().map_err(err)? = Some(path.clone());

    let crawler = Crawler::new(cfg, &start).map_err(err)?;
    let control = CrawlControl::default();
    *state.control.lock().map_err(err)? = Some(control.clone());
    let (tx, rx) = tokio::sync::mpsc::channel(1024);
    let crawl = tauri::async_runtime::spawn(crawler.run(seeds, tx, control));
    pump(app, state.store.clone(), rx, crawl);
    Ok(path.to_string_lossy().into_owned())
}

/// Continue a stopped or interrupted crawl from its file, with the
/// configuration it was started with.
#[tauri::command]
async fn continue_crawl(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<()> {
    if let Some(c) = state.control.lock().map_err(err)?.as_ref() {
        c.stop();
    }
    let path = PathBuf::from(path);
    let store = Store::open(&path).map_err(err)?;
    let start = store.meta("start_url").map_err(err)?.and_then(|u| Url::parse(&u).ok()).ok_or("This crawl has no start URL.")?;
    let mut cfg = store.config().clone();
    settings::prepare_spelling(&app, &mut cfg);
    let (crawled, pending) = store.resume_state().map_err(err)?;
    let mode = store.meta("mode").map_err(err)?.unwrap_or_else(|| "spider".into());
    store.mark_started("", &mode).map_err(err)?;
    *state.config.lock().map_err(err)? = cfg.clone();
    *state.store.lock().map_err(err)? = Some(store);
    *state.crawl_path.lock().map_err(err)? = Some(path);

    let crawler = Crawler::new(cfg, &start).map_err(err)?;
    let control = CrawlControl::default();
    *state.control.lock().map_err(err)? = Some(control.clone());
    let (tx, rx) = tokio::sync::mpsc::channel(1024);
    let crawl = tauri::async_runtime::spawn(crawler.resume(crawled, pending, tx, control));
    pump(app, state.store.clone(), rx, crawl);
    Ok(())
}

/// Write a running crawl's events to the store in batches, emit progress to
/// the UI, then record how it ended, run crawl analysis and apply retention.
fn pump(
    app: AppHandle,
    store_ref: Arc<Mutex<Option<Store>>>,
    mut rx: tokio::sync::mpsc::Receiver<CrawlEvent>,
    crawl: tauri::async_runtime::JoinHandle<anyhow::Result<Progress>>,
) {
    tauri::async_runtime::spawn(async move {
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
        let mut queued_at_end = 0;
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
                    queued_at_end = p.queued;
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
                if let Err(e) = store.mark_finished(queued_at_end) {
                    eprintln!("store error: {e}");
                }
                if let Err(e) = store.analyse() {
                    eprintln!("analysis error: {e}");
                }
            }
        }
        settings::apply_retention(&app);
        app.emit("crawl-finished", ()).ok();
    });
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

/// A report as a table (panels that show a report, e.g. Response Times).
#[tauri::command]
fn get_report(state: State<'_, AppState>, name: String) -> CmdResult<Table> {
    if state.store.lock().map_err(err)?.is_none() {
        return Ok(Table::default());
    }
    with_store(&state, |s| s.report(&name))
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
fn get_language_details(state: State<'_, AppState>, url: String) -> CmdResult<Table> {
    with_store(&state, |s| s.language_details(&url))
}

#[tauri::command]
fn get_top_language_errors(state: State<'_, AppState>) -> CmdResult<Table> {
    if state.store.lock().map_err(err)?.is_none() {
        return Ok(Table::default());
    }
    with_store(&state, |s| s.top_language_errors())
}

/// Ignore All (a word), Add to Dictionary (a word, globally) or Ignore rule
/// (a grammar rule): removes those errors from the open crawl without
/// re-crawling, and remembers the choice for future crawls.
#[tauri::command]
fn dismiss_language_error(app: AppHandle, state: State<'_, AppState>, kind: String, text: String, action: String) -> CmdResult<u64> {
    let n = {
        let mut guard = state.store.lock().map_err(err)?;
        let store = guard.as_mut().ok_or("No crawl is open.")?;
        store.dismiss_language_error(&kind, &text).map_err(err)?
    };
    match action.as_str() {
        "dictionary" => settings::add_dictionary_word(&app, &text)?,
        "ignore_rule" => {
            let mut cfg = state.config.lock().map_err(err)?;
            if !cfg.spelling.disabled_grammar_rules.contains(&text) {
                cfg.spelling.disabled_grammar_rules.push(text);
            }
        }
        _ => {
            let mut cfg = state.config.lock().map_err(err)?;
            if !cfg.spelling.ignore_words.iter().any(|w| w.eq_ignore_ascii_case(&text)) {
                cfg.spelling.ignore_words.push(text);
            }
        }
    }
    Ok(n)
}

#[tauri::command]
fn spelling_languages() -> Vec<(String, String)> {
    toad_core::spelling::LANGUAGES.iter().map(|(c, n, _)| (c.to_string(), n.to_string())).collect()
}

#[tauri::command]
fn get_http_headers(state: State<'_, AppState>, url: String) -> CmdResult<Table> {
    with_store(&state, |s| s.http_headers(&url))
}

#[tauri::command]
fn get_cookies(state: State<'_, AppState>, url: String) -> CmdResult<Table> {
    with_store(&state, |s| s.cookies(&url))
}

/// View Source: the stored HTML, or None when Store HTML was off.
#[tauri::command]
fn get_page_source(state: State<'_, AppState>, url: String) -> CmdResult<Option<String>> {
    with_store(&state, |s| s.page_source(&url))
}

/// Bulk Export > Web > All PDF Content.
#[tauri::command]
fn export_pdf_text(state: State<'_, AppState>, dir: String) -> CmdResult<u64> {
    with_store(&state, |s| s.export_pdf_text(std::path::Path::new(&dir)))
}

/// Bulk Export > Web > All Page Source.
#[tauri::command]
fn export_page_source(state: State<'_, AppState>, dir: String) -> CmdResult<u64> {
    with_store(&state, |s| s.export_page_source(std::path::Path::new(&dir)))
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

/// The Test panes of URL Rewriting, CDNs, Include and Exclude.
#[tauri::command]
fn test_url(config: CrawlConfig, start: Option<String>, url: String) -> toad_core::urlutil::UrlTest {
    toad_core::urlutil::test_url(&config, start.as_deref(), &url)
}

/// Custom Search / Extraction: the validity tick per row. None = valid.
#[tauri::command]
fn check_expression(kind: String, expression: String) -> Option<String> {
    if expression.trim().is_empty() {
        return None;
    }
    match kind.as_str() {
        "xpath" => toad_core::xpath::validate(&expression).err(),
        "css" => scraper::Selector::parse(&expression).err().map(|e| format!("invalid CSS selector: {e:?}")),
        "regex" => regex::Regex::new(&expression).err().map(|e| e.to_string()),
        _ => None,
    }
}

/// robots.txt > Test: whether `url` is allowed by `txt` for `user_agent`.
#[tauri::command]
fn test_robots(txt: String, user_agent: String, url: String) -> CmdResult<toad_core::robots::RobotsTest> {
    let path = match Url::parse(url.trim()) {
        Ok(u) => match u.query() {
            Some(q) => format!("{}?{q}", u.path()),
            None => u.path().to_string(),
        },
        // A bare path such as /private/page.
        Err(_) if url.trim().starts_with('/') => url.trim().to_string(),
        Err(e) => return Err(format!("not a valid URL or path: {e}")),
    };
    Ok(toad_core::robots::Robots::parse(&txt).explain(&user_agent, &path))
}

/// robots.txt > Custom Robots > Download: the live robots.txt of a host.
#[tauri::command]
async fn download_robots(state: State<'_, AppState>, host: String) -> CmdResult<String> {
    let cfg = state.config.lock().map_err(err)?.clone();
    let h = host.trim();
    let base = if h.contains("://") { h.to_string() } else { format!("https://{h}") };
    let url = Url::parse(&base).and_then(|u| u.join("/robots.txt")).map_err(err)?;
    let res = toad_core::fetch::Fetcher::new(&cfg).map_err(err)?.fetch(&url).await;
    match res.status {
        200 => Ok(String::from_utf8_lossy(&res.body).into_owned()),
        0 => Err(format!("{url}: no response ({})", res.error.unwrap_or(res.status_text))),
        code => Err(format!("{url} returned {code} {}", res.status_text)),
    }
}

/// Configuration > Exclude edited while a crawl runs: applies to URLs not
/// yet requested. Returns whether a crawl was running.
#[tauri::command]
fn update_exclude(state: State<'_, AppState>, patterns: Vec<String>) -> CmdResult<bool> {
    let control = state.control.lock().map_err(err)?;
    match control.as_ref() {
        Some(c) => {
            c.set_exclude(&patterns).map_err(err)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> CmdResult<CrawlConfig> {
    Ok(state.config.lock().map_err(err)?.clone())
}

#[tauri::command]
fn set_config(state: State<'_, AppState>, config: CrawlConfig) -> CmdResult<()> {
    let problems = config.validate();
    if !problems.is_empty() {
        return Err(problems.join("
"));
    }
    *state.config.lock().map_err(err)? = config;
    Ok(())
}

#[tauri::command]
fn reset_config(app: AppHandle, state: State<'_, AppState>) -> CmdResult<CrawlConfig> {
    // Back to the saved default if there is one, as Screaming Frog does.
    let cfg = profiles::startup_config(&app);
    *state.config.lock().map_err(err)? = cfg.clone();
    Ok(cfg)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            let cfg = profiles::startup_config(app.handle());
            *app.state::<AppState>().config.lock().expect("config lock") = cfg;
            *app.state::<AppState>().settings.lock().expect("settings lock") = settings::load(app.handle());
            settings::apply_retention(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_crawl,
            continue_crawl,
            settings::get_app_settings,
            settings::set_app_settings,
            settings::crawls_list,
            settings::crawl_rename,
            settings::crawl_set_project,
            settings::crawl_set_locked,
            settings::crawl_duplicate,
            settings::crawl_delete,
            settings::crawl_export,
            settings::current_crawl_path,
            settings::fetch_sitemap_urls,
            settings::read_url_file,
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
            get_page_source,
            get_http_headers,
            get_language_details,
            get_top_language_errors,
            dismiss_language_error,
            spelling_languages,
            settings::get_dictionary_words,
            settings::set_dictionary_words,
            get_cookies,
            export_page_source,
            export_pdf_text,
            get_near_duplicates,
            get_reports,
            export_report,
            get_report,
            get_structured_details,
            generate_sitemap,
            export_csv,
            open_crawl,
            save_crawl_as,
            get_config,
            test_url,
            update_exclude,
            test_robots,
            check_expression,
            download_robots,
            set_config,
            reset_config,
            profiles::recent_profiles,
            profiles::load_profile,
            profiles::save_profile,
            profiles::save_default_profile,
            profiles::clear_default_profile,
            profiles::has_default_profile,
            profiles::builtin_config,
            profiles::validate_config,
            compare::list_crawls,
            compare::crawl_info_for,
            compare::get_compare_config,
            compare::set_compare_config,
            compare::test_url_mapping,
            compare::run_compare,
            compare::open_comparison,
            compare::close_comparison,
            compare::compare_summary,
            compare::compare_tabs,
            compare::compare_overview,
            compare::compare_issues,
            compare::compare_rows,
            compare::compare_export,
            compare::compare_export_overview,
            compare::compare_url_details,
            compare::compare_links,
            compare::compare_site_structure,
            compare::compare_source,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Screaming Toad");
}
