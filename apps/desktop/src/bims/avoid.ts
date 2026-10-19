// les bims: walks that never go through another bim (bims-avoid). A new walk is planned against
// every other bim's walk already planned (its track: where it is at each time, then standing at
// its end for good, or gone off the map): a search over the walk network and time that may wait at
// a junction, step into a side way to let one pass, or go round a standing bim on a ring (the
// network's legs cut where they come near it, an arc each side, kept only clear of furniture and
// the floor's edges). The search's legs never cross furniture but the bed or deck chair of the
// walk's own ends. The first walk found that keeps every pair SEP apart wins; its waits ride on the
// segment (Segment.waits). No walk found within the budget: the caller walks the plain shortest
// one, as before. Pure: the same office and tracks give the same walk; no clock, no random.
import type { Box, Office, Pt, Segment, Wait } from './model';
import { makePath, network, path } from './paths';

/** a bim's body radius, floor tiles (render/bim.ts: its hoodie's capsule) */
export const BODY_R = 0.21;
/** how far apart a planned walk keeps two bims' middles: two bodies and a hair (the closest spots, the board's, are 0.45 apart) */
export const SEP = 0.44;
/** the ring a walk takes round a standing bim: half a tile at least (bims m_31438) */
const RING = 0.52;
/** a wait's step, s */
const STEP = 0.2;
/** how much later than the plain walk's end a planned walk may end, s */
const HORIZON = 30;
/** states a search expands at most (its cost when nothing works) */
const BUDGET = 3000;
/** states all the searches of one update expand at most (a snapshot never stalls a frame): past it, walks are stuck */
export const UPDATE_BUDGET = 3000;

/** a search's graph, counted as this many states */
const GRAPH_COST = 60;
/**
 * a search's graph has this many legs at most: rings round bims standing close together cut each
 * other's arcs, and each ring past the first few multiplied the legs (17 bims coming in at once: 21,889
 * nodes, 13 s for one walk, the page frozen). Past it, the remaining standing bims get no ring
 * (the search still keeps clear of them). The walking law's graphs: 455 legs at most (california), so its walks are the same.
 */
const GRAPH_MAX = 500;

/** what an update's searches may still expand (one per update, timeline.ts) */
export interface Budget {
  left: number;
}
/** how far from its spot a bim stands when the spot is taken */
const BESIDE = 1.2;
/** a walk that cannot be planned from here keeps on as planned this long at most, tried every WALK_STEP s */
const WALK_ON = 3;
const WALK_STEP = 0.25;
/** a bim coming in by the door waits outside at most this long, then comes in and waits inside (bims m_31438), s */
const HIDDEN_MAX = 1.5;
/** a bim stands still on its walk at most this long at once: past it, it takes a side way or a ring (bims m_31438), s */
const MAX_WAIT = 2;

/** one timed point of a track */
export interface Key {
  readonly t: number;
  readonly x: number;
  readonly z: number;
}

/** a bim's motion from `keys[0].t`: linear between keys, then still at the last one until `end` (gone after) */
export interface Track {
  readonly keys: readonly Key[];
  readonly end: number;
}

/** a planned walk: when it starts (later than asked for a hidden bim), its corners and its waits */
export interface Plan {
  readonly t: number;
  readonly pts: readonly Pt[];
  readonly waits: readonly Wait[];
}

type Walk = Pick<Segment, 'path' | 'speed' | 'waits'>;

/** the seconds a segment walks: its legs at its speed and its waits */
export function walkTime(seg: Walk): number {
  const waited = (seg.waits ?? []).reduce((s, w) => s + w.s, 0);
  return (seg.speed > 0 ? seg.path.len / seg.speed : 0) + waited;
}

/** where a segment's walk is `e` seconds in: the distance along its path, and whether it stands waiting */
export function progress(seg: Walk, e: number): { d: number; waiting: boolean } {
  let tt = 0;
  let d = 0;
  for (const w of seg.waits ?? []) {
    const walk = (w.d - d) / seg.speed;
    if (e < tt + walk) return { d: d + (e - tt) * seg.speed, waiting: false };
    tt += walk;
    d = w.d;
    if (e < tt + w.s) return { d, waiting: true };
    tt += w.s;
  }
  return { d: Math.min(seg.path.len, d + (e - tt) * seg.speed), waiting: false };
}

