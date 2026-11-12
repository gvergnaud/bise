// Preview (plugins/media): its pure parts, on the emitted .js beside their .ts. node:test, no deps
// (gate.sh quick's gallery_plugins). Moved from office/sdk/test/viewer.test.ts with the
// renderers (image, pdf, video), plus the anchor keys and the old notes' shape (architect m_60380).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const view = join(here, "..", "ai.mistral.vibe", "views", "preview");
const load = (name) => import(pathToFileURL(join(view, name)).href);

/** A module with pdf.js's bundle import cut (its helpers need none, and no DOM). */
async function withoutPdfjs(name) {
  const src = readFileSync(join(view, name), "utf8").replace(/^import (.*) from "\.\/([^"]+)";$/gm, (all, what, file) =>
    file === "pdfjs.js" ? "" : `import ${what} from ${JSON.stringify(pathToFileURL(join(view, file)).href)};`,
  );
  return import(`data:text/javascript;base64,${Buffer.from(src).toString("base64")}`);
}

test("a place's key is its JSON with sorted keys, no words in it, and parses back", async () => {
  const { keyOf, placeOf, canonical } = await load("keys.js");
  const box = { kind: "region", rect: { x: 10, y: 5, w: 30, h: 20 } };
  assert.equal(keyOf(box), '{"kind":"region","rect":{"h":20,"w":30,"x":10,"y":5}}');
  assert.deepEqual(placeOf(keyOf(box)), box);
  const page = { kind: "region", page: 3, rect: { x: 1.5, y: 2, w: 3, h: 4 } };
  assert.deepEqual(placeOf(keyOf(page)), page);
  assert.equal(keyOf({ kind: "time", t: 12.5 }), '{"kind":"time","t":12.5}');
  assert.deepEqual(placeOf('{"kind":"time","rect":{"h":1,"w":2,"x":3,"y":4},"t":0}'), { kind: "time", t: 0, rect: { x: 3, y: 4, w: 2, h: 1 } });
  assert.equal(canonical({ b: 1, a: [{ d: 2, c: undefined }] }), '{"a":[{"d":2}],"b":1}');
});

test("an older note's typed anchor, as the hub sends it (as_view_key), places as a new one", async () => {
  const { placeOf } = await load("keys.js");
  // serde_json writes a whole f64 as 10.0: the same number to JSON.parse
  assert.deepEqual(placeOf('{"kind":"region","page":2,"rect":{"h":3.0,"w":50.0,"x":10.0,"y":20.0}}'), { kind: "region", page: 2, rect: { x: 10, y: 20, w: 50, h: 3 } });
  assert.deepEqual(placeOf('{"kind":"time","t":12.0}'), { kind: "time", t: 12 });
});

test("a key this view can't read is no place (its note shows detached, never dropped)", async () => {
  const { placeOf } = await load("keys.js");
  for (const k of ["", "line:3", "{", "[]", "null", '{"kind":"block","block":"b2"}', '{"kind":"region"}', '{"kind":"region","rect":{"x":-1,"y":0,"w":1,"h":1}}', '{"kind":"region","page":0,"rect":{"x":0,"y":0,"w":1,"h":1}}', '{"kind":"time","t":-1}', '{"kind":"time","t":"3"}', "x".repeat(513)]) {
    assert.equal(placeOf(k), null, k);
  }
});

test("a box drawn is a region in % of what it's on; a click is no box", async () => {
  const { boxPct, isClick, clock } = await load("keys.js");
  assert.deepEqual(boxPct({ x: 300, y: 100 }, { x: 100, y: 50 }, 400, 200), { x: 25, y: 25, w: 50, h: 25 });
  assert.deepEqual(boxPct({ x: -10, y: 0 }, { x: 500, y: 300 }, 400, 200), { x: 0, y: 0, w: 100, h: 100 });
  assert.ok(isClick({ x: 1, y: 1, w: 0.5, h: 30 }) && !isClick({ x: 1, y: 1, w: 1, h: 1 }));
  assert.deepEqual([clock(0), clock(12.9), clock(725), clock(3725), clock(NaN)], ["0:00", "0:12", "12:05", "1:02:05", "0:00"]);
});

