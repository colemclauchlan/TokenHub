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
    // The bar overlaps the taskbar, which is also a topmost window that keeps
    // re-raising itself — without countermeasures it buries the bar and steals
    // its clicks. Two layers of defense:
    //  1. Make the taskbar the bar's *owner*: Windows keeps owned windows
    //     above their owner in z-order, so the bar rides on top whenever the
    //     taskbar raises itself.
    //  2. Re-assert TOPMOST periodically as belt-and-braces (flicker-free
    //     no-op when already on top); exits once the window is closed.
    #[cfg(windows)]
    keep_above_taskbar(&win);
    let w = win.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        if w.set_always_on_top(true).is_err() {
            break;
        }
        #[cfg(windows)]
        keep_above_taskbar(&w);
    });
    Ok(win)
}

/// Own the mini-bar to the Windows taskbar (Shell_TrayWnd) and push it to the
/// top of the topmost band. Owned windows always render above their owner, so
/// the taskbar can no longer sit on top of (or take clicks from) the bar. The
/// bar stays a normal top-level window — unlike `SetParent`-style embedding,
/// this keeps cross-process click input working.
#[cfg(windows)]
fn keep_above_taskbar(win: &WebviewWindow) {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWLP_HWNDPARENT,
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };
    let bar = match win.hwnd() {
        Ok(h) => HWND(h.0 as _),
        Err(_) => return,
    };
    unsafe {
        if let Ok(taskbar) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) {
            if !taskbar.0.is_null()
                && GetWindowLongPtrW(bar, GWLP_HWNDPARENT) != taskbar.0 as isize
            {
                SetWindowLongPtrW(bar, GWLP_HWNDPARENT, taskbar.0 as isize);
            }
        }
        let _ = SetWindowPos(
            bar,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
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
