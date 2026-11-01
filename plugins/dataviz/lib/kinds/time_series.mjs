// time_series: lines over time, one line per series.
import { COMMON_PROPS, common, list, num, time, text, unique, fmt, isoDay, fail, LIMITS } from "../common.mjs";

const MAX_SERIES = 8, MAX_POINTS = 5000;

export default {
  name: "time_series",
  description:
    "Show values over time as lines (a trend: daily signups, latency, price). One line per series, up to 8. " +
    "Each point is [time, value]: time an ISO date (\"2026-10-01\", \"2026-10-01T14:00:00Z\") or epoch ms. " +
    "Example: {\"title\":\"Daily signups\",\"series\":[{\"name\":\"web\",\"points\":[[\"2026-10-01\",120],[\"2026-10-02\",134]]}]}",
  inputSchema: {
    type: "object",
    properties: {
      ...COMMON_PROPS,
      series: {
        type: "array", minItems: 1, maxItems: MAX_SERIES,
        description: "The lines. Their points need not share times.",
        items: {
          type: "object",
          properties: {
            name: { type: "string", description: "The legend's name of this line." },
            points: {
              type: "array", minItems: 1, maxItems: MAX_POINTS,
              description: "[time, value] pairs, any order (sorted for you).",
              items: { type: "array", prefixItems: [{ type: ["string", "number"] }, { type: "number" }], minItems: 2, maxItems: 2 },
            },
          },
          required: ["name", "points"],
        },
      },
      area: { type: "boolean", description: "Fill under the line (a single series reads well with it). Default: true for one series." },
      from_zero: { type: "boolean", description: "Start the value axis at 0. Default true; false for prices, temperatures, anything far from 0." },
    },
    required: ["title", "series"],
  },

  normalize(args) {
    const out = { ...common(args) };
    let zoned = false, withTime = false;
    out.series = list(args.series, "series", { max: MAX_SERIES }).map((s, i) => {
      if (!s || typeof s !== "object") fail(`series[${i}] must be {name, points}`);
      const name = text(s.name, `series[${i}].name`, LIMITS.label, true);
      const points = list(s.points, `series[${i}].points`, { max: MAX_POINTS }).map((p, j) => {
        const where = `series[${i}].points[${j}]`;
        if (!Array.isArray(p) || p.length !== 2) fail(`${where} must be [time, value]`);
        const { t, zoned: z } = time(p[0], `${where}[0]`);
        zoned ||= z;
        if (typeof p[0] === "number" || /[T ]\d/.test(p[0])) withTime = true;
        return [t, num(p[1], `${where}[1]`)];
      });
      points.sort((a, b) => a[0] - b[0]);
      return { name, points };
    });
    unique(out.series.map((s) => s.name), "series names");
    out.area = typeof args.area === "boolean" ? args.area : out.series.length === 1;
    out.from_zero = args.from_zero !== false;
    out.time = { zoned, with_time: withTime };
    return out;
  },

  describe(c) {
    const all = c.series.flatMap((s) => s.points.map((p) => p[0]));
    const from = isoDay(Math.min(...all), c.time.with_time), to = isoDay(Math.max(...all), c.time.with_time);
    const lines = c.series.map((s) => {
      let lo = s.points[0], hi = s.points[0];
      for (const p of s.points) { if (p[1] < lo[1]) lo = p; if (p[1] > hi[1]) hi = p; }
      const first = s.points[0][1], last = s.points[s.points.length - 1][1];
      const change = first !== 0 ? `, ${last >= first ? "+" : ""}${Math.round(((last - first) / Math.abs(first)) * 100)}% from first to last` : "";
      return `${s.name}: ${s.points.length} points, min ${fmt(lo[1], c.unit)} (${isoDay(lo[0], c.time.with_time)}), max ${fmt(hi[1], c.unit)} (${isoDay(hi[0], c.time.with_time)}), last ${fmt(last, c.unit)}${change}`;
    });
    return `Time series "${c.title}", ${from} to ${to}. ${lines.join("; ")}.`;
  },
};
