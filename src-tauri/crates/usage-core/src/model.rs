//! Normalized data model shared across the app and serialized to the web UI.
//!
//! Timestamps are epoch **milliseconds UTC** (`i64`) everywhere in the core so the
//! window math needs no date library. Parsing (feature `io`) converts RFC3339 → ms.

pub const FIVE_HOURS_MS: i64 = 5 * 60 * 60 * 1000;
pub const SEVEN_DAYS_MS: i64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    pub fn label(&self) -> &'static str {
        match self {
            Provider::Claude => "Claude Code",
            Provider::Codex => "Codex",
        }
    }
}

/// One normalized usage record (typically one assistant turn).
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UsageEvent {
    /// epoch milliseconds, UTC
    pub ts_ms: i64,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// Optional dedup key (message id + request id) so repeated log lines don't double-count.
    pub id: Option<String>,
}

impl UsageEvent {
    /// All token classes summed.
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenBreakdown {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl TokenBreakdown {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
    pub fn add_event(&mut self, e: &UsageEvent) {
        self.input += e.input;
        self.output += e.output;
        self.cache_read += e.cache_read;
        self.cache_write += e.cache_write;
    }
}

/// Where a window's utilization figure came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum WindowSource {
    /// Computed locally from logs against a token budget (labelled "est.").
    #[default]
    Estimate,
    /// Read from the provider usage API — matches the in-app counter.
    ProviderApi,
}

/// A rolling / limit window (either the 5-hour block or the 7-day window).
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct WindowStat {
    pub start_ms: i64,
    /// reset time (window end)
    pub end_ms: i64,
    pub now_ms: i64,
    pub tokens: TokenBreakdown,
    pub messages: u64,
    /// 0.0..=1.0 if known (from API, or from a configured budget); else None.
    pub utilization: Option<f64>,
    pub source: WindowSource,
}

impl WindowStat {
    /// Milliseconds until the window resets (never negative).
    pub fn remaining_ms(&self) -> i64 {
        (self.end_ms - self.now_ms).max(0)
    }
    /// Utilization as a 0..100 percentage, if known.
    pub fn percent(&self) -> Option<f64> {
        self.utilization.map(|u| u * 100.0)
    }
    /// Human "3h" / "3d" style string for time remaining.
    pub fn remaining_label(&self) -> String {
        let secs = self.remaining_ms() / 1000;
        let days = secs / 86_400;
        let hours = (secs % 86_400) / 3_600;
        let mins = (secs % 3_600) / 60;
        if days >= 1 {
            format!("{}d", days)
        } else if hours >= 1 {
            format!("{}h", hours)
        } else {
            format!("{}m", mins)
        }
    }
}
