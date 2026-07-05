// Offline algorithm cross-check: a faithful JS mirror of usage-core's window math
// and pricing, exercised with the SAME scenarios as the Rust tests. Lets us verify
// the trust-core logic instantly without the Rust toolchain / network.
//
// Run: node tools/algo-check/check.mjs

const MIN = 60 * 1000, HOUR = 60 * MIN, DAY = 24 * HOUR;
const FIVE_H = 5 * HOUR, SEVEN_D = 7 * DAY;

const ev = (ts, model, input, output, cr = 0, cw = 0) => ({ ts, model, input, output, cr, cw });
const tot = (t) => t.input + t.output + t.cr + t.cw;

function active5h(events, now) {
  const evs = [...events].sort((a, b) => a.ts - b.ts);
  const blocks = [];
  let lastTs = -Infinity;
  for (const e of evs) {
    const last = blocks[blocks.length - 1];
    const startNew = !last || (e.ts - last.start >= FIVE_H) || (e.ts - lastTs >= FIVE_H);
    if (startNew) blocks.push({ start: e.ts, t: { input: e.input, output: e.output, cr: e.cr, cw: e.cw }, msgs: 1 });
    else { last.t.input += e.input; last.t.output += e.output; last.t.cr += e.cr; last.t.cw += e.cw; last.msgs++; }
    lastTs = e.ts;
  }
  for (let i = blocks.length - 1; i >= 0; i--) {
    const b = blocks[i];
    if (now >= b.start && now < b.start + FIVE_H)
      return { start: b.start, end: b.start + FIVE_H, now, t: b.t, msgs: b.msgs };
  }
  return { start: now, end: now + FIVE_H, now, t: { input: 0, output: 0, cr: 0, cw: 0 }, msgs: 0 };
}

function window7d(events, now, reset = null) {
  let start, end;
  if (reset != null) {
    let r = reset;
    while (r <= now) r += SEVEN_D;
    while (r - SEVEN_D > now) r -= SEVEN_D;
    start = r - SEVEN_D; end = r;
  } else { start = now - SEVEN_D; end = now; }
  const t = { input: 0, output: 0, cr: 0, cw: 0 }; let msgs = 0;
  for (const e of events) if (e.ts >= start && e.ts < end) { t.input += e.input; t.output += e.output; t.cr += e.cr; t.cw += e.cw; msgs++; }
  return { start, end, now, t, msgs };
}

const remainingMs = (w) => Math.max(0, w.end - w.now);
function remainingLabel(w) {
  const s = Math.floor(remainingMs(w) / 1000), d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60);
  return d >= 1 ? `${d}d` : h >= 1 ? `${h}h` : `${m}m`;
}
const withBudget = (w, budget) => budget > 0 ? Math.min(1, tot(w.t) / budget) : null;

function priceFor(model) {
  const m = model.toLowerCase();
  if (m.includes("opus")) return { i: 15, o: 75, cr: 1.5, cw: 18.75 };
  if (m.includes("sonnet")) return { i: 3, o: 15, cr: 0.3, cw: 3.75 };
  if (m.includes("haiku")) return { i: 0.8, o: 4, cr: 0.08, cw: 1 };
  if (m.includes("gpt-5") || m.includes("codex")) return { i: 2.5, o: 10, cr: 0.25, cw: 2.5 };
  return { i: 3, o: 15, cr: 0.3, cw: 3.75 };
}
const eventCost = (e) => { const p = priceFor(e.model); return (e.input * p.i + e.output * p.o + e.cr * p.cr + e.cw * p.cw) / 1e6; };

// ---- assertions ----
let pass = 0, fail = 0;
const eq = (name, a, b) => { const ok = a === b; ok ? pass++ : fail++; if (!ok) console.log(`  FAIL ${name}: got ${a}, want ${b}`); };
const approx = (name, a, b) => { const ok = Math.abs(a - b) < 1e-6; ok ? pass++ : fail++; if (!ok) console.log(`  FAIL ${name}: got ${a}, want ${b}`); };

