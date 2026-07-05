//! Model → API list price, and cost estimation.
//!
//! IMPORTANT: these are *API list prices*, used only for the "API list est." hero
//! number ("what you'd pay at API prices") — NOT your subscription spend, matching
//! the reference app's disclaimer. Prices are USD per 1,000,000 tokens.
//!
//! Update `TABLE` in one place as prices change. Dated 2026-07; treat as illustrative.

use crate::model::UsageEvent;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelPrice {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

/// Look up a price by (loose) model-name match. Falls back to a Sonnet-ish default.
pub fn price_for(model: &str) -> ModelPrice {
    let m = model.to_ascii_lowercase();
    // Anthropic
    if m.contains("opus") {
        return ModelPrice { input: 15.0, output: 75.0, cache_read: 1.50, cache_write: 18.75 };
    }
    if m.contains("sonnet") {
        return ModelPrice { input: 3.0, output: 15.0, cache_read: 0.30, cache_write: 3.75 };
    }
    if m.contains("haiku") {
        return ModelPrice { input: 0.80, output: 4.0, cache_read: 0.08, cache_write: 1.0 };
    }
    // OpenAI / Codex family (illustrative — update when confirmed)
    if m.contains("gpt-5") || m.contains("codex") || m.starts_with("o4") || m.starts_with("o3") {
        return ModelPrice { input: 2.50, output: 10.0, cache_read: 0.25, cache_write: 2.50 };
    }
    if m.contains("gpt-4") {
        return ModelPrice { input: 2.50, output: 10.0, cache_read: 0.25, cache_write: 2.50 };
    }
    // default
    ModelPrice { input: 3.0, output: 15.0, cache_read: 0.30, cache_write: 3.75 }
}

/// Cost of a single event in USD at API list prices.
pub fn event_cost_usd(e: &UsageEvent) -> f64 {
    let p = price_for(&e.model);
    (e.input as f64 * p.input
        + e.output as f64 * p.output
        + e.cache_read as f64 * p.cache_read
        + e.cache_write as f64 * p.cache_write)
        / 1_000_000.0
}

/// Total cost of many events in USD at API list prices.
pub fn total_cost_usd(events: &[UsageEvent]) -> f64 {
    events.iter().map(event_cost_usd).sum()
}
