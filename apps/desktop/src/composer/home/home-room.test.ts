// Lorenzo's shot (m_52509): in a narrow window the empty home's third idea ('show me what you can
// do') ran under the outer card (the bims' world, Agents, Artifacts...) and was clipped. The slot
// spanned the whole screen and centred on it, while the column slides left of the card. The home's
// slot is the column's box (the composer's), so its words and ideas sit in it, on its axis, left of
// the right side's band, at every width. Computed from the stylesheets themselves with the
// column's vars from column.ts (css-box.testkit.ts: happy-dom lays nothing out).
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { sideLayout } from '@/app/side3d/layout';
import { columnPlace, COL_PAD } from '@/thread/column';
import { absSpan, length, rule } from '@/thread/css-box.testkit';

const read = (rel: string): string => readFileSync(new URL(rel, import.meta.url), 'utf8');
const threadCss = read('../../thread/thread.css');
const homeCss = read('./home.css');

/** rail 56 + sidebar 240: the screen starts where the left ends */
const LEFT = 296;

/** The screen's vars: .th-screen's own, the place useColumn sets over them when it has one. */
function screenVars(place: { x: number; w: number } | null): Record<string, string> {
  const s = rule(threadCss, '.th-screen');
  return {
    '--w-col': '720px',
    '--pad-column': `${String(COL_PAD)}px`,
    '--s-16': '80px',
    '--th-col': place === null ? (s.get('--th-col') ?? '') : `${String(place.w)}px`,
    '--th-col-x': place === null ? (s.get('--th-col-x') ?? '') : `${String(place.x)}px`,
  };
}

/** The home's slot (.th-first over .th-main, the screen's width) and the composer, screen px. */
function boxes(w: number, place: { x: number; w: number } | null) {
  const vars = screenVars(place);
  const slot = absSpan(rule(threadCss, '.th-first'), w, vars);
  const cmp = rule(threadCss, '.th-screen .b-composer');
  const cl = length(cmp.get('margin-left') ?? '', w, vars);
  return { slot, composer: { l: cl, r: cl + length(cmp.get('width') ?? '', w, vars) } };
}

describe("the empty home's room", () => {
  it.each([
    // the outer card shows from a 1108 window (outerFit): the column 480 beside it, Lorenzo's case
    ["1110, the outer card at its least column (Lorenzo's shot)", 1110],
    ['1200, the outer card', 1200],
    ['1280, the outer card', 1280],
    ['1440, the outer card', 1440],
    ['1920, the outer card', 1920],
  ] as const)('%s: the slot is the composer\'s box, left of the card\'s band', (_, win) => {
    const lay = sideLayout(win, LEFT, false);
    expect(lay.coins).toBe('card');
    const w = win - LEFT;
    const place = columnPlace(lay, LEFT, w);
    expect(place).not.toBeNull();
    const b = boxes(w, place);
    expect(b.slot.l).toBeCloseTo(b.composer.l, 3);
    expect(b.slot.r).toBeCloseTo(b.composer.r, 3);
    expect(LEFT + b.slot.r).toBeLessThanOrEqual(lay.bandL);
    expect(b.slot.l).toBeGreaterThanOrEqual(0);
  });

  it.each([
    ['1000, the coins at the edge', 1000],
    ['900, the world a bubble', 900],
  ] as const)("%s: the slot ends left of the coins' band", (_, win) => {
    const lay = sideLayout(win, LEFT, false, undefined, true, undefined, true);
    const w = win - LEFT;
    const place = columnPlace(lay, LEFT, w);
    const b = boxes(w, place);
    expect(LEFT + b.slot.r).toBeLessThanOrEqual(lay.bandL);
    expect(b.slot.l).toBeCloseTo(b.composer.l, 3);
  });

  it("a thread in a panel's tab (no layout place): the CSS's centred column", () => {
    const b = boxes(560, null);
    expect(b.slot).toEqual({ l: 0, r: 560 });
    expect((b.slot.l + b.slot.r) / 2).toBeCloseTo((b.composer.l + b.composer.r) / 2, 3);
  });

  it('its words sit 24 in from the slot, and the ideas wrap in it rather than run past it', () => {
    const slot = rule(threadCss, '.th-first');
    // the slot's width holds its padding, so the home's text is the column's text
    expect(slot.get('box-sizing')).toBe('border-box');
    expect(rule(homeCss, '.th-home').get('padding')).toBe('0 var(--pad-column) var(--s-16)');
    const home = rule(homeCss, '.th-home > .cmp-home');
    expect(home.get('max-width')).toBe('100%');
    expect(home.get('min-width')).toBe('0');
    expect(rule(homeCss, '.cmp-home-ideas').get('flex-wrap')).toBe('wrap');
  });
});
