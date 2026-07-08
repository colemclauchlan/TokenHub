// AI Usage Bar — Windows taskbar tracker for Claude Code & Codex.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod git;
mod github;
mod minibar;
mod panel;
mod processes;
mod provider;
mod secrets;
mod sessions;
mod snapshot;
mod tray;

use std::sync::Mutex;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

struct AppState {
    settings: Mutex<config::Settings>,
    snap: Mutex<Option<snapshot::AllSnapshots>>,
    /// Highest usage threshold already notified per "provider:window" key.
    notified: Mutex<std::collections::HashMap<String, u32>>,
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
fn get_processes(provider: String) -> Vec<processes::ProcGroup> {
    processes::list_groups(&provider)
}

#[tauri::command]
fn focus_process(pid: u32) -> bool {
    processes::focus(pid)
}

#[tauri::command]
fn get_windows() -> Vec<processes::WindowInfo> {
    processes::list_windows()
}

#[tauri::command]
fn focus_window(hwnd: i64) -> bool {
    processes::focus_hwnd(hwnd)
}

#[tauri::command]
fn delete_session(provider: String, id: String) -> bool {
    let root = if provider == "codex" {
        usage_core::logs_codex::codex_dir().map(|d| d.join("sessions"))
    } else {
        usage_core::logs_claude::claude_dir().map(|d| d.join("projects"))
    };
    let Some(root) = root else { return false };
    let Some(path) = find_by_stem(&root, &id) else { return false };
    let Some(home) = usage_core::logs_claude::home_dir() else { return false };
    let trash = home.join(".tokenhub-trash");
    let _ = std::fs::create_dir_all(&trash);
    let fname = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    std::fs::rename(&path, trash.join(fname)).is_ok()
}

fn find_by_stem(dir: &std::path::Path, stem: &str) -> Option<std::path::PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = find_by_stem(&p, stem) {
                return Some(f);
            }
        } else if p.file_stem().and_then(|s| s.to_str()) == Some(stem) {
            return Some(p);
        }
    }
    None
}

#[tauri::command]
fn kill_process(pid: u32) -> bool {
    processes::kill(pid)
}

#[tauri::command]
fn open_url(url: String) {
    // Only allow http(s) URLs; open in the default browser. `explorer` receives
    // the URL as a plain argument (no shell), so cmd metacharacters like `&`
    // can't be interpreted — which also makes URLs with query strings work.
    if url.starts_with("http://") || url.starts_with("https://") {
        let _ = std::process::Command::new("explorer").arg(&url).spawn();
    }
}

#[tauri::command]
fn open_folder(path: String) {
    if !path.is_empty() {
        let _ = std::process::Command::new("explorer").arg(&path).spawn();
    }
}

/// Resume a chat/session in its client (opens a new terminal in the project dir).
#[tauri::command]
fn open_chat(provider: String, id: String, cwd: String) {
    let dir = if cwd.is_empty() {
        std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into())
    } else {
        cwd
    };
    let mut args: Vec<String> = vec![
        "/C".into(), "start".into(), "TokenHub Chat".into(), "cmd".into(), "/K".into(),
    ];
    if provider == "codex" {
        args.extend(["codex".into(), "resume".into(), id]);
    } else {
        args.extend(["claude".into(), "--resume".into(), id]);
    }
    let _ = std::process::Command::new("cmd").current_dir(&dir).args(&args).spawn();
}

/// Bring the already-open window for a chat to the front (no new window). Returns
/// false if no matching window is open.
#[tauri::command]
fn focus_chat(provider: String, cwd: String) -> bool {
    processes::focus_chat_window(&provider, &cwd)
}

/// Unique project working directories seen across sessions.
#[tauri::command]
fn get_project_dirs() -> Vec<String> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for s in sessions::claude_sessions().into_iter().chain(sessions::codex_sessions()) {
        if !s.cwd.is_empty() && seen.insert(s.cwd.clone()) {
            out.push(s.cwd);
        }
    }
    out.sort();
    out.truncate(50);
    out
}

