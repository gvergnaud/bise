// One office's brain (page town-live p1, architect m_28994): what its bims do, out of Office.tsx so
// the minimap and the town drive an office the same way. Rows in (the project's agents, cards,
// places, his map pick, its growth, hub/activity's events), out per frame: the office's plan,
// each bim's pose with its easter egg and halo, the look the renderer draws (night, eggs, growth,
// the unlock moment) and the labels; plus what the still office's clock check compares. It keeps
// the timeline, today's lands, his konami time, his pokes and the unlock moment. No React, no three, no DOM:
// the caller hands it the clock (t, his local time, night, reduced motion) and the forced eggs of
// a check; the look carries his local time's sky (sky.ts skyAt; none for the hero's film). Owner: bims-wire.
import type { Agent, Plugin } from '@/proto';
import type { Activity, BimEvent, BimFrame, Box, Label, Look, NewTimelineFn, ActivitiesFn, LabelsFn, LayoutFn, Office, Snapshot, Timeline } from './model';
import { clockSig, eggLabels, eggsAt, NO_EGGS, noteLands, withForced, type Forced, type Land, type Local } from './eggs';
import { builtAt, NO_GROW, UNLOCK_LABEL, UNLOCK_WALK, unlockOf, type Grow } from './growth';
import { dayOf } from './skate';
import { MAX_WALK_S, SETTLE_S } from './timeline';
import type { MapId } from './maps/ids';
import { skyAt, skyOf, type Sky, type Weather } from './sky';
import { forcedPokes, notePokes, NO_POKES, pokesAt, pokeWake, roomOf, type Click, type PokeLog } from './pokes';
import { uniqueObjects, type PluginObject } from './plugin-objects';
import { NO_CORNERS, objectSpots, type Corners } from './corners';
import { GROWS, NO_NEED, either, grownFor, type Need } from './extensions';
import { standApart, NO_TRIPS, nextTrips, pluginOfUse, howUsed, standSpotsOf, type Trips } from './uses';

/** The pure modules a driver runs on: feed.ts's, or fakes in a test. */
export interface DriverParts {
  readonly activities: ActivitiesFn;
  readonly layoutOf: LayoutFn;
  readonly newTimeline: NewTimelineFn;
  readonly labelsOf: LabelsFn;
  /** false: its idle bims never take up a pastime (the hero's film: no skater, bims-film v8); absent: the app's */
  readonly pastimes?: boolean;
}

/** The clock and the room a frame is drawn in, read by the caller. */
export interface Clock {
  /** s */
  readonly t: number;
  readonly local: Local;
  readonly night: boolean;
  readonly reduced: boolean;
  /** a check's frozen eggs (test mode, window.__biseEggs): only these, never a roll; absent: the scheduler's */
  readonly forced?: Forced | null;
  /** a check's pinned weather (test mode, window.__biseSky); absent: the slot's spell (sky.ts) */
  readonly weather?: Weather;
  /** false: no sky (the theme's fixed light, the hero's film); absent: his local time's */
  readonly sky?: boolean;
}

/** What he points at in this office: the hovered bim, ctrl held, the row under his pointer, the bim the camera follows. */
export interface Attention {
  readonly hover: string | null;
  readonly held: boolean;
  readonly lit: string | null;
  readonly follow: string | null;
}

export const NO_ATTENTION: Attention = { hover: null, held: false, lit: null, follow: null };

