// @vitest-environment happy-dom
// Nothing goes through a desk, a bed or a bim (Gabriel, bims-clip: on the patio the agaves stood
// inside the desks, their leaves through the tops; in its nap place the nappers lay across each
// other and off their beds, the hammock posts and a lamp's pole through their bodies). For every
// ready map:
// - the plan, 0..31 agents: the nap beds never overlap each other nor any furniture, and each
//   napper (pose.ts napping, the bim as bim.ts builds it, each mesh's own box) lies inside its own
//   bed (maps/plan.ts napBedAt);
// - the world as world.ts builds it, the smallest and the biggest office, stages 0..14: under a
//   desk's top nothing but the desk; through a bed nothing but its own pieces (render/nap.ts bedGroup
//   tags them: riad's tea table stood in bed 5 and counted as its own, bims-nap); through a napper on
//   each bed the stage has, nothing but its own bed's low pieces (a cushion, a hammock's cloth: never
//   a post, a frame or a backrest 0.5 tall); through a bim at its desk nothing but its desk and its
//   chair. A merged cozy mesh counts by its pieces (render/cozy.ts meshBoxes); flat things on the
//   floor (a rug, a mat's cloth) never count;
// - and at the office's camera (camera.ts: 320 and 640 docked), every room size of 0..31 agents,
//   no string strung across the room
//   (a look tags it: userData.string; the patio's papel picado) drawn over a napper's head nearer
//   the camera than it (bims m_34171: the flags' line crossed three faces), as shade.test.ts does
//   for lamp shades over desk heads.
import { describe, expect, it } from 'vitest';
import { Box3, Mesh, type Object3D, Vector3 } from 'three';
import { crowd } from '../fixtures';
import { layoutOf, readyMaps } from '../maps';
import { bedsAt, LAST_STAGE, NO_GROW } from '../growth';
import type { Activity, Box, MapId, Office, Pose, Pt, Spot } from '../model';
import { bedKey, bedPieces, restAt, shapeAt, type NapRest } from './nap';
import { buildWorld } from './world';
import { meshBoxes } from './cozy';
import { lookOf } from './maps';
import { makeBim } from './bim';
import { rigOf } from './pose';
import { HEAD_Y } from './laptop';
import { headBoxOf } from './heads.testkit';
import { BACK, type Fit, fitSite, siteOf, toScreen, wideSiteOf } from './camera';
import { STYLE } from './style';
import { ARCADE_W } from './maps/blook';

/** what two boxes may share (float noise, a hat's edge on a cushion) */
const SLACK = 0.02;
/** a mesh this low on the floor is a rug or a mat's cloth: walked on, lain on, never in the way */
const FLAT = 0.09;
/** a bed's own piece a napper may sink into is lower than this (a cushion, a hammock's cloth); a post, a frame or a backrest is taller */
const SOFT_H = 0.5;
/** the offices the worlds are built for: the smallest and the biggest */
const WORLDS = [3, 31] as const;
const NAP: Activity = { k: 'nap', at: 'lounge' };
const WORK: Activity = { k: 'work' } as Activity;

/** one event-loop turn: vitest's worker reads its RPC replies only between macrotasks (the lying law's fix, 8f7d3842a) */
const turn = (): Promise<void> => new Promise((done) => { setTimeout(done, 0); });

function shown(m: Object3D): boolean {
  for (let p: Object3D | null = m; p; p = p.parent) if (!p.visible) return false;
  return true;
}

/**
 * each mesh's box of the bim `name` posed with `act` at `spot`, world units, shrunk by SLACK, headwear
 * and all (nothing may poke through a bed); `head`: its head's box alone as the view laws see it,
 * without its headwear (heads.testkit.ts headBoxOf)
 */
function bimBoxes(name: string, spot: Spot, act: Activity, head = false, bedAt?: (p: Pt) => NapRest | undefined): Box3[] {
  const bim = makeBim(name, false, STYLE, undefined, undefined, !head);
  const pose: Pose = {
    x: spot.x, z: spot.z, dir: spot.face, moving: false, since: 30,
    seg: { t: 0, act, sig: act.k, spot, path: { pts: [spot], seg: [], len: 0 }, speed: 2.4, from: null },
  };
  bim.apply(rigOf({ pose, t: 30, main: false, phase: 1.3, reduced: false, ...(bedAt ? { bedAt } : {}) }));
  bim.group.updateMatrixWorld(true);
  const out: Box3[] = [];
  let top: Object3D | undefined;
  bim.group.traverse((o) => { if (!top && o.children.length > 0 && Math.abs(o.position.y - HEAD_Y) < 1e-6) top = o; });
  if (head && top) out.push(headBoxOf(top, { wear: false }).expandByScalar(-SLACK));
  else bim.group.traverse((m) => { if (m instanceof Mesh && shown(m)) out.push(new Box3().setFromObject(m, true).expandByScalar(-SLACK)); });
  bim.dispose();
  return out;
}

