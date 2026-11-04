// The viewer's pure parts (kit/office/*.js, plan §4) and the bundles' split (architect m_23781):
// office.js stays DOM-free (bise office run loads it), office-view.js carries docx-preview.
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
const PLAIN = new Set(["frame.js", "zoom.js"]);

/** A kit/office module with its bundle imports cut (its helpers need none); zoom.js and frame.js kept. */
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
  assert.ok(!js.includes("docx-preview"), "docx-preview lives in office-view.js only");
  const view = readFileSync(join(root, "office", "dist", "office-view.js"), "utf8");
  assert.match(view, /export\s*\{[^}]*renderDocx/, "office-view.js exports renderDocx");
  assert.ok(view.length < 2 * 1024 * 1024, "within its 2 MB budget");
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

test("a slide's boxes are placed in % of the slide", async () => {
  const { pct } = await helpers("slides.js");
  assert.deepEqual(pct([72, 54, 480, 270], 960, 540), { left: 7.5, top: 10, width: 50, height: 50 });
});

test("drawn paragraphs pair with the outline's, passing over what it skips", async () => {
  const { matchParas, parasOf } = await helpers("docx.js");
  const outline = parasOf([
    { id: "p1", type: "para", text: "Title" },
    { id: "t", type: "table", children: [{ id: "p2", type: "para", text: "cell  one" }, { id: "c", type: "comment" }] },
    { id: "p3", type: "para", text: "Body" },
  ]);
  type Para = { id: string; text: string };
  assert.deepEqual(outline.map((n: Para) => n.id), ["p1", "p2", "p3"], "a table's paragraphs in place, no comment");
  // a text box's paragraph drawn between them is passed over; whitespace does not count
  assert.deepEqual(matchParas(["Title", "in a text box", "cell one", "Body"], outline.map((n: Para) => n.text)), [0, -1, 1, 2]);
  // a paragraph docx-preview did not draw: the next ones still pair
  assert.deepEqual(matchParas(["Title", "Body"], outline.map((n: Para) => n.text)), [0, 2]);
});

// art-notes: the notes layer's pure parts (kit/office/select.js, image.js, kit/notes-anchors.js)
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

test("a box drawn on an image is a region in % of it", async () => {
  const { boxPct } = await helpers("image.js");
  assert.deepEqual(boxPct({ x: 300, y: 100 }, { x: 100, y: 50 }, 400, 200), { x: 25, y: 25, w: 50, h: 25 });
  assert.deepEqual(boxPct({ x: -10, y: 0 }, { x: 500, y: 300 }, 400, 200), { x: 0, y: 0, w: 100, h: 100 });
});

// image-pinch (his m_22822; architect m_30628): the zoom's math, moved from the desktop's
// zoom.test.ts case for case. A 2000×1000 image in a 500×500 box: fit is 25% (500×250), centred
// with 125 px above and under.
const D = { bw: 500, bh: 500, nw: 2000, nh: 1000 };
const zoom = () => import(pathToFileURL(join(root, "kit", "office", "zoom.js")).href);

test("an image fits whole, never above 100%, centred", async () => {
  const { fitK, fitted } = await zoom();
  assert.equal(fitK(D), 0.25);
  assert.deepEqual(fitted(D), { k: 0.25, x: 0, y: 125 });
  assert.equal(fitK({ bw: 500, bh: 500, nw: 100, nh: 50 }), 1);
});

test("a zoom keeps the image point under the pointer, held between fit and 8×", async () => {
  const { fitted, isFit, MAX_K, zoomAt } = await zoom();
  const z = zoomAt(D, fitted(D), 1, 250, 250);
  // the point under (250, 250) was image (1000, 500): still under it at 100%
  assert.ok(Math.abs((250 - z.x) / z.k - 1000) < 1e-6);
  assert.ok(Math.abs((250 - z.y) / z.k - 500) < 1e-6);
  assert.equal(zoomAt(D, z, 100, 250, 250).k, MAX_K);
  assert.ok(isFit(D, zoomAt(D, z, 0.01, 250, 250)));
});

test("a zoomed image pans, never past an edge; shift+wheel goes sideways", async () => {
  const { fitted, panBy, wheelPan, zoomAt } = await zoom();
  const z = zoomAt(D, fitted(D), 1, 0, 0);
  assert.deepEqual(panBy(D, z, 100, 100), { k: 1, x: 0, y: 0 });
  assert.deepEqual(panBy(D, z, -5000, -5000), { k: 1, x: -1500, y: -500 });
  assert.deepEqual(wheelPan(3, 10, 0, false), { dx: -3, dy: -10 });
  assert.deepEqual(wheelPan(0, 10, 0, true), { dx: -10, dy: -0 });
  assert.deepEqual(wheelPan(0, 1, 1, false), { dx: -0, dy: -16 });
});

