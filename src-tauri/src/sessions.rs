//! Recent sessions / projects for Claude Code and Codex, read from local logs.

use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub cwd: String,
    pub branch: String,
    pub model: String,
    pub client: String,
    pub last_ms: i64,
    pub messages: u64,
    pub tokens: u64,
    pub context_tokens: u64,
    pub cost_usd: f64,
    pub active: bool,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Coarse agent state from time since last activity:
/// working (<45s), waiting for you (<5m), or stopped/done.
pub fn agent_status(last_ms: i64) -> &'static str {
    let idle = now_ms() - last_ms;
    if idle < 45_000 {
        "working"
    } else if idle < 5 * 60_000 {
        "waiting"
    } else {
        "stopped"
    }
}

/// All sessions (Claude + Codex), most-recently-active first.
pub fn all_sessions() -> Vec<SessionInfo> {
    let mut out = claude_sessions();
    out.extend(codex_sessions());
    out.sort_by_key(|s| std::cmp::Reverse(s.last_ms));
    out
}

/// Scanner diagnostics for Settings → Diagnostics (why Cowork/Codex may not show).
pub fn diag() -> serde_json::Value {
    let mut roots = Vec::new();
    for r in cowork_roots() {
        let mut files = Vec::new();
        collect_paths(&r, &mut files);
        roots.push(serde_json::json!({
            "path": r.display().to_string(),
            "exists": r.exists(),
            "jsonlFiles": files.len(),
        }));
    }
    let all = all_sessions();
    let now = now_ms();
    let count = |c: &str| all.iter().filter(|s| s.client == c).count();
    let recent: Vec<_> = all
        .iter()
        .take(6)
        .map(|s| {
            serde_json::json!({
                "client": s.client,
                "model": s.model,
                "name": s.name.chars().take(36).collect::<String>(),
                "ageMin": (now - s.last_ms) / 60000,
            })
        })
        .collect();
    serde_json::json!({
        "coworkRoots": roots,
        "claudeCode": count("Claude Code"),
        "cowork": count("Claude Cowork"),
        "codex": count("Codex") + count("GPT"),
        "total": all.len(),
        "recent": recent,
    })
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub name: String,
    pub model: String,
    /// working | waiting | stopped (main agent) · running | done (sub-agents)
    pub status: String,
    /// Very short (few-word) task summary shown on the agent row.
    pub goal: String,
    /// Fuller description of what the agent is doing, shown when expanded.
    pub detail: String,
}

/// First `max_words` words of `s` (with an ellipsis if truncated) — the
/// few-word summary shown on a collapsed agent row.
fn short_goal(s: &str, max_words: usize) -> String {
    let mut words = s.split_whitespace();
    let head: Vec<&str> = words.by_ref().take(max_words).collect();
    let mut out = head.join(" ");
    if words.next().is_some() {
        out.push('…');
    }
    out
}

