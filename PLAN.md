# AI Usage Taskbar — Build Plan (Windows port of TermTracker)

Status: **DRAFT for approval** · Last updated: 2026-07-05

Mirror of [isaacaudet/TermTracker](https://github.com/isaacaudet/TermTracker) (native macOS
menu-bar app, SwiftUI) rebuilt for **Windows**, living in the taskbar/tray with an
expandable panel. Tracks token usage + rolling limit windows for **Claude Code** and
**OpenAI Codex CLI**, with a swappable Claude ⇄ Codex view.

---

## 1. Answers to your two questions (these drive the design)

### Q1 — "Will the 5h / 7d calculation match the circle counter inside Claude Code?"
**Yes — if we read the same source Claude Code reads, not just local logs.** TermTracker's
own README is explicit about this:

> Claude/Codex **quota windows** (optional) → **Provider usage APIs via local OAuth credentials**

So there are two different numbers, from two different sources:

| Number | Source | Matches in-app counter? |
|---|---|---|
| Token totals, cost estimate, sessions, models, sparkline | Local JSONL logs (`~/.claude/projects/**/*.jsonl`, `stats-cache.json`) | N/A (these are totals, not the % limit) |
| **5h / 7d limit %** (the thing you care about) | **Provider usage API** (`platform.claude.com` / `api.anthropic.com`) called with your **local Claude Code OAuth token** | **Yes** — same source the CLI's counter uses |

Pure local-log summing (the "ccusage" approach) only gives an **estimate** of the window and
will *not* reliably match, because Anthropic weights consumption server-side (model, context,
cache, output all count differently) and the weekly window resets on a fixed, account-specific
schedule. To match the counter we must hit the provider usage endpoint with your OAuth creds —
exactly what the reference app does.

**Design decision:** primary = provider usage API (matches the counter); fallback = local-log
estimate when offline / not authed (clearly labelled "estimate"). This is a first-run,
local-only, opt-in read of the token that Claude Code already stored on your machine — nothing
is uploaded anywhere.

### Q2 — "Can it be a widget like the Win11 weather one, bottom-left, replacing that space?"
**Partly — and we'll get the look you want, just not by literally overwriting Microsoft's
weather text.** Facts:

- The Win11 taskbar weather is the **Widgets button**. Third-party widgets are allowed, but they
  must be **Adaptive Cards** (JSON: text/images/actions) rendered **inside Microsoft's Widgets
  board** — you do **not** get arbitrary HTML/canvas, and you **cannot** replace just the weather
  label on the taskbar in a supported way.
- So the rich TermTracker-style panel **cannot** live inside the official Widgets board at full
  fidelity.

**What we'll do instead (gets the same result):**
1. **Docked mini-bar** — a slim always-on-top strip we position **bottom-left where the weather
   sits**. You turn off Microsoft's Widgets button; ours occupies that visual space. This is
   exactly how the reference screenshot achieves it.
2. **Dynamic tray icon** — a tiny icon that *draws* the 5h/7d mini-pills (the green icon you
   sent), redrawn as usage changes.
3. **Flyout panel** — click either surface → the big panel opens like the Start menu.
4. **Optional bonus (later):** a real Win11 Adaptive-Card widget for the official board, accepting
   its styling limits, for people who want it there.

---

## 2. Tech stack — decision: **Tauri v2 (Rust core + web UI)**

You said "Rust or whatever you deem best." Tauri v2 is the best fit:

- **Rust backend** (your preference) for log parsing, the usage-API client, Win32 tray/window work.
- **Web frontend** (HTML/CSS/JS + canvas) reproduces the glassy gradient dashboard **far** more
  faithfully and quickly than pure-native Rust UI (egui) would.
- **Lightweight** (~5–15 MB installer, low RAM) — right for an always-on widget. Uses the
  WebView2 runtime already present on Win10/11.
- Direct Win32 access (via `windows`/`tauri` APIs) for the dynamic tray icon, always-on-top
  docked strip, DPI/work-area positioning, and autostart.

**Tradeoff (stated honestly):** the reference brags "no web views." We use WebView2. That
philosophical purity isn't your requirement — visual fidelity + build speed are — so this is the
right call. If you'd rather go pure-native, the alternative is **C#/.NET WinUI 3** (native, no
webview, excellent tray support) but it's slower to match the exact glass styling and isn't Rust.

