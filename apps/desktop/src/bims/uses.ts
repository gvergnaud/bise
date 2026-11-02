// A working bim walks to the object of the plugin its turn uses (Gabriel m_52113: « quand un agent
// utilise un plugin ... il se déplace physiquement dans la map des BIMs vers l'objet qui correspond
// au plugin »). The hub says what the current call names (Agent.doing.uses: a tool group, a skill,
// an MCP server; architect m_52117); `pluginOfUse` matches it against his plugins rows, exactly by
// kind; `standSpotOf` is where a bim stands to use a placed object (in front of it, else beside it,
// clear of every box and near the walk network); `nextTrips` keeps the trips calm (bims-plugins
// m_52119): a burst of calls to one plugin is one trip, the bim stays HOLD_S after the last call,
// and goes to the same object at most once per FLOOR_S. States win: only a working bim goes (the
// caller asks only for those). Pure: the same rows, office and clock give the same trips.
import type { Plugin, Uses } from '@/proto';
import { pluginKey } from './plugin-objects';
import type { Placed } from './corners';
import type { Box, Office, Pt, Spot, UseHow } from './model';
import { nearOnNet, network, wayIn } from './paths';

/** s a bim stays at the object after its last call there: a burst is one trip */
export const HOLD_S = 8;
/** s at least between two trips of one bim to one object: a busy agent does not commute all day */
export const FLOOR_S = 30;

/** The plugin key (plugin-objects.ts pluginKey) of the on plugin a call uses; null: none of his. Pure. */
export function pluginOfUse(u: Uses, rows: readonly Plugin[] | null): string | null {
  const on = (rows ?? []).filter((r) => r.on);
  const has = (r: Plugin, kind: string, name: string): boolean => (r.parts ?? []).some((p) => p.kind === kind && p.name === name);
  const named = (r: Plugin, name: string): boolean => r.name === name || r.id === name;
  let row: Plugin | undefined;
  switch (u.kind) {
    case 'skill':
      row = u.plugin !== undefined ? on.find((r) => named(r, u.plugin as string)) : on.find((r) => has(r, 'skill', u.name));
      break;
    case 'server':
      row = on.find((r) => has(r, 'mcp', u.name)) ?? on.find((r) => named(r, u.name));
      break;
    case 'group':
      // a Mistral connector's group (mistral:<group>), a plugin's MCP server of that name, else the plugin's own name
      row = on.find((r) => r.id === `mistral:${u.name}`) ?? on.find((r) => has(r, 'mcp', u.name)) ?? on.find((r) => named(r, u.name));
      break;
    default:
      row = undefined;
  }
  return row === undefined ? null : pluginKey(row);
}

/** how a bim uses an object: a few ways (bims-plugins m_52119), the object's own hover move does the rest */
const HOW: Readonly<Record<string, UseHow>> = {
  macdesk: 'types', retro: 'types', crt: 'types', rack: 'types', printer: 'types', printer3d: 'types', switchboard: 'types', telegraph: 'types', rotary: 'types',
  electronics: 'types', robotarm: 'types', dashboard: 'looks', visio: 'types', booth: 'types', photobooth: 'types', snacks: 'types', drone: 'types', robot: 'types',
  library: 'reads', shelf: 'reads', lectern: 'reads', files: 'reads', map: 'reads', mailbox: 'reads', calendar: 'reads', blackboard: 'reads', trophies: 'reads', packing: 'reads',
  telescope: 'looks', charts: 'looks', globe: 'looks', orrery: 'looks', dish: 'looks', aquarium: 'looks', clock: 'looks', hourglass: 'looks', dinosaur: 'looks',
  arcade: 'plays', pinball: 'plays', piano: 'plays', jukebox: 'plays', synth: 'plays', drums: 'plays', guitar: 'plays', claw: 'plays', babyfoot: 'plays', pingpong: 'plays',
  chess: 'plays', go: 'plays', cards: 'plays', jenga: 'plays', billiards: 'plays', turntable: 'plays', gramophone: 'plays', mic: 'plays', harp: 'plays', cello: 'plays',
};

