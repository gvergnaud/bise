// sideLayout at the widths of plan §2 (1000, 1100, 1280, 1440, 1728, 1920), the panel open and closed.
import { describe, expect, it } from 'vitest';
import type { Route } from '../route';
import { BETWEEN_BAND, BUBBLE_BAND, CARD_MAX, CARD_ROOM, cardMaxH, COL_W, MIN_LEFT, OUTER, OUTER_BAND, OUTER_COL_MIN, OUTER_W, sideLayout, sideShows, THREAD_KEEPS } from './layout';

/** rail 56 + sidebar 240 */
const SIDE = 296;

/** what the right side leaves between the column and the window's edge */
const right = (winW: number, l: { colLeft: number; colW: number }) => winW - l.colLeft - l.colW;

describe('sideLayout, the panel folded', () => {
  it('1920, 1728 and 1440 (page outer-side v2): the outer card, its 304 band kept, the column centred in what it leaves', () => {
    for (const w of [1920, 1728, 1440]) {
      const l = sideLayout(w, SIDE, false);
      expect(l).toMatchObject({ colW: COL_W, gutter: OUTER_BAND, side: true, coins: 'card' });
      expect(right(w, l) - OUTER_BAND).toBe(l.colLeft - SIDE);
    }
  });
  it('the card while it leaves the column 480 and its two 24, narrowing to 260 (designer m_48013); a px less: the coins at the edge', () => {
    const just = SIDE + OUTER_COL_MIN + 2 * MIN_LEFT + OUTER_BAND;
    expect(sideLayout(just, SIDE, false)).toMatchObject({ coins: 'card', colW: OUTER_COL_MIN, colLeft: SIDE + MIN_LEFT, outerW: OUTER_W });
    expect(sideLayout(just - 1, SIDE, false)).toMatchObject({ coins: 'card', outerW: OUTER_W - 1, colW: OUTER_COL_MIN });
    const least = just - (OUTER_W - OUTER.min);
    expect(sideLayout(least, SIDE, false)).toMatchObject({ coins: 'card', outerW: OUTER.min, gutter: OUTER.min + 24 });
    expect(sideLayout(least - 1, SIDE, false).coins).toBe('edge');
    expect(sideLayout(1440, SIDE, true, 480).coins).toBe('between');
  });
  it("his width (outerwidth.ts): drawn in 260-480 and never past the column's 480 + 2×24; with the coins picked, no card", () => {
    expect(sideLayout(1920, SIDE, false, undefined, true, undefined, false, 400)).toMatchObject({ coins: 'card', outerW: 400, gutter: 424 });
    expect(sideLayout(2560, SIDE, false, undefined, true, undefined, false, 900).outerW).toBe(OUTER.max);
    expect(sideLayout(2560, SIDE, false, undefined, true, undefined, false, 10).outerW).toBe(OUTER.min);
    const l = sideLayout(1300, SIDE, false, undefined, true, undefined, false, 480);
    expect(l).toMatchObject({ outerW: 452, colW: OUTER_COL_MIN });
    expect(sideLayout(1920, SIDE, false, undefined, true, undefined, true, 400).coins).not.toBe('card');
  });
  it('1280: the card (292, his m_47018) still leaves a 668 column room: the column whole, centred in it', () => {
    const l = sideLayout(1280, SIDE, false);
    expect(l).toMatchObject({ colW: 620, coins: 'card', gutter: OUTER_BAND, colLeft: SIDE + MIN_LEFT });
  });
  it('1100: the world folds into a 5th bubble, then the column narrows, 24 from the sidebar', () => {
    const l = sideLayout(1100, SIDE, false);
    expect(l).toMatchObject({ colLeft: SIDE + MIN_LEFT, world: 'bubble', gutter: BUBBLE_BAND, side: true });
    expect(l.colW).toBe(1100 - SIDE - MIN_LEFT - BUBBLE_BAND);
    expect(right(1100, l)).toBe(BUBBLE_BAND);
  });
  it('1100 with the sidebar hidden (the rail alone): the card again', () => {
    const l = sideLayout(1100, 56, false);
    expect(l).toMatchObject({ colW: 680, coins: 'card' });
    expect(right(1100, l)).toBe(OUTER_BAND + MIN_LEFT);
  });
  it('1000: the column narrows, never under the bubbles', () => {
    const l = sideLayout(1000, SIDE, false);
    expect(l.world).toBe('bubble');
    expect(right(1000, l)).toBe(BUBBLE_BAND);
    expect(l.colLeft).toBe(SIDE + MIN_LEFT);
  });
  it('compact by hand (his m_46698): the coins at the edge however wide, the column centred beside their 180 band', () => {
    for (const w of [1920, 1728, 1440]) {
      const l = sideLayout(w, SIDE, false, undefined, true, undefined, true);
      expect(l).toMatchObject({ coins: 'edge', world: 'small', gutter: 180, side: true, colW: COL_W });
    }
    // narrow windows fall back the same with or without the pick
    for (const w of [1100, 1000]) expect(sideLayout(w, SIDE, false, undefined, true, undefined, true)).toEqual(sideLayout(w, SIDE, false));
    // the panel open: the pick changes nothing
    expect(sideLayout(1440, SIDE, true, 480, true, undefined, true)).toEqual(sideLayout(1440, SIDE, true, 480));
  });
  it('never a negative width', () => {
    expect(sideLayout(200, SIDE, false).colW).toBe(0);
  });
});

