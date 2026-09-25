use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;
use x11_dl::{xinput2, xlib};
static BINDINGS: Lazy<Mutex<HashMap<(bool, u32), (String, bool)>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static STARTED: AtomicBool = AtomicBool::new(false);
fn modifier_keysyms(accel: &str) -> Option<[&'static str; 2]> {
    Some(match accel {
        "Control" | "Ctrl" | "CommandOrControl" | "CmdOrCtrl" => ["Control_L", "Control_R"],
        "Alt" | "Option" | "AltGr" => ["Alt_L", "Alt_R"],
        "Shift" => ["Shift_L", "Shift_R"],
        "Meta" | "Cmd" | "Command" | "Super" => ["Super_L", "Super_R"],
        _ => return None,
    })
}
fn mouse_button(accel: &str) -> Option<u32> {
    match accel.to_ascii_lowercase().as_str() {
        "mouse3" | "middleclick" => Some(2),
        "mouse4" => Some(8),
        "mouse5" => Some(9),
        _ => None,
    }
}
fn keycodes(syms: [&str; 2]) -> Vec<u32> {
    let Ok(xl) = xlib::Xlib::open() else { return vec![] };
    unsafe {
        let d = (xl.XOpenDisplay)(std::ptr::null());
        let out = if d.is_null() { vec![] } else { syms.iter().filter_map(|s| CString::new(*s).ok()).map(|s| (xl.XKeysymToKeycode)(d, (xl.XStringToKeysym)(s.as_ptr())) as u32).filter(|k| *k != 0).collect() };
        (!d.is_null()).then(|| (xl.XCloseDisplay)(d));
        out
    }
}
pub fn set_bindings(app: &AppHandle, entries: &[(String, String, bool)]) -> Vec<String> {
    let mut map = HashMap::new();
    let mut served = Vec::new();
    for (name, accel, hold) in entries {
        let inputs: Vec<(bool, u32)> = mouse_button(accel).map(|b| vec![(false, b)]).or_else(|| modifier_keysyms(accel).map(|s| keycodes(s).into_iter().map(|k| (true, k)).collect())).unwrap_or_default();
        (!inputs.is_empty()).then(|| served.push(name.clone()));
        inputs.into_iter().for_each(|i| { map.insert(i, (name.clone(), *hold)); });
    }
    let any = !map.is_empty();
    *BINDINGS.lock() = map;
    if any && !STARTED.swap(true, Ordering::SeqCst) {
        let app = app.clone();
        std::thread::spawn(move || listen(app));
    }
    served
}
fn listen(app: AppHandle) {
    let (Ok(xl), Ok(xi)) = (xlib::Xlib::open(), xinput2::XInput2::open()) else { return STARTED.store(false, Ordering::SeqCst) };
    unsafe {
        let d = (xl.XOpenDisplay)(std::ptr::null());
        let (mut op, mut ev, mut er) = (0, 0, 0);
        let name = CString::new("XInputExtension").unwrap_or_default();
        if d.is_null() || (xl.XQueryExtension)(d, name.as_ptr(), &mut op, &mut ev, &mut er) == 0 {
            return STARTED.store(false, Ordering::SeqCst);
        }
        let mut bits = [0u8; 4];
        for t in [xinput2::XI_RawKeyPress, xinput2::XI_RawKeyRelease, xinput2::XI_RawButtonPress, xinput2::XI_RawButtonRelease] {
            bits[(t >> 3) as usize] |= 1 << (t & 7);
        }
        let mut m = xinput2::XIEventMask { deviceid: xinput2::XIAllMasterDevices, mask_len: 4, mask: bits.as_mut_ptr() };
        (xi.XISelectEvents)(d, (xl.XDefaultRootWindow)(d), &mut m, 1);
        (xl.XFlush)(d);
        let mut held = std::collections::HashSet::new();
        loop {
            let mut e: xlib::XEvent = std::mem::zeroed();
            (xl.XNextEvent)(d, &mut e);
            let mut ck = e.generic_event_cookie;
            if e.get_type() != xlib::GenericEvent || ck.extension != op || (xl.XGetEventData)(d, &mut ck) == 0 {
                continue;
            }
            let detail = (*(ck.data as *const xinput2::XIRawEvent)).detail as u32;
            let (key, pressed) = match ck.evtype {
                xinput2::XI_RawKeyPress => (true, true),
                xinput2::XI_RawKeyRelease => (true, false),
                xinput2::XI_RawButtonPress => (false, true),
                _ => (false, false),
            };
            (xl.XFreeEventData)(d, &mut ck);
            let map = BINDINGS.lock().clone();
            let Some((name, hold)) = map.get(&(key, detail)).cloned() else { continue };
            let before = held.iter().any(|i| map.get(i).is_some_and(|a| a.0 == name));
            let _ = if pressed { held.insert((key, detail)) } else { held.remove(&(key, detail)) };
            let after = held.iter().any(|i| map.get(i).is_some_and(|a| a.0 == name));
            (before != after).then(|| crate::commands::fire(&app, &name, hold, after));
        }
    }
}
