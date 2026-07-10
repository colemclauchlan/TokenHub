// AI Usage Bar — panel renderer. Renders a Snapshot into the UI and draws the
// sparkline + 14-day combo chart on canvas. At runtime the snapshot comes from the
// Rust backend (Tauri invoke); in the browser/screenshot harness it uses window.MOCK_SNAPSHOT.

const COLORS = {
  cacheRead: "#4aa8c9", cacheWrite: "#d0774a", input: "#6f93b4", output: "#d9a441",
  orange: "#d0774a", blue: "#4aa8c9",
};

// Per-provider theme: Claude = orange highlights, Codex = blue highlights.
const THEME = {
  claude: { accent: "#d0774a", bars: "#d0774a", line: "#4aa8c9", spark: "#d9a441" },
  codex:  { accent: "#4aa8c9", bars: "#4aa8c9", line: "#d9a441", spark: "#4aa8c9" },
  overview: { accent: "#9f7bd0", bars: "#9f7bd0", line: "#4aa8c9", spark: "#9f7bd0" },
};
function hexToRgba(hex, a) {
  const h = hex.replace("#", "");
  const full = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const n = parseInt(full, 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${a})`;
}

let currentProvider = "overview"; // Usage tab: overview / claude / codex
let histProvider = "claude";       // History tab: claude / codex (no overview)
let activeTab = "usage";
let showClock = false; // alternates limit rows between countdown and reset clock

// Provider glyphs (Codex uses an OpenAI-style blossom instead of a plain circle).
const LOGOS = {
  claude: "✳",
  overview: "Σ",
  codex: '<svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor" aria-hidden="true"><g>' +
    '<ellipse cx="12" cy="7" rx="2.3" ry="4.3"/>' +
    '<ellipse cx="12" cy="7" rx="2.3" ry="4.3" transform="rotate(60 12 12)"/>' +
    '<ellipse cx="12" cy="7" rx="2.3" ry="4.3" transform="rotate(120 12 12)"/>' +
    '<ellipse cx="12" cy="7" rx="2.3" ry="4.3" transform="rotate(180 12 12)"/>' +
    '<ellipse cx="12" cy="7" rx="2.3" ry="4.3" transform="rotate(240 12 12)"/>' +
    '<ellipse cx="12" cy="7" rx="2.3" ry="4.3" transform="rotate(300 12 12)"/></g></svg>',
};

/* ---------- formatting ---------- */
function fmtCompact(n) {
  const a = Math.abs(n);
  if (a >= 1e9) return (n / 1e9).toFixed(1) + "B";
  if (a >= 1e6) return (n / 1e6).toFixed(1) + "M";
  if (a >= 1e3) return (n / 1e3).toFixed(1) + "K";
  return String(n);
}
function fmtCost(n) {
  return n >= 1000 ? "$" + (n / 1000).toFixed(1) + "K" : "$" + n.toFixed(2);
}
// Show a utilization percentage as "used" or "remaining" per the pctRemaining pref.
function pctLabel(p) {
  return (window.SNAPSHOT && window.SNAPSHOT.pctRemaining) ? (100 - p) + "% left" : p + "%";
}
const $ = (sel, root = document) => root.querySelector(sel);

/* ---------- generic renderers ---------- */
function renderSegments(el, pct) {
  const n = parseInt(el.dataset.segments || "10", 10);
  const filled = Math.round((pct / 100) * n);
  const cls = pct >= 90 ? "crit" : pct >= 75 ? "warn" : "on";
  el.innerHTML = "";
  for (let i = 0; i < n; i++) {
    const b = document.createElement("span");
    b.className = "blk" + (i < filled ? " " + cls : "");
    el.appendChild(b);
  }
}

function renderStack(el, breakdown) {
  const parts = [
    ["cacheRead", breakdown.cacheRead], ["cacheWrite", breakdown.cacheWrite],
    ["input", breakdown.input], ["output", breakdown.output],
  ];
  const total = parts.reduce((s, [, v]) => s + v, 0) || 1;
  el.innerHTML = "";
  for (const [k, v] of parts) {
    if (v <= 0) continue;
    const s = document.createElement("span");
    s.style.width = (v / total) * 100 + "%";
    s.style.background = COLORS[k];
    el.appendChild(s);
  }
}

function renderLegend(el, breakdown) {
  const items = [
    ["cacheRead", "cache read", breakdown.cacheRead], ["cacheWrite", "cache write", breakdown.cacheWrite],
    ["input", "input", breakdown.input], ["output", "output", breakdown.output],
  ];
  el.innerHTML = items.map(([k, label, v]) =>
    `<span class="li"><i style="background:${COLORS[k]}"></i><b>${fmtCompact(v)}</b> ${label}</span>`
  ).join("");
}

/* ---------- canvas charts ---------- */
function setupCanvas(cv, cssHeight) {
  const dpr = window.devicePixelRatio || 1;
  const w = cv.clientWidth || 400;
  cv.width = w * dpr;
  cv.height = cssHeight * dpr;
  cv.style.height = cssHeight + "px"; // pin display height so DPI>1 doesn't double it
  const ctx = cv.getContext("2d");
  ctx.scale(dpr, dpr);
  return { ctx, w, h: cssHeight };
}

function drawSparkline(cv, data, color = "#d9a441") {
  const { ctx, w, h } = setupCanvas(cv, 90);
  ctx.clearRect(0, 0, w, h);
  const max = Math.max(...data, 1), pad = 6;
  const stepX = w / (data.length - 1);
  const y = (v) => h - pad - (v / max) * (h - pad * 2);
  // area
  const grad = ctx.createLinearGradient(0, 0, 0, h);
  grad.addColorStop(0, hexToRgba(color, 0.28));
  grad.addColorStop(1, hexToRgba(color, 0));
  ctx.beginPath();
  ctx.moveTo(0, h);
  data.forEach((v, i) => ctx.lineTo(i * stepX, y(v)));
  ctx.lineTo(w, h);
  ctx.closePath();
  ctx.fillStyle = grad;
  ctx.fill();
  // line
  ctx.beginPath();
  data.forEach((v, i) => (i ? ctx.lineTo(i * stepX, y(v)) : ctx.moveTo(0, y(v))));
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.8;
  ctx.lineJoin = "round";
  ctx.stroke();
}

function drawTrend(cv, trend, barsColor = "#d0774a", lineColor = "#4aa8c9") {
  const { ctx, w, h } = setupCanvas(cv, 150);
  ctx.clearRect(0, 0, w, h);
  const labelH = 18, chartH = h - labelH, pad = 4;
  const maxMsgs = Math.max(...trend.map((d) => d.msgs), 1);
  const maxTok = Math.max(...trend.map((d) => d.tokens), 1);
  const n = trend.length, slot = w / n, barW = slot * 0.5;

  // bars (msgs)
  trend.forEach((d, i) => {
    const bh = (d.msgs / maxMsgs) * (chartH - pad);
    const x = i * slot + (slot - barW) / 2;
    const y = chartH - bh;
    ctx.fillStyle = hexToRgba(barsColor, 0.85);
    const r = 3;
    ctx.beginPath();
    ctx.moveTo(x, y + r); ctx.arcTo(x, y, x + r, y, r);
    ctx.lineTo(x + barW - r, y); ctx.arcTo(x + barW, y, x + barW, y + r, r);
    ctx.lineTo(x + barW, chartH); ctx.lineTo(x, chartH); ctx.closePath(); ctx.fill();
  });

  // smooth line (tokens)
  const px = (i) => i * slot + slot / 2;
  const py = (v) => chartH - (v / maxTok) * (chartH - pad) - 2;
  ctx.beginPath();
  trend.forEach((d, i) => {
    const x = px(i), y = py(d.tokens);
    if (i === 0) ctx.moveTo(x, y);
    else {
      const x0 = px(i - 1), y0 = py(trend[i - 1].tokens);
      const cx = (x0 + x) / 2;
      ctx.bezierCurveTo(cx, y0, cx, y, x, y);
    }
  });
  ctx.strokeStyle = lineColor;
  ctx.lineWidth = 2;
  ctx.lineJoin = "round";
  ctx.stroke();

  // day labels
  ctx.fillStyle = "#8b929c";
  ctx.font = "10px -apple-system, Segoe UI, sans-serif";
  ctx.textAlign = "center";
  trend.forEach((d, i) => ctx.fillText(d.day, px(i), h - 5));
}

/* ---------- render a full snapshot ---------- */
function render(providerKey) {
  const source = window.SNAPSHOT || window.MOCK_SNAPSHOT;
  const snap = providerKey === "overview" ? buildCombined(source) : source[providerKey];
  if (!snap) return;

  const theme = THEME[providerKey] || THEME.claude;
  document.documentElement.style.setProperty("--accent", theme.accent);
  // Codex bars stay blue at every fill level (matches the mini-bar).
  document.documentElement.classList.toggle("prov-codex", providerKey === "codex");

  // limits — usage bars for a single provider; quota status cards for Overview
  const limitsCard = document.querySelector(".card.limits");
  const ovStatus = document.getElementById("overviewStatus");
  if (providerKey === "overview") {
    if (limitsCard) limitsCard.classList.add("hidden");
    if (ovStatus) ovStatus.classList.remove("hidden");
    renderOverviewStatus(source);
  } else {
    if (ovStatus) ovStatus.classList.add("hidden");
    if (limitsCard) limitsCard.classList.remove("hidden");
    for (const key of ["fiveHour", "sevenDay"]) {
      const row = document.querySelector(`.limit-row[data-k="${key}"]`);
      const lim = snap.limits[key];
      renderSegments(row.querySelector(".seg"), lim.pct);
      row.querySelector(".pct").textContent = pctLabel(lim.pct);
      const r = row.querySelector(".reset");
      r.dataset.rel = lim.resetLabel;
      r.dataset.clock = lim.resetClock || lim.resetLabel;
    }
    updateResets();
    const anySrc = snap.limits.fiveHour.source;
    $("#limitsSrc").textContent = anySrc === "providerApi"
      ? (providerKey === "codex" ? "live · matches ChatGPT/Codex counter" : "live · matches Claude Code counter")
      : "estimated from local logs";
  }

  // hero
  $("#provLogo").innerHTML = LOGOS[providerKey] || snap.logo;
  $("#provTitle").textContent = snap.title;
  $("#planBadge").innerHTML = snap.plan.replace(/ {2}/g, "&nbsp;&nbsp;");
  $("#sinceLabel").textContent = snap.since;
  $("#hTokens").textContent = fmtCompact(snap.hero.tokens);
  $("#hSessions").textContent = fmtCompact(snap.hero.sessions);
  $("#hMessages").textContent = fmtCompact(snap.hero.messages);
  $("#hCost").textContent = fmtCost(snap.hero.costUsd);
  renderStack($("#heroStack"), snap.hero.breakdown);
  renderLegend($("#heroLegend"), snap.hero.breakdown);

  // today row
  $("#tMsgs").textContent = snap.today.msgs;
  $("#tSessions").textContent = snap.today.sessions;
  $("#tTools").textContent = snap.today.tools;
  $("#tTokens").textContent = fmtCompact(snap.today.tokens);

  // today's usage
  $("#todayCost").textContent = "~" + fmtCost(snap.today.costUsd) + " API list est.";
  $("#todayTokens").textContent = fmtCompact(snap.today.tokens);
  renderStack($("#todayStack"), snap.today.breakdown);
  renderLegend($("#todayLegend"), snap.today.breakdown);
  $("#sparkRate").textContent = fmtCompact(snap.today.lastHourRatePerMin) + "/m";
  drawSparkline($("#sparkline"), snap.sparkline, theme.spark);

  // trend
  drawTrend($("#trend"), snap.trend, theme.bars, theme.line);
  $("#dotMsgs").style.background = theme.bars;
  $("#dashTokens").style.background = theme.line;
  const avg = (snap.trendPills && snap.trendPills.avgPerDay) || "";
  $("#pillAvg").textContent = avg.includes("💬") ? avg : "💬 " + avg; // tolerate older backends that embed the icon
  $("#pillTotalMsgs").textContent = snap.trendPills.totalMsgs;
  $("#pillTotalTokens").textContent = snap.trendPills.totalTokens;

  // models — each model's share of token usage this period
  $("#modelsTotal").textContent = fmtCompact(snap.models.total);
  const stackEl = $("#modelsStack");
  const mtotal = snap.models.list.reduce((s, m) => s + m.in + m.out, 0) || 1;
  stackEl.innerHTML = snap.models.list.map((m) =>
    `<span style="width:${((m.in + m.out) / mtotal) * 100}%;background:${m.color}"></span>`).join("");
  $("#modelList").innerHTML = snap.models.list.length
    ? snap.models.list.map((m) => {
        const pct = Math.round(((m.in + m.out) / mtotal) * 100);
        return `<div class="model-row"><span class="mdot" style="background:${m.color}"></span>
         <span class="mname">${m.name}</span>
         <span class="mpct">${pct}%</span>
         <span class="mnums"><b>${fmtCompact(m.in)}</b> in &nbsp; <b>${fmtCompact(m.out)}</b> out</span></div>`;
      }).join("")
    : '<div class="placeholder" style="padding:10px">No model usage in this period yet.</div>';

  // Joke: "water guilt" meter (opt-in) — wildly unscientific, for laughs
  const jokeBox = document.getElementById("jokeBox");
  if (jokeBox) {
    if (source.jokeMode) {
      const total = ((source.claude && source.claude.hero.tokens) || 0) + ((source.codex && source.codex.hero.tokens) || 0);
      const liters = total * 0.05 / 1000;   // joke rate: 0.05 mL of "cooling water" per token
      const households = liters / 300;       // ~300 L per household per day
      const towns = households / 1000;       // ~1,000 households per average town
      const hstr = households < 10 ? households.toFixed(1) : fmtCompact(Math.round(households));
      const tstr = towns < 1 ? towns.toFixed(3) : (towns < 10 ? towns.toFixed(1) : fmtCompact(Math.round(towns)));
      jokeBox.classList.remove("hidden");
      const t = document.getElementById("jokeText");
      const sub = document.getElementById("jokeSub");
      if (t) t.innerHTML = `You've evaporated ~<b>${hstr}</b> households' worth of daily water — about <b>${tstr}</b> average towns <span class="muted">(≈${fmtCompact(Math.round(liters))} L across ${fmtCompact(total)} tokens)</span>.`;
      if (sub) sub.textContent = "";
    } else {
      jokeBox.classList.add("hidden");
    }
  }

  updateInfoBar();
  fitWindow();
}

// Overview quota lights (green = quota left, red = capped) + CAD subscription total.
function renderOverviewStatus(source) {
  const codexLogoEl = document.querySelector("#ovsCodex .ovs-logo");
  if (codexLogoEl && !codexLogoEl.dataset.set) { codexLogoEl.innerHTML = LOGOS.codex; codexLogoEl.dataset.set = "1"; }
  const setCard = (id, lim) => {
    const el = document.getElementById(id);
    if (!el || !lim) return;
    const five = lim.fiveHour ? lim.fiveHour.pct : 0;
    const seven = lim.sevenDay ? lim.sevenDay.pct : 0;
    const capped = five >= 100 || seven >= 100;
    const light = el.querySelector(".ovs-light");
    light.classList.toggle("green", !capped);
    light.classList.toggle("red", capped);
    el.querySelector(".ovs-sub").textContent = capped
      ? "Capped out — no quota left" : `Quota available · 5h ${pctLabel(five)} · 7d ${pctLabel(seven)}`;
  };
  setCard("ovsClaude", source.claude && source.claude.limits);
  setCard("ovsCodex", source.codex && source.codex.limits);
  const sub = source.subscription;
  const costEl = document.getElementById("ovsCost");
  if (costEl) costEl.textContent = (sub && typeof sub.totalCad === "number")
    ? `$${sub.totalCad.toFixed(2)} CAD/mo` : "–";
}

function updateResets() {
  document.querySelectorAll(".limit-row .reset").forEach((el) => {
    el.textContent = showClock ? (el.dataset.clock || el.dataset.rel || "") : (el.dataset.rel || "");
  });
}

function updateInfoBar() {
  const bar = document.getElementById("infoBar");
  if (!bar) return;
  const s = window.SNAPSHOT;
  if (window.__lastError) {
    bar.className = "infobar err";
    bar.textContent = "Backend error: " + window.__lastError;
  } else if (s && s.claude.hero.tokens === 0 && s.codex.hero.tokens === 0) {
    bar.className = "infobar warn";
    bar.textContent =
      "No usage logs found yet. Looking in %USERPROFILE%\\.claude\\projects, %APPDATA%\\Claude\\local-agent-mode-sessions (Cowork), and %USERPROFILE%\\.codex\\sessions — run Claude Code / Cowork / Codex, then this updates. See Settings → Diagnostics.";
  } else {
    bar.className = "infobar hidden";
    bar.textContent = "";
  }
}

let lockedH = 0; // the Usage-overview height; all tabs use it so nothing shrinks
function fitWindow() {
  if (!hasTauri() || window.__maxed) return;
  requestAnimationFrame(() => {
    const panel = document.getElementById("panel");
    if (!panel) return;
    const usageVisible = !document.getElementById("tab-usage").classList.contains("hidden");
    const h = Math.ceil(panel.getBoundingClientRect().height) + 2;
    if (usageVisible) lockedH = h;          // remember the Usage size
    const target = lockedH > 0 ? lockedH : h; // apply Usage size on every tab
    if (target !== window.__lastFitH) {
      window.__lastFitH = target;
      try { invoke("fit_panel", { height: target }); } catch (e) {}
    }
  });
}

// Combined Claude+Codex snapshot in the same shape as a single provider.
function buildCombined(source) {
  const c = source.claude, x = source.codex;
  if (!c || !x) return c || x;
  const sb = (a, b) => ({ cacheRead: a.cacheRead + b.cacheRead, cacheWrite: a.cacheWrite + b.cacheWrite, input: a.input + b.input, output: a.output + b.output });
  const higher = (a, b) => (a.pct >= b.pct ? a : b);
  const trend = c.trend.map((d, i) => ({ day: d.day, msgs: d.msgs + (x.trend[i] ? x.trend[i].msgs : 0), tokens: d.tokens + (x.trend[i] ? x.trend[i].tokens : 0) }));
  const totalMsgs = trend.reduce((s, d) => s + d.msgs, 0);
  const totalTokens = trend.reduce((s, d) => s + d.tokens, 0);
  const spark = c.sparkline.map((v, i) => v + (x.sparkline[i] || 0));
  return {
    provider: "overview", title: "Combined", logo: "Σ", plan: "Claude + Codex", since: "all providers ›",
    limits: { fiveHour: higher(c.limits.fiveHour, x.limits.fiveHour), sevenDay: higher(c.limits.sevenDay, x.limits.sevenDay) },
    hero: { tokens: c.hero.tokens + x.hero.tokens, sessions: c.hero.sessions + x.hero.sessions, messages: c.hero.messages + x.hero.messages, costUsd: c.hero.costUsd + x.hero.costUsd, breakdown: sb(c.hero.breakdown, x.hero.breakdown) },
    today: { msgs: c.today.msgs + x.today.msgs, sessions: c.today.sessions + x.today.sessions, tools: c.today.tools + x.today.tools, tokens: c.today.tokens + x.today.tokens, costUsd: c.today.costUsd + x.today.costUsd, breakdown: sb(c.today.breakdown, x.today.breakdown), lastHourRatePerMin: c.today.lastHourRatePerMin + x.today.lastHourRatePerMin },
    sparkline: spark,
    trend,
    trendPills: { avgPerDay: fmtCompact(Math.round(totalMsgs / 14)) + " msgs/day", totalMsgs: "Σ " + fmtCompact(totalMsgs) + " total msgs", totalTokens: "# " + fmtCompact(totalTokens) + " tokens" },
    models: mergeModels(c.models, x.models),
  };
}

// Merge two providers' model breakdowns by model name (sum in/out), keep top 6.
function mergeModels(a, b) {
  const la = (a && a.list) || [], lb = (b && b.list) || [];
  const map = new Map();
  for (const m of [...la, ...lb]) {
    const k = m.name || "unknown";
    const e = map.get(k) || { name: k, color: m.color, in: 0, out: 0 };
    e.in += m.in || 0; e.out += m.out || 0;
    if (!e.color) e.color = m.color;
    map.set(k, e);
  }
  const list = [...map.values()].sort((p, q) => (q.in + q.out) - (p.in + p.out)).slice(0, 6);
  const total = ((a && a.total) || 0) + ((b && b.total) || 0);
  return { total, list };
}

/* ---------- interactions ---------- */
function initTabs() {
  document.querySelectorAll(".tab[data-tab]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const tab = btn.dataset.tab;
      activeTab = tab;
      document.querySelectorAll(".tab[data-tab]").forEach((b) => b.classList.toggle("is-active", b === btn));
      ["usage", "sessions", "history", "processes", "git", "settings"].forEach((t) =>
        $("#tab-" + t).classList.toggle("hidden", t !== tab));
      updateProviderSwitch(tab);
      loadTab(tab);
    });
  });
}

// The provider switch belongs only to Usage (Overview/Claude/Codex) and
// History (Claude/Codex — no Overview). Hidden on every other tab.
function updateProviderSwitch(tab) {
  const sw = document.querySelector(".provider-switch");
  if (!sw) return;
  const show = tab === "usage" || tab === "history";
  sw.classList.toggle("hidden", !show);
  const ovBtn = sw.querySelector('[data-provider="overview"]');
  if (ovBtn) ovBtn.classList.toggle("hidden", tab !== "usage");
  const active = tab === "history" ? histProvider : currentProvider;
  sw.querySelectorAll(".prov").forEach((b) => b.classList.toggle("is-active", b.dataset.provider === active));
}

const hasTauri = () => !!window.__TAURI__?.core?.invoke;
const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args);

let ALIASES = { sessions: {}, projects: {} };
async function loadAliases() { if (hasTauri()) { try { ALIASES = await invoke("get_aliases"); } catch (e) {} } }

// Inline-rename a text element; calls onSave(newValue) on commit, then re-renders.
function inlineRename(spanEl, current, onSave) {
  const input = document.createElement("input");
  input.className = "rename-input";
  input.value = current;
  spanEl.replaceWith(input);
  input.focus(); input.select();
  let done = false;
  const commit = (save) => { if (done) return; done = true; if (save) onSave(input.value.trim()); else onSave(null); };
  input.addEventListener("keydown", (e) => { if (e.key === "Enter") commit(true); else if (e.key === "Escape") commit(false); });
  input.addEventListener("blur", () => commit(true));
  input.addEventListener("click", (e) => e.stopPropagation());
}

function confirmDialog(message, onYes) {
  const ov = document.createElement("div");
  ov.className = "confirm-ov";
  ov.innerHTML = `<div class="confirm-box"><div class="confirm-msg"></div>
    <div class="confirm-actions"><button class="cbtn cancel">Cancel</button><button class="cbtn danger">Delete</button></div></div>`;
  ov.querySelector(".confirm-msg").textContent = message;
  document.body.appendChild(ov);
  const close = () => ov.remove();
  ov.querySelector(".cancel").addEventListener("click", close);
  ov.querySelector(".danger").addEventListener("click", () => { close(); onYes(); });
  ov.addEventListener("click", (e) => { if (e.target === ov) close(); });
}

function loadTab(tab) {
  if (tab === "processes") loadProcesses();
  else if (tab === "git") loadGit();
  else if (tab === "settings") loadSettings();
  else if (tab === "sessions") loadSessions();
  else if (tab === "history") loadHistory();
  fitWindow();
  setTimeout(fitWindow, 80); // refit after async content lands
}

const escapeHtml = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const basenameJs = (p) => (p || "").split(/[\\/]/).filter(Boolean).pop() || "";
function fmtAgo(ms) {
  const s = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (s < 60) return "just now";
  const m = Math.floor(s / 60); if (m < 60) return m + "m ago";
  const h = Math.floor(m / 60); if (h < 24) return h + "h ago";
  return Math.floor(h / 24) + "d ago";
}

function shortModel(m) {
  const s = String(m || "").toLowerCase();
  if (s.includes("opus")) return "Opus";
  if (s.includes("sonnet")) return "Sonnet";
  if (s.includes("haiku")) return "Haiku";
  if (s.includes("codex")) return "Codex";
  if (s.includes("gpt-5")) return "GPT-5";
  if (s.includes("o4")) return "o4-mini";
  return m || "";
}

const providerOf = (s) => (/codex|gpt/i.test(s.client || "") ? "codex" : "claude");
async function fetchSessions(prov) {
  if (prov === "overview") {
    const [c, x] = await Promise.all([
      invoke("get_sessions", { provider: "claude" }),
      invoke("get_sessions", { provider: "codex" }),
    ]);
    return [...c, ...x];
  }
  return invoke("get_sessions", { provider: prov });
}

// Coarse agent state from last activity — matches the backend widget indicator.
function agentStatus(lastMs) {
  const idle = Date.now() - lastMs;
  if (idle < 45000) return "working";
  if (idle < 300000) return "waiting";
  return "stopped";
}
const STATUS_LABEL = { working: "Working", waiting: "Waiting for you", stopped: "Stopped" };
const STATUS_CLASS = { working: "c-green", waiting: "c-amber", stopped: "muted" };

// Shared expandable detail panel for a session/chat (Sessions + History).
function sessionDetailHtml(s) {
  const pct = s.contextTokens ? Math.min(100, Math.round(s.contextTokens / 200000 * 100)) : 0;
  const ctx = s.contextTokens ? `${fmtCompact(s.contextTokens)}/200k ${pct}%` : "—";
  const st = agentStatus(s.lastMs);
  const status = STATUS_LABEL[st] || "—";
  return `<div class="sl-detail">
    <div class="sld-row"><span>Agent</span><b class="${STATUS_CLASS[st] || "muted"}">${status}</b></div>
    <div class="sld-row"><span>Model</span><b>${escapeHtml(s.model || "—")}</b></div>
    <div class="sld-row"><span>Context</span><b>${ctx}</b></div>
    <div class="sld-row"><span>Messages</span><b>${s.messages}</b></div>
    <div class="sld-row"><span>Total tokens</span><b>${fmtCompact(s.tokens)}</b></div>
    <div class="sld-row"><span>Cost (est)</span><b>$${(s.costUsd || 0).toFixed(2)}</b></div>
    <div class="sld-row"><span>Last activity</span><b>${fmtAgo(s.lastMs)}</b></div>
  </div>`;
}

const AG_LABEL = { working: "Working", waiting: "Waiting", stopped: "Stopped", running: "Running", done: "Done" };
const AG_DOT = { working: "working", waiting: "waiting", stopped: "off", running: "working", done: "off" };
function agentRow(a) {
  return `<div class="ag-row">
    <span class="sl-dot ${AG_DOT[a.status] || "off"}"></span>
    <div class="ag-main">
      <div class="ag-name">${escapeHtml(a.name)} <span class="muted">· ${escapeHtml(shortModel(a.model))}</span></div>
      <div class="ag-goal">${escapeHtml(a.goal || "—")}</div>
    </div>
    <span class="ag-status">${AG_LABEL[a.status] || escapeHtml(a.status || "")}</span>
  </div>`;
}

// Sessions = chats with an agent active in the last 30 minutes.
async function loadSessions() {
  const list = $("#sessionList");
  if (!hasTauri()) { list.innerHTML = '<section class="card"><div class="placeholder">Sessions are available in the installed app.</div></section>'; return; }
  try {
    const all = await fetchSessions("overview");
    const THIRTY_M = 30 * 60 * 1000;
    const sess = all.filter((s) => Date.now() - s.lastMs < THIRTY_M);
    const projName = (cwd) => { const b = basenameJs(cwd); return ALIASES.projects[b] || b; };
    const chatName = (s) => ALIASES.sessions[s.id] || s.name || projName(s.cwd) || "chat";
    const statline = (s) => {
      const pct = s.contextTokens ? Math.min(100, Math.round(s.contextTokens / 200000 * 100)) : 0;
      const ctx = s.contextTokens ? `${fmtCompact(s.contextTokens)}/200k <span class="muted">${pct}%</span>` : "";
      const live = Date.now() - s.lastMs < 120000; // working within the last 2 min
      const proj = projName(s.cwd);
      return `
      <section class="card statline" data-id="${escapeHtml(s.id)}" data-cwd="${escapeHtml(s.cwd)}" data-provider="${providerOf(s)}" title="Click to open its window">
        <div class="sl-top">
          <span class="sl-dot ${live ? "working" : "off"}" title="${live ? "Working" : "Idle"}"></span>
          <span class="sl-name">${escapeHtml(chatName(s))}</span>
          <button class="ren sl-ren" data-id="${escapeHtml(s.id)}" title="Rename chat">✎</button>
          <span class="sl-ago">${fmtAgo(s.lastMs)}</span>
          <button class="sl-exp" title="Agents">▸</button>
        </div>
        <div class="sl-bot">
          <span class="sl-model">${escapeHtml(shortModel(s.model))}</span>
          ${proj ? `<span class="sl-projtag">${escapeHtml(proj)}</span>` : ""}
          ${s.branch ? `<span class="sl-branch">⑂ ${escapeHtml(s.branch)}</span>` : ""}
          ${ctx ? `<span class="sl-ctx">${ctx}</span>` : ""}
          <span class="sl-cost">$${(s.costUsd || 0).toFixed(2)}</span>
          <span class="sl-msgs">${s.messages} msgs · ${fmtCompact(s.tokens)} tok</span>
        </div>
        <div class="sl-agents"><div class="hint" style="padding:6px 2px">Loading agents…</div></div>
        ${sessionDetailHtml(s)}
      </section>`;
    };
    let html = "";
    if (!sess.length) {
      html = `<section class="card"><div class="placeholder">No chats with agents active in the last 30 minutes.<br><span class="muted">Start a Claude Code / Codex chat, or see History for older sessions.</span></div></section>`;
    } else {
      const groups = {};
      for (const s of sess) { const c = s.client || "Other"; (groups[c] = groups[c] || []).push(s); }
      const order = ["Claude Code", "Claude Cowork", "Claude Chat", "Codex", "GPT"];
      const keys = Object.keys(groups).sort((a, b) => { const ia = order.indexOf(a), ib = order.indexOf(b); return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib); });
      html = keys.map((c) => `
        <div class="sess-group">
          <div class="sg-head">${escapeHtml(c)}<span class="sg-count">${groups[c].length}</span></div>
          ${groups[c].map(statline).join("")}
        </div>`).join("");
    }
    list.innerHTML = html;
    list.querySelectorAll(".statline").forEach((r) => {
      const exp = r.querySelector(".sl-exp");
      if (exp) exp.addEventListener("click", async (e) => {
        e.stopPropagation();
        const open = r.classList.toggle("expanded");
        exp.textContent = open ? "▾" : "▸";
        if (open && !r.dataset.loaded) {
          r.dataset.loaded = "1";
          const box = r.querySelector(".sl-agents");
          try {
            const agents = await invoke("get_session_agents", { provider: r.dataset.provider, id: r.dataset.id });
            box.innerHTML = agents.length
              ? '<div class="ag-head">Agents</div>' + agents.map(agentRow).join("")
              : '<div class="hint" style="padding:6px 2px">No agent detail available.</div>';
          } catch (err) { box.innerHTML = '<div class="hint" style="padding:6px 2px">Could not read agents.</div>'; }
        }
        fitWindow();
      });
      const ren = r.querySelector(".sl-ren");
      if (ren) ren.addEventListener("click", (e) => {
        e.stopPropagation();
        const label = r.querySelector(".sl-name"), id = ren.dataset.id;
        inlineRename(label, ALIASES.sessions[id] || label.textContent, (nv) => {
          if (nv !== null) { if (nv) ALIASES.sessions[id] = nv; else delete ALIASES.sessions[id]; invoke("set_session_alias", { id, name: nv || "" }); }
          loadSessions();
        });
      });
      r.addEventListener("click", async (e) => {
        if (e.target.closest(".ren") || e.target.closest(".sl-exp") || e.target.tagName === "INPUT") return;
        // Bring the chat's already-open window to the front — don't spawn a new one.
        try { await invoke("focus_chat", { provider: r.dataset.provider, cwd: r.dataset.cwd }); } catch (e2) {}
      });
    });
  } catch (e) {
    list.innerHTML = '<section class="card"><div class="placeholder">Could not read sessions.</div></section>';
  }
  fitWindow();
}

const historyCollapsed = new Set();
async function loadHistory() {
  const list = $("#historyList");
  if (!hasTauri()) { list.innerHTML = '<section class="card"><div class="placeholder">History is available in the installed app.</div></section>'; return; }
  try {
    const sess = await fetchSessions(histProvider);
    if (!sess.length) { list.innerHTML = `<section class="card"><div class="placeholder">No history found.</div></section>`; fitWindow(); return; }
    const groups = {};
    for (const s of sess) { const p = basenameJs(s.cwd) || "other"; (groups[p] = groups[p] || []).push(s); }
    const keys = Object.keys(groups).sort((a, b) => Math.max(...groups[b].map((x) => x.lastMs)) - Math.max(...groups[a].map((x) => x.lastMs)));
    const projName = (p) => ALIASES.projects[p] || p;
    const chatName = (s) => ALIASES.sessions[s.id] || s.name;
    list.innerHTML = keys.map((p) => {
      const collapsed = historyCollapsed.has(p);
      const chats = groups[p];
      const pt = chats.reduce((a, s) => ({ tokens: a.tokens + (s.tokens || 0), messages: a.messages + (s.messages || 0), cost: a.cost + (s.costUsd || 0) }), { tokens: 0, messages: 0, cost: 0 });
      const pModels = [...new Set(chats.map((s) => shortModel(s.model)).filter(Boolean))].join(", ");
      const pLast = Math.max(...chats.map((s) => s.lastMs));
      return `
      <section class="card hist-group ${collapsed ? "collapsed" : ""}" data-proj="${escapeHtml(p)}">
        <div class="hg-head">
          <span class="hg-name"><span class="hg-chev">▾</span> <span class="hg-label">${escapeHtml(projName(p))}</span></span>
          <span class="hg-right"><button class="ren proj-exp" title="Project totals">Σ</button><button class="ren ren-proj" title="Rename project">✎</button><button class="ren del-proj" title="Delete project">🗑</button><span class="hg-count">${chats.length} chat${chats.length === 1 ? "" : "s"}</span></span>
        </div>
        <div class="hist-proj-detail">
          <div class="sld-row"><span>Chats</span><b>${chats.length}</b></div>
          <div class="sld-row"><span>Total tokens</span><b>${fmtCompact(pt.tokens)}</b></div>
          <div class="sld-row"><span>Messages</span><b>${pt.messages}</b></div>
          <div class="sld-row"><span>Total cost (est)</span><b>$${pt.cost.toFixed(2)}</b></div>
          <div class="sld-row"><span>Models</span><b>${escapeHtml(pModels || "—")}</b></div>
          <div class="sld-row"><span>Last activity</span><b>${fmtAgo(pLast)}</b></div>
        </div>
        <div class="hg-body">
          ${groups[p].sort((a, b) => b.lastMs - a.lastMs).map((s) => `
            <div class="hist-row" data-id="${escapeHtml(s.id)}" data-cwd="${escapeHtml(s.cwd)}" data-provider="${providerOf(s)}" title="Open in ${escapeHtml(s.client || "client")}">
              <span class="hdot ${s.active ? "on" : ""}"></span>
              <div class="hmain"><div class="hname">${escapeHtml(chatName(s))}</div>
                <div class="hmeta">${escapeHtml(shortModel(s.model))}${s.branch ? " · ⑂ " + escapeHtml(s.branch) : ""}</div></div>
              <button class="ren ren-chat" data-id="${escapeHtml(s.id)}" title="Rename chat">✎</button>
              <button class="ren del-chat" data-id="${escapeHtml(s.id)}" title="Delete chat">🗑</button>
              <button class="ren hist-exp" title="Details">▸</button>
              <div class="hright">${fmtAgo(s.lastMs)}<div class="hsub">${s.messages} msgs</div></div>
              ${sessionDetailHtml(s)}
            </div>`).join("")}
        </div>
      </section>`;
    }).join("");

    list.querySelectorAll(".hist-group .hg-head").forEach((h) => h.addEventListener("click", (e) => {
      if (e.target.closest(".ren") || e.target.tagName === "INPUT") return;
      const g = h.closest(".hist-group"), p = g.dataset.proj;
      g.classList.toggle("collapsed");
      if (g.classList.contains("collapsed")) historyCollapsed.add(p); else historyCollapsed.delete(p);
      fitWindow();
    }));
    list.querySelectorAll(".hist-row").forEach((r) => r.addEventListener("click", (e) => {
      if (e.target.closest(".ren") || e.target.tagName === "INPUT") return;
      invoke("open_chat", { provider: r.dataset.provider, id: r.dataset.id, cwd: r.dataset.cwd });
    }));
    list.querySelectorAll(".hist-exp").forEach((b) => b.addEventListener("click", (e) => {
      e.stopPropagation();
      const row = b.closest(".hist-row");
      const open = row.classList.toggle("expanded");
      b.textContent = open ? "▾" : "▸";
      fitWindow();
    }));
    list.querySelectorAll(".del-proj").forEach((b) => b.addEventListener("click", (e) => {
      e.stopPropagation();
      const g = b.closest(".hist-group");
      const rows = [...g.querySelectorAll(".hist-row")];
      const label = (g.querySelector(".hg-label") || {}).textContent || "this project";
      confirmDialog(`Delete all ${rows.length} chat${rows.length === 1 ? "" : "s"} in “${label}”? They'll move to a recoverable trash folder.`, async () => {
        for (const row of rows) {
          try { await invoke("delete_session", { provider: row.dataset.provider, id: row.dataset.id }); } catch (err) {}
        }
        loadHistory();
      });
    }));
    list.querySelectorAll(".proj-exp").forEach((b) => b.addEventListener("click", (e) => {
      e.stopPropagation();
      const g = b.closest(".hist-group");
      const open = g.classList.toggle("proj-open");
      b.classList.toggle("on", open);
      fitWindow();
    }));
    list.querySelectorAll(".del-chat").forEach((b) => b.addEventListener("click", (e) => {
      e.stopPropagation();
      const row = b.closest(".hist-row"), id = row.dataset.id, prov = row.dataset.provider;
      confirmDialog("Delete this chat from history? It will be moved to a recoverable trash folder.", async () => {
        try { await invoke("delete_session", { provider: prov, id }); } catch (err) {}
        loadHistory(); // reload only after the move finished, so the row is really gone
      });
    }));
    list.querySelectorAll(".ren-proj").forEach((b) => b.addEventListener("click", (e) => {
      e.stopPropagation();
      const g = b.closest(".hist-group"), p = g.dataset.proj, label = g.querySelector(".hg-label");
      inlineRename(label, projName(p), (nv) => {
        if (nv !== null) { if (nv) ALIASES.projects[p] = nv; else delete ALIASES.projects[p]; invoke("set_project_alias", { key: p, name: nv || "" }); }
        loadHistory();
      });
    }));
    list.querySelectorAll(".ren-chat").forEach((b) => b.addEventListener("click", (e) => {
      e.stopPropagation();
      const row = b.closest(".hist-row"), id = b.dataset.id, label = row.querySelector(".hname");
      inlineRename(label, ALIASES.sessions[id] || label.textContent, (nv) => {
        if (nv !== null) { if (nv) ALIASES.sessions[id] = nv; else delete ALIASES.sessions[id]; invoke("set_session_alias", { id, name: nv || "" }); }
        loadHistory();
      });
    }));
  } catch (e) { list.innerHTML = '<section class="card"><div class="placeholder">Could not read history.</div></section>'; }
  fitWindow();
}

