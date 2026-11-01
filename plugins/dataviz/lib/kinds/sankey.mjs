// sankey: flows between named stages, band width = value.
import { COMMON_PROPS, common, list, num, text, fmt, fail, LIMITS } from "../common.mjs";

const MAX_LINKS = 200, MAX_NODES = 60;

export default {
  name: "sankey",
  description:
    "Show flows from one stage to the next as bands whose width is the amount: a budget from income to spending, a funnel, traffic from sources to pages, energy from source to use. " +
    "Give `links` {from, to, value}; the nodes come from their names. No loops (a flow never comes back to a node it left). " +
    "Example: {\"title\":\"October budget\",\"unit\":\"€\",\"links\":[{\"from\":\"Salary\",\"to\":\"Budget\",\"value\":4200},{\"from\":\"Budget\",\"to\":\"Rent\",\"value\":1500},{\"from\":\"Budget\",\"to\":\"Food\",\"value\":600}]}",
  inputSchema: {
    type: "object",
    properties: {
      ...COMMON_PROPS,
      links: {
        type: "array", minItems: 1, maxItems: MAX_LINKS,
        description: "The flows, each {from, to, value} with value above 0. The same pair twice is summed.",
        items: {
          type: "object",
          properties: { from: { type: "string" }, to: { type: "string" }, value: { type: "number", exclusiveMinimum: 0 } },
          required: ["from", "to", "value"],
        },
      },
    },
    required: ["title", "links"],
  },

  normalize(args) {
    const out = { ...common(args) };
    const nodes = [], index = new Map(), pairs = new Map();
    const node = (name) => {
      if (!index.has(name)) {
        if (nodes.length >= MAX_NODES) fail(`more than ${MAX_NODES} nodes: group the small ones`);
        index.set(name, nodes.length);
        nodes.push(name);
      }
      return index.get(name);
    };
    list(args.links, "links", { max: MAX_LINKS }).forEach((l, i) => {
      if (!l || typeof l !== "object") fail(`links[${i}] must be {from, to, value}`);
      const from = text(l.from, `links[${i}].from`, LIMITS.label, true), to = text(l.to, `links[${i}].to`, LIMITS.label, true);
      if (from === to) fail(`links[${i}] goes from "${from}" to itself`);
      const value = num(l.value, `links[${i}].value`, { min: 0 });
      if (value === 0) return;
      const key = `${node(from)}>${node(to)}`;
      pairs.set(key, (pairs.get(key) || 0) + value);
    });
    if (!pairs.size) fail("every link is 0: nothing to show");
    out.nodes = nodes.map((name) => ({ name }));
    out.links = [...pairs].map(([k, value]) => { const [s, t] = k.split(">").map(Number); return { source: s, target: t, value }; });
    const loop = findLoop(nodes.length, out.links);
    if (loop) fail(`the links make a loop: ${loop.map((i) => nodes[i]).join(" → ")}; a Sankey only goes forward`);
    return out;
  },

  describe(c) {
    const inn = new Array(c.nodes.length).fill(0), outv = new Array(c.nodes.length).fill(0);
    for (const l of c.links) { outv[l.source] += l.value; inn[l.target] += l.value; }
    const sources = c.nodes.map((n, i) => i).filter((i) => !inn[i]);
    const sinks = c.nodes.map((n, i) => i).filter((i) => !outv[i]);
    const total = sources.reduce((a, i) => a + outv[i], 0);
    const big = [...c.links].sort((a, b) => b.value - a.value).slice(0, 8)
      .map((l) => `${c.nodes[l.source].name} → ${c.nodes[l.target].name} ${fmt(l.value, c.unit)}`);
    return `Sankey "${c.title}": ${c.nodes.length} nodes, ${c.links.length} flows, ${fmt(total, c.unit)} leaving the sources (${sources.map((i) => c.nodes[i].name).join(", ")}), ending in ${sinks.map((i) => `${c.nodes[i].name} ${fmt(inn[i], c.unit)}`).join(", ")}. Biggest flows: ${big.join("; ")}.`;
  },
};

// A cycle in the graph as node indices (first node repeated at the end), or null.
export function findLoop(n, links) {
  const next = Array.from({ length: n }, () => []);
  for (const l of links) next[l.source].push(l.target);
  const state = new Array(n).fill(0), stack = [];
  const visit = (v) => {
    state[v] = 1; stack.push(v);
    for (const w of next[v]) {
      if (state[w] === 1) return [...stack.slice(stack.indexOf(w)), w];
      if (state[w] === 0) { const r = visit(w); if (r) return r; }
    }
    state[v] = 2; stack.pop();
    return null;
  };
  for (let v = 0; v < n; v++) if (state[v] === 0) { const r = visit(v); if (r) return r; }
  return null;
}
