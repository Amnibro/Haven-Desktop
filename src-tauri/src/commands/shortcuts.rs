use crate::state;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

static PTT_DOWN: AtomicBool = AtomicBool::new(false);
static PTM_DOWN: AtomicBool = AtomicBool::new(false);
static MEDIA_WANTED: AtomicBool = AtomicBool::new(false);
const MEDIA_KEYS: [(&str, &str); 3] = [(if cfg!(target_os = "linux") { "MediaPlay" } else { "MediaPlayPause" }, "playpause"), ("MediaTrackNext", "nexttrack"), ("MediaTrackPrevious", "previoustrack")];

pub fn types_text(accel: &str) -> bool {
    const MODS: [&str; 12] = ["commandorcontrol", "cmdorctrl", "control", "ctrl", "alt", "option", "altgr", "shift", "meta", "cmd", "command", "super"];
    const KEYS: [&str; 21] = ["space", "tab", "enter", "return", "backspace", "plus", "num0", "num1", "num2", "num3", "num4", "num5", "num6", "num7", "num8", "num9", "numdec", "numadd", "numsub", "nummult", "numdiv"];
    let parts: Vec<String> = accel.split('+').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).collect();
    parts.split_last().is_some_and(|(key, mods)| mods.iter().all(|m| m == "shift") && !MODS.contains(&key.as_str()) && (key.chars().count() == 1 || KEYS.contains(&key.as_str())))
}
fn held(key: &str, cfg: &serde_json::Map<String, Value>) -> bool {
    key == "ptm" || (key == "ptt" && cfg.get("pttMode").and_then(Value::as_str) != Some("toggle"))
}
pub fn set_media_keys(app: &AppHandle, wanted: Option<bool>) {
    let before = MEDIA_WANTED.swap(wanted.unwrap_or(MEDIA_WANTED.load(Ordering::SeqCst)), Ordering::SeqCst);
    if wanted == Some(before) { return }
    let on = MEDIA_WANTED.load(Ordering::SeqCst);
    let voice: Vec<Shortcut> = state::get_value(app, "desktopShortcuts").ok().and_then(|v| v.as_object().cloned()).unwrap_or_default().values().filter_map(|v| v.as_str()?.parse().ok()).collect();
    for (accel, action) in MEDIA_KEYS {
        let Ok(key) = accel.parse::<Shortcut>() else { continue };
        let ours = app.global_shortcut().is_registered(key) && !voice.contains(&key);
        let done = match (on, ours || voice.contains(&key)) {
            (true, false) => app.global_shortcut().on_shortcut(key, move |app, _, e| { let _ = (e.state() == ShortcutState::Pressed).then(|| app.emit("music:media-key", action)); }),
            (false, _) if ours => app.global_shortcut().unregister(key),
            _ => Ok(()),
        };
        let _ = done.map_err(|e| eprintln!("[MediaKeys] {accel}: {e}"));
    }
}
#[tauri::command]
pub fn music_media_keys(app: AppHandle, webview: tauri::Webview, wanted: bool) {
    let _ = (webview.label() == "main").then(|| set_media_keys(&app, Some(wanted)));
}

#[tauri::command]
pub fn shortcuts_get(app: AppHandle) -> Result<Value, String> {
    let mut cfg = state::get_value(&app, "desktopShortcuts")?.as_object().cloned().unwrap_or_default();
    cfg.entry("ptm").or_insert(json!(""));
    Ok(Value::Object(cfg))
}

