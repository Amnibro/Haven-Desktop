//! When a Haven server is down, WebView2 shows Edge's "can't reach this page"
//! and the injected bridge never runs — no Home, no Retry. Watch the main
//! webview and swap that dead document for a local Haven page.

use crate::state::AppState;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri::webview::{PageLoadEvent, PageLoadPayload};
use url::Url;

/// Cheap reachability check so a dead host never becomes Edge's error page.
pub fn server_tcp_reachable(server_url: &str) -> bool {
    let Ok(parsed) = Url::parse(server_url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let Some(port) = parsed.port_or_known_default() else {
        return false;
    };
    let Ok(mut addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    let timeout = Duration::from_millis(1500);
    addrs.any(|addr| TcpStream::connect_timeout(&addr, timeout).is_ok())
}

pub fn is_desktop_asset_url(url: &Url) -> bool {
    if url.scheme() == "data" {
        return true;
    }
    if url.scheme() == "about" && url.as_str().contains("blank") {
        return true;
    }
    if url.path().contains("connection-error.html") || url.path().contains("splash.html") {
        return true;
    }
    match url.host_str() {
        Some("tauri.localhost") | Some("tauri.localhost.") => true,
        Some("localhost") | Some("127.0.0.1") if url.port() == Some(14370) => true,
        _ => false,
    }
}

pub fn looks_like_browser_error(url: &Url) -> bool {
    if url.scheme() == "about" && url.as_str().contains("blank") {
        return false;
    }
    matches!(
        url.scheme(),
        "chrome-error" | "chrome" | "edge" | "edge-error" | "about"
    ) || url.host_str() == Some("chromewebdata")
        || url.as_str().contains("chromewebdata")
}

/// Connection-error buttons navigate here so WebView2 starts a real load
/// `allow_navigation` can cancel. Same origin as the desktop UI, not `.invalid`.
pub fn haven_nav_action(url: &Url) -> Option<String> {
    if url.host_str() == Some("haven.invalid") {
        let action = url.path().trim_start_matches('/');
        return (!action.is_empty()).then(|| action.to_string());
    }
    url.path()
        .strip_prefix("/__haven_nav__/")
        .filter(|action| !action.is_empty())
        .map(|action| action.to_string())
}

pub fn haven_nav_href(action: &str) -> String {
    let base = if cfg!(debug_assertions) {
        "http://localhost:14370"
    } else {
        "http://tauri.localhost"
    };
    format!("{base}/__haven_nav__/{action}")
}

pub fn other_server_url(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let active = state.active_server_url.lock().clone().unwrap_or_default();
    let primary = state.primary_server_url.lock().clone().unwrap_or_default();
    if !primary.is_empty() && primary != active {
        return Some(primary);
    }
    if let Ok(prefs) = crate::state::get_user_prefs(app) {
        if let Some(url) = prefs
            .get("serverUrl")
            .and_then(|v| v.as_str())
            .and_then(crate::state::normalize_server_url)
        {
            if url != active {
                return Some(url);
            }
        }
    }
    let history = crate::state::get_value(app, "serverHistory")
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    for item in history.iter().rev() {
        if let Some(url) = item
            .get("url")
            .and_then(|v| v.as_str())
            .and_then(crate::state::normalize_server_url)
        {
            if url != active {
                return Some(url);
            }
        }
    }
    None
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn error_page_html(server_url: &str, primary: Option<&str>) -> String {
    let shown = html_escape(server_url);
    let server_href = haven_nav_href("server");
    let retry_href = haven_nav_href("retry");
    let welcome_href = haven_nav_href("welcome");
    let back = primary
        .filter(|p| !p.is_empty() && *p != server_url)
        .map(|p| {
            let label = html_escape(p);
            format!(
                "<a class=\"secondary\" href=\"{server_href}\">Go Back to My Server</a><div class=\"hint\">{label}</div>"
            )
        })
        .unwrap_or_default();
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Haven</title>
<style>
  html,body{{margin:0;height:100%;background:#0d0d1a;color:#e0e0e0;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif}}
  .wrap{{min-height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;padding:40px 24px;text-align:center;gap:14px}}
  .hex{{font-size:48px;line-height:1;color:#6b4fdb}}
  h1{{font-size:22px;margin:0}}
  p{{max-width:420px;color:#888;font-size:14px;line-height:1.5;margin:0}}
  code{{display:inline-block;max-width:100%;overflow-wrap:anywhere;padding:6px 10px;border-radius:6px;background:#1a1a34;border:1px solid #2a2a50;color:#e0e0e0;font-size:13px}}
  .actions{{display:flex;flex-wrap:wrap;gap:10px;justify-content:center;margin-top:8px}}
  a{{display:inline-block;padding:10px 22px;border-radius:6px;font-size:15px;font-weight:600;text-decoration:none}}
  .primary{{background:#6b4fdb;color:#fff}}
  .secondary{{background:#1a1a34;color:#e0e0e0;border:1px solid #2a2a50}}
  .hint{{width:100%;color:#555;font-size:12px}}
</style>
</head>
<body>
<div class="wrap" id="haven-connection-error">
  <div class="hex">⬡</div>
  <h1>Connection Problem</h1>
  <p>Haven couldn't load that server. It may be down, the address may be wrong, or the network dropped.</p>
  <code>{shown}</code>
  <div class="actions">
    <a class="primary" href="{retry_href}">Try Again</a>
    {back}
    <a class="secondary" href="{welcome_href}">Go Back to Welcome</a>
  </div>
</div>
</body>
</html>"#
    )
}

fn connection_error_page_url(server_url: &str) -> Result<Url, String> {
    let encoded: String = url::form_urlencoded::byte_serialize(server_url.as_bytes()).collect();
    let base = if cfg!(debug_assertions) {
        "http://localhost:14370/connection-error.html"
    } else {
        "http://tauri.localhost/connection-error.html"
    };
    Url::parse(&format!("{base}?url={encoded}")).map_err(|e| e.to_string())
}

pub fn show_connection_error(app: &AppHandle, server_url: &str) {
    let Some(main) = app.get_webview_window("main") else { return };
    if let Ok(current) = main.url() {
        if current.path().contains("connection-error.html") {
            return;
        }
    }
    let Ok(url) = connection_error_page_url(server_url) else { return };
    *app.state::<AppState>().pending_server_load.lock() = None;
    let _ = main.navigate(url);
}

fn mark_pending(app: &AppHandle, url: &Url) {
    *app.state::<AppState>().pending_server_load.lock() = Some(url.to_string());
}

fn clear_pending(app: &AppHandle) {
    *app.state::<AppState>().pending_server_load.lock() = None;
}

fn maybe_timeout(app: &AppHandle, watched: &str) {
    let state = app.state::<AppState>();
    let pending = state.pending_server_load.lock().clone();
    if pending.as_deref() != Some(watched) {
        return;
    }
    if let Some(main) = app.get_webview_window("main") {
        if let Ok(current) = main.url() {
            if is_desktop_asset_url(&current) {
                return;
            }
        }
    }
    let failed = state
        .active_server_url
        .lock()
        .clone()
        .unwrap_or_else(|| watched.to_string());
    drop(pending);
    show_connection_error(app, &failed);
}

pub fn on_page_load(app: &AppHandle, payload: &PageLoadPayload<'_>) {
    let url = payload.url();
    if is_desktop_asset_url(url) {
        clear_pending(app);
        return;
    }
    match payload.event() {
        PageLoadEvent::Started => {
            if looks_like_browser_error(url) {
                return;
            }
            mark_pending(app, url);
            let app = app.clone();
            let watched = url.to_string();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(15));
                maybe_timeout(&app, &watched);
            });
        }
        PageLoadEvent::Finished => {
            if looks_like_browser_error(url) {
                let failed = app
                    .state::<AppState>()
                    .active_server_url
                    .lock()
                    .clone()
                    .unwrap_or_else(|| url.to_string());
                show_connection_error(app, &failed);
            }
        }
    }
}

#[tauri::command]
pub fn nav_page_ready(app: AppHandle) {
    clear_pending(&app);
}

#[tauri::command]
pub fn nav_connection_info(app: AppHandle, state: tauri::State<AppState>) -> Result<serde_json::Value, String> {
    let failed = state.active_server_url.lock().clone().unwrap_or_default();
    let back = other_server_url(&app).unwrap_or_default();
    Ok(serde_json::json!({
        "url": failed,
        "primary": back,
        "canGoBackServer": !back.is_empty(),
    }))
}

pub fn attach_fail_watch(window: &WebviewWindow, app: AppHandle) {
    #[cfg(windows)]
    attach_webview2(window, app);
    #[cfg(not(windows))]
    let _ = (window, app);
}

#[cfg(windows)]
fn attach_webview2(window: &WebviewWindow, app: AppHandle) {
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED;
    use webview2_com::NavigationCompletedEventHandler;
    use windows_core::BOOL;

    let _ = window.with_webview(move |w| unsafe {
        let Ok(core) = w.controller().CoreWebView2() else { return };
        let handler = NavigationCompletedEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else { return Ok(()) };
            let mut success = BOOL::default();
            let _ = args.IsSuccess(&mut success);
            if success.as_bool() {
                return Ok(());
            }
            let mut status = Default::default();
            let _ = args.WebErrorStatus(&mut status);
            if status == COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED {
                return Ok(());
            }
            let failed = app
                .state::<AppState>()
                .active_server_url
                .lock()
                .clone()
                .unwrap_or_default();
            if failed.is_empty() {
                return Ok(());
            }
            let app = app.clone();
            let _ = app.clone().run_on_main_thread(move || {
                show_connection_error(&app, &failed);
            });
            Ok(())
        }));
        let mut token = 0;
        let _ = core.add_NavigationCompleted(&handler, &mut token);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_assets_are_local() {
        let url = Url::parse("http://localhost:14370/connection-error.html?url=x").unwrap();
        assert!(is_desktop_asset_url(&url));
        let prod = Url::parse("http://tauri.localhost/connection-error.html").unwrap();
        assert!(is_desktop_asset_url(&prod));
        let blank = Url::parse("about:blank").unwrap();
        assert!(is_desktop_asset_url(&blank));
        assert!(!looks_like_browser_error(&blank));
        let haven = Url::parse("http://127.0.0.1:3847/app").unwrap();
        assert!(!is_desktop_asset_url(&haven));
        let html = error_page_html("http://localhost:3000", None);
        assert!(html.contains("localhost:3000"));
        assert!(html.contains("/__haven_nav__/welcome"));
        let nav = Url::parse("http://localhost:14370/__haven_nav__/welcome").unwrap();
        assert_eq!(haven_nav_action(&nav).as_deref(), Some("welcome"));
        let reserved = Url::parse("https://haven.invalid/server").unwrap();
        assert_eq!(haven_nav_action(&reserved).as_deref(), Some("server"));
    }

    #[test]
    fn edge_error_schemes() {
        let url = Url::parse("chrome-error://chromewebdata/").unwrap();
        assert!(looks_like_browser_error(&url));
    }

    #[test]
    fn closed_port_is_unreachable() {
        assert!(!server_tcp_reachable("http://127.0.0.1:1"));
    }
}
