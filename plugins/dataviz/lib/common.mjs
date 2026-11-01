// The parts every chart kind shares: the common fields of a chart, input
// checks that say what is wrong in words an agent can fix, time parsing,
// and number formatting for the text the model reads. Pure, no I/O.

export class InputError extends Error {}

export const fail = (msg) => { throw new InputError(msg); };

// The fields every chart has (merged into each kind's input schema).
export const COMMON_PROPS = {
  title: { type: "string", description: "What the chart shows, short (shown above it)." },
  subtitle: { type: "string", description: "Optional second line: the period, the scope, the filter." },
  unit: { type: "string", description: "Optional unit of the values: \"€\", \"$\", \"%\", \"ms\", \"GB\", \"commits\"... Currency signs $ £ go before the number, the rest after." },
  note: { type: "string", description: "Optional footnote under the chart: the source, a caveat." },
};

export const LIMITS = { title: 120, label: 80, note: 240 };

export function text(v, field, max, required = false) {
  if (v === undefined || v === null || v === "") {
    if (required) fail(`\`${field}\` is required`);
    return undefined;
  }
  if (typeof v !== "string" && typeof v !== "number") fail(`\`${field}\` must be a string`);
  const s = String(v).replace(/\s+/g, " ").trim();
  return s.length > max ? s.slice(0, max - 1) + "…" : s;
}

export function common(args) {
  if (!args || typeof args !== "object") fail("the arguments must be an object");
  const out = { title: text(args.title, "title", LIMITS.title, true) };
  const sub = text(args.subtitle, "subtitle", LIMITS.title);
  if (sub) out.subtitle = sub;
  const unit = text(args.unit, "unit", 16);
  if (unit) out.unit = unit;
  const note = text(args.note, "note", LIMITS.note);
  if (note) out.note = note;
  return out;
}

export function list(v, field, { min = 1, max }) {
  if (!Array.isArray(v)) fail(`\`${field}\` must be an array`);
  if (v.length < min) fail(`\`${field}\` needs at least ${min} item${min > 1 ? "s" : ""}`);
  if (v.length > max) fail(`\`${field}\` has ${v.length} items, ${max} at most: aggregate first`);
  return v;
}

export function num(v, field, { min } = {}) {
  const n = typeof v === "string" && v.trim() !== "" ? Number(v) : v;
  if (typeof n !== "number" || !Number.isFinite(n)) fail(`\`${field}\` must be a finite number, got ${JSON.stringify(v)}`);
  if (min !== undefined && n < min) fail(`\`${field}\` must be ${min} or more, got ${n}`);
  return n;
}

// A time: epoch milliseconds (a number; below 1e11 it is read as seconds), or
// an ISO 8601 string ("2026-10-01", "2026-10", "2026", "2026-10-01T14:00",
// "2026-10-01T14:00:00Z", "2026-10-01T14:00:00+02:00"). Returns {t, zoned}:
// `zoned` is true when the input says its zone (a number, Z or an offset).
// A string without a zone is read as UTC and shown as written.
const ISO = /^(\d{4})(?:-(\d{2})(?:-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3})\d*)?)?)?)?)?(Z|[+-]\d{2}:?\d{2})?$/;

export function time(v, field) {
  if (typeof v === "number") {
    if (!Number.isFinite(v)) fail(`\`${field}\` must be a time, got ${v}`);
    return { t: Math.abs(v) < 1e11 ? v * 1000 : v, zoned: true };
  }
  const m = typeof v === "string" ? ISO.exec(v.trim()) : null;
  if (!m) fail(`\`${field}\` must be an ISO date ("2026-10-01", "2026-10-01T14:00:00Z") or epoch ms, got ${JSON.stringify(v)}`);
  const [, y, mo = "01", d = "01", h = "00", mi = "00", s = "00", ms = "0", zone] = m;
  let t = Date.UTC(+y, +mo - 1, +d, +h, +mi, +s, +ms.padEnd(3, "0"));
  if (Number.isNaN(t) || +mo > 12 || +d > 31) fail(`\`${field}\` is not a real date: ${JSON.stringify(v)}`);
  if (zone && zone !== "Z") {
    const sign = zone[0] === "-" ? -1 : 1, z = zone.slice(1).replace(":", "");
    t -= sign * (+z.slice(0, 2) * 60 + +z.slice(2)) * 60000;
  }
  return { t, zoned: Boolean(zone) };
}

// Numbers for the model's text: short, no locale surprises.
export function fmt(v, unit) {
  const a = Math.abs(v);
  let s;
  if (a >= 1e9) s = trim(v / 1e9) + "B";
  else if (a >= 1e6) s = trim(v / 1e6) + "M";
  else if (a >= 1e4) s = trim(v / 1e3) + "k";
  else s = trim(v);
  if (!unit) return s;
  if (unit === "$" || unit === "£") return (v < 0 ? "-" : "") + unit + s.replace(/^-/, "");
  if (unit === "%") return s + "%";
  return s + " " + unit;
}

function trim(v) {
  const a = Math.abs(v);
  const d = a >= 100 ? 0 : a >= 10 ? 1 : 2;
  return String(+v.toFixed(d));
}

export function isoDay(t, withTime) {
  const s = new Date(t).toISOString();
  return withTime ? s.slice(0, 16).replace("T", " ") : s.slice(0, 10);
}

// Same labels twice in a list are an agent's mistake worth saying.
export function unique(labels, field) {
  const seen = new Set();
  for (const l of labels) {
    if (seen.has(l)) fail(`\`${field}\` has "${l}" twice: each must be unique`);
    seen.add(l);
  }
}
