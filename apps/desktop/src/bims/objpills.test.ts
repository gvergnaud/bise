// The objects' pills (objpills.ts): only the '?' and '✗' ones, on each object's drawn top, none off view.
import { describe, expect, it } from 'vitest';
import { layoutOf } from './maps';
import { objectSpots } from './corners';
import { crewOf, objsOf } from './corners.test';
import { settings } from '@/proto/words.gen';
import { BIM_HALF_OF_HEIGHT, BIM_HALF_PX, bimBox, deskBox, OBJ_PILL_PX, objPillsOf, objTipOf, placeObjTip, TIP_CLEAR, type ObjTip, type Rect } from './objpills';
import { TIP_GAP } from '@/ds';

describe("the objects' pills", () => {
  const o = layoutOf(crewOf(5), 'loft');
  const objs = objsOf(10).map((p, i) => ({ ...p, pill: i === 0 ? ('✗' as const) : i === 1 ? ('?' as const) : null }));
  const { placed } = objectSpots(o, objs);

  it('shows a pill on the top of each object that has one, and no other', () => {
    const asked: string[] = [];
    const pills = objPillsOf(placed, (key) => {
      asked.push(key);
      return { x: key.length * 10.4, y: 7.6 };
    });
    const want = placed.filter((p) => p.pill !== null);
    expect(want.length).toBeGreaterThan(0);
    expect(pills).toEqual(want.map((p) => ({ key: p.key, pill: p.pill, tip: p.pill === '✗' ? settings.plugins_waits_error({ name: p.key }) : settings.plugins_waits_login({ name: p.key }), x: Math.round(p.key.length * 10.4), y: 8 })));
    expect(asked).toEqual(want.map((p) => p.key));
  });

  it("an object's pill for a plugin not drawn is that plugin's: its key opens it, the object's own places it", () => {
    const p = placed[0];
    if (p === undefined) throw new Error('no object placed');
    const asked: string[] = [];
    const pills = objPillsOf([{ ...p, pill: '✗', pillOf: 'slides', pillName: 'Slides' }], (key) => {
      asked.push(key);
      return { x: 1, y: 2 };
    });
    // its tip names that plugin and its state (designer m_43241)
    expect(pills).toEqual([{ key: 'slides', pill: '✗', tip: 'Slides failed to start', x: 1, y: 2 }]);
    expect(objPillsOf([{ ...p, pill: '?', pillName: 'Gmail' }], () => ({ x: 0, y: 0 }))[0]?.tip).toBe('Gmail needs a login');
    expect(asked).toEqual([p.key]);
  });

  it('drops the pill of an object off view or not drawn', () => {
    expect(objPillsOf(placed, () => null)).toEqual([]);
  });
});

// hovering a plugin's object names its plugins in a tip over its top (Gabriel m_51985, designer m_51998)
describe("an object's name tip", () => {
  const o = layoutOf(crewOf(5), 'loft');
  const { placed } = objectSpots(o, objsOf(4).map((p, i) => (i === 0 ? { ...p, names: ['Gmail'] } : p)));
  const first = placed.find((p) => p.names[0] === 'Gmail');
  const at = { x: 40.4, y: 12.6, box: { left: 20.2, top: 12.6, right: 60.4, bottom: 60.2 } };

  it('names the hovered object’s plugin, as he reads it, from its drawn top and foot', () => {
    if (first === undefined) throw new Error('no object placed');
    const asked: string[] = [];
    const tip = objTipOf(placed, first.key, (key) => {
      asked.push(key);
      return at;
    });
    expect(tip).toEqual({ key: first.key, words: 'Gmail', pill: false, x: 40, y: 13, left: 20, right: 60, foot: 60 });
    expect(asked).toEqual([first.key]);
    // it knows the object wears a pill (the tip goes over it)
    expect(objTipOf([{ ...first, pill: '?' }], first.key, () => at)?.pill).toBe(true);
  });

  it('an object that stands for several plugins names them all, never just the first', () => {
    if (first === undefined) throw new Error('no object placed');
    expect(objTipOf([{ ...first, names: ['docs', 'sheets', 'slides'] }], first.key, () => at)?.words).toBe('docs, sheets and slides');
    expect(objTipOf([{ ...first, names: ['docs', 'slides'] }], first.key, () => at)?.words).toBe('docs and slides');
  });

  it('none when nothing is hovered, the key is no drawn object, or it is off view', () => {
    if (first === undefined) throw new Error('no object placed');
    expect(objTipOf(placed, null, () => at)).toBeNull();
    expect(objTipOf(placed, 'gone', () => at)).toBeNull();
    expect(objTipOf(placed, first.key, () => null)).toBeNull();
  });
});

