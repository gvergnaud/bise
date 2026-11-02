// Where his plugins' objects stand in the bims' office (bims-plugins plan p4, page bims-plugins v15,
// layout B "by corners"): each object goes to the corner its kind belongs to (CORNER: a play
// corner by the nap place, a reading corner along the left wall, a machine corner round main's
// desk, the kitchen round the eating place, the door, the terrace; the calendar hangs on a wall),
// and only there (a scooter or a bike may take the door or the terrace): a game never stands among
// the desks. Nothing stands alone mid-floor: each object touches a wall, its corner's seed (the
// nap place, main's desk, the island, the door, the terrace's edge) or another object of its
// corner (designer m_41180). In an assemblage (ORGANIC) things are a little turned and closer; tall
// things stand with their back to a wall (AGAINST); a cluster of 3 or more gets its rug. A free
// spot is off every piece of furniture, cozy piece, decor and inbox, a body's room off every walk
// and place, a bim's width behind every chair, and a bim's width off the terrace's own pieces: a
// walk never crosses an object, and the desks never move for one (the plan lays them without the
// objects). When the office changes (an agent comes, a new size), an object keeps its spot while
// it is still free, else slides to the nearest free one of its corner; one with no room takes its
// compact one (COMPACT), and only then waits in `left` (p5's growth makes room). A compact swap
// that is another of his objects is never taken: each object stands once (Gabriel m_42685).
// objectSpots is the one seam p4's per-map corners plug into: the signature stays.
// Pure, no three. Owner: bims-life.
import type { Box, Office, Prop, Pt } from './model';
import type { PluginObject } from './plugin-objects';
import type { ObjId } from './render/objects/ids';
import { FEET, type FootOf } from './render/objects/feet';
import { inboxBox } from './inbox';
import { backBusy, backFrames } from './cozy';
import { BODY_R } from './avoid';
import { network } from './paths';
import { wingLamp, wingWindow } from './wing';
import { decorOf, terraceSlab } from './render/decor';
import { BACK, RIGHT, UP, type V3 } from './render/camera';

export type Corner = 'play' | 'read' | 'machines' | 'kitchen' | 'door' | 'hang' | 'terrace' | 'garden';

/** the corner each object belongs to (bims-plugins' SENSE_B, signed by Gabriel on v15; every family has its default) */
export const CORNER: Readonly<Record<ObjId, Corner>> = {
  // games: the play corner, by the nap place; the big table outside
  chess: 'play', pingpong: 'terrace', arcade: 'play', pinball: 'play', babyfoot: 'play', billiards: 'play', go: 'play', jenga: 'play', claw: 'play', cards: 'play',
  // music: with the games
  piano: 'play', jukebox: 'play', turntable: 'play', guitar: 'play', drums: 'play', synth: 'play', gramophone: 'play', cello: 'play', mic: 'play', harp: 'play',
  // knowledge: the reading corner; the telescope under the sky
  shelf: 'read', library: 'read', globe: 'read', telescope: 'terrace', map: 'hang', lectern: 'read', blackboard: 'read', dinosaur: 'read', hourglass: 'read', orrery: 'read',
  // workshop: the easel reads, the printer prints by main's desk
  easel: 'read', printer: 'machines', printer3d: 'machines', workbench: 'machines', sewing: 'read', pottery: 'read', loom: 'read', press: 'machines', electronics: 'machines', robotarm: 'machines',
  // kitchen: round the eating place; the grill and the pizza oven outside
  espresso: 'kitchen', pizzaoven: 'terrace', grill: 'terrace', popcorn: 'kitchen', icecream: 'kitchen', drinks: 'kitchen', juicebar: 'kitchen', teaset: 'kitchen', candy: 'kitchen', fridge: 'kitchen',
  // garden: outside, but the plant stays in
  plant: 'kitchen', campfire: 'terrace', pond: 'terrace', beehive: 'terrace', veggies: 'terrace', swing: 'terrace', hammock: 'terrace', fountain: 'terrace', birdhouse: 'terrace', treehouse: 'terrace', glasshouse: 'terrace',
  // animals: the cat and its things by the nap place, the dog by main, the yard's outside
  cat: 'play', catwalk: 'play', cushion: 'play', dog: 'machines', aquarium: 'play', parrot: 'read', hens: 'terrace', tortoise: 'terrace', rabbit: 'terrace', hamster: 'read', goldfish: 'read', ducks: 'terrace',
  // comms: round main; the mailbox by the door, the birds outside
  mailbox: 'door', booth: 'machines', visio: 'machines', radio: 'machines', dovecote: 'terrace', rotary: 'machines', telegraph: 'machines', dish: 'terrace', pneumatic: 'machines', switchboard: 'machines',
  // machines: round main; the snacks by the kitchen, the old games with the games
  rack: 'machines', macdesk: 'machines', retro: 'play', crt: 'machines', snacks: 'kitchen', photobooth: 'play', robot: 'machines', drone: 'machines', clock: 'machines', dashboard: 'machines',
  // travel: outside; the scooter by the door
  scooter: 'door', bike: 'terrace', hoop: 'terrace', surf: 'terrace', kayak: 'terrace', balloon: 'terrace', rocket: 'terrace', car: 'terrace', yoga: 'terrace', trampoline: 'terrace',
  // office: the calendar hangs, the extinguisher by the door, the files and trophies read
  calendar: 'hang', files: 'read', trophies: 'read', extinguisher: 'door', charts: 'machines', packing: 'door',
};

/** the other corner a thing may take when its own is full: the travel things go by the door or outside (designer m_41180) */
export const ALSO: Partial<Record<ObjId, Corner>> = { scooter: 'terrace', bike: 'door', mailbox: 'terrace' };

