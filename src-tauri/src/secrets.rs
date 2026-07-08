//! Secure API-key storage via the OS credential store (Windows Credential
//! Manager) using the `keyring` crate. Keys are never written to config.json and
//! never returned to the UI — only stored, checked for presence, validated with a
//! cheap authenticated request, and deleted.

const SERVICE: &str = "TokenHub";

fn account(provider: &str) -> &'static str {
    if provider == "openai" {
        "openai_api_key"
    } else {
        "anthropic_api_key"
    }
}

fn entry(provider: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account(provider)).map_err(|e| e.to_string())
}

/// Store (or overwrite) the API key for a provider.
pub fn set_key(provider: &str, key: &str) -> Result<(), String> {
    entry(provider)?.set_password(key).map_err(|e| e.to_string())
}

/// Retrieve the stored key (used internally for validation only).
pub fn get_key(provider: &str) -> Option<String> {
    entry(provider).ok()?.get_password().ok()
}

/// Whether a non-empty key is stored for the provider.
pub fn has_key(provider: &str) -> bool {
    get_key(provider).map(|k| !k.is_empty()).unwrap_or(false)
}

/// Delete the stored key (no-op if none present).
pub fn clear_key(provider: &str) -> Result<(), String> {
    match entry(provider)?.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Validate the stored key with a cheap authenticated request (`GET /v1/models`).
pub fn validate(provider: &str) -> Result<(), String> {
    let key = get_key(provider).ok_or_else(|| "no key stored".to_string())?;
    let req = if provider == "openai" {
        ureq::get("https://api.openai.com/v1/models").set("Authorization", &format!("Bearer {key}"))
    } else {
        ureq::get("https://api.anthropic.com/v1/models")
            .set("x-api-key", &key)
            .set("anthropic-version", "2023-06-01")
    };
    match req.call() {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, _)) => Err(format!("HTTP {code}")),
        Err(e) => Err(e.to_string()),
    }
}
