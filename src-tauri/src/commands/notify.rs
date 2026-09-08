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

    // Desktop toasts through notify-rust carry no click callback, so there is
    // nothing to emit here. Emitting "notification-clicked" on show made the
    // web app switch channels every time a message arrived.
    let _ = opts.channel_code;
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
    // Red dot on the taskbar icon while anything is unread (Electron's overlay icon).
    if let Some(main) = app.get_webview_window("main") {
        let any = badges.values().any(|v| *v);
        let dot = tauri::image::Image::from_bytes(include_bytes!("../../icons/unread.png")).ok();
        let _ = main.set_overlay_icon(if any { dot } else { None });
    }
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