/** tall things that stand with their back to a wall (the back one or the left one) */
const AGAINST: ReadonlySet<ObjId> = new Set<ObjId>([
  'library', 'shelf', 'piano', 'jukebox', 'files', 'trophies', 'aquarium', 'rack', 'booth', 'fridge', 'blackboard', 'workbench', 'snacks', 'clock',
  'dashboard', 'switchboard', 'charts', 'drinks', 'juicebar', 'icecream', 'espresso', 'claw', 'photobooth', 'harp', 'cello', 'guitar', 'synth', 'press', 'electronics',
]);

/** the corners an assemblage lives in: things a little turned (±TURN, seeded by the plugin), closer together */
const ORGANIC: ReadonlySet<Corner> = new Set<Corner>(['play', 'machines', 'kitchen', 'terrace', 'garden']);
/** the outdoor corners: the terrace, and his sketch's garden (extensions.ts) */
const OUT: ReadonlySet<Corner> = new Set<Corner>(['terrace', 'garden']);
/** the travel things that go out together to a garden, by its stepping stones' start (designer m_41450) */
const TRAVEL: ReadonlySet<ObjId> = new Set<ObjId>(['mailbox', 'scooter', 'bike']);
/** the garden's hedge along its front and its tree in its far corner (render/extensions.ts): what its things keep off */
const HEDGE_D = 0.8;
export const TURN = 0.3;

/** a big thing's compact one in an office with no room for it (the registry's COMPACT, three-free); the others scale by COMPACT_SCALE */
export const COMPACT: Partial<Record<ObjId, ObjId>> = { library: 'shelf', pingpong: 'babyfoot', piano: 'jukebox' };
export const COMPACT_SCALE = 0.8;

/** how far an object keeps from a walk's middle line: a body and a hair */
export const WALK_GAP = BODY_R + 0.1;
/** a bim's width: what stays free behind a chair and round the terrace's own pieces (designer m_41180) */
export const BIM_W = 2 * BODY_R + 0.03;
/** how far from furniture, cozy pieces and the decor indoors */
const AROUND = 0.2;
/** a poster's strip along its wall (cozy.ts boxOn's depth) */
const POSTER_STRIP = 0.42;
/** the parasol's canopy's and the palm's fronds' half reach */
const CANOPY = 0.9;
const EDGE = 0.12;
/** two objects apart: close when both stand in an assemblage, a step otherwise */
const GAP_ORGANIC = 0.3;
const GAP = 0.6;
/** outside, long low things (a bike, a scooter, a mailbox) read as one in the view at 0.3: half a step (designer m_41229) */
const GAP_OUT = 0.5;
/** how close counts as touching (a wall, a seed, a neighbour) */
export const TOUCH = 0.35;
/** the search's step */
const STEP = 0.25;
/** a wall thing's back off its wall */
const OFF_WALL = 0.08;

export interface Placed {
  readonly key: string;
  /** the names of the plugins it stands for, its own first (PluginObject's; its hover tip, objpills.ts objTipOf) */
  readonly names: readonly string[];
  /** what is drawn: the plugin's object, or its compact one */
  readonly obj: ObjId;
  /** the plugin's own object (its corner's: CORNER) */
  readonly of: ObjId;
  readonly n?: number;
  readonly glass: boolean;
  readonly pill: PluginObject['pill'];
  /** the plugin its pill is for when not its own (plugin-objects.ts uniqueObjects) */
  readonly pillOf?: string;
  /** the name of the plugin its pill is for (the pill's tip) */
  readonly pillName?: string;
  /** its middle on the floor (office units) and its turn (three's y rotation; 0: its front to +z) */
  readonly x: number;
  readonly z: number;
  readonly rot: number;
  /** it took its small one: a swap (obj is the swap's) or scaled by COMPACT_SCALE */
  readonly compact: boolean;
  /** scaled by COMPACT_SCALE (no swap for it): makeObject's compact for this obj */
  readonly scaled: boolean;
  /** its foot as drawn (compact: the swap's, or scaled) */
  readonly foot: FootOf;
  readonly corner: Corner;
  /** the floor it takes, turned (what walks and other objects keep off) */
  readonly box: Box;
}

/** a rug under a play or reading corner's cluster of 3 or more */
export interface Rug {
  readonly box: Box;
  readonly corner: 'play' | 'read';
}

export interface Corners {
  readonly placed: readonly Placed[];
  /** what found no room in its corner, even compact (p5: the office grows for them); in its order */
  readonly left: readonly PluginObject[];
  readonly rugs: readonly Rug[];
}

export const NO_CORNERS: Corners = { placed: [], left: [], rugs: [] };

/** a string's small hash in [0, 1) (FNV-1a): the plugin's turn, the same on every run */
export function seedOf(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return (h >>> 0) / 4294967296;
}

const overlaps = (a: Box, b: Box, m: number): boolean => a.x0 < b.x1 + m && a.x1 > b.x0 - m && a.z0 < b.z1 + m && a.z1 > b.z0 - m;
const grow = (b: Box, m: number): Box => ({ x0: b.x0 - m, x1: b.x1 + m, z0: b.z0 - m, z1: b.z1 + m });

/** a w x d foot turned by rot round (x, z): its floor box */
export function boxOf(x: number, z: number, w: number, d: number, rot: number): Box {
  const c = Math.abs(Math.cos(rot));
  const s = Math.abs(Math.sin(rot));
  const hw = (w * c + d * s) / 2;
  const hd = (w * s + d * c) / 2;
  return { x0: x - hw, x1: x + hw, z0: z - hd, z1: z + hd };
}

/** how small a thing is drawn: 0 itself, 1 its compact one (its swap, or scaled), 2 its swap scaled (the ping-pong's small babyfoot) */
type Size = 0 | 1 | 2;
const scaledFoot = (f: FootOf): FootOf => {
  const k = COMPACT_SCALE;
  return { w: f.w * k, d: f.d * k, h: f.h * k, ...(f.wall ? { wall: true as const } : {}) };
};
const footOf = (obj: ObjId, size: Size): { obj: ObjId; foot: FootOf } => {
  const swap = size > 0 ? COMPACT[obj] : undefined;
  if (swap !== undefined) return { obj: swap, foot: size === 2 ? scaledFoot(FEET[swap]) : FEET[swap] };
  return { obj, foot: size === 0 ? FEET[obj] : scaledFoot(FEET[obj]) };
};
/** a placed thing's size, from what it drew */
const sizeOf = (p: Placed): Size => (!p.compact ? 0 : p.scaled && p.obj !== p.of ? 2 : 1);

