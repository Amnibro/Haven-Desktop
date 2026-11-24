#![cfg(unix)]
use haven_desktop_lib::app_activity::{encode_frame, system_lookup, Current, FrameReader, Hub, Lookup, CLOSE, FRAME, HANDSHAKE, MAX_FRAME, PING, PONG};
use serde_json::{json, Value};
use std::{os::unix::fs::PermissionsExt, path::{Path, PathBuf}, sync::Arc, time::Duration};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{UnixListener, UnixStream}, sync::mpsc, time::timeout};
static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Rig { hub: Hub, dir: PathBuf, rx: mpsc::UnboundedReceiver<Current> }
impl Drop for Rig { fn drop(&mut self) { self.hub.stop(); let _ = std::fs::remove_dir_all(&self.dir); } }
fn named(name: &'static str) -> Lookup { Arc::new(move |_| Box::pin(async move { name.to_string() })) }
async fn rig(lookup: Lookup) -> Rig {
    let dir = std::env::temp_dir().join(format!("hv{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).unwrap();
    let (tx, rx) = mpsc::unbounded_channel();
    let hub = Hub::default();
    hub.start(&dir, lookup, Arc::new(move |c| { let _ = tx.send(c); })).await.unwrap();
    Rig { hub, dir, rx }
}
struct Client { s: UnixStream, r: FrameReader, queue: Vec<(i32, Value)> }
impl Client {
    async fn open(path: &Path) -> Client { Client { s: UnixStream::connect(path).await.unwrap(), r: FrameReader::default(), queue: vec![] } }
    async fn send(&mut self, op: i32, v: Value) { self.s.write_all(&encode_frame(op, &v)).await.unwrap() }
    async fn next(&mut self) -> Option<(i32, Value)> {
        let mut buf = [0u8; 4096];
        while self.queue.is_empty() {
            let n = timeout(Duration::from_secs(5), self.s.read(&mut buf)).await.expect("timed out").unwrap_or(0);
            if n == 0 { return None }
            self.queue = self.r.push(&buf[..n]).unwrap();
            self.queue.reverse();
        }
        self.queue.pop()
    }
    async fn hello(&mut self, id: &str) -> Value { self.send(HANDSHAKE, json!({ "v": 1, "client_id": id })).await; self.next().await.unwrap().1 }
    async fn set(&mut self, pid: i64, activity: Value, nonce: &str) -> Value { self.send(FRAME, json!({ "cmd": "SET_ACTIVITY", "args": { "pid": pid, "activity": activity }, "nonce": nonce })).await; self.next().await.unwrap().1 }
}
async fn change(rig: &mut Rig) -> Current { timeout(Duration::from_secs(5), rig.rx.recv()).await.expect("no change").unwrap() }
async fn quiet(rig: &mut Rig) { assert!(timeout(Duration::from_millis(300), rig.rx.recv()).await.is_err(), "unexpected change") }
#[tokio::test]
async fn a_game_connects_sets_an_activity_and_it_clears_when_it_leaves() {
    let mut rig = rig(named("Elden Ring.exe")).await;
    let path = rig.hub.path().unwrap();
    assert_eq!(path, rig.dir.join("discord-ipc-0"));
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    let mut c = Client::open(&path).await;
    let ready = c.hello("1234567890").await;
    assert_eq!((ready["cmd"].as_str(), ready["evt"].as_str(), ready["data"]["v"].as_i64()), (Some("DISPATCH"), Some("READY"), Some(1)));
    assert_eq!(ready["data"]["user"]["username"], "haven");
    let reply = c.set(4242, json!({ "details": "Limgrave", "state": "Level 12", "assets": { "large_image": "https://evil.example/a.png" } }), "n1").await;
    assert_eq!((reply["cmd"].as_str(), reply["nonce"].as_str(), reply["data"]["application_id"].as_str(), reply["data"]["name"].as_str()), (Some("SET_ACTIVITY"), Some("n1"), Some("1234567890"), Some("Elden Ring")));
    let now = change(&mut rig).await;
    let game = now.playing.unwrap();
    assert_eq!((game.name.as_str(), game.details.as_str(), game.app.as_str()), ("Elden Ring", "Limgrave · Level 12", "Elden Ring"));
    assert!(now.listening.is_none());
    assert!(!serde_json::to_string(&rig.hub.current()).unwrap().contains("evil"));
    let reply = c.set(4242, Value::Null, "n2").await;
    assert_eq!((reply["nonce"].as_str(), reply["data"].is_null()), (Some("n2"), true));
    assert_eq!(change(&mut rig).await, Current::default());
    c.set(4242, json!({ "details": "Back again" }), "n3").await;
    assert!(change(&mut rig).await.playing.is_some());
    drop(c);
    assert_eq!(change(&mut rig).await, Current::default());
    quiet(&mut rig).await;
}
#[tokio::test]
async fn a_music_player_shows_as_listening_beside_a_game() {
    let mut rig = rig(named("feishin")).await;
    let path = rig.hub.path().unwrap();
    let (mut game, mut music) = (Client::open(&path).await, Client::open(&path).await);
    game.hello("1").await;
    music.hello("2").await;
    game.set(1, json!({ "details": "Boss fight" }), "a").await;
    assert_eq!(change(&mut rig).await.playing.unwrap().name, "feishin");
    music.set(2, json!({ "type": 2, "details": "Song", "state": "Artist", "timestamps": { "start": 1, "end": 4000000000i64 } }), "b").await;
    let both = change(&mut rig).await;
    assert_eq!((both.playing.is_some(), both.listening.as_ref().map(|l| (l.name.as_str(), l.details.as_str()))), (true, Some(("Song", "Artist"))));
    drop(music);
    let left = change(&mut rig).await;
    assert!((left.playing.is_some(), left.listening.is_none()) == (true, true));
}
#[tokio::test]
async fn framing_rules_hold_ping_pong_close_and_refusals() {
    let mut rig = rig(named("x")).await;
    let path = rig.hub.path().unwrap();
    let mut c = Client::open(&path).await;
    c.send(PING, json!({ "n": 7 })).await;
    assert_eq!(c.next().await.unwrap(), (PONG, json!({ "n": 7 })));
    c.hello("5").await;
    c.send(HANDSHAKE, json!({ "v": 1, "client_id": "5" })).await;
    assert!(c.next().await.is_none());
    let mut early = Client::open(&path).await;
    early.send(FRAME, json!({ "cmd": "SET_ACTIVITY", "args": { "activity": { "details": "sneaky" } } })).await;
    early.send(PING, json!({})).await;
    assert_eq!(early.next().await.unwrap().0, PONG);
    quiet(&mut rig).await;
    let mut bad = Client::open(&path).await;
    bad.send(HANDSHAKE, json!({ "v": 1, "client_id": "not-a-number" })).await;
    assert!(bad.next().await.is_none());
    let mut old = Client::open(&path).await;
    old.send(HANDSHAKE, json!({ "v": 2, "client_id": "1" })).await;
    assert!(old.next().await.is_none());
    let mut big = Client::open(&path).await;
    big.s.write_all(&[FRAME.to_le_bytes(), (MAX_FRAME as i32 + 1).to_le_bytes()].concat()).await.unwrap();
    assert!(big.next().await.is_none());
    let mut kind = Client::open(&path).await;
    kind.s.write_all(&[9i32.to_le_bytes(), 0i32.to_le_bytes()].concat()).await.unwrap();
    assert!(kind.next().await.is_none());
    let mut junk = Client::open(&path).await;
    junk.s.write_all(&[FRAME.to_le_bytes(), 3i32.to_le_bytes(), *b"{{{\0"].concat()[..11]).await.unwrap();
    assert!(junk.next().await.is_none());
    let mut closer = Client::open(&path).await;
    closer.hello("9").await;
    closer.set(1, json!({ "details": "Here" }), "z").await;
    assert!(change(&mut rig).await.playing.is_some());
    closer.send(CLOSE, json!({})).await;
    assert!(closer.next().await.is_none());
    assert_eq!(change(&mut rig).await, Current::default());
}
#[tokio::test]
async fn text_is_capped_and_cleaned_and_pids_are_validated() {
    let mut rig = rig(named("/usr/bin/some-game")).await;
    let mut c = Client::open(&rig.hub.path().unwrap()).await;
    c.hello("77").await;
    c.set(-5, json!({ "details": format!("A\u{202E}{}\n\u{0}B", "x".repeat(400)), "state": 12 }), "t").await;
    let a = change(&mut rig).await.playing.unwrap();
    assert_eq!((a.name.chars().count(), a.name.contains('\u{202E}'), a.details.as_str(), a.app.as_str()), (120, false, "", ""));
    c.set(1.5e300 as i64, json!({ "details": "d" }), "u").await;
    assert_eq!(change(&mut rig).await.playing.unwrap().name, "d");
    c.set(321, json!({ "details": "e" }), "v").await;
    let e = change(&mut rig).await.playing.unwrap();
    assert_eq!((e.name.as_str(), e.details.as_str(), e.app.as_str()), ("some-game", "e", "some-game"));
}
#[tokio::test]
async fn a_running_app_on_the_first_slot_is_left_alone_and_stale_files_are_replaced() {
    let dir = std::env::temp_dir().join(format!("hvs{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let theirs = UnixListener::bind(dir.join("discord-ipc-0")).unwrap();
    std::fs::write(dir.join("discord-ipc-1"), b"not a socket").unwrap();
    drop(UnixListener::bind(dir.join("discord-ipc-2")).unwrap());
    let hub = Hub::default();
    let got = hub.start(&dir, system_lookup(), Arc::new(|_| {})).await;
    assert_eq!(got, Some(dir.join("discord-ipc-2")));
    assert_eq!(std::fs::read(dir.join("discord-ipc-1")).unwrap(), b"not a socket");
    assert!(UnixStream::connect(dir.join("discord-ipc-2")).await.is_ok());
    hub.stop();
    assert!(!dir.join("discord-ipc-2").exists());
    assert!(dir.join("discord-ipc-0").exists());
    assert_eq!(hub.path(), None);
    drop(theirs);
    let again = hub.start(&dir, system_lookup(), Arc::new(|_| {})).await;
    assert_eq!(again, Some(dir.join("discord-ipc-0")));
    hub.stop();
    let _ = std::fs::remove_dir_all(&dir);
}
#[tokio::test]
async fn stopping_closes_clients_clears_activity_and_removes_the_socket() {
    let mut rig = rig(named("g")).await;
    let path = rig.hub.path().unwrap();
    let mut c = Client::open(&path).await;
    c.hello("3").await;
    c.set(1, json!({ "details": "Playing" }), "s").await;
    assert!(change(&mut rig).await.playing.is_some());
    rig.hub.stop();
    assert_eq!(change(&mut rig).await, Current::default());
    assert!(c.next().await.is_none());
    assert!(!path.exists());
    assert_eq!(rig.hub.current(), Current::default());
}
#[tokio::test]
async fn at_most_ten_apps_talk_at_once() {
    let rig = rig(named("g")).await;
    let path = rig.hub.path().unwrap();
    let mut held = vec![];
    for i in 0..10 { let mut c = Client::open(&path).await; c.hello(&format!("{}", i + 1)).await; held.push(c) }
    let mut extra = Client::open(&path).await;
    extra.send(HANDSHAKE, json!({ "v": 1, "client_id": "99" })).await;
    assert!(extra.next().await.is_none());
}
#[tokio::test]
async fn the_system_lookup_names_a_real_program_and_skips_sandbox_pids() {
    let lookup = system_lookup();
    assert!(!lookup(i64::from(std::process::id())).await.is_empty());
    assert!(lookup(999_999_999).await.is_empty());
    #[cfg(target_os = "linux")]
    assert!(lookup(1).await.is_empty());
}
