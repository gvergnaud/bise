// The objects as drawn in the office (objects-layer.ts on corners.ts's spots, designer m_41229):
// within a cluster things keep a small gap and never overlap, on the drawn shapes (every mesh of
// each object but its contact shadow, turned), not just the footprints; and a lit lamp's rug is
// above the floor. Fixtures only; three without a renderer.
import { describe, expect, it } from 'vitest';
import { Box3, Mesh, type Object3D } from 'three';
import { layoutOf } from '../maps';
import { MAP_IDS } from '../maps/ids';
import { objectSpots } from '../corners';
import { crewOf, objsOf } from '../corners.test';
import { makeObjectsLayer } from './objects-layer';
import { STYLE } from './style';

/** the gap two drawn objects keep at least (designer: ~0.2; the footprints keep 0.3) */
const DRAWN_GAP = 0.15;

function meshes(o: Object3D): Mesh[] {
  const out: Mesh[] = [];
  o.traverse((m) => {
    if (m instanceof Mesh) out.push(m as Mesh);
  });
  return out;
}

/** each object's drawn floor box (its meshes but the contact shadow, in the office's frame) */
function drawn(root: Object3D): { key: string; box: Box3 }[] {
  root.updateMatrixWorld(true);
  return root.children
    .filter((g) => typeof g.userData.plugin === 'string')
    .map((g) => {
      const box = new Box3();
      for (const m of meshes(g)) if (m.userData.contact !== true) box.union(new Box3().setFromObject(m));
      return { key: g.userData.plugin as string, box };
    });
}

describe('objects-layer: the objects as drawn', () => {
  it.each(MAP_IDS)('%s: no two drawn objects overlap or touch closer than the gap', (map) => {
    const found: string[] = [];
    for (const [n, k] of [[3, 10], [12, 20], [30, 34]] as const) {
      const o = layoutOf(crewOf(n), map);
      const layer = makeObjectsLayer(STYLE);
      layer.set(objectSpots(o, objsOf(k)), false);
      const ds = drawn(layer.root);
      for (let i = 0; i < ds.length; i++)
        for (let j = i + 1; j < ds.length; j++) {
          const a = (ds[i] as { key: string; box: Box3 }).box;
          const b = (ds[j] as { key: string; box: Box3 }).box;
          const gap = Math.max(b.min.x - a.max.x, a.min.x - b.max.x, b.min.z - a.max.z, a.min.z - b.max.z);
          if (gap < DRAWN_GAP) found.push(`${map}/${String(n)} ${ds[i]?.key ?? ''} ~ ${ds[j]?.key ?? ''} ${gap.toFixed(2)}`);
        }
      layer.dispose();
    }
    expect(found.slice(0, 8)).toEqual([]);
  });

  // his hover's tip stands 8 px from the drawn edge (designer m_52085): an object's silhouette is
  // its meshes' shape, never its contact shadow's (a disc wider than the thing)
  it("an object's silhouette is its drawn meshes', its contact shadow left out", () => {
    const layer = makeObjectsLayer(STYLE);
    layer.set(objectSpots(layoutOf(crewOf(5), 'loft'), objsOf(12)), false);
    const ds = drawn(layer.root);
    expect(ds.length).toBeGreaterThan(0);
    let shadowWider = 0;
    for (const { key, box } of ds) {
      const pts = layer.shapes().get(key) ?? [];
      expect(pts.length, key).toBeGreaterThan(0);
      const out = pts.filter((p) => p.x < box.min.x - 1e-6 || p.x > box.max.x + 1e-6 || p.z < box.min.z - 1e-6 || p.z > box.max.z + 1e-6);
      expect(out, `${key}: a point past its drawn meshes`).toEqual([]);
      const whole = new Box3().setFromObject(layer.root.getObjectByName(`obj:${key}`) as Object3D);
      if (whole.max.x - whole.min.x > box.max.x - box.min.x + 0.05) shadowWider++;
    }
    // the shadow is wider than the thing for some: the silhouette is what keeps the tip close
    expect(shadowWider).toBeGreaterThan(0);
    layer.dispose();
  });
});
