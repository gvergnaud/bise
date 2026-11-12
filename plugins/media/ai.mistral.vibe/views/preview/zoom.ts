// Preview · an image's zoom (his m_22822; image-pinch, designer m_30627, architect m_30628): a
// trackpad pinch zooms the image itself around the pointer, never the window; two fingers, the
// wheel or space+drag pan it while zoomed; a double-click toggles fit and 100%; esc or ⌘0 back to
// fit. The math's one home, pure: image.js keeps the state and the DOM. Laws in
// plugins/media/test/zoom.test.mjs.
//
// Dims: the box and the image's natural size, in CSS px { bw, bh, nw, nh }.
// Zoom: the image drawn at `k` CSS px per image px, its top-left corner at (x, y) in the box.

export type Dims = { bw: number; bh: number; nw: number; nh: number };
export type Zoom = { k: number; x: number; y: number };

/** The most it zooms: 8 screen px per image px. */
export const MAX_K = 8;

/** The whole image in the box, never above 100% (a small image keeps its size). */
export function fitK(d: Dims) {
  if (d.nw <= 0 || d.nh <= 0 || d.bw <= 0 || d.bh <= 0) return 1;
  return Math.min(1, d.bw / d.nw, d.bh / d.nh);
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Centred on an axis where it is smaller than the box; else never a gap at an edge. */
export function place(d: Dims, z: Zoom): Zoom {
  const w = d.nw * z.k;
  const h = d.nh * z.k;
  return {
    k: z.k,
    x: w <= d.bw ? (d.bw - w) / 2 : clamp(z.x, d.bw - w, 0),
    y: h <= d.bh ? (d.bh - h) / 2 : clamp(z.y, d.bh - h, 0),
  };
}

/** The image fitted in its box, centred. */
export function fitted(d: Dims) {
  return place(d, { k: fitK(d), x: 0, y: 0 });
}

/** It shows whole (no chip, the wheel does nothing, esc closes the artifact). */
export function isFit(d: Dims, z: Zoom) {
  return Math.abs(z.k - fitK(d)) < 1e-3;
}

/** Zoom to `k` (between fit and MAX_K), the image point under (px, py) staying under it. */
export function zoomAt(d: Dims, z: Zoom, k: number, px: number, py: number) {
  const k2 = clamp(k, fitK(d), MAX_K);
  return place(d, { k: k2, x: px - ((px - z.x) * k2) / z.k, y: py - ((py - z.y) * k2) / z.k });
}

/** Pan by (dx, dy) screen px, held at the edges. */
export function panBy(d: Dims, z: Zoom, dx: number, dy: number) {
  return place(d, { k: z.k, x: z.x + dx, y: z.y + dy });
}

/** A pinch's wheel step (Chromium sends a trackpad pinch as wheel + ctrlKey) as a zoom factor. */
export function pinchFactor(deltaY: number, deltaMode: number) {
  const px = deltaMode === 1 ? deltaY * 16 : deltaY;
  return Math.exp(-px * 0.01);
}

/** A wheel's pan while zoomed, in screen px: shift+wheel goes sideways (a mouse's one wheel). */
export function wheelPan(deltaX: number, deltaY: number, deltaMode: number, shift: boolean) {
  const m = deltaMode === 1 ? 16 : 1;
  const sideways = shift && deltaX === 0;
  return { dx: -(sideways ? deltaY : deltaX) * m, dy: -(sideways ? 0 : deltaY) * m };
}

/** A double-click: fitted, to 100% (2× for an image smaller than its box) around the pointer; else back to fit. */
export function toggled(d: Dims, z: Zoom, px: number, py: number) {
  if (!isFit(d, z)) return fitted(d);
  return zoomAt(d, z, fitK(d) >= 1 ? 2 : 1, px, py);
}

/** The chip's words: '240%' of the image's own size. */
export const zoomWords = (z: Zoom) => `${String(Math.round(z.k * 100))}%`;

/** A key that takes a zoomed image back to fit: a plain esc, or ⌘0 (⌥⌘0 stays the window's). */
export function isFitKey(e: Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">) {
  const plain = !e.ctrlKey && !e.altKey && !e.shiftKey;
  return (e.key === "Escape" && plain && !e.metaKey) || (e.code === "Digit0" && plain && e.metaKey);
}