/** One frame's brain. */
export interface Step {
  readonly office: Office;
  /** the live agents it was laid for (the renderer's setOffice) and the map he picked (the drawn one is office.map: the loft for a map not ready yet) */
  readonly agents: readonly Agent[];
  readonly want: MapId;
  /** the office's plan is new this frame (another agent, another map): the renderer gets it again */
  readonly reshaped: boolean;
  /** each bim's pose at t, before its egg */
  readonly frames: readonly BimFrame[];
  /** what the renderer draws: the poses with their eggs and halos */
  readonly played: readonly BimFrame[];
  readonly look: Look;
  readonly labels: ReadonlyMap<string, Label>;
  /** whether anything walks at t (timeline.moving) */
  readonly moving: boolean;
  /** s until a poke starts or ends that no frame would see (a delayed one, a word alone); null: none */
  readonly wake: number | null;
  /** his plugins' objects at their spots in this office (corners.ts objectSpots): the same value until the office or the objects change */
  readonly corners: Corners;
  /** the extensions come on this map (extensions.ts: a high-water mark, Office.tsx keeps it per project); null: a map that does not grow */
  readonly grown: { readonly map: MapId; readonly need: Need } | null;
  /** the plugin objects (their keys) a working bim's call uses now, gone to or not (uses.ts): each plays its move while used */
  readonly using: ReadonlySet<string>;
}

export interface OfficeDriver {
  /** a new snapshot, agents or map: the next step lays the office again if its shape changed */
  rows(snap: Snapshot, agents: readonly Agent[], map: MapId): void;
  /** the growth drawn now (its high-water mark); a stage past the last one starts the unlock moment at t */
  grow(g: Grow, t: number): void;
  /** hub/activity: a plane, a visit, a report */
  event(e: BimEvent): void;
  /** his plugins' objects (plugin-objects.ts officeObjects): each at most once (uniqueObjects), the next step places them (corners.ts); no desk moves for them */
  objects(list: readonly PluginObject[]): void;
  /** his plugins rows (the store's usePlugins): a working bim walks to the object of the plugin its call uses (uses.ts); null: none heard, no trips */
  plugins(rows: readonly Plugin[] | null): void;
  /** the extensions this map already had (Office.tsx's kept mark): they come back, and never go */
  seedNeed(map: MapId, kept: Need): void;
  /** he typed the konami code at t */
  konami(t: number): void;
  /** he clicked this bim: its poke (pokes.ts) starts at t */
  poke(name: string, t: number): void;
  /** the frame at this clock; null before any rows */
  step(c: Clock, a: Attention): Step | null;
  /**
   * The office still (the town's far offices, page town-live): the rows heard at c.t, every bim
   * posed STILL_AHEAD s later (its walk done, at its state's spot), reduced motion, no egg, no
   * halo, no unlock moment; the labels are the pills ('?', '✗'). null before any rows.
   */
  still(c: Clock): Step | null;
  /** the still office's check: true when the clock's eggs or an activity changed since the last step (the next step hears the rows again) */
  stale(c: Clock): boolean;
}

/** the sky of a clock (sky.ts): its slot's, a check's pinned weather on its sun; none when the clock says so */
function skyOfClock(c: Clock): Sky | null {
  if (c.sky === false) return null;
  const m = skyAt(c.local);
  return c.weather === undefined ? m.sky : skyOf(m.sun, c.weather);
}
const skyKey = (c: Clock): string => (c.sky === false ? '' : `${String(skyAt(c.local).slot)}|${c.weather ?? ''}`);

/** The office's agents in a stable shape (the live rows: archived ones have left). Pure. */
export const officeKey = (agents: readonly Agent[], map: MapId): string => `${map}|${agents.map((a) => `${a.name}${a.main ? '*' : ''}`).join(',')}`;

/** how far ahead a still office is posed: past a walk (MAX_WALK_S) and its settling */
export const STILL_AHEAD = MAX_WALK_S + SETTLE_S + 1;

