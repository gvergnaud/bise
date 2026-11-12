// The viewer's pure parts (kit/office/*.js, plan §4) and the bundles' split (architect m_23781):
// office.js stays DOM-free (bise office run and the views load it); Writer's and Slides' (plugins/docs,
// plugins/slides) are tested in their plugins' test/.
// The renderers import the bundles by their served path (/kit/office/…): their pure helpers
// are loaded here without those imports, so no DOM is needed.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import type { UWorkbook } from "../../dist/office.js";
import { list, must } from "./must.ts";

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

test("the grid names cells and ranges as the sheets module does", async () => {
  const { parseRef, colName, rangeOf } = await helpers("sheet.js");
  assert.deepEqual(parseRef("B4"), { col: 2, row: 4 });
  assert.deepEqual(parseRef("$AA$10"), { col: 27, row: 10 });
  assert.equal(parseRef("Sheet1"), null);
  assert.deepEqual([1, 26, 27, 52, 703].map(colName), ["A", "Z", "AA", "AZ", "AAA"]);
  assert.equal(rangeOf({ col: 4, row: 9 }, { col: 2, row: 4 }), "B4:D9");
  assert.equal(rangeOf({ col: 2, row: 4 }, { col: 2, row: 4 }), "B4");
});

// xlsx-univer: his edits saved (pages/artifact/save.rs reads this body)
test("a save is its header line then the file's bytes", async () => {
  const { saveBody } = await helpers("sheet.js");
  const body = saveBody({ base: 3, rev: 2, session: "s1", seq: 1, force: false }, new Uint8Array([80, 75, 3, 4]));
  const nl = body.indexOf(10);
  assert.deepEqual(JSON.parse(new TextDecoder().decode(body.subarray(0, nl))), { base: 3, rev: 2, session: "s1", seq: 1, force: false });
  assert.deepEqual([...body.subarray(nl + 1)], [80, 75, 3, 4]);
});

