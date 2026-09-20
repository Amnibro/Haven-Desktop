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

/// Read a picture off the OS clipboard as a PNG data URL. WebKitGTK hands the
/// page a paste event with an empty DataTransfer for a bitmap (WebKit bug
/// 218519), so the bridge asks here and re-dispatches the paste with a File.
#[tauri::command]
pub fn clipboard_read_image(app: AppHandle) -> Result<serde_json::Value, String> {
    let image = match app.clipboard().read_image() {
        Ok(img) => img,
        Err(e) => return Ok(json!({ "ok": false, "reason": e.to_string() })),
    };
    let (w, h) = (image.width(), image.height());
    let rgba = image.rgba();
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return Ok(json!({ "ok": false, "reason": "empty-image" }));
    }
    let Some(mut pm) = tiny_skia::Pixmap::new(w, h) else {
        return Ok(json!({ "ok": false, "reason": "pixmap" }));
    };
    for (dst, src) in pm.data_mut().chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        let a = src[3] as u32;
        dst[0] = ((src[0] as u32 * a + 127) / 255) as u8;
        dst[1] = ((src[1] as u32 * a + 127) / 255) as u8;
        dst[2] = ((src[2] as u32 * a + 127) / 255) as u8;
        dst[3] = src[3];
    }
    let png = pm.encode_png().map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Ok(json!({ "ok": true, "width": w, "height": h, "dataUrl": format!("data:image/png;base64,{b64}") }))
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
