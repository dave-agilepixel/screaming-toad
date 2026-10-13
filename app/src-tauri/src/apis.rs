//! Configuration > API Access in the desktop app: keys in the OS credential
//! store, Google sign-in, property lists, and fetching API data for a crawl.

use crate::{err, AppState, CmdResult};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use toad_apis::{keys, ApiRunner, Secrets};
use toad_core::CrawlConfig;

const SERVICE: &str = "Screaming Toad";

fn entry(key: &str) -> CmdResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, key).map_err(err)
}

fn read_secret(key: &str) -> Option<String> {
    entry(key).ok()?.get_password().ok().filter(|s| !s.is_empty())
}

/// Every stored secret, for one run.
pub fn load_secrets() -> Secrets {
    Secrets::new(keys::ALL.iter().filter_map(|k| read_secret(k).map(|v| (k.to_string(), v))).collect())
}

/// An API runner for a crawl, reporting progress to the UI.
pub fn runner(app: &AppHandle, cfg: &CrawlConfig, start: Option<url::Url>) -> Arc<ApiRunner> {
    let app = app.clone();
    let mut r = ApiRunner::new(cfg.clone(), load_secrets()).with_progress(Arc::new(move |p| {
        let _ = app.emit("api-progress", p);
    }));
    if let Some(s) = start {
        r = r.with_site(s);
    }
    Arc::new(r)
}

/// Which secrets are set (never their values).
#[tauri::command]
pub fn api_secret_status() -> HashMap<String, bool> {
    keys::ALL.iter().map(|k| (k.to_string(), read_secret(k).is_some())).collect()
}

/// Save a key; an empty value removes it.
#[tauri::command]
pub fn set_api_secret(key: String, value: String) -> CmdResult<()> {
    if !keys::ALL.contains(&key.as_str()) {
        return Err(format!("unknown key {key}"));
    }
    let e = entry(&key)?;
    if value.trim().is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(x) => Err(err(x)),
        }
    } else {
        e.set_password(value.trim()).map_err(err)
    }
}

/// Open a URL in the default browser.
fn open_url(url: &str) {
    #[cfg(windows)]
    let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// Google > Connect: sign in in the browser, keep the refresh token.
#[tauri::command]
pub async fn google_connect() -> CmdResult<()> {
    let id = read_secret(keys::GOOGLE_CLIENT_ID).ok_or("Enter the Google OAuth client ID first.")?;
    let secret = read_secret(keys::GOOGLE_CLIENT_SECRET).ok_or("Enter the Google OAuth client secret first.")?;
    let client = reqwest::Client::new();
    let token = toad_apis::google::authorize(&client, &toad_apis::Endpoints::default(), &id, &secret, open_url).await.map_err(err)?;
    set_api_secret(keys::GOOGLE_REFRESH_TOKEN.into(), token)
}

#[tauri::command]
pub fn google_disconnect() -> CmdResult<()> {
    set_api_secret(keys::GOOGLE_REFRESH_TOKEN.into(), String::new())
}

#[derive(Serialize)]
pub struct GoogleProperties {
    gsc: Vec<String>,
    ga4: Vec<(String, String)>,
    errors: Vec<String>,
}

/// The Search Console and GA4 properties the signed-in account can read.
#[tauri::command]
pub async fn google_properties(state: State<'_, AppState>) -> CmdResult<GoogleProperties> {
    let cfg = state.config.lock().map_err(err)?.clone();
    let r = ApiRunner::new(cfg, load_secrets());
    let mut errors = Vec::new();
    let gsc = r.gsc_properties().await.unwrap_or_else(|e| {
        errors.push(format!("Search Console: {e:#}"));
        vec![]
    });
    let ga4 = r.ga4_properties().await.unwrap_or_else(|e| {
        errors.push(format!("GA4: {e:#}"));
        vec![]
    });
    Ok(GoogleProperties { gsc, ga4, errors })
}

/// Test an AI prompt on one URL's text (the prompt editor's Test button).
#[tauri::command]
pub async fn test_ai_prompt(state: State<'_, AppState>, prompt: toad_core::api_config::AiPrompt, text: String) -> CmdResult<String> {
    let cfg = state.config.lock().map_err(err)?.clone();
    let r = ApiRunner::new(cfg, load_secrets());
    r.complete(prompt.provider, &prompt.model, &prompt.prompt, &text).await.map_err(err)
}

/// Request API Data: every enabled source for the open crawl. Pages that
/// already have a source's data are skipped. Returns the errors met.
#[tauri::command]
pub async fn request_api_data(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    let (cfg, start) = {
        let guard = state.store.lock().map_err(err)?;
        let store = guard.as_ref().ok_or("No crawl is open.")?;
        // The open crawl's own settings, with API Access from the current configuration.
        let mut cfg = store.config().clone();
        cfg.apis = state.config.lock().map_err(err)?.apis.clone();
        (cfg, store.meta("start_url").map_err(err)?.and_then(|u| url::Url::parse(&u).ok()))
    };
    if !cfg.apis.any_enabled() {
        return Err("No API is switched on. Turn them on in Configuration > API Access.".into());
    }
    {
        let mut guard = state.store.lock().map_err(err)?;
        if let Some(s) = guard.as_mut() {
            // The tabs' columns follow the API Access settings.
            s.set_api_config(cfg.apis.clone()).map_err(err)?;
        }
    }
    let r = runner(&app, &cfg, start);
    app.emit("crawl-analysing", ()).ok();
    let errors = r.request_all(state.store.as_ref()).await;
    if let Ok(mut guard) = state.store.lock() {
        if let Some(s) = guard.as_mut() {
            let _ = s.analyse();
        }
    }
    app.emit("crawl-finished", ()).ok();
    Ok(errors)
}
