// les bims: each bim's life as segments (the prototype's plan(), made live). `update` takes the
// activities of a snapshot (activity.ts) and appends a segment for each bim whose activity
// changed: it walks from where it is now (a walk cut by a new state restarts from its current
// point: no teleport, spec §1 rule 5) along the network (paths.ts) to its new spot, at the
// spec §3 speed, faster when far (no walk past MAX_WALK_S, §10). A new agent comes in by the
// door; an archived one (gone from the activities) skates off the terrace. His messages (q-planes,
// a BimEvent message from { who: 'you' }) put a paper plane on the target's pose; the agents'
// messages and reports (the hub's hub/activity, spec §6) play a coffee, a visit or a report for a
// few seconds over the row's activity (playEvents). `poseAt` / `names` / `moving` read it
// at any t. On a map with a park, some idle bims skate (skate.ts pastimes, a look over the activity):
// their segment goes to a park spot and carries the pastime the renderer poses; back, the board in
// hand (fromSkate). Pure: the same updates give the same poses. Spots shared by several bims (bean
// bags, island, the coffee corner, whiteboard, deck chairs) go first come first served; past the last one, a bim
// stands beside (spec §3). Every walk keeps clear of the other bims' walks (avoid.ts, bims-avoid):
// it may wait, step aside, go round one standing, stop beside a spot someone stands on, or come in
// by the door a little later; with no clear way at all it goes on with what it did and tries again
// at the next update (a walk that ended beside its spot too), never through anyone.
import type { Built } from './growth';
import { type Activity, type BimEvent, type FunIn, type Office, type Pastime, type Pose, type Pt, QUEUE_GAP, type Segment, type SitAt, type Sits, type Spot, type Timeline } from './model';
import { pastimeSig, pastimeSpot, pastimesOf } from './pastimes';
import { hash } from './activity';
import { chairSpots, NAP_UP, seatedAct, seatWaits, SIT_S, sitWay, visitAt } from './sit';
import { coffeeAtCorner } from './cozy';
import { along, makePath, path } from './paths';
import { BODY_R, type Budget, planOn, posAt, progress, trackOf, UPDATE_BUDGET, walkTime, type Track } from './avoid';

/** tiles per second (spec §3): walk, hurry (an urgent ask), skate (done, leaving), carrying a box (landing) */
export const SPEED = { walk: 2.4, hurry: 3.5, skate: 3.4, carry: 1.8 } as const;
/** no walk lasts longer (spec §10): a far spot is walked faster */
export const MAX_WALK_S = 6;
/** after a walk (or a change in place), the renderer's transition (spec §5): the loop still runs */
export const SETTLE_S = 1.5;
/** his paper plane: the flight from his side of the screen (pose.ts PLANE_FLIGHT, 3 s), then the catch, read, put away, thumbs up (render/catch.ts PLANE_CATCH, 2.2 s) */
export const PLANE_S = 5.2;
/** the segments kept per bim (a pose asked a bit in the past still answers) */
const KEEP = 8;

type Shared = 'lounge' | 'island' | 'coffee' | 'nook' | 'board' | 'terrace';

interface Bim {
  segs: Segment[];
  /** the shared spot it holds ('lounge:3', 'island:+0' beside the island) */
  key: string | null;
  /** archived: it skates off, out of the map at `outAt` */
  leaving: boolean;
  outAt: number;
  /** the shared spot it had last, per kind: back from a snack, the same bean bag */
  pref: Map<Shared, number>;
  /** when his planes left for it */
  planes: number[];
  /** a hub event it plays now (coffee, visit, report), over its row's activity until `until` */
  over: Over | null;
}

/** an event's activity over the row's: `stay` s once there; `until` set when its walk is known */
interface Over {
  act: Activity;
  stay: number;
  until: number;
}

/** seconds at the spot (spec §6): a coffee and a chat, a post-it or a tap, a sheet handed to main */
export const COFFEE_S = 6;
export const VISIT_S = 3;
export const REPORT_S = 3;
/** an event older than this when it reaches the office is not played (a late or replayed one) */
const STALE_S = 10;

/** the kinds a sender is in when it can get up for a message or a report: busy in its turn, never asking or failed */
const CAN_GO: ReadonlySet<Activity['k']> = new Set(['work', 'hot', 'nap', 'snack', 'done']);

/**
 * A film's timeline (the hero, bims-film v13); the app passes none. `enterAtSpot`: a new agent pops
 * at its spot instead of coming in by the door; `spotOf`: a bim's own spot for that activity (the
 * reviewer beside the first crate), null for the office's.
 */
