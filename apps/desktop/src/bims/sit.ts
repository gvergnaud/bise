// les bims: sitting down at a desk without going through its chair (Gabriel: « les agents passent
// a travers leur chaise pour s'assoir »; the move: bims m_32740). Every desk's way in ends at its
// chair's open side: the walk comes beside the chair (S), the chair slides straight back, the bim
// steps sideways in front of it (G, just behind the desk's edge), hops, the chair slides in under
// it, it lands seated and both slide in to the spot. Getting up is the same backwards. A bim that
// stands at its desk (asks, the big screen, a build) steps in the same way and its chair stays out.
//   - `withSeats`: the office's walk graph with each chair spot's way in through S and G
//     (maps/index.ts layoutOf), so every walk to or from a desk goes round its chair;
//   - `sitWay` / `seatWaits`: what a walk to or from a chair spot plays (timeline.ts), as waits;
//   - `sitBeat`: the bim and its chair at a moment of the sit or the get-up (render/pose.ts).
// The law (sit.test.ts): on every map and desk, no part of the bim crosses its chair. Pure, no three.
import type { Activity, Box, Leg, Office, Path, Pt, SitAt, Sits, Spot, Wait } from './model';
import { BODY_R, progress } from './avoid';


/** the desk chair (render/furniture.ts chair): its middle this far behind its spot, its half width, its back behind its middle */
// seatTop: under a seated bim's hips (render/pose.ts SEATED: its hoodie's bottom at 0.14, its legs'
// at ~0.16; his m_50679, at 0.36 they sat 20 cm into the seat), backTop 0.4 over it as before; front:
// the seat's front edge ahead of its middle (shallower than `half`: low now, main's walking feet beside
// it slid back SIT.pull reached a front at 0.21, and there is no room to slide it further on cyclades)
export const CHAIR = { dz: 0.05, half: 0.21, front: 0.17, back: 0.235, seatTop: 0.14, backTop: 0.54 } as const;

/** the sit's sizes, office units (bims m_32740) */
export const SIT = {
  /** the side point S: this far to the chair's open side (clear of the chair, and of main's toes walking at it) */
  side: 0.56,
  /** G and S: this far along the desk's way from the spot (behind it), the bim just behind the desk's edge */
  gap: -0.08,
  /** how far the chair slides back to let its bim in (its front clears the bim at G, and one standing at its build, 0.15 back) */
  pull: 0.55,
  /** the hop: the body this high while the chair slides in under it (its hoodie and legs over the seat) */
  hop: 0.1,
} as const;

/** the beats' lengths, s: the chair out (the bim beside it), the hop (up, chair in, down, slide in), the chair back in */
// (a sit at its desk without a walk, Segment.sit up / down: the hop alone, where it stands)
export const SIT_S = { pull: 0.35, hop: 0.75, back: 0.3 } as const;
/** a napper gets up from its bed (s) before it walks off: it sits up, then stands (timeline.ts waits; render/pose.ts lieDown) */
export const NAP_UP = 1.0;

/** the chair's floor box at `back` (how far it slid back), as a box in the spot's frame: u across, v along the desk's way */
export interface Local {
  readonly u: number;
  readonly v: number;
}

/** the spot's frame: its way ahead (v) and its right (u) */
export function toLocal(s: Spot, p: Pt): Local {
  const fx = Math.sin(s.face);
  const fz = Math.cos(s.face);
  const dx = p.x - s.x;
  const dz = p.z - s.z;
  return { u: dx * fz - dz * fx, v: dx * fx + dz * fz };
}

export function toWorld(s: Spot, u: number, v: number): Pt {
  const fx = Math.sin(s.face);
  const fz = Math.cos(s.face);
  return { x: s.x + u * fz + v * fx, z: s.z - u * fx + v * fz };
}

/** the chair's floor box in its spot's frame, slid back `back` */
export function chairBox(back: number): { u0: number; u1: number; v0: number; v1: number } {
  const c = -CHAIR.dz - back;
  return { u0: -CHAIR.half, u1: CHAIR.half, v0: c - CHAIR.back, v1: c + CHAIR.half };
}

/** the places a bim sits on a chair: every desk and main's */
export const chairSpots = (o: Office): Spot[] => [...o.desks.values(), o.mainDesk];

