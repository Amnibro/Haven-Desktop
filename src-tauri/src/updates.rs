use crate::state::{self, AppState};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::{Update, UpdaterExt};
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
#[derive(Default)]
pub struct UpdateState { found: Option<Update>, bytes: Option<Vec<u8>>, dismissed: bool, downloading: bool }
impl UpdateState {
    fn available(&mut self, update: Update, asked: bool) -> bool {
        let fresh = self.found.as_ref().map(|f| f.version != update.version).unwrap_or(true);
        if fresh { *self = UpdateState { found: Some(update), ..Default::default() }; return true }
        self.dismissed = self.dismissed && !asked;
        self.bytes.is_none() && !self.dismissed
    }
    fn for_page(&self) -> Value {
        match &self.found { Some(f) if !self.dismissed => json!({ "status": if self.bytes.is_some() { "downloaded" } else { "available" }, "version": f.version }), _ => Value::Null }
    }
    fn should_check(&self) -> bool { !self.downloading && self.bytes.is_none() }
}
fn with<T>(app: &AppHandle, f: impl FnOnce(&mut UpdateState) -> T) -> T { let st = app.state::<AppState>(); let mut s = st.update.lock(); f(&mut s) }
fn unsupported() -> bool { std::env::var_os("FLATPAK_ID").is_some() || std::path::Path::new("/.flatpak-info").exists() }
fn tell(app: &AppHandle, event: &str, payload: Value) { let _ = app.emit_to("main", event, payload); }
fn notice(app: &AppHandle, kind: MessageDialogKind, message: String) {
    app.dialog().message(message).title(state::t(app, "menu.checkForUpdates")).kind(kind).buttons(MessageDialogButtons::OkCustom(state::t(app, "dialog.ok"))).show(|_| {});
}
pub fn start(app: AppHandle) {
    if unsupported() { return }
    tauri::async_runtime::spawn(async move {
        loop {
            let due = with(&app, |s| s.should_check());
            if due { check(&app, false).await }
            if cfg!(debug_assertions) { break }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}
pub async fn check(app: &AppHandle, asked: bool) {
    if asked && (unsupported() || cfg!(debug_assertions)) { return notice(app, MessageDialogKind::Info, state::t(app, if unsupported() { "update.unavailable" } else { "update.notPackaged" })) }
    let result = match app.updater() { Ok(u) => u.check().await.map_err(|e| e.to_string()), Err(e) => Err(e.to_string()) };
    match result {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let (show, ready) = with(app, |s| (s.available(update, asked), s.bytes.is_some()));
            if show { tell(app, "update:available", json!({ "version": version })) } else if asked && ready { tell(app, "update:downloaded", Value::Null) }
            let main = app.get_webview_window("main").filter(|m| m.is_visible().unwrap_or(false));
            match (asked, main) {
                (true, Some(m)) => { let _ = m.unminimize(); let _ = m.set_focus(); }
                (true, None) => offer_install(app.clone(), version),
                _ => {}
            }
        }
        Ok(None) if asked => notice(app, MessageDialogKind::Info, state::tv(app, "update.upToDate", &[("version", &app.package_info().version.to_string())])),
        Err(e) if asked => notice(app, MessageDialogKind::Error, state::tv(app, "update.error", &[("error", &e)])),
        _ => {}
    }
}
fn offer_install(app: AppHandle, version: String) {
    let msg = state::tv(&app, "update.available", &[("version", &version)]);
    let worker = app.clone();
    app.dialog().message(msg).title(state::t(&app, "menu.checkForUpdates")).buttons(MessageDialogButtons::OkCancelCustom(state::t(&app, "update.now"), state::t(&app, "update.later"))).show(move |yes| {
        let _ = yes.then(|| tauri::async_runtime::spawn(async move { if download(&worker).await.is_ok() { install(&worker) } }));
    });
}
async fn download(app: &AppHandle) -> Result<(), String> {
    let (ready, update) = with(app, |s| { s.downloading = s.bytes.is_none() && s.found.is_some(); (s.bytes.is_some(), s.found.clone()) });
    if ready { return Ok(()) }
    let Some(update) = update else { return Err(state::t(app, "update.unavailable")) };
    let (mut got, mut shown) = (0usize, -1i64);
    let result = update.download(|chunk, total| {
        got += chunk;
        let percent = total.filter(|t| *t > 0).map(|t| (got as f64 * 100.0 / t as f64).round() as i64).unwrap_or(-1);
        let _ = (percent >= 0 && percent != shown).then(|| { shown = percent; tell(app, "update:download-progress", json!({ "percent": percent })) });
    }, || {}).await.map_err(|e| e.to_string());
    let ok = with(app, |s| { s.downloading = false; result.map(|bytes| { s.bytes = Some(bytes); s.dismissed = false; }) });
    match &ok { Ok(()) => tell(app, "update:downloaded", Value::Null), Err(e) => tell(app, "update:error", json!({ "message": e })) }
    ok
}
fn install(app: &AppHandle) {
    let (update, bytes) = with(app, |s| (s.found.clone(), s.bytes.clone()));
    let (Some(update), Some(bytes)) = (update, bytes) else { return };
    let _ = app.state::<AppState>().server.lock().stop_server();
    if update.install(bytes).is_ok() { app.restart() } else { tell(app, "update:error", json!({ "message": state::t(app, "update.installFailed") })) }
}
#[tauri::command]
pub fn update_state(app: AppHandle) -> Value {
    let page = with(&app, |s| s.for_page());
    let labels: serde_json::Map<String, Value> = ["available", "now", "downloading", "downloadingProgress", "downloaded", "restartNow", "failed", "error", "close"].iter().map(|k| (k.to_string(), json!(state::t(&app, &format!("update.{k}"))))).collect();
    json!({ "state": page, "labels": labels })
}
#[tauri::command]
pub async fn update_download(app: AppHandle) -> Value {
    match download(&app).await { Ok(()) => json!({ "success": true }), Err(e) => json!({ "error": e }) }
}
#[tauri::command]
pub fn update_install(app: AppHandle) { install(&app) }
#[tauri::command]
pub fn update_dismiss(app: AppHandle) { with(&app, |s| s.dismissed = s.found.is_some()) }
