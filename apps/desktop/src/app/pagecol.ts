// A project's page column (agents, archived, changes, PRs, environments, review, commit, feature,
// branch) where the window's one sideLayout puts the thread's (app/side3d/layout.ts): centred in
// what the right side's card leaves, slid left when the coins need their band, beside the panel's
// coins. Without it a page centred its column in its whole width (--pad-page-x) and ran under the
// outer card (his m_46849). usePageColumn measures the page's box and hands it two CSS variables:
// --page-col-l and --page-col-r, the column's margins in the page; the page's CSS reads them with
// --pad-page-x as their fallback (no side drawn, a page the layout doesn't describe), and the ds
// PageBanner puts its title on --page-col-l. A page with no reading column that fills its box
// (/debug) keeps the right side's band clear instead: usePageBand hands it --page-band, the width
// of its box right of the band's left edge (layout bandL). Owner: nm-pages-b (reads nm-side's layout).
import { useLayoutEffect, useState, type CSSProperties, type RefObject } from 'react';
import { useSideLayout } from './side3d/place';
import type { SideLayout } from './side3d/layout';

/** The column's margins in a page whose box runs from `pageLeft`, `pageW` wide (window px). */
export interface PageColumn {
  readonly l: number;
  readonly r: number;
}

/**
 * The layout's column (`colLeft`, `colW`, window px) as margins in the page's box; null when no
 * right side is drawn (the CSS's centred column is the same) or the column isn't in this box (a
 * page in a panel's tab, a box measured before it is laid out). Pure.
 */
export function pageColumn(lay: Pick<SideLayout, 'colLeft' | 'colW' | 'side'>, pageLeft: number, pageW: number): PageColumn | null {
  if (!lay.side || lay.colW <= 0) return null;
  const l = lay.colLeft - pageLeft;
  const r = pageW - l - lay.colW;
  if (l < 0 || r < -0.5) return null;
  return { l: Math.round(l), r: Math.max(0, Math.round(r)) };
}

/** The page's variables for a column (none: the CSS's centred column). Pure. */
export function pageColumnStyle(c: PageColumn | null): CSSProperties | undefined {
  if (c === null) return undefined;
  return { '--page-col-l': `${String(c.l)}px`, '--page-col-r': `${String(c.r)}px` } as CSSProperties;
}

/**
 * What a page that fills its box (no reading column: /debug) keeps free at its right for the right
 * side's band (its coins, its card): the part of its box right of `bandL`, 0 when no side is drawn
 * or the band is outside the box. Without it the coins drew over /debug's charts (his m_51994). Pure.
 */
export function pageBand(lay: Pick<SideLayout, 'side' | 'bandL'>, pageRight: number): number {
  if (!lay.side) return 0;
  return Math.max(0, Math.round(pageRight - lay.bandL));
}

/** The page's right band from the window's sideLayout (--page-band, px), measured again when the layout or the page's box changes. */
export function usePageBand(ref: RefObject<HTMLElement | null>): CSSProperties | undefined {
  const lay = useSideLayout();
  const [band, setBand] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el === null) return;
    const measure = () => {
      setBand(pageBand(lay, el.getBoundingClientRect().right));
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, [ref, lay]);
  return band === 0 ? undefined : ({ '--page-band': `${String(band)}px` } as CSSProperties);
}

/** The page's column from the window's sideLayout, measured again when the layout or the page's box changes. */
export function usePageColumn(ref: RefObject<HTMLElement | null>): CSSProperties | undefined {
  const lay = useSideLayout();
  const [col, setCol] = useState<PageColumn | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el === null) return;
    const measure = () => {
      const b = el.getBoundingClientRect();
      const next = pageColumn(lay, b.left, b.width);
      setCol((c) => (c?.l === next?.l && c?.r === next?.r ? c : next));
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, [ref, lay]);
  return pageColumnStyle(col);
}
