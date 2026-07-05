//! Offline unit tests for the trust core (window math + pricing).
//! Run with: `cargo test -p usage-core` (no network / no default features needed).

use usage_core::model::*;
use usage_core::pricing;
use usage_core::windows_calc::*;

const MIN: i64 = 60 * 1000;
const HOUR: i64 = 60 * MIN;
const DAY: i64 = 24 * HOUR;

fn ev(ts_ms: i64, model: &str, input: u64, output: u64, cr: u64, cw: u64) -> UsageEvent {
    UsageEvent {
        ts_ms,
        model: model.to_string(),
        input,
        output,
        cache_read: cr,
        cache_write: cw,
        id: None,
    }
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "expected {a} ≈ {b}");
}

#[test]
fn five_hour_single_block_sums_and_bounds() {
    let base = 1_000_000_000_000; // arbitrary epoch ms
    let events = vec![
        ev(base, "claude-sonnet-4", 100, 50, 10, 5),
        ev(base + 30 * MIN, "claude-opus-4", 200, 80, 20, 0),
    ];
    let now = base + HOUR;
    let w = active_5h(&events, now);
    assert_eq!(w.start_ms, base);
    assert_eq!(w.end_ms, base + FIVE_HOURS_MS);
    assert_eq!(w.messages, 2);
    assert_eq!(w.tokens.input, 300);
    assert_eq!(w.tokens.output, 130);
    assert_eq!(w.tokens.cache_read, 30);
    assert_eq!(w.tokens.cache_write, 5);
    assert_eq!(w.tokens.total(), 465);
    // remaining = 5h - 1h = 4h
    assert_eq!(w.remaining_ms(), 4 * HOUR);
    assert_eq!(w.remaining_label(), "4h");
    assert_eq!(w.source, WindowSource::Estimate);
}

#[test]
fn five_hour_new_block_after_gap() {
    let base = 1_700_000_000_000;
    let events = vec![
        ev(base, "claude-sonnet-4", 100, 0, 0, 0),          // block 1
        ev(base + 6 * HOUR, "claude-sonnet-4", 999, 0, 0, 0), // 6h gap -> block 2
        ev(base + 6 * HOUR + 10 * MIN, "claude-opus-4", 1, 2, 3, 4),
    ];
    let now = base + 6 * HOUR + 20 * MIN;
    let w = active_5h(&events, now);
    // active block is block 2, anchored at base+6h
    assert_eq!(w.start_ms, base + 6 * HOUR);
    assert_eq!(w.messages, 2);
    assert_eq!(w.tokens.input, 1000);
    assert_eq!(w.tokens.output, 2);
}

#[test]
fn five_hour_boundary_exactly_5h_starts_new_block() {
    let base = 1_700_000_000_000;
    let events = vec![
        ev(base, "sonnet", 10, 0, 0, 0),
        ev(base + FIVE_HOURS_MS, "sonnet", 20, 0, 0, 0), // exactly 5h -> new block
    ];
    // now inside the second block
    let w = active_5h(&events, base + FIVE_HOURS_MS + MIN);
    assert_eq!(w.start_ms, base + FIVE_HOURS_MS);
    assert_eq!(w.tokens.input, 20);
    assert_eq!(w.messages, 1);
}

#[test]
fn five_hour_idle_returns_empty_window_at_now() {
    let base = 1_700_000_000_000;
    let events = vec![ev(base, "sonnet", 10, 0, 0, 0)];
    let now = base + 10 * HOUR; // long idle
    let w = active_5h(&events, now);
    assert_eq!(w.tokens.total(), 0);
    assert_eq!(w.messages, 0);
    assert_eq!(w.start_ms, now);
    assert_eq!(w.end_ms, now + FIVE_HOURS_MS);
}