#[tauri::command]
fn get_aliases(state: tauri::State<AppState>) -> serde_json::Value {
    let s = state.settings.lock().unwrap();
    serde_json::json!({ "sessions": s.session_aliases, "projects": s.project_aliases })
}

#[tauri::command]
fn set_session_alias(state: tauri::State<AppState>, id: String, name: String) {
    let snap = {
        let mut s = state.settings.lock().unwrap();
        if name.trim().is_empty() {
            s.session_aliases.remove(&id);
        } else {
            s.session_aliases.insert(id, name);
        }
        s.clone()
    };
    let _ = config::save(&snap);
}

#[tauri::command]
fn set_project_alias(state: tauri::State<AppState>, key: String, name: String) {
    let snap = {
        let mut s = state.settings.lock().unwrap();
        if name.trim().is_empty() {
            s.project_aliases.remove(&key);
        } else {
            s.project_aliases.insert(key, name);
        }
        s.clone()
    };
    let _ = config::save(&snap);
}

#[tauri::command]
fn get_git() -> git::GitData {
    git::dashboard()
}

#[tauri::command]
fn get_sessions(provider: String) -> Vec<sessions::SessionInfo> {
    if provider == "codex" {
        sessions::codex_sessions()
    } else {
        sessions::claude_sessions()
    }
}

/// Agents inside a chat (main agent + sub-agents), with model/status/goal.
#[tauri::command]
fn get_session_agents(provider: String, id: String) -> Vec<sessions::AgentInfo> {
    sessions::session_agents(&provider, &id)
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> config::Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    settings: config::Settings,
) -> Result<(), String> {
    config::save(&settings).map_err(|e| e.to_string())?;
    let old = state.settings.lock().unwrap().clone();
    let mgr = app.autolaunch();
    let _ = if settings.autostart { mgr.enable() } else { mgr.disable() };

    // Live-apply mini-bar enable/disable/corner (used to need an app restart).
    if settings.minibar_enabled {
        match app.get_webview_window("minibar") {
            Some(w) => {
                if old.minibar_corner != settings.minibar_corner {
                    panel::position_minibar(&w, &settings.minibar_corner);
                }
            }
            None => {
                let _ = minibar::create(&app, &settings.minibar_corner);
            }
        }
    } else if let Some(w) = app.get_webview_window("minibar") {
        let _ = w.close();
    }

    // Live-apply a hotkey change (best effort; invalid strings are ignored).
    if old.hotkey != settings.hotkey {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        if let Ok(sc) = old.hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = app.global_shortcut().unregister(sc);
        }
        if let Ok(sc) = settings.hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let h = app.clone();
            let _ = app
                .global_shortcut()
                .on_shortcut(sc, move |_app, _sc, _event| toggle(&h));
        }
    }

    let display_changed = old.tray_style != settings.tray_style
        || old.tray_color != settings.tray_color
        || old.pct_remaining != settings.pct_remaining
        || old.track_claude != settings.track_claude
        || old.track_codex != settings.track_codex
        || old.use_provider_api != settings.use_provider_api;
    *state.settings.lock().unwrap() = settings;
    if display_changed {
        refresh_async(&app); // tray + panel reflect the change promptly
    }
    Ok(())
}

/// Snapshot the current display settings into a named profile (replacing any
/// existing profile with the same name) and mark it active.
#[tauri::command]
fn save_profile(state: tauri::State<AppState>, name: String) -> Result<(), String> {
    let name: String = name.trim().chars().take(40).collect();
    if name.is_empty() {
        return Err("profile name is empty".into());
    }
    let snap = {
        let mut s = state.settings.lock().unwrap();
        let p = config::Profile {
            name: name.clone(),
            track_claude: s.track_claude,
            track_codex: s.track_codex,
            tray_style: s.tray_style.clone(),
            tray_color: s.tray_color.clone(),
            pct_remaining: s.pct_remaining,
        };
        s.profiles.retain(|q| q.name != name);
        s.profiles.push(p);
        s.active_profile = name;
        s.clone()
    };
    config::save(&snap).map_err(|e| e.to_string())
}

