use crate::state::{self, AppState};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use url::Url;

const INJECT_SCRIPT: &str = include_str!("../../inject/app-bridge.js");

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

/// Linux counterpart of `--ignore-certificate-errors`: WebKitGTK rejects
/// self-signed certificates unless the web context's TLS policy says otherwise.
/// macOS (WKWebView) has no equivalent hook in wry; a self-signed server there
/// needs a trusted cert or plain http.
pub fn accept_self_signed(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::{TLSErrorsPolicy, WebContextExt, WebViewExt};
        let _ = window.with_webview(|w| {
            if let Some(ctx) = w.inner().web_context() {
                ctx.set_tls_errors_policy(TLSErrorsPolicy::Ignore);
            }
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = window;
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
    if active.as_deref().map(|a| same_origin(url, a)).unwrap_or(false)
        || known.iter().any(|k| same_origin(url, k))
        || history.iter().any(|k| same_origin(url, k))
        || EMBED_ORIGINS.iter().any(|e| same_origin(url, e))
    {
        return true;
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

pub fn open_app_window(app: &AppHandle, server_url: &str) -> Result<(), String> {
    let normalized = state::normalize_server_url(server_url)
        .ok_or_else(|| "invalid server URL".to_string())?;
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

    let mut history = state::get_value(app, "serverHistory")?
        .as_array()
        .cloned()
        .unwrap_or_default();
    history = state::sanitize_server_history(&history);
    if let Some(entry) = history
        .iter_mut()
        .find(|h| h.get("url").and_then(|u| u.as_str()) == Some(normalized.as_str()))
    {
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("lastConnected".into(), json!(now_ms()));
        }
    } else {
        history.push(json!({
            "url": normalized,
            "name": normalized,
            "lastConnected": now_ms()
        }));
    }
    while history.len() > 20 {
        history.remove(0);
    }
    state::set_value(app, "serverHistory", json!(history))?;

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
    if app.get_webview_window("main").is_none() {
        create_main_window(app, WebviewUrl::App("connection-error.html".into()))?;
    } else {
        crate::nav_fail::show_connection_error(app, server_url);
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
    let guard = app.clone();
    let load_guard = app.clone();
    let builder = WebviewWindowBuilder::new(app, "main", initial)
        .title("Haven")
        .inner_size(width, height)
        .min_inner_size(800.0, 600.0)
        .initialization_script(INJECT_SCRIPT)
        .on_navigation(move |url| allow_navigation(&guard, url))
        .on_page_load(move |_window, payload| {
            crate::nav_fail::on_page_load(&load_guard, &payload);
        })
        .theme(Some(tauri::Theme::Dark))
        .background_color(tauri::webview::Color(13, 13, 26, 255))
        .focused(true);
    #[cfg(windows)]
    let builder = builder.additional_browser_args(BROWSER_ARGS);
    let main = builder.build().map_err(|e| e.to_string())?;
    accept_self_signed(&main);
    crate::nav_fail::attach_fail_watch(&main, app.clone());
    let (bg, fg) = crate::theme_icon::stored(app);
    crate::theme_icon::apply(app, bg, fg);
    Ok(())
}

fn handle_error_nav(app: &AppHandle, action: &str) {
    let app = app.clone();
    let action = action.to_string();
    // Finish cancelling the click navigation before we show/close windows.
    std::thread::spawn(move || {
        let on_main = app.clone();
        let _ = app.run_on_main_thread(move || match action.as_str() {
            "welcome" => {
                let _ = go_back_to_welcome(&on_main);
            }
            "retry" => {
                let url = on_main.state::<AppState>().active_server_url.lock().clone();
                if let Some(url) = url {
                    let _ = open_app_window(&on_main, &url);
                }
            }
            "server" => {
                if let Some(url) = crate::nav_fail::other_server_url(&on_main) {
                    let _ = nav_switch_server(on_main.clone(), on_main.state::<AppState>(), url);
                }
            }
            _ => {}
        });
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
    *state.returning_to_welcome.lock() = true;
    show_welcome(app)?;
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.close();
    }
    Ok(())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn nav_open_app(app: AppHandle, server_url: String) -> Result<(), String> {
    open_app_window(&app, &server_url)
}

#[tauri::command]
pub fn nav_back_to_welcome(app: AppHandle, _state: State<AppState>) -> Result<(), String> {
    go_back_to_welcome(&app)
}

#[tauri::command]
pub fn nav_switch_server(
    app: AppHandle,
    state: State<AppState>,
    server_url: String,
) -> Result<(), String> {
    let normalized = state::normalize_server_url(&server_url)
        .ok_or_else(|| "invalid server URL".to_string())?;
    *state.active_server_url.lock() = Some(normalized.clone());
    if !crate::nav_fail::server_tcp_reachable(&normalized) {
        show_or_create_connection_error(&app, &normalized)?;
        return Ok(());
    }
    let app_url = state::build_server_app_url(&normalized);
    let parsed: Url = app_url.parse().map_err(|e: url::ParseError| e.to_string())?;
    show_or_create_server(&app, parsed)
}

#[tauri::command]
pub fn nav_change_primary_server(
    app: AppHandle,
    state: State<AppState>,
    server_url: String,
) -> Result<(), String> {
    let normalized = state::normalize_server_url(&server_url)
        .ok_or_else(|| "invalid server URL".to_string())?;
    *state.primary_server_url.lock() = Some(normalized.clone());
    *state.active_server_url.lock() = Some(normalized.clone());
    state.server_badges.lock().clear();
    state.known_server_urls.lock().clear();

    let mut prefs = state::get_user_prefs(&app)?;
    if let Some(obj) = prefs.as_object_mut() {
        obj.insert("serverUrl".into(), json!(normalized));
        obj.insert("mode".into(), json!("join"));
    }
    state::set_value(&app, "userPrefs", prefs)?;
    open_app_window(&app, &normalized)
}
