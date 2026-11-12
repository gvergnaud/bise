// Preview · a pdf (pdf-notes, architect m_24668): pdf.js (pdfjs.js and its worker
// pdf.worker.js, built from plugins/media/src/ by office/sdk's esbuild into this folder, served
// under the view's own script rule) draws each page on a canvas when it scrolls near, with its
// text layer over it for the browser's own selection. Two selections, both a `region` place in %
// of the page (keys.ts): a passage (a drag on words: its words' box, the words its quote) and a
// box (a drag that starts off the words). A note's place on this version is found again by its
// words (pdftext.ts).
//
// pdf.js on an untrusted file (architect m_24668, pinned by plugins/media/test/pdf.test.mjs): no
// eval, no pdf JavaScript, no annotation layer (a link never navigates the frame), nothing
// fetched but the file: the standard fonts, cMaps and wasm decoders are not served; what needed
// them is said in the foot. A pdf bigger than one read is read by its ranges from the session
// url (resources/url), the same file.
//
// Pure helpers (exported for plugins/media/test/pdf.test.mjs): OPTIONS, rectPct, footOf.

import { getDocument, GlobalWorkerOptions, TextLayer, Util } from "./pdfjs.js";
import { capture } from "./dom.js";
import type { Place, Rect } from "./keys.js";
import { itemsIn, pageText, place as placeOn, type Item, type Span } from "./pdftext.js";
import type { Ctx, Shown, Source } from "./shown.js";

// pdf.js's types, read off its functions (the bundle exports no type names)
type PdfInit = Parameters<typeof getDocument>[0];
type PdfDoc = Awaited<ReturnType<typeof getDocument>["promise"]>;
type PdfPage = Awaited<ReturnType<PdfDoc["getPage"]>>;
type PdfText = Awaited<ReturnType<PdfPage["getTextContent"]>>;
/** A page as this view keeps it: pdf.js's, its text for the notes, its element and its draw. */
type Pg = {
  n: number; page: PdfPage; vp: ReturnType<PdfPage["getViewport"]>; content: PdfText; text: string; spans: Span[];
  el: HTMLDivElement; drawn: number; task: ReturnType<PdfPage["render"]> | null;
};
/** A box in px: {left, top, width, height}. */
type PxBox = { left: number; top: number; width: number; height: number };

/** pdf.js's options for an untrusted file (architect m_24668). */
export const OPTIONS = Object.freeze({
  isEvalSupported: false,
  enableScripting: false,
  enableXfa: false,
  useWasm: false,
  useWorkerFetch: false,
  cMapUrl: null,
  standardFontDataUrl: null,
  wasmUrl: null,
  stopAtErrors: false,
});

/** How much of a big pdf pdf.js asks for at a time, by its ranges. */
const CHUNK = 1024 * 1024;

/** A box (px, in an element at `frame`: {left, top, width, height}) in % of it, 0.1 apart, clamped. */
export function rectPct(box: PxBox, frame: PxBox): Rect {
  const c = (v: number) => Math.max(0, Math.min(100, Math.round(v * 10) / 10));
  const x0 = c(((box.left - frame.left) / frame.width) * 100);
  const y0 = c(((box.top - frame.top) / frame.height) * 100);
  const x1 = c(((box.left + box.width - frame.left) / frame.width) * 100);
  const y1 = c(((box.top + box.height - frame.top) / frame.height) * 100);
  return { x: x0, y: y0, w: Math.round((x1 - x0) * 10) / 10, h: Math.round((y1 - y0) * 10) / 10 };
}

/** The foot's lines for what pdf.js asked for and Preview does not serve (designer signs). */
export function footOf(missing: Set<string>) {
  const out: string[] = [];
  if (missing.has("cMapUrl")) out.push("some characters may not show: this pdf needs character maps Preview doesn't ship (often Chinese, Japanese or Korean text).");
  if (missing.has("standardFontDataUrl")) out.push("some text is drawn in a stand-in font: this pdf names fonts it doesn't carry.");
  if (missing.has("wasmUrl")) out.push("some images may not show (JPEG 2000 or JBIG2).");
  return out;
}

/** The longest quote a passage keeps (bise_proto::anchor::QUOTE_MAX). */
const QUOTE_MAX = 200;