test("a pinch out zooms in, a pinch in zooms out; lines count as 16 px", async () => {
  const { pinchFactor } = await zoom();
  assert.ok(pinchFactor(-10, 0) > 1);
  assert.ok(pinchFactor(10, 0) < 1);
  assert.ok(Math.abs(pinchFactor(1, 1) - pinchFactor(16, 0)) < 1e-9);
});

test("a double-click goes to 100% and back to fit; a small image to 2×", async () => {
  const { fitted, toggled, zoomWords } = await zoom();
  const z = toggled(D, fitted(D), 250, 250);
  assert.equal(z.k, 1);
  assert.deepEqual(toggled(D, z, 10, 10), fitted(D));
  const small = { bw: 500, bh: 500, nw: 100, nh: 50 };
  assert.equal(toggled(small, fitted(small), 250, 250).k, 2);
  assert.equal(zoomWords(z), "100%");
});

test("esc and ⌘0 take a zoomed image back to fit; ⌥⌘0 and ⌘esc don't", async () => {
  const { isFitKey } = await zoom();
  const key = (o: Record<string, unknown>) => ({ key: "", code: "", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...o });
  assert.ok(isFitKey(key({ key: "Escape" })));
  assert.ok(isFitKey(key({ code: "Digit0", metaKey: true })));
  assert.ok(!isFitKey(key({ code: "Digit0", metaKey: true, altKey: true })), "⌥⌘0 stays the panel");
  assert.ok(!isFitKey(key({ key: "Escape", metaKey: true })));
  assert.ok(!isFitKey(key({ code: "Digit0" })));
});

