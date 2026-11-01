// The tools: each kind's input checks, its normalized chart and its text.
// node --test plugins/dataviz/test/
import { test } from "node:test";
import assert from "node:assert/strict";
import { chart, TOOLS } from "../server.mjs";
import { KINDS } from "../lib/kinds/index.mjs";
import { time, fmt } from "../lib/common.mjs";
import { findLoop } from "../lib/kinds/sankey.mjs";
import { guessGrain } from "../lib/kinds/bucket_chart.mjs";
import { FIXTURES } from "./fixtures.mjs";

const ok = (name, args) => {
  const r = chart(name, args);
  assert.equal(r.isError, undefined, r.content[0].text);
  return r;
};
const err = (name, args) => {
  const r = chart(name, args);
  assert.equal(r.isError, true, `expected an error for ${JSON.stringify(args).slice(0, 80)}`);
  return r.content[0].text;
};

test("one tool per kind, each with a view and a fixture", () => {
  assert.deepEqual(TOOLS.map((t) => t.name), KINDS.map((k) => k.name));
  for (const t of TOOLS) {
    assert.equal(t._meta.ui.resourceUri, "ui://dataviz/chart");
    assert.ok(t.inputSchema.required.includes("title"), t.name);
    assert.ok(FIXTURES[t.name], `no fixture for ${t.name}`);
  }
});

test("every fixture gives a chart and a text the model reads", () => {
  for (const [name, args] of Object.entries(FIXTURES)) {
    const r = ok(name, args);
    assert.equal(r.structuredContent.kind, name);
    assert.equal(r.structuredContent.v, 1);
    assert.match(r.content[0].text, /The user sees the chart\.$/);
    assert.doesNotMatch(r.content[0].text, /NaN|undefined|Infinity/);
  }
});

test("an unknown tool is null, a bad input an error in words", () => {
  assert.equal(chart("pie3d", {}), null);
  assert.match(err("bar_chart", { bars: [{ label: "a", value: 1 }] }), /`title` is required/);
  assert.match(err("bar_chart", { title: "t", bars: [{ label: "a", value: -1 }] }), /0 or more/);
  assert.match(err("bar_chart", { title: "t", bars: [{ label: "a", value: 1 }, { label: "a", value: 2 }] }), /"a" twice/);
  assert.match(err("time_series", { title: "t", series: [{ name: "s", points: [["yesterday", 1]] }] }), /ISO date/);
  assert.match(err("bucket_chart", { title: "t", buckets: ["2026-01-01", "2026-02-01"], series: [{ name: "s", values: [1] }] }), /one per bucket/);
  assert.match(err("treemap", { title: "t", items: [{ name: "a", value: 0 }] }), /nothing to show/);
  assert.match(err("sankey", { title: "t", links: [{ from: "a", to: "b", value: 1 }, { from: "b", to: "a", value: 1 }] }), /loop: a → b → a/);
  assert.match(err("sankey", { title: "t", links: [{ from: "a", to: "a", value: 1 }] }), /to itself/);
});

test("times: ISO strings without a zone are UTC as written, numbers are zoned", () => {
  assert.deepEqual(time("2026-10-01", "t"), { t: Date.UTC(2026, 9, 1), zoned: false });
  assert.deepEqual(time("2026-10", "t"), { t: Date.UTC(2026, 9, 1), zoned: false });
  assert.deepEqual(time("2026-10-01T14:30:00+02:00", "t"), { t: Date.UTC(2026, 9, 1, 12, 30), zoned: true });
  assert.deepEqual(time("2026-10-01 14:30", "t"), { t: Date.UTC(2026, 9, 1, 14, 30), zoned: false });
  assert.deepEqual(time(1790000000, "t"), { t: 1790000000000, zoned: true });
  assert.throws(() => time("2026-13-01", "t"));
});

test("time series: points sorted, area by default for one line", () => {
  const c = ok("time_series", { title: "t", series: [{ name: "s", points: [["2026-01-03", 3], ["2026-01-01", 1]] }] }).structuredContent;
  assert.deepEqual(c.series[0].points.map((p) => p[1]), [1, 3]);
  assert.equal(c.area, true);
  assert.equal(c.from_zero, true);
  assert.deepEqual(c.time, { zoned: false, with_time: false });
});

