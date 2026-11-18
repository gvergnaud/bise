// His sketch's growth (extensions.ts): an extension only when his objects need it, in its sketch
// slot (the wing on the right from the back wall, the garden in front of the terrace from the left),
// every object placed again on the grown floor by p4's laws, its walks clear of every piece.
import { describe, expect, it } from 'vitest';
import { layoutOf } from './maps';
import { MAP_IDS } from './maps/ids';
import { objectSpots } from './corners';
import { breaksOf, crewOf, objsOf } from './corners-fixtures';
import { GROWS, NO_NEED, either, gardenOf, grownFor, needOf, wingOf, withExtensions } from './extensions';
import { terraceSlab } from './render/decor';
import { network } from './paths';
import type { Box, Leg } from './model';

const overlaps = (a: Box, b: Box): boolean => a.x0 < b.x1 - 1e-9 && a.x1 > b.x0 + 1e-9 && a.z0 < b.z1 - 1e-9 && a.z1 > b.z0 + 1e-9;
/** a leg through a box (both ends' segment meets it) */
function legThrough(l: Leg, b: Box): boolean {
  const [a, c] = l;
  for (let i = 0; i <= 40; i++) {
    const x = a.x + ((c.x - a.x) * i) / 40;
    const z = a.z + ((c.z - a.z) * i) / 40;
    if (x > b.x0 + 1e-6 && x < b.x1 - 1e-6 && z > b.z0 + 1e-6 && z < b.z1 - 1e-6) return true;
  }
  return false;
}

