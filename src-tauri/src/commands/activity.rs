use super::history::{action_flag, origin_of, sender, Sender};
use crate::state::{self, AppState};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use tauri::{AppHandle, Manager, Webview};
const MAX_SERVERS: usize = 200;
static ASKING: AtomicBool = AtomicBool::new(false);
pub fn clean_allowed(list: &Value) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    list.as_array().into_iter().flatten().filter_map(|v| v.as_str().and_then(origin_of)).filter(|o| seen.insert(o.clone())).take(MAX_SERVERS).collect()
}
fn allowed(app: &AppHandle) -> Vec<String> { clean_allowed(&state::get_value(app, "appActivityServers").unwrap_or(Value::Null)) }
fn page(app: &AppHandle, webview: &Webview) -> Option<String> {
    match sender(app, webview)? { Sender::Page(url) => origin_of(&url), Sender::Local => None }
}
#[cfg(unix)]
mod listener {
    use super::*;
    use crate::app_activity::{in_flatpak, runtime_dir, system_lookup, Current};
    use tauri::Emitter;
    pub fn unwatch(app: &AppHandle) {
        let st = app.state::<AppState>();
        let _ = st.activity_page.lock().take();
        st.activity.stop();
    }
    fn deliver(app: &AppHandle, current: &Current) {
        let watching = app.state::<AppState>().activity_page.lock().clone();
        let showing = app.get_webview_window("main").and_then(|w| w.url().ok()).and_then(|u| origin_of(u.as_str()));
        match (watching, showing) {
            (Some(w), Some(s)) if w == s => { let _ = app.emit_to("main", "app-activity:update", current); }
            (Some(_), _) => unwatch(app),
            _ => {}
        }
    }
    pub async fn watch(app: &AppHandle, origin: String) {
        *app.state::<AppState>().activity_page.lock() = Some(origin);
        let sink = app.clone();
        let _ = app.state::<AppState>().activity.start(&runtime_dir(|k| std::env::var(k).ok(), in_flatpak()), system_lookup(), std::sync::Arc::new(move |c| deliver(&sink, &c))).await;
        deliver(app, &app.state::<AppState>().activity.current());
    }
    pub fn stop(app: &AppHandle) { app.state::<AppState>().activity.stop() }
}
#[cfg(not(unix))]
mod listener {
    use super::*;
    pub fn unwatch(_: &AppHandle) {}
    pub async fn watch(_: &AppHandle, _: String) {}
    pub fn stop(_: &AppHandle) {}
}
pub use listener::{stop as activity_stop, unwatch as activity_unwatch};
async fn ask(app: &AppHandle, origin: &str) -> bool {
    let (app, host) = (app.clone(), url::Url::parse(origin).ok().and_then(|u| u.host_str().map(|h| u.port().map_or(h.to_string(), |p| format!("{h}:{p}")))).unwrap_or_else(|| origin.to_string()));
    super::dialogs::off_main(move || {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
        let parent = app.get_webview_window("main");
        let dialog = app.dialog().message(format!("{}\n\n{}", state::tv(&app, "appActivity.ask.message", &[("host", &host)]), state::t(&app, "appActivity.ask.detail"))).title(state::t(&app, "appActivity.ask.title")).kind(MessageDialogKind::Info).buttons(MessageDialogButtons::OkCancelCustom(state::t(&app, "appActivity.ask.confirm"), state::t(&app, "dialog.cancel")));
        let dialog = match &parent { Some(p) => dialog.parent(p), None => dialog };
        dialog.blocking_show()
    }).await.unwrap_or(false)
}
#[tauri::command]
pub async fn app_activity_status(app: AppHandle, webview: Webview) -> Value {
    let Some(origin) = page(&app, &webview).filter(|_| cfg!(unix)) else { return json!({ "supported": false, "enabled": false }) };
    let enabled = allowed(&app).contains(&origin);
    if enabled { listener::watch(&app, origin).await }
    json!({ "supported": true, "enabled": enabled })
}
#[tauri::command]
pub async fn app_activity_enable(app: AppHandle, webview: Webview, action: Option<Value>) -> Value {
    let Some(origin) = page(&app, &webview).filter(|_| cfg!(unix)) else { return json!({ "enabled": false }) };
    if !allowed(&app).contains(&origin) {
        if !action_flag(&action, "fresh") || ASKING.swap(true, SeqCst) { return json!({ "enabled": false }) }
        let yes = ask(&app, &origin).await;
        ASKING.store(false, SeqCst);
        if !yes { return json!({ "enabled": false }) }
        let _ = state::set_value(&app, "appActivityServers", json!(allowed(&app).into_iter().chain([origin.clone()]).collect::<Vec<_>>()));
    }
    let still = page(&app, &webview).as_deref() == Some(origin.as_str());
    if still { listener::watch(&app, origin).await }
    json!({ "enabled": still })
}
#[tauri::command]
pub fn app_activity_disable(app: AppHandle, webview: Webview) -> Value {
    if let Some(origin) = page(&app, &webview) {
        let _ = state::set_value(&app, "appActivityServers", json!(allowed(&app).into_iter().filter(|o| *o != origin).collect::<Vec<_>>()));
        listener::unwatch(&app);
    }
    json!({ "enabled": false })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_web_origins_are_remembered_as_allowed_servers() {
        assert_eq!(origin_of("https://chat.example:3000/app").as_deref(), Some("https://chat.example:3000"));
        assert_eq!(origin_of("file:///etc/passwd"), None);
        assert_eq!(origin_of("nonsense"), None);
        assert_eq!(clean_allowed(&json!(["https://a.example/x", "https://a.example", "javascript:1", 7, "http://b.example:8080/x"])), vec!["https://a.example", "http://b.example:8080"]);
        assert!(clean_allowed(&json!("https://a.example")).is_empty());
        assert!(clean_allowed(&Value::Null).is_empty());
        assert_eq!(clean_allowed(&json!((0..300).map(|i| format!("https://s{i}.example")).collect::<Vec<_>>())).len(), 200);
    }
}
