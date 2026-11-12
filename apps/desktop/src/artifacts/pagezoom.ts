// A web page's pinch in the app (web-pinch, his m_45441; architect m_45447): as Safari and Chrome
// do on a Mac, a trackpad pinch zooms the page visually around the fingers, two fingers pan the
// zoomed page, a pinch out goes back to fit. Every page is a pages-server frame in the window's own
// webContents (no page has a webContents of its own), so Chromium's visual zoom would zoom the
// whole app: the window scales the iframe element instead. The page never reflows and its text
// zoom (⌘+/⌘-, the window's zoomFactor) is untouched. The frame's injected forwarder (main/pinch.ts)
// sends the wheels; useFrameNote.ts keeps one View per frame and draws it with these. The notes and
// selections are drawn inside the frame by the viewer, so they follow the same transform; an anchor
// is read in the page's own coordinates. Pure. Laws in pagezoom.test.ts. Owner: art-desktop.
//
// A View: the frame drawn at `z` (1 = fit) with its top-left corner at (tx, ty), in the window's CSS
// px, relative to the frame's own box (w × h). A point p of the frame's box shows at t + z·p.

/** The most it zooms: 8 screen px per page px, Preview's image zoom (plugins/media's zoom.ts MAX_K; a law checks they are equal). */
export const MAX_PAGE_ZOOM = 8;
/** how much one wheel pixel of a pinch zooms (Chromium sends a trackpad pinch as a ctrl wheel), as the image viewer's */
const PINCH_RATE = 0.01;

export interface View {
  readonly z: number;
  readonly tx: number;
  readonly ty: number;
}

/** The frame's box, CSS px. */
export interface Box {
  readonly w: number;
  readonly h: number;
}

export const FIT: View = { z: 1, tx: 0, ty: 0 };

const within = (v: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, v));

/** Whether the view is the fit (nothing to go back to). */
export function atFit(v: View): boolean {
  return v.z <= 1 + 1e-6;
}

/** A view held inside the box: z in [1, MAX_PAGE_ZOOM], never a gap at an edge; the fit is exactly FIT. */
export function clampView(v: View, b: Box): View {
  const z = within(Number.isFinite(v.z) ? v.z : 1, 1, MAX_PAGE_ZOOM);
  if (z <= 1 + 1e-6 || b.w <= 0 || b.h <= 0) return FIT;
  const tx = Number.isFinite(v.tx) ? v.tx : 0;
  const ty = Number.isFinite(v.ty) ? v.ty : 0;
  return { z, tx: within(tx, b.w * (1 - z), 0), ty: within(ty, b.h * (1 - z), 0) };
}

/** A pinch's wheel delta (px) as a zoom factor: fingers apart (deltaY < 0) zoom in. */
export function pinchFactor(deltaY: number): number {
  return Math.exp(-deltaY * PINCH_RATE);
}

/** Zoom by `factor` keeping the frame's point (x, y) (its box's px) under the fingers. */
export function zoomAt(v: View, factor: number, x: number, y: number, b: Box): View {
  const z = within(v.z * factor, 1, MAX_PAGE_ZOOM);
  // the point shows at t + z·p before and after: the same
  return clampView({ z, tx: v.tx + (v.z - z) * x, ty: v.ty + (v.z - z) * y }, b);
}

/** Two fingers moved the content by a wheel's (dx, dy) screen px: the view follows, inside the box. */
export function panBy(v: View, dx: number, dy: number, b: Box): View {
  if (atFit(v)) return FIT;
  return clampView({ z: v.z, tx: v.tx - dx, ty: v.ty - dy }, b);
}

/** Where the frame's point p shows in its box, and back. */
export function toScreen(v: View, x: number, y: number): { x: number; y: number } {
  return { x: v.tx + v.z * x, y: v.ty + v.z * y };
}
export function toPage(v: View, x: number, y: number): { x: number; y: number } {
  return { x: (x - v.tx) / v.z, y: (y - v.ty) / v.z };
}

/**
 * How far a wheel can still pan each way (screen px), as the frame reads it to take a wheel or
 * leave it to the page's own scroll: l for a wheel left (deltaX < 0), r right, u up, d down. Null
 * at fit (every wheel is the page's).
 */
export interface Room {
  readonly l: number;
  readonly r: number;
  readonly u: number;
  readonly d: number;
}
export function roomOf(v: View, b: Box): Room | null {
  if (atFit(v)) return null;
  return { l: 0 - v.tx, r: v.tx - b.w * (1 - v.z), u: 0 - v.ty, d: v.ty - b.h * (1 - v.z) };
}

/** The frame's style for a view: its transform, and a clip back to its own box (the corners' radius kept). */
export interface Look {
  readonly transform: string;
  readonly transformOrigin: string;
  readonly clipPath: string;
}
export function lookOf(v: View, b: Box, radius: number): Look | null {
  if (atFit(v)) return null;
  const px = (n: number): string => `${String(Math.round(n * 1000) / 1000)}px`;
  // the box in the frame's own (untransformed) px: what still shows of it
  const left = -v.tx / v.z;
  const top = -v.ty / v.z;
  const right = b.w - (b.w - v.tx) / v.z;
  const bottom = b.h - (b.h - v.ty) / v.z;
  return {
    transform: `translate(${px(v.tx)}, ${px(v.ty)}) scale(${String(v.z)})`,
    transformOrigin: '0 0',
    clipPath: `inset(${px(top)} ${px(right)} ${px(bottom)} ${px(left)} round ${px(radius / v.z)})`,
  };
}
