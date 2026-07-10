//! Positioning for the flyout panel (near the tray, above the taskbar) and the
//! docked mini-bar (a chosen screen corner). Uses monitor geometry from Tauri;
//! the taskbar height is approximated and clamped to the visible work area.

use tauri::{PhysicalPosition, WebviewWindow};

const MARGIN: i32 = 12;
const TASKBAR_APPROX: i32 = 48;

/// Place the panel bottom-right, sitting just above the taskbar (Start-menu style).
pub fn position_panel(win: &WebviewWindow) {
    if let Ok(Some(mon)) = win.current_monitor() {
        let ms = mon.size();
        let mp = mon.position();
        let scale = mon.scale_factor();
        let margin = (MARGIN as f64 * scale) as i32;
        let taskbar = (TASKBAR_APPROX as f64 * scale) as i32;
        let ws = win.outer_size().unwrap_or_default();
        // bottom-left, sitting just above the mini-bar / taskbar
        let x = mp.x + margin;
        let y = mp.y + ms.height as i32 - ws.height as i32 - taskbar - margin;
        let _ = win.set_position(PhysicalPosition::new(x.max(mp.x), y.max(mp.y)));
    }
}

/// Place the mini-bar in a chosen corner (default bottom-left, over the weather slot).
pub fn position_minibar(win: &WebviewWindow, corner: &str) {
    if let Ok(Some(mon)) = win.current_monitor() {
        let ms = mon.size();
        let mp = mon.position();
        let scale = mon.scale_factor();
        let margin = (6.0 * scale) as i32;
        let taskbar = (TASKBAR_APPROX as f64 * scale) as i32;
        let ws = win.outer_size().unwrap_or_default();
        let (w, h) = (ws.width as i32, ws.height as i32);
        let left = mp.x + margin;
        let right = mp.x + ms.width as i32 - w - margin;
        let top = mp.y + margin;
        // overlay the taskbar band, vertically centered in it (the window is
        // always-on-top so it renders over the taskbar; it stays a normal
        // top-level window because reparenting into the Win11 taskbar breaks
        // cross-process click input)
        let bottom = (mp.y + ms.height as i32 - taskbar + ((taskbar - h) / 2).max(0))
            .min(mp.y + ms.height as i32 - h);
        let (x, y) = match corner {
            "bottomRight" => (right, bottom),
            "topLeft" => (left, top),
            "topRight" => (right, top),
            _ => (left, bottom), // bottomLeft (default)
        };
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
}
