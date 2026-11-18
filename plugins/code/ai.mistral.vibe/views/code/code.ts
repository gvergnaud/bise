// Code (plugins/code, code-view: architect m_66499, designer m_66504): the view that opens a code
// or text file, read only. The hub calls this opener on his click with the file's handle
// (`view-file://…`); @bise/view (~kit/view/) reads it by ranges and keeps the view's state. Its
// language: kit/view/code.ts langOf (the ending, a whole name like Dockerfile, else the shebang);
// its grammar (prismjs, vendored in prism/) loaded on demand with the ones it needs (lines.ts
// loadOrder). Drawn as lines on a 20 px grid, only the ones on screen and a margin, colored by
// blocks (lines.ts); past PLAIN_OVER bytes drawn plain, and the note says so. Code goes in as
// text, never as HTML: every run is a text node (a `<script>` in a .js or an .html is words).

import { connect, viewkit, type View } from "./~kit/view/view.js";
import { langOf } from "./~kit/view/code.js";
import { BLOCK, CHUNK, LINE, PLAIN_OVER, blockText, classesOf, linesOf, loadOrder, runsOf, shown, sizeWords, widest, type Run, type Tok } from "./lines.js";

type Prism = { languages: Record<string, unknown>; tokenize(text: string, grammar: unknown): Tok[] };

const main = document.getElementById("code")!;

/** The view's state before its file is drawn: opening, or it didn't open and why. */
function say(state: "opening" | "failed" | "empty", why?: string) {
  const p = document.createElement("div");
  p.className = "cd-state";
  p.dataset.state = state;
  const head = document.createElement("p");
  head.textContent = state === "opening" ? viewkit.opening : state === "empty" ? viewkit.code_empty : viewkit.didnt_open;
  p.append(head);
  if (why) {
    const reason = document.createElement("p");
    reason.className = "why";
    reason.textContent = why;
    p.append(reason);
  }
  main.replaceChildren(p);
}

/** The file's text: one read when it is small, else by ranges of CHUNK bytes. */
async function readAll(view: View, size: number): Promise<string> {
  if (size <= CHUNK) {
    const r = await view.read();
    return r.text ?? new TextDecoder("utf-8").decode(r.bytes ?? new Uint8Array());
  }
  const dec = new TextDecoder("utf-8");
  let out = "";
  for (let at = 0; at < size; at += CHUNK) {
    const r = await view.read({ range: { offset: at, length: Math.min(CHUNK, size - at) } });
    out += r.text ?? dec.decode(r.bytes ?? new Uint8Array(), { stream: true });
  }
  return out + dec.decode();
}

/** prism and the grammars `lang` needs, loaded once each; null: no grammar (plain text). */
async function prismFor(lang: string | undefined): Promise<Prism | null> {
  const order = loadOrder(lang);
  if (order.length === 0) return null;
  const g = globalThis as { Prism?: Prism & { manual?: boolean } };
  // prism-core highlights the page by itself unless told it is manual
  if (g.Prism === undefined) (g as Record<string, unknown>)["Prism"] = { manual: true, disableWorkerMessageHandler: true };
  await import("./prism/core.js");
  for (const l of order) if (g.Prism!.languages[l] === undefined) await import(`./prism/${l}.js`);
  return g.Prism ?? null;
}

/** One line's row: its number (not selectable) and its runs, each a text node in its classes. */
function rowOf(n: number, text: string, runs: readonly Run[] | null, digits: number): HTMLElement {
  const row = document.createElement("div");
  row.className = "cd-row";
  row.style.top = `${n * LINE}px`;
  row.style.position = "absolute";
  const num = document.createElement("span");
  num.className = "cd-n";
  num.style.width = `${digits}ch`;
  num.textContent = String(n + 1);
  const t = document.createElement("span");
  t.className = "cd-t";
  if (runs === null) t.textContent = text;
  else
    for (const r of runs) {
      const cls = classesOf(r.types, r.text);
      if (cls === "") t.append(document.createTextNode(r.text));
      else {
        const s = document.createElement("span");
        s.className = cls;
        s.textContent = r.text;
        t.append(s);
      }
    }
  // an empty line keeps its height and its newline when copied
  if (text === "") t.textContent = "\n";
  row.append(num, t);
  return row;
}