describe("where an object's tip goes (placeObjTip, designer m_52074)", () => {
  // an object 40 wide, its top's middle at (x, y), its foot at `foot`
  const tip = (x: number, y: number, foot: number, pill = false): ObjTip => ({ key: 'k', words: 'Gmail', pill, x, y, left: x - 20, right: x + 20, foot });
  const size = { w: 60, h: 24 };
  const tile = { w: 400, h: 300 };
  const none = { bims: [], chips: [] };
  // a bim whose head's top is at (x, y), feet 40 under it
  const bim = (x: number, y: number): Rect => bimBox({ x, y, feet: y + 40 });

  it('a tip’s gap over the object’s top, centred on it; over its pill when it wears one', () => {
    expect(placeObjTip(tip(200, 150, 190), size, tile, none)).toEqual({ left: 170, top: 150 - TIP_GAP - 24, side: 'above' });
    expect(placeObjTip(tip(200, 150, 190, true), size, tile, none)).toEqual({ left: 170, top: 150 - TIP_GAP - OBJ_PILL_PX - 24, side: 'above' });
  });

  it('under the object’s foot when a bim’s head or body is in that place, never over the bim (ui-review m_52072 a)', () => {
    // main at his desk right behind the object: his head where the tip would go
    expect(placeObjTip(tip(200, 150, 190), size, tile, { bims: [bim(205, 110)], chips: [] })).toEqual({ left: 170, top: 190 + TIP_GAP, side: 'below' });
    // TIP_CLEAR from a bim counts: a head 3 px off the tip's box still sends it away
    const above = 150 - TIP_GAP - 24;
    expect(placeObjTip(tip(200, 150, 190), size, tile, { bims: [bimBox({ x: 170 + 60 + BIM_HALF_PX + TIP_CLEAR - 1, y: above, feet: above + 40 })], chips: [] })?.side).not.toBe('above');
    // a bim elsewhere changes nothing
    expect(placeObjTip(tip(200, 150, 190), size, tile, { bims: [bim(330, 110)], chips: [] })?.side).toBe('above');
  });

  it('off the tile’s chips: the tip never lands on ⤢ fit (ui-review m_52072 b)', () => {
    // pinched, the object near the bottom right; fit's chip over the tip's place above it
    const fit: Rect = { left: 330, top: 220, right: 390, bottom: 250 };
    // over it: on the chip; under it: past the tile's bottom; at its left (its box 340-380), centred on its height
    expect(placeObjTip(tip(360, 275, 290), size, tile, { bims: [], chips: [fit] })).toEqual({ left: 340 - TIP_GAP - 60, top: Math.round((275 + 290) / 2 - 12), side: 'left' });
    // without the chip: over it
    expect(placeObjTip(tip(360, 275, 290), size, tile, none)?.side).toBe('above');
  });

  it('beside it, left then right, when over and under have no room; none when nothing fits', () => {
    // a bim over it, the tile's bottom under it: at its left
    const low = tip(200, 250, 296);
    expect(placeObjTip(low, size, tile, { bims: [bim(200, 205)], chips: [] })?.side).toBe('left');
    // a bim at its left too: at its right
    expect(placeObjTip(low, size, tile, { bims: [bim(200, 205), bim(140, 245)], chips: [] })?.side).toBe('right');
    // bims all round: no tip
    expect(placeObjTip(low, size, tile, { bims: [bim(200, 205), bim(140, 245), bim(260, 245)], chips: [] })).toBeNull();
  });

  it('pinched, a bim is as wide as it is drawn and its desk counts: the tip never lands on main’s hand or laptop (ui-review m_52091)', () => {
    // zoomed in, main 160 px tall: his box reaches his hands, not 14 px round his head
    const main = bimBox({ x: 100, y: 120, feet: 280 });
    expect(main.right - main.left).toBe(2 * BIM_HALF_OF_HEIGHT * 160);
    // the printer behind his desk, its left part hidden: no room over it (a chip) or under it (the tile's bottom); its left spot 132-192 × 203-227
    const printer = tip(220, 150, 280);
    const top = { left: 0, top: 0, right: 400, bottom: 140 };
    // his drawn width reaches the left spot: at the printer's right
    expect(placeObjTip(printer, size, tile, { bims: [main], chips: [top] })?.side).toBe('right');
    // 14 px round his head (the old box) never reached it: the tip sat on his hand
    expect(placeObjTip(printer, size, tile, { bims: [bimBox({ x: 100, y: 120, feet: 140 })], chips: [top] })?.side).toBe('left');
    // his desk in the left spot sends it away too
    expect(placeObjTip(printer, size, tile, { bims: [deskBox({ x: 150, y: 200, r: 30 })], chips: [top] })?.side).toBe('right');
  });

  it('TIP_CLEAR inside the tile: slid along its edge, never past it', () => {
    expect(placeObjTip(tip(10, 150, 190), size, tile, none)?.left).toBe(TIP_CLEAR);
    expect(placeObjTip(tip(395, 150, 190), size, tile, none)?.left).toBe(400 - TIP_CLEAR - 60);
    // no room over it: under its foot
    expect(placeObjTip(tip(200, 20, 70), size, tile, none)).toEqual({ left: 170, top: 70 + TIP_GAP, side: 'below' });
  });
});