describe("his sketch's growth", () => {
  it('needs the wing for an indoor thing left (as deep as they need), the garden for an outdoor or a travel one, nothing for none', () => {
    const o = layoutOf(crewOf(3), 'loft');
    const all = objsOf(34);
    const of = (id: string) => all.find((p) => p.obj === id) ?? (() => { throw new Error(id); })();
    const [lib, ping, mail] = [of('library'), of('pingpong'), of('mailbox')];
    expect(needOf(o, [])).toEqual(NO_NEED);
    expect(needOf(o, [lib]).garden).toBe(false);
    expect(needOf(o, [lib]).wing).toBeGreaterThan(0);
    expect(needOf(o, [ping])).toEqual({ wing: 0, garden: true });
    expect(needOf(o, [mail])).toEqual({ wing: 0, garden: true });
    // more things, a deeper wing, never past its sketch step
    const many = all.filter((p) => !['pingpong', 'mailbox', 'scooter', 'bike', 'telescope', 'campfire', 'calendar'].includes(p.obj));
    expect(needOf(o, many).wing).toBeGreaterThan(needOf(o, [lib]).wing);
    expect(needOf(o, many).wing).toBeLessThanOrEqual(wingOf(o).z1);
    expect(either({ wing: 3, garden: false }, { wing: 4, garden: true })).toEqual({ wing: 4, garden: true });
  });

  it('a few objects that fit: the square alone, the same office', () => {
    const o = layoutOf(crewOf(3), 'loft');
    const g = grownFor(o, objsOf(10), []);
    expect(g.need).toEqual(NO_NEED);
    expect(g.office).toBe(o);
  });

  it('each extension in its sketch slot: the wing past the right side from the back wall, the garden in front of the terrace from the left', () => {
    for (const n of [1, 12, 20, 31]) {
      const o = layoutOf(crewOf(n), 'loft');
      const w = wingOf(o);
      const g = gardenOf(o);
      expect(w.x0).toBe(o.x0 + o.W);
      expect(w.z0).toBe(0);
      expect(w.z1).toBeLessThanOrEqual(o.D);
      expect(g.x0).toBe(o.x0);
      expect(g.z0).toBe(terraceSlab(o).z1);
      expect(g.x1).toBeGreaterThan(o.door.x);
    }
  });

  it('a full collection grows each map, and every object stands by the laws on the grown floor', () => {
    for (const [m, n] of MAP_IDS.flatMap((id) => [1, 3, 12, 20, 31].map((k) => [id, k] as const))) {
      const o = layoutOf(crewOf(n), m);
      const objs = objsOf(34);
      const square = objectSpots(o, objs);
      const g = grownFor(o, objs, []);
      expect(g.need, `${m} ${String(n)} agents`).toEqual(needOf(o, square.left));
      const c = objectSpots(g.office, objs, square.placed);
      expect(breaksOf(g.office, c), `${m} ${String(n)} agents`).toEqual([]);
      // it takes in more than the square did
      expect(c.left.length, `${m} ${String(n)} agents`).toBeLessThanOrEqual(square.left.length);
    }
  }, 60_000);

  it("on every map, the wing's walk and the garden's path cross no furniture, and join the office's network", () => {
    for (const [m, n] of MAP_IDS.flatMap((id) => [1, 12, 20, 31].map((k) => [id, k] as const))) {
      const o = layoutOf(crewOf(n), m);
      const g = withExtensions(o, { wing: 5, garden: true });
      const added = g.walks.slice(o.walks.length);
      expect(added.length).toBeGreaterThan(0);
      for (const l of added) for (const f of o.furniture) expect(legThrough(l, f), `${m} ${String(n)} agents: ${JSON.stringify(l)} through ${JSON.stringify(f)}`).toBe(false);
      // every new leg's end is on the network: the wing's and the garden's far ends are reached from the door
      const net = network(g);
      const ends = added.flatMap(([a, b]) => [a, b]);
      for (const e of ends) expect(net.some(([a, b]) => (Math.hypot(a.x - e.x, a.z - e.z) < 1e-6 || Math.hypot(b.x - e.x, b.z - e.z) < 1e-6))).toBe(true);
    }
  });

  it('nothing drawn of the square moves: the grown office keeps its desks, furniture and walls', () => {
    const o = layoutOf(crewOf(12), 'loft');
    const g = withExtensions(o, { wing: 5, garden: true });
    expect([g.x0, g.W, g.D]).toEqual([o.x0, o.W, o.D]);
    expect(g.desks).toBe(o.desks);
    expect(g.furniture).toBe(o.furniture);
    expect(g.wing?.z1).toBe(Math.min(5, wingOf(o).z1));
    expect(overlaps(g.wing ?? { x0: 0, x1: 0, z0: 0, z1: 0 }, { x0: o.x0, x1: o.x0 + o.W, z0: 0, z1: o.D })).toBe(false);
  });

  it("grows every map; california its wing past the office's left side (never past the park)", () => {
    const c = layoutOf(crewOf(3), 'california');
    const w = wingOf(c);
    expect(w.x1).toBe(c.x0);
    expect(w.z0).toBe(0);
    expect([...GROWS].sort()).toEqual(['california', 'campus', 'climb', 'cyclades', 'greenhouse', 'haveli', 'kyoto', 'loft', 'nordic', 'patio', 'riad', 'sahel', 'tropical', 'workshop']);
  });

  it("a garden takes the door's trio first: mailbox, scooter and bike stand in it together, an outdoor piece waits before them (designer m_41494)", () => {
    const trio = ['mailbox', 'scooter', 'bike'];
    let gardens = 0;
    for (const map of GROWS) {
      const o = layoutOf(crewOf(3), map);
      // his collection and every outdoor piece: the garden can't take them all
      const outdoor = ['library', 'files', 'visio', 'booth', 'arcade', 'pingpong', 'scooter', 'mailbox', 'easel', 'macdesk', 'telescope', 'bike', 'campfire', 'grill', 'hammock', 'swing', 'pond', 'veggies'] as const;
      const objs = objsOf(outdoor.length, outdoor);
      const before = objectSpots(o, objs);
      const { office } = grownFor(o, objs, before.placed);
      if (office.garden === undefined) continue;
      gardens += 1;
      const after = objectSpots(office, objs, before.placed);
      const out = after.placed.filter((p) => trio.includes(p.of));
      expect(out.map((p) => `${map} ${p.of} ${p.corner}`).sort(), map).toEqual(trio.map((t) => `${map} ${t} garden`).sort());
    }
    expect(gardens).toBeGreaterThan(GROWS.size / 2);
  }, 30_000);
});
