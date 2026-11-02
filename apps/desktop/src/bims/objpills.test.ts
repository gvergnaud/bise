// The objects' pills (objpills.ts): only the '?' and '✗' ones, on each object's drawn top, none off view.
import { describe, expect, it } from 'vitest';
import { layoutOf } from './maps';
import { objectSpots } from './corners';
import { crewOf, objsOf } from './corners.test';
import { settings } from '@/proto/words.gen';
import { objPillsOf, objTipOf } from './objpills';

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

// hovering a plugin's object names it in a tip over its top (Gabriel m_51985)
describe("an object's name tip", () => {
  const o = layoutOf(crewOf(5), 'loft');
  const { placed } = objectSpots(o, objsOf(4).map((p, i) => (i === 0 ? { ...p, name: 'Gmail' } : p)));
  const first = placed[0];

  it('names the hovered object’s plugin, as he reads it, on its drawn top', () => {
    if (first === undefined) throw new Error('no object placed');
    const asked: string[] = [];
    const tip = objTipOf(placed, first.key, (key) => {
      asked.push(key);
      return { x: 40.4, y: 12.6 };
    });
    expect(tip).toEqual({ key: first.key, name: first.name, pill: false, x: 40, y: 13 });
    expect(asked).toEqual([first.key]);
    expect(placed.some((p) => p.name === 'Gmail')).toBe(true);
    // over its pill when it wears one
    expect(objTipOf([{ ...first, pill: '?' }], first.key, () => ({ x: 0, y: 0 }))?.pill).toBe(true);
  });

  it('none when nothing is hovered, the key is no drawn object, or it is off view', () => {
    if (first === undefined) throw new Error('no object placed');
    expect(objTipOf(placed, null, () => ({ x: 0, y: 0 }))).toBeNull();
    expect(objTipOf(placed, 'gone', () => ({ x: 0, y: 0 }))).toBeNull();
    expect(objTipOf(placed, first.key, () => null)).toBeNull();
  });
});
