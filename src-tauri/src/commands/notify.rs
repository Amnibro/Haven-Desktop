use crate::state::{self, AppState};
use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, State};
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifyOpts {
    pub title: Option<String>,
    pub body: Option<String>,
    pub silent: Option<bool>,
    pub channel_code: Option<String>,
}

#[cfg(target_os = "linux")]
static WAITING: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_os = "linux")]
fn open_from_notification(app: &AppHandle, channel_code: Option<&str>) {
    let Some(main) = app.get_webview_window("main") else { return };
    let _ = main.show();
    let _ = main.unminimize();
    let _ = main.set_focus();
    let _ = channel_code.map(|c| app.emit_to("main", "notification-clicked", c));
}
#[tauri::command]
pub fn notify(app: AppHandle, opts: NotifyOpts) -> Result<bool, String> {
    let _ = app.get_webview_window("main").filter(|m| !m.is_focused().unwrap_or(true)).map(|m| m.request_user_attention(Some(tauri::UserAttentionType::Informational)));
    let title = opts.title.unwrap_or_else(|| "Haven".into());
    let body = opts.body.unwrap_or_default();
    #[cfg(target_os = "linux")]
    if WAITING.load(Ordering::Relaxed) < 8 {
        let mut n = notify_rust::Notification::new();
        n.summary(&title).body(&body).auto_icon().action("default", "Open");
        let _ = opts.silent.unwrap_or(false).then(|| n.hint(notify_rust::Hint::SuppressSound(true)));
        let handle = n.show().map_err(|e| e.to_string())?;
        let code = opts.channel_code.clone();
        WAITING.fetch_add(1, Ordering::Relaxed);
        std::thread::spawn(move || {
            handle.wait_for_action(|action| {
                let _ = (action == "default").then(|| open_from_notification(&app, code.as_deref()));
            });
            WAITING.fetch_sub(1, Ordering::Relaxed);
        });
        return Ok(true);
    }
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

#[cfg(target_os = "macos")]
extern "C" {
    fn haven_macos_set_badge_label(label: *const std::ffi::c_char);
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
    #[cfg(any(windows, target_os = "macos"))]
    let any = badges.values().any(|v| *v);
    // Red dot on the taskbar icon while anything is unread (Electron's overlay
    // icon). Tauri only implements set_overlay_icon on Windows.
    #[cfg(windows)]
    if let Some(main) = app.get_webview_window("main") {
        let dot = tauri::image::Image::from_bytes(include_bytes!("../../icons/unread.png")).ok();
        let _ = main.set_overlay_icon(if any { dot } else { None });
    }
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        let c_label = if any {
            CString::new("•").ok()
        } else {
            None
        };
        unsafe {
            haven_macos_set_badge_label(c_label.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()));
        }
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
