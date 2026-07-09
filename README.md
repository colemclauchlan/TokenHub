# TokenHub

A Windows **taskbar tracker for Claude Code, Claude Cowork, and OpenAI Codex/ChatGPT**. It lives in
the tray with a dynamic icon and a docked mini-bar, and clicks open a full panel. It tracks your
**5-hour** and **7-day** rolling limit windows, token usage, cost estimates, per-model share, live
agent status, and your local git repos.

Built with **Tauri v2** (Rust core + web UI). See `PLAN.md` for the full design + critique.

## Features

- **Usage tab** with a provider switch — **Overview** (Claude + Codex combined), **Claude**, **Codex**.
  - 5h / 7d limit bars (Claude/Codex) or green/red **quota status cards** + a monthly **subscription total in CAD** (Overview).
  - Hero stats, Today's Usage + last-hour sparkline, **Models** card (share of period), 14-day trend.
- **Sessions** — chats with an agent active in the last 30 min, a working/idle light, and an expandable **agent list** (model, name, status, goal). Rename chats inline.
- **History** — every chat grouped by project, per-chat and per-project totals, rename, and delete (per-chat and whole-project) with confirmation.
- **Open** — open AI-client windows grouped by Claude / Codex-GPT; click to bring one to the front.
- **Git** — local repos (branch, sync, dirty, last commit) plus one card per connected GitHub account, with the account's avatar; click a repo to open it.
- **Dynamic tray icon** — fill / ring / bar styles, multi-colour or mono, plus a taskbar **mini-bar** with a green/amber/red **agent-status light** for a chosen chat.
- **Settings** — profiles (save/switch display presets), notifications (75/90/95% alerts), used-vs-remaining %, connect GitHub accounts, encrypted API-key storage, and a joke "water guilt" meter.

## How the 5h / 7d numbers match Claude Code's counter

Two different sources, on purpose:

| Number | Source |
|---|---|
| 5h / 7d **limit %** (headline) | Provider usage API read with your **local OAuth token** — the same source the CLI's counter uses |
| Tokens, cost, sessions, models, sparkline | Local logs: `%USERPROFILE%\.claude\projects\**\*.jsonl`, `%APPDATA%\Claude\local-agent-mode-sessions\**` (Cowork), `%USERPROFILE%\.codex\sessions\**\rollout-*.jsonl` |

If the API is off/unavailable, the app falls back to a local-log **estimate**, clearly labelled
`estimated from local logs` (vs `live · matches Claude Code counter`).

Reading the OAuth token is **opt-in**, local-only, and never uploaded. API keys you enter are stored
in the **Windows Credential Manager** (encrypted), never in config or plaintext.

## Build

Requires Rust (stable), Node 18+, and the Tauri prerequisites (WebView2 ships on Win10/11; you also
need the MSVC Build Tools).

```bash
npm install
npm run dev      # run locally
npm run build    # produce .exe (NSIS) + .msi in src-tauri/target/release/bundle
```

**No local toolchain?** Push to GitHub — the `build` workflow compiles the installers on a
`windows-latest` runner and uploads them (`.github/workflows/build.yml`).

## Test

```bash
npm run test:core    # cargo unit tests for the pure core (window math, parsing, pricing, tray raster)
npm run check:algo   # fast Node cross-check of the window/pricing algorithms (33 assertions)
```

## Privacy

TokenHub needs **no cloud secrets**. It reads local logs and the OAuth token Claude Code / Codex
already store, and any API key you add lives in the Windows Credential Manager. Nothing is uploaded.

## Project layout

```
src/                     web UI (index.html, styles.css, app.js, minibar.html)
src-tauri/
  crates/usage-core/     pure, tested core (parsing, 5h/7d math, pricing, provider API)
  src/                   Tauri app (tray, mini-bar, panel, snapshot, sessions, git, secrets)
tools/algo-check/        offline algorithm cross-check
.github/workflows/       Windows installer build
```

## License

MIT — see `LICENSE`.