describe('sideLayout, the panel open: the width rule (folded-side v9)', () => {
  const beside = (threadMin: number) => ({ over: false, full: false, threadMin });
  it('the coins between the thread and the panel while the window holds the left, the thread\'s least, their 68 and the panel; the world a 5th coin; the panel\'s left edge their right', () => {
    // 1440, a terminal (480): 296 + 560 + 68 + 480 = 1404
    const l = sideLayout(1440, SIDE, true, 480, true, beside(THREAD_KEEPS.rows));
    expect(l).toMatchObject({ coins: 'between', side: true, world: 'bubble', gutter: BETWEEN_BAND, right: 480 });
    expect(1440 - 480 - l.colLeft - l.colW).toBeGreaterThanOrEqual(BETWEEN_BAND);
    // 1728, an artifact (950): 296 + 400 + 68 + 950 = 1714
    expect(sideLayout(1728, SIDE, true, 950, true, beside(THREAD_KEEPS.artifact))).toMatchObject({ coins: 'between', right: 950 });
  });
  it('the table: each width gives its row', () => {
    // [window, panel, beside an artifact, where the coins go]
    const rows: [number, number, boolean, string][] = [
      [1728, 480, false, 'between'],
      [1728, 950, true, 'between'],
      [1440, 480, false, 'between'],
      [1440, 744, true, 'header'],
      [1308, 452, false, 'header'],
      [1308, 612, true, 'header'],
    ];
    for (const [w, p, art, at] of rows) expect([w, p, art, sideLayout(w, SIDE, true, p, true, beside(art ? THREAD_KEEPS.artifact : THREAD_KEEPS.rows)).coins]).toEqual([w, p, art, at]);
  });
  it('the switch to the header is exactly where the band no longer fits: one px under, the header; the status line back at the limit', () => {
    for (const [p, min] of [[384, THREAD_KEEPS.rows], [480, THREAD_KEEPS.rows], [700, THREAD_KEEPS.artifact]] as const) {
      const limit = SIDE + min + BETWEEN_BAND + p;
      expect(sideLayout(limit, SIDE, true, p, true, beside(min)).coins).toBe('between');
      const under = sideLayout(limit - 1, SIDE, true, p, true, beside(min));
      expect(under).toMatchObject({ coins: 'header', side: false, gutter: 0 });
      // in the header, the column centred between the sidebar and the panel
      expect(under.colLeft - SIDE).toBeCloseTo(limit - 1 - p - under.colLeft - under.colW);
    }
  });
  it('a sheet or a full-width panel covers the thread: the coins go in its tab row', () => {
    expect(sideLayout(1100, SIDE, true, 384, true, { over: true, full: false, threadMin: 560 })).toMatchObject({ coins: 'tabs', side: false });
    expect(sideLayout(1920, SIDE, true, 1624, true, { over: false, full: true, threadMin: 560 })).toMatchObject({ coins: 'tabs', side: false });
  });
  it('the thread keeps its width under a sheet (ui-review m_47216): its column centred in the whole room, never in what the sheet leaves', () => {
    // as place.ts asks it: a sheet's box is 0 wide, so its width comes undefined; 680 with the
    // rail alone (the sidebar slides over) and with the sidebar docked
    for (const left of [56, SIDE]) {
      const room = 680 - left;
      const l = sideLayout(680, left, true, undefined, true, { over: true, full: false, threadMin: THREAD_KEEPS.rows });
      expect(l).toMatchObject({ coins: 'tabs', colW: Math.min(COL_W, room - 2 * MIN_LEFT), colLeft: left + MIN_LEFT });
    }
  });
  it('a page that shows no side: no coins anywhere, panel or not', () => {
    expect(sideLayout(1440, SIDE, false, undefined, false).coins).toBe('none');
    expect(sideLayout(1440, SIDE, true, 480, false).coins).toBe('none');
  });
  it('the panel folded: the outer card, or the coins at the window\'s edge when it leaves too little', () => {
    for (const w of [1440, 1920]) expect(sideLayout(w, SIDE, false)).toMatchObject({ coins: 'card', right: 0, side: true });
    expect(sideLayout(1100, SIDE, false)).toMatchObject({ coins: 'edge', right: 0, side: true });
  });
});