/// Clip to `max_chars` characters (with an ellipsis) for the expanded detail.
fn clip(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect();
    if s.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// Reduce text to at most one sentence (first line, cut at the first sentence
/// end, capped at ~70 chars on a word boundary) — the chat-card display name.
fn one_sentence(s: &str) -> String {
    let line = s.lines().find(|l| !l.trim().is_empty()).map(|l| l.trim()).unwrap_or("");
    let chars: Vec<char> = line.chars().collect();
    let mut end = chars.len();
    for i in 0..chars.len() {
        if matches!(chars[i], '.' | '!' | '?')
            && (i + 1 == chars.len() || chars[i + 1].is_whitespace())
        {
            end = i + 1;
            break;
        }
    }
    let sent: String = chars[..end].iter().collect();
    let sent = sent.trim_end_matches(['.', ':']).trim();
    let mut out = String::new();
    for w in sent.split_whitespace() {
        if !out.is_empty() && out.chars().count() + w.chars().count() + 1 > 70 {
            out.push('…');
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    out
}

/// Drop later duplicates of the same chat (same project dir + same name),
/// e.g. continuation/resume files of one conversation. Input must be sorted
/// most-recent first, so the newest copy is the one kept.
fn dedupe(mut v: Vec<SessionInfo>) -> Vec<SessionInfo> {
    let mut seen = std::collections::HashSet::new();
    v.retain(|s| seen.insert((s.cwd.to_ascii_lowercase(), s.name.to_ascii_lowercase())));
    v
}

fn find_claude_file(id: &str) -> Option<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Some(d) = usage_core::logs_claude::claude_dir() {
        collect_paths(&d.join("projects"), &mut files);
    }
    for r in cowork_roots() {
        collect_paths(&r, &mut files);
    }
    files
        .into_iter()
        .find(|p| p.file_stem().map(|s| s.to_string_lossy() == id).unwrap_or(false))
}

fn find_codex_file(id: &str) -> Option<std::path::PathBuf> {
    let d = usage_core::logs_codex::codex_dir()?;
    let mut files = Vec::new();
    collect_codex(&d.join("sessions"), &mut files);
    files
        .into_iter()
        .find(|p| p.file_stem().map(|s| s.to_string_lossy() == id).unwrap_or(false))
}

/// The agents inside a chat: the main agent + any sub-agents (Claude `Task` tool),
/// each with model, name, status, and its goal/activity. Best-effort from the log.
pub fn session_agents(provider: &str, id: &str) -> Vec<AgentInfo> {
    if provider == "codex" {
        return codex_agents(id);
    }
    let Some(path) = find_claude_file(id) else { return vec![] };
    let Ok(text) = std::fs::read_to_string(&path) else { return vec![] };

    let mut main_model = String::new();
    let mut last_ms = 0i64;
    let mut goal = String::new();
    let mut subs: Vec<(String, String, String, String)> = Vec::new(); // (tool_id, name, desc, prompt)
    let mut done: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if let Some(m) = v.pointer("/message/model").and_then(|x| x.as_str()) {
            main_model = m.to_string();
        }
        if let Some(ts) = v
            .get("timestamp")
            .and_then(|x| x.as_str())
            .and_then(usage_core::logs_claude::parse_rfc3339_ms)
        {
            if ts > last_ms {
                last_ms = ts;
            }
        }
        if v.get("type").and_then(|x| x.as_str()) == Some("user") {
            if let Some(t) = v.pointer("/message/content").and_then(first_text) {
                let t = t.trim();
                if !t.is_empty() && !t.starts_with('<') {
                    goal = t.to_string();
                }
            }
        }
        if let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) {
            for b in content {
                match b.get("type").and_then(|x| x.as_str()) {
                    Some("tool_use") if b.get("name").and_then(|x| x.as_str()) == Some("Task") => {
                        let tid = b.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let name = b
                            .pointer("/input/subagent_type")
                            .and_then(|x| x.as_str())
                            .unwrap_or("subagent")
                            .to_string();
                        let desc = b
                            .pointer("/input/description")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let prompt = b
                            .pointer("/input/prompt")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        subs.push((tid, name, desc, prompt));
                    }
                    Some("tool_result") => {
                        if let Some(tid) = b.get("tool_use_id").and_then(|x| x.as_str()) {
                            done.insert(tid.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if main_model.is_empty() {
        main_model = "claude".into();
    }
    let main_status = agent_status(last_ms);
    let mut out = vec![AgentInfo {
        name: "Main agent".into(),
        model: main_model,
        status: main_status.to_string(),
        goal: if goal.is_empty() { "—".into() } else { short_goal(&goal, 7) },
        detail: if goal.is_empty() { "—".into() } else { clip(&goal, 400) },
    }];
    // Most-recent sub-agents first, capped. A stopped chat can't have running
    // sub-agents — a Task whose result never landed is finished, not live.
    for (tid, name, desc, prompt) in subs.into_iter().rev().take(8) {
        let summary = if desc.is_empty() { &prompt } else { &desc };
        let detail = if prompt.is_empty() { &desc } else { &prompt };
        let running = !done.contains(&tid) && main_status != "stopped";
        out.push(AgentInfo {
            name,
            model: "sub-agent".into(),
            status: if running { "running".into() } else { "done".into() },
            goal: if summary.is_empty() { "—".into() } else { short_goal(summary, 7) },
            detail: if detail.is_empty() { "—".into() } else { clip(detail, 400) },
        });
    }
    out
}

fn codex_agents(id: &str) -> Vec<AgentInfo> {
    let Some(path) = find_codex_file(id) else { return vec![] };
    let Ok(text) = std::fs::read_to_string(&path) else { return vec![] };
    let mut model = String::new();
    let mut last_ms = 0i64;
    let mut goal = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let payload = v.get("payload").cloned().unwrap_or(v.clone());
        if let Some(m) = payload.get("model").and_then(|x| x.as_str()) {
            model = m.to_string();
        }
        if let Some(ts) = v
            .get("timestamp")
            .or_else(|| payload.get("timestamp"))
            .and_then(|x| x.as_str())
            .and_then(usage_core::logs_claude::parse_rfc3339_ms)
        {
            if ts > last_ms {
                last_ms = ts;
            }
        }
        if let Some(t) = payload.get("content").and_then(first_text) {
            let t = t.trim();
            if !t.is_empty() && !t.starts_with('<') {
                goal = t.to_string();
            }
        }
    }
    if model.is_empty() {
        model = "gpt-5-codex".into();
    }
    vec![AgentInfo {
        name: "Main agent".into(),
        model,
        status: agent_status(last_ms).to_string(),
        goal: if goal.is_empty() { "—".into() } else { short_goal(&goal, 7) },
        detail: if goal.is_empty() { "—".into() } else { clip(&goal, 400) },
    }]
}

fn first_text(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(a) => a.iter().find_map(|b| {
            if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                b.get("text").and_then(|t| t.as_str()).map(String::from)
            } else {
                None
            }
        }),
        _ => None,
    }
}

fn basename(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

/* ------------------------------ Claude Code ------------------------------ */

pub fn claude_sessions() -> Vec<SessionInfo> {
    // Two sources: the Claude Code CLI (~/.claude/projects) and the Cowork
    // desktop app (%APPDATA%\Claude\local-agent-mode-sessions\**\.claude\projects).
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    if let Some(dir) = usage_core::logs_claude::claude_dir() {
        collect_paths(&dir.join("projects"), &mut files);
    }
    for root in cowork_roots() {
        collect_paths(&root, &mut files);
    }
    // Parse only the newest files (by mtime) so a large log history stays fast.
    files.sort_by_key(|p| std::cmp::Reverse(mtime_ms(p)));
    files.truncate(60);
    let mut out: Vec<SessionInfo> = files
        .iter()
        .filter_map(|p| parse_claude_session(p))
        // Workflow/sub-agent transcripts aren't chats — don't list them.
        .filter(|s| !s.id.starts_with("wf_") && !s.id.starts_with("agent-"))
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.last_ms));
    let mut out = dedupe(out);
    out.truncate(30);
    out
}

/// Cowork keeps Claude sessions outside ~/.claude — under the desktop app's
/// per-session data dirs. Scan both roaming and local app-data locations.
fn cowork_roots() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    for key in ["APPDATA", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(key) {
            v.push(std::path::PathBuf::from(base).join("Claude").join("local-agent-mode-sessions"));
        }
    }
    v
}

fn collect_paths(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_paths(&p, out);
        } else if p.extension().map(|x| x == "jsonl").unwrap_or(false) {
            out.push(p);
        }
    }
}