/** a segment as a track; `end`: when it leaves the map (a leaving bim), else never */
export function trackOf(seg: Walk & Pick<Segment, 't'>, end = Infinity): Track {
  const { pts, seg: lens } = seg.path;
  const first = pts[0];
  if (!first) return { keys: [], end };
  const keys: Key[] = [{ t: seg.t, x: first.x, z: first.z }];
  const waits = seg.waits ?? [];
  let extra = 0;
  let wi = 0;
  let cum = 0;
  const at = (d: number): number => seg.t + (seg.speed > 0 ? d / seg.speed : 0) + extra;
  for (let w = waits[wi]; w !== undefined && w.d <= 1e-9; w = waits[wi]) {
    extra += w.s;
    keys.push({ t: at(0), x: first.x, z: first.z });
    wi++;
  }
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1] as Pt;
    const b = pts[i] as Pt;
    const len = lens[i - 1] ?? 0;
    for (let w = waits[wi]; w !== undefined && w.d < cum + len - 1e-9; w = waits[wi]) {
      const f = len > 0 ? (w.d - cum) / len : 0;
      const p = { x: a.x + (b.x - a.x) * f, z: a.z + (b.z - a.z) * f };
      keys.push({ t: at(w.d), ...p });
      extra += w.s;
      keys.push({ t: at(w.d), ...p });
      wi++;
    }
    cum += len;
    keys.push({ t: at(cum), x: b.x, z: b.z });
    for (let w = waits[wi]; w !== undefined && w.d <= cum + 1e-9; w = waits[wi]) {
      extra += w.s;
      keys.push({ t: at(cum), x: b.x, z: b.z });
      wi++;
    }
  }
  return { keys, end };
}

/**
 * A walk from `from` to `to` at `speed`, from `t`, that keeps SEP from every track in `others`.
 * `hidden`: it is not on the map before it starts (a bim coming in by the door): it may start
 * later. Null when no such walk is found (the caller walks the plain one).
 */
