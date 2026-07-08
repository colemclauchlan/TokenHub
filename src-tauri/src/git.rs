//! Git Dash: local-repo status table (branch, ahead/behind, dirty count, last
//! commit, clean/dirty/unpushed). Repos are discovered from the project folders
//! seen in Claude Code / Codex sessions. Each has a browsable remote URL so the
//! UI can open it in the browser.

use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub name: String,
    pub path: String,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub dirty: u32,
    pub last_commit: String,
    pub status: String, // clean | dirty | unpushed
    pub url: String,    // browsable https URL, or ""
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GitData {
    pub available: bool,
    pub message: String,
    pub repos: Vec<RepoInfo>,
    pub total: usize,
    pub dirty: usize,
    pub unpushed: usize,
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Unique project folders seen across Claude Code + Codex sessions.
fn candidate_dirs() -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut dirs = Vec::new();
    let sessions = crate::sessions::claude_sessions()
        .into_iter()
        .chain(crate::sessions::codex_sessions());
    for s in sessions {
        if s.cwd.is_empty() {
            continue;
        }
        if seen.insert(s.cwd.clone()) {
            dirs.push(PathBuf::from(s.cwd));
        }
    }
    dirs
}

/// Convert an origin URL (https or ssh) into a browsable https URL.
fn to_browsable(url: &str) -> String {
    let u = url.trim();
    if u.starts_with("http://") || u.starts_with("https://") {
        return u.trim_end_matches(".git").to_string();
    }
    if let Some(rest) = u.strip_prefix("git@") {
        if let Some((host, path)) = rest.split_once(':') {
            return format!("https://{}/{}", host, path.trim_end_matches(".git"));
        }
    }
    if let Some(rest) = u.strip_prefix("ssh://") {
        let rest = rest.trim_start_matches("git@");
        return format!("https://{}", rest.trim_end_matches(".git"));
    }
    String::new()
}

fn parse_repo(dir: &Path) -> Option<RepoInfo> {
    if git(dir, &["rev-parse", "--is-inside-work-tree"]).as_deref() != Some("true") {
        return None;
    }
    let branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "?".into());
    let dirty = git(dir, &["status", "--porcelain"])
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count() as u32)
        .unwrap_or(0);

    let (mut ahead, mut behind) = (0u32, 0u32);
    if let Some(counts) = git(dir, &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"]) {
        let mut it = counts.split_whitespace();
        behind = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        ahead = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    }

    let last_commit = git(dir, &["log", "-1", "--format=%cr"]).unwrap_or_default();
    let url = git(dir, &["remote", "get-url", "origin"])
        .map(|u| to_browsable(&u))
        .unwrap_or_default();
    let status = if ahead > 0 {
        "unpushed"
    } else if dirty > 0 {
        "dirty"
    } else {
        "clean"
    };
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".into());

    Some(RepoInfo {
        name,
        path: dir.to_string_lossy().to_string(),
        branch,
        ahead,
        behind,
        dirty,
        last_commit,
        status: status.into(),
        url,
    })
}

pub fn dashboard() -> GitData {
    let has_git = Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_git {
        return GitData {
            available: false,
            message: "git not found on PATH".into(),
            ..Default::default()
        };
    }

    let mut repos = Vec::new();
    let mut seen = HashSet::new();
    for d in candidate_dirs() {
        // resolve to the repo top-level so subdirs of one repo dedupe
        let dir = git(&d, &["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .unwrap_or(d);
        if !seen.insert(dir.to_string_lossy().to_string()) {
            continue;
        }
        if let Some(r) = parse_repo(&dir) {
            repos.push(r);
        }
        if repos.len() >= 40 {
            break;
        }
    }
    repos.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let dirty = repos.iter().filter(|r| r.dirty > 0).count();
    let unpushed = repos.iter().filter(|r| r.ahead > 0).count();
    let total = repos.len();
    GitData {
        available: true,
        message: if total == 0 { "No git repos found in your recent project folders.".into() } else { String::new() },
        repos,
        total,
        dirty,
        unpushed,
    }
}
