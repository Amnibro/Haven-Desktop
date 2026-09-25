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
fn reason(code: &str) -> &'static str {
    match code.trim_start_matches("Errorcode(").trim_end_matches(')') {
        "ERR_CERT_AUTHORITY_INVALID" => "it is self-signed or from an unknown issuer",
        "ERR_CERT_COMMON_NAME_INVALID" => "it was issued for a different name",
        "ERR_CERT_DATE_INVALID" => "it has expired or is not valid yet",
        "ERR_CERT_REVOKED" => "it has been revoked",
        "ERR_CERT_WEAK_KEY" | "ERR_CERT_WEAK_SIGNATURE_ALGORITHM" => "it uses weak cryptography",
        _ => "it did not pass verification",
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
    let (title, body, yes) = match pinned {
        Some(old) => ("Certificate changed", format!("The certificate for {shown} is not the one you trusted before.\n\nThis happens when the server owner replaces it, but it can also mean someone is intercepting the connection. Only continue if you expected the change.\n\nTrusted before:\n{old}\n\nNow:\n{fp}"), "Trust the new certificate"),
        None => ("Untrusted certificate", format!("{shown} uses a certificate your computer does not trust: {}. Servers you host yourself usually do this.\n\nOnly trust it if you know this server. Its SHA-256 fingerprint:\n{fp}\n\nHaven remembers your answer and warns you if the certificate ever changes.", reason(&err.error)), "Trust this server"),
    };
    let (app2, host2) = (app.clone(), host.clone());
    app.dialog().message(body).title(title).kind(MessageDialogKind::Warning).buttons(MessageDialogButtons::OkCancelCustom(yes.into(), "Cancel".into())).show(move |ok| {
        if ok {
            pin(&app2, &host2, &fp);
        }
        settle(&host2, ok);
    });
}
