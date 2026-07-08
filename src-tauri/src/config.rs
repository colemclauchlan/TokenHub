//! Persistent settings, stored at `%USERPROFILE%\.ai-usage-bar\config.json`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Read local OAuth + call the provider usage API so 5h/7d match the CLI counter.
    pub use_provider_api: bool,
    pub refresh_secs: u64,
    pub minibar_enabled: bool,
    /// "bottomLeft" | "bottomRight" | "topLeft" | "topRight" | "nearTray"
    pub minibar_corner: String,
    pub track_claude: bool,
    pub track_codex: bool,
    pub hotkey: String,
    pub autostart: bool,
    /// Estimate-mode token budgets (only used when the provider API is off/unavailable).
    pub claude_5h_budget: u64,
    pub claude_7d_budget: u64,
    pub codex_5h_budget: u64,
    pub codex_7d_budget: u64,
    /// Optional fixed weekly reset (epoch ms) for estimate mode; None = trailing 7d.
    pub claude_weekly_reset_ms: Option<i64>,
    pub codex_weekly_reset_ms: Option<i64>,
    /// User-defined display names: session id → name, project key → name.
    #[serde(default)]
    pub session_aliases: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub project_aliases: std::collections::HashMap<String, String>,
    /// Monthly subscription prices (USD) + FX rate for the combined CAD readout.
    #[serde(default = "default_claude_plan_usd")]
    pub claude_plan_usd: f64,
    #[serde(default = "default_openai_plan_usd")]
    pub openai_plan_usd: f64,
    #[serde(default = "default_usd_to_cad")]
    pub usd_to_cad: f64,
    /// GitHub username/profile for listing repos in the Git tab (public API).
    /// `github_user` is the legacy single value; `github_users` supports several.
    #[serde(default)]
    pub github_user: String,
    #[serde(default)]
    pub github_users: Vec<String>,
    /// GitHub accounts toggled off (hidden from the Git tab but kept in the list).
    #[serde(default)]
    pub github_users_disabled: Vec<String>,
    /// Tray icon appearance: style ("fill" | "ring" | "bar") + color ("multi" | "mono").
    #[serde(default = "default_tray_style")]
    pub tray_style: String,
    #[serde(default = "default_tray_color")]
    pub tray_color: String,
    /// Show remaining % instead of used % in the panel, mini-bar, and tooltip.
    #[serde(default)]
    pub pct_remaining: bool,
    /// Desktop notifications when a usage window crosses 75/90/95%.
    #[serde(default = "default_true")]
    pub notify_enabled: bool,
    /// Named presets of display settings, and the currently active one.
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub active_profile: String,
    /// Session id whose agent status drives the widget indicator light
    /// (empty = follow the most-recently-active chat).
    #[serde(default)]
    pub indicator_session_id: String,
    /// Joke "water guilt" meter on the Usage tab (off by default).
    #[serde(default)]
    pub joke_mode: bool,
}

/// A saved preset of display settings the user can switch between.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub track_claude: bool,
    #[serde(default)]
    pub track_codex: bool,
    #[serde(default)]
    pub tray_style: String,
    #[serde(default)]
    pub tray_color: String,
    #[serde(default)]
    pub pct_remaining: bool,
}

fn default_true() -> bool {
    true
}
fn default_tray_style() -> String {
    "fill".into()
}
fn default_tray_color() -> String {
    "multi".into()
}

fn default_claude_plan_usd() -> f64 {
    100.0
}
fn default_openai_plan_usd() -> f64 {
    20.0
}
fn default_usd_to_cad() -> f64 {
    1.38
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            use_provider_api: true,
            refresh_secs: 5,
            minibar_enabled: true,
            minibar_corner: "bottomLeft".into(),
            track_claude: true,
            track_codex: true,
            hotkey: "CmdOrControl+Shift+U".into(),
            autostart: false,
            // rough defaults; real % comes from the provider API
            claude_5h_budget: 20_000_000,
            claude_7d_budget: 300_000_000,
            codex_5h_budget: 8_000_000,
            codex_7d_budget: 120_000_000,
            claude_weekly_reset_ms: None,
            codex_weekly_reset_ms: None,
            session_aliases: std::collections::HashMap::new(),
            project_aliases: std::collections::HashMap::new(),
            claude_plan_usd: 100.0,
            openai_plan_usd: 20.0,
            usd_to_cad: 1.38,
            github_user: String::new(),
            github_users: Vec::new(),
            github_users_disabled: Vec::new(),
            tray_style: "fill".into(),
            tray_color: "multi".into(),
            pct_remaining: false,
            notify_enabled: true,
            profiles: Vec::new(),
            active_profile: String::new(),
            indicator_session_id: String::new(),
            joke_mode: false,
        }
    }
}

pub fn config_dir() -> Option<PathBuf> {
    usage_core::logs_claude::home_dir().map(|h| h.join(".ai-usage-bar"))
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}

pub fn load() -> Settings {
    let Some(path) = config_path() else { return Settings::default() };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(s: &Settings) -> std::io::Result<()> {
    if let Some(dir) = config_dir() {
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.json");
        std::fs::write(path, serde_json::to_string_pretty(s).unwrap_or_default())?;
    }
    Ok(())
}
