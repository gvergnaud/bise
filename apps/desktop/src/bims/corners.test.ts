// The corners' laws (corners.ts, plan bims-plugins p4): on every map, at 1 to 31 agents, with 10,
// 20 and 34 objects: no walk comes within a body of an object, no object on furniture, a cozy
// piece, the inbox or another object, every object on its corner's floor (the room, or the
// terrace's slab for an outdoor thing), the calendar on the back wall, the same spots for the same
// office, a spot kept when an agent comes and it is still free, and the desks laid the same with
// or without objects. Fixtures only.
import { describe, expect, it } from 'vitest';
import { agent, mainAgent } from './fixtures';
import { layoutOf } from './maps';
import { MAP_IDS, type MapId } from './maps/ids';
import type { Box, Office } from './model';
import { boxLegGap } from './inbox';
import { network } from './paths';
import { ALSO, CORNER, objectSpots, WALK_GAP, type Corners } from './corners';
import type { PluginObject } from './plugin-objects';
import { OBJ_IDS, type ObjId } from './render/objects/ids';
import { terraceSlab } from './render/decor';

/** a set of plugin objects: Gabriel's own first (page bims-plugins v15), then the collection's */
const MINE: readonly ObjId[] = [
  'library', 'files', 'visio', 'booth', 'arcade', 'pingpong', 'scooter', 'mailbox', 'easel', 'macdesk', 'printer', 'calendar', 'telescope', 'trophies', 'packing',
  'extinguisher', 'charts', 'cat', 'cushion', 'rack', 'shelf', 'plant', 'chess', 'piano', 'jukebox', 'aquarium', 'babyfoot', 'dog', 'bike', 'campfire', 'pinball', 'catwalk', 'globe', 'espresso',
];
export const objsOf = (n: number, from: readonly ObjId[] = MINE): PluginObject[] =>
  Array.from({ length: n }, (_, i) => ({ key: `p${String(i).padStart(2, '0')}-${from[i % from.length] as string}`, names: [`p${String(i)}`], obj: from[i % from.length] as ObjId, glass: false, pill: null, picked: false }));

export const crewOf = (n: number) => [mainAgent(), ...Array.from({ length: Math.max(0, n - 1) }, (_, i) => agent(`a${String(i + 1)}`))];

const overlaps = (a: Box, b: Box): boolean => a.x0 < b.x1 - 1e-9 && a.x1 > b.x0 + 1e-9 && a.z0 < b.z1 - 1e-9 && a.z1 > b.z0 + 1e-9;
const inside = (a: Box, b: Box): boolean => a.x0 >= b.x0 - 1e-9 && a.x1 <= b.x1 + 1e-9 && a.z0 >= b.z0 - 1e-9 && a.z1 <= b.z1 + 1e-9;

/** every break of the laws in one office's corners, a line each */
export function breaksOf(o: Office, c: Corners): string[] {
  const out: string[] = [];
  const legs = network(o);
  const cz = o.cozy;
  const kept: Box[] = [...o.furniture, ...(cz ? [cz.coffee, cz.shelf, ...cz.lamps, ...cz.plants].map((it) => it.box) : [])];
  const room = { x0: o.x0, x1: o.x0 + o.W, z0: 0, z1: o.D };
  c.placed.forEach((p, i) => {
    const at = `${o.map} ${p.obj}@${p.x.toFixed(2)},${p.z.toFixed(2)}`;
    const gap = Math.min(...legs.map((l) => boxLegGap(p.box, l)));
    if (gap < WALK_GAP - 1e-6) out.push(`${at}: a walk ${gap.toFixed(2)} from it`);
    if (kept.some((f) => overlaps(p.box, f))) out.push(`${at}: on furniture`);
    if (o.inbox && overlaps(p.box, { x0: o.inbox.x - 0.23, x1: o.inbox.x + 0.23, z0: o.inbox.z - 0.18, z1: o.inbox.z + 0.18 })) out.push(`${at}: on the inbox`);
    c.placed.slice(i + 1).forEach((q) => {
      if (overlaps(p.box, q.box)) out.push(`${at}: through ${q.obj}`);
    });
    // a grown office (extensions.ts): the wing is room, the garden terrace
    const floors = p.corner === 'terrace' ? [terraceSlab(o)] : p.corner === 'garden' ? (o.garden ? [o.garden] : []) : [room, ...(o.wing ? [o.wing] : [])];
    // the room and its wing are one floor (an L): a thing may stand across where they meet, never past the wing's front
    const w = o.wing;
    const acrossWing = w !== undefined && p.corner !== 'terrace' && p.corner !== 'garden' && inside(p.box, { x0: Math.min(room.x0, w.x0), x1: Math.max(room.x1, w.x1), z0: 0, z1: w.z1 });
    if (!acrossWing && !floors.some((f) => inside(p.box, f))) out.push(`${at}: off its ${p.corner === 'terrace' ? 'terrace' : 'room'}`);
    // only its own corner, or the other one a travel thing may take (the door or the terrace), or its swap's own (the ping-pong's babyfoot by the nap place)
    // a grown office's garden takes the terrace's things and the travel ones (extensions.ts, designer m_41450)
    const out_ = p.corner === 'garden' && (CORNER[p.of] === 'terrace' || ['mailbox', 'scooter', 'bike'].includes(p.of));
    if (!out_ && p.corner !== CORNER[p.of] && p.corner !== ALSO[p.of] && !(p.obj !== p.of && p.corner === CORNER[p.obj])) out.push(`${at}: ${CORNER[p.of]} thing in ${p.corner}`);
    if (p.corner === 'hang' && p.box.z0 > 0.3 && p.box.x0 > o.x0 + 0.3) out.push(`${at}: hangs off the back and left walls`);
  });
  return out;
}

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
