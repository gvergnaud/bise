// Preview · an image (png, jpg, gif, webp): the picture fit to the view, and a box drawn on it
// with the pointer as his selection, a `region` place in % of the image (keys.ts). His notes'
// boxes are drawn on it, the kit marks them (data-bise-mark).
//
// It zooms (his m_22822; image-pinch, designer m_30627, architect m_30628), the math is zoom.ts's:
// a trackpad pinch (Chromium's wheel + ctrlKey) zooms the image around the pointer, never the
// window; two fingers or the wheel (shift: sideways) pan it while zoomed, and space+drag for a
// mouse; a plain drag always draws a box, zoomed or not; a double-click toggles fit and 100%;
// esc, ⌘0 or a click on the '240%' chip go back to fit. The .frame (the img and every box, all in
// % of it) is sized k × the image and moved, never scaled: boxes and marks ride the zoom, their
// lines stay 1 px; past 200% its pixels show (designer m_30807). The zoom is the view's state: a
// tab unloaded and drawn again comes back zoomed where he was.

import { capture } from "./dom.js";
import { boxPct, isClick, type Place, type Rect } from "./keys.js";
import type { Ctx, Shown, Source } from "./shown.js";
import { fitted, isFit, isFitKey, panBy, pinchFactor, place, toggled, wheelPan, zoomAt, zoomWords } from "./zoom.js";
import type { Dims, Zoom } from "./zoom.js";

const MIME: Record<string, string> = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp" };

/** How long the chip stays after the last zoom step, then it fades (designer m_30627). */
const CHIP_MS = 1500;

/** A field he types in: space is his there, never a pan. */
const typing = (t: EventTarget | null) => !!t && ((t as HTMLElement).isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test((t as HTMLElement).tagName || ""));

/** The state this view keeps: the image's size and its zoom (none at fit). */
type State = { nw: number; nh: number; zoom: Zoom | null };

const boxAt = (el: HTMLElement, r: Rect) => Object.assign(el.style, { left: `${r.x}%`, top: `${r.y}%`, width: `${r.w}%`, height: `${r.h}%` });

