// The room's plan (spots.ts): each object on the spot drawn for it, else one it fits, else the
// roomiest scaled down; the on ones first; the next free spots dotted; and the icons' objects.
import { describe, expect, it } from 'vitest';
import type { PluginObject } from '@/bims/plugin-objects';
import type { ObjId } from '@/bims/render/objects/ids';
import { iconKey, iconObject, iconScale, planRoom, seedOf, SPOTS, type Foot } from './spots';

const obj = (key: string, id: ObjId, glass = false): PluginObject => ({ key, names: [key], obj: id, glass, pill: null, picked: false });
/** every object 1 x 1 but the library (3 x 0.6), the calendar on the wall */
const feet = (id: ObjId): Foot => (id === 'library' ? { w: 3, d: 0.6, h: 2 } : id === 'calendar' ? { w: 0.8, d: 0.05, h: 0.8, wall: true } : { w: 1, d: 1, h: 1 });

describe('planRoom', () => {
  it('puts an object on the spot drawn for it, and the cat on the cushion', () => {
    const p = planRoom([obj('notion', 'library'), obj('bise', 'cat')], feet);
    expect(p.placed.map((x) => [x.o.key, x.spot.for, x.scale])).toEqual([['bise', 'cushion', 1], ['notion', 'library', 1]]);
  });

  it('gives an object with no spot of its own the first free one it fits, never a wall spot', () => {
    const p = planRoom([obj('gmail', 'mailbox')], feet);
    expect(p.placed[0]?.spot.for).toBe('cushion');
    expect(p.placed[0]?.scale).toBe(1);
  });

  it('a wall object goes on the wall', () => {
    const p = planRoom([obj('cal', 'calendar'), obj('cal2', 'calendar')], feet);
    expect(p.placed.map((x) => x.spot.wall)).toEqual([true]);
  });

  it('a big object no free spot holds goes on the roomiest one, scaled down to it', () => {
    const taken = SPOTS.filter((s) => s.for !== 'library' && s.wall !== true).map((s, i) => obj(`k${String(i).padStart(2, '0')}`, s.for));
    // the library's spot is taken by a library first; a second library has no spot it fits
    const p = planRoom([obj('a-lib', 'library'), obj('b-lib', 'library'), ...taken.slice(0, 3)], (id) => (id === 'library' ? { w: 3, d: 0.6, h: 2 } : { w: 1, d: 1, h: 1 }));
    const second = p.placed.find((x) => x.o.key === 'b-lib');
    expect(second).toBeDefined();
    expect(second?.scale).toBeLessThan(1);
  });

  it('fills the on ones first and keeps the room to its spots', () => {
    const many = Array.from({ length: 30 }, (_, i) => obj(`p${String(i).padStart(2, '0')}`, 'plant', i < 20));
    const p = planRoom(many, feet);
    expect(p.placed.length).toBe(SPOTS.length - 1);
    expect(p.placed.slice(0, 10).every((x) => !x.o.glass)).toBe(true);
    expect(p.dotted).toEqual([]);
  });

  it('dots the next three free floor spots', () => {
    const p = planRoom([obj('bise', 'cat')], feet);
    expect(p.dotted.length).toBe(3);
    expect(p.dotted.every((s) => s.wall !== true && s.for !== 'cushion')).toBe(true);
  });
});

describe('iconObject', () => {
  it('gives a connector he never added its own object, never glass', () => {
    const o = iconObject({ name: 'gmail', id: 'mistral:gmail', kind: 'mistral', state: 'available' as const }, new Map());
    expect(o).toMatchObject({ obj: 'mailbox', glass: false, pill: null });
  });

  it('draws a lit row (computer before its set up) out of glass', () => {
    const row = { name: 'computer', id: 'computer', kind: 'plugin', state: 'disabled' as const };
    expect(iconObject(row, new Map())?.glass).toBe(true);
    expect(iconObject(row, new Map(), true)?.glass).toBe(false);
  });

  it('gives a connector with no object of its own the plain tile, never the plant', () => {
    expect(iconObject({ name: 'hubspot', id: 'mistral:hubspot', kind: 'mistral', state: 'available' as const }, new Map())).toBeNull();
  });

  it('keeps a row that is on lit, a failed server too (only its ✗ says it)', () => {
    const parts = [{ kind: 'mcp', on: true, error: 'exited' }];
    expect(iconObject({ name: 'pg', id: 'pg', kind: 'mcp', state: 'loaded' as const, parts }, new Map())).toMatchObject({ glass: false, pill: '✗' });
  });

  it('gives each skills folder a look of its own', () => {
    const keys = ['skills:bise', 'skills:personal', 'skills:vibe'];
    expect(new Set(keys.map(seedOf)).size).toBe(3);
    expect(iconObject({ name: 'personal', id: 'skills:personal', kind: 'skills', state: 'loaded' as const }, new Map())?.seed).toBe(seedOf('skills:personal'));
  });

  it('gives a shadowed row none (the plain tile)', () => {
    expect(iconObject({ name: 'x', state: 'shadowed' as const }, new Map())).toBeNull();
  });

  it('keys an icon by its object, count, glass and size', () => {
    expect(iconKey({ obj: 'rack', n: 3, glass: true }, 40)).toBe('rack|3|g|0|40');
    expect(iconKey({ obj: 'shelf', glass: false, seed: 4 }, 56)).toBe('shelf|||4|56');
    expect(iconKey({ obj: 'shelf', glass: false }, 176, 3)).toBe('shelf|||0|176@3x');
  });

  it('draws an icon at 2x at least, the screen ratio rounded up past it, 3x at most', () => {
    expect([1, 1.5, 2, 2.5, 3, 4, 0, Number.NaN].map(iconScale)).toEqual([2, 2, 2, 3, 3, 3, 2, 2]);
  });
});
