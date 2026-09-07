use crate::state::{self, AppState};
use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifyOpts {
    pub title: Option<String>,
    pub body: Option<String>,
    pub silent: Option<bool>,
    pub channel_code: Option<String>,
}

#[tauri::command]
pub fn notify(app: AppHandle, opts: NotifyOpts) -> Result<bool, String> {
    let title = opts.title.unwrap_or_else(|| "Haven".into());
    let body = opts.body.unwrap_or_default();
    let mut builder = app.notification().builder().title(title).body(body);
    if opts.silent.unwrap_or(false) {
        builder = builder.silent();
    }
    builder.show().map_err(|e| e.to_string())?;

    if let Some(code) = opts.channel_code {
        let _ = app.emit("notification-clicked", code);
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main;
    }
    Ok(true)
}

#[tauri::command]
pub fn notification_badge(
    app: AppHandle,
    state: State<AppState>,
    has_unread: bool,
) -> Result<(), String> {
    if let Some(url) = state.active_server_url.lock().clone() {
        state.server_badges.lock().insert(url, has_unread);
    }
    let badges = state.server_badges.lock().clone();
    let payload = json!({ "badges": badges, "names": {} });
    let _ = app.emit("server-badge-update", payload);
    Ok(())
}

#[tauri::command]
pub fn get_server_badges(state: State<AppState>) -> Result<serde_json::Value, String> {
    let badges = state.server_badges.lock().clone();
    Ok(json!({ "badges": badges, "names": {} }))
}

#[tauri::command]
pub fn report_known_server_urls(
    state: State<AppState>,
    urls: Vec<String>,
) -> Result<(), String> {
    let active = state.active_server_url.lock().clone();
    if let Some(active) = active {
        let set = urls
            .into_iter()
            .filter_map(|u| state::normalize_server_url(&u))
            .collect();
        state.known_server_urls.lock().insert(active, set);
    }
    Ok(())
}