// 5h single block
{
  const base = 1_000_000_000_000;
  const events = [ev(base, "claude-sonnet-4", 100, 50, 10, 5), ev(base + 30 * MIN, "claude-opus-4", 200, 80, 20, 0)];
  const w = active5h(events, base + HOUR);
  eq("5h.start", w.start, base); eq("5h.end", w.end, base + FIVE_H); eq("5h.msgs", w.msgs, 2);
  eq("5h.input", w.t.input, 300); eq("5h.output", w.t.output, 130); eq("5h.total", tot(w.t), 465);
  eq("5h.remaining", remainingMs(w), 4 * HOUR); eq("5h.label", remainingLabel(w), "4h");
}
// 5h new block after gap
{
  const base = 1_700_000_000_000;
  const events = [ev(base, "sonnet", 100, 0), ev(base + 6 * HOUR, "sonnet", 999, 0), ev(base + 6 * HOUR + 10 * MIN, "opus", 1, 2, 3, 4)];
  const w = active5h(events, base + 6 * HOUR + 20 * MIN);
  eq("gap.start", w.start, base + 6 * HOUR); eq("gap.msgs", w.msgs, 2); eq("gap.input", w.t.input, 1000); eq("gap.output", w.t.output, 2);
}
// 5h boundary exactly 5h
{
  const base = 1_700_000_000_000;
  const events = [ev(base, "sonnet", 10, 0), ev(base + FIVE_H, "sonnet", 20, 0)];
  const w = active5h(events, base + FIVE_H + MIN);
  eq("bound.start", w.start, base + FIVE_H); eq("bound.input", w.t.input, 20); eq("bound.msgs", w.msgs, 1);
}
// 5h idle
{
  const base = 1_700_000_000_000;
  const w = active5h([ev(base, "sonnet", 10, 0)], base + 10 * HOUR);
  eq("idle.total", tot(w.t), 0); eq("idle.msgs", w.msgs, 0); eq("idle.start", w.start, base + 10 * HOUR);
}
// 7d trailing
{
  const now = 2_000_000_000_000;
  const events = [ev(now - 8 * DAY, "sonnet", 500, 0), ev(now - 3 * DAY, "sonnet", 100, 10), ev(now - HOUR, "opus", 1, 2, 3, 4)];
  const w = window7d(events, now, null);
  eq("7d.msgs", w.msgs, 2); eq("7d.input", w.t.input, 101); eq("7d.output", w.t.output, 12);
}
// 7d with reset
{
  const now = 2_000_000_000_000, reset = now + 3 * DAY;
  const events = [ev(now - 2 * DAY, "sonnet", 10, 0), ev(now - 5 * DAY, "sonnet", 20, 0)];
  const w = window7d(events, now, reset);
  eq("7dr.end", w.end, reset); eq("7dr.remaining", remainingMs(w), 3 * DAY); eq("7dr.label", remainingLabel(w), "3d");
  // window is [reset-7d, reset) = [now-4d, now+3d): the now-5d event is excluded
  eq("7dr.msgs", w.msgs, 1); eq("7dr.input", w.t.input, 10);
}
// weekly reset in past normalizes forward
{
  const now = 2_000_000_000_000;
  const w = window7d([], now, now - 2 * DAY);
  eq("7dpast.remaining", remainingMs(w), 5 * DAY);
}
// budget
{
  const base = 1_700_000_000_000;
  const w = active5h([ev(base, "sonnet", 300, 100)], base + MIN);
  approx("budget.util", withBudget(w, 1000), 0.4);
  approx("budget.clamp", withBudget(w, 100), 1.0);
}
// pricing
{
  approx("price.opus.in", eventCost(ev(0, "claude-opus-4-6", 1e6, 0)), 15.0);
  approx("price.sonnet.out", eventCost(ev(0, "claude-sonnet-4-6", 0, 1e6)), 15.0);
  approx("price.haiku.cr", eventCost(ev(0, "claude-haiku-4-5", 0, 0, 1e6, 0)), 0.08);
  approx("price.total", eventCost(ev(0, "opus", 1e6, 0)) + eventCost(ev(0, "sonnet", 0, 1e6)), 30.0);
}

console.log("\nalgo-check: " + pass + " passed, " + fail + " failed");
process.exit(fail === 0 ? 0 : 1);
