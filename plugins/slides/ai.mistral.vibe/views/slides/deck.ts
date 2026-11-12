// Slides · the deck (docs/plugin-apps.md, migration step 3; office-artifacts plan §4): the slides
// module's outline (the office SDK at ~kit/office/v1/office.js: slides.open(bytes).inspect(),
// office-slides m_23797: boxes in pt, colors resolved) as HTML, one 16:9 (or the deck's) box per
// slide scaled to the frame, a slide rail on the left, ←/→ to move. Text with its paragraphs,
// filled and outlined shapes, pictures (their bytes from the file's zip), tables, and charts as
// SVG from their data. A click on a shape selects it. Not drawn, said in the foot: what the
// module marks notDrawn (SmartArt and animations show their text only).
//
// A note's anchor is the canonical JSON of the old viewer's anchor minus its quote (sorted keys:
// architect m_60380): a shape `{"kind":"shape","shape":"4","slide":2}`, a slide
// `{"kind":"slide","slide":2}` (a box on it keeps its `rect`), so the notes made before place
// too. A shape's note marks the shape and its slide's thumb; a slide's, its thumb.
//
// Pure helpers (plugins/slides/test/deck.test.mjs): pct, shapeKey, slideKey, anchorOf.

import { slides, openPackage } from "./~kit/office/v1/office.js";
import type { OutlineNode, Package } from "./~kit/office/v1/office.js";

// what slides' inspect() says in a node's props (office/sdk/src/slides), the fields drawn here
type Chart = { series?: { values?: (number | null)[] }[]; categories?: unknown[]; title?: string; type?: string };
type Para = { text?: string; bullet?: string; level?: number; size?: number; bold?: boolean; color?: string; align?: string };
type Props = {
  valign?: string; paras?: Para[]; fontSize?: number; bold?: boolean; italic?: boolean; color?: string; align?: string;
  notDrawn?: string; box?: number[]; fill?: string; line?: string; geometry?: string; font?: string; image?: string;
  rows?: unknown[][]; chart?: Chart; w?: number; h?: number; background?: string;
};
type Ctx = { zip: Package["zip"] | null; urls: string[]; notDrawn: Set<string> };
type Slide = { n: number; w: number; h: number };
type Drawn = Slide & { box: HTMLDivElement; fit?: () => void; thumb?: Drawn };
const propsOf = (n: OutlineNode) => (n.props || {}) as Props;

const SVG = "http://www.w3.org/2000/svg";

/** What the view gives the deck: say his selection (its anchor, words and where it is). */
export type Host = { select(anchor: string | null, text: string | null, at: Element | null): void };

/** What the deck gives the view back. */
export type Deck = {
  /** what it could not draw, one line each (the foot) */
  notes: string[];
  place(): { slide: number };
  restore(place: unknown): void;
  onChange(fn: () => void): void;
  /** the parts a note's anchor names; [] when this version has none */
  find(anchor: string): Element[];
};

/** A box in pt as left/top/width/height in % of a slide of w × h pt. */
export function pct(box: number[], w: number, h: number) {
  const [x, y, bw, bh] = box;
  return { left: (x / w) * 100, top: (y / h) * 100, width: (bw / w) * 100, height: (bh / h) * 100 };
}

/** A shape's anchor: `{"kind":"shape","shape":"<id>","slide":<n>}`. */
export function shapeKey(slide: number, shape: string) {
  return JSON.stringify({ kind: "shape", shape, slide });
}

/** A slide's anchor: `{"kind":"slide","slide":<n>}`. */
export function slideKey(slide: number) {
  return JSON.stringify({ kind: "slide", slide });
}

/** The slide (and the shape) an anchor names; null when it is not one of a deck's. */
export function anchorOf(key: string): { slide: number; shape?: string } | null {
  try {
    const a = JSON.parse(key);
    if (!a || !Number.isInteger(a.slide) || a.slide < 1) return null;
    if (a.kind === "slide") return { slide: a.slide };
    if (a.kind === "shape" && typeof a.shape === "string") return { slide: a.slide, shape: a.shape };
    return null;
  } catch {
    return null;
  }
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string | null, text?: string) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function svgEl(tag: string, attrs: Record<string, string | number>) {
  const e = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  return e;
}

