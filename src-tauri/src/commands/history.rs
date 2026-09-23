use crate::state::{self, AppState};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

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

    // A removed server must not come back: "Go Back to My Server" reads the
    // primary, and skipWelcome reopens userPrefs.serverUrl on the next launch
    // (which also re-adds it to this history).
    if let Some(removed) = normalized {
        let app_state = app.state::<AppState>();
        {
            let mut primary = app_state.primary_server_url.lock();
            if primary.as_deref() == Some(removed.as_str()) {
                *primary = None;
            }
        }
        app_state.known_server_urls.lock().remove(&removed);
        app_state.server_badges.lock().remove(&removed);
        let mut prefs = state::get_user_prefs(&app)?;
        let remembered = prefs
            .get("serverUrl")
            .and_then(|v| v.as_str())
            .and_then(state::normalize_server_url);
        if remembered.as_deref() == Some(removed.as_str()) {
            if let Some(obj) = prefs.as_object_mut() {
                obj.insert("serverUrl".into(), Value::Null);
            }
            state::set_value(&app, "userPrefs", prefs)?;
        }
    }
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
