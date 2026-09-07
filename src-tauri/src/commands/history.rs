use crate::state;
use serde_json::{json, Value};
use tauri::AppHandle;

#[tauri::command]
pub fn server_history_get(app: AppHandle) -> Result<Vec<Value>, String> {
    let raw = state::get_value(&app, "serverHistory")?
        .as_array()
        .cloned()
        .unwrap_or_default();
    let cleaned = state::sanitize_server_history(&raw);
    if cleaned != raw {
        state::set_value(&app, "serverHistory", json!(cleaned.clone()))?;
    }
    Ok(cleaned)
}

#[tauri::command]
pub fn server_history_add(app: AppHandle, url: String, name: Option<String>) -> Result<(), String> {
    let Some(normalized) = state::normalize_server_url(&url) else {
        return Ok(());
    };
    let mut history = server_history_get(app.clone())?;
    if history
        .iter()
        .any(|h| h.get("url").and_then(|u| u.as_str()) == Some(normalized.as_str()))
    {
        return Ok(());
    }
    history.push(json!({
        "url": normalized.clone(),
        "name": name.unwrap_or(normalized),
        "lastConnected": 0
    }));
    while history.len() > 20 {
        history.remove(0);
    }
    state::set_value(&app, "serverHistory", json!(history))
}

#[tauri::command]
pub fn server_history_remove(app: AppHandle, url: String) -> Result<Vec<Value>, String> {
    let normalized = state::normalize_server_url(&url);
    let history: Vec<Value> = server_history_get(app.clone())?
        .into_iter()
        .filter(|h| h.get("url").and_then(|u| u.as_str()) != normalized.as_deref())
        .collect();
    state::set_value(&app, "serverHistory", json!(history.clone()))?;
    Ok(history)
}

#[tauri::command]
pub fn server_history_update_name(app: AppHandle, url: String, name: String) -> Result<(), String> {
    let Some(normalized) = state::normalize_server_url(&url) else {
        return Ok(());
    };
    let mut history = server_history_get(app.clone())?;
    if let Some(entry) = history
        .iter_mut()
        .find(|h| h.get("url").and_then(|u| u.as_str()) == Some(normalized.as_str()))
    {
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("name".into(), json!(name));
        }
    }
    state::set_value(&app, "serverHistory", json!(history))
}
