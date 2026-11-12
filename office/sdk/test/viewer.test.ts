// The viewer's pure parts (kit/office/*.js, plan §4) and the bundles' split (architect m_23781):
// office.js stays DOM-free (bise office run and the views load it); Writer's (plugins/docs) are
// tested in plugins/docs/test.
// The renderers import the bundles by their served path (/kit/office/…): their pure helpers
// are loaded here without those imports, so no DOM is needed.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..", "..");

/** The kit's own plain modules (no bundle, no DOM at load): imported from their files. */
const PLAIN = new Set(["frame.js"]);

/** A kit/office module with its bundle imports cut (its helpers need none); frame.js kept. */
async function helpers(name: string) {
  const src = readFileSync(join(root, "kit", "office", name), "utf8").replace(/^import (.*) from "\/kit\/office\/([^"]+)";$/gm, (all, what, file) =>
    PLAIN.has(file) ? `import ${what} from ${JSON.stringify(pathToFileURL(join(root, "kit", "office", file)).href)};` : "",
  );
  return import(`data:text/javascript;base64,${Buffer.from(src).toString("base64")}`);
}

test("office.js loads with no window or document, and has no docx-preview in it", async () => {
  assert.equal(typeof Reflect.get(globalThis, "window"), "undefined");
  assert.equal(typeof Reflect.get(globalThis, "document"), "undefined");
  const office = await import("../../dist/office.js");
  assert.equal(typeof office.sheets.open, "function");
  const js = readFileSync(join(root, "office", "dist", "office.js"), "utf8");
  assert.ok(!js.includes("docx-preview"), "docx-preview lives in Writer's office-view.js only (plugins/docs)");
});

// art-notes: the notes layer's pure parts (kit/office/select.js, kit/notes-anchors.js)
test("a note's anchor says its place in bise_proto's words (anchor_label.json)", async () => {
  const { createRequire } = await import("node:module");
  createRequire(import.meta.url)(join(root, "kit", "notes-anchors.js"));
  const rows = JSON.parse(readFileSync(join(root, "rust", "proto", "fixtures", "anchor_label.json"), "utf8"));
  const { label } = Reflect.get(globalThis, "BiseAnchors");
  for (const r of rows) assert.equal(label(r.anchor), r.label, JSON.stringify(r.anchor));
  const { message, typed } = await helpers("select.js");
  const m = message({ id: "q3-xlsx", version: 4, rev: 2 }, { kind: "cell", sheet: "Q3", range: "B4:D9" }, "1200 1350");
  // exactly its tag and the four facts (architect m_23971), nothing else from the frame
  assert.deepEqual(Object.keys(m).sort(), ["anchor", "artifact", "bise", "quote", "version"]);
  assert.deepEqual(m, { bise: "anchor", artifact: "q3-xlsx", version: 4, anchor: { kind: "cell", sheet: "Q3", range: "B4:D9" }, quote: "1200 1350" });
  // his first typed character (architect m_24155): {bise, artifact, version, anchor, quote, key?}
  assert.deepEqual(Object.keys(message({ id: "q", version: 1 }, { kind: "slide", slide: 1 }, "", "w")).sort(), ["anchor", "artifact", "bise", "key", "quote", "version"]);
  const k = (key: string, o = {}) => typed({ key, ...o });
  assert.deepEqual([k("w"), k(" "), k("é"), k("👍🏽"), k("Enter"), k("Dead"), k("w", { metaKey: true }), k("w", { ctrlKey: true }), k("w", { altKey: true }), k("w", { isComposing: true }), k("\t")], ["w", " ", "é", "👍🏽", null, null, null, null, null, null, null]);
});

test("marks: sent and drafts marked, ✓ only on the version that answered, then gone", async () => {
  const { markOf, quoteOf } = await helpers("select.js");
  assert.equal(markOf({ status: "sent" }, 3), "sent");
  assert.equal(markOf({ status: "draft" }, 3), "draft");
  assert.equal(markOf({ status: "done", done_in: 4 }, 4), "done");
  assert.equal(markOf({ status: "done", done_in: 4 }, 5), null);
  assert.equal(markOf({ status: "done", done_in: 4 }, 3), null);
  assert.equal(quoteOf("  a\n b  "), "a b");
  assert.equal(quoteOf("x".repeat(300)).length, 200);
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
