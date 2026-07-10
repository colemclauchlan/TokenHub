//! HTTPS transport for the provider usage API (injected into usage-core), plus a
//! best-effort quota fetch that never blocks the UI on failure.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use usage_core::model::Provider;
use usage_core::usage_api::{
    self, build_claude_usage_request, build_codex_usage_request, load_claude_oauth,
    load_codex_oauth, HttpRequest, QuotaWindows, Transport,
};

/// Minimal blocking HTTPS transport backed by ureq.
pub struct UreqTransport;

impl Transport for UreqTransport {
    fn send(&self, req: &HttpRequest) -> Result<String, String> {
        let mut r = ureq::request(req.method, &req.url).timeout(Duration::from_secs(10));
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

/// Minimum time between network attempts per provider. The usage endpoints
/// rate-limit aggressively — polling on every UI refresh tick gets 429'd into
/// permanent estimate mode, which is exactly the inaccuracy we must avoid.
const POLL_INTERVAL: Duration = Duration::from_secs(60);
/// How long the last good API result keeps being served while fetches fail
/// (rate limit blip, network hiccup). Past this we fall back to the estimate.
const STALE_MAX: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
struct CacheEntry {
    last_attempt: Option<Instant>,
    last_good: Option<(Instant, QuotaWindows)>,
}

fn cache() -> &'static Mutex<HashMap<&'static str, CacheEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, CacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_quota(e: &CacheEntry, now: Instant) -> Option<QuotaWindows> {
    match e.last_good {
        Some((t, q)) if now.duration_since(t) <= STALE_MAX => Some(q),
        _ => None,
    }
}

/// Fetch the authoritative quota windows for a provider, throttled and cached.
/// Hits the network at most once per `POLL_INTERVAL`; between polls (and across
/// transient failures) it serves the last good result so the UI stays on the
/// authoritative numbers instead of flapping back to the local estimate.
/// Returns `None` only when no recent API result exists.
pub fn fetch_quota(provider: Provider) -> Option<QuotaWindows> {
    let key = match provider {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
    };
    let now = Instant::now();
    {
        let map = cache().lock().unwrap();
        if let Some(e) = map.get(key) {
            if matches!(e.last_attempt, Some(t) if now.duration_since(t) < POLL_INTERVAL) {
                return cached_quota(e, now);
            }
        }
    }

    let fresh = fetch_quota_uncached(provider);

    let mut map = cache().lock().unwrap();
    let e = map.entry(key).or_default();
    e.last_attempt = Some(now);
    if let Some(q) = fresh {
        e.last_good = Some((now, q));
        return Some(q);
    }
    cached_quota(e, now)
}

/// One network round-trip using the local OAuth token. Returns `None` on any
/// failure (no creds, network error, unexpected shape).
fn fetch_quota_uncached(provider: Provider) -> Option<QuotaWindows> {
    let t = UreqTransport;
    let creds = match provider {
        Provider::Claude => load_claude_oauth()?,
        Provider::Codex => load_codex_oauth()?,
    };
    let req = match provider {
        Provider::Claude => build_claude_usage_request(&creds.access_token),
        Provider::Codex => {
            build_codex_usage_request(&creds.access_token, creds.account_id.as_deref())
        }
    };
    let body = t.send(&req).ok()?;
    let q = usage_api::parse_usage_response(&body);
    if q.five_hour.is_none() && q.seven_day.is_none() {
        None
    } else {
        Some(q)
    }
}
