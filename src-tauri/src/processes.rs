//! Processes tab: show active AI CLIs grouped by tool, one entry per launched
//! instance (tool × host-terminal). Clicking an entry brings that terminal window
//! to the front (Win32). Enumerated via PowerShell + CIM (Win32_Process).

use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProcInfo {
    /// window-owning host PID to bring to front
    pub pid: u32,
    pub name: String,
    pub host: String,
    #[serde(rename = "memMB")]
    pub mem_mb: u64,
    pub runtime: String,
}

#[derive(Serialize, Clone)]
pub struct ProcGroup {
    pub group: String,
    pub procs: Vec<ProcInfo>,
}

struct Raw {
    pid: u32,
    ppid: u32,
    name: String,
    cmd: String,
    mem: u64,
    start_ms: i64,
}

const HOSTS: &[(&str, &str)] = &[
    ("windowsterminal", "Windows Terminal"),
    ("wt.exe", "Windows Terminal"),
    ("powershell", "PowerShell"),
    ("pwsh", "PowerShell"),
    ("cmd.exe", "Command Prompt"),
    ("code.exe", "VS Code"),
    ("cursor.exe", "Cursor"),
    ("alacritty", "Alacritty"),
    ("wezterm", "WezTerm"),
];

/// Which AI tool a process is, filtered to the given provider family.
fn tool_for(name: &str, cmd: &str, provider: &str) -> Option<&'static str> {
    let hay = format!("{} {}", name.to_ascii_lowercase(), cmd.to_ascii_lowercase());
    if provider == "codex" {
        if hay.contains("codex") {
            Some("Codex")
        } else if hay.contains("chatgpt") || hay.contains("gpt-") {
            Some("GPT")
        } else {
            None
        }
    } else if hay.contains("cowork") {
        Some("Cowork")
    } else if hay.contains("claude") {
        Some("Claude Code")
    } else if hay.contains("cursor") {
        Some("Cursor")
    } else {
        None
    }
}

fn host_for(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    HOSTS.iter().find(|(k, _)| n.contains(k)).map(|(_, l)| *l)
}

fn parse_wmi_date(s: &str) -> i64 {
    if s.len() < 14 {
        return 0;
    }
    let p = |a: usize, b: usize| s.get(a..b).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
    use chrono::{Local, TimeZone};
    Local
        .with_ymd_and_hms(
            p(0, 4) as i32,
            p(4, 6) as u32,
            p(6, 8) as u32,
            p(8, 10) as u32,
            p(10, 12) as u32,
            p(12, 14) as u32,
        )
        .single()
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0)
}