export function plan(o: Office, from: Pt, to: Pt, speed: number, t: number, others: readonly Track[], hidden: boolean, budget: Budget): Plan | null {
  if (budget.left <= 0) return null;
  const live = others.filter((k) => k.keys.length > 0 && k.end > t);
  const rests = live.map(restOf).filter((p): p is Pt => p !== null);
  const clearOf = (p: Pt, r: number): boolean => rests.every((c) => Math.hypot(c.x - p.x, c.z - p.z) >= r);
  // a track it already stands too close to (two bims put on one spot): it only may not come closer
  const thr = live.map((k) => {
    if (hidden) return SEP;
    const p = posAt(k, t);
    return p ? Math.min(SEP, Math.hypot(p.x - from.x, p.z - from.z) - 1e-6) : SEP;
  });
  const boxes = live.map((k) => boxOf(k, t));
  const free = (a: Pt, ta: number, b: Pt, tb: number): boolean => {
    const x0 = Math.min(a.x, b.x);
    const x1 = Math.max(a.x, b.x);
    const z0 = Math.min(a.z, b.z);
    const z1 = Math.max(a.z, b.z);
    for (let i = 0; i < live.length; i++) {
      const bx = boxes[i] as Box;
      const r = thr[i] as number;
      if (bx.x0 > x1 + r || bx.x1 < x0 - r || bx.z0 > z1 + r || bx.z1 < z0 - r) continue;
      if (minDist(live[i] as Track, a, ta, b, tb) < r) return false;
    }
    return true;
  };
  // standing at the end for good: no track ever comes near
  const stays = (p: Pt, ta: number): boolean => {
    for (let i = 0; i < live.length; i++) {
      const k = live[i] as Track;
      const last = k.keys[k.keys.length - 1] as Key;
      if (minDist(k, p, ta, p, Math.max(ta, last.t) + 1) < (thr[i] as number)) return false;
    }
    return true;
  };
  // the shortest walk is clear already (most walks): no search
  if (!hidden && clearOf(to, SEP)) {
    const short = path(o, from, to).pts;
    const own = (b: Box): boolean => b.of !== undefined && (near(b.of, from) || near(b.of, to) || (from.x > b.x0 && from.x < b.x1 && from.z > b.z0 && from.z < b.z1));
    let at = t;
    let ok = true;
    for (let k = 1; k < short.length && ok; k++) {
      const a = short[k - 1] as Pt;
      const b = short[k] as Pt;
      const tb = at + Math.hypot(b.x - a.x, b.z - a.z) / speed;
      ok = free(a, at, b, tb) && !o.furniture.some((w) => !own(w) && crosses(a, b, w));
      at = tb;
    }
    if (ok && stays(to, at)) return { t, pts: short, waits: [] };
  }
  budget.left -= GRAPH_COST;
  const g = graphOf(o, from, to, rests);
  /** the network's points next to q where nobody stands, nearest first */
  const besides = (q: Pt): number[] => g.nodes
    .map((p, i) => ({ i, d: Math.hypot(p.x - q.x, p.z - q.z) }))
    .filter(({ i, d }) => d > 1e-4 && d < BESIDE && clearOf(g.nodes[i] as Pt, SEP + 1e-3) && onNetwork(o, g.nodes[i] as Pt))
    .sort((a, b) => a.d - b.d || a.i - b.i)
    .slice(0, 4)
    .map(({ i }) => i);
  // coming in where a bim stands for good: it comes in beside
  const entry = hidden && !clearOf(from, SEP) ? besides(from)[0] : g.start;
  if (entry === undefined) return null;

  /** the earliest walk to one of `goals` (and standing there for good), within the budget */
  const search = (goals: readonly number[]): Plan | null => {
    if (goals.length === 0) return null;
    const h = dijkstra(g.adj, g.nodes.length, goals);
    const plain = (h[entry] ?? Infinity) / speed;
    if (!Number.isFinite(plain)) return null;
    const limit = t + plain + HORIZON + (hidden ? HIDDEN_MAX : 0);
    interface St { readonly n: number; readonly t: number; readonly up: number; readonly f: number; readonly d: number; readonly w: number }
    const states: St[] = [];
    const heap = new Heap((a, b) => {
      const x = states[a] as St;
      const y = states[b] as St;
      return x.f - y.f || x.d - y.d || a - b;
    });
    const seen = new Set<number>();
    // w: how long it has stood still there (a wait is a few seconds at most, bims m_31438)
    const add = (n: number, at: number, up: number, d: number, w: number): void => {
      const k = (n * 1e6 + Math.round((at - t) / (STEP / 2))) * 16 + Math.round(w / STEP);
      if (seen.has(k) || at > limit) return;
      seen.add(k);
      states.push({ n, t: at, up, d, w, f: at + (h[n] ?? Infinity) / speed });
      heap.push(states.length - 1);
    };
    if (hidden) for (let s = 0; s <= HIDDEN_MAX / STEP; s++) add(entry, t + s * STEP, -1, 0, 0);
    else add(entry, t, -1, 0, 0);
    for (let expanded = 0; expanded < BUDGET && budget.left > 0; expanded++) {
      const i = heap.pop();
      if (i === undefined) break;
      budget.left--;
      const s = states[i] as St;
      const p = g.nodes[s.n] as Pt;
      if (goals.includes(s.n) && stays(p, s.t)) return planOf(states, i, g.nodes);
      if (s.w + STEP <= MAX_WAIT + 1e-9 && free(p, s.t, p, s.t + STEP)) add(s.n, s.t + STEP, i, s.d, s.w + STEP);
      for (const e of g.adj[s.n] ?? []) {
        const q = g.nodes[e.to] as Pt;
        const tb = s.t + e.w / speed;
        if (free(p, s.t, q, tb)) add(e.to, tb, i, s.d + e.w, 0);
      }
    }
    return null;
  };
  // its spot, or beside it when a bim stands on it for good (two visitors at one desk, two bims at the door) or in its way in
  const out = clearOf(to, SEP) ? (search([g.goal]) ?? search(besides(to))) : search(besides(to));
  return out;
}

/**
 * plan(), and when nothing works from here (another bim walks where it would stop): it keeps on
 * its walk as planned (`own`, at its speed) a little longer and plans from there, up to WALK_ON s.
 * Null: no clear way (the caller keeps it doing what it did, or out of the door, and tries again
 * at its next update).
 */
