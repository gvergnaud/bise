// The furniture every map's look may use (three): a laptop whose screen the frame lights, a desk
// lamp lit at night, a chair, and the box in front of a chair (a desk is ahead of its spot, +z).
// Split out of world.ts when the maps came (render/maps/look.ts); nothing here decides.
import { Group, type MeshLambertMaterial, type Object3D } from 'three';
import type { Box, Pt } from '../model';
import { CHAIR } from '../sit';
import type { Kit } from './kit';
import { LAPTOP } from './laptop';
import { WORLD } from './palette';

/** a laptop, its lid towards the one who sits at it (towards -z), its screen a material of its own; its sizes: laptop.ts LAPTOP (a sleeper's head stays clear of it) */
export function laptop(k: Kit, parent: Object3D, x: number, z: number): MeshLambertMaterial {
  const { style, pal } = k;
  const { base, lid: l } = LAPTOP;
  k.add(style.box(base.w, base.h, base.d), k.mat(WORLD.metal), parent, x, base.y, z);
  const lid = new Group();
  lid.position.set(x, l.y, z + l.back);
  lid.rotation.x = l.tilt;
  parent.add(lid);
  k.add(style.box(l.w, l.h, l.d), k.mat(WORLD.metal), lid, 0, l.h / 2, 0);
  const screen = style.material(pal.screenOff, { emissive: pal.screenOn, ei: 0 }) as MeshLambertMaterial;
  // lit and dimmed while an office is drawn live (lightDesk): never baked into its walls (merge.ts)
  screen.userData.live = true;
  k.add(style.box(0.44, 0.28, 0.01), screen, lid, 0, l.h / 2, -0.016);
  return screen;
}

// a small cream lamp on a short stem, day and night (bims m_27836, m_28080: dark ones read as ink
// glyphs, a 'T' on each desk, at 640), its bulb lit at night only (lightDesk)
export function deskLamp(k: Kit, parent: Object3D, x: number, z: number): MeshLambertMaterial {
  const { style, pal } = k;
  k.add(style.box(0.11, 0.025, 0.11), k.mat(pal.lamp), parent, x, 0.56, z);
  k.add(style.box(0.025, 0.13, 0.025), k.mat(pal.lamp), parent, x, 0.635, z);
  k.add(style.box(0.12, 0.06, 0.1), k.mat(pal.lamp), parent, x - 0.03, 0.725, z);
  const bulb = style.material(pal.lamp, { emissive: pal.lampOn, ei: 0 }) as MeshLambertMaterial;
  bulb.userData.live = true;
  k.add(style.box(0.08, 0.02, 0.06), bulb, parent, x - 0.03, 0.69, z);
  return bulb;
}

/** a desk chair, where it stands in its parent: a poke's chair spin turns it with its bim (office-scene.ts seatChair) */
export interface Seat {
  readonly obj: Object3D;
  readonly x: number;
  readonly z: number;
  readonly face: number;
}

/** a chair at (x, z), its back away from where it faces */
export function chair(k: Kit, parent: Object3D, x: number, z: number, color: string, face: number): Seat {
  const g = new Group();
  g.position.set(x, 0, z);
  g.rotation.y = face;
  parent.add(g);
  // the seat at a seated bim's hips (sit.ts CHAIR.seatTop; his m_50679: they sat 20 cm into a seat
  // at 0.36), the back from it up, on a short post
  const top = CHAIR.seatTop;
  // its front CHAIR.front ahead of its middle, its back where it always was (0.21 behind)
  const deep = CHAIR.front + 0.21;
  k.add(k.style.box(0.42, 0.08, deep), k.mat(color), g, 0, top - 0.04, (CHAIR.front - 0.21) / 2);
  k.add(k.style.box(0.42, CHAIR.backTop - top + 0.02, 0.07), k.mat(color), g, 0, (CHAIR.backTop + top - 0.02) / 2, -0.2);
  k.add(k.style.cylinder(0.03, top - 0.08), k.mat(k.pal.ink), g, 0, (top - 0.08) / 2, 0);
  // kept out of a town's bake of its office: the chair slides under its bim (town.ts walls, looseChairs)
  g.userData.chair = true;
  return { obj: g, x, z, face };
}

/** a desk chair under a poke's chair spin (Rig.seat): rolled back along its own way and turned; none: where it stands */
export function seatChair(c: Seat | undefined, s: { readonly back: number; readonly turn: number } | undefined): void {
  if (!c) return;
  const back = s?.back ?? 0;
  c.obj.position.set(c.x - Math.sin(c.face) * back, 0, c.z - Math.cos(c.face) * back);
  c.obj.rotation.y = c.face + (s?.turn ?? 0);
}

/**
 * The desk chairs (furniture.ts chair: userData.chair) taken out of `world` into `root`, each in a
 * holder standing where its parent stood, so a bake of the world (town.ts walls) leaves them out and
 * seatChair still slides them in their parent's frame (his m_50679: in the hero's town the bake drew
 * every chair where it was built, the bims going through it). The holders, for the caller to drop.
 */
export function looseChairs(world: Object3D, root: Object3D): Group[] {
  root.updateMatrixWorld(true);
  const chairs: Object3D[] = [];
  world.traverse((o) => {
    if (o.userData.chair === true) chairs.push(o);
  });
  const toRoot = root.matrixWorld.clone().invert();
  return chairs.flatMap((c) => {
    const parent = c.parent;
    if (parent === null) return [];
    const holder = new Group();
    holder.matrixAutoUpdate = false;
    holder.matrix.multiplyMatrices(toRoot, parent.matrixWorld);
    holder.userData.chairsOf = world.id;
    holder.add(c);
    root.add(holder);
    holder.updateMatrixWorld(true);
    return [holder];
  });
}

/** the furniture box in front of a chair (Office.furniture: a desk is ahead of its spot, +z) */
export function boxAhead(boxes: readonly Box[], p: Pt): Box | null {
  let best: Box | null = null;
  for (const b of boxes) {
    if (b.of || p.x < b.x0 || p.x > b.x1 || b.z0 < p.z) continue;
    if (!best || b.z0 < best.z0) best = b;
  }
  return best;
}