const mid = (ps: readonly Pt[], or: Pt): Pt => (ps.length === 0 ? or : { x: ps.reduce((s, p) => s + p.x, 0) / ps.length, z: ps.reduce((s, p) => s + p.z, 0) / ps.length });
const hull = (bs: readonly Box[]): Box => ({ x0: Math.min(...bs.map((b) => b.x0)), x1: Math.max(...bs.map((b) => b.x1)), z0: Math.min(...bs.map((b) => b.z0)), z1: Math.max(...bs.map((b) => b.z1)) });

/** the kept floor's cell: what the office keeps is laid on a grid once per office, a box's question is then a sum */
const CELL = 0.125;

/**
 * What the office keeps for itself, as a grid of kept cells over the room and the terrace, summed
 * (a box's kept cells in four reads): furniture, cozy pieces, the decor, the inbox, a park, a gym
 * (AROUND off; a bim's width off the terrace's pieces), a body's room round every walk and every
 * place a bim stands, and a bim's width behind every chair.
 */
interface Keep {
  readonly boxes: readonly Box[];
  readonly gx: number;
  readonly gz: number;
  readonly nx: number;
  readonly nz: number;
  /** (nx + 1) x (nz + 1) running sums of kept cells */
  readonly sum: Int32Array;
  /** the map's pieces high up (Plan.props with y0): a thing as tall as that may not stand under one */
  readonly high: readonly (Prop & { readonly y0: number })[];
}

function keepOf(o: Office): Keep {
  const boxes: Box[] = [...o.furniture];
  const c = o.cozy;
  if (c !== undefined) {
    for (const it of [c.coffee, c.shelf, ...c.lamps, ...c.plants]) boxes.push(it.box);
    // a poster's strip along its wall: nothing stands in front of it
    for (const h of c.posters) {
      if (h.wall === 'back') boxes.push({ x0: h.x - h.w / 2, x1: h.x + h.w / 2, z0: 0, z1: POSTER_STRIP });
      if (h.wall === 'left') boxes.push({ x0: o.x0, x1: o.x0 + POSTER_STRIP, z0: h.z - h.w / 2, z1: h.z + h.w / 2 });
    }
  }
  if (o.inbox !== undefined) boxes.push(inboxBox(o.inbox));
  // the map's own drawn pieces (Plan.props: a lamppost, a street tree, a bench): never under an object; one high up (a peg, a hanging pot) only over a lower one (free)
  boxes.push(...(o.props ?? []).filter((p) => p.y0 === undefined));
  if (o.park !== undefined) boxes.push(o.park.area);
  if (o.gym !== undefined) boxes.push(o.gym.area, ...o.gym.mats);
  for (const g of o.ghosts ?? []) boxes.push({ x0: g.x - 0.8, x1: g.x + 0.8, z0: g.z - 1.0, z1: g.z + 0.4 });
  // the decor every look draws: the plants, the door mat, and on the terrace the ramp, the parasol's canopy, the palm's fronds
  const dec = decorOf(o);
  for (const f of [...dec.plants, dec.doorMat, dec.ramp]) boxes.push({ x0: f.x - f.hw, x1: f.x + f.hw, z0: f.z - f.hd, z1: f.z + f.hd });
  for (const f of [dec.parasol, dec.palm]) boxes.push({ x0: f.x - CANOPY, x1: f.x + CANOPY, z0: f.z - CANOPY, z1: f.z + CANOPY });
  // his sketch's wing: its lamp and the floor under its window (wing.ts)
  if (o.wing !== undefined) boxes.push(wingLamp(o.wing, o.wing.x1 <= o.x0 + 1e-9), wingWindow(o.wing));
  // his sketch's garden: its tree (render/extensions.ts), its crown's reach
  if (o.garden !== undefined) boxes.push({ x0: o.garden.x0 + 0.7 - CANOPY, x1: o.garden.x0 + 0.7 + CANOPY, z0: o.garden.z1 - 1.1 - CANOPY, z1: o.garden.z1 - 1.1 + CANOPY });
  const spots: Pt[] = [o.door, o.mainDesk, ...o.desks.values(), ...o.islandSpots, ...o.lounge, ...o.boardSpots, ...o.queue, ...o.terrace, ...(o.nooks ?? []), ...(c?.coffeeSpots ?? [])];
  const t = terraceSlab(o);
  const gx = Math.min(o.x0, t.x0, o.garden?.x0 ?? t.x0, o.wing?.x0 ?? t.x0) - CELL;
  const gz = Math.min(0, t.z0) - CELL;
  const nx = Math.ceil((Math.max(o.x0 + o.W, t.x1, o.wing?.x1 ?? t.x1, o.garden?.x1 ?? t.x1) + CELL - gx) / CELL);
  const nz = Math.ceil((Math.max(o.D, t.z1, o.garden?.z1 ?? t.z1) + CELL - gz) / CELL);
  const kept = new Uint8Array(nx * nz);
  const mark = (x0: number, x1: number, z0: number, z1: number, keepIf?: (cx: number, cz: number) => boolean): void => {
    const i0 = Math.max(0, Math.floor((x0 - gx) / CELL));
    const i1 = Math.min(nx - 1, Math.floor((x1 - gx) / CELL));
    const j0 = Math.max(0, Math.floor((z0 - gz) / CELL));
    const j1 = Math.min(nz - 1, Math.floor((z1 - gz) / CELL));
    for (let i = i0; i <= i1; i++) for (let j = j0; j <= j1; j++) if (keepIf === undefined || keepIf(gx + (i + 0.5) * CELL, gz + (j + 0.5) * CELL)) kept[i * nz + j] = 1;
  };
  // a cell is kept when any of its square comes within reach: its middle within reach + half its diagonal
  const half = CELL * Math.SQRT1_2;
  // a grown office: what is no floor in the room-and-wing's or the terrace-and-garden's bounds (the corner past the wing, beside the garden)
  if (o.wing !== undefined || o.garden !== undefined) {
    const floors = floorsOf(o);
    const h = CELL / 2;
    const onFloor = (x: number, z: number): boolean => floors.some((f) => x >= f.x0 + h && x <= f.x1 - h && z >= f.z0 + h && z <= f.z1 - h);
    mark(gx, gx + nx * CELL, gz, gz + nz * CELL, (x, z) => !onFloor(x, z) && !(z > o.D && z < t.z0));
  }
  for (const b of boxes) {
    // outside the room (the terrace's loungers, the parasol, the ramp): a bim's width round them
    const m = b.z0 >= o.D - 1e-6 ? BIM_W : AROUND;
    mark(b.x0 - m, b.x1 + m, b.z0 - m, b.z1 + m);
  }
  const r = WALK_GAP + half;
  for (const s of spots) mark(s.x - r, s.x + r, s.z - r, s.z + r, (x, z) => Math.hypot(x - s.x, z - s.z) < r);
  // a bim's width behind each chair: the desk is in front of its spot (its face), the chair's back the other way
  const rc = BIM_W / 2 + half;
  for (const s of [o.mainDesk, ...o.desks.values()]) {
    const bx = s.x - Math.sin(s.face) * (BODY_R + BIM_W / 2);
    const bz = s.z - Math.cos(s.face) * (BODY_R + BIM_W / 2);
    mark(bx - rc, bx + rc, bz - rc, bz + rc, (x, z) => Math.hypot(x - bx, z - bz) < rc);
  }
  for (const [a, b] of network(o)) {
    mark(Math.min(a.x, b.x) - r, Math.max(a.x, b.x) + r, Math.min(a.z, b.z) - r, Math.max(a.z, b.z) + r, (x, z) => segGap({ x, z }, a, b) < r);
  }
  const sum = new Int32Array((nx + 1) * (nz + 1));
  for (let i = 0; i < nx; i++)
    for (let j = 0; j < nz; j++) sum[(i + 1) * (nz + 1) + j + 1] = (kept[i * nz + j] as number) + (sum[i * (nz + 1) + j + 1] as number) + (sum[(i + 1) * (nz + 1) + j] as number) - (sum[i * (nz + 1) + j] as number);
  return { boxes, gx, gz, nx, nz, sum, high: (o.props ?? []).filter((p): p is Prop & { y0: number } => p.y0 !== undefined) };
}

