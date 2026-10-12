//! When a Haven server is down, WebView2 shows Edge's "can't reach this page"
//! and the injected bridge never runs — no Home, no Retry. Watch the main
//! webview and swap that dead document for a local Haven page.

use crate::state::AppState;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri::webview::{PageLoadEvent, PageLoadPayload};
use url::Url;

/// Cheap reachability check so a dead host never becomes Edge's error page.
pub fn server_tcp_reachable(server_url: &str) -> bool {
    server_tcp_reachable_with_timeout(server_url, 3000)
}

pub fn server_tcp_reachable_with_timeout(server_url: &str, timeout_ms: u64) -> bool {
    let Ok(parsed) = Url::parse(server_url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let Some(port) = parsed.port_or_known_default() else {
        return false;
    };
    let Ok(addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    let timeout = Duration::from_millis(timeout_ms);
    for addr in addrs {
        if TcpStream::connect_timeout(&addr, timeout).is_ok() {
            return true;
        }
    }
    false
}

pub fn is_local_ui(url: &Url) -> bool {
    url.scheme() == "tauri" || matches!(url.host_str(), Some("tauri.localhost") | Some("tauri.localhost.")) || (cfg!(debug_assertions) && matches!(url.host_str(), Some("localhost") | Some("127.0.0.1")) && url.port() == Some(14370))
}
pub fn https_upgrade(server_url: &str) -> String {
    use std::io::{Read, Write};
    let Some(rest) = server_url.strip_prefix("http://") else { return server_url.to_string() };
    let Ok(parsed) = Url::parse(server_url) else { return server_url.to_string() };
    let (Some(host), Some(port)) = (parsed.host_str(), parsed.port_or_known_default()) else { return server_url.to_string() };
    let addr = (host.trim_start_matches('[').trim_end_matches(']'), port).to_socket_addrs().ok().and_then(|mut a| a.next());
    let Some(mut stream) = addr.and_then(|a| TcpStream::connect_timeout(&a, Duration::from_millis(3000)).ok()) else { return server_url.to_string() };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(3000)));
    let _ = stream.write_all(format!("HEAD / HTTP/1.0\r\nHost: {host}\r\n\r\n").as_bytes());
    let mut head = [0u8; 5];
    let mut got = 0;
    while got < head.len() { match stream.read(&mut head[got..]) { Ok(0) | Err(_) => break, Ok(n) => got += n } }
    if got == head.len() && &head == b"HTTP/" { server_url.to_string() } else { format!("https://{rest}") }
}
pub fn is_desktop_asset_url(url: &Url) -> bool {
    if url.scheme() == "data" || url.scheme() == "tauri" {
        return true;
    }
    if url.scheme() == "about" && url.as_str().contains("blank") {
        return true;
    }
    if url.path().contains("connection-error.html") || url.path().contains("splash.html") {
        return true;
    }
    is_local_ui(url) || (matches!(url.host_str(), Some("localhost") | Some("127.0.0.1")) && url.port() == Some(14370))
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

/// Where the bundled desktop pages are served. Tauri uses
/// http://tauri.localhost on Windows but the tauri:// scheme on Linux and
/// macOS; hard-coding the Windows origin left Linux release builds on
/// "Could not connect to tauri.localhost" with no Retry or Home buttons.
fn app_origin() -> &'static str {
    if cfg!(debug_assertions) {
        "http://localhost:14370"
    } else if cfg!(windows) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    }
}