#[tauri::command]
pub fn shortcuts_register(app: AppHandle, updates: Value) -> Result<Value, String> {
    let mut cfg = state::get_value(&app, "desktopShortcuts")?
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut result = serde_json::Map::new();
    if let Some(obj) = updates.as_object() {
        let _ = obj.get("pttMode").filter(|m| matches!(m.as_str(), Some("toggle" | "hold"))).map(|m| cfg.insert("pttMode".into(), m.clone()));
        for (k, v) in obj.iter().filter(|(k, _)| matches!(k.as_str(), "mute" | "deafen" | "ptt" | "ptm")) {
            let accel = v.as_str().filter(|a| a.len() <= 50);
            let refused = accel.is_some_and(|a| types_text(a) && !held(k, &cfg));
            let _ = match accel { Some(a) if refused => result.insert(k.clone(), json!({ "ok": false, "reason": "typing-key", "accel": a })), Some(_) => cfg.insert(k.clone(), v.clone()), None => None };
        }
    }
    state::set_value(&app, "desktopShortcuts", Value::Object(cfg.clone()))?;

    // Best-effort: unregister all then register known shortcuts.
    let _ = app.global_shortcut().unregister_all();
    PTT_DOWN.store(false, Ordering::SeqCst);
    PTM_DOWN.store(false, Ordering::SeqCst);
    for key in ["mute", "deafen", "ptt", "ptm"] {
        let accel = cfg.get(key).and_then(|v| v.as_str()).unwrap_or("");
        let hold = held(key, &cfg);
        let outcome = match accel.parse::<Shortcut>() {
            _ if accel.is_empty() => json!({ "ok": true, "reason": "ok" }),
            _ if types_text(accel) && !hold => json!({ "ok": false, "reason": "typing-key", "accel": accel }),
            Ok(shortcut) => {
                // The handler was empty, so mute, deafen and push-to-talk did
                // nothing in the Tauri build. The plugin reports press and
                // release, which is exactly what hold-mode PTT needs (the
                // Electron app has to emulate it for combos). The bridge in
                // app-bridge.js listens for these names.
                let key_name = key.to_string();
                let ok = app.global_shortcut().on_shortcut(shortcut, move |app, _s, e| {
                    let pressed = e.state() == ShortcutState::Pressed;
                    match key_name.as_str() {
                        "mute" => { if pressed { let _ = app.emit("voice:mute-toggle", ()); } }
                        "deafen" => { if pressed { let _ = app.emit("voice:deafen-toggle", ()); } }
                        _ if hold => {
                            // OS auto-repeat can fire Pressed again while held; only the first one talks.
                            let (down, event) = if key_name == "ptm" { (&PTM_DOWN, "voice:ptm") } else { (&PTT_DOWN, "voice:ptt") };
                            if pressed && !down.swap(true, Ordering::SeqCst) { let _ = app.emit(&format!("{event}-down"), ()); }
                            if !pressed && down.swap(false, Ordering::SeqCst) { let _ = app.emit(&format!("{event}-up"), ()); }
                        }
                        _ => { if pressed { let _ = app.emit("voice:ptt-toggle", ()); } }
                    }
                }).is_ok();
                if ok { json!({ "ok": true, "reason": "ok" }) } else { json!({ "ok": false, "reason": "conflict", "accel": accel }) }
            }
            Err(_) => json!({ "ok": false, "reason": "unsupported", "accel": accel }),
        };
        result.entry(key.to_string()).or_insert(outcome);
    }
    if let Some(mode) = cfg.get("pttMode") {
        result.insert("pttMode".into(), json!({ "ok": true, "reason": "ok", "mode": mode }));
    }
    set_media_keys(&app, None);
    Ok(Value::Object(result))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typing_keys() {
        for a in ["M", "Shift+M", "Space", "shift+space", "Num5", "7", "Plus"] { assert!(types_text(a), "{a}") }
        for a in ["CommandOrControl+Shift+M", "Alt+V", "F7", "Shift", "Alt", "MediaPlayPause", "", "Capslock"] { assert!(!types_text(a), "{a}") }
        let toggle = json!({ "pttMode": "toggle" }).as_object().cloned().unwrap();
        assert!(held("ptm", &toggle) && !held("ptt", &toggle) && !held("mute", &toggle) && held("ptt", &Default::default()));
    }
}