export interface TimelineOpts {
  readonly enterAtSpot?: boolean;
  readonly spotOf?: (name: string, o: Office, act: Activity) => Spot | null;
}

export function newTimeline(opts: TimelineOpts = {}): Timeline {
  const bims = new Map<string, Bim>();
  let office: Office | null = null;
  /**
   * the office has had a bim: until then every update is its first look (everyone at their spot). The
   * app's first rows are often empty (the hub not heard yet): the crew that came next walked in by
   * the door all at once, past the update's walk budget, and most stayed out (bims missing in the
   * side tile until another update; bim-review m_49981)
   */
  let peopled = false;
  /** the hub events already played (an event passed again is not played twice) */
  const seen = new Set<string>();

  const segAt = (b: Bim, t: number): Segment | null => {
    return b.segs.findLast((s) => s.t <= t) ?? null;
  };

  const poseOf = (b: Bim, t: number): Pose | null => {
    if (b.leaving && t >= b.outAt) return null;
    const seg = segAt(b, t);
    if (!seg) return null;
    const walking = t - seg.t < walkTime(seg) - 1e-9;
    const at = walking ? progress(seg, t - seg.t) : null;
    // arrived: where its walk ended (its spot, or beside it when another bim stood there, avoid.ts)
    const end = seg.path.pts.at(-1) ?? seg.spot;
    const p = at ? along(seg.path, at.d) : { x: end.x, z: end.z, dir: seg.spot.face };
    const plane = b.planes.findLast((te) => te <= t && t - te < PLANE_S);
    // the ones before it still on their way (the renderer ends each on its own catch)
    const aloft = plane === undefined ? [] : b.planes.filter((te) => te < plane && t - te < PLANE_S).map((te) => ({ since: t - te }));
    // its sheet, read and laid on its desk: the newest plane past its catch, while the bim never left
    // that desk since it left (render/catch.ts awayOf: its desk or main's; a state change there keeps it)
    const letter = b.planes.findLast((te) => t - te >= PLANE_S && stayed(b, te, t, seg));
    return {
      x: p.x, z: p.z, dir: p.dir, moving: walking, since: t - seg.t, seg,
      // it stops on its walk to let one pass, or it stands beside its spot (avoid.ts): standing, not its activity's pose
      ...(at?.waiting || (!walking && aside(seg)) ? { waiting: true } : {}),
      ...(plane === undefined ? {} : { plane: { since: t - plane } }),
      ...(aloft.length === 0 ? {} : { aloft }),
      ...(letter === undefined ? {} : { letter: t - letter }),
    };
  };

  /** at the same desk from the plane's start `te` to t: every segment since is at that desk spot */
  const stayed = (b: Bim, te: number, t: number, now: Segment): boolean => {
    if (now.act.at !== 'desk' && now.act.at !== 'main') return false;
    const from = b.segs.findLastIndex((s) => s.t <= te);
    if (from < 0) return false;
    return b.segs.slice(from).every((s) => s.t > t || (s.act.at === now.act.at && Math.hypot(s.spot.x - now.spot.x, s.spot.z - now.spot.z) < 1e-6));
  };

  const push = (b: Bim, seg: Segment): void => {
    b.segs.push(seg);
    if (b.segs.length > KEEP) b.segs.splice(0, b.segs.length - KEEP);
  };

  /** every other bim's walk as planned, the one a new walk keeps clear of (avoid.ts) */
  const tracks = (but: Bim): Track[] => {
    const out: Track[] = [];
    for (const b of bims.values()) {
      const seg = b === but ? undefined : b.segs.at(-1);
      if (seg) out.push(trackOf(seg, b.leaving ? b.outAt : Infinity));
    }
    return out;
  };

  return {
    update(acts: ReadonlyMap<string, Activity>, next: Office, t: number, events: readonly BimEvent[], built?: Built, fun?: FunIn): void {
      const first = office === null || !peopled;
      // a room that grew (the loft's rows, its 3 columns) or another map: its world is rebuilt, every bim at its spot in it
      const rebuilt = office !== null && (office.map !== next.map || office.size !== next.size);
      office = next;
      const held = new Map<string, string>();
      for (const [name, b] of bims) if (b.key && !b.leaving) held.set(b.key, name);
      playEvents(bims, acts, events, t, seen);
      for (const b of bims.values()) if (b.over && t >= b.over.until) b.over = null;
      // a map with a park or a gym: some idle bims skate or climb there (pastimes.ts; a look over the activity, never one played over an event)
      const idleNow = new Map([...acts].filter(([name]) => bims.get(name)?.over == null));
      const fun2 = pastimesOf(idleNow, next, t, fun);
      const later: Later[] = [];
      // what this update's searches may expand, all together (avoid.ts)
      const budget: Budget = { left: UPDATE_BUDGET };

      for (const [name, row] of acts) {
        let b = bims.get(name);
        const want = nookFor(name, coffeeFor(bedFor(b?.over?.act ?? row, b?.key ?? null, held, next.lounge.length, built?.beds), next, fun?.stage), next);
        const pt = b?.over ? undefined : fun2.get(name);
        const parkSpot = pt ? pastimeSpot(next, pt) : undefined;
        const last = b?.segs.at(-1);
        // a done bim reclines only on a deck chair (chairFor): at the spot it holds now, then at the one it gets
        let act = chairFor(want, next, last?.spot, built?.chairs);
        // the stage too: a skater rides the park as built now, never one a stage behind (a new piece starts a new line)
        const skate = pt && parkSpot ? pastimeSig(pt) : null;
        let sig = skate ?? sigOf(act);
        const fixed = opts.spotOf?.(name, next, act) ?? parkSpot ?? fixedSpot(next, name, act);
        // a napper at its desk goes to a bean bag built since (bedFor): the same sig, another place
        // one that stopped beside its spot (another bim stood in its way, avoid.ts) goes on to it; one kept out comes in
        if (b && last && !b.leaving && last.sig === sig && (parkSpot !== undefined || last.act.at === act.at) && !rebuilt && (!fixed || near(fixed, last.spot)) && !aside(last) && last.t <= t) continue;
        if (b?.key) held.delete(b.key);
        const shared = fixed ? null : sharedSpot(next, act.at as Shared, held, b?.pref);
        const spot = fixed ?? shared?.spot ?? next.door;
        act = sitsAt(chairFor(want, next, spot, built?.chairs), next, spot);
        sig = skate ?? sigOf(act);
        const key = shared?.key ?? null;
        if (key) held.set(key, name);
        if (b && shared?.ix !== undefined) b.pref.set(act.at as Shared, shared.ix);
        if (!b) {
          b = { segs: [], key, leaving: false, outAt: Infinity, pref: new Map(), planes: [], over: null };
          if (shared?.ix !== undefined) b.pref.set(act.at as Shared, shared.ix);
          bims.set(name, b);
          // the office's first look: everyone at their spot; later, a new agent comes in by the door
          // (a film's may pop at its spot: opts.enterAtSpot)
          const atSpot = first || opts.enterAtSpot === true;
          const from = atSpot ? spot : next.door;
          const [seg, ok] = walk(next, t, act, sig, from, spot, null, atSpot ? null : tracks(b), true, undefined, budget);
          push(b, dress(seg, pt, undefined));
          if (!ok) later.push({ b, act, sig, from, spot, prev: null, hidden: true, pt, last: undefined, left: null });
          continue;
        }
        b.key = key;
        // back before it was out of the map (archived, then not): it stays, unless it finds no way below
        const left = { leaving: b.leaving, outAt: b.outAt };
        b.leaving = false;
        b.outAt = Infinity;
        if (rebuilt && last) {
          // the world moved under it: it stands at its spot in the new room, its segment's clock kept
          push(b, dress({ ...last, act, sig, spot, path: makePath([spot]), t: last.sig === sig ? last.t : t }, pt, undefined));
          continue;
        }
        // still kept out at the door (no clear way in yet, avoid.ts): it tries to come in again
        const out = last !== undefined && last.t > t;
        if (out) b.segs.pop();
        const from = out ? next.door : (poseOf(b, t) ?? spot);
        const was = out ? undefined : last;
        const [raw, ok] = walk(next, t, act, sig, from, spot, was ? was.act.k : null, tracks(b), out, was, budget);
        const seg = dress(raw, pt, was);
        // no clear way: it goes on as it was (the others keep clear of that) and tries again below
        if (ok || out) push(b, seg);
        else Object.assign(b, left);
        if (ok && b.over && b.over.act === act && b.over.until === Infinity) b.over.until = seg.t + walkTime(seg) + b.over.stay;
        if (!ok) later.push({ b, act, sig, from, spot, prev: was ? was.act.k : null, hidden: out, pt, last: was, left });
      }


      // gone from the activities (archived): it skates off the terrace's edge
      for (const [name, b] of bims) {
        if (acts.has(name) || b.leaving) continue;
        const last = b.segs.at(-1);
        if (b.key) held.delete(b.key);
        b.key = null;
        b.leaving = true;
        const act: Activity = { k: 'done', at: 'terrace' };
        const now = rebuilt ? null : poseOf(b, t);
        const [seg, ok] = walk(next, t, act, 'gone', now ?? next.exit, next.exit, last?.act.k ?? null, rebuilt ? null : tracks(b), false, rebuilt ? undefined : last, budget);
        if (ok) push(b, seg);
        b.outAt = ok ? seg.t + walkTime(seg) : Infinity;
        if (!ok) later.push({ b, act, sig: 'gone', from: now ?? next.exit, spot: next.exit, prev: last?.act.k ?? null, hidden: false, pt: undefined, last, left: null });
      }
      // a walk with no clear way when it was planned: planned again once every other bim's walk of this update is known
      // still none: one coming in stays out, one leaving stays (it leaves at its next update), the others go on as they were
      for (const w of later) {
        if (w.hidden) w.b.segs.pop();
        const [raw, ok] = walk(next, t, w.act, w.sig, w.from, w.spot, w.prev, tracks(w.b), w.hidden, w.last, budget);
        const seg = w.sig === 'gone' ? raw : dress(raw, w.pt, w.last);
        if (ok || w.hidden) push(w.b, seg);
        if (ok && w.left) Object.assign(w.b, { leaving: false, outAt: Infinity });
        if (w.sig === 'gone') {
          w.b.leaving = ok;
          w.b.outAt = ok ? seg.t + walkTime(seg) : Infinity;
        }
        if (ok && w.b.over && w.b.over.act === w.act && w.b.over.until === Infinity) w.b.over.until = seg.t + walkTime(seg) + w.b.over.stay;
      }
      if (bims.size > 0) peopled = true;
      chats(bims, next);
      // an event whose walk wasn't made here (the room was rebuilt, the same stretch went on, no clear way): it ends anyway
      for (const b of bims.values()) if (b.over && b.over.until === Infinity) b.over.until = t + MAX_WALK_S + b.over.stay;
      for (const [name, b] of bims) if (b.leaving && b.outAt < t) bims.delete(name);

      for (const e of events) {
        // his message (from { who: 'you' }) is a paper plane to its agent (agents' messages: playEvents)
        if (e.kind !== 'message' || e.from.who !== 'you' || e.to.who !== 'agent') continue;
        const b = bims.get(e.to.name);
        if (!b || b.planes.includes(e.t)) continue;
        b.planes.push(e.t);
        b.planes.sort((x, y) => x - y);
      }
      for (const b of bims.values()) b.planes = b.planes.filter((te, i, all) => te > t - PLANE_S - 60 || i === all.length - 1);
    },

    poseAt(name: string, t: number): Pose | null {
      const b = bims.get(name);
      const pose = b ? poseOf(b, t) : null;
      if (!pose?.waiting) return pose;
      // waiting: it looks at the nearest bim (the one it lets pass, or the one on its spot)
      let near: Pose | null = null;
      let best = Infinity;
      for (const o of bims.values()) {
        const q = o === b ? null : poseOf(o, t);
        const d = q ? Math.hypot(q.x - pose.x, q.z - pose.z) : Infinity;
        if (q && d < best) {
          best = d;
          near = q;
        }
      }
      return near ? { ...pose, look: Math.atan2(near.x - pose.x, near.z - pose.z) } : pose;
    },

    names(t: number): readonly string[] {
      return [...bims].filter(([, b]) => poseOf(b, t) !== null).map(([name]) => name);
    },

    moving(t: number): boolean {
      for (const b of bims.values()) {
        const pose = poseOf(b, t);
        if (!pose) continue;
        if (pose.moving || pose.plane) return true;
        // an event played (a coffee, a visit, a report): the loop runs until the bim is back
        if (b.over) return true;
        const seg = pose.seg;
        if (seg.from !== null && t < seg.t + walkTime(seg) + SETTLE_S) return true;
      }
      return false;
    },
  };
}

