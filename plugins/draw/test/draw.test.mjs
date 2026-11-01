// The draw plugin's units: the core (layout, geometry, words, SVG) and the
// server's tools through answer(), in process, on a throwaway $PLUGIN_DATA.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

process.env.PLUGIN_DATA = mkdtempSync(join(tmpdir(), "draw-test-"));
const { answer, D } = await import("../server.mjs");
const call = (name, args) => answer("tools/call", { name, arguments: args || {} });
const textOf = (r) => r.content.map((c) => c.text).join("\n");
const overlap = (a, b) => a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;

test("a laid out graph has no two shapes on top of each other, and reads left to right", () => {
  const nodes = ["a", "b", "c", "d", "e", "f"].map((id) => D.fit(D.clean({ id, text: "node " + id.repeat(3) }, id)));
  const edges = [["a", "b"], ["a", "c"], ["b", "d"], ["c", "d"], ["d", "e"], ["e", "a"], ["f", "f"]].map(([from, to]) => ({ from, to }));
  D.layout(nodes, [], edges, "right", () => null);
  for (const x of nodes) for (const y of nodes) if (x !== y) assert.ok(!overlap(x, y), `${x.id} and ${y.id} overlap`);
  const at = Object.fromEntries(nodes.map((n) => [n.id, n]));
  assert.ok(at.a.x < at.b.x && at.b.x < at.d.x && at.d.x < at.e.x, "each edge goes right (the cycle e->a is broken)");
});

test("a group's frame holds no other node", () => {
  const ids = ["you", "hub", "journal", "deliver", "steer"];
  const nodes = ids.map((id) => D.fit(D.clean({ id, text: id }, id)));
  const groupOf = (id) => (id === "hub" || id === "journal" ? "g" : null);
  D.layout(nodes, [], [["you", "hub"], ["hub", "journal"], ["hub", "deliver"], ["deliver", "steer"]].map(([from, to]) => ({ from, to })), "right", groupOf);
  const frame = D.clean({ type: "frame", members: ["hub", "journal"], text: "g" }, "g");
  D.resolve([frame, ...nodes]);
  const box = { x: frame.x, y: frame.y - D.FRAME_LABEL, w: frame.w, h: frame.h + D.FRAME_LABEL };
  for (const n of nodes) if (!frame.members.includes(n.id)) assert.ok(!overlap(n, box), `${n.id} is inside the frame`);
});

test("a bound arrow ends on its shapes' borders and follows a moved shape", () => {
  const a = D.clean({ type: "rect", x: 0, y: 0, w: 100, h: 50 }, "a");
  const b = D.clean({ type: "ellipse", x: 300, y: 0, w: 100, h: 50 }, "b");
  const arrow = D.clean({ type: "arrow", from: "a", to: "b" }, "x");
  D.resolve([a, b, arrow]);
  assert.deepEqual(arrow.points[0], [106, 25]);
  assert.deepEqual(arrow.points[1], [294, 25]);
  D.move(b, 0, 200);
  D.resolve([a, b, arrow]);
  assert.ok(arrow.points[1][1] > 150, "the end went down with b");
});

test("a dirty element is cleaned: unknown type, wild numbers, a bad color, a script in its text", () => {
  const el = D.clean({ type: "blob", x: 1e9, w: -5, stroke: "javascript:x", text: "<script>alert(1)</script>", fontSize: 9999, extra: 1 }, "e");
  assert.equal(el.type, "rect");
  assert.equal(el.x, 1e5);
  assert.equal(el.w, 0);
  assert.equal(el.stroke, "ink");
  assert.equal(el.fontSize, 120);
  assert.equal(el.extra, undefined);
  const svg = D.toSVG({ id: "s", title: "<t>", elements: [D.fit(el)] });
  assert.ok(!svg.includes("<script>") && svg.includes("&lt;script&gt;"));
});

test("the same seed draws the same strokes", () => {
  const el = D.fit(D.clean({ type: "diamond", text: "same", fill: "blue" }, "d"));
  assert.equal(D.render(el, "light", "#fff"), D.render(JSON.parse(JSON.stringify(el)), "light", "#fff"));
});

test("draw, his edit by hand, then read_drawing tells the agent what he changed, once", () => {
  const r = call("draw", { title: "Signup flow", nodes: [{ id: "form", text: "Form" }, { id: "api", text: "API" }, { id: "db", text: "DB", shape: "ellipse" }], edges: [{ from: "form", to: "api", label: "POST" }, { from: "api", to: "db" }] });
  assert.ok(!r.isError, textOf(r));
  const d = r.structuredContent.drawing;
  assert.equal(d.id, "signup-flow");
  assert.equal(d.elements.length, 5);
  // the view saves his edits: api moved, a text added, db's text changed
  const api = { ...d.elements.find((e) => e.id === "api") };
  api.x += 40;
  const db = { ...d.elements.find((e) => e.id === "db"), text: "Postgres" };
  const saved = call("save_edits", { id: d.id, base_rev: d.rev, upserts: [api, db, { id: "t-u1", type: "text", x: 0, y: 200, text: "rate limit?" }] });
  assert.equal(saved.structuredContent.drawing.rev, d.rev + 1);
  const read = textOf(call("read_drawing", { id: d.id })).split("\n\nDrawing ")[0];
  assert.match(read, /box api "API": moved to/);
  assert.match(read, /ellipse db "Postgres": text "DB" -> "Postgres"/);
  assert.match(read, /added text t-u1 "rate limit\?"/);
  assert.doesNotMatch(read, /arrow/, "an arrow that followed its shapes is not a change");
  assert.match(textOf(call("read_drawing", {})), /^No change since you last looked/);
});