async function loadProcesses() {
  const list = $("#procList"), empty = $("#procEmpty");
  if (!hasTauri()) { list.innerHTML = '<section class="card"><div class="placeholder">Process list is available in the installed app.</div></section>'; return; }
  try {
    const wins = await invoke("get_windows");
    empty.classList.add("hidden");
    const row = (w) => `
        <div class="proc-row" data-hwnd="${w.hwnd}" title="Bring to front">
          <span class="pdot"></span>
          <span class="tool">${escapeHtml(w.title)}</span>
          <span class="pmeta">${escapeHtml((w.process || "").replace(/\.exe$/i, ""))}</span>
          <span class="pfocus">↗</span>
        </div>`;
    const isClaude = (w) => /claude/i.test((w.title || "") + " " + (w.process || ""));
    const isCodex = (w) => /codex|gpt|chatgpt|copilot/i.test((w.title || "") + " " + (w.process || ""));
    const claude = wins.filter(isClaude);
    const codex = wins.filter((w) => !isClaude(w) && isCodex(w));
    const card = (t, arr) => `<section class="card proc-group"><div class="gh"><span class="host">${t}</span><span class="count">${arr.length}</span></div>` +
      (arr.length ? arr.map(row).join("") : '<div class="placeholder" style="padding:14px">None open</div>') + '</section>';
    list.innerHTML = card("Claude", claude) + card("Codex / GPT", codex);
    list.querySelectorAll(".proc-row").forEach((r) => r.addEventListener("click", () => invoke("focus_window", { hwnd: parseInt(r.dataset.hwnd, 10) })));
  } catch (e) { list.innerHTML = '<section class="card"><div class="placeholder">Could not read windows.</div></section>'; }
  fitWindow();
}

