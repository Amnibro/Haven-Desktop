use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_store::StoreExt;
type Cont = Box<dyn FnOnce(bool) + Send>;
static WAITING: Lazy<Mutex<HashMap<String, Vec<Cont>>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static REFUSED: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));
const STORE: &str = "haven-desktop.json";
const KEY: &str = "certPins";
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
    let (host, fp) = (err.host.clone(), fingerprint(&err.der));
    if err.der.is_empty() || REFUSED.lock().contains(&host) {
        return cont(false);
    }
    let pinned = pins(app).get(&host).and_then(|v| v.as_str()).map(str::to_string);
    if pinned.as_deref() == Some(fp.as_str()) {
        return cont(true);
    }
    {
        let mut w = WAITING.lock();
        let first = !w.contains_key(&host);
        w.entry(host.clone()).or_default().push(cont);
        if !first {
            return;
        }
    }
    let shown = host.trim_end_matches(":443");
    let t = |k: &str| crate::state::t(app, k).replace("{host}", shown);
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
