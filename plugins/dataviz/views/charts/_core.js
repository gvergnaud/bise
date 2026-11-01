// dataviz's renderer core: scales, ticks, time ticks, formatting and the colors.
// Every renderer is DV.kinds[<kind>] = { draw(chart, opts) -> drawing }, a pure
// function of the chart and the box that returns SVG as a string:
//   opts:    { width, fullscreen, hidden: Set<series name>, locale, utc }
//   drawing: { height, svg, tips: [html], hover?(x, y) -> { tip, guide?, dots?, band? } | null,
//              legend?: [{ name, color, off }] }
// Elements with data-tip="<i>" show tips[i] on hover (and the others dim);
// hover() covers the rest (the nearest point of a line, a bucket's column).
// Colors and sizes are the view kit's CSS variables in style attributes
// (var(--k-…), plugins/.view-kit/kit.css), so a theme change is a repaint and
// the export (kit.saveSvg) resolves them. Text is --k-t-xs (11.5 px, the kit's floor).
(function (root) {
  const DV = (root.DV = root.DV || {});
  DV.kinds = DV.kinds || {};

  DV.esc = (t) => String(t).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  DV.color = (i) => `var(--k-series-${(i % 8) + 1})`;
  DV.px = (n) => Math.round(n * 10) / 10;
  DV.FONT = 11.5; // --k-t-xs

  // Text width, close enough for layout (the UI font: ~0.56 em per char on average).
  DV.textW = (s, size = DV.FONT) => String(s).length * size * 0.56;
  // Whole or nothing: the first of the texts that fits in maxW, else "" (the tip has it).
  DV.fit = (texts, maxW, size = DV.FONT) => texts.find((s) => s && DV.textW(s, size) <= maxW) || "";
  DV.clip = (s, maxW, size = DV.FONT) => {
    s = String(s);
    const n = Math.floor(maxW / (size * 0.56));
    return s.length <= n ? s : n <= 1 ? "" : s.slice(0, n - 1) + "…";
  };

  // ---- numbers
  const UNIT_BEFORE = new Set(["$", "£"]);
  DV.num = (v, unit, opts = {}) => {
    const loc = opts.locale || undefined;
    const a = Math.abs(v);
    const nf = opts.compact && a >= 10000
      ? new Intl.NumberFormat(loc, { notation: "compact", maximumFractionDigits: 1 })
      : new Intl.NumberFormat(loc, { maximumFractionDigits: a >= 100 ? 0 : a >= 1 ? (opts.compact ? 1 : 2) : 3 });
    const s = nf.format(v);
    if (!unit) return s;
    if (UNIT_BEFORE.has(unit)) return v < 0 ? "-" + unit + s.replace(/^[-−]/, "") : unit + s;
    if (unit === "%") return s + "%";
    return s + " " + unit;
  };
  DV.pct = (v, total) => (total ? (Math.round((v / total) * 1000) / 10).toLocaleString() + "%" : "–");

  // ---- linear scale and nice ticks
  DV.step = (lo, hi, n) => {
    const raw = (hi - lo) / Math.max(1, n);
    if (!(raw > 0)) return 1;
    const mag = Math.pow(10, Math.floor(Math.log10(raw)));
    const e = raw / mag;
    return (e >= 7.07 ? 10 : e >= 3.16 ? 5 : e >= 1.41 ? 2 : 1) * mag;
  };
  // A domain [lo, hi] widened to whole steps, and its ticks.
  DV.nice = (lo, hi, n) => {
    if (lo === hi) { if (lo === 0) hi = 1; else if (lo > 0) lo = 0; else hi = 0; }
    const s = DV.step(lo, hi, n);
    const a = Math.floor(lo / s + 1e-9) * s, b = Math.ceil(hi / s - 1e-9) * s;
    const ticks = [];
    for (let v = a; v <= b + s / 2; v += s) ticks.push(+v.toPrecision(12));
    return { lo: a, hi: b, ticks };
  };
  DV.scale = (d0, d1, r0, r1) => (v) => (d1 === d0 ? (r0 + r1) / 2 : r0 + ((v - d0) / (d1 - d0)) * (r1 - r0));

  // ---- time
  // A calendar on UTC (a chart whose times said no zone: shown as written) or
  // on the viewer's own zone.
  const cal = (utc) => utc
    ? { parts: (t) => { const d = new Date(t); return [d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate(), d.getUTCHours(), d.getUTCMinutes(), d.getUTCDay()]; },
        make: (y, mo, d, h, mi) => Date.UTC(y, mo, d, h, mi) }
    : { parts: (t) => { const d = new Date(t); return [d.getFullYear(), d.getMonth(), d.getDate(), d.getHours(), d.getMinutes(), d.getDay()]; },
        make: (y, mo, d, h, mi) => new Date(y, mo, d, h, mi).getTime() };

  const MIN = 60000, HOUR = 3600000, DAY = 86400000;
  // [unit, step, approx ms]
  const STEPS = [
    ["minute", 1, MIN], ["minute", 5, 5 * MIN], ["minute", 15, 15 * MIN], ["minute", 30, 30 * MIN],
    ["hour", 1, HOUR], ["hour", 3, 3 * HOUR], ["hour", 6, 6 * HOUR], ["hour", 12, 12 * HOUR],
    ["day", 1, DAY], ["day", 2, 2 * DAY], ["week", 1, 7 * DAY], ["week", 2, 14 * DAY],
    ["month", 1, 30 * DAY], ["month", 3, 91 * DAY], ["month", 6, 182 * DAY], ["year", 1, 365 * DAY],
  ];
  // Round t down to a whole `unit` (weeks start on Monday).
  DV.floorTime = (t, unit, step, utc) => {
    const c = cal(utc), [y, mo, d, h, mi, wd] = c.parts(t);
    switch (unit) {
      case "minute": return c.make(y, mo, d, h, Math.floor(mi / step) * step);
      case "hour": return c.make(y, mo, d, Math.floor(h / step) * step, 0);
      case "day": return c.make(y, mo, d, 0, 0);
      case "week": return c.make(y, mo, d - ((wd + 6) % 7), 0, 0);
      case "month": return c.make(y, Math.floor(mo / step) * step, 1, 0, 0);
      case "quarter": return c.make(y, Math.floor(mo / 3) * 3, 1, 0, 0);
      default: return c.make(Math.floor(y / step) * step, 0, 1, 0, 0);
    }
  };
  DV.addTime = (t, unit, n, utc) => {
    const c = cal(utc), [y, mo, d, h, mi] = c.parts(t);
    switch (unit) {
      case "minute": return c.make(y, mo, d, h, mi + n);
      case "hour": return c.make(y, mo, d, h + n, mi);
      case "day": return c.make(y, mo, d + n, h, mi);
      case "week": return c.make(y, mo, d + 7 * n, h, mi);
      case "month": return c.make(y, mo + n, d, h, mi);
      case "quarter": return c.make(y, mo + 3 * n, d, h, mi);
      default: return c.make(y + n, mo, d, h, mi);
    }
  };
  // About n ticks over [lo, hi]: [{t, label}], labels in the unit of the step.
  DV.timeTicks = (lo, hi, n, opts) => {
    const span = Math.max(1, hi - lo);
    let pick = STEPS.find((s) => span / s[2] <= n);
    if (!pick) { const years = DV.step(0, span / (365 * DAY), n); pick = ["year", Math.max(1, Math.round(years)), years * 365 * DAY]; }
    const [unit, step] = pick;
    const out = [];
    let t = DV.floorTime(lo, unit, step, opts.utc);
    if (t < lo) t = DV.addTime(t, unit, step, opts.utc);
    for (let guard = 0; t <= hi && guard < 200; guard++) {
      out.push(t);
      t = DV.addTime(t, unit, step, opts.utc);
    }
    return out.map((t, i) => ({ t, label: DV.timeLabel(t, unit, opts, i === 0 ? null : out[i - 1]) }));
  };
  // A tick's label: short, and the bigger unit only where it changes.
  DV.timeLabel = (t, unit, opts, prev) => {
    const tz = opts.utc ? "UTC" : undefined, loc = opts.locale || undefined;
    const f = (o) => new Intl.DateTimeFormat(loc, { timeZone: tz, ...o }).format(t);
    const c = cal(opts.utc), p = c.parts(t), q = prev == null ? null : c.parts(prev);
    if (unit === "year") return f({ year: "numeric" });
    if (unit === "month" || unit === "quarter") return !q || q[0] !== p[0] ? f({ month: "short", year: "numeric" }) : f({ month: "short" });
    if (unit === "day" || unit === "week") return f({ month: "short", day: "numeric" });
    if (!q || q[2] !== p[2]) return f({ month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
    return f({ hour: "2-digit", minute: "2-digit" });
  };
  // A point's time in a tooltip: as precise as the data.
  DV.timeTip = (t, opts, grain) => {
    const tz = opts.utc ? "UTC" : undefined, loc = opts.locale || undefined;
    const o = grain === "year" ? { year: "numeric" }
      : grain === "month" || grain === "quarter" ? { month: "long", year: "numeric" }
      : grain === "day" || grain === "week" || !opts.withTime ? { weekday: "short", day: "numeric", month: "short", year: "numeric" }
      : { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" };
    const s = new Intl.DateTimeFormat(loc, { timeZone: tz, ...o }).format(t);
    if (grain === "week") return "week of " + s;
    if (grain === "quarter") return "Q" + (Math.floor(cal(opts.utc).parts(t)[1] / 3) + 1) + " " + cal(opts.utc).parts(t)[0];
    return s;
  };

  // ---- shared bits of drawings
  DV.tipRows = (head, rows) =>
    `<div class="tip-h">${DV.esc(head)}</div>` +
    rows.map((r) => `<div class="tip-r">${r.color ? `<i style="background:${r.color}"></i>` : ""}<span>${DV.esc(r.name)}</span><b>${DV.esc(r.value)}</b></div>`).join("");

  // The value axis: grid lines and labels on the left; returns the left margin used.
  DV.yAxis = (ticks, y, x0, x1, opts) => {
    let s = "";
    for (const v of ticks) {
      const yy = DV.px(y(v));
      s += `<line x1="${x0}" x2="${x1}" y1="${yy}" y2="${yy}" style="stroke:var(--k-${v === 0 ? "line-strong" : "line"});stroke-width:1"/>`;
      s += `<text x="${x0 - 8}" y="${yy}" dy="0.32em" text-anchor="end" style="fill:var(--k-text-3)">${DV.esc(DV.num(v, opts.unit, { compact: true, locale: opts.locale }))}</text>`;
    }
    return s;
  };
  DV.yLabelW = (ticks, opts) => Math.max(...ticks.map((v) => DV.textW(DV.num(v, opts.unit, { compact: true, locale: opts.locale })))) + 12;
})(typeof window !== "undefined" ? window : globalThis);
