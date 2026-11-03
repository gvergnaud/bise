// The selection pop's place (threadQuote.tsx popAt, popRoom): a word picked at the reading
// column's end keeps its pop in the column, never under the right side the window draws over the
// screen (the bims' bubbles, the outer card: Lorenzo's shot, m_52529). The window's own sideLayout
// and the column's place (column.ts), as the screen gets them.
import { describe, expect, it } from 'vitest';
import { sideLayout } from '@/app/side3d/layout';
import { columnPlace } from './column';
import { popAt, popRoom, POP_W, type Box } from './threadQuote';

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
        // under the word's line
        expect(pop.y).toBe(word.bottom + 6);
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

  it('none when the selection is out of the list\'s view', () => {
    const list = box(296, BAR, 800, 500);
    expect(popAt([box(400, 10, 30, 15)], list, popRoom(list, 296, null))).toBeNull();
    expect(popAt([box(400, 0, 0, 0)], list, popRoom(list, 296, null))).toBeNull();
  });
});