/** A chart's data as an SVG (bars, or lines for a line chart), in a 400 × 240 box. */
function chartSvg(chart: Chart | null | undefined) {
  const svg = svgEl("svg", { viewBox: "0 0 400 240", class: "s-chart", role: "img" });
  const series = (chart && chart.series) || [];
  const cats = (chart && chart.categories) || [];
  const values = series.flatMap((s) => (s.values || []).filter((v) => typeof v === "number"));
  if (chart && chart.title) {
    const t = svgEl("text", { x: 200, y: 18, "text-anchor": "middle", class: "title" });
    t.textContent = chart.title;
    svg.append(t);
  }
  if (values.length === 0) return svg;
  const max = Math.max(0, ...values);
  const min = Math.min(0, ...values);
  const [x0, y0, w, h] = [36, 30, 352, 180];
  const y = (v: number) => y0 + h - ((v - min) / (max - min || 1)) * h;
  svg.append(svgEl("line", { x1: x0, y1: y(0), x2: x0 + w, y2: y(0), class: "axis" }));
  const n = Math.max(cats.length, ...series.map((s) => (s.values || []).length));
  const line = /line|scatter/i.test((chart && chart.type) || "");
  series.forEach((s, si) => {
    const vs = s.values || [];
    if (line) {
      const pts = vs.map((v, i) => `${x0 + ((i + 0.5) / n) * w},${y(v || 0)}`).join(" ");
      svg.append(svgEl("polyline", { points: pts, class: `s${si % 6}`, fill: "none" }));
      return;
    }
    const bw = w / n / (series.length + 1);
    vs.forEach((v, i) => {
      if (typeof v !== "number") return;
      const top = Math.min(y(v), y(0));
      svg.append(svgEl("rect", { x: x0 + (i * w) / n + bw * (si + 0.5), y: top, width: bw, height: Math.abs(y(v) - y(0)), class: `s${si % 6}` }));
    });
  });
  cats.forEach((c, i) => {
    const t = svgEl("text", { x: x0 + ((i + 0.5) / n) * w, y: y0 + h + 16, "text-anchor": "middle", class: "cat" });
    t.textContent = String(c);
    svg.append(t);
  });
  return svg;
}