---

## 3. Feature / tab parity map

| TermTracker (macOS) | Windows port | Primary data source (Windows paths) |
|---|---|---|
| Menu-bar glyph w/ mini bars | **Tray icon (dynamic) + docked mini-bar** | computed 5h/7d values |
| **Usage** tab (hero card, Today, Last-Hour sparkline, 14-day trend, Models) | Same, swappable **Claude ⇄ Codex** | `%USERPROFILE%\.claude\projects\**\*.jsonl`, `.claude\stats-cache.json`; Codex `%USERPROFILE%\.codex\sessions\**\rollout-*.jsonl` |
| **Processes** tab (AI procs grouped by terminal, RAM, kill) | Same, via Windows APIs | `tasklist` / WMI / `sysinfo` crate; detect Windows Terminal, PowerShell, VS Code, Alacritty, etc. |
| **Git** tab (open PRs, CI health) | Same | `gh api graphql` if `gh` CLI present (optional) |
| Settings / hotkey | Same (global hotkey, autostart, position, refresh, Claude/Codex toggles) | local config JSON |
| 5h / 7d quota windows | Same | **Provider usage API + local OAuth** (primary), local-log estimate (fallback) |

---

## 4. Architecture

```
src-tauri/
  crates/
    usage-core/        # PURE Rust, zero Tauri deps → unit-tested in this sandbox
      logs_claude.rs   #   parse ~/.claude JSONL + stats-cache
      logs_codex.rs    #   parse ~/.codex sessions rollout JSONL
      windows_calc.rs  #   5h rolling-from-first-use + 7d window math
      pricing.rs       #   model → $/Mtok table, cost estimate
      usage_api.rs     #   provider usage API client + OAuth token loading (traited/mockable)
      model.rs         #   shared structs serialized to the frontend
    app/               # Tauri app (depends on usage-core) — built on Windows
      tray.rs          #   dynamic tray icon rendering (GDI/bitmap)
      minibar.rs       #   always-on-top docked strip window
      panel.rs         #   flyout panel window + positioning near tray
      hotkey.rs, autostart.rs, ipc.rs
src/                   # frontend (index.html, styles.css, app.js, charts)
.github/workflows/build.yml   # builds signed-ish .exe/.msi on a Windows runner
```

**Data flow:** a Rust poller refreshes every N seconds → builds a `Snapshot` struct → emits to the
webview (Usage/Processes/Git render from it) → also rasterizes the tray icon + mini-bar. All local;
no telemetry.

## 5. 5h / 7d window computation
- **Primary:** GET provider usage endpoint with the local OAuth bearer → read the returned window
  utilization + reset timestamps → this is what the CLI shows. Cache short-term; refresh on a timer.
- **Fallback estimate:** 5h window = rolling window anchored at the first message ≤5h ago (matches
  Claude's "starts at first message, renews 5h later" behavior); 7d = trailing 7 days (with a
  user-set fixed reset time in Settings, since the weekly reset is account-specific). Clearly
  labelled "est." when the API isn't used.

## 6. Pricing model
Static model→price table (input / output / cache-read / cache-write per Mtok) for Opus/Sonnet/Haiku
and GPT/Codex models, used only for the "API list est." hero number. Labelled "not your subscription
spend," matching the reference. Table lives in one file for easy updates.

## 7. Build, packaging, secrets
- **Local build:** `npm install` + `cargo tauri build` on Windows (needs Rust + WebView2 + MSVC
  Build Tools). Produces `.exe` (NSIS) / `.msi`.
- **No-toolchain build:** a **GitHub Actions** workflow on a `windows-latest` runner produces the
  installer as an artifact, so you can get a real build without installing anything.
- **Autostart** via registry Run key (toggle in Settings).
- **Secrets:** the app needs **no cloud secrets**. It reuses the OAuth token Claude Code/Codex
  already stored locally. The **only** secret is optional: a GitHub token for the Git tab — and `gh`
  CLI supplies that itself. If we ever add code-signing in CI, the cert + password go in **GitHub
  → Settings → Secrets and variables → Actions** (`WINDOWS_CERT_BASE64`, `WINDOWS_CERT_PASSWORD`),
  never in the repo. (Guide included when we get there.)

