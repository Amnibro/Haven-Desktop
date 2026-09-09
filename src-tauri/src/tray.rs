use crate::state::{self, AppState};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

/// Whether a tray icon can be created on this machine.
///
/// On Linux the tray is an AppIndicator, which `libappindicator-sys` dlopens
/// lazily. When neither the ayatana nor the legacy library is installed that
/// crate panics inside a `LazyLock`, and with `panic = "abort"` in the release
/// profile the whole app dies with SIGABRT before the window opens. The
/// AppImage and deb ship the library, but a bare binary on a distro without it
/// (Arch without `libayatana-appindicator`, for instance) used to crash on
/// launch. Probe the same names the crate tries so we can run without a tray.
#[cfg(target_os = "linux")]
pub fn is_supported() -> bool {
    const CANDIDATES: [&str; 4] = [
        "libayatana-appindicator3.so.1",
        "libappindicator3.so.1",
        "libayatana-appindicator3.so",
        "libappindicator3.so",
    ];
    CANDIDATES
        .iter()
        // SAFETY: only loads the library to check it resolves; it runs no code
        // beyond its constructors, which libappindicator-sys runs anyway.
        .any(|name| unsafe { libloading::Library::new(name) }.is_ok())
}

#[cfg(not(target_os = "linux"))]
pub fn is_supported() -> bool {
    true
}

/// True once `setup_tray` has built the tray icon.
pub fn is_active(app: &AppHandle) -> bool {
    app.tray_by_id("main").is_some()
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show_i = MenuItem::with_id(app, "show", state::t(app, "tray.show"), true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", state::t(app, "tray.quit"), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))?;

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .tooltip(state::t(app, "tray.tooltip"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                } else if let Some(win) = app.get_webview_window("welcome") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "quit" => {
                let state = app.state::<AppState>();
                *state.quitting.lock() = true;
                {
                    let mut server = state.server.lock();
                    let _ = server.stop_server();
                }
                crate::audio::cleanup();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                } else if let Some(win) = app.get_webview_window("welcome") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
