// Code (plugins/code): its pure parts on the emitted .js beside their .ts, its manifest against
// the one code table (kit/view/code.ts), its vendored grammars with prism run for real, and the
// rule that code goes in as text. node:test, no deps (gate.sh quick's gallery_plugins).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { runInNewContext } from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
const view = join(here, "..", "ai.mistral.vibe", "views", "code");
const kit = join(here, "..", "..", "..", "kit", "view");
const code = await import(pathToFileURL(join(kit, "code.js")).href);

/** lines.js with its kit import pointed at kit/view/code.js (the hub serves it as ./~kit/view/). */
async function loadLines() {
  const src = readFileSync(join(view, "lines.js"), "utf8").replace('"./~kit/view/code.js"', JSON.stringify(pathToFileURL(join(kit, "code.js")).href));
  return import(`data:text/javascript;base64,${Buffer.from(src).toString("base64")}`);
}
const lines = await loadLines();

/** prism with `lang`'s grammars, loaded as the view loads them (lines.js loadOrder), in a sandbox. */
function prismWith(lang) {
  const box = { Prism: { manual: true, disableWorkerMessageHandler: true } };
  box.window = box;
  for (const f of ["core", ...lines.loadOrder(lang)]) runInNewContext(readFileSync(join(view, "prism", `${f}.js`), "utf8"), box);
  return box.Prism;
}

/** A text's lines of runs, as the view colors one block. */
function colored(text, lang) {
  const P = prismWith(lang);
  return lines.runsOf(P.tokenize(text, P.languages[lang]));
}

test("the manifest claims exactly the table's endings and names, each titled as the table says", () => {
  const ui = JSON.parse(readFileSync(join(view, "view.json"), "utf8"))._meta["dev.bise/ui"];
  const globs = ui.files.flatMap((e) => e.match ?? []);
  const want = [...Object.keys(code.LANGS).map((x) => `*.${x}`), ...Object.keys(code.NAMES), "dockerfile.*"];
  assert.deepEqual([...globs].sort(), [...want].sort());
  // its title and the kind the head shows ('Code · TypeScript · v10'): the table's name for it
  for (const e of ui.files.filter((x) => x.match)) {
    for (const g of e.match) {
      const name = code.fileTitle(g.replace("*", "file").replace(/\.\*$/, ".dev"));
      assert.equal(e.title, name, g);
      assert.equal(e.kind, name, `${g}'s kind`);
    }
  }
  // others own html and markdown: Code is only listed for them (Open with)
  assert.deepEqual(ui.files.filter((e) => e.rank === "option").map((e) => e.title), ["HTML", "Markdown"]);
  // any other text file: the one text entry, plain text
  assert.deepEqual(ui.files.filter((e) => e.text), [{ text: true, title: code.PLAIN, icon: "doc" }]);
});

test("a file's language: its ending, its whole name, else its shebang", () => {
  assert.equal(code.langOf("src/prompts.ts"), "typescript");
  assert.equal(code.langOf("app.py"), "python");
  assert.equal(code.langOf("Dockerfile"), "docker");
  assert.equal(code.langOf("Dockerfile.dev"), "docker");
  assert.equal(code.langOf("run", "#!/usr/bin/env python3 -u"), "python");
  assert.equal(code.langOf("run", "#!/usr/bin/env -S node --no-warnings"), "javascript");
  assert.equal(code.langOf("deploy", "#!/bin/bash"), "bash");
  assert.equal(code.langOf("notes.txt", "#!/bin/bash"), "bash", "a shebang names a file with no language of its own");
  assert.equal(code.langOf("notes.txt"), undefined);
  assert.equal(code.fileTitle("prompts.ts"), "TypeScript");
  assert.equal(code.fileTitle("core.bend"), "Bend");
  assert.equal(code.fileTitle("notes.log"), "Plain text");
});

test("every language the table names has its grammar vendored, with the ones it needs", () => {
  const files = new Set(readdirSync(join(view, "prism")).filter((f) => f.endsWith(".js")).map((f) => f.slice(0, -3)));
  for (const lang of new Set([...Object.values(code.LANGS), ...Object.values(code.NAMES)])) {
    const order = lines.loadOrder(lang);
    assert.equal(order.at(-1), lang, lang);
    for (const f of order) assert.ok(files.has(f), `${lang} needs prism/${f}.js`);
    assert.ok(prismWith(lang).languages[lang], `${lang} loads`);
  }
  assert.deepEqual(lines.loadOrder("tsx"), ["markup", "clike", "javascript", "jsx", "typescript", "tsx"]);
  assert.deepEqual(lines.loadOrder(undefined), []);
});

