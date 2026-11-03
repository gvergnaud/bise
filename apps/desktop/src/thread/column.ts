// The thread's reading column in the window (bise fait mac, page native-mac §6, QA §4): 720 at most
// (--w-col), 24 each side (--pad-column), so its text is 672, where the window's one sideLayout puts
// it (app/side3d/place.ts, nm-side): centred in what the right side leaves, slid left when the
// bims' world needs its band, the whole width with the panel folded. columnPlace turns that layout
// into the screen's own coordinates; a screen the layout doesn't describe (a thread in a panel's
// tab, beside an artifact) keeps the CSS's centred column. useColumn measures the screen and hands
// ThreadScreen the two CSS variables its rows read (thread.css --th-col, --th-col-x), and the place
// itself: what floats over the rows (the selection pop, threadQuote.tsx) stays in it, never under
// the right side. Owner: nm-thread.
import { useLayoutEffect, useMemo, useState, type CSSProperties, type RefObject } from 'react';
import { useSideLayout } from '@/app/side3d/place';

/** --pad-column: the column's room each side of its text */
export const COL_PAD = 24;

/** The text's place in the screen: its left edge from the screen's, and its width (px). */
export interface ColumnPlace {
  readonly x: number;
  readonly w: number;
}

/**
 * The column's text in a screen whose box runs from `screenLeft`, `screenW` wide (window px), for
 * the layout's column (`colLeft`, `colW`, window px); null when that column isn't in this screen
 * (a panel's tab, a screen beside an artifact): the CSS centres it there. Pure.
 */
export function columnPlace(col: { readonly colLeft: number; readonly colW: number }, screenLeft: number, screenW: number): ColumnPlace | null {
  const left = col.colLeft - screenLeft;
  if (col.colW <= 2 * COL_PAD || left < 0 || left + col.colW > screenW + 0.5) return null;
  return { x: Math.round(left + COL_PAD), w: Math.round(col.colW - 2 * COL_PAD) };
}

/** The column's variables for a place (none: the CSS's centred column). Pure. */
export function columnStyle(p: ColumnPlace | null): CSSProperties | undefined {
  if (p === null) return undefined;
  return { '--th-col': `${String(p.w)}px`, '--th-col-x': `${String(p.x)}px` } as CSSProperties;
}

/** The screen's column: its CSS variables and its place (null: the CSS's centred column). */
export interface Column {
  readonly style: CSSProperties | undefined;
  readonly place: ColumnPlace | null;
}

/** The screen's column, from the window's sideLayout; measured again when the layout or the screen's box changes. */
export function useColumn(ref: RefObject<HTMLElement | null>): Column {
  const lay = useSideLayout();
  const [place, setPlace] = useState<ColumnPlace | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el === null) return;
    const measure = () => {
      const b = el.getBoundingClientRect();
      const next = columnPlace(lay, b.left, b.width);
      setPlace((p) => (p?.x === next?.x && p?.w === next?.w ? p : next));
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, [ref, lay]);
  return useMemo(() => ({ style: columnStyle(place), place }), [place]);
}