test("ring: biggest first, more than 8 slices fold into Other", () => {
  const c = ok("ring_chart", FIXTURES.ring_chart).structuredContent;
  assert.equal(c.slices.length, 8);
  assert.equal(c.slices[0].label, "bise");
  assert.equal(c.slices[7].label, "Other (2)");
  assert.equal(c.slices[7].value, 1.25);
});

test("bars: limit keeps the top and sums the rest", () => {
  const c = ok("bar_chart", FIXTURES.bar_chart).structuredContent;
  assert.equal(c.bars.length, 8);
  assert.deepEqual(c.rest, { count: 2, value: 47 });
  const asc = ok("bar_chart", { title: "t", sort: "none", bars: [{ label: "b", value: 1 }, { label: "a", value: 9 }] }).structuredContent;
  assert.deepEqual(asc.bars.map((b) => b.label), ["b", "a"]);
});

test("buckets: time or categories, the grain guessed, stacking only for 0 or more", () => {
  const c = ok("bucket_chart", { title: "t", buckets: ["2026-01-05", "2026-01-12", "2026-01-19"], series: [{ name: "s", values: [1, 2, 3] }] }).structuredContent;
  assert.equal(c.time.grain, "week");
  assert.equal(c.stacked, false);
  const cat = ok("bucket_chart", { title: "t", buckets: ["mon", "tue"], series: [{ name: "a", values: [1, 2] }, { name: "b", values: [3, 4] }] }).structuredContent;
  assert.equal(cat.time, undefined);
  assert.equal(cat.stacked, true);
  assert.match(err("bucket_chart", { title: "t", buckets: ["a", "b"], series: [{ name: "a", values: [1, -2] }, { name: "b", values: [3, 4] }] }), /stacked:false/);
  assert.match(err("bucket_chart", { title: "t", buckets: ["2026-01-02", "2026-01-01"], series: [{ name: "s", values: [1, 2] }] }), /time order/);
  assert.equal(guessGrain([0, 3600000, 7200000]), "hour");
  assert.equal(guessGrain([Date.UTC(2026, 0, 1), Date.UTC(2026, 1, 1), Date.UTC(2026, 2, 1)]), "month");
});

test("treemap: a group's value is its children's sum, zeros dropped, 3 levels at most", () => {
  const c = ok("treemap", FIXTURES.treemap).structuredContent;
  const code = c.items.find((n) => n.name === "Code");
  assert.equal(code.value, 41 + 9 + 1.2 + 18);
  const z = ok("treemap", { title: "t", items: [{ name: "a", value: 1 }, { name: "b", value: 0 }] }).structuredContent;
  assert.deepEqual(z.items.map((n) => n.name), ["a"]);
  const deep = { name: "1", children: [{ name: "2", children: [{ name: "3", children: [{ name: "4", value: 1 }] }] }] };
  assert.match(err("treemap", { title: "t", items: [deep] }), /deeper than 3/);
});

test("sankey: the same pair twice is summed, nodes from the names", () => {
  const c = ok("sankey", { title: "t", links: [{ from: "a", to: "b", value: 1 }, { from: "a", to: "b", value: 2 }, { from: "b", to: "c", value: 3 }] }).structuredContent;
  assert.deepEqual(c.nodes.map((n) => n.name), ["a", "b", "c"]);
  assert.deepEqual(c.links, [{ source: 0, target: 1, value: 3 }, { source: 1, target: 2, value: 3 }]);
  assert.equal(findLoop(3, [{ source: 0, target: 1 }, { source: 1, target: 2 }]), null);
});

test("numbers for the model: short, unit on the right side", () => {
  assert.equal(fmt(1234, "€"), "1234 €");
  assert.equal(fmt(12345, "$"), "$12.3k");
  assert.equal(fmt(-3.456, "%"), "-3.46%");
  assert.equal(fmt(2.5e6), "2.5M");
});