/** The lines drawn as the scroll moves: only [from, to) are in the page. */
function draw(view: View, lines: string[], prism: Prism | null, lang: string | undefined, note: string | null) {
  const scroll = document.createElement("div");
  scroll.className = "cd-scroll";
  scroll.tabIndex = 0;
  scroll.dataset.testid = "code.lines";
  const sizer = document.createElement("div");
  sizer.className = "cd-sizer";
  sizer.style.height = `${lines.length * LINE}px`;
  const digits = Math.max(2, String(lines.length).length);
  // the widest line and the gutter (digits, 16 + 12 px) set the width the code scrolls by
  sizer.style.width = `calc(${widest(lines) + digits}ch + 44px)`;
  const rows = document.createElement("div");
  rows.className = "cd-rows";
  sizer.append(rows);
  scroll.append(sizer);
  const parts: HTMLElement[] = [];
  if (note !== null) {
    const p = document.createElement("p");
    p.className = "cd-note";
    p.dataset.testid = "code.note";
    p.textContent = note;
    parts.push(p);
  }
  main.replaceChildren(...parts, scroll);
  main.dataset.lang = lang ?? "";

  // a block's runs, tokenized once, when it first comes on screen
  const colored = new Map<number, Run[][]>();
  const runsAt = (n: number): Run[] | null => {
    if (prism === null || lang === undefined) return null;
    const b = Math.floor(n / BLOCK);
    let got = colored.get(b);
    if (got === undefined) {
      const { text, skip } = blockText(lines, b);
      got = runsOf(prism.tokenize(text, prism.languages[lang])).slice(skip);
      colored.set(b, got);
    }
    return got[n - b * BLOCK] ?? null;
  };

  let drawn = { from: -1, to: -1 };
  const paint = () => {
    const { from, to } = shown(scroll.scrollTop, scroll.clientHeight || 800, lines.length);
    if (from === drawn.from && to === drawn.to) return;
    drawn = { from, to };
    const out: HTMLElement[] = [];
    for (let n = from; n < to; n++) out.push(rowOf(n, lines[n] ?? "", runsAt(n), digits));
    rows.replaceChildren(...out);
  };
  let queued = false;
  scroll.addEventListener("scroll", () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      paint();
      view.setState({ top: scroll.scrollTop, left: scroll.scrollLeft });
    });
  });
  new ResizeObserver(() => paint()).observe(scroll);

  // ⌘A then ⌘C: the whole file, not only the lines drawn
  let all = false;
  scroll.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "a") {
      e.preventDefault();
      all = true;
      const sel = getSelection();
      sel?.selectAllChildren(rows);
    }
  });
  scroll.addEventListener("mousedown", () => {
    all = false;
  });
  document.addEventListener("copy", (e) => {
    if (!all || e.clipboardData === null) return;
    e.clipboardData.setData("text/plain", lines.join("\n"));
    e.preventDefault();
  });

  // where he was, when the tab is drawn again
  const st = view.state() as { top?: number; left?: number } | undefined;
  paint();
  if (st && typeof st.top === "number") {
    scroll.scrollTop = st.top;
    scroll.scrollLeft = typeof st.left === "number" ? st.left : 0;
    paint();
  }
  scroll.focus({ preventScroll: true });
}

async function start() {
  say("opening");
  let view: View;
  try {
    view = await connect({ name: "code" });
  } catch (e) {
    say("failed", e instanceof Error ? e.message : String(e));
    return;
  }
  const doc = view.document();
  if (doc === null) {
    say("failed");
    return;
  }
  try {
    const text = await readAll(view, doc.size);
    const lines = linesOf(text);
    if (lines.length === 0) {
      say("empty");
      return;
    }
    const lang = langOf(doc.name, lines[0] ?? "");
    const big = text.length > PLAIN_OVER;
    const prism = big ? null : await prismFor(lang).catch(() => null);
    draw(view, lines, prism, prism === null ? undefined : lang, big ? viewkit.code_plain({ size: sizeWords(PLAIN_OVER) }) : null);
  } catch (e) {
    say("failed", e instanceof Error ? e.message : String(e));
  }
}

void start();
