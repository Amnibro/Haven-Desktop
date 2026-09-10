use crate::state;
use base64::Engine;
use serde_json::json;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

fn decode_image_payload(payload: &str) -> Result<Vec<u8>, String> {
    let raw = payload.trim();
    let data = raw
        .rsplit(',')
        .next()
        .unwrap_or(raw)
        .split_whitespace()
        .collect::<String>();
    base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| e.to_string())
}

fn safe_filename(name: &str) -> String {
    let base = name.replace('\\', "/");
    let leaf = base.rsplit('/').next().unwrap_or("image.png");
    let cleaned: String = leaf
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    if cleaned.is_empty() || cleaned.starts_with('.') {
        "haven-image.png".into()
    } else {
        cleaned
    }
}

#[tauri::command]
pub fn dialog_alert(app: AppHandle, message: String) -> bool {
    app.dialog()
        .message(message)
        .kind(MessageDialogKind::Info)
        .title("Haven")
        .buttons(MessageDialogButtons::Ok)
        .blocking_show();
    true
}

#[tauri::command]
pub fn dialog_confirm(app: AppHandle, message: String) -> bool {
    app.dialog()
        .message(message)
        .kind(MessageDialogKind::Info)
        .title("Haven")
        .buttons(MessageDialogButtons::OkCancel)
        .blocking_show()
}

#[tauri::command]
pub fn dialog_prompt(
    app: AppHandle,
    message: String,
    default_value: Option<String>,
) -> Option<String> {
    let default_value = default_value.unwrap_or_default();
    let ok = app
        .dialog()
        .message(format!(
            "{message}{}",
            if default_value.is_empty() {
                String::new()
            } else {
                format!("\n\n{}", state::t(&app, "dialog.defaultValue").replace("{value}", &default_value))
            }
        ))
        .kind(MessageDialogKind::Info)
        .title("Haven")
        .buttons(MessageDialogButtons::OkCancel)
        .blocking_show();
    if ok {
        Some(default_value)
    } else {
        None
    }
}

#[tauri::command]
pub fn clipboard_write_text(app: AppHandle, text: String) -> Result<serde_json::Value, String> {
    match app.clipboard().write_text(text) {
        Ok(()) => Ok(serde_json::json!({ "ok": true })),
        Err(e) => Ok(serde_json::json!({ "ok": false, "reason": e.to_string() })),
    }
}

#[tauri::command]
pub fn clipboard_write_image(app: AppHandle, payload: String) -> Result<serde_json::Value, String> {
    if payload.is_empty() {
        return Ok(serde_json::json!({ "ok": false, "reason": "no-payload" }));
    }
    match app.clipboard().write_text(&payload) {
        Ok(()) => Ok(serde_json::json!({ "ok": true, "reason": "text-fallback" })),
        Err(e) => Ok(serde_json::json!({ "ok": false, "reason": e.to_string() })),
    }
}

#[tauri::command]
pub fn save_image(
    app: AppHandle,
    payload: String,
    filename: Option<String>,
) -> Result<serde_json::Value, String> {
    if payload.is_empty() {
        return Ok(json!({ "ok": false, "reason": "no-payload" }));
    }
    let bytes = decode_image_payload(&payload)?;
    if bytes.is_empty() {
        return Ok(json!({ "ok": false, "reason": "empty-image" }));
    }
    let name = safe_filename(filename.as_deref().unwrap_or("haven-image.png"));
    let ext = name.rsplit('.').next().unwrap_or("png");
    let picked = app
        .dialog()
        .file()
        .set_title(state::t(&app, "dialog.saveImage"))
        .set_file_name(&name)
        .add_filter("Image", &[ext, "png", "jpg", "jpeg", "gif", "webp"])
        .blocking_save_file();
    let Some(picked) = picked else {
        return Ok(json!({ "ok": false, "cancelled": true }));
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true, "path": path.to_string_lossy() }))
}
