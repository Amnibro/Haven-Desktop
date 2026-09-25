use crate::audio;

#[tauri::command]
pub fn audio_is_supported() -> bool {
    audio::is_supported()
}

#[tauri::command]
pub fn audio_get_apps() -> Vec<audio::AudioApp> {
    audio::get_applications()
}

#[tauri::command]
pub fn audio_stop_capture() {
    audio::stop_capture()
}
