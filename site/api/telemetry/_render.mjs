// generated from _render.mts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
/** The smallest group of installs a number is shown for. */
export const K = 10;
const esc = (s) => s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c);
/** A number: a fraction keeps one decimal under 100 and none from 100 up (designer m_58387). */
const fmt = (n) => n.toLocaleString("en-US", { maximumFractionDigits: Number.isInteger(n) || Math.abs(n) >= 100 ? 0 : 1 });
/** A version or a day: mono, like the numbers. */
const id = (s) => `<span class="id">${esc(s)}</span>`;
/** Is a number from 1 to K-1 installs (hidden)? */
export const hidden = (installs) => installs > 0 && installs < K;
/** A cell as shown: its number, or "< 10". */
export function shown(c, unit = "") {
    if (hidden(c.installs))
        return `<span class="few" title="from fewer than ${K} installs">&lt; ${K}</span>`;
    return fmt(c.value) + unit;
}
/** Rows of fewer than K installs folded into one "others" row. */
function folded(rows, other) {
    const big = rows.filter((r) => r.installs >= K);
    const rest = rows.filter((r) => r.installs < K);
    return rest.length ? [...big, other(rest)] : big;
}
const sum = (xs) => xs.reduce((a, b) => a + b, 0);
function tile(label, value, sub) {
    return `<div class="tile"><div class="label">${label}</div><div class="value">${value}</div><div class="sub">${sub}</div></div>`;
}
/** Active installs per day, one bar a day; a hidden day is a faint tick. */
function chart(s) {
    const byDay = new Map(s.days.map((d) => [d.day, d.active]));
    const days = [];
    for (let t = Date.parse(`${s.first}T00:00:00Z`); t <= Date.parse(`${s.last}T00:00:00Z`); t += 86_400_000) {
        days.push(new Date(t).toISOString().slice(0, 10));
    }
    const top = Math.max(K, ...days.map((d) => byDay.get(d)).filter((c) => !!c && !hidden(c.installs)).map((c) => c.value));
    const w = 720, h = 140, gap = 4, bw = (w - gap * (days.length - 1)) / days.length;
    const bars = days.map((d, i) => {
        const c = byDay.get(d) ?? { value: 0, installs: 0 };
        const x = (i * (bw + gap)).toFixed(1);
        if (hidden(c.installs))
            return `<rect class="few" x="${x}" y="${h - 3}" width="${bw.toFixed(1)}" height="3"><title>${d}: &lt; ${K}</title></rect>`;
        const bh = Math.max(c.value > 0 ? 2 : 0, (c.value / top) * (h - 16));
        return `<rect x="${x}" y="${(h - bh).toFixed(1)}" width="${bw.toFixed(1)}" height="${bh.toFixed(1)}"><title>${d}: ${fmt(c.value)}</title></rect>`;
    });
    return `<svg class="chart" viewBox="0 0 ${w} ${h + 18}" role="img" aria-label="active installs per day">
    <line class="axis" x1="0" y1="${h + 0.5}" x2="${w}" y2="${h + 0.5}"/>
    ${bars.join("\n    ")}
    <text x="0" y="${h + 15}">${esc(s.first.slice(5))}</text><text x="${w}" y="${h + 15}" text-anchor="end">${esc(s.last.slice(5))}</text>
    <text x="0" y="10">${fmt(top)}</text><text x="0" y="${h - 4}">0</text>
  </svg>`;
}
function table(head, rows, numeric) {
    const th = head.map((t, i) => `<th${numeric[i] ? ' class="n"' : ""}>${t}</th>`).join("");
    const body = rows.length
        ? rows.map((r) => `<tr>${r.map((c, i) => `<td${numeric[i] ? ' class="n"' : ""}>${c}</td>`).join("")}</tr>`).join("\n")
        : `<tr><td colspan="${head.length}" class="empty">nothing yet</td></tr>`;
    return `<table><thead><tr>${th}</tr></thead><tbody>${body}</tbody></table>`;
}
const pct = (part, whole) => (whole ? `${Math.round((100 * part) / whole)}%` : "–");
/** The dashboard page. */
export function renderDashboard(s) {
    const yesterday = s.days.find((d) => d.day === s.last)?.active ?? { value: 0, installs: 0 };
    const perFocused = s.focusedHours7.value > 0
        ? { value: Math.round((10 * s.agentHours7.value) / s.focusedHours7.value) / 10, installs: Math.min(s.agentHours7.installs, s.focusedHours7.installs) }
        : { value: 0, installs: 0 };
    const versions = folded(s.versions, (rest) => ({ version: `${rest.length} other${rest.length > 1 ? "s" : ""}`, channel: "", installs: sum(rest.map((r) => r.installs)) }));
    const surfaces = folded(s.surfaces, (rest) => ({ surface: "others", installs: sum(rest.map((r) => r.installs)) }));
    const quality = folded(s.quality, (rest) => ({
        version: `${rest.length} other${rest.length > 1 ? "s" : ""}`,
        installs: sum(rest.map((r) => r.installs)),
        installDays: sum(rest.map((r) => r.installDays)),
        crashFreeDays: sum(rest.map((r) => r.crashFreeDays)),
        crashes: sum(rest.map((r) => r.crashes)),
    }));
    const n = (installs) => shown({ value: installs, installs });
    const days = [...s.days].reverse();
    return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex, nofollow">
<title>bise · usage</title>
<style>
  :root { --paper:#f2ede2; --ink:#1d1a17; --dim:#5a5349; --kick:#8a8174; --pen:#c8264a; --rule:#d9d1c2; --chip:#e8e1d3; color-scheme: light dark; }
  @media (prefers-color-scheme: dark) { :root { --paper:#171513; --ink:#ece6da; --dim:#b8b0a3; --kick:#8d8579; --pen:#f4a6b0; --rule:#34302b; --chip:#25211e; } }
  * { box-sizing: border-box; }
  body { margin: 0; background: var(--paper); color: var(--ink); font: 16px/1.5 Georgia, "Times New Roman", serif; }
  main { max-width: 820px; margin: 0 auto; padding: 40px 24px 64px; }
  .kick, .label, th, .sub, .note, svg text { font-family: ui-monospace, Menlo, monospace; }
  .kick { color: var(--kick); font-size: 12px; letter-spacing: .04em; margin: 0 0 8px; }
  h1 { font-weight: 400; font-size: 32px; line-height: 1.2; margin: 0 0 8px; }
  h2 { font-weight: 400; font-size: 20px; margin: 40px 0 12px; }
  .note { color: var(--dim); font-size: 12px; margin: 0; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); gap: 12px; margin: 32px 0 0; }
  .tile { border-top: 1px solid var(--rule); padding: 12px 0 0; }
  .label { color: var(--kick); font-size: 12px; }
  .value { font: 28px/1.3 ui-monospace, Menlo, monospace; font-variant-numeric: tabular-nums; margin: 4px 0; }
  .sub { color: var(--dim); font-size: 11px; }
  .chart { width: 100%; height: auto; display: block; }
  .chart rect { fill: var(--ink); }
  .chart rect.few { fill: var(--rule); }
  .chart .axis { stroke: var(--rule); }
  .chart text { fill: var(--kick); font-size: 11px; }
  table { width: 100%; border-collapse: collapse; font-size: 14px; }
  th { color: var(--kick); font-size: 11px; font-weight: 400; text-align: left; border-bottom: 1px solid var(--rule); padding: 6px 8px 6px 0; }
  td { border-bottom: 1px solid var(--chip); padding: 6px 8px 6px 0; }
  .n { text-align: right; font-family: ui-monospace, Menlo, monospace; font-variant-numeric: tabular-nums; }
  .id { font-family: ui-monospace, Menlo, monospace; font-weight: 400; font-size: 13px; }
  .few { color: var(--kick); }
  .empty { color: var(--kick); }
  .cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 0 32px; }
</style>
</head>
<body>
<main>
<p class="kick">bise · usage · private</p>
<h1>how bise is used</h1>
<p class="note">the ${s.days.length ? "" : "no "}packets of the last 30 closed days, ${esc(s.first)} to ${esc(s.last)} (UTC). a number from fewer than ${K} installs shows &lt; ${K}. active = wrote to an agent or answered a card that day.</p>

<div class="tiles">
  ${tile("active yesterday", shown(yesterday), esc(s.last))}
  ${tile("active, 7 days", shown(s.wau), "installs")}
  ${tile("active, 28 days", shown(s.mau), "installs")}
  ${tile("agent-hours, 7 days", shown(s.agentHours7), "all installs")}
  ${tile("agent-hours per your hour", shown(perFocused), "7 days, focused hours")}
</div>

<h2>active installs per day</h2>
${chart(s)}

<h2>per day</h2>
${table(["day", "sending", "active", "agents started", "by you", "by main", "by agents", "agent-hours", "crashes"], days.map((d) => [id(d.day), shown(d.reporting), shown(d.active), shown(d.agents), shown(d.byUser), shown(d.byMain), shown(d.byAgent), shown(d.agentHours), shown(d.crashes)]), [false, true, true, true, true, true, true, true, true])}

<div class="cols">
<section>
<h2>versions in use, 7 days</h2>
${table(["version", "channel", "installs"], versions.map((v) => [v.channel ? id(v.version) : esc(v.version), esc(v.channel), n(v.installs)]), [false, false, true])}
</section>
<section>
<h2>surfaces, 7 days</h2>
${table(["surface", "installs"], surfaces.map((v) => [esc(v.surface), n(v.installs)]), [false, true])}
</section>
</div>

<h2>quality per version, 7 days</h2>
${table(["version", "installs", "install-days", "without a crash", "crashes"], quality.map((q) => hidden(q.installs)
        ? [esc(q.version), n(q.installs), n(q.installs), n(q.installs), n(q.installs)]
        : [id(q.version), n(q.installs), fmt(q.installDays), pct(q.crashFreeDays, q.installDays), fmt(q.crashes)]), [false, true, true, true, true])}

<div class="cols">
<section>
<h2>the app, a day (median minutes), 7 days</h2>
${table(["open", "focused", "engaged"], [[shown(s.attention.open, " min"), shown(s.attention.focused, " min"), shown(s.attention.engaged, " min")]], [true, true, true])}
</section>
<section>
<h2>setup, 28 days (installs)</h2>
${table(["started", "done", "failed: auth", "network", "other"], [[shown(s.setup.started), shown(s.setup.done), shown(s.setup.failedAuth), shown(s.setup.failedNetwork), shown(s.setup.failedOther)]], [true, true, true, true, true])}
</section>
</div>
</main>
</body>
</html>
`;
}
/** A short page for when the dashboard can't show numbers (no store yet). */
export function renderNote(title, text) {
    return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="robots" content="noindex, nofollow"><title>bise · usage</title>
<style>body{margin:0;padding:48px 24px;font:16px/1.5 Georgia,serif;background:#f2ede2;color:#1d1a17}main{max-width:640px;margin:0 auto}</style></head>
<body><main><h1 style="font-weight:400">${esc(title)}</h1><p>${esc(text)}</p></main></body></html>
`;
}
