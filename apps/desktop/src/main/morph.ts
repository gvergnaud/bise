// The capsule's frame morph (capsule-morph, his ask m_66232, designer m_66237): when the
// spotlight's thread pane opens or closes, the window glides from where it is to its new frame,
// centred on its screen, with the page's own curve. The vibrancy fills the whole window
// (glass.ts), so the glass's size IS the native frame: CSS can't grow it. The page lays out its
// end state at once (the pane on the right, clipped by the window while it grows); main steps
// the frame with this curve. Open: 220 ms; close: 160 ms (after the pane's 80 ms fade out).
// A new size mid-move (an answer growing) retargets it; the time and the curve go on.
// Pure: the curve, the frame at a time; capsule.ts runs the timer and setBounds.
import type { CapsuleMorph } from '../shared/api';
import type { Rect } from './place';

export type MorphKind = CapsuleMorph;

/** How long the frame moves: open widens in --d-base, close narrows in 160 ms (designer m_66237). */
export const MORPH_MS: Readonly<Record<MorphKind, number>> = { open: 220, close: 160 };

/** ds/tokens.css's --ease-out, cubic-bezier(.2, 0, 0, 1): the page's own curve (morph.test reads the token). */
export const EASE_OUT = [0.2, 0, 0, 1] as const;

/** How often main moves the frame: under a 120 Hz frame, so a 60 Hz screen never skips a step. */
export const MORPH_STEP_MS = 8;

/** The y of a CSS cubic-bezier at x = `t` (0..1). Pure. */
export function cubicBezier(x1: number, y1: number, x2: number, y2: number): (t: number) => number {
  const cx = 3 * x1;
  const bx = 3 * (x2 - x1) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * y1;
  const by = 3 * (y2 - y1) - cy;
  const ay = 1 - cy - by;
  const xOf = (s: number): number => ((ax * s + bx) * s + cx) * s;
  const yOf = (s: number): number => ((ay * s + by) * s + cy) * s;
  return (t) => {
    if (t <= 0) return 0;
    if (t >= 1) return 1;
    // x(s) is monotone on [0, 1]: bisection, 30 halvings is far under a pixel
    let lo = 0;
    let hi = 1;
    for (let i = 0; i < 30; i++) {
      const mid = (lo + hi) / 2;
      if (xOf(mid) < t) lo = mid;
      else hi = mid;
    }
    return yOf((lo + hi) / 2);
  };
}

const easeOut = cubicBezier(...EASE_OUT);

/** A frame on its way: from `from` at `start` to `to` in `ms`. */
export interface Morph {
  readonly from: Rect;
  readonly to: Rect;
  readonly start: number;
  readonly ms: number;
}

export function startMorph(from: Rect, to: Rect, now: number, kind: MorphKind): Morph {
  return { from, to, start: now, ms: MORPH_MS[kind] };
}

/** A new end mid-move (a new size): the same start and curve, toward the new frame. */
export const retarget = (m: Morph, to: Rect): Morph => ({ ...m, to });

/** The frame at `now`, whole pixels, and whether it has arrived. Pure. */
export function frameAt(m: Morph, now: number): { readonly rect: Rect; readonly done: boolean } {
  const t = m.ms <= 0 ? 1 : (now - m.start) / m.ms;
  if (t >= 1) return { rect: m.to, done: true };
  const k = easeOut(Math.max(0, t));
  const at = (a: number, b: number): number => Math.round(a + (b - a) * k);
  return {
    rect: { x: at(m.from.x, m.to.x), y: at(m.from.y, m.to.y), width: at(m.from.width, m.to.width), height: at(m.from.height, m.to.height) },
    done: false,
  };
}
