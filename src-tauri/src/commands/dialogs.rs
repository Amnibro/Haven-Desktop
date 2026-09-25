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

// The dialog commands are async so Tauri runs them off the GTK main thread.
// tauri-plugin-dialog shows every dialog on the main thread and its
// blocking_* calls wait on a channel for the answer; called from a sync
// command (which runs on the main thread) that wait can never finish, and the
// whole app froze until the desktop killed it.
async fn off_main<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dialog_alert(app: AppHandle, message: String) -> Result<bool, String> {
    off_main(move || {
        app.dialog()
            .message(message)
            .kind(MessageDialogKind::Info)
            .title("Haven")
            .buttons(MessageDialogButtons::Ok)
            .blocking_show();
        true
    })
    .await
}

#[tauri::command]
pub async fn dialog_confirm(app: AppHandle, message: String) -> Result<bool, String> {
    off_main(move || {
        app.dialog()
            .message(message)
            .kind(MessageDialogKind::Info)
            .title("Haven")
            .buttons(MessageDialogButtons::OkCancel)
            .blocking_show()
    })
    .await
}

#[tauri::command]
pub async fn dialog_prompt(
    app: AppHandle,
    message: String,
    default_value: Option<String>,
) -> Result<Option<String>, String> {
    off_main(move || {
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
    })
    .await
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
    // Put a real picture on the clipboard. This used to write the base64 text
    // and report ok, so "Copy image" pasted a wall of text and the web app
    // skipped its own fallback. Anything but PNG reports not-ok so it runs.
    let bytes = match decode_image_payload(&payload) {
        Ok(b) => b,
        Err(e) => return Ok(json!({ "ok": false, "reason": e })),
    };
    let Ok(pm) = tiny_skia::Pixmap::decode_png(&bytes) else {
        return Ok(json!({ "ok": false, "reason": "not-png" }));
    };
    let mut rgba = Vec::with_capacity(pm.data().len());
    for px in pm.pixels() {
        let c = px.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    let image = tauri::image::Image::new_owned(rgba, pm.width(), pm.height());
    match app.clipboard().write_image(&image) {
        Ok(()) => Ok(json!({ "ok": true })),
        Err(e) => Ok(json!({ "ok": false, "reason": e.to_string() })),
    }
}

/// Read a picture off the OS clipboard as a PNG data URL. WebKitGTK hands the
/// page a paste event with an empty DataTransfer for a bitmap (WebKit bug
/// 218519), so the bridge asks here and re-dispatches the paste with a File.
#[tauri::command]
pub async fn save_image(
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
    let ext = ext.to_string();
    let title = state::t(&app, "dialog.saveImage");
    let picked = off_main(move || {
        app.dialog()
            .file()
            .set_title(title)
            .set_file_name(&name)
            .add_filter("Image", &[ext.as_str(), "png", "jpg", "jpeg", "gif", "webp"])
            .blocking_save_file()
    })
    .await?;
    let Some(picked) = picked else {
        return Ok(json!({ "ok": false, "cancelled": true }));
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true, "path": path.to_string_lossy() }))
}
