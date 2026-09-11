use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::i18n::{self, I18nState};
use crate::server_manager::ServerManager;

pub struct AppState {
    pub server: Mutex<ServerManager>,
    pub active_server_url: Mutex<Option<String>>,
    pub primary_server_url: Mutex<Option<String>>,
    pub server_badges: Mutex<HashMap<String, bool>>,
    pub known_server_urls: Mutex<HashMap<String, HashSet<String>>>,
    pub server_language_states: Mutex<HashMap<String, (String, String)>>,
    pub current_locale: Mutex<String>,
    pub quitting: Mutex<bool>,
    pub audio_emitter: Mutex<Option<AppHandle>>,
    pub pending_server_load: Mutex<Option<String>>,
    pub returning_to_welcome: Mutex<bool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            server: Mutex::new(ServerManager::new()),
            active_server_url: Mutex::new(None),
            primary_server_url: Mutex::new(None),
            server_badges: Mutex::new(HashMap::new()),
            known_server_urls: Mutex::new(HashMap::new()),
            server_language_states: Mutex::new(HashMap::new()),
            current_locale: Mutex::new("en".into()),
            quitting: Mutex::new(false),
            audio_emitter: Mutex::new(None),
            pending_server_load: Mutex::new(None),
            returning_to_welcome: Mutex::new(false),
        }
    }
}

const STORE_NAME: &str = "haven-desktop.json";

pub fn store(app: &AppHandle) -> Result<Arc<tauri_plugin_store::Store<tauri::Wry>>, String> {
    app.store(STORE_NAME).map_err(|e| e.to_string())
}

pub fn ensure_defaults(app: &AppHandle) -> Result<(), String> {
    let s = store(app)?;
    let defaults = json!({
        "userPrefs": {
            "mode": null,
            "serverUrl": null,
            "serverPath": null,
            "skipWelcome": false,
            "audioInput": null,
            "audioOutput": null
        },
        "windowBounds": { "width": 1200, "height": 800 },
        "desktopShortcuts": {
            "mute": "CommandOrControl+Shift+M",
            "deafen": "CommandOrControl+Shift+D",
            "ptt": ""
        },
        "startOnLogin": false,
        "startHidden": false,
        "minimizeToTray": false,
        "forceSDR": false,
        "hideMenuBar": false,
        "disableGpuVsync": false,
        "unlimitFrameRate": false,
        "serverHistory": [],
        "language": "auto",
        "languagePreferenceSet": false
    });

    if let Some(obj) = defaults.as_object() {
        for (k, v) in obj {
            if s.get(k).is_none() {
                let _ = s.set(k.clone(), v.clone());
            }
        }
    }
    let _ = s.save();
    Ok(())
}

pub fn get_value(app: &AppHandle, key: &str) -> Result<Value, String> {
    let s = store(app)?;
    Ok(s.get(key).unwrap_or(Value::Null))
}

pub fn set_value(app: &AppHandle, key: &str, value: Value) -> Result<(), String> {
    let s = store(app)?;
    s.set(key.to_string(), value);
    s.save().map_err(|e| e.to_string())
}

pub fn get_bool(app: &AppHandle, key: &str) -> Result<bool, String> {
    Ok(get_value(app, key)?.as_bool().unwrap_or(false))
}

pub fn get_user_prefs(app: &AppHandle) -> Result<Value, String> {
    Ok(get_value(app, "userPrefs")?.as_object().cloned().map(Value::Object).unwrap_or(json!({})))
}

pub fn refresh_locale(app: &AppHandle) -> Result<String, String> {
    let pref = get_value(app, "language")?
        .as_str()
        .unwrap_or("auto")
        .to_string();
    let system = i18n::system_languages();
    let locale = i18n::resolve_locale(&pref, &system);
    let state = app.state::<AppState>();
    *state.current_locale.lock() = locale.clone();
    Ok(locale)
}

pub fn i18n_state(app: &AppHandle) -> Result<I18nState, String> {
    let preference = get_value(app, "language")?
        .as_str()
        .unwrap_or("auto")
        .to_string();
    let state = app.state::<AppState>();
    let locale = state.current_locale.lock().clone();
    let system_locale = i18n::resolve_locale("auto", &i18n::system_languages());
    let is_preference_stored = get_bool(app, "languagePreferenceSet")?;
    Ok(i18n::build_state(
        &preference,
        &locale,
        &system_locale,
        is_preference_stored,
    ))
}

pub fn t(app: &AppHandle, key: &str) -> String {
    let state = app.state::<AppState>();
    let locale = state.current_locale.lock().clone();
    i18n::translate(&locale, key, &HashMap::new())
}

pub fn normalize_server_url(input: &str) -> Option<String> {
    let mut value = input.trim().to_string();
    if value.is_empty() {
        return None;
    }
    if !value.to_lowercase().starts_with("http://") && !value.to_lowercase().starts_with("https://")
    {
        value = format!("https://{value}");
    }
    let mut parsed = url::Url::parse(&value).ok()?;
    parsed.set_fragment(None);
    parsed.set_query(None);
    let mut path = parsed.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        path = "/".into();
    }
    let lower = path.to_lowercase();
    if lower.ends_with("/app") || lower.ends_with("/app.html") {
        path = path.rsplit_once('/').map(|(a, _)| a).unwrap_or("").to_string();
        if path.is_empty() {
            path = "/".into();
        }
    }
    path = path.trim_end_matches('/').to_string();
    if path.is_empty() {
        path = "/".into();
    }
    Some(if path == "/" {
        parsed.origin().ascii_serialization()
    } else {
        format!("{}{}", parsed.origin().ascii_serialization(), path)
    })
}

pub fn sanitize_server_history(list: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for item in list {
        let url = item.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let Some(normalized) = normalize_server_url(url) else {
            continue;
        };
        if !seen.insert(normalized.clone()) {
            continue;
        }
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&normalized)
            .to_string();
        let last = item
            .get("lastConnected")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        out.push(json!({
            "url": normalized,
            "name": name,
            "lastConnected": last
        }));
    }
    out
}

pub fn build_server_app_url(server_url: &str) -> String {
    let base = normalize_server_url(server_url).unwrap_or_else(|| server_url.to_string());
    if base.ends_with('/') {
        format!("{base}app")
    } else {
        format!("{base}/app")
    }
}