export async function draw(root: HTMLElement, src: Source, ctx: Ctx): Promise<Shown> {
  root.classList.add("pv-image");
  const ext = src.name.split(".").pop()!.toLowerCase();
  const url = src.url ?? URL.createObjectURL(new Blob([src.bytes as Uint8Array<ArrayBuffer>], { type: MIME[ext] || "image/png" }));
  const frame = document.createElement("div");
  frame.className = "frame";
  const img = document.createElement("img");
  img.alt = src.name;
  img.draggable = false;
  frame.append(img);
  const chip = document.createElement("button");
  chip.type = "button";
  chip.className = "pv-zoom";
  chip.title = "fit · ⌘0";
  chip.hidden = true;
  root.append(frame, chip);
  await new Promise<void>((ok, no) => {
    img.onload = () => ok();
    img.onerror = () => no(Object.assign(new Error("not an image this browser draws"), { code: "unreadable" }));
    img.src = url;
  });

  // ---- the zoom: the box's and the image's sizes, where it is drawn
  const nw = img.naturalWidth;
  const nh = img.naturalHeight;
  let d: Dims = { bw: 0, bh: 0, nw, nh };
  let z: Zoom = { k: 1, x: 0, y: 0 };
  // a zoom put back (restore), applied once the box has its size
  let pending: Zoom | null = null;
  let chipTimer = 0;
  const apply = (next: Zoom, ease = false) => {
    const was = z;
    z = next;
    root.classList.toggle("eased", ease);
    Object.assign(frame.style, { width: `${nw * z.k}px`, height: `${nh * z.k}px`, transform: `translate(${z.x}px, ${z.y}px)` });
    const fit = isFit(d, z);
    root.dataset.fit = String(fit);
    root.dataset.zoom = zoomWords(z);
    // past 200% its pixels show, as in Figma; smooth up to it (designer m_30807)
    root.classList.toggle("pixels", z.k > 2);
    if (fit) {
      chip.hidden = true;
      chip.classList.remove("on");
      clearTimeout(chipTimer);
    }
    if (was.k !== z.k || was.x !== z.x || was.y !== z.y) ctx.moved();
  };
  /** The chip says the zoom while he zooms and CHIP_MS after; never at fit. */
  const showChip = () => {
    if (isFit(d, z)) return;
    chip.textContent = zoomWords(z);
    chip.hidden = false;
    chip.classList.add("on");
    clearTimeout(chipTimer);
    chipTimer = setTimeout(() => chip.classList.remove("on"), CHIP_MS);
  };
  const toFit = () => apply(fitted(d), true);
  const rel = (e: MouseEvent) => {
    const r = root.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };

  // the box's size: a fitted image refits, a zoomed one keeps its place, held at the edges
  new ResizeObserver(() => {
    if (!root.isConnected) return;
    const was = d.bw > 0 && isFit(d, z);
    d = { bw: root.clientWidth, bh: root.clientHeight, nw, nh };
    if (d.bw <= 0 || d.bh <= 0) return;
    const back = pending;
    pending = null;
    apply(back ? place(d, back) : was || z.k === 1 ? fitted(d) : place(d, z));
  }).observe(root);

  // a pinch zooms here, never the window; two fingers or the wheel pan while zoomed
  root.addEventListener(
    "wheel",
    (e) => {
      if (e.ctrlKey) {
        e.preventDefault();
        const p = rel(e);
        apply(zoomAt(d, z, z.k * pinchFactor(e.deltaY, e.deltaMode), p.x, p.y));
        showChip();
        return;
      }
      if (isFit(d, z)) return;
      e.preventDefault();
      const m = wheelPan(e.deltaX, e.deltaY, e.deltaMode, e.shiftKey);
      apply(panBy(d, z, m.dx, m.dy));
    },
    { passive: false },
  );
  root.addEventListener("dblclick", (e) => {
    if (e.target === chip) return;
    const p = rel(e);
    apply(toggled(d, z, p.x, p.y), true);
    showChip();
  });
  chip.addEventListener("click", toFit);

  // esc and ⌘0 while zoomed: back to fit, used; capture, before the kit reads a typed key as
  // his first word. Space held: a drag pans.
  let space = false;
  const spaceUp = () => {
    space = false;
    root.classList.remove("panning");
  };
  document.addEventListener(
    "keydown",
    (e) => {
      if (!root.isConnected) return;
      if (e.code === "Space" && !typing(e.target) && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        space = true;
        root.classList.add("panning");
        return;
      }
      if (isFit(d, z) || e.repeat || !isFitKey(e)) return;
      e.preventDefault();
      e.stopPropagation();
      toFit();
    },
    true,
  );
  document.addEventListener("keyup", (e) => e.code === "Space" && spaceUp());
  window.addEventListener("blur", spaceUp);

  // ---- the box: a plain drag on the image, zoomed or not (designer m_30627)
  type XY = { x: number; y: number };
  let start: XY | null = null;
  let pan: XY | null = null;
  const box = document.createElement("div");
  box.className = "pv-box";
  // the pointer in % of the image, read against the image as drawn at that moment: a press and
  // its release during a zoom's ease (160 ms) fall on the same image point, never a box between
  const at = (e: MouseEvent) => {
    const r = img.getBoundingClientRect();
    const pct = (v: number, size: number) => (size > 0 ? Math.max(0, Math.min(100, (v / size) * 100)) : 0);
    return { x: pct(e.clientX - r.left, r.width), y: pct(e.clientY - r.top, r.height) };
  };
  const show = (rect: Rect) => {
    boxAt(box, rect);
    if (!box.isConnected) frame.append(box);
  };
  root.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || e.target === chip) return;
    if (space) {
      pan = { x: e.clientX, y: e.clientY };
      root.classList.add("grabbing");
      capture(root, e.pointerId);
      return;
    }
    if (!frame.contains(e.target as Node)) return;
    start = at(e);
    capture(root, e.pointerId);
  });
  // a box drawn selects nothing on the way
  root.addEventListener("selectstart", (e) => (start || pan) && e.preventDefault());
  root.addEventListener("pointermove", (e) => {
    if (pan) {
      const from = pan;
      pan = { x: e.clientX, y: e.clientY };
      apply(panBy(d, z, e.clientX - from.x, e.clientY - from.y));
      return;
    }
    if (start) show(boxPct(start, at(e), 100, 100));
  });
  const up = (e: PointerEvent) => {
    if (pan) {
      pan = null;
      root.classList.remove("grabbing");
      return;
    }
    if (!start) return;
    const rect = boxPct(start, at(e), 100, 100);
    start = null;
    // a click, not a box: the selection goes (a double-click never leaves one)
    if (isClick(rect)) {
      box.remove();
      ctx.select(null);
      return;
    }
    show(rect);
    ctx.select({ kind: "region", rect }, null, box);
  };
  root.addEventListener("pointerup", up);
  root.addEventListener("pointercancel", up);

  // ---- his notes' boxes: one element per place, kept; the kit marks the ones it places
  const marks = new Map<string, HTMLElement>();
  return {
    foot: [],
    place(p: Place) {
      // a box on an image has no page and no moment; another kind is not this image's
      if (p.kind !== "region" || p.page !== undefined) return null;
      const k = `${p.rect.x},${p.rect.y},${p.rect.w},${p.rect.h}`;
      let m = marks.get(k);
      if (!m) {
        m = document.createElement("div");
        m.className = "pv-mark";
        boxAt(m, p.rect);
        frame.append(m);
        marks.set(k, m);
      }
      return [m];
    },
    state: (): State => ({ nw, nh, zoom: d.bw > 0 && !isFit(d, z) ? { ...z } : null }),
    restore(s) {
      const st = s as Partial<State> | null;
      if (!st || !st.zoom || st.nw !== nw || st.nh !== nh) return;
      if (d.bw > 0) apply(place(d, st.zoom));
      else pending = st.zoom;
    },
  };
}