/** a segment with its pastime (skate.ts), and whether it walks off the park with its board in hand */
function dress(seg: Segment, pt: Pastime | undefined, last: Segment | undefined): Segment {
  const sit = seg.sit ? keptSit(seg.sit, seg.path.len) : {};
  const plain: Segment = { t: seg.t, act: seg.act, sig: seg.sig, spot: seg.spot, path: seg.path, speed: seg.speed, from: seg.from, ...(seg.waits ? { waits: seg.waits } : {}), ...sit };
  return { ...plain, ...(pt ? { pastime: pt } : {}), ...(!pt && last?.pastime?.kind === 'skate' ? { fromSkate: true } : {}) };
}

/** a sit's walk beats belong to its walk: a bim put at its spot (the world rebuilt) is there at once, standing with its chair out or seated */
function keptSit(sit: Sits, len: number): { sit?: Sits } {
  if (len > 1e-6) return { sit };
  const still = sit.in?.standing === true ? sit.in : sit.still;
  const kept: Sits = { ...(sit.up ? { up: sit.up } : {}), ...(sit.down ? { down: sit.down } : {}), ...(still ? { still } : {}) };
  return Object.keys(kept).length > 0 ? { sit: kept } : {};
}

/** a walk to plan again at the end of the update (walk's arguments) */
interface Later {
  readonly b: Bim;
  readonly act: Activity;
  readonly sig: string;
  readonly from: { x: number; z: number };
  readonly spot: Spot;
  readonly prev: Segment['from'];
  readonly hidden: boolean;
  readonly pt: Pastime | undefined;
  readonly last: Segment | undefined;
  /** it was leaving when it came back: leaving it stays until a way back is found */
  readonly left: { readonly leaving: boolean; readonly outAt: number } | null;
}

