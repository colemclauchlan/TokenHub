//! usage-core — the provider-agnostic heart of AI Usage Bar.
//!
//! Layers:
//!   * `model`         — normalized structs shared with the web UI (no deps)
//!   * `windows_calc`  — 5h rolling-block + 7d window math (no deps, fully tested)
//!   * `pricing`       — model → API list price, cost estimates (no deps)
//!   * `logs_claude`   — parse ~/.claude JSONL + stats-cache   (feature = "io")
//!   * `logs_codex`    — parse ~/.codex sessions rollout JSONL  (feature = "io")
//!   * `usage_api`     — provider usage endpoint request/parse  (feature = "io")
//!
//! The `io` modules only build request/parse data and read local files; the actual
//! HTTP transport is injected by the app crate so the core stays testable & light.

pub mod aggregate;
pub mod model;
pub mod pricing;
pub mod windows_calc;

#[cfg(feature = "io")]
pub mod logs_claude;
#[cfg(feature = "io")]
pub mod logs_codex;
#[cfg(feature = "io")]
pub mod usage_api;

pub use model::*;