export function planOn(o: Office, from: Pt, to: Pt, speed: number, t: number, others: readonly Track[], hidden: boolean, own: { readonly track: Track; readonly speed: number } | null, budget: Budget): Planned | null {
  const now = plan(o, from, to, speed, t, others, hidden, budget);
  if (now !== null) return { plan: now, speed };
  if (own === null || hidden || own.speed <= 0) return null;
  const last = own.track.keys[own.track.keys.length - 1];
  for (let k = 1; last !== undefined && t + k * WALK_STEP < Math.min(last.t, t + WALK_ON); k++) {
    const t2 = t + k * WALK_STEP;
    const p = posAt(own.track, t2);
    if (p === null) break;
    const rest = plan(o, p, to, own.speed, t2, others, false, budget);
    if (rest === null) continue;
    const head = walkOf(own.track, t, t2);
    const len = head.pts.reduce((s, q, i) => (i === 0 ? 0 : s + Math.hypot(q.x - (head.pts[i - 1] as Pt).x, q.z - (head.pts[i - 1] as Pt).z)), 0);
    const pts = [...head.pts, ...rest.pts.slice(1)];
    const waits = [...head.waits, ...rest.waits.map((w) => ({ d: w.d + len, s: w.s }))];
    return { plan: { t, pts: makePath(pts).pts, waits: merged(waits) }, speed: own.speed };
  }
  return null;
}

/** planOn's walk and its speed (the one it walks on at, when it walked on) */
export interface Planned {
  readonly plan: Plan;
  readonly speed: number;
}

/** a track from ta to tb as a walk's corners and waits */
function walkOf(k: Track, ta: number, tb: number): { pts: Pt[]; waits: Wait[] } {
  const marks: Key[] = [];
  const at = (t: number): Key => ({ t, ...(posAt(k, t) as Pt) });
  marks.push(at(ta));
  for (const q of k.keys) if (q.t > ta && q.t < tb) marks.push(q);
  marks.push(at(tb));
  const pts: Pt[] = [{ x: (marks[0] as Key).x, z: (marks[0] as Key).z }];
  const waits: Wait[] = [];
  let d = 0;
  for (let i = 1; i < marks.length; i++) {
    const a = marks[i - 1] as Key;
    const b = marks[i] as Key;
    const step = Math.hypot(b.x - a.x, b.z - a.z);
    if (step < 1e-9) {
      if (b.t - a.t > 1e-9) waits.push({ d, s: b.t - a.t });
      continue;
    }
    d += step;
    pts.push({ x: b.x, z: b.z });
  }
  return { pts, waits: merged(waits) };
}

/** two waits at one place: one */
function merged(ws: readonly Wait[]): Wait[] {
  const out: { d: number; s: number }[] = [];
  for (const w of ws) {
    const prev = out.at(-1);
    if (prev && Math.abs(prev.d - w.d) < 1e-9) prev.s += w.s;
    else if (w.s > 1e-9) out.push({ d: w.d, s: w.s });
  }
  return out;
}

/** the states back from the goal, as a walk: its corners and its waits (at a distance along it) */
function planOf(states: readonly { n: number; t: number; up: number }[], last: number, nodes: readonly Pt[]): Plan {
  const chain: { n: number; t: number }[] = [];
  for (let i = last; i >= 0; i = (states[i] as { up: number }).up) chain.unshift(states[i] as { n: number; t: number });
  const head = chain[0] as { n: number; t: number };
  const pts: Pt[] = [nodes[head.n] as Pt];
  const waits: { d: number; s: number }[] = [];
  let d = 0;
  for (let i = 1; i < chain.length; i++) {
    const a = chain[i - 1] as { n: number; t: number };
    const b = chain[i] as { n: number; t: number };
    if (a.n === b.n) {
      const w = waits.at(-1);
      if (w && Math.abs(w.d - d) < 1e-9) w.s += b.t - a.t;
      else waits.push({ d, s: b.t - a.t });
      continue;
    }
    const p = nodes[a.n] as Pt;
    const q = nodes[b.n] as Pt;
    d += Math.hypot(q.x - p.x, q.z - p.z);
    pts.push(q);
  }
  return { t: head.t, pts: makePath(pts).pts, waits };
}

// ---- the graph a walk is searched on

interface Graph {
  readonly nodes: readonly Pt[];
  readonly adj: readonly (readonly { readonly to: number; readonly w: number }[] | undefined)[];
  readonly start: number;
  readonly goal: number;
}

/**
 * The network's legs cut at `from` and `to`, those through furniture dropped (but the bed or deck
 * chair of the walk's own ends), and a ring round each point a bim will stand at.
 */
