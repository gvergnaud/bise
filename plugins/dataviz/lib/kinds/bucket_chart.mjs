// bucket_chart: vertical bars, one per bucket (time buckets, or plain categories),
// stacked or side by side when there are several series.
import { COMMON_PROPS, common, list, num, text, time, unique, fmt, isoDay, fail, LIMITS } from "../common.mjs";

const MAX_BUCKETS = 400, MAX_SERIES = 8;
const GRAINS = ["minute", "hour", "day", "week", "month", "quarter", "year"];
const DAY = 86400000;

// The bucket size from the gap between buckets, when not given.
export function guessGrain(ts) {
  if (ts.length < 2) return "day";
  const gaps = ts.slice(1).map((t, i) => t - ts[i]).sort((a, b) => a - b);
  const g = gaps[Math.floor(gaps.length / 2)];
  if (g < 3600000) return "minute";
  if (g < DAY) return "hour";
  if (g < 7 * DAY) return "day";
  if (g < 28 * DAY) return "week";
  if (g < 89 * DAY) return "month";
  if (g < 365 * DAY) return "quarter";
  return "year";
}

export default {
  name: "bucket_chart",
  description:
    "Show amounts per time bucket as vertical bars: commits per week, orders per day, errors per hour. " +
    "`buckets` are the bucket starts (ISO dates or epoch ms; plain words work too, for categories), " +
    "each series gives one value per bucket; several series stack (or sit side by side with stacked:false). " +
    "Example: {\"title\":\"Commits per week\",\"bucket\":\"week\",\"buckets\":[\"2026-09-07\",\"2026-09-14\"],\"series\":[{\"name\":\"commits\",\"values\":[41,57]}]}",
  inputSchema: {
    type: "object",
    properties: {
      ...COMMON_PROPS,
      buckets: {
        type: "array", minItems: 1, maxItems: MAX_BUCKETS,
        description: "The bucket starts in order: ISO dates or epoch ms (time buckets), or short labels (categories).",
        items: { type: ["string", "number"] },
      },
      bucket: { type: "string", enum: GRAINS, description: "The bucket size, for the axis labels. Default: guessed from the gaps." },
      series: {
        type: "array", minItems: 1, maxItems: MAX_SERIES,
        description: "One value per bucket each (same length as `buckets`).",
        items: {
          type: "object",
          properties: { name: { type: "string" }, values: { type: "array", items: { type: "number" } } },
          required: ["name", "values"],
        },
      },
      stacked: { type: "boolean", description: "Several series: stack them (default) or put them side by side (false)." },
    },
    required: ["title", "buckets", "series"],
  },

  normalize(args) {
    const out = { ...common(args) };
    const raw = list(args.buckets, "buckets", { max: MAX_BUCKETS });
    let times = null, zoned = false;
    try {
      times = raw.map((b, i) => { const r = time(b, `buckets[${i}]`); zoned ||= r.zoned; return r.t; });
    } catch { times = null; }
    if (times) {
      for (let i = 1; i < times.length; i++) if (times[i] <= times[i - 1]) fail(`buckets[${i}] is not after buckets[${i - 1}]: give them in time order, each once`);
      if (args.bucket !== undefined && !GRAINS.includes(args.bucket)) fail(`\`bucket\` must be one of ${GRAINS.join(", ")}`);
      out.buckets = times;
      out.time = { zoned, grain: args.bucket ?? guessGrain(times) };
    } else {
      out.buckets = raw.map((b, i) => text(b, `buckets[${i}]`, LIMITS.label, true));
      unique(out.buckets, "buckets");
    }
    out.series = list(args.series, "series", { max: MAX_SERIES }).map((s, i) => {
      if (!s || typeof s !== "object") fail(`series[${i}] must be {name, values}`);
      const values = list(s.values, `series[${i}].values`, { max: MAX_BUCKETS });
      if (values.length !== raw.length) fail(`series[${i}].values has ${values.length} values for ${raw.length} buckets: give one per bucket (0 when none)`);
      return { name: text(s.name, `series[${i}].name`, LIMITS.label, true), values: values.map((v, j) => num(v, `series[${i}].values[${j}]`)) };
    });
    unique(out.series.map((s) => s.name), "series names");
    out.stacked = out.series.length > 1 && args.stacked !== false;
    if (out.stacked && out.series.some((s) => s.values.some((v) => v < 0))) fail("stacked bars need values of 0 or more: use stacked:false for negative values");
    return out;
  },

  describe(c) {
    const label = (i) => (c.time ? isoDay(c.buckets[i], c.time.grain === "hour" || c.time.grain === "minute") : c.buckets[i]);
    const totals = c.buckets.map((_, i) => c.series.reduce((a, s) => a + s.values[i], 0));
    let hi = 0, lo = 0;
    totals.forEach((v, i) => { if (v > totals[hi]) hi = i; if (v < totals[lo]) lo = i; });
    const sum = totals.reduce((a, v) => a + v, 0);
    const span = c.time ? `${c.buckets.length} ${c.time.grain} buckets from ${label(0)} to ${label(c.buckets.length - 1)}` : `${c.buckets.length} buckets`;
    const per = c.series.length > 1 ? `; by series: ${c.series.map((s) => `${s.name} ${fmt(s.values.reduce((a, v) => a + v, 0), c.unit)}`).join(", ")}` : "";
    return `Bar chart over buckets "${c.title}", ${span}: total ${fmt(sum, c.unit)}, mean ${fmt(sum / c.buckets.length, c.unit)}, highest ${label(hi)} (${fmt(totals[hi], c.unit)}), lowest ${label(lo)} (${fmt(totals[lo], c.unit)}), last ${fmt(totals[totals.length - 1], c.unit)}${per}.`;
  },
};
