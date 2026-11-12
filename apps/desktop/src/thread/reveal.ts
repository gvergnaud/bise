// A reveal's frame (ThreadList.reveal: ⌘F's match, his answer far back, a tick of his): what the
// list does this frame to bring one row into view, from what the frame measured. Pure: the shell
// (ThreadList) reads the DOM, calls revealStep, applies its move.
//
// The row is followed by its KEY, read again each frame: never by the index it had when the reveal
// began (msg-rail-fix, his m_63243 (4): a tick's click landed "at a random place"). While the list
// moves toward a row near the view's first rows, the older page comes in front (asksOlder), a trim
// takes far rows out (trim.ts), a window's gap fills; each moves every row's index, and the old index
// then named another row, ~a page away, which the reveal brought to the top instead.
// A row reached is held there STILL_FRAMES frames before the reveal is done: the rows around it
// still being measured (an image decoded, a long answer laid out) move it after the first frame it
// sat right. Owner: msg-rail-fix.

/** Frames the row stays at its place (within 2 px) before the reveal is done. */
export const STILL_FRAMES = 8;

/** What a frame measured. Boxes are viewport px; `top` none: centred. */
export interface RevealFrame {
  /** the row revealed, by its key, and the list's keys and firstItemIndex NOW (this frame's) */
  readonly key: string;
  readonly keys: readonly string[];
  readonly first: number;
  /** the box of what is drawn at an item index (blockOf when `top`, else the row), null: not drawn */
  readonly box: (item: number) => { readonly top: number; readonly bottom: number } | null;
  readonly view: { readonly top: number; readonly bottom: number };
  /** the item indexes drawn now (Virtuoso's data-item-index) */
  readonly drawn: readonly number[];
  readonly scrollTop: number;
  /** the scrollTop of the last frame that found no row (a stalled scrollToIndex), none: first */
  readonly lastTop: number | undefined;
  /** frames in a row it sat at its place, before this one */
  readonly still: number;
  readonly top?: number;
}

/** This frame's move: Virtuoso's scrollToIndex to `at`, scrollTop by `px`, or done. `still`: the
 *  frames it sat right, for the next frame; `seek`: the row wasn't drawn, the scrollTop to compare
 *  the next frame's with (the next frame's `lastTop`). `gone`: the row left the list. */
export type RevealMove =
  | { readonly kind: 'index'; readonly at: number; readonly seek: number }
  | { readonly kind: 'by'; readonly px: number; readonly still: number; readonly seek?: number }
  | { readonly kind: 'done' }
  | { readonly kind: 'gone' };

/** What the list does this frame to bring the row into view (module header). Pure. */
export function revealStep(f: RevealFrame): RevealMove {
  const at = f.keys.indexOf(f.key);
  if (at < 0) return { kind: 'gone' };
  const item = f.first + at;
  const row = f.box(item);
  const vh = f.view.bottom - f.view.top;
  if (row === null) {
    // Virtuoso's scrollToIndex can stall (find-walk-red: a window replaced, then a new one in
    // front, its sizes by index still the old ones: scrollTop unmoved): then a viewport at a time
    // toward the row
    if (f.scrollTop === f.lastTop && f.drawn.length > 0) return { kind: 'by', px: (item < Math.min(...f.drawn) ? -0.8 : 0.8) * vh, still: 0, seek: f.scrollTop };
    return { kind: 'index', at, seek: f.scrollTop };
  }
  const h = row.bottom - row.top;
  const d = f.top === undefined ? row.top + h / 2 - (f.view.top + vh / 2) : row.top - (f.view.top + f.top);
  // at its top place, or whole in the view and centred: a row taller than the view only ever gets its top there
  const whole = row.top >= f.view.top && row.bottom <= f.view.bottom;
  if (Math.abs(d) < 2 && (f.top !== undefined || whole)) return f.still + 1 >= STILL_FRAMES ? { kind: 'done' } : { kind: 'by', px: 0, still: f.still + 1 };
  return { kind: 'by', px: f.top === undefined && h > vh ? row.top - f.view.top : d, still: 0 };
}