test("the agent's own changes are not reported back to it as his", () => {
  call("draw", { title: "Mine", nodes: [{ id: "a", text: "A" }] });
  call("add_shapes", { id: "mine", nodes: [{ id: "b", text: "B" }], edges: [{ from: "a", to: "b" }] });
  call("update_shapes", { id: "mine", changes: [{ id: "a", text: "A longer text", color: "blue" }] });
  assert.match(textOf(call("read_drawing", { id: "mine" })), /^No change since you last looked/);
});

test("an agent change made before reading says his pending edits first, so none is lost", () => {
  const d = call("draw", { title: "Pending", nodes: [{ id: "a", text: "A" }] }).structuredContent.drawing;
  call("save_edits", { id: d.id, upserts: [{ ...d.elements[0], text: "A by him" }] });
  const r = textOf(call("add_shapes", { id: d.id, nodes: [{ id: "b", text: "B" }] }));
  assert.match(r, /^Since you last looked \(rev 1 -> 2\), the user changed:\n\* box a "A by him": text "A" -> "A by him"\nAdded b\./);
  assert.match(textOf(call("read_drawing", { id: d.id })), /^No change since you last looked/);
});

test("a save that only moves a frame's derived box keeps the author mark", () => {
  const d = call("draw", { title: "Frames", nodes: [{ id: "a", text: "A", group: "g" }] }).structuredContent.drawing;
  const frame = { ...d.elements.find((e) => e.id === "g"), x: 999 };
  const s = call("save_edits", { id: d.id, upserts: [frame] }).structuredContent.drawing;
  assert.equal(s.elements.find((e) => e.id === "g").by, "agent");
});

test("remove_shapes takes the arrows bound to what it removes", () => {
  call("draw", { title: "Remove", nodes: [{ id: "a", text: "A" }, { id: "b", text: "B" }], edges: [{ from: "a", to: "b", id: "ab" }] });
  const r = call("remove_shapes", { id: "remove", ids: ["b"] });
  assert.match(textOf(r), /Removed b, ab\./);
  assert.deepEqual(r.structuredContent.drawing.elements.map((e) => e.id), ["a"]);
});

test("a wrong call answers an error the agent can act on, never a dead server", () => {
  call("draw", { title: "Errors", nodes: [{ id: "a", text: "A" }] });
  for (const [name, args, words] of [
    ["add_shapes", { id: "errors", edges: [{ from: "a", to: "nope" }] }, /no such element/],
    ["add_shapes", { id: "errors", nodes: [{ id: "a", text: "again" }] }, /already in the drawing/],
    ["update_shapes", { id: "errors", changes: [{ id: "zz", text: "x" }] }, /no element "zz"/],
    ["show_drawing", { id: "no-such" }, /no drawing "no-such"/],
    ["export_drawing", { id: "errors", format: "png" }, /no PNG of errors yet/],
  ]) {
    const r = call(name, args);
    assert.equal(r.isError, true, name);
    assert.match(textOf(r), words);
  }
});

test("export_drawing writes an SVG; png gives the view's snapshot", () => {
  call("draw", { title: "Export", nodes: [{ id: "a", text: "A & B" }] });
  const svgOut = textOf(call("export_drawing", { id: "export" }));
  const path = svgOut.match(/Wrote (\S+\.svg)/)[1];
  const svg = readFileSync(path, "utf8");
  assert.ok(svg.startsWith("<svg") && svg.includes("A &amp; B"));
  const png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
  assert.ok(!call("save_snapshot", { id: "export", rev: 1, png }).isError);
  const out = textOf(call("export_drawing", { id: "export", format: "png" }));
  assert.ok(existsSync(out.match(/Wrote (\S+\.png)/)[1]));
});

test("the view's tools stay out of the model's sight; its view is served whole", () => {
  const tools = answer("tools/list", {}).tools;
  const vis = (n) => tools.find((t) => t.name === n)._meta.ui.visibility;
  for (const n of ["get_drawing", "save_edits", "save_snapshot"]) assert.deepEqual(vis(n), ["app"]);
  for (const n of ["add_shapes", "update_shapes", "remove_shapes", "read_drawing", "export_drawing"]) assert.deepEqual(vis(n), ["model"]);
  const html = answer("resources/read", { uri: "ui://draw/canvas" }).contents[0].text;
  assert.ok(html.includes("var DrawCore") && html.includes("window.kit =") && !html.includes("<!-- draw-core -->"));
});