/** a point's distance to a leg */
function segGap(p: Pt, a: Pt, b: Pt): number {
  const dx = b.x - a.x;
  const dz = b.z - a.z;
  const l = dx * dx + dz * dz;
  const t = l === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.z - a.z) * dz) / l));
  return Math.hypot(p.x - (a.x + t * dx), p.z - (a.z + t * dz));
}

/** how many kept cells a box touches */
function keptIn(k: Keep, b: Box): number {
  const i0 = Math.max(0, Math.floor((b.x0 - k.gx) / CELL));
  const i1 = Math.min(k.nx, Math.ceil((b.x1 - k.gx) / CELL));
  const j0 = Math.max(0, Math.floor((b.z0 - k.gz) / CELL));
  const j1 = Math.min(k.nz, Math.ceil((b.z1 - k.gz) / CELL));
  if (i1 <= i0 || j1 <= j0) return 0;
  const s = k.sum;
  const w = k.nz + 1;
  return (s[i1 * w + j1] as number) - (s[i0 * w + j1] as number) - (s[i1 * w + j0] as number) + (s[i0 * w + j0] as number);
}

/** the floor an object may stand on in a corner: the room's, or the terrace's slab */
function areaOf(o: Office, corner: Corner): Box {
  // a grown office's (extensions.ts): the wing widens the room, the garden the terrace; what is no floor between them is kept (keepOf)
  if (corner === 'terrace') return terraceSlab(o);
  if (corner === 'garden') {
    const g = o.garden ?? { x0: 0, x1: 0, z0: 0, z1: 0 };
    return { x0: g.x0 + EDGE, x1: g.x1 - EDGE, z0: g.z0 + EDGE, z1: g.z1 - HEDGE_D };
  }
  return { x0: Math.min(o.x0, o.wing?.x0 ?? o.x0) + EDGE, x1: Math.max(o.x0 + o.W, o.wing?.x1 ?? o.x0) - EDGE, z0: EDGE, z1: o.D - EDGE };
}

/** the floors a grown office stands things on: the room and its wing, the terrace and its garden */
function floorsOf(o: Office): Box[] {
  return [{ x0: o.x0, x1: o.x0 + o.W, z0: 0, z1: o.D }, terraceSlab(o), ...(o.wing ? [o.wing] : []), ...(o.garden ? [o.garden] : [])];
}

/** where a corner gathers (its things are searched nearest first) */
function anchorOf(o: Office, corner: Corner): Pt {
  const isl = { x: (o.island.x0 + o.island.x1) / 2, z: (o.island.z0 + o.island.z1) / 2 };
  switch (corner) {
    case 'play':
      return mid(o.lounge, isl);
    case 'read':
      // the left wall's front half: its back half is by the nap place, the play corner's
      return { x: o.x0, z: o.D * 0.65 };
    case 'machines':
      return o.mainDesk;
    case 'kitchen':
      return isl;
    case 'door':
      return { x: o.door.x, z: o.D };
    case 'hang':
      return { x: o.mainDesk.x, z: 0 };
    case 'terrace': {
      const t = terraceSlab(o);
      return { x: t.x1, z: (t.z0 + t.z1) / 2 };
    }
    case 'garden':
      // the stepping stones' start, where the path from the door arrives
      return { x: o.door.x, z: (o.garden?.z0 ?? o.D) + 0.6 };
  }
}