/** a box cut in pieces 0.1 long along its longest side */
function pieces(b: Box3): Box3[] {
  const size = b.getSize(new Vector3());
  const axis = size.x >= size.y && size.x >= size.z ? 'x' : size.y >= size.z ? 'y' : 'z';
  const n = Math.max(1, Math.ceil(size[axis] / 0.1));
  return Array.from({ length: n }, (_, i) => {
    const p = b.clone();
    p.min[axis] = b.min[axis] + (size[axis] * i) / n;
    p.max[axis] = b.min[axis] + (size[axis] * (i + 1)) / n;
    return p;
  });
}

interface Rect { readonly x0: number; readonly x1: number; readonly y0: number; readonly y1: number }
/** a box's 8 corners on the screen: their bounds, css px (shade.test.ts) */
function onScreen(fit: Fit, b: Box3): Rect {
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (const x of [b.min.x, b.max.x]) for (const y of [b.min.y, b.max.y]) for (const z of [b.min.z, b.max.z]) {
    const p = toScreen(fit, { x, y, z });
    x0 = Math.min(x0, p.x); x1 = Math.max(x1, p.x); y0 = Math.min(y0, p.y); y1 = Math.max(y1, p.y);
  }
  return { x0, x1, y0, y1 };
}
const meets = (a: Rect, b: Rect): boolean => a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1;
/** how near the camera a box's middle is */
const depth = (b: Box3): number => ((b.min.x + b.max.x) / 2) * BACK.x + ((b.min.y + b.max.y) / 2) * BACK.y + ((b.min.z + b.max.z) / 2) * BACK.z;

const fmt = (b: Box3): string => `${b.min.toArray().map((v) => v.toFixed(2)).join(',')}..${b.max.toArray().map((v) => v.toFixed(2)).join(',')}`;
const fmtBox = (b: Box): string => `[${[b.x0, b.x1, b.z0, b.z1].map((v) => v.toFixed(2)).join(',')}]`;
const within = (b: Box3, f: Box, s: number): boolean => b.min.x >= f.x0 - s && b.max.x <= f.x1 + s && b.min.z >= f.z0 - s && b.max.z <= f.z1 + s;
const overlap = (a: Box, b: Box): boolean => a.x0 < b.x1 - SLACK && b.x0 < a.x1 - SLACK && a.z0 < b.z1 - SLACK && b.z0 < a.z1 - SLACK;
const volume = (f: Box, inset: number, y0: number, y1: number): Box3 => new Box3(new Vector3(f.x0 + inset, y0, f.z0 + inset), new Vector3(f.x1 - inset, y1, f.z1 - inset));
const bedOf = (o: Office, s: Spot): Box | undefined => o.furniture.find((f) => f.of?.x === s.x && f.of.z === s.z);
const isBed = (o: Office, f: Box): boolean => f.of !== undefined && o.lounge.some((l) => l.x === f.of?.x && l.z === f.of.z);
/** the floor box of a desk: ahead of its chair the way it faces */
function deskBox(o: Office, d: Spot): Box | undefined {
  const cx = d.x + Math.sin(d.face) * 0.5;
  const cz = d.z + Math.cos(d.face) * 0.5;
  return o.furniture.find((f) => (!f.of || f.of.x === d.x && f.of.z === d.z) && cx > f.x0 && cx < f.x1 && cz > f.z0 && cz < f.z1);
}
/** a point deep under a turned desk's top (the narrowest is 1.1 by 0.55, 0.5 ahead of its chair): what hangs on its sides stays out */
function underTurned(d: Spot, p: Vector3): boolean {
  const ax = Math.sin(d.face);
  const az = Math.cos(d.face);
  const along = (p.x - d.x) * ax + (p.z - d.z) * az - 0.5;
  const across = (p.x - d.x) * az - (p.z - d.z) * ax;
  return Math.abs(along) < 0.18 && Math.abs(across) < 0.45;
}
const napName = (i: number): string => `agent-${String(i + 1)}`;

/**
 * the plan's reds: beds that overlap, a napper off its bed. `bodies`: the nappers' boxes already
 * posed, by map, bed and spot: the same napper on the same spot is the same box, and the lounge
 * barely moves from one office size to the next (riad: 8 beds at every size), so posing each once
 * keeps 0..31 agents under 3 s alone (it took 61 s at the whole suite's load, its budget 60 s).
 */