export function makeDriver(parts: DriverParts): OfficeDriver {
  const timeline: Timeline = parts.newTimeline();
  let snap: Snapshot | null = null;
  let agents: readonly Agent[] = [];
  let map: MapId | null = null;
  let fresh = true;
  let events: BimEvent[] = [];
  let office: Office | null = null;
  let shape = '';
  let g: Grow = NO_GROW;
  let seen: Grow | null = null;
  let unlock: { stage: number; at: number } | undefined;
  let lands: { day: string; lands: readonly Land[] } = { day: '', lands: [] };
  let konamiAt: number | null = null;
  let clicks: Click[] = [];
  let pokes: PokeLog = NO_POKES;
  let lastActs = '';
  let lastClock = '';
  /** the sky's slot and pin the last step drew (skyKey): a new slot is a frame (its blend) */
  let lastSky = '';
  /** his plugins' objects as last heard, and their spots in the office they were placed in */
  let objs: readonly PluginObject[] = [];
  let corners: Corners = NO_CORNERS;
  let placedIn: { office: Office; objs: readonly PluginObject[] } | null = null;
  /** the office as its plan lays it (before his sketch's extensions), the extensions come (never go: a high-water mark per map), and his objects changed since */
  let laid: Office | null = null;
  let need: Need = NO_NEED;
  let needMap: MapId | null = null;
  let regrow = false;
  /** his plugins rows, the bims' trips to their objects, and the objects in use at the last step */
  let rowsP: readonly Plugin[] | null = null;
  let trips: Trips = NO_TRIPS;
  let using: ReadonlySet<string> = new Set();
  /** each placed object's use spots (uses.ts standSpotsOf), for the corners they were found in */
  let spotsFor: { corners: Corners; spots: Map<string, ReturnType<typeof standSpotsOf>> } | null = null;

  return {
    rows(s, as, m) {
      snap = s;
      agents = as;
      map = m;
      fresh = true;
    },
    grow(next, t) {
      g = next;
      // the first growth drawn is no moment; a stage past the last drawn one is
      const u = unlockOf(seen, next);
      if (u !== null) unlock = { stage: u, at: t };
      seen = next;
      // the timeline hears of it too: a napper at its desk takes a bean bag the stage built (timeline.ts bedFor)
      fresh = true;
    },
    event(e) {
      events.push(e);
      fresh = true;
    },
    objects(list) {
      // each object at most once in an office (plugin-objects.ts uniqueObjects, Gabriel m_42685)
      objs = uniqueObjects(list);
      // a map that grows like his sketch may need an extension for them (extensions.ts)
      if (map !== null && GROWS.has(map)) {
        regrow = true;
        fresh = true;
      }
    },
    plugins(rows) {
      rowsP = rows;
      fresh = true;
    },
    seedNeed(m, kept) {
      if (needMap !== m) {
        need = NO_NEED;
        needMap = m;
      }
      const next = either(need, kept);
      if (next.wing === need.wing && next.garden === need.garden) return;
      need = next;
      regrow = true;
      fresh = true;
    },
    konami(t) {
      konamiAt = t;
    },
    poke(name, t) {
      clicks.push({ name, t });
    },
    step(c, a) {
      const heard = hear(c);
      if (heard === null) return null;
      return pose(heard.office, heard.reshaped, c, a);
    },
    still(c) {
      const heard = hear(c);
      if (heard === null) return null;
      return pose(heard.office, heard.reshaped, { ...c, t: c.t + STILL_AHEAD, reduced: true, forced: { bims: {} } }, NO_ATTENTION, true);
    },
    stale(c) {
      if (snap === null) return false;
      // the clock's eggs (the eclipse, friday 17:00, midnight) start and end without a row change
      // a new sky slot (sky.ts: every 5 min at most) is a frame too: its blend
      const changed = clockSig(c.local, c.night, agents) !== lastClock || skyKey(c) !== lastSky || JSON.stringify([...withUses(parts.activities(snap, c.t), c.t, false)]) !== lastActs;
      // the next step hears the rows again: the timeline walks the bims to their new activities
      if (changed) fresh = true;
      return changed;
    },
  };

  /** the rows since the last frame, heard at c.t: the office laid again if its shape changed, the timeline told */
  function hear(c: Clock): { office: Office; reshaped: boolean } | null {
      if (snap === null || map === null) return null;
      const { t, local, reduced } = c;
      let reshaped = false;
      if (fresh) {
        fresh = false;
        const key = officeKey(agents, map);
        if (office === null || laid === null || key !== shape) {
          shape = key;
          laid = parts.layoutOf(agents, map);
          office = grown(laid);
          reshaped = true;
        } else if (regrow) {
          // his objects changed: the office grows when they need it (a new wing or garden is a new office)
          const next = grown(laid);
          if (next.wing !== office.wing || next.garden !== office.garden) {
            office = next;
            reshaped = true;
          }
        }
        regrow = false;
        const acts = withUses(parts.activities(snap, t), t, true);
        lastActs = JSON.stringify([...acts]);
        // a map with a park: who skates reads the stage, the jobs' high-water mark (never the live count), his day, reduced motion
        const fun = parts.pastimes === false ? undefined : { stage: g.stage, jobs: g.jobs, dow: local.dow, day: dayOf(local.y, local.mo, local.d), reduced };
        timeline.update(acts, office, t, events, builtAt(g.stage), fun);
        events = [];
      }
      return office === null ? null : { office, reshaped };
  }

  /** every bim at c.t, its egg and halo, the look and the labels; `still`: no unlock moment */
  function pose(office: Office, reshaped: boolean, c: Clock, a: Attention, still = false): Step {
      const { t, local, night, reduced } = c;
      const main = new Set(agents.filter((x) => x.main).map((x) => x.name));
      const frames: BimFrame[] = [];
      for (const name of timeline.names(t)) {
        const pose = timeline.poseAt(name, t);
        if (pose !== null) frames.push({ name, main: main.has(name), pose });
      }
      // his easter eggs (eggs.ts): one pass between the poses and the labels; a check's forced ones alone
      lands = noteLands(lands, frames, local);
      const eg = c.forced !== undefined ? withForced(NO_EGGS, c.forced) : eggsAt({ t, frames, agents, cards: snap?.cards.length ?? 0, local, night, reduced, lands: lands.lands, konami: konamiAt, stage: g.stage });
      lastClock = clockSig(local, night, agents);
      lastSky = skyKey(c);
      // his pokes (pokes.ts): a click heard against the bim's state now; a poke beats its egg while it plays
      pokes = notePokes(pokes, clicks, frames, roomOf(office));
      clicks = [];
      const pk = c.forced !== undefined ? forcedPokes(c.forced?.pokes, reduced) : pokesAt(pokes, frames, t, reduced);
      const played = frames.map((f) => {
        const e = pk.get(f.name) ?? eg.bims.get(f.name);
        // its row hovered wins over the follow: the row's ring is the one he looks for
        const lit: BimFrame['lit'] = f.name === a.lit ? 'row' : f.name === a.follow ? 'follow' : undefined;
        return e === undefined ? (lit ? { ...f, lit } : f) : { ...f, egg: e, ...(lit ? { lit } : {}) };
      });
      // the office grows: what it draws, and the unlock moment while it lasts
      const u = still ? undefined : unlock;
      const sky = skyOfClock(c);
      const look: Look = { night, reduced, eggs: eg.office, grow: g, inbox: snap?.cards.length ?? 0, ...(u ? { unlock: u } : {}), ...(sky ? { sky } : {}), ...(using.size > 0 ? { using } : {}) };
      if (u && t - u.at > UNLOCK_WALK + UNLOCK_LABEL) unlock = undefined;
      const words = new Map([...pk].map(([n, x]) => [n, x.label]));
      const labels = parts.labelsOf(played, { t, hover: a.hover, held: a.held, agents, eggs: eggLabels(eg), pokes: words, follow: a.follow, park: office.park });
      const wake = c.forced !== undefined ? null : pokeWake(pokes, frames, t, reduced);
      return { office, agents, want: map ?? office.map, reshaped, frames, played, look, labels, moving: timeline.moving(t), wake, corners: cornersIn(office), grown: GROWS.has(office.map) && needMap === office.map ? { map: office.map, need } : null, using };
  }

  /**
   * The activities with each working bim's trip to the plugin object its call uses (uses.ts, Gabriel
   * m_52113): at that object's use spot, its second one for a second bim (none free: it stays at its
   * desk), the object in `using` either way. A state wins: only a bim at work (main too) goes.
   * `commit`: the trips move on (a step); else only looked at (stale's check).
   */
  function withUses(acts: ReadonlyMap<string, Activity>, t: number, commit: boolean): ReadonlyMap<string, Activity> {
    if (rowsP === null || office === null) return acts;
    const co = cornersIn(office);
    if (spotsFor === null || spotsFor.corners !== co) spotsFor = { corners: co, spots: new Map() };
    const cache = spotsFor.spots;
    const spotsOf = (key: string): ReturnType<typeof standSpotsOf> => {
      let s = cache.get(key);
      if (s === undefined) {
        const p = co.placed.find((q) => q.key === key);
        s = p === undefined || office === null ? [] : standSpotsOf(office, p, co.placed);
        cache.set(key, s);
      }
      return s;
    };
    const calls = new Map<string, string | null>();
    for (const a of agents) {
      const act = acts.get(a.name);
      if (act === undefined || !WORKING.has(act.k)) continue;
      const u = a.doing?.uses;
      calls.set(a.name, u === undefined ? null : pluginOfUse(u, rowsP));
    }
    const next = nextTrips(trips, calls, t, (_bim, key) => spotsOf(key).length > 0);
    const inUse = new Set<string>();
    for (const k of calls.values()) if (k !== null && co.placed.some((q) => q.key === k)) inUse.add(k);
    if (commit) {
      trips = next;
      using = inUse;
    }
    if (next.now.size === 0) return acts;
    const out = new Map(acts);
    // the bims at one object take its spots in the order they came (then by name): one there keeps its spot
    const at = [...next.now].sort(([a, x], [b, y]) => x.began - y.began || (a < b ? -1 : a > b ? 1 : 0));
    // a spot APART from every one taken, at this object or the next one (two objects side by side)
    const taken: { x: number; z: number }[] = [];
    for (const [bim, trip] of at) {
      const act = acts.get(bim);
      const p = co.placed.find((q) => q.key === trip.key);
      const spot = spotsOf(trip.key).find((s) => taken.every((q) => standApart(q, s)));
      if (act === undefined || p === undefined || spot === undefined) continue;
      taken.push(spot);
      out.set(bim, { ...act, at: 'object', use: { key: trip.key, obj: p.obj, spot, how: howUsed(p.obj) } });
    }
    return out;
  }

  /** the office his objects need (extensions.ts): the laid one on a map that does not grow yet; the extensions already come stay */
  function grown(o: Office): Office {
    if (!GROWS.has(o.map)) return o;
    if (needMap !== o.map) {
      need = NO_NEED;
      needMap = o.map;
    }
    const prev = placedIn !== null && placedIn.office.map === o.map ? corners.placed : [];
    const r = grownFor(o, objs, prev, need);
    need = r.need;
    // the same extensions as the office on screen: that office (its wing and garden kept by identity)
    if (office !== null && office.map === o.map && laid === o && sameExt(office.wing, r.office.wing) && sameExt(office.garden, r.office.garden)) return office;
    return r.office;
  }

  /** the objects' spots in this office: placed again only when the office or the objects changed, each kept where it still fits */
  function cornersIn(office: Office): Corners {
    if (placedIn === null || placedIn.office !== office || placedIn.objs !== objs) {
      // another map starts afresh; the same map keeps what still fits
      const prev = placedIn !== null && placedIn.office.map === office.map ? corners.placed : [];
      corners = objectSpots(office, objs, prev);
      // the spots kept from before can leave one out that a fresh placing fits (objects heard one by
      // one: his m_49375): every object drawn beats each keeping its spot
      if (corners.left.length > 0 && prev.length > 0) {
        const anew = objectSpots(office, objs);
        if (anew.placed.length > corners.placed.length) corners = anew;
      }
      placedIn = { office, objs };
    }
    return corners;
  }
}

/** the activities of a bim at work: it may walk to the object its call uses (main too) */
const WORKING: ReadonlySet<Activity['k']> = new Set(['work', 'hot', 'main']);

const sameExt = (a: Box | undefined, b: Box | undefined): boolean =>
  a === b || (a !== undefined && b !== undefined && a.x0 === b.x0 && a.x1 === b.x1 && a.z0 === b.z0 && a.z1 === b.z1);
