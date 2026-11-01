// The mermaid plugin's server: the parse that tells the agent its syntax error, the cheat
// sheet's examples, and the view it serves. node:test, no deps (gate.sh quick's gallery_plugins).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { answer, call, cleanSource, EXAMPLES, hint } from "../server.mjs";

const text = (r) => r.content[0].text;

test("a good source draws and names its type", async () => {
  const r = await call("render_diagram", { source: "flowchart LR\n  A[agent] --> B{green?}\n  B -->|yes| C[land]", title: "Land" });
  assert.equal(r.isError, undefined);
  assert.match(text(r), /^Drew a flowchart "Land" \(3 lines\)/);
  assert.deepEqual({ ok: r.structuredContent.ok, type: r.structuredContent.type, title: r.structuredContent.title }, { ok: true, type: "flowchart-v2", title: "Land" });
});

test("a jison syntax error comes back with its line, its text and what was expected", async () => {
  const source = "sequenceDiagram\n  participant A\n  A->>B: hi\n  B-->>A";
  const r = await call("render_diagram", { source });
  assert.equal(r.isError, true);
  assert.match(text(r), /^Mermaid syntax error on line 4, column 8: `B-->>A`: expected TXT, found a line break\. Fix the source and call render_diagram again\./);
  assert.equal(r.structuredContent.error.line, 4);
  assert.equal(r.structuredContent.source, source, "the view gets the source to show the error in it");
});

test("a langium syntax error (pie, packet...) has its line and column too", async () => {
  const r = await call("check_diagram", { source: 'pie\n  "a" 1' });
  assert.equal(r.isError, true);
  assert.match(text(r), /line 2, column 7: `"a" 1`: expected :, found "1"/);
});

test("an error at the end of the source points at its last line", async () => {
  const r = await call("check_diagram", { source: "graph TD\n  A-->" });
  assert.match(text(r), /line 2, column \d+ \(an arrow points to nothing\): `A-->`: expected .*found the end of the source/);
});

test("a plain hint says what is wrong on the line when a look can tell", async () => {
  assert.equal(hint("  B -->|yes| C[Land"), "a bracket isn't closed");
  assert.equal(hint('  A["step 1] --> B'), "a quote isn't closed");
  assert.equal(hint("  A] --> B"), "a bracket closes that was never opened");
  assert.equal(hint("  A -->|no|"), "an arrow points to nothing");
  assert.equal(hint("  A --> B", "the end of the source"), "the source ends too early");
  assert.equal(hint('  A["a [x] (y)"] --> B'), null, "brackets inside a quoted label do not count");
  const r = await call("render_diagram", { source: "flowchart TD\n  A[Start] --> B{ok?}\n  B -->|yes| C[Land\n  B -->|no| D" });
  assert.match(text(r), /^Mermaid syntax error on line 3, column \d+ \(a bracket isn't closed\): `B -->\|yes\| C\[Land`/);
  assert.equal(r.structuredContent.error.hint, "a bracket isn't closed");
  assert.equal((await call("check_diagram", { source: "nonsense" })).structuredContent.error.hint, "the first line names no diagram type");
});

test("a source with no type says what the first line must be", async () => {
  const r = await call("check_diagram", { source: "nonsense here" });
  assert.match(text(r), /line 1 \(the first line names no diagram type\): `nonsense here`: no diagram type: the first line must name one/);
});

test("diagrams whose parse sanitizes labels parse with no DOM (the DOMPurify stub)", async () => {
  for (const source of ["classDiagram\n  A <|-- B", "stateDiagram-v2\n  [*] --> A", "mindmap\n  root\n    a", "gantt\n  dateFormat YYYY-MM-DD\n  a :a1, 2024-01-01, 3d"]) {
    const r = await call("check_diagram", { source });
    assert.equal(r.isError, undefined, `${source.split("\n")[0]}: ${text(r)}`);
    assert.equal(r.structuredContent.unchecked, undefined);
  }
});

test("every example of the cheat sheet parses", async () => {
  assert.ok(EXAMPLES.length >= 20);
  for (const [type, , example] of EXAMPLES) {
    const r = await call("check_diagram", { source: example });
    assert.equal(r.isError, undefined, `${type}: ${text(r)}`);
  }
  const one = await call("diagram_examples", { type: "gantt" });
  assert.equal(one.structuredContent.types.length, 1);
  const none = await call("diagram_examples", { type: "nope" });
  assert.equal(none.isError, true);
});

test("a fence around the source and CRLF are dropped; empty and huge sources are refused", async () => {
  assert.equal(cleanSource("```mermaid\r\nflowchart TD\r\n  A-->B\r\n```"), "flowchart TD\n  A-->B");
  assert.equal((await call("render_diagram", { source: "  " })).isError, true);
  assert.match(text(await call("render_diagram", { source: "flowchart TD\n" + "A-->B\n".repeat(10_000) })), /the most is 50000/);
});

test("the view is one HTML: the kit and mermaid inlined, no network", async () => {
  const r = await answer("resources/read", { uri: "ui://mermaid/diagram" });
  const html = r.contents[0].text;
  assert.equal(r.contents[0].mimeType, "text/html;profile=mcp-app");
  assert.ok(!html.includes("<!-- bise-view-kit -->") && html.includes("<script data-kit>"), "the kit is inlined");
  assert.ok(!html.includes("<!-- mermaid -->") && html.includes("mermaid.initialize") , "mermaid is inlined");
  const lib = readFileSync(new URL("../vendor/mermaid.min.js", import.meta.url), "utf8");
  assert.ok(html.length > lib.length);
  assert.deepEqual(r.contents[0]._meta.ui.csp, undefined, "no domain asked: the view reaches nothing");
  // the inlined mermaid never closes its <script> early
  const body = html.slice(html.indexOf("<script>", html.indexOf("</style>", html.indexOf("<style>\n  /* the kit's tokens"))));
  assert.equal((body.match(/<\/script>/g) || []).length, 2, "mermaid's script and the view's own");
});

test("the tools: render for the model and the view, check and examples for the model only", async () => {
  const { tools } = await answer("tools/list", {});
  const by = Object.fromEntries(tools.map((t) => [t.name, t]));
  assert.equal(by.render_diagram._meta.ui.resourceUri, "ui://mermaid/diagram");
  assert.equal(by.render_diagram._meta.ui.visibility, undefined);
  assert.deepEqual(by.check_diagram._meta.ui.visibility, ["model"]);
  assert.deepEqual(by.diagram_examples._meta.ui.visibility, ["model"]);
});
