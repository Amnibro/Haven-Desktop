use crate::state;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

static PTT_DOWN: AtomicBool = AtomicBool::new(false);

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
    for key in ["mute", "deafen", "ptt"] {
        let accel = cfg.get(key).and_then(|v| v.as_str()).unwrap_or("");
        if accel.is_empty() {
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
                let key_name = key.to_string();
                let hold = key == "ptt" && ptt_mode == "hold";
                let ok = app.global_shortcut().on_shortcut(shortcut, move |app, _s, e| {
                    let pressed = e.state() == ShortcutState::Pressed;
                    match key_name.as_str() {
                        "mute" => { if pressed { let _ = app.emit("voice:mute-toggle", ()); } }
                        "deafen" => { if pressed { let _ = app.emit("voice:deafen-toggle", ()); } }
                        _ if hold => {
                            // OS auto-repeat can fire Pressed again while held; only the first one talks.
                            if pressed && !PTT_DOWN.swap(true, Ordering::SeqCst) { let _ = app.emit("voice:ptt-down", ()); }
                            if !pressed && PTT_DOWN.swap(false, Ordering::SeqCst) { let _ = app.emit("voice:ptt-up", ()); }
                        }
                        _ => { if pressed { let _ = app.emit("voice:ptt-toggle", ()); } }
                    }
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