/** whether this activity sits on its chair (render/pose.ts: its rest pose is seated; pose.test.ts checks they agree) */
export function seatedAct(act: Activity): boolean {
  if (act.at !== 'desk' && act.at !== 'main') return false;
  switch (act.k) {
    case 'work':
    case 'hot':
    case 'main':
      return act.doing !== 'computer' && act.doing !== 'publish' && act.doing !== 'compact' && act.doing !== 'build';
    case 'mainidle':
    case 'blocked':
    case 'failed':
    case 'stopped':
    case 'sleep':
    case 'waitagent':
    case 'nap':
      return true;
    default:
      return false;
  }
}

// ---- the walk graph

/** S and G of a chair spot on its side */
export function sidePoints(s: Spot, side: 1 | -1): { S: Pt; G: Pt } {
  return { S: toWorld(s, side * SIT.side, SIT.gap), G: toWorld(s, 0, SIT.gap) };
}

/**
 * The office's walk graph with each chair's way in round it: a leg [spot, P] becomes spot - G - S -
 * Q - P, Q beside the chair on P's line (the walkway behind, or the aisle across). The side: the
 * one towards the spot's aisle when there is room for the bim and the chair slides back clear of
 * the rest, else the other; neither: the spot keeps its way (sit.test.ts lists none). Pure.
 */
export function withSeats(o: Office): Office {
  const spots = chairSpots(o);
  let walks: Leg[] = [...o.walks];
  for (const s of spots) {
    const at = walks.filter(([a, b]) => near(a, s) || near(b, s));
    if (at.length === 0) continue;
    const side = sideOf(o, s, at);
    if (side === null) continue;
    const { S, G } = sidePoints(s, side);
    const next: Leg[] = [[s, G], [G, S]];
    for (const [a, b] of at) {
      const p = near(a, s) ? b : a;
      const q = toWorld(s, side * SIT.side, Math.min(SIT.gap, toLocal(s, p).v));
      next.push([S, q], [q, p]);
    }
    walks = [...walks.filter((l) => !at.includes(l)), ...next.filter(([a, b]) => !near(a, b))];
  }
  return { ...o, walks };
}

/**
 * Where another bim stands at this desk (a visit, a report): its walkway point, but on a map whose
 * walkway starts at the chair (the greenhouse, the nordic table: wz is the spot), beside the chair
 * at its S, never on it. Pure.
 */
export function visitAt(o: Office, s: Spot): Spot {
  const way = { x: s.x, z: s.wz };
  if (!near(way, s)) return { x: s.x, z: s.wz, wz: s.wz, ax: s.ax, face: s.face };
  const G = toWorld(s, 0, SIT.gap);
  for (const [a, b] of o.walks) {
    const S = near(a, G) && !near(b, s) ? b : near(b, G) && !near(a, s) ? a : null;
    if (S) return { x: S.x, z: S.z, wz: S.z, ax: S.x, face: s.face };
  }
  return { x: s.x, z: s.wz, wz: s.wz, ax: s.ax, face: s.face };
}

/** the chair's open side (1 its right, -1 its left), null when neither has room */
function sideOf(o: Office, s: Spot, at: readonly Leg[]): 1 | -1 | null {
  const aisle = toLocal(s, { x: s.ax, z: s.wz }).u;
  const ends = at.map(([a, b]) => toLocal(s, near(a, s) ? b : a));
  const lateral = ends.find((e) => Math.abs(e.u) > 0.01)?.u ?? aisle;
  const first: 1 | -1 = lateral < -0.01 ? -1 : 1;
  for (const side of [first, -first as 1 | -1]) if (roomOn(o, s, side, ends)) return side;
  return null;
}

/** room for the sit on that side: the bim at S, G and on its way to Q clear of the other chairs, the furniture and the floor's edge */
function roomOn(o: Office, s: Spot, side: 1 | -1, ends: readonly Local[]): boolean {
  const others = chairSpots(o).filter((c) => !near(c, s));
  const pts: Local[] = [{ u: side * SIT.side, v: SIT.gap }, { u: 0, v: SIT.gap }];
  for (const e of ends) {
    const qv = Math.min(SIT.gap, e.v);
    for (let k = 0; k <= 4; k++) pts.push({ u: side * SIT.side, v: SIT.gap + ((qv - SIT.gap) * k) / 4 });
  }
  const own = ownDesk(o, s);
  // its own desk (ahead of it, or the box the plan gives the spot: the greenhouse's) never blocks it
  const mine = (b: Box): boolean => b === own || (b.of !== undefined && near(b.of, s));
  for (const l of pts) {
    const p = toWorld(s, l.u, l.v);
    if (p.x < o.x0 || p.x > o.x0 + o.W || p.z < 0 || p.z > o.D + 0.4) return false;
    if (o.furniture.some((b) => !mine(b) && circleInBox(p, BODY_R - 0.04, b))) return false;
    if (others.some((c) => circleInChair(c, 0, p, BODY_R - 0.02))) return false;
  }
  return true;
}