test("a .ts and a .py file come out colored in bise's roles", () => {
  const ts = colored("export const a: number = 1; // one\nfunction f() { return 'x'; }", "typescript");
  const cls = (runs, text) => lines.classesOf(runs.find((r) => r.text === text)?.types ?? [], text);
  assert.equal(cls(ts[0], "export"), "syn-keyword syn-b");
  assert.equal(cls(ts[0], "// one"), "syn-comment syn-i");
  assert.equal(cls(ts[1], "'x'"), "syn-string");
  assert.equal(cls(ts[1], "f"), "syn-call");
  const py = colored('def f(x: int) -> str:\n    """doc"""\n    return 42', "python");
  assert.equal(cls(py[0], "def"), "syn-keyword syn-b");
  assert.equal(cls(py[0], "int"), "syn-type syn-n", "a built-in type word as a type (the TUI's)");
  assert.equal(cls(py[2], "42"), "syn-number");
});

test("code is text: a <script> in a .js or an .html file stays its words", () => {
  for (const [text, lang] of [["const s = '<script>alert(1)</script>';", "javascript"], ["<script>alert(1)</script>", "markup"]]) {
    const runs = colored(text, lang);
    assert.equal(runs.flat().map((r) => r.text).join(""), text, lang);
  }
  // the view draws every run as a text node or textContent, never as HTML
  const src = readFileSync(join(view, "code.ts"), "utf8");
  assert.doesNotMatch(src, /innerHTML|outerHTML|insertAdjacentHTML|document\.write|DOMParser/);
});

test("a token over several lines splits on its newlines, its types kept", () => {
  const runs = colored("/* a\n b */ x", "javascript");
  assert.deepEqual(runs.map((l) => l.map((r) => r.text)), [["/* a"], [" b */", " x"]]);
  assert.deepEqual(runs[1][0].types, ["comment"]);
});

test("a big file: only the lines on screen and a margin, colored by blocks with their lead", () => {
  const n = 1_000_000;
  const at = lines.shown(500_000 * lines.LINE, 800, n);
  assert.ok(at.to - at.from <= 800 / lines.LINE + 2 * lines.MARGIN + 1, `${at.to - at.from} rows drawn`);
  assert.deepEqual(lines.shown(0, 800, 3), { from: 0, to: 3 });
  const all = Array.from({ length: 2000 }, (_, i) => `line ${i}`);
  const b = lines.blockText(all, 2);
  assert.equal(b.skip, lines.LEAD);
  assert.equal(b.text.split("\n").length, lines.LEAD + lines.BLOCK);
  assert.equal(b.text.split("\n")[b.skip], `line ${2 * lines.BLOCK}`);
  // a comment opened in the lead (the block before's last LEAD lines) still reads as one in the block
  const src = [...Array.from({ length: lines.BLOCK - 10 }, () => "x;"), "/*", ...Array.from({ length: 20 }, () => "still a comment")];
  const blk = lines.blockText(src, 1);
  const P = prismWith("javascript");
  const runs = lines.runsOf(P.tokenize(blk.text, P.languages.javascript)).slice(blk.skip);
  assert.deepEqual(runs[0][0].types, ["comment"]);
  assert.equal(lines.PLAIN_OVER, 1024 * 1024);
  assert.equal(lines.sizeWords(lines.PLAIN_OVER), "1 MB");
});

test("lines: a final newline makes no empty last line, CRLF reads as LF, tabs to 4 columns", () => {
  assert.deepEqual(lines.linesOf(""), []);
  assert.deepEqual(lines.linesOf("a\r\nb\n"), ["a", "b"]);
  assert.deepEqual(lines.linesOf("a\n\n"), ["a", ""]);
  assert.equal(lines.columns("\tab"), 6);
  assert.equal(lines.columns("ab\tc"), 5);
  assert.equal(lines.widest(["a", "\t\tb", "abcd"]), 9);
});
