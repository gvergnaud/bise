// Where the capsule goes (pure; Electron's screen coordinates: y grows down).
// Ported from the Swift app's Placement (desktop v9, S8):
//   top     what he opens by hand (fn space, the spotlight): the glass's top
//           170 px under the display's top, centred; it grows downward
//   bottom  what bise brings (a card, the talk, an answer): the glass's bottom
//           96 px above the work area's bottom (the Dock), centred; it grows upward
//   menu    the tray's dropdown: the glass's top 6 px under the tray icon,
//           centred on it
//   corner  the fn talk (his pick C, designer m_22119): the glass's bottom-right
//           16 px from the right edge and 16 px above the Dock (a Dock on the
//           right: 16 px left of it; a hidden Dock: 16 px from the bottom); it
//           grows upward and keeps its right edge
// Never outside the work area (no menu bar, no Dock), checked once, at show.
// While shown it never re-centres or climbs (lead m_11455: the input must not
// move while he types): `pinned` notes where its glass sits at show, and each
// new size keeps that glass's left edge and its anchored edge (top for the
// spotlight and the menu, bottom for what bise brings) where they were; it
// grows right and away from that edge only. A bottom view that grows while
// shown (the voice view, lead m_11532: `fitBounds`) is placed so its glass at
// its max height would still end above the Dock, then pinned by its top-left
// and grows down.

export interface Rect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface DisplayBox {
  /** the whole display */
  readonly bounds: Rect;
  /** without the menu bar and the Dock */
  readonly workArea: Rect;
}

export type Anchor = { readonly at: 'top' } | { readonly at: 'bottom' } | { readonly at: 'corner' } | { readonly at: 'menu'; readonly icon: Rect };

export const TOP = 170;
export const BOTTOM = 96;
export const MENU_GAP = 6;
export const CORNER = 16;
/** a work area edge this close to the display's is no Dock (a hidden Dock leaves a few px): the display's edge counts */
const NO_DOCK = 8;

/** The corner's bottom-right: CORNER px inside the Dock's side of the work area, or of the display where no Dock is. */
function cornerOf(d: DisplayBox): { right: number; bottom: number } {
  const b = d.bounds;
  const wa = d.workArea;
  const waRight = wa.x + wa.width;
  const waBottom = wa.y + wa.height;
  const right = b.x + b.width - waRight < NO_DOCK ? b.x + b.width : waRight;
  const bottom = b.y + b.height - waBottom < NO_DOCK ? b.y + b.height : waBottom;
  return { right: right - CORNER, bottom: bottom - CORNER };
}

/** Which side of the display the Dock takes (the work area's widest gap past NO_DOCK), or 'hidden'. Pure. */
export function dockOf(d: DisplayBox): 'bottom' | 'right' | 'left' | 'hidden' {
  const b = d.bounds;
  const wa = d.workArea;
  const gaps = [
    ['bottom', b.y + b.height - (wa.y + wa.height)],
    ['right', b.x + b.width - (wa.x + wa.width)],
    ['left', wa.x - b.x],
  ] as const;
  const [side, gap] = gaps.reduce((a, g) => (g[1] > a[1] ? g : a));
  return gap < NO_DOCK ? 'hidden' : side;
}

/** Where a window goes to be on no display (test mode, BISE_DESKTOP_OFFSCREEN): far from every real one. */
export const OFF = -30000;

/** A test run's display, far from every real one (never shown anyway: test mode is offscreen). */
export const OFF_DISPLAY: DisplayBox = {
  bounds: { x: OFF, y: OFF, width: 1920, height: 1200 },
  workArea: { x: OFF, y: OFF, width: 1920, height: 1200 },
};

/**
 * The window's bounds for a content of `size`, whose glass (the visible part,
 * inside the window's transparent margin) is `glass` (default: all of it).
 */
