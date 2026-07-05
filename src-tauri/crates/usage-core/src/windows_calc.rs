//! Rolling-window math for the 5-hour block and 7-day window.
//!
//! These functions are pure (no external deps) and are the trust core of the app:
//! when the provider usage API isn't available we fall back to these *estimates*,
//! clearly labelled as such. When the API is available its utilization/reset values
//! overwrite `utilization`/`end_ms` and flip `source` to `ProviderApi`.

use crate::model::*;

/// Compute the active 5-hour block (ccusage-style gap/anchor logic).
///
/// A block starts at its first event and spans `[start, start + 5h)`. A new block
/// begins when an event lands 5h+ after the block start, **or** when there is a 5h+
/// gap since the previous event. Returns the block whose window contains `now_ms`;
/// if none does (idle), returns an empty window anchored at `now_ms`.
pub fn active_5h(events: &[UsageEvent], now_ms: i64) -> WindowStat {
    let mut ev: Vec<&UsageEvent> = events.iter().collect();
    ev.sort_by_key(|e| e.ts_ms);

    // (start_ms, tokens, messages)
    let mut blocks: Vec<(i64, TokenBreakdown, u64)> = Vec::new();
    let mut last_ts: i64 = i64::MIN;

    for e in &ev {
        let start_new = match blocks.last() {
            Some((start, _, _)) => {
                e.ts_ms - *start >= FIVE_HOURS_MS || e.ts_ms - last_ts >= FIVE_HOURS_MS
            }
            None => true,
        };
        if start_new {
            let mut tb = TokenBreakdown::default();
            tb.add_event(e);
            blocks.push((e.ts_ms, tb, 1));
        } else {
            let b = blocks.last_mut().unwrap();
            b.1.add_event(e);
            b.2 += 1;
        }
        last_ts = e.ts_ms;
    }

    match blocks
        .into_iter()
        .rev()
        .find(|(s, _, _)| now_ms >= *s && now_ms < *s + FIVE_HOURS_MS)
    {
        Some((start, tokens, msgs)) => WindowStat {
            start_ms: start,
            end_ms: start + FIVE_HOURS_MS,
            now_ms,
            tokens,
            messages: msgs,
            utilization: None,
            source: WindowSource::Estimate,
        },
        None => WindowStat {
            start_ms: now_ms,
            end_ms: now_ms + FIVE_HOURS_MS,
            now_ms,
            tokens: TokenBreakdown::default(),
            messages: 0,
            utilization: None,
            source: WindowSource::Estimate,
        },
    }
}

/// Compute the 7-day window.
///
/// * `weekly_reset_ms = Some(t)`: the account's fixed weekly reset. The function
///   normalizes it to the next reset ≥ `now_ms` and reports the window
///   `[reset - 7d, reset)` — so `remaining_ms()` counts down to that reset
///   (matches the "· 3d" style label).
/// * `weekly_reset_ms = None`: a pure trailing 7-day window ending at `now_ms`.
pub fn window_7d(events: &[UsageEvent], now_ms: i64, weekly_reset_ms: Option<i64>) -> WindowStat {
    let (start, end) = match weekly_reset_ms {
        Some(reset) => {
            let mut r = reset;
            // move forward to the next reset strictly in the future
            while r <= now_ms {
                r += SEVEN_DAYS_MS;
            }
            // and pull back if it's more than a period ahead (reset far in future)
            while r - SEVEN_DAYS_MS > now_ms {
                r -= SEVEN_DAYS_MS;
            }
            (r - SEVEN_DAYS_MS, r)
        }
        None => (now_ms - SEVEN_DAYS_MS, now_ms),
    };

    let mut tokens = TokenBreakdown::default();
    let mut msgs = 0u64;
    for e in events {
        if e.ts_ms >= start && e.ts_ms < end {
            tokens.add_event(e);
            msgs += 1;
        }
    }

    WindowStat {
        start_ms: start,
        end_ms: end,
        now_ms,
        tokens,
        messages: msgs,
        utilization: None,
        source: WindowSource::Estimate,
    }
}

/// Attach an estimated utilization from a token budget (plan limit, in tokens).
/// Clamped to 1.0. No-op if `budget_tokens == 0`.
pub fn with_budget(mut w: WindowStat, budget_tokens: u64) -> WindowStat {
    if budget_tokens > 0 {
        w.utilization = Some((w.tokens.total() as f64 / budget_tokens as f64).min(1.0));
    }
    w
}

/// Overwrite a window with authoritative values from the provider usage API.
/// `utilization` is 0.0..=1.0; `reset_ms` is the API-reported reset time.
pub fn apply_provider(mut w: WindowStat, utilization: f64, reset_ms: i64) -> WindowStat {
    w.utilization = Some(utilization.clamp(0.0, 1.0));
    if reset_ms > 0 {
        w.end_ms = reset_ms;
    }
    w.source = WindowSource::ProviderApi;
    w
}