/// Apply a saved profile's display settings onto the live settings.
#[tauri::command]
fn switch_profile(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    name: String,
) -> Result<(), String> {
    let snap = {
        let mut s = state.settings.lock().unwrap();
        let Some(p) = s.profiles.iter().find(|q| q.name == name).cloned() else {
            return Err(format!("no profile named \"{name}\""));
        };
        s.track_claude = p.track_claude;
        s.track_codex = p.track_codex;
        if !p.tray_style.is_empty() {
            s.tray_style = p.tray_style;
        }
        if !p.tray_color.is_empty() {
            s.tray_color = p.tray_color;
        }
        s.pct_remaining = p.pct_remaining;
        s.active_profile = name;
        s.clone()
    };
    config::save(&snap).map_err(|e| e.to_string())?;
    refresh_async(&app); // tray icon + % mode update right away
    Ok(())
}

/// Remove a saved profile by name (clears active_profile if it matched).
#[tauri::command]
fn delete_profile(state: tauri::State<AppState>, name: String) -> Result<(), String> {
    let snap = {
        let mut s = state.settings.lock().unwrap();
        s.profiles.retain(|q| q.name != name);
        if s.active_profile == name {
            s.active_profile = String::new();
        }
        s.clone()
    };
    config::save(&snap).map_err(|e| e.to_string())
}

#[tauri::command]
fn toggle_panel(app: tauri::AppHandle) {
    toggle(&app);
}

#[tauri::command]
fn show_panel(app: tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("panel") {
        panel::position_panel(&win);
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        let _ = win.set_always_on_top(true);
    }
}

#[tauri::command]
fn fit_panel(app: tauri::AppHandle, height: f64) {
    if let Some(win) = app.get_webview_window("panel") {
        // Cap to the monitor's usable height (minus taskbar + a margin) so tall tabs
        // like Overview stay fully on-screen and any overflow scrolls inside the
        // webview instead of running off the top / behind the taskbar.
        let max_h = win
            .current_monitor()
            .ok()
            .flatten()
            .map(|m| (m.size().height as f64 / m.scale_factor() - 72.0).max(360.0))
            .unwrap_or(900.0);
        let h = height.clamp(320.0, max_h);
        let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(452.0, h)));
        panel::position_panel(&win);
    }
}

#[tauri::command]
fn debug_info() -> serde_json::Value {
    let cdir = usage_core::logs_claude::claude_dir();
    let xdir = usage_core::logs_codex::codex_dir();
    let roots = snapshot::claude_log_roots();
    let cevents = usage_core::logs_claude::parse_trees_since(&roots, 0).len();
    let xevents = xdir
        .as_ref()
        .map(|d| usage_core::logs_codex::parse_all(d).len())
        .unwrap_or(0);
    let claude_sessions = sessions::claude_sessions().len();
    serde_json::json!({
        "claudeDir": cdir.as_ref().map(|d| d.display().to_string()),
        "claudeRoots": roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>(),
        "claudeEvents": cevents,
        "claudeSessions": claude_sessions,
        "codexDir": xdir.as_ref().map(|d| d.display().to_string()),
        "codexSessionsExists": xdir.as_ref().map(|d| d.join("sessions").exists()).unwrap_or(false),
        "codexEvents": xevents,
        "sessions": sessions::diag(),
    })
}

/// Whether local OAuth credentials exist for each provider (used to show
/// "connected" state and enable exact usage from the provider API).
#[tauri::command]
fn connections_status() -> serde_json::Value {
    let claude = usage_core::usage_api::load_claude_oauth().is_some();
    let codex = usage_core::usage_api::load_codex_oauth().is_some();
    serde_json::json!({ "claude": claude, "codex": codex })
}

