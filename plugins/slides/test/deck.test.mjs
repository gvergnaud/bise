// Slides' deck (plugins/slides/ai.mistral.vibe/views/slides/deck.js): the pure helpers: a box in %
// of its slide, and the anchors (architect m_60380: the old viewer's anchors minus their quote,
// in sorted keys, so the notes made before place). deck.js imports the office SDK by its served
// path (~kit/office/v1/), so its helpers are loaded here with that import cut.
//   node --test plugins/slides/test/deck.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

async function helpers() {
  const src = readFileSync(join(root, "plugins", "slides", "ai.mistral.vibe", "views", "slides", "deck.js"), "utf8").replace(/^import .* from "[^"]+";$/gm, "");
  return import(`data:text/javascript;base64,${Buffer.from(src).toString("base64")}`);
}

test("a slide's boxes are placed in % of the slide", async () => {
  const { pct } = await helpers();
  assert.deepEqual(pct([72, 54, 480, 270], 960, 540), { left: 7.5, top: 10, width: 50, height: 50 });
});

test("a shape's and a slide's anchors: the old viewer's minus the quote, keys sorted", async () => {
  const { shapeKey, slideKey, anchorOf } = await helpers();
  assert.equal(shapeKey(2, "4"), '{"kind":"shape","shape":"4","slide":2}');
  assert.equal(slideKey(3), '{"kind":"slide","slide":3}');
  assert.deepEqual(anchorOf(shapeKey(2, "4")), { slide: 2, shape: "4" });
  assert.deepEqual(anchorOf(slideKey(3)), { slide: 3 });
  // a box drawn on a slide (the old viewer's) places on its slide
  assert.deepEqual(anchorOf('{"kind":"slide","rect":{"h":10,"w":10,"x":5,"y":5},"slide":1}'), { slide: 1 });
  // not a deck's: none
  for (const k of ['{"block":"b1","kind":"block"}', '{"kind":"slide","slide":0}', '{"kind":"shape","slide":1}', '{"kind":"shape","shape":4,"slide":1}', "x"]) {
    assert.equal(anchorOf(k), null, k);
  }
});