export async function draw(root: HTMLElement, src: Source, ctx: Ctx): Promise<Shown> {
  root.classList.add("pv-pdf");
  GlobalWorkerOptions.workerSrc = new URL("./pdf.worker.js", import.meta.url).href;
  // what pdf.js asks the page for (fonts, cMaps, decoders): never fetched, only noted
  const missing = new Set<string>();
  class Unserved {
    async fetch({ kind }: { kind: string }) {
      missing.add(kind);
      throw new Error(`Preview doesn't serve pdf.js's ${kind}`);
    }
  }
  const from = src.url ? { url: src.url, rangeChunkSize: CHUNK, disableAutoFetch: true, disableStream: true } : { data: src.bytes!.slice() };
  // (null urls: pdf.js reads them as none; its types say string | undefined)
  const doc = await getDocument({ ...OPTIONS, ...from, BinaryDataFactory: Unserved } as unknown as PdfInit).promise.catch((e) => {
    throw Object.assign(new Error(e && e.message ? e.message : String(e)), { code: "unreadable" });
  });

  // every page's size and text first (the notes are found again on it), drawn later as they show
  const pages: Pg[] = [];
  for (let n = 1; n <= doc.numPages; n++) {
    const page = await doc.getPage(n);
    const vp = page.getViewport({ scale: 1 });
    const content = await page.getTextContent();
    // (el: set just below, before anything reads it)
    pages.push({ n, page, vp, content, ...pageText(content.items as Item[]), el: null as unknown as HTMLDivElement, drawn: 0, task: null });
  }
  const texts = pages.map((p) => p.text);

  const list = document.createElement("div");
  list.className = "pv-pages";
  for (const p of pages) {
    const el = document.createElement("div");
    el.className = "page";
    el.dataset.page = String(p.n);
    el.style.aspectRatio = `${p.vp.width} / ${p.vp.height}`;
    el.style.maxWidth = `${Math.round(p.vp.width * 1.5)}px`;
    p.el = el;
    list.append(el);
  }
  root.append(list);

  // ---- drawing a page: its canvas at the device's pixels, its text layer at its css size
  async function render(p: Pg) {
    const width = p.el.clientWidth;
    if (!width || p.drawn === width) return;
    p.drawn = width;
    if (p.task) p.task.cancel();
    const scale = width / p.vp.width;
    const vp = p.page.getViewport({ scale });
    const dpr = window.devicePixelRatio || 1;
    const canvas = document.createElement("canvas");
    canvas.width = Math.floor(vp.width * dpr);
    canvas.height = Math.floor(vp.height * dpr);
    const text = document.createElement("div");
    text.className = "textLayer";
    p.el.style.setProperty("--total-scale-factor", String(scale));
    // (transform null: none, as pdf.js reads it; its types say undefined)
    p.task = p.page.render({ canvas, canvasContext: canvas.getContext("2d")!, viewport: vp, transform: (dpr === 1 ? null : [dpr, 0, 0, dpr, 0, 0]) as number[], annotationMode: 0 });
    try {
      await p.task.promise;
    } catch {
      return; // cancelled by a newer draw
    }
    await new TextLayer({ textContentSource: p.content, container: text, viewport: vp }).render().catch(() => {});
    for (const old of p.el.querySelectorAll(":scope > canvas, :scope > .textLayer")) old.remove();
    p.el.prepend(canvas, text);
  }
  const near = new IntersectionObserver((es) => es.forEach((e) => e.isIntersecting && render(pages[Number((e.target as HTMLElement).dataset.page) - 1])), { root, rootMargin: "800px 0px" });
  pages.forEach((p) => near.observe(p.el));
  let resized = 0;
  new ResizeObserver(() => {
    clearTimeout(resized);
    resized = setTimeout(() => pages.forEach((p) => p.drawn && render(p)), 150);
  }).observe(root);
  let scrolled = 0;
  root.addEventListener("scroll", () => {
    clearTimeout(scrolled);
    scrolled = setTimeout(() => ctx.moved(), 200);
  });

  // ---- the selections: a passage (the browser's, on the text layer) or a box
  let start: { p: Pg; x: number; y: number; r: DOMRect } | null = null;
  const drawn = document.createElement("div");
  drawn.className = "pv-box";
  const pageOf = (node: Node | EventTarget | null) => {
    const el = node && ((node as Node).nodeType === 1 ? (node as Element) : (node as Node).parentElement);
    const pg = el && el.closest && el.closest<HTMLElement>(".pv-pdf .page");
    return pg ? pages[Number(pg.dataset.page) - 1] : null;
  };
  const pointIn = (p: Pg, e: MouseEvent) => {
    const r = p.el.getBoundingClientRect();
    const x = Math.max(r.left, Math.min(r.right, e.clientX));
    const y = Math.max(r.top, Math.min(r.bottom, e.clientY));
    return { x, y, r };
  };
  list.addEventListener("pointerdown", (e) => {
    const p = pageOf(e.target);
    if (e.button !== 0 || !p) return;
    // on words: the browser selects them; off them: a box
    if ((e.target as Element).closest(".textLayer span, .textLayer br")) {
      drawn.remove();
      return;
    }
    window.getSelection()?.removeAllRanges();
    start = { p, ...pointIn(p, e) };
    capture(list, e.pointerId);
  });
  // a box drawn selects no words on the way
  list.addEventListener("selectstart", (e) => start && e.preventDefault());
  // (start: a drag is on whenever this runs)
  const boxNow = (e: MouseEvent) => {
    const q = pointIn(start!.p, e);
    const b = { left: Math.min(start!.x, q.x), top: Math.min(start!.y, q.y), width: Math.abs(q.x - start!.x), height: Math.abs(q.y - start!.y) };
    return rectPct(b, q.r);
  };
  const show = (n: number, rect: Rect) => {
    Object.assign(drawn.style, { left: `${rect.x}%`, top: `${rect.y}%`, width: `${rect.w}%`, height: `${rect.h}%` });
    if (drawn.parentElement !== pages[n - 1].el) pages[n - 1].el.append(drawn);
  };
  list.addEventListener("pointermove", (e) => start && show(start.p.n, boxNow(e)));
  list.addEventListener("pointerup", (e) => {
    if (!start) return;
    const rect = boxNow(e);
    const n = start.p.n;
    start = null;
    // a click, not a box: the selection goes
    if (rect.w < 1 || rect.h < 1) {
      drawn.remove();
      ctx.select(null);
      return;
    }
    show(n, rect);
    ctx.select({ kind: "region", page: n, rect }, null, drawn);
  });

  /** The passage selected now: its page, its words' box, its words; or null. */
  function passage() {
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || !sel.rangeCount) return null;
    const range = sel.getRangeAt(0);
    const p = pageOf(range.startContainer);
    if (!p || !root.contains(range.startContainer)) return null;
    const words = String(sel.toString()).replace(/\s+/g, " ").trim();
    if (!words) return null;
    const frame = p.el.getBoundingClientRect();
    const rs = [...range.getClientRects()].filter((r) => r.width > 0 && r.height > 0 && r.bottom > frame.top && r.top < frame.bottom);
    if (!rs.length) return null;
    const left = Math.min(...rs.map((r) => r.left)), top = Math.min(...rs.map((r) => r.top));
    const right = Math.max(...rs.map((r) => r.right)), bottom = Math.max(...rs.map((r) => r.bottom));
    const rect = rectPct({ left, top, width: right - left, height: bottom - top }, frame);
    return { place: { kind: "region", page: p.n, rect } as Place, words: words.slice(0, QUOTE_MAX), at: new DOMRect(left, top, right - left, bottom - top) };
  }
  // the browser's selection settles on the pointer's release and the keys that move it
  const passageNow = () => {
    if (start) return;
    const s = passage();
    if (s) ctx.select(s.place, s.words, s.at);
    else if (!drawn.isConnected) ctx.select(null);
  };
  document.addEventListener("pointerup", () => setTimeout(passageNow, 0));
  document.addEventListener("keyup", (e) => e.shiftKey && passageNow());

  // ---- a note's place on this version: its marks (boxes in % of their page), kept by place
  function itemBox(p: Pg, i: number): PxBox {
    // (an item with words: itemsIn gives only those)
    const it = p.content.items[i] as { transform: number[]; width: number };
    const tx = Util.transform(p.vp.transform, it.transform);
    const h = Math.hypot(tx[2], tx[3]);
    const w = it.width * p.vp.scale;
    return { left: tx[4], top: tx[5] - h, width: w, height: h };
  }
  function mark(p: Pg, rect: Rect, cls: string) {
    const m = document.createElement("div");
    m.className = cls;
    Object.assign(m.style, { left: `${rect.x}%`, top: `${rect.y}%`, width: `${rect.w}%`, height: `${rect.h}%` });
    p.el.append(m);
    return m;
  }
  function marksOf(at: NonNullable<ReturnType<typeof placeOn>>): Element[] {
    const p = pages[at.page - 1];
    if ("rect" in at) return [mark(p, at.rect, "pv-mark")];
    // the words, item by item, merged by line
    const frame = { left: 0, top: 0, width: p.vp.width, height: p.vp.height };
    const lines: PxBox[] = [];
    for (const i of itemsIn(p.spans, at.start, at.end)) {
      const b = itemBox(p, i);
      const line = lines.find((l) => Math.abs(l.top - b.top) < b.height / 2);
      if (line) {
        const right = Math.max(line.left + line.width, b.left + b.width);
        line.left = Math.min(line.left, b.left);
        line.width = right - line.left;
      } else lines.push({ ...b });
    }
    if (!lines.length) return [];
    const els = lines.map((l) => mark(p, rectPct(l, frame), "pv-mark pv-mark-words"));
    // one 3 px bar down all its lines (designer m_24905)
    const top = Math.min(...lines.map((l) => l.top)), bottom = Math.max(...lines.map((l) => l.top + l.height));
    const bar = mark(p, rectPct({ left: Math.min(...lines.map((l) => l.left)), top, width: 0, height: bottom - top }, frame), "pv-mark pv-mark-bar");
    return [...els, bar];
  }
  const marks = new Map<string, Element[] | null>();

  return {
    foot: footOf(missing),
    place(pl: Place, quote: string) {
      if (pl.kind !== "region") return null;
      const k = JSON.stringify([pl.page ?? 1, pl.rect, quote]);
      if (!marks.has(k)) {
        const at = placeOn(pl, quote, texts);
        marks.set(k, at ? marksOf(at) : null);
      }
      return marks.get(k)!;
    },
    state: () => ({ top: root.scrollTop }),
    restore(s) {
      const top = (s as { top?: unknown } | null)?.top;
      if (typeof top === "number") root.scrollTop = top;
    },
  };
}
