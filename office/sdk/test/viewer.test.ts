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

test("a site's selection is read again before it counts (architect m_24172)", async () => {
  const { readSelect } = await helpers("site.js");
  const ok = readSelect({ anchor: { kind: "block", block: "#intro", quote: "open the waitlist" }, quote: "open the waitlist", rect: { x: 1, y: 2, w: 3, h: 4 } });
  assert.deepEqual(ok, { anchor: { kind: "block", block: "#intro", quote: "open the waitlist" }, quote: "open the waitlist", rect: { x: 1, y: 2, w: 3, h: 4 } });
  assert.equal(readSelect({ anchor: { kind: "cell", sheet: "x", range: "A1" } }), null, "a site only makes block anchors");
  assert.equal(readSelect({ anchor: { kind: "block", block: "x".repeat(301) } }), null);
  assert.equal(readSelect({ anchor: { kind: "block", block: 7 } }), null);
  const long = readSelect({ anchor: { kind: "block", block: "p", quote: "q".repeat(900), extra: "token?" }, quote: "q".repeat(900), rect: { x: Infinity, y: 0, w: 1, h: 1 } });
  assert.equal(long.anchor.quote.length, 300);
  assert.equal(long.quote.length, 300);
  assert.equal(long.rect, null);
  assert.deepEqual(Object.keys(long.anchor).sort(), ["block", "kind", "quote"], "nothing else the site put in it");
});

test("a site's localStorage reaches the hub only from its own frame, read again (web-storage, architect m_46151)", async () => {
  const { storageOf, storageSender } = await helpers("site.js");
  const frame = { name: "the site's frame" };
  const other = { name: "another frame" };
  const msg = { bise: "site-storage", local: { budget: "{\"rent\":900}", n: 3 } };
  assert.deepEqual(storageOf(frame, msg, frame), { budget: "{\"rent\":900}" }, "its strings only");
  assert.equal(storageOf(other, msg, frame), null, "another frame's post is ignored");
  assert.equal(storageOf(null, msg, null), null, "no frame: nothing");
  assert.equal(storageOf(frame, { bise: "site-select", local: {} }, frame), null, "another kind");
  assert.equal(storageOf(frame, { bise: "site-storage", local: ["x"] }, frame), null, "not a map");
  assert.equal(storageOf(frame, { bise: "site-storage", local: { x: "y".repeat(5 * 1024 * 1024) } }, frame), null, "over 5 MB");
  // one request at a time, the newest map last: a slow POST never lands after a newer one
  const sent: Record<string, string>[] = [];
  const done: (() => void)[] = [];
  const keep = storageSender((local: Record<string, string>) => {
    sent.push(local);
    return new Promise<void>((r) => done.push(r));
  });
  keep({ v: "1" });
  keep({ v: "2" });
  keep({ v: "3" });
  assert.deepEqual(sent, [{ v: "1" }], "one in flight");
  done.shift()!();
  await new Promise((r) => setTimeout(r, 0));
  assert.deepEqual(sent, [{ v: "1" }, { v: "3" }], "then the newest, the stale one skipped");
});