describe('a card: its top on its bubble, at most the room under it less 16 (designer m_44499)', () => {
  // the side starts under the 52 bar; the agents bubble at 152, the processes one at 392
  const BAR = 52;
  it('560 at most; else what is left under its bubble less 16; never below 0', () => {
    expect(cardMaxH(BAR, 152, 900)).toBe(CARD_MAX);
    expect(cardMaxH(BAR, 392, 900)).toBe(900 - CARD_ROOM - BAR - 392);
    expect(cardMaxH(BAR, 392, 400)).toBe(0);
  });
  it('a card never runs past the window: its bottom at the cap is 16 above the window\'s bottom at the lowest', () => {
    for (const winH of [500, 640, 900, 1200])
      for (const bubble of [152, 232, 312, 392]) expect(BAR + bubble + cardMaxH(BAR, bubble, winH)).toBeLessThanOrEqual(winH - CARD_ROOM);
  });
});

describe('the folded side shows inside a project only (designer m_44292)', () => {
  const P = 'acme-1a2b3c4d';
  // every screen of route.ts, once (a new screen breaks sideShows' switch at typecheck)
  const table: readonly (readonly [Route, boolean])[] = [
    [{ screen: 'bise' }, true],
    [{ screen: 'thread', project: P, agent: 'main' }, true],
    [{ screen: 'thread', project: P, agent: 'perf' }, true],
    [{ screen: 'agents', project: P }, true],
    [{ screen: 'agents', project: P, archived: true }, true],
    [{ screen: 'changes', project: P }, true],
    [{ screen: 'prs', project: P }, true],
    [{ screen: 'feature', project: P, name: 'native-mac' }, true],
    [{ screen: 'environments', project: P }, true],
    [{ screen: 'debug', project: P, agent: 'perf' }, true],
    [{ screen: 'artifacts', project: P }, false],
    [{ screen: 'artifacts', project: P, id: 'a1' }, true],
    [{ screen: 'artifacts', project: '', id: 'a1' }, false],
    [{ screen: 'artifacts', project: P, at: 'recent' }, false],
    [{ screen: 'scheduled' }, false],
    [{ screen: 'running' }, false],
    [{ screen: 'running', scope: { project: P } }, false],
    [{ screen: 'plugins' }, false],
    [{ screen: 'plugins', kind: 'skills' }, false],
    [{ screen: 'inbox' }, false],
    [{ screen: 'report' }, false],
    [{ screen: 'settings' }, false],
    [{ screen: 'onboarding' }, false],
    [{ screen: 'lab', lab: 'composer' }, false],
    [{ screen: 'notfound', path: 'x' }, false],
  ];
  it.each(table)('%j: %s', (r, shows) => {
    expect(sideShows(r)).toBe(shows);
  });
  it('no side: no band, the column centred in the whole room (the banners and columns take the freed width)', () => {
    for (const w of [1000, 1100, 1280, 1440, 1728, 1920]) {
      const l = sideLayout(w, SIDE, false, undefined, false);
      expect(l).toMatchObject({ side: false, gutter: 0 });
      expect(l.colLeft - SIDE).toBeCloseTo(right(w, l));
    }
  });
});
