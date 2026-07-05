#!/usr/bin/env bash
# Files the AI Usage Bar backlog as GitHub issues via the gh CLI.
# Prereqs: `gh auth login` done, and run from inside the repo (after `git push`).
# Usage: bash scripts/create-github-issues.sh
set -euo pipefail

command -v gh >/dev/null || { echo "gh CLI not found — install https://cli.github.com"; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "Run 'gh auth login' first."; exit 1; }

echo "Ensuring labels exist…"
ensure_label() { gh label create "$1" --color "$2" --description "$3" 2>/dev/null || true; }
ensure_label build     "1d76db" "Build / CI"
ensure_label core      "0e8a16" "usage-core logic"
ensure_label ui        "5319e7" "Frontend / panel"
ensure_label provider-api "b60205" "Provider usage API"
ensure_label packaging "fbca04" "Installer / packaging"
ensure_label windows   "c5def5" "Windows-specific"
ensure_label P0        "d93f0b" "Priority 0"
ensure_label P1        "e99695" "Priority 1"
ensure_label P2        "fef2c0" "Priority 2"

new_issue() { echo "Creating: $1"; gh issue create --title "$1" --label "$2" --body "$3"; }

new_issue "Green the Windows build" "build,P0" \
'The pure core is verified offline (cargo test -p usage-core + node algo-check). The Tauri app crate needs its first Windows compile; small Tauri v2 API nits may surface (tray event enum, global-shortcut handler signature, Image::new_owned).

### Acceptance criteria
- [ ] `test-core` CI job passes (core unit tests + algo cross-check)
- [ ] `build-windows` job compiles and uploads .exe (NSIS) + .msi
- [ ] App launches: tray icon visible, left-click toggles panel, mini-bar docks

### Test cases
1. CI: push to main → both jobs green; artifact downloadable
2. Manual: run .exe on Win11 → tray shows two bars; click opens panel above taskbar
3. Manual: restart explorer.exe → mini-bar re-anchors

See docs/ISSUES.md #0.'

new_issue "Exact usage-API parity + plan/reset auto-detect" "core,provider-api,P1" \
'Lock the provider usage endpoint + response mapping so 5h/7d exactly equal Claude Code /usage, and auto-detect plan label and weekly reset instead of placeholders.

### Acceptance criteria
- [ ] parse_usage_response maps real Claude + Codex payloads (captured fixtures) → correct utilization + reset
- [ ] App 5h/7d equal /usage (±1%) for both providers when authenticated
- [ ] Plan label + weekly reset read from API (config fallback only when absent)
- [ ] live-vs-estimated badge reflects true source

### Test cases
1. Unit: fixtures claude_usage.json / codex_usage.json → asserted util/reset
2. Unit: percent normalization (0-1 vs 0-100), missing-field tolerance, RFC3339 + epoch reset
3. Manual: app vs `claude /usage` and Codex `/status` side by side

See docs/ISSUES.md #1.'

new_issue "Session & tool intelligence (fill tools stat + session list)" "core,ui,P1" \
'Parse tool_use blocks to populate the Today tools count (currently a 0 stub), extract session name/branch/cwd, mark active sessions with a green dot, add a day timeline.

### Acceptance criteria
- [ ] today.tools reflects real tool-call counts
- [ ] sessions list shows name, branch, cwd, last-active; active (<10 min) get a green dot
- [ ] parsing defensive + unit-tested

### Test cases
1. Unit: fixture JSONL with N tool_use blocks → tools == N
2. Unit: session extraction from cwd/gitBranch/summary; active-dot boundary
3. Manual: run a Claude Code session; confirm active + correct branch

See docs/ISSUES.md #2.'

new_issue "Native Win11 Widgets-board card + CI code-signing" "packaging,windows,P2" \
'Optional Adaptive-Card Widget Provider so the 5h/7d bars can live in the official Win11 Widgets board, plus CI code-signing to remove SmartScreen.

### Acceptance criteria
- [ ] registered widget provider renders a 5h/7d Adaptive Card in the Widgets board (Win+W)
- [ ] installer registers/unregisters the provider cleanly
- [ ] CI signs .exe/.msi when WINDOWS_CERT_BASE64 + WINDOWS_CERT_PASSWORD secrets exist; unsigned still works without them

### Test cases
1. Manual: enable widget → card shows current 5h/7d and refreshes
2. Manual: signed artifact installs without SmartScreen; signtool verify passes
3. CI: build succeeds with and without signing secrets

See docs/ISSUES.md #3.'

echo "Done. View: gh issue list"