test("the save's words: nothing at rest, then edited, saving, saved, or what he can do", async () => {
  const { saveState } = await helpers("sheet.js");
  assert.deepEqual(saveState({ kind: "idle" }), { text: "", actions: [] });
  assert.equal(saveState({ kind: "waiting" }).text, "edited");
  assert.equal(saveState({ kind: "saving" }).text, "saving…");
  assert.equal(saveState({ kind: "saved", v: 4 }).text, "saved · v4");
  const c = saveState({ kind: "conflict", v: 5, rev: 1 });
  assert.equal(c.text, "v5 came in while you edited");
  assert.deepEqual(c.actions.map((a: { text: string }) => a.text), ["keep mine", "take v5"]);
  assert.equal(saveState({ kind: "conflict", v: 5, rev: 1, disk: true }).text, "the file changed on disk while you edited");
  assert.deepEqual(saveState({ kind: "failed", why: "the file is gone" }).actions.map((a: { id: string }) => a.id), ["retry"]);
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
  const { markOf, inRange, quoteOf } = await helpers("select.js");
  assert.equal(markOf({ status: "sent" }, 3), "sent");
  assert.equal(markOf({ status: "draft" }, 3), "draft");
  assert.equal(markOf({ status: "done", done_in: 4 }, 4), "done");
  assert.equal(markOf({ status: "done", done_in: 4 }, 5), null);
  assert.equal(markOf({ status: "done", done_in: 4 }, 3), null);
  assert.ok(inRange("C5", "B4:D9") && inRange("B4", "B4") && !inRange("E5", "B4:D9") && !inRange("A1", "nope"));
  assert.ok(inRange("AA10", "Z9:AB12"));
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

// xlsx-univer: his resize and edit in the grid, saved by the viewer's save loop (sheet.js
// saver) on the SDK, with a fake grid (Univer needs a canvas) and a fake hub.
test("a column resized and a cell typed in the grid save as the next version, one per sitting", async () => {
  const sdk = pathToFileURL(join(root, "office", "dist", "office.js")).href;
  const src = readFileSync(join(root, "kit", "office", "sheet.js"), "utf8").replace('from "/kit/office/office.js"', `from ${JSON.stringify(sdk)}`);
  const { saver, SAVE_AFTER_MS } = await import(`data:text/javascript;base64,${Buffer.from(src).toString("base64")}`);
  const { sheets }: typeof import("../../dist/office.js") = await import(sdk);
  const start = await sheets.create({ sheet: "Q3", rows: [["Item", "Amount"], ["a", 1200], ["Total", "=SUM(B2:B2)"]], date: new Date(Date.UTC(2026, 0, 1)) });
  const book = await sheets.open(await start.save());
  // the grid: Univer's snapshot of the book, changed the way his gestures change it
  let snap = book.univer();
  const grid = { snapshot: () => JSON.parse(JSON.stringify(snap)) };
  type Sheet = UWorkbook["sheets"][string];
  const gesture = (fn: (s: Sheet) => unknown) => {
    snap = JSON.parse(JSON.stringify(snap));
    fn(snap.sheets.sheet1);
    save.changed();
  };
  const posts: { head: unknown; bytes: Uint8Array }[] = [];
  let answer = { status: 200, answer: { v: 2, rev: 1 } };
  const post = async (head: unknown, bytes: Uint8Array) => {
    posts.push({ head, bytes });
    return answer;
  };
  type Timer = { fn: () => unknown; ms: number };
  const timers: Timer[] = [];
  const fire = async () => await must(timers.pop(), "a waiting save").fn();
  const later = (fn: () => unknown, ms: number) => {
    const t = { fn, ms };
    timers.push(t);
    return () => timers.splice(timers.indexOf(t), 1);
  };
  const states: string[] = [];
  const info = { id: "q3", version: 1, rev: 1 };
  const save = saver({ book, grid, info, session: "s1", post, later, onState: (s: { kind: string }) => states.push(s.kind), take: () => {} });

  gesture((s) => (s.columnData[1] = { w: 180 })); // B dragged wider
  gesture((s) => (s.cellData[1][1] = { v: 5000, t: 2 })); // B2 typed
  gesture((s) => (s.cellData[2][1].v = 5000)); // the total, recomputed by the grid
  assert.equal(timers.length, 1, "one save waiting, pushed back by each edit");
  assert.equal(timers[0].ms, SAVE_AFTER_MS);
  await fire();
  assert.deepEqual(states, ["waiting", "waiting", "waiting", "saving", "saved"]);
  assert.equal(posts.length, 1);
  assert.deepEqual(posts[0].head, { base: 1, rev: 1, session: "s1", seq: 1, force: false });
  const o = (await sheets.open(posts[0].bytes)).inspect();
  const cell = (id: string) => must(must(o.nodes[0]?.children, "Q3's cells").find((c) => c.id === id)?.props, id);
  assert.equal(list(must(o.nodes[0]?.props, "Q3's props").widths, "widths")[1], 180, "the width he dragged");
  assert.equal(cell("Q3!B2").value, 5000, "the value he typed");
  assert.deepEqual([cell("Q3!B3").formula, cell("Q3!B3").value], ["=SUM(B2:B2)", 5000], "the formula kept, its result the grid's");
  assert.deepEqual(info, { id: "q3", version: 2, latest: 2, rev: 1 });
  assert.ok(save.hold({ n: 2, rev: 1 }), "his own save comes back on the stream: not drawn again");

  // the next edit of the sitting: seq 2, from the version he made
  answer = { status: 200, answer: { v: 2, rev: 2 } };
  gesture((s) => (s.rowData[0] = { h: 40 }));
  await fire();
  assert.deepEqual(posts[1].head, { base: 2, rev: 1, session: "s1", seq: 2, force: false });

  // an agent's v3 lands while he has an edit not saved: he picks, nothing is overwritten
  gesture((s) => (s.cellData[0][0] = { v: "Line", t: 1 }));
  assert.ok(save.hold({ n: 3, rev: 1 }), "not drawn over his edit");
  assert.deepEqual(save.state, { kind: "conflict", v: 3, rev: 1 });
  await fire();
  assert.equal(posts.length, 2, "no autosave while he has not picked");
  answer = { status: 200, answer: { v: 4, rev: 1 } };
  await save.keep();
  assert.deepEqual(posts[2].head, { base: 3, rev: 1, session: "s1", seq: 3, force: false }, "keep mine: saved over v3");
  assert.equal(save.state.kind, "saved");
});

