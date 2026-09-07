use crate::state::{self, AppState};
use serde_json::json;
use std::sync::mpsc;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn server_detect(app: AppHandle, state: State<AppState>) -> Result<serde_json::Value, String> {
    let saved = state::get_user_prefs(&app)?
        .get("serverPath")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let mgr = state.server.lock();
    let result = mgr.detect_server(saved.as_deref());
    Ok(serde_json::to_value(result).unwrap_or(json!({ "found": false })))
}

#[tauri::command]
pub fn server_start(
    app: AppHandle,
    state: State<AppState>,
    dir: String,
) -> Result<serde_json::Value, String> {
    let (tx, rx) = mpsc::channel::<String>();
    {
        let mut mgr = state.server.lock();
        mgr.set_log_sender(tx);
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            let _ = handle.emit("server:log", msg);
        }
    });

    let mut mgr = state.server.lock();
    let result = mgr.start_server(&dir);
    if result.success {
        let mut prefs = state::get_user_prefs(&app)?;
        if let Some(obj) = prefs.as_object_mut() {
            obj.insert("serverPath".into(), json!(dir));
        }
        state::set_value(&app, "userPrefs", prefs)?;
    }
    Ok(serde_json::to_value(result).unwrap_or(json!({ "success": false })))
}

#[tauri::command]
pub fn server_stop(state: State<AppState>) -> Result<serde_json::Value, String> {
    let mut mgr = state.server.lock();
    Ok(serde_json::to_value(mgr.stop_server()).unwrap_or(json!({})))
}

#[tauri::command]
pub fn server_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let mgr = state.server.lock();
    Ok(serde_json::to_value(mgr.status()).unwrap_or(json!({})))
}

#[tauri::command]
pub fn server_browse(app: AppHandle) -> Result<Option<String>, String> {
    let folder = app
        .dialog()
        .file()
        .set_title(state::t(&app, "dialog.selectServerDirectory"))
        .blocking_pick_folder();
    Ok(folder.map(|p| p.to_string()))
}
