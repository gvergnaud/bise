// src/views · what the window draws over a view's frame (review mcp-apps-code-review: a press on
// a menu or a popover over a view counted as that view's gesture, and its ui/message passed).
// The layer's step reads these surfaces' rects each frame and sends each frame's covers to Electron
// main with it (window.bise.views.frames); main's frameAt never counts a press inside a cover as
// the view's (main/viewrules.ts). Pure but for coverRects, which reads the DOM. Owner: mcp-apps-host.
import type { Rect } from './place';

/**
 * The window's surfaces drawn above the views layer that take a press: a menu or a dialog (escape.ts's
 * POPUP), a scrim, a sheet, the sidebar slid over, ⌘K, a popover, the floating composer and its
 * floats. Tips are not: they never take a press (pointer-events: none).
 */
export const OVER = [
  '[role="dialog"]',
  '[role="alertdialog"]',
  '[role="menu"]',
  '[aria-modal="true"]',
  '.app-menu',
  '.b-composer-pick',
  '.b-picker-pop',
  '.b-scrim',
  '.b-sheet',
  '.app-scrim',
  '.app-over',
  '.app-palette',
  '.app-ts-pop',
  '.help-over',
  '.cmp-float',
  '.q-float',
  '.art-give',
  '.art-fold-pop',
  '.art-selbar',
  '.art-toast',
].join(',');

/** Pure: the part of `c` over `r`, or null when they don't meet. */
function meet(r: Rect, c: Rect): Rect | null {
  const x = Math.max(r.x, c.x);
  const y = Math.max(r.y, c.y);
  const w = Math.min(r.x + r.w, c.x + c.w) - x;
  const h = Math.min(r.y + r.h, c.y + c.h) - y;
  return w > 0 && h > 0 ? { x, y, w, h } : null;
}

/** Pure: the covers of a frame at `r`: the parts of the window's surfaces `over` on it. */
export function coversOf(r: Rect, over: readonly Rect[]): Rect[] {
  const out: Rect[] = [];
  for (const c of over) {
    const m = meet(r, c);
    if (m !== null) out.push(m);
  }
  return out;
}

/** The rects of the surfaces open now (window CSS px), the ones with a size. */
export function coverRects(doc: Document): Rect[] {
  const out: Rect[] = [];
  for (const el of doc.querySelectorAll(OVER)) {
    const b = el.getBoundingClientRect();
    if (b.width > 0 && b.height > 0) out.push({ x: b.left, y: b.top, w: b.width, h: b.height });
  }
  return out;
}
