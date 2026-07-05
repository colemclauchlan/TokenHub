//! HTTPS transport for the provider usage API (injected into usage-core), plus a
//! best-effort quota fetch that never blocks the UI on failure.

use usage_core::model::Provider;
use usage_core::usage_api::{
    self, build_claude_usage_request, build_codex_usage_request, load_claude_oauth,
    load_codex_oauth, HttpRequest, QuotaWindows, Transport,
};

/// Minimal blocking HTTPS transport backed by ureq.
pub struct UreqTransport;

impl Transport for UreqTransport {
    fn send(&self, req: &HttpRequest) -> Result<String, String> {
        let mut r = ureq::request(req.method, &req.url);
        for (k, v) in &req.headers {
            r = r.set(k, v);
        }
        let resp = match &req.body {
            Some(b) => r.send_string(b),
            None => r.call(),
        }
        .map_err(|e| e.to_string())?;
        resp.into_string().map_err(|e| e.to_string())
    }
}

/// Fetch the authoritative quota windows for a provider using the local OAuth token.
/// Returns `None` on any failure (no creds, network error, unexpected shape) so the
/// caller can fall back to the local-log estimate.
pub fn fetch_quota(provider: Provider) -> Option<QuotaWindows> {
    let t = UreqTransport;
    let creds = match provider {
        Provider::Claude => load_claude_oauth()?,
        Provider::Codex => load_codex_oauth()?,
    };
    let req = match provider {
        Provider::Claude => build_claude_usage_request(&creds.access_token),
        Provider::Codex => build_codex_usage_request(&creds.access_token),
    };
    let body = t.send(&req).ok()?;
    let q = usage_api::parse_usage_response(&body);
    if q.five_hour.is_none() && q.seven_day.is_none() {
        None
    } else {
        Some(q)
    }
}
