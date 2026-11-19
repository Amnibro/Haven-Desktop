use std::{env, fs::{self, File, OpenOptions}, io::{Read, Seek, Write}, path::{Path, PathBuf}, sync::OnceLock, thread, time::Duration};
static MARK: OnceLock<File> = OnceLock::new();
fn dir() -> Option<PathBuf> { env::var_os("XDG_STATE_HOME").map(PathBuf::from).or_else(|| env::var_os("HOME").map(|h| Path::new(&h).join(".local/state"))).map(|d| d.join("haven-desktop")) }
fn nvidia() -> bool { Path::new("/proc/driver/nvidia").exists() || fs::read_dir("/sys/class/drm").map(|r| r.flatten().any(|e| fs::read_link(e.path().join("device/driver")).is_ok_and(|p| p.ends_with("nvidia")))).unwrap_or(false) }
fn level(d: &Path) -> u8 { fs::read_to_string(d.join("safe-graphics")).map_or(0, |s| s.trim().parse().unwrap_or(1).min(2)) }
pub fn plan(nvidia: bool, wayland: bool, x11: bool, level: u8) -> Vec<(&'static str, &'static str)> { let on = nvidia && wayland || level > 0; [(on, "__NV_DISABLE_EXPLICIT_SYNC", "1"), (on, "WEBKIT_DISABLE_DMABUF_RENDERER", "1"), (level > 1 && wayland && x11, "GDK_BACKEND", "x11")].into_iter().filter_map(|(o, k, v)| o.then_some((k, v))).collect() }
fn begin(d: &Path) -> Option<(File, bool)> {
    let _ = fs::create_dir_all(d);
    let mut f = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(d.join("starting")).ok()?;
    f.try_lock().ok()?;
    let mut was = String::new();
    let _ = f.read_to_string(&mut was);
    let crashed = was == "run";
    let _ = crashed.then(|| fs::write(d.join("safe-graphics"), (level(d) + 1).min(2).to_string()));
    f.set_len(0).and_then(|_| f.rewind()).and_then(|_| f.write_all(b"run")).ok()?;
    Some((f, crashed))
}
pub fn prepare() {
    let Some(d) = dir() else { return };
    let held = begin(&d);
    let wayland = env::var_os("WAYLAND_DISPLAY").is_some() && env::var("GDK_BACKEND").map_or(true, |b| !b.starts_with("x11"));
    plan(nvidia(), wayland, env::var_os("DISPLAY").is_some(), level(&d)).into_iter().filter(|(k, _)| env::var_os(k).is_none()).for_each(|(k, v)| { eprintln!("haven-desktop: graphics workaround {k}={v}"); env::set_var(k, v) });
    if let Some((f, _)) = held { let _ = MARK.set(f); thread::spawn(|| { thread::sleep(Duration::from_secs(17)); done() }); }
}
pub fn done() { let _ = MARK.get().map(|f| f.set_len(0)); }
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plan_matches_the_known_fixes() {
        let both = [("__NV_DISABLE_EXPLICIT_SYNC", "1"), ("WEBKIT_DISABLE_DMABUF_RENDERER", "1")];
        assert_eq!(plan(true, true, true, 0), both);
        assert!([(true, false), (false, true), (false, false)].iter().all(|&(n, w)| plan(n, w, true, 0).is_empty()));
        assert_eq!(plan(false, true, true, 1), both);
        assert_eq!(plan(false, false, false, 1), both);
        assert_eq!(plan(false, true, true, 2), [both[0], both[1], ("GDK_BACKEND", "x11")]);
        assert_eq!(plan(false, true, false, 2), both, "no X server, as inside the Flatpak on Wayland: stay on Wayland");
    }
    #[test]
    fn a_start_that_never_settled_steps_up_safe_graphics() {
        let d = env::temp_dir().join(format!("haven-gfx-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let (f, c) = begin(&d).unwrap();
        assert!(!c && level(&d) == 0);
        assert!(begin(&d).is_none(), "a second copy while the first runs is not a crash");
        f.set_len(0).unwrap();
        drop(f);
        let (f, c) = begin(&d).unwrap();
        assert!(!c && level(&d) == 0, "a settled start is not a crash");
        drop(f);
        let (f, c) = begin(&d).unwrap();
        assert!(c && level(&d) == 1, "dying before settling is a crash");
        drop(f);
        let (f, c) = begin(&d).unwrap();
        assert!(c && level(&d) == 2, "a second crash steps up");
        drop(f);
        let (_f, c) = begin(&d).unwrap();
        assert!(c && level(&d) == 2, "two is the top step");
        let _ = fs::remove_dir_all(&d);
    }
}
