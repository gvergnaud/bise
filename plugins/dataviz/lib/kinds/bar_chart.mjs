// bar_chart: a ranked top list as horizontal bars.
import { COMMON_PROPS, common, list, num, text, unique, fmt, fail, LIMITS } from "../common.mjs";

const MAX_IN = 1000, MAX_SHOWN = 50;

export default {
  name: "bar_chart",
  description:
    "Show a ranking as horizontal bars (a top list): biggest files, busiest endpoints, top customers. " +
    "Sorted biggest first unless `sort` says otherwise; `limit` bars are drawn (default 15) and the rest is summed in one line. " +
    "Example: {\"title\":\"Slowest endpoints\",\"unit\":\"ms\",\"bars\":[{\"label\":\"/search\",\"value\":840},{\"label\":\"/export\",\"value\":610}]}",
  inputSchema: {
    type: "object",
    properties: {
      ...COMMON_PROPS,
      bars: {
        type: "array", minItems: 1, maxItems: MAX_IN,
        description: "The rows, each {label, value}, value 0 or more; `note` is an optional short text after the value.",
        items: {
          type: "object",
          properties: { label: { type: "string" }, value: { type: "number", minimum: 0 }, note: { type: "string" } },
          required: ["label", "value"],
        },
      },
      sort: { type: "string", enum: ["desc", "asc", "none"], description: "desc (default): biggest first; none: keep the given order." },
      limit: { type: "integer", minimum: 1, maximum: MAX_SHOWN, description: "How many bars to draw, default 15, 50 at most." },
    },
    required: ["title", "bars"],
  },

  normalize(args) {
    const out = { ...common(args) };
    let bars = list(args.bars, "bars", { max: MAX_IN }).map((b, i) => {
      if (!b || typeof b !== "object") fail(`bars[${i}] must be {label, value}`);
      const bar = { label: text(b.label, `bars[${i}].label`, LIMITS.label, true), value: num(b.value, `bars[${i}].value`, { min: 0 }) };
      const note = text(b.note, `bars[${i}].note`, 40);
      if (note) bar.note = note;
      return bar;
    });
    unique(bars.map((b) => b.label), "bar labels");
    const sort = args.sort ?? "desc";
    if (!["desc", "asc", "none"].includes(sort)) fail("`sort` must be desc, asc or none");
    if (sort === "desc") bars.sort((a, b) => b.value - a.value);
    if (sort === "asc") bars.sort((a, b) => a.value - b.value);
    const limit = args.limit === undefined ? 15 : Math.round(num(args.limit, "limit", { min: 1 }));
    if (limit > MAX_SHOWN) fail(`\`limit\` is ${MAX_SHOWN} at most`);
    if (bars.length > limit) {
      const rest = bars.slice(limit);
      out.rest = { count: rest.length, value: rest.reduce((a, b) => a + b.value, 0) };
      bars = bars.slice(0, limit);
    }
    out.bars = bars;
    return out;
  },

  describe(c) {
    const rows = c.bars.map((b, i) => `${i + 1}. ${b.label} ${fmt(b.value, c.unit)}${b.note ? ` (${b.note})` : ""}`);
    const rest = c.rest ? `; ${c.rest.count} more, ${fmt(c.rest.value, c.unit)} together` : "";
    return `Bar chart "${c.title}": ${rows.join("; ")}${rest}.`;
  },
};
