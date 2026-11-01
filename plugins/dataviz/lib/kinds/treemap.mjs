// treemap: a hierarchy of sizes as nested rectangles (area = value).
import { COMMON_PROPS, common, list, num, text, unique, fmt, fail, LIMITS } from "../common.mjs";

const MAX_DEPTH = 3, MAX_LEAVES = 500, MAX_KIDS = 200;

export default {
  name: "treemap",
  description:
    "Show what a total is made of as nested rectangles, area = value: disk usage by folder, spend by team then vendor, bundle size by package. " +
    "`items` is a tree: a leaf is {name, value}, a group is {name, children: [...]} (its value is its children's sum), 3 levels at most. " +
    "Example: {\"title\":\"Disk usage\",\"unit\":\"GB\",\"items\":[{\"name\":\"Photos\",\"children\":[{\"name\":\"2025\",\"value\":40},{\"name\":\"2026\",\"value\":22}]},{\"name\":\"Apps\",\"value\":31}]}",
  inputSchema: {
    type: "object",
    properties: {
      ...COMMON_PROPS,
      items: {
        type: "array", minItems: 1, maxItems: MAX_KIDS,
        description: "The top level: leaves {name, value} and groups {name, children}. Top-level items get the colors.",
        items: {
          type: "object",
          properties: {
            name: { type: "string" },
            value: { type: "number", minimum: 0, description: "A leaf's size." },
            children: { type: "array", description: "A group's items, same shape.", items: { type: "object" } },
          },
          required: ["name"],
        },
      },
    },
    required: ["title", "items"],
  },

  normalize(args) {
    const out = { ...common(args) };
    let leaves = 0;
    const walk = (items, where, depth) => {
      list(items, where, { max: MAX_KIDS });
      const nodes = items.map((it, i) => {
        const at = `${where}[${i}]`;
        if (!it || typeof it !== "object") fail(`${at} must be {name, value} or {name, children}`);
        const name = text(it.name, `${at}.name`, LIMITS.label, true);
        if (Array.isArray(it.children) && it.children.length) {
          if (depth >= MAX_DEPTH) fail(`${at} is deeper than ${MAX_DEPTH} levels: fold the last level into values`);
          const children = walk(it.children, `${at}.children`, depth + 1);
          return { name, value: children.reduce((a, c) => a + c.value, 0), children };
        }
        leaves += 1;
        if (leaves > MAX_LEAVES) fail(`more than ${MAX_LEAVES} leaves: group the small ones`);
        return { name, value: num(it.value, `${at}.value`, { min: 0 }) };
      }).filter((n) => n.value > 0);
      unique(nodes.map((n) => n.name), `${where} names`);
      return nodes.sort((a, b) => b.value - a.value);
    };
    out.items = walk(args.items, "items", 1);
    if (!out.items.length) fail("every value is 0: nothing to show");
    return out;
  },

  describe(c) {
    const total = c.items.reduce((a, n) => a + n.value, 0);
    const pct = (v) => `${Math.round((v / total) * 1000) / 10}%`;
    const top = c.items.slice(0, 10).map((n) => {
      const kids = n.children ? ` [biggest: ${n.children.slice(0, 3).map((k) => `${k.name} ${fmt(k.value, c.unit)}`).join(", ")}]` : "";
      return `${n.name} ${fmt(n.value, c.unit)} (${pct(n.value)})${kids}`;
    });
    const more = c.items.length > 10 ? `; ${c.items.length - 10} more` : "";
    return `Treemap "${c.title}", total ${fmt(total, c.unit)}: ${top.join("; ")}${more}.`;
  },
};
