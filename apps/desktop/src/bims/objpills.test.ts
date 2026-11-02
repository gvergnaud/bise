// The objects' pills (objpills.ts): only the '?' and '✗' ones, on each object's drawn top, none off view.
import { describe, expect, it } from 'vitest';
import { layoutOf } from './maps';
import { objectSpots } from './corners';
import { crewOf, objsOf } from './corners.test';
import { settings } from '@/proto/words.gen';
import { OBJ_PILL_PX, objPillsOf, objTipOf, placeObjTip, type ObjTip } from './objpills';
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
  const at = { x: 40.4, y: 12.6, foot: 60.2 };

  it('names the hovered object’s plugin, as he reads it, from its drawn top and foot', () => {
    if (first === undefined) throw new Error('no object placed');
    const asked: string[] = [];
    const tip = objTipOf(placed, first.key, (key) => {
      asked.push(key);
      return at;
    });
    expect(tip).toEqual({ key: first.key, words: 'Gmail', pill: false, x: 40, y: 13, foot: 60 });
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

describe("where an object's tip goes (placeObjTip, designer m_51998)", () => {
  const tip = (x: number, y: number, foot: number, pill = false): ObjTip => ({ key: 'k', words: 'Gmail', pill, x, y, foot });
  const size = { w: 60, h: 24 };
  const tile = { w: 400, h: 300 };

  it('a tip’s gap over the object’s top, centred on it; over its pill when it wears one', () => {
    expect(placeObjTip(tip(200, 150, 190), size, tile, [])).toEqual({ left: 170, top: 150 - TIP_GAP - 24, side: 'above' });
    expect(placeObjTip(tip(200, 150, 190, true), size, tile, [])).toEqual({ left: 170, top: 150 - TIP_GAP - OBJ_PILL_PX - 24, side: 'above' });
  });

  it('under the object’s foot when a bim’s head sits in its place, never over the bim', () => {
    expect(placeObjTip(tip(200, 150, 190), size, tile, [{ x: 205, y: 125 }])).toEqual({ left: 170, top: 190 + TIP_GAP, side: 'below' });
    // a head elsewhere changes nothing
    expect(placeObjTip(tip(200, 150, 190), size, tile, [{ x: 320, y: 125 }]).side).toBe('above');
  });

  it('stays a gap inside the tile: under the foot when there is no room over it, clamped at the sides', () => {
    expect(placeObjTip(tip(200, 20, 70), size, tile, [])).toEqual({ left: 170, top: 70 + TIP_GAP, side: 'below' });
    expect(placeObjTip(tip(10, 150, 190), size, tile, []).left).toBe(TIP_GAP);
    expect(placeObjTip(tip(395, 150, 190), size, tile, []).left).toBe(400 - TIP_GAP - 60);
    // a foot at the tile's bottom: the tip stays inside
    expect(placeObjTip(tip(200, 20, 295), size, tile, []).top).toBe(300 - TIP_GAP - 24);
  });
});
