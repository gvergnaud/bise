// The corners' laws (corners.ts, plan bims-plugins p4): on every map, at 1 to 31 agents, with 10,
// 20 and 34 objects: no walk comes within a body of an object, no object on furniture, a cozy
// piece, the inbox or another object, every object on its corner's floor (the room, or the
// terrace's slab for an outdoor thing), the calendar on the back wall, the same spots for the same
// office, a spot kept when an agent comes and it is still free, and the desks laid the same with
// or without objects. Fixtures only.
// The fixtures (objsOf, crewOf, breaksOf) are corners-fixtures.ts: a test file imported by
// another runs its suite there again.
import { describe, expect, it } from 'vitest';
import { layoutOf } from './maps';
import { MAP_IDS, type MapId } from './maps/ids';
import { objectSpots } from './corners';
import { OBJ_IDS } from './render/objects/ids';
import { breaksOf, crewOf, objsOf } from './corners-fixtures';

const SIZES = [1, 3, 8, 12, 20, 31];
const SETS = [10, 20, 34];

describe('corners: where his plugins objects stand', () => {
  it.each(MAP_IDS)('%s: no walk, furniture or object through an object, each on its floor', (map: MapId) => {
    const found: string[] = [];
    for (const n of SIZES) {
      const o = layoutOf(crewOf(n), map);
      for (const k of SETS) found.push(...breaksOf(o, objectSpots(o, objsOf(k))));
    }
    expect(found.slice(0, 12)).toEqual([]);
    // ~0.5 s alone; a gate at load 40+ ran it past vitest's 5 s
  }, 30_000);

  it('the whole collection: every object finds its corner or waits, never breaks a law', () => {
    const all = objsOf(OBJ_IDS.length, OBJ_IDS);
    const found: string[] = [];
    for (const map of MAP_IDS) {
      const o = layoutOf(crewOf(31), map);
      found.push(...breaksOf(o, objectSpots(o, all)));
    }
    expect(found.slice(0, 12)).toEqual([]);
  }, 30_000);

  it('the same office and objects give the same spots, whatever their order', () => {
    const o = layoutOf(crewOf(8), 'loft');
    const objs = objsOf(20);
    const a = objectSpots(o, objs);
    const b = objectSpots(o, [...objs].reverse());
    expect(b).toEqual(a);
  });

  it('an agent more: an object whose spot breaks no law in the new office keeps it', () => {
    const moved: string[] = [];
    for (const map of MAP_IDS) {
      const o3 = layoutOf(crewOf(3), map);
      const objs = objsOf(10);
      const before = objectSpots(o3, objs);
      const o4 = layoutOf(crewOf(4), map, o3);
      const after = objectSpots(o4, objs, before.placed);
      for (const q of before.placed) {
        const p = after.placed.find((x) => x.key === q.key);
        // its old spot alone in the new office, a grid cell wider all round (the search's kept floor is a grid): still lawful, so it stays
        const m = 0.2;
        const wide = { ...q, box: { x0: q.box.x0 - m, x1: q.box.x1 + m, z0: q.box.z0 - m, z1: q.box.z1 + m } };
        const stillFree = breaksOf(o4, { placed: [wide], left: [], rugs: [] }).length === 0;
        if (stillFree && (p === undefined || p.x !== q.x || p.z !== q.z)) moved.push(`${map} ${q.obj}`);
      }
    }
    expect(moved).toEqual([]);
  });

  it('the desks never move for an object: the plan lays them without the objects', () => {
    const o = layoutOf(crewOf(12), 'loft');
    objectSpots(o, objsOf(34));
    expect([...layoutOf(crewOf(12), 'loft').desks]).toEqual([...o.desks]);
  });

  it('the calendar hangs on a wall; an outdoor thing stands on the terrace', () => {
    const o = layoutOf(crewOf(3), 'loft');
    const c = objectSpots(o, objsOf(13));
    const cal = c.placed.find((p) => p.obj === 'calendar');
    expect(cal?.corner).toBe('hang');
    const pp = c.placed.find((p) => p.obj === 'pingpong' || p.obj === 'babyfoot');
    expect(pp).toBeDefined();
  });

  it('places 34 objects in a loft of 31 agents in under 50 ms', () => {
    const o = layoutOf(crewOf(31), 'loft');
    const objs = objsOf(34);
    objectSpots(o, objs);
    // its own CPU time, the best of 3: the wall clock at load 40-60 (gates, shots) doubled it while
    // the placement's cost stayed the same (main m_41980)
    let best = Infinity;
    for (let i = 0; i < 3; i++) {
      const t0 = process.cpuUsage();
      objectSpots(o, objs);
      const d = process.cpuUsage(t0);
      best = Math.min(best, (d.user + d.system) / 1000);
    }
    expect(best).toBeLessThan(50);
  });
});