#[test]
fn seven_day_trailing_window_filters_old_events() {
    let now = 2_000_000_000_000;
    let events = vec![
        ev(now - 8 * DAY, "sonnet", 500, 0, 0, 0), // too old
        ev(now - 3 * DAY, "sonnet", 100, 10, 0, 0), // in window
        ev(now - 1 * HOUR, "opus", 1, 2, 3, 4),     // in window
    ];
    let w = window_7d(&events, now, None);
    assert_eq!(w.messages, 2);
    assert_eq!(w.tokens.input, 101);
    assert_eq!(w.tokens.output, 12);
    assert_eq!(w.start_ms, now - SEVEN_DAYS_MS);
    assert_eq!(w.end_ms, now);
}

#[test]
fn seven_day_with_weekly_reset_counts_down_to_reset() {
    let now = 2_000_000_000_000;
    // reset configured 3 days in the future
    let reset = now + 3 * DAY;
    let events = vec![
        ev(now - 2 * DAY, "sonnet", 10, 0, 0, 0), // inside [now-4d, now+3d)
        ev(now - 5 * DAY, "sonnet", 20, 0, 0, 0), // OUTSIDE — older than now-4d
    ];
    let w = window_7d(&events, now, Some(reset));
    assert_eq!(w.end_ms, reset);
    assert_eq!(w.start_ms, reset - SEVEN_DAYS_MS);
    assert_eq!(w.remaining_ms(), 3 * DAY);
    assert_eq!(w.remaining_label(), "3d");
    // window is [reset-7d, reset) = [now-4d, now+3d): only the now-2d event qualifies
    assert_eq!(w.messages, 1);
    assert_eq!(w.tokens.input, 10);
}

#[test]
fn weekly_reset_in_past_normalizes_forward() {
    let now = 2_000_000_000_000;
    let reset_past = now - 2 * DAY; // reset was 2 days ago
    let w = window_7d(&[], now, Some(reset_past));
    // next reset should be 5 days out (7 - 2)
    assert_eq!(w.remaining_ms(), 5 * DAY);
}

#[test]
fn budget_utilization_and_clamp() {
    let base = 1_700_000_000_000;
    let events = vec![ev(base, "sonnet", 300, 100, 0, 0)]; // 400 tokens
    let w = with_budget(active_5h(&events, base + MIN), 1000);
    approx(w.utilization.unwrap(), 0.4);
    approx(w.percent().unwrap(), 40.0);
    // over budget clamps to 1.0
    let w2 = with_budget(active_5h(&events, base + MIN), 100);
    approx(w2.utilization.unwrap(), 1.0);
}

#[test]
fn provider_api_overrides_estimate() {
    let base = 1_700_000_000_000;
    let w = active_5h(&[ev(base, "sonnet", 1, 1, 0, 0)], base + MIN);
    let reset = base + 3 * HOUR;
    let w = apply_provider(w, 0.36, reset);
    approx(w.percent().unwrap(), 36.0);
    assert_eq!(w.end_ms, reset);
    assert_eq!(w.source, WindowSource::ProviderApi);
}

#[test]
fn pricing_known_values() {
    // 1M input on opus = $15
    let e = ev(0, "claude-opus-4-6", 1_000_000, 0, 0, 0);
    approx(pricing::event_cost_usd(&e), 15.0);
    // 1M output on sonnet = $15
    let e = ev(0, "claude-sonnet-4-6", 0, 1_000_000, 0, 0);
    approx(pricing::event_cost_usd(&e), 15.0);
    // 1M cache_read on haiku = $0.08
    let e = ev(0, "claude-haiku-4-5", 0, 0, 1_000_000, 0);
    approx(pricing::event_cost_usd(&e), 0.08);
}

#[test]
fn pricing_totals() {
    let events = vec![
        ev(0, "opus", 1_000_000, 0, 0, 0), // $15
        ev(0, "sonnet", 0, 1_000_000, 0, 0), // $15
    ];
    approx(pricing::total_cost_usd(&events), 30.0);
}

#[test]
fn remaining_label_formats() {
    let mut w = WindowStat::default();
    w.now_ms = 0;
    w.end_ms = 3 * DAY + 5 * HOUR;
    assert_eq!(w.remaining_label(), "3d");
    w.end_ms = 4 * HOUR + 20 * MIN;
    assert_eq!(w.remaining_label(), "4h");
    w.end_ms = 45 * MIN;
    assert_eq!(w.remaining_label(), "45m");
}
