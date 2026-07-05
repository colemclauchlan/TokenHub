//! Provider usage API — the authoritative source for the 5h / 7d windows, read
//! with the local OAuth token so our numbers match the in-app counter.
//!
//! Transport is injected (the app supplies an HTTPS client) so this module stays
//! pure/testable: it (a) loads local OAuth creds, (b) builds the request, and
//! (c) parses the response into `QuotaWindows`.
//!
//! ⚠️ The exact endpoint + response shape are not publicly documented and can
//! change; like the reference app, hosts/URLs are overridable via env vars, and
//! the parser is deliberately tolerant. Verify against the live API on Windows (P4).

use std::path::PathBuf;

/// One limit window as reported by the provider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuotaWindow {
    /// 0.0..=1.0
    pub utilization: f64,
    /// epoch ms of the next reset (0 if unknown)
    pub reset_ms: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QuotaWindows {
    pub five_hour: Option<QuotaWindow>,
    pub seven_day: Option<QuotaWindow>,
}

#[derive(Clone, Debug)]
pub struct OAuthCreds {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_ms: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

/// Injected HTTPS transport (implemented by the app with reqwest/ureq/etc.).
pub trait Transport {
    fn send(&self, req: &HttpRequest) -> Result<String, String>;
}

fn env_or(keys: &[&str], default: &str) -> String {
    for k in keys {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    default.to_string()
}

/* ----------------------------- Claude ----------------------------- */

/// Read Claude Code OAuth creds from `~/.claude/.credentials.json`.
/// (On macOS the CLI may use Keychain; on Windows/Linux this JSON file is used.)
pub fn load_claude_oauth() -> Option<OAuthCreds> {
    let path: PathBuf = crate::logs_claude::claude_dir()?.join(".credentials.json");
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    // shape: { "claudeAiOauth": { "accessToken", "refreshToken", "expiresAt" } }
    let o = v.get("claudeAiOauth").unwrap_or(&v);
    let access = o
        .get("accessToken")
        .or_else(|| o.get("access_token"))
        .and_then(|x| x.as_str())?
        .to_string();
    Some(OAuthCreds {
        access_token: access,
        refresh_token: o
            .get("refreshToken")
            .or_else(|| o.get("refresh_token"))
            .and_then(|x| x.as_str())
            .map(String::from),
        expires_at_ms: o
            .get("expiresAt")
            .or_else(|| o.get("expires_at"))
            .and_then(|x| x.as_i64()),
    })
}

pub fn build_claude_usage_request(access_token: &str) -> HttpRequest {
    let url = env_or(
        &["AIUSAGEBAR_CLAUDE_USAGE_URL", "TERMTRACKER_CLAUDE_USAGE_URL"],
        "https://api.anthropic.com/api/oauth/usage",
    );
    let beta = env_or(
        &["AIUSAGEBAR_ANTHROPIC_BETA", "TERMTRACKER_ANTHROPIC_BETA"],
        "oauth-2025-04-20",
    );
    HttpRequest {
        method: "GET",
        url,
        headers: vec![
            ("Authorization".into(), format!("Bearer {access_token}")),
            ("anthropic-beta".into(), beta),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: None,
    }
}

/* ----------------------------- Codex ------------------------------ */

/// Read Codex OAuth creds from `~/.codex/auth.json`.
pub fn load_codex_oauth() -> Option<OAuthCreds> {
    let path: PathBuf = crate::logs_codex::codex_dir()?.join("auth.json");
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let tok = v.get("tokens").unwrap_or(&v);
    let access = tok
        .get("access_token")
        .or_else(|| tok.get("accessToken"))
        .and_then(|x| x.as_str())?
        .to_string();
    Some(OAuthCreds {
        access_token: access,
        refresh_token: tok
            .get("refresh_token")
            .and_then(|x| x.as_str())
            .map(String::from),
        expires_at_ms: None,
    })
}

pub fn build_codex_usage_request(access_token: &str) -> HttpRequest {
    let url = env_or(
        &["AIUSAGEBAR_CODEX_USAGE_URL", "TERMTRACKER_CODEX_USAGE_URL"],
        "https://chatgpt.com/backend-api/codex/usage",
    );
    HttpRequest {
        method: "GET",
        url,
        headers: vec![
            ("Authorization".into(), format!("Bearer {access_token}")),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: None,
    }
}

/* --------------------------- response parse --------------------------- */

/// Tolerant parse of a usage response into `QuotaWindows`. Recognizes several
/// plausible field spellings; returns whatever it can find.
pub fn parse_usage_response(json: &str) -> QuotaWindows {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return QuotaWindows::default();
    };
    QuotaWindows {
        five_hour: extract_window(&v, &["five_hour", "5h", "fiveHour", "session", "primary"]),
        seven_day: extract_window(&v, &["seven_day", "7d", "sevenDay", "weekly", "week"]),
    }
}

fn extract_window(root: &serde_json::Value, keys: &[&str]) -> Option<QuotaWindow> {
    // find a sub-object under any of the candidate keys (searching common containers)
    let containers = [root, root.get("windows").unwrap_or(root), root.get("limits").unwrap_or(root)];
    for c in containers {
        for k in keys {
            if let Some(obj) = c.get(*k) {
                if let Some(w) = window_from_obj(obj) {
                    return Some(w);
                }
            }
        }
    }
    None
}

fn window_from_obj(o: &serde_json::Value) -> Option<QuotaWindow> {
    let util = o
        .get("utilization")
        .or_else(|| o.get("used_percent"))
        .or_else(|| o.get("percent"))
        .or_else(|| o.get("usage"))
        .and_then(|x| x.as_f64())?;
    // normalize percent (0..100) → 0..1
    let util = if util > 1.0 { util / 100.0 } else { util };
    let reset_ms = o
        .get("reset_ms")
        .and_then(|x| x.as_i64())
        .or_else(|| {
            o.get("resets_at")
                .or_else(|| o.get("reset_at"))
                .or_else(|| o.get("reset"))
                .and_then(|x| x.as_str())
                .and_then(crate::logs_claude::parse_rfc3339_ms)
        })
        .unwrap_or(0);
    Some(QuotaWindow {
        utilization: util.clamp(0.0, 1.0),
        reset_ms,
    })
}

/// Convenience: load creds → build request → send via transport → parse.
pub fn fetch_claude_quota<T: Transport>(t: &T) -> Result<QuotaWindows, String> {
    let creds = load_claude_oauth().ok_or("no Claude OAuth creds found")?;
    let req = build_claude_usage_request(&creds.access_token);
    let body = t.send(&req)?;
    Ok(parse_usage_response(&body))
}

pub fn fetch_codex_quota<T: Transport>(t: &T) -> Result<QuotaWindows, String> {
    let creds = load_codex_oauth().ok_or("no Codex OAuth creds found")?;
    let req = build_codex_usage_request(&creds.access_token);
    let body = t.send(&req)?;
    Ok(parse_usage_response(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeTransport(&'static str);
    impl Transport for FakeTransport {
        fn send(&self, _r: &HttpRequest) -> Result<String, String> {
            Ok(self.0.to_string())
        }
    }

    #[test]
    fn parses_percent_and_reset() {
        let json = r#"{
          "windows": {
            "five_hour": { "utilization": 36, "resets_at": "2026-07-05T13:00:00Z" },
            "seven_day": { "utilization": 0.23, "reset_ms": 1751800000000 }
          }
        }"#;
        let q = parse_usage_response(json);
        let five = q.five_hour.unwrap();
        assert!((five.utilization - 0.36).abs() < 1e-9);
        assert!(five.reset_ms > 0);
        let seven = q.seven_day.unwrap();
        assert!((seven.utilization - 0.23).abs() < 1e-9);
        assert_eq!(seven.reset_ms, 1751800000000);
    }

    #[test]
    fn request_has_bearer_and_beta() {
        let req = build_claude_usage_request("tok_xyz");
        assert!(req.headers.iter().any(|(k, v)| k == "Authorization" && v == "Bearer tok_xyz"));
        assert!(req.headers.iter().any(|(k, _)| k == "anthropic-beta"));
    }

    #[test]
    fn transport_roundtrip() {
        let t = FakeTransport(r#"{"five_hour":{"utilization":50,"reset_ms":123}}"#);
        // load_* will fail (no creds on CI), so just test parse path directly here
        let q = parse_usage_response(
            &t.send(&build_claude_usage_request("x")).unwrap(),
        );
        assert_eq!(q.five_hour.unwrap().utilization, 0.5);
    }
}