test("image.js loads its zoom and the frame's kinds", async () => {
  const { boxPct } = await helpers("image.js");
  assert.equal(typeof boxPct, "function");
  const { FRAME } = await import(pathToFileURL(join(root, "kit", "office", "frame.js")).href);
  assert.deepEqual(FRAME, { anchor: "anchor", esc: "esc", zoom: "zoom", fit: "fit", size: "size", pinch: "pinch", pan: "pan", room: "room", storage: "site-storage", find: "find", found: "found" });
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

// pdf-notes: a pdf's notes (kit/notes-anchors.js's pdf, kit/office/pdf.js), architect m_24668
const anchors = async () => {
  const { createRequire } = await import("node:module");
  return createRequire(import.meta.url)(join(root, "kit", "notes-anchors.js")).pdf;
};
// a page's text items as pdf.js gives them: a line's words, its end
const items = (...lines: string[]) => lines.flatMap((l) => l.split("|").map((str, i, all) => ({ str, hasEOL: i === all.length - 1 })));

test("a pdf page's text: its items in order, a line's end a space, each item's place in it", async () => {
  const { pageText, itemsIn } = await anchors();
  const t = pageText(items("Net revenue | grew 4%", "in  Q3."));
  assert.equal(t.text, "Net revenue grew 4% in Q3.");
  assert.deepEqual(t.ranges, [{ i: 0, start: 0, end: 11 }, { i: 1, start: 12, end: 19 }, { i: 2, start: 20, end: 26 }]);
  assert.equal(t.text, Reflect.get(globalThis, "BiseAnchors").block.norm("Net revenue  grew 4%\nin  Q3."), "the same text a selection gives once collapsed");
  assert.deepEqual(itemsIn(t.ranges, 4, 16), [0, 1]);
  assert.deepEqual(pageText([{ str: "", hasEOL: true }, { str: "  " }]), { text: "", ranges: [] });
});

test("a pdf note re-anchors on a new version: its words where they are, a box on its page", async () => {
  const { place, pageText } = await anchors();
  const v1 = [pageText(items("Summary")).text, pageText(items("Net revenue grew 4% in Q3.", "Costs held.")).text];
  const passage = { kind: "region", page: 2, rect: { x: 10, y: 20, w: 50, h: 3 }, quote: "revenue grew 4%" };
  const box = { kind: "region", page: 2, rect: { x: 5, y: 5, w: 20, h: 20 } };
  // the same version: the same place
  assert.deepEqual(place(passage, v1), { page: 2, start: 4, end: 19 });
  assert.deepEqual(place(box, v1), { page: 2, rect: box.rect });
  // a page put in front: the words are found on page 3, the box stays on page 2 (a box never moves pages)
  const v2 = [v1[0], "A new page", v1[1]];
  assert.equal(place(passage, v2).page, 3);
  assert.deepEqual(place(box, v2), { page: 2, rect: box.rect });
  // the words gone: moved; the pdf shorter than its page: the box moved
  assert.equal(place(passage, [v1[0], "Costs held."]), null);
  assert.equal(place(box, [v1[0]]), null);
  // the words on two pages: the one nearest its page
  assert.equal(place(passage, ["revenue grew 4%", "x", "revenue grew 4%", "y"]).page, 1, "1 and 3 as near: the first");
  assert.equal(place({ ...passage, page: 1 }, ["revenue grew 4%", "x", "revenue grew 4%"]).page, 1);
  // the words twice on its page: the one nearest its old box
  const twice = "revenue grew 4% " + "filler ".repeat(40) + "revenue grew 4%";
  assert.equal(place({ ...passage, rect: { ...passage.rect, y: 95 } }, ["", twice]).start, twice.lastIndexOf("revenue grew 4%"));
  assert.equal(place({ ...passage, rect: { ...passage.rect, y: 2 } }, ["", twice]).start, 0);
  // an image's box (no page) is page 1; not a region: nothing
  assert.deepEqual(place({ kind: "region", rect: box.rect }, ["a"]), { page: 1, rect: box.rect });
  assert.equal(place({ kind: "cell", sheet: "Q3", range: "A1" }, v1), null);
});

test("pdf.js runs a pdf with no eval, no pdf JavaScript, nothing fetched (architect m_24668)", async () => {
  const { OPTIONS, rectPct, footOf } = await helpers("pdf.js");
  assert.equal(OPTIONS.isEvalSupported, false);
  assert.equal(OPTIONS.enableScripting, false);
  assert.equal(OPTIONS.enableXfa, false);
  assert.equal(OPTIONS.useWorkerFetch, false);
  for (const k of ["cMapUrl", "standardFontDataUrl", "wasmUrl"]) assert.equal(OPTIONS[k], null, k);
  assert.ok(Object.isFrozen(OPTIONS));
  const src = readFileSync(join(root, "kit", "office", "pdf.js"), "utf8");
  assert.match(src, /getDocument\(\{ \.\.\.OPTIONS, data: bytes\.slice\(\), BinaryDataFactory: Unserved \}\)/, "the options and the factory that never fetches");
  assert.match(src, /annotationMode: 0/, "no annotation layer: a link never navigates the frame");
  assert.doesNotMatch(src, /https?:\/\//, "no CDN");
  assert.match(src, /workerSrc = "\/kit\/office\/pdf\.worker\.js"/);
  // a selection's box in % of its page
  assert.deepEqual(rectPct({ left: 150, top: 120, width: 100, height: 20 }, { left: 100, top: 100, width: 400, height: 200 }), { x: 12.5, y: 10, w: 25, h: 10 });
  assert.deepEqual(rectPct({ left: 0, top: 0, width: 900, height: 900 }, { left: 100, top: 100, width: 400, height: 200 }), { x: 0, y: 0, w: 100, h: 100 });
  assert.deepEqual(footOf(new Set()), []);
  assert.equal(footOf(new Set(["cMapUrl", "standardFontDataUrl", "wasmUrl"])).length, 3);
  // the bundles: pdf.js and its worker, within their budgets, no eval of their own when told not to
  const pdf = readFileSync(join(root, "office", "dist", "office-pdf.js"), "utf8");
  const worker = readFileSync(join(root, "office", "dist", "pdf.worker.js"), "utf8");
  assert.match(pdf, /export\s*\{[^}]*getDocument/);
  assert.ok(pdf.length < 1024 * 1024 && worker.length < 1.5 * 1024 * 1024);
  assert.ok(!readFileSync(join(root, "office", "dist", "office.js"), "utf8").includes("pdfjs"), "pdf.js stays out of office.js");
});

// video-view: the player's states, keys and moments (kit/office/video.js), the words of its time
test("a video's player: its states, its keys, its moments", async () => {
  const { createRequire } = await import("node:module");
  createRequire(import.meta.url)(join(root, "kit", "notes-anchors.js"));
  const { stateOf, keyAction, stepped, seekAt, nextRate, near, timeWords, screenHeight, RATES, JUMP_S } = await helpers("video.js");
  // the screen at the video's own ratio (designer m_32678): 16:9 in 737 px is 415 px tall
  assert.deepEqual([screenHeight(737, 1920, 1080), screenHeight(400, 1080, 1920), screenHeight(0, 16, 9), screenHeight(500, 0, 0)], [415, 711, 0, 0]);
  const m = (o: Record<string, unknown>) => ({ error: null, readyState: 4, ended: false, paused: true, ...o });
  assert.deepEqual(
    [m({ readyState: 0 }), m({}), m({ paused: false }), m({ ended: true, paused: true }), m({ error: { code: 4 }, paused: false })].map(stateOf),
    ["loading", "paused", "playing", "ended", "failed"],
  );
  const k = (key: string, o = {}) => keyAction({ key, ...o });
  assert.deepEqual(k(" "), { do: "toggle" });
  assert.deepEqual(keyAction({ key: "", code: "Space" }), { do: "toggle" });
  assert.deepEqual([k("ArrowLeft"), k("ArrowRight")], [{ do: "jump", by: -JUMP_S }, { do: "jump", by: JUMP_S }]);
  assert.deepEqual([k(","), k(".")], [{ do: "step", by: -1 }, { do: "step", by: 1 }]);
  // his: a shortcut, a field he types in, a composition, any other key
  for (const e of [k(" ", { metaKey: true }), k("ArrowLeft", { altKey: true }), k(".", { ctrlKey: true }), k(" ", { typing: true }), k(",", { isComposing: true }), k("f"), k("Escape")]) assert.equal(e, null);
  // moves held in the file
  assert.equal(stepped(3, -JUMP_S, 90), 0);
  assert.equal(stepped(88, JUMP_S, 90), 90);
  assert.equal(stepped(10, 1, 90, 1 / 25), 10.04, "one frame of a 25 fps file");
  assert.equal(stepped(0, -1, 90, 1 / 30), 0);
  assert.equal(stepped(4, 5, NaN), 9, "no duration yet: never below 0, no end");
  assert.equal(seekAt(50, 200, 80), 20);
  assert.deepEqual([seekAt(-5, 200, 80), seekAt(300, 200, 80), seekAt(10, 0, 80), seekAt(10, 200, NaN)], [0, 80, 0, 0]);
  assert.deepEqual(RATES.map(nextRate), [1.5, 2, 0.5, 1], "1 → 1.5 → 2 → 0.5 → 1");
  assert.equal(nextRate(3), 1);
  assert.ok(near(12.3, 12) && !near(13, 12));
  assert.equal(timeWords(12.6, 90), "0:12 / 1:30");
  assert.equal(timeWords(3725, 7200), "1:02:05 / 2:00:00");
  assert.equal(timeWords(4, NaN), "0:04");
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

// md-artifact (architect m_38679): a .md artifact drawn as a document by kit/office/markdown.js on
// office-md.js (micromark + GFM, the thread's parser): its own bundle, DOM-free, raw html escaped
const BLOCK_TAGS = /<(h[1-6]|p|blockquote|ul|ol|li|table|tr|th|td|pre|hr)[\s>/]/g;

test("office-md.js draws gfm.md with the block structure the thread's Markdown.tsx draws", async () => {
  const { markdownHtml } = await import("../../dist/office-md.js");
  const html = markdownHtml(readFileSync(join(root, "office", "fixtures", "gfm.md"), "utf8"));
  const want = JSON.parse(readFileSync(join(root, "office", "fixtures", "gfm.blocks.json"), "utf8")).tags;
  assert.deepEqual([...html.matchAll(BLOCK_TAGS)].map((m) => m[1]), want);
  // raw html shows as text, an unsafe link loses its url
  assert.ok(!html.includes("<script") && !html.includes("<b>"), "raw html escaped");
  assert.ok(html.includes("&lt;script&gt;"));
  assert.doesNotMatch(markdownHtml("[x](javascript:alert(1))"), /javascript:/);
  const view = readFileSync(join(root, "office", "dist", "office-view.js"), "utf8");
  assert.ok(!view.includes("micromark"), "the docx bundle carries no markdown parser");
});

test("a markdown file's text and links: utf-8 without BOM, relative paths beside the copy, the web out", async () => {
  const { textOf, outUrl } = await helpers("markdown.js");
  assert.equal(textOf(new Uint8Array([0xef, 0xbb, 0xbf, 0x23, 0x20, 0xc3, 0xa9, 0x0d, 0x0a, 0x61])), "# é\na");
  const file = "/a/learn/3/learnings.md";
  const base = "http://127.0.0.1:4000/a/learn/view";
  assert.deepEqual(outUrl("https://bise.dev", file, base), { href: "https://bise.dev", out: true });
  assert.deepEqual(outUrl("mailto:g@x.y", file, base), { href: "mailto:g@x.y", out: true });
  assert.deepEqual(outUrl("shots/a.png", file, base), { href: "/a/learn/3/shots/a.png", out: false });
  assert.deepEqual(outUrl("#f01", file, base), { href: "#f01", out: false });
  assert.deepEqual(outUrl("artifact:deck@v2", file, base), { href: "artifact:deck@v2", out: false });
});
