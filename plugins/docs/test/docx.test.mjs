// Writer's docx (plugins/docs/ai.mistral.vibe/views/writer/docx.js, office-artifacts plan §4): its
// bundle carries docx-preview and nothing of the markdown parser; the pure helpers: drawn
// paragraphs paired with the outline's, and the anchors (architect m_60380: a paragraph's key is
// the old viewer's anchor minus its quote, so old notes place). docx.js imports its bundles by
// their served paths, so its helpers are loaded here with those imports cut.
//   node --test plugins/docs/test/docx.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const writer = join(root, "plugins", "docs", "ai.mistral.vibe", "views", "writer");

/** docx.js without its imports (office-view.js needs a DOM, ~kit/office is the hub's). */
async function helpers() {
  const src = readFileSync(join(writer, "docx.js"), "utf8").replace(/^import .* from "[^"]+";$/gm, "");
  return import(`data:text/javascript;base64,${Buffer.from(src).toString("base64")}`);
}

test("office-view.js draws a docx with docx-preview, within its 2 MB, and no markdown parser rides along", () => {
  const view = readFileSync(join(writer, "office-view.js"), "utf8");
  assert.match(view, /export\s*\{[^}]*renderDocx/, "office-view.js exports renderDocx");
  assert.ok(view.length < 2 * 1024 * 1024, "within its 2 MB budget");
  assert.ok(!view.includes("micromark"), "the docx bundle carries no markdown parser");
});

test("drawn paragraphs pair with the outline's, passing over what it skips", async () => {
  const { matchParas, parasOf } = await helpers();
  const outline = parasOf([
    { id: "p1", type: "para", text: "Title" },
    { id: "t", type: "table", children: [{ id: "p2", type: "para", text: "cell  one" }, { id: "c", type: "comment" }] },
    { id: "p3", type: "para", text: "Body" },
  ]);
  assert.deepEqual(outline.map((n) => n.id), ["p1", "p2", "p3"], "a table's paragraphs in place, no comment");
  // a text box's paragraph drawn between them is passed over; whitespace does not count
  assert.deepEqual(matchParas(["Title", "in a text box", "cell one", "Body"], outline.map((n) => n.text)), [0, -1, 1, 2]);
  // a paragraph docx-preview did not draw: the next ones still pair
  assert.deepEqual(matchParas(["Title", "Body"], outline.map((n) => n.text)), [0, 2]);
});

test("a paragraph's anchor is the old viewer's minus its quote, in sorted keys; anything else names none", async () => {
  const { paraKey, paraOfKey } = await helpers();
  assert.equal(paraKey("3F2A1B00"), '{"kind":"para","para":"3F2A1B00"}');
  assert.equal(paraOfKey(paraKey("00AB")), "00AB");
  assert.equal(paraOfKey('{"block":"b1","kind":"block"}'), null);
  assert.equal(paraOfKey('{"kind":"para","para":7}'), null);
  assert.equal(paraOfKey("not json"), null);
});
