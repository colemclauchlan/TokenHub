// AI Usage Bar — panel renderer. Renders a Snapshot into the UI and draws the
// sparkline + 14-day combo chart on canvas. At runtime the snapshot comes from the
// Rust backend (Tauri invoke); in the browser/screenshot harness it uses window.MOCK_SNAPSHOT.

const COLORS = {
  cacheRead: "#4aa8c9", cacheWrite: "#d0774a", input: "#6f93b4", output: "#d9a441",
  orange: "#d0774a", blue: "#4aa8c9",
};

let currentProvider = "claude";

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
  const ctx = cv.getContext("2d");
  ctx.scale(dpr, dpr);
  return { ctx, w, h: cssHeight };
}

function drawSparkline(cv, data) {
  const { ctx, w, h } = setupCanvas(cv, 90);
  ctx.clearRect(0, 0, w, h);
  const max = Math.max(...data, 1), pad = 6;
  const stepX = w / (data.length - 1);
  const y = (v) => h - pad - (v / max) * (h - pad * 2);
  // area
  const grad = ctx.createLinearGradient(0, 0, 0, h);
  grad.addColorStop(0, "rgba(217,164,65,.28)");
  grad.addColorStop(1, "rgba(217,164,65,0)");
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
  ctx.strokeStyle = "#d9a441";
  ctx.lineWidth = 1.8;
  ctx.lineJoin = "round";
  ctx.stroke();
}

function drawTrend(cv, trend) {
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
    ctx.fillStyle = "rgba(208,119,74,.85)";
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
  ctx.strokeStyle = "#4aa8c9";
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
  const snap = (window.SNAPSHOT || window.MOCK_SNAPSHOT)[providerKey];
  if (!snap) return;

  // limits
  for (const key of ["fiveHour", "sevenDay"]) {
    const row = document.querySelector(`.limit-row[data-k="${key}"]`);
    const lim = snap.limits[key];
    renderSegments(row.querySelector(".seg"), lim.pct);
    row.querySelector(".pct").textContent = lim.pct + "%";
    row.querySelector(".reset").textContent = lim.resetLabel;
  }
  const anySrc = snap.limits.fiveHour.source;
  $("#limitsSrc").textContent = anySrc === "providerApi"
    ? "live · matches Claude Code counter" : "estimated from local logs";

  // hero
  $("#provLogo").textContent = snap.logo;
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
  drawSparkline($("#sparkline"), snap.sparkline);

  // trend
  drawTrend($("#trend"), snap.trend);
  $("#pillAvg").innerHTML = "💬 " + snap.trendPills.avgPerDay;
  $("#pillTotalMsgs").textContent = snap.trendPills.totalMsgs;
  $("#pillTotalTokens").textContent = snap.trendPills.totalTokens;

  // models
  $("#modelsTotal").textContent = fmtCompact(snap.models.total);
  const stackEl = $("#modelsStack");
  const total = snap.models.list.reduce((s, m) => s + m.in + m.out, 0) || 1;
  stackEl.innerHTML = snap.models.list.map((m) =>
    `<span style="width:${((m.in + m.out) / total) * 100}%;background:${m.color}"></span>`).join("");
  $("#modelList").innerHTML = snap.models.list.map((m) =>
    `<div class="model-row"><span class="mdot" style="background:${m.color}"></span>
     <span class="mname">${m.name}</span>
     <span class="mnums"><b>${fmtCompact(m.in)}</b> in &nbsp; <b>${fmtCompact(m.out)}</b> out</span></div>`
  ).join("");
}

/* ---------- interactions ---------- */
function initTabs() {
  document.querySelectorAll(".tab[data-tab]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const tab = btn.dataset.tab;
      document.querySelectorAll(".tab[data-tab]").forEach((b) => b.classList.toggle("is-active", b === btn));
      ["usage", "processes", "git", "settings"].forEach((t) =>
        $("#tab-" + t).classList.toggle("hidden", t !== tab));
      loadTab(tab);
    });
  });
}

const hasTauri = () => !!window.__TAURI__?.core?.invoke;
const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args);

function loadTab(tab) {
  if (tab === "processes") loadProcesses();
  else if (tab === "git") loadGit();
  else if (tab === "settings") loadSettings();
}

