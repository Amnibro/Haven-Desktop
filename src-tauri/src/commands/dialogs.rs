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
#[cfg(target_os = "linux")]
fn get_linux_clipboard_image() -> Option<(Vec<u8>, &'static str, Option<String>)> {
    // 1. Wayland clipboard via wl-paste
    if let Ok(output) = std::process::Command::new("wl-paste")
        .arg("--list-types")
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            let types: Vec<&str> = s.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
            let preferred = [
                ("image/png", "image/png"),
                ("image/jpeg", "image/jpeg"),
                ("image/jpg", "image/jpeg"),
                ("image/webp", "image/webp"),
                ("image/bmp", "image/bmp"),
                ("image/x-bmp", "image/bmp"),
                ("image/gif", "image/gif"),
                ("image/svg+xml", "image/svg+xml"),
                ("image/x-png", "image/png"),
            ];
            let mut target_type = None;
            for (p, m) in preferred {
                if types.contains(&p) {
                    target_type = Some((p, m));
                    break;
                }
            }
            if target_type.is_none() {
                for t in &types {
                    if t.starts_with("image/") {
                        target_type = Some((*t, "image/png"));
                        break;
                    }
                }
            }
            if let Some((wl_type, out_mime)) = target_type {
                if let Ok(img_output) = std::process::Command::new("wl-paste")
                    .args(["--type", wl_type])
                    .output()
                {
                    if img_output.status.success() && !img_output.stdout.is_empty() {
                        let mut suggested_name = None;
                        if types.contains(&"application/x-kde-suggestedfilename") {
                            if let Ok(fn_out) = std::process::Command::new("wl-paste")
                                .args(["--type", "application/x-kde-suggestedfilename"])
                                .output()
                            {
                                if fn_out.status.success() {
                                    let s = String::from_utf8_lossy(&fn_out.stdout).trim().to_string();
                                    if !s.is_empty() && !s.contains('\n') {
                                        suggested_name = Some(s);
                                    }
                                }
                            }
                        }
                        return Some((img_output.stdout, out_mime, suggested_name));
                    }
                }
            }
        }
    }

    // 2. X11 clipboard via xclip
    if let Ok(output) = std::process::Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "TARGETS", "-o"])
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            let targets: Vec<&str> = s.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
            let preferred = [
                ("image/png", "image/png"),
                ("image/jpeg", "image/jpeg"),
                ("image/jpg", "image/jpeg"),
                ("image/webp", "image/webp"),
                ("image/bmp", "image/bmp"),
                ("image/gif", "image/gif"),
            ];
            for (p, m) in preferred {
                if targets.contains(&p) {
                    if let Ok(img_output) = std::process::Command::new("xclip")
                        .args(["-selection", "clipboard", "-t", p, "-o"])
                        .output()
                    {
                        if img_output.status.success() && !img_output.stdout.is_empty() {
                            return Some((img_output.stdout, m, None));
                        }
                    }
                }
            }
        }
    }

    None
}

/// Read a picture off the OS clipboard as a PNG/image data URL. WebKitGTK hands the
/// page a paste event with an empty DataTransfer for a bitmap (WebKit bug
/// 218519), so the bridge asks here and queues or re-dispatches with a File.
#[tauri::command]
pub async fn clipboard_read_image(app: AppHandle) -> Result<serde_json::Value, String> {
    off_main(move || {
        let err_reason = match app.clipboard().read_image() {
            Ok(image) => {
                let (w, h) = (image.width(), image.height());
                let rgba = image.rgba();
                if w > 0 && h > 0 && rgba.len() >= (w * h * 4) as usize {
                    if let Some(mut pm) = tiny_skia::Pixmap::new(w, h) {
                        for (dst, src) in pm.data_mut().chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
                            let a = src[3] as u32;
                            dst[0] = ((src[0] as u32 * a + 127) / 255) as u8;
                            dst[1] = ((src[1] as u32 * a + 127) / 255) as u8;
                            dst[2] = ((src[2] as u32 * a + 127) / 255) as u8;
                            dst[3] = src[3];
                        }
                        if let Ok(png) = pm.encode_png() {
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
                            return Ok(json!({
                                "ok": true,
                                "width": w,
                                "height": h,
                                "mime": "image/png",
                                "filename": null,
                                "dataUrl": format!("data:image/png;base64,{b64}")
                            }));
                        }
                    }
                }
                "empty-image".to_string()
            }
            Err(e) => e.to_string(),
        };

        // 2. Linux fallback: wl-paste (Wayland) and xclip (X11)
        #[cfg(target_os = "linux")]
        {
            if let Some((bytes, mime, suggested_name)) = get_linux_clipboard_image() {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                return Ok(json!({
                    "ok": true,
                    "mime": mime,
                    "filename": suggested_name,
                    "dataUrl": format!("data:{mime};base64,{b64}")
                }));
            }
        }

        Ok(json!({ "ok": false, "reason": err_reason }))
    })
    .await?
}

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

