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
    // Autostart is platform-specific; persist preference for now.
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
