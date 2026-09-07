use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::Serialize;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::slice;
use tauri::{AppHandle, Emitter};

#[repr(C)]
struct HavenAudioAppC {
    pid: u32,
    name: *mut c_char,
    icon: *mut c_char,
    active: c_int,
}

type HavenAudioDataCb = Option<unsafe extern "C" fn(*const f32, usize, *mut c_void)>;
type HavenAudioStatusCb =
    Option<unsafe extern "C" fn(*const c_char, *const c_char, i64, *mut c_void)>;

extern "C" {
    fn haven_audio_is_supported() -> c_int;
    fn haven_audio_get_apps(out_apps: *mut *mut HavenAudioAppC, out_count: *mut usize) -> c_int;
    fn haven_audio_free_apps(apps: *mut HavenAudioAppC, count: usize);
    fn haven_audio_start_capture(
        pid: u32,
        mode: *const c_char,
        data_cb: HavenAudioDataCb,
        status_cb: HavenAudioStatusCb,
        user: *mut c_void,
    ) -> c_int;
    fn haven_audio_stop_capture();
    fn haven_audio_cleanup();
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioApp {
    pub pid: u32,
    pub name: String,
    pub icon: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureStatus {
    pub kind: String,
    pub message: String,
    pub code: i64,
}

static EMITTER: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));

pub fn set_emitter(app: AppHandle) {
    *EMITTER.lock() = Some(app);
}

pub fn is_supported() -> bool {
    unsafe { haven_audio_is_supported() != 0 }
}

pub fn get_applications() -> Vec<AudioApp> {
    unsafe {
        let mut ptr: *mut HavenAudioAppC = std::ptr::null_mut();
        let mut count: usize = 0;
        if haven_audio_get_apps(&mut ptr, &mut count) == 0 || ptr.is_null() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let app = &*ptr.add(i);
            let name = if app.name.is_null() {
                String::new()
            } else {
                CStr::from_ptr(app.name).to_string_lossy().into_owned()
            };
            let icon = if app.icon.is_null() {
                String::new()
            } else {
                CStr::from_ptr(app.icon).to_string_lossy().into_owned()
            };
            out.push(AudioApp {
                pid: app.pid,
                name,
                icon,
                active: app.active != 0,
            });
        }
        haven_audio_free_apps(ptr, count);
        out
    }
}

unsafe extern "C" fn on_data(data: *const f32, count: usize, _user: *mut c_void) {
    if data.is_null() || count == 0 {
        return;
    }
    let samples = slice::from_raw_parts(data, count).to_vec();
    if let Some(app) = EMITTER.lock().as_ref() {
        let _ = app.emit("audio:capture-data", samples);
    }
}

unsafe extern "C" fn on_status(
    kind: *const c_char,
    message: *const c_char,
    code: i64,
    _user: *mut c_void,
) {
    let kind = if kind.is_null() {
        "stopped".into()
    } else {
        CStr::from_ptr(kind).to_string_lossy().into_owned()
    };
    let message = if message.is_null() {
        String::new()
    } else {
        CStr::from_ptr(message).to_string_lossy().into_owned()
    };
    let status = CaptureStatus { kind, message, code };
    if let Some(app) = EMITTER.lock().as_ref() {
        let _ = app.emit("audio:capture-status", status);
    }
}

pub fn start_capture(pid: u32, mode: &str) -> bool {
    let mode_c = CString::new(mode).unwrap_or_else(|_| CString::new("include").unwrap());
    unsafe {
        haven_audio_start_capture(
            pid,
            mode_c.as_ptr(),
            Some(on_data),
            Some(on_status),
            std::ptr::null_mut(),
        ) != 0
    }
}

pub fn stop_capture() {
    unsafe { haven_audio_stop_capture() }
}

pub fn cleanup() {
    unsafe { haven_audio_cleanup() }
}
