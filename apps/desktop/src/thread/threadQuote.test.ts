// The selection pop's place (threadQuote.tsx popAt, popRoom): a word picked at the reading
// column's end keeps its pop in the column, never under the right side the window draws over the
// screen (the bims' bubbles, the outer card: Lorenzo's shot, m_52529). The window's own sideLayout
// and the column's place (column.ts), as the screen gets them.
import { describe, expect, it } from 'vitest';
import { sideLayout } from '@/app/side3d/layout';
import { columnPlace } from './column';
import { popAt, popRoom, POP_GAP, POP_H, POP_W, type Box } from './threadQuote';

const box = (left: number, top: number, width: number, height: number): Box => ({ left, top, width, height, right: left + width, bottom: top + height });

/** rail 56, with the sidebar (240) or without */
const LEFTS = [56, 296];
const WIDTHS = [900, 1000, 1100, 1280, 1440, 1920];
const H = 800;
const BAR = 52;

describe('the selection pop stays in the reading column', () => {
  for (const left of LEFTS)
    for (const w of WIDTHS) {
      it(`${String(w)} wide, the screen from ${String(left)}: a word at the column's end`, () => {
        const lay = sideLayout(w, left, false);
        const place = columnPlace(lay, left, w - left);
        expect(place).not.toBeNull();
        if (place === null) return;
        // the list: the screen's whole width, under the bar, above the composer
        const list = box(left, BAR, w - left, H - BAR - 200);
        // 'target', the last word of a line that ends at the text's right edge
        const textRight = left + place.x + place.w;
        const word = box(textRight - 34, 370, 34, 15);
        const pop = popAt([word], list, popRoom(list, left, place));
        expect(pop).not.toBeNull();
        if (pop === null) return;
        // inside the column (24 out of the text), so left of the band the right side keeps
        expect(pop.x + POP_W).toBeLessThanOrEqual(textRight + 24);
        expect(pop.x + POP_W).toBeLessThanOrEqual(lay.bandL);
        // above the word's line, 4 px clear: neither it nor the next row is covered
        expect(pop.y + POP_H).toBe(word.top - POP_GAP);
        // and a word at the text's left edge keeps its pop at its left
        const first = box(left + place.x, 370, 30, 15);
        expect(popAt([first], list, popRoom(list, left, place))?.x).toBe(first.left);
      });
    }

  it('a screen the layout does not describe (a panel tab, beside an artifact): the list\'s box', () => {
    const list = box(1056, BAR, 384, 500);
    const word = box(1400, 200, 30, 15);
    expect(popRoom(list, 1056, null)).toEqual({ left: 1056, right: 1440 });
    expect(popAt([word], list, popRoom(list, 1056, null))?.x).toBe(1440 - POP_W);
  });
});

describe('the selection pop above the selection, as in the TUI (quote.rs hint_rect)', () => {
  const list = box(296, BAR, 800, 500);
  const room = popRoom(list, 296, null);
  const covers = (y: number, line: Box) => y < line.bottom && y + POP_H > line.top;

  it('three lines picked: above the first, at its first word; no picked line covered', () => {
    const lines = [box(420, 300, 200, 15), box(344, 322, 600, 15), box(344, 344, 120, 15)];
    const pop = popAt(lines, list, room);
    expect(pop).toEqual({ x: 420, y: 300 - POP_GAP - POP_H });
    for (const l of lines) expect(covers(pop?.y ?? 0, l)).toBe(false);
  });

  it('the first line at the list\'s top: under the last line, 4 px clear', () => {
    const lines = [box(344, BAR + 10, 200, 15), box(360, BAR + 32, 100, 15)];
    expect(popAt(lines, list, room)).toEqual({ x: 360, y: BAR + 32 + 15 + POP_GAP });
  });

  it('the selection fills the list (no room above nor under): none, the key bar says it', () => {
    expect(popAt([box(344, BAR + 5, 600, 15), box(344, list.bottom - 20, 600, 15)], list, room)).toBeNull();
  });
});

describe('the selection pop in view', () => {
  it('none when the selection is out of the list\'s view', () => {
    const list = box(296, BAR, 800, 500);
    expect(popAt([box(400, 10, 30, 15)], list, popRoom(list, 296, null))).toBeNull();
    expect(popAt([box(400, 0, 0, 0)], list, popRoom(list, 296, null))).toBeNull();
  });
});