pub fn haven_nav_href(action: &str) -> String {
    format!("{}/__haven_nav__/{action}", app_origin())
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
    Url::parse(&format!("{}/connection-error.html?url={encoded}", app_origin())).map_err(|e| e.to_string())
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
pub enum Problem { Down, Offline, Untrusted, Captive, Redirect { host: String, href: String } }
pub const PORTAL_URL: &str = "http://connectivitycheck.gstatic.com/generate_204";
static PROBLEM_GEN: AtomicU64 = AtomicU64::new(0);
static QUIET: AtomicBool = AtomicBool::new(true);
static RETRYING: AtomicBool = AtomicBool::new(false);
static CONN: AtomicU8 = AtomicU8::new(4);
static ONLINE: AtomicBool = AtomicBool::new(true);
#[cfg_attr(not(windows), allow(dead_code))]
pub static CERT_CANCELLED: AtomicBool = AtomicBool::new(false);
pub fn captive() -> bool { CONN.load(Ordering::Relaxed) == 3 }
pub fn online() -> bool { ONLINE.load(Ordering::Relaxed) }
#[cfg_attr(windows, allow(dead_code))]
pub fn vpn_like(name: &str, kind: &str) -> bool { name != "lo" && (kind == "65534" || kind == "512" || ["tun", "tap", "wg", "ppp", "ipsec", "vpn", "utun", "nordlynx", "proton", "mullvad", "cscotun", "gpd", "zt"].iter().any(|p| name.starts_with(p))) }
#[cfg_attr(not(windows), allow(dead_code))]
pub fn vpn_adapter(if_type: u32, text: &str) -> bool {
    let t = text.to_lowercase();
    !["teredo", "isatap", "6to4", "ip-https", "loopback", "hyper-v", "virtualbox", "vmware"].iter().any(|s| t.contains(s)) && (if_type == 23 || ["vpn", "wireguard", "wintun", "tap-", "nordlynx", "openvpn", "anyconnect", "pangp", "globalprotect", "fortinet", "forticlient", "zerotier", "tailscale", "mullvad", "proton", "juniper", "pulse secure", "sonicwall", "warp", "zscaler"].iter().any(|s| t.contains(s)))
}
#[cfg(target_os = "linux")]
pub fn vpn_active() -> bool {
    let read = |p: std::path::PathBuf| std::fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default();
    std::fs::read_dir("/sys/class/net").map(|d| d.flatten().any(|e| vpn_like(&e.file_name().to_string_lossy(), &read(e.path().join("type"))) && read(e.path().join("operstate")) != "down")).unwrap_or(false)
}
#[cfg(windows)]
pub fn vpn_active() -> bool {
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH};
    let wide = |p: *mut u16| if p.is_null() { String::new() } else { unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(p, (0..).take_while(|&i| *p.add(i) != 0).count())) } };
    let (mut size, mut buf) = (32768u32, Vec::<u64>::new());
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let ok = (0..3).any(|_| { buf = vec![0u64; size as usize / 8 + 1]; unsafe { GetAdaptersAddresses(0, flags, std::ptr::null(), buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH, &mut size) == 0 } });
    let mut p = if ok { buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH } else { std::ptr::null() };
    let mut found = false;
    while let Some(a) = unsafe { p.as_ref() } {
        found |= a.OperStatus == 1 && vpn_adapter(a.IfType, &format!("{} {}", wide(a.Description), wide(a.FriendlyName)));
        p = a.Next;
    }
    found
}
#[cfg(target_os = "macos")]
pub fn vpn_active() -> bool {
    extern "C" {
        fn haven_macos_ipv4_interfaces(found: extern "C" fn(*const std::ffi::c_char, *mut std::ffi::c_void), user: *mut std::ffi::c_void);
    }
    extern "C" fn each(name: *const std::ffi::c_char, user: *mut std::ffi::c_void) {
        let name = unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy();
        unsafe { *(user as *mut bool) |= vpn_like(&name, "") }
    }
    let mut found = false;
    unsafe { haven_macos_ipv4_interfaces(each, &mut found as *mut bool as *mut std::ffi::c_void) };
    found
}
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub fn vpn_active() -> bool { false }
fn on_error_page(app: &AppHandle) -> bool { app.get_webview_window("main").and_then(|m| m.url().ok()).map(|c| c.path().contains("connection-error.html")).unwrap_or(false) }
pub fn show_problem(app: &AppHandle, server_url: &str, p: Problem) {
    let Some(main) = app.get_webview_window("main") else { return };
    let changed = app.state::<AppState>().net_problem.lock().replace(p.clone()).as_ref() != Some(&p);
    if !changed && on_error_page(app) { return }
    PROBLEM_GEN.fetch_add(1, Ordering::Relaxed);
    *app.state::<AppState>().pending_server_load.lock() = None;
    *app.state::<AppState>().nav_inflight.lock() = None;
    let _ = connection_error_page_url(server_url).map(|u| main.navigate(u));
    let _ = (p == Problem::Offline).then(|| watch_back(app, server_url));
}
pub fn watch_back(app: &AppHandle, server_url: &str) {
    let (app, url, generation) = (app.clone(), server_url.to_string(), PROBLEM_GEN.load(Ordering::Relaxed));
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(4));
        let st = app.state::<AppState>();
        if PROBLEM_GEN.load(Ordering::Relaxed) != generation || st.net_problem.lock().as_ref() != Some(&Problem::Offline) || st.active_server_url.lock().as_deref() != Some(url.as_str()) { return }
        if online() && !captive() && server_tcp_reachable_with_timeout(&url, 2000) { let _ = crate::commands::open_app_window(&app, &url); return }
    });
}
pub fn offline_problem() -> Problem { if captive() { Problem::Captive } else { Problem::Offline } }
fn fail(app: &AppHandle, server_url: &str, quiet: bool) {
    let (app, url) = (app.clone(), server_url.to_string());
    std::thread::spawn(move || {
        let quiet = quiet && QUIET.swap(false, Ordering::Relaxed);
        if !quiet && RETRYING.load(Ordering::Relaxed) { return }
        RETRYING.store(quiet, Ordering::Relaxed);
        let back = (0..if quiet { 4 } else { 1 }).any(|i| { let _ = (i > 0).then(|| std::thread::sleep(Duration::from_secs(2))); !captive() && server_tcp_reachable_with_timeout(&url, 1500) });
        RETRYING.store(false, Ordering::Relaxed);
        match (back, quiet) { (true, true) => { let _ = crate::commands::open_app_window(&app, &url); } (true, false) => show_problem(&app, &url, Problem::Down), _ => show_problem(&app, &url, offline_problem()) }
    });
}
pub fn show_connection_error(app: &AppHandle, server_url: &str) { fail(app, server_url, false) }
pub fn connection_failed(app: &AppHandle, server_url: &str) { fail(app, server_url, true) }
pub fn redirected(app: &AppHandle, target: &Url) {
    let server = app.state::<AppState>().active_server_url.lock().clone().unwrap_or_default();
    show_problem(app, &server, if captive() { Problem::Captive } else { Problem::Redirect { host: target.host_str().unwrap_or_default().to_string(), href: target.to_string() } });
}
pub fn page_ready(app: &AppHandle) {
    QUIET.store(true, Ordering::Relaxed);
    *app.state::<AppState>().net_problem.lock() = None;
}
pub fn sign_in_href(app: &AppHandle) -> Option<String> {
    match app.state::<AppState>().net_problem.lock().clone()? { Problem::Redirect { href, .. } => Some(href), Problem::Captive => Some(PORTAL_URL.into()), _ => None }
}
#[cfg(target_os = "linux")]
pub fn watch_network(app: &AppHandle) {
    use gtk::gio::prelude::*;
    use std::cell::Cell;
    use std::rc::Rc;
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let mon = gtk::gio::NetworkMonitor::default();
        let read = |m: &gtk::gio::NetworkMonitor| { CONN.store(match m.connectivity() { gtk::gio::NetworkConnectivity::Local => 1, gtk::gio::NetworkConnectivity::Limited => 2, gtk::gio::NetworkConnectivity::Portal => 3, _ => 4 }, Ordering::Relaxed); ONLINE.store(m.is_network_available(), Ordering::Relaxed) };
        read(&mon);
        let tick = Rc::new(Cell::new(0u64));
        let changed: Rc<dyn Fn(&gtk::gio::NetworkMonitor)> = Rc::new(move |m| {
            read(m);
            let (t, tick, app) = (tick.get() + 1, tick.clone(), app.clone());
            tick.set(t);
            gtk::glib::timeout_add_local_once(Duration::from_millis(1500), move || { let _ = (tick.get() == t).then(|| network_changed(&app)); });
        });
        let c2 = changed.clone();
        mon.connect_network_changed(move |m, _| changed(m));
        mon.connect_connectivity_notify(move |m| c2(m));
    });
}
#[cfg(target_os = "linux")]
fn network_changed(app: &AppHandle) {
    let Some(main) = app.get_webview_window("main").filter(|_| online() && !captive()) else { return };
    let (problem, active) = { let st = app.state::<AppState>(); let p = st.net_problem.lock().clone(); let a = st.active_server_url.lock().clone(); (p, a) };
    match (on_error_page(app), problem, active) {
        (true, Some(p), Some(url)) if p != Problem::Untrusted => { let app = app.clone(); std::thread::spawn(move || { let _ = crate::commands::open_app_window(&app, &url); }); }
        (false, _, Some(_)) => { let app = app.clone(); let _ = main.with_webview(move |w| crate::cert_trust::recheck(&w.inner(), &app)); }
        _ => {}
    }
}

