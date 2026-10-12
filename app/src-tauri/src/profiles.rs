//! Configuration > Profiles: load and save `.toadconfig` files, keep a list
//! of recent profiles, and the saved default the app starts with.

use crate::{err, AppState, CmdResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};
use toad_core::CrawlConfig;

const DEFAULT_FILE: &str = "default.toadconfig";
const RECENT_FILE: &str = "recent_profiles.json";
const MAX_RECENT: usize = 10;

fn config_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    let dir = app.path().app_config_dir().map_err(err)?;
    std::fs::create_dir_all(&dir).map_err(err)?;
    Ok(dir)
}

/// The configuration the app starts with: the saved default if there is one
/// (and it loads), otherwise the built-in defaults.
pub fn startup_config(app: &AppHandle) -> CrawlConfig {
    config_dir(app)
        .ok()
        .map(|d| d.join(DEFAULT_FILE))
        .filter(|p| p.exists())
        .and_then(|p| match CrawlConfig::load(&p) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                eprintln!("ignoring saved default configuration {}: {e}", p.display());
                None
            }
        })
        .unwrap_or_default()
}

fn recent(app: &AppHandle) -> Vec<String> {
    config_dir(app)
        .ok()
        .and_then(|d| std::fs::read_to_string(d.join(RECENT_FILE)).ok())
        .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
        .unwrap_or_default()
}

fn remember(app: &AppHandle, path: &Path) -> CmdResult<()> {
    let p = path.to_string_lossy().into_owned();
    let mut list = recent(app);
    list.retain(|x| x != &p);
    list.insert(0, p);
    list.truncate(MAX_RECENT);
    std::fs::write(config_dir(app)?.join(RECENT_FILE), serde_json::to_string_pretty(&list).map_err(err)?).map_err(err)
}

#[derive(Serialize)]
pub struct ProfileInfo {
    path: String,
    name: String,
    exists: bool,
}

/// Load Recent: newest first; missing files are flagged rather than hidden.
#[tauri::command]
pub fn recent_profiles(app: AppHandle) -> CmdResult<Vec<ProfileInfo>> {
    Ok(recent(&app)
        .into_iter()
        .map(|p| {
            let path = PathBuf::from(&p);
            ProfileInfo {
                name: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone()),
                exists: path.exists(),
                path: p,
            }
        })
        .collect())
}

/// Load…: read a profile and make it the current configuration.
#[tauri::command]
pub fn load_profile(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<CrawlConfig> {
    let cfg = CrawlConfig::load(Path::new(&path)).map_err(|e| format!("{path} is not a valid configuration profile: {e}"))?;
    *state.config.lock().map_err(err)? = cfg.clone();
    remember(&app, Path::new(&path))?;
    Ok(cfg)
}

/// Save As…: write the given configuration to a profile file.
#[tauri::command]
pub fn save_profile(app: AppHandle, path: String, config: CrawlConfig) -> CmdResult<()> {
    config.save(Path::new(&path)).map_err(err)?;
    remember(&app, Path::new(&path))
}

/// Save Current Configuration as Default.
#[tauri::command]
pub fn save_default_profile(app: AppHandle, state: State<'_, AppState>, config: CrawlConfig) -> CmdResult<()> {
    config.save(&config_dir(&app)?.join(DEFAULT_FILE)).map_err(err)?;
    *state.config.lock().map_err(err)? = config;
    Ok(())
}

/// Clear Default Configuration: remove the saved default and go back to the
/// built-in defaults.
#[tauri::command]
pub fn clear_default_profile(app: AppHandle, state: State<'_, AppState>) -> CmdResult<CrawlConfig> {
    let path = config_dir(&app)?.join(DEFAULT_FILE);
    if path.exists() {
        std::fs::remove_file(&path).map_err(err)?;
    }
    let cfg = CrawlConfig::default();
    *state.config.lock().map_err(err)? = cfg.clone();
    Ok(cfg)
}

#[tauri::command]
pub fn has_default_profile(app: AppHandle) -> CmdResult<bool> {
    Ok(config_dir(&app)?.join(DEFAULT_FILE).exists())
}

/// The built-in defaults (for "Reset" and the diff-against-default view).
#[tauri::command]
pub fn builtin_config() -> CrawlConfig {
    CrawlConfig::default()
}

/// Problems with a configuration; empty means valid.
#[tauri::command]
pub fn validate_config(config: CrawlConfig) -> Vec<String> {
    config.validate()
}