/** its walk ended beside its spot, not on it */
function aside(seg: Segment): boolean {
  const end = seg.path.pts.at(-1);
  return end !== undefined && !near(end, seg.spot);
}

/** what makes two activities the same stretch: its kind, its rank in the queue, its trip, the other bim of a message */
function sigOf(a: Activity): string {
  return `${a.k}|${a.rank === undefined ? '' : String(a.rank)}|${a.key === undefined ? '' : String(a.key)}|${a.with ?? ''}${a.standing ? '|stand' : ''}`;
}

/**
 * A walk from `from` (where it stands) to `spot`, at its activity's speed (capped at MAX_WALK_S on
 * the shortest walk), clear of the other bims' walks (`others`, avoid.ts: it may wait, step aside
 * or go round; null: as the crow walks, the world rebuilt or the first look). `hidden`: it comes in
 * by the door and may come in later.
 */
function walk(o: Office, t: number, act: Activity, sig: string, from: { x: number; z: number }, spot: Spot, prev: Segment['from'], others: readonly Track[] | null, hidden: boolean, was: Segment | undefined, budget: Budget): [Segment, boolean] {
  const p = path(o, from, spot);
  const base = sig === 'gone' || act.k === 'done' ? SPEED.skate : act.k === 'landing' ? SPEED.carry : act.urgent ? SPEED.hurry : SPEED.walk;
  const seg: Segment = { t, act, sig, spot, path: p, speed: Math.max(base, p.len / MAX_WALK_S), from: prev };
  // a bim standing still where it was needs no plan; one coming in by the door does (someone may stand there)
  if (others === null || (p.len < 1e-6 && !hidden)) return [seated(o, seg, was), true];
  // its sit's beats are part of the walk the others keep clear of: the get-up before the plan (it
  // plans from beside its chair, once there), the sit-down after it (it plans to beside its chair)
  const sit = seatsOf(o, seg, was);
  const head = sit.out ? p.pts.slice(0, 3) : [];
  const tail = sit.in ? p.pts.slice(-2) : [];
  const headLen = makePath(head).len;
  // up from its bed first (bims-nap: never a pop out of the lie)
  const up = was && was.act.k === 'nap' && was.act.at === 'lounge' && near(from, was.spot) ? NAP_UP : 0;
  const headT = (sit.out ? (sit.out.standing ? 0 : SIT_S.hop) + headLen / seg.speed + SIT_S.back : 0) + up;
  const start = head.at(-1) ?? from;
  const end = sit.in ? (p.pts.at(-3) ?? spot) : spot;
  // beside its chair taken (a visitor stands there): it stays seated and tries again
  if (sit.out && others.some((k) => standsNear(k, start, t, t + headT + 0.5))) return [{ ...seg, t: Infinity, path: makePath([from]) }, false];
  const clear = planOn(o, start, end, seg.speed, t + headT, others, hidden, was && !sit.out ? { track: trackOf(was), speed: was.speed } : null, budget);
  // no clear way: kept out of the door (hidden), else the caller keeps its walk as it was
  if (clear === null) return [{ ...seg, t: Infinity, path: makePath([from]) }, false];
  const { plan, speed } = clear;
  // its way to beside its chair ended beside that (someone stands there): it stands aside, no sit
  const there = plan.t + makePath(plan.pts).len / speed + plan.waits.reduce((n, w) => n + w.s, 0);
  const tailFree = !sit.in || !others.some((k) => tail.some((q) => standsNear(k, q, there, there + SIT_S.pull + SIT_S.hop + 1)));
  const reached = !sit.in || (plan.pts.at(-1) !== undefined && near(plan.pts.at(-1) as Spot, end) && tailFree);
  const pts = [...head, ...plan.pts.slice(head.length > 0 ? 1 : 0), ...(reached ? tail : [])];
  const walked = makePath(pts);
  const waits = [...(up > 0 ? [{ d: 0, s: up }] : []), ...plan.waits.map((w) => ({ d: w.d + headLen, s: w.s }))];
  const planned: Segment = { ...seg, t: sit.out || up > 0 ? t : plan.t, speed, path: walked, ...(waits.length > 0 ? { waits } : {}) };
  const sat = sitWay(walked, sit.in && reached ? { spot, act } : null, sit.out && was ? { spot: was.spot, act: was.act } : null);
  return [sat.in || sat.out ? { ...planned, sit: sat, waits: seatWaits(walked, sat, waits) } : planned, true];
}