async function loadGit() {
  const list = $("#gitList");
  if (!hasTauri()) { list.innerHTML = '<section class="card"><div class="placeholder">Git Dash is available in the installed app.</div></section>'; return; }
  let html = "";
  // Local repos (discovered from session working dirs)
  try {
    const g = await invoke("get_git");
    if (g.available && g.repos && g.repos.length) {
      const rows = g.repos.map((r) => {
        const sync = [];
        if (r.ahead) sync.push(`<span class="ahead">+${r.ahead}</span>`);
        if (r.behind) sync.push(`<span class="behind">-${r.behind}</span>`);
        const syncStr = sync.join(" ") || `<span class="muted">=</span>`;
        const dirtyStr = r.dirty ? `<span class="dnum">${r.dirty}</span>` : `<span class="muted">–</span>`;
        return `
        <div class="repo-row ${r.url ? "" : "nolink"}" data-url="${escapeHtml(r.url)}" title="${r.url ? "Open on GitHub" : "No remote"}">
          <span class="rname"><span class="rdot ${r.status}"></span>${escapeHtml(r.name)}</span>
          <span class="rbranch">${escapeHtml(r.branch)}</span>
          <span class="rsync">${syncStr}</span>
          <span class="rdirty">${dirtyStr}</span>
          <span class="rcommit">${escapeHtml(r.lastCommit)}</span>
        </div>`;
      }).join("");
      html += `
        <section class="card gitdash">
          <div class="gd-title">Local repos</div>
          <div class="gd-head"><span>Repo</span><span>Branch</span><span>Sync</span><span>Dirty</span><span>Last commit</span></div>
          ${rows}
          <div class="gd-foot">
            <span>${g.total} repos</span><span>${g.dirty} dirty</span><span>${g.unpushed} unpushed</span>
            <span class="gd-legend">clean<i class="lg clean"></i> dirty<i class="lg dirty"></i> unpushed<i class="lg unpushed"></i></span>
          </div>
        </section>`;
    }
  } catch (e) {}
  // GitHub repos — one card per connected account
  let ghUsers = [];
  try {
    const st = await invoke("get_settings");
    ghUsers = (st.githubUsers && st.githubUsers.length) ? st.githubUsers : (st.githubUser ? [st.githubUser] : []);
    const off = st.githubUsersDisabled || [];
    ghUsers = ghUsers.filter((u) => !off.includes(u));
  } catch (e) {}
  for (const ghUser of ghUsers) {
    try {
      const repos = await invoke("github_repos", { user: ghUser });
      const rows = repos.slice(0, 40).map((r) => `
        <div class="repo-row" data-url="${escapeHtml(r.url)}" title="Open on GitHub">
          <span class="rname"><span class="rdot ${r.private ? "dirty" : "clean"}"></span>${escapeHtml(r.name)}</span>
          <span class="rbranch">${escapeHtml(r.language || "")}</span>
          <span class="rsync">${r.stars ? "★ " + r.stars : ""}</span>
          <span class="rdirty"></span>
          <span class="rcommit">${escapeHtml((r.description || "").slice(0, 44))}</span>
        </div>`).join("");
      html += `
        <section class="card gitdash">
          <div class="gd-title"><img class="gh-avatar" src="https://github.com/${encodeURIComponent(ghUser)}.png?size=48" alt="" onerror="this.style.display='none'" />GitHub · ${escapeHtml(ghUser)} <span class="muted">(${repos.length})</span></div>
          ${rows || '<div class="placeholder">No public repos.</div>'}
        </section>`;
    } catch (e) {
      html += `<section class="card"><div class="placeholder">GitHub (${escapeHtml(ghUser)}): ${escapeHtml(String(e))}</div></section>`;
    }
  }
  if (!html) html = '<section class="card"><div class="placeholder">No local repos found. Connect a GitHub account in Settings to list your repos.</div></section>';
  list.innerHTML = html;
  list.querySelectorAll(".repo-row").forEach((r) => r.addEventListener("click", () => { const u = r.dataset.url; if (u) invoke("open_url", { url: u }); }));
  fitWindow();
}