/** what a corner's first thing may stand against besides a wall: the nap place, main's desk, the island, the door */
function seedsOf(o: Office, corner: Corner): Box[] {
  const near = (p: Pt, r: number): Box => ({ x0: p.x - r, x1: p.x + r, z0: p.z - r, z1: p.z + r });
  switch (corner) {
    case 'play': {
      const beds = o.furniture.filter((f) => f.of !== undefined && o.lounge.some((l) => l.x === f.of?.x && l.z === f.of.z));
      return [...beds, ...(o.cozy?.rug ? [o.cozy.rug] : [])];
    }
    case 'machines':
      return [near(o.mainDesk, 1.4)];
    case 'kitchen':
      return [o.island];
    case 'door':
      return [near({ x: o.door.x, z: o.D }, 1.6)];
    case 'garden':
      return o.garden === undefined ? [] : [near({ x: o.door.x, z: o.garden.z0 + 0.6 }, 1.6)];
    default:
      return [];
  }
}

/** a wall's spans along it where nothing tall stands (the board, main's neon, the mural, the frames, a poster); x for the back wall, z for the left one */
function wallSpans(o: Office): { back: [number, number][]; left: [number, number][] } {
  const back: [number, number][] = [...backBusy(o), ...backFrames(o)];
  const left: [number, number][] = [];
  for (const h of o.cozy?.posters ?? []) {
    if (h.wall === 'back') back.push([h.x - h.w / 2, h.x + h.w / 2]);
    if (h.wall === 'left') left.push([h.z - h.w / 2, h.z + h.w / 2]);
  }
  return { back, left };
}

const crosses = (spans: readonly (readonly [number, number])[], a: number, b: number): boolean => spans.some(([s, e]) => a < e && b > s);

/** one call's searches: the kept floor, each corner's floor, seeds and points nearest its anchor (laid once) */
interface Ctx {
  readonly o: Office;
  readonly keep: Keep;
  readonly spans: { back: [number, number][]; left: [number, number][] };
  readonly grids: Map<Corner, readonly Pt[]>;
  readonly zoned: Map<Corner, readonly Pt[]>;
  readonly seeds: Map<Corner, readonly Box[]>;
  /** the objects his plugins bring: a compact swap that is one of them is never taken (each object once, Gabriel m_42685) */
  readonly have: ReadonlySet<ObjId>;
}

/** a size of `p`'s that draws another object already in the office: never taken */
const swapTaken = (c: Ctx, p: PluginObject, obj: ObjId): boolean => obj !== p.obj && c.have.has(obj);

function gridOf(c: Ctx, corner: Corner): readonly Pt[] {
  const had = c.grids.get(corner);
  if (had !== undefined) return had;
  const area = areaOf(c.o, corner);
  const a = anchorOf(c.o, corner);
  const ps: { p: Pt; r: number }[] = [];
  for (let x = area.x0; x <= area.x1 + 1e-9; x += STEP) for (let z = area.z0; z <= area.z1 + 1e-9; z += STEP) ps.push({ p: { x, z }, r: Math.hypot(x - a.x, z - a.z) });
  ps.sort((p, q) => p.r - q.r || p.p.x - q.p.x || p.p.z - q.p.z);
  const out = ps.map((p) => p.p);
  c.grids.set(corner, out);
  return out;
}

/** the biggest half size a thing takes on the floor (the ping-pong table turned), for the zone's prefilter */
const HALF_MAX = 1.6;

/** a corner's floor points that may hold a thing in its place (inZone for the biggest one): the others are never tried (laid once) */
function zoneGridOf(c: Ctx, corner: Corner): readonly Pt[] {
  const had = c.zoned.get(corner);
  if (had !== undefined) return had;
  const area = areaOf(c.o, corner);
  const seeds = seedOfCorner(c, corner);
  const out = gridOf(c, corner).filter((p) => {
    if (corner === 'read') return p.x <= area.x0 + ZONE + HALF_MAX;
    const b = { x0: p.x - HALF_MAX, x1: p.x + HALF_MAX, z0: p.z - HALF_MAX, z1: p.z + HALF_MAX };
    return inZone(area, b, corner, seeds, [], c.o.wing);
  });
  c.zoned.set(corner, out);
  return out;
}

function seedOfCorner(c: Ctx, corner: Corner): readonly Box[] {
  const had = c.seeds.get(corner);
  if (had !== undefined) return had;
  const s = seedsOf(c.o, corner);
  c.seeds.set(corner, s);
  return s;
}

interface Try {
  readonly x: number;
  readonly z: number;
  readonly rot: number;
}

/** a wall thing's spots in a corner, nearest its anchor first: along the back wall, then the left one (turned to face the room), off their busy spans; a hung one flat on the wall */
function wallTries(c: Ctx, corner: Corner, foot: FootOf): readonly Try[] {
  const area = areaOf(c.o, corner);
  const a = anchorOf(c.o, corner);
  const off = corner === 'hang' ? 0 : OFF_WALL;
  const out: Try[] = [];
  const z = area.z0 + foot.d / 2 + off;
  for (let x = area.x0 + foot.w / 2; x <= area.x1 - foot.w / 2 + 1e-9; x += STEP) {
    if (!crosses(c.spans.back, x - foot.w / 2, x + foot.w / 2)) out.push({ x, z, rot: 0 });
  }
  const x = area.x0 + foot.d / 2 + off;
  for (let z2 = area.z0 + foot.w / 2; z2 <= area.z1 - foot.w / 2 + 1e-9; z2 += STEP) {
    if (!crosses(c.spans.left, z2 - foot.w / 2, z2 + foot.w / 2)) out.push({ x, z: z2, rot: Math.PI / 2 });
  }
  const r = (t: Try): number => Math.hypot(t.x - a.x, t.z - a.z);
  return out.sort((p, q) => r(p) - r(q) || p.x - q.x || p.z - q.z);
}

