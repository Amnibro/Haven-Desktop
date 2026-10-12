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
    pub pending_loading: Mutex<bool>,
    pub load_generation: Mutex<u64>,
    pub list_gate: Mutex<ListGate>,
    pub list_gate_turn: tauri::async_runtime::Mutex<()>,
    pub cert_questions: Mutex<HashSet<String>>,
    pub net_problem: Mutex<Option<crate::nav_fail::Problem>>,
    pub nav_inflight: Mutex<Option<std::time::Instant>>,
    pub update: Mutex<crate::updates::UpdateState>,
}
#[derive(Default)]
pub struct ListGate { pub asking: HashSet<String>, pub declined: HashSet<String>, pub blocked: HashSet<String>, pub waiting: usize }

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
            pending_loading: Mutex::new(false),
            load_generation: Mutex::new(0),
            list_gate: Mutex::new(ListGate::default()),
            list_gate_turn: tauri::async_runtime::Mutex::new(()),
            cert_questions: Mutex::new(HashSet::new()),
            net_problem: Mutex::new(None),
            nav_inflight: Mutex::new(None),
            update: Mutex::new(Default::default()),
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
        "flashTaskbar": true,
        "forceSDR": false,
        "hideMenuBar": false,
        "disableGpuVsync": false,
        "unlimitFrameRate": false,
        "serverHistory": [],
        "serverListRemoved": [],
        "serverListOrder": [],
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
    tv(app, key, &[])
}
pub fn tv(app: &AppHandle, key: &str, values: &[(&str, &str)]) -> String {
    let locale = app.state::<AppState>().current_locale.lock().clone();
    i18n::translate(&locale, key, &values.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
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

pub fn build_server_app_url(server_url: &str) -> String {
    let base = normalize_server_url(server_url).unwrap_or_else(|| server_url.to_string());
    if base.ends_with('/') {
        format!("{base}app")
    } else {
        format!("{base}/app")
    }
}