fn fmt_runtime(start_ms: i64) -> String {
    if start_ms == 0 {
        return "—".into();
    }
    let secs = ((chrono::Utc::now().timestamp_millis() - start_ms) / 1000).max(0);
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h >= 1 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

fn query_raw() -> Vec<Raw> {
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,CommandLine,WorkingSetSize,CreationDate | ConvertTo-Json -Compress -Depth 2",
        ])
        .output();
    let Ok(out) = out else { return Vec::new() };
    let text = String::from_utf8_lossy(&out.stdout);
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return Vec::new() };
    let arr = match v {
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    arr.into_iter()
        .map(|o| Raw {
            pid: o.get("ProcessId").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            ppid: o.get("ParentProcessId").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            name: o.get("Name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            cmd: o.get("CommandLine").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            mem: o.get("WorkingSetSize").and_then(|x| x.as_u64()).unwrap_or(0),
            start_ms: parse_wmi_date(o.get("CreationDate").and_then(|x| x.as_str()).unwrap_or("")),
        })
        .collect()
}

struct Inst {
    tool: &'static str,
    host: String,
    host_pid: u32,
    mem: u64,
    start: i64,
}

/// Active AI instances for `provider`, grouped by tool.
pub fn list_groups(provider: &str) -> Vec<ProcGroup> {
    let raws = query_raw();
    let by_pid: HashMap<u32, &Raw> = raws.iter().map(|r| (r.pid, r)).collect();

    let mut insts: HashMap<(String, u32), Inst> = HashMap::new();
    for r in &raws {
        let Some(tool) = tool_for(&r.name, &r.cmd, provider) else { continue };
        // walk up to a window-owning host terminal
        let mut host = "Background".to_string();
        let mut host_pid = r.pid;
        let mut cur = r.ppid;
        for _ in 0..12 {
            let Some(p) = by_pid.get(&cur) else { break };
            if let Some(h) = host_for(&p.name) {
                host = h.to_string();
                host_pid = p.pid;
                break;
            }
            cur = p.ppid;
        }
        let e = insts.entry((tool.to_string(), host_pid)).or_insert(Inst {
            tool,
            host: host.clone(),
            host_pid,
            mem: 0,
            start: i64::MAX,
        });
        e.mem += r.mem;
        if r.start_ms > 0 && r.start_ms < e.start {
            e.start = r.start_ms;
        }
    }

    let mut groups: HashMap<&'static str, Vec<ProcInfo>> = HashMap::new();
    for inst in insts.into_values() {
        let start = if inst.start == i64::MAX { 0 } else { inst.start };
        groups.entry(inst.tool).or_default().push(ProcInfo {
            pid: inst.host_pid,
            name: inst.host.clone(),
            host: inst.host,
            mem_mb: inst.mem / (1024 * 1024),
            runtime: fmt_runtime(start),
        });
    }

    let mut out: Vec<ProcGroup> = groups
        .into_iter()
        .map(|(g, mut procs)| {
            procs.sort_by(|a, b| b.mem_mb.cmp(&a.mem_mb));
            ProcGroup { group: g.to_string(), procs }
        })
        .collect();
    out.sort_by(|a, b| a.group.cmp(&b.group));
    out
}

/// Bring the top-level visible window owned by `pid` to the foreground.
#[cfg(windows)]
pub fn focus(pid: u32) -> bool {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow,
        SW_RESTORE,
    };

    struct Ctx {
        pid: u32,
        hwnd: HWND,
    }
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        let mut wpid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut wpid));
        if wpid == ctx.pid && IsWindowVisible(hwnd).as_bool() {
            ctx.hwnd = hwnd;
            return BOOL(0); // stop enumerating
        }
        BOOL(1) // continue
    }

    let mut ctx = Ctx { pid, hwnd: HWND(std::ptr::null_mut()) };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut _ as isize));
        if !ctx.hwnd.0.is_null() {
            let _ = ShowWindow(ctx.hwnd, SW_RESTORE);
            return SetForegroundWindow(ctx.hwnd).as_bool();
        }
    }
    false
}

#[cfg(not(windows))]
pub fn focus(_pid: u32) -> bool {
    false
}

/// Terminate a process by PID (kept for potential use).
pub fn kill(pid: u32) -> bool {
    std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub hwnd: i64,
    pub title: String,
    pub process: String,
}

/// Enumerate visible top-level windows (title + owning process).
#[cfg(windows)]
pub fn list_windows() -> Vec<WindowInfo> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    let procs: HashMap<u32, String> = query_raw().into_iter().map(|r| (r.pid, r.name)).collect();

    struct Ctx {
        out: Vec<WindowInfo>,
        procs: HashMap<u32, String>,
    }
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        if IsWindowVisible(hwnd).as_bool() {
            let len = GetWindowTextLengthW(hwnd);
            if len > 0 {
                let mut buf = vec![0u16; (len + 1) as usize];
                let n = GetWindowTextW(hwnd, &mut buf);
                if n > 0 {
                    let title = String::from_utf16_lossy(&buf[..n as usize]);
                    if !title.trim().is_empty() {
                        let mut pid = 0u32;
                        GetWindowThreadProcessId(hwnd, Some(&mut pid));
                        let process = ctx.procs.get(&pid).cloned().unwrap_or_default();
                        ctx.out.push(WindowInfo {
                            hwnd: hwnd.0 as isize as i64,
                            title,
                            process,
                        });
                    }
                }
            }
        }
        BOOL(1)
    }

    let mut ctx = Ctx { out: Vec::new(), procs };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut _ as isize));
    }
    // Keep only AI-client windows (by title or owning process).
    const KW: &[&str] = &["claude", "codex", "chatgpt", "cursor", "copilot", "gpt"];
    ctx.out.retain(|w| {
        let hay = format!("{} {}", w.title.to_ascii_lowercase(), w.process.to_ascii_lowercase());
        KW.iter().any(|k| hay.contains(k))
    });
    ctx.out.sort_by(|a, b| {
        a.process.to_lowercase().cmp(&b.process.to_lowercase()).then(a.title.cmp(&b.title))
    });
    ctx.out.truncate(60);
    ctx.out
}

