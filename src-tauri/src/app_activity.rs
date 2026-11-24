use icu_properties::{props::{GeneralCategory, GeneralCategoryGroup}, CodePointMapData};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};
use std::{collections::HashMap, future::Future, os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt}, path::{Path, PathBuf}, pin::Pin, sync::{atomic::{AtomicBool, Ordering::SeqCst}, Arc}, time::{SystemTime, UNIX_EPOCH}};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{UnixListener, UnixStream}, sync::{mpsc, watch}};
pub const HANDSHAKE: i32 = 0;
pub const FRAME: i32 = 1;
pub const CLOSE: i32 = 2;
pub const PING: i32 = 3;
pub const PONG: i32 = 4;
pub const MAX_FRAME: usize = 64 * 1024;
const MAX_CLIENTS: usize = 10;
const PIPE_TRIES: u32 = 10;
const TEXT_MAX: usize = 120;
const NAME_MAX: usize = 60;
const DAY_MS: i64 = 24 * 3600 * 1000;
const GENERIC: [&str; 24] = ["java", "javaw", "python", "python3", "pythonw", "node", "electron", "wine", "wine64", "wine-preloader", "wine64-preloader", "wineserver", "bash", "sh", "cmd", "powershell", "pwsh", "dotnet", "mono", "steam", "proton", "bwrap", "flatpak", "systemd"];
pub type Lookup = Arc<dyn Fn(i64) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;
pub type OnChange = Arc<dyn Fn(Current) + Send + Sync>;
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Slot { Playing, Listening }
#[derive(Clone, PartialEq, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Act { pub name: String, pub details: String, pub started_at: i64, pub duration: i64, pub app: String }
#[derive(Clone, Default, PartialEq, Serialize, Debug)]
pub struct Current { pub playing: Option<Act>, pub listening: Option<Act> }
#[derive(Default)]
struct Client { activity: Option<(Slot, Act)>, at: u64 }
#[derive(Default)]
struct Me { ready: bool, client_id: String, pid: i64, name: String }
#[derive(Default)]
struct Shared { clients: HashMap<u64, Client>, next: u64, order: u64, last: String, tx: Option<mpsc::UnboundedSender<Current>> }
struct Ctx { shared: Arc<Mutex<Shared>>, lookup: Lookup, uid: u32 }
struct Run { path: PathBuf, stop: watch::Sender<bool> }
#[derive(Default)]
pub struct Hub { shared: Arc<Mutex<Shared>>, run: Mutex<Option<Run>>, wanted: AtomicBool, starting: AtomicBool }
#[derive(Default)]
pub struct FrameReader { buf: Vec<u8> }
pub fn encode_frame(op: i32, data: &Value) -> Vec<u8> {
    let json = data.to_string().into_bytes();
    op.to_le_bytes().into_iter().chain((json.len() as i32).to_le_bytes()).chain(json).collect()
}
impl FrameReader {
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<(i32, Value)>, String> {
        self.buf.extend_from_slice(chunk);
        let mut out = vec![];
        while let [a, b, c, d, e, f, g, h, ..] = self.buf[..] {
            let (op, len) = (i32::from_le_bytes([a, b, c, d]), i32::from_le_bytes([e, f, g, h]));
            (PONG >= op && op >= HANDSHAKE).then_some(()).ok_or("unknown message kind")?;
            (0..=MAX_FRAME as i32).contains(&len).then_some(()).ok_or("message too large")?;
            let end = 8 + len as usize;
            if self.buf.len() < end { break }
            out.push((op, serde_json::from_slice(&self.buf[8..end]).map_err(|x| x.to_string())?));
            self.buf.drain(..end);
        }
        (self.buf.len() <= MAX_FRAME + 8).then_some(out).ok_or_else(|| "message too large".to_string())
    }
}
fn cap(s: &str, max: usize) -> String {
    let mut used = 0;
    s.chars().take_while(|c| { used += c.len_utf16(); used <= max }).collect()
}
fn hidden(c: char) -> bool { GeneralCategoryGroup::Other.contains(CodePointMapData::<GeneralCategory>::new().get(c)) || matches!(c, '\u{2028}' | '\u{2029}') }
pub fn clean_text(s: &str, max: usize) -> String {
    let plain: String = s.chars().map(|c| if hidden(c) { ' ' } else { c }).collect();
    cap(&plain.split_whitespace().collect::<Vec<_>>().join(" "), max)
}
fn clean_value(v: Option<&Value>, max: usize) -> String { v.and_then(Value::as_str).map_or_else(String::new, |s| clean_text(s, max)) }
pub fn program_name(raw: &str) -> String {
    let leaf = clean_text(raw.rsplit(['/', '\\']).next().unwrap_or(""), NAME_MAX);
    let cut = [".exe", ".appimage", ".bin", ".x86_64"].iter().find(|x| leaf.len() >= x.len() && leaf.is_char_boundary(leaf.len() - x.len()) && leaf[leaf.len() - x.len()..].eq_ignore_ascii_case(x)).map_or(leaf.len(), |x| leaf.len() - x.len());
    let name = leaf[..cut].to_string();
    if GENERIC.contains(&name.to_lowercase().as_str()) { String::new() } else { name }
}
fn num(v: &Value) -> Option<f64> {
    match v { Value::Number(n) => n.as_f64(), Value::String(s) => s.trim().parse().ok(), _ => None }.filter(|n| n.is_finite())
}
pub fn to_millis(v: Option<&Value>) -> i64 {
    v.and_then(num).filter(|n| *n > 0.0).map_or(0, |n| (if n < 1e11 { n * 1000.0 } else { n }).floor() as i64)
}
pub fn to_haven_activity(activity: &Value, app_name: &str, now: i64) -> Option<(Slot, Act)> {
    let o = activity.as_object()?;
    let app = Some(clean_value(o.get("name"), NAME_MAX)).filter(|n| !n.is_empty()).unwrap_or_else(|| clean_text(app_name, NAME_MAX));
    let (details, state) = (clean_value(o.get("details"), TEXT_MAX), clean_value(o.get("state"), TEXT_MAX));
    let stamp = |k: &str| to_millis(o.get("timestamps").and_then(|t| t.get(k)));
    let started_at = Some(stamp("start")).filter(|s| *s > 0 && *s <= now && *s >= now - 7 * DAY_MS).unwrap_or(now);
    let end = stamp("end");
    let duration = if end > started_at && end - started_at <= DAY_MS { end - started_at } else { 0 };
    let act = |name: String, details: String, duration: i64, app: String| Act { name, details, started_at, duration, app };
    let text = cap(&[details.as_str(), state.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "), TEXT_MAX);
    match () {
        _ if o.get("type").and_then(num) == Some(2.0) && !details.is_empty() => Some((Slot::Listening, act(details, state, duration, app))),
        _ if !app.is_empty() => Some((Slot::Playing, act(app.clone(), text, 0, app))),
        _ if !details.is_empty() => Some((Slot::Playing, act(details, state, 0, String::new()))),
        _ if !state.is_empty() => Some((Slot::Playing, act(state, String::new(), 0, String::new()))),
        _ => None,
    }
}
fn pick_current<'a>(clients: impl IntoIterator<Item = &'a Client>) -> Current {
    let (mut out, mut seen) = (Current::default(), [-1i64; 2]);
    for c in clients {
        let Some((slot, act)) = &c.activity else { continue };
        let i = *slot as usize;
        if (c.at as i64) <= seen[i] { continue }
        seen[i] = c.at as i64;
        match slot { Slot::Playing => out.playing = Some(act.clone()), Slot::Listening => out.listening = Some(act.clone()) }
    }
    out
}
pub fn in_flatpak() -> bool { Path::new("/.flatpak-info").exists() }
pub fn runtime_dir(env: impl Fn(&str) -> Option<String>, flatpak: bool) -> PathBuf {
    let base = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"].iter().find_map(|k| env(k).filter(|v| !v.is_empty())).map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    if flatpak { base.join("app/com.discordapp.Discord") } else { base }
}
pub fn pipe_path(dir: &Path, index: u32) -> PathBuf { dir.join(format!("discord-ipc-{index}")) }
fn ready() -> Value {
    json!({ "cmd": "DISPATCH", "evt": "READY", "nonce": null, "data": { "v": 1, "config": { "cdn_host": "cdn.discordapp.com", "api_endpoint": "//discord.com/api", "environment": "production" }, "user": { "id": "0", "username": "haven", "discriminator": "0", "global_name": "Haven", "avatar": null, "bot": false, "flags": 0, "premium_type": 0 } } })
}
fn now_ms() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64) }
pub fn system_lookup() -> Lookup {
    Arc::new(|pid| -> Pin<Box<dyn Future<Output = String> + Send>> { Box::pin(async move {
        if in_flatpak() { return String::new() }
        #[cfg(target_os = "linux")]
        { if pid < 100 { return String::new() } std::fs::read_to_string(format!("/proc/{pid}/comm")).map(|s| s.trim().to_string()).unwrap_or_default() }
        #[cfg(not(target_os = "linux"))]
        { tokio::task::spawn_blocking(move || std::process::Command::new("/bin/ps").args(["-p", &pid.to_string(), "-o", "comm="]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()).await.unwrap_or_default() }
    }) })
}
impl Shared {
    fn publish(&mut self) {
        let current = pick_current(self.clients.values());
        let json = serde_json::to_string(&current).unwrap_or_default();
        if json != self.last { self.last = json; let _ = self.tx.as_ref().map(|t| t.send(current)); }
    }
}
fn handshake(data: &Value, ready: bool) -> Option<String> {
    let id = match data.get("client_id")? { Value::String(s) => s.clone(), Value::Number(n) => n.to_string(), _ => return None };
    let version = data.get("v").filter(|v| !v.is_null()).map_or(Some(1.0), num);
    (!ready && !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_digit()) && version == Some(1.0)).then_some(id)
}
async fn name_for(me: &mut Me, pid: Option<&Value>, lookup: &Lookup) -> String {
    let id = pid.and_then(num).filter(|n| n.fract() == 0.0 && *n > 0.0 && *n <= 2147483647.0).map(|n| n as i64);
    if let Some(id) = id.filter(|id| *id != me.pid) { me.pid = id; me.name = program_name(&lookup(id).await); }
    me.name.clone()
}
async fn request(s: &mut UnixStream, ctx: &Ctx, id: u64, me: &mut Me, msg: &Value) -> Result<(), ()> {
    let Some(o) = msg.as_object().filter(|o| o.get("cmd").and_then(Value::as_str) == Some("SET_ACTIVITY")) else { return Ok(()) };
    let args = o.get("args");
    let raw = args.and_then(|a| a.get("activity")).filter(|a| a.is_object());
    let turn = { let mut sh = ctx.shared.lock(); sh.order += 1; sh.order };
    let name = match raw { Some(_) => name_for(me, args.and_then(|a| a.get("pid")), &ctx.lookup).await, None => String::new() };
    let act = raw.and_then(|r| to_haven_activity(r, &name, now_ms()));
    let data = raw.map_or(Value::Null, |r| {
        let mut m = r.as_object().cloned().unwrap_or_default();
        m.insert("name".into(), json!(act.as_ref().map_or("", |(_, a)| a.app.as_str())));
        m.insert("application_id".into(), json!(me.client_id));
        m.insert("type".into(), json!(r.get("type").and_then(num).map_or(0, |n| n as i64)));
        Value::Object(m)
    });
    {
        let mut sh = ctx.shared.lock();
        let client = sh.clients.get_mut(&id).ok_or(())?;
        client.activity = act;
        client.at = turn;
        sh.publish();
    }
    let reply = json!({ "cmd": "SET_ACTIVITY", "evt": null, "nonce": o.get("nonce").cloned().unwrap_or(Value::Null), "data": data });
    s.write_all(&encode_frame(FRAME, &reply)).await.map_err(|_| ())
}
async fn talk(s: &mut UnixStream, stop: &mut watch::Receiver<bool>, ctx: &Ctx, id: u64) {
    let (mut reader, mut buf, mut me) = (FrameReader::default(), [0u8; 4096], Me::default());
    loop {
        let n = tokio::select! { _ = stop.changed() => return, r = s.read(&mut buf) => match r { Ok(n) if n > 0 => n, _ => return } };
        let Ok(frames) = reader.push(&buf[..n]) else { return };
        for (op, data) in frames {
            let sent = match op {
                PING => s.write_all(&encode_frame(PONG, &data)).await.map_err(|_| ()),
                CLOSE => return,
                HANDSHAKE => match handshake(&data, me.ready) {
                    Some(cid) => { me.ready = true; me.client_id = cid; s.write_all(&encode_frame(FRAME, &ready())).await.map_err(|_| ()) }
                    None => return,
                },
                FRAME if me.ready => request(s, ctx, id, &mut me, &data).await,
                _ => Ok(()),
            };
            if sent.is_err() { return }
        }
    }
}
async fn serve(mut s: UnixStream, mut stop: watch::Receiver<bool>, ctx: Arc<Ctx>) {
    if s.peer_cred().map_or(true, |c| c.uid() != ctx.uid) { return }
    let id = {
        let mut sh = ctx.shared.lock();
        if sh.clients.len() >= MAX_CLIENTS { return }
        sh.next += 1;
        let id = sh.next;
        sh.clients.insert(id, Client::default());
        id
    };
    talk(&mut s, &mut stop, &ctx, id).await;
    let mut sh = ctx.shared.lock();
    if sh.clients.remove(&id).is_some() { sh.publish() }
}
async fn bind_first(dir: &Path) -> Option<(UnixListener, PathBuf)> {
    for i in 0..PIPE_TRIES {
        let path = pipe_path(dir, i);
        if let Ok(meta) = std::fs::metadata(&path) {
            if !meta.file_type().is_socket() || UnixStream::connect(&path).await.is_ok() || std::fs::remove_file(&path).is_err() { continue }
        }
        if let Ok(listener) = UnixListener::bind(&path) {
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            return Some((listener, path));
        }
    }
    None
}
impl Hub {
    pub fn path(&self) -> Option<PathBuf> { self.run.lock().as_ref().map(|r| r.path.clone()) }
    pub fn current(&self) -> Current { pick_current(self.shared.lock().clients.values()) }
    pub async fn start(&self, dir: &Path, lookup: Lookup, on_change: OnChange) -> Option<PathBuf> {
        self.wanted.store(true, SeqCst);
        if self.run.lock().is_some() || self.starting.swap(true, SeqCst) { return self.path() }
        let found = bind_first(dir).await;
        self.starting.store(false, SeqCst);
        let (listener, path) = found?;
        let uid = std::fs::metadata(&path).ok().map(|m| m.uid());
        let (Some(uid), true) = (uid, self.wanted.load(SeqCst)) else { drop(listener); let _ = std::fs::remove_file(&path); return None };
        let (tx, mut changes) = mpsc::unbounded_channel::<Current>();
        {
            let mut sh = self.shared.lock();
            sh.clients.clear();
            sh.last = serde_json::to_string(&Current::default()).unwrap_or_default();
            sh.tx = Some(tx);
        }
        tokio::spawn(async move { while let Some(c) = changes.recv().await { on_change(c) } });
        let (stop, mut stopped) = watch::channel(false);
        let ctx = Arc::new(Ctx { shared: self.shared.clone(), lookup, uid });
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = stopped.changed() => return,
                    r = listener.accept() => match r { Ok((s, _)) => { tokio::spawn(serve(s, stopped.clone(), ctx.clone())); } Err(_) => return },
                }
            }
        });
        *self.run.lock() = Some(Run { path: path.clone(), stop });
        Some(path)
    }
    pub fn stop(&self) {
        self.wanted.store(false, SeqCst);
        let _ = self.run.lock().take().map(|r| { let _ = r.stop.send(true); let _ = std::fs::remove_file(&r.path); });
        let mut sh = self.shared.lock();
        sh.clients.clear();
        sh.publish();
        sh.tx = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(op: i32, data: Value) -> Vec<u8> { encode_frame(op, &data) }
    fn head(op: i32, len: i32) -> Vec<u8> { op.to_le_bytes().into_iter().chain(len.to_le_bytes()).collect() }
    #[test]
    fn messages_survive_being_split_and_joined_and_junk_is_refused() {
        let all: Vec<u8> = [frame(FRAME, json!({ "cmd": "SET_ACTIVITY", "n": 1 })), frame(PING, json!({ "n": 2 }))].concat();
        let mut reader = FrameReader::default();
        let got: Vec<_> = all.chunks(3).flat_map(|c| reader.push(c).unwrap()).collect();
        assert_eq!(got, vec![(FRAME, json!({ "cmd": "SET_ACTIVITY", "n": 1 })), (PING, json!({ "n": 2 }))]);
        assert_eq!(FrameReader::default().push(&head(FRAME, MAX_FRAME as i32 + 1)).unwrap_err(), "message too large");
        assert_eq!(FrameReader::default().push(&head(FRAME, -1)).unwrap_err(), "message too large");
        assert_eq!(FrameReader::default().push(&head(9, 0)).unwrap_err(), "unknown message kind");
        assert_eq!(FrameReader::default().push(&head(-1, 0)).unwrap_err(), "unknown message kind");
        assert!(FrameReader::default().push(&[head(1, 3), b"{{{".to_vec()].concat()).is_err());
        assert!(FrameReader::default().push(&head(1, 0)).is_err());
        let body = format!("\"{}\"", "x".repeat(MAX_FRAME - 2));
        assert_eq!(FrameReader::default().push(&[head(FRAME, MAX_FRAME as i32), body.into_bytes()].concat()).unwrap().len(), 1);
        assert_eq!(FrameReader::default().push(&head(FRAME, 4)).unwrap(), vec![]);
    }
    #[test]
    fn the_wire_form_is_little_endian_op_then_length_then_json() {
        assert_eq!(encode_frame(PONG, &json!({ "a": 1 })), [4, 0, 0, 0, 7, 0, 0, 0].into_iter().chain(*b"{\"a\":1}").collect::<Vec<u8>>());
    }
    #[test]
    fn text_from_another_program_is_made_plain() {
        assert_eq!(clean_text("  Elden\u{202E} Ring\n\u{0}  ", TEXT_MAX), "Elden Ring");
        assert_eq!(clean_text(&"x".repeat(500), TEXT_MAX).len(), 120);
        assert_eq!(clean_text("a\u{2028}b\u{200B}c\u{E000}d\u{7F}e", TEXT_MAX), "a b c d e");
        assert_eq!(clean_text(&"\u{1F3AE}".repeat(100), TEXT_MAX).chars().count(), 60);
        assert_eq!(clean_value(Some(&json!(42)), TEXT_MAX), "");
        assert_eq!(program_name("C:\\Games\\Heroic.exe"), "Heroic");
        assert_eq!(program_name("/usr/bin/feishin"), "feishin");
        assert_eq!(program_name("/opt/Foo.AppImage"), "Foo");
        assert_eq!(program_name("javaw.exe"), "");
        assert_eq!(program_name("wine64-preloader"), "");
        assert_eq!(program_name("systemd"), "");
        assert_eq!(program_name(""), "");
        assert_eq!(to_millis(Some(&json!(1700000000))), 1700000000000);
        assert_eq!(to_millis(Some(&json!(1700000000123i64))), 1700000000123);
        assert_eq!(to_millis(Some(&json!("1700000000"))), 1700000000000);
        assert_eq!(to_millis(Some(&json!("soon"))), 0);
        assert_eq!(to_millis(Some(&json!(-5))), 0);
        assert_eq!(to_millis(None), 0);
    }
    #[test]
    fn an_announcement_becomes_the_little_haven_shows() {
        let now = 1_800_000_000_000i64;
        let game = to_haven_activity(&json!({ "details": "Limgrave", "state": "Level 12", "timestamps": { "start": now / 1000 - 60 }, "assets": { "large_image": "https://evil.example/x.png" }, "buttons": [{ "label": "Join", "url": "https://evil.example" }] }), "Elden Ring", now).unwrap();
        assert_eq!(game, (Slot::Playing, Act { name: "Elden Ring".into(), details: "Limgrave · Level 12".into(), started_at: now - 60000, duration: 0, app: "Elden Ring".into() }));
        assert!(!format!("{game:?}").contains("evil"));
        let song = to_haven_activity(&json!({ "type": 2, "details": "Track", "state": "Artist", "timestamps": { "start": now - 5000, "end": now + 195000 } }), "feishin", now).unwrap();
        assert_eq!(song, (Slot::Listening, Act { name: "Track".into(), details: "Artist".into(), started_at: now - 5000, duration: 200000, app: "feishin".into() }));
        assert_eq!(to_haven_activity(&json!({ "name": "Own Name", "details": "x" }), "prog", now).unwrap().1.name, "Own Name");
        assert_eq!(to_haven_activity(&json!({ "details": "Only line" }), "", now), Some((Slot::Playing, Act { name: "Only line".into(), details: String::new(), started_at: now, duration: 0, app: String::new() })));
        assert_eq!(to_haven_activity(&json!({ "state": "Just state" }), "", now).unwrap().1.name, "Just state");
        assert_eq!(to_haven_activity(&json!({}), "", now), None);
        assert_eq!(to_haven_activity(&json!(null), "App", now), None);
        assert_eq!(to_haven_activity(&json!([1]), "App", now), None);
        assert_eq!(to_haven_activity(&json!({ "details": "x", "timestamps": { "start": now + 9_000_000_000i64 } }), "App", now).unwrap().1.started_at, now);
        assert_eq!(to_haven_activity(&json!({ "details": "x", "timestamps": { "start": now - 8 * DAY_MS } }), "App", now).unwrap().1.started_at, now);
        assert_eq!(to_haven_activity(&json!({ "type": 2, "details": "T", "timestamps": { "start": now - 1000, "end": now + 2 * DAY_MS } }), "", now).unwrap().1.duration, 0);
        assert_eq!(to_haven_activity(&json!({ "type": 2, "state": "only state" }), "Player", now).unwrap().0, Slot::Playing);
        assert_eq!(to_haven_activity(&json!({ "details": "a".repeat(300), "state": "b".repeat(300) }), "App", now).unwrap().1.details.len(), 120);
    }
    #[test]
    fn the_newest_activity_for_each_slot_is_the_one_shown() {
        let act = |n: &str| Act { name: n.into(), details: String::new(), started_at: 0, duration: 0, app: String::new() };
        let client = |slot, name: &str, at| Client { activity: Some((slot, act(name))), at };
        let clients = [client(Slot::Playing, "Old", 1), client(Slot::Listening, "Song", 2), client(Slot::Playing, "New", 3), Client { activity: None, at: 4 }];
        assert_eq!(pick_current(&clients), Current { playing: Some(act("New")), listening: Some(act("Song")) });
        assert_eq!(pick_current(&Vec::<Client>::new()), Current::default());
        assert_eq!(serde_json::to_string(&Current::default()).unwrap(), r#"{"playing":null,"listening":null}"#);
        assert_eq!(serde_json::to_value(Current { playing: Some(act("G")), listening: None }).unwrap(), json!({ "playing": { "name": "G", "details": "", "startedAt": 0, "duration": 0, "app": "" }, "listening": null }));
    }
    #[test]
    fn the_pipe_is_where_apps_look_for_it() {
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        assert_eq!(pipe_path(&runtime_dir(env(&[("XDG_RUNTIME_DIR", "/run/user/1000"), ("TMPDIR", "/x")]), false), 3), PathBuf::from("/run/user/1000/discord-ipc-3"));
        assert_eq!(pipe_path(&runtime_dir(env(&[("XDG_RUNTIME_DIR", ""), ("TMPDIR", "/var/folders/ab/T/")]), false), 0), PathBuf::from("/var/folders/ab/T/discord-ipc-0"));
        assert_eq!(runtime_dir(env(&[("TMP", "/t1"), ("TEMP", "/t2")]), false), PathBuf::from("/t1"));
        assert_eq!(runtime_dir(env(&[("TEMP", "/t2")]), false), PathBuf::from("/t2"));
        assert_eq!(runtime_dir(env(&[]), false), PathBuf::from("/tmp"));
        assert_eq!(pipe_path(&runtime_dir(env(&[("XDG_RUNTIME_DIR", "/run/user/1000")]), true), 0), PathBuf::from("/run/user/1000/app/com.discordapp.Discord/discord-ipc-0"));
    }
    #[test]
    fn a_handshake_needs_a_numeric_id_and_version_one_and_only_once() {
        assert_eq!(handshake(&json!({ "v": 1, "client_id": "123456789012345678" }), false).as_deref(), Some("123456789012345678"));
        assert_eq!(handshake(&json!({ "client_id": 42 }), false).as_deref(), Some("42"));
        assert_eq!(handshake(&json!({ "v": null, "client_id": "1" }), false).as_deref(), Some("1"));
        assert_eq!(handshake(&json!({ "v": 2, "client_id": "1" }), false), None);
        assert_eq!(handshake(&json!({ "v": 1, "client_id": "1" }), true), None);
        assert_eq!(handshake(&json!({ "v": 1, "client_id": "abc" }), false), None);
        assert_eq!(handshake(&json!({ "v": 1, "client_id": "" }), false), None);
        assert_eq!(handshake(&json!({ "v": 1, "client_id": "1".repeat(33) }), false), None);
        assert_eq!(handshake(&json!({ "v": 1 }), false), None);
        assert_eq!(handshake(&json!(null), false), None);
    }
}
