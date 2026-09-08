mod audio;
mod commands;
mod i18n;
mod lowmem;
mod server_manager;
mod state;
mod theme_icon;
mod tray;

use state::AppState;
use tauri::Manager;
use tauri_plugin_store::StoreExt;

/// Ask the GitHub release feed once at startup; on a newer build, offer to
/// install it (signed with the key in ~/.tauri, verified against the pubkey
/// in tauri.conf.json) and relaunch. Failures stay silent: offline is normal.
fn check_for_update(app: tauri::AppHandle) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
    use tauri_plugin_updater::UpdaterExt;
    tauri::async_runtime::spawn(async move {
        let Ok(updater) = app.updater() else { return };
        let Ok(Some(update)) = updater.check().await else { return };
        let msg = format!(
            "Haven Desktop {} is available (you have {}). Install it now? Haven restarts when it is done.",
            update.version, update.current_version
        );
        let yes = app
            .dialog()
            .message(msg)
            .title("Update available")
            .buttons(MessageDialogButtons::OkCancelCustom("Install".into(), "Later".into()))
            .blocking_show();
        if !yes {
            return;
        }
        if update.download_and_install(|_, _| {}, || {}).await.is_ok() {
            app.restart();
        }
    });
}

pub fn run() {
    tauri::Builder::default()
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
            commands::audio_is_supported,
            commands::audio_get_apps,
            commands::audio_start_capture,
            commands::audio_stop_capture,
            commands::dialog_alert,
            commands::dialog_confirm,
            commands::dialog_prompt,
            commands::get_inject_script,
            theme_icon::theme_colors,
        ])
        .setup(|app| {
            // Ensure store file exists with defaults
            let _ = app.store("haven-desktop.json")?;
            state::ensure_defaults(app.handle())?;
            state::refresh_locale(app.handle())?;
            tray::setup_tray(app.handle())?;
            {
                let (bg, fg) = theme_icon::stored(app.handle());
                theme_icon::apply(app.handle(), bg, fg);
            }
            check_for_update(app.handle().clone());
            if let Some(welcome) = app.get_webview_window("welcome") {
                commands::accept_self_signed(&welcome);
            }

            // Auto-open remembered server if skipWelcome is set
            let prefs = state::get_user_prefs(app.handle())?;
            if prefs
                .get("skipWelcome")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                if let Some(url) = prefs.get("serverUrl").and_then(|v| v.as_str()) {
                    let url = url.to_string();
                    let handle = app.handle().clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(400));
                        let _ = commands::open_app_window(&handle, &url);
                    });
                }
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
                    tauri::WindowEvent::Focused(true) => { if let Some(w) = &main { lowmem::set_low_memory(w, false); } }
                    tauri::WindowEvent::Resized(_) if window.is_minimized().unwrap_or(false) => { if let Some(w) = &main { lowmem::set_low_memory(w, true); } }
                    _ => {}
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    if let Ok(minimize) = state::get_bool(window.app_handle(), "minimizeToTray") {
                        if minimize {
                            api.prevent_close();
                            let _ = window.hide();
                            if let Some(w) = window.app_handle().get_webview_window("main") { lowmem::set_low_memory(&w, true); }
                        }
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Haven Desktop");
}