fn mtime_ms(p: &Path) -> i64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn parse_claude_session(path: &Path) -> Option<SessionInfo> {
    let text = std::fs::read_to_string(path).ok()?;
    let id = path.file_stem()?.to_string_lossy().to_string();
    let mut cwd = String::new();
    let mut branch = String::new();
    let mut name = String::new();
    let mut model = String::new();
    let mut last_ms = 0i64;
    let mut messages = 0u64;
    let mut tokens = 0u64;
    let mut ctx = 0u64;
    let mut cost = 0f64;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };

        if let Some(m) = v.pointer("/message/model").and_then(|x| x.as_str()) {
            model = m.to_string();
        }
        if cwd.is_empty() {
            if let Some(c) = v.get("cwd").and_then(|x| x.as_str()) {
                cwd = c.to_string();
            }
        }
        if branch.is_empty() {
            if let Some(b) = v.get("gitBranch").and_then(|x| x.as_str()) {
                branch = b.to_string();
            }
        }
        if name.is_empty() {
            // Prefer the log's own conversation summary; else the first real
            // user ask. Either way condense to one sentence for the card name.
            if let Some(s) = v.get("summary").and_then(|x| x.as_str()) {
                name = one_sentence(s);
            } else if v.get("type").and_then(|x| x.as_str()) == Some("user") {
                if let Some(t) = v.pointer("/message/content").and_then(first_text) {
                    let t = t.trim();
                    if !t.is_empty() && !t.starts_with('<') && !t.starts_with("Caveat:") {
                        name = one_sentence(t);
                    }
                }
            }
        }
        if let Some(ts) = v
            .get("timestamp")
            .and_then(|x| x.as_str())
            .and_then(usage_core::logs_claude::parse_rfc3339_ms)
        {
            if ts > last_ms {
                last_ms = ts;
            }
        }
        if let Some(u) = v.pointer("/message/usage") {
            let g = |k: &str| u.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
            let inp = g("input_tokens");
            let out = g("output_tokens");
            let cr = g("cache_read_input_tokens");
            let cw = g("cache_creation_input_tokens");
            tokens += inp + out + cr + cw;
            ctx = inp + cr + cw; // last assistant turn ≈ current context size
            let ev = usage_core::model::UsageEvent {
                ts_ms: 0,
                model: model.clone(),
                input: inp,
                output: out,
                cache_read: cr,
                cache_write: cw,
                id: None,
            };
            cost += usage_core::pricing::event_cost_usd(&ev);
        }
        messages += 1;
    }

    if last_ms == 0 {
        last_ms = mtime_ms(path); // fall back to file mtime (Cowork logs may vary)
    }
    if last_ms == 0 {
        return None;
    }
    if cwd.is_empty() {
        cwd = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
    }
    if name.is_empty() {
        name = basename(&cwd);
    }
    if model.is_empty() {
        model = "claude".into();
    }
    let ppath = path.to_string_lossy().to_ascii_lowercase();
    let client = if ppath.contains("local-agent-mode-sessions") {
        "Claude Cowork"
    } else {
        "Claude Code"
    }
    .to_string();
    Some(SessionInfo {
        id,
        name,
        cwd,
        branch,
        model,
        client,
        last_ms,
        messages,
        tokens,
        context_tokens: ctx,
        cost_usd: cost,
        active: now_ms() - last_ms < 10 * 60 * 1000,
    })
}

