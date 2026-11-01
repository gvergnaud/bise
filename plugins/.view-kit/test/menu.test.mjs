// kit.menu and .k-menu: exported, tokens only, items as text. Its behaviour (below or flipped
// up, ↑ ↓ ⏎ esc, outside click) is driven in a browser by the gallery's view shots.
import { test } from "node:test";
import assert from "node:assert/strict";
import { kitParts } from "../inline.mjs";

test("kit.menu is exported, and a menu's items are text, never html", () => {
  const { js } = kitParts();
  assert.match(js, /window\.kit = \{[^}]*\bmenu\b[^}]*\}/);
  const body = js.slice(js.indexOf("function menu("), js.indexOf("window.kit ="));
  assert.ok(body.includes("b.textContent = it.label"));
  assert.ok(!/innerHTML/.test(body));
  for (const key of ["ArrowDown", "ArrowUp", "Escape", "Home", "End"]) assert.ok(body.includes(`"${key}"`), key);
});

test(".k-menu uses the kit's tokens only (no raw color, no hand-picked size but its min width)", () => {
  const { css } = kitParts();
  const block = css.slice(css.indexOf("/* the menu (kit.js menu())"));
  assert.ok(block.includes(".k-menu {") && block.includes(".k-menu button"));
  assert.ok(!/#[0-9a-f]{3,8}\b|rgb\(/i.test(block), "no raw color");
  const px = [...block.matchAll(/(\d+)px/g)].map((m) => m[1]);
  assert.deepEqual(px, ["120"], "only the min width is in px");
});
