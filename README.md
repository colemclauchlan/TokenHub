# AI Usage Bar

A Windows taskbar tracker for **Claude Code** and **OpenAI Codex** — a Windows port of
[TermTracker](https://github.com/isaacaudet/TermTracker) (macOS). Lives in the tray with a
click-to-expand panel, a dynamic tray icon, and a docked mini-bar. Tracks the **5-hour** and
**7-day** rolling limit windows, token usage, API-cost estimates, models, and processes.

Built with **Tauri v2** (Rust core + web UI). See `PLAN.md` for the full design + critique.

## How the 5h / 7d numbers match Claude Code's counter

Two different sources, on purpose:

| Number | Source |
|---|---|
| 5h / 7d **limit %** (headline) | Provider usage API (`api.anthropic.com` / `chatgpt.com`) read with your **local OAuth token** — the same source the CLI's own counter uses |
| Tokens, cost, sessions, models, sparkline | Local logs: `%USERPROFILE%\.claude\projects\**\*.jsonl`, `.claude\stats-cache.json`, `%USERPROFILE%\.codex\sessions\**\rollout-*.jsonl` |

If the API is off/unavailable, the app falls back to a local-log **estimate**, clearly labelled
`estimated from local logs` (vs `live · matches Claude Code counter`).

Reading the OAuth token is **opt-in**, local-only, and never uploaded anywhere. Toggle it in
Settings (`useProviderApi`). Endpoints are overridable via env vars (`AIUSAGEBAR_CLAUDE_USAGE_URL`,
`AIUSAGEBAR_CODEX_USAGE_URL`, …) for when the provider changes them.

## Build

Requires Rust (stable), Node 18+, and the Tauri prerequisites (WebView2 is preinstalled on
Win10/11; you also need the MSVC Build Tools).

```bash
npm install
npm run dev      # run locally
npm run build    # produce .exe (NSIS) + .msi in src-tauri/target/release/bundle
```

**No local toolchain?** Push to GitHub — the `build` workflow compiles the installers on a
`windows-latest` runner and uploads them as an artifact (`.github/workflows/build.yml`).

## Test

```bash
npm run test:core    # cargo unit tests for the pure core (window math, parsing, pricing, tray raster)
npm run check:algo   # fast Node cross-check of the window/pricing algorithms (33 assertions)
```

## Secrets

The app needs **no cloud secrets** — it reuses the OAuth token Claude Code / Codex already stored
locally, and `gh` supplies its own token for the Git tab. The only secrets you'd ever add are for
**code-signing in CI** (optional, removes the SmartScreen warning):

- GitHub → repo → Settings → Secrets and variables → Actions → New repository secret
  - `WINDOWS_CERT_BASE64` (base64 of your .pfx), `WINDOWS_CERT_PASSWORD`
- These are referenced only by the CI signing step (added when you have a cert). Never commit them.

## Settings

Hotkey (default `Ctrl+Shift+U`), autostart, mini-bar position (default bottom-left, over the
weather slot — turn off the Windows Widgets button to free the space), refresh interval,
Claude/Codex toggles, and estimate-mode budgets.

## Windows QA checklist

- [ ] Tray icon appears and its two bars track 5h/7d as usage changes
- [ ] Left-click tray (and mini-bar) toggles the panel; it anchors above the taskbar
- [ ] Mini-bar docks at the chosen corner, survives an `explorer.exe` restart
- [ ] Usage tab matches the mockup; Claude⇄Codex swap works
- [ ] With a Claude Code login present, 5h/7d read `live · matches Claude Code counter` and equal `/usage`
- [ ] Processes tab lists AI processes grouped by terminal; kill works (with confirm)
- [ ] Git tab shows open PRs when `gh` is authenticated; graceful message when not
- [ ] Hotkey, autostart, and DPI/multi-monitor positioning behave

## Project layout

```
src/                     web UI (index.html, styles.css, app.js, minibar.html)
src-tauri/
  crates/usage-core/     pure, tested core (parsing, 5h/7d math, pricing, provider API)
  src/                   Tauri app (tray, mini-bar, panel, snapshot, processes, git)
tools/algo-check/        offline algorithm cross-check
.github/workflows/       Windows installer build
```
