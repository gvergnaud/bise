// His plugins' objects drawn in the office (bims-plugins plan p4): each placed object
// (corners.ts objectSpots) built by the registry (objects/index.ts makeObject: glass when off, its
// compact one when the office had no room for it) at its spot, turned, and a rug under each
// corner's cluster. Built again only when the spots or the theme change (a still office draws no
// frame for them). Owner: bims-life.
import { Box3, type BufferGeometry, Group, Mesh, type Object3D, Vector3 } from 'three';
import type { Corners, Rug } from '../corners';
import { makeObject } from './objects';
import { HOVER_MS, MOVES, REST, hoverAt, type HoverPose } from './objects/hover';
import type { ObjId } from './objects/ids';
import type { Style } from './style';

/** a point in the office, world units */
export interface Vec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

/** at most this many of a mesh's points stand for it in its object's silhouette (evenly taken) */
const POINTS_PER_MESH = 48;

/**
 * An object's drawn shape as points in the office (world units): its meshes' points, but its
 * contact shadow's (a flat disc wider than the thing: its box read 120 px off the printer, designer
 * m_52085), at most POINTS_PER_MESH a mesh. Projected, their span is its silhouette's on screen.
 */
export function silhouetteOf(group: Object3D): Vec3[] {
  const out: Vec3[] = [];
  const v = new Vector3();
  group.updateMatrixWorld(true);
  group.traverse((m) => {
    if (!(m instanceof Mesh) || m.userData.contact === true) return;
    const pos = (m.geometry as BufferGeometry).getAttribute('position');
    const step = Math.max(1, Math.ceil(pos.count / POINTS_PER_MESH));
    for (let i = 0; i < pos.count; i += step) {
      v.fromBufferAttribute(pos, i).applyMatrix4(m.matrixWorld);
      out.push({ x: v.x, y: v.y, z: v.z });
    }
  });
  return out;
}

/** the corners' rugs (no pink or red: palette.test.ts), by day and at night */
export const RUG = {
  play: { day: '#bfe1f2', night: '#4c6b80' },
  read: { day: '#ffd166', night: '#8a7240' },
} as const;
/** a rug's thickness and its height: the nap rug's (stages.ts loftLounge), over the floor's top, under every foot */
const RUG_H = 0.02;
const RUG_Y = 0.035;

export interface ObjectsLayer {
  readonly root: Group;
  /** draw these spots in this theme; the same ones again: nothing rebuilt */
  set(c: Corners, night: boolean): void;
  /** the middle of an object's top as drawn (world units: its pill sits there, objpills.ts); null: not drawn */
  topOf(key: string): { readonly x: number; readonly y: number; readonly z: number } | null;
  /** every drawn object's silhouette (silhouetteOf, world units): what his hover picks (pick.ts pickObjectAt) and its tip stands by */
  shapes(): ReadonlyMap<string, readonly Vec3[]>;
  /**
   * the objects a working bim's call uses at t (uses.ts, Step.using): each plays its own move
   * (objects/hover.ts MOVES) every USE_EVERY s while used, the others at rest; whether one moves
   * now (the loop runs at full rate then). Reduced motion: none moves.
   */
  play(using: ReadonlySet<string>, t: number, reduced: boolean): boolean;
  dispose(): void;
}

/** s between two of an object's moves while a call uses it */
export const USE_EVERY = 2.4;

/** an object's move at t while used: its hover move at the start of every USE_EVERY s, rest between */
export function moveWhileUsed(obj: ObjId, t: number): HoverPose {
  const f = (((t % USE_EVERY) + USE_EVERY) % USE_EVERY) / (HOVER_MS / 1000);
  return f >= 1 ? REST : hoverAt(MOVES[obj], f);
}

export function makeObjectsLayer(style: Style): ObjectsLayer {
  const root = new Group();
  root.name = 'plugin-objects';
  let drawn: Corners | null = null;
  let drawnNight = false;
  const tops = new Map<string, { x: number; y: number; z: number }>();
  const shapes = new Map<string, readonly Vec3[]>();
  /** each drawn object's group, what it is and its rest place; the ones off rest now */
  const groups = new Map<string, { g: Object3D; obj: ObjId; x: number; z: number; rot: number }>();
  const moved = new Set<string>();

  const clear = (): void => {
    for (const c of [...root.children]) {
      root.remove(c);
      c.traverse((m) => {
        if (m instanceof Mesh) (m.geometry as BufferGeometry).dispose();
      });
    }
  };

  const rug = (r: Rug, night: boolean): Mesh => {
    const w = r.box.x1 - r.box.x0;
    const d = r.box.z1 - r.box.z0;
    const m = new Mesh(style.box(w, RUG_H, d, 0.2), style.material(night ? RUG[r.corner].night : RUG[r.corner].day));
    m.position.set((r.box.x0 + r.box.x1) / 2, RUG_Y, (r.box.z0 + r.box.z1) / 2);
    m.receiveShadow = true;
    m.name = `rug:${r.corner}`;
    return m;
  };

  return {
    root,
    set(c, night) {
      if (c === drawn && night === drawnNight) return;
      drawn = c;
      drawnNight = night;
      clear();
      tops.clear();
      shapes.clear();
      groups.clear();
      moved.clear();
      for (const r of c.rugs) root.add(rug(r, night));
      for (const p of c.placed) {
        // a swapped one is already its compact id; makeObject's compact only scales the others
        const made = makeObject(p.obj, { off: p.glass, night: night ? 1 : 0, ...(p.n === undefined ? {} : { n: p.n }), compact: p.scaled });
        made.group.position.set(p.x, 0, p.z);
        made.group.rotation.y = p.rot;
        made.group.name = `obj:${p.key}`;
        made.group.userData.plugin = p.key;
        root.add(made.group);
        groups.set(p.key, { g: made.group, obj: p.obj, x: p.x, z: p.z, rot: p.rot });
        made.group.updateMatrixWorld(true);
        const b = new Box3().setFromObject(made.group);
        if (b.isEmpty()) continue;
        tops.set(p.key, { x: (b.min.x + b.max.x) / 2, y: b.max.y, z: (b.min.z + b.max.z) / 2 });
        shapes.set(p.key, silhouetteOf(made.group));
      }
    },
    topOf: (key) => tops.get(key) ?? null,
    shapes: () => shapes,
    play(using, t, reduced) {
      let moving = false;
      for (const [key, d] of groups) {
        const on = using.has(key) && !reduced;
        if (!on && !moved.has(key)) continue;
        const h = on ? moveWhileUsed(d.obj, t) : REST;
        // the move in the object's own frame: aside along its x, its turns about its own axes
        d.g.position.set(d.x + Math.cos(d.rot) * h.dx, h.lift, d.z - Math.sin(d.rot) * h.dx);
        d.g.rotation.set(h.pitch, d.rot + h.yaw, h.roll, 'YXZ');
        d.g.scale.set(h.sx, h.sy, h.sz);
        if (h === REST) moved.delete(key);
        else {
          moved.add(key);
          moving = true;
        }
      }
      return moving;
    },
    dispose() {
      clear();
      tops.clear();
      shapes.clear();
      groups.clear();
      moved.clear();
      drawn = null;
    },
  };
}
