// His plugins' objects drawn in the office (bims-plugins plan p4): each placed object
// (corners.ts objectSpots) built by the registry (objects/index.ts makeObject: glass when off, its
// compact one when the office had no room for it) at its spot, turned, and a rug under each
// corner's cluster. Built again only when the spots or the theme change (a still office draws no
// frame for them). Owner: bims-life.
import { Box3, type BufferGeometry, Group, Mesh } from 'three';
import type { Corners, Rug } from '../corners';
import { makeObject } from './objects';
import type { Style } from './style';

/** a point in the office, world units */
interface Vec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
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
  /** every drawn object's box (world units, min and max corners): what his hover picks (pick.ts pickObjectAt) */
  boxes(): ReadonlyMap<string, { readonly min: Vec3; readonly max: Vec3 }>;
  dispose(): void;
}

export function makeObjectsLayer(style: Style): ObjectsLayer {
  const root = new Group();
  root.name = 'plugin-objects';
  let drawn: Corners | null = null;
  let drawnNight = false;
  const tops = new Map<string, { x: number; y: number; z: number }>();
  const boxes = new Map<string, { min: Vec3; max: Vec3 }>();

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
      boxes.clear();
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
        boxes.set(p.key, { min: { x: b.min.x, y: b.min.y, z: b.min.z }, max: { x: b.max.x, y: b.max.y, z: b.max.z } });
      }
    },
    topOf: (key) => tops.get(key) ?? null,
    boxes: () => boxes,
    dispose() {
      clear();
      tops.clear();
      boxes.clear();
      drawn = null;
    },
  };
}
