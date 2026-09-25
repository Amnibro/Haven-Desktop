use crate::state;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

static PTT_DOWN: AtomicBool = AtomicBool::new(false);
pub(crate) fn fire(app: &AppHandle, key: &str, hold: bool, pressed: bool) {
    let ev = match (key, hold, pressed) {
        ("mute", _, true) => Some("voice:mute-toggle"),
        ("deafen", _, true) => Some("voice:deafen-toggle"),
        ("ptt", true, true) => (!PTT_DOWN.swap(true, Ordering::SeqCst)).then_some("voice:ptt-down"),
        ("ptt", true, false) => PTT_DOWN.swap(false, Ordering::SeqCst).then_some("voice:ptt-up"),
        ("ptt", false, true) => Some("voice:ptt-toggle"),
        _ => None,
    };
    if let Some(ev) = ev {
        let _ = app.emit(ev, ());
    }
}

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
    PTT_DOWN.store(false, Ordering::SeqCst);
    let ptt_mode = cfg.get("pttMode").and_then(|v| v.as_str()).map(|m| if m == "toggle" { "toggle" } else { "hold" }).unwrap_or("hold").to_string();
    let mut result = serde_json::Map::new();
    #[cfg(target_os = "linux")]
    let hooked = crate::ptt_hook::set_bindings(&app, &["mute", "deafen", "ptt"].map(|k| (k.to_string(), cfg.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string(), k == "ptt" && ptt_mode == "hold")));
    #[cfg(not(target_os = "linux"))]
    let hooked: Vec<String> = Vec::new();
    for key in ["mute", "deafen", "ptt"] {
        let accel = cfg.get(key).and_then(|v| v.as_str()).unwrap_or("");
        if accel.is_empty() || hooked.iter().any(|h| h == key) {
            result.insert(key.into(), json!({ "ok": true, "reason": "ok" }));
            continue;
        }
        match accel.parse::<Shortcut>() {
            Ok(shortcut) => {
                // The handler was empty, so mute, deafen and push-to-talk did
                // nothing in the Tauri build. The plugin reports press and
                // release, which is exactly what hold-mode PTT needs (the
                // Electron app has to emulate it for combos). The bridge in
                // app-bridge.js listens for these names.
                let (key_name, hold) = (key.to_string(), key == "ptt" && ptt_mode == "hold");
                let ok = app.global_shortcut().on_shortcut(shortcut, move |app, _s, e| fire(app, &key_name, hold, e.state() == ShortcutState::Pressed)).is_ok();
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
