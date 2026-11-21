use crate::state::{self, AppState};
use once_cell::sync::OnceCell;
use serde_json::json;
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Wry,
};

type Rect = (i32, i32, u32, u32);
const MIN_VISIBLE: u32 = 100;
static KEEP: OnceCell<CheckMenuItem<Wry>> = OnceCell::new();
static BASE: OnceCell<Image<'static>> = OnceCell::new();
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show_i = MenuItem::with_id(app, "show", state::t(app, "tray.show"), true, None::<&str>)?;
    let move_i = MenuItem::with_id(app, "move", state::t(app, "tray.moveToMainScreen"), true, None::<&str>)?;
    let keep_i = CheckMenuItem::with_id(app, "keep", state::t(app, "tray.keepInTray"), true, state::get_bool(app, "minimizeToTray").unwrap_or(false), None::<&str>)?;
    let check_i = MenuItem::with_id(app, "check", state::t(app, "menu.checkForUpdates"), true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", state::t(app, "tray.quit"), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &move_i, &keep_i, &check_i, &quit_i])?;
    let _ = KEEP.set(keep_i);
    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))?;
    let _ = BASE.set(icon.clone());
    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .tooltip(state::t(app, "tray.tooltip"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_window(app, false),
            "move" => show_window(app, true),
            "keep" => { let on = !state::get_bool(app, "minimizeToTray").unwrap_or(false); let _ = state::set_value(app, "minimizeToTray", json!(on)); set_keep_checked(on); }
            "check" => { let app = app.clone(); tauri::async_runtime::spawn(async move { crate::updates::check(&app, true).await }); }
            "quit" => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle(), false);
            }
        })
        .build(app)?;
    Ok(())
}
pub fn set_keep_checked(on: bool) {
    let _ = KEEP.get().map(|k| k.set_checked(on));
}
pub fn set_unread(app: &AppHandle, unread: bool) {
    let (Some(tray), Some(base)) = (app.tray_by_id("main"), BASE.get()) else { return };
    let _ = tray.set_icon(Some(if unread { dotted(base) } else { base.clone() }));
    let _ = tray.set_tooltip(Some(state::t(app, if unread { "tray.tooltipUnread" } else { "tray.tooltip" })));
}
fn blend(px: &mut [u8], c: [f32; 3], a: f32) {
    let da = px[3] as f32 / 255.0;
    let out = a + da * (1.0 - a);
    (0..3).for_each(|k| px[k] = if out > 0.0 { ((c[k] * a + px[k] as f32 * da * (1.0 - a)) / out).round() as u8 } else { 0 });
    px[3] = (out * 255.0).round() as u8;
}
fn dotted(base: &Image<'_>) -> Image<'static> {
    let (w, h) = (base.width(), base.height());
    let mut px = base.rgba().to_vec();
    let r = (w.min(h) as f32 * 0.24).max(3.0);
    let (cx, cy) = (w as f32 - r - 1.0, r + 1.0);
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let i = ((y * w + x) * 4) as usize;
            blend(&mut px[i..i + 4], [255.0, 255.0, 255.0], (r - d + 0.5).clamp(0.0, 1.0));
            blend(&mut px[i..i + 4], [231.0, 76.0, 60.0], (r * 0.72 - d + 0.5).clamp(0.0, 1.0));
        }
    }
    Image::new_owned(px, w, h)
}
pub fn reachable(win: Rect, areas: &[Rect]) -> bool {
    let span = |a: i32, al: u32, b: i32, bl: u32| (a + al as i32).min(b + bl as i32) - a.max(b);
    areas.is_empty() || areas.iter().any(|&(x, y, w, h)| span(win.0, win.2, x, w) >= MIN_VISIBLE.min(win.2).max(1) as i32 && span(win.1, win.3, y, h) >= MIN_VISIBLE.min(win.3).max(1) as i32)
}
pub fn centered(outer: (u32, u32), inner: (u32, u32), area: Rect) -> Rect {
    let frame = (outer.0.saturating_sub(inner.0), outer.1.saturating_sub(inner.1));
    let (w, h) = (inner.0.min(area.2.saturating_sub(frame.0)), inner.1.min(area.3.saturating_sub(frame.1)));
    (area.0 + (area.2.saturating_sub(w + frame.0) / 2) as i32, area.1 + (area.3.saturating_sub(h + frame.1) / 2) as i32, w, h)
}
pub fn show_window(app: &AppHandle, to_main_screen: bool) {
    let Some(win) = app.get_webview_window("main").or_else(|| app.get_webview_window("welcome")) else { return };
    let _ = win.show();
    let _ = win.unminimize();
    let rect = |p: PhysicalPosition<i32>, s: PhysicalSize<u32>| (p.x, p.y, s.width, s.height);
    let areas: Vec<Rect> = win.available_monitors().unwrap_or_default().iter().map(|m| rect(m.work_area().position, m.work_area().size)).collect();
    let lost = !to_main_screen && win.outer_position().ok().zip(win.outer_size().ok()).is_some_and(|(p, s)| !reachable(rect(p, s), &areas));
    if let (true, Ok(Some(primary))) = (to_main_screen || lost, win.primary_monitor()) {
        let _ = win.set_fullscreen(false);
        let _ = win.unmaximize();
        let (outer, inner) = (win.outer_size().unwrap_or_default(), win.inner_size().unwrap_or_default());
        let (x, y, w, h) = centered((outer.width, outer.height), (inner.width, inner.height), rect(primary.work_area().position, primary.work_area().size));
        let _ = ((w, h) != (inner.width, inner.height)).then(|| win.set_size(PhysicalSize::new(w, h)));
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
    let _ = win.set_focus();
}
pub fn quit_app(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.quitting.lock() = true;
    {
        let mut server = state.server.lock();
        let _ = server.stop_server();
    }
    crate::audio::cleanup();
    #[cfg(target_os = "linux")]
    crate::gfx::done();
    app.exit(0);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unread_dot_sits_top_right_and_leaves_the_rest() {
        let d = dotted(&Image::new_owned(vec![0u8; 32 * 32 * 4], 32, 32));
        let at = |x: usize, y: usize| d.rgba()[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4].to_vec();
        assert_eq!(at(23, 8), vec![231, 76, 60, 255]);
        assert_eq!(at(0, 31), vec![0, 0, 0, 0]);
        assert_eq!(at(0, 0), vec![0, 0, 0, 0]);
    }
    #[test]
    fn window_placement() {
        let screens = [(0, 0, 1920, 1040), (1920, 0, 2560, 1400)];
        assert!(reachable((100, 100, 1200, 800), &screens));
        assert!(reachable((4300, 100, 1200, 800), &screens));
        assert!(!reachable((4500, 100, 1200, 800), &screens));
        assert!(!reachable((-3000, 200, 1200, 800), &screens));
        assert!(!reachable((100, 1000, 1200, 800), &screens));
        assert!(reachable((100, 100, 1200, 800), &[]));
        assert_eq!(centered((1216, 839), (1200, 800), (0, 0, 1920, 1040)), (352, 100, 1200, 800));
        assert_eq!(centered((1216, 839), (1200, 800), (1920, 30, 1280, 720)), (1952, 30, 1200, 681));
    }
}
