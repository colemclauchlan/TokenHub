//! Minimal GitHub public-repo listing for the Git tab. Uses the unauthenticated
//! REST API (public repos only), accepting either a bare username or a full
//! profile URL (e.g. `https://github.com/octocat`).

use serde::Serialize;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    pub name: String,
    pub full_name: String,
    pub url: String,
    pub description: String,
    pub language: String,
    pub stars: u64,
    pub updated: String,
    pub private: bool,
}

/// Normalize a user's input to a bare GitHub login (handles profile URLs and `@`).
pub fn normalize_user(input: &str) -> String {
    let s = input.trim().trim_end_matches('/');
    // strip a leading profile URL if present
    let s = s
        .rsplit(['/', '\\'])
        .find(|p| !p.is_empty())
        .unwrap_or(s);
    s.trim_start_matches('@').to_string()
}

/// Fetch a user's public repositories, most-recently-updated first.
pub fn repos(user: &str) -> Result<Vec<Repo>, String> {
    let login = normalize_user(user);
    if login.is_empty() {
        return Ok(vec![]);
    }
    let url = format!(
        "https://api.github.com/users/{}/repos?per_page=100&sort=updated",
        login
    );
    let resp = ureq::get(&url)
        .set("User-Agent", "TokenHub")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub request failed: {e}"))?;
    let text = resp.into_string().map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let arr = match v {
        serde_json::Value::Array(a) => a,
        // API returns an object with a "message" on error (e.g. Not Found / rate limit)
        serde_json::Value::Object(o) => {
            let msg = o
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unexpected response");
            return Err(msg.to_string());
        }
        _ => return Ok(vec![]),
    };
    let mut out = Vec::new();
    for r in arr {
        let g = |k: &str| r.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        out.push(Repo {
            name: g("name"),
            full_name: g("full_name"),
            url: g("html_url"),
            description: g("description"),
            language: g("language"),
            stars: r.get("stargazers_count").and_then(|x| x.as_u64()).unwrap_or(0),
            updated: g("updated_at"),
            private: r.get("private").and_then(|x| x.as_bool()).unwrap_or(false),
        });
    }
    Ok(out)
}