/// List a GitHub user's public repositories (bare username or profile URL).
#[tauri::command]
fn github_repos(user: String) -> Result<Vec<github::Repo>, String> {
    github::repos(&user)
}

/// Store an API key in the OS credential store (empty key clears it).
#[tauri::command]
fn set_api_key(provider: String, key: String) -> Result<(), String> {
    if key.trim().is_empty() {
        secrets::clear_key(&provider)
    } else {
        secrets::set_key(&provider, key.trim())
    }
}

#[tauri::command]
fn clear_api_key(provider: String) -> Result<(), String> {
    secrets::clear_key(&provider)
}

#[tauri::command]
fn validate_api_key(provider: String) -> Result<(), String> {
    secrets::validate(&provider)
}

/// Which providers have an API key stored (booleans only — never the key).
#[tauri::command]
fn api_keys_status() -> serde_json::Value {
    serde_json::json!({
        "anthropic": secrets::has_key("anthropic"),
        "openai": secrets::has_key("openai"),
    })
}

#[tauri::command]
fn win_minimize(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("panel") {
        let _ = w.hide();
    }
}

#[tauri::command]
fn win_close(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("panel") {
        let _ = w.hide();
    }
}

#[tauri::command]
fn win_toggle_fullscreen(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("panel") {
        let maxed = w.is_maximized().unwrap_or(false);
        let _ = if maxed { w.unmaximize() } else { w.maximize() };
    }
}

fn toggle(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("panel") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            panel::position_panel(&win);
            let _ = win.unminimize();
            let _ = win.show();
            let _ = win.set_focus();
            let _ = win.set_always_on_top(true);
        }
    }
}

/// Rebuild the snapshot + tray on a background thread. Used after settings /
/// profile changes so the tray and panel reflect them promptly instead of
/// waiting for the next scheduled tick.
fn refresh_async(app: &tauri::AppHandle) {
    let h = app.clone();
    std::thread::spawn(move || refresh(&h));
}

/// Rebuild the snapshot, refresh the tray icon/tooltip, and notify the UI.
fn refresh(app: &tauri::AppHandle) {
    let settings = app.state::<AppState>().settings.lock().unwrap().clone();
    let snap = snapshot::build_all(&settings);

    // Tray reflects Claude by default, or Codex when it's the only tracked provider.
    let src = if !settings.track_claude && settings.track_codex {
        &snap.codex
    } else {
        &snap.claude
    };
    let five = src.limits.five_hour.pct;
    let seven = src.limits.seven_day.pct;
    if let Some(tray) = app.tray_by_id("main") {
        let (buf, w, h) = tray::render_icon(&settings.tray_style, &settings.tray_color, five, seven);
        let _ = tray.set_icon(Some(tauri::image::Image::new_owned(buf, w, h)));
        let _ = tray.set_tooltip(Some(tray::tooltip(
            five,
            &src.limits.five_hour.reset_label,
            seven,
            &src.limits.seven_day.reset_label,
        )));
    }

    maybe_notify(app, &settings, &snap);

    *app.state::<AppState>().snap.lock().unwrap() = Some(snap.clone());
    let _ = app.emit("snapshot", &snap);
}

