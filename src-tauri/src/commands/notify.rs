use crate::state::{self, AppState};
use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, State};
#[cfg(windows)]
use tauri::Manager;
#[cfg(not(target_os = "linux"))]
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
    #[cfg(target_os = "linux")]
    return crate::linux_desktop::notify(&app, &title, &body, opts.silent.unwrap_or(false), opts.channel_code).map(|_| true);
    #[cfg(not(target_os = "linux"))]
    {
        let mut builder = app.notification().builder().title(title).body(body);
        if opts.silent.unwrap_or(false) {
            builder = builder.silent();
        }
        builder.show().map_err(|e| e.to_string())?;
        let _ = opts.channel_code;
        Ok(true)
    }
}

fn server_names(app: &AppHandle) -> serde_json::Map<String, serde_json::Value> {
    state::get_value(app, "serverHistory").ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|e| {
        let url = state::normalize_server_url(e.get("url")?.as_str()?)?;
        let name = e.get("name").and_then(|n| n.as_str()).filter(|n| !n.is_empty() && *n != e.get("url").and_then(|u| u.as_str()).unwrap_or_default()).map(String::from).or_else(|| url::Url::parse(&url).ok().and_then(|u| u.host_str().map(String::from))).unwrap_or_else(|| url.clone());
        Some((url, json!(name)))
    }).collect()
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
    let payload = json!({ "badges": badges, "names": server_names(&app) });
    let _ = app.emit("server-badge-update", payload);
    // Red dot on the taskbar icon while anything is unread (Electron's overlay
    // icon). Tauri only implements set_overlay_icon on Windows.
    #[cfg(target_os = "linux")]
    crate::linux_desktop::set_unread(&app, badges.values().filter(|v| **v).count());
    #[cfg(windows)]
    if let Some(main) = app.get_webview_window("main") {
        let any = badges.values().any(|v| *v);
        let dot = tauri::image::Image::from_bytes(include_bytes!("../../icons/unread.png")).ok();
        let _ = main.set_overlay_icon(if any { dot } else { None });
    }
    Ok(())
}

#[tauri::command]
pub fn get_server_badges(app: AppHandle, state: State<AppState>) -> Result<serde_json::Value, String> {
    let badges = state.server_badges.lock().clone();
    Ok(json!({ "badges": badges, "names": server_names(&app) }))
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