fn mark_pending(app: &AppHandle, url: &Url) {
    *app.state::<AppState>().pending_server_load.lock() = Some(url.to_string());
}

fn clear_pending(app: &AppHandle) {
    *app.state::<AppState>().pending_server_load.lock() = None;
}

fn still_waiting(app: &AppHandle, watched: &str, generation: u64) -> Option<bool> {
    let state = app.state::<AppState>();
    if state.pending_server_load.lock().as_deref() != Some(watched) || *state.load_generation.lock() != generation { return None }
    let current = app.get_webview_window("main").and_then(|m| m.url().ok());
    if current.map(|c| is_desktop_asset_url(&c)).unwrap_or(false) { return None }
    Some(crate::cert_trust::question_pending(app) || *state.pending_loading.lock())
}
fn watch_load(app: AppHandle, watched: String, generation: u64) {
    let started = std::time::Instant::now();
    std::thread::sleep(Duration::from_secs(15));
    while let Some(waiting) = still_waiting(&app, &watched, generation) {
        if !waiting || started.elapsed() >= Duration::from_secs(90) {
            let failed = app.state::<AppState>().active_server_url.lock().clone().unwrap_or_else(|| watched.clone());
            return show_connection_error(&app, &failed);
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}
pub fn on_page_load(app: &AppHandle, payload: &PageLoadPayload<'_>) {
    let url = payload.url();
    if is_desktop_asset_url(url) {
        clear_pending(app);
        return;
    }
    match payload.event() {
        PageLoadEvent::Started => {
            *app.state::<AppState>().nav_inflight.lock() = None;
            if looks_like_browser_error(url) {
                return;
            }
            mark_pending(app, url);
            *app.state::<AppState>().pending_loading.lock() = true;
            let generation = { let st = app.state::<AppState>(); let mut g = st.load_generation.lock(); *g += 1; *g };
            let (app, watched) = (app.clone(), url.to_string());
            std::thread::spawn(move || watch_load(app, watched, generation));
        }
        PageLoadEvent::Finished => {
            *app.state::<AppState>().pending_loading.lock() = false;
            *app.state::<AppState>().nav_inflight.lock() = None;
            if looks_like_browser_error(url) {
                let failed = app
                    .state::<AppState>()
                    .active_server_url
                    .lock()
                    .clone()
                    .unwrap_or_else(|| url.to_string());
                connection_failed(app, &failed);
            }
        }
    }
}

#[tauri::command]
pub fn nav_page_ready(app: AppHandle, webview: tauri::Webview) {
    clear_pending(&app);
    *app.state::<AppState>().nav_inflight.lock() = None;
    let _ = webview.url().ok().filter(|u| webview.label() == "main" && !is_desktop_asset_url(u)).map(|_| page_ready(&app));
}

#[tauri::command]
pub fn nav_connection_info(app: AppHandle, state: tauri::State<AppState>) -> Result<serde_json::Value, String> {
    let failed = state.active_server_url.lock().clone().unwrap_or_default();
    let back = other_server_url(&app).unwrap_or_default();
    let t = |k: &str| crate::state::t(&app, k);
    let (title, message, open, note) = match state.net_problem.lock().clone() {
        Some(Problem::Captive) => (t("net.captiveTitle"), t("net.captiveBody"), t("net.openSignIn"), String::new()),
        Some(Problem::Redirect { host, .. }) => (t("net.redirectTitle"), crate::state::tv(&app, "net.redirectBody", &[("host", &host)]), t("net.openSignIn"), String::new()),
        Some(Problem::Untrusted) => (String::new(), t("net.untrustedBody"), String::new(), String::new()),
        Some(Problem::Offline) => (String::new(), String::new(), String::new(), t("net.autoRetry")),
        _ => Default::default(),
    };
    Ok(serde_json::json!({
        "url": failed,
        "primary": back,
        "canGoBackServer": !back.is_empty(),
        "title": title,
        "message": message,
        "openLabel": open,
        "note": note,
    }))
}

pub fn attach_fail_watch(window: &WebviewWindow, app: AppHandle) {
    #[cfg(windows)]
    attach_webview2(window, app);
    #[cfg(target_os = "linux")]
    attach_webkit2(window, app);
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = (window, app);
}

#[cfg(target_os = "linux")]
fn attach_webkit2(window: &WebviewWindow, app: AppHandle) {
    use webkit2gtk::WebViewExt;
    let _ = window.with_webview(move |w| {
        let view = w.inner();
        let app_handle = app.clone();
        view.connect_load_failed(move |_view, _event, uri, error| {
            if error.matches(webkit2gtk::NetworkError::Cancelled) || error.matches(gtk::gio::IOErrorEnum::Cancelled) || error.matches(webkit2gtk::PolicyError::FrameLoadInterruptedByPolicyChange) || error.matches(webkit2gtk::PolicyError::CannotShowMimeType) {
                return false;
            }
            if let Ok(parsed) = Url::parse(uri) {
                if is_desktop_asset_url(&parsed) {
                    return false;
                }
            }
            let failed = app_handle
                .state::<AppState>()
                .active_server_url
                .lock()
                .clone()
                .unwrap_or_else(|| uri.to_string());
            connection_failed(&app_handle, &failed);
            true
        });
        let inflight = app.clone();
        view.connect_load_changed(move |view, event| {
            let local = view.uri().and_then(|u| Url::parse(&u).ok()).map(|u| is_desktop_asset_url(&u)).unwrap_or(true);
            let st = inflight.state::<AppState>();
            let mut slot = st.nav_inflight.lock();
            *slot = match event { webkit2gtk::LoadEvent::Started if !local => Some(std::time::Instant::now()), webkit2gtk::LoadEvent::Redirected => *slot, _ => None };
        });
    });
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
            // Certificate failures are cert_trust's: it asks, or shows the
            // redirect / untrusted page, instead of "server is down". A load
            // it cancelled ends as an aborted connection, not a cert error.
            if status == COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED || (1..=5).contains(&status.0) || CERT_CANCELLED.swap(false, Ordering::Relaxed) {
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
            connection_failed(&app, &failed);
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
    fn vpn_interfaces() {
        for (n, t) in [("nordlynx", "65534"), ("wg0", "65534"), ("tun0", "65534"), ("ppp0", "512"), ("tailscale0", "65534"), ("utun3", "1"), ("tap0", "1")] { assert!(vpn_like(n, t), "{n}") }
        for (n, t) in [("lo", "772"), ("enp8s0", "1"), ("wlan0", "1"), ("docker0", "1"), ("virbr0", "1")] { assert!(!vpn_like(n, t), "{n}") }
    }
    #[test]
    fn vpn_adapters() {
        for (t, d) in [(53, "WireGuard Tunnel wg0"), (53, "Wintun Userspace Tunnel Tailscale"), (6, "TAP-Windows Adapter V9 Ethernet 3"), (53, "NordLynx Tunnel NordLynx"), (23, "WAN Miniport (PPTP) Work"), (6, "PANGP Virtual Ethernet Adapter Ethernet 2"), (6, "Cisco AnyConnect Secure Mobility Client Virtual Miniport Adapter for Windows x64 Ethernet 4")] { assert!(vpn_adapter(t, d), "{d}") }
        for (t, d) in [(6, "Intel(R) Ethernet Controller I225-V Ethernet"), (71, "Intel(R) Wi-Fi 6E AX211 160MHz Wi-Fi"), (131, "Teredo Tunneling Pseudo-Interface"), (24, "Software Loopback Interface 1"), (6, "Hyper-V Virtual Ethernet Adapter vEthernet (Default Switch)"), (131, "Microsoft ISATAP Adapter")] { assert!(!vpn_adapter(t, d), "{d}") }
    }
    #[test]
    fn closed_port_is_unreachable() {
        assert!(!server_tcp_reachable("http://127.0.0.1:1"));
    }
}
