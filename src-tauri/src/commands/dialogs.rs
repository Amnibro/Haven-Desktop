use crate::state;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

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
