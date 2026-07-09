# Backlog — AI Usage Bar

Issue #0 is the immediate operational step (get the Windows build green). Issues #1–#3 are
the **three delivery ideas** derived from the spec/reference, each with acceptance criteria and
test cases. Run `scripts/create-github-issues.sh` after the repo exists to file all of these.

---

## #0 — Green the Windows build (operational)

**Summary.** The pure core is verified offline (33/33 + `cargo test -p usage-core`). The Tauri app
crate has never been compiled in this environment; first Windows compile may need small Tauri v2
API adjustments (tray event enum, global-shortcut handler signature, `Image::new_owned`).

**Acceptance criteria**
- [ ] `.github/workflows/build.yml` `test-core` job passes (core unit tests + algo cross-check).
- [ ] `build-windows` job compiles and uploads `.exe` (NSIS) and `.msi` artifacts.
- [ ] App launches: tray icon visible, left-click toggles the panel, mini-bar docks.

**Test cases**
1. CI: push to `main` → both jobs green; artifact downloadable.
2. Manual: run the `.exe` on Win11 → tray icon renders the two bars; click opens panel above taskbar.
3. Manual: kill `explorer.exe`, let it restart → mini-bar re-anchors.

**Labels:** `build`, `P0`

---

## #1 — Exact usage-API parity + plan / reset auto-detect

**Summary.** Lock the provider usage endpoint + response mapping so the 5h/7d percentages **exactly**
equal Claude Code's `/usage`, and auto-detect the plan label ($/mo) and the weekly reset time instead
of using placeholders/config.

**Motivation.** The headline promise is "matches the counter." Today the endpoint/shape is behind a
tolerant parser + env overrides (`usage_api.rs`); it must be verified against the live API and pinned.

**Acceptance criteria**
- [ ] `usage_api::parse_usage_response` maps the real Claude + Codex payloads (fixtures captured from
      a live session) to `QuotaWindows` with correct utilization + reset.
- [ ] 5h/7d shown in the app equal `/usage` (±1%) for both providers when authenticated.
- [ ] Plan label and weekly reset are read from the API (fallback to config only when absent).
- [ ] Clear "live · matches counter" vs "estimated" badge reflects the true source.

**Test cases**
1. Unit: add `tests/fixtures/claude_usage.json`, `codex_usage.json`; assert parsed util/reset.
2. Unit: percent normalization (0–1 vs 0–100), missing-field tolerance, reset RFC3339 + epoch.
3. Manual: compare app vs `claude /usage` and Codex `/status` side by side.

**Labels:** `core`, `provider-api`, `P1`

---

## #2 — Session & tool intelligence (fill the `tools` stat + session list)

**Summary.** Parse `tool_use` blocks to populate the Today `tools` count (currently `0`, a known
stub), extract session name / git branch / working dir from Claude Code JSONL, mark active sessions
with a green dot, and add a day timeline — full session intelligence.

**Acceptance criteria**
- [ ] `today.tools` reflects real tool-call counts for the day.
- [ ] A sessions list shows name, branch, cwd, last-active; active (<10 min) sessions get a green dot.
- [ ] Parsing stays defensive (missing fields don't panic) and is covered by unit tests.

**Test cases**
1. Unit: fixture JSONL with N `tool_use` blocks → `tools == N`.
2. Unit: session extraction from `cwd` / `gitBranch` / summary fields; active-dot threshold boundary.
3. Manual: run a Claude Code session; confirm it appears active with correct branch.

**Labels:** `core`, `ui`, `P1`

---

## #3 — Native Win11 Widgets-board card + CI code-signing

**Summary.** Ship an optional Adaptive-Card **Widget Provider** so the 5h/7d bars can also live in the
official Windows 11 Widgets board (accepting Adaptive-Card styling limits), and add code-signing to
CI to remove the SmartScreen warning.

**Acceptance criteria**
- [ ] A registered widget provider renders a 5h/7d Adaptive Card in the Widgets board (Win+W).
- [ ] Packaged installer registers/unregisters the provider cleanly.
- [ ] CI signs the `.exe`/`.msi` when `WINDOWS_CERT_BASE64` + `WINDOWS_CERT_PASSWORD` secrets exist;
      unsigned build still works when they don't.

**Test cases**
1. Manual: enable the widget in the board → card shows current 5h/7d, refreshes.
2. Manual: signed artifact installs without SmartScreen prompt; signature verifies (`signtool verify`).
3. CI: build succeeds both with and without signing secrets present.

**Labels:** `packaging`, `windows`, `P2`