function planReds(o: Office, bodies: Map<string, Box3>): string[] {
  const out: string[] = [];
  const beds = o.lounge.map((s) => ({ s, bed: bedOf(o, s) }));
  beds.forEach(({ s, bed }, i) => {
    if (!bed) { out.push(`bed ${String(i)}: no box`); return; }
    for (const f of o.furniture) {
      if (f === bed || f.of?.x === s.x && f.of.z === s.z) continue;
      if (overlap(bed, f)) out.push(`bed ${String(i)} ${fmtBox(bed)} over ${isBed(o, f) ? 'bed' : 'furniture'} ${fmtBox(f)}`);
    }
    // its napper as it lies on its bed's shape (render/nap.ts), on a cushion's height
    const key = `${o.map}|${String(i)}|${String(s.x)},${String(s.z)},${String(s.face)}`;
    let body = bodies.get(key);
    if (!body) {
      const rest: NapRest = { shape: shapeAt(lookOf(o.map), i), top: 0.2 };
      body = bimBoxes(napName(i), s, NAP, false, () => rest).reduce((a, b) => a.union(b), new Box3());
      bodies.set(key, body);
    }
    if (!within(body, bed, 0)) out.push(`napper ${String(i)} ${fmt(body)} off its bed ${fmtBox(bed)}`);
  });
  return out;
}

/** the world's reds at one stage */
function worldReds(o: Office, stage: number, seated: ReadonlyMap<string, Box3[]>): string[] {
  const w = buildWorld(o, 'main', STYLE, lookOf(o.map), false, { ...NO_GROW, stage });
  const meshes = meshBoxes(w.root).filter((b) => !b.isEmpty() && b.max.y >= FLAT);
  // each bed's own pieces, as tagged (render/nap.ts bedGroup): anything else in its box is in the way (bims m_35422: riad's tea table stood in bed 5)
  const pieces = bedPieces(w.root);
  const naps = w.naps;
  const bedAt = (p: Pt): NapRest | undefined => restAt(o, naps, p);
  w.dispose();
  const out: string[] = [];
  // under a desk's top: the desk alone (its body and top cover most of its floor box)
  const desks = [...o.desks.entries(), ['main', o.mainDesk] as const];
  for (const [name, d] of desks) {
    const f = deskBox(o, d);
    if (!f) { out.push(`${name}'s desk: no box`); continue; }
    const v = volume(f, 0.1, 0.1, 0.45);
    const area = (f.x1 - f.x0) * (f.z1 - f.z0);
    // a desk turned off the axes (the greenhouse's ring): its floor box is wider than the desk, so
    // what stands under its top is a mesh whose middle is inside the desk itself
    const turned = Math.abs(Math.sin(2 * d.face)) > 0.01;
    for (const m of meshes) {
      if ((m.max.x - m.min.x) * (m.max.z - m.min.z) > area * 0.5) continue;
      if (turned ? m.max.y > 0.1 && m.min.y < 0.45 && underTurned(d, m.getCenter(new Vector3())) : m.intersectsBox(v)) out.push(`${name}'s desk ${fmtBox(f)}: ${fmt(m)} under its top`);
    }
  }
  // a bed and its napper: their own pieces only
  o.lounge.slice(0, bedsAt(stage)).forEach((s, i) => {
    const bed = bedOf(o, s);
    if (!bed) return;
    // the bed's own height (a hammock's posts); what hangs on the wall over its head is the napper's to clear
    const v = volume(bed, 0.05, FLAT, 0.9);
    const body = bimBoxes(napName(i), s, NAP, false, bedAt);
    const mine = pieces.get(bedKey(s)) ?? [];
    for (const m of meshes) {
      const own = mine.some((p) => p.equals(m));
      if (!own && m.intersectsBox(v)) out.push(`bed ${String(i)} ${fmtBox(bed)}: ${fmt(m)} through it`);
      if (own && m.max.y - m.min.y < SOFT_H) continue;
      if (body.some((p) => p.intersectsBox(m))) out.push(`napper ${String(i)}: ${fmt(m)} through it`);
    }
  });
  // a bim at its desk: its desk and its chair only
  for (const [name, d] of o.desks) {
    const f = deskBox(o, d);
    const body = seated.get(name) ?? [];
    for (const m of meshes) {
      if (f && m.min.x < f.x1 + 0.05 && m.max.x > f.x0 - 0.05 && m.min.z < f.z1 + 0.05 && m.max.z > f.z0 - 0.05) continue;
      const c = m.getCenter(new Vector3());
      if (Math.hypot(c.x - d.x, c.z - d.z) < 0.45) continue;
      if (body.some((p) => p.intersectsBox(m))) out.push(`${name} at its desk: ${fmt(m)} through it`);
    }
  }
  return out;
}

/**
 * the office of `n` agents on map `id`, laid out once for the whole file: the string and nap laws
 * both walk 0..31 agents on every map, and the 32 layouts were most of the nap law's time (61 s at
 * the whole suite's load, its budget 60 s). An office is a plain value no law changes.
 */