## 8. Testing / bugtest strategy
- **In this Linux sandbox (now):** `usage-core` is pure Rust → unit tests against synthetic
  `.claude`/`.codex` JSONL fixtures verify token sums, 5h/7d math, and pricing. Frontend rendered in
  headless Chromium → screenshot diffed against your mockup to tune styling.
- **On your Windows machine (final):** tray icon, mini-bar docking, flyout positioning, hotkey,
  autostart, and real OAuth/usage-API calls. I'll provide a QA checklist + smoke steps.
- **Honest limitation:** I can't compile or run a native Windows GUI in this environment — the
  webview/tray/positioning must be verified on Windows. I'll get everything else green here first.

## 9. Phased delivery (all features included, sequenced)
1. **P0 Scaffold** — Tauri workspace, core crate, frontend shell, CI.
2. **P1 Core (tested)** — Claude+Codex log parsing, 5h/7d math, pricing → `cargo test` green.
3. **P2 Usage tab** — hero card, Today, Last-Hour sparkline, 14-day trend, Models; Claude⇄Codex swap.
4. **P3 Taskbar surfaces** — dynamic tray icon, docked mini-bar, flyout panel + positioning.
5. **P4 Provider usage API** — OAuth token load + endpoint → 5h/7d matches the counter.
6. **P5 Processes tab** — grouped AI processes, RAM, kill.
7. **P6 Git tab + Settings** — PRs via `gh`; hotkey/autostart/position/refresh/toggles.
8. **P7 Package** — installer + GitHub Actions Windows build + QA pass.

---

## 10. SELF-CRITIQUE — holes in this plan & mitigations
1. **Exact counter match is the riskiest claim.** The provider usage endpoint shape/auth isn't
   publicly documented and can change; TermTracker guards it behind env overrides for a reason.
   → *Mitigation:* isolate it behind a trait with the endpoint/host configurable (same env-override
   pattern), ship the local-log estimate as guaranteed-working fallback, and label which is live.
2. **Reading the OAuth token = handling a credential.** → *Mitigation:* opt-in first-run consent,
   read-only, stays on device, never logged/transmitted except to the official provider host over
   HTTPS. Clearly disclosed; can be disabled (estimate mode).
3. **I cannot compile/run the Windows GUI here.** → *Mitigation:* prove all logic in sandbox tests;
   deliver a GitHub Actions Windows build so you get a real installer; provide a Windows QA checklist.
4. **JSONL/stats-cache schema drift** (Anthropic/OpenAI can change formats). → *Mitigation:* defensive
   parsing (tolerate missing/renamed fields), fixtures, graceful "no data" states.
5. **Weekly (7d) reset is account-specific and reportedly irregular.** → *Mitigation:* prefer the API's
   reset timestamp; expose a manual reset day/time in Settings for estimate mode.
6. **Always-on-top strip vs multi-monitor / DPI / taskbar position / `explorer.exe` restarts.**
   → *Mitigation:* robust work-area + DPI detection, reposition on display-change events, watch for
   shell restarts and re-anchor.
7. **Dynamic tray icon GDI handle leaks / blurry on HiDPI.** → *Mitigation:* render per-DPI bitmaps,
   destroy old HICONs, throttle redraws to actual value changes.
8. **Codex limits differ from Claude** (own 5h window, different plan tiers). → *Mitigation:* separate
   provider adapters; don't assume Claude's semantics for Codex.
9. **Unsigned installer → SmartScreen warning.** → *Mitigation:* document it; leave a clean hook for
   code-signing secrets in CI when you have a cert.
10. **Process kill / permissions.** → *Mitigation:* confirm-before-kill; handle access-denied on
    protected processes gracefully.
11. **Pricing table goes stale.** → *Mitigation:* single-file table, dated, easy to update; it only
    affects the "est." number, never the token truth.

## 11. Open items for you (non-blocking — defaults chosen)
- App display name (default: **"AI Usage Bar"**; internal id `ai-usage-bar`). Rename in one place.
- Track **Cursor / Gemini** too (reference does)? Default: **Claude + Codex only** per your ask;
  easy to add later.
- Exact-match OAuth path **on by default** (recommended) with estimate fallback — say the word if
  you'd rather estimate-only.
