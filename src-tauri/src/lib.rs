mod audio;
mod commands;
mod i18n;
mod server_manager;
mod state;
mod tray;

use state::AppState;
use tauri::Manager;
use tauri_plugin_store::StoreExt;

pub fn run() {
    tauri::Builder::default()
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
        ])
        .setup(|app| {
            // Ensure store file exists with defaults
            let _ = app.store("haven-desktop.json")?;
            state::ensure_defaults(app.handle())?;
            state::refresh_locale(app.handle())?;
            tray::setup_tray(app.handle())?;

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
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    if let Ok(minimize) = state::get_bool(window.app_handle(), "minimizeToTray") {
                        if minimize {
                            api.prevent_close();
                            let _ = window.hide();
                        }
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Haven Desktop");
}
