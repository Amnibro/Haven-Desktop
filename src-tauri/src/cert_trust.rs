use crate::state::{self, AppState};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
#[derive(Debug, PartialEq)]
pub enum Verdict { Local, Pinned, Changed, Grandfathered, Unknown }
pub fn normalize_host(h: &str) -> String { h.trim().to_lowercase().trim_start_matches('[').trim_end_matches(']').trim_end_matches('.').to_string() }
pub fn is_local_host(raw: &str) -> bool {
    let h = normalize_host(raw);
    let v4 = |s: &str| s.parse::<std::net::Ipv4Addr>().ok().map(|ip| { let [a, b, ..] = ip.octets(); a == 127 || a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168) || (a == 169 && b == 254) });
    !h.is_empty() && (h == "localhost" || h.ends_with(".localhost") || h.ends_with(".local") || h == "::1" || v4(&h).or_else(|| h.strip_prefix("::ffff:").and_then(v4)).unwrap_or_else(|| h.parse::<std::net::Ipv6Addr>().map(|ip| { let s = ip.segments()[0]; (s & 0xffc0) == 0xfe80 || (s & 0xfe00) == 0xfc00 }).unwrap_or(false)))
}
pub fn decision(host: &str, fingerprint: &str, pins: &Value, grandfathered: &Value) -> Verdict {
    let host = normalize_host(host);
    match pins.get(&host).and_then(Value::as_str) {
        _ if is_local_host(&host) => Verdict::Local,
        Some(pin) if pin == fingerprint => Verdict::Pinned,
        Some(_) => Verdict::Changed,
        None if grandfathered.as_array().map(|g| g.iter().any(|x| x.as_str() == Some(host.as_str()))).unwrap_or(false) => Verdict::Grandfathered,
        None => Verdict::Unknown,
    }
}
fn host_of(url: &str) -> Option<String> { url::Url::parse(url).ok().and_then(|u| u.host_str().map(normalize_host)) }
pub fn migrate(app: &AppHandle) {
    if state::get_bool(app, "certTrustMigrated").unwrap_or(false) { return }
    let prefs = state::get_user_prefs(app).unwrap_or(Value::Null);
    let mut hosts: Vec<String> = crate::server_list::read(app).history.iter().map(|e| crate::server_list::url_of(e).to_string()).chain(prefs.get("serverUrl").and_then(Value::as_str).map(str::to_string)).filter_map(|u| host_of(&u)).filter(|h| !h.is_empty() && !is_local_host(h)).collect();
    hosts.sort();
    hosts.dedup();
    let _ = state::store(app).map(|s| { s.set("certTrustGrandfathered", json!(hosts)); s.set("certTrustMigrated", json!(true)); s.save() });
}
fn remember(app: &AppHandle, host: &str, fingerprint: &str) {
    let mut pins = state::get_value(app, "trustedCertificates").ok().and_then(|v| v.as_object().cloned()).unwrap_or_default();
    pins.insert(host.to_string(), json!(fingerprint));
    let waiting: Vec<Value> = state::get_value(app, "certTrustGrandfathered").ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().into_iter().filter(|h| h.as_str() != Some(host)).collect();
    let _ = state::store(app).map(|s| { s.set("trustedCertificates", Value::Object(pins)); s.set("certTrustGrandfathered", json!(waiting)); s.save() });
}
fn verdict_for(app: &AppHandle, host: &str, fingerprint: &str) -> Verdict {
    let get = |k: &str| state::get_value(app, k).unwrap_or(Value::Null);
    decision(host, fingerprint, &get("trustedCertificates"), &get("certTrustGrandfathered"))
}
pub fn question_pending(app: &AppHandle) -> bool { !app.state::<AppState>().cert_questions.lock().is_empty() }
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use gtk::gio::prelude::*;
    use gtk::glib;
    use webkit2gtk::{WebContextExt, WebViewExt, WebsiteDataManagerExt};
    fn fingerprint(cert: &gtk::gio::TlsCertificate) -> String {
        let der = cert.property::<Option<glib::ByteArray>>("certificate");
        let hex = der.and_then(|d| glib::compute_checksum_for_data(glib::ChecksumType::Sha256, &d)).map(|s| s.to_uppercase()).unwrap_or_default();
        hex.as_bytes().chunks(2).map(|c| String::from_utf8_lossy(c).to_string()).collect::<Vec<_>>().join(":")
    }
    fn about(cert: &gtk::gio::TlsCertificate) -> (Option<String>, Option<String>) {
        let issuer = cert.find_property("issuer-name").and_then(|_| cert.property::<Option<String>>("issuer-name"));
        let expires = cert.find_property("not-valid-after").and_then(|_| cert.property::<Option<glib::DateTime>>("not-valid-after")).and_then(|d| d.format("%x").ok()).map(|s| s.to_string());
        (issuer, expires)
    }
    fn trusted(app: &AppHandle, host: &str, fp: &str) -> Option<bool> {
        match verdict_for(app, host, fp) { Verdict::Local | Verdict::Pinned => Some(true), Verdict::Grandfathered => { remember(app, host, fp); Some(true) } Verdict::Changed => Some(false), Verdict::Unknown => None }
    }
    pub fn attach(view: &webkit2gtk::WebView, app: AppHandle, label: String) {
        let _ = view.website_data_manager().map(|m| m.set_tls_errors_policy(webkit2gtk::TLSErrorsPolicy::Fail));
        view.connect_load_failed_with_tls_errors(move |view, uri, cert, _| {
            let Some(host) = host_of(uri) else { return false };
            let fp = fingerprint(cert);
            let verdict = verdict_for(&app, &host, &fp);
            if matches!(verdict, Verdict::Local | Verdict::Pinned | Verdict::Grandfathered) {
                let _ = (verdict == Verdict::Grandfathered).then(|| remember(&app, &host, &fp));
                let _ = view.web_context().map(|c| c.allow_tls_certificate_for_host(cert, &host));
                view.load_uri(uri);
                return true;
            }
            let key = format!("{host}|{fp}");
            if !app.state::<AppState>().cert_questions.lock().insert(key.clone()) { return true }
            let changed = verdict == Verdict::Changed;
            let previous = state::get_value(&app, "trustedCertificates").ok().and_then(|p| p.get(&host).and_then(Value::as_str).map(str::to_string));
            let (issuer, expires) = about(cert);
            let tv = |k: &str, v: &str| state::tv(&app, k, &[("value", v), ("host", v)]);
            let lines: Vec<String> = [Some(state::t(&app, if changed { "cert.changedDetail" } else { "cert.unknownDetail" })), Some(String::new()), Some(tv("cert.fingerprint", &fp)), previous.filter(|_| changed).map(|p| tv("cert.previousFingerprint", &p)), issuer.map(|i| tv("cert.issuer", &i)), expires.map(|e| tv("cert.expires", &e))].into_iter().flatten().collect();
            let message = format!("{}\n\n{}", tv(if changed { "cert.changedMessage" } else { "cert.unknownMessage" }, &host), lines.join("\n"));
            let (app2, uri, label) = (app.clone(), uri.to_string(), label.clone());
            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
            let dialog = app.dialog().message(message).title(state::t(&app, "cert.title")).kind(MessageDialogKind::Warning).buttons(MessageDialogButtons::OkCancelCustom(state::t(&app, if changed { "cert.trustNew" } else { "cert.trust" }), state::t(&app, "dialog.cancel")));
            let dialog = match app.get_webview_window(&label) { Some(w) => dialog.parent(&w), None => dialog };
            dialog.show(move |yes| {
                let _ = yes.then(|| remember(&app2, &host, &fp));
                app2.state::<AppState>().cert_questions.lock().remove(&key);
                let target = app2.get_webview_window(&label);
                match (yes, target, label.as_str()) {
                    (true, Some(w), _) => { let _ = url::Url::parse(&uri).map(|u| w.navigate(u)); }
                    (false, Some(_), "main") => crate::nav_fail::show_connection_error(&app2, &app2.state::<AppState>().active_server_url.lock().clone().unwrap_or(uri)),
                    _ => {}
                }
            });
            true
        });
    }
    pub fn prewarm(view: &webkit2gtk::WebView, app: &AppHandle) {
        let Some(ctx) = view.web_context() else { return };
        let targets: Vec<(String, u16)> = crate::server_list::read(app).history.iter().filter_map(|e| url::Url::parse(crate::server_list::url_of(e)).ok()).filter(|u| u.scheme() == "https").filter_map(|u| Some((normalize_host(u.host_str()?), u.port_or_known_default()?))).filter(|(h, _)| is_local_host(h) || verdict_for(app, h, "").ne(&Verdict::Unknown)).collect();
        for (host, port) in targets {
            let client = gtk::gio::SocketClient::new();
            client.set_tls(true);
            let (ctx, app, h) = (ctx.clone(), app.clone(), host.clone());
            client.connect_event(move |_, ev, _, conn| {
                let Some(tls) = conn.filter(|_| ev == gtk::gio::SocketClientEvent::TlsHandshaking).and_then(|c| c.downcast_ref::<gtk::gio::TlsClientConnection>()) else { return };
                let (ctx, app, h) = (ctx.clone(), app.clone(), h.clone());
                tls.connect_accept_certificate(move |_, cert, _| { let ok = trusted(&app, &h, &fingerprint(cert)) == Some(true); let _ = ok.then(|| ctx.allow_tls_certificate_for_host(cert, &h)); ok });
            });
            let addr = if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") };
            client.connect_to_host_async(&addr, port, None::<&gtk::gio::Cancellable>, |c| { let _ = c.map(|c| c.close(None::<&gtk::gio::Cancellable>)); });
        }
    }
}
#[cfg(target_os = "linux")]
pub use linux::{attach, prewarm};
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_hosts() {
        for h in ["localhost", "a.localhost", "nas.local", "127.0.0.1", "10.1.2.3", "172.16.0.1", "172.31.9.9", "192.168.1.5", "169.254.1.1", "[::1]", "::ffff:192.168.0.2", "fe80::1", "fd12::1"] { assert!(is_local_host(h), "{h}") }
        for h in ["example.com", "8.8.8.8", "172.32.0.1", "2001:db8::1", ""] { assert!(!is_local_host(h), "{h}") }
    }
    #[test]
    fn verdicts() {
        let pins = json!({ "a.example.com": "AA" });
        let old = json!(["b.example.com"]);
        assert_eq!(decision("192.168.1.2", "x", &pins, &old), Verdict::Local);
        assert_eq!(decision("A.example.com.", "AA", &pins, &old), Verdict::Pinned);
        assert_eq!(decision("a.example.com", "BB", &pins, &old), Verdict::Changed);
        assert_eq!(decision("b.example.com", "x", &pins, &old), Verdict::Grandfathered);
        assert_eq!(decision("c.example.com", "x", &pins, &old), Verdict::Unknown);
    }
}
