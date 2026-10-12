use crate::nav_fail::Problem;
use crate::state::{self, AppState};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::net::ToSocketAddrs;
use std::time::Duration;
use tauri::{AppHandle, Manager};
#[derive(Debug, PartialEq)]
pub enum Verdict { Local, Pinned, Changed, Grandfathered, Unknown }
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Kind { Retry, Foreign, Captive, Trust }
const BUNDLE: &str = include_str!("../certs/tls_intermediates.pem");
static POOL: Lazy<RwLock<Vec<Vec<u8>>>> = Lazy::new(|| RwLock::new(ders(BUNDLE)));
static AIA_DIR: Lazy<RwLock<Option<std::path::PathBuf>>> = Lazy::new(|| RwLock::new(None));
pub fn normalize_host(h: &str) -> String { h.trim().to_lowercase().trim_start_matches('[').trim_end_matches(']').trim_end_matches('.').to_string() }
pub fn is_local_host(raw: &str) -> bool {
    let h = normalize_host(raw);
    let v4 = |s: &str| s.parse::<std::net::Ipv4Addr>().ok().map(|ip| { let [a, b, ..] = ip.octets(); a == 127 || a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168) || (a == 169 && b == 254) });
    !h.is_empty() && (h == "localhost" || h.ends_with(".localhost") || h.ends_with(".local") || h == "::1" || v4(&h).or_else(|| h.strip_prefix("::ffff:").and_then(v4)).unwrap_or_else(|| h.parse::<std::net::Ipv6Addr>().map(|ip| { let s = ip.segments()[0]; (s & 0xffc0) == 0xfe80 || (s & 0xfe00) == 0xfc00 }).unwrap_or(false)))
}
pub fn public_name(raw: &str) -> bool {
    let h = normalize_host(raw);
    h.contains('.') && !h.contains(':') && h.parse::<std::net::Ipv4Addr>().is_err() && ![".local", ".lan", ".home", ".home.arpa", ".internal", ".localdomain", ".localhost", ".intranet", ".corp", ".private"].iter().any(|s| h.ends_with(s))
}
pub fn pins_of(v: Option<&Value>) -> Vec<String> {
    match v { Some(Value::String(s)) => vec![s.clone()], Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(str::to_string).collect(), _ => vec![] }
}
pub fn decision(host: &str, presented: &[&str], pins: &Value, grandfathered: &Value) -> Verdict {
    let host = normalize_host(host);
    let have = pins_of(pins.get(&host));
    match () {
        _ if is_local_host(&host) => Verdict::Local,
        _ if have.iter().any(|p| !p.is_empty() && presented.contains(&p.as_str())) => Verdict::Pinned,
        _ if !have.is_empty() => Verdict::Changed,
        _ if grandfathered.as_array().map(|g| g.iter().any(|x| x.as_str() == Some(host.as_str()))).unwrap_or(false) => Verdict::Grandfathered,
        _ => Verdict::Unknown,
    }
}
pub fn kind(chosen: bool, trusted: bool, captive: bool, host: &str) -> Kind {
    match () { _ if trusted => Kind::Retry, _ if !chosen => if captive { Kind::Captive } else { Kind::Foreign }, _ if captive && public_name(host) => Kind::Captive, _ => Kind::Trust }
}
fn tlv(d: &[u8], i: usize) -> Option<(u8, usize, usize)> {
    let (t, l0) = (*d.get(i)?, *d.get(i + 1)? as usize);
    let (n, len) = if l0 < 0x80 { (0, l0) } else { let n = l0 & 0x7f; (n, (0..n).try_fold(0usize, |a, k| d.get(i + 2 + k).map(|b| a << 8 | *b as usize))?) };
    let s = i + 2 + n;
    (l0 != 0x80 && n <= 4 && s.checked_add(len)? <= d.len()).then_some((t, s, s + len))
}
fn tbs(der: &[u8]) -> Option<Vec<&[u8]>> {
    let (_, cs, _) = tlv(der, 0)?;
    let (_, ts, te) = tlv(der, cs)?;
    let (mut f, mut i) = (Vec::new(), ts);
    while i < te && f.len() < 7 { let (t, _, e) = tlv(der, i)?; f.push((t, i, e)); i = e }
    let f = if f.first()?.0 == 0xa0 { &f[1..] } else { &f[..] };
    Some(f.iter().map(|&(_, s, e)| &der[s..e]).collect())
}
pub fn cert_parts(der: &[u8]) -> Option<(&[u8], &[u8], &[u8])> { let f = tbs(der)?; Some((f.get(2)?, f.get(4)?, f.get(5)?)) }
fn kids(d: &[u8]) -> Vec<&[u8]> {
    let Some((_, s, e)) = tlv(d, 0) else { return vec![] };
    std::iter::successors(Some(s), |&i| tlv(d, i).map(|(_, _, end)| end)).take_while(|&i| i < e).filter_map(|i| tlv(d, i).map(|(_, _, end)| &d[i..end])).collect()
}
fn body(d: &[u8]) -> Option<(u8, &[u8])> { tlv(d, 0).map(|(t, s, e)| (t, &d[s..e])) }
fn oid_name(o: &[u8]) -> String {
    match o {
        [0x55, 4, 3] => "CN".into(), [0x55, 4, 6] => "C".into(), [0x55, 4, 7] => "L".into(), [0x55, 4, 8] => "ST".into(), [0x55, 4, 10] => "O".into(), [0x55, 4, 11] => "OU".into(), [0x55, 4, 5] => "serialNumber".into(),
        [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 1, 9, 1] => "EMAIL".into(), [0x09, 0x92, 0x26, 0x89, 0x93, 0xf2, 0x2c, 0x64, 1, 0x19] => "DC".into(),
        _ => {
            let arcs = o.iter().fold((vec![], 0u64), |(mut a, v), b| { let v = v << 7 | (b & 0x7f) as u64; if b & 0x80 == 0 { a.push(v); (a, 0) } else { (a, v) } }).0;
            arcs.first().map(|&f| { let top = (f / 40).min(2); [top, f - 40 * top].into_iter().chain(arcs[1..].iter().copied()).map(|n| n.to_string()).collect::<Vec<_>>().join(".") }).unwrap_or_default()
        }
    }
}
fn asn1_text(t: u8, v: &[u8]) -> String { if t == 0x1e { String::from_utf16_lossy(&v.chunks(2).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])).collect::<Vec<_>>()) } else { String::from_utf8_lossy(v).to_string() } }
pub fn dn_text(name: &[u8]) -> String {
    let esc = |v: String| v.chars().flat_map(|c| ",+\"\\<>;".contains(c).then_some('\\').into_iter().chain([c])).collect::<String>();
    let rdn = |set: &&[u8]| kids(set).iter().filter_map(|atv| { let p = kids(atv); let ((_, oid), (t, v)) = (body(p.first()?)?, body(p.get(1)?)?); Some(format!("{}={}", oid_name(oid), esc(asn1_text(t, v)))) }).collect::<Vec<_>>().join("+");
    kids(name).iter().rev().map(rdn).collect::<Vec<_>>().join(",")
}
fn asn1_date(d: &[u8]) -> Option<String> {
    let (t, v) = body(d)?;
    let v = std::str::from_utf8(v).ok()?;
    let (y, rest) = if t == 0x17 { let yy: u32 = v.get(..2)?.parse().ok()?; (if yy >= 50 { 1900 + yy } else { 2000 + yy }, v.get(2..)?) } else { (v.get(..4)?.parse().ok()?, v.get(4..)?) };
    let (m, day): (u32, u32) = (rest.get(..2)?.parse().ok()?, rest.get(2..4)?.parse().ok()?);
    Some(format!("{y:04}-{m:02}-{day:02}"))
}
#[derive(Debug, Default, PartialEq)]
pub struct Info { pub subject: String, pub issuer: String, pub from: String, pub to: String }
pub fn info(der: &[u8]) -> Info {
    let f = tbs(der).unwrap_or_default();
    let dates = f.get(3).map(|v| kids(v)).unwrap_or_default();
    let date = |k: usize| dates.get(k).and_then(|d| asn1_date(d)).unwrap_or_else(|| "?".into());
    Info { subject: f.get(4).map(|n| dn_text(n)).unwrap_or_default(), issuer: f.get(2).map(|n| dn_text(n)).unwrap_or_default(), from: date(0), to: date(1) }
}
pub fn spki_pin(der: &[u8]) -> Option<String> { cert_parts(der).map(|(_, _, k)| format!("sha256/{}", STANDARD.encode(Sha256::digest(k)))) }
pub fn fingerprint(der: &[u8]) -> String { Sha256::digest(der).iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":") }
pub fn fp_lines(fp: &str) -> String { fp.as_bytes().chunks(48).map(|c| String::from_utf8_lossy(c).trim_end_matches(':').to_string()).collect::<Vec<_>>().join("\n") }
pub fn ders(pem: &str) -> Vec<Vec<u8>> {
    pem.split("-----BEGIN CERTIFICATE-----").skip(1).filter_map(|b| b.split("-----END CERTIFICATE-----").next()).filter_map(|b| STANDARD.decode(b.chars().filter(|c| !c.is_whitespace()).collect::<String>()).ok()).filter(|d| cert_parts(d).is_some()).collect()
}
pub fn pem(der: &[u8]) -> String { format!("-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n", STANDARD.encode(der).as_bytes().chunks(64).map(String::from_utf8_lossy).collect::<Vec<_>>().join("\n")) }
fn self_signed(der: &[u8]) -> bool { cert_parts(der).map(|(i, s, _)| i == s).unwrap_or(true) }
fn issuers<'a>(cert: &[u8], pool: &'a [Vec<u8>]) -> Vec<&'a Vec<u8>> {
    let want = cert_parts(cert).map(|(i, _, _)| i);
    pool.iter().filter(|p| want.is_some() && cert_parts(p).map(|(_, s, _)| s) == want).collect()
}
pub fn completions(chain: &[Vec<u8>], pool: &[Vec<u8>], depth: usize) -> Vec<Vec<Vec<u8>>> {
    let Some(last) = chain.last().filter(|l| depth > 0 && !self_signed(l)) else { return vec![] };
    issuers(last, pool).into_iter().filter(|p| !chain.contains(p)).flat_map(|p| { let next = [chain, std::slice::from_ref(p)].concat(); let deeper = completions(&next, pool, depth - 1); std::iter::once(next).chain(deeper) }).collect()
}
pub fn aia_urls(der: &[u8]) -> Vec<String> {
    const OID: [u8; 10] = [0x06, 0x08, 0x2B, 0x06, 0x01, 0x05, 0x05, 0x07, 0x30, 0x02];
    der.windows(OID.len()).enumerate().filter(|(_, w)| *w == OID).filter_map(|(i, _)| { let j = i + OID.len(); let n = *der.get(j + 1)? as usize; (der.get(j) == Some(&0x86) && n < 0x80).then(|| der.get(j + 2..j + 2 + n)).flatten().map(|u| String::from_utf8_lossy(u).to_string()) }).filter(|u| u.starts_with("http://")).collect()
}
pub fn dn_field(dn: &str, key: &str) -> Option<String> {
    let (mut parts, mut esc) = (vec![String::new()], false);
    for c in dn.chars() { match (esc, c) { (false, '\\') => esc = true, (false, ',') => parts.push(String::new()), _ => { esc = false; parts.last_mut()?.push(c) } } }
    parts.iter().filter_map(|p| p.split_once('=')).find(|(k, _)| k.trim().eq_ignore_ascii_case(key)).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty())
}
pub fn issuer_label(dn: &str) -> String {
    match (dn_field(dn, "O"), dn_field(dn, "CN")) { (Some(o), Some(c)) if o != c => format!("{o} ({c})"), (o, c) => o.or(c).unwrap_or_else(|| if dn.is_empty() { "?".into() } else { dn.to_string() }) }
}
fn pool() -> Vec<Vec<u8>> { POOL.read().clone() }
pub fn needs_fetch(chain: &[Vec<u8>]) -> bool { chain.first().map(|l| !self_signed(l) && issuers(l, &[pool(), chain[1..].to_vec()].concat()).is_empty() && !aia_urls(l).is_empty()).unwrap_or(false) }
fn http_get(u: &str) -> Option<Vec<u8>> {
    use std::io::{Read, Write};
    let u = url::Url::parse(u).ok().filter(|u| u.scheme() == "http")?;
    let (host, port) = (u.host_str()?.to_string(), u.port_or_known_default()?);
    let addr = (host.as_str(), port).to_socket_addrs().ok()?.next()?;
    let mut s = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(4)).ok()?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    s.write_all(format!("GET {} HTTP/1.0\r\nHost: {host}\r\nUser-Agent: Haven-Desktop\r\nAccept: */*\r\nConnection: close\r\n\r\n", &u[url::Position::BeforePath..]).as_bytes()).ok()?;
    let mut buf = Vec::new();
    s.take(256 * 1024).read_to_end(&mut buf).ok()?;
    let at = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
    (String::from_utf8_lossy(&buf[..at]).split_whitespace().nth(1) == Some("200")).then(|| buf[at + 4..].to_vec())
}
fn der_or_pem(b: &[u8]) -> Option<Vec<u8>> { if b.first() == Some(&0x30) { Some(b.to_vec()).filter(|d| cert_parts(d).is_some()) } else { ders(&String::from_utf8_lossy(b)).into_iter().next() } }
pub fn fetch_issuers(leaf: &[u8]) {
    let mut cur = leaf.to_vec();
    for _ in 0..3 {
        if self_signed(&cur) || !issuers(&cur, &pool()).is_empty() { return }
        let want = cert_parts(&cur).map(|(i, _, _)| i.to_vec());
        let Some((u, next)) = aia_urls(&cur).into_iter().find_map(|u| http_get(&u).and_then(|b| der_or_pem(&b)).filter(|d| cert_parts(d).map(|(_, s, _)| s.to_vec()) == want).map(|d| (u, d))) else { return };
        let _ = AIA_DIR.read().as_ref().map(|d| std::fs::write(d.join(Sha256::digest(u.as_bytes()).iter().map(|b| format!("{b:02x}")).collect::<String>()), &next));
        POOL.write().push(next.clone());
        cur = next;
    }
}
pub fn init(app: &AppHandle) {
    let Some(dir) = app.path().app_cache_dir().ok().map(|d| d.join("aia")).filter(|d| std::fs::create_dir_all(d).is_ok()) else { return };
    let cached: Vec<Vec<u8>> = std::fs::read_dir(&dir).map(|r| r.flatten().filter_map(|e| std::fs::read(e.path()).ok()).filter(|d| cert_parts(d).is_some()).collect()).unwrap_or_default();
    POOL.write().extend(cached);
    *AIA_DIR.write() = Some(dir);
}
fn host_of(url: &str) -> Option<String> { url::Url::parse(url).ok().and_then(|u| u.host_str().map(normalize_host)) }
pub fn chosen(app: &AppHandle, host: &str) -> bool {
    let st = app.state::<AppState>();
    let prefs = state::get_user_prefs(app).unwrap_or(Value::Null);
    let urls: Vec<String> = [st.active_server_url.lock().clone(), st.primary_server_url.lock().clone(), prefs.get("serverUrl").and_then(Value::as_str).map(str::to_string)].into_iter().flatten().chain(crate::server_list::read(app).history.iter().map(|e| crate::server_list::url_of(e).to_string())).collect();
    let host = normalize_host(host);
    urls.iter().filter_map(|u| host_of(u)).any(|h| h == host)
}
pub fn migrate(app: &AppHandle) {
    if state::get_bool(app, "certTrustMigrated").unwrap_or(false) { return }
    let prefs = state::get_user_prefs(app).unwrap_or(Value::Null);
    let mut hosts: Vec<String> = crate::server_list::read(app).history.iter().map(|e| crate::server_list::url_of(e).to_string()).chain(prefs.get("serverUrl").and_then(Value::as_str).map(str::to_string)).filter_map(|u| host_of(&u)).filter(|h| !h.is_empty() && !is_local_host(h)).collect();
    hosts.sort();
    hosts.dedup();
    let _ = state::store(app).map(|s| { s.set("certTrustGrandfathered", json!(hosts)); s.set("certTrustMigrated", json!(true)); s.save() });
}
fn remember(app: &AppHandle, host: &str, pin: &str) {
    let mut pins = state::get_value(app, "trustedCertificates").ok().and_then(|v| v.as_object().cloned()).unwrap_or_default();
    let mut set = pins_of(pins.get(host));
    let _ = (!pin.is_empty() && !set.iter().any(|p| p == pin)).then(|| set.push(pin.to_string()));
    pins.insert(host.to_string(), json!(set));
    let waiting: Vec<Value> = state::get_value(app, "certTrustGrandfathered").ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().into_iter().filter(|h| h.as_str() != Some(host)).collect();
    let _ = state::store(app).map(|s| { s.set("trustedCertificates", Value::Object(pins)); s.set("certTrustGrandfathered", json!(waiting)); s.save() });
}
fn verdict_for(app: &AppHandle, host: &str, presented: &[&str]) -> Verdict {
    let get = |k: &str| state::get_value(app, k).unwrap_or(Value::Null);
    decision(host, presented, &get("trustedCertificates"), &get("certTrustGrandfathered"))
}
fn auto_trusted(app: &AppHandle, host: &str, leaf: &[u8]) -> bool {
    let (fp, pin) = (fingerprint(leaf), spki_pin(leaf).unwrap_or_default());
    let v = verdict_for(app, host, &[&fp, &pin]);
    let _ = (v == Verdict::Grandfathered).then(|| remember(app, host, &pin));
    matches!(v, Verdict::Local | Verdict::Pinned | Verdict::Grandfathered)
}
pub fn question_pending(app: &AppHandle) -> bool { !app.state::<AppState>().cert_questions.lock().is_empty() }
pub struct Question { pub title: String, pub heading: String, pub body: String, pub details: String, pub more: String, pub trust: String, pub cancel: String }
pub fn question(app: &AppHandle, host: &str, leaf: &[u8], changed: bool) -> Question {
    let (public, i, fp) = (public_name(host), info(leaf), fingerprint(leaf));
    let vpn = public && crate::nav_fail::vpn_active();
    let tv = |k: &str, v: &[(&str, &str)]| state::tv(app, k, v);
    let fpv = format!("\n{}", fp_lines(&fp));
    let heading = if public { state::t(app, if vpn { "cert.vpnTitle" } else { "cert.networkTitle" }) } else { tv(if changed { "cert.changedMessage" } else { "cert.unknownMessage" }, &[("host", host)]) };
    let body = if public { [Some(tv("cert.certFor", &[("host", host), ("issuer", &issuer_label(&i.issuer))])), vpn.then(|| tv("cert.vpnTip", &[("host", host)])), changed.then(|| state::t(app, "cert.keyChanged"))].into_iter().flatten().collect::<Vec<_>>().join("\n\n") } else { format!("{}\n\n{}", state::t(app, if changed { "cert.changedDetail" } else { "cert.unknownDetail" }), tv("cert.fingerprint", &[("value", &fpv)])) };
    let subject = if i.subject.is_empty() { host.to_string() } else { dn_field(&i.subject, "CN").unwrap_or(i.subject.clone()) };
    let details = [tv("cert.issuedTo", &[("value", &subject)]), tv("cert.issuer", &[("value", if i.issuer.is_empty() { "?" } else { &i.issuer })]), tv("cert.valid", &[("from", &i.from), ("to", &i.to)]), tv("cert.fingerprint", &[("value", &fpv)])].join("\n");
    Question { title: state::t(app, "cert.title"), heading, body, details, more: state::t(app, "cert.details"), trust: state::t(app, if changed { "cert.trustNew" } else { "cert.trust" }), cancel: state::t(app, "dialog.cancel") }
}
pub struct Waiter { pub resume: Box<dyn FnOnce(bool)>, pub nav: Option<String>, pub main: bool }
thread_local! {
    static ASKING: std::cell::RefCell<std::collections::HashMap<String, Vec<Waiter>>> = Default::default();
    static DECLINED: std::cell::RefCell<std::collections::HashSet<String>> = Default::default();
}
fn active_or(app: &AppHandle, uri: &str) -> String { app.state::<AppState>().active_server_url.lock().clone().unwrap_or_else(|| uri.to_string()) }
pub fn settle(app: &AppHandle, host: &str, chain: &[Vec<u8>], w: Waiter, show: impl FnOnce(String, Question)) -> bool {
    let pin = spki_pin(&chain[0]).unwrap_or_default();
    let trusted = auto_trusted(app, host, &chain[0]);
    let changed = !trusted && verdict_for(app, host, &[]) == Verdict::Changed;
    let key = format!("{host}|{pin}");
    let declined = DECLINED.with(|d| d.borrow().contains(&key));
    let problem = |p: Problem| w.nav.clone().filter(|_| w.main).map(|uri| (uri, p));
    let shown = match kind(chosen(app, host), trusted, crate::nav_fail::captive(), host) {
        Kind::Retry => { (w.resume)(true); return true }
        Kind::Captive => problem(Problem::Captive),
        Kind::Foreign => problem(Problem::Redirect { host: host.to_string(), href: w.nav.clone().unwrap_or_default() }),
        Kind::Trust if w.nav.is_some() || !declined => {
            if ASKING.with(|a| { let mut a = a.borrow_mut(); let list = a.entry(key.clone()).or_default(); list.push(w); list.len() == 1 }) {
                app.state::<AppState>().cert_questions.lock().insert(key.clone());
                show(key, question(app, host, &chain[0], changed));
            }
            return true;
        }
        Kind::Trust => None,
    };
    (w.resume)(false);
    shown.map(|(uri, p)| crate::nav_fail::show_problem(app, &active_or(app, &uri), p)).is_some()
}
pub fn answer(app: &AppHandle, key: &str, yes: bool) {
    let Some(waiters) = ASKING.with(|a| a.borrow_mut().remove(key)) else { return };
    let _ = yes.then(|| key.split_once('|').map(|(host, pin)| remember(app, host, pin)));
    app.state::<AppState>().cert_questions.lock().remove(key);
    DECLINED.with(|d| { let _ = if yes { d.borrow_mut().remove(key) } else { d.borrow_mut().insert(key.to_string()) }; });
    for w in waiters {
        let _ = (yes && w.nav.is_some()).then(|| *app.state::<AppState>().pending_loading.lock() = true);
        let untrusted = w.nav.clone().filter(|_| !yes && w.main);
        (w.resume)(yes);
        let _ = untrusted.map(|uri| crate::nav_fail::show_problem(app, &active_or(app, &uri), Problem::Untrusted));
    }
}
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use gtk::gio::prelude::*;
    use gtk::gio::TlsCertificate;
    use gtk::glib;
    use gtk::prelude::*;
    use webkit2gtk::{WebContext, WebContextExt, WebView, WebViewExt, WebsiteDataManagerExt};
    fn der(cert: &TlsCertificate) -> Vec<u8> { cert.property::<Option<glib::ByteArray>>("certificate").map(|b| b.to_vec()).unwrap_or_default() }
    fn presented(cert: &TlsCertificate) -> Vec<Vec<u8>> { std::iter::successors(Some(cert.clone()), |c| c.issuer()).take(8).map(|c| der(&c)).filter(|d| !d.is_empty()).collect() }
    fn os_trusts(db: &impl IsA<gtk::gio::TlsDatabase>, chain: &[Vec<u8>], host: &str, port: u16) -> bool {
        let Ok(c) = TlsCertificate::from_pem(&chain.iter().map(|d| pem(d)).collect::<String>()) else { return false };
        db.verify_chain(&c, gtk::gio::TLS_DATABASE_PURPOSE_AUTHENTICATE_SERVER, Some(&gtk::gio::NetworkAddress::new(host, port)), None::<&gtk::gio::TlsInteraction>, gtk::gio::TlsDatabaseVerifyFlags::NONE, None::<&gtk::gio::Cancellable>).map(|f| f.is_empty()).unwrap_or(false)
    }
    fn completes_in(db: &impl IsA<gtk::gio::TlsDatabase>, chain: &[Vec<u8>], pool: &[Vec<u8>], host: &str, port: u16) -> bool { !chain.is_empty() && completions(&chain[..1], &[pool, &chain[1..]].concat(), 4).iter().any(|c| os_trusts(db, c, host, port)) }
    fn completes(chain: &[Vec<u8>], host: &str, port: u16) -> bool { completes_in(&gtk::gio::TlsBackend::default().default_database(), chain, &pool(), host, port) }
    fn allow(ctx: &WebContext, cert: &TlsCertificate, host: &str, nav: Option<(WebView, String)>) -> bool {
        ctx.allow_tls_certificate_for_host(cert, host);
        let _ = nav.map(|(v, u)| v.load_uri(&u));
        true
    }
    pub fn attach(view: &WebView, app: AppHandle, label: String) {
        let _ = view.website_data_manager().map(|m| m.set_tls_errors_policy(webkit2gtk::TLSErrorsPolicy::Fail));
        view.connect_load_failed_with_tls_errors(move |view, uri, cert, _| check(view, &app, &label, uri, cert, true));
    }
    fn check(view: &WebView, app: &AppHandle, label: &str, uri: &str, cert: &TlsCertificate, fetch: bool) -> bool {
        let Some((host, port)) = url::Url::parse(uri).ok().and_then(|u| Some((normalize_host(u.host_str()?), u.port_or_known_default()?))) else { return false };
        let (Some(ctx), chain) = (view.web_context(), presented(cert)) else { return false };
        if chain.is_empty() { return false }
        if completes(&chain, &host, port) { return allow(&ctx, cert, &host, Some((view.clone(), uri.to_string()))) }
        if fetch && needs_fetch(&chain) {
            let (view, app, label, uri, cert, leaf) = (view.clone(), app.clone(), label.to_string(), uri.to_string(), cert.clone(), chain[0].clone());
            glib::MainContext::default().spawn_local(async move { let _ = gtk::gio::spawn_blocking(move || fetch_issuers(&leaf)).await; check(&view, &app, &label, &uri, &cert, false); });
            return true;
        }
        settle_on(app, view, &ctx, cert, &host, &chain, Some(uri.to_string()), label == "main")
    }
    fn settle_on(app: &AppHandle, view: &WebView, ctx: &WebContext, cert: &TlsCertificate, host: &str, chain: &[Vec<u8>], nav: Option<String>, main: bool) -> bool {
        let (c, t, h, v, n) = (ctx.clone(), cert.clone(), host.to_string(), view.clone(), nav.clone());
        let resume = Box::new(move |yes: bool| { let _ = yes.then(|| allow(&c, &t, &h, n.map(|u| (v, u)))); });
        let (a, parent) = (app.clone(), view.clone());
        settle(app, host, chain, Waiter { resume, nav, main }, move |key, q| show(&a, &parent, key, q))
    }
    fn show(app: &AppHandle, parent: &WebView, key: String, q: Question) {
        let window = parent.toplevel().and_then(|t| t.downcast::<gtk::Window>().ok());
        let md = gtk::MessageDialog::new(window.as_ref(), gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT, gtk::MessageType::Warning, gtk::ButtonsType::None, &q.heading);
        md.set_title(&q.title);
        md.set_secondary_text(Some(&q.body));
        md.add_button(&q.cancel, gtk::ResponseType::Cancel);
        md.add_button(&q.trust, gtk::ResponseType::Accept);
        md.set_default_response(gtk::ResponseType::Cancel);
        let more = gtk::Label::new(Some(&q.details));
        more.set_selectable(true);
        more.set_xalign(0.0);
        more.set_line_wrap(true);
        more.set_max_width_chars(64);
        let exp = gtk::Expander::new(Some(&q.more));
        exp.add(&more);
        let _ = md.message_area().downcast::<gtk::Box>().map(|b| b.pack_end(&exp, false, false, 0));
        let (a, k) = (app.clone(), key.clone());
        md.connect_response(move |d, r| { answer(&a, &k, r == gtk::ResponseType::Accept); unsafe { d.destroy() } });
        let app = app.clone();
        md.connect_destroy(move |_| answer(&app, &key, false));
        md.show_all();
    }
    fn probe(view: &WebView, app: &AppHandle, targets: Vec<(String, u16)>, ask: bool) {
        let Some(ctx) = view.web_context() else { return };
        for (host, port) in targets {
            let client = gtk::gio::SocketClient::new();
            client.set_tls(true);
            let (ctx, app, h, view) = (ctx.clone(), app.clone(), host.clone(), view.clone());
            client.connect_event(move |_, ev, _, conn| {
                let Some(tls) = conn.filter(|_| ev == gtk::gio::SocketClientEvent::TlsHandshaking).and_then(|c| c.downcast_ref::<gtk::gio::TlsClientConnection>()) else { return };
                let (ctx, app, h, view) = (ctx.clone(), app.clone(), h.clone(), view.clone());
                tls.connect_accept_certificate(move |_, cert, _| {
                    let chain = presented(cert);
                    let ok = !chain.is_empty() && (completes(&chain, &h, port) || auto_trusted(&app, &h, &chain[0]));
                    if ok { ctx.allow_tls_certificate_for_host(cert, &h) } else if ask && !chain.is_empty() {
                        let (ctx, app, h, view, cert) = (ctx.clone(), app.clone(), h.clone(), view.clone(), cert.clone());
                        glib::idle_add_local_once(move || { settle_on(&app, &view, &ctx, &cert, &h, &chain, None, true); });
                    }
                    ok
                });
            });
            let addr = if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") };
            client.connect_to_host_async(&addr, port, None::<&gtk::gio::Cancellable>, |c| { let _ = c.map(|c| c.close(None::<&gtk::gio::Cancellable>)); });
        }
    }
    fn https_target(u: &str) -> Option<(String, u16)> { url::Url::parse(u).ok().filter(|u| u.scheme() == "https").and_then(|u| Some((normalize_host(u.host_str()?), u.port_or_known_default()?))) }
    pub fn prewarm(view: &WebView, app: &AppHandle) {
        let targets = crate::server_list::read(app).history.iter().filter_map(|e| https_target(crate::server_list::url_of(e))).filter(|(h, _)| is_local_host(h) || verdict_for(app, h, &[]) != Verdict::Unknown).collect();
        probe(view, app, targets, false);
    }
    pub fn recheck(view: &WebView, app: &AppHandle) {
        let active = app.state::<AppState>().active_server_url.lock().clone();
        probe(view, app, active.as_deref().and_then(https_target).into_iter().collect(), true);
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn gnutls_completes_leaf_only_chains() {
            let dir = std::env::temp_dir().join(format!("haven-cert-test-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("root.pem"), include_str!("../certs/test/root.pem")).unwrap();
            let db = gtk::gio::TlsFileDatabase::new(dir.join("root.pem")).unwrap();
            let one = |p: &str| ders(p).remove(0);
            let (int, leaf, mitm) = (one(include_str!("../certs/test/int.pem")), one(include_str!("../certs/test/leaf.pem")), one(include_str!("../certs/test/mitm-leaf.pem")));
            assert!(!os_trusts(&db, &[leaf.clone()], "haven.test", 443));
            assert!(completes_in(&db, &[leaf.clone()], &[int.clone()], "haven.test", 443));
            assert!(completes_in(&db, &[leaf.clone(), int.clone()], &[], "haven.test", 443));
            assert!(!completes_in(&db, &[leaf.clone()], &[int.clone()], "other.test", 443));
            assert!(!completes_in(&db, &[leaf.clone()], &[], "haven.test", 443));
            assert!(!completes_in(&db, &[mitm], &[int], "haven.test", 443));
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
#[cfg(target_os = "linux")]
pub use linux::{attach, prewarm, recheck};
#[cfg(windows)]
mod webview2 {
    use super::*;
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_14, ICoreWebView2ServerCertificateErrorDetectedEventArgs, COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW, COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_CANCEL};
    use webview2_com::{take_pwstr, NavigationStartingEventHandler, ServerCertificateErrorDetectedEventHandler};
    use windows_core::{Interface, PWSTR};
    thread_local! { static NAVS: std::cell::RefCell<std::collections::HashMap<String, String>> = Default::default(); }
    fn bare(u: &str) -> String { u.split('#').next().unwrap_or(u).to_string() }
    pub fn navigating(label: &str, url: &str) { NAVS.with(|n| n.borrow_mut().insert(label.to_string(), bare(url))); }
    pub fn attach(window: &tauri::WebviewWindow) {
        let (app, label) = (window.app_handle().clone(), window.label().to_string());
        let _ = window.with_webview(move |w| unsafe {
            let Ok(core) = w.controller().CoreWebView2() else { return };
            let l = label.clone();
            let started = NavigationStartingEventHandler::create(Box::new(move |_, args| {
                let mut uri = PWSTR::null();
                let _ = args.filter(|a| a.Uri(&mut uri).is_ok()).map(|_| navigating(&l, &take_pwstr(uri)));
                Ok(())
            }));
            let mut token = 0;
            let _ = core.add_NavigationStarting(&started, &mut token);
            let Ok(core) = core.cast::<ICoreWebView2_14>() else { return };
            let handler = ServerCertificateErrorDetectedEventHandler::create(Box::new(move |_, args| { let _ = args.map(|a| detected(&app, &label, a)); Ok(()) }));
            let _ = core.add_ServerCertificateErrorDetected(&handler, &mut token);
        });
    }
    fn detected(app: &AppHandle, label: &str, args: ICoreWebView2ServerCertificateErrorDetectedEventArgs) {
        let text = |f: &dyn Fn(*mut PWSTR) -> windows_core::Result<()>| { let mut p = PWSTR::null(); f(&mut p).ok().map(|_| take_pwstr(p)).unwrap_or_default() };
        let uri = text(&|p| unsafe { args.RequestUri(p) });
        let pems = unsafe { args.ServerCertificate() }.ok().map(|c| {
            let rest = unsafe { c.PemEncodedIssuerCertificateChain() }.ok().map(|s| { let mut n = 0; let _ = unsafe { s.Count(&mut n) }; (0..n).map(|i| text(&|p| unsafe { s.GetValueAtIndex(i, p) })).collect::<String>() }).unwrap_or_default();
            text(&|p| unsafe { c.ToPemEncoding(p) }) + &rest
        });
        let chain = ders(&pems.unwrap_or_default());
        let deferral = host_of(&uri).filter(|_| !chain.is_empty()).and_then(|h| unsafe { args.GetDeferral() }.ok().map(|d| (h, d)));
        let Some((host, deferral)) = deferral else { return unsafe { let _ = args.SetAction(COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_CANCEL); } };
        let nav = NAVS.with(|n| n.borrow().get(label).cloned()).filter(|n| *n == bare(&uri)).map(|_| uri.clone());
        let ours = nav.is_some() && label == "main";
        let resume = Box::new(move |yes: bool| unsafe { let _ = (!yes && ours).then(|| crate::nav_fail::CERT_CANCELLED.store(true, std::sync::atomic::Ordering::Relaxed)); let _ = args.SetAction(if yes { COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW } else { COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_CANCEL }); let _ = deferral.Complete(); });
        let (a, l) = (app.clone(), label.to_string());
        settle(app, &host, &chain, Waiter { resume, nav, main: label == "main" }, move |key, q| show(&a, &l, key, q));
    }
    fn show(app: &AppHandle, label: &str, key: String, q: Question) {
        let hwnd = app.get_webview_window(label).or_else(|| app.get_webview_window("main")).and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize).unwrap_or(0);
        let app = app.clone();
        std::thread::spawn(move || { let yes = unsafe { ask(hwnd, &q) }; let a = app.clone(); let _ = app.run_on_main_thread(move || answer(&a, &key, yes)); });
    }
    unsafe fn ask(hwnd: isize, q: &Question) -> bool {
        use windows_sys::Win32::UI::Controls::*;
        let w = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (title, heading, body, details, more, trust, cancel) = (w(&q.title), w(&q.heading), w(&q.body), w(&q.details), w(&q.more), w(&q.trust), w(&q.cancel));
        let buttons = [TASKDIALOG_BUTTON { nButtonID: 100, pszButtonText: trust.as_ptr() }, TASKDIALOG_BUTTON { nButtonID: 2, pszButtonText: cancel.as_ptr() }];
        let mut cfg: TASKDIALOGCONFIG = std::mem::zeroed();
        cfg.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
        cfg.hwndParent = hwnd as _;
        cfg.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW;
        cfg.pszWindowTitle = title.as_ptr();
        cfg.Anonymous1.pszMainIcon = TD_WARNING_ICON;
        cfg.pszMainInstruction = heading.as_ptr();
        cfg.pszContent = body.as_ptr();
        cfg.cButtons = buttons.len() as u32;
        cfg.pButtons = buttons.as_ptr();
        cfg.nDefaultButton = 2;
        cfg.pszExpandedInformation = details.as_ptr();
        cfg.pszExpandedControlText = more.as_ptr();
        cfg.pszCollapsedControlText = more.as_ptr();
        let mut pressed = 0;
        TaskDialogIndirect(&cfg, &mut pressed, std::ptr::null_mut(), std::ptr::null_mut()) == 0 && pressed == 100
    }
}
#[cfg(windows)]
pub use webview2::{attach, navigating};
#[cfg(target_os = "macos")]
mod wkwebview {
    use super::*;
    use std::ffi::{c_char, c_void, CStr, CString};
    type Decide = extern "C" fn(*mut c_void, *const c_char, *const *const u8, *const usize, usize, *const c_char, i32, u64);
    extern "C" {
        fn haven_macos_trust_attach(webview: *mut c_void, decide: Decide);
        fn haven_macos_trust_finish(request: u64, allow: i32);
        fn haven_macos_trust_ask(webview: *mut c_void, key: *const c_char, title: *const c_char, heading: *const c_char, body: *const c_char, details: *const c_char, more: *const c_char, trust: *const c_char, cancel: *const c_char, done: extern "C" fn(*const c_char, i32));
    }
    static APP: once_cell::sync::OnceCell<AppHandle> = once_cell::sync::OnceCell::new();
    static VIEWS: Lazy<parking_lot::Mutex<std::collections::HashMap<usize, String>>> = Lazy::new(Default::default);
    fn utf8(p: *const c_char) -> String { if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() } }
    pub fn attach(window: &tauri::WebviewWindow) {
        let _ = APP.set(window.app_handle().clone());
        let label = window.label().to_string();
        let _ = window.with_webview(move |w| { let view = w.inner(); VIEWS.lock().insert(view as usize, label); unsafe { haven_macos_trust_attach(view, decide) } });
    }
    extern "C" fn decide(view: *mut c_void, host: *const c_char, ders: *const *const u8, lens: *const usize, count: usize, page: *const c_char, loading: i32, request: u64) {
        let (host, page) = (normalize_host(&utf8(host)), utf8(page));
        let chain: Vec<Vec<u8>> = (0..count).map(|i| unsafe { std::slice::from_raw_parts(*ders.add(i), *lens.add(i)).to_vec() }).filter(|d| !d.is_empty()).collect();
        let Some(app) = APP.get().filter(|_| !chain.is_empty() && !host.is_empty()) else { return unsafe { haven_macos_trust_finish(request, 0) } };
        let main = VIEWS.lock().get(&(view as usize)).map(|l| l == "main").unwrap_or(false);
        let nav = (loading != 0 && host_of(&page).as_deref() == Some(host.as_str())).then_some(page);
        let resume = Box::new(move |yes: bool| unsafe { haven_macos_trust_finish(request, yes as i32) });
        let view = view as usize;
        settle(app, &host, &chain, Waiter { resume, nav, main }, move |key, q| show(view, key, q));
    }
    fn show(view: usize, key: String, q: Question) {
        let c = |s: &str| CString::new(s.replace('\0', "")).unwrap_or_default();
        let (key, title, heading, body, details, more, trust, cancel) = (c(&key), c(&q.title), c(&q.heading), c(&q.body), c(&q.details), c(&q.more), c(&q.trust), c(&q.cancel));
        unsafe { haven_macos_trust_ask(view as *mut c_void, key.as_ptr(), title.as_ptr(), heading.as_ptr(), body.as_ptr(), details.as_ptr(), more.as_ptr(), trust.as_ptr(), cancel.as_ptr(), answered) }
    }
    extern "C" fn answered(key: *const c_char, yes: i32) { let _ = APP.get().map(|app| answer(app, &utf8(key), yes != 0)); }
}
#[cfg(target_os = "macos")]
pub use wkwebview::attach;
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
        let pins = json!({ "a.example.com": "AA", "m.example.com": ["sha256/old", "sha256/vpn"] });
        let old = json!(["b.example.com"]);
        assert_eq!(decision("192.168.1.2", &["x"], &pins, &old), Verdict::Local);
        assert_eq!(decision("A.example.com.", &["AA"], &pins, &old), Verdict::Pinned);
        assert_eq!(decision("a.example.com", &["BB", "sha256/x"], &pins, &old), Verdict::Changed);
        assert_eq!(decision("m.example.com", &["FP", "sha256/vpn"], &pins, &old), Verdict::Pinned);
        assert_eq!(decision("m.example.com", &["FP", "sha256/new"], &pins, &old), Verdict::Changed);
        assert_eq!(decision("b.example.com", &["x"], &pins, &old), Verdict::Grandfathered);
        assert_eq!(decision("c.example.com", &["x"], &pins, &old), Verdict::Unknown);
        assert_eq!(decision("c.example.com", &[""], &json!({ "c.example.com": [""] }), &old), Verdict::Changed);
    }
    #[test]
    fn kinds() {
        assert_eq!(kind(false, false, false, "portal.example.net"), Kind::Foreign);
        assert_eq!(kind(false, false, true, "portal.example.net"), Kind::Captive);
        assert_eq!(kind(false, true, true, "x.example.net"), Kind::Retry);
        assert_eq!(kind(true, false, true, "haven.example.com"), Kind::Captive);
        assert_eq!(kind(true, true, true, "haven.example.com"), Kind::Retry);
        assert_eq!(kind(true, false, true, "203.0.113.5"), Kind::Trust);
        assert_eq!(kind(true, false, false, "haven.example.com"), Kind::Trust);
    }
    #[test]
    fn names() {
        for h in ["haven.moviethingy.xyz", "chat.example.com."] { assert!(public_name(h), "{h}") }
        for h in ["nas", "box.lan", "srv.home.arpa", "x.internal", "203.0.113.5", "[2001:db8::1]", "fileserver.corp"] { assert!(!public_name(h), "{h}") }
        assert_eq!(issuer_label("CN=R11,O=Let's Encrypt,C=US"), "Let's Encrypt (R11)");
        assert_eq!(issuer_label("CN=FortiGate CA"), "FortiGate CA");
        assert_eq!(issuer_label("O=Acme\\, Inc.,CN=Acme\\, Inc."), "Acme, Inc.");
        assert_eq!(issuer_label(""), "?");
    }
    #[test]
    fn certificate_details() {
        let one = |p: &str| ders(p).remove(0);
        assert_eq!(info(&one(include_str!("../certs/test/leaf-only.pem"))), Info { subject: "CN=haven.moviethingy.xyz".into(), issuer: "CN=YE1,O=Let's Encrypt,C=US".into(), from: "2026-07-19".into(), to: "2026-10-17".into() });
        let mitm = info(&one(include_str!("../certs/test/mitm-leaf.pem")));
        assert_eq!((mitm.subject.as_str(), issuer_label(&mitm.issuer).as_str(), mitm.to.as_str()), ("CN=haven.test", "Acme VPN (Acme VPN Threat Protection CA)", "2126-09-18"));
        assert_eq!(info(b"junk"), Info { from: "?".into(), to: "?".into(), ..Default::default() });
        let name = [0x30, 0x22, 0x31, 0x0d, 0x30, 0x0b, 0x06, 0x03, 0x55, 0x04, 0x0a, 0x0c, 0x04, b'A', b',', b'B', b'+', 0x31, 0x11, 0x30, 0x0f, 0x06, 0x03, 0x2a, 0x03, 0x04, 0x1e, 0x08, 0, b'V', 0, b'P', 0, b'N', 0, b'!'];
        assert_eq!(dn_text(&name), "1.2.3.4=VPN!,O=A\\,B\\+");
        assert_eq!(dn_field(&dn_text(&name), "O").as_deref(), Some("A,B+"));
        assert_eq!(asn1_date(b"\x18\x0f20491231235959Z").as_deref(), Some("2049-12-31"));
        assert_eq!(asn1_date(b"\x17\x0d991231235959Z").as_deref(), Some("1999-12-31"));
        assert_eq!(asn1_date(b"\x17\x0dxx1231235959Z"), None);
    }
    #[test]
    fn bundle_builds_chains() {
        let pool = ders(BUNDLE);
        assert_eq!(pool.len(), 37);
        let ye1 = pool.iter().find(|d| cert_parts(d).map(|(_, s, _)| s.windows(3).any(|w| w == b"YE1")).unwrap_or(false)).unwrap();
        let leaf = ders(include_str!("../certs/test/leaf-only.pem")).remove(0);
        assert_eq!(issuers(&leaf, &pool), vec![ye1]);
        let chains = completions(&[leaf.clone()], &pool, 4);
        assert!(chains.iter().any(|c| c.len() == 3 && &c[1] == ye1), "{}", chains.len());
        assert!(needs_fetch(&[leaf.clone()]) == false);
        assert_eq!(aia_urls(&leaf), vec!["http://ye1.i.lencr.org/".to_string()]);
        assert!(spki_pin(&leaf).unwrap().starts_with("sha256/"));
        assert_eq!(fingerprint(&leaf).len(), 95);
        assert_eq!(fp_lines(&fingerprint(&leaf)).lines().map(str::len).collect::<Vec<_>>(), vec![47, 47]);
        assert_eq!(ders(&pem(&leaf)), vec![leaf]);
    }
}