/// Fire a desktop notification the first time a usage window crosses 75/90/95%.
/// Re-arms once that window drops back under 75% (i.e. after a reset).
fn maybe_notify(app: &tauri::AppHandle, settings: &config::Settings, snap: &snapshot::AllSnapshots) {
    if !settings.notify_enabled {
        return;
    }
    use tauri_plugin_notification::NotificationExt;
    let checks = [
        (settings.track_claude, "Claude", "claude:5h", "5h", snap.claude.limits.five_hour.pct),
        (settings.track_claude, "Claude", "claude:7d", "7d", snap.claude.limits.seven_day.pct),
        (settings.track_codex, "Codex", "codex:5h", "5h", snap.codex.limits.five_hour.pct),
        (settings.track_codex, "Codex", "codex:7d", "7d", snap.codex.limits.seven_day.pct),
    ];
    let thresholds = [95u32, 90, 75];
    let st = app.state::<AppState>();
    let mut notified = st.notified.lock().unwrap();
    for (tracked, label, key, window, pct) in checks {
        if !tracked {
            continue; // no alerts for a provider the user isn't tracking
        }
        let crossed = thresholds.iter().copied().find(|&t| pct >= t).unwrap_or(0);
        let prev = notified.get(key).copied().unwrap_or(0);
        if crossed > prev {
            let _ = app
                .notification()
                .builder()
                .title("TokenHub usage alert")
                .body(format!("{label} {window} window at {pct}%"))
                .show();
            notified.insert(key.to_string(), crossed);
        } else if pct < 75 {
            notified.insert(key.to_string(), 0);
        }
    }
}

fn main() {
    let settings = config::load();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A second launch just focuses the existing instance's panel.
            if let Some(win) = app.get_webview_window("panel") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(Vec::<&str>::new()),
        ))
        .manage(AppState {
            settings: Mutex::new(settings.clone()),
            snap: Mutex::new(None),
            notified: Mutex::new(std::collections::HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_processes,
            focus_process,
            get_windows,
            focus_window,
            delete_session,
            kill_process,
            open_url,
            open_folder,
            open_chat,
            focus_chat,
            get_project_dirs,
            get_aliases,
            set_session_alias,
            set_project_alias,
            get_git,
            get_sessions,
            get_session_agents,
            get_settings,
            set_settings,
            save_profile,
            switch_profile,
            delete_profile,
            toggle_panel,
            show_panel,
            fit_panel,
            debug_info,
            connections_status,
            github_repos,
            set_api_key,
            clear_api_key,
            validate_api_key,
            api_keys_status,
            win_minimize,
            win_close,
            win_toggle_fullscreen
        ])
        .on_window_event(|window, event| {
            // Click off the panel (focus lost) → minimize it to the tray.
            if let WindowEvent::Focused(false) = event {
                if window.label() == "panel" {
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();

            // Remove any leftover decorated window from older builds.
            processes::close_legacy_windows();

            // Sync autostart with the saved setting.
            {
                let mgr = app.autolaunch();
                let _ = if settings.autostart { mgr.enable() } else { mgr.disable() };
            }

            // Right-click tray menu: Open · Start on boot · Quit.
            let open_i = MenuItem::with_id(app, "open", "Open TokenHub", true, None::<&str>)?;
            let boot_i = CheckMenuItem::with_id(
                app, "boot", "Start on boot", true, settings.autostart, None::<&str>,
            )?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&open_i, &boot_i, &PredefinedMenuItem::separator(app)?, &quit_i],
            )?;
            let boot_item = boot_i.clone();

            // Dynamic tray icon (starts empty, updated by refresh()).
            let (buf, w, h) = tray::render_rgba(0, 0);
            let _tray = tauri::tray::TrayIconBuilder::with_id("main")
                .icon(tauri::image::Image::new_owned(buf, w, h))
                .tooltip("TokenHub")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(win) = app.get_webview_window("panel") {
                            panel::position_panel(&win);
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    "boot" => {
                        let mgr = app.autolaunch();
                        let enabled = mgr.is_enabled().unwrap_or(false);
                        let _ = if enabled { mgr.disable() } else { mgr.enable() };
                        let now = mgr.is_enabled().unwrap_or(!enabled);
                        let _ = boot_item.set_checked(now);
                        let snapshot = {
                            let st = app.state::<AppState>();
                            let mut s = st.settings.lock().unwrap();
                            s.autostart = now;
                            s.clone()
                        };
                        let _ = config::save(&snapshot);
                    }
                    _ => {}
                })
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