/** A picture's bytes as an object URL, or null when the part is not in the file. */
function pictureUrl(zip: Ctx["zip"], path: string | undefined, urls: string[]) {
  if (!path || !zip) return null;
  const b = zip.bytes(String(path).replace(/^\//, ""));
  if (!b) return null;
  const ext = String(path).split(".").pop()!.toLowerCase();
  const type = ({ png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", svg: "image/svg+xml", webp: "image/webp" } as Record<string, string>)[ext];
  if (!type) return null;
  const u = URL.createObjectURL(new Blob([b as Uint8Array<ArrayBuffer>], { type }));
  urls.push(u);
  return u;
}

function textOf(shape: OutlineNode) {
  const p = propsOf(shape);
  const box = el("div", "txt");
  if (p.valign) box.dataset.valign = p.valign;
  const paras: Para[] = p.paras && p.paras.length ? p.paras : shape.text ? String(shape.text).split("\n").map((t) => ({ text: t })) : [];
  for (const para of paras) {
    const e = el("p", null, para.text || "\u00a0");
    if (para.bullet) e.dataset.bullet = para.bullet;
    if (para.level) e.style.marginLeft = `${para.level * 1.2}em`;
    const size = para.size || p.fontSize;
    if (size) e.style.fontSize = `${size}pt`;
    if (para.bold ?? p.bold) e.style.fontWeight = "600";
    if (p.italic) e.style.fontStyle = "italic";
    if (para.color || p.color) e.style.color = (para.color || p.color) as string;
    if (para.align || p.align) e.style.textAlign = (para.align || p.align) as string;
    box.append(e);
  }
  return box;
}

/** One shape (or a group's children, already in slide coordinates) into the slide box. */
function drawShape(into: HTMLElement, shape: OutlineNode, slide: Slide, ctx: Ctx) {
  const p = propsOf(shape);
  if (shape.type === "group") {
    for (const c of shape.children || []) drawShape(into, c, slide, ctx);
    return;
  }
  if (p.notDrawn) ctx.notDrawn.add(p.notDrawn);
  if (!Array.isArray(p.box)) return;
  const b = pct(p.box, slide.w, slide.h);
  const e = el("div", `shape ${shape.type}`);
  Object.assign(e.style, { left: `${b.left}%`, top: `${b.top}%`, width: `${b.width}%`, height: `${b.height}%` });
  e.dataset.slide = String(slide.n);
  const an = shape.anchor as { shape?: string | number } | undefined;
  e.dataset.shape = String(an && an.shape ? an.shape : shape.id);
  // "none": no fill or no outline (office-slides m_23821)
  if (p.fill && p.fill !== "none") e.style.background = p.fill;
  if (p.line && p.line !== "none") e.style.border = `1pt solid ${p.line}`;
  if (p.geometry === "ellipse") e.style.borderRadius = "50%";
  if (p.font) e.style.fontFamily = `"${p.font}", sans-serif`;
  if (shape.type === "picture") {
    const u = pictureUrl(ctx.zip, p.image, ctx.urls);
    if (u) {
      const img = el("img");
      img.src = u;
      img.alt = shape.name || "";
      e.append(img);
    } else e.append(el("span", "missing", shape.name || "picture"));
  } else if (shape.type === "table" && Array.isArray(p.rows)) {
    const t = el("table");
    for (const r of p.rows) {
      const tr = el("tr");
      for (const c of r) tr.append(el("td", null, String(c ?? "")));
      t.append(tr);
    }
    e.append(t);
  } else if (shape.type === "chart") {
    e.append(chartSvg(p.chart));
  } else if (shape.text || (p.paras && p.paras.length)) {
    e.append(textOf(shape));
  }
  into.append(e);
}

/** One slide as a fixed w × h pt box; the frame scales it (fit()). */
function drawSlide(node: OutlineNode, n: number, ctx: Ctx): Drawn {
  const p = propsOf(node);
  const slide = { n, w: p.w || 960, h: p.h || 540 };
  const box = el("div", "slide");
  box.dataset.slide = String(n);
  Object.assign(box.style, { width: `${slide.w}pt`, height: `${slide.h}pt` });
  if (p.background) box.style.background = p.background;
  for (const s of node.children || []) drawShape(box, s, slide, ctx);
  return { box, ...slide };
}

export async function draw(root: HTMLElement, bytes: Uint8Array, host: Host): Promise<Deck> {
  const deck = await slides.open(bytes);
  const nodes = deck.inspect().nodes.filter((n) => n.type === "slide");
  let zip: Ctx["zip"] = null;
  try {
    zip = openPackage(bytes).zip;
  } catch {}
  const ctx: Ctx = { zip, urls: [], notDrawn: new Set() };
  root.classList.add("s-deck");
  const rail = el("nav", "s-rail");
  rail.setAttribute("aria-label", "slides");
  const stage = el("div", "s-stage");
  stage.tabIndex = 0;
  root.append(rail, stage);

  const big = nodes.map((n, i) => drawSlide(n, i + 1, ctx));
  let current = 0;
  const changed = new Set<() => void>();

  /** Scale a w × h pt slide to `width` px wide (1pt = 4/3 px). */
  function scaleTo(holder: HTMLElement, s: Drawn, width: number) {
    const k = width / ((s.w * 4) / 3);
    holder.style.height = `${((s.h * 4) / 3) * k}px`;
    s.box.style.transform = `scale(${k})`;
  }

  for (const [i, s] of big.entries()) {
    const thumb = drawSlide(nodes[i], i + 1, { ...ctx, notDrawn: new Set() });
    const b = el("button", "thumb");
    b.type = "button";
    b.dataset.slide = String(i + 1);
    b.setAttribute("aria-label", `slide ${i + 1}`);
    const holder = el("div", "holder");
    holder.append(thumb.box);
    b.append(el("span", "n", String(i + 1)), holder);
    b.addEventListener("click", () => go(i));
    rail.append(b);
    thumb.fit = () => scaleTo(holder, thumb, holder.clientWidth || 120);
    s.thumb = thumb;
  }

  function fit() {
    const s = big[current];
    if (!s) return;
    const holder = stage.firstElementChild as HTMLElement;
    scaleTo(holder, s, stage.clientWidth - 16);
    for (const t of big) t.thumb!.fit!();
  }

  function select(shape: HTMLElement | null) {
    for (const x of stage.querySelectorAll(".shape.sel")) x.classList.remove("sel");
    if (!shape) return host.select(null, null, null);
    shape.classList.add("sel");
    const slide = Number(shape.dataset.slide);
    host.select(shapeKey(slide, shape.dataset.shape!), (shape.textContent || "").replace(/\s+/g, " ").trim() || null, shape);
  }

  function go(i: number) {
    if (big.length === 0) {
      stage.replaceChildren(el("p", "s-empty", "this deck has no slides."));
      return;
    }
    const next = Math.max(0, Math.min(big.length - 1, i));
    const moved = next !== current;
    current = next;
    const holder = el("div", "holder");
    holder.append(big[current].box);
    stage.replaceChildren(holder);
    for (const b of rail.children) b.setAttribute("aria-current", String(b === rail.children[current]));
    rail.children[current]?.scrollIntoView({ block: "nearest" });
    fit();
    select(null);
    if (moved) changed.forEach((fn) => fn());
  }

  new ResizeObserver(fit).observe(stage);
  root.addEventListener("keydown", (e) => {
    if (e.key === "ArrowRight" || e.key === "ArrowDown" || e.key === "PageDown") go(current + 1);
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp" || e.key === "PageUp") go(current - 1);
    else return;
    e.preventDefault();
  });
  stage.addEventListener("click", (e) => select((e.target as Element).closest<HTMLElement>(".shape")));
  current = -1;
  go(0);

  const words: Record<string, string> = { smartart: "SmartArt shows its text only", animation: "animations are not played" };
  return {
    notes: [...ctx.notDrawn].map((k) => `${words[k] || `${k} is not drawn`} here.`),
    place: () => ({ slide: current + 1 }),
    restore(p) {
      const at = p as { slide?: number } | null;
      if (at && Number.isInteger(at.slide)) go(at.slide! - 1);
    },
    onChange: (fn) => changed.add(fn),
    find(anchor) {
      const a = anchorOf(anchor);
      const s = a && big[a.slide - 1];
      if (!a || !s) return [];
      const thumb = rail.children[a.slide - 1];
      if (!a.shape) return [thumb];
      const shape = s.box.querySelector(`.shape[data-shape="${CSS.escape(a.shape)}"]`);
      return shape ? [shape, thumb] : [];
    },
  };
}
