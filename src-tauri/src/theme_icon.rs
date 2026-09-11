//! Themed app icon and title bar, the way the mobile app does it.
//!
//! Amni-Haven ships one launcher icon per theme: a solid background in the
//! theme's base color and the hexagon-plus-H glyph stroked in its accent
//! (res/drawable/ic_launcher_{background,foreground}_<theme>.xml). The table
//! below is those 35 color pairs. The desktop renders the same vector at
//! runtime, sets it as the window and tray icon, and on Windows paints the
//! native caption bar with the same pair so minimize / maximize / close sit
//! on the theme instead of on a white strip.
use serde_json::json;
use tauri::{image::Image, AppHandle, Manager};
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// (theme id as the web app writes it in data-theme, background, accent)
const TABLE: [(&str, u32, u32); 35] = [
    ("abyss", 0x040810, 0x1a5aff),
    ("amni", 0x0a0a0a, 0x00ff9d),
    ("amni-ai", 0x0a0a0a, 0xffb74d),
    ("amni-calc", 0x0a0a0a, 0xff6b35),
    ("amni-core", 0x0a0a0a, 0xef5350),
    ("amni-crypt", 0x0a0a0a, 0x2979ff),
    ("amni-explore", 0x0a0a0a, 0x00b4ff),
    ("amni-haven", 0x0a0a0a, 0x7c5cfc),
    ("amni-light", 0xf4f4f4, 0x00875a),
    ("bloodborne", 0x0a0000, 0xcc0020),
    ("braid", 0x0b0d12, 0x00ff9d),
    ("braid-light", 0xf7f7f8, 0x00875a),
    ("chapel", 0x0e0c18, 0x8a5cf0),
    ("crt", 0x080808, 0xffb000),
    ("cyberpunk", 0x0a0a12, 0xff2d6f),
    ("darksouls", 0x0c0a08, 0xc8a84e),
    ("discord", 0x313338, 0x5865f2),
    ("dracula", 0x1a1b26, 0xbd93f9),
    ("eldenring", 0x08080e, 0xd4a832),
    ("fallout", 0x0a0e08, 0x14fe17),
    ("ffx", 0x080e1a, 0x40a8e0),
    ("gospel", 0xf5f0e8, 0x8b2032),
    ("halo", 0x0d1a0d, 0xd4a020),
    ("haven", 0x191b28, 0x7c5cfc),
    ("holo", 0x000000, 0x33b5e5),
    ("ice", 0x0a1520, 0x00d4ff),
    ("lotr", 0x1a1510, 0xd4a844),
    ("matrix", 0x0a0a0a, 0x00ff41),
    ("midnightpurple", 0x0d0a1a, 0x8a5cf6),
    ("minecraft", 0x1a1a1a, 0x55bb55),
    ("nord", 0x2e3440, 0x88c0d0),
    ("scripture", 0x1a1610, 0xc8a84c),
    ("tron", 0x0c141f, 0x6fecff),
    ("win95", 0xbfbfbf, 0x000080),
    ("zelda", 0x0a120a, 0x44bb44),
];

pub fn lookup(theme: &str) -> Option<(u32, u32)> {
    let key = theme.trim().to_ascii_lowercase().replace('_', "-");
    TABLE.iter().find(|(id, _, _)| *id == key).map(|(_, bg, fg)| (*bg, *fg))
}

/// "#rrggbb", "#rgb", "rgb(r, g, b)" or "rgba(...)" -> 0xRRGGBB
pub fn parse_css_color(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return match hex.len() {
            6 | 8 => u32::from_str_radix(&hex[..6], 16).ok(),
            3 | 4 => {
                let v = u32::from_str_radix(&hex[..3], 16).ok()?;
                let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
                Some((r * 0x11) << 16 | (g * 0x11) << 8 | (b * 0x11))
            }
            _ => None,
        };
    }
    let inner = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb("))?.strip_suffix(')')?;
    let mut it = inner.split(|c| c == ',' || c == ' ' || c == '/').filter(|p| !p.is_empty());
    let mut ch = || it.next()?.trim().parse::<f32>().ok().map(|v| v.round().clamp(0.0, 255.0) as u32);
    let (r, g, b) = (ch()?, ch()?, ch()?);
    Some(r << 16 | g << 8 | b)
}

fn rgb(c: u32) -> (u8, u8, u8) {
    (((c >> 16) & 0xff) as u8, ((c >> 8) & 0xff) as u8, (c & 0xff) as u8)
}