#[cfg(not(windows))]
pub fn list_windows() -> Vec<WindowInfo> {
    Vec::new()
}

/// Bring a specific window (by HWND) to the foreground.
#[cfg(windows)]
pub fn focus_hwnd(hwnd: i64) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE};
    unsafe {
        let h = HWND(hwnd as _);
        let _ = ShowWindow(h, SW_RESTORE);
        SetForegroundWindow(h).as_bool()
    }
}

#[cfg(not(windows))]
pub fn focus_hwnd(_hwnd: i64) -> bool {
    false
}

/// Focus an already-open window that best matches a chat (project folder in the
/// title + provider keyword) instead of spawning a new one. Returns false if none.
#[cfg(windows)]
pub fn focus_chat_window(provider: &str, cwd: &str) -> bool {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };
    let base = cwd
        .rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_ascii_lowercase();

    struct Ctx {
        base: String,
        provider: String,
        procs: HashMap<u32, String>,
        best_score: i32,
        best_hwnd: i64,
    }
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return BOOL(1);
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let n = GetWindowTextW(hwnd, &mut buf);
        if n <= 0 {
            return BOOL(1);
        }
        let title = String::from_utf16_lossy(&buf[..n as usize]).to_ascii_lowercase();
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let proc = ctx.procs.get(&pid).cloned().unwrap_or_default().to_ascii_lowercase();
        let mut score = 0;
        if !ctx.base.is_empty() && title.contains(&ctx.base) {
            score += 5;
        }
        if ctx.provider == "codex" {
            if proc.contains("codex") || title.contains("codex") || title.contains("gpt") {
                score += 2;
            }
        } else if proc.contains("claude") || title.contains("claude") {
            score += 2;
        }
        if score > ctx.best_score {
            ctx.best_score = score;
            ctx.best_hwnd = hwnd.0 as isize as i64;
        }
        BOOL(1)
    }
    let procs: HashMap<u32, String> = query_raw().into_iter().map(|r| (r.pid, r.name)).collect();
    let mut ctx = Ctx {
        base,
        provider: provider.to_string(),
        procs,
        best_score: 0,
        best_hwnd: 0,
    };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut _ as isize));
    }
    if ctx.best_score > 0 {
        focus_hwnd(ctx.best_hwnd)
    } else {
        false
    }
}

#[cfg(not(windows))]
pub fn focus_chat_window(_provider: &str, _cwd: &str) -> bool {
    false
}

/// Force-kill leftover *old-build* processes (their window title starts with
/// "AI Usage Bar" — the current app is titled "TokenHub") so the stray decorated
/// window at the mini-bar spot disappears on launch. Uses taskkill by PID because
/// WM_CLOSE is unreliable for a live app. PID-guarded: never our own process.
#[cfg(windows)]
pub fn close_legacy_windows() {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    };
    unsafe extern "system" fn cb(hwnd: HWND, _l: LPARAM) -> BOOL {
        let mut wpid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut wpid));
        if wpid == 0 || wpid == std::process::id() {
            return BOOL(1); // skip our own / unknown
        }
        let len = GetWindowTextLengthW(hwnd);
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            let n = GetWindowTextW(hwnd, &mut buf);
            if n > 0 {
                let title = String::from_utf16_lossy(&buf[..n as usize]);
                if title.trim_start().starts_with("AI Usage Bar") {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/PID", &wpid.to_string(), "/F", "/T"])
                        .output();
                }
            }
        }
        BOOL(1)
    }
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(0));
    }
}

#[cfg(not(windows))]
pub fn close_legacy_windows() {}
