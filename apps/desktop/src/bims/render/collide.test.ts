// @vitest-environment happy-dom
// The no-collision law (Gabriel: « il ne doit jamais y avoir de collisions ... rendre les collisions
// impossible dans le code »; « la cohérence de l'espace est plus importante que l'exhaustivité »):
// on every map, for every set of his plugins' objects (none, one, some, the whole collection,
// mixes) and office size, the office grown as they need it (extensions.ts) is built by world.ts and
// each placed object's box (its floor box, its height) goes through no mesh of the world: the
// furniture, the cozy pieces, each map's own decor (a lamppost, a tree, a column), the extensions'
// pieces; and through no other object. An object with no room may wait (left); two boxes never meet.
import { describe, expect, it } from 'vitest';
import { Box3, Mesh, Vector3 } from 'three';
import { layoutOf } from '../maps';
import { MAP_IDS, type MapId } from '../maps/ids';
import { objectSpots, type Placed } from '../corners';
import { crewOf, objsOf } from '../corners-fixtures';
import { grownFor } from '../extensions';
import { OBJ_IDS, type ObjId } from './objects/ids';
import type { Office } from '../model';
import { buildWorld } from './world';
import { lookOf } from './maps';
import { STYLE } from './style';

const turn = (): Promise<void> => new Promise((done) => { setTimeout(done, 0); });

/** the sets: none, one, Gabriel's ten, twenty, his 34, the whole collection, and two mixes (outdoor-heavy, games-heavy) */
const SETS: readonly (readonly ObjId[])[] = [
  [],
  ['pingpong'],
  OBJ_IDS.slice(0, 10),
  OBJ_IDS.slice(0, 20),
  OBJ_IDS,
  ['pingpong', 'mailbox', 'scooter', 'bike', 'telescope', 'campfire', 'grill', 'hammock', 'swing', 'pond', 'veggies', 'calendar'],
  ['arcade', 'pinball', 'pingpong', 'babyfoot', 'chess', 'piano', 'jukebox', 'cat', 'dog', 'cushion', 'easel'],
];
const SIZES = [1, 3, 12, 20, 31];

/** the world's solid meshes (anything standing above the floor): their boxes */
function solidsOf(o: Office): Box3[] {
  const w = buildWorld(o, 'main', STYLE, lookOf(o.map), false);
  w.root.updateMatrixWorld(true);
  const out: Box3[] = [];
  const seen = new Set<unknown>();
  w.root.traverse((m) => {
    if (!(m instanceof Mesh)) return;
    const parts = m.userData.parts as Box3[] | undefined;
    if (parts) {
      if (!seen.has(parts)) { seen.add(parts); out.push(...parts); }
      return;
    }
    out.push(new Box3().setFromObject(m, true));
  });
  w.dispose();
  // what is stood on or against is no solid: the floor, the slabs, a rug (flat, low)
  return out.filter((b) => b.max.y > 0.08 && !(b.max.y - b.min.y < 0.06 && b.max.y < 0.12));
}

/** an object's box: its floor box, from the floor to its height */
const boxOf = (p: Placed): Box3 => new Box3(new Vector3(p.box.x0, 0, p.box.z0), new Vector3(p.box.x1, p.foot.h, p.box.z1));

/** two boxes through each other, more than touching */
function through(a: Box3, b: Box3): boolean {
  const e = 0.02;
  return a.min.x < b.max.x - e && a.max.x > b.min.x + e && a.min.y < b.max.y - e && a.max.y > b.min.y + e && a.min.z < b.max.z - e && a.max.z > b.min.z + e;
}

const fmt = (b: Box3): string => `${b.min.x.toFixed(2)},${b.min.z.toFixed(2)}..${b.max.x.toFixed(2)},${b.max.z.toFixed(2)} h${b.min.y.toFixed(2)}..${b.max.y.toFixed(2)}`;

/** every collision of the objects placed on this map at this size with this set */
export async function collisionsOf(map: MapId, n: number, set: readonly ObjId[]): Promise<string[]> {
  const o0 = layoutOf(crewOf(n), map);
  const objs = objsOf(set.length, set);
  const square = objectSpots(o0, objs);
  const { office } = grownFor(o0, objs, square.placed);
  const c = office === o0 ? square : objectSpots(office, objs, square.placed);
  const world = solidsOf(office);
  await turn();
  const out: string[] = [];
  const at = (p: Placed): string => `${map} ${String(n)} agents ${String(set.length)} objects: ${p.obj}@${p.x.toFixed(2)},${p.z.toFixed(2)}`;
  c.placed.forEach((p, i) => {
    const b = boxOf(p);
    const hit = world.find((w) => through(b, w));
    if (hit) out.push(`${at(p)} through the world's ${fmt(hit)}`);
    for (const q of c.placed.slice(i + 1)) if (through(b, boxOf(q))) out.push(`${at(p)} through ${q.obj}`);
  });
  return out;
}

describe('no collision: every map, every set of objects, every size', () => {
  for (const map of MAP_IDS) {
    it(`${map}: no object through the world or another object`, async () => {
      const found: string[] = [];
      for (const n of SIZES) for (const set of SETS) found.push(...(await collisionsOf(map, n, set)));
      expect(found.slice(0, 20)).toEqual([]);
    }, 120_000);
  }
});
