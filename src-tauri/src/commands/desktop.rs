use crate::state;
use serde_json::json;
use tauri::AppHandle;

#[tauri::command]
pub fn desktop_get_prefs(app: AppHandle) -> Result<serde_json::Value, String> {
    Ok(json!({
        "startOnLogin": state::get_bool(&app, "startOnLogin")?,
        "startHidden": state::get_bool(&app, "startHidden")?,
        "minimizeToTray": state::get_bool(&app, "minimizeToTray")?,
        "forceSDR": state::get_bool(&app, "forceSDR")?,
        "hideMenuBar": state::get_bool(&app, "hideMenuBar")?,
        "disableGpuVsync": state::get_bool(&app, "disableGpuVsync")?,
        "unlimitFrameRate": state::get_bool(&app, "unlimitFrameRate")?,
        "language": state::i18n_state(&app)?,
    }))
}

#[cfg(target_os = "linux")]
pub fn set_autostart(_app: &AppHandle, enabled: bool, only_create: bool) -> Result<(), String> {
    let file = dirs::config_dir().ok_or("no config dir")?.join("autostart").join("haven-desktop.desktop");
    if only_create && (!enabled || file.exists()) {
        return Ok(());
    }
    if !enabled {
        return std::fs::remove_file(&file).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }).map_err(|e| e.to_string());
    }
    let exe = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from).or_else(|| which::which("haven-desktop").ok()).or_else(|| std::env::current_exe().ok()).ok_or("no executable")?;
    std::fs::create_dir_all(file.parent().ok_or("no autostart dir")?).map_err(|e| e.to_string())?;
    std::fs::write(&file, format!("[Desktop Entry]\nType=Application\nName=Haven\nComment=Start Haven when you log in\nExec=\"{}\" --hidden\nIcon=haven-desktop\nTerminal=false\nX-GNOME-Autostart-enabled=true\nX-KDE-autostart-after=panel\n", exe.display())).map_err(|e| e.to_string())
}
#[cfg(not(target_os = "linux"))]
pub fn set_autostart(app: &AppHandle, enabled: bool, only_create: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    if only_create && (!enabled || launcher.is_enabled().unwrap_or(false)) {
        return Ok(());
    }
    if enabled { launcher.enable() } else { launcher.disable() }.map_err(|e| e.to_string())
}
#[tauri::command]
pub fn desktop_set_start_on_login(app: AppHandle, enabled: bool) -> Result<bool, String> {
    set_autostart(&app, enabled, false)?;
    state::set_value(&app, "startOnLogin", json!(enabled))?;
    Ok(true)
}

#[tauri::command]
pub fn desktop_set_start_hidden(app: AppHandle, enabled: bool) -> Result<bool, String> {
    state::set_value(&app, "startHidden", json!(enabled))?;
    Ok(true)
}

#[tauri::command]
pub fn desktop_set_minimize_to_tray(app: AppHandle, enabled: bool) -> Result<bool, String> {
    state::set_value(&app, "minimizeToTray", json!(enabled))?;
    Ok(true)
}

#[tauri::command]
pub fn desktop_set_force_sdr(app: AppHandle, enabled: bool) -> Result<serde_json::Value, String> {
    state::set_value(&app, "forceSDR", json!(enabled))?;
    Ok(json!({ "requiresRestart": true }))
}

#[tauri::command]
pub fn desktop_set_hide_menu_bar(app: AppHandle, enabled: bool) -> Result<bool, String> {
    state::set_value(&app, "hideMenuBar", json!(enabled))?;
    Ok(true)
}

#[tauri::command]
pub fn desktop_set_disable_gpu_vsync(
    app: AppHandle,
    enabled: bool,
) -> Result<serde_json::Value, String> {
    state::set_value(&app, "disableGpuVsync", json!(enabled))?;
    Ok(json!({ "requiresRestart": true }))
}

#[tauri::command]
pub fn desktop_set_unlimit_frame_rate(
    app: AppHandle,
    enabled: bool,
) -> Result<serde_json::Value, String> {
    state::set_value(&app, "unlimitFrameRate", json!(enabled))?;
    Ok(json!({ "requiresRestart": true }))
}
