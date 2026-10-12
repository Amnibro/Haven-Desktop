use crate::state::{self, AppState};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use url::Url;

const INJECT_SCRIPT: &str = include_str!("../../inject/app-bridge.js");
const POPUP_CLOSE_TITLE: &str = "\u{2063}haven-popup-close";
const POPUP_CLOSE_SCRIPT: &str = "(()=>{const c=window.close.bind(window);window.close=function(){if(window.top===window)document.title='\\u2063haven-popup-close';c();};})();";

/// Haven servers ship a self-signed certificate by default. The Electron app
/// accepted every certificate (certificate-error + setCertificateVerifyProc);
/// WebView2 has no per-request hook, so the flag goes on the browser process.
/// Must match `additionalBrowserArgs` on the welcome window in tauri.conf.json,
/// because WebView2 only honours the args of the first webview it creates.
#[cfg(windows)]
pub const BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --ignore-certificate-errors";

/// Embed origins the Haven web UI loads in iframes / navigations.
const EMBED_ORIGINS: [&str; 5] = [
    "https://w.soundcloud.com",
    "https://open.spotify.com",
    "https://www.youtube.com",
    "https://www.youtube-nocookie.com",
    "https://challenges.cloudflare.com",
];

// Linux counterpart of `--ignore-certificate-errors`: WebKitGTK rejects
// self-signed certificates unless the web context's TLS policy says otherwise.
// On macOS, our Objective-C runtime swizzling in native/src/macos/macos_ssl.mm
// handles self-signed server certificates seamlessly.
#[cfg(target_os = "macos")]
extern "C" {
    fn haven_macos_accept_self_signed(webview: *mut std::ffi::c_void);
}

pub fn accept_self_signed(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::{SettingsExt, WebViewExt};
        let (app, label) = (window.app_handle().clone(), window.label().to_string());
        let _ = window.with_webview(move |w| {
            crate::cert_trust::attach(&w.inner(), app.clone(), label.clone());
            let _ = (label == "main").then(|| crate::cert_trust::prewarm(&w.inner(), &app));
            if let Some(settings) = w.inner().settings() {
                settings.set_enable_webrtc(true);
                settings.set_enable_media_stream(true);
                settings.set_javascript_can_open_windows_automatically(true);
            }
            attach_script_dialogs(&w.inner());
        });
    }
    #[cfg(target_os = "macos")]
    {
        let _ = window.with_webview(|w| {
            let inner = w.inner();
            unsafe {
                haven_macos_accept_self_signed(inner);
            }
        });
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = window;
}

/// alert / confirm / prompt as real GTK dialogs. WebKitGTK's own in-view
/// overlay never appeared in this app, so confirm() answered on its own and
/// Haven's "remove server?" / "delete user?" checks never really asked.
/// Answering from the signal keeps the page's call synchronous, like a browser.
#[cfg(target_os = "linux")]
fn attach_script_dialogs(view: &webkit2gtk::WebView) {
    use gtk::prelude::*;
    use webkit2gtk::{ScriptDialogType, WebViewExt};
    view.connect_script_dialog(|view, dialog| {
        let kind = dialog.dialog_type();
        let message = dialog.message().map(|m| m.to_string()).unwrap_or_default();
        let parent = view.toplevel().and_then(|t| t.downcast::<gtk::Window>().ok());
        let (msg_type, buttons) = match kind {
            ScriptDialogType::Alert => (gtk::MessageType::Info, gtk::ButtonsType::Ok),
            _ => (gtk::MessageType::Question, gtk::ButtonsType::OkCancel),
        };
        let md = gtk::MessageDialog::new(
            parent.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            msg_type,
            buttons,
            &message,
        );
        md.set_title("Haven");
        let entry = (kind == ScriptDialogType::Prompt).then(|| {
            let entry = gtk::Entry::new();
            entry.set_text(dialog.prompt_get_default_text().as_deref().unwrap_or(""));
            entry.set_activates_default(true);
            md.content_area().pack_end(&entry, false, false, 6);
            entry.show();
            entry
        });
        md.set_default_response(gtk::ResponseType::Ok);
        let ok = md.run() == gtk::ResponseType::Ok;
        match kind {
            ScriptDialogType::Confirm | ScriptDialogType::BeforeUnloadConfirm => {
                dialog.confirm_set_confirmed(ok)
            }
            // Leaving the text unset is how WebKit reports a cancelled prompt (null).
            ScriptDialogType::Prompt if ok => {
                if let Some(entry) = &entry {
                    dialog.prompt_set_text(&entry.text());
                }
            }
            _ => {}
        }
        unsafe { md.destroy() };
        true
    });
}

