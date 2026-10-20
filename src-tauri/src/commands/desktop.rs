use crate::state;
use serde_json::json;
use tauri::AppHandle;

#[tauri::command]
pub fn desktop_get_prefs(app: AppHandle) -> Result<serde_json::Value, String> {
    Ok(json!({
        "startOnLogin": state::get_bool(&app, "startOnLogin")?,
        "startHidden": state::get_bool(&app, "startHidden")?,
        "minimizeToTray": state::get_bool(&app, "minimizeToTray")?,
        "forceSDR": state::get_bool(&app, "forceSDR")?,
        "hideMenuBar": state::get_bool(&app, "hideMenuBar")?,
        "disableGpuVsync": state::get_bool(&app, "disableGpuVsync")?,
        "unlimitFrameRate": state::get_bool(&app, "unlimitFrameRate")?,
        "language": state::i18n_state(&app)?,
    }))
}

#[tauri::command]
pub fn desktop_set_start_on_login(app: AppHandle, enabled: bool) -> Result<bool, String> {
    state::set_value(&app, "startOnLogin", json!(enabled))?;
    #[cfg(target_os = "linux")]
    apply_autostart(&app)?;
    Ok(true)
}

#[tauri::command]
pub fn desktop_set_start_hidden(app: AppHandle, enabled: bool) -> Result<bool, String> {
    state::set_value(&app, "startHidden", json!(enabled))?;
    #[cfg(target_os = "linux")]
    apply_autostart(&app)?;
    Ok(true)
}

#[tauri::command]
pub fn desktop_set_minimize_to_tray(app: AppHandle, enabled: bool) -> Result<bool, String> {
    state::set_value(&app, "minimizeToTray", json!(enabled))?;
    Ok(true)
}

#[tauri::command]
pub fn desktop_set_force_sdr(app: AppHandle, enabled: bool) -> Result<serde_json::Value, String> {
    state::set_value(&app, "forceSDR", json!(enabled))?;
    Ok(json!({ "requiresRestart": true }))
}

#[tauri::command]
pub fn desktop_set_hide_menu_bar(app: AppHandle, enabled: bool) -> Result<bool, String> {
    state::set_value(&app, "hideMenuBar", json!(enabled))?;
    Ok(true)
}

#[tauri::command]
pub fn desktop_set_disable_gpu_vsync(
    app: AppHandle,
    enabled: bool,
) -> Result<serde_json::Value, String> {
    state::set_value(&app, "disableGpuVsync", json!(enabled))?;
    Ok(json!({ "requiresRestart": true }))
}

#[tauri::command]
pub fn desktop_set_unlimit_frame_rate(
    app: AppHandle,
    enabled: bool,
) -> Result<serde_json::Value, String> {
    state::set_value(&app, "unlimitFrameRate", json!(enabled))?;
    Ok(json!({ "requiresRestart": true }))
}

/// Writes or removes ~/.config/autostart/haven-desktop.desktop to match the
/// start-on-login and start-hidden preferences.
#[cfg(target_os = "linux")]
pub(crate) fn apply_autostart(app: &AppHandle) -> Result<(), String> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let dir = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".config")).join("autostart");
    let file = dir.join("haven-desktop.desktop");
    if !state::get_bool(app, "startOnLogin")? {
        let _ = std::fs::remove_file(&file);
        return Ok(());
    }
    let on_path = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).map(|d| d.join("haven-desktop")).find(|f| f.is_file())).flatten();
    let exec = std::env::var("APPIMAGE").ok().map(std::path::PathBuf::from).or(on_path).or_else(|| std::env::current_exe().ok()).ok_or("no executable path")?;
    let hidden = if state::get_bool(app, "startHidden")? { " --hidden" } else { "" };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(&file, format!("[Desktop Entry]\nType=Application\nName=Haven\nComment=Start Haven when you log in\nExec=\"{}\"{hidden}\nIcon=haven-desktop\nTerminal=false\nX-GNOME-Autostart-enabled=true\nX-KDE-autostart-after=panel\n", exec.display())).map_err(|e| e.to_string())
}

fn find_ffmpeg() -> Option<std::path::PathBuf> {
    which::which("ffmpeg").ok().or_else(|| {
        for cand in ["/usr/bin/ffmpeg", "/usr/local/bin/ffmpeg", "/bin/ffmpeg"] {
            let p = std::path::PathBuf::from(cand);
            if p.is_file() {
                return Some(p);
            }
        }
        None
    })
}

fn hash_bytes_128(bytes: &[u8]) -> String {
    let mut h1: u64 = 0xcbf29ce484222325;
    let mut h2: u64 = 0x84222325cbf29ce4;
    for (i, &b) in bytes.iter().enumerate() {
        if i & 1 == 0 {
            h1 ^= b as u64;
            h1 = h1.wrapping_mul(0x100000001b3);
        } else {
            h2 ^= b as u64;
            h2 = h2.wrapping_mul(0x100000001b3);
        }
    }
    format!("{h1:016x}{h2:016x}_{}", bytes.len())
}