async function loadProcesses() {
  const list = $("#procList"), empty = $("#procEmpty");
  if (!hasTauri()) { list.innerHTML = '<section class="card"><div class="placeholder">Process list is available in the installed app.</div></section>'; return; }
  try {
    const groups = await invoke("get_processes");
    empty.classList.toggle("hidden", groups.length > 0);
    list.innerHTML = groups.map((g) => `
      <section class="card proc-group">
        <div class="gh"><span class="host">${g.host}</span><span class="count">${g.procs.length} process${g.procs.length === 1 ? "" : "es"}</span></div>
        ${g.procs.map((p) => `
          <div class="proc-row"><span class="pdot"></span>
            <span><span class="tool">${p.tool}</span> <span class="pname">${p.name}</span></span>
            <span class="pmeta">${p.memMB} MB · ${p.runtime}</span>
            <button class="kill" data-pid="${p.pid}">Kill</button>
          </div>`).join("")}
      </section>`).join("");
    list.querySelectorAll(".kill").forEach((b) => b.addEventListener("click", async () => {
      await invoke("kill_process", { pid: parseInt(b.dataset.pid, 10) });
      loadProcesses();
    }));
  } catch (e) { list.innerHTML = '<section class="card"><div class="placeholder">Could not read processes.</div></section>'; }
}

async function loadGit() {
  const list = $("#gitList");
  if (!hasTauri()) { list.innerHTML = '<section class="card"><div class="placeholder">GitHub PRs are available in the installed app (requires the gh CLI).</div></section>'; return; }
  try {
    const g = await invoke("get_git");
    if (!g.available) { list.innerHTML = `<section class="card"><div class="placeholder">${g.message || "gh CLI not available."}</div></section>`; return; }
    if (!g.prs.length) { list.innerHTML = '<section class="card"><div class="placeholder">No open pull requests. 🎉</div></section>'; return; }
    list.innerHTML = '<section class="card">' + g.prs.map((p) => `
      <div class="pr-row" data-url="${p.url}">
        <span class="pr-ci ci-${p.ci}"></span>
        <span class="pr-main"><div class="pr-title">${p.title}</div><div class="pr-repo">${p.repo} #${p.number}</div></span>
        <span class="pr-badge">${p.review === "APPROVED" ? "✓ approved" : p.mergeable === "MERGEABLE" ? "mergeable" : p.review.toLowerCase()}</span>
      </div>`).join("") + "</section>";
    list.querySelectorAll(".pr-row").forEach((r) => r.addEventListener("click", () => window.__TAURI__?.opener?.openUrl?.(r.dataset.url)));
  } catch (e) { list.innerHTML = '<section class="card"><div class="placeholder">Could not query GitHub.</div></section>'; }
}

const settingToggles = [
  ["useProviderApi", "Match Claude Code counter", "Read local OAuth to call the provider usage API"],
  ["minibarEnabled", "Show docked mini-bar", "Slim always-on-top 5h/7d strip"],
  ["trackClaude", "Track Claude Code", ""],
  ["trackCodex", "Track Codex", ""],
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
    <div class="set-row"><div class="lbl">Hotkey</div><input type="text" data-k="hotkey" value="${s.hotkey}" /></div>
    <div class="set-row"><div class="lbl">Refresh (seconds)</div><input type="number" min="2" data-k="refreshSecs" value="${s.refreshSecs}" /></div>`;
  const push = () => invoke("set_settings", { settings: s });
  card.querySelectorAll(".toggle").forEach((b) => b.addEventListener("click", () => { const k = b.dataset.k; s[k] = !s[k]; b.classList.toggle("on", s[k]); push(); }));
  card.querySelectorAll("select,input").forEach((el) => el.addEventListener("change", () => {
    const k = el.dataset.k; s[k] = el.type === "number" ? parseInt(el.value, 10) : el.value; push();
  }));
}
function initProviderSwitch() {
  document.querySelectorAll(".prov[data-provider]").forEach((btn) => {
    btn.addEventListener("click", () => {
      currentProvider = btn.dataset.provider;
      document.querySelectorAll(".prov").forEach((b) => b.classList.toggle("is-active", b === btn));
      render(currentProvider);
    });
  });
}

window.addEventListener("DOMContentLoaded", () => {
  initTabs();
  initProviderSwitch();
  render(currentProvider);
  // If running under Tauri, pull the real snapshot and refresh on a timer.
  if (window.__TAURI__?.core?.invoke) {
    const pull = async () => {
      try {
        window.SNAPSHOT = await window.__TAURI__.core.invoke("get_snapshot");
        render(currentProvider);
      } catch (e) { /* keep mock */ }
    };
    pull();
    setInterval(pull, 5000);
  }
});