/** the box the spot's desk is (the furniture box straight ahead of it, if any) */
function ownDesk(o: Office, s: Spot): Box | undefined {
  const ahead = toWorld(s, 0, 0.35);
  return o.furniture.find((b) => !b.of && ahead.x > b.x0 && ahead.x < b.x1 && ahead.z > b.z0 && ahead.z < b.z1);
}

/** whether the chair slid back `back` has room: off the floor's edge, the furniture and the other chairs. Pure. */
export function pullRoom(o: Office, s: Spot, back: number = SIT.pull): boolean {
  const c = chairBox(back);
  const corners = [toWorld(s, c.u0, c.v0), toWorld(s, c.u1, c.v0), toWorld(s, c.u0, c.v1), toWorld(s, c.u1, c.v1)];
  if (corners.some((p) => p.x < o.x0 || p.x > o.x0 + o.W || p.z < 0)) return false;
  const mid = toWorld(s, 0, (c.v0 + c.v1) / 2);
  const r = Math.hypot(CHAIR.half, (c.v1 - c.v0) / 2) - 0.06;
  if (o.furniture.some((b) => !b.of && circleInBox(mid, r, b))) return false;
  return chairSpots(o).every((x) => near(x, s) || Math.hypot(...minus(chairMid(x, 0), mid)) >= 2 * r);
}

const chairMid = (s: Spot, back: number): Pt => toWorld(s, 0, -CHAIR.dz - back);
const minus = (a: Pt, b: Pt): [number, number] => [a.x - b.x, a.z - b.z];

/** a circle of radius r at p meets the box */
function circleInBox(p: Pt, r: number, b: Box): boolean {
  const nx = Math.max(b.x0, Math.min(p.x, b.x1));
  const nz = Math.max(b.z0, Math.min(p.z, b.z1));
  return Math.hypot(p.x - nx, p.z - nz) < r;
}

/** a circle of radius r at p meets the chair of spot c slid back `back` */
export function circleInChair(c: Spot, back: number, p: Pt, r: number): boolean {
  const l = toLocal(c, p);
  const b = chairBox(back);
  const nu = Math.max(b.u0, Math.min(l.u, b.u1));
  const nv = Math.max(b.v0, Math.min(l.v, b.v1));
  return Math.hypot(l.u - nu, l.v - nv) < r;
}

// ---- a walk's sit

/**
 * What a walk to `spot` (sitting or standing there: `act`) plays at its end, and what a walk from
 * `prev` (the chair spot it leaves, its activity there) plays at its start: the walk must go
 * through G and S (withSeats' way), else nothing (a walk the avoid search bent). Pure.
 */
export function sitWay(path: Path, to: { spot: Spot; act: Activity } | null, from: { spot: Spot; act: Activity } | null): Sits {
  const pts = path.pts;
  const out: { in?: SitAt; out?: SitAt } = {};
  if (to && pts.length >= 3) {
    const [s, g, end] = [pts.at(-3), pts.at(-2), pts.at(-1)] as [Pt, Pt, Pt];
    const side = sideAt(to.spot, s, g, end);
    if (side !== null) out.in = { spot: to.spot, side, standing: !seatedAct(to.act) };
  }
  if (from && pts.length >= 3) {
    const [start, g, s] = [pts[0], pts[1], pts[2]] as [Pt, Pt, Pt];
    const side = sideAt(from.spot, s, g, start);
    if (side !== null) out.out = { spot: from.spot, side, standing: !seatedAct(from.act) };
  }
  return out;
}

/** the side of a walk's S when g is the spot's G and s its S on either side */
function sideAt(spot: Spot, s: Pt, g: Pt, end: Pt): 1 | -1 | null {
  if (!near(end, spot)) return null;
  for (const side of [1, -1] as const) {
    const p = sidePoints(spot, side);
    if (near(g, p.G) && near(s, p.S)) return side;
  }
  return null;
}

