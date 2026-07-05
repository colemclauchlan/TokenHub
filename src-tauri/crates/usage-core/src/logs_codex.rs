//! Parse OpenAI Codex CLI local logs into normalized `UsageEvent`s.
//!
//! Source (Windows): `%USERPROFILE%\.codex\sessions\**\rollout-*.jsonl` (also
//! `archived_sessions`). Lines include `event_msg` entries whose
//! `payload.type == "token_count"` report **cumulative** totals; we subtract the
//! previous cumulative within a file to recover per-turn usage (input, cached
//! input, output, reasoning).

use crate::model::UsageEvent;
use std::path::{Path, PathBuf};

pub fn codex_dir() -> Option<PathBuf> {
    // CODEX_HOME overrides, else ~/.codex
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| crate::logs_claude::home_dir().map(|h| h.join(".codex")))
}

fn collect_rollouts(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_rollouts(&p, out);
        } else {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                out.push(p);
            }
        }
    }
}

pub fn parse_all(codex_dir: &Path) -> Vec<UsageEvent> {
    let mut files = Vec::new();
    collect_rollouts(&codex_dir.join("sessions"), &mut files);
    collect_rollouts(&codex_dir.join("archived_sessions"), &mut files);

    let mut events = Vec::new();
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        events.extend(parse_session(&text));
    }
    events.sort_by_key(|e| e.ts_ms);
    events
}

/// Parse one rollout file's text, diffing cumulative token_count events.
pub fn parse_session(text: &str) -> Vec<UsageEvent> {
    let mut out = Vec::new();
    let mut prev = (0u64, 0u64, 0u64, 0u64); // input, cached, output, reasoning
    let mut model = String::from("gpt-5-codex");
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };

        // capture a model name if present anywhere obvious
        if let Some(m) = v.get("model").and_then(|x| x.as_str()) {
            model = m.to_string();
        }
        let payload = v.get("payload").unwrap_or(&v);
        let ptype = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ptype != "token_count" && v.get("type").and_then(|t| t.as_str()) != Some("token_count") {
            continue;
        }
        if let Some(m) = payload.get("model").and_then(|x| x.as_str()) {
            model = m.to_string();
        }
        let u = payload.get("usage").or_else(|| payload.get("info")).unwrap_or(payload);
        let g = |k: &str| u.get(k).and_then(|x| x.as_u64());
        let input = g("input_tokens").unwrap_or(0);
        let cached = g("cached_input_tokens").or_else(|| g("cache_read_input_tokens")).unwrap_or(0);
        let output = g("output_tokens").unwrap_or(0);
        let reasoning = g("reasoning_output_tokens").or_else(|| g("reasoning_tokens")).unwrap_or(0);

        // cumulative → delta (guard against resets)
        let d_input = input.saturating_sub(prev.0);
        let d_cached = cached.saturating_sub(prev.1);
        let d_output = output.saturating_sub(prev.2);
        let d_reason = reasoning.saturating_sub(prev.3);
        prev = (input, cached, output, reasoning);

        if d_input + d_cached + d_output + d_reason == 0 {
            continue;
        }
        let ts_ms = v
            .get("timestamp")
            .or_else(|| payload.get("timestamp"))
            .and_then(|t| t.as_str())
            .and_then(crate::logs_claude::parse_rfc3339_ms)
            .unwrap_or(0);

        out.push(UsageEvent {
            ts_ms,
            model: model.clone(),
            input: d_input,
            output: d_output + d_reason, // fold reasoning into output
            cache_read: d_cached,
            cache_write: 0,
            id: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffs_cumulative_token_counts() {
        let text = [
            r#"{"timestamp":"2026-07-05T10:00:00Z","payload":{"type":"token_count","model":"gpt-5-codex","usage":{"input_tokens":100,"cached_input_tokens":0,"output_tokens":40,"reasoning_output_tokens":10}}}"#,
            r#"{"timestamp":"2026-07-05T10:05:00Z","payload":{"type":"token_count","usage":{"input_tokens":250,"cached_input_tokens":50,"output_tokens":90,"reasoning_output_tokens":25}}}"#,
        ].join("\n");
        let evs = parse_session(&text);
        assert_eq!(evs.len(), 2);
        // first turn = cumulative baseline
        assert_eq!(evs[0].input, 100);
        assert_eq!(evs[0].output, 50); // 40 + 10 reasoning
        // second turn = delta
        assert_eq!(evs[1].input, 150);
        assert_eq!(evs[1].cache_read, 50);
        assert_eq!(evs[1].output, 65); // (90-40)+(25-10)=50+15
        assert_eq!(evs[1].model, "gpt-5-codex");
    }
}