/** a track within two bodies of p at some time from t0 to t1 */
function standsNear(k: Track, p: { x: number; z: number }, t0: number, t1: number): boolean {
  for (let u = t0; u <= t1 + 1e-9; u += 0.1) {
    const q = posAt(k, u);
    if (q && Math.hypot(q.x - p.x, q.z - p.z) < 2 * BODY_R) return true;
  }
  return false;
}

/** the sit a walk would play on its plain way (sit.ts): in at a chair spot, out of the chair it sits at */
function seatsOf(o: Office, seg: Segment, was: Segment | undefined): Sits {
  const chairs = chairSpots(o);
  const isChair = (s: Spot): boolean => chairs.some((c) => near(c, s));
  const start = seg.path.pts[0];
  const to = isChair(seg.spot) && atOwnDesk(seg.act) ? { spot: seg.spot, act: seg.act } : null;
  const from = was && start && near(start, was.spot) && isChair(was.spot) && atOwnDesk(was.act) ? { spot: was.spot, act: was.act } : null;
  return to || from ? sitWay(seg.path, to, from) : {};
}

/** at its own desk (or main at its): a visitor or a reporter stands at another's, where a map may put it on the chair's spot (behind: its walkway is the spot) */
const atOwnDesk = (a: Activity): boolean => (a.at === 'desk' || a.at === 'main') && a.k !== 'visit' && a.k !== 'report';