fn luminance(c: u32) -> f32 {
    let (r, g, b) = rgb(c);
    (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0
}

/// Taskbar tiles are 16–32px. A light plate reads as a blank page on the
/// Windows taskbar (and on a dark tray). Use the accent as the plate.
fn taskbar_colors(bg: u32, fg: u32) -> (u32, u32) {
    if luminance(bg) <= 0.72 {
        return (bg, fg);
    }
    if luminance(fg) > 0.72 {
        (0x191b28, 0x7c5cfc)
    } else {
        (fg, 0xffffff)
    }
}

/// Same geometry as ic_launcher_foreground_*.xml on a 108 dp viewport, on a
/// rounded square instead of Android's launcher mask.
pub fn render(bg: u32, fg: u32, size: u32) -> Image<'static> {
    let mut pm = Pixmap::new(size, size).expect("pixmap");
    let s = size as f32 / 108.0;
    let t = Transform::from_scale(s, s);
    let mut paint = Paint::default();
    paint.anti_alias = true;
    let (r, g, b) = rgb(bg);
    paint.set_color_rgba8(r, g, b, 255);
    let radius = 24.0;
    let mut pb = PathBuilder::new();
    pb.move_to(radius, 0.0);
    pb.line_to(108.0 - radius, 0.0);
    pb.quad_to(108.0, 0.0, 108.0, radius);
    pb.line_to(108.0, 108.0 - radius);
    pb.quad_to(108.0, 108.0, 108.0 - radius, 108.0);
    pb.line_to(radius, 108.0);
    pb.quad_to(0.0, 108.0, 0.0, 108.0 - radius);
    pb.line_to(0.0, radius);
    pb.quad_to(0.0, 0.0, radius, 0.0);
    pb.close();
    if let Some(p) = pb.finish() {
        pm.fill_path(&p, &paint, FillRule::Winding, t, None);
    }
    let (r, g, b) = rgb(fg);
    paint.set_color_rgba8(r, g, b, 255);
    let mut hex = PathBuilder::new();
    hex.move_to(54.0, 26.0);
    hex.line_to(78.0, 40.0);
    hex.line_to(78.0, 68.0);
    hex.line_to(54.0, 82.0);
    hex.line_to(30.0, 68.0);
    hex.line_to(30.0, 40.0);
    hex.close();
    let stroke = Stroke { width: 4.0, line_join: LineJoin::Round, line_cap: LineCap::Round, ..Stroke::default() };
    if let Some(p) = hex.finish() {
        pm.stroke_path(&p, &paint, &stroke, t, None);
    }
    let mut h = PathBuilder::new();
    h.move_to(45.0, 44.0);
    h.line_to(45.0, 64.0);
    h.move_to(45.0, 54.0);
    h.line_to(63.0, 54.0);
    h.move_to(63.0, 44.0);
    h.line_to(63.0, 64.0);
    let stroke = Stroke { width: 5.0, ..stroke };
    if let Some(p) = h.finish() {
        pm.stroke_path(&p, &paint, &stroke, t, None);
    }
    let rgba: Vec<u8> = pm
        .pixels()
        .iter()
        .flat_map(|px| {
            let c = px.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    Image::new_owned(rgba, size, size)
}

#[cfg(windows)]
fn paint_caption(window: &tauri::WebviewWindow, bg: u32, fg: u32) {
    use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR};
    let Ok(hwnd) = window.hwnd() else { return };
    // COLORREF is 0x00BBGGRR
    let colorref = |c: u32| ((c & 0xff) << 16) | (c & 0xff00) | ((c >> 16) & 0xff);
    for (attr, value) in [(DWMWA_CAPTION_COLOR, colorref(bg)), (DWMWA_BORDER_COLOR, colorref(bg)), (DWMWA_TEXT_COLOR, colorref(fg))] {
        unsafe {
            DwmSetWindowAttribute(hwnd.0 as _, attr as u32, &value as *const u32 as *const _, 4);
        }
    }
}

#[cfg(not(windows))]
fn paint_caption(_window: &tauri::WebviewWindow, _bg: u32, _fg: u32) {}

/// Apply a color pair everywhere it shows: window icons, tray, caption bar.
pub fn apply(app: &AppHandle, bg: u32, fg: u32) {
    let (ibg, ifg) = taskbar_colors(bg, fg);
    let icon = render(ibg, ifg, 256);
    for label in ["main", "welcome"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_icon(icon.clone());
            paint_caption(&w, bg, fg);
        }
    }
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_icon(Some(render(ibg, ifg, 64)));
    }
}

/// Colors remembered from the last page, so the first frame of the next
/// launch is already on the theme.
pub fn stored(app: &AppHandle) -> (u32, u32) {
    let v = crate::state::get_value(app, "themeColors").unwrap_or_default();
    let bg = v.get("bg").and_then(|x| x.as_u64()).map(|x| x as u32);
    let fg = v.get("fg").and_then(|x| x.as_u64()).map(|x| x as u32);
    match (bg, fg) {
        (Some(b), Some(f)) => (b, f),
        _ => lookup("haven").unwrap(),
    }
}

pub fn remember(app: &AppHandle, bg: u32, fg: u32) {
    let _ = crate::state::set_value(app, "themeColors", json!({ "bg": bg, "fg": fg }));
}

/// From the injected bridge: the page's data-theme plus its live --bg-primary
/// and --accent. Known theme ids use the mobile table so both apps match
/// pixel for pixel; file themes and custom themes fall back to the live colors.
#[tauri::command]
pub fn theme_colors(app: AppHandle, theme: Option<String>, bg: Option<String>, accent: Option<String>) -> Result<(), String> {
    let from_table = theme.as_deref().and_then(lookup);
    let live = match (bg.as_deref().and_then(parse_css_color), accent.as_deref().and_then(parse_css_color)) {
        (Some(b), Some(f)) => Some((b, f)),
        _ => None,
    };
    let Some((bg, fg)) = from_table.or(live) else { return Ok(()) };
    if stored(&app) != (bg, fg) {
        remember(&app, bg, fg);
    }
    apply(&app, bg, fg);
    Ok(())
}