/** two objects apart: close when both stand in an assemblage, a step when either does not (the same both ways, so a kept spot stays free) */
const gapOf = (a: Corner, b: Corner): number => (OUT.has(a) && OUT.has(b) ? GAP_OUT : ORGANIC.has(a) && ORGANIC.has(b) ? GAP_ORGANIC : GAP);

/** whether a box is free in a corner: on its area, off what the office keeps, apart from the others, never under a canopy in the view */
function free(c: Ctx, b: Box, corner: Corner, area: Box, taken: readonly Placed[], h = 0): boolean {
  if (b.x0 < area.x0 || b.x1 > area.x1 || b.z0 < area.z0 || b.z1 > area.z1) return false;
  if (keptIn(c.keep, b) > 0) return false;
  if (c.keep.high.some((p) => h > p.y0 && overlaps(b, p, 0))) return false;
  for (const p of taken) if (overlaps(b, p.box, gapOf(corner, p.corner))) return false;
  return corner !== 'terrace' || !underCanopy(c.o, b, h);
}

/** a thing high up over the floor: x, z its floor box, y its heights */
interface Solid {
  readonly x0: number;
  readonly x1: number;
  readonly y0: number;
  readonly y1: number;
  readonly z0: number;
  readonly z1: number;
}

/** the terrace's high pieces that draw over what stands behind them in the office's view: the parasol's canopy, the palm's fronds (stages.ts) */
function canopiesOf(o: Office): Solid[] {
  const d = decorOf(o);
  return [
    { x0: d.parasol.x - 0.7, x1: d.parasol.x + 0.7, y0: 1.7, y1: 2.0, z0: d.parasol.z - 0.7, z1: d.parasol.z + 0.7 },
    { x0: d.palm.x - 0.45, x1: d.palm.x + 0.45, y0: 1.55, y1: 1.7, z0: d.palm.z - 0.45, z1: d.palm.z + 0.45 },
  ];
}

const dot3 = (x: number, y: number, z: number, v: V3): number => x * v.x + y * v.y + z * v.z;

/** a solid's rectangle on the screen (the office's isometric view: camera.ts RIGHT, UP) and how near the camera its middle is (BACK) */
function screenOf(s: Solid): { sx0: number; sx1: number; sy0: number; sy1: number; near: number } {
  let sx0 = Infinity;
  let sx1 = -Infinity;
  let sy0 = Infinity;
  let sy1 = -Infinity;
  for (const x of [s.x0, s.x1])
    for (const y of [s.y0, s.y1])
      for (const z of [s.z0, s.z1]) {
        const sx = dot3(x, y, z, RIGHT);
        const sy = dot3(x, y, z, UP);
        sx0 = Math.min(sx0, sx);
        sx1 = Math.max(sx1, sx);
        sy0 = Math.min(sy0, sy);
        sy1 = Math.max(sy1, sy);
      }
  return { sx0, sx1, sy0, sy1, near: dot3((s.x0 + s.x1) / 2, (s.y0 + s.y1) / 2, (s.z0 + s.z1) / 2, BACK) };
}

/** how far apart on the screen a canopy and what stands behind it stay (office units) */
const SCREEN_GAP = 0.1;

/** whether a canopy, nearer the camera, draws over a thing of height h standing on b (designer m_41229: test it in the camera's projection) */
export function underCanopy(o: Office, b: Box, h: number): boolean {
  const t = screenOf({ x0: b.x0, x1: b.x1, y0: 0, y1: Math.max(h, 0.1), z0: b.z0, z1: b.z1 });
  return canopiesOf(o).some((s) => {
    const c = screenOf(s);
    return c.near > t.near && c.sx0 < t.sx1 + SCREEN_GAP && c.sx1 > t.sx0 - SCREEN_GAP && c.sy0 < t.sy1 + SCREEN_GAP && c.sy1 > t.sy0 - SCREEN_GAP;
  });
}

/**
 * Whether a box in a corner touches what its corner gathers round (designer m_41180: nothing alone
 * mid-floor, nothing in another corner's place): the reading corner its left wall, the hung things
 * a wall, the terrace its slab's edge, the others their seed (the nap place, main's desk, the
 * island, the door); and any of them another thing of its corner.
 */
export function touches(o: Office, b: Box, corner: Corner, taken: readonly Placed[], seeds: readonly Box[], play: readonly Box[] = []): boolean {
  const area = areaOf(o, corner);
  if (!inZone(area, b, corner, seeds, play, o.wing)) return false;
  // the door's things stand against its wall, on the way-out side, each of them (designer m_41307): never on a rug or a pod's place inside
  if (corner === 'door') return b.z1 >= area.z1 - TOUCH && seeds.some((s) => overlaps(b, s, TOUCH));
  if (OUT.has(corner) && (b.x0 <= area.x0 + TOUCH || b.x1 >= area.x1 - TOUCH || b.z0 <= area.z0 + TOUCH || b.z1 >= area.z1 - TOUCH)) return true;
  if (!OUT.has(corner) && (b.z0 <= area.z0 + TOUCH || b.x0 <= area.x0 + TOUCH)) return true;
  if (seeds.some((s) => overlaps(b, s, TOUCH))) return true;
  return taken.some((p) => p.corner === corner && overlaps(b, p.box, gapOf(corner, p.corner) + TOUCH));
}

/** how far a corner reaches from its seed (the nap place, main's desk, the island, the door), and the reading corner from its wall */
export const ZONE = 2.5;
export const PLAY_ZONE = 1.5;

/**
 * Whether a box stands in its corner's place: near its seed (or its wall: the reading corner, the
 * hung things; the terrace anywhere on its slab). The reading corner's wall stops where the play
 * corner's place starts (`play`: the nap place's seeds): the games come first by the nap place.
 */
