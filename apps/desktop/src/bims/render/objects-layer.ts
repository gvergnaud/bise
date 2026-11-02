// His plugins' objects drawn in the office (bims-plugins plan p4): each placed object
// (corners.ts objectSpots) built by the registry (objects/index.ts makeObject: glass when off, its
// compact one when the office had no room for it) at its spot, turned, and a rug under each
// corner's cluster. Built again only when the spots or the theme change (a still office draws no
// frame for them). Owner: bims-life.
import { Box3, type BufferGeometry, Group, Mesh, type Object3D, Vector3 } from 'three';
import type { Corners, Rug } from '../corners';
import { makeObject } from './objects';
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
  dispose(): void;
}

export function makeObjectsLayer(style: Style): ObjectsLayer {
  const root = new Group();
  root.name = 'plugin-objects';
  let drawn: Corners | null = null;
  let drawnNight = false;
  const tops = new Map<string, { x: number; y: number; z: number }>();
  const shapes = new Map<string, readonly Vec3[]>();

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
      for (const r of c.rugs) root.add(rug(r, night));
      for (const p of c.placed) {
        // a swapped one is already its compact id; makeObject's compact only scales the others
        const made = makeObject(p.obj, { off: p.glass, night: night ? 1 : 0, ...(p.n === undefined ? {} : { n: p.n }), compact: p.scaled });
        made.group.position.set(p.x, 0, p.z);
        made.group.rotation.y = p.rot;
        made.group.name = `obj:${p.key}`;
        made.group.userData.plugin = p.key;
        root.add(made.group);
        made.group.updateMatrixWorld(true);
        const b = new Box3().setFromObject(made.group);
        if (b.isEmpty()) continue;
        tops.set(p.key, { x: (b.min.x + b.max.x) / 2, y: b.max.y, z: (b.min.z + b.max.z) / 2 });
        shapes.set(p.key, silhouetteOf(made.group));
      }
    },
    topOf: (key) => tops.get(key) ?? null,
    shapes: () => shapes,
    dispose() {
      clear();
      tops.clear();
      shapes.clear();
      drawn = null;
    },
  };
}
