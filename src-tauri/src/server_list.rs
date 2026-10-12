use crate::state;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use tauri::{AppHandle, Emitter};
pub const HISTORY_CAP: usize = 100;
const REMOVED_CAP: usize = 200;
const NAME_MAX: usize = 100;
const ICON_MAX: usize = 2048;
const TEXT_MAX: usize = 80;
const ICON_TEXT_MAX: usize = 200;
pub type Entry = Map<String, Value>;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerList { pub history: Vec<Entry>, pub removed: Vec<String>, pub order: Vec<String> }
#[derive(Clone, Debug, Default)]
pub struct Request { pub kind: String, pub url: String, pub name: String, pub opts: Value, pub user: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt { pub kind: &'static str, pub url: String, pub name: String, pub old_name: String, pub icon: String, pub icon_change: &'static str }
#[derive(Clone, Debug, PartialEq)]
pub enum Plan { Ignore(Value), Apply, Ask(String, Prompt) }
pub struct PromptText { pub title: String, pub message: String, pub detail: String, pub confirm: String, pub cancel: String, pub block: String }
pub fn url_of(e: &Entry) -> &str { e.get("url").and_then(Value::as_str).unwrap_or("") }
pub fn name_of(e: &Entry) -> &str { e.get("name").and_then(Value::as_str).unwrap_or("") }
fn flag(e: &Entry, k: &str) -> bool { e.get(k).and_then(Value::as_bool).unwrap_or(false) }
fn last(e: &Entry) -> f64 { e.get("lastConnected").and_then(Value::as_f64).unwrap_or(0.0) }
fn norm(raw: &str) -> Option<String> { state::normalize_server_url(raw) }
pub fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }
pub fn valid_host(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else { return false };
    match u.host() {
        Some(url::Host::Ipv4(_)) | Some(url::Host::Ipv6(_)) => true,
        Some(url::Host::Domain(h)) => h == "localhost" || (h.contains('.') && !h.eq_ignore_ascii_case("http") && !h.eq_ignore_ascii_case("https")) || (u.port().is_some() && !h.eq_ignore_ascii_case("http") && !h.eq_ignore_ascii_case("https")),
        None => false,
    }
}
pub fn sanitize(list: &Value) -> Vec<Entry> {
    let mut seen = HashSet::new();
    list.as_array().into_iter().flatten().filter_map(Value::as_object).filter_map(|item| {
        let url = norm(item.get("url").and_then(Value::as_str)?).filter(|u| valid_host(u) && seen.insert(u.clone()))?;
        let mut e = item.clone();
        let name = e.get("name").and_then(Value::as_str).filter(|n| !n.trim().is_empty()).map(str::to_string).unwrap_or_else(|| url.clone());
        e.insert("name".into(), json!(name));
        e.insert("lastConnected".into(), json!(item.get("lastConnected").and_then(Value::as_f64).filter(|n| n.is_finite() && *n >= 0.0).map(|n| n as u64).unwrap_or(0)));
        e.insert("url".into(), json!(url));
        Some(e)
    }).collect()
}
fn clean_urls(v: &Value) -> Vec<String> {
    let mut seen = HashSet::new();
    v.as_array().into_iter().flatten().filter_map(Value::as_str).filter_map(norm).filter(|u| valid_host(u) && seen.insert(u.clone())).collect()
}
pub fn from_values(history: &Value, removed: &Value, order: &Value) -> ServerList {
    let removed = clean_urls(removed);
    let gone: HashSet<String> = removed.iter().cloned().collect();
    let history = sanitize(history).into_iter().filter(|e| !gone.contains(url_of(e))).collect();
    let order = clean_urls(order).into_iter().filter(|u| !gone.contains(u)).collect();
    ServerList { history, removed, order }
}
pub fn read(app: &AppHandle) -> ServerList {
    let get = |k: &str| state::get_value(app, k).unwrap_or(Value::Null);
    from_values(&get("serverHistory"), &get("serverListRemoved"), &get("serverListOrder"))
}
pub fn save(app: &AppHandle, list: &ServerList, before: &ServerList) -> bool {
    if list == before { return false }
    let written = state::store(app).map(|s| { s.set("serverHistory", json!(list.history)); s.set("serverListRemoved", json!(list.removed)); s.set("serverListOrder", json!(list.order)); s.save().is_ok() }).unwrap_or(false);
    let _ = app.emit_to("main", "server-list:changed", ());
    written
}
fn cap(l: &mut ServerList, keep: &str) {
    while l.history.len() > HISTORY_CAP {
        let Some(i) = l.history.iter().enumerate().filter(|(_, e)| url_of(e) != keep).min_by(|a, b| last(a.1).total_cmp(&last(b.1))).map(|(i, _)| i) else { break };
        let gone = l.history.remove(i);
        l.order.retain(|u| u != url_of(&gone));
    }
}
fn clean_name(name: &str) -> String { name.trim().chars().take(NAME_MAX).collect() }
pub fn add(l: &mut ServerList, raw: &str, raw_name: &str, user_initiated: bool, now: u64) -> &'static str {
    let Some(url) = norm(raw).filter(|u| valid_host(u)) else { return "invalid" };
    if l.removed.contains(&url) { if !user_initiated { return "refused" } l.removed.retain(|u| u != &url) }
    let name = clean_name(raw_name);
    if let Some(e) = l.history.iter_mut().find(|e| url_of(e) == url) {
        let bare = { let n = name_of(e); n.is_empty() || n == url };
        let _ = (!name.is_empty() && !flag(e, "customName") && bare).then(|| e.insert("name".into(), json!(name)));
        return "exists";
    }
    let mut e = Entry::new();
    e.insert("url".into(), json!(url));
    e.insert("name".into(), json!(if name.is_empty() { url.clone() } else { name.clone() }));
    e.insert("lastConnected".into(), json!(0));
    if user_initiated && !name.is_empty() && name != "Haven" && norm(&name).as_deref() != Some(url.as_str()) { e.insert("customName".into(), json!(true)); e.insert("editedAt".into(), json!(now)); }
    l.history.push(e);
    cap(l, &url);
    "added"
}
pub fn remove(l: &mut ServerList, raw: &str) -> bool {
    let Some(url) = norm(raw) else { return false };
    let had = l.history.iter().any(|e| url_of(e) == url);
    l.history.retain(|e| url_of(e) != url);
    l.order.retain(|u| u != &url);
    l.removed.retain(|u| u != &url);
    l.removed.push(url);
    while l.removed.len() > REMOVED_CAP { l.removed.remove(0); }
    had
}
pub fn mark_connected(l: &mut ServerList, raw: &str, now: u64) {
    let Some(url) = norm(raw) else { return };
    l.removed.retain(|u| u != &url);
    match l.history.iter_mut().find(|e| url_of(e) == url) {
        Some(e) => { e.insert("lastConnected".into(), json!(now)); }
        None => { let mut e = Entry::new(); e.insert("url".into(), json!(url)); e.insert("name".into(), json!(url)); e.insert("lastConnected".into(), json!(now)); l.history.push(e); }
    }
    cap(l, &url);
}
fn rename_opts(opts: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(o) = opts.as_object() else { return out };
    let _ = o.get("custom").and_then(Value::as_bool).map(|c| out.insert("custom".into(), json!(c)));
    let _ = o.get("editedAt").and_then(Value::as_f64).filter(|n| n.is_finite() && *n > 0.0).map(|n| out.insert("editedAt".into(), json!(n as u64)));
    let _ = o.get("icon").map(|i| out.insert("icon".into(), i.clone()));
    out
}
pub fn update_name(l: &mut ServerList, raw: &str, raw_name: &str, opts: &Value) -> bool {
    let url = norm(raw).unwrap_or_default();
    let name = clean_name(raw_name);
    let o = rename_opts(opts);
    let Some(e) = l.history.iter_mut().find(|e| url_of(e) == url) else { return false };
    if name.is_empty() { return false }
    let before = e.clone();
    match o.get("custom").and_then(Value::as_bool) {
        None => { if flag(e, "customName") { return false } e.insert("name".into(), json!(name)); }
        Some(custom) => {
            e.insert("name".into(), json!(name));
            let _ = if custom { e.insert("customName".into(), json!(true)) } else { e.remove("customName") };
            e.insert("editedAt".into(), o.get("editedAt").cloned().unwrap_or_else(|| json!(now_ms())));
            if let Some(icon) = o.get("icon") {
                let s = icon.as_str().map(str::trim).unwrap_or("");
                let ok = !s.is_empty() && s.len() <= ICON_MAX && { let lower = s.to_ascii_lowercase(); lower.starts_with("http://") || lower.starts_with("https://") };
                let _ = if ok { e.insert("icon".into(), json!(s)); e.insert("customIcon".into(), json!(true)) } else { e.remove("icon"); e.remove("customIcon") };
            }
        }
    }
    *e != before
}
pub fn ordered_urls(l: &ServerList) -> Vec<String> {
    let known: HashSet<&str> = l.history.iter().map(url_of).collect();
    let mut out: Vec<String> = l.order.iter().filter(|u| known.contains(u.as_str())).cloned().collect();
    let placed: HashSet<String> = out.iter().cloned().collect();
    out.extend(l.history.iter().map(url_of).filter(|u| !placed.contains(*u)).map(str::to_string));
    out
}
pub fn set_order(l: &mut ServerList, raw: &Value) -> bool {
    let listed: HashSet<&str> = l.history.iter().map(url_of).collect();
    let want: Vec<String> = clean_urls(raw).into_iter().filter(|u| listed.contains(u.as_str())).collect();
    let mut seen = HashSet::new();
    let base: Vec<String> = l.order.iter().cloned().chain(ordered_urls(l)).filter(|u| seen.insert(u.clone())).collect();
    let in_base: Vec<String> = want.into_iter().filter(|u| seen.contains(u)).collect();
    let wanted: HashSet<&String> = in_base.iter().collect();
    let mut next = in_base.iter();
    let out: Vec<String> = base.iter().map(|u| if wanted.contains(u) { next.next().cloned().unwrap_or_else(|| u.clone()) } else { u.clone() }).take(HISTORY_CAP * 2).collect();
    let changed = out != l.order;
    l.order = out;
    changed
}
pub fn view(l: &ServerList) -> Value {
    let order = ordered_urls(l);
    let servers: Vec<Value> = order.iter().filter_map(|u| l.history.iter().find(|e| url_of(e) == u)).map(|e| Value::Object(e.clone())).collect();
    json!({ "servers": servers, "removed": l.removed, "order": order, "hasOrder": !l.order.is_empty() })
}
fn invisible(c: char) -> bool { c.is_control() || matches!(c, '\u{ad}' | '\u{61c}' | '\u{180e}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206f}' | '\u{feff}' | '\u{fff9}'..='\u{fffb}') }
pub fn safe_text(v: &str, max: usize) -> String {
    let t = v.chars().map(|c| if invisible(c) { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > max { t.chars().take(max - 3).collect::<String>() + "..." } else { t }
}
fn host_of(url: &str) -> String { url::Url::parse(url).ok().and_then(|u| u.host_str().map(|h| u.port().map(|p| format!("{h}:{p}")).unwrap_or_else(|| h.to_string()))).unwrap_or_else(|| url.to_string()) }
pub fn display_name(name: &str, url: &str) -> String {
    let t = name.trim();
    safe_text(&if t.is_empty() || t == url || norm(t).as_deref() == Some(url) { host_of(url) } else { name.to_string() }, TEXT_MAX)
}
fn is_web_address(raw: &str) -> bool { let r = raw.trim(); let lower = r.to_ascii_lowercase(); (lower.starts_with("http://") || lower.starts_with("https://")) && url::Url::parse(r).ok().and_then(|u| u.host_str().map(|h| !h.is_empty())).unwrap_or(false) }
fn ask(p: Prompt) -> Plan { Plan::Ask(json!([p.kind, p.url, p.name, p.icon, p.icon_change]).to_string(), p) }
fn prompt(kind: &'static str, url: &str, name: String) -> Prompt { Prompt { kind, url: url.to_string(), name, old_name: String::new(), icon: String::new(), icon_change: "" } }
pub fn plan(l: &ServerList, r: &Request, requester: &str, open_urls: &[String]) -> Plan {
    let url = norm(&r.url).unwrap_or_default();
    let valid = !url.is_empty() && valid_host(&url);
    let entry = valid.then(|| l.history.iter().find(|e| url_of(e) == url)).flatten();
    match r.kind.as_str() {
        "add" if !valid => Plan::Ignore(json!("invalid")),
        "add" if entry.is_some() => Plan::Ignore(json!("exists")),
        "add" if r.opts.get("userInitiated").and_then(Value::as_bool) != Some(true) => Plan::Ignore(json!("refused")),
        "add" => ask(prompt(if l.removed.contains(&url) { "addBack" } else { "add" }, &url, display_name(&r.name, &url))),
        "remove" => match entry { Some(e) if r.user => ask(prompt("remove", &url, display_name(name_of(e), &url))), _ => Plan::Ignore(Value::Null) },
        "rename" => {
            let Some(e) = entry else { return Plan::Ignore(json!(false)) };
            if rename_opts(&r.opts).get("custom").is_none() { return if !requester.is_empty() && url == requester { Plan::Apply } else { Plan::Ignore(json!(false)) } }
            if !r.user { return Plan::Ignore(json!(false)) }
            let mut trial = ServerList { history: vec![e.clone()], ..Default::default() };
            if !update_name(&mut trial, &url, &r.name, &r.opts) { return Plan::Ignore(json!(false)) }
            let after = &trial.history[0];
            let icon = |x: &Entry| x.get("icon").and_then(Value::as_str).unwrap_or("").to_string();
            let (name_changed, icon_changed) = (name_of(after) != name_of(e), icon(after) != icon(e));
            if !name_changed && !icon_changed { return Plan::Ignore(json!(false)) }
            ask(Prompt { kind: "rename", url: url.clone(), name: display_name(name_of(after), &url), old_name: display_name(name_of(e), &url), icon: if icon_changed { icon(after) } else { String::new() }, icon_change: if !icon_changed { "" } else if icon(after).is_empty() { "removed" } else { "set" } })
        }
        "open" if !is_web_address(&r.url) => Plan::Ignore(json!(false)),
        "open" if l.history.iter().any(|e| url_of(e) == url) || open_urls.contains(&url) => Plan::Apply,
        "open" => ask(prompt("open", &url, String::new())),
        _ => Plan::Ignore(Value::Null),
    }
}
pub fn apply(l: &mut ServerList, r: &Request, now: u64) -> Value {
    match r.kind.as_str() {
        "add" => json!(add(l, &r.url, &r.name, r.opts.get("userInitiated").and_then(Value::as_bool) == Some(true), now)),
        "remove" => json!(remove(l, &r.url)),
        "rename" => json!(update_name(l, &r.url, &r.name, &r.opts)),
        _ => Value::Null,
    }
}
pub fn prompt_text(app: &AppHandle, p: &Prompt, requester_name: &str, requester_url: &str) -> PromptText {
    let by = display_name(requester_name, requester_url);
    let url = safe_text(&p.url, ICON_TEXT_MAX);
    let tv = |k: &str, v: &[(&str, &str)]| state::tv(app, k, v);
    let (message, confirm) = match p.kind {
        "add" | "addBack" => (tv(&format!("serverList.gate.{}", p.kind), &[("name", &p.name), ("url", &url)]), tv("serverList.gate.confirmAdd", &[])),
        "remove" => (tv("serverList.gate.remove", &[("name", &p.name), ("url", &url)]), tv("serverList.gate.confirmRemove", &[])),
        "rename" if p.name != p.old_name => (tv("serverList.gate.rename", &[("oldName", &p.old_name), ("name", &p.name), ("url", &url)]), tv("serverList.gate.confirmChange", &[])),
        "rename" => (tv("serverList.gate.changeIcon", &[("name", &p.name), ("url", &url)]), tv("serverList.gate.confirmChange", &[])),
        _ => (tv("serverList.gate.open", &[("url", &url)]), tv("serverList.gate.confirmOpen", &[])),
    };
    let icon = match p.icon_change { "set" => vec![tv("serverList.gate.iconSet", &[("icon", &safe_text(&p.icon, ICON_TEXT_MAX))])], "removed" => vec![tv("serverList.gate.iconRemoved", &[])], _ => vec![] };
    let detail = icon.into_iter().chain([tv("serverList.gate.askedBy", &[("name", &by), ("url", &safe_text(requester_url, ICON_TEXT_MAX))]), tv("serverList.gate.explain", &[])]).collect::<Vec<_>>().join("\n\n");
    PromptText { title: tv("serverList.gate.title", &[]), message, detail, confirm, cancel: tv("dialog.cancel", &[]), block: tv("serverList.gate.block", &[("name", &by)]) }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn list(urls: &[&str]) -> ServerList { from_values(&json!(urls.iter().map(|u| json!({ "url": u, "name": u })).collect::<Vec<_>>()), &json!([]), &json!([])) }
    fn req(kind: &str, url: &str, name: &str, opts: Value, user: bool) -> Request { Request { kind: kind.into(), url: url.into(), name: name.into(), opts, user } }
    #[test]
    fn damaged_lists_are_read() {
        let l = from_values(&json!({ "x": 1 }), &json!("bad"), &json!([3, null]));
        assert!(l.history.is_empty() && l.removed.is_empty() && l.order.is_empty());
        let l = from_values(&json!([null, 5, { "url": 7 }, { "url": "https://a.example.com/app", "name": 3, "lastConnected": "x", "customName": true }, { "url": "https://https" }]), &json!([]), &json!([]));
        assert_eq!(l.history.len(), 1);
        assert_eq!(url_of(&l.history[0]), "https://a.example.com");
        assert_eq!(name_of(&l.history[0]), "https://a.example.com");
        assert!(flag(&l.history[0], "customName"));
    }
    #[test]
    fn hosts() {
        assert!(valid_host("http://nas:3000") && valid_host("https://10.0.0.2") && valid_host("http://localhost") && valid_host("https://[::1]:3000"));
        assert!(!valid_host("https://https") && !valid_host("https://nas") && !valid_host("https://https:443"));
    }
    #[test]
    fn add_remove_and_restore() {
        let mut l = list(&["https://a.example.com"]);
        assert_eq!(add(&mut l, "https://b.example.com", "Bee", true, 5), "added");
        assert!(flag(&l.history[1], "customName"));
        assert!(remove(&mut l, "https://b.example.com"));
        assert_eq!(add(&mut l, "https://b.example.com", "", false, 5), "refused");
        mark_connected(&mut l, "https://b.example.com", 9);
        assert!(l.removed.is_empty() && l.history.len() == 2);
        assert_eq!(add(&mut l, "https://a.example.com", "Ay", false, 5), "exists");
        assert_eq!(name_of(&l.history[0]), "Ay");
    }
    #[test]
    fn cap_drops_least_recent() {
        let mut l = ServerList::default();
        for i in 0..HISTORY_CAP { mark_connected(&mut l, &format!("https://s{i}.example.com"), 100 + i as u64); }
        mark_connected(&mut l, "https://s0.example.com", 1000);
        mark_connected(&mut l, "https://new.example.com", 1);
        assert_eq!(l.history.len(), HISTORY_CAP);
        assert!(l.history.iter().any(|e| url_of(e) == "https://new.example.com") && l.history.iter().any(|e| url_of(e) == "https://s0.example.com"));
        assert!(!l.history.iter().any(|e| url_of(e) == "https://s1.example.com"));
    }
    #[test]
    fn user_names_win_and_order_only_moves() {
        let mut l = list(&["https://a.example.com", "https://b.example.com", "https://c.example.com"]);
        assert!(update_name(&mut l, "https://a.example.com", "Mine", &json!({ "custom": true, "editedAt": 7, "icon": "https://a.example.com/i.png" })));
        assert!(!update_name(&mut l, "https://a.example.com", "Server", &json!({})));
        assert_eq!(name_of(&l.history[0]), "Mine");
        assert!(set_order(&mut l, &json!(["https://c.example.com", "https://z.example.com", "https://a.example.com"])));
        assert_eq!(ordered_urls(&l), vec!["https://c.example.com", "https://b.example.com", "https://a.example.com"]);
        assert_eq!(view(&l)["servers"].as_array().unwrap().len(), 3);
    }
    #[test]
    fn page_requests() {
        let l = list(&["https://a.example.com", "https://b.example.com"]);
        let me = "https://a.example.com";
        assert_eq!(plan(&l, &req("add", "https://c.example.com", "C", json!({}), false), me, &[]), Plan::Ignore(json!("refused")));
        assert!(matches!(plan(&l, &req("add", "https://c.example.com", "C", json!({ "userInitiated": true }), true), me, &[]), Plan::Ask(_, Prompt { kind: "add", .. })));
        assert_eq!(plan(&l, &req("remove", "https://b.example.com", "", json!({}), false), me, &[]), Plan::Ignore(Value::Null));
        assert!(matches!(plan(&l, &req("remove", "https://b.example.com", "", json!({}), true), me, &[]), Plan::Ask(..)));
        assert_eq!(plan(&l, &req("rename", "https://a.example.com", "Own", json!({}), false), me, &[]), Plan::Apply);
        assert_eq!(plan(&l, &req("rename", "https://b.example.com", "Theirs", json!({}), false), me, &[]), Plan::Ignore(json!(false)));
        assert!(matches!(plan(&l, &req("rename", "https://b.example.com", "Bee", json!({ "custom": true }), true), me, &[]), Plan::Ask(..)));
        assert_eq!(plan(&l, &req("open", "https://b.example.com", "", json!({}), true), me, &[]), Plan::Apply);
        assert_eq!(plan(&l, &req("open", "http://nas:3000", "", json!({}), true), me, &["http://nas:3000".into()]), Plan::Apply);
        assert!(matches!(plan(&l, &req("open", "http://nas", "", json!({}), true), me, &[]), Plan::Ask(..)));
        assert_eq!(plan(&l, &req("open", "javascript:alert(1)", "", json!({}), true), me, &[]), Plan::Ignore(json!(false)));
    }
    #[test]
    fn dialog_text_is_safe() {
        assert_eq!(safe_text("a\nb\u{202e}c  d", 80), "a b c d");
        assert_eq!(safe_text(&"x".repeat(100), 10), "xxxxxxx...");
        assert_eq!(display_name("https://a.example.com:3000", "https://a.example.com:3000"), "a.example.com:3000");
    }
}