/** how a bim uses this object; the rest tend it (a plant, a pet, a grill, a scooter) */
export const howUsed = (obj: string): UseHow => HOW[obj] ?? 'tends';

/** a bim's half width (avoid.ts BODY_R) */
const BODY = 0.22;
/**
 * how far its middle stays off any box: its head (render/laptop.ts HEAD_R 0.245) is wider than its
 * body and its box may stand at head height on a post or legs (a mailbox, an easel's canvas: bim-review
 * m_52169), so no part of it goes into an object, whatever its pose leans
 */
const CLEAR = 0.245 + 0.17;

const inside = (p: Pt, b: Box, m: number): boolean => p.x > b.x0 - m && p.x < b.x1 + m && p.z > b.z0 - m && p.z < b.z1 + m;

/** a straight way from a to b (a bim's width) through none of the boxes; the box it starts in (the desk its network point serves) is left behind */
function clearWay(a: Pt, b: Pt, boxes: readonly Box[]): boolean {
  const n = Math.max(2, Math.ceil(Math.hypot(b.x - a.x, b.z - a.z) / 0.1));
  const ahead = boxes.filter((x) => !inside(a, x, BODY));
  for (let i = 1; i <= n; i++) {
    const p = { x: a.x + ((b.x - a.x) * i) / n, z: a.z + ((b.z - a.z) * i) / n };
    if (ahead.some((x) => inside(p, x, BODY))) return false;
  }
  return true;
}

const lenOf = (way: readonly Pt[]): number => way.reduce((n, q, i) => (i === 0 ? 0 : n + Math.hypot(q.x - (way[i - 1] as Pt).x, q.z - (way[i - 1] as Pt).z)), 0);

/**
 * The way in to p from the walk network, every leg clear (clearWay): from its nearest points on the
 * network (each leg's nearest point and ends, within reach, nearest first, a few), a straight step,
 * else an L of two (along x then z, or z then x: round a table to its end, round a mailbox to its
 * slot). null: none. Pure.
 */
function wayFrom(legs: ReturnType<typeof network>, p: Pt, boxes: readonly Box[], reach: number): readonly Pt[] | null {
  const near: { d: number; at: Pt }[] = [];
  const add = (q: Pt): void => {
    const d = Math.hypot(p.x - q.x, p.z - q.z);
    if (d <= reach) near.push({ d, at: q });
  };
  const first = nearOnNet(legs, p);
  if (first !== null) add(first.at);
  for (const [a, b] of legs) {
    const dx = b.x - a.x;
    const dz = b.z - a.z;
    const l = dx * dx + dz * dz;
    const f = l === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.z - a.z) * dz) / l));
    add({ x: a.x + f * dx, z: a.z + f * dz });
    add(a);
    add(b);
  }
  near.sort((u, v) => u.d - v.d);
  const free = (q: Pt): boolean => !boxes.some((x) => inside(q, x, BODY));
  for (const { at } of near.slice(0, 8)) {
    if (clearWay(at, p, boxes)) return [at, p];
    for (const c of [{ x: at.x, z: p.z }, { x: p.x, z: at.z }]) if (free(c) && clearWay(at, c, boxes) && clearWay(c, p, boxes)) return [at, c, p];
  }
  return null;
}

/**
 * where a bim stands at some objects (designer m_52165, m_52168): at a ping-pong or a billiards
 * table, at a short end, facing the net; at an easel, beside its canvas (the brush arm toward it);
 * else in front first
 */
const SIDE: Readonly<Record<string, 'ends' | 'side'>> = { pingpong: 'ends', billiards: 'ends', easel: 'side' };

/**
 * Where a bim stands to use a placed object, facing it: round its floor box at arm's length, then
 * up to four steps further out (16 points a ring), its front first; a spot clear of every box (the
 * furniture, the cozy pieces, the map's props, every object) with a clear straight way from the
 * walk network within `reach`. The nearest to the network wins, the front breaking ties. null: none
 * (the bim stays at its desk, its object shows the call). Pure.
 */
