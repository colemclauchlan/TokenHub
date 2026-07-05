//! Pure aggregation helpers that turn a stream of `UsageEvent`s into the summary
//! shapes the UI needs (hero totals, session count, sparkline, 14-day trend, model
//! breakdown). No external deps → fully unit-tested offline.

use crate::model::{TokenBreakdown, UsageEvent};
use std::collections::HashMap;

/// Count sessions by time-gap clustering: a new session begins after a gap ≥ `gap_ms`.
pub fn sessions_by_gap(events: &[UsageEvent], gap_ms: i64) -> u64 {
    let mut ev: Vec<&UsageEvent> = events.iter().collect();
    ev.sort_by_key(|e| e.ts_ms);
    let mut sessions = 0u64;
    let mut last = i64::MIN;
    for e in ev {
        if e.ts_ms.saturating_sub(last) >= gap_ms {
            sessions += 1;
        }
        last = e.ts_ms;
    }
    sessions
}

/// Sum a breakdown over all events.
pub fn total_breakdown(events: &[UsageEvent]) -> TokenBreakdown {
    let mut tb = TokenBreakdown::default();
    for e in events {
        tb.add_event(e);
    }
    tb
}

/// Bin the last hour into `bins` buckets (tokens per bucket), ending at `now_ms`.
pub fn sparkline_last_hour(events: &[UsageEvent], now_ms: i64, bins: usize) -> Vec<u64> {
    let mut v = vec![0u64; bins.max(1)];
    let hour = 3_600_000i64;
    let start = now_ms - hour;
    let span = (hour / bins.max(1) as i64).max(1);
    for e in events {
        if e.ts_ms >= start && e.ts_ms < now_ms {
            let idx = (((e.ts_ms - start) / span) as usize).min(v.len() - 1);
            v[idx] += e.total();
        }
    }
    v
}

/// Group by local day for the last `days` days. Returns `(day_start_ms, msgs, tokens)`
/// oldest-first. `tz_offset_ms` shifts UTC to the local day boundary.
pub fn trend_by_day(
    events: &[UsageEvent],
    now_ms: i64,
    days: usize,
    tz_offset_ms: i64,
) -> Vec<(i64, u64, u64)> {
    let day = 86_400_000i64;
    let local_day = |ts: i64| (ts + tz_offset_ms).div_euclid(day);
    let today = local_day(now_ms);
    let start_day = today - (days as i64 - 1);
    let mut out: Vec<(i64, u64, u64)> = (0..days)
        .map(|i| ((start_day + i as i64) * day - tz_offset_ms, 0u64, 0u64))
        .collect();
    for e in events {
        let d = local_day(e.ts_ms);
        if d >= start_day && d <= today {
            let idx = (d - start_day) as usize;
            out[idx].1 += 1;
            out[idx].2 += e.total();
        }
    }
    out
}

/// Group tokens by model: `(model, in_bucket, out_bucket)` sorted by total desc.
/// `in_bucket` folds input + cache read + cache write; `out_bucket` is output.
pub fn models_summary(events: &[UsageEvent]) -> Vec<(String, u64, u64)> {
    let mut map: HashMap<String, (u64, u64)> = HashMap::new();
    for e in events {
        let ent = map.entry(e.model.clone()).or_default();
        ent.0 += e.input + e.cache_read + e.cache_write;
        ent.1 += e.output;
    }
    let mut v: Vec<(String, u64, u64)> = map.into_iter().map(|(k, (i, o))| (k, i, o)).collect();
    v.sort_by_key(|(_, i, o)| std::cmp::Reverse(i + o));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    const MIN: i64 = 60_000;
    const HOUR: i64 = 60 * MIN;
    const DAY: i64 = 24 * HOUR;

    fn ev(ts: i64, model: &str, i: u64, o: u64) -> UsageEvent {
        UsageEvent { ts_ms: ts, model: model.into(), input: i, output: o, cache_read: 0, cache_write: 0, id: None }
    }

    #[test]
    fn sessions_gap_clustering() {
        let base = 1_000_000_000_000;
        let events = vec![
            ev(base, "s", 1, 1),
            ev(base + 5 * MIN, "s", 1, 1),      // same session
            ev(base + 60 * MIN, "s", 1, 1),     // new session (>30m gap)
            ev(base + 62 * MIN, "s", 1, 1),     // same
        ];
        assert_eq!(sessions_by_gap(&events, 30 * MIN), 2);
    }

    #[test]
    fn sparkline_bins_last_hour() {
        let now = 2_000_000_000_000;
        let events = vec![
            ev(now - 59 * MIN, "s", 100, 0), // bin 0
            ev(now - 1 * MIN, "s", 200, 0),  // last bin
            ev(now - 2 * HOUR, "s", 999, 0), // outside window
        ];
        let bins = sparkline_last_hour(&events, now, 60);
        assert_eq!(bins.len(), 60);
        assert_eq!(bins[0], 100);
        assert_eq!(bins[59], 200);
        assert_eq!(bins.iter().sum::<u64>(), 300);
    }

    #[test]
    fn trend_groups_by_day() {
        let now = 100 * DAY + 12 * HOUR; // midday
        let events = vec![
            ev(now, "s", 10, 5),               // today
            ev(now - 1 * DAY, "s", 20, 0),     // yesterday
            ev(now - 1 * DAY - HOUR, "s", 1, 0), // yesterday
            ev(now - 20 * DAY, "s", 500, 0),   // outside 14d
        ];
        let t = trend_by_day(&events, now, 14, 0);
        assert_eq!(t.len(), 14);
        assert_eq!(t[13].1, 1); // today: 1 msg
        assert_eq!(t[13].2, 15);
        assert_eq!(t[12].1, 2); // yesterday: 2 msgs
        assert_eq!(t[12].2, 21);
    }

    #[test]
    fn models_summary_sorts_desc() {
        let events = vec![
            ev(0, "opus", 100, 900),   // total 1000
            ev(0, "sonnet", 50, 50),   // total 100
            ev(0, "opus", 0, 100),     // opus total -> 1100
        ];
        let m = models_summary(&events);
        assert_eq!(m[0].0, "opus");
        assert_eq!(m[0].1, 100); // in
        assert_eq!(m[0].2, 1000); // out (900+100)
        assert_eq!(m[1].0, "sonnet");
    }
}
