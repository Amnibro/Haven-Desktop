#[cfg(target_os = "linux")]
mod cert_trust;
#[cfg(target_os = "linux")]
mod linux_desktop;
#[cfg(target_os = "linux")]
mod ptt_hook;
mod audio;
mod commands;
mod i18n;
mod lowmem;
mod nav_fail;
mod server_manager;
mod state;
mod theme_icon;
mod tray;

use state::AppState;
use tauri::Manager;
use tauri_plugin_store::StoreExt;

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "linux")]
    let prefs: serde_json::Value = dirs::data_dir().and_then(|d| std::fs::read(d.join("com.haven.desktop/haven-desktop.json")).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    #[cfg(target_os = "linux")]
    let lang = i18n::resolve_locale(prefs.get("language").and_then(|v| v.as_str()).unwrap_or("auto"), &i18n::system_languages());
    #[cfg(target_os = "linux")]
    let cef = [("forceSDR", "force-color-profile", Some("srgb")), ("disableGpuVsync", "disable-gpu-vsync", None), ("unlimitFrameRate", "disable-frame-rate-limit", None)].into_iter().filter(|(k, ..)| prefs.get(*k).and_then(|v| v.as_bool()).unwrap_or(false)).fold(tauri_runtime_cef::Cef::default(), |c, (_, s, v)| c.command_line_arg(s, v.map(String::from))).accept_language_list(if lang == "en" { "en-US,en".to_string() } else { format!("{lang},{},en-US,en", lang.split('-').next().unwrap_or("en")) }).command_line_arg("js-flags", Some("--max-old-space-size=512")).command_line_arg("disable-gpu-memory-buffer-video-frames", None::<String>).command_line_arg("image-decode-ct", Some("3")).command_line_arg("force-gpu-mem-available-mb", Some("256")).command_line_arg("password-store", Some("basic")).command_line_arg("disable-background-timer-throttling", None::<String>).command_line_arg("disable-renderer-backgrounding", None::<String>).command_line_arg("disable-backgrounding-occluded-windows", None::<String>).disable_features(["LocalNetworkAccessChecks", "IntensiveWakeUpThrottling", "CalculateNativeWinOcclusion", "AutofillServerCommunication"]).enable_features(["WebRTCPipeWireCapturer"]);
    #[cfg(target_os = "linux")]
    tauri_runtime_cef::grant_display_capture(true);
    #[cfg(all(target_os = "linux", debug_assertions))]
    let cef = match std::env::var("HAVEN_CEF_DEBUG_PORT").ok().and_then(|p| p.parse::<u16>().ok()) { Some(port) => cef.remote_debugging(tauri_runtime_cef::RemoteDebugging::Port { port, allowed_origins: vec![] }).command_line_arg("use-fake-device-for-media-stream", None::<String>).command_line_arg("use-fake-ui-for-media-stream", None::<String>), None => cef };
    #[cfg(target_os = "linux")]
    let builder = builder.runtime(cef);
    #[cfg(target_os = "linux")]
    let builder = builder.on_permission_request(|_, kind| { use tauri::webview::{PermissionKind as K, PermissionResponse as R}; match kind { K::Microphone | K::Camera | K::DisplayCapture | K::Notifications => R::Allow, _ => R::Default } });
    builder
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A second launch (taskbar pin, Start menu) focuses the running app
            // instead of booting a second copy, like Electron's requestSingleInstanceLock.
            for label in ["main", "welcome"] {
                if let Some(win) = app.get_webview_window(label) {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                    break;
                }
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::i18n_get_state,
            commands::i18n_set_language,
            commands::i18n_refresh_automatic,
            commands::server_detect,
            commands::server_start,
            commands::server_stop,
            commands::server_status,
            commands::server_browse,
            commands::settings_get,
            commands::settings_set,
            commands::app_version,
            commands::nav_open_app,
            commands::nav_back_to_welcome,
            commands::nav_switch_server,
            commands::nav_change_primary_server,
            nav_fail::nav_page_ready,
            nav_fail::nav_connection_info,
            commands::window_minimize,
            commands::window_maximize,
            commands::window_close,
            commands::window_enter_fullscreen,
            commands::window_leave_fullscreen,
            commands::open_external,
            commands::notify,
            commands::notification_badge,
            commands::get_server_badges,
            commands::report_known_server_urls,
            commands::server_history_get,
            commands::server_history_add,
            commands::server_history_remove,
            commands::server_history_update_name,
            commands::desktop_get_prefs,
            commands::desktop_set_start_on_login,
            commands::desktop_set_start_hidden,
            commands::desktop_set_minimize_to_tray,
            commands::desktop_set_force_sdr,
            commands::desktop_set_hide_menu_bar,
            commands::desktop_set_disable_gpu_vsync,
            commands::desktop_set_unlimit_frame_rate,
            commands::shortcuts_get,
            commands::shortcuts_register,
            commands::update_download,
            commands::update_install,
            commands::clipboard_write_text,
            commands::clipboard_write_image,
            commands::save_image,
            commands::audio_is_supported,
            commands::audio_get_apps,
            commands::share_picker_open,
            commands::share_picker_choose,
            commands::audio_stop_capture,
            commands::dialog_alert,
            commands::dialog_confirm,
            commands::dialog_prompt,
            commands::get_inject_script,
            theme_icon::theme_colors,
        ])
        .setup(|app| {
            #[cfg(target_os = "linux")]
            {
                let h = app.handle().clone();
                cert_trust::migrate(&h);
                tauri_runtime_cef::set_certificate_error_handler(move |e, c| cert_trust::decide(&h, e, c));
            }
            // Ensure store file exists with defaults
            let _ = app.store("haven-desktop.json")?;
            state::ensure_defaults(app.handle())?;
            state::refresh_locale(app.handle())?;
            match tray::setup_tray(app.handle()) { Ok(()) => tray::mark_up(), Err(e) => eprintln!("haven: no system tray ({e}); running without one") }
            {
                let (bg, fg) = theme_icon::stored(app.handle());
                theme_icon::apply(app.handle(), bg, fg);
            }
            commands::check_for_update(app.handle().clone(), false);
            // Saved mute / deafen / push-to-talk keys were only registered when
            // the settings page re-saved them, so after a restart they did nothing.
            let _ = commands::shortcuts_register(app.handle().clone(), serde_json::json!({}));
            if let Some(welcome) = app.get_webview_window("welcome") {
                commands::accept_self_signed(&welcome);
            }

            #[cfg(target_os = "linux")]
            let _ = commands::apply_autostart(app.handle());
            let hidden = std::env::args().any(|a| a == "--hidden") && tray::available();
            // Welcome starts hidden so skipWelcome never flashes Host / Join.
            let prefs = state::get_user_prefs(app.handle())?;
            let host_path = (prefs.get("mode").and_then(|v| v.as_str()) == Some("host")).then(|| prefs.get("serverPath").and_then(|v| v.as_str()).map(str::to_string)).flatten().filter(|p| !p.is_empty());
            let skip = prefs
                .get("skipWelcome")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let remembered = prefs
                .get("serverUrl")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if skip {
                if let Some(url) = remembered {
                    let handle = app.handle().clone();
                    std::thread::spawn(move || {
                        let url = match host_path {
                            Some(dir) => {
                                let res = handle.state::<AppState>().server.lock().start_server(&dir);
                                tray::rebuild(&handle);
                                if !res.success {
                                    if let Some(welcome) = handle.get_webview_window("welcome") { let _ = welcome.show(); let _ = welcome.set_focus(); }
                                    return;
                                }
                                res.url.unwrap_or(url)
                            }
                            None => url,
                        };
                        // A failure here used to leave nothing but a tray icon.
                        let opened = commands::open_app_window(&handle, &url);
                        if opened.is_ok() && hidden { if let Some(main) = handle.get_webview_window("main") { let _ = main.hide(); } }
                        if opened.is_err() {
                            if let Some(welcome) = handle.get_webview_window("welcome") {
                                let _ = welcome.show();
                                let _ = welcome.set_focus();
                            }
                        }
                    });
                } else if let Some(welcome) = app.get_webview_window("welcome") {
                    let _ = welcome.show();
                    let _ = welcome.set_focus();
                }
            } else if let (Some(welcome), false) = (app.get_webview_window("welcome"), hidden) {
                let _ = welcome.show();
                let _ = welcome.set_focus();
            }

            Ok(())
        })
        .on_web_content_process_terminate(|webview, _| {
            static LAST: once_cell::sync::Lazy<parking_lot::Mutex<(u32, std::time::Instant)>> = once_cell::sync::Lazy::new(|| parking_lot::Mutex::new((0, std::time::Instant::now())));
            if webview.label() != "main" {
                return;
            }
            let n = { let mut l = LAST.lock(); l.0 = if l.1.elapsed().as_secs() > 60 { 1 } else { l.0 + 1 }; l.1 = std::time::Instant::now(); l.0 };
            let (w, app) = (webview.clone(), webview.app_handle().clone());
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500 * 2u64.pow(n.min(5))));
                let url = app.state::<state::AppState>().active_server_url.lock().clone();
                let _ = match url.filter(|_| n > 5).and_then(|u| state::build_server_app_url(&u).parse().ok()) { Some(u) => w.navigate(u), None => w.reload() };
            });
        })
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_)) && !window.is_minimized().unwrap_or(false) {
                let (app, max) = (window.app_handle(), window.is_maximized().unwrap_or(false));
                let mut prev = state::get_value(app, "windowBounds").unwrap_or_default();
                let bounds = match (window.inner_size(), window.outer_position(), window.scale_factor()) {
                    (Ok(s), Ok(p), Ok(k)) if !max && s.width as f64 / k >= 800.0 && s.height as f64 / k >= 600.0 => serde_json::json!({ "x": p.x as f64 / k, "y": p.y as f64 / k, "width": s.width as f64 / k, "height": s.height as f64 / k, "maximized": false }),
                    _ => { prev["maximized"] = serde_json::json!(max); prev }
                };
                let _ = state::set_value(app, "windowBounds", bounds);
            }
            if window.label() == "main" {
                let main = window.app_handle().get_webview_window("main");
                match event {
                    tauri::WindowEvent::Focused(f) => { if let Some(w) = &main { if *f { lowmem::set_low_memory(w, false); let _ = w.request_user_attention(None); } let _ = w.eval(&format!("window.__havenFocus&&window.__havenFocus({f})")); } }
                    tauri::WindowEvent::Resized(_) if window.is_minimized().unwrap_or(false) => { if let Some(w) = &main { lowmem::set_low_memory(w, true); } }
                    _ => {}
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                if window.label() == "main" {
                    let returning = *app.state::<state::AppState>().returning_to_welcome.lock();
                    if returning {
                        *app.state::<state::AppState>().returning_to_welcome.lock() = false;
                        return;
                    }
                    if let (Ok(true), true) = (state::get_bool(app, "minimizeToTray"), tray::available()) {
                        api.prevent_close();
                        let _ = window.hide();
                        if let Some(w) = app.get_webview_window("main") {
                            lowmem::set_low_memory(&w, true);
                            let _ = w.eval("window.__havenFocus&&window.__havenFocus(false)");
                        }
                        return;
                    }
                    tray::quit_app(app);
                    return;
                }
                if window.label() == "welcome" && app.get_webview_window("main").is_none() {
                    tray::quit_app(app);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Haven Desktop");
}