fn same_origin(url: &Url, server: &str) -> bool {
    Url::parse(server)
        .map(|s| s.origin() == url.origin())
        .unwrap_or(false)
}

/// Keep the main webview on the active Haven server (or a known embed);
/// anything else opens in the system browser, like the Electron `will-navigate`.
pub fn allow_navigation(app: &AppHandle, url: &Url) -> bool {
    if let Some(action) = crate::nav_fail::haven_nav_action(url) {
        handle_error_nav(app, &action);
        return false;
    }
    if url.scheme() == "tauri"
        || url.host_str() == Some("tauri.localhost")
        || crate::nav_fail::is_desktop_asset_url(url)
    {
        return true;
    }
    let state = app.state::<AppState>();
    let active = state.active_server_url.lock().clone();
    let known: Vec<String> = state
        .known_server_urls
        .lock()
        .values()
        .flat_map(|set| set.iter().cloned())
        .collect();
    // Servers the user has visited (serverHistory) count too, so a redirect
    // on the way into one of them never bounces to the system browser.
    let history: Vec<String> = state::get_value(app, "serverHistory")
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|h| h.get("url").and_then(|u| u.as_str()).map(String::from))
        .collect();
    static SIGN_IN: AtomicBool = AtomicBool::new(false);
    if active.as_deref().map(|a| same_origin(url, a)).unwrap_or(false)
        || known.iter().any(|k| same_origin(url, k))
        || history.iter().any(|k| same_origin(url, k))
    {
        SIGN_IN.store(url.path().starts_with("/api/auth/oidc/start"), Ordering::Relaxed);
        let _ = cfg!(not(target_os = "linux")).then(|| *state.nav_inflight.lock() = Some(std::time::Instant::now()));
        return true;
    }
    if EMBED_ORIGINS.iter().any(|e| same_origin(url, e)) || (SIGN_IN.load(Ordering::Relaxed) && matches!(url.scheme(), "http" | "https")) {
        return true;
    }
    let redirect = state.nav_inflight.lock().map(|t| t.elapsed() < std::time::Duration::from_secs(30)).unwrap_or(false) && matches!(url.scheme(), "http" | "https");
    if redirect && active.as_deref().and_then(|a| Url::parse(a).ok()).map(|a| a.host_str() == url.host_str()).unwrap_or(false) {
        return true;
    }
    if redirect {
        *state.nav_inflight.lock() = None;
        let (app, target) = (app.clone(), url.clone());
        std::thread::spawn(move || crate::nav_fail::redirected(&app, &target));
        return false;
    }
    if url.scheme() == "http" || url.scheme() == "https" {
        let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
    }
    false
}

#[tauri::command]
pub fn get_inject_script() -> String {
    INJECT_SCRIPT.to_string()
}