/* --------------------------------- Codex --------------------------------- */

pub fn codex_sessions() -> Vec<SessionInfo> {
    let Some(dir) = usage_core::logs_codex::codex_dir() else { return vec![] };
    let mut files = Vec::new();
    collect_codex(&dir.join("sessions"), &mut files);
    let mut out: Vec<SessionInfo> = files.iter().filter_map(|p| parse_codex_session(p)).collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.last_ms));
    let mut out = dedupe(out);
    out.truncate(30);
    out
}

fn collect_codex(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_codex(&p, out);
        } else {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if n.starts_with("rollout-") && n.ends_with(".jsonl") {
                out.push(p);
            }
        }
    }
}

fn parse_codex_session(path: &Path) -> Option<SessionInfo> {
    let text = std::fs::read_to_string(path).ok()?;
    let id = path.file_stem()?.to_string_lossy().to_string();
    let mut cwd = String::new();
    let mut name = String::new();
    let mut model = String::new();
    let mut last_ms = 0i64;
    let mut messages = 0u64;
    let mut tokens = 0u64;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let payload = v.get("payload").unwrap_or(&v);
        if let Some(m) = payload.get("model").and_then(|x| x.as_str()) {
            model = m.to_string();
        }

        if cwd.is_empty() {
            if let Some(c) = payload.get("cwd").or_else(|| v.get("cwd")).and_then(|x| x.as_str()) {
                cwd = c.to_string();
            }
        }
        if name.is_empty() {
            if let Some(t) = payload.get("content").and_then(first_text) {
                let t = t.trim();
                if !t.is_empty() && !t.starts_with('<') {
                    name = one_sentence(t);
                }
            }
        }
        if let Some(ts) = v
            .get("timestamp")
            .or_else(|| payload.get("timestamp"))
            .and_then(|x| x.as_str())
            .and_then(usage_core::logs_claude::parse_rfc3339_ms)
        {
            if ts > last_ms {
                last_ms = ts;
            }
        }
        let ptype = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ptype == "token_count" {
            let u = payload.get("usage").unwrap_or(payload);
            if let Some(total) = u.get("total_tokens").and_then(|x| x.as_u64()) {
                tokens = total; // cumulative → last one wins
            }
        }
        messages += 1;
    }

    if last_ms == 0 {
        last_ms = mtime_ms(path); // fall back to file mtime
    }
    if last_ms == 0 {
        return None;
    }
    if name.is_empty() {
        name = if cwd.is_empty() { "Codex session".into() } else { basename(&cwd) };
    }
    if model.is_empty() {
        model = "gpt-5-codex".into();
    }
    let ml = model.to_ascii_lowercase();
    let client = if ml.contains("codex") {
        "Codex"
    } else if ml.contains("gpt") {
        "GPT"
    } else {
        "Codex"
    }
    .to_string();
    Some(SessionInfo {
        id,
        name,
        cwd,
        branch: String::new(),
        model,
        client,
        last_ms,
        messages,
        tokens,
        context_tokens: 0,
        cost_usd: 0.0,
        active: now_ms() - last_ms < 10 * 60 * 1000,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_sentence_cuts_at_sentence_end() {
        assert_eq!(
            one_sentence("Build the PRD. Then start implementing the MVP."),
            "Build the PRD"
        );
        // A '.' inside a filename is not a sentence end.
        assert_eq!(one_sentence("fix app.js please"), "fix app.js please");
        // Only the first non-empty line counts, trailing colon dropped.
        assert_eq!(one_sentence("\nFiles to review:\n- a.rs\n- b.rs"), "Files to review");
    }

    #[test]
    fn one_sentence_caps_length_on_word_boundary() {
        let long = "word ".repeat(40);
        let out = one_sentence(&long);
        assert!(out.chars().count() <= 71, "got {} chars", out.chars().count());
        assert!(out.ends_with('…'));
    }

    fn si(name: &str, cwd: &str, last_ms: i64) -> SessionInfo {
        SessionInfo {
            id: format!("{name}-{last_ms}"),
            name: name.into(),
            cwd: cwd.into(),
            branch: String::new(),
            model: String::new(),
            client: String::new(),
            last_ms,
            messages: 0,
            tokens: 0,
            context_tokens: 0,
            cost_usd: 0.0,
            active: false,
        }
    }

    #[test]
    fn dedupe_keeps_newest_copy_per_cwd_and_name() {
        let v = vec![
            si("Fix login", "C:/proj/BillShare", 300),
            si("fix login", "c:/proj/billshare", 200), // same chat, older resume file
            si("Fix login", "C:/proj/Other", 100),     // same name, different repo — kept
        ];
        let out = dedupe(v);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].last_ms, 300);
        assert_eq!(out[1].cwd, "C:/proj/Other");
    }
}
