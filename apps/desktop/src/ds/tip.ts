// src/ds · the hover tip's rules (designer m_24359, his 'every compact mark explains itself'): where
// it goes and when it shows. Pure: TipLayer.tsx reads the pointer and the DOM, these decide.
//
// Where: 8 px from its mark, above it, below when there is no room above, clamped to the window
// (8 px from its edges); centred on the mark. It never covers the mark's row: above or below, never
// over. A mark may ask for below first (tipAttrs' side, designer m_45938: the agents float's ask
// options, whose tip above covered the question they answer); above when below has no room. A small
// mark inside a row that is a mark itself may ask to clear the row (clearBox: the sidebar's day
// bars, designer m_45992): its tip goes off the row, as the row's would, centred on the small mark.
// A row in a card at the window's right may ask for its left (side 'left', designer m_46992: the
// outer card's agents): beside the card it is in (besideBox), centred on the row, never over the
// row above; above when there is no room at its left.
// When: 400 ms after the pointer rests on a mark; at once on keyboard focus; warm: for 1 s after a
// tip closes (or while one shows), the next one shows at once, so moving along a row of marks
// never blinks.

/** a box on the screen, in CSS px */
export interface Box {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

/** the gap between the tip and its mark, and between the tip and the window's edges */
export const TIP_GAP = 8;
/** how long the pointer rests on a mark before its tip shows */
export const TIP_DELAY_MS = 400;
/** after a tip closes, how long the next one shows at once */
export const TIP_WARM_MS = 1000;

/** a side of its mark the tip goes on; 'left': beside it, centred on its height (a row in a card at the window's right edge, its tip off the card toward the page: designer m_46992) */
export type TipSide = 'above' | 'below' | 'left';

/** Where the tip goes: its left and top, and the side of the mark it is on. `first`: the side it tries first ('above' unless the mark asks). Pure. */
export function placeTip(mark: Box, tip: { width: number; height: number }, view: { width: number; height: number }, first: TipSide = 'above', align: 'centre' | 'start' = 'centre'): { left: number; top: number; side: TipSide } {
  if (first === 'left') {
    // beside the mark, centred on its height, clamped to the window; no room at its left: above or below
    const besideLeft = mark.left - TIP_GAP - tip.width;
    if (besideLeft >= TIP_GAP) {
      const mid = mark.top + mark.height / 2 - tip.height / 2;
      const top = Math.max(TIP_GAP, Math.min(mid, view.height - TIP_GAP - tip.height));
      return { left: Math.round(besideLeft), top: Math.round(top), side: 'left' };
    }
    return placeTip(mark, tip, view, 'above');
  }
  const above = mark.top - TIP_GAP - tip.height;
  const below = mark.top + mark.height + TIP_GAP;
  const fits = { above: above >= TIP_GAP, below: below + tip.height <= view.height - TIP_GAP };
  const other = first === 'above' ? 'below' : 'above';
  // its first side when it fits; else the other when that fits; else the side with more room
  const roomAbove = mark.top;
  const roomBelow = view.height - mark.top - mark.height;
  const side: TipSide = fits[first] ? first : fits[other] ? other : roomAbove > roomBelow ? 'above' : 'below';
  const top = side === 'above' ? Math.max(TIP_GAP, above) : below;
  // 'start': its left edge on the mark's (a mark at a column's left edge, settings · Setup's ring:
  // the tip never leaves the content column, designer m_54574); else centred on it
  const centred = align === 'start' ? mark.left : mark.left + mark.width / 2 - tip.width / 2;
  const left = Math.max(TIP_GAP, Math.min(centred, view.width - TIP_GAP - tip.width));
  return { left: Math.round(left), top: Math.round(top), side };
}

/**
 * A mark's tip: spread on the element (`<span {...tipAttrs("17% of perf's context used")}>`), a key
 * at its right when it has one, 'below' when it goes below first, 'left' beside it (placeTip; it
 * clears the closest data-tip-box around it, besideBox); '' or none: no tip. The
 * app's TipLayer reads them. Pure.
 */
export function tipAttrs(
  words: string | undefined,
  keys?: string,
  side?: 'below' | 'left',
  clear?: 'outer',
  align?: 'start',
): { 'data-tip'?: string; 'data-tip-keys'?: string; 'data-tip-side'?: 'below' | 'left'; 'data-tip-clear'?: 'outer'; 'data-tip-align'?: 'start' } {
  if (words === undefined || words === '') return {};
  return {
    'data-tip': words,
    ...(keys === undefined || keys === '' ? {} : { 'data-tip-keys': keys }),
    ...(side === undefined ? {} : { 'data-tip-side': side }),
    ...(clear === undefined ? {} : { 'data-tip-clear': clear }),
    ...(align === undefined ? {} : { 'data-tip-align': align }),
  };
}

/**
 * The box a tip clears: its mark's own, or for a mark that asks (tipAttrs' clear 'outer', a small
 * mark inside a row that is a mark itself: the row's day bars, designer m_45992) its outer mark's
 * top and height, so the tip goes where the row's own does and never covers the row, still centred
 * on the small mark. No outer mark: its own. Pure.
 */
export function clearBox(mark: Box, outer: Box | null): Box {
  return outer === null ? mark : { left: mark.left, width: mark.width, top: outer.top, height: outer.height };
}

/**
 * The box a 'left' tip clears: its mark's top and height, its outer box's left and width (the card
 * the row is in, data-tip-box), so the tip goes off the card, centred on the row. No outer box: the
 * mark's own. Pure.
 */
export function besideBox(mark: Box, outer: Box | null): Box {
  return outer === null ? mark : { left: outer.left, width: outer.width, top: mark.top, height: mark.height };
}

/** How long before a mark's tip shows: at once while warm or on keyboard focus, else the delay. Pure. */
export function tipWait(now: number, warmUntil: number, focus: boolean): number {
  return focus || now < warmUntil ? 0 : TIP_DELAY_MS;
}
