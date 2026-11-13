// The bundles' split (architect m_23781): office.js stays DOM-free (bise office run and the views
// load it); Writer's (plugins/docs) are tested in plugins/docs/test. The viewer shell that drew
// kit/office/*.js went with docs/plugin-apps.md step 5: every type opens in its plugin's view.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..", "..");

test("office.js loads with no window or document, and has no docx-preview in it", async () => {
  assert.equal(typeof Reflect.get(globalThis, "window"), "undefined");
  assert.equal(typeof Reflect.get(globalThis, "document"), "undefined");
  const office = await import("../../dist/office.js");
  assert.equal(typeof office.sheets.open, "function");
  const js = readFileSync(join(root, "office", "dist", "office.js"), "utf8");
  assert.ok(!js.includes("docx-preview"), "docx-preview lives in Writer's office-view.js only (plugins/docs)");
});

// art-notes: a note's place in words (kit/notes-anchors.js)
test("a note's anchor says its place in bise_proto's words (anchor_label.json)", async () => {
  const { createRequire } = await import("node:module");
  createRequire(import.meta.url)(join(root, "kit", "notes-anchors.js"));
  const rows = JSON.parse(readFileSync(join(root, "rust", "proto", "fixtures", "anchor_label.json"), "utf8"));
  const { label } = Reflect.get(globalThis, "BiseAnchors");
  for (const r of rows) assert.equal(label(r.anchor), r.label, JSON.stringify(r.anchor));
});
