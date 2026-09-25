use crate::audio;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
type Answer = Option<u32>;
static PENDING: Lazy<Mutex<Option<tauri::async_runtime::Sender<Answer>>>> = Lazy::new(|| Mutex::new(None));
#[derive(Serialize)]
pub struct ShareChoice {
    ok: bool,
    audio: bool,
}
fn answer(a: Answer) {
    if let Some(tx) = PENDING.lock().take() {
        let _ = tx.try_send(a);
    }
}
#[tauri::command]
pub async fn share_picker_open(app: AppHandle) -> Result<ShareChoice, String> {
    if !audio::is_supported() {
        return Ok(ShareChoice { ok: true, audio: false });
    }
    let (tx, mut rx) = tauri::async_runtime::channel::<Answer>(1);
    answer(None);
    *PENDING.lock() = Some(tx);
    if let Some(w) = app.get_webview_window("share-picker") {
        let _ = w.destroy();
    }
    let mut b = WebviewWindowBuilder::new(&app, "share-picker", WebviewUrl::App("share-picker.html".into())).title("Share your screen").inner_size(440.0, 540.0).resizable(false).minimizable(false).maximizable(false).center().always_on_top(true).focused(true);
    if let Some(main) = app.get_webview_window("main") {
        b = b.parent(&main).map_err(|e| e.to_string())?;
    }
    let w = b.build().map_err(|e| e.to_string())?;
    w.on_window_event(|e| {
        if let tauri::WindowEvent::Destroyed = e {
            answer(None);
        }
    });
    let picked = rx.recv().await.flatten();
    let Some(pid) = picked else { return Ok(ShareChoice { ok: false, audio: false }) };
    let audio = pid != 0 && {
        audio::set_emitter(app.clone());
        audio::start_capture(pid, "include")
    };
    Ok(ShareChoice { ok: true, audio })
}
#[tauri::command]
pub fn share_picker_choose(app: AppHandle, cancelled: bool, pid: Option<u32>) {
    answer((!cancelled).then(|| pid.unwrap_or(0)));
    if let Some(w) = app.get_webview_window("share-picker") {
        let _ = w.destroy();
    }
}