/**
 * A walk planned as the crow walks (the world rebuilt, the first look) or no walk at all, with its
 * chair (sit.ts): at its desk without a walk it gets up (its chair slides out), sits down, or stands
 * there, its chair out; a plain walk plays its sit as waits.
 */
function seated(o: Office, seg: Segment, was: Segment | undefined): Segment {
  const chairs = chairSpots(o);
  const isChair = (s: Spot): boolean => chairs.some((c) => near(c, s));
  const start = seg.path.pts[0];
  if (seg.path.len < 1e-6) {
    if (!isChair(seg.spot) || !atOwnDesk(seg.act) || !start || !near(start, seg.spot)) return seg;
    const at: SitAt = { spot: seg.spot, side: 1, standing: !seatedAct(seg.act) };
    const end = was?.path.pts.at(-1);
    const sat = was && atOwnDesk(was.act) && near(was.spot, seg.spot) && (end === undefined || near(end, seg.spot)) ? seatedAct(was.act) : null;
    if (at.standing) return { ...seg, sit: sat === true ? { up: at } : { still: at } };
    return sat === false ? { ...seg, sit: { down: at } } : seg;
  }
  const sit = seatsOf(o, seg, was);
  if (!sit.in && !sit.out) return seg;
  return { ...seg, sit, waits: seatWaits(seg.path, sit, seg.waits) };
}

/** a spot that belongs to the bim or to its rank (desk, main's desk, the door, the queue); null for a shared one */
function fixedSpot(o: Office, name: string, act: Activity): Spot | null {
  switch (act.at) {
    case 'desk':
      if (act.k === 'visit' && act.with !== undefined) return visitAt(o, o.desks.get(act.with) ?? o.mainDesk);
      return o.desks.get(name) ?? o.door;
    case 'main':
      return act.k === 'report' ? visitAt(o, o.mainDesk) : o.mainDesk;
    case 'door':
      return o.door;
    case 'queue': {
      const rank = Math.max(1, act.rank ?? 1);
      const place = o.queue[rank - 1];
      if (place) return place;
      // past the queue's end: in line along the map's queueMore leg, from its first end
      const [a, b] = o.queueMore;
      const len = Math.hypot(b.x - a.x, b.z - a.z);
      const f = len > 0 ? Math.min(1, (QUEUE_GAP * (rank - o.queue.length)) / len) : 0;
      const x = a.x + (b.x - a.x) * f;
      const z = a.z + (b.z - a.z) * f;
      return { x, z, wz: z, ax: x, face: o.queue.at(-1)?.face ?? Math.PI / 2 };
    }
    default:
      return null;
  }
}

