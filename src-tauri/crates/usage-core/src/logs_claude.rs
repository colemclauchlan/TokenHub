//! Parse Claude Code local logs into normalized `UsageEvent`s.
//!
//! Sources (Windows): `%USERPROFILE%\.claude\projects\**\*.jsonl` (per-session token
//! logs) and `%USERPROFILE%\.claude\stats-cache.json` (historical rollups).
//!
//! Each JSONL line is a JSON object; assistant turns carry `message.usage` with
//! `input_tokens`, `output_tokens`, `cache_creation_input_tokens`,
//! `cache_read_input_tokens`, plus `message.model` and a top-level `timestamp`.
//! We dedup by (message id + requestId) so re-emitted lines don't double count.

use crate::model::UsageEvent;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// `%USERPROFILE%\.claude` (or `$HOME/.claude`).
pub fn claude_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".claude"))
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Recursively collect `*.jsonl` files under `root`.
fn collect_jsonl(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_jsonl(&p, out);
        } else if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
            out.push(p);
        }
    }
}

/// Parse all Claude Code session logs under `<claude_dir>/projects`.
pub fn parse_all(claude_dir: &Path) -> Vec<UsageEvent> {
    parse_trees_since(&[claude_dir.join("projects")], 0)
}

/// Parse every `*.jsonl` under each root (recursively), deduped by event id.
/// Files whose modified-time is older than `since_ms` are skipped (0 = no cutoff),
/// which bounds work when a root holds a long history of large logs (e.g. Cowork).
pub fn parse_trees_since(roots: &[PathBuf], since_ms: i64) -> Vec<UsageEvent> {
    let mut files = Vec::new();
    for r in roots {
        collect_jsonl(r, &mut files);
    }
    let mut events = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for f in files {
        if since_ms > 0 && file_mtime_ms(&f) < since_ms {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(ev) = event_from_value(&v) {
                    if let Some(id) = &ev.id {
                        if !seen.insert(id.clone()) {
                            continue; // duplicate
                        }
                    }
                    events.push(ev);
                }
            }
        }
    }
    events.sort_by_key(|e| e.ts_ms);
    events
}

fn file_mtime_ms(p: &Path) -> i64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Extract a `UsageEvent` from one parsed JSONL line, tolerating schema drift.
pub fn event_from_value(v: &serde_json::Value) -> Option<UsageEvent> {
    let msg = v.get("message")?;
    let usage = msg.get("usage")?;
    let get = |k: &str| usage.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    let input = get("input_tokens");
    let output = get("output_tokens");
    let cache_write = get("cache_creation_input_tokens");
    let cache_read = get("cache_read_input_tokens");
    if input + output + cache_read + cache_write == 0 {
        return None;
    }
    let model = msg
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown")
        .to_string();
    let ts_ms = v
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(parse_rfc3339_ms)
        .unwrap_or(0);
    // dedup key: message id (+ requestId if present)
    let id = msg.get("id").and_then(|i| i.as_str()).map(|mid| {
        let req = v
            .get("requestId")
            .or_else(|| v.get("request_id"))
            .and_then(|r| r.as_str())
            .unwrap_or("");
        format!("{mid}:{req}")
    });
    Some(UsageEvent {
        ts_ms,
        model,
        input,
        output,
        cache_read,
        cache_write,
        id,
    })
}

/// RFC3339 / ISO8601 → epoch milliseconds (UTC).
pub fn parse_rfc3339_ms(s: &str) -> Option<i64> {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_assistant_usage_line() {
        let line = serde_json::json!({
            "type": "assistant",
            "timestamp": "2026-07-05T10:00:00.000Z",
            "requestId": "req_123",
            "message": {
                "id": "msg_abc",
                "model": "claude-sonnet-4-6",
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 50,
                    "cache_creation_input_tokens": 10,
                    "cache_read_input_tokens": 2000
                }
            }
        });
        let ev = event_from_value(&line).unwrap();
        assert_eq!(ev.input, 100);
        assert_eq!(ev.output, 50);
        assert_eq!(ev.cache_write, 10);
        assert_eq!(ev.cache_read, 2000);
        assert_eq!(ev.model, "claude-sonnet-4-6");
        assert_eq!(ev.id.as_deref(), Some("msg_abc:req_123"));
        assert!(ev.ts_ms > 0);
    }

    #[test]
    fn skips_lines_without_usage() {
        let line = serde_json::json!({ "type": "user", "message": { "role": "user" } });
        assert!(event_from_value(&line).is_none());
    }
}