function graphOf(o: Office, from: Pt, to: Pt, rests: readonly Pt[]): Graph {
  // the bed or deck chair it walks into, or the one it gets out of (it may stand half in it)
  const inBox = (p: Pt, b: Box): boolean => p.x > b.x0 && p.x < b.x1 && p.z > b.z0 && p.z < b.z1;
  const own = (b: Box): boolean => b.of !== undefined && (near(b.of, from) || near(b.of, to) || inBox(from, b));
  const walls = o.furniture.filter((b) => !own(b));
  let legs: [Pt, Pt][] = [];
  for (const l of cutAt(cutAt(baseOf(o), from), to)) if (l.boxes.every(own)) legs.push([l.a, l.b]);
  const floor = { x0: o.x0 + 0.15, x1: o.x0 + o.W - 0.15, z0: 0.15 };
  const clear = (pts: readonly Pt[]): boolean =>
    pts.every((p) => p.x > floor.x0 && p.x < floor.x1 && p.z > floor.z0) && pts.every((p, j) => j === 0 || !o.furniture.some((w) => crosses(pts[j - 1] as Pt, p, w)));
  for (const c of rests) {
    if (legs.length > GRAPH_MAX) break;
    // a straight way past a standing bim: a smooth swerve round it (bump), no corner where it leaves or joins the way
    const bumped: (readonly [Pt, Pt])[] = [];
    for (const raw of o.walks) {
      const bump = bumpOf(raw, c, clear);
      if (bump === null) continue;
      legs = cutPair(cutPair(legs, bump[0] as Pt), bump[bump.length - 1] as Pt);
      for (let j = 1; j < bump.length; j++) legs.push([bump[j - 1] as Pt, bump[j] as Pt]);
      bumped.push(raw);
    }
    const hits: Pt[] = [];
    const next: [Pt, Pt][] = [];
    for (const [a, b] of legs) {
      // far from the ring: as it is
      if (Math.min(a.x, b.x) > c.x + RING || Math.max(a.x, b.x) < c.x - RING || Math.min(a.z, b.z) > c.z + RING || Math.max(a.z, b.z) < c.z - RING) {
        next.push([a, b]);
        continue;
      }
      const fs = ringCuts(a, b, c);
      let prev = a;
      for (const f of fs) {
        const p = { x: a.x + (b.x - a.x) * f, z: a.z + (b.z - a.z) * f };
        hits.push(p);
        if (!near(prev, p) && !near(p, b)) {
          next.push([prev, p]);
          prev = p;
        }
      }
      if (!near(prev, b)) next.push([prev, b]);
    }
    legs = next;
    // the ring: an arc between each two points next to each other on it, kept when clear
    const ang = uniq(hits).map((p) => ({ p, a: Math.atan2(p.z - c.z, p.x - c.x) })).sort((x, y) => x.a - y.a);
    if (ang.length < 2) continue;
    for (let i = 0; i < ang.length; i++) {
      const u = ang[i] as { p: Pt; a: number };
      const v = ang[(i + 1) % ang.length] as { p: Pt; a: number };
      // a way that has its swerve needs no arc across it (the arc's corners)
      if (bumped.some(([a, b]) => onLeg(a, b, u.p) && onLeg(a, b, v.p))) continue;
      let span = v.a - u.a;
      if (span <= 0) span += 2 * Math.PI;
      const n = Math.max(2, Math.ceil(span / (Math.PI / 8)));
      // the arc's corners just outside the ring, so its chords stay on it or past it
      const r = RING / Math.cos(span / n / 2);
      const arc: Pt[] = [u.p];
      for (let j = 1; j < n; j++) {
        const a = u.a + (span * j) / n;
        arc.push({ x: c.x + Math.cos(a) * r, z: c.z + Math.sin(a) * r });
      }
      arc.push(v.p);
      const pieces: [Pt, Pt][] = [];
      for (let j = 1; j < arc.length; j++) pieces.push([arc[j - 1] as Pt, arc[j] as Pt]);
      const inside = arc.every((p) => p.x > floor.x0 && p.x < floor.x1 && p.z > floor.z0);
      if (inside && pieces.every(([a, b]) => !o.furniture.some((w) => crosses(a, b, w)))) legs.push(...pieces);
    }
  }
  // the nodes and their legs
  const nodes: Pt[] = [];
  const idx = new Map<string, number>();
  const nodeOf = (p: Pt): number => {
    const k = key(p);
    let i = idx.get(k);
    if (i === undefined) {
      i = nodes.length;
      nodes.push(p);
      idx.set(k, i);
    }
    return i;
  };
  const adj: { to: number; w: number }[][] = [];
  const start = nodeOf(from);
  const goal = nodeOf(to);
  for (const [a, b] of legs) {
    const u = nodeOf(a);
    const v = nodeOf(b);
    if (u === v) continue;
    const w = Math.hypot(a.x - b.x, a.z - b.z);
    (adj[u] ??= []).push({ to: v, w });
    (adj[v] ??= []).push({ to: u, w });
  }
  // off the network (on a ring, where a walk was cut short): a straight leg to the nearest junction it sees past the furniture
  for (const end of [start, goal]) {
    if ((adj[end] ?? []).length > 0) continue;
    const p = nodes[end] as Pt;
    let best = -1;
    let bd = Infinity;
    nodes.forEach((q, i) => {
      const d = Math.hypot(q.x - p.x, q.z - p.z);
      if (i !== end && (adj[i] ?? []).length > 0 && d < bd && !walls.some((w) => crosses(p, q, w))) {
        bd = d;
        best = i;
      }
    });
    if (best >= 0) {
      (adj[end] ??= []).push({ to: best, w: bd });
      (adj[best] ??= []).push({ to: end, w: bd });
    }
  }
  return { nodes, adj, start, goal };
}