export function capsuleBounds(size: { width: number; height: number }, d: DisplayBox, anchor: Anchor, glass?: Rect): Rect {
  const g = glass ?? { x: 0, y: 0, width: size.width, height: size.height };
  const wa = d.workArea;
  const w = Math.min(size.width, wa.width);
  const h = Math.min(size.height, wa.height);
  const gMidX = g.x + g.width / 2;
  let x = d.bounds.x + d.bounds.width / 2 - gMidX;
  let y: number;
  switch (anchor.at) {
    case 'top':
      y = d.bounds.y + TOP - g.y;
      break;
    case 'bottom':
      y = wa.y + wa.height - BOTTOM - (g.y + g.height);
      break;
    case 'menu':
      x = anchor.icon.x + anchor.icon.width / 2 - gMidX;
      y = Math.max(anchor.icon.y + anchor.icon.height, wa.y) + MENU_GAP - g.y;
      break;
    case 'corner': {
      const c = cornerOf(d);
      x = c.right - (g.x + g.width);
      y = c.bottom - (g.y + g.height);
      break;
    }
  }
  if (anchor.at === 'corner') {
    // the glass is placed exactly (it may sit in the strip a hidden Dock leaves);
    // only a transparent margin wider than CORNER goes past; a glass taller or wider than the screen keeps its top-left in
    x = Math.max(x, wa.x);
    y = Math.max(y, wa.y);
  } else {
    x = Math.min(Math.max(x, wa.x), wa.x + wa.width - w);
    y = Math.min(Math.max(y, wa.y), wa.y + wa.height - h);
  }
  return { x: Math.round(x), y: Math.round(y), width: Math.round(w), height: Math.round(h) };
}

/**
 * Where the glass was pinned at show: its anchored side's x (left, or right
 * for 'corner') and its anchored edge's y (top, or bottom for 'bottom' and 'corner').
 */
export interface Pin {
  readonly x: number;
  readonly y: number;
  readonly edge: 'top' | 'bottom';
  readonly side: 'left' | 'right';
}

/** The pin of a capsule placed at `b` (capsuleBounds) with this glass. */
export function pinOf(b: Rect, size: { width: number; height: number }, anchor: Anchor, glass?: Rect): Pin {
  const g = glass ?? { x: 0, y: 0, width: size.width, height: size.height };
  const bottom = b.y + g.y + g.height;
  if (anchor.at === 'corner') return { x: b.x + g.x + g.width, y: bottom, edge: 'bottom', side: 'right' };
  return anchor.at === 'bottom' ? { x: b.x + g.x, y: bottom, edge: 'bottom', side: 'left' } : { x: b.x + g.x, y: b.y + g.y, edge: 'top', side: 'left' };
}

/** The window's bounds for a new size while shown: the glass's pinned side and pinned edge stay put; no centring, no clamp. */
export function pinnedBounds(size: { width: number; height: number }, pin: Pin, glass?: Rect): Rect {
  const g = glass ?? { x: 0, y: 0, width: size.width, height: size.height };
  const x = pin.side === 'left' ? pin.x - g.x : pin.x - (g.x + g.width);
  const y = pin.edge === 'top' ? pin.y - g.y : pin.y - (g.y + g.height);
  return { x: Math.round(x), y: Math.round(y), width: Math.round(size.width), height: Math.round(size.height) };
}

/**
 * A window pinned by its left that would pass the work area's right edge (the spotlight's thread
 * pane opening, 640 -> 1120): shifted left just enough, never past the work area's left, never
 * re-centred (designer's condition 1, page spotlight-reponse). The pin moves with it, so it stays
 * shifted when it shrinks back. A right-pinned window (the corner) is left as it is. Pure.
 */
export function onScreen(b: Rect, pin: Pin, wa: Rect): { readonly bounds: Rect; readonly pin: Pin } {
  if (pin.side !== 'left') return { bounds: b, pin };
  const dx = Math.min(b.x + b.width - (wa.x + wa.width), b.x - wa.x);
  if (dx <= 0) return { bounds: b, pin };
  return { bounds: { ...b, x: b.x - dx }, pin: { ...pin, x: pin.x - dx } };
}

/**
 * At show, a 'bottom' view that grows down: its glass's top where `maxHeight`
 * of glass would end BOTTOM px above the work area's bottom (inside the work
 * area), centred; the window is its current size. Pin it with edge 'top'.
 */
export function fitBounds(size: { width: number; height: number }, d: DisplayBox, glass: Rect | undefined, maxHeight: number): Rect {
  const g = glass ?? { x: 0, y: 0, width: size.width, height: size.height };
  const gMax = { ...g, height: Math.max(g.height, maxHeight) };
  const tall = { width: size.width, height: size.height - g.height + gMax.height };
  const b = capsuleBounds(tall, d, { at: 'bottom' }, gMax);
  return { x: b.x, y: b.y, width: Math.round(size.width), height: Math.round(size.height) };
}
