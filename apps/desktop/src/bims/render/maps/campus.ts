// The campus's look (render/maps/look.ts; its plan: maps/campus.ts): a pale floor, white walls with
// windows on the left, a round coloured carpet under each pod of four white desks, the long orange
// counter on the left wall (the eating place), stepped seats against the back wall behind the bean
// bags; its growth is the loft's (blook.ts loftGrower). Owner: maps-b.
import type { CampusOffice } from '../../maps/campus';
import { decorOf } from '../decor';
import { boxOf, onFloor } from '../kit';
import { WORLD } from '../palette';
import { loftLounge, loftTree } from '../stages';
import { bigDesk, boxHolding, dim, grow, loftGrower, palettes, pastBoard, pieceAtOf, turnedDesk } from './blook';
import type { Build, MapLook } from './look';

const CARPETS = ['#bfe1f2', '#ffe3a3', '#cfe8c3', '#e2dcf7'] as const;
const STEPS = ['#f4a259', '#ffd166', '#e9c46a'] as const;

const cp = (b: Build): CampusOffice => b.o as CampusOffice;
const loungeX = (o: { lounge: readonly { x: number }[]; x0: number }): [number, number] => {
  const xs = o.lounge.map((s) => s.x);
  return xs.length ? [Math.min(...xs) - 0.4, Math.max(...xs) + 0.4] : [o.x0 + 0.3, o.x0 + 4];
};

const grower = loftGrower({
  fruit: (o) => ({ x: (o.island.x0 + o.island.x1) / 2, z: o.island.z0 + 1.6 }),
  fruit2: (o) => ({ x: (o.island.x0 + o.island.x1) / 2, z: o.island.z0 + 2.9 }),
  boombox: (o) => ({ x: (o.island.x0 + o.island.x1) / 2, z: o.island.z1 - 0.5 }),
  mural: (o) => loungeX(o),
  // past the whiteboard, as the loft's: at the lounge's end it stood 0.27 into the board (main m_51988)
  arcade: pastBoard,
  nap: (o) => ({ x: (o.lounge[0]?.x ?? o.x0) + 1.0, z: 1.5 }),
});

/** the floor, the walls and windows, the pods' carpets, the stepped seats, the door mat */
function shell(b: Build): void {
  const { k, root } = b;
  const { style, pal } = k;
  const o = cp(b);
  const { x0, W, D } = o;
  k.add(style.box(W, 0.14, D), style.floor(pal.floor, W, D), root, x0 + W / 2, -0.05, D / 2);
  k.add(style.box(W + 0.2, 1.7, 0.2), k.mat(pal.wall), root, x0 + W / 2, 0.85, -0.1);
  k.add(style.box(0.2, 1.0, D + 0.2), k.mat(pal.wall), root, x0 - 0.1, 0.5, D / 2);
  const glass = k.glow(pal.window, { night: 0.4 });
  for (let z = 4.6; z < D - 1; z += 2.3) k.add(style.box(0.05, 0.45, 1.2), glass, root, x0, 0.62, z + 0.6);
  // a round carpet under every pod that has a desk
  const used = new Set([...o.slots.values()].map((s) => Math.floor(s / 4)));
  o.pods.forEach((p, i) => {
    if (!used.has(i)) return;
    const c = CARPETS[i % CARPETS.length] ?? CARPETS[0];
    k.add(style.cylinder(2.0, 0.03), k.mat(pal.pendants ? dim(c, 0.55) : c), root, p.x, 0.02, p.z);
  });
  // the stepped seats against the back wall, behind the bean bags
  const [l0, l1] = loungeX(o);
  STEPS.forEach((c, s) => k.add(style.box(l1 - l0, 0.2 * (3 - s), 0.18), k.mat(pal.pendants ? dim(c, 0.7) : c), root, (l0 + l1) / 2, 0.1 * (3 - s), 0.1 + s * 0.17));
  onFloor(k, root, boxOf(decorOf(o).doorMat), 0.04, 0.03, WORLD.mat);
}

/** the long counter on the left wall, the coffee machine at its back end */
function island(b: Build): void {
  const { k, root, o } = b;
  const { style, pal } = k;
  const I = o.island;
  const x = (I.x0 + I.x1) / 2;
  const zc = (I.z0 + I.z1) / 2;
  const len = I.z1 - I.z0;
  k.add(style.box(I.x1 - I.x0 - 0.1, 0.62, len - 0.1, 0.15), k.mat(pal.island), root, x, 0.31, zc);
  k.add(style.box(I.x1 - I.x0 + 0.05, 0.07, len), k.mat(pal.islandTop), root, x, 0.66, zc);
  k.add(style.box(0.42, 0.45, 0.4), k.mat(WORLD.coffee), root, x, 0.92, I.z0 + 0.4);
  k.add(style.box(0.04, 0.05, 0.16), style.material(WORLD.coffeeLed, { emissive: WORLD.coffeeLed, ei: 0.5 }), root, x + 0.22, 1.04, I.z0 + 0.4);
}

export const campus: MapLook = {
  id: 'campus',
  // its cozy pieces (render/cozy.ts; page bims-cozy v2 « la base, carte par carte »)
  cozy: { plant: 'ficus', small: 'succulent', coffee: 'filter', bed: 'sofa', shelf: 'globe', poster: 'pennant', wood: '#9a6a48', cloth: ['#3d6fb6', '#e9c46a', '#e9e2d4'], art: ['#3d6fb6', '#e9c46a', '#7cc26b'], pot: '#c98a3e', rug: '#3d6fb6' },
  pal: palettes(
    { floor: '#efe6d8', wall: '#fbf6ec', wallTop: '#e4d6bf', desk: '#f6f2ea', deskTop: '#ffffff', island: '#f39a4c', islandTop: '#fffaf0' },
    { desk: '#c9c2b4', deskTop: '#ddd6c8' },
  ),
  shell,
  island,
  bedShape: 'sink',
  lounge: loftLounge,
  desk: (b, d, name) => turnedDesk(b, d, name, { w: 1.6, d: 0.78, at: 0.5 }),
  mainDesk: (b) => {
    const md = b.o.mainDesk;
    const box = boxHolding(b.o, { x: md.x, z: md.z + 0.6 }) ?? { x0: md.x - 1.15, x1: md.x + 1.15, z0: md.z + 0.1, z1: md.z + 1.1 };
    return bigDesk(b, box, md);
  },
  tree: ({ k }, parent, x, z, size) => { loftTree(k, parent, x, z, size); },
  growth: (b) => { grow(b, grower); },
  pieceAt: pieceAtOf(grower),
};
