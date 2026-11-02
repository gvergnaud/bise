// The bims review's skatepark (bim-review, bim-3d m_49876: California's park whole from stage 0, 13
// new tricks): the real California office built by the app's own world (maps layoutOf, world.ts
// buildWorld, the map's look) at a growth stage, seen pinched in on the park; and a skater on its
// piece at any trick of its lines (skate.ts lineOf, render/skate.ts skateRig: the app's rig), so a
// strip shows the trick from its start to its end. Studios only, never the app. Owner: bim-review.
import { Box3, Vector3 } from 'three';
import { makeBim } from '../render/bim';
import { type Fit, RIGHT, UP } from '../render/camera';
import { DAY, NIGHT, outfitOf } from '../render/palette';
import { skateRig } from '../render/skate';
import { STYLE } from '../render/style';
import type { View } from '../render/view';
import { buildWorld, castShadows, lightOffice } from '../render/world';
import { lookOf } from '../render/maps/index';
import { layoutOf } from '../maps/index';
import { NO_GROW } from '../growth';
import { SESSION_S, type TrickKey, lineOf } from '../skate';
import type { MapId, Office, Park, Pose, SkatePastime } from '../model';
import type { Feature } from '../parkpath';
import { agent } from '../fixtures';

/** a California office with a few desks: its park is the map's, whatever the agents */
function office(): Office {
  return layoutOf([agent('main', { main: true }), agent('a1'), agent('a2'), agent('a3')], 'california');
}

const parkOf = (o: Office): Park => (o as unknown as { park: Park }).park;

/** the iso camera framed on a box of the world, `size` px square */
function fitOn(b: Box3, size: number): Fit {
  const target = { x: (b.min.x + b.max.x) / 2, y: (b.min.y + b.max.y) / 2, z: (b.min.z + b.max.z) / 2 };
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (const x of [b.min.x, b.max.x]) for (const y of [b.min.y, b.max.y]) for (const z of [b.min.z, b.max.z]) {
    const p = { x: x - target.x, y: y - target.y, z: z - target.z };
    const u = p.x * RIGHT.x + p.y * RIGHT.y + p.z * RIGHT.z;
    const v = p.x * UP.x + p.y * UP.y + p.z * UP.z;
    x0 = Math.min(x0, u); x1 = Math.max(x1, u); y0 = Math.min(y0, v); y1 = Math.max(y1, v);
  }
  const half = Math.max(x1 - x0, y1 - y0) / 2;
  const cx = (x0 + x1) / 2;
  const cy = (y0 + y1) / 2;
  return { target, left: cx - half, right: cx + half, top: cy + half, bottom: cy - half, scale: (2 * half) / size, widthPx: size, heightPx: size };
}

function shoot(v: View, size: number, fit: Fit): HTMLCanvasElement {
  v.renderer.setSize(size, size, false);
  v.look(fit);
  v.render();
  const out = document.createElement('canvas');
  out.width = size;
  out.height = size;
  v.copyTo(out);
  return out;
}

/** the park at a stage, pinched in on its slab, by day or night */
export function parkShot(v: View, stage: number, night: boolean, size: number): HTMLCanvasElement {
  const o = office();
  const w = buildWorld(o, 'main', STYLE, lookOf('california'), night, { ...NO_GROW, stage });
  // the sky's lamps (the app's sky-light does it): the floodlights' and the room's pools at night
  w.lit(night ? 1 : 0);
  castShadows(w.root);
  v.scene.add(w.root);
  lightOffice(v.hemi, v.sun, night ? NIGHT : DAY, o, STYLE);
  const a = parkOf(o).area;
  const out = shoot(v, size, fitOn(new Box3(new Vector3(a.x0, 0, a.z0), new Vector3(a.x1, 1.6, a.z1)), size));
  v.scene.remove(w.root);
  return out;
}

/**
 * Any map's office of `n` agents at a stage, by day or night, framed round two of its stage
 * pieces (look.pieceAt: 11 the arcade, 4 the whiteboard by default) with `pad` units round them,
 * or whole when `pieces` is empty: a check that two pieces clear each other where they stand.
 */
