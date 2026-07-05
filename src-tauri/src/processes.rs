//! Processes tab: enumerate AI-tool processes on Windows and group them by the
//! terminal/host that launched them. Uses PowerShell + CIM (Win32_Process) so we
//! get ProcessId, ParentProcessId, CommandLine, memory, and start time reliably
//! without a native-API crate.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub tool: String,
    #[serde(rename = "memMB")]
    pub mem_mb: u64,
    pub runtime: String,
}

#[derive(Serialize, Clone)]
pub struct ProcGroup {
    pub host: String,
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
    ("windowsterminalpreview", "Windows Terminal"),
];

fn tool_for(name: &str, cmd: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    let c = cmd.to_ascii_lowercase();
    let hay = format!("{n} {c}");
    if hay.contains("claude") {
        Some("Claude Code")
    } else if hay.contains("codex") {
        Some("Codex")
    } else {
        None
    }
}

fn host_for(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    HOSTS
        .iter()
        .find(|(k, _)| n.contains(k))
        .map(|(_, label)| *label)
}

fn parse_wmi_date(s: &str) -> i64 {
    // "yyyymmddHHMMSS.ffffff+zzz" -> epoch ms (best effort, treated as local)
    if s.len() < 14 {
        return 0;
    }
    let p = |a: usize, b: usize| s.get(a..b).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
    use chrono::{Local, TimeZone};
    let (y, mo, d, h, mi, se) = (p(0, 4), p(4, 6), p(6, 8), p(8, 10), p(10, 12), p(12, 14));
    Local
        .with_ymd_and_hms(y as i32, mo as u32, d as u32, h as u32, mi as u32, se as u32)
        .single()
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0)
}

fn fmt_runtime(start_ms: i64) -> String {
    if start_ms == 0 {
        return "—".into();
    }
    let now = chrono::Utc::now().timestamp_millis();
    let secs = ((now - start_ms) / 1000).max(0);
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
            start_ms: parse_wmi_date(
                o.get("CreationDate").and_then(|x| x.as_str()).unwrap_or(""),
            ),
        })
        .collect()
}

/// Enumerate AI processes grouped by host terminal.
pub fn list_groups() -> Vec<ProcGroup> {
    let raws = query_raw();
    let by_pid: HashMap<u32, &Raw> = raws.iter().map(|r| (r.pid, r)).collect();

    let mut groups: HashMap<String, Vec<ProcInfo>> = HashMap::new();
    for r in &raws {
        let Some(tool) = tool_for(&r.name, &r.cmd) else { continue };
        // walk up parents to find a known host terminal
        let mut host = "Background".to_string();
        let mut cur = r.ppid;
        for _ in 0..12 {
            let Some(p) = by_pid.get(&cur) else { break };
            if let Some(h) = host_for(&p.name) {
                host = h.to_string();
                break;
            }
            cur = p.ppid;
        }
        groups.entry(host).or_default().push(ProcInfo {
            pid: r.pid,
            name: r.name.clone(),
            tool: tool.to_string(),
            mem_mb: r.mem / (1024 * 1024),
            runtime: fmt_runtime(r.start_ms),
        });
    }

    let mut out: Vec<ProcGroup> = groups
        .into_iter()
        .map(|(host, mut procs)| {
            procs.sort_by(|a, b| b.mem_mb.cmp(&a.mem_mb));
            ProcGroup { host, procs }
        })
        .collect();
    out.sort_by(|a, b| a.host.cmp(&b.host));
    out
}

/// Terminate a process by PID (best effort).
pub fn kill(pid: u32) -> bool {
    std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
