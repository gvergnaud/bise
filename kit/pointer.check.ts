// bise kit · the hand on every thing he can click (his words: « chaque élément interactif, chaque
// bouton, chaque chose sur lequel tu peux cliquer ... devrait avoir un cursor pointer »), on the
// web surfaces outside the desktop app: the kit's pages, the artifact viewers, bise.dev.
// The kit owns the web's root rule (kit/kit.css: a control or a clickable role gets the pointer, a
// disabled one the arrow); each stylesheet a page here starts from carries the same two lines, and
// none puts the arrow back on a control. The desktop app has its own rule since bise fait mac
// (17242f0d0, apps/desktop/src/ds/css/base.css: macOS's arrow on controls, the hand on links only;
// ds/pointer.test.tsx keeps it), so this check no longer copies base.css. No browser, no deps.
//   node --experimental-strip-types kit/pointer.check.ts
// Mutation: the rule's line dropped from kit/kit.css (or any site page) turns it red.
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p: string) => readFileSync(join(root, p), "utf8");
const noComments = (s: string) => s.replace(/\/\*[\s\S]*?\*\//g, "");

let pass = 0, fail = 0;
function law(name: string, fn: () => void) {
  try { fn(); pass++; console.log("  ✓ " + name); } catch (e) { fail++; console.log("  ✗ " + name + "\n    " + String(e instanceof Error ? e.message : e).split("\n")[0]); }
}

/** the root rule's two lines, as the kit writes them */
const base = noComments(read("kit/kit.css")).split("\n").map((l) => l.trim());
const HAND = base.find((l) => l.startsWith(":where(") && l.endsWith("{cursor:pointer}")) ?? "";
const ARROW = base.find((l) => l.startsWith(":is(:disabled") && l.includes("cursor:default")) ?? "";

/** the page roots: a stylesheet every page of its kind loads, and every site page with its own <style> */
const pages = (dir: string) => (existsSync(join(root, dir)) ? readdirSync(join(root, dir)).filter((f) => f.endsWith(".html")).map((f) => `${dir}/${f}`) : []);
const ROOTS = [
  "kit/kit.css", // the hub's pages (with tokens.css and notes.css)
  "site/docs/docs.css", // bise.dev/docs
  ...["site", "site/book", "site/m"].flatMap(pages).filter((p) => read(p).includes("<style")),
];
/** every stylesheet text of the web surfaces: the roots and the kit's other sheets */
const SHEETS = [...ROOTS, "kit/notes.css", "kit/site.css", "site/book/paper.css"];
const styleOf = (p: string) => (p.endsWith(".html") ? [...read(p).matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]).join("\n") : read(p));

law("the kit has the root rule this check copies (kit.css: the hand, then the arrow when disabled)", () => {
  assert.ok(HAND.includes("button") && HAND.includes("[role=button]") && HAND.includes("a[href]"), "no :where(button,a[href],...,[role=button],...){cursor:pointer} line in kit/kit.css");
  assert.ok(ARROW !== "", "no :is(:disabled,...){cursor:default} line in kit/kit.css");
});

law("every page root carries the root rule, the same text as the kit", () => {
  const missing = ROOTS.filter((p) => {
    const lines = noComments(styleOf(p)).split("\n").map((l) => l.trim());
    return !lines.includes(HAND) || !lines.includes(ARROW);
  });
  assert.ok(ROOTS.length > 10, `only ${ROOTS.length} page roots found`);
  assert.deepEqual(missing, [], "a page root without the hand: copy base.css's two lines into it");
});

/** what may show the arrow: a disabled state, an answered question (no more picks), a site card that takes no click */
const MAY = /:disabled|\[disabled\]|aria-disabled|\[data-answer\]/;
const MAY_SITE = new Set([".feat"]);

law("no sheet puts the arrow back on a control (only a disabled state or a thing that takes no click)", () => {
  const found = [];
  for (const p of SHEETS) {
    for (const m of noComments(styleOf(p)).matchAll(/([^{}]*)\{([^{}]*)\}/g)) {
      if (!/(^|[;\s])cursor\s*:\s*(default|auto)\b/.test(m[2])) continue;
      for (const sel of m[1].split(",").map((s) => s.trim().replace(/\s+/g, " "))) {
        if (sel !== "" && !MAY.test(sel) && !MAY_SITE.has(sel)) found.push(`${relative(root, join(root, p))} ${sel}`);
      }
    }
  }
  assert.deepEqual(found, [], "cursor:default on a control undoes the hand");
});

console.log(`\n${pass} passed, ${fail} failed`);
if (fail > 0) process.exit(1);