test("Preview's kinds by the file's name, and how each reads its file", async () => {
  const { kindOf, wayOf, words, READ_MAX } = await load("kind.js");
  assert.deepEqual(["a.PNG", "b.jpeg", "c.webp", "d.gif", "r.pdf", "f.MOV", "g.m4v", "h.webm", "i.mp4"].map(kindOf), ["image", "image", "image", "image", "pdf", "video", "video", "video", "video"]);
  assert.deepEqual(["notes.md", "x.mkv", "noext", "a.pdf.zip"].map(kindOf), [null, null, null, null]);
  assert.equal(wayOf("video", 10), "url", "a video plays by its ranges, never read whole");
  assert.equal(wayOf("pdf", READ_MAX), "bytes");
  assert.equal(wayOf("pdf", READ_MAX + 1), "url", "a big pdf is read by its ranges");
  assert.equal(wayOf("image", READ_MAX + 1), "bytes");
  assert.match(words({ code: -32041 }, "a.png"), /a\.png is gone/);
  assert.match(words({ code: "unreadable", message: "not an image this browser draws" }, "a.png"), /can't be shown/);
});

test("view.json: one opener, Preview, for every kind kind.js draws", async () => {
  const { kindOf } = await load("kind.js");
  const v = JSON.parse(readFileSync(join(view, "view.json"), "utf8"));
  const [entry] = v._meta["ai.mistral.vibe/ui"].entrypoints;
  assert.equal(entry.title, "Preview");
  const exts = entry.extensions;
  assert.ok(exts.every((e) => kindOf(`x${e}`) !== null), "every ending it claims is drawn");
  const globs = v._meta["dev.bise/ui"].files.flatMap((f) => f.match);
  assert.deepEqual(globs.map((g) => g.slice(1)).sort(), [...exts].sort(), "Vibe's endings and bise's globs name the same types");
  assert.equal(v._meta["dev.bise/ui"].write, undefined, "read only: no write grant");
  // each type keeps its own icon (a ds icon name, designer m_62222): the card shows it
  assert.deepEqual(v._meta["dev.bise/ui"].files.map((f) => f.icon), ["image", "doc", "video"]);
});

// image-pinch (his m_22822; architect m_30628): the zoom's math. A 2000×1000 image in a 500×500
// box: fit is 25% (500×250), centred with 125 px above and under.
const D = { bw: 500, bh: 500, nw: 2000, nh: 1000 };

test("an image fits whole, never above 100%, centred", async () => {
  const { fitK, fitted } = await load("zoom.js");
  assert.equal(fitK(D), 0.25);
  assert.deepEqual(fitted(D), { k: 0.25, x: 0, y: 125 });
  assert.equal(fitK({ bw: 500, bh: 500, nw: 100, nh: 50 }), 1);
});

test("a zoom keeps the image point under the pointer, held between fit and 8×", async () => {
  const { fitted, isFit, MAX_K, zoomAt } = await load("zoom.js");
  const z = zoomAt(D, fitted(D), 1, 250, 250);
  assert.ok(Math.abs((250 - z.x) / z.k - 1000) < 1e-6);
  assert.ok(Math.abs((250 - z.y) / z.k - 500) < 1e-6);
  assert.equal(zoomAt(D, z, 100, 250, 250).k, MAX_K);
  assert.ok(isFit(D, zoomAt(D, z, 0.01, 250, 250)));
});

test("a zoomed image pans, never past an edge; shift+wheel goes sideways", async () => {
  const { fitted, panBy, wheelPan, zoomAt } = await load("zoom.js");
  const z = zoomAt(D, fitted(D), 1, 0, 0);
  assert.deepEqual(panBy(D, z, 100, 100), { k: 1, x: 0, y: 0 });
  assert.deepEqual(panBy(D, z, -5000, -5000), { k: 1, x: -1500, y: -500 });
  assert.deepEqual(wheelPan(3, 10, 0, false), { dx: -3, dy: -10 });
  assert.deepEqual(wheelPan(0, 10, 0, true), { dx: -10, dy: -0 });
  assert.deepEqual(wheelPan(0, 1, 1, false), { dx: -0, dy: -16 });
});

test("a pinch out zooms in, a pinch in zooms out; lines count as 16 px", async () => {
  const { pinchFactor } = await load("zoom.js");
  assert.ok(pinchFactor(-10, 0) > 1);
  assert.ok(pinchFactor(10, 0) < 1);
  assert.ok(Math.abs(pinchFactor(1, 1) - pinchFactor(16, 0)) < 1e-9);
});

test("a double-click goes to 100% and back to fit; a small image to 2×", async () => {
  const { fitted, toggled, zoomWords } = await load("zoom.js");
  const z = toggled(D, fitted(D), 250, 250);
  assert.equal(z.k, 1);
  assert.deepEqual(toggled(D, z, 10, 10), fitted(D));
  const small = { bw: 500, bh: 500, nw: 100, nh: 50 };
  assert.equal(toggled(small, fitted(small), 250, 250).k, 2);
  assert.equal(zoomWords(z), "100%");
});

test("esc and ⌘0 take a zoomed image back to fit; ⌥⌘0 and ⌘esc don't", async () => {
  const { isFitKey } = await load("zoom.js");
  const key = (o) => ({ key: "", code: "", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...o });
  assert.ok(isFitKey(key({ key: "Escape" })));
  assert.ok(isFitKey(key({ code: "Digit0", metaKey: true })));
  assert.ok(!isFitKey(key({ code: "Digit0", metaKey: true, altKey: true })));
  assert.ok(!isFitKey(key({ key: "Escape", metaKey: true })));
  assert.ok(!isFitKey(key({ code: "Digit0" })));
});

// pdf-notes: where a note sits on a pdf (pdftext.js), architect m_24668
const items = (...lines) => lines.flatMap((l) => l.split("|").map((str, i, all) => ({ str, hasEOL: i === all.length - 1 })));

test("a pdf page's text: its items in order, a line's end a space, each item's place in it", async () => {
  const { pageText, itemsIn } = await load("pdftext.js");
  const t = pageText(items("Net revenue | grew 4%", "in  Q3."));
  assert.equal(t.text, "Net revenue grew 4% in Q3.");
  assert.deepEqual(t.spans, [{ i: 0, start: 0, end: 11 }, { i: 1, start: 12, end: 19 }, { i: 2, start: 20, end: 26 }]);
  assert.deepEqual(itemsIn(t.spans, 4, 16), [0, 1]);
  assert.deepEqual(pageText([{ str: "", hasEOL: true }, { str: "  " }]), { text: "", spans: [] });
});

test("a pdf note on a new version: its words where they are, a box on its page, else detached", async () => {
  const { place, pageText } = await load("pdftext.js");
  const v1 = [pageText(items("Summary")).text, pageText(items("Net revenue grew 4% in Q3.", "Costs held.")).text];
  const passage = { page: 2, rect: { x: 10, y: 20, w: 50, h: 3 } };
  const q = "revenue grew 4%";
  const box = { page: 2, rect: { x: 5, y: 5, w: 20, h: 20 } };
  assert.deepEqual(place(passage, q, v1), { page: 2, start: 4, end: 19 });
  assert.deepEqual(place(box, "", v1), { page: 2, rect: box.rect });
  // a page put in front: the words are found on page 3, the box stays on page 2
  const v2 = [v1[0], "A new page", v1[1]];
  assert.equal(place(passage, q, v2).page, 3);
  assert.deepEqual(place(box, "", v2), { page: 2, rect: box.rect });
  // the words gone, or an old note's page no more (architect m_60380 rule 4): null, shown detached
  assert.equal(place(passage, q, [v1[0], "Costs held."]), null);
  assert.equal(place(box, "", [v1[0]]), null);
  // the words on two pages: the one nearest its page; twice on its page: nearest its old box
  assert.equal(place(passage, q, ["revenue grew 4%", "x", "revenue grew 4%", "y"]).page, 1);
  const twice = "revenue grew 4% " + "filler ".repeat(40) + "revenue grew 4%";
  assert.equal(place({ ...passage, rect: { ...passage.rect, y: 95 } }, q, ["", twice]).start, twice.lastIndexOf(q));
  assert.equal(place({ ...passage, rect: { ...passage.rect, y: 2 } }, q, ["", twice]).start, 0);
  // no page: page 1
  assert.deepEqual(place({ rect: box.rect }, "", ["a"]), { page: 1, rect: box.rect });
});

test("pdf.js runs a pdf with no eval, no pdf JavaScript, nothing fetched but the file (architect m_24668)", async () => {
  const { OPTIONS, rectPct, footOf } = await withoutPdfjs("pdf.js");
  assert.equal(OPTIONS.isEvalSupported, false);
  assert.equal(OPTIONS.enableScripting, false);
  assert.equal(OPTIONS.enableXfa, false);
  assert.equal(OPTIONS.useWorkerFetch, false);
  for (const k of ["cMapUrl", "standardFontDataUrl", "wasmUrl"]) assert.equal(OPTIONS[k], null, k);
  assert.ok(Object.isFrozen(OPTIONS));
  const src = readFileSync(join(view, "pdf.js"), "utf8");
  assert.match(src, /getDocument\(\{ \.\.\.OPTIONS, \.\.\.from, BinaryDataFactory: Unserved \}\)/, "the options and the factory that never fetches");
  assert.match(src, /annotationMode: 0/, "no annotation layer: a link never navigates the frame");
  assert.doesNotMatch(src, /https?:\/\//, "no CDN");
  assert.match(src, /new URL\("\.\/pdf\.worker\.js", import\.meta\.url\)/, "its worker from its own folder, never a blob");
  assert.deepEqual(rectPct({ left: 150, top: 120, width: 100, height: 20 }, { left: 100, top: 100, width: 400, height: 200 }), { x: 12.5, y: 10, w: 25, h: 10 });
  assert.deepEqual(rectPct({ left: 0, top: 0, width: 900, height: 900 }, { left: 100, top: 100, width: 400, height: 200 }), { x: 0, y: 0, w: 100, h: 100 });
  assert.deepEqual(footOf(new Set()), []);
  assert.equal(footOf(new Set(["cMapUrl", "standardFontDataUrl", "wasmUrl"])).length, 3);
});

// video-view: the player's states, keys and moments (video.js), the words of its time
test("a video's player: its states, its keys, its moments", async () => {
  const { stateOf, keyAction, stepped, seekAt, nextRate, near, timeWords, screenHeight, RATES, JUMP_S } = await load("video.js");
  assert.deepEqual([screenHeight(737, 1920, 1080), screenHeight(400, 1080, 1920), screenHeight(0, 16, 9), screenHeight(500, 0, 0)], [415, 711, 0, 0]);
  const m = (o) => ({ error: null, readyState: 4, ended: false, paused: true, ...o });
  assert.deepEqual(
    [m({ readyState: 0 }), m({}), m({ paused: false }), m({ ended: true, paused: true }), m({ error: { code: 4 }, paused: false })].map(stateOf),
    ["loading", "paused", "playing", "ended", "failed"],
  );
  const k = (key, o = {}) => keyAction({ key, ...o });
  assert.deepEqual(k(" "), { do: "toggle" });
  assert.deepEqual(keyAction({ key: "", code: "Space" }), { do: "toggle" });
  assert.deepEqual([k("ArrowLeft"), k("ArrowRight")], [{ do: "jump", by: -JUMP_S }, { do: "jump", by: JUMP_S }]);
  assert.deepEqual([k(","), k(".")], [{ do: "step", by: -1 }, { do: "step", by: 1 }]);
  for (const e of [k(" ", { metaKey: true }), k("ArrowLeft", { altKey: true }), k(".", { ctrlKey: true }), k(" ", { typing: true }), k(",", { isComposing: true }), k("f"), k("Escape")]) assert.equal(e, null);
  assert.equal(stepped(3, -JUMP_S, 90), 0);
  assert.equal(stepped(88, JUMP_S, 90), 90);
  assert.equal(stepped(10, 1, 90, 1 / 25), 10.04, "one frame of a 25 fps file");
  assert.equal(stepped(0, -1, 90, 1 / 30), 0);
  assert.equal(stepped(4, 5, NaN), 9, "no duration yet: never below 0, no end");
  assert.equal(seekAt(50, 200, 80), 20);
  assert.deepEqual([seekAt(-5, 200, 80), seekAt(300, 200, 80), seekAt(10, 0, 80), seekAt(10, 200, NaN)], [0, 80, 0, 0]);
  assert.deepEqual(RATES.map(nextRate), [1.5, 2, 0.5, 1]);
  assert.equal(nextRate(3), 1);
  assert.ok(near(12.3, 12) && !near(13, 12));
  assert.equal(timeWords(12.6, 90), "0:12 / 1:30");
  assert.equal(timeWords(3725, 7200), "1:02:05 / 2:00:00");
  assert.equal(timeWords(4, NaN), "0:04");
});

test("the view loads only its own folder and bise's kit: no inline script, no other origin", async () => {
  const html = readFileSync(join(view, "index.html"), "utf8");
  assert.doesNotMatch(html, /<script>(?!<)/, "no inline script: the view's CSP refuses one");
  assert.doesNotMatch(html, /https?:\/\/|src="\//, "relative to the base the hub writes");
  assert.match(html, /href="~kit\/view\/view\.css"/);
  for (const f of ["preview.js", "image.js", "video.js", "pdf.js"]) assert.doesNotMatch(readFileSync(join(view, f), "utf8"), /from "\/|import\("\//, `${f}: no absolute import`);
});
