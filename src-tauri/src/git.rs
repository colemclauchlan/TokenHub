//! Git tab: open PRs + CI health for the signed-in user via the `gh` CLI.
//! Gracefully returns `available:false` when `gh` isn't installed or authenticated.

use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct PrInfo {
    pub repo: String,
    pub title: String,
    pub number: i64,
    pub url: String,
    pub ci: String,
    pub review: String,
    pub mergeable: String,
}

#[derive(Serialize, Clone, Default)]
pub struct GitData {
    pub available: bool,
    pub message: String,
    pub prs: Vec<PrInfo>,
}

const QUERY: &str = r#"query {
  viewer {
    pullRequests(first: 20, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        title number url mergeable reviewDecision
        repository { nameWithOwner }
        commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
      }
    }
  }
}"#;

pub fn fetch() -> GitData {
    let out = std::process::Command::new("gh")
        .args(["api", "graphql", "-f", &format!("query={QUERY}")])
        .output();
    let Ok(out) = out else {
        return GitData { available: false, message: "gh CLI not found".into(), prs: vec![] };
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        let msg = if err.to_lowercase().contains("auth") {
            "gh not authenticated — run `gh auth login`".into()
        } else {
            "gh query failed".into()
        };
        return GitData { available: false, message: msg, prs: vec![] };
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return GitData { available: false, message: "could not parse gh output".into(), prs: vec![] };
    };
    let nodes = v
        .pointer("/data/viewer/pullRequests/nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    let prs = nodes
        .iter()
        .map(|n| PrInfo {
            repo: n.pointer("/repository/nameWithOwner").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            title: n.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            number: n.get("number").and_then(|x| x.as_i64()).unwrap_or(0),
            url: n.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            ci: n
                .pointer("/commits/nodes/0/commit/statusCheckRollup/state")
                .and_then(|x| x.as_str())
                .unwrap_or("NONE")
                .to_string(),
            review: n.get("reviewDecision").and_then(|x| x.as_str()).unwrap_or("NONE").to_string(),
            mergeable: n.get("mergeable").and_then(|x| x.as_str()).unwrap_or("UNKNOWN").to_string(),
        })
        .collect();

    GitData { available: true, message: String::new(), prs }
}