/** where on the walk the sit's beats are: the distance of S and G along it (in), and of G and S (out) */
function marks(path: Path, sit: Sits): { inS: number; inG: number; outG: number; outS: number } {
  const n = path.seg.length;
  const len = path.len;
  const last = path.seg[n - 1] ?? 0;
  const before = path.seg[n - 2] ?? 0;
  return {
    inS: sit.in ? len - last - before : len,
    inG: sit.in ? len - last : len,
    outG: sit.out ? (path.seg[0] ?? 0) : 0,
    outS: sit.out ? (path.seg[0] ?? 0) + (path.seg[1] ?? 0) : 0,
  };
}

/** the waits a sit adds to its walk (the chair out, the hop; the hop, the chair back in), merged with the walk's own, in order */
export function seatWaits(path: Path, sit: Sits, waits: readonly Wait[] = []): Wait[] {
  const m = marks(path, sit);
  const add: Wait[] = [];
  if (sit.out) {
    if (!sit.out.standing) add.push({ d: 0, s: SIT_S.hop });
    add.push({ d: m.outS, s: SIT_S.back });
  }
  if (sit.in) {
    add.push({ d: m.inS, s: SIT_S.pull });
    if (!sit.in.standing) add.push({ d: m.inG, s: SIT_S.hop });
  }
  // a sit's wait before the walk's own at the same place: a beat is never cut in two
  return [...add, ...waits].map((w, i) => ({ w, i })).sort((a, b) => a.w.d - b.w.d || a.i - b.i).map(({ w }) => w);
}

/** when (s into its segment) the walk reaches distance d: its legs at its speed and every wait before d (or at d too, `after`) */
export function timeAt(path: Path, speed: number, waits: readonly Wait[], d: number, after: boolean): number {
  let t = speed > 0 ? Math.min(d, path.len) / speed : 0;
  for (const w of waits) if (w.d < d - 1e-9 || (after && w.d <= d + 1e-9)) t += w.s;
  return t;
}

/**
 * The bim and its chair at a moment of a sit or a get-up, in the spot's frame: where it stands (u,
 * v), how high its body (y over its pose's), its legs (0 standing .. 1 seated), how far its chair is
 * slid back, which way it faces (its spot's, or null: its walk's), and whether it is sat on its
 * chair. Null: the walk plays as a walk. `e`: seconds into the segment. Pure.
 */
export interface Beat {
  readonly u: number;
  readonly v: number;
  readonly y: number;
  readonly legs: number;
  readonly back: number;
  readonly facing: boolean;
  readonly seated: boolean;
  /** stepping sideways (its legs swing), standing still beside or in front of the chair */
  readonly step: boolean;
}

export function sitBeat(seg: { readonly path: Path; readonly speed: number; readonly waits?: readonly Wait[]; readonly sit?: Sits }, e: number): { at: SitAt; beat: Beat } | null {
  const sit = seg.sit;
  if (!sit) return null;
  // at its desk without a walk: the hop where it stands, up (the chair out) or down (the chair in)
  if (sit.up) return e < SIT_S.hop ? { at: sit.up, beat: standBeat(e / SIT_S.hop, 0) } : null;
  if (sit.down) return e < SIT_S.hop ? { at: sit.down, beat: hopBeat(e / SIT_S.hop, 0) } : null;
  if (!sit.in && !sit.out) return null;
  const waits = seg.waits ?? [];
  if (e >= timeAt(seg.path, seg.speed, waits, seg.path.len, true)) return null;
  const m = marks(seg.path, sit);
  const walked = (): number => progress({ path: seg.path, speed: seg.speed, waits }, e).d;
  if (sit.out) {
    const o = sit.out;
    const tS = timeAt(seg.path, seg.speed, waits, m.outS, false);
    const tEnd = timeAt(seg.path, seg.speed, waits, m.outS, true);
    if (e < tS) {
      const hop = o.standing ? 0 : SIT_S.hop;
      if (e < hop) return { at: o, beat: standBeat(e / hop) };
      // in front of the desk at G (the chair's slide's last bit), then sideways out to S
      const d = walked();
      const f = clamp01((d - m.outG) / Math.max(1e-6, m.outS - m.outG));
      return { at: o, beat: { u: o.side * SIT.side * f, v: SIT.gap, y: 0, legs: 0, back: SIT.pull, facing: true, seated: false, step: d > m.outG } };
    }
    if (e < tEnd) {
      // beside it, the chair slides back in; it turns to its way
      const f = ease((e - tS) / SIT_S.back);
      return { at: o, beat: { u: o.side * SIT.side, v: SIT.gap, y: 0, legs: 0, back: SIT.pull * (1 - f), facing: f < 0.5, seated: false, step: false } };
    }
  }
  if (sit.in) {
    const i = sit.in;
    const tS = timeAt(seg.path, seg.speed, waits, m.inS, false);
    if (e < tS) return null;
    if (e < timeAt(seg.path, seg.speed, waits, m.inS, true)) {
      // beside it, facing the desk: the chair slides back
      const f = ease((e - tS) / SIT_S.pull);
      return { at: i, beat: { u: i.side * SIT.side, v: SIT.gap, y: 0, legs: 0, back: SIT.pull * f, facing: true, seated: false, step: false } };
    }
    const tG = timeAt(seg.path, seg.speed, waits, m.inG, false);
    if (e < tG) {
      // sideways in front of the chair, to G
      const f = clamp01((walked() - m.inS) / Math.max(1e-6, m.inG - m.inS));
      return { at: i, beat: { u: i.side * SIT.side * (1 - f), v: SIT.gap, y: 0, legs: 0, back: SIT.pull, facing: true, seated: false, step: true } };
    }
    if (i.standing) {
      // it stays standing: up to its spot, the chair left out
      const f = clamp01((walked() - m.inG) / Math.max(1e-6, seg.path.len - m.inG));
      return { at: i, beat: { u: 0, v: SIT.gap * (1 - f), y: 0, legs: 0, back: SIT.pull, facing: true, seated: false, step: false } };
    }
    const tH = timeAt(seg.path, seg.speed, waits, m.inG, true);
    if (e < tH) return { at: i, beat: hopBeat((e - tG) / Math.max(1e-6, tH - tG)) };
    // the walk's last bit (G to the spot): already there, seated
    return { at: i, beat: SEATED_BEAT };
  }
  return null;
}

