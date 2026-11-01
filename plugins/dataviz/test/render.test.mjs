// The renderers (views/charts/*.js) run here as they run in the view: one
// global DV, every kind drawn from its fixture at a thread's and a panel's width.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import vm from "node:vm";
import { chart, viewHtml } from "../server.mjs";
import { FIXTURES } from "./fixtures.mjs";

const dir = new URL("../views/charts/", import.meta.url);
const ctx = vm.createContext({ Intl, Math, Date, Number, String, Array, Object, Set, Map, JSON });
for (const f of readdirSync(dir).filter((f) => f.endsWith(".js")).sort()) vm.runInContext(readFileSync(new URL(f, dir), "utf8"), ctx, { filename: f });
const DV = ctx.DV;
const opts = (width, extra = {}) => ({ width, hidden: new Set(), locale: "en-US", utc: true, fullscreen: false, ...extra });
const drawn = (name, width, extra) => DV.kinds[name].draw(chart(name, FIXTURES[name]).structuredContent, opts(width, extra));

test("every kind has a renderer", () => {
  assert.deepEqual(Object.keys(DV.kinds).sort(), Object.keys(FIXTURES).sort());
});

for (const name of Object.keys(FIXTURES)) {
  test(`${name} draws clean SVG at 320, 600 and a 900 panel`, () => {
    for (const [w, extra] of [[320], [600], [900, { fullscreen: true }]]) {
      const d = drawn(name, w, extra);
      assert.ok(d.height > 100 && d.height < 1000, `height ${d.height}`);
      assert.doesNotMatch(d.svg, /NaN|undefined|Infinity|null/, `${name} at ${w}`);
      // every data-tip has its tip
      for (const m of d.svg.matchAll(/data-tip="(\d+)"/g)) assert.ok(d.tips[+m[1]], `tip ${m[1]}`);
      // nothing drawn past the box's sides
      for (const m of d.svg.matchAll(/<rect x="(-?[\d.]+)"[^>]*width="([\d.]+)"/g)) assert.ok(+m[1] >= -0.5 && +m[1] + +m[2] <= w + 0.5, `${name} rect ${m[0]}`);
    }
  });
}

test("a label from the data is escaped in the SVG", () => {
  const c = chart("bar_chart", { title: "t", bars: [{ label: "<img src=x onerror=alert(1)>", value: 1 }] }).structuredContent;
  const d = DV.kinds.bar_chart.draw(c, opts(600));
  assert.doesNotMatch(d.svg, /<img/);
  assert.doesNotMatch(d.tips.join(""), /<img/);
});

test("hover finds the nearest time on every shown line", () => {
  const d = drawn("time_series", 600);
  const h = d.hover(300, 100);
  assert.equal(h.dots.length, 2);
  assert.match(h.tip, /desktop/);
  assert.equal(d.hover(-50, 100), null);
  const one = DV.kinds.time_series.draw(chart("time_series", FIXTURES.time_series).structuredContent, opts(600, { hidden: new Set(["terminal"]) }));
  assert.equal(one.hover(300, 100).dots.length, 1);
  assert.equal(one.legend.find((l) => l.name === "terminal").off, true);
});

test("bucket hover covers its column and sums the series", () => {
  const d = drawn("bucket_chart", 600);
  const h = d.hover(400, 100);
  assert.match(h.tip, /total/);
  assert.ok(h.band.w > 10);
});

test("squarify fills its box with each value's share", () => {
  const vals = [6, 6, 4, 3, 2, 2, 1];
  const boxes = DV.squarify(vals, 0, 0, 600, 400);
  const total = vals.reduce((a, v) => a + v, 0);
  boxes.forEach((b, i) => {
    assert.ok(Math.abs(b.w * b.h - (vals[i] / total) * 240000) < 1, `area ${i}`);
    assert.ok(b.x >= -1e-6 && b.y >= -1e-6 && b.x + b.w <= 600 + 1e-6 && b.y + b.h <= 400 + 1e-6, `inside ${i}`);
  });
});

test("sankey: columns by depth, sinks on the right, no two nodes overlap", () => {
  const c = chart("sankey", FIXTURES.sankey).structuredContent;
  const L = DV.sankeyLayout(c, 600, 260);
  const by = (n) => L.nodes.find((d) => d.name === n);
  assert.equal(by("Salary").depth, 0);
  assert.equal(by("Budget").depth, 1);
  assert.equal(by("Food").depth, L.last); // a sink goes right
  for (const a of L.nodes) for (const b of L.nodes) {
    if (a === b || a.depth !== b.depth) continue;
    assert.ok(a.y1 <= b.y0 + 1e-6 || b.y1 <= a.y0 + 1e-6, `${a.name} overlaps ${b.name}`);
  }
  for (const d of L.nodes) assert.ok(d.y0 >= -1e-6 && d.y1 <= 260 + 1e-6, `${d.name} inside`);
});

test("time ticks: whole units, in order, labels change their year once", () => {
  const ticks = DV.timeTicks(Date.UTC(2025, 10, 3), Date.UTC(2026, 3, 20), 6, { utc: true, locale: "en-US" });
  assert.ok(ticks.length >= 3 && ticks.length <= 7);
  for (const t of ticks) assert.equal(new Date(t.t).getUTCDate(), 1);
  assert.equal(ticks.filter((t) => /20\d\d/.test(t.label)).length, 2);
});

test("the view's html carries the kit and every renderer, no marker left", () => {
  const html = viewHtml();
  assert.doesNotMatch(html, /<!-- (bise-view-kit|dataviz-charts) -->/);
  assert.match(html, /window\.kit = /);
  for (const name of Object.keys(FIXTURES)) assert.match(html, new RegExp(`DV\\.kinds\\.${name} =`));
});
