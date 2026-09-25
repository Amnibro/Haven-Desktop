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

#[tauri::command]
pub fn desktop_set_start_on_login(app: AppHandle, enabled: bool) -> Result<bool, String> {
    state::set_value(&app, "startOnLogin", json!(enabled))?;
    #[cfg(target_os = "linux")]
    apply_autostart(&app)?;
    Ok(true)
}

#[tauri::command]
pub fn desktop_set_start_hidden(app: AppHandle, enabled: bool) -> Result<bool, String> {
    state::set_value(&app, "startHidden", json!(enabled))?;
    #[cfg(target_os = "linux")]
    apply_autostart(&app)?;
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

/// Writes or removes ~/.config/autostart/haven-desktop.desktop to match the
/// start-on-login and start-hidden preferences.
#[cfg(target_os = "linux")]
pub(crate) fn apply_autostart(app: &AppHandle) -> Result<(), String> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let dir = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".config")).join("autostart");
    let file = dir.join("haven-desktop.desktop");
    if !state::get_bool(app, "startOnLogin")? {
        let _ = std::fs::remove_file(&file);
        return Ok(());
    }
    let on_path = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).map(|d| d.join("haven-desktop")).find(|f| f.is_file())).flatten();
    let exec = std::env::var("APPIMAGE").ok().map(std::path::PathBuf::from).or(on_path).or_else(|| std::env::current_exe().ok()).ok_or("no executable path")?;
    let hidden = if state::get_bool(app, "startHidden")? { " --hidden" } else { "" };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(&file, format!("[Desktop Entry]\nType=Application\nName=Haven\nComment=Start Haven when you log in\nExec=\"{}\"{hidden}\nIcon=haven-desktop\nTerminal=false\nX-GNOME-Autostart-enabled=true\nX-KDE-autostart-after=panel\n", exec.display())).map_err(|e| e.to_string())
}
