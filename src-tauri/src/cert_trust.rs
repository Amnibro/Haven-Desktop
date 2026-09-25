use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_store::StoreExt;
type Cont = Box<dyn FnOnce(bool) + Send>;
static WAITING: Lazy<Mutex<HashMap<String, Vec<Cont>>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static REFUSED: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));
const STORE: &str = "haven-desktop.json";
const KEY: &str = "trustedCertificates";
const GRANDFATHERED: &str = "certTrustGrandfathered";
fn normalize_host(h: &str) -> String {
    let h = h.trim().to_lowercase();
    let h = h.strip_prefix('[').and_then(|r| r.split_once(']')).map(|(a, _)| a.to_string()).unwrap_or_else(|| if h.matches(':').count() == 1 { h.split(':').next().unwrap_or_default().to_string() } else { h.clone() });
    h.trim_end_matches('.').to_string()
}
fn is_local_host(h: &str) -> bool {
    let h = normalize_host(h);
    h == "localhost" || h.ends_with(".localhost") || h.ends_with(".local") || match h.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v)) => v.is_loopback() || v.is_private() || v.is_link_local(),
        Ok(std::net::IpAddr::V6(v)) => v.is_loopback() || v.to_ipv4_mapped().is_some_and(|m| is_local_host(&m.to_string())) || (v.segments()[0] & 0xffc0) == 0xfe80 || (v.segments()[0] & 0xfe00) == 0xfc00,
        Err(_) => false,
    }
}
fn url_host(u: &str) -> Option<String> {
    url::Url::parse(u).ok().and_then(|u| u.host_str().map(normalize_host))
}
fn known_server_urls(app: &AppHandle) -> Vec<String> {
    let st = app.store(STORE).ok();
    let get = |k: &str| st.as_ref().and_then(|s| s.get(k)).unwrap_or_default();
    let mut urls: Vec<String> = get("serverHistory").as_array().into_iter().flatten().filter_map(|e| e.get("url").and_then(Value::as_str).map(String::from)).collect();
    urls.extend(get("userPrefs").get("serverUrl").and_then(Value::as_str).map(String::from));
    urls.extend(app.try_state::<crate::state::AppState>().and_then(|s| s.active_server_url.try_lock().and_then(|u| u.clone())));
    urls
}
pub fn migrate(app: &AppHandle) {
    let Ok(s) = app.store(STORE) else { return };
    if s.get("certTrustMigrated").and_then(|v| v.as_bool()).unwrap_or(false) {
        return;
    }
    let hosts: HashSet<String> = known_server_urls(app).iter().filter_map(|u| url_host(u)).filter(|h| !is_local_host(h)).collect();
    s.set(GRANDFATHERED, json!(hosts.into_iter().collect::<Vec<_>>()));
    s.set("certTrustMigrated", json!(true));
    let _ = s.save();
}
fn fingerprint(der: &[u8]) -> String {
    Sha256::digest(der).iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}
fn pins(app: &AppHandle) -> serde_json::Map<String, Value> {
    app.store(STORE).ok().and_then(|s| s.get(KEY)).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}
fn pin(app: &AppHandle, host: &str, fp: &str) {
    if let Ok(s) = app.store(STORE) {
        let mut p = pins(app);
        p.insert(host.to_string(), json!(fp));
        s.set(KEY, Value::Object(p));
        let waiting: Vec<Value> = s.get(GRANDFATHERED).and_then(|v| v.as_array().cloned()).unwrap_or_default().into_iter().filter(|h| h.as_str() != Some(host)).collect();
        s.set(GRANDFATHERED, Value::Array(waiting));
        let _ = s.save();
    }
}
fn settle(host: &str, ok: bool) {
    if !ok {
        REFUSED.lock().insert(host.to_string());
    }
    for c in WAITING.lock().remove(host).unwrap_or_default() {
        c(ok);
    }
}
pub fn decide(app: &AppHandle, err: tauri_runtime_cef::CertificateError, cont: Cont) {
    let (host, fp) = (normalize_host(&err.host), fingerprint(&err.der));
    if is_local_host(&host) {
        return cont(true);
    }
    if err.der.is_empty() || REFUSED.lock().contains(&host) {
        return cont(false);
    }
    let pinned = pins(app).get(&host).and_then(|v| v.as_str()).map(str::to_string);
    if pinned.as_deref() == Some(fp.as_str()) {
        return cont(true);
    }
    let grandfathered = app.store(STORE).ok().and_then(|s| s.get(GRANDFATHERED)).and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().any(|h| h.as_str() == Some(host.as_str()));
    if pinned.is_none() && grandfathered {
        pin(app, &host, &fp);
        return cont(true);
    }
    if pinned.is_none() && !known_server_urls(app).iter().any(|u| url_host(u).as_deref() == Some(host.as_str())) {
        return cont(false);
    }
    {
        let mut w = WAITING.lock();
        let first = !w.contains_key(&host);
        w.entry(host.clone()).or_default().push(cont);
        if !first {
            return;
        }
    }
    let t = |k: &str| crate::state::t(app, k).replace("{host}", &host);
    let (title, body, yes) = match pinned {
        Some(old) => (t("cert.title"), format!("{}\n\n{}\n\n{}\n{}", t("cert.changedMessage"), t("cert.changedDetail"), t("cert.previousFingerprint").replace("{value}", &old), t("cert.fingerprint").replace("{value}", &fp)), t("cert.trustNew")),
        None => (t("cert.title"), format!("{}\n\n{}\n\n{}", t("cert.unknownMessage"), t("cert.unknownDetail"), t("cert.fingerprint").replace("{value}", &fp)), t("cert.trust")),
    };
    let (app2, host2) = (app.clone(), host.clone());
    app.dialog().message(body).title(title).kind(MessageDialogKind::Warning).buttons(MessageDialogButtons::OkCancelCustom(yes, crate::state::t(app, "dialog.cancel"))).show(move |ok| {
        if ok {
            pin(&app2, &host2, &fp);
        }
        settle(&host2, ok);
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hosts() {
        assert_eq!(normalize_host("Example.COM.:8443"), "example.com");
        assert_eq!(normalize_host("[::1]:443"), "::1");
        for h in ["127.0.0.1:3099", "localhost", "10.1.2.3", "172.16.0.9", "192.168.1.5:443", "169.254.1.1", "nas.local", "[::1]:443", "[fe80::1]:443", "[fd00::5]:443", "[::ffff:192.168.0.2]:443"] {
            assert!(is_local_host(h), "{h}");
        }
        for h in ["8.8.8.8", "172.32.0.1", "haven.example.com:443", "[2001:db8::1]:443", "local.example.com"] {
            assert!(!is_local_host(h), "{h}");
        }
    }
}
