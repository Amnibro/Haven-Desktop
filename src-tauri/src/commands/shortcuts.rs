use crate::state;
use serde_json::{json, Value};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[tauri::command]
pub fn shortcuts_get(app: AppHandle) -> Result<Value, String> {
    state::get_value(&app, "desktopShortcuts")
}

#[tauri::command]
pub fn shortcuts_register(app: AppHandle, updates: Value) -> Result<Value, String> {
    let mut cfg = state::get_value(&app, "desktopShortcuts")?
        .as_object()
        .cloned()
        .unwrap_or_default();

    if let Some(obj) = updates.as_object() {
        for (k, v) in obj {
            if matches!(k.as_str(), "mute" | "deafen" | "ptt" | "pttMode") {
                cfg.insert(k.clone(), v.clone());
            }
        }
    }
    state::set_value(&app, "desktopShortcuts", Value::Object(cfg.clone()))?;

    // Best-effort: unregister all then register known shortcuts.
    let _ = app.global_shortcut().unregister_all();
    let mut result = serde_json::Map::new();
    for key in ["mute", "deafen", "ptt"] {
        let accel = cfg.get(key).and_then(|v| v.as_str()).unwrap_or("");
        if accel.is_empty() {
            result.insert(key.into(), json!({ "ok": true, "reason": "ok" }));
            continue;
        }
        match accel.parse::<Shortcut>() {
            Ok(shortcut) => {
                let ok = app.global_shortcut().on_shortcut(shortcut, move |_app, _s, _e| {
                    // Events are also emitted for the inject bridge to consume.
                }).is_ok();
                result.insert(
                    key.into(),
                    if ok {
                        json!({ "ok": true, "reason": "ok" })
                    } else {
                        json!({ "ok": false, "reason": "conflict", "accel": accel })
                    },
                );
            }
            Err(_) => {
                result.insert(
                    key.into(),
                    json!({ "ok": false, "reason": "conflict", "accel": accel }),
                );
            }
        }
    }
    if let Some(mode) = cfg.get("pttMode") {
        result.insert("pttMode".into(), json!({ "ok": true, "reason": "ok", "mode": mode }));
    }
    Ok(Value::Object(result))
}
