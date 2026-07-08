//! The docked mini-bar: a slim always-on-top borderless webview showing the 5h/7d
//! bars, pinned to a screen corner. Clicking it toggles the main panel.

use tauri::{WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub fn create(app: &tauri::AppHandle, corner: &str) -> tauri::Result<WebviewWindow> {
    let win = WebviewWindowBuilder::new(app, "minibar", WebviewUrl::App("minibar.html".into()))
        .title("TokenHub Mini")
        .inner_size(200.0, 46.0)
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

#[allow(dead_code)]

/// Reparent the mini-bar into the Windows taskbar (Shell_TrayWnd) so it renders
/// on top of it at the bottom-left, instead of being hidden behind it.
#[cfg(windows)]
fn dock_to_taskbar(win: &WebviewWindow) {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetParent, SetWindowPos, HWND_TOP, SWP_NOSIZE, SWP_SHOWWINDOW,
    };
    // Convert Tauri's HWND to this crate's HWND in a version-tolerant way.
    let child = match win.hwnd() {
        Ok(h) => HWND(h.0 as _),
        Err(_) => return,
    };
    unsafe {
        let taskbar = match FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) {
            Ok(h) => h,
            Err(_) => return,
        };
        if taskbar.0.is_null() {
            return;
        }
        let _ = SetParent(child, taskbar);
        // position relative to the taskbar's left edge, over the weather/widgets area
        let _ = SetWindowPos(child, HWND_TOP, 12, 1, 0, 0, SWP_NOSIZE | SWP_SHOWWINDOW);
    }
}