fn mime_for_extension(ext: &str) -> &'static str {
    match ext {
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" | "tgz" => "application/gzip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        "bz2" => "application/x-bzip2",
        "xz" => "application/x-xz",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "txt" | "log" | "md" => "text/plain",
        "json" => "application/json",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "weba" => "audio/webm",
        "flac" => "audio/flac",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "application/octet-stream",
    }
}

fn read_single_file_payload(clean_path: std::path::PathBuf) -> Result<serde_json::Value, String> {
    if !clean_path.is_file() {
        return Err(format!("Not a file: {}", clean_path.display()));
    }
    let meta = std::fs::metadata(&clean_path).map_err(|e| e.to_string())?;
    let size = meta.len();
    if size > 500 * 1024 * 1024 {
        return Err("File too large (exceeds 500 MB)".to_string());
    }
    let bytes = std::fs::read(&clean_path).map_err(|e| e.to_string())?;
    let name = clean_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let ext = clean_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mime = mime_for_extension(&ext);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    Ok(json!({
        "ok": true,
        "name": name,
        "path": clean_path.to_string_lossy(),
        "size": size,
        "mime": mime,
        "dataBase64": b64
    }))
}

#[tauri::command]
pub async fn read_file_for_upload(app: AppHandle, path: String) -> Result<serde_json::Value, String> {
    off_main(move || {
        let trimmed = path.trim();
        let clean_path = if let Ok(url) = url::Url::parse(trimmed) {
            if url.scheme() == "file" {
                url.to_file_path().map_err(|_| "Invalid file URL".to_string())?
            } else {
                std::path::PathBuf::from(trimmed)
            }
        } else {
            std::path::PathBuf::from(trimmed)
        };
        let wanted = clean_path.canonicalize().map_err(|e| e.to_string())?;
        clipboard_paths(&app).iter().any(|p| p.canonicalize().ok().as_ref() == Some(&wanted)).then_some(()).ok_or_else(|| "not-on-clipboard".to_string())?;
        read_single_file_payload(wanted)
    })
    .await?
}

#[tauri::command]
pub async fn pick_files_for_upload(app: AppHandle) -> Result<Vec<serde_json::Value>, String> {
    off_main(move || {
        let files = app.dialog().file().blocking_pick_files();
        let Some(files) = files else {
            return Ok(Vec::new());
        };
        let mut results = Vec::new();
        for file in files {
            if let Ok(path) = file.into_path() {
                if let Ok(payload) = read_single_file_payload(path) {
                    results.push(payload);
                }
            }
        }
        Ok(results)
    })
    .await?
}