/**
 * A napper with no bean bag free at this stage (`beds`: the lounge spots that have one) naps at its
 * desk, head down: never lying in the air where a bean bag isn't built yet (his bug, bims-fix). The
 * bean bag it holds stays its own. Pure.
 */
function bedFor(act: Activity, mine: string | null, held: ReadonlyMap<string, string>, spots: number, beds: number | undefined): Activity {
  if (act.k !== 'nap' || act.at !== 'lounge') return act;
  const n = Math.min(spots, beds ?? spots);
  const free = (i: number): boolean => { const k = `lounge:${String(i)}`; return k === mine || !held.has(k); };
  for (let i = 0; i < n; i++) if (free(i)) return act;
  return { ...act, at: 'desk' };
}

/**
 * A done bim at a terrace spot with no deck chair built (`chairs`: the terrace spots that have one,
 * growth.ts chairsAt; all when absent), or beside one (every spot taken), stands there with a mug:
 * never reclined in the air (bims-chair, the law in render/deck.test.ts). Pure.
 */
export function chairFor(act: Activity, o: Office, spot: Pt | undefined, chairs: number | undefined): Activity {
  if (act.k !== 'done') return act;
  const i = spot ? o.terrace.findIndex((t) => near(t, spot)) : -1;
  const chair = i >= 0 && i < (chairs ?? o.terrace.length);
  if (chair) return act.standing ? { k: act.k, at: act.at } : act;
  return act.standing ? act : { ...act, standing: true };
}

/** how far two standing done bims turn to each other, radians (~35°, bims m_33802) */
export const CHAT_TURN = 0.6;

/**
 * The standing done bims' looks, once every bim of an update has its spot (bims m_33802: four in a
 * row read as a lineup): two on a pair of neighbouring terrace spots (0-1, 2-3: the same row) turn
 * CHAT_TURN to each other, a chat; a lone one faces the view; one in three sips its mug (its name's
 * hash). Set on its last segment's activity, its sig kept: no walk, the same spot. Pure on `bims`.
 */
function chats(bims: ReadonlyMap<string, Bim>, o: Office): void {
  const at = (s: Segment): number => (s.act.k === 'done' && s.act.standing ? o.terrace.findIndex((t) => near(t, s.spot)) : -1);
  const standing = new Set<number>();
  for (const b of bims.values()) {
    const s = b.segs.at(-1);
    if (s && !b.leaving && at(s) >= 0) standing.add(at(s));
  }
  for (const [name, b] of bims) {
    const s = b.segs.at(-1);
    if (!s || b.leaving || !s.act.standing) continue;
    const i = at(s);
    const mate = i >= 0 && standing.has(i ^ 1) ? o.terrace[i ^ 1] : undefined;
    const me = o.terrace[i];
    const turn = mate && me ? Math.sign(mate.x - me.x) * CHAT_TURN : 0;
    const sip = hash(name) % 3 === 0;
    if ((s.act.turn ?? 0) === turn && (s.act.sip === true) === sip) continue;
    const act: Activity = { k: s.act.k, at: s.act.at, standing: true, ...(turn !== 0 ? { turn } : {}), ...(sip ? { sip: true as const } : {}) };
    b.segs[b.segs.length - 1] = { ...s, act };
  }
}

/**
 * A coffee at the coffee corner while its machine stands there (cozy.ts coffeeAtCorner: stages 0-1,
 * page bims-cozy v2): an idle trip whose drink is coffee (fruit 2) or two bims' coffee; the rest at
 * the island. Pure.
 */
function coffeeFor(act: Activity, o: Office, stage: number | undefined): Activity {
  if (act.at !== 'island') return act;
  const coffee = act.k === 'coffee' || (act.k === 'snack' && act.fruit === 2);
  return coffeeAtCorner(o, stage, coffee) ? { ...act, at: 'coffee' } : act;
}

/**
 * A trip to a corner's nook instead of the island (the workshop's corners, page bims-workshop): on an
 * office with nooks, every other idle trip (its name and trip number) goes to one: the sofa, the drum
 * stool, the maker bench, the lift. Its length and rhythm stay the island's (activity.ts idleLife). Pure.
 */
