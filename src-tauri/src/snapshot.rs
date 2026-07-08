//! Build the UI `Snapshot` (per-provider) from parsed usage events + provider quota.
//! Field names are camelCase to match the web frontend exactly.

use crate::config::Settings;
use crate::provider;
use chrono::{Datelike, Local, TimeZone, Timelike};
use serde::Serialize;
use usage_core::aggregate;
use usage_core::model::{Provider, UsageEvent, WindowSource};
use usage_core::pricing;
use usage_core::windows_calc;

#[derive(Serialize, Clone, Default)]
pub struct Breakdown {
    #[serde(rename = "cacheRead")]
    pub cache_read: u64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: u64,
    pub input: u64,
    pub output: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LimitWin {
    pub pct: u32,
    pub reset_label: String,
    pub reset_clock: String,
    pub source: String,
}

#[derive(Serialize, Clone)]
pub struct Limits {
    #[serde(rename = "fiveHour")]
    pub five_hour: LimitWin,
    #[serde(rename = "sevenDay")]
    pub seven_day: LimitWin,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Hero {
    pub tokens: u64,
    pub sessions: u64,
    pub messages: u64,
    pub cost_usd: f64,
    pub breakdown: Breakdown,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Today {
    pub msgs: u64,
    pub sessions: u64,
    pub tools: u64,
    pub tokens: u64,
    pub cost_usd: f64,
    pub breakdown: Breakdown,
    pub last_hour_rate_per_min: u64,
}

#[derive(Serialize, Clone)]
pub struct TrendItem {
    pub day: String,
    pub msgs: u64,
    pub tokens: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrendPills {
    pub avg_per_day: String,
    pub total_msgs: String,
    pub total_tokens: String,
}

#[derive(Serialize, Clone)]
pub struct ModelItem {
    pub name: String,
    pub color: String,
    #[serde(rename = "in")]
    pub input: u64,
    #[serde(rename = "out")]
    pub output: u64,
}

#[derive(Serialize, Clone)]
pub struct Models {
    pub total: u64,
    pub list: Vec<ModelItem>,
}

#[derive(Serialize, Clone)]
pub struct ProviderSnapshot {
    pub provider: String,
    pub title: String,
    pub logo: String,
    pub plan: String,
    pub since: String,
    pub limits: Limits,
    pub hero: Hero,
    pub today: Today,
    pub sparkline: Vec<u64>,
    pub trend: Vec<TrendItem>,
    #[serde(rename = "trendPills")]
    pub trend_pills: TrendPills,
    pub models: Models,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub claude_usd: f64,
    pub openai_usd: f64,
    pub total_usd: f64,
    pub usd_to_cad: f64,
    pub total_cad: f64,
    pub currency: String,
}

/// Agent-status indicator for the widget: the chosen chat's live state.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Indicator {
    /// "working" (green) | "waiting" (yellow) | "stopped" (red) | "none".
    pub status: String,
    pub chat: String,
    pub model: String,
    pub id: String,
}

#[derive(Serialize, Clone)]
pub struct AllSnapshots {
    pub claude: ProviderSnapshot,
    pub codex: ProviderSnapshot,
    pub subscription: Subscription,
    #[serde(rename = "pctRemaining")]
    pub pct_remaining: bool,
    pub indicator: Indicator,
    #[serde(rename = "jokeMode")]
    pub joke_mode: bool,
}

fn fc(n: u64) -> String {
    let f = n as f64;
    if f >= 1e9 {
        format!("{:.1}B", f / 1e9)
    } else if f >= 1e6 {
        format!("{:.1}M", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.1}K", f / 1e3)
    } else {
        n.to_string()
    }
}

const MODEL_COLORS: [&str; 6] = [
    "#d0774a", "#4aa8c9", "#43b0a3", "#57b26a", "#d9a441", "#9f7bd0",
];

fn weekday2(ms: i64) -> String {
    let dt = Local.timestamp_millis_opt(ms).single();
    match dt.map(|d| d.weekday()) {
        Some(chrono::Weekday::Mon) => "Mo",
        Some(chrono::Weekday::Tue) => "Tu",
        Some(chrono::Weekday::Wed) => "We",
        Some(chrono::Weekday::Thu) => "Th",
        Some(chrono::Weekday::Fri) => "Fr",
        Some(chrono::Weekday::Sat) => "Sa",
        Some(chrono::Weekday::Sun) => "Su",
        None => "--",
    }
    .to_string()
}

fn start_of_today_ms() -> i64 {
    let now = Local::now();
    let naive = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap_or_else(|| now.naive_local());
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|| now.timestamp_millis())
}

fn tz_offset_ms() -> i64 {
    Local::now().offset().local_minus_utc() as i64 * 1000
}

fn breakdown(tb: usage_core::model::TokenBreakdown) -> Breakdown {
    Breakdown {
        cache_read: tb.cache_read,
        cache_write: tb.cache_write,
        input: tb.input,
        output: tb.output,
    }
}

const WEEK_MS: i64 = 7 * 24 * 60 * 60 * 1000;

fn fmt_countdown(ms: i64) -> String {
    let s = (ms / 1000).max(0);
    let d = s / 86_400;
    let h = (s % 86_400) / 3_600;
    let m = (s % 3_600) / 60;
    if d > 0 {
        format!("{}d {}h", d, h)
    } else if h > 0 {
        format!("{}h {}m", h, m)
    } else {
        format!("{}m", m)
    }
}

fn fmt_clock(end_ms: i64) -> String {
    match Local.timestamp_millis_opt(end_ms).single() {
        Some(dt) => {
            let h24 = dt.hour();
            let m = dt.minute();
            let ampm = if h24 < 12 { "am" } else { "pm" };
            let h12 = match h24 % 12 {
                0 => 12,
                x => x,
            };
            format!("{}:{:02}{}", h12, m, ampm)
        }
        None => String::new(),
    }
}

/// A stable weekly reset anchored at first-ever use (used only in estimate mode
/// when the provider API hasn't given a real reset), so the 7-day countdown works.
fn rolling_weekly_reset(events: &[UsageEvent], now: i64) -> i64 {
    let first = events.iter().map(|e| e.ts_ms).min().unwrap_or(now);
    if first >= now {
        return now + WEEK_MS;
    }
    let k = (now - first) / WEEK_MS + 1;
    first + k * WEEK_MS
}

fn fmt_date(end_ms: i64) -> String {
    match Local.timestamp_millis_opt(end_ms).single() {
        Some(dt) => format!("{} {}", month_abbr(dt.month()), dt.day()),
        None => String::new(),
    }
}

/// `weekly` chooses the alternate readout: a date ("Jul 16") for the 7-day window,
/// a clock time ("7:30pm") for the 5-hour window.
fn build_limit(w: &usage_core::model::WindowStat, weekly: bool) -> LimitWin {
    LimitWin {
        pct: w.percent().unwrap_or(0.0).round() as u32,
        reset_label: fmt_countdown(w.remaining_ms()),
        reset_clock: if weekly { fmt_date(w.end_ms) } else { fmt_clock(w.end_ms) },
        source: match w.source {
            WindowSource::ProviderApi => "providerApi".into(),
            WindowSource::Estimate => "estimate".into(),
        },
    }
}

fn build_provider(
    provider: Provider,
    events: &[UsageEvent],
    settings: &Settings,
    budgets: (u64, u64),
    weekly_reset: Option<i64>,
) -> ProviderSnapshot {
    let now = chrono::Utc::now().timestamp_millis();
    let today0 = start_of_today_ms();
    let tzo = tz_offset_ms();

    // hero (all-time in the logs)
    let hero_bd = aggregate::total_breakdown(events);
    let hero = Hero {
        tokens: hero_bd.total(),
        sessions: aggregate::sessions_by_gap(events, 30 * 60 * 1000),
        messages: events.len() as u64,
        cost_usd: pricing::total_cost_usd(events),
        breakdown: breakdown(hero_bd),
    };

    // today
    let today_events: Vec<UsageEvent> =
        events.iter().filter(|e| e.ts_ms >= today0).cloned().collect();
    let today_bd = aggregate::total_breakdown(&today_events);
    let last_hour: u64 = events
        .iter()
        .filter(|e| e.ts_ms >= now - 3_600_000)
        .map(|e| e.total())
        .sum();
    let today = Today {
        msgs: today_events.len() as u64,
        sessions: aggregate::sessions_by_gap(&today_events, 30 * 60 * 1000),
        tools: 0, // TODO(P6): count tool_use blocks
        tokens: today_bd.total(),
        cost_usd: pricing::total_cost_usd(&today_events),
        breakdown: breakdown(today_bd),
        last_hour_rate_per_min: last_hour / 60,
    };

    // limits (estimate first, then provider-API override if enabled)
    let mut five = windows_calc::with_budget(windows_calc::active_5h(events, now), budgets.0);
    let wr = weekly_reset.or_else(|| Some(rolling_weekly_reset(events, now)));
    let mut seven =
        windows_calc::with_budget(windows_calc::window_7d(events, now, wr), budgets.1);
    if settings.use_provider_api {
        if let Some(q) = provider::fetch_quota(provider) {
            if let Some(w) = q.five_hour {
                five = windows_calc::apply_provider(five, w.utilization, w.reset_ms);
            }
            if let Some(w) = q.seven_day {
                seven = windows_calc::apply_provider(seven, w.utilization, w.reset_ms);
            }
        }
    }

    // sparkline + trend
    let sparkline = aggregate::sparkline_last_hour(events, now, 60);
    let trend_raw = aggregate::trend_by_day(events, now, 14, tzo);
    let total_msgs: u64 = trend_raw.iter().map(|(_, m, _)| *m).sum();
    let total_tokens: u64 = trend_raw.iter().map(|(_, _, t)| *t).sum();
    let trend: Vec<TrendItem> = trend_raw
        .iter()
        .map(|(day_ms, m, t)| TrendItem {
            day: weekday2(*day_ms),
            msgs: *m,
            tokens: *t,
        })
        .collect();
    let trend_pills = TrendPills {
        // Plain text — the frontend adds the 💬 icon (avoids doubling it up).
        avg_per_day: format!("{} msgs/day", fc(total_msgs / 14)),
        total_msgs: format!("Σ {} total msgs", fc(total_msgs)),
        total_tokens: format!("# {} tokens", fc(total_tokens)),
    };

    // models
    let msum = aggregate::models_summary(events);
    let mtotal: u64 = msum.iter().map(|(_, i, o)| i + o).sum();
    let list: Vec<ModelItem> = msum
        .iter()
        .take(6)
        .enumerate()
        .map(|(i, (name, inp, out))| ModelItem {
            name: name.clone(),
            color: MODEL_COLORS[i % MODEL_COLORS.len()].to_string(),
            input: *inp,
            output: *out,
        })
        .collect();

    // since (earliest event)
    let since = events
        .iter()
        .map(|e| e.ts_ms)
        .min()
        .and_then(|ms| Local.timestamp_millis_opt(ms).single())
        .map(|d| format!("since {} {} ›", month_abbr(d.month()), d.day()))
        .unwrap_or_else(|| "no data yet".into());

    let (title, logo, plan) = match provider {
        Provider::Claude => ("Claude Code", "✳", "Max 5×  $100/mo"),
        Provider::Codex => ("Codex", "◯", "Plus  $20/mo"),
    };

    ProviderSnapshot {
        provider: match provider {
            Provider::Claude => "claude".into(),
            Provider::Codex => "codex".into(),
        },
        title: title.into(),
        logo: logo.into(),
        plan: plan.into(),
        since,
        limits: Limits {
            five_hour: build_limit(&five, false),
            seven_day: build_limit(&seven, true),
        },
        hero,
        today,
        sparkline,
        trend,
        trend_pills,
        models: Models { total: mtotal, list },
    }
}

fn month_abbr(m: u32) -> &'static str {
    ["", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
        .get(m as usize)
        .copied()
        .unwrap_or("")
}

/// Roots that hold Claude usage logs: the CLI's `~/.claude/projects` plus the
/// Cowork desktop app's per-session dirs under app-data.
pub fn claude_log_roots() -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();
    if let Some(d) = usage_core::logs_claude::claude_dir() {
        roots.push(d.join("projects"));
    }
    for key in ["APPDATA", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(key) {
            roots.push(std::path::PathBuf::from(base).join("Claude").join("local-agent-mode-sessions"));
        }
    }
    roots
}

/// Files older than this are skipped when parsing (keeps the 5h/7d/14d windows
/// exact while bounding work over a large Cowork history). ~45 days back.
fn recent_cutoff_ms() -> i64 {
    chrono::Utc::now().timestamp_millis() - 45 * 24 * 3600 * 1000
}

pub fn build_all(settings: &Settings) -> AllSnapshots {
    // Claude usage comes from BOTH the CLI (~/.claude/projects) and the Cowork
    // desktop app (%APPDATA%/%LOCALAPPDATA%\Claude\local-agent-mode-sessions).
    let claude_events = usage_core::logs_claude::parse_trees_since(&claude_log_roots(), recent_cutoff_ms());
    let codex_events = usage_core::logs_codex::codex_dir()
        .map(|d| usage_core::logs_codex::parse_all(&d))
        .unwrap_or_default();

    // Combined monthly subscription cost (only count services being tracked).
    let claude_usd = if settings.track_claude { settings.claude_plan_usd } else { 0.0 };
    let openai_usd = if settings.track_codex { settings.openai_plan_usd } else { 0.0 };
    let total_usd = claude_usd + openai_usd;
    let rate = if settings.usd_to_cad > 0.0 { settings.usd_to_cad } else { 1.38 };
    let subscription = Subscription {
        claude_usd,
        openai_usd,
        total_usd,
        usd_to_cad: rate,
        total_cad: (total_usd * rate * 100.0).round() / 100.0,
        currency: "CAD".into(),
    };

    // Widget agent-status indicator: the chosen chat, else the most-recent one.
    let sessions = crate::sessions::all_sessions();
    let sel = if settings.indicator_session_id.is_empty() {
        sessions.first()
    } else {
        sessions
            .iter()
            .find(|s| s.id == settings.indicator_session_id)
            .or_else(|| sessions.first())
    };
    let indicator = match sel {
        Some(s) => Indicator {
            status: crate::sessions::agent_status(s.last_ms).to_string(),
            chat: s.name.clone(),
            model: s.model.clone(),
            id: s.id.clone(),
        },
        None => Indicator {
            status: "none".into(),
            chat: String::new(),
            model: String::new(),
            id: String::new(),
        },
    };

    AllSnapshots {
        claude: build_provider(
            Provider::Claude,
            &claude_events,
            settings,
            (settings.claude_5h_budget, settings.claude_7d_budget),
            settings.claude_weekly_reset_ms,
        ),
        codex: build_provider(
            Provider::Codex,
            &codex_events,
            settings,
            (settings.codex_5h_budget, settings.codex_7d_budget),
            settings.codex_weekly_reset_ms,
        ),
        subscription,
        pct_remaining: settings.pct_remaining,
        indicator,
        joke_mode: settings.joke_mode,
    }
}
