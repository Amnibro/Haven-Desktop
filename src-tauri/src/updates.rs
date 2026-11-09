use crate::state::{self, AppState};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::{Update, UpdaterExt};
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq)]
enum Route { Plugin, OwnCopy, PackageManager }
#[cfg(target_os = "linux")]
fn pick_route(appimage: bool, deb_package: bool, writable: bool) -> Route {
    match (appimage || deb_package, writable) { (true, _) => Route::Plugin, (_, true) => Route::OwnCopy, _ => Route::PackageManager }
}
#[cfg(target_os = "linux")]
fn route() -> Route {
    use tauri::utils::{config::BundleType, platform::{bundle_type, current_exe}};
    let exe = current_exe().unwrap_or_default();
    let dpkg = std::process::Command::new("dpkg").arg("-S").arg(&exe).output().is_ok_and(|o| o.status.success());
    let probe = exe.with_file_name(".haven-update-probe");
    let writable = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    pick_route(std::env::var_os("APPIMAGE").is_some() || bundle_type() == Some(BundleType::AppImage), bundle_type() == Some(BundleType::Deb) && exe.starts_with("/usr") && dpkg, writable)
}
#[cfg(not(target_os = "linux"))]
fn route() -> Route { Route::Plugin }
#[cfg(target_os = "linux")]
fn deb_binary(deb: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let bad = || "the download is not a Haven package".to_string();
    let field = |bytes: &[u8]| std::str::from_utf8(bytes).map(|s| s.trim().trim_end_matches('/').to_string()).map_err(|_| bad());
    let mut at = if deb.starts_with(b"!<arch>\n") { 8 } else { return Err(bad()) };
    while let Some(head) = deb.get(at..at + 60) {
        let size: usize = field(&head[48..58])?.parse().map_err(|_| bad())?;
        let body = deb.get(at + 60..at + 60 + size).ok_or_else(bad)?;
        if field(&head[..16])? == "data.tar.gz" {
            let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(body));
            for entry in archive.entries().map_err(|e| e.to_string())? {
                let mut entry = entry.map_err(|e| e.to_string())?;
                if !entry.path().map_err(|e| e.to_string())?.ends_with("usr/bin/haven-desktop") { continue }
                let mut binary = Vec::new();
                entry.read_to_end(&mut binary).map_err(|e| e.to_string())?;
                return if binary.starts_with(b"\x7fELF") { Ok(binary) } else { Err(bad()) };
            }
            return Err(bad());
        }
        at += 60 + size + size % 2;
    }
    Err(bad())
}
#[cfg(target_os = "linux")]
fn replace_at(exe: &std::path::Path, deb: &[u8], version: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let binary = deb_binary(deb)?;
    let new = exe.with_file_name(".haven-desktop.new");
    let written = std::fs::write(&new, binary).and_then(|_| std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))).and_then(|_| std::fs::rename(&new, exe));
    if let Err(e) = written { let _ = std::fs::remove_file(&new); return Err(e.to_string()) }
    if exe.ends_with("amni-os/haven/haven-desktop") { let _ = std::fs::write(exe.with_file_name("version"), format!("{version}\n")); }
    Ok(())
}
#[cfg(target_os = "linux")]
fn replace_copy(deb: &[u8], version: &str) -> Result<(), String> { tauri::utils::platform::current_exe().map_err(|e| e.to_string()).and_then(|exe| replace_at(&exe, deb, version)) }
#[cfg(not(target_os = "linux"))]
fn replace_copy(_: &[u8], _: &str) -> Result<(), String> { Err(String::new()) }
#[derive(Default)]
pub struct UpdateState { found: Option<Update>, bytes: Option<Vec<u8>>, dismissed: bool, downloading: bool }
impl UpdateState {
    fn available(&mut self, update: Update, asked: bool) -> bool {
        let fresh = self.found.as_ref().map(|f| f.version != update.version).unwrap_or(true);
        if fresh { *self = UpdateState { found: Some(update), ..Default::default() }; return true }
        self.dismissed = self.dismissed && !asked;
        self.bytes.is_none() && !self.dismissed
    }
    fn for_page(&self) -> Value {
        match &self.found { Some(f) if !self.dismissed => json!({ "status": if self.bytes.is_some() { "downloaded" } else { "available" }, "version": f.version }), _ => Value::Null }
    }
    fn should_check(&self) -> bool { !self.downloading && self.bytes.is_none() }
}
fn with<T>(app: &AppHandle, f: impl FnOnce(&mut UpdateState) -> T) -> T { let st = app.state::<AppState>(); let mut s = st.update.lock(); f(&mut s) }
fn flatpak() -> bool { std::env::var_os("FLATPAK_ID").is_some() || std::path::Path::new("/.flatpak-info").exists() }
fn unsupported() -> bool { flatpak() || route() == Route::PackageManager }
fn tell(app: &AppHandle, event: &str, payload: Value) { let _ = app.emit_to("main", event, payload); }
fn notice(app: &AppHandle, kind: MessageDialogKind, message: String) {
    app.dialog().message(message).title(state::t(app, "menu.checkForUpdates")).kind(kind).buttons(MessageDialogButtons::OkCustom(state::t(app, "dialog.ok"))).show(|_| {});
}
pub fn start(app: AppHandle) {
    if unsupported() { return }
    tauri::async_runtime::spawn(async move {
        loop {
            let due = with(&app, |s| s.should_check());
            if due { check(&app, false).await }
            if cfg!(debug_assertions) { break }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}
pub async fn check(app: &AppHandle, asked: bool) {
    if asked && (unsupported() || cfg!(debug_assertions)) { return notice(app, MessageDialogKind::Info, state::t(app, if flatpak() { "update.unavailable" } else if unsupported() { "update.packageManager" } else { "update.notPackaged" })) }
    let updater = if route() == Route::OwnCopy { app.updater_builder().target(format!("linux-{}-deb", std::env::consts::ARCH)).build() } else { app.updater() };
    let result = match updater { Ok(u) => u.check().await.map_err(|e| e.to_string()), Err(e) => Err(e.to_string()) };
    match result {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let (show, ready) = with(app, |s| (s.available(update, asked), s.bytes.is_some()));
            if show { tell(app, "update:available", json!({ "version": version })) } else if asked && ready { tell(app, "update:downloaded", Value::Null) }
            let main = app.get_webview_window("main").filter(|m| m.is_visible().unwrap_or(false));
            match (asked, main) {
                (true, Some(m)) => { let _ = m.unminimize(); let _ = m.set_focus(); }
                (true, None) => offer_install(app.clone(), version),
                _ => {}
            }
        }
        Ok(None) if asked => notice(app, MessageDialogKind::Info, state::tv(app, "update.upToDate", &[("version", &app.package_info().version.to_string())])),
        Err(e) if asked => notice(app, MessageDialogKind::Error, state::tv(app, "update.error", &[("error", &e)])),
        _ => {}
    }
}
fn offer_install(app: AppHandle, version: String) {
    let msg = state::tv(&app, "update.available", &[("version", &version)]);
    let worker = app.clone();
    app.dialog().message(msg).title(state::t(&app, "menu.checkForUpdates")).buttons(MessageDialogButtons::OkCancelCustom(state::t(&app, "update.now"), state::t(&app, "update.later"))).show(move |yes| {
        let _ = yes.then(|| tauri::async_runtime::spawn(async move { if download(&worker).await.is_ok() { install(&worker) } }));
    });
}
async fn download(app: &AppHandle) -> Result<(), String> {
    let (ready, update) = with(app, |s| { s.downloading = s.bytes.is_none() && s.found.is_some(); (s.bytes.is_some(), s.found.clone()) });
    if ready { return Ok(()) }
    let Some(update) = update else { return Err(state::t(app, "update.unavailable")) };
    let (mut got, mut shown) = (0usize, -1i64);
    let result = update.download(|chunk, total| {
        got += chunk;
        let percent = total.filter(|t| *t > 0).map(|t| (got as f64 * 100.0 / t as f64).round() as i64).unwrap_or(-1);
        let _ = (percent >= 0 && percent != shown).then(|| { shown = percent; tell(app, "update:download-progress", json!({ "percent": percent })) });
    }, || {}).await.map_err(|e| e.to_string());
    let ok = with(app, |s| { s.downloading = false; result.map(|bytes| { s.bytes = Some(bytes); s.dismissed = false; }) });
    match &ok { Ok(()) => tell(app, "update:downloaded", Value::Null), Err(e) => tell(app, "update:error", json!({ "message": e })) }
    ok
}
fn install(app: &AppHandle) {
    let (update, bytes) = with(app, |s| (s.found.clone(), s.bytes.clone()));
    let (Some(update), Some(bytes)) = (update, bytes) else { return };
    let _ = app.state::<AppState>().server.lock().stop_server();
    let result = if route() == Route::OwnCopy { replace_copy(&bytes, &update.version) } else { update.install(bytes).map_err(|e| e.to_string()) };
    match result { Ok(()) => app.restart(), Err(e) => tell(app, "update:error", json!({ "message": state::tv(app, "update.installFailed", &[("error", &e)]) })) }
}
#[tauri::command]
pub fn update_state(app: AppHandle) -> Value {
    let page = with(&app, |s| s.for_page());
    let labels: serde_json::Map<String, Value> = ["available", "now", "downloading", "downloadingProgress", "downloaded", "restartNow", "failed", "error", "close"].iter().map(|k| (k.to_string(), json!(state::t(&app, &format!("update.{k}"))))).collect();
    json!({ "state": page, "labels": labels })
}
#[tauri::command]
pub async fn update_download(app: AppHandle) -> Value {
    match download(&app).await { Ok(()) => json!({ "success": true }), Err(e) => json!({ "error": e }) }
}
#[tauri::command]
pub fn update_install(app: AppHandle) { install(&app) }
#[tauri::command]
pub fn update_dismiss(app: AppHandle) { with(&app, |s| s.dismissed = s.found.is_some()) }
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    const ELF: &[u8] = b"\x7fELF new build";
    fn tar_gz(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (path, body) in files {
            let mut head = tar::Header::new_gnu();
            head.set_size(body.len() as u64);
            head.set_mode(0o755);
            head.set_cksum();
            out.append_data(&mut head, path, *body).unwrap();
        }
        out.into_inner().unwrap().finish().unwrap()
    }
    fn ar(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = b"!<arch>\n".to_vec();
        for (name, body) in members {
            out.extend(format!("{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n", name, 0, 0, 0, 100644, body.len()).into_bytes());
            out.extend_from_slice(body);
            if body.len() % 2 == 1 { out.push(b'\n') }
        }
        out
    }
    fn deb(data_name: &str, files: &[(&str, &[u8])]) -> Vec<u8> { ar(&[("debian-binary", b"2.0\n"), ("control.tar.gz", b"odd"), (data_name, &tar_gz(files))]) }
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("haven-update-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
    #[test]
    fn route_follows_the_install() {
        assert_eq!(pick_route(true, false, true), Route::Plugin);
        assert_eq!(pick_route(true, false, false), Route::Plugin);
        assert_eq!(pick_route(false, true, false), Route::Plugin);
        assert_eq!(pick_route(false, false, true), Route::OwnCopy);
        assert_eq!(pick_route(false, false, false), Route::PackageManager);
    }
    #[test]
    fn finds_the_binary_in_a_deb() {
        let files: &[(&str, &[u8])] = &[("usr/share/applications/haven.desktop", b"x"), ("usr/bin/haven-desktop", ELF)];
        assert_eq!(deb_binary(&deb("data.tar.gz", files)).unwrap(), ELF);
        assert_eq!(deb_binary(&deb("data.tar.gz/", files)).unwrap(), ELF);
        assert_eq!(deb_binary(&deb("data.tar.gz", &[("./usr/bin/haven-desktop", ELF)])).unwrap(), ELF);
    }
    #[test]
    fn refuses_anything_else() {
        let good = deb("data.tar.gz", &[("usr/bin/haven-desktop", ELF)]);
        assert!(deb_binary(b"\x7fELF an AppImage").is_err());
        assert!(deb_binary(b"").is_err());
        assert!(deb_binary(&good[..good.len() - 7]).is_err());
        assert!(deb_binary(&deb("data.tar.xz", &[("usr/bin/haven-desktop", ELF)])).is_err());
        assert!(deb_binary(&deb("data.tar.gz", &[("usr/bin/other", ELF)])).is_err());
        assert!(deb_binary(&deb("data.tar.gz", &[("usr/bin/haven-desktop", b"#!/bin/sh")])).is_err());
    }
    #[test]
    fn replaces_the_amni_os_copy_and_its_version() {
        let dir = scratch("amni").join("amni-os/haven");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("haven-desktop");
        std::fs::write(&exe, b"old").unwrap();
        std::fs::write(dir.join("version"), "2.7.0\n").unwrap();
        replace_at(&exe, &deb("data.tar.gz", &[("usr/bin/haven-desktop", ELF)]), "2.7.1").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), ELF);
        assert_eq!(std::fs::metadata(&exe).unwrap().permissions().mode() & 0o777, 0o755);
        assert_eq!(std::fs::read_to_string(dir.join("version")).unwrap(), "2.7.1\n");
        assert!(!dir.join(".haven-desktop.new").exists());
    }
    #[test]
    fn leaves_other_files_alone() {
        let dir = scratch("other");
        let exe = dir.join("haven-desktop");
        std::fs::write(&exe, b"old").unwrap();
        std::fs::write(dir.join("version"), "mine\n").unwrap();
        replace_at(&exe, &deb("data.tar.gz", &[("usr/bin/haven-desktop", ELF)]), "2.7.1").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), ELF);
        assert_eq!(std::fs::read_to_string(dir.join("version")).unwrap(), "mine\n");
    }
    #[test]
    fn a_bad_download_keeps_the_old_copy() {
        let dir = scratch("bad").join("amni-os/haven");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("haven-desktop");
        std::fs::write(&exe, b"old").unwrap();
        std::fs::write(dir.join("version"), "2.7.0\n").unwrap();
        assert!(replace_at(&exe, b"\x7fELF an AppImage", "2.7.1").is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"old");
        assert_eq!(std::fs::read_to_string(dir.join("version")).unwrap(), "2.7.0\n");
        assert!(!dir.join(".haven-desktop.new").exists());
    }
}