const settingToggles = [
  ["useProviderApi", "Match Claude Code counter", "Read local OAuth to call the provider usage API"],
  ["minibarEnabled", "Show docked mini-bar", "Slim always-on-top 5h/7d strip"],
  ["trackClaude", "Track Claude Code", ""],
  ["trackCodex", "Track Codex", ""],
  ["pctRemaining", "Show remaining %", "Display % left instead of % used"],
  ["notifyEnabled", "Usage alerts", "Notify at 75 / 90 / 95% per provider"],
  ["jokeMode", "💧 Water guilt mode", "A droplet estimating the households of water your tokens ‘evaporated’"],
  ["autostart", "Start on login", ""],
];

async function loadSettings() {
  const card = $("#settingsCard");
  if (!hasTauri()) { card.innerHTML = '<div class="placeholder">Settings are available in the installed app.</div>'; return; }
  let s;
  try { s = await invoke("get_settings"); } catch (e) { card.innerHTML = '<div class="placeholder">Could not load settings.</div>'; return; }
  const rows = settingToggles.map(([k, label, hint]) => `
    <div class="set-row"><div><div class="lbl">${label}</div>${hint ? `<div class="hint">${hint}</div>` : ""}</div>
      <button class="toggle ${s[k] ? "on" : ""}" data-k="${k}"><span class="knob"></span></button></div>`).join("");
  card.innerHTML = rows + `
    <div class="set-row"><div class="lbl">Mini-bar position</div>
      <select data-k="minibarCorner">${["bottomLeft","bottomRight","topLeft","topRight"].map((c) => `<option value="${c}" ${s.minibarCorner === c ? "selected" : ""}>${c}</option>`).join("")}</select></div>
    <div class="set-row"><div><div class="lbl">Hotkey</div><div class="hint">e.g. CmdOrControl+Shift+U · applies immediately</div></div>
      <input type="text" data-k="hotkey" value="${escapeHtml(s.hotkey)}" /></div>
    <div class="set-row"><div class="lbl">Refresh (seconds)</div><input type="number" min="2" data-k="refreshSecs" value="${s.refreshSecs}" /></div>
    <div class="set-row"><div class="lbl">Tray icon style</div>
      <select data-k="trayStyle">${["fill","ring","bar"].map((c) => `<option value="${c}" ${s.trayStyle === c ? "selected" : ""}>${c}</option>`).join("")}</select></div>
    <div class="set-row"><div class="lbl">Tray colors</div>
      <select data-k="trayColor">${["multi","mono"].map((c) => `<option value="${c}" ${s.trayColor === c ? "selected" : ""}>${c}</option>`).join("")}</select></div>`;
  const push = () => invoke("set_settings", { settings: s }).catch((e) => console.error("set_settings failed:", e));
  card.querySelectorAll(".toggle").forEach((b) => b.addEventListener("click", () => { const k = b.dataset.k; s[k] = !s[k]; b.classList.toggle("on", s[k]); push(); }));
  card.querySelectorAll("select,input").forEach((el) => el.addEventListener("change", () => {
    const k = el.dataset.k;
    if (!k) return;
    if (el.type === "number") {
      const n = parseInt(el.value, 10);
      if (!Number.isFinite(n)) { el.value = s[k]; return; } // ignore blank/garbage input
      s[k] = k === "refreshSecs" ? Math.max(2, n) : n;
      el.value = s[k];
    } else {
      s[k] = el.value;
    }
    push();
  }));

  // Profiles — named presets of display settings (tracking, tray style/color, % mode)
  const prof = document.createElement("div");
  prof.className = "diag";
  prof.innerHTML = "<b>Profiles</b>";
  const profs = Array.isArray(s.profiles) ? s.profiles : [];
  const pStatus = document.createElement("div");
  pStatus.className = "hint";
  if (profs.length) {
    const pRow = document.createElement("div");
    pRow.className = "conn-row";
    pRow.innerHTML = `<div class="lbl">Active</div>
      <div class="conn-in"><select id="profSel">${profs.map((p) =>
        `<option value="${escapeHtml(p.name)}" ${p.name === s.activeProfile ? "selected" : ""}>${escapeHtml(p.name)}</option>`).join("")}</select>
      <button class="folder-btn" id="profSwitch" title="Apply this profile's display settings">Switch</button>
      <button class="gh-rm" id="profDel" title="Delete selected profile">×</button></div>`;
    prof.appendChild(pRow);
    const sel = pRow.querySelector("#profSel");
    pRow.querySelector("#profSwitch").addEventListener("click", async () => {
      pStatus.textContent = "Switching…";
      try { await invoke("switch_profile", { name: sel.value }); loadSettings(); }
      catch (e) { pStatus.textContent = "Error: " + e; }
    });
    pRow.querySelector("#profDel").addEventListener("click", () => {
      const name = sel.value;
      confirmDialog(`Delete profile "${name}"? Your current settings are not affected.`, async () => {
        try { await invoke("delete_profile", { name }); } catch (e) {}
        loadSettings();
      });
    });
  } else {
    pStatus.textContent = "No profiles yet — save the current display settings below to create one.";
  }
  const pSave = document.createElement("div");
  pSave.className = "conn-row";
  pSave.innerHTML = `<div class="lbl">Save current as</div>
    <div class="conn-in"><input type="text" id="profName" placeholder="profile name" maxlength="40" />
    <button class="folder-btn" id="profSaveBtn" title="Save the current display settings as a profile">Save</button></div>`;
  prof.appendChild(pSave);
  const profNameIn = pSave.querySelector("#profName");
  const saveProfile = async () => {
    const name = profNameIn.value.trim();
    if (!name) { profNameIn.focus(); return; }
    pStatus.textContent = "Saving…";
    try { await invoke("save_profile", { name }); loadSettings(); }
    catch (e) { pStatus.textContent = "Error: " + e; }
  };
  pSave.querySelector("#profSaveBtn").addEventListener("click", saveProfile);
  profNameIn.addEventListener("keydown", (e) => { if (e.key === "Enter") saveProfile(); });
  prof.appendChild(pStatus);
  const pHint = document.createElement("div");
  pHint.className = "hint";
  pHint.textContent = "A profile stores: track Claude/Codex, tray icon style + colors, and used/remaining % mode.";
  prof.appendChild(pHint);
  card.appendChild(prof);

  // Widget indicator — which chat's agent status drives the taskbar light
  try {
    const isess = (await fetchSessions("overview")).slice().sort((a, b) => b.lastMs - a.lastMs).slice(0, 20);
    const ind = document.createElement("div"); ind.className = "diag";
    ind.innerHTML = "<b>Widget indicator</b>";
    const optFor = (x) => `<option value="${escapeHtml(x.id)}" ${x.id === s.indicatorSessionId ? "selected" : ""}>${escapeHtml(((ALIASES.sessions[x.id] || x.name || "chat").slice(0, 40)) + " · " + shortModel(x.model))}</option>`;
    // Sub-grouped by client: Claude ▸ Code / Cowork / Chat, GPT ▸ Codex / ChatGPT.
    const GROUPS = [
      ["Claude ▸ Code", "Claude Code"],
      ["Claude ▸ Cowork", "Claude Cowork"],
      ["Claude ▸ Chat", "Claude Chat"],
      ["GPT ▸ Codex", "Codex"],
      ["GPT ▸ ChatGPT", "GPT"],
    ];
    let opts = '<option value="">Auto (most recent chat)</option>';
    for (const [label, client] of GROUPS) {
      const items = isess.filter((x) => (x.client || "") === client);
      if (items.length) opts += `<optgroup label="${label}">${items.map(optFor).join("")}</optgroup>`;
    }
    const known = GROUPS.map((g) => g[1]);
    const other = isess.filter((x) => !known.includes(x.client || ""));
    if (other.length) opts += `<optgroup label="Other">${other.map(optFor).join("")}</optgroup>`;
    const iRow = document.createElement("div"); iRow.className = "conn-row";
    iRow.innerHTML = `<div class="lbl">Agent status chat</div><div class="conn-in"><select id="indSel">${opts}</select></div>`;
    ind.appendChild(iRow);
    const iHint = document.createElement("div"); iHint.className = "hint";
    iHint.textContent = "The widget light tracks this chat: green = working, amber = waiting for you, grey = stopped.";
    ind.appendChild(iHint);
    card.appendChild(ind);
    iRow.querySelector("#indSel").addEventListener("change", (e) => { s.indicatorSessionId = e.target.value; push(); });
  } catch (e) {}

  // Connections — GitHub + exact-usage (provider API via local sign-in)
  const conn = document.createElement("div");
  conn.className = "diag";
  conn.innerHTML = "<b>Connections</b>";
  // GitHub — multiple accounts, each shown as a connected badge (like Claude/OpenAI)
  if (!Array.isArray(s.githubUsers)) s.githubUsers = [];
  if (!s.githubUsers.length && s.githubUser) s.githubUsers = [s.githubUser]; // migrate legacy single value
  const gh = document.createElement("div");
  gh.className = "conn-row";
  gh.innerHTML = `<div class="lbl">GitHub</div>
    <div class="conn-in"><input type="text" id="ghUser" placeholder="username or profile URL" />
    <button class="folder-btn" id="ghAdd">Add</button></div>`;
  conn.appendChild(gh);
  if (!Array.isArray(s.githubUsersDisabled)) s.githubUsersDisabled = [];
  const ghOn = (u) => !s.githubUsersDisabled.includes(u);
  const ghList = document.createElement("div"); ghList.className = "gh-accts"; conn.appendChild(ghList);
  const renderGh = () => {
    ghList.innerHTML = s.githubUsers.length
      ? s.githubUsers.map((u) => `<div class="gh-acct">
          <button class="toggle sm ${ghOn(u) ? "on" : ""}" data-u="${escapeHtml(u)}" title="Show in Git tab"><span class="knob"></span></button>
          <b class="gh-name">${escapeHtml(u)}</b>
          <button class="gh-rm" data-u="${escapeHtml(u)}" title="Remove account">×</button>
        </div>`).join("")
      : '<div class="hint">Add a GitHub username to list its public repos in the Git tab.</div>';
    ghList.querySelectorAll(".gh-acct .toggle").forEach((t) => t.addEventListener("click", () => {
      const u = t.dataset.u;
      if (s.githubUsersDisabled.includes(u)) s.githubUsersDisabled = s.githubUsersDisabled.filter((x) => x !== u);
      else s.githubUsersDisabled.push(u);
      t.classList.toggle("on", ghOn(u));
      push();
    }));
    ghList.querySelectorAll(".gh-rm").forEach((b) => b.addEventListener("click", () => {
      s.githubUsers = s.githubUsers.filter((x) => x !== b.dataset.u);
      s.githubUsersDisabled = s.githubUsersDisabled.filter((x) => x !== b.dataset.u);
      s.githubUser = s.githubUsers[0] || "";
      push(); renderGh();
    }));
  };
  renderGh();
  gh.querySelector("#ghAdd").addEventListener("click", async () => {
    const inp = document.getElementById("ghUser"); const v = inp.value.trim();
    if (!v) return;
    const u = (v.replace(/\/+$/, "").split(/[\\/]/).filter(Boolean).pop() || "").replace(/^@/, "");
    if (u && !s.githubUsers.includes(u)) s.githubUsers.push(u);
    s.githubUser = s.githubUsers[0] || "";
    inp.value = ""; push(); renderGh();
    try { await invoke("github_repos", { user: u }); } catch (e) {}
  });
  gh.querySelector("#ghUser").addEventListener("keydown", (e) => { if (e.key === "Enter") gh.querySelector("#ghAdd").click(); });
  try {
    const cs = await invoke("connections_status");
    const badge = (name, ok) => `<div class="conn-badge"><span class="ovs-light ${ok ? "green" : "red"}"></span>${name}: <b>${ok ? "connected" : "sign in via CLI / app"}</b></div>`;
    const ex = document.createElement("div"); ex.className = "conn-status";
    ex.innerHTML = badge("Claude", cs.claude) + badge("OpenAI / Codex", cs.codex);
    conn.appendChild(ex);
    const note = document.createElement("div"); note.className = "hint";
    note.textContent = 'Exact 5h/7d comes from the provider API using your local Claude Code / Codex sign-in — enable "Match Claude Code counter" above. TokenHub reads local credentials only and never stores your keys.';
    conn.appendChild(note);
  } catch (e) {}

  // API keys — stored encrypted in the OS credential store; never shown or saved to config
  try {
    const ks = await invoke("api_keys_status");
    const keyRow = (label, prov, stored) => {
      const row = document.createElement("div");
      row.className = "conn-row";
      row.innerHTML = `<div class="lbl">${label}${stored ? ' <span class="ovs-light green" style="display:inline-block;width:9px;height:9px;vertical-align:middle;margin-left:4px"></span>' : ""}</div>
        <div class="conn-in"><input type="password" placeholder="${stored ? "•••••• stored" : "paste API key"}" />
        <button class="folder-btn kv-save">Save</button>${stored ? '<button class="gh-rm kv-rm" title="Remove">×</button>' : ""}</div>`;
      const st2 = document.createElement("div"); st2.className = "hint"; row.appendChild(st2);
      const inp = row.querySelector("input");
      row.querySelector(".kv-save").addEventListener("click", async () => {
        const v = inp.value.trim(); if (!v) return;
        st2.textContent = "Saving…";
        try {
          await invoke("set_api_key", { provider: prov, key: v });
          inp.value = "";
          st2.textContent = "Validating…";
          try { await invoke("validate_api_key", { provider: prov }); st2.textContent = "Connected — key valid."; }
          catch (e) { st2.textContent = "Saved, but validation failed: " + e; }
          setTimeout(loadSettings, 700);
        } catch (e) { st2.textContent = "Error: " + e; }
      });
      const rm = row.querySelector(".kv-rm");
      if (rm) rm.addEventListener("click", async () => { try { await invoke("clear_api_key", { provider: prov }); } catch (e) {} loadSettings(); });
      return row;
    };
    conn.appendChild(keyRow("Anthropic API key", "anthropic", ks.anthropic));
    conn.appendChild(keyRow("OpenAI API key", "openai", ks.openai));
    const kn = document.createElement("div"); kn.className = "hint";
    kn.textContent = "Stored in Windows Credential Manager (encrypted) — never written to config or shown here. Used for API-console spend/validation.";
    conn.appendChild(kn);
  } catch (e) {}
  card.appendChild(conn);

  try {
    const d = await invoke("debug_info");
    // Folders
    const folders = document.createElement("div");
    folders.className = "diag";
    folders.innerHTML = "<b>Folders</b>";
    const mkBtn = (label, path, cls) => {
      const b = document.createElement("button");
      b.className = "folder-btn" + (cls ? " " + cls : "");
      b.textContent = label; b.title = path || "";
      b.addEventListener("click", () => path && invoke("open_folder", { path }));
      return b;
    };
    const row = document.createElement("div"); row.className = "folder-row";
    row.appendChild(mkBtn("Open .claude", d.claudeDir));
    row.appendChild(mkBtn("Open .codex", d.codexDir));
    folders.appendChild(row);
    try {
      const dirs = await invoke("get_project_dirs");
      if (dirs.length) {
        const pl = document.createElement("div"); pl.className = "proj-dirs";
        pl.innerHTML = `<div class="muted" style="margin:8px 0 4px">Project folders (${dirs.length})</div>`;
        const wrap = document.createElement("div"); wrap.className = "folder-row wrap";
        dirs.forEach((p) => wrap.appendChild(mkBtn(basenameJs(p) || p, p, "sm")));
        pl.appendChild(wrap); folders.appendChild(pl);
      }
    } catch (e) {}
    card.appendChild(folders);
    // Diagnostics
    const line = (label, ok, extra) => `<div><b>${label}:</b> <span class="${ok ? "ok" : "bad"}">${ok ? "found" : "missing"}</span> ${extra || ""}</div>`;
    const diag = document.createElement("div");
    diag.className = "diag";
    diag.innerHTML = "<b>Diagnostics</b>"
      + line("Claude logs", d.claudeEvents > 0, `${d.claudeEvents} events · ${d.claudeSessions || 0} sessions`)
      + (d.claudeRoots || []).map((r) => `<div>${escapeHtml(r)}</div>`).join("")
      + line("Codex logs", d.codexSessionsExists, `${d.codexEvents} events`)
      + `<div>${d.codexDir || ""}</div>`;
    const sd = d.sessions || {};
    const sroots = (sd.coworkRoots || []).map((r) => `<div>${escapeHtml(r.path)} — <span class="${r.exists ? "ok" : "bad"}">${r.exists ? r.jsonlFiles + " files" : "missing"}</span></div>`).join("");
    const srecent = (sd.recent || []).map((x) => `<div>· ${escapeHtml(x.client || "?")} · ${escapeHtml(shortModel(x.model))} · ${x.ageMin}m ago · ${escapeHtml(x.name || "")}</div>`).join("");
    diag.innerHTML += `<div style="margin-top:8px"><b>Sessions:</b> ${sd.total || 0} total <span class="muted">(Code ${sd.claudeCode || 0} · Cowork ${sd.cowork || 0} · Codex ${sd.codex || 0})</span></div>`
      + `<div class="muted" style="margin:3px 0 1px">Cowork roots:</div>${sroots || '<div class="muted">none</div>'}`
      + (srecent ? `<div class="muted" style="margin:5px 0 1px">Most recent:</div>${srecent}` : "");
    card.appendChild(diag);
  } catch (e) {}
  fitWindow();
}
function initProviderSwitch() {
  document.querySelectorAll(".prov[data-provider]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const prov = btn.dataset.provider;
      if (activeTab === "history") {
        if (prov === "overview") return; // Overview isn't offered on History
        histProvider = prov;
        document.querySelectorAll(".prov").forEach((b) => b.classList.toggle("is-active", b === btn));
        loadHistory();
      } else {
        currentProvider = prov;
        document.querySelectorAll(".prov").forEach((b) => b.classList.toggle("is-active", b === btn));
        render(currentProvider);
      }
    });
  });
}

