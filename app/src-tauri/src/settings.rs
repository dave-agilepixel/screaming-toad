//! File > Settings (storage folder, crawl retention, last mode) and
//! File > Crawls (the saved-crawl browser).

use crate::{err, AppState, CmdResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};
use toad_core::fetch::Fetcher;
use toad_core::sitemap::{self, SitemapKind};
use toad_store::crawls::{self, CrawlEntry};
use toad_store::Store;
use url::Url;

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// Storage > Directory: where crawls are saved. None = the app data folder.
    pub storage_dir: Option<String>,
    /// Crawl Retention: delete unlocked crawls older than this many days. None = Never.
    pub retention_days: Option<u64>,
    /// Mode menu: the last mode used ("spider", "list" or "compare").
    pub last_mode: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { storage_dir: None, retention_days: None, last_mode: "spider".into() }
    }
}

fn settings_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join(SETTINGS_FILE))
}

pub fn load(app: &AppHandle) -> AppSettings {
    settings_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// The folder crawls are saved in (created if missing).
pub fn crawls_dir(app: &AppHandle, state: &AppState) -> CmdResult<PathBuf> {
    let custom = state.settings.lock().map_err(err)?.storage_dir.clone().filter(|d| !d.trim().is_empty());
    let dir = match custom {
        Some(d) => PathBuf::from(d),
        None => app.path().app_data_dir().map_err(err)?.join("crawls"),
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot use the storage folder {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Crawl Retention: run at startup and after each crawl. Never touches the
/// open crawl, locked crawls or a crawl in progress.
pub fn apply_retention(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(days) = state.settings.lock().ok().and_then(|s| s.retention_days) else { return };
    let Ok(dir) = crawls_dir(app, &state) else { return };
    let open = state.crawl_path.lock().ok().and_then(|p| p.clone());
    for p in crawls::apply_retention(&dir, days, open.as_deref()) {
        eprintln!("crawl retention: deleted {}", p.display());
    }
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> CmdResult<AppSettings> {
    Ok(state.settings.lock().map_err(err)?.clone())
}

#[tauri::command]
pub fn set_app_settings(app: AppHandle, state: State<'_, AppState>, settings: AppSettings) -> CmdResult<()> {
    if let Some(dir) = settings.storage_dir.as_deref().filter(|d| !d.trim().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot use the storage folder {dir}: {e}"))?;
    }
    if settings.retention_days == Some(0) {
        return Err("Crawl Retention needs at least 1 day.".into());
    }
    let path = settings_path(&app).ok_or("no settings folder")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&settings).map_err(err)?).map_err(err)?;
    *state.settings.lock().map_err(err)? = settings;
    apply_retention(&app);
    Ok(())
}

/// File > Crawls: every saved crawl, newest first.
#[tauri::command]
pub fn crawls_list(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Vec<CrawlEntry>> {
    Ok(crawls::list(&crawls_dir(&app, &state)?))
}

#[tauri::command]
pub fn current_crawl_path(state: State<'_, AppState>) -> CmdResult<Option<String>> {
    Ok(state.crawl_path.lock().map_err(err)?.as_ref().map(|p| p.to_string_lossy().into_owned()))
}

/// Whether `path` is the crawl the app has open (its file is in use).
fn is_open(state: &AppState, path: &Path) -> bool {
    let open = state.crawl_path.lock().ok().and_then(|p| p.clone());
    open.is_some_and(|o| std::fs::canonicalize(o).ok() == std::fs::canonicalize(path).ok())
}

#[tauri::command]
pub fn crawl_rename(path: String, name: String) -> CmdResult<()> {
    crawls::rename(Path::new(&path), &name).map_err(err)
}

#[tauri::command]
pub fn crawl_set_project(path: String, project: String) -> CmdResult<()> {
    crawls::set_project(Path::new(&path), &project).map_err(err)
}

#[tauri::command]
pub fn crawl_set_locked(path: String, locked: bool) -> CmdResult<()> {
    crawls::set_locked(Path::new(&path), locked).map_err(err)
}

#[tauri::command]
pub fn crawl_duplicate(path: String) -> CmdResult<String> {
    crawls::duplicate(Path::new(&path)).map(|p| p.to_string_lossy().into_owned()).map_err(err)
}

#[tauri::command]
pub fn crawl_delete(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    if is_open(&state, Path::new(&path)) {
        return Err("This crawl is open. Open or start another crawl before deleting it.".into());
    }
    let in_comparison = state
        .compare
        .lock()
        .map_err(err)?
        .as_ref()
        .is_some_and(|c| [c.current_path(), c.previous_path()].iter().any(|p| std::fs::canonicalize(p).ok() == std::fs::canonicalize(&path).ok()));
    if in_comparison {
        return Err("This crawl is part of the open comparison. Clear the comparison first.".into());
    }
    crawls::delete(Path::new(&path)).map_err(err)
}

/// Export: a compacted copy of the crawl at `dest`.
#[tauri::command]
pub fn crawl_export(path: String, dest: String) -> CmdResult<()> {
    Store::open(Path::new(&path)).and_then(|s| s.save_copy(Path::new(&dest))).map_err(err)
}

/// List mode > Download XML Sitemap: the page URLs in one or more sitemaps
/// (sitemap indexes are followed, up to 500 files).
#[tauri::command]
pub async fn fetch_sitemap_urls(state: State<'_, AppState>, urls: Vec<String>) -> CmdResult<Vec<String>> {
    let cfg = state.config.lock().map_err(err)?.clone();
    let fetcher = Fetcher::new(&cfg).map_err(err)?;
    let mut pending: Vec<String> = urls.into_iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect();
    let mut fetched = std::collections::HashSet::new();
    let mut out = Vec::new();
    while let Some(next) = pending.pop() {
        if fetched.len() >= 500 || !fetched.insert(next.clone()) {
            continue;
        }
        let url = Url::parse(&next).map_err(|e| format!("{next}: {e}"))?;
        let res = fetcher.fetch(&url).await;
        if res.status != 200 {
            return Err(format!("{next} returned {} {}", res.status, res.status_text));
        }
        let parsed = sitemap::parse(&res.body);
        match parsed.kind {
            SitemapKind::Index => pending.extend(parsed.locs),
            SitemapKind::UrlSet => out.extend(parsed.locs),
        }
    }
    Ok(out)
}
