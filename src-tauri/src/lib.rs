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
use tauri::{Emitter, Manager};
use tauri_plugin_store::StoreExt;

/// Ask the GitHub release feed once at startup; on a newer build, offer to
/// install it (signed with the key in ~/.tauri, verified against the pubkey
/// in tauri.conf.json) and relaunch. Failures stay silent: offline is normal.
pub(crate) fn check_for_update(app: tauri::AppHandle, manual: bool) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
    use tauri_plugin_updater::UpdaterExt;
    tauri::async_runtime::spawn(async move {
        let t = |k: &str| state::t(&app, k);
        let say = |msg: String| { app.dialog().message(msg).title("Haven Desktop").buttons(MessageDialogButtons::OkCustom(t("update.ok"))).show(|_| {}); };
        let result = match app.updater() { Ok(u) => u.check().await.map_err(|e| e.to_string()), Err(e) => Err(e.to_string()) };
        let update = match result {
            Ok(Some(u)) => u,
            Ok(None) => { if manual { say(t("update.upToDate").replace("{version}", &app.package_info().version.to_string())); } return; }
            Err(e) => { if manual { say(t("update.error").replace("{error}", &e)); } return; }
        };
        let yes = app.dialog().message(t("update.available").replace("{version}", &update.version)).title("Haven Desktop").buttons(MessageDialogButtons::OkCancelCustom(t("update.now"), t("dialog.cancel"))).blocking_show();
        if !yes {
            return;
        }
        let _ = app.emit("update:download-progress", 0);
        let (mut got, h) = (0u64, app.clone());
        match update.download_and_install(move |chunk, total| { got += chunk as u64; if let Some(t) = total.filter(|t| *t > 0) { let _ = h.emit("update:download-progress", got * 100 / t); } }, || {}).await {
            Ok(()) => app.restart(),
            Err(e) => say(t("update.failed").replace("{error}", &e.to_string())),
        }
    });
}

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "linux")]
    let cef = tauri_runtime_cef::Cef::default().command_line_arg("password-store", Some("basic")).command_line_arg("disable-background-timer-throttling", None::<String>).command_line_arg("disable-renderer-backgrounding", None::<String>).command_line_arg("disable-backgrounding-occluded-windows", None::<String>).disable_features(["LocalNetworkAccessChecks", "IntensiveWakeUpThrottling", "CalculateNativeWinOcclusion"]).enable_features(["WebRTCPipeWireCapturer"]);
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
            check_for_update(app.handle().clone(), false);
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
        .on_window_event(|window, event| {
            // Persist the main window size so the next launch reopens at it
            // (the Electron app saved windowBounds; the store key was read but never written).
            if window.label() == "main" {
                if let tauri::WindowEvent::Resized(_) = event {
                    if let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor()) {
                        let (w, h) = ((size.width as f64 / scale) as u64, (size.height as f64 / scale) as u64);
                        if w >= 800 && h >= 600 && !window.is_maximized().unwrap_or(false) {
                            let _ = state::set_value(
                                window.app_handle(),
                                "windowBounds",
                                serde_json::json!({ "width": w, "height": h }),
                            );
                        }
                    }
                }
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