export function mapShot(v: View, id: MapId, stage: number, night: boolean, n: number, size: number, pieces: readonly number[] = [11, 4], pad = 2.2): HTMLCanvasElement {
  const o = layoutOf([agent('main', { main: true }), ...Array.from({ length: Math.max(0, n - 1) }, (_, i) => agent(`a${String(i + 1)}`))], id);
  const look = lookOf(id);
  const w = buildWorld(o, 'main', STYLE, look, night, { ...NO_GROW, stage });
  w.lit(night ? 1 : 0);
  castShadows(w.root);
  v.scene.add(w.root);
  lightOffice(v.hemi, v.sun, night ? NIGHT : DAY, o, STYLE);
  const box = new Box3();
  if (pieces.length === 0) box.setFromObject(w.root, true);
  else for (const s of pieces) {
    const p = look.pieceAt(o, s);
    box.expandByPoint(new Vector3(p.x - pad, 0, p.z - pad));
    box.expandByPoint(new Vector3(p.x + pad, 2.2, p.z + pad));
  }
  const out = shoot(v, size, fitOn(box, size));
  v.scene.remove(w.root);
  return out;
}

/** a line holding `trick` (on that piece, when `on` is given) for some skater: its name, session and the move's start and length (s) */
export function findTrick(trick: TrickKey, park: Park, p: SkatePastime, on?: Feature): { name: string; n: number; t0: number; dur: number } | null {
  for (let i = 0; i < 400; i++) {
    const name = `sk8-${String(i)}`;
    for (let n = 0; n < 6; n++) {
      const m = lineOf(name, n, p, park).moves.find((x) => x.trick === trick && !x.bail && (on === undefined || x.on === on));
      if (m) return { name, n, t0: m.t0, dur: m.dur };
    }
  }
  return null;
}

/** a skater's pose `u` s after it reached its spot */
function skatePose(p: SkatePastime, u: number): Pose {
  const spot = { x: 0, z: 0, wz: 0, ax: 0, face: 0 };
  const act = { k: 'nap', at: 'lounge' } as const;
  return { x: 0, z: 0, dir: 0, moving: false, since: u, seg: { t: 0, act, sig: 'nap', spot, path: { pts: [spot], seg: [], len: 0 }, speed: 3.4, from: null, pastime: p } };
}

/** `frames` shots of one trick on its piece (the park at `stage`), from its start to its end, framed on the skater's whole way */
export function trickStrip(v: View, trick: TrickKey, stage: number, night: boolean, size: number, frames: number, on?: Feature): { name: string; shots: HTMLCanvasElement[] } | null {
  const o = office();
  const park = parkOf(o);
  const p: SkatePastime = { kind: 'skate', slot: 0, pool: false, nine: false, stage };
  const found = findTrick(trick, park, p, on);
  if (!found) return null;
  const w = buildWorld(o, 'main', STYLE, lookOf('california'), night, { ...NO_GROW, stage });
  w.lit(night ? 1 : 0);
  castShadows(w.root);
  v.scene.add(w.root);
  lightOffice(v.hemi, v.sun, night ? NIGHT : DAY, o, STYLE);
  const bim = makeBim(found.name, false, STYLE, outfitOf(found.name, false));
  castShadows(bim.group);
  bim.detail(true);
  bim.daylight(night ? 0 : 1);
  v.scene.add(bim.group);
  const ts = Array.from({ length: frames }, (_, i) => found.n * SESSION_S + found.t0 + (found.dur * (i + 0.5)) / frames);
  // the frame: the skater's whole way through the trick, with room round it
  const way = new Box3();
  for (const t of ts) {
    const r = skateRig(found.name, skatePose(p, t), park, 0);
    if (r) way.expandByPoint(new Vector3(r.x, r.y, r.z));
  }
  way.min.y = 0;
  // a bim's height and an air's reach over its rig's point (a bs_air at the vert's lip went out of the frame)
  way.max.y += 2.4;
  way.expandByScalar(0.9);
  const fit = fitOn(way, size);
  const shots: HTMLCanvasElement[] = [];
  for (const t of ts) {
    const r = skateRig(found.name, skatePose(p, t), park, 0);
    if (!r) continue;
    bim.apply(r);
    shots.push(shoot(v, size, fit));
  }
  v.scene.remove(bim.group);
  v.scene.remove(w.root);
  bim.dispose();
  return { name: found.name, shots };
}
