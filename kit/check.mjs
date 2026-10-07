// bise kit · the laws of kit.js (node, no deps): the in-place swap by data-id (nodes kept,
// order, scroll kept, bise:swapped), what changed by block hash, the components' behavior
// (numbers in tables, question picks), and that the kit itself never writes a style. kit.js runs
// in a vm with a small fake DOM (no browser).
//   node kit/check.mjs
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import vm from "node:vm";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));

// ---- a fake DOM: elements with attributes, children, text; a tiny HTML parser for fragments ----
const VOID = new Set(["br", "img", "hr"]);
class Text { constructor(t) { this.text = t; this.parent = null; } get textContent() { return this.text; } get outerHTML() { return this.text; } }
class El {
  constructor(tag) { this.tagName = tag.toUpperCase(); this.attrs = new Map(); this.nodes = []; this.parent = null; }
  get children() { return this.nodes.filter((n) => n instanceof El); }
  getAttribute(k) { return this.attrs.has(k) ? this.attrs.get(k) : null; }
  setAttribute(k, v) { this.attrs.set(k, String(v)); }
  removeAttribute(k) { this.attrs.delete(k); }
  hasAttribute(k) { return this.attrs.has(k); }
  get textContent() { return this.nodes.map((n) => n.textContent).join(""); }
  set textContent(t) { for (const n of this.nodes) n.parent = null; this.nodes = []; this.append(new Text(String(t))); }
  get outerHTML() {
    const t = this.tagName.toLowerCase();
    const a = [...this.attrs].map(([k, v]) => (v === "" ? ` ${k}` : ` ${k}="${v}"`)).join("");
    return VOID.has(t) ? `<${t}${a}>` : `<${t}${a}>${this.nodes.map((n) => n.outerHTML).join("")}</${t}>`;
  }
  append(...ns) { for (const n of ns) { if (n.parent) n.parent.removeChild(n); n.parent = this; this.nodes.push(n); } }
  removeChild(n) { const i = this.nodes.indexOf(n); assert.ok(i >= 0, "removeChild of a non-child"); this.nodes.splice(i, 1); n.parent = null; return n; }
  insertBefore(n, ref) {
    if (n.parent) n.parent.removeChild(n);
    const i = ref == null ? this.nodes.length : this.nodes.indexOf(ref);
    assert.ok(i >= 0, "insertBefore: ref is not a child");
    this.nodes.splice(i, 0, n); n.parent = this; ops.push(["insert", n.getAttribute && n.getAttribute("data-id")]); return n;
  }
  // descendants by tag name only (what kit.js's pure half asks for)
  querySelectorAll(tag) {
    const out = [], T = tag.toUpperCase();
    const walk = (e) => { for (const c of e.children) { if (c.tagName === T) out.push(c); walk(c); } };
    walk(this);
    return out;
  }
}
let ops = [];
function parse(html) {
  const root = new El("main");
  const stack = [root];
  const re = /<!--[\s\S]*?-->|<\/([a-z0-9]+)\s*>|<([a-z0-9]+)((?:\s+[a-z-]+(?:="[^"]*")?)*)\s*\/?>|([^<]+)/gi;
  for (const m of html.matchAll(re)) {
    const top = stack[stack.length - 1];
    if (m[1]) { while (stack.length > 1 && stack.pop().tagName !== m[1].toUpperCase()); continue; }
    if (m[2]) {
      const e = new El(m[2]);
      for (const a of (m[3] || "").matchAll(/([a-z-]+)(?:="([^"]*)")?/gi)) e.setAttribute(a[1], a[2] ?? "");
      top.append(e);
      if (!VOID.has(m[2].toLowerCase())) stack.push(e);
      continue;
    }
    if (m[4]) top.append(new Text(m[4]));
  }
  return root;
}
const blocks = (main) => main.children.filter((c) => c.getAttribute("data-kit") != null);
const ids = (main) => blocks(main).map((b) => b.getAttribute("data-id"));

// ---- kit.js in a vm: no document, so only BiseKit (the pure half) loads ----
const events = [];
class CustomEvent { constructor(type, init) { this.type = type; this.detail = init && init.detail; } }
const ctx = vm.createContext({ console, Math, JSON, Object, Array, Set, Map, WeakMap, String, Number, Date, CustomEvent, parseInt });
vm.runInContext(readFileSync(join(here, "kit.js"), "utf8"), ctx, { filename: "kit.js" });
const K = ctx.BiseKit;
const doc = { dispatchEvent: (e) => events.push({ type: e.type, detail: JSON.parse(JSON.stringify(e.detail)) }), createElement: (t) => new El(t) };
const j = (x) => JSON.parse(JSON.stringify(x));

// FNV-1a 64 over the folded block source, like rust/switchboard/src/pages/lint.rs, for fake metas
function fnv(s) {
  let h = 0xcbf29ce484222325n;
  for (const b of Buffer.from(s)) { h ^= BigInt(b); h = (h * 0x100000001b3n) & 0xffffffffffffffffn; }
  return h.toString(16).padStart(16, "0");
}
const blockList = (html) => blocks(parse(html)).map((b) => ({ id: b.getAttribute("data-id"), kit: b.getAttribute("data-kit"), hash: fnv(K.norm(b.outerHTML)) }));
const metaOf = (...versions) => ({ versions: versions.map((html, i) => ({ n: i + 1, at_ms: 1000 * i, blocks: blockList(html) })) });

let pass = 0, fail = 0;
function law(name, fn) {
  ops = []; events.length = 0;
  try { fn(); pass++; } catch (e) { fail++; console.log(`✗ ${name}\n  ${String(e.stack || e.message).split("\n").slice(0, 6).join("\n  ")}`); }
}

const V1 = `<section data-kit="heading" data-id="h"><h1>week 41</h1></section>
<section data-kit="prose" data-id="p1"><p>signup is 4× faster.</p></section>
<section data-kit="table" data-id="t1"><table><tr><td>signups</td><td>412</td></tr></table></section>
<section data-kit="prose" data-id="p2"><p>the banner is smaller.</p></section>`;
const V2 = `<section data-kit="heading" data-id="h"><h1>week 41</h1></section>
<section data-kit="prose" data-id="p1"><p>signup is faster: 4.1 s → 0.9 s.</p></section>
<section data-kit="callout" data-id="c1" data-tone="risk"><h3>heads-up</h3><p>2.5 slips.</p></section>
<section data-kit="table" data-id="t1"><table><tr><td>signups</td><td>412</td></tr></table></section>`;

// ---- the swap ----
law("an unchanged block stays the same node; a changed one is replaced; a new one comes in; a gone one goes", () => {
  const main = parse(V1);
  blocks(main).forEach(K.remember);
  const [h, p1, t1, p2] = blocks(main);
  const res = K.swap(main, blocks(parse(V2)), null, null);
  assert.deepEqual(ids(main), ["h", "p1", "c1", "t1"]);
  const now = blocks(main);
  assert.equal(now[0], h, "heading kept");
  assert.equal(now[3], t1, "table kept");
  assert.notEqual(now[1], p1, "p1 replaced");
  assert.equal(now[1].textContent, "signup is faster: 4.1 s → 0.9 s.");
  assert.ok(!now.includes(p2) && p2.parent === null, "p2 removed");
  assert.deepEqual(j(res), { changed: ["p1", "c1"], removed: ["p2"] });
  assert.ok(now[1].hasAttribute("data-arrived") && !now[0].hasAttribute("data-arrived"));
});

law("with hashes, only the blocks whose hash differs are replaced, even if notes.js decorated the others", () => {
  const main = parse(V1);
  const p2 = blocks(main)[3];
  p2.append(new El("span")); // a pin notes.js put in
  const meta = metaOf(V1, V1.replace("4× faster", "faster"));
  const changed = K.diffBlocks(meta.versions[0].blocks, meta.versions[1].blocks).changed;
  assert.deepEqual(j(changed), ["p1"]);
  K.swap(main, blocks(parse(meta && V1.replace("4× faster", "faster"))), changed, null);
  assert.equal(blocks(main)[3], p2, "the decorated block is kept with its pin");
  assert.equal(p2.children.length, 2);
});

law("without hashes, a block is compared with the source it came with, not with notes.js's decorations", () => {
  const main = parse(V1);
  blocks(main).forEach(K.remember);
  const p2 = blocks(main)[3];
  p2.setAttribute("data-bn-anchored", ""); // notes.js's mark
  const res = K.swap(main, blocks(parse(V1)), null, null);
  assert.equal(blocks(main)[3], p2);
  assert.deepEqual(j(res.changed), []);
});

law("a reorder moves blocks, it does not rebuild them", () => {
  const main = parse(V1);
  const before = blocks(main);
  const re = `${V1.split("\n")[3]}\n${V1.split("\n")[0]}\n${V1.split("\n")[1]}\n${V1.split("\n")[2]}`;
  blocks(main).forEach(K.remember);
  const res = K.swap(main, blocks(parse(re)), null, null);
  assert.deepEqual(ids(main), ["p2", "h", "p1", "t1"]);
  assert.deepEqual(blocks(main), [before[3], before[0], before[1], before[2]]);
  assert.deepEqual(j(res.changed), []);
  assert.equal(ops.length, 1, `one move, got ${JSON.stringify(ops)}`);
});

law("the same version again changes nothing and moves nothing", () => {
  const main = parse(V2);
  blocks(main).forEach(K.remember);
  const before = blocks(main);
  const res = K.swap(main, blocks(parse(V2)), null, null);
  assert.deepEqual(blocks(main), before);
  assert.equal(ops.length, 0);
  assert.deepEqual(j(res), { changed: [], removed: [] });
});

// a fake layout: each block is as tall as its text, the page scrolled by scrollY
function layout(main, scrollY) {
  const view = {
    scrollY,
    heights: () => blocks(main).map((b) => 40 + b.textContent.length * 2),
    topOf(id) { let y = -view.scrollY; for (const b of blocks(main)) { if (b.getAttribute("data-id") === id) return y; y += 40 + b.textContent.length * 2; } return null; },
    anchor() { let y = -view.scrollY; for (const b of blocks(main)) { const h = 40 + b.textContent.length * 2; if (y + h > 46) return { id: b.getAttribute("data-id"), top: y }; y += h; } return null; },
    scrollBy(dy) { view.scrollY += dy; },
  };
  return view;
}
law("scroll is kept: the block you read stays where it was when blocks above it change", () => {
  const main = parse(V1);
  const view = layout(main, 120);
  const a = view.anchor();
  assert.equal(a.id, "t1");
  const longer = V1.replace("signup is 4× faster.", "signup is faster: 4.1 s → 0.9 s on a mid-range phone, from #perf.");
  K.swap(main, blocks(parse(longer)), ["p1"], view);
  assert.equal(view.topOf("t1"), a.top, "t1 still at the same place on screen");
  assert.ok(view.scrollY > 120);
});

law("scroll is kept: when the block you read is gone, the next one takes its place", () => {
  const main = parse(V1);
  const view = layout(main, 120);
  assert.equal(view.anchor().id, "t1");
  const without = V1.split("\n").filter((l) => !l.includes('data-id="t1"')).join("\n");
  K.swap(main, blocks(parse(without)), null, view);
  assert.deepEqual(ids(main), ["h", "p1", "p2"]);
});

law("applyVersion: swaps, marks what changed against the version before, and fires one bise:swapped", () => {
  const main = parse(V1);
  main.setAttribute("data-version", "1");
  const meta = metaOf(V1, V2);
  const res = K.applyVersion(doc, main, blocks(parse(V2)), meta, 1, 2, { agent: "weekly update" });
  assert.equal(main.getAttribute("data-version"), "2");
  assert.deepEqual(j(res.marked), ["p1", "c1"]);
  assert.deepEqual(blocks(main).filter((b) => b.hasAttribute("data-changed")).map((b) => b.getAttribute("data-id")), ["p1", "c1"]);
  assert.deepEqual(events, [{ type: "bise:swapped", detail: { version: 2, changed: ["p1", "c1"], removed: ["p2"] } }]);
});

law("applyVersion skipping a version: replaced by v1→v3 hashes, marked by v2→v3", () => {
  const V3 = V2.replace("2.5 slips.", "2.5 ships wednesday.");
  const main = parse(V1);
  const meta = metaOf(V1, V2, V3);
  const res = K.applyVersion(doc, main, blocks(parse(V3)), meta, 1, 3, {});
  assert.deepEqual(j(res.changed), ["p1", "c1"]);
  assert.deepEqual(j(res.marked), ["c1"]);
});

// ---- what changed ----
law("what changed: by hash; v1 has none; new blocks count, removed ones are listed apart", () => {
  const meta = metaOf(V1, V2);
  assert.deepEqual(j(K.changedIn(meta, 1)), []);
  assert.deepEqual(j(K.changedIn(meta, 2)), ["p1", "c1"]);
  assert.deepEqual(j(K.diffBlocks(meta.versions[0].blocks, meta.versions[1].blocks)), { changed: ["p1", "c1"], removed: ["p2"] });
  assert.deepEqual(j(K.changedIn(null, 2)), []);
  assert.deepEqual(j(K.changedIn({ versions: [{ n: 2, blocks: [] }] }, 2)), []);
});

law("the kit's hash is the lint's: re-indenting a block changes nothing", () => {
  const a = blockList(`<section data-kit="prose" data-id="p"><p>one two</p></section>`)[0].hash;
  const b = blockList(`<section data-kit="prose" data-id="p">\n   <p>one\n two</p>\n</section>`)[0].hash;
  assert.equal(a, b);
});

// ---- components ----
law("tables: numeric columns get data-num, text columns don't, the header follows its column", () => {
  const main = parse(`<section data-kit="table" data-id="t"><table><thead><tr><th>metric</th><th>last</th><th>this</th><th>note</th></tr></thead>
<tbody><tr><td>signups</td><td>412</td><td>1,468</td><td>up</td></tr><tr><td>p50</td><td>4.1 s</td><td>0.9 s</td><td>12</td></tr>
<tr><td>conversion</td><td>3.2 %</td><td>−3.4%</td><td></td></tr></tbody></table></section>`);
  K.decorate(blocks(main), "a");
  const rows = main.querySelectorAll("tr").map((tr) => tr.children.map((c) => (c.hasAttribute("data-num") ? "n" : "-")).join(""));
  assert.deepEqual(rows, ["-nn-", "-nn-", "-nn-", "-nn-"]);
});

law("numbers: what reads as a number", () => {
  for (const t of ["412", "1,468", "4.1 s", "3.2 %", "−3.4%", "$1,200", "€ 12", "+12%", "(3)", "4.1 s → 0.9 s", "~40 ms", "12k"]) assert.ok(K.isNumeric(t), t);
  for (const t of ["signups", "v2", "Q3 plan", "up 12%", "", "dark mode"]) assert.ok(!K.isNumeric(t), t);
});

const Q = `<section data-kit="question" data-id="q1"><p>post it now?</p><ol><li>post now</li><li>wait for <strong>Marc</strong></li></ol></section>`;
law("question: a pick marks one option and says what it is; the same pick again takes it back", () => {
  const main = parse(Q);
  const q = blocks(main)[0];
  K.decorate([q], "weekly update");
  assert.equal(q.getAttribute("data-by"), "weekly update");
  const seen = [K.pick(q, 2)];
  assert.deepEqual(K.options(q).map((li) => li.hasAttribute("data-picked")), [false, true]);
  seen.push(K.pick(q, 1));
  assert.deepEqual(K.options(q).map((li) => li.hasAttribute("data-picked")), [true, false]);
  seen.push(K.pick(q, 1));
  assert.deepEqual(K.options(q).map((li) => li.hasAttribute("data-picked")), [false, false]);
  assert.deepEqual(j(seen), [
    { block: "q1", option: 2, text: "wait for Marc" },
    { block: "q1", option: 1, text: "post now" },
    { block: "q1", option: null, text: null },
  ]);
  assert.equal(K.pick(q, 3), null, "no option 3");
});

law("question: an answer from anywhere (SSE answered) shows on the page and tells notes.js", () => {
  const q = blocks(parse(Q))[0];
  K.decorate([q], "weekly update");
  K.pick(q, 1);
  K.answered(doc, q, "2");
  assert.equal(q.getAttribute("data-answer"), "2");
  assert.deepEqual(K.options(q).map((li) => li.hasAttribute("data-picked")), [false, true]);
  assert.equal(K.pick(q, 1), null, "answered: no other pick");
  assert.deepEqual(events, [{ type: "bise:answered", detail: { block: "q1", reply: "2" } }]);
});

law("question: an answered one (data-answer, by number or by text) shows its pick and takes no other", () => {
  for (const ans of ["2", "Wait for Marc"]) {
    events.length = 0;
    const q = blocks(parse(Q.replace('data-id="q1"', `data-id="q1" data-answer="${ans}"`)))[0];
    K.decorate([q], "a");
    assert.deepEqual(K.options(q).map((li) => li.hasAttribute("data-picked")), [false, true], ans);
    assert.equal(K.pick(q, 1), null);
    assert.equal(events.length, 0);
  }
});

// ---- review and email (§4.3) ----
const R = `<section data-kit="review" data-id="inbox"><h2>3 mails</h2><ol>
<li data-id="m1"><p>Léa · offsite</p><p>reply: yes, booked.</p></li>
<li data-id="m2"><p>Marc · invoice</p><p>reply: paid friday.</p></li>
<li data-id="m3"><p>Sam · intro</p><p>reply: happy to.</p></li></ol></section>`;
const acted = (b) => K.options(b).map((li) => li.getAttribute("data-verdict") || "-");
const allButton = (b) => b.children.find((c) => c.hasAttribute("data-kit-ui"));
law("review: each item gets approve / skip once, and 'approve N' counts what is left", () => {
  const b = blocks(parse(R))[0];
  K.decorate([b], "inbox", doc);
  K.decorate([b], "inbox", doc);
  for (const li of K.options(b)) assert.equal(li.children.filter((c) => c.hasAttribute("data-kit-ui")).length, 1, "one bar per item");
  assert.equal(allButton(b).textContent, "approve 3");
  assert.deepEqual(j(K.react(b, "m2", "skip")), { block: "inbox", item: "m2", kind: "skip", text: "Marc · invoice reply: paid friday." });
  K.decorate([b], "inbox", doc);
  assert.equal(allButton(b).textContent, "approve both");
});
law("review: 'approve N' approves the rest, never an item you skipped or edited", () => {
  const b = blocks(parse(R))[0];
  K.decorate([b], "inbox", doc);
  K.react(b, "m1", "skip");
  K.options(b)[2].setAttribute("data-edited", ""); // notes.js: you changed m3
  const out = K.approveAll(b);
  assert.deepEqual(j(out).map((d) => [d.item, d.kind]), [["m2", "approve"]]);
  assert.deepEqual(acted(b), ["skip", "approve", "-"]);
  K.decorate([b], "inbox", doc);
  assert.ok(allButton(b).children[0].hasAttribute("disabled"), "nothing left: disabled");
});
law("review and email: the same reaction again takes it back; an email reacts as a whole", () => {
  const b = blocks(parse(R))[0];
  K.react(b, "m1", "approve");
  assert.deepEqual(j(K.react(b, "m1", "approve")), { block: "inbox", item: "m1", kind: null, text: "Léa · offsite reply: yes, booked." });
  assert.deepEqual(acted(b), ["-", "-", "-"]);
  assert.equal(K.react(b, "m9", "approve"), null);
  const e = blocks(parse(`<section data-kit="email" data-id="e1"><p data-field="to">Léa</p><p data-field="subject">june</p><p>hi Léa,</p></section>`))[0];
  K.decorate([e], "inbox", doc);
  const d = K.react(e, null, "approve");
  assert.deepEqual(j(d), { block: "e1", item: null, kind: "approve", text: "Léa june hi Léa," });
  assert.equal(e.getAttribute("data-verdict"), "approve");
});
law("the kit's own controls never count as a change of the agent's block", () => {
  const main = parse(R);
  blocks(main).forEach(K.remember);
  K.decorate(blocks(main), "inbox", doc);
  const before = blocks(main)[0];
  const res = K.swap(main, blocks(parse(R)), null, null);
  assert.equal(blocks(main)[0], before);
  assert.deepEqual(j(res.changed), []);
});

law("names: an agent's id reads with spaces on the page (by launch recap), the id stays", () => {
  assert.equal(K.W.by("launch-recap"), "by launch recap");
  assert.equal(K.W.by("main"), "by bise", "main speaks as bise on the ambient surfaces");
  assert.equal(K.W.updating("weekly-update"), "weekly update is updating this page…");
  const q = blocks(parse(Q))[0];
  K.decorate([q], "launch-recap", doc);
  assert.equal(q.getAttribute("data-by"), "launch recap");
  assert.equal(q.getAttribute("data-id"), "q1");
});

law("compare: ♡ ✗ pick on each variant, one pick per block (the last wins), the variant's facts left inside", () => {
  const c = blocks(parse(`<section data-kit="compare" data-id="venues"><ol>
<li data-id="moulin"><h3>Le Moulin</h3><ul><li>€12,400</li><li>22 rooms</li></ul><p>why: in budget.</p></li>
<li data-id="pins"><h3>Domaine des Pins</h3><ul><li>€13,900</li></ul></li></ol></section>`))[0];
  K.decorate([c], "offsite", doc);
  const variants = K.options(c);
  assert.equal(variants.length, 2, "the inner facts lists are not variants");
  assert.deepEqual(variants[0].children.find((x) => x.hasAttribute("data-kit-ui")).children.map((b) => b.getAttribute("data-act")), ["keep", "drop", "pick"]);
  assert.equal(j(K.react(c, "moulin", "pick")).kind, "pick");
  assert.equal(j(K.react(c, "pins", "pick")).kind, "pick");
  assert.deepEqual(variants.map((v) => v.getAttribute("data-verdict")), [null, "pick"]);
  assert.equal(j(K.react(c, "moulin", "keep")).text, "Le Moulin €12,400 22 rooms why: in budget.");
});

law("checklist: a tick box first in each item; an item the agent marked data-done starts ticked; a tick toggles", () => {
  const c = blocks(parse(`<section data-kit="checklist" data-id="todo"><ol><li data-id="t1" data-done>book the venue <small>Gabriel · friday</small></li><li data-id="t2">send the invite</li></ol></section>`))[0];
  K.decorate([c], "offsite", doc);
  const [t1, t2] = K.options(c);
  assert.ok(t1.nodes[0].hasAttribute && t1.nodes[0].hasAttribute("data-kit-ui"), "the box comes first");
  assert.equal(t1.getAttribute("data-verdict"), "tick");
  assert.deepEqual(j(K.react(c, "t2", "tick")), { block: "todo", item: "t2", kind: "tick", text: "send the invite" });
  assert.equal(j(K.react(c, "t2", "tick")).kind, null);
  assert.equal(t2.getAttribute("data-verdict"), null);
});

law("message: copy gives Slack markup of what the page shows, the kit's button left out", () => {
  const m = blocks(parse(`<section data-kit="message" data-id="slack" data-to="Slack · #launch"><p><strong>bise is live</strong> today, <em>finally</em>.</p>
<ul><li>the repo is <a href="https://github.com/acme/bise">public</a></li><li>install: <code>brew install bise</code></li></ul><p>thanks <del>all</del> everyone!</p></section>`))[0];
  K.decorate([m], "launch", doc);
  assert.equal(m.children.filter((c) => c.hasAttribute("data-kit-ui")).length, 1);
  assert.equal(K.toSlack(m), "*bise is live* today, _finally_.\n\n• the repo is <https://github.com/acme/bise|public>\n• install: `brew install bise`\n\nthanks ~all~ everyone!");
});

law("writing: the last 5 intents, newest at the bottom, the rest folded; the last one is current while working", () => {
  const seven = ["reading your Gmail", "reading #launch", "counting stars", "reading HN", "comparing", "drafting", "checking links"];
  const w = j(K.intentLines(seven, true));
  assert.equal(w.earlier, 2);
  assert.deepEqual(w.lines.map((x) => x.text), seven.slice(2));
  assert.deepEqual(w.lines.map((x) => x.current), [false, false, false, false, true]);
  assert.equal(K.W.earlier(w.earlier), "+2 earlier");
  const done = j(K.intentLines(seven.slice(0, 2), false));
  assert.deepEqual(done, { earlier: 0, lines: [{ text: "reading your Gmail", current: false }, { text: "reading #launch", current: false }] });
  assert.deepEqual(j(K.intentLines(["", " "], true)), { earlier: 0, lines: [] });
});

law("writing: the frame's time counts up; the words are ambient's", () => {
  assert.equal(K.since(0, 12_400), "12 s");
  assert.equal(K.since(0, 61_000), "1 min");
  assert.equal(K.since(5_000, 0), "0 s");
  assert.equal(K.W.asked("the launch recap for Slack"), "you asked: “the launch recap for Slack”");
  assert.equal(K.W.failed("the Slack token expired"), "▲ couldn't finish: the Slack token expired. main is on it.");
  assert.equal(K.W.tabWriting("launch recap"), "launch recap · writing");
});

law("reply items: X gets a prefilled reply intent, the other sites 'copy + open' the thread; bise never posts", () => {
  const x = j(K.replyTarget("https://x.com/someone/status/1842/photo/1", "thanks! it's one thread per repo"));
  assert.deepEqual(x, { site: "X", open: "https://x.com/intent/post?in_reply_to=1842&text=thanks!%20it's%20one%20thread%20per%20repo", copy: false });
  assert.deepEqual(j(K.replyTarget("https://news.ycombinator.com/item?id=4242", "t")), { site: "Hacker News", open: "https://news.ycombinator.com/item?id=4242", copy: true });
  assert.equal(j(K.replyTarget("https://old.reddit.com/r/rust/comments/abc", "t")).site, "Reddit");
  assert.equal(j(K.replyTarget("https://bsky.app/profile/a.bsky.social/post/3k", "t")).site, "Bluesky");
  assert.equal(K.replyTarget("javascript:alert(1)", "t"), null);
  assert.equal(K.W.replyOn("X"), "reply on X");
  assert.equal(K.W.copyOpen("Hacker News"), "copy + open Hacker News");
});

law("reply items: the reply is what follows the quoted comment; the approve button says where it goes", () => {
  const b = blocks(parse(`<section data-kit="review" data-id="watch"><ol><li data-id="hn1" data-reply="https://news.ycombinator.com/item?id=4242"><p>dang · Hacker News · 2 h ago</p><blockquote>how is it different from tmux?</blockquote><p>one thread per repo, and agents that <em>talk</em> to each other.</p></li>
<li data-id="x1" data-reply="https://x.com/a/status/9"><p>@a · X</p><blockquote>nice</blockquote><p>thanks!</p></li></ol></section>`))[0];
  K.decorate([b], "watch", doc);
  const [hn, x] = K.options(b);
  assert.equal(K.replyText(hn), "one thread per repo, and agents that talk to each other.");
  const label = (li) => li.children.find((c) => c.hasAttribute("data-kit-ui")).children.find((c) => c.getAttribute("data-act") === "approve").textContent;
  assert.equal(label(hn), "open on Hacker News");
  assert.equal(label(x), "open on X");
});

law("checklist who and when: due dates in words, late in its own state, done items never late", () => {
  const now = new Date(2026, 4, 13, 10).getTime(); // wednesday 13 may 2026
  assert.deepEqual(j(K.dueWords("2026-05-13", now)), { text: "today", late: false });
  assert.deepEqual(j(K.dueWords("2026-05-14", now)), { text: "tomorrow", late: false });
  assert.deepEqual(j(K.dueWords("2026-05-15", now)), { text: "friday", late: false });
  assert.deepEqual(j(K.dueWords("2026-05-20", now)), { text: "20 may", late: false });
  assert.deepEqual(j(K.dueWords("2026-05-10", now)), { text: "late · 3 days", late: true });
  assert.deepEqual(j(K.dueWords("2026-05-12", now)), { text: "late · 1 day", late: true });
  assert.equal(K.dueWords("friday", now), null);
  const c = blocks(parse(`<section data-kit="checklist" data-id="todo"><ol><li data-id="t1" data-who="Camille" data-due="2020-01-01">trains</li><li data-id="t2" data-done data-due="2020-01-01">venue</li><li data-id="t3">invite</li></ol></section>`))[0];
  K.decorate([c], "plan", doc);
  const [t1, t2, t3] = K.options(c);
  const when = (li) => li.children.find((n) => n.hasAttribute("data-when"));
  assert.ok(when(t1).textContent.startsWith("Camille · late · "));
  assert.ok(t1.hasAttribute("data-late") && !t2.hasAttribute("data-late"), "a done item is never late");
  assert.equal(when(t2), undefined, "nor says 'late' (no who, a late due: no tag at all)");
  assert.equal(when(t3), undefined, "no who, no due: no tag");
  // roadmap D: his own step says 'you'; ticked elsewhere (SSE 'ticked'), it shows ticked, never late
  const y = blocks(parse(`<section data-kit="checklist" data-id="steps"><ol><li data-id="s1" data-who="yours" data-due="2020-01-01">pay the deposit</li></ol></section>`))[0];
  K.decorate([y], "plan", doc);
  const s1 = K.options(y)[0];
  assert.ok(when(s1).textContent.startsWith("you · late"));
  assert.equal(K.ticked(doc, y, "s1"), true);
  assert.equal(s1.getAttribute("data-verdict"), "tick");
  assert.ok(!s1.hasAttribute("data-late"));
  assert.equal(when(s1).textContent, "you", "ticked: who, not 'late'");
  // bise took its own step (pm's 26): it says what it did and takes you to the draft
  const bd = blocks(parse(`<section data-kit="checklist" data-id="buy"><ol><li data-id="b1" data-who="bise" data-did="drafted" data-draft="mail-legal">the mail to legal</li><li data-id="b2" data-who="bise" data-did="checked · it loads">acme.app loads</li></ol></section>`))[0];
  K.decorate([bd], "plan", doc);
  const [b1, b2] = K.options(bd);
  assert.equal(when(b1).textContent, "bise · drafted · in the page");
  assert.equal(when(b1).getAttribute("data-goto"), "mail-legal");
  assert.equal(when(b2).textContent, "bise · checked · it loads");
  assert.equal(when(b2).getAttribute("data-goto"), null);
  // a step that is a decision (pm's 35): it points at its question until it is done
  const dq = blocks(parse(`<section data-kit="checklist" data-id="buy"><ol><li data-id="s2" data-who="yours" data-question="years">how long</li><li data-id="s3" data-who="yours" data-question="years" data-done>how long</li></ol></section>`))[0];
  K.decorate([dq], "plan", doc);
  const [q1, q2] = K.options(dq);
  assert.equal(when(q1).textContent, "you · pick in the question");
  assert.equal(when(q1).getAttribute("data-goto"), "years");
  assert.equal(when(q2).textContent, "you");
  assert.equal(when(q2).getAttribute("data-goto"), null);
  assert.equal(K.ticked(doc, y, "nope"), false);
});

law("where a draft went: the user's tool and its link on the block it came from, copied with the channel", () => {
  const main = parse(`<section data-kit="email" data-id="e1" data-to="Gmail · reply to Camille"><p data-field="to">c</p><p data-field="subject">s</p><p>hi</p></section><section data-kit="message" data-id="m1" data-to="Slack · #launch" data-open="slack://channel?team=T1&id=C1"><p>x</p></section><section data-kit="message" data-id="m2" data-to="Slack · @benjamin"><p>y</p></section>`);
  const [e1, m1, m2] = blocks(main);
  assert.deepEqual(j(K.wentWords({ kind: "gmail-draft", url: "https://mail.google.com/x" }, e1)), { text: "in your Gmail drafts", open: "open", href: "https://mail.google.com/x" });
  assert.deepEqual(j(K.wentWords({ kind: "copy" }, m1)), { text: "copied", open: "open #launch", href: "slack://channel?team=T1&id=C1" });
  assert.deepEqual(j(K.wentWords({ kind: "slack", url: "javascript:alert(1)" }, m2)), { text: "posted in @benjamin", open: null, href: null }, "a link that isn't https/slack/mailto is dropped");
  const went = [{ kind: "gmail-draft", ref: "r1", url: "https://mail.google.com/a", at: 1 }, { kind: "copy", ref: "c", at: 2, block: "m2" }, { kind: "kit-draft", ref: "k", at: 3, block: "gone" }];
  const map = K.wentFor(went, blocks(main));
  assert.deepEqual([...map.keys()].sort(), ["e1", "m2"], "no block: a draft goes to the first email; a missing block is skipped");
  K.applyWent(doc, blocks(main), went);
  const line = (b) => b.children.find((n) => n.hasAttribute("data-went"));
  assert.equal(line(e1).textContent, "in your Gmail draftsopen");
  assert.equal(line(e1).getAttribute("data-went"), "gmail-draft");
  assert.equal(line(m1), undefined);
  K.applyWent(doc, blocks(main), [{ kind: "outlook-draft", ref: "r2", url: "https://outlook.office.com/b", at: 5 }]);
  assert.equal(e1.children.filter((n) => n.hasAttribute("data-went")).length, 1, "redrawn in place");
  assert.ok(line(e1).textContent.startsWith("in your Outlook drafts"));
  assert.ok(line(m2), "a local 'copied' stays until the server says where it went");
});

law("send, not approve, when the draft leaves; the editable parts are the drafts only", () => {
  const main = parse(`<section data-kit="review" data-id="r" data-verb="send"><ol><li data-id="m1"><p>Léa · dates · 9:12</p><p>yes</p></li><li data-id="m2"><p>Marc · invoice</p><blockquote>paid?</blockquote><p>yes, friday</p></li></ol></section><section data-kit="review" data-id="b"><ol><li data-id="x1"><p>a</p><p>b</p></li></ol></section><section data-kit="email" data-id="e"><p data-field="to">c</p><p data-field="subject">s</p><p>hi</p></section>`);
  const [r, b, e] = blocks(main);
  K.decorate([r, b, e], "inbox", doc);
  const labels = (el) => el.children.filter((n) => n.hasAttribute("data-kit-ui")).flatMap((u) => u.children).map((x) => x.textContent);
  assert.deepEqual(labels(K.options(r)[0]), ["send", "skip"]);
  assert.deepEqual(labels(r), ["send both"]);
  assert.deepEqual(labels(K.options(b)[0]), ["approve", "skip"]);
  assert.deepEqual(labels(b), ["approve 1"]);
  assert.deepEqual(labels(e), ["send", "skip"]);
  const nl = blocks(parse(`<section data-kit="email" data-id="nl" data-verb="draft"><p data-field="to">all subscribers</p><p data-field="subject">s</p><p>hi</p></section>`))[0];
  K.decorate([nl], "newsletter", doc);
  assert.deepEqual(labels(nl), ["put it in my drafts", "skip"], "a newsletter bise only drafts: never 'send'");
  // two items: 'send both'; a message says what happens next: send / skip, or copy + open in Slack
  const two = blocks(parse(`<section data-kit="review" data-id="r2" data-verb="send"><ol><li data-id="a"><p>x</p><p>y</p></li><li data-id="b"><p>x</p><p>y</p></li></ol></section>`))[0];
  K.decorate([two], "inbox", doc);
  assert.deepEqual(labels(two), ["send both"]);
  const [ms, mc, mn] = blocks(parse(`<section data-kit="message" data-id="ms" data-to="Slack · reply to Benjamin in #bise-feedback" data-verb="send"><p>x</p></section><section data-kit="message" data-id="mc" data-to="Slack · reply to Benjamin in #bise-feedback" data-open="https://acme.slack.com/archives/C1/p1"><p>x</p></section><section data-kit="message" data-id="mn" data-to="Slack · #launch"><p>x</p></section>`));
  K.decorate([ms, mc, mn], "feedback", doc);
  assert.deepEqual(labels(ms), ["send", "skip"]);
  assert.deepEqual(labels(mc), ["copy", "open in Slack"]);
  assert.deepEqual(labels(mn), ["copy"]);
  const k = blocks(parse(`<section data-kit="review" data-id="taste" data-verb="keep"><ol><li data-id="no-emoji"><p>from your notes</p><p>no emoji</p></li></ol></section>`))[0];
  K.decorate([k], "main", doc);
  assert.deepEqual(labels(K.options(k)[0]), ["keep", "strike"], "what bise keeps: keep / strike");
  const ed = (el) => K.editableParts(el).map((x) => x.textContent);
  assert.deepEqual(ed(r), ["yes", "yes, friday"], "never the who line nor the quoted comment");
  assert.deepEqual(ed(e), ["s", "hi"], "subject and body, not to");
  assert.ok(K.editableParts(e).every((x) => x.hasAttribute("data-editable")));
});

law("a watching page: 'watching · checked 3 min ago · until tomorrow 18:00'; a read-only reply says what happened", () => {
  const now = new Date(2026, 4, 13, 14, 20).getTime();
  const at = (d, h, m) => new Date(2026, 4, d, h, m).getTime();
  assert.equal(K.untilWords(at(13, 18, 0), now), "until 18:00");
  assert.equal(K.untilWords(at(14, 18, 0), now), "until tomorrow 18:00");
  assert.equal(K.untilWords(at(16, 9, 5), now), "until saturday 9:05");
  assert.equal(K.untilWords(at(30, 9, 0), now), "until 30 may");
  assert.equal(K.untilWords(null, now), null);
  assert.equal(K.watchLine({ timer: 3, every: "every 10m", checked_ms: now - 3 * 60000, until_ms: at(14, 18, 0) }, now), "watching · checked 3 min ago · until tomorrow 18:00");
  assert.equal(K.watchLine({ timer: 3, every: "every 10m" }, now), "watching");
  assert.equal(K.watchLine(null, now), null);
  // a page link's #<id> (the morning page's lines, a card's url) names an item, else a block
  const tg = blocks(parse(`<section data-kit="checklist" data-id="todo"><ol><li data-id="t4">agenda</li></ol></section><section data-kit="prose" data-id="p1"><p>x</p></section>`));
  assert.equal(K.targetOf(tg, "t4").getAttribute("data-id"), "t4");
  assert.equal(K.targetOf(tg, "p1").getAttribute("data-id"), "p1");
  assert.equal(K.targetOf(tg, "nope"), null);
  assert.equal(K.targetOf(tg, ""), null);
  // his taste file was followed (meta.taste {rules}): the frame says so, with the count
  assert.equal(K.tasteWords({ rules: 4 }), "following your taste · 4 rules");
  assert.equal(K.tasteWords({ rules: 1 }), "following your taste · 1 rule");
  assert.equal(K.tasteWords({ rules: 0 }), null);
  assert.equal(K.tasteWords(undefined), null);
  const r = blocks(parse(`<section data-kit="review" data-id="w" data-verb="open-reply"><ol><li data-id="hn-1" data-reply="https://news.ycombinator.com/item?id=1"><p>pg · Hacker News · 1 h ago</p><blockquote>q?</blockquote><p>a.</p></li><li data-id="x-2" data-reply="https://x.com/a/status/2"><p>@a · X</p><blockquote>q</blockquote><p>b</p></li></ol></section>`))[0];
  K.decorate([r], "launch-watch", doc);
  const [hn, x] = K.options(r);
  const labels = (el) => el.children.filter((n) => n.hasAttribute("data-kit-ui")).flatMap((u) => u.children).map((y) => y.textContent);
  assert.deepEqual(labels(hn), ["open on Hacker News", "skip"], "never 'send' on a read-only place");
  assert.equal(K.replyWent(doc, hn, K.replyTarget(hn.getAttribute("data-reply"), "a.")).textContent, "copiedpaste it in the thread");
  assert.equal(K.replyWent(doc, x, K.replyTarget(x.getAttribute("data-reply"), "b")).textContent, "opened on X · your reply is in the box");
  assert.equal(x.children.filter((n) => n.hasAttribute("data-went")).length, 1);
});

law("feedback into work: start an agent / answer Benjamin / skip; started, the start goes and the agent line shows", () => {
  const r = blocks(parse(`<section data-kit="review" data-id="reports" data-verb="start"><ol><li data-id="s-101"><p>Benjamin Roy · #bise-feedback · 9:12</p><blockquote>install fails on fish</blockquote><p>i think: the installer only writes .zshrc.</p></li><li data-id="s-102" data-agent="fix-install-path"><p>Benjamin Roy · #bise-feedback · 9:40</p><blockquote>x</blockquote><p>y</p></li></ol></section>`))[0];
  K.decorate([r], "feedback", doc);
  const [a, b] = K.options(r);
  const btns = (li) => li.children.filter((n) => n.hasAttribute("data-kit-ui") && !n.hasAttribute("data-agent-line")).flatMap((u) => u.children);
  assert.deepEqual(btns(a).map((x) => x.textContent), ["start an agent", "answer Benjamin", "skip"]);
  assert.equal(btns(a)[0].hidden, false);
  assert.equal(btns(b)[0].hidden, true, "an agent on it: no start");
  assert.ok(b.children.some((n) => n.hasAttribute("data-agent-line")));
  a.setAttribute("data-starting", "");
  K.decorate([r], "feedback", doc);
  assert.equal(btns(a)[0].textContent, "starting…");
  assert.equal(btns(a).length, 3, "never a second bar");
  // a mention to answer (data-verb="reply"): send the drafted answer, start an agent, skip; bulk 'send all'
  const m = blocks(parse(`<section data-kit="review" data-id="asks" data-verb="reply"><ol><li data-id="m1"><p>Léa Martin · #design · 9:12</p><blockquote>can you check the pricing copy?</blockquote><p>yes, i'll look at it before noon.</p></li><li data-id="m2"><p>Hugo · mail · 9:40</p><blockquote>x</blockquote><p>y</p></li></ol></section>`))[0];
  K.decorate([m], "mentions", doc);
  const [m1] = K.options(m);
  assert.deepEqual(btns(m1).map((x) => x.textContent), ["send", "start an agent", "skip"]);
  assert.equal(m.children.filter((n) => n.hasAttribute("data-kit-ui")).flatMap((u) => u.children)[0].textContent, "send both");
  assert.deepEqual(K.editableParts(m).map((e) => K.norm(e.textContent || (e.nodes || []).map((n) => n.textContent).join(""))).slice(0, 1).length, 1, "the drafted answer is editable");
});

law("an item's agent: its name first (spaces, not hyphens), then its live status and note", () => {
  assert.equal(K.agentLine("fix-login"), "∿ fix login");
  assert.equal(K.agentLine("fix-login", { name: "fix-login", status: "working", note: "reading the Safari logs" }), "∿ fix login working · reading the Safari logs");
  assert.equal(K.agentLine("fix-login", { name: "fix-login", status: "blocked", note: "which branch?" }), "? fix login needs you · which branch?");
  assert.equal(K.agentLine("fix-login", { name: "fix-login", status: "idle", note: "x" }), "∿ fix login waiting");
  assert.equal(K.agentLine("fix-login", { name: "fix-login", status: "nonsense" }), "∿ fix login");
  const r = blocks(parse(`<section data-kit="review" data-id="bugs"><ol><li data-id="b1" data-agent="fix-login">login loops</li><li data-id="b2">typo</li></ol></section>`))[0];
  K.agents.set("fix-login", { name: "fix-login", status: "failed" });
  K.decorate([r], "bugs", doc);
  const [b1, b2] = K.options(r);
  const line = (li) => li.children.find((n) => n.hasAttribute("data-agent-line"));
  assert.equal(line(b1).textContent, "▲ fix login failed");
  assert.equal(line(b1).getAttribute("data-status"), "failed");
  assert.equal(line(b2), undefined, "no agent: no line");
  K.agents.set("fix-login", { name: "fix-login", status: "done" });
  K.decorate([r], "bugs", doc);
  assert.equal(K.options(r)[0].children.filter((n) => n.hasAttribute("data-agent-line")).length, 1, "redrawn in place, never twice");
  assert.equal(line(K.options(r)[0]).textContent, "✓ fix login done");
  K.agents.clear();
});

law("ago: short words", () => {
  const now = 10_000_000;
  assert.equal(K.ago(now - 5_000, now), "just now");
  assert.equal(K.ago(now - 3 * 60_000, now), "3 min ago");
  assert.equal(K.ago(now - 2 * 3_600_000, now), "2 h ago");
  assert.equal(K.ago(now - 26 * 3_600_000, now), "yesterday");
  assert.equal(K.ago(now - 72 * 3_600_000, now), "3 days ago");
  // past a week: the date; past a year: with its year (designer m_7475)
  const local = (y, mo, d) => new Date(y, mo, d, 12).getTime();
  assert.equal(K.ago(local(2026, 9, 3), local(2026, 9, 20)), "3 oct");
  assert.equal(K.ago(local(2025, 9, 3), local(2026, 9, 4)), "3 oct 2025");
  assert.equal(K.ago(local(2025, 11, 20), local(2026, 0, 5)), "20 dec 2025");
});

// ---- no style anywhere ----
law("the kit never writes a style: no style= in kit.js, no .style in its code, none in the examples", () => {
  const js = readFileSync(join(here, "kit.js"), "utf8").replace(/\/\/.*$/gm, "");
  assert.ok(!/style\s*=|\.style\b|setProperty\(/.test(js), "kit.js touches style");
  for (const f of readdirSync(join(here, "examples"))) {
    const html = readFileSync(join(here, "examples", f), "utf8");
    assert.ok(!/style/i.test(html), `${f} has a style`);
  }
});

law("a write his word triggers is its own action block: its button says the write, skip next to it", () => {
  const a = blocks(parse(`<section data-kit="action" data-id="a-ops12" data-do="close OPS-12"><p>close OPS-12 in Linear · fixed in 0.4.2</p></section>`))[0];
  K.decorate([a], "feedback", doc);
  K.decorate([a], "feedback", doc);
  const bars = a.children.filter((n) => n.hasAttribute("data-kit-ui"));
  assert.equal(bars.length, 1, "one bar, however often it is decorated");
  assert.deepEqual(bars[0].children.map((b) => b.textContent), ["close OPS-12", "skip"]);
  assert.equal(K.react(a, null, "approve").kind, "approve");
});

law("a page opened with ?theme=light|dark wears it (the desktop window's pref), else the system's", () => {
  assert.equal(K.themeOf("?theme=dark"), "dark");
  assert.equal(K.themeOf("?a=1&theme=light"), "light");
  assert.equal(K.themeOf("?theme=dark&x=2"), "dark");
  assert.equal(K.themeOf("?theme=blue"), null);
  assert.equal(K.themeOf("?nottheme=dark"), null);
  assert.equal(K.themeOf(""), null);
  // tokens.css: the system's dark block yields to data-theme=light, and data-theme=dark has the same values
  const css = readFileSync(join(here, "tokens.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  const sys = /@media \(prefers-color-scheme: dark\) \{\s*:root:not\(\[data-theme="light"\]\) \{([^}]*)\}/.exec(css);
  const told = /:root\[data-theme="dark"\] \{([^}]*)\}/.exec(css);
  assert.ok(sys, "the system's dark block, unless the page was told light");
  assert.ok(told, "the told dark block");
  const decls = (s) => s.split(";").map((d) => d.trim()).filter((d) => d !== "");
  assert.deepEqual(decls(told[1]), decls(sys[1]), "the two dark blocks hold the same values");
});

law("kit.css takes its colors from tokens.css only", () => {
  const css = readFileSync(join(here, "kit.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  const hex = css.match(/#[0-9a-f]{3,8}\b/gi) || [];
  assert.deepEqual(hex, [], `raw colors in kit.css: ${hex.join(" ")}`);
  assert.ok(!/rgba?\(|hsla?\(/.test(css), "raw rgb/hsl in kit.css");
});

law("the fonts tokens.css names are in the kit (the CSP loads nothing from outside), each with its OFL", () => {
  const css = readFileSync(join(here, "tokens.css"), "utf8");
  const urls = [...css.matchAll(/url\(([^)]+)\)/g)].map((m) => m[1]);
  assert.ok(urls.length >= 3, "font urls");
  for (const u of urls) {
    assert.ok(!/^(https?:)?\/\//.test(u), `${u} is outside`);
    assert.ok(readFileSync(join(here, u)).length > 10000, `${u} missing`);
  }
  for (const f of ["OFL-newsreader.txt", "OFL-jetbrains-mono.txt"]) assert.ok(readFileSync(join(here, "fonts", f), "utf8").includes("SIL Open Font License"), f);
});

law("every example page is a fragment of known blocks with unique ids", () => {
  const kinds = new Set(["prose", "heading", "callout", "sources", "table", "question", "review", "email", "message", "compare", "checklist", "ui"]);
  for (const f of readdirSync(join(here, "examples"))) {
    const main = parse(readFileSync(join(here, "examples", f), "utf8"));
    assert.ok(main.children.length > 0 && main.children.every((c) => c.tagName === "SECTION" && kinds.has(c.getAttribute("data-kit"))), f);
    const all = ids(main);
    assert.equal(new Set(all).size, all.length, `${f}: duplicate ids`);
  }
});

console.log(`${fail ? "✗" : "✓"} kit.js laws: ${pass} passed, ${fail} failed`);
process.exit(fail ? 1 : 0);
