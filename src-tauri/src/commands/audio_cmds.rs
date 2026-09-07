use crate::audio;
use tauri::AppHandle;

#[tauri::command]
pub fn audio_is_supported() -> bool {
    audio::is_supported()
}

#[tauri::command]
pub fn audio_get_apps() -> Vec<audio::AudioApp> {
    audio::get_applications()
}

#[tauri::command]
pub fn audio_start_capture(app: AppHandle, pid: u32, mode: Option<String>) -> bool {
    audio::set_emitter(app);
    audio::start_capture(pid, mode.as_deref().unwrap_or("include"))
}

#[tauri::command]
pub fn audio_stop_capture() {
    audio::stop_capture()
}
