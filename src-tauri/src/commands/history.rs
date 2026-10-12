use crate::server_list::{self as list, Plan, PromptText, Request};
use crate::state::{self, AppState};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, Webview};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult};
pub enum Sender { Local, Page(String) }
pub fn sender(app: &AppHandle, webview: &Webview) -> Option<Sender> {
    let url = webview.url().ok()?;
    match webview.label() {
        "welcome" => Some(Sender::Local),
        "main" if crate::nav_fail::is_local_ui(&url) => Some(Sender::Local),
        "main" => state::normalize_server_url(url.as_str()).or_else(|| app.state::<AppState>().active_server_url.lock().clone()).map(Sender::Page),
        _ => None,
    }
}
fn action_flag(action: &Option<Value>, key: &str) -> bool { action.as_ref().and_then(|a| a.get(key)).and_then(Value::as_bool).unwrap_or(false) }
async fn ask(app: &AppHandle, text: PromptText) -> (bool, bool) {
    let app = app.clone();
    let (confirm, block) = (text.confirm.clone(), text.block.clone());
    let answer = super::dialogs::off_main(move || {
        let parent = app.get_webview_window("main").or_else(|| app.get_webview_window("welcome"));
        let _ = parent.as_ref().map(|p| p.unminimize());
        let dialog = app.dialog().message(format!("{}\n\n{}", text.message, text.detail)).title(text.title).kind(MessageDialogKind::Warning).buttons(MessageDialogButtons::YesNoCancelCustom(text.confirm, text.cancel, text.block));
        let dialog = match &parent { Some(p) => dialog.parent(p), None => dialog };
        dialog.blocking_show_with_result()
    }).await.unwrap_or(MessageDialogResult::Cancel);
    match answer { MessageDialogResult::Custom(s) if s == confirm => (true, false), MessageDialogResult::Yes | MessageDialogResult::Ok => (true, false), MessageDialogResult::Custom(s) => (false, s == block), _ => (false, false) }
}
async fn gate(app: &AppHandle, requester: &str, key: &str, text: PromptText, fresh: bool) -> &'static str {
    let id = format!("{requester}\n{key}");
    {
        let st = app.state::<AppState>();
        let mut g = st.list_gate.lock();
        if g.blocked.contains(requester) { return "blocked" }
        let _ = fresh.then(|| g.declined.remove(&id));
        if g.declined.contains(&id) { return "declined" }
        if g.asking.contains(requester) || g.waiting >= 4 { return "busy" }
        g.asking.insert(requester.to_string());
        g.waiting += 1;
    }
    let st = app.state::<AppState>();
    let turn = st.list_gate_turn.lock().await;
    let skip = { let g = st.list_gate.lock(); if g.blocked.contains(requester) { Some("blocked") } else if g.declined.contains(&id) { Some("declined") } else { None } };
    let answer = match skip { Some(a) => a, None => {
        let (confirmed, block) = ask(app, text).await;
        let mut g = st.list_gate.lock();
        let _ = block.then(|| g.blocked.insert(requester.to_string()));
        let _ = (!confirmed).then(|| g.declined.insert(id.clone()));
        if confirmed { "confirmed" } else { "declined" }
    } };
    drop(turn);
    let mut g = st.list_gate.lock();
    g.asking.remove(requester);
    g.waiting -= 1;
    answer
}
pub async fn request_change(app: &AppHandle, webview: &Webview, mut r: Request, action: &Option<Value>) -> (bool, Value, bool) {
    r.user = action_flag(action, "user");
    let Some(who) = sender(app, webview) else { return (false, json!("denied"), false) };
    let mut current = list::read(app);
    if let Sender::Page(requester) = &who {
        let open: Vec<String> = app.state::<AppState>().active_server_url.lock().clone().into_iter().collect();
        match list::plan(&current, &r, requester, &open) {
            Plan::Ignore(v) => return (false, v, false),
            Plan::Apply => {}
            Plan::Ask(key, prompt) => {
                let asker = current.history.iter().find(|e| list::url_of(e) == requester).map(|e| list::name_of(e).to_string()).unwrap_or_default();
                let text = list::prompt_text(app, &prompt, &asker, requester);
                let answer = gate(app, requester, &key, text, action_flag(action, "fresh")).await;
                if answer != "confirmed" { return (false, json!(answer), false) }
                current = list::read(app);
                if let Plan::Ignore(v) = list::plan(&current, &r, requester, &open) { return (false, v, false) }
            }
        }
    }
    let before = current.clone();
    let result = list::apply(&mut current, &r, list::now_ms());
    let saved = list::save(app, &current, &before);
    (true, result, saved)
}
pub fn history_values(app: &AppHandle) -> Vec<Value> { list::read(app).history.into_iter().map(Value::Object).collect() }
#[tauri::command]
pub fn server_history_get(app: AppHandle) -> Result<Vec<Value>, String> {
    let raw = state::get_value(&app, "serverHistory")?;
    let cleaned = history_values(&app);
    let _ = (raw.as_array() != Some(&cleaned)).then(|| state::set_value(&app, "serverHistory", json!(cleaned)));
    Ok(cleaned)
}
#[tauri::command]
pub async fn server_history_add(app: AppHandle, webview: Webview, url: String, name: Option<String>, opts: Option<Value>, action: Option<Value>) -> Result<Value, String> {
    let r = Request { kind: "add".into(), url, name: name.unwrap_or_default(), opts: opts.unwrap_or(Value::Null), user: false };
    let (_, result, _) = request_change(&app, &webview, r, &action).await;
    Ok(if result == json!("denied") { json!("refused") } else { result })
}
#[tauri::command]
pub async fn server_history_remove(app: AppHandle, webview: Webview, url: String, action: Option<Value>) -> Result<Vec<Value>, String> {
    let r = Request { kind: "remove".into(), url: url.clone(), ..Default::default() };
    let (applied, _, _) = request_change(&app, &webview, r, &action).await;
    if let Some(removed) = state::normalize_server_url(&url).filter(|_| applied) {
        let app_state = app.state::<AppState>();
        let _ = app_state.primary_server_url.lock().take_if(|p| *p == removed);
        app_state.known_server_urls.lock().remove(&removed);
        app_state.server_badges.lock().remove(&removed);
        let mut prefs = state::get_user_prefs(&app)?;
        let remembered = prefs.get("serverUrl").and_then(|v| v.as_str()).and_then(state::normalize_server_url);
        if remembered.as_deref() == Some(removed.as_str()) {
            let _ = prefs.as_object_mut().map(|o| o.insert("serverUrl".into(), Value::Null));
            state::set_value(&app, "userPrefs", prefs)?;
        }
    }
    Ok(history_values(&app))
}
#[tauri::command]
pub async fn server_history_update_name(app: AppHandle, webview: Webview, url: String, name: String, opts: Option<Value>, action: Option<Value>) -> Result<bool, String> {
    let r = Request { kind: "rename".into(), url, name, opts: opts.unwrap_or(Value::Null), user: false };
    let (applied, _, saved) = request_change(&app, &webview, r, &action).await;
    Ok(applied && saved)
}
#[tauri::command]
pub fn server_list_get(app: AppHandle) -> Value { list::view(&list::read(&app)) }
#[tauri::command]
pub fn server_list_set_order(app: AppHandle, webview: Webview, urls: Value) -> bool {
    if sender(&app, &webview).is_none() { return false }
    let mut current = list::read(&app);
    let before = current.clone();
    list::set_order(&mut current, &urls);
    list::save(&app, &current, &before)
}