/** a network leg and the furniture it goes through (the bed or deck chair of the place at its end) */
interface Base {
  readonly a: Pt;
  readonly b: Pt;
  readonly boxes: readonly Box[];
}

const bases = new WeakMap<Office, readonly Base[]>();

/** the office's network, each leg with what it goes through (once per office) */
function baseOf(o: Office): readonly Base[] {
  let out = bases.get(o);
  if (!out) {
    out = network(o).map(([a, b]) => ({ a, b, boxes: o.furniture.filter((w) => crosses(a, b, w)) }));
    bases.set(o, out);
  }
  return out;
}

/**
 * A swerve round a bim standing at c by the straight way a-b: off the way, round c past the ring
 * and back, smooth (a sin² profile: no corner where it leaves or joins), on the side away from c
 * (the other if that one is blocked), all of it on the way's length. Null: c is not by the way,
 * or no swerve fits.
 */
function bumpOf([a, b]: readonly [Pt, Pt], c: Pt, clear: (pts: readonly Pt[]) => boolean): Pt[] | null {
  const len = Math.hypot(b.x - a.x, b.z - a.z);
  if (len < 1e-6) return null;
  const u = { x: (b.x - a.x) / len, z: (b.z - a.z) / len };
  const along = (c.x - a.x) * u.x + (c.z - a.z) * u.z;
  const side = (c.x - a.x) * -u.z + (c.z - a.z) * u.x;
  if (Math.abs(side) >= RING || along <= 0 || along >= len) return null;
  const n = { x: -u.z, z: u.x };
  for (const sign of side > 0 ? [-1, 1] : [1, -1]) {
    // the peak's offset: past the ring by a hair, from the side c is on
    const y = RING + 0.02 + sign * side;
    for (let half = Math.max(1.2, 2.2 * y); half <= 4 * y + 1.2; half *= 1.25) {
      if (along - half < 0 || along + half > len) break;
      const pts: Pt[] = [];
      const N = 12;
      for (let j = 0; j <= N; j++) {
        const s = -half + (2 * half * j) / N;
        const off = sign * y * Math.cos((Math.PI * s) / (2 * half)) ** 2;
        pts.push({ x: a.x + u.x * (along + s) + n.x * off, z: a.z + u.z * (along + s) + n.z * off });
      }
      if (!pts.every((p) => Math.hypot(p.x - c.x, p.z - c.z) >= RING - 1e-3)) continue;
      if (clear(pts)) return pts;
      break;
    }
  }
  return null;
}

/** the legs with p a junction (the one it lies on cut there) */
function cutPair(legs: readonly [Pt, Pt][], p: Pt): [Pt, Pt][] {
  const i = legs.findIndex(([a, b]) => !near(a, p) && !near(b, p) && onLeg(a, b, p));
  const l = legs[i];
  if (!l) return [...legs];
  return [...legs.slice(0, i), [l[0], p], [p, l[1]], ...legs.slice(i + 1)];
}

