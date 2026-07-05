//! Persistent settings, stored at `%USERPROFILE%\.ai-usage-bar\config.json`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
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
