import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { inlineKit, kitParts, MARKER } from "../inline.mjs";

test("the kit goes in at the marker, once, whole", () => {
  const html = `<html><head>${MARKER}</head><body></body></html>`;
  const out = inlineKit(html);
  assert.ok(!out.includes(MARKER));
  assert.ok(out.includes("--k-series-8"), "the CSS is in");
  assert.ok(out.includes("window.kit ="), "the script is in");
  assert.equal(out.split("<style data-kit>").length, 2);
});

test("a $ in the kit is kept as is (no replacement patterns)", () => {
  const out = inlineKit(`<head>${MARKER}</head>`, { css: "a{content:'$&'}", js: "x='$1'" });
  assert.ok(out.includes("content:'$&'") && out.includes("x='$1'"));
});

test("a view with no marker is refused", () => {
  assert.throws(() => inlineKit("<head></head>"), /marker/);
});

test("a kit that would close its own tag is refused", () => {
  assert.throws(() => inlineKit(MARKER, { css: "</style>", js: "" }), /closing tag/);
});

test("every color of the kit has a light and a dark value, every spec variable a fallback", () => {
  const { css } = kitParts();
  for (const m of css.matchAll(/var\((--(?:color|font|border-radius|shadow)-[a-z0-9-]+)(,?)/g)) {
    assert.equal(m[2], ",", `${m[1]} has no fallback`);
  }
  for (const m of css.matchAll(/--k-series-\d: ([^;]+);/g)) assert.match(m[1], /^light-dark\(/);
});

test("the kit's script draws nothing into innerHTML unescaped but the tip's own content", () => {
  const js = readFileSync(new URL("../kit.js", import.meta.url), "utf8");
  const sinks = [...js.matchAll(/innerHTML\s*=\s*([^;]+);/g)].map((m) => m[1].trim());
  assert.deepEqual(sinks, ["html"], "the tip's html is the caller's, escaped by kit.esc");
});
