// Sites (plugins/sites): its pure parts, on the emitted .js beside their .ts. node:test, no deps.
// The anchor keys (and an older site note's typed anchor, architect m_60380), the link rule and
// the kept state.
import { test } from "node:test";
import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const view = join(here, "..", "ai.mistral.vibe", "views", "site");
const load = (name) => import(pathToFileURL(join(view, name)).href);

test("a block's key is its JSON with sorted keys, its words folded, and parses back", async () => {
  const { keyOf, placeOf } = await load("keys.js");
  const p = { kind: "block", block: "#intro", quote: "  net\n revenue " };
  assert.equal(keyOf(p), '{"block":"#intro","kind":"block","quote":"net revenue"}');
  assert.deepEqual(placeOf(keyOf(p)), { kind: "block", block: "#intro", quote: "net revenue" });
  assert.equal(keyOf({ kind: "block", block: "body>p:nth-of-type(2)", quote: "" }), '{"block":"body>p:nth-of-type(2)","kind":"block"}');
  assert.deepEqual(placeOf('{"block":"body","kind":"block"}'), { kind: "block", block: "body" });
});

test("an older site note's typed anchor (as_view_key of Anchor::Block) places as it was", async () => {
  const { placeOf } = await load("keys.js");
  // the hub's canonical key of {kind: block, block, quote}: sorted keys, no whitespace
  assert.deepEqual(placeOf('{"block":"#plan>li:nth-of-type(3)","kind":"block","quote":"ship friday"}'), {
    kind: "block",
    block: "#plan>li:nth-of-type(3)",
    quote: "ship friday",
  });
});

test("a key it can't read is no place (the note shows detached, never dropped)", async () => {
  const { placeOf, KEY_MAX } = await load("keys.js");
  for (const k of ["", "nope", "[]", "null", '{"kind":"region","rect":{"x":1,"y":1,"w":1,"h":1}}', '{"kind":"block","block":""}', '{"kind":"block","block":7}', '{"kind":"block","block":"p","quote":3}', 7, null]) {
    assert.equal(placeOf(k), null, String(k));
  }
  assert.equal(placeOf(`{"kind":"block","block":"${"a".repeat(KEY_MAX)}"}`), null, "too long");
});

test("a key never passes KEY_MAX: the quote is cut, a path too long is no key", async () => {
  const { keyOf, KEY_MAX, BLOCK_MAX } = await load("keys.js");
  const k = keyOf({ kind: "block", block: "b".repeat(BLOCK_MAX), quote: "word ".repeat(400) });
  assert.ok(k.length <= KEY_MAX, `${k.length}`);
  assert.equal(keyOf({ kind: "block", block: "b".repeat(BLOCK_MAX + 1) }), null);
  assert.equal(keyOf({ kind: "block", block: "" }), null);
});

test("a link is the site's when it is on its origin under its base", async () => {
  const { inside } = await load("where.js");
  const base = "http://v-ab.localhost:47438/h/s1/t1/";
  assert.equal(inside("http://v-ab.localhost:47438/h/s1/t1/about.html", base), true);
  assert.equal(inside("http://v-ab.localhost:47438/h/s1/t1/", base), true);
  assert.equal(inside("http://v-ab.localhost:47438/h/s1/t2/x.html", base), false, "another handle");
  assert.equal(inside("http://v-cd.localhost:47438/h/s1/t1/x.html", base), false, "another origin");
  assert.equal(inside("https://example.com/", base), false);
  assert.equal(inside("mailto:a@b.c", base), false);
  assert.equal(inside("not a url", base), false);
});

test("the kept state comes back only for a page under this session's base", async () => {
  const { stateOf } = await load("where.js");
  const base = "http://v-ab.localhost:47438/h/s2/t2/";
  assert.deepEqual(stateOf({ path: "/h/s2/t2/about.html", y: 120.4 }, base), { path: "/h/s2/t2/about.html", y: 120 });
  assert.equal(stateOf({ path: "/h/s1/t1/about.html", y: 5 }, base), null, "an older session's");
  assert.deepEqual(stateOf({ path: "/h/s2/t2/", y: -3 }, base), { path: "/h/s2/t2/", y: 0 });
  for (const v of [null, undefined, 3, "x", { path: 3, y: 1 }, { path: "/h/s2/t2/", y: Infinity }, { path: "/h/s2/t2/" }]) {
    assert.equal(stateOf(v, base), null, JSON.stringify(v));
  }
});
