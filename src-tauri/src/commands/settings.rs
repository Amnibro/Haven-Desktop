use crate::state;
use serde_json::Value;
use tauri::AppHandle;

const ALLOWED: &[&str] = &[
    "userPrefs",
    "windowBounds",
    "audioInputDevice",
    "audioOutputDevice",
    "lastServer",
    "pushToTalk",
    "pushToTalkKey",
    "noiseGate",
    "noiseThreshold",
    "desktopShortcuts",
    "startOnLogin",
    "startHidden",
    "minimizeToTray",
    "forceSDR",
    "disableGpuVsync",
    "unlimitFrameRate",
];

#[tauri::command]
pub fn settings_get(app: AppHandle, key: String) -> Result<Value, String> {
    state::get_value(&app, &key)
}

#[tauri::command]
pub fn settings_set(app: AppHandle, key: String, value: Value) -> Result<bool, String> {
    if !ALLOWED.contains(&key.as_str()) {
        return Ok(false);
    }
    state::set_value(&app, &key, value)?;
    Ok(true)
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("only http(s) URLs are allowed".into());
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())?;
    let _ = app;
    Ok(())
}