/** the legs with `p` a junction (the leg it lies on cut there, each half with what it goes through) */
function cutAt(legs: readonly Base[], p: Pt): readonly Base[] {
  const i = legs.findIndex((l) => !near(l.a, p) && !near(l.b, p) && onLeg(l.a, l.b, p));
  const l = legs[i];
  if (!l) return legs;
  const half = (a: Pt, b: Pt): Base => ({ a, b, boxes: l.boxes.filter((w) => crosses(a, b, w)) });
  return [...legs.slice(0, i), half(l.a, p), half(p, l.b), ...legs.slice(i + 1)];
}

/** where the leg a-b meets the ring round c, its ends included (fractions along it, in order) */
function ringCuts(a: Pt, b: Pt, c: Pt): number[] {
  const dx = b.x - a.x;
  const dz = b.z - a.z;
  const fx = a.x - c.x;
  const fz = a.z - c.z;
  const A = dx * dx + dz * dz;
  const B = 2 * (fx * dx + fz * dz);
  const C = fx * fx + fz * fz - RING * RING;
  const disc = B * B - 4 * A * C;
  if (A < 1e-12 || disc <= 0) return [];
  const s = Math.sqrt(disc);
  return [(-B - s) / (2 * A), (-B + s) / (2 * A)].filter((f) => f > -1e-6 && f < 1 + 1e-6).map((f) => Math.min(1, Math.max(0, f)));
}

/** the shortest walk from every node to the nearest of `goals` */
function dijkstra(adj: readonly (readonly { to: number; w: number }[] | undefined)[], n: number, goals: readonly number[]): number[] {
  const dist = new Array<number>(n).fill(Infinity);
  const heap = new Heap((a, b) => (dist[a] ?? 0) - (dist[b] ?? 0) || a - b);
  for (const g of goals) {
    dist[g] = 0;
    heap.push(g);
  }
  const done = new Array<boolean>(n).fill(false);
  for (let u = heap.pop(); u !== undefined; u = heap.pop()) {
    if (done[u]) continue;
    done[u] = true;
    for (const e of adj[u] ?? []) {
      const d = (dist[u] ?? 0) + e.w;
      if (d < (dist[e.to] ?? Infinity) - 1e-9) {
        dist[e.to] = d;
        heap.push(e.to);
      }
    }
  }
  return dist;
}

// ---- tracks

/** where a track stands for good (null: it leaves the map) */
function restOf(k: Track): Pt | null {
  if (k.end !== Infinity) return null;
  const last = k.keys[k.keys.length - 1];
  return last ? { x: last.x, z: last.z } : null;
}

/** where a track is at t (null: not on the map) */
function posAt(k: Track, t: number): Pt | null {
  const first = k.keys[0];
  if (!first || t < first.t || t > k.end) return null;
  for (let i = 1; i < k.keys.length; i++) {
    const a = k.keys[i - 1] as Key;
    const b = k.keys[i] as Key;
    if (t <= b.t) {
      const f = b.t > a.t ? (t - a.t) / (b.t - a.t) : 1;
      return { x: a.x + (b.x - a.x) * f, z: a.z + (b.z - a.z) * f };
    }
  }
  const last = k.keys[k.keys.length - 1] as Key;
  return { x: last.x, z: last.z };
}

/** the floor box a track covers from t on */
function boxOf(k: Track, t: number): Box {
  let x0 = Infinity;
  let x1 = -Infinity;
  let z0 = Infinity;
  let z1 = -Infinity;
  const take = (p: Pt): void => {
    x0 = Math.min(x0, p.x);
    x1 = Math.max(x1, p.x);
    z0 = Math.min(z0, p.z);
    z1 = Math.max(z1, p.z);
  };
  const now = posAt(k, t);
  if (now) take(now);
  for (const p of k.keys) if (p.t >= t) take(p);
  if (x0 === Infinity) for (const p of k.keys) take(p);
  return { x0, x1, z0, z1 };
}

