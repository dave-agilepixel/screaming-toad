//! Configuration > Authentication: session-held credentials, the in-crawl
//! login prompt, forms-based login windows and encrypted import/export.
//!
//! Credentials stay in memory for the session. They are never written into
//! crawl files or configuration profiles; Export saves them encrypted.

use crate::{err, AppState, CmdResult};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use toad_core::auth::{AuthConfig, AuthPrompt, FormLogin, Network};
use toad_core::render::LoginWindow;
use url::Url;

#[derive(Clone, Serialize)]
struct AuthRequired {
    id: u64,
    url: String,
    realm: String,
    scheme: String,
}

/// The network for a crawl: File > Settings (proxy, certificates) plus the
/// session's authentication, with prompts shown in the app.
pub fn crawl_network(app: &AppHandle, state: &AppState) -> CmdResult<Arc<Network>> {
    let settings = state.settings.lock().map_err(err)?.network.clone();
    let auth = state.auth.read().map_err(err)?.clone();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<AuthPrompt>(8);
    let pending = state.pending_auth.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut next = 0u64;
        while let Some(prompt) = rx.recv().await {
            next += 1;
            let ev = AuthRequired { id: next, url: prompt.challenge.url.clone(), realm: prompt.challenge.realm.clone(), scheme: prompt.challenge.scheme.clone() };
            if let Ok(mut p) = pending.lock() {
                p.insert(next, prompt.reply);
            }
            let _ = app.emit("auth-required", ev);
        }
    });
    Ok(Network::new(settings, auth, Some(tx)).shared())
}

/// A network without prompts, for one-off requests (robots.txt, sitemaps).
pub fn quiet_network(state: &AppState) -> CmdResult<Arc<Network>> {
    let settings = state.settings.lock().map_err(err)?.network.clone();
    let auth = state.auth.read().map_err(err)?.clone();
    Ok(Network::new(settings, auth, None).shared())
}

#[tauri::command]
pub fn get_auth_config(state: State<'_, AppState>) -> CmdResult<AuthConfig> {
    Ok(state.auth.read().map_err(err)?.clone())
}

#[tauri::command]
pub fn set_auth_config(state: State<'_, AppState>, auth: AuthConfig) -> CmdResult<()> {
    *state.auth.write().map_err(err)? = auth;
    Ok(())
}

/// The login dialog's answer. `None` skips the realm for this crawl.
#[tauri::command]
pub fn answer_auth(state: State<'_, AppState>, id: u64, url: String, credentials: Option<(String, String)>) -> CmdResult<()> {
    if let Some((username, password)) = &credentials {
        // Kept for later crawls in this session too.
        let origin = Url::parse(&url).map(|u| u.origin().ascii_serialization()).unwrap_or(url);
        let mut auth = state.auth.write().map_err(err)?;
        auth.standards.retain(|c| c.url != origin);
        auth.standards.push(toad_core::auth::Credential { url: origin, username: username.clone(), password: password.clone() });
    }
    if let Some(reply) = state.pending_auth.lock().map_err(err)?.remove(&id) {
        let _ = reply.send(credentials);
    }
    Ok(())
}

#[tauri::command]
pub fn export_auth(state: State<'_, AppState>, path: String, password: String) -> CmdResult<()> {
    let auth = state.auth.read().map_err(err)?.clone();
    let text = toad_core::auth::export_encrypted(&auth, &password).map_err(err)?;
    std::fs::write(&path, text).map_err(err)
}

#[tauri::command]
pub fn import_auth(state: State<'_, AppState>, path: String, password: String) -> CmdResult<AuthConfig> {
    let text = std::fs::read_to_string(&path).map_err(err)?;
    let auth = toad_core::auth::import_encrypted(&text, &password).map_err(err)?;
    *state.auth.write().map_err(err)? = auth.clone();
    Ok(auth)
}

/// Forms Based > Add: open a browser window at the login page.
#[tauri::command]
pub async fn start_form_login(state: State<'_, AppState>, url: String) -> CmdResult<()> {
    let u = url.trim();
    let url = Url::parse(if u.contains("://") { u.to_string() } else { format!("https://{u}") }.as_str()).map_err(err)?;
    let rendering = state.config.lock().map_err(err)?.rendering.clone();
    let network = quiet_network(&state)?;
    let mut slot = state.login.lock().await;
    if let Some(old) = slot.take() {
        old.close().await;
    }
    *slot = Some(LoginWindow::open(&rendering, &network, &url).await.map_err(err)?);
    Ok(())
}

/// Forms Based > Done: capture the window's cookies as a login.
#[tauri::command]
pub async fn finish_form_login(state: State<'_, AppState>, name: String, login_url: String) -> CmdResult<FormLogin> {
    let window = state.login.lock().await.take().ok_or("No login window is open.")?;
    let cookies = window.cookies().await;
    window.close().await;
    let cookies = cookies.map_err(err)?;
    if cookies.is_empty() {
        return Err("The browser has no cookies. Log in before clicking Done.".into());
    }
    let login = FormLogin { name, login_url, cookies };
    let mut auth = state.auth.write().map_err(err)?;
    auth.forms.retain(|f| f.name != login.name);
    auth.forms.push(login.clone());
    Ok(login)
}

#[tauri::command]
pub async fn cancel_form_login(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(w) = state.login.lock().await.take() {
        w.close().await;
    }
    Ok(())
}