const offices = new Map<string, Office>();
function officeAt(id: MapId, n: number): Office {
  const key = `${id}|${String(n)}`;
  let o = offices.get(key);
  if (!o) {
    o = layoutOf(crowd(n).agents, id);
    offices.set(key, o);
  }
  return o;
}

/** a string over a napper's head on the screen, at the office's fits (index.ts applyFit): the last stage, every bed */
function stringReds(o: Office): string[] {
  const w = buildWorld(o, 'main', STYLE, lookOf(o.map), false, { ...NO_GROW, stage: LAST_STAGE });
  w.root.updateMatrixWorld(true);
  const naps = w.naps;
  const bedAt = (p: Pt): NapRest | undefined => restAt(o, naps, p);
  const strings: Box3[] = [];
  // a long string in short pieces along it: its whole box on the screen is a wide rectangle its line only crosses
  w.root.traverse((m) => { if (m.userData.string === true) strings.push(...pieces(new Box3().setFromObject(m, true))); });
  w.dispose();
  if (strings.length === 0) return [];
  const out: string[] = [];
  const fits: readonly [string, Fit][] = [['320', fitSite(siteOf(o), 320, 0)], ['640', fitSite(wideSiteOf(o), 640, 0)]];
  o.lounge.slice(0, bedsAt(LAST_STAGE)).forEach((s, i) => {
    const head = bimBoxes(napName(i), s, NAP, true, bedAt).reduce((a, b) => a.union(b), new Box3());
    for (const [px, fit] of fits) {
      const r = onScreen(fit, head);
      for (const b of strings) if (depth(b) > depth(head) && meets(r, onScreen(fit, b))) out.push(`${px}: napper ${String(i)}'s head under a string ${fmt(b)}`);
    }
  });
  return out;
}

describe('clip: no string over a napper\'s head on the screen', () => {
  for (const id of readyMaps()) {
    it(`${id}: every bed, 320 and 640, 0..31 agents (each room size once)`, async () => {
      const out: string[] = [];
      const seen = new Set<string>();
      for (let n = 0; n <= 31; n++) {
        const o = officeAt(id, n);
        if (seen.has(o.size)) continue;
        seen.add(o.size);
        out.push(...stringReds(o).map((r) => `${String(n)} agents, ${r}`));
        await turn();
      }
      expect([...new Set(out)]).toEqual([]);
    }, 120_000);
  }
});

describe('clip: the nap beds hold their nappers', () => {
  for (const id of readyMaps()) {
    it(`${id}: beds apart and off the furniture, each napper inside its own, 0..31 agents`, async () => {
      const out: string[] = [];
      const bodies = new Map<string, Box3>();
      for (let n = 0; n <= 31; n++) {
        out.push(...planReds(officeAt(id, n), bodies).map((r) => `${String(n)} agents, ${r}`));
        await turn();
      }
      expect([...new Set(out)]).toEqual([]);
    }, 60_000);
  }
});

describe('clip: nothing through a desk, a bed or a bim', () => {
  for (const id of readyMaps()) {
    it(`${id}: stages 0..${String(LAST_STAGE)}, ${WORLDS.join(' and ')} agents`, async () => {
      const out: string[] = [];
      for (const n of WORLDS) {
        const o = officeAt(id, n);
        const seated = new Map([...o.desks.entries()].map(([name, d]) => [name, bimBoxes(name, d, WORK)] as const));
        for (let stage = 0; stage <= LAST_STAGE; stage++) {
          out.push(...worldReds(o, stage, seated).map((r) => `${String(n)} agents, stage ${String(stage)}, ${r}`));
          await turn();
        }
      }
      expect(out).toEqual([]);
      // 30 worlds built per map: half a minute alone, more on a loaded machine
    }, 180_000);
  }
});

describe('clip: the arcade cabinet stands clear of the whiteboard', () => {
  // campus put its arcade (stage 11) at the lounge's end, 0.27 into the whiteboard (stage 4) on the
  // back wall: the cabinet's purple box through the board (Gabriel, main m_51988)
  for (const id of ['loft', 'greenhouse', 'campus'] as const) {
    it(`${id}: 0..31 agents`, () => {
      const out: string[] = [];
      for (let n = 0; n <= 31; n++) {
        const o = officeAt(id, n);
        const look = lookOf(id);
        const a = look.pieceAt(o, 11);
        const b = look.pieceAt(o, 4);
        // the board 1.8 wide on the back wall, the cabinet ARCADE_W wide, 0.45 deep from the wall
        const gap = Math.abs(a.x - b.x) - 0.9 - ARCADE_W / 2;
        if (a.z < 1 && gap < 0.1) out.push(`${String(n)} agents: arcade at ${a.x.toFixed(2)}, board at ${b.x.toFixed(2)}, ${gap.toFixed(2)} apart`);
      }
      expect([...new Set(out)]).toEqual([]);
    });
  }
});
