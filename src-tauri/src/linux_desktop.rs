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