function nookFor(name: string, act: Activity, o: Office): Activity {
  if (act.k !== 'snack' || act.at !== 'island' || !o.nooks?.length) return act;
  return hash(`${name}#nook#${String(act.key ?? 0)}`) % 2 === 0 ? { ...act, at: 'nook' } : act;
}

/** a trip at a nook sits when its nook is a seat (the sofa, the drum stool); beside a taken one, or at a bench, it stands. Pure. */
function sitsAt(act: Activity, o: Office, spot: Pt): Activity {
  if (act.at !== 'nook') return act;
  const sits = o.nooks?.some((n) => n.sit && near(n, spot)) === true;
  if (sits === (act.sits === true)) return act;
  if (sits) return { ...act, sits: true };
  const rest: { -readonly [K in keyof Activity]: Activity[K] } = { ...act };
  delete rest.sits;
  return rest;
}

/** the first free spot of that kind (its last one if free); all taken: standing beside one; none: null */
function sharedSpot(o: Office, at: Shared, held: ReadonlyMap<string, string>, pref: ReadonlyMap<Shared, number> | undefined): { spot: Spot; key: string; ix?: number } | null {
  const list = at === 'lounge' ? o.lounge : at === 'island' ? o.islandSpots : at === 'coffee' ? (o.cozy?.coffeeSpots ?? o.islandSpots) : at === 'nook' ? (o.nooks ?? o.islandSpots) : at === 'board' ? o.boardSpots : o.terrace;
  const free = (i: number) => !held.has(`${at}:${String(i)}`);
  const mine = pref?.get(at);
  const ix = mine !== undefined && mine < list.length && free(mine) ? mine : list.findIndex((_, i) => free(i));
  const spot = list[ix];
  if (spot) return { spot, key: `${at}:${String(ix)}`, ix };
  let k = 0;
  while (held.has(`${at}:+${String(k)}`)) k++;
  const s = list[k % list.length];
  if (!s) return null;
  return { spot: { x: s.ax, z: s.wz, wz: s.wz, ax: s.ax, face: s.face }, key: `${at}:+${String(k)}` };
}


/**
 * The hub's events (BimEvent, spec §6), each played once: an agent's message to an idle bim = both
 * at the island for a coffee; to a busy one = the sender at its desk (a post-it if queued, a tap if
 * steer); a report = the sender at main's desk with a sheet. A bim asking you, failed or in line
 * stays put (the map never lies); his messages are planes (update). Pure: `seen` is the caller's.
 */
function playEvents(bims: Map<string, Bim>, acts: ReadonlyMap<string, Activity>, events: readonly BimEvent[], t: number, seen: Set<string>): void {
  const free = (name: string): Bim | null => {
    const b = bims.get(name);
    const k = b?.over?.act.k ?? acts.get(name)?.k;
    return b && !b.leaving && b.over === null && k !== undefined && CAN_GO.has(k) ? b : null;
  };
  for (const e of [...events].sort((x, y) => x.t - y.t)) {
    const id = e.kind === 'report' ? `r|${e.agent}|${String(e.t)}` : `m|${JSON.stringify(e.from)}|${JSON.stringify(e.to)}|${String(e.t)}`;
    if (seen.has(id) || e.t > t || t - e.t > STALE_S) continue;
    seen.add(id);
    if (e.kind === 'report') {
      const b = acts.get(e.agent)?.k === 'main' ? null : free(e.agent);
      if (b) b.over = { act: { k: 'report', at: 'main' }, stay: REPORT_S, until: Infinity };
      continue;
    }
    if (e.from.who !== 'agent' || e.to.who !== 'agent' || e.from.name === e.to.name) continue;
    const from = free(e.from.name);
    if (!from || acts.get(e.from.name)?.k === 'main') continue;
    const to = acts.get(e.to.name)?.k;
    const idle = to === 'nap' || to === 'snack';
    const other = idle ? free(e.to.name) : null;
    if (other) {
      from.over = { act: { k: 'coffee', at: 'island', with: e.to.name }, stay: COFFEE_S, until: Infinity };
      other.over = { act: { k: 'coffee', at: 'island', with: e.from.name }, stay: COFFEE_S, until: Infinity };
    } else if (e.mode !== 'unknown' && to !== undefined) {
      from.over = { act: { k: 'visit', at: 'desk', with: e.to.name, mode: e.mode }, stay: VISIT_S, until: Infinity };
    }
  }
  if (seen.size > 500) for (const k of [...seen].slice(0, seen.size - 500)) seen.delete(k);
}

const near = (a: { x: number; z: number }, b: { x: number; z: number }): boolean => Math.hypot(a.x - b.x, a.z - b.z) < 1e-4;
