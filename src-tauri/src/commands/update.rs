use crate::state;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_updater::{Update, UpdaterExt};
static PENDING: Lazy<Mutex<Option<Update>>> = Lazy::new(|| Mutex::new(None));
static READY: Lazy<Mutex<Option<(Update, Vec<u8>)>>> = Lazy::new(|| Mutex::new(None));
fn say(app: &AppHandle, msg: String) {
    app.dialog().message(msg).title("Haven Desktop").buttons(MessageDialogButtons::OkCustom(state::t(app, "update.ok"))).show(|_| {});
}
pub(crate) fn check_for_update(app: AppHandle, manual: bool) {
    tauri::async_runtime::spawn(async move {
        let t = |k: &str| state::t(&app, k);
        let result = match app.updater() { Ok(u) => u.check().await.map_err(|e| e.to_string()), Err(e) => Err(e.to_string()) };
        let update = match result {
            Ok(Some(u)) => u,
            Ok(None) => return if manual { say(&app, t("update.upToDate").replace("{version}", &app.package_info().version.to_string())) },
            Err(e) => return if manual { say(&app, t("update.error").replace("{error}", &e)) },
        };
        let version = update.version.clone();
        *PENDING.lock() = Some(update);
        let _ = app.emit("update:available", json!({ "version": version }));
        if manual && app.dialog().message(t("update.available").replace("{version}", &version)).title("Haven Desktop").buttons(MessageDialogButtons::OkCancelCustom(t("update.now"), t("dialog.cancel"))).blocking_show() {
            match update_download(app.clone()).await.get("error").and_then(Value::as_str) {
                Some(e) => say(&app, t("update.failed").replace("{error}", e)),
                None => update_install(app.clone()),
            }
        }
    });
}
#[tauri::command]
pub async fn update_download(app: AppHandle) -> Value {
    let Some(update) = PENDING.lock().take() else { return if READY.lock().is_some() { json!({}) } else { json!({ "errorKey": "update.unavailable" }) } };
    let (mut got, h) = (0u64, app.clone());
    let _ = app.emit("update:download-progress", json!({ "percent": 0 }));
    match update.download(move |chunk, total| { got += chunk as u64; if let Some(t) = total.filter(|t| *t > 0) { let _ = h.emit("update:download-progress", json!({ "percent": got * 100 / t })); } }, || {}).await {
        Ok(bytes) => {
            *READY.lock() = Some((update, bytes));
            let _ = app.emit("update:downloaded", ());
            json!({})
        }
        Err(e) => {
            *PENDING.lock() = Some(update);
            let _ = app.emit("update:error", json!({ "message": e.to_string() }));
            json!({ "error": e.to_string() })
        }
    }
}
#[tauri::command]
pub fn update_install(app: AppHandle) {
    let Some((update, bytes)) = READY.lock().take() else { return };
    match update.install(&bytes) {
        Ok(()) => app.restart(),
        Err(e) => say(&app, state::t(&app, "update.failed").replace("{error}", &e.to_string())),
    }
}
