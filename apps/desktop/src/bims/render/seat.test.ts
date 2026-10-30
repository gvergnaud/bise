// @vitest-environment happy-dom
// A bim on its desk chair (his m_50679, the hero film): its bottom on the seat, never in it; and in
// a town's bake of its office the chair stays its own object, so it slides as its bim sits and gets up.
import { describe, expect, it } from 'vitest';
import { Box3, type BufferGeometry, Group, Mesh, Vector3, type Object3D } from 'three';
import { makeBim } from './bim';
import { NO_PROPS, type Rig } from './pose';
import { STYLE } from './style';
import { chair, looseChairs, seatChair } from './furniture';
import { mergeRoot } from './merge';
import { kitOf } from './kit';
import { DAY } from './palette';
import { CHAIR } from '../sit';

/** the seated rig (render/pose.ts SEATED: down 0.05, its legs out at -1.45) */
const seated: Rig = {
  x: 0, z: 0, dir: 0, y: -0.05, body: [0, 0, 0], head: [0, 0, 0], armL: [-1.2, 0, 0], armR: [-1.2, 0, 0], legL: -1.45, legR: -1.45,
  eyes: 1, mouth: 'smile', props: NO_PROPS, screen: 0, lamp: false, grey: 0, opacity: 1, motion: 'still',
};

const boxOf = (o: Object3D): Box3 => {
  o.updateWorldMatrix(true, true);
  const b = new Box3();
  o.traverse((m) => {
    if (m instanceof Mesh && m.visible) b.union(new Box3().setFromObject(m));
  });
  return b;
};

describe('a bim on its chair', () => {
  it('sits on the seat: its hoodie and legs over it rest on its top, nothing under it (his m_50679: 20 cm into it)', () => {
    const b = makeBim('ada', false, STYLE);
    b.apply(seated);
    b.group.updateWorldMatrix(true, true);
    // the chair's seat under its bim: CHAIR.dz behind it, CHAIR.half each way
    const seat = { x: CHAIR.half, z0: -CHAIR.dz - CHAIR.half, z1: -CHAIR.dz + CHAIR.front };
    const under = (o: Object3D): boolean => {
      for (let p: Object3D | null = o; p; p = p.parent) if (['armL', 'armR', 'head'].includes(p.name) || !p.visible) return false;
      return true;
    };
    let low = Infinity;
    const v = new Vector3();
    b.group.traverse((m) => {
      if (!(m instanceof Mesh) || !under(m)) return;
      const pos = (m.geometry as BufferGeometry).getAttribute('position');
      for (let i = 0; i < pos.count; i++) {
        v.fromBufferAttribute(pos, i).applyMatrix4(m.matrixWorld);
        if (Math.abs(v.x) <= seat.x && v.z >= seat.z0 && v.z <= seat.z1) low = Math.min(low, v.y);
      }
    });
    expect(low).toBeGreaterThanOrEqual(CHAIR.seatTop - 0.02);
    expect(low).toBeLessThanOrEqual(CHAIR.seatTop + 0.04);
  });

  it('the chair built at CHAIR: its seat top and its back top', () => {
    const k = kitOf(STYLE, DAY);
    const g = new Group();
    const c = chair(k, g, 0, 0, '#c49a6c', 0);
    const b = boxOf(c.obj);
    expect(b.max.y).toBeCloseTo(CHAIR.backTop, 2);
    // the seat: the top of what stands in front of its back
    let seatTop = 0;
    c.obj.traverse((m) => {
      if (m instanceof Mesh) {
        const mb = new Box3().setFromObject(m);
        if (mb.max.z > 0) seatTop = Math.max(seatTop, mb.max.y);
      }
    });
    expect(seatTop).toBeCloseTo(CHAIR.seatTop, 2);
  });

  it("stays its own object in a town's bake of its office, and slides there (his m_50679: the bake's chair never moved)", () => {
    const k = kitOf(STYLE, DAY);
    const root = new Group();
    const world = new Group();
    root.add(world);
    // a desk group turned and placed, its chair in it (blook, nordic)
    const desk = new Group();
    desk.position.set(2, 0, 1);
    desk.rotation.y = 0.7;
    world.add(desk);
    const c = chair(k, desk, 0, -0.05, '#c49a6c', 0);
    const before = new Vector3().setFromMatrixPosition(c.obj.matrixWorld.clone());
    c.obj.updateWorldMatrix(true, false);
    before.setFromMatrixPosition(c.obj.matrixWorld);
    const holders = looseChairs(world, root);
    expect(holders).toHaveLength(1);
    // out of the world: a bake of it leaves the chair out; where it stands, unmoved
    let inWorld = false;
    world.traverse((o) => {
      if (o === c.obj) inWorld = true;
    });
    expect(inWorld).toBe(false);
    c.obj.updateWorldMatrix(true, false);
    expect(new Vector3().setFromMatrixPosition(c.obj.matrixWorld).distanceTo(before)).toBeLessThan(1e-6);
    const baked = mergeRoot(world, STYLE, false);
    expect(baked.group.children.length).toBe(0);
    baked.dispose();
    // its slide back, in its desk's frame as before
    seatChair(c, { back: 0.5, turn: 0 });
    c.obj.updateWorldMatrix(true, false);
    const after = new Vector3().setFromMatrixPosition(c.obj.matrixWorld);
    expect(after.distanceTo(before)).toBeCloseTo(0.5, 5);
  });
});
