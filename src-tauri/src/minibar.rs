//! The docked mini-bar: a slim always-on-top borderless webview showing the 5h/7d
//! bars, pinned to a screen corner. Clicking it toggles the main panel.

use tauri::{WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub fn create(app: &tauri::AppHandle, corner: &str) -> tauri::Result<WebviewWindow> {
    let win = WebviewWindowBuilder::new(app, "minibar", WebviewUrl::App("minibar.html".into()))
        .title("AI Usage Bar mini")
        .inner_size(150.0, 46.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .build()?;
    crate::panel::position_minibar(&win, corner);
    Ok(win)
}