#[tauri::command]
pub fn media_transcode_check() -> serde_json::Value {
    let ffmpeg = find_ffmpeg();
    json!({
        "available": ffmpeg.is_some(),
        "ffmpeg": ffmpeg.and_then(|p| p.to_str().map(str::to_string)),
    })
}

#[tauri::command]
pub async fn media_transcode(
    data_b64: String,
    ext_hint: Option<String>,
    audio_only: Option<bool>,
) -> Result<serde_json::Value, String> {
    static TRANSCODE_LOCK: once_cell::sync::Lazy<parking_lot::Mutex<()>> =
        once_cell::sync::Lazy::new(|| parking_lot::Mutex::new(()));
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = TRANSCODE_LOCK.lock();
        use base64::Engine as _;
        let ffmpeg = find_ffmpeg().ok_or_else(|| "ffmpeg not found on system".to_string())?;
        let cleaned = data_b64
            .strip_prefix("data:")
            .and_then(|s| s.split_once(";base64,").map(|(_, b)| b))
            .unwrap_or(&data_b64);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(cleaned.trim())
            .map_err(|e| format!("invalid base64 payload: {e}"))?;
        if bytes.is_empty() {
            return Err("empty media payload".to_string());
        }

        let is_audio = audio_only.unwrap_or(false);
        let digest = hash_bytes_128(&bytes);
        let mode_tag = if is_audio { "audio" } else { "video" };
        let mime = if is_audio { "audio/webm" } else { "video/webm" };

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("haven-desktop")
            .join("media-cache");
        std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

        let out_path = cache_dir.join(format!("{digest}_{mode_tag}.webm"));
        if let Ok(existing) = std::fs::read(&out_path) {
            if !existing.is_empty() {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&existing);
                return Ok(json!({
                    "ok": true,
                    "mime": mime,
                    "dataB64": b64,
                    "cached": true,
                }));
            }
        }

        let safe_ext: String = ext_hint
            .unwrap_or_else(|| if is_audio { "m4a".to_string() } else { "mp4".to_string() })
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(8)
            .collect();
        let ext = if safe_ext.is_empty() { "mp4" } else { &safe_ext };
        let uniq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let in_path = cache_dir.join(format!("tmp_in_{digest}_{uniq}.{ext}"));
        let tmp_out = cache_dir.join(format!("tmp_out_{digest}_{uniq}.webm"));

        std::fs::write(&in_path, &bytes).map_err(|e| format!("failed writing temp input: {e}"))?;

        let mut cmd = std::process::Command::new(&ffmpeg);
        cmd.arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-threads")
            .arg("2")
            .arg("-y")
            .arg("-i")
            .arg(&in_path);

        if is_audio {
            cmd.arg("-vn")
                .arg("-c:a")
                .arg("libopus")
                .arg("-b:a")
                .arg("128k")
                .arg("-f")
                .arg("webm")
                .arg(&tmp_out);
        } else {
            cmd.arg("-map")
                .arg("0:v:0")
                .arg("-map")
                .arg("0:a:0?")
                .arg("-c:v")
                .arg("libvpx")
                .arg("-threads")
                .arg("2")
                .arg("-crf")
                .arg("22")
                .arg("-b:v")
                .arg("2500k")
                .arg("-deadline")
                .arg("realtime")
                .arg("-cpu-used")
                .arg("5")
                .arg("-pix_fmt")
                .arg("yuv420p")
                .arg("-vf")
                .arg("scale=trunc(iw/2)*2:trunc(ih/2)*2")
                .arg("-c:a")
                .arg("libopus")
                .arg("-b:a")
                .arg("128k")
                .arg("-f")
                .arg("webm")
                .arg(&tmp_out);
        }

        let output = cmd.output();
        let _ = std::fs::remove_file(&in_path);

        let output = output.map_err(|e| {
            let _ = std::fs::remove_file(&tmp_out);
            format!("ffmpeg execution failed: {e}")
        })?;

        if !output.status.success() {
            let _ = std::fs::remove_file(&tmp_out);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ffmpeg transcode failed: {}", stderr.trim()));
        }

        let transcoded = std::fs::read(&tmp_out).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_out);
            format!("failed reading transcoded output: {e}")
        })?;
        if transcoded.is_empty() {
            let _ = std::fs::remove_file(&tmp_out);
            return Err("ffmpeg produced empty output".to_string());
        }

        let _ = std::fs::rename(&tmp_out, &out_path).or_else(|_| {
            let res = std::fs::copy(&tmp_out, &out_path).map(|_| ());
            let _ = std::fs::remove_file(&tmp_out);
            res
        });

        let b64 = base64::engine::general_purpose::STANDARD.encode(&transcoded);
        Ok(json!({
            "ok": true,
            "mime": mime,
            "dataB64": b64,
            "cached": false,
        }))
    })
    .await
    .map_err(|e| format!("transcode task join error: {e}"))?
}