/** the least distance between a track and a bim going straight from a (at ta) to b (at tb) */
function minDist(k: Track, a: Pt, ta: number, b: Pt, tb: number): number {
  const first = k.keys[0];
  if (!first) return Infinity;
  const lo = Math.max(ta, first.t);
  const hi = Math.min(tb, k.end);
  if (hi < lo) return Infinity;
  const cuts = [lo];
  for (const p of k.keys) if (p.t > lo && p.t < hi) cuts.push(p.t);
  cuts.push(hi);
  const me = (t: number): Pt => {
    const f = tb > ta ? (t - ta) / (tb - ta) : 0;
    return { x: a.x + (b.x - a.x) * f, z: a.z + (b.z - a.z) * f };
  };
  let best = Infinity;
  for (let i = 1; i < cuts.length; i++) {
    const s0 = cuts[i - 1] as number;
    const s1 = cuts[i] as number;
    const p0 = posAt(k, s0) as Pt;
    const p1 = posAt(k, s1) as Pt;
    const m0 = me(s0);
    const m1 = me(s1);
    const rx = m0.x - p0.x;
    const rz = m0.z - p0.z;
    const vx = m1.x - p1.x - rx;
    const vz = m1.z - p1.z - rz;
    const vv = vx * vx + vz * vz;
    const f = vv > 1e-12 ? Math.max(0, Math.min(1, -(rx * vx + rz * vz) / vv)) : 0;
    best = Math.min(best, Math.hypot(rx + vx * f, rz + vz * f));
  }
  return best;
}

// ---- small helpers

/** the leg a-b goes through the box (not along or touching its edge) */
function crosses(a: Pt, b: Pt, w: Box): boolean {
  const e = 1e-3;
  let t0 = 0;
  let t1 = 1;
  const dx = b.x - a.x;
  const dz = b.z - a.z;
  const sides: readonly (readonly [number, number])[] = [[-dx, a.x - (w.x0 + e)], [dx, w.x1 - e - a.x], [-dz, a.z - (w.z0 + e)], [dz, w.z1 - e - a.z]];
  for (const [p, q] of sides) {
    if (Math.abs(p) < 1e-12) {
      if (q < 0) return false;
      continue;
    }
    const r = q / p;
    if (p < 0) t0 = Math.max(t0, r);
    else t1 = Math.min(t1, r);
    if (t0 > t1) return false;
  }
  return t1 - t0 > 1e-6;
}

/** p on the office's walk network (a ring's corner is not: nobody stands there) */
function onNetwork(o: Office, p: Pt): boolean {
  return baseOf(o).some(({ a, b }) => near(a, p) || near(b, p) || onLeg(a, b, p));
}

/** p strictly inside the leg a-b */
function onLeg(a: Pt, b: Pt, p: Pt): boolean {
  const dx = b.x - a.x;
  const dz = b.z - a.z;
  const l2 = dx * dx + dz * dz;
  if (l2 < 1e-12) return false;
  const f = ((p.x - a.x) * dx + (p.z - a.z) * dz) / l2;
  if (f <= 0 || f >= 1) return false;
  return Math.abs((p.x - a.x) * dz - (p.z - a.z) * dx) / Math.sqrt(l2) < 1e-4;
}

function uniq(ps: readonly Pt[]): Pt[] {
  const out: Pt[] = [];
  for (const p of ps) if (!out.some((q) => near(p, q))) out.push(p);
  return out;
}

const near = (a: Pt, b: Pt): boolean => Math.hypot(a.x - b.x, a.z - b.z) < 1e-4;
const key = (p: Pt): string => `${String(Math.round(p.x * 1e4))},${String(Math.round(p.z * 1e4))}`;

/** a binary min-heap of indices, by `cmp` */
class Heap {
  private readonly xs: number[] = [];
  constructor(private readonly cmp: (a: number, b: number) => number) {}
  push(x: number): void {
    const xs = this.xs;
    xs.push(x);
    let i = xs.length - 1;
    while (i > 0) {
      const p = (i - 1) >> 1;
      if (this.cmp(xs[i] as number, xs[p] as number) >= 0) break;
      [xs[i], xs[p]] = [xs[p] as number, xs[i] as number];
      i = p;
    }
  }
  pop(): number | undefined {
    const xs = this.xs;
    const top = xs[0];
    const last = xs.pop();
    if (xs.length > 0 && last !== undefined) {
      xs[0] = last;
      let i = 0;
      for (;;) {
        const l = 2 * i + 1;
        const r = l + 1;
        let m = i;
        if (l < xs.length && this.cmp(xs[l] as number, xs[m] as number) < 0) m = l;
        if (r < xs.length && this.cmp(xs[r] as number, xs[m] as number) < 0) m = r;
        if (m === i) break;
        [xs[i], xs[m]] = [xs[m] as number, xs[i] as number];
        i = m;
      }
    }
    return top;
  }
}
