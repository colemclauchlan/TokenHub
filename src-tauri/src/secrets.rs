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

// ---- Environment-variable export (opt-in) ----------------------------------
//
// Writes a stored key to `HKCU\Environment` so other tools (CLIs, scripts,
// editors) pick it up as ANTHROPIC_API_KEY / OPENAI_API_KEY. This is strictly
// opt-in from Settings → Connections: unlike the credential store, environment
// variables are readable by any process the user runs.

/// The environment variable a provider's key is exported under.
pub fn env_var(provider: &str) -> &'static str {
    if provider == "openai" {
        "OPENAI_API_KEY"
    } else {
        "ANTHROPIC_API_KEY"
    }
}

#[cfg(windows)]
fn user_env() -> Result<winreg::RegKey, String> {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .create_subkey("Environment")
        .map(|(k, _)| k)
        .map_err(|e| e.to_string())
}

/// Current value of the provider's variable in the user environment, if any.
#[cfg(windows)]
pub fn env_value(provider: &str) -> Option<String> {
    user_env().ok()?.get_value::<String, _>(env_var(provider)).ok()
}

#[cfg(not(windows))]
pub fn env_value(_provider: &str) -> Option<String> {
    None
}

/// Whether the provider's key is exported to the user environment.
pub fn env_status(provider: &str) -> bool {
    env_value(provider).map(|v| !v.is_empty()).unwrap_or(false)
}

/// Export the stored key as a user environment variable.
#[cfg(windows)]
pub fn export_to_env(provider: &str) -> Result<(), String> {
    let key = get_key(provider).ok_or_else(|| "no key stored".to_string())?;
    user_env()?
        .set_value(env_var(provider), &key)
        .map_err(|e| e.to_string())?;
    broadcast_env_change();
    Ok(())
}

#[cfg(not(windows))]
pub fn export_to_env(_provider: &str) -> Result<(), String> {
    Err("environment export is Windows-only".into())
}

/// Remove the provider's variable from the user environment (no-op if absent).
#[cfg(windows)]
pub fn remove_from_env(provider: &str) -> Result<(), String> {
    match user_env()?.delete_value(env_var(provider)) {
        Ok(_) => {
            broadcast_env_change();
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(not(windows))]
pub fn remove_from_env(_provider: &str) -> Result<(), String> {
    Ok(())
}

/// Tell running apps (Explorer, new terminals) that the environment changed,
/// so the export takes effect without a sign-out.
#[cfg(windows)]
fn broadcast_env_change() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    let name: Vec<u16> = "Environment\0".encode_utf16().collect();
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(name.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            3000,
            None,
        );
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
