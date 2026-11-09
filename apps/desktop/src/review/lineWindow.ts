// The lines of a diff's file worth drawing (code-viewer-width: a 29k-line .diff artifact drew a div
// per line and froze the window): the ones in the scroller's view, one screen above and below,
// widened to steps of ROW_STEP rows so a scroll redraws once every ROW_STEP rows, never per row.
// A line is one row of a fixed height (the diff's white-space: pre, one line-height); the rows out
// of the window are a gap of their height (ds Diff's `at` and `total`). Pure. Owner: nm-pages-b.

/** the window grows and moves by this many rows */
export const ROW_STEP = 64;
/** no layout to measure (a test's DOM, a hidden panel): the first rows, as many */
export const NO_LAYOUT_ROWS = 400;

/** rows [from, to) of a file's lines */
export interface Span {
  readonly from: number;
  readonly to: number;
}

export interface Where {
  /** px from the top of the file's lines to the top of the scroller's view (negative: below it) */
  readonly top: number;
  /** the view's height, px; 0: no layout */
  readonly view: number;
  /** one row's height, px */
  readonly rowH: number;
  /** the file's line count */
  readonly total: number;
}

/** The rows to draw of a file of `total` lines seen from `where`. Pure. */
export function lineWindow({ top, view, rowH, total }: Where): Span {
  if (!(view > 0) || !(rowH > 0)) return { from: 0, to: Math.min(total, NO_LAYOUT_ROWS) };
  const first = Math.floor((top - view) / rowH);
  const last = Math.ceil((top + 2 * view) / rowH);
  const from = clamp(Math.floor(first / ROW_STEP) * ROW_STEP, total);
  const to = clamp(Math.ceil(last / ROW_STEP) * ROW_STEP, total);
  return to > from ? { from, to } : { from: 0, to: 0 };
}

function clamp(n: number, total: number): number {
  return Math.max(0, Math.min(total, n));
}