export function standSpotOf(o: Office, p: Placed, all: readonly Placed[], reach = 7): Spot | null {
  return standSpotsOf(o, p, all, reach)[0] ?? null;
}

/** two bims at objects stand at least this far apart, and as far from a chair: a bim's width of floor between their silhouettes, arms out (designer m_52165) */
export const APART = 4 * BODY + 0.15;

/**
 * two standing bims read apart: APART on the floor, and side by side on screen too (the camera looks
 * along +x +z, so two bims one behind the other on that line stack: designer m_52165, « from every
 * camera angle »): a body and a half between them across the view
 */
export const standApart = (a: Pt, b: Pt): boolean => Math.hypot(a.x - b.x, a.z - b.z) >= APART && Math.abs(a.x - b.x - (a.z - b.z)) / Math.SQRT2 >= 3 * BODY;

/**
 * Every spot round a placed object a bim can use it from (standSpotOf's, best first), each at least
 * APART from the better ones: a second bim at the same object takes the second, beside the first
 * (bims-plugins m_52119). Pure.
 */
export function standSpotsOf(o: Office, p: Placed, all: readonly Placed[], reach = 7): readonly Spot[] {
  const c = o.cozy;
  const boxes: Box[] = [...o.furniture, ...(c ? [c.coffee, c.shelf, ...c.lamps, ...c.plants].map((i) => i.box) : []), ...(o.props ?? []), ...all.map((q) => q.box)];
  // its front, the way it faces (three's y turn, front to +z)
  const fx = Math.sin(p.rot);
  const fz = Math.cos(p.rot);
  const seats = [...o.desks.values(), o.mainDesk];
  // the tall pieces round the room (its lamps and plants): the middles of their boxes
  const tall = (c ? [...c.lamps, ...c.plants] : []).map(({ box: x }) => ({ x: (x.x0 + x.x1) / 2, z: (x.z0 + x.z1) / 2 }));
  // q between the camera (+x +z) and a bim at p, within a body and a half across the view
  const screened = (p: Pt, q: Pt): boolean => {
    const ahead = (q.x - p.x + (q.z - p.z)) / Math.SQRT2;
    const across = Math.abs(q.x - p.x - (q.z - p.z)) / Math.SQRT2;
    return ahead > 0 && ahead < 2 && across < 3 * BODY;
  };
  const found: { s: Spot; d: number; i: number; way: readonly Pt[] }[] = [];
  // how far a point is from a box (0 inside)
  const off = (q: Pt, x: Box): number => Math.hypot(Math.max(x.x0 - q.x, 0, q.x - x.x1), Math.max(x.z0 - q.z, 0, q.z - x.z1));
  const others = all.filter((q) => q.key !== p.key);
  // its long axis (a table's ends are on it)
  const long = p.box.x1 - p.box.x0 >= p.box.z1 - p.box.z0 ? { x: 1, z: 0 } : { x: 0, z: 1 };
  const side = SIDE[p.obj];
  for (const more of [0, 0.3, 0.6, 0.9]) {
    const m = CLEAR + more;
    const b = { x0: p.box.x0 - m, x1: p.box.x1 + m, z0: p.box.z0 - m, z1: p.box.z1 + m };
    const cx = (p.box.x0 + p.box.x1) / 2;
    const cz = (p.box.z0 + p.box.z1) / 2;
    for (let i = 0; i < 16; i++) {
      // a ray from its middle, to where it leaves the ring's box
      const a = (i / 16) * 2 * Math.PI;
      const dx = Math.sin(a);
      const dz = Math.cos(a);
      const k = Math.min(Math.abs(dx) > 1e-9 ? (dx > 0 ? b.x1 - cx : cx - b.x0) / Math.abs(dx) : Infinity, Math.abs(dz) > 1e-9 ? (dz > 0 ? b.z1 - cz : cz - b.z0) / Math.abs(dz) : Infinity);
      const at = { x: cx + dx * k, z: cz + dz * k };
      if (boxes.some((x) => inside(at, x, CLEAR - 0.01))) continue;
      // never on a bim's chair (one sits there, or comes back to it)
      if (seats.some((q) => Math.hypot(q.x - at.x, q.z - at.z) < APART)) continue;
      // no other object much nearer: it reads as using this one
      if (others.some((q) => off(at, q.box) + 0.5 < off(at, p.box))) continue;
      // its way in is the straight step from its nearest point on the network (paths.ts path)
      const way = wayFrom(network(o), at, boxes, reach);
      if (way === null) continue;
      const net = { d: lenOf(way) };
      // close to it first, then its side (in front by default: behind costs more than beside, beside more than in front), then the walk in
      const front = dx * fx + dz * fz;
      const along = Math.abs(dx * long.x + dz * long.z);
      const turn = side === 'ends' ? (1 - along) * 6 : side === 'side' ? Math.abs(front) + (front < -0.3 ? 1 : 0) : (1 - front) * 0.5;
      // the camera looks from +x +z (render/camera.ts): a bim on the far side is hidden by the object, its head seems in it (bim-review m_52169)
      // (a low table hides no one: none for it); right behind it, never
      const back = side === 'ends' ? 0 : -(dx + dz) / Math.SQRT2;
      if (back > 0.4) continue;
      // nor right behind a tall piece seen from the camera (a floor lamp, a plant: designer m_52181)
      if (tall.some((q) => screened(at, q))) continue;
      const hidden = Math.max(0, back) * 1.5;
      const d = net.d * 0.5 + turn + hidden + more * 3;
      found.push({ s: { x: at.x, z: at.z, wz: at.z, ax: at.x, face: Math.atan2(cx - at.x, cz - at.z) }, d, i, way });
    }
  }
  // a second bim stands round it, never behind the first on its ray (the iso view stacks those): two rays apart at least
  const out: { s: Spot; i: number; way: readonly Pt[] }[] = [];
  const rays = (a: number, b: number): number => Math.min((a - b + 16) % 16, (b - a + 16) % 16);
  for (const f of found.sort((u, v) => u.d - v.d)) if (out.every((q) => standApart(q.s, f.s) && rays(q.i, f.i) >= 2)) out.push(f);
  // the walk goes in by the way found (paths.ts path)
  for (const f of out) wayIn(o, f.way);
  return out.map((f) => f.s);
}

