// The inbox list's row as its room shrinks (designer m_53025: the panel open beside the inbox
// page): never overlapping, the state tabs never shrink or wrap. In order: the search goes from 200
// to 160; then 'All projects ⌄' and 'All agents ⌄' are one 'Filter ⌄' menu; then the search goes
// on its own row under the tabs, at the right edge. Pure.

/** 0 as drawn, 1 the search 160, 2 one 'Filter ⌄' menu too, 3 the search on its own row too. */
export type BarFit = 0 | 1 | 2 | 3;

export const SEARCH_W = 200;
export const SEARCH_NARROW_W = 160;

/** The widths the row holds (px): its room, the tabs, the two menus side by side (their gap in), the one menu, the gap between items. */
export interface BarWidths {
  readonly room: number;
  readonly tabs: number;
  readonly menus: number;
  readonly filter: number;
  readonly gap: number;
}

/** The first fit, in the designer's order, whose row holds in `room`. */
export function barFit(w: BarWidths): BarFit {
  const need = (menus: number, search: number) => w.tabs + w.gap + menus + w.gap + search;
  if (w.room >= need(w.menus, SEARCH_W)) return 0;
  if (w.room >= need(w.menus, SEARCH_NARROW_W)) return 1;
  if (w.room >= need(w.filter, SEARCH_NARROW_W)) return 2;
  return 3;
}