function initWindowButtons() {
  const wire = (id, cmd) => {
    const el = document.getElementById(id);
    if (el) el.addEventListener("click", (e) => { e.stopPropagation(); if (hasTauri()) invoke(cmd); });
  };
  wire("tlClose", "win_close");
  wire("tlMin", "win_minimize");
  const full = document.getElementById("tlFull");
  if (full) full.addEventListener("click", (e) => {
    e.stopPropagation();
    window.__maxed = !window.__maxed;
    if (hasTauri()) invoke("win_toggle_fullscreen");
    if (!window.__maxed) fitWindow();
  });
}

window.addEventListener("DOMContentLoaded", () => {
  initTabs();
  initProviderSwitch();
  updateProviderSwitch("usage"); // start on Usage → 3-way switch visible
  initWindowButtons();
  loadAliases();
  // Re-play the slide-in each time the panel is shown/focused.
  window.addEventListener("focus", () => {
    const p = document.getElementById("panel");
    if (p) { p.classList.remove("slide"); void p.offsetWidth; p.classList.add("slide"); }
  });
  // Alternate limit rows between countdown and reset clock.
  setInterval(() => { showClock = !showClock; updateResets(); }, 6000);

  if (hasTauri()) {
    const pull = async () => {
      try {
        window.SNAPSHOT = await invoke("get_snapshot");
        window.__lastError = null;
      } catch (e) {
        window.__lastError = String(e && e.message ? e.message : e);
        console.error("get_snapshot failed:", e);
      }
      render(currentProvider);
    };
    pull();
    setInterval(pull, 4000);
    // The backend emits "snapshot" after every refresh (including right after a
    // settings/profile change) — listen so changes reflect promptly.
    try {
      const ev = window.__TAURI__.event;
      if (ev && ev.listen) ev.listen("snapshot", (msg) => {
        if (msg && msg.payload) { window.SNAPSHOT = msg.payload; window.__lastError = null; render(currentProvider); }
      });
    } catch (e) {}
  } else {
    render(currentProvider); // browser preview → demo data
  }
});