function inZone(area: Box, b: Box, corner: Corner, seeds: readonly Box[], play: readonly Box[] = [], wing?: Box): boolean {
  if (OUT.has(corner)) return true;
  // his sketch's wing (extensions.ts) is the office going on: any indoor corner may gather there too (not the door's, not a hung thing)
  if (wing !== undefined && corner !== 'door' && corner !== 'hang' && b.x0 >= wing.x0 - 1e-9 && b.x1 <= wing.x1 + 1e-9) return true;
  if (corner === 'read') return b.x0 <= area.x0 + ZONE && !play.some((s) => overlaps(b, s, ZONE));
  if (corner === 'hang') return b.z0 <= area.z0 + TOUCH || b.x0 <= area.x0 + TOUCH;
  // the games stay right by the nap place, never halfway to main's desk (designer m_41180)
  const reach = corner === 'play' ? PLAY_ZONE : ZONE;
  return seeds.some((s) => overlaps(b, s, reach));
}

/**
 * the order objects take their spots: with a garden, the door's trio first (it goes out together:
 * an outdoor piece waits, never the mailbox, designer m_41494); then hung and wall things (they need
 * a wall), then the biggest; the key breaks ties
 */
function orderOf(objs: readonly PluginObject[], garden: boolean): PluginObject[] {
  const rank = (p: PluginObject): number => (garden && TRAVEL.has(p.obj) ? -1 : CORNER[p.obj] === 'hang' ? 0 : AGAINST.has(p.obj) ? 1 : 2);
  const area = (p: PluginObject): number => FEET[p.obj].w * FEET[p.obj].d;
  return [...objs].sort((a, b) => rank(a) - rank(b) || area(b) - area(a) || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
}

/**
 * Every plugin object's spot in this office. `prev`: the spots drawn before (another size, an
 * agent more): each keeps its own while it is still free (those go first, so no newcomer takes
 * one). The same office, objects and prev give the same spots. Pure.
 */
export function objectSpots(o: Office, objs: readonly PluginObject[], prev: readonly Placed[] = []): Corners {
  if (objs.length === 0) return NO_CORNERS;
  const c: Ctx = { o, keep: keepOf(o), spans: wallSpans(o), grids: new Map(), zoned: new Map(), seeds: new Map(), have: new Set(objs.map((p) => p.obj)) };
  const was = new Map(prev.map((p) => [p.key, p]));
  const placed: Placed[] = [];
  const left: PluginObject[] = [];
  const todo: PluginObject[] = [];
  // the ones still free where they were, first
  const garden = o.garden !== undefined;
  const order = orderOf(objs, garden);
  for (const p of order) {
    const b = was.get(p.key);
    // a garden come: the door's trio goes out to it, from wherever it stood
    const out = garden && TRAVEL.has(p.obj) && b !== undefined && b.corner !== 'garden';
    const at = b === undefined || out ? null : kept(c, p, b, placed);
    if (at === null) todo.push(p);
    else placed.push(at);
  }
  const missed: PluginObject[] = [];
  for (const p of todo) {
    const at = place(c, p, placed);
    if (at === null) missed.push(p);
    else placed.push(at);
  }
  // one more try for what found no room, once the others stand: a swap in another corner (the
  // ping-pong's babyfoot) has their side to stand by now
  for (const p of missed) {
    const at = place(c, p, placed);
    if (at === null) left.push(p);
    else placed.push(at);
  }
  return { placed, left, rugs: rugsOf(o, placed, c.keep) };
}

function placedOf(p: PluginObject, obj: ObjId, size: Size, foot: FootOf, corner: Corner, t: Try): Placed {
  return {
    key: p.key, names: p.names, obj, of: p.obj, ...(p.n === undefined ? {} : { n: p.n }), glass: p.glass, pill: p.pill, ...(p.pillOf === undefined ? {} : { pillOf: p.pillOf }), ...(p.pillName === undefined ? {} : { pillName: p.pillName }),
    x: t.x, z: t.z, rot: t.rot, compact: size > 0, scaled: size === 2 || (size === 1 && COMPACT[p.obj] === undefined), foot, corner, box: boxOf(t.x, t.z, foot.w, foot.d, t.rot),
  };
}

/** its spot of before, while it is still free and the same object; null: it moves */
function kept(c: Ctx, p: PluginObject, before: Placed, taken: readonly Placed[]): Placed | null {
  const { obj, foot } = footOf(p.obj, sizeOf(before));
  if (before.obj !== obj || before.of !== p.obj || swapTaken(c, p, obj)) return null;
  return free(c, before.box, before.corner, areaOf(c.o, before.corner), taken, foot.h) ? placedOf(p, obj, sizeOf(before), foot, before.corner, before) : null;
}

function place(c: Ctx, p: PluginObject, taken: readonly Placed[]): Placed | null {
  const home = CORNER[p.obj];
  const also = ALSO[p.obj];
  const own0: readonly Corner[] = also === undefined ? [home] : [home, also];
  // his sketch's garden (extensions.ts): the terrace's things that find no room there go out to it; the travel things go together, by its path's start
  const corners: readonly Corner[] = c.o.garden === undefined ? own0 : TRAVEL.has(p.obj) ? ['garden', ...own0] : own0.includes('terrace') ? [...own0, 'garden'] : own0;
  // a tall thing tries a wall first, its compact one at a wall next, then standing free (touching its corner's others)
  const walled = AGAINST.has(p.obj);
  // last, a swap scaled down (the ping-pong's small babyfoot: the one a babyfoot plugin would take)
  const swapped = COMPACT[p.obj] === undefined ? [] : [[2, false] as const];
  const modes: readonly (readonly [Size, boolean])[] = walled ? [[0, true], [1, true], [0, false], [1, false], ...swapped] : [[0, false], [1, false], ...swapped];
  for (const [size, against] of modes) {
    if (size === 1 && COMPACT[p.obj] === undefined && !(FEET[p.obj].w * FEET[p.obj].d > 1)) continue;
    const { obj, foot } = footOf(p.obj, size);
    if (swapTaken(c, p, obj)) continue;
    // a swap may stand in its own corner too (a yard's terrace with no room for the ping-pong: its babyfoot by the nap place)
    const own = size > 0 && obj !== p.obj ? CORNER[obj] : undefined;
    for (const corner of own === undefined || corners.includes(own) ? corners : [...corners, own]) {
      const area = areaOf(c.o, corner);
      const seeds = seedOfCorner(c, corner);
      const play = seedOfCorner(c, 'play');
      if ((against && !OUT.has(corner)) || corner === 'hang') {
        for (const t of wallTries(c, corner, foot)) {
          const b = boxOf(t.x, t.z, foot.w, foot.d, t.rot);
          if (touches(c.o, b, corner, taken, seeds, play) && free(c, b, corner, area, taken, foot.h)) return placedOf(p, obj, size, foot, corner, t);
        }
        continue;
      }
      // the floor's points nearest the corner's anchor: one turned box's half sizes for all of them
      const turn = ORGANIC.has(corner) ? (seedOf(p.key) * 2 - 1) * TURN : 0;
      const cs = Math.abs(Math.cos(turn));
      const sn = Math.abs(Math.sin(turn));
      const hw = (foot.w * cs + foot.d * sn) / 2;
      const hd = (foot.w * sn + foot.d * cs) / 2;
      for (const q of zoneGridOf(c, corner)) {
        const b = { x0: q.x - hw, x1: q.x + hw, z0: q.z - hd, z1: q.z + hd };
        if (!inZone(area, b, corner, seeds, play, c.o.wing)) continue;
        if (b.x0 < area.x0 || b.x1 > area.x1 || b.z0 < area.z0 || b.z1 > area.z1 || keptIn(c.keep, b) > 0) continue;
        if (!touches(c.o, b, corner, taken, seeds, play)) continue;
        if (free(c, b, corner, area, taken, foot.h)) return placedOf(p, obj, size, foot, corner, { x: q.x, z: q.z, rot: turn });
      }
    }
  }
  return null;
}

/** the rug's margin round its cluster (a smaller one when the big one meets furniture), and how close two things are to be one cluster */
const RUG_PAD = 0.3;
const RUG_PAD_MIN = 0.12;
const CLUSTER = 1.0;
const RUG_MIN = 3;

/** a rug under each play or reading corner's biggest cluster of RUG_MIN or more, off the furniture, the nap rug and the walls */
function rugsOf(o: Office, placed: readonly Placed[], keep: Keep): Rug[] {
  const out: Rug[] = [];
  const room = { x0: o.x0 + EDGE, x1: o.x0 + o.W - EDGE, z0: EDGE, z1: o.D - EDGE };
  for (const corner of ['play', 'read'] as const) {
    const ps = placed.filter((p) => p.corner === corner && !p.foot.wall);
    // its clusters: things within CLUSTER of each other, chained
    const seen = new Set<number>();
    let best: Placed[] = [];
    for (let i = 0; i < ps.length; i++) {
      if (seen.has(i)) continue;
      const group: number[] = [i];
      seen.add(i);
      for (let k = 0; k < group.length; k++) {
        const a = ps[group[k] as number] as Placed;
        ps.forEach((b, j) => {
          if (!seen.has(j) && overlaps(a.box, b.box, CLUSTER)) {
            seen.add(j);
            group.push(j);
          }
        });
      }
      if (group.length > best.length) best = group.map((j) => ps[j] as Placed);
    }
    if (best.length < RUG_MIN) continue;
    const h = hull(best.map((p) => p.box));
    for (const pad of [RUG_PAD, RUG_PAD_MIN]) {
      const g = grow(h, pad);
      const box = { x0: Math.max(g.x0, room.x0), x1: Math.min(g.x1, room.x1), z0: Math.max(g.z0, room.z0), z1: Math.min(g.z1, room.z1) };
      if (keep.boxes.some((f) => overlaps(box, f, 0))) continue;
      out.push({ box, corner });
      break;
    }
  }
  // his sketch's wing (extensions.ts): its biggest cluster gets a rug too, so it reads as more office (designer m_41450)
  const w = o.wing;
  if (w !== undefined) {
    const ps = placed.filter((p) => p.box.x0 >= w.x0 - 1e-9 && !p.foot.wall && p.corner !== 'hang');
    const best = biggestCluster(ps);
    if (best.length >= WING_RUG_MIN) {
      const reads = best.filter((p) => p.corner === 'read').length;
      const area = { x0: w.x0 + EDGE, x1: w.x1 - EDGE, z0: EDGE, z1: w.z1 - EDGE };
      for (const pad of [RUG_PAD, RUG_PAD_MIN]) {
        const g = grow(hull(best.map((p) => p.box)), pad);
        const box = { x0: Math.max(g.x0, area.x0), x1: Math.min(g.x1, area.x1), z0: Math.max(g.z0, area.z0), z1: Math.min(g.z1, area.z1) };
        if (keep.boxes.some((f) => overlaps(box, f, 0)) || out.some((r) => overlaps(box, r.box, 0))) continue;
        out.push({ box, corner: reads * 2 > best.length ? 'read' : 'play' });
        break;
      }
    }
  }
  return out;
}

/** a wing cluster's least size for a rug */
const WING_RUG_MIN = 2;

/** the biggest group of things within CLUSTER of each other, chained */
function biggestCluster(ps: readonly Placed[]): Placed[] {
  const seen = new Set<number>();
  let best: Placed[] = [];
  for (let i = 0; i < ps.length; i++) {
    if (seen.has(i)) continue;
    const group: number[] = [i];
    seen.add(i);
    for (let k = 0; k < group.length; k++) {
      const a = ps[group[k] as number] as Placed;
      ps.forEach((b, j) => {
        if (!seen.has(j) && overlaps(a.box, b.box, CLUSTER)) {
          seen.add(j);
          group.push(j);
        }
      });
    }
    if (group.length > best.length) best = group.map((j) => ps[j] as Placed);
  }
  return best;
}