fn mark_connected(app: &AppHandle, url: &str) {
    let mut current = crate::server_list::read(app);
    let before = current.clone();
    crate::server_list::mark_connected(&mut current, url, crate::server_list::now_ms());
    crate::server_list::save(app, &current, &before);
}
pub fn open_app_window(app: &AppHandle, server_url: &str) -> Result<(), String> {
    let asked = state::normalize_server_url(server_url)
        .ok_or_else(|| "invalid server URL".to_string())?;
    let normalized = state::normalize_server_url(&crate::nav_fail::https_upgrade(&asked)).unwrap_or(asked.clone());
    let mut prefs = state::get_user_prefs(app).unwrap_or_default();
    let remembered = prefs.get("serverUrl").and_then(|v| v.as_str()).and_then(state::normalize_server_url);
    if normalized != asked && remembered.as_deref() == Some(asked.as_str()) {
        let _ = prefs.as_object_mut().map(|o| o.insert("serverUrl".into(), json!(normalized)));
        let _ = state::set_value(app, "userPrefs", prefs);
    }
    let app_url = state::build_server_app_url(&normalized);
    let parsed: Url = app_url.parse().map_err(|e: url::ParseError| e.to_string())?;

    {
        let state = app.state::<AppState>();
        *state.active_server_url.lock() = Some(normalized.clone());
        if state.primary_server_url.lock().is_none() {
            *state.primary_server_url.lock() = Some(normalized.clone());
        }
        crate::audio::set_emitter(app.clone());
        *state.audio_emitter.lock() = Some(app.clone());
    }

    mark_connected(app, &normalized);
    if !crate::nav_fail::server_tcp_reachable(&normalized) {
        show_or_create_connection_error(app, &normalized)?;
        hide_welcome(app);
        let _ = app.emit("nav:app-opened", &normalized);
        return Ok(());
    }

    show_or_create_server(app, parsed)?;
    hide_welcome(app);

    let _ = app.emit("nav:app-opened", &normalized);
    Ok(())
}

fn show_or_create_connection_error(app: &AppHandle, server_url: &str) -> Result<(), String> {
    let problem = crate::nav_fail::offline_problem();
    if app.get_webview_window("main").is_none() {
        *app.state::<AppState>().net_problem.lock() = Some(problem.clone());
        create_main_window(app, WebviewUrl::App("connection-error.html".into()))?;
        crate::nav_fail::watch_back(app, server_url);
    } else {
        crate::nav_fail::show_problem(app, server_url, problem);
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    Ok(())
}

fn show_or_create_server(app: &AppHandle, parsed: Url) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("main") {
        let _ = existing.navigate(parsed);
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    create_main_window(app, WebviewUrl::External(parsed))?;
    Ok(())
}

fn create_main_window(app: &AppHandle, initial: WebviewUrl) -> Result<(), String> {
    let bounds = state::get_value(app, "windowBounds")?;
    let width = bounds.get("width").and_then(|v| v.as_u64()).unwrap_or(1200) as f64;
    let height = bounds.get("height").and_then(|v| v.as_u64()).unwrap_or(800) as f64;

    // Respect screen dimensions so the window does not overflow below taskbars / docks on launch.
    let (max_w, max_h) = if let Ok(Some(monitor)) = app.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size();
        let screen_w = size.width as f64 / scale;
        let screen_h = size.height as f64 / scale;
        // Leave room for taskbar/dock, titlebar, and margins (at least 90px vertically, 60px horizontally).
        (
            (screen_w * 0.95).min(screen_w - 60.0).max(640.0),
            (screen_h * 0.88).min(screen_h - 90.0).max(480.0),
        )
    } else {
        (1200.0, 720.0)
    };
    let min_w = 720.0_f64.min(max_w);
    let min_h = 480.0_f64.min(max_h);
    let width = width.min(max_w).max(min_w);
    let height = height.min(max_h).max(min_h);

    let guard = app.clone();
    let load_guard = app.clone();
    let popup_app = app.clone();
    let download_app = app.clone();
    let builder = WebviewWindowBuilder::new(app, "main", initial)
        .title("Haven")
        .inner_size(width, height)
        .min_inner_size(min_w, min_h)
        .center()
        .disable_drag_drop_handler()
        .initialization_script(INJECT_SCRIPT)
        .on_navigation(move |url| allow_navigation(&guard, url))
        .on_page_load(move |_window, payload| {
            crate::nav_fail::on_page_load(&load_guard, &payload);
        })
        .on_new_window(move |url, _| open_popup(&popup_app, url))
        .theme(Some(tauri::Theme::Dark))
        .background_color(tauri::webview::Color(13, 13, 26, 255))
        .focused(true);
    #[cfg(windows)]
    let builder = builder.additional_browser_args(BROWSER_ARGS);
    #[cfg(target_os = "linux")]
    let builder = builder.on_download(move |_, event| on_download(&download_app, event));
    #[cfg(not(target_os = "linux"))]
    let _ = download_app;
    let main = builder.build().map_err(|e| e.to_string())?;
    let _ = main.center();
    accept_self_signed(&main);
    allow_media_permissions(&main);
    crate::nav_fail::attach_fail_watch(&main, app.clone());
    let (bg, fg) = crate::theme_icon::stored(app);
    crate::theme_icon::apply(app, bg, fg);
    Ok(())
}