/**
 * one bim's trip to an object: its plugin key, when it began, when its call there was last seen
 * running (or ended: the rows change only on a change, so a call runs until the look that finds it
 * gone), and whether it still ran at the last look
 */
export interface Trip {
  readonly key: string;
  readonly began: number;
  readonly seen: number;
  readonly on: boolean;
}

/** every bim's trip now, and each bim's last trip start per object (the floor) */
export interface Trips {
  readonly now: ReadonlyMap<string, Trip>;
  readonly last: ReadonlyMap<string, number>;
}

export const NO_TRIPS: Trips = { now: new Map(), last: new Map() };

/**
 * The trips at t from what each working bim's turn uses now (`using`: bim -> plugin key, or null
 * for a call of no plugin; a bim absent is not working: its trip ends at once, a state wins).
 * `can`: whether this bim can go to this object (it is placed and has a use spot). Pure.
 */
export function nextTrips(prev: Trips, using: ReadonlyMap<string, string | null>, t: number, can: (bim: string, key: string) => boolean): Trips {
  const now = new Map<string, Trip>();
  const last = new Map(prev.last);
  for (const [bim, key] of using) {
    const had = prev.now.get(bim);
    if (key !== null && had?.key === key) {
      now.set(bim, { ...had, seen: t, on: true });
      continue;
    }
    if (key !== null && can(bim, key)) {
      const was = last.get(`${bim}|${key}`);
      if (was === undefined || t - was >= FLOOR_S) {
        now.set(bim, { key, began: t, seen: t, on: true });
        last.set(`${bim}|${key}`, t);
        continue;
      }
    }
    // a call of no plugin (or one it may not go to now): it stays HOLD_S after its last call there
    if (had === undefined) continue;
    const ended = had.on ? t : had.seen;
    if (t - ended < HOLD_S) now.set(bim, { ...had, seen: ended, on: false });
  }
  return { now, last };
}
