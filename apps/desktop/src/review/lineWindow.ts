// The lines of a diff's file worth drawing (code-viewer-width: a 29k-line .diff artifact drew a div
// per line and froze the window): the ones in the scroller's view, one screen above and below,
// widened to steps of ROW_STEP lines so a scroll redraws once every ROW_STEP lines, never per line.
// A line takes whole rows of one line-height: one, or more when it wraps (designer m_57777: a long
// line wraps at any character, its next rows at its own indent; ds thread.css .b-dl>.cd), counted
// here from its columns (lineRows); the lines out of the window are a gap of their rows (ds Diff's
// `gap`). Pure. Owner: nm-pages-b.
import { codeCols, indentCols } from '@/ds';

/** the window grows and moves by this many lines */
export const ROW_STEP = 64;
/** no layout to measure (a test's DOM, a hidden panel): the first lines, as many */
export const NO_LAYOUT_ROWS = 400;

/** lines [from, to) of a file */
export interface Span {
  readonly from: number;
  readonly to: number;
}

/**
 * The rows a code line takes in a cell `cols` columns wide: its first row holds `cols` columns,
 * each next one `cols` less its indent (at most half the cell, as thread.css caps it). A hunk's
 * head stays one row (cut with …); `cols` 0 (not measured): one row. Pure.
 */
export function lineRows(text: string, cols: number, head = false): number {
  if (head || cols <= 0) return 1;
  const n = codeCols(text);
  if (n <= cols) return 1;
  const next = cols - Math.min(indentCols(text), Math.floor(cols / 2));
  return 1 + Math.ceil((n - cols) / next);
}

/** The row each line starts at, and the file's rows at the end (its length: lines + 1). Pure. */
export function rowStarts(rows: readonly number[]): number[] {
  const out = [0];
  for (const r of rows) out.push((out.at(-1) ?? 0) + r);
  return out;
}

export interface Where {
  /** px from the top of the file's lines to the top of the scroller's view (negative: below it) */
  readonly top: number;
  /** the view's height, px; 0: no layout */
  readonly view: number;
  /** one row's height, px */
  readonly rowH: number;
  /** rowStarts of the file's lines */
  readonly starts: readonly number[];
}

/** The lines to draw of a file seen from `where`. Pure. */
export function lineWindow({ top, view, rowH, starts }: Where): Span {
  const total = starts.length - 1;
  if (!(view > 0) || !(rowH > 0)) return { from: 0, to: Math.min(total, NO_LAYOUT_ROWS) };
  const first = lineAt(starts, Math.floor((top - view) / rowH));
  const last = lineAt(starts, Math.ceil((top + 2 * view) / rowH)) + 1;
  const from = clamp(Math.floor(first / ROW_STEP) * ROW_STEP, total);
  const to = clamp(Math.ceil(last / ROW_STEP) * ROW_STEP, total);
  return to > from && (starts[total] ?? 0) * rowH > top - view && top + 2 * view > 0 ? { from, to } : { from: 0, to: 0 };
}

/** The line that row `row` falls in (the last line with starts ≤ row; -1 above the first). Pure. */
function lineAt(starts: readonly number[], row: number): number {
  let lo = -1;
  let hi = starts.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if ((starts[mid] ?? 0) <= row) lo = mid;
    else hi = mid;
  }
  return lo;
}

function clamp(n: number, total: number): number {
  return Math.max(0, Math.min(total, n));
}
