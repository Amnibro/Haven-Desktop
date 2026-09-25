use crate::state::{self, AppState};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

static TRAY_UP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn mark_up() { TRAY_UP.store(true, std::sync::atomic::Ordering::Relaxed) }
pub fn available() -> bool { TRAY_UP.load(std::sync::atomic::Ordering::Relaxed) }
fn show_any(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main").or_else(|| app.get_webview_window("welcome")) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::DynRuntime>> {
    let t = |k: &str| state::t(app, k);
    let st = app.state::<AppState>();
    let (active, primary) = (st.active_server_url.lock().clone(), st.primary_server_url.lock().clone());
    let running = st.server.lock().status().running;
    let pref = state::get_value(app, "language").ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "auto".into());
    let version = MenuItem::with_id(app, "version", t("menu.version").replace("{version}", &app.package_info().version.to_string()), false, None::<&str>)?;
    let updates = MenuItem::with_id(app, "updates", t("menu.checkForUpdates"), true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", t("tray.show"), true, None::<&str>)?;
    let away = active.is_some() && primary.is_some() && active != primary;
    let server = MenuItem::with_id(app, if away { "back" } else { "change" }, t(if away { "tray.backToServer" } else { "tray.changeServer" }), app.get_webview_window("main").is_some(), None::<&str>)?;
    let mut langs: Vec<CheckMenuItem<tauri::DynRuntime>> = vec![CheckMenuItem::with_id(app, "lang:auto", t("language.automatic"), true, pref == "auto", None::<&str>)?];
    for l in crate::i18n::supported_locales() {
        langs.push(CheckMenuItem::with_id(app, format!("lang:{}", l.code), l.name.clone(), true, pref == l.code, None::<&str>)?);
    }
    let lang_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::DynRuntime>> = langs.iter().map(|m| m as &dyn tauri::menu::IsMenuItem<tauri::DynRuntime>).collect();
    let language = Submenu::with_items(app, t("menu.language"), true, &lang_refs)?;
    let status = MenuItem::with_id(app, "status", format!("{} {}", if running { "●" } else { "○" }, t(if running { "tray.serverRunning" } else { "tray.serverStopped" })), false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", t("tray.quit"), true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    Menu::with_items(app, &[&version, &updates, &sep()?, &show, &server, &sep()?, &language, &sep()?, &status, &sep()?, &quit])
}
pub fn rebuild(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))?;
    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&build_menu(app)?)
        .tooltip(state::t(app, "tray.tooltip"))
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            match id {
                "show" => show_any(app),
                "updates" => crate::check_for_update(app.clone(), true),
                "change" => { let _ = crate::commands::go_back_to_welcome(app); }
                "back" => { if let Some(p) = app.state::<AppState>().primary_server_url.lock().clone() { let _ = crate::commands::switch_server(app, &p); } }
                "quit" => quit_app(app),
                _ => { if let Some(code) = id.strip_prefix("lang:") { let _ = crate::commands::set_language(app, code.to_string()); } }
            }
            rebuild(app);
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_any(tray.app_handle());
            }
        })
        .build(app)?;
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
        let h = handle.clone();
        let _ = handle.run_on_main_thread(move || rebuild(&h));
    });
    Ok(())
}

pub fn quit_app(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.quitting.lock() = true;
    {
        let mut server = state.server.lock();
        let _ = server.stop_server();
    }
    crate::audio::cleanup();
    app.exit(0);
}
