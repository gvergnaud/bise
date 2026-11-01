// ring_chart: parts of a whole as a donut, with a legend that carries the numbers.
import { COMMON_PROPS, common, list, num, text, unique, fmt, fail, LIMITS } from "../common.mjs";

const MAX_IN = 200, SHOWN = 8;

export default {
  name: "ring_chart",
  description:
    "Show parts of a whole as a ring (donut): market share, a budget split, time per project. " +
    "Values are added up; more than 8 slices fold the smallest into \"Other\". " +
    "Example: {\"title\":\"Time this week\",\"unit\":\"h\",\"slices\":[{\"label\":\"bise\",\"value\":21},{\"label\":\"meetings\",\"value\":6}]}",
  inputSchema: {
    type: "object",
    properties: {
      ...COMMON_PROPS,
      slices: {
        type: "array", minItems: 1, maxItems: MAX_IN,
        description: "The parts, each {label, value}, value 0 or more.",
        items: { type: "object", properties: { label: { type: "string" }, value: { type: "number", minimum: 0 } }, required: ["label", "value"] },
      },
      center: { type: "string", description: "Optional words under the total in the middle of the ring (\"total\", \"hours\")." },
    },
    required: ["title", "slices"],
  },

  normalize(args) {
    const out = { ...common(args) };
    let slices = list(args.slices, "slices", { max: MAX_IN }).map((s, i) => {
      if (!s || typeof s !== "object") fail(`slices[${i}] must be {label, value}`);
      return { label: text(s.label, `slices[${i}].label`, LIMITS.label, true), value: num(s.value, `slices[${i}].value`, { min: 0 }) };
    });
    unique(slices.map((s) => s.label), "slice labels");
    if (slices.every((s) => s.value === 0)) fail("every slice is 0: nothing to show");
    slices.sort((a, b) => b.value - a.value);
    if (slices.length > SHOWN) {
      const rest = slices.slice(SHOWN - 1);
      slices = [...slices.slice(0, SHOWN - 1), { label: `Other (${rest.length})`, value: rest.reduce((a, s) => a + s.value, 0), other: true }];
    }
    out.slices = slices;
    const center = text(args.center, "center", 24);
    if (center) out.center = center;
    return out;
  },

  describe(c) {
    const total = c.slices.reduce((a, s) => a + s.value, 0);
    const parts = c.slices.map((s) => `${s.label} ${fmt(s.value, c.unit)} (${Math.round((s.value / total) * 1000) / 10}%)`);
    return `Ring chart "${c.title}", total ${fmt(total, c.unit)}: ${parts.join(", ")}.`;
  },
};
