// The campus (page bims-maps v4; its look: render/maps/campus.ts): pods of four desks back to back,
// a little scattered, each on its round carpet; a long kitchen counter along the left wall (the
// eating place); steps to sit on in the nap place. The loft's back zone (bplan.ts), the door at the
// front left. Its walk graph: the left aisle (the counter's places stand on it), a corridor between
// every row of pods (the back one is the back corridor, the last one the front), each chair straight
// out to the corridor behind it. Pure. Owner: maps-b.
import type { Agent } from '@/proto';
import type { Box, Leg, Office, Pt, Spot } from '../model';
import { type BOffice, LOFT_LIKE, type Way, backZone, corridor, keepSlots, outside, queueMoreOf, walksOf } from './bplan';
import { type Plan, boxAt } from './plan';
import { ROOM } from './room';

export interface CampusOffice extends BOffice {
  /** every pod's middle, by pod (slot / 4) */
  readonly pods: readonly Pt[];
  /** the left aisle's x */
  readonly aisle: number;
}

const BACK = 4.05;
const PITCH_X = 5.2;
const PITCH_Z = 4.6;
/** the first pod column's middle from the left wall */
const PX0 = 4.2;
/** a little scatter, by pod: never off its band (its chairs stay 0.95 from the corridors) */
const JIT_Z = [0, 0.25, -0.2, 0.15, -0.1, 0.3] as const;

export function campusLayout(agents: readonly Agent[], prev?: CampusOffice): CampusOffice {
  const { names, slots, used } = keepSlots(agents, prev?.slots);
  const pods = Math.max(2, Math.ceil(Math.max(used, prev?.cap ?? 0) / 4));
  const cap = pods * 4;
  const cols = pods > 4 ? 3 : 2;
  const rowsP = Math.ceil(pods / cols);
  const W = cols * PITCH_X + 2.4;
  const x0 = -W / 2;
  const aisle = x0 + 1.45;
  const front = BACK + rowsP * PITCH_Z;
  const D = front + 0.6;

  // the beds clear of the stepped seats along the back wall (render/maps/campus.ts shell)
  const back = backZone(x0, W, BACK, { napZ: 1.65 });
  const out = outside(x0, D, aisle, front);
  const ways: Way[] = [...back.ways, ...out.ways];
  const furniture: Box[] = [...back.furniture, ...out.furniture];

  const podAt: Pt[] = [];
  for (let p = 0; p < pods; p++) {
    const c = p % cols;
    const rr = Math.floor(p / cols);
    podAt.push({ x: x0 + PX0 + c * PITCH_X + (rr % 2) * 0.6, z: BACK + PITCH_Z / 2 + rr * PITCH_Z + (JIT_Z[p % JIT_Z.length] ?? 0) });
  }

  // a pod's four desks back to back; the two at its back face the front, the two at its front face back
  const desks = new Map<string, Spot>();
  for (const name of names) {
    const s0 = slots.get(name) ?? 0;
    const p = Math.floor(s0 / 4);
    const k = s0 % 4;
    const pod = podAt[p] ?? { x: 0, z: BACK + PITCH_Z / 2 };
    const rr = Math.floor(p / cols);
    const x = pod.x + (k % 2 ? 0.8 : -0.8);
    const before = k < 2;
    const z = pod.z + (before ? -1.05 : 1.05);
    const wz = BACK + (before ? rr : rr + 1) * PITCH_Z;
    const s: Spot = { x, z, wz, ax: x, face: before ? 0 : Math.PI };
    desks.set(name, s);
    furniture.push(boxAt(x, z + (before ? 0.5 : -0.5), 1.6, 0.78));
    ways.push({ s });
  }

  // the counter along the left wall, its places on the aisle
  const len = Math.min(6, rowsP * PITCH_Z - 1.0);
  const island: Box = { x0: x0 + 0.2, x1: x0 + 1.1, z0: BACK + 0.55, z1: BACK + 0.55 + len };
  furniture.push(island);
  const islandSpots: Spot[] = [];
  for (let z = island.z0 + 0.6; z < island.z1 - 0.2; z += 0.85) islandSpots.push({ x: aisle, z, wz: z, ax: aisle, face: -Math.PI / 2 });

  // the corridors between the rows of pods, all from the aisle; the aisle from the back to the front
  const rightX = Math.max(aisle, ...[...desks.values()].map((d) => d.x));
  const rows: Leg[] = [];
  for (let r = 1; r <= rowsP; r++) rows.push([{ x: aisle, z: BACK + r * PITCH_Z }, { x: rightX, z: BACK + r * PITCH_Z }]);
  const backLeg = corridor(BACK, ways, [aisle, rightX, ...[...desks.values()].filter((d) => d.wz === BACK).map((d) => d.x)]);
  const trunk: Leg[] = [backLeg, ...out.trunk, ...rows, [{ x: aisle, z: BACK }, { x: aisle, z: front }]];
  const walks = walksOf(trunk, ways);

  const places = { desks, mainDesk: back.mainDesk, islandSpots, lounge: back.lounge, boardSpots: back.boardSpots, queue: back.queue, door: out.door, terrace: out.terrace, exit: out.exit };
  return {
    map: 'campus',
    size: `pods${String(pods)}`,
    W, D, x0, island, ...places,
    walks, furniture,
    queueMore: queueMoreOf(back.queue, back.mainDesk, backLeg, x0),
    slots, cap, crossBack: BACK,
    pods: podAt, aisle,
  };
}

/** where its look puts the loft's pieces past the common ones (render/maps/campus.ts loftGrower: the arcade, the nap place's middle): nothing cozy there */
const campusPieces = (o: Office): Pt[] => {
  // the arcade past the whiteboard (render/maps/blook.ts pastBoard): the board's middle + its half (0.9) + 0.25 + the cabinet's half
  const bx = o.boardSpots.length ? o.boardSpots.reduce((s, p) => s + p.x, 0) / o.boardSpots.length : o.x0 + o.W / 2;
  return [{ x: bx + 0.9 + 0.25 + 0.55 / 2, z: 0.3 }, { x: (o.lounge[0]?.x ?? o.x0) + 1.0, z: 1.5 }];
};

export const campus: Plan<CampusOffice> = { id: 'campus', ready: true, layout: campusLayout, things: LOFT_LIKE, cozy: { pieces: campusPieces }, room: ROOM };
