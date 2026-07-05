// AI Usage Bar — Windows taskbar tracker for Claude Code & Codex.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod git;
mod minibar;
mod panel;
mod processes;
mod provider;
mod snapshot;
mod tray;

use std::sync::Mutex;
use tauri::{Emitter, Manager};

struct AppState {
    settings: Mutex<config::Settings>,
    snap: Mutex<Option<snapshot::AllSnapshots>>,
}

#[tauri::command]
fn get_snapshot(state: tauri::State<AppState>) -> snapshot::AllSnapshots {
    if let Some(s) = state.snap.lock().unwrap().clone() {
        return s;
    }
    let settings = state.settings.lock().unwrap().clone();
    let s = snapshot::build_all(&settings);
    *state.snap.lock().unwrap() = Some(s.clone());
    s
}

#[tauri::command]
fn get_processes() -> Vec<processes::ProcGroup> {
    processes::list_groups()
}

#[tauri::command]
fn kill_process(pid: u32) -> bool {
    processes::kill(pid)
}

#[tauri::command]
fn get_git() -> git::GitData {
    git::fetch()
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> config::Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(state: tauri::State<AppState>, settings: config::Settings) -> Result<(), String> {
    config::save(&settings).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
fn toggle_panel(app: tauri::AppHandle) {
    toggle(&app);
}

fn toggle(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("panel") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            panel::position_panel(&win);
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

/// Rebuild the snapshot, refresh the tray icon/tooltip, and notify the UI.
fn refresh(app: &tauri::AppHandle) {
    let settings = app.state::<AppState>().settings.lock().unwrap().clone();
    let snap = snapshot::build_all(&settings);

    let five = snap.claude.limits.five_hour.pct;
    let seven = snap.claude.limits.seven_day.pct;
    if let Some(tray) = app.tray_by_id("main") {
        let (buf, w, h) = tray::render_rgba(five, seven);
        let _ = tray.set_icon(Some(tauri::image::Image::new_owned(buf, w, h)));
        let _ = tray.set_tooltip(Some(tray::tooltip(
            five,
            &snap.claude.limits.five_hour.reset_label,
            seven,
            &snap.claude.limits.seven_day.reset_label,
        )));
    }

    *app.state::<AppState>().snap.lock().unwrap() = Some(snap.clone());
    let _ = app.emit("snapshot", &snap);
}

fn main() {
    let settings = config::load();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(Vec::<&str>::new()),
        ))
        .manage(AppState {
            settings: Mutex::new(settings.clone()),
            snap: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_processes,
            kill_process,
            get_git,
            get_settings,
            set_settings,
            toggle_panel
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Dynamic tray icon (starts empty, updated by refresh()).
            let (buf, w, h) = tray::render_rgba(0, 0);
            let _tray = tauri::tray::TrayIconBuilder::with_id("main")
                .icon(tauri::image::Image::new_owned(buf, w, h))
                .tooltip("AI Usage Bar")
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle(tray.app_handle());
                    }
                })
                .build(app)?;

            // Docked mini-bar (optional).
            if settings.minibar_enabled {
                let _ = minibar::create(&handle, &settings.minibar_corner);
            }

            // Global hotkey.
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                if let Ok(shortcut) = settings.hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>()
                {
                    let h = handle.clone();
                    let _ = app
                        .global_shortcut()
                        .on_shortcut(shortcut, move |_app, _sc, _event| toggle(&h));
                }
            }

            // Background refresh loop.
            let h = handle.clone();
            std::thread::spawn(move || loop {
                refresh(&h);
                let secs = h
                    .state::<AppState>()
                    .settings
                    .lock()
                    .unwrap()
                    .refresh_secs
                    .max(2);
                std::thread::sleep(std::time::Duration::from_secs(secs));
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building AI Usage Bar")
        .run(|_app, event| {
            // Keep running in the tray when the window is closed.
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}