#[cfg(target_os = "linux")]
fn on_download(app: &AppHandle, event: tauri::webview::DownloadEvent<'_>) -> bool {
    use tauri_plugin_notification::NotificationExt;
    if let tauri::webview::DownloadEvent::Finished { path, success, .. } = event {
        let body = path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let _ = app.notification().builder().title(state::t(app, if success { "download.saved" } else { "download.failed" })).body(body).show();
    }
    true
}
fn allow_media_permissions(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "linux")]
    let _ = window.with_webview(|w| {
        use gtk::glib::prelude::*;
        use webkit2gtk::{PermissionRequestExt, WebViewExt};
        w.inner().connect_permission_request(|_, request| {
            let geo = request.is::<webkit2gtk::GeolocationPermissionRequest>();
            let _ = if geo { request.deny() } else { request.allow() };
            true
        });
    });
    #[cfg(not(target_os = "linux"))]
    let _ = window;
}
fn open_popup(app: &AppHandle, url: Url) -> NewWindowResponse<tauri::Wry> {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    if !matches!(url.scheme(), "http" | "https") {
        return NewWindowResponse::Deny;
    }
    let label = format!("popup-{}", SEQ.fetch_add(1, Ordering::Relaxed));
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title("Haven")
        .inner_size(820.0, 760.0)
        .center()
        .focused(true)
        .theme(Some(tauri::Theme::Dark))
        .background_color(tauri::webview::Color(13, 13, 26, 255))
        .initialization_script(POPUP_CLOSE_SCRIPT)
        .on_document_title_changed(|w, title| {
            let _ = if title == POPUP_CLOSE_TITLE { w.destroy() } else { w.set_title(if title.is_empty() { "Haven" } else { &title }) };
        });
    #[cfg(windows)]
    let builder = builder.additional_browser_args(BROWSER_ARGS);
    let _ = builder.build().map(|w| accept_self_signed(&w));
    NewWindowResponse::Deny
}
fn handle_error_nav(app: &AppHandle, action: &str) {
    let app = app.clone();
    let action = action.to_string();
    // Finish cancelling the click navigation before we show/close windows.
    // Retry and "back to my server" probe the server over TCP first, so they
    // stay on this worker thread instead of blocking the GTK main thread.
    std::thread::spawn(move || match action.as_str() {
        "welcome" => {
            let on_main = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = go_back_to_welcome(&on_main);
            });
        }
        "retry" => {
            let url = app.state::<AppState>().active_server_url.lock().clone();
            if let Some(url) = url {
                let _ = open_app_window(&app, &url);
            }
        }
        "server" => {
            if let Some(url) = crate::nav_fail::other_server_url(&app) {
                let _ = switch_server(&app, &url);
            }
        }
        "signin" => {
            let _ = crate::nav_fail::sign_in_href(&app).map(|href| tauri_plugin_opener::open_url(href, None::<&str>));
        }
        _ => {}
    });
}

fn hide_welcome(app: &AppHandle) {
    if let Some(welcome) = app.get_webview_window("welcome") {
        let _ = welcome.hide();
    }
}

