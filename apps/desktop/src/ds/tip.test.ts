// src/ds · the hover tip's rules (tip.ts): where it goes, when it shows, its attributes.
import { describe, expect, it } from 'vitest';
import { besideBox, clearBox, placeTip, tipAttrs, tipWait, TIP_DELAY_MS } from './tip';

const VIEW = { width: 1400, height: 900 };

describe('the hover tip (designer m_24359)', () => {
  it('goes 8 px above its mark, centred on it', () => {
    expect(placeTip({ left: 100, top: 300, width: 20, height: 16 }, { width: 120, height: 24 }, VIEW)).toEqual({ left: 50, top: 268, side: 'above' });
  });

  it("'start': its left edge on its mark's, never left of a column's edge (designer m_54574)", () => {
    expect(placeTip({ left: 100, top: 300, width: 40, height: 16 }, { width: 160, height: 24 }, VIEW, 'below', 'start')).toEqual({ left: 100, top: 324, side: 'below' });
    expect(tipAttrs('5 of 7', undefined, 'below', undefined, 'start')).toEqual({ 'data-tip': '5 of 7', 'data-tip-side': 'below', 'data-tip-align': 'start' });
  });

  it('goes 8 px below when there is no room above', () => {
    expect(placeTip({ left: 100, top: 20, width: 20, height: 16 }, { width: 120, height: 24 }, VIEW)).toEqual({ left: 50, top: 44, side: 'below' });
  });

  it('stays 8 px inside the window on both sides', () => {
    expect(placeTip({ left: 2, top: 300, width: 10, height: 16 }, { width: 200, height: 24 }, VIEW).left).toBe(8);
    expect(placeTip({ left: 1390, top: 300, width: 10, height: 16 }, { width: 200, height: 24 }, VIEW).left).toBe(1192);
  });

  it('never covers its mark: no room either side, it takes the bigger one', () => {
    const at = placeTip({ left: 100, top: 30, width: 20, height: 16 }, { width: 120, height: 80 }, { width: 400, height: 120 });
    expect(at.side).toBe('below');
    expect(at.top).toBe(54);
  });

  it('a mark that asks for below gets it, above when below has no room (designer m_45938)', () => {
    expect(placeTip({ left: 100, top: 300, width: 20, height: 16 }, { width: 120, height: 24 }, VIEW, 'below')).toEqual({ left: 50, top: 324, side: 'below' });
    expect(placeTip({ left: 100, top: 870, width: 20, height: 16 }, { width: 120, height: 24 }, VIEW, 'below')).toEqual({ left: 50, top: 838, side: 'above' });
  });

  it("a mark that clears its outer one (designer m_45992): the outer's top and height, its own left and width; above, 8 off the row", () => {
    const bar = { left: 188, top: 211, width: 6, height: 12 };
    const row = { left: 64, top: 184, width: 224, height: 52 };
    expect(clearBox(bar, row)).toEqual({ left: 188, top: 184, width: 6, height: 52 });
    expect(clearBox(bar, null)).toBe(bar);
    const at = placeTip(clearBox(bar, row), { width: 179, height: 27 }, { width: 1440, height: 900 });
    expect(at).toEqual({ left: 102, top: 184 - 8 - 27, side: 'above' });
  });

  it("a 'left' tip (designer m_46992): off the card it is in, 8 left of it, centred on the row; no room at the left: above", () => {
    const row = { left: 1160, top: 300, width: 260, height: 28 };
    const card = { left: 1148, top: 64, width: 280, height: 824 };
    expect(besideBox(row, card)).toEqual({ left: 1148, top: 300, width: 280, height: 28 });
    expect(besideBox(row, null)).toBe(row);
    expect(placeTip(besideBox(row, card), { width: 200, height: 24 }, { width: 1440, height: 900 }, 'left')).toEqual({ left: 1148 - 8 - 200, top: 302, side: 'left' });
    // a row near the window's top: the tip stays 8 in
    expect(placeTip({ left: 1148, top: 0, width: 280, height: 8 }, { width: 200, height: 24 }, { width: 1440, height: 900 }, 'left').top).toBe(8);
    // a card at the window's left edge: above, as any tip
    expect(placeTip({ left: 100, top: 300, width: 20, height: 16 }, { width: 120, height: 24 }, { width: 1440, height: 900 }, 'left')).toEqual({ left: 50, top: 268, side: 'above' });
    expect(tipAttrs('perf · speed', undefined, 'left')).toEqual({ 'data-tip': 'perf · speed', 'data-tip-side': 'left' });
  });

  it('waits 400 ms cold, shows at once warm or on keyboard focus', () => {
    expect(tipWait(1000, 0, false)).toBe(TIP_DELAY_MS);
    expect(tipWait(1000, 1500, false)).toBe(0);
    expect(tipWait(1000, 0, true)).toBe(0);
  });

  it("its words and key go in data attributes; none for no words", () => {
    expect(tipAttrs('4 agents working')).toEqual({ 'data-tip': '4 agents working' });
    expect(tipAttrs('sidebar', '⌘\\')).toEqual({ 'data-tip': 'sidebar', 'data-tip-keys': '⌘\\' });
    expect(tipAttrs('the full option', undefined, 'below')).toEqual({ 'data-tip': 'the full option', 'data-tip-side': 'below' });
    expect(tipAttrs('today · 2 h of agent work', undefined, undefined, 'outer')).toEqual({ 'data-tip': 'today · 2 h of agent work', 'data-tip-clear': 'outer' });
    expect(tipAttrs('')).toEqual({});
    expect(tipAttrs(undefined)).toEqual({});
  });
});