#[cfg(target_os = "linux")]
fn get_linux_clipboard_uris() -> Vec<String> {
    let mut uris = Vec::new();

    // 1. Wayland clipboard via wl-paste
    let wl_targets = [
        "text/uri-list",
        "x-special/nautilus-clipboard",
        "x-special/gnome-copied-files",
    ];
    for target in &wl_targets {
        if let Ok(output) = std::process::Command::new("wl-paste")
            .args(["--type", target])
            .output()
        {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout);
                for line in s.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("file://")
                        || (!trimmed.starts_with('#')
                            && trimmed.starts_with('/')
                            && !trimmed.contains('\n'))
                    {
                        if !uris.contains(&trimmed.to_string()) {
                            uris.push(trimmed.to_string());
                        }
                    }
                }
                if !uris.is_empty() {
                    return uris;
                }
            }
        }
    }

    // 2. X11 clipboard via xclip
    let x11_targets = [
        "text/uri-list",
        "x-special/nautilus-clipboard",
        "x-special/gnome-copied-files",
    ];
    for target in &x11_targets {
        if let Ok(output) = std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-t", target, "-o"])
            .output()
        {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout);
                for line in s.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("file://")
                        || (!trimmed.starts_with('#')
                            && trimmed.starts_with('/')
                            && !trimmed.contains('\n'))
                    {
                        if !uris.contains(&trimmed.to_string()) {
                            uris.push(trimmed.to_string());
                        }
                    }
                }
                if !uris.is_empty() {
                    return uris;
                }
            }
        }
    }

    uris
}

#[cfg(target_os = "macos")]
extern "C" {
    fn haven_macos_get_clipboard_files(
        cb: extern "C" fn(path: *const std::ffi::c_char, user: *mut std::ffi::c_void),
        user: *mut std::ffi::c_void,
    );
}

#[cfg(target_os = "macos")]
fn get_macos_clipboard_paths() -> Vec<std::path::PathBuf> {
    extern "C" fn on_file(path: *const std::ffi::c_char, user: *mut std::ffi::c_void) {
        if path.is_null() || user.is_null() {
            return;
        }
        unsafe {
            let vec = &mut *(user as *mut Vec<std::path::PathBuf>);
            let cstr = std::ffi::CStr::from_ptr(path);
            if let Ok(s) = cstr.to_str() {
                vec.push(std::path::PathBuf::from(s));
            }
        }
    }
    let mut paths = Vec::new();
    unsafe {
        haven_macos_get_clipboard_files(on_file, &mut paths as *mut _ as *mut std::ffi::c_void);
    }
    paths
}

fn clipboard_paths(app: &AppHandle) -> Vec<std::path::PathBuf> {
    let mut candidate_paths: Vec<std::path::PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        for p in get_macos_clipboard_paths() {
            if p.is_file() && !candidate_paths.contains(&p) {
                candidate_paths.push(p);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let uris = get_linux_clipboard_uris();
        for uri in uris {
            if let Ok(url) = url::Url::parse(&uri) {
                if url.scheme() == "file" {
                    if let Ok(path) = url.to_file_path() {
                        if path.is_file() && !candidate_paths.contains(&path) {
                            candidate_paths.push(path);
                        }
                    }
                }
            } else {
                let path = std::path::PathBuf::from(&uri);
                if path.is_file() && !candidate_paths.contains(&path) {
                    candidate_paths.push(path);
                }
            }
        }
    }

    // If no paths found from native uri-list, check clipboard text
    if candidate_paths.is_empty() {
        if let Ok(text) = app.clipboard().read_text() {
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("file://") {
                    if let Ok(url) = url::Url::parse(trimmed) {
                        if let Ok(path) = url.to_file_path() {
                            if path.is_file() && !candidate_paths.contains(&path) {
                                candidate_paths.push(path);
                            }
                        }
                    }
                } else if trimmed.starts_with('/') && !trimmed.contains('\n') {
                    let path = std::path::PathBuf::from(trimmed);
                    if path.is_file() && !candidate_paths.contains(&path) {
                        candidate_paths.push(path);
                    }
                }
            }
        }
    }

    candidate_paths
}

#[tauri::command]
pub async fn clipboard_read_files(app: AppHandle) -> Result<serde_json::Value, String> {
    off_main(move || {
        let candidate_paths = clipboard_paths(&app);
        if candidate_paths.is_empty() {
            return Ok(json!({ "ok": false, "reason": "no-files" }));
        }

        let mut files = Vec::new();
        for path in candidate_paths {
            if let Ok(payload) = read_single_file_payload(path) {
                files.push(payload);
            }
        }

        if files.is_empty() {
            return Ok(json!({ "ok": false, "reason": "could-not-read" }));
        }

        Ok(json!({ "ok": true, "files": files }))
    })
    .await?
}