fn create_welcome_window(app: &AppHandle) -> Result<(), String> {
    let builder = WebviewWindowBuilder::new(app, "welcome", WebviewUrl::App("index.html".into()))
        .title("Haven")
        .inner_size(720.0, 560.0)
        .min_inner_size(620.0, 480.0)
        .resizable(false)
        .decorations(false)
        .disable_drag_drop_handler()
        .center()
        .visible(true)
        .background_color(tauri::webview::Color(13, 13, 26, 255));
    #[cfg(windows)]
    let builder = builder.additional_browser_args(BROWSER_ARGS);
    let welcome = builder.build().map_err(|e| e.to_string())?;
    accept_self_signed(&welcome);
    let _ = welcome.set_focus();
    Ok(())
}

fn show_welcome(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("welcome") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    create_welcome_window(app)
}

fn go_back_to_welcome(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    *state.active_server_url.lock() = None;
    show_welcome(app)?;
    if let Some(main) = app.get_webview_window("main") {
        *state.returning_to_welcome.lock() = true;
        let _ = main.close();
    }
    Ok(())
}

// The nav commands are async so Tauri runs them off the GTK main thread:
// each one probes the server over TCP first (up to 1.5 s per address), and a
// sync command would freeze the whole window for that long.
#[tauri::command]
pub async fn nav_open_app(app: AppHandle, webview: tauri::Webview, server_url: String) -> Result<(), String> {
    match super::sender(&app, &webview) { Some(super::Sender::Local) => open_app_window(&app, &server_url), _ => Err("not allowed".into()) }
}
async fn page_may_open(app: &AppHandle, webview: &tauri::Webview, server_url: &str, action: &Option<serde_json::Value>) -> bool {
    let lower = server_url.trim().to_ascii_lowercase();
    let r = crate::server_list::Request { kind: "open".into(), url: server_url.to_string(), ..Default::default() };
    (lower.starts_with("http://") || lower.starts_with("https://")) && super::request_change(app, webview, r, action).await.0
}

#[tauri::command]
pub fn nav_back_to_welcome(app: AppHandle, _state: State<AppState>) -> Result<(), String> {
    go_back_to_welcome(&app)
}

#[tauri::command]
pub async fn nav_switch_server(app: AppHandle, webview: tauri::Webview, server_url: String, action: Option<serde_json::Value>) -> Result<(), String> {
    if !page_may_open(&app, &webview, &server_url, &action).await { return Ok(()) }
    switch_server(&app, &server_url)
}

fn switch_server(app: &AppHandle, server_url: &str) -> Result<(), String> {
    let normalized = state::normalize_server_url(server_url)
        .ok_or_else(|| "invalid server URL".to_string())?;
    *app.state::<AppState>().active_server_url.lock() = Some(normalized.clone());
    mark_connected(app, &normalized);
    if !crate::nav_fail::server_tcp_reachable(&normalized) {
        show_or_create_connection_error(app, &normalized)?;
        return Ok(());
    }
    let app_url = state::build_server_app_url(&normalized);
    let parsed: Url = app_url.parse().map_err(|e: url::ParseError| e.to_string())?;
    show_or_create_server(app, parsed)
}

#[tauri::command]
pub async fn nav_change_primary_server(app: AppHandle, webview: tauri::Webview, server_url: String, action: Option<serde_json::Value>) -> Result<(), String> {
    if !page_may_open(&app, &webview, &server_url, &action).await { return Ok(()) }
    let normalized = state::normalize_server_url(&server_url)
        .ok_or_else(|| "invalid server URL".to_string())?;
    {
        let state = app.state::<AppState>();
        *state.primary_server_url.lock() = Some(normalized.clone());
        *state.active_server_url.lock() = Some(normalized.clone());
        state.server_badges.lock().clear();
        state.known_server_urls.lock().clear();
    }

    let mut prefs = state::get_user_prefs(&app)?;
    if let Some(obj) = prefs.as_object_mut() {
        obj.insert("serverUrl".into(), json!(normalized));
        obj.insert("mode".into(), json!("join"));
    }
    state::set_value(&app, "userPrefs", prefs)?;
    open_app_window(&app, &normalized)
}
