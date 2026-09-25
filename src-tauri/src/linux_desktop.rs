use notify_rust::{Hint, Notification};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager, UserAttentionType};
static BUS: Lazy<Option<zbus::blocking::Connection>> = Lazy::new(|| zbus::blocking::Connection::session().ok());
const DESKTOP_IDS: [&str; 2] = ["haven-desktop.desktop", "Haven.desktop"];
pub fn focus_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
pub fn notify(app: &AppHandle, title: &str, body: &str, silent: bool, channel: Option<String>) -> Result<(), String> {
    let mut n = Notification::new();
    n.appname("Haven").summary(title).body(body).icon("haven-desktop").action("default", "Open").hint(Hint::DesktopEntry("haven-desktop".into())).hint(Hint::Category("im.received".into()));
    if silent {
        n.hint(Hint::SuppressSound(true));
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let Ok(handle) = n.show() else { return };
        handle.wait_for_action(|action| {
            if action == "default" {
                let a = app.clone();
                let ch = channel.clone();
                let _ = app.run_on_main_thread(move || {
                    focus_main(&a);
                    if let Some(c) = ch {
                        let _ = a.emit_to("main", "notification-clicked", c);
                    }
                });
            }
        })
    });
    Ok(())
}
pub fn set_unread(app: &AppHandle, count: usize) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Some(bus) = BUS.as_ref() {
            let props: HashMap<&str, zbus::zvariant::Value> = HashMap::from([("count", zbus::zvariant::Value::I64(count as i64)), ("count-visible", zbus::zvariant::Value::Bool(count > 0))]);
            for id in DESKTOP_IDS {
                let _ = bus.emit_signal(None::<()>, "/com/haven/desktop", "com.canonical.Unity.LauncherEntry", "Update", &(format!("application://{id}"), &props));
            }
        }
        if let Some(w) = app.get_webview_window("main") {
            let focused = w.is_focused().unwrap_or(false);
            let _ = w.request_user_attention(if count > 0 && !focused { Some(UserAttentionType::Informational) } else { None });
        }
    });
}
fn tree_pids() -> Vec<u32> {
    let me = std::process::id();
    let parents: HashMap<u32, u32> = std::fs::read_dir("/proc").into_iter().flatten().flatten().filter_map(|e| {
        let pid: u32 = e.file_name().to_str()?.parse().ok()?;
        let stat = std::fs::read_to_string(e.path().join("stat")).ok()?;
        Some((pid, stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse().ok()?))
    }).collect();
    let owned = |mut p: u32| { while p > 1 { if p == me { return true; } p = parents.get(&p).copied().unwrap_or(0); } false };
    parents.keys().copied().filter(|p| owned(*p)).collect()
}
pub fn launcher_app_id() -> String {
    let unit = std::fs::read_to_string("/proc/self/cgroup").unwrap_or_default().rsplit('/').find(|u| u.starts_with("app-")).unwrap_or_default().to_string();
    let id = unit.trim_start_matches("app-").trim_end_matches(".scope").trim_end_matches(".service").split('@').next().unwrap_or_default();
    let id = id.rsplit_once('-').filter(|(_, n)| n.chars().all(|c| c.is_ascii_digit())).map_or(id, |(a, _)| a);
    let installed = |d: &str| dirs::data_dir().into_iter().chain(["/usr/local/share".into(), "/usr/share".into()]).any(|p: std::path::PathBuf| p.join(format!("applications/{d}.desktop")).exists());
    if id.to_lowercase().contains("haven") { id.to_string() } else if installed("haven-desktop") { "haven\\x2ddesktop".into() } else { "Haven".into() }
}
pub fn claim_scope(app_id: String) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Some(bus) = BUS.as_ref() else { return };
        let unit = format!("app-{app_id}-{}.scope", std::process::id());
        let _ = bus.call_method(Some("org.freedesktop.systemd1"), "/org/freedesktop/systemd1", Some("org.freedesktop.systemd1.Manager"), "StartTransientUnit", &(unit.as_str(), "fail", vec![("PIDs", zbus::zvariant::Value::from(tree_pids()))], Vec::<(&str, Vec<(&str, zbus::zvariant::Value)>)>::new()));
        let _ = tx.send(());
        std::thread::sleep(std::time::Duration::from_secs(3));
        let _ = bus.call_method(Some("org.freedesktop.systemd1"), "/org/freedesktop/systemd1", Some("org.freedesktop.systemd1.Manager"), "AttachProcessesToUnit", &(unit.as_str(), "", tree_pids()));
    });
    let _ = rx.recv_timeout(std::time::Duration::from_millis(1500));
}