/** sat on its chair at its spot */
export const SEATED_BEAT: Beat = { u: 0, v: 0, y: 0, legs: 1, back: 0, facing: true, seated: true, step: false };

/**
 * The hop at h (0..1): up with its legs out ahead, the chair slides in under it, down onto the seat,
 * then the bim and its chair slide in together from G to the spot. The get-up plays it backwards.
 */
export function hopBeat(h: number, gap: number = SIT.gap): Beat {
  const UP = 0.28;
  const IN = 0.62;
  const DOWN = 0.8;
  const up = ease(clamp01(h / UP));
  const chairIn = ease(clamp01((h - UP) / (IN - UP)));
  const down = ease(clamp01((h - IN) / (DOWN - IN)));
  const slide = ease(clamp01((h - DOWN) / (1 - DOWN)));
  // seated relation: the chair's middle CHAIR.dz behind the bim, so at G it is slid back -gap
  const back = SIT.pull + (-gap - SIT.pull) * chairIn - -gap * slide;
  return { u: 0, v: gap * (1 - slide), y: SIT.hop * up * (1 - down), legs: up, back, facing: true, seated: h >= IN, step: false };
}

/**
 * Getting up from its chair, s from 0 (seated) to 1 (standing in front of it): the chair pushed back
 * first, the bim rising once its seat's front has cleared its legs, then a step back to `gap` (his
 * m_50679: « quand on se lève on la recule un peu »; the hop played backwards rose the bim inside its
 * chair, the chair sliding out only once it was up). Pure.
 */
export function standBeat(s: number, gap: number = SIT.gap): Beat {
  const push = ease(clamp01(s / STAND_AT.push));
  const rise = ease(clamp01((s - STAND_AT.rise) / (1 - STAND_AT.rise)));
  const step = ease(clamp01((s - STAND_AT.step) / (1 - STAND_AT.step)));
  // its weight off the seat a little as it pushes back (the seat slides out from under it)
  return { u: 0, v: gap * step, y: STAND_LIFT * push * (1 - rise), legs: 1 - rise, back: SIT.pull * push, facing: true, seated: rise < 0.5, step: false };
}

/** the get-up's beats, as shares of it: the chair pushed back by `push`, the bim rising from `rise`, its step back from `step` */
export const STAND_AT = { push: 0.55, rise: 0.25, step: 0.7 } as const;
/** how far it lifts off the seat as it pushes back */
const STAND_LIFT = 0.04;

const ease = (f: number): number => {
  const x = clamp01(f);
  return x * x * (3 - 2 * x);
};
const clamp01 = (f: number): number => Math.max(0, Math.min(1, f));
const near = (a: Pt, b: Pt): boolean => Math.hypot(a.x - b.x, a.z - b.z) < 1e-4;
