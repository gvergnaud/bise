// les bims (page bims-office v7, spec artifact les-bims-sc-narios-du-bureau): the right bar's AGENTS
// section drawn as an isometric office, one bim per agent row. This file is the contract between
// the pure parts (activity.ts, maps/ (each map's plan), paths.ts, timeline.ts, labels.ts: no three, no DOM, unit
// tests on fake rows) and the effectful ones (render/: three.js; Office.tsx: the canvas, the label
// overlay, the loop). Time is real time in seconds (`t`), read by the shell and passed in.
// It reads the rows (the spec's [a]) and the hub's two facts for his q-planes / q-data answers
// (bims-facts, architect m_27522): Agent.doing -> Activity.doing, hub/activity -> BimEvent, both
// mapped in the feed only, never from thread text. Owner: bims-lead.
import type { Agent, Card, Place } from '@/proto';
import type { Cam } from './zoom';
import type { BimEgg, OfficeEggs } from './eggs';
import type { Built, Grow } from './growth';
import type { Cozy } from './cozy';
import type { MapId } from './maps/ids';
import type { Sky } from './sky';
import type { Corners } from './corners';

/** The office's maps (maps/ids.ts MAP_IDS); the project's pick, the loft when he never picked one. */
export type { MapId };

/** What the office reads at one moment: the bar's rows for one project, nothing else. */
export interface Snapshot {
  readonly agents: readonly Agent[];
  readonly cards: readonly Card[];
  readonly places: readonly Place[];
}

/**
 * Where a bim is and what it does, from its row alone (activity.ts). `k` is the pose the renderer
 * draws; `at` the spot kind the layout resolves. The spec's §4 table, [a] lines only.
 */
export type ActivityKind =
  | 'work' // working at its desk
  | 'hot' // working, a long turn (turn_ms over HOT_MS)
  | 'sleep' // waiting on a watch (timer, pid, file): asleep at its desk
  | 'waitagent' // waiting on another agent's reply
  | 'nap' // idle: in a bean bag in the lounge
  | 'snack' // idle: office life, a trip to the island (computed ahead, replayable)
  | 'asks' // asks you: standing by its desk, '?' pill
  | 'blocked' // blocked with no card: arms crossed
  | 'failed' // slumped on its desk, '✗' pill
  | 'landing' // waits to land: in main's queue at its rank, carrying a box
  | 'done' // on the terrace
  | 'stopped' // stopped by you: grey at its desk, lamp off
  | 'starting' // came in by the door
  | 'main' // main working at the big desk
  | 'mainidle' // main idle: reads the paper
  // from events (timeline.ts makes them, a few seconds each, then the row's activity again)
  | 'coffee' // a message to an idle bim: both at the island
  | 'visit' // a message to a busy bim: the sender at its desk (post-it or tap, Activity.mode)
  | 'report'; // a report: the sender at main's desk with a sheet

/** his pokes (pokes.ts, page bims-pokes v4): a click on a bim, and it reacts by what it does */
export type PokeKind =
  | 'hi' // stops, turns to you, waves
  | 'boop' // squashes like a squeeze toy (the rare one)
  | 'startle' // asleep at its desk: jumps up, '!', slumps back
  | 'snooze' // asleep, napping: a lazy hand waves you off
  | 'rollover' // napping in a bean bag: rolls onto its side and back
  | 'shoo' // working: flaps a hand at you, eyes on the screen
  | 'turbo' // working: types ten times faster, sweats
  | 'spin' // working (not hot): one full spin on its chair
  | 'bite' // eating: holds out its fruit
  | 'hiccup' // eating: three hiccup hops
  | 'meme' // waits on you: both arms up, jumps
  | 'watch' // waits on you, blocked: taps its wrist
  | 'shrug' // stopped, blocked
  | 'fine' // failed: one arm rises bravely under its cloud
  | 'brave' // failed: sniffs, the cloud shrinks, a small smile
  | 'peek' // done on the terrace: lifts its sunglasses
  | 'cheers' // done on the terrace: raises a mug
  | 'juggle' // in the land queue: the box flies up, caught
  | 'windmill' // skating: arms windmilling, saves it
  | 'enough'; // his 3rd poke in 2.5 s: arms crossed, turns its back

/** what a poke plays: its reaction, or under reduced motion its word alone */
export type PlayKind = PokeKind | 'word';

export type SpotKind = 'desk' | 'main' | 'lounge' | 'island' | 'coffee' | 'nook' | 'board' | 'queue' | 'door' | 'terrace';

export interface Activity {
  readonly k: ActivityKind;
  readonly at: SpotKind;
  /** the land queue's 1-based rank ('landing') */
  readonly rank?: number;
  /** the agent it waits on ('waitagent') */
  readonly on?: string;
  /** an office-life trip's number, so two trips in a row are two segments ('snack') */
  readonly key?: number;
  /** 0 pineapple, 1 orange, 2 coffee ('snack') */
  readonly fruit?: number;
  /** a card holds running work (asks, urgent: jumps faster, holds a sheet) */
  readonly urgent?: boolean;
  /** what its turn does now (q-data, the hub's Agent.doing, bims-facts): working poses follow it (spec §4 [t]) */
  readonly doing?: Doing;
  /** the other bim of a message ('coffee', 'visit') */
  readonly with?: string;
  /** how the message went ('visit': queued = a post-it on its desk, steer = a tap on the shoulder) */
  readonly mode?: 'steer' | 'queued';
  /** ('done') no deck chair at its terrace spot (timeline.ts chairFor): it stands there, a mug in hand, never reclined in the air */
  readonly standing?: true;
  /** (standing 'done') how far it turns from its spot's way to a standing neighbour, radians (timeline.ts chats): a chat */
  readonly turn?: number;
  /** (standing 'done') it sips its mug, the mug at its mouth */
  readonly sip?: true;
  /** ('snack' at a nook) it sits there (the sofa, the drum stool); absent: it stands, thinking (the bench, the lift, beside a taken nook) */
  readonly sits?: true;
}

/** The hub's Agent.doing kinds (architect m_27522, bims-facts m_27638); absent = nothing running. */
export type Doing = 'think' | 'tool' | 'edit' | 'search' | 'computer' | 'image' | 'publish' | 'compact' | 'build' | 'unknown';

/** Who a message is from or to: typed, never a name compared with 'you'. */
export type Party = { readonly who: 'you' } | { readonly who: 'agent'; readonly name: string };

/**
 * What happened between two snapshots that the rows don't carry, from the hub's hub/activity
 * notification (bims-facts) mapped in one place (the feed). His message is a message from
 * { who: 'you' }: the paper plane. A message to an idle bim = coffee at the island; queued = a
 * post-it; steer = a tap; a report = a sheet to main's desk. The timeline turns them into short
 * segments ('coffee', 'visit', 'report') and planes; it never invents one.
 */
export type BimEvent =
  | {
      readonly kind: 'message';
      readonly from: Party;
      readonly to: Party;
      readonly mode: 'steer' | 'queued' | 'unknown';
      readonly expectsReply: boolean;
      readonly t: number;
    }
  | { readonly kind: 'report'; readonly agent: string; readonly report: 'progress' | 'done' | 'failed' | 'blocked'; readonly t: number };

/** A point on the floor, in office units (1 = one floor tile); x right, z towards the viewer. */
export interface Pt {
  readonly x: number;
  readonly z: number;
}

/**
 * A place a bim stands or sits: the point, where it joins the walkway (`wz`) and its aisle (`ax`),
 * and the way it faces there (radians, three's y rotation).
 */
export interface Spot extends Pt {
  readonly wz: number;
  readonly ax: number;
  readonly face: number;
}

/**
 * A place in a workshop corner an idle bim hangs at on a trip (maps/workshop-floor.ts, timeline.ts
 * nookFor): seated (the sofa, the drum stool) or standing, thinking (the maker bench, the lift).
 */
export interface Nook extends Spot {
  readonly sit: boolean;
}

/** A piece of furniture's floor box; `of`: the spot it belongs to (a bim sits in its bean bag). */
export interface Box {
  readonly x0: number;
  readonly x1: number;
  readonly z0: number;
  readonly z1: number;
  readonly of?: Pt;
}

/** a drawn piece's floor box; `y0`: how high up it starts (absent: on the floor) */
export type Prop = Box & { readonly y0?: number };

/** A straight leg of a walk graph, between two points. */
export type Leg = readonly [Pt, Pt];

/**
 * The office for N agents on one map (its plan, maps/<map>.ts): pure numbers the renderer builds the
 * world from and the walks follow. Every map gives the same places (desks, main's desk and its land
 * queue, the eating place and its spots, the nap place, the board, the door, the terrace, the way
 * out) and its own walk graph; maps/contract.test.ts checks every plan against this contract.
 */
export interface Office {
  /** the map its plan drew */
  readonly map: MapId;
  /** what places the desks (the loft: its columns and rows): when it changes the world is rebuilt and every bim stands at its spot again */
  readonly size: string;
  /** floor size and its left edge (x right, z towards the viewer, 0 at the back wall) */
  readonly W: number;
  readonly D: number;
  readonly x0: number;
  /** the eating place's floor box (the loft's kitchen island, a riad's fountain): an egg puts its cake on it */
  readonly island: Box;
  /** one desk per non-main agent, by name, in start order; never moves while the agent lives */
  readonly desks: ReadonlyMap<string, Spot>;
  readonly mainDesk: Spot;
  readonly islandSpots: readonly Spot[];
  readonly lounge: readonly Spot[];
  readonly boardSpots: readonly Spot[];
  readonly queue: readonly Spot[];
  readonly door: Spot;
  readonly terrace: readonly Spot[];
  /** where a leaving (archived) bim walks to, out of the map */
  readonly exit: Spot;
  /** the land queue past its last place: bims wait along this leg of `walks`, from its first end, QUEUE_GAP apart */
  readonly queueMore: Leg;
  /**
   * The plan's walk graph: straight legs a bim walks along. Each place above is an end of a leg,
   * the graph is connected, and no leg crosses `furniture` but the box of a place at its end
   * (paths.ts splits it at its junctions and finds the walks).
   */
  readonly walks: readonly Leg[];
  /** what a walk never crosses, where the look draws it: desks, main's desk, the eating place, each bed and deck chair */
  readonly furniture: readonly Box[];
  /** a skatepark (the California map): where idle bims skate (skate.ts); absent on a map without one */
  readonly park?: Park;
  /** a bouldering wall (the climbing gym map): where idle bims climb (climb.ts); absent on a map without one */
  readonly gym?: Gym;
  /** how high its building stands past the common 2.5 (the gym's hall): the camera's fit takes it in (render/camera.ts siteOf); absent: 2.5 */
  readonly height?: number;
  /**
   * His sketch's extensions (extensions.ts, bims-plugins p5), when his plugins' objects ran out of
   * room: `wing` the office going on past its right side, open to it, aligned on the back wall
   * (office floor, bims walk there, no desk); `garden` a garden in front of the terrace, aligned
   * on the left, a stone path to it from the door. Absent: the square alone.
   */
  readonly wing?: Box;
  readonly garden?: Box;
  /** its cozy pieces (cozy.ts placeCozy, maps/index.ts layoutOf): the coffee corner's spot is on `walks`; absent: a bare room */
  readonly cozy?: Cozy;
  /** where it hangs night lamps past the common pools (the workshop's pods): render/world.ts poolsOf lights them, within the town's cap there; absent: none */
  readonly lamps?: readonly Pt[];
  /** the nooks of its corners (the workshop's: a sofa, a drum stool, a bench), places of the walk graph an idle trip may go to instead of the island; absent: none */
  readonly nooks?: readonly Nook[];
  /** a new project's welcome (ghosts.ts): the free desks drawn as dashed outlines, no desk built there; absent: none */
  readonly ghosts?: readonly Spot[];
  /** the inbox stand's middle on the floor (inbox.ts, just inside the door); absent: none fits */
  readonly inbox?: Pt;
  /**
   * The floor boxes of what its look draws past the furniture and the cozy pieces (a lamppost, a
   * street tree, a bench, a hanging pot's reach): the map's Plan.props, the very numbers its look
   * draws them at. His plugins' objects never stand on one (corners.ts, render/collide.test.ts).
   * `y0`: it is that high up (a peg on the wall, a hanging pot): a thing lower than that may stand under it.
   */
  readonly props?: readonly Prop[];
}

/** A straight piece to grind or ride across, along x: a curb, a ledge, a rail, a flat lane (h 0). */
export interface Bar {
  readonly x0: number;
  readonly x1: number;
  readonly z: number;
  readonly h: number;
}

/**
 * A skatepark's numbers (pure, office units), what the skate poses ride on (render/skate.ts) and the
 * look draws (render/maps/california.ts). Every piece is here whatever the stage; the stage says
 * which are built (skate.ts FEATURE_STAGE). The spots are places of the walk graph like any other.
 */
export interface Park {
  /** the slab */
  readonly area: Box;
  /** where a skater rests between two lines, on its board ([0], [1]); the rest: the pool session's line-up */
  readonly spots: readonly Spot[];
  readonly flat: Bar;
  readonly curb: Bar;
  readonly rail: Bar;
  readonly ledge: Bar;
  /**
   * the 4-stair: its deck's top (h) from x0 to xTop, the steps down to xBottom, across z0..z1; the
   * handrail at railZ; the bank a skater rides up to the deck: bankX0..xTop, from its foot at bankZ up to z1
   */
  readonly stairs: {
    readonly x0: number; readonly xTop: number; readonly xBottom: number; readonly z0: number; readonly z1: number; readonly h: number; readonly railZ: number;
    readonly bankX0: number; readonly bankZ: number;
  };
  /** the mini ramp, across x: the left lip at xa, the right one at xb, transitions of radius r, z0..z1 wide */
  readonly ramp: { readonly xa: number; readonly xb: number; readonly r: number; readonly z0: number; readonly z1: number };
  /** the lone quarter pipe (the vert from stage 13), facing +z: its lip at zLip, x0..x1 wide; r and r13 its radius before and from 13 */
  readonly qp: { readonly x0: number; readonly x1: number; readonly zLip: number; readonly r: number; readonly r13: number };
  /** the funbox: its flat top's middle, half sizes, its height, each slope's run along x */
  readonly funbox: { readonly x: number; readonly z: number; readonly hw: number; readonly hd: number; readonly h: number; readonly run: number };
  /** the pool bowl, carved into the ground: its middle, half sizes, its corners' radius (seen from above), its depth (its walls' transition radius too) */
  readonly bowl: { readonly x: number; readonly z: number; readonly hw: number; readonly hd: number; readonly rc: number; readonly depth: number };
  /** the benches by the rest spots, and the floodlights' poles: what a skater rides round */
  readonly benches: readonly Box[];
  readonly poles: readonly Pt[];
}

/**
 * A bouldering wall's numbers (pure, office units; climbwall.ts makes its surface, holds and
 * problems from them, render/maps/climb.ts draws them). The wall is one folded surface along a
 * polyline on the floor: each vertex has its way into the room (nx, nz) and how far the surface
 * stands out from it at the wall's middle height and at its top (o1, o2: > 0 overhangs into the
 * room, < 0 leans back as a slab). The mats lie along its foot; the rest spots are places of the
 * walk graph, on the floor past the mats.
 */
export interface Gym {
  /** the hall's floor */
  readonly area: Box;
  readonly line: readonly WallPt[];
  /** the mats' footprints (their top at climbwall.ts MAT_H): nobody walks through one, a climber stands on it */
  readonly mats: readonly Box[];
  /** where a climber rests between two problems ([0], [1]) */
  readonly spots: readonly Spot[];
}

/** a vertex of a climbing wall's foot line (Gym.line) */
export interface WallPt extends Pt {
  readonly nx: number;
  readonly nz: number;
  readonly o1: number;
  readonly o2: number;
}

/** a land queue's places, apart (layout and the queue's overflow) */
export const QUEUE_GAP = 0.5;

/** A walk: its corners and the length of each leg (paths.ts). */
export interface Path {
  readonly pts: readonly Pt[];
  readonly seg: readonly number[];
  readonly len: number;
}

/** A stop on a walk (avoid.ts): at `d` tiles along its path, it stands `s` seconds to let another bim pass. */
export interface Wait {
  readonly d: number;
  readonly s: number;
}

/** One stretch of a bim's life: from `t`, it goes along `path` at `speed` to `spot` and does `act` there. */
export interface Segment {
  readonly t: number;
  readonly act: Activity;
  /** what makes two activities the same stretch (kind, rank, key) */
  readonly sig: string;
  readonly spot: Spot;
  readonly path: Path;
  /** tiles per second (walk 2.4, hurry 3.5, skate 3.4, carrying 1.8) */
  readonly speed: number;
  /** where it stops on the way to let another bim pass (avoid.ts), in order; absent: none */
  readonly waits?: readonly Wait[];
  /** the activity before (the transition the renderer plays, spec §5), null for the first */
  readonly from: ActivityKind | null;
  /** how the map draws an idle bim here (skate.ts): skating in the park; absent: the activity's own pose */
  readonly pastime?: Pastime;
  /** it skated just before: it walks off with its board kicked up into its hand */
  readonly fromSkate?: boolean;
  /**
   * its desk's chair (sit.ts, never through it): it sits down at the walk's end (in) or gets up at
   * its start (out); at its desk without a walk, it gets up to stand (up), sits down (down), or
   * stands there, its chair out (still)
   */
  readonly sit?: Sits;
}

/** What a segment does with its desk's chair (Segment.sit). */
export interface Sits {
  readonly in?: SitAt;
  readonly out?: SitAt;
  readonly up?: SitAt;
  readonly down?: SitAt;
  readonly still?: SitAt;
}

/** A chair a walk sits at or gets up from (sit.ts): its spot, the side the bim steps in from (1 the chair's right), whether it stays standing there. */
export interface SitAt {
  readonly spot: Spot;
  readonly side: 1 | -1;
  readonly standing: boolean;
}

/**
 * An idle bim's pastime on a map with a park (skate.ts, architect m_28726): a look, never a fact,
 * so the activity stays 'nap' or 'snack'. `slot`: its park spot; `pool`: the Sunday pool session
 * (the bowl only); `nine`: it tries the 900 on the vert (the 900th job); `stage`: what is built.
 */
export interface SkatePastime {
  readonly kind: 'skate';
  readonly slot: number;
  readonly pool: boolean;
  readonly nine: boolean;
  readonly stage: number;
}

/**
 * An idle bim climbing on a map with a gym (climb.ts, the climbing gym): a look like skating.
 * `slot`: its rest spot on the gym's floor (Gym.spots); `stage`: what is built (the bell, the volumes).
 */
export interface ClimbPastime {
  readonly kind: 'climb';
  readonly slot: number;
  readonly stage: number;
}

/** what an idle bim does on its map's own place (pastimes.ts: one row per kind) */
export type Pastime = SkatePastime | ClimbPastime;

/** What the pastimes read past the activities (timeline.update): the stage, the jobs' high-water mark, his local day, reduced motion. */
export interface FunIn {
  readonly stage: number;
  readonly jobs: number;
  /** his local date: 0 sunday .. 6 saturday, and the days since 1970 (one Sunday in four) */
  readonly dow: number;
  readonly day: number;
  readonly reduced: boolean;
}

/** Where a bim is at a time t (timeline.ts `poseAt`): the renderer's only input for its place. */
export interface Pose extends Pt {
  /** the way it faces, radians */
  readonly dir: number;
  readonly moving: boolean;
  /** on its walk, it stands still a moment to let another bim pass (avoid.ts) */
  readonly waiting?: boolean;
  /** waiting: the way to the bim it waits for (radians, three's y rotation), its head turns there */
  readonly look?: number;
  /** seconds since its segment started (the renderer's loops and transitions) */
  readonly since: number;
  readonly seg: Segment;
  /**
   * his message flying to it (q-planes): seconds since the plane left his side of the screen (the
   * flight, render/pose.ts PLANE_FLIGHT, then the catch). The app's timeline sets `since` alone; a
   * film's driver (hero-town m_31686) may set the flight's length, its loops (0, 1, 2; unset: 1, the
   * y1 egg 2) and where it leaves (a fraction of the view's width, unset: 0.5, the middle); `glide`:
   * the film's paper plane (render/plane.ts planGlide: tossed, then gliding), its loops ignored
   */
  readonly plane?: PlaneFlight;
  /**
   * his earlier planes to it still on their way when a newer one left (the newest is `plane`): each
   * flies its own flight to its catch, never cut by the next (Gabriel via designer m_32877), oldest first
   */
  readonly aloft?: readonly PlaneFlight[];
  /**
   * seconds since his newest plane to it left, once its catch is over (timeline.ts PLANE_S), when it
   * left in the state the bim still holds: a bim at its desk keeps the read sheet on it (render/catch.ts)
   */
  readonly letter?: number;
}

/** one of his paper planes to a bim (Pose.plane) */
export interface PlaneFlight {
  readonly since: number;
  readonly dur?: number;
  readonly loops?: 0 | 1 | 2;
  readonly startX?: number;
  readonly glide?: boolean;
  /** false: no thumbs up after the catch (an egg follows it: the hero's hop) */
  readonly thumbs?: boolean;
}

/**
 * How the loop runs after a frame (battery, architect m_27495): 'walk' at the display's rate (a walk,
 * a transition, a plane, a fade), 'loop' at LOOP_FPS (only ambient loops: typing, the '?' jumps, a
 * standing bim's breath and weight shifts, render/alive.ts), 'blend' at ~10 fps while the sky
 * changes (sky.ts SKY_FADE_S, 30 s every 5 min at most), 'still' stopped until the next snapshot,
 * hover, size or theme change. Asleep, napping, on a deck chair, failed, blocked or stopped are
 * still poses: no breathing loop (a standing bim never is, bim-alive). 'idle' at 30 fps
 * only while a resting bim plays one of its rare idle moves (render/idle.ts: a sway, a yawn, a turn in
 * its sleep), still between them (the loop wakes for the next one: OfficeRenderer.idleWake).
 */
export type Motion = 'walk' | 'skate' | 'idle' | 'blend' | 'loop' | 'still';
export const LOOP_FPS = 8;

// ---- the pure modules' entry points (each module exports a function of this type under that name)

/** activity.ts `activities`: every live agent's activity at t (archived rows absent). */
export type ActivitiesFn = (snap: Snapshot, t: number) => ReadonlyMap<string, Activity>;

/** maps/index.ts `layoutOf`: the office for these agents (main and the others, start order) on the project's map. */
export type LayoutFn = (agents: readonly Agent[], map?: MapId) => Office;

/** timeline.ts `newTimeline`: one per mounted office, fed at each snapshot change and each frame. */
export interface Timeline {
  /** a new snapshot's activities (or the same ones at a later t): new segments for what changed */
  /**
   * `built`: what the stage has built (growth.ts builtAt; all when absent): past its bean bags a
   * napper naps at its desk, past its deck chairs a done bim stands with a mug.
   * `fun`: what the pastimes read (skate.ts: who skates on a map with a park); absent: nobody skates
   */
  update(acts: ReadonlyMap<string, Activity>, office: Office, t: number, events: readonly BimEvent[], built?: Built, fun?: FunIn): void;
  /** where a bim is at t; null for one the timeline doesn't know (or that has left) */
  poseAt(name: string, t: number): Pose | null;
  /** the bims on the map at t, a leaving one until it's out */
  names(t: number): readonly string[];
  /** whether anything moves at t (a walk, a transition, a plane): the loop stops when false */
  moving(t: number): boolean;
}
export type NewTimelineFn = () => Timeline;

/** labels.ts `labelsOf`: what the overlay shows at t; events are read from the poses' segments (seg.from, seg.t). */
export type LabelsFn = (
  bims: readonly BimFrame[],
  o: {
    readonly t: number;
    readonly hover: string | null;
    readonly held: boolean;
    readonly agents: readonly Agent[];
    /** the eggs' words by bim (eggs.ts eggLabels): shown where no hover or event label is */
    readonly eggs?: ReadonlyMap<string, string>;
    /** his pokes' words by bim while they play (pokes.ts), '' between: over its hover card and its egg's word */
    readonly pokes?: ReadonlyMap<string, string>;
    /** the bim the office follows (focus.ts, the 2.2x zoom): a skater's trick name shows over it alone, never in the normal view */
    readonly follow?: string | null;
    /** the office's park (skate.ts trickNow rides its lines on it) */
    readonly park?: Park;
  },
) => ReadonlyMap<string, Label>;

/** One bim's input to the renderer for a frame. */
export interface BimFrame {
  readonly name: string;
  readonly main: boolean;
  readonly pose: Pose;
  /** the easter egg it plays now (eggs.ts), on top of its pose; absent: none */
  readonly egg?: BimEgg;
  /** a ring on the floor under it (focus.ts, render/halo.ts): 'row' its row hovered (reads at any zoom), 'follow' the bim the office follows (soft) */
  readonly lit?: 'row' | 'follow';
}

/** The look of the moment: the app's theme (day light, night dark), reduced motion (no loops, fades for walks). */
export interface Look {
  readonly night: boolean;
  readonly reduced: boolean;
  /** the office's own easter eggs now (eggs.ts): the eclipse, the ghost, the tree...; absent: none */
  readonly eggs?: OfficeEggs;
  /** what the project's growth draws (growth.ts, the client's high-water mark); absent: a bare room */
  readonly grow?: Grow;
  /** the unlock moment (growth.ts unlockOf): the stage whose piece two bims bring in, and when it began (t) */
  readonly unlock?: { readonly stage: number; readonly at: number };
  /** the sky of his local time and the slot's weather (sky.ts skyAt): the renderer blends to it; absent: the theme's fixed light */
  readonly sky?: Sky;
  /** his drag or pinch is on: a sky blend due now waits until he lets go */
  readonly hold?: boolean;
  /**
   * where his planes leave from on the view, fractions from its top left (the hero's send button),
   * and their length on screen there, css px (the button's dart); absent: the view's bottom edge,
   * big near his eye
   */
  readonly planeFrom?: { readonly x: number; readonly y: number; readonly px?: number };
  /** the film's glides drawn in the layer over the page (TownOpts.planesOver: the hero's planes fly over its chat) */
  readonly planesUp?: boolean;
  /** his open items in this project: the sheets in the inbox's top tray (0..3, render/inbox.ts); absent: none */
  readonly inbox?: number;
}

/**
 * The three.js side (render/, loaded by a dynamic import so the list view never pays for it).
 * Office.tsx owns the canvas and the loop; the renderer only draws and answers where things are.
 */
export interface OfficeRenderer {
  /** the office changed (an agent came or left): rebuild the world */
  setOffice(office: Office, agents: readonly Agent[]): void;
  /** his plugins' objects at their spots (corners.ts objectSpots, the driver's Step.corners); resolves once they are in the scene, true when their own chunk had to load first (the shell draws a frame then) */
  setObjects(c: Corners): Promise<boolean>;
  /**
   * css px; the camera fits the office to the width (never cropped), returns the height it takes.
   * `box`: the canvas is the whole width × maxHeightPx, the office whole and centred in it (a tab
   * that fills its pane: app/panel/Office.tsx)
   */
  setSize(widthPx: number, maxHeightPx: number, box?: boolean): number;
  /** his pinch and pan (zoom.ts): the view inside the fit; FIT is the whole office. The next frame shows it */
  setCam(cam: Cam): void;
  /** draw time t; how the loop must run next (Motion). Reduced motion: never 'loop' */
  frame(t: number, bims: readonly BimFrame[], look: Look): Motion;
  /** s from the last frame's t until a resting bim's next idle move (render/idle.ts): the still office's wake; null: none */
  idleWake(): number | null;
  /** a bim's head on the canvas, css px from its top left (labels and pills go there), and its feet's y (the hover card flips under them); null off view */
  headOf(name: string): { readonly x: number; readonly y: number; readonly feet: number } | null;
  /** where a bim stands even off view (focus.ts): its head on the canvas (css px, maybe outside it) and its middle as fractions of the whole fit (the follow camera aims there); null: not drawn */
  spotOf(name: string): { readonly x: number; readonly y: number; readonly u: number; readonly v: number } | null;
  /** where a stage's piece stands on the canvas, css px (the unlock moment's label); null off view */
  pieceOf(stage: number): { readonly x: number; readonly y: number } | null;
  /** a plugin object's top as drawn on the canvas, css px (its '?' / '✗' pill sits there); null: not drawn or off view */
  objectTopOf(key: string): { readonly x: number; readonly y: number } | null;
  /** a new project's welcome pins (Office pins): the inbox's top tray (render/inbox.ts; main's desk tray or top on a plan without one) or a ghost desk's middle on the floor, css px; null off view */
  pointOf(what: 'tray' | { readonly ghost: number }): { readonly x: number; readonly y: number } | null;
  /** the bim (or its desk) under a canvas point, css px; null on nothing */
  pick(x: number, y: number): string | null;
  /** the plugin object (its key) under a canvas point, css px: the front one of the drawn boxes that hold it (pick.ts pickObjectAt); null on none */
  objectAt(x: number, y: number): string | null;
  /** a plugin object's drawn box on the canvas, css px (its tip goes over, under or beside it: objpills.ts placeObjTip); null: not drawn */
  objectBoxOf(key: string): { readonly left: number; readonly top: number; readonly right: number; readonly bottom: number } | null;
  /** the office as drawn now, copied onto a 2d canvas of the same pixel size (the map's crossfade, /bims) */
  copyTo(target: HTMLCanvasElement): void;
  /**
   * What falls now (sky.ts: rain, snow, 0..1, as drawn at the last frame) and the building's
   * outline on the canvas (css px, convex): the CSS rain falls outside it only (bims m_33030: never
   * inside the room), and the cube's outline it falls inside (never on the page). null: no sky.
   */
  falling(): { readonly rain: number; readonly snow: number; readonly hull: readonly { readonly x: number; readonly y: number }[]; readonly cube: readonly { readonly x: number; readonly y: number }[] } | null;
  /** WebGL lost: the shell falls back to the list with one line */
  onLost(cb: () => void): void;
  dispose(): void;
}

/** render/index.ts's one export, behind `import('./render')`. */
/** `transparent`: the canvas clears to nothing, the page shows under the cube (the welcome, in the thread's column); absent: the theme's page colour (the bar) */
export type MakeRenderer = (canvas: HTMLCanvasElement, o?: { readonly transparent?: boolean }) => OfficeRenderer;

/** What the overlay shows over one bim (labels.ts): nothing at rest, the spec's §2. */
export interface Label {
  readonly name: string;
  /** 'name' while walking or ctrl held, 'event' the 3 s after an event, 'hover' the tip line, 'pill' the '?' / '✗' alone (no name drawn, words ''), 'egg' an easter egg's words alone ('1:47', '!') */
  readonly mode: 'name' | 'event' | 'hover' | 'pill' | 'egg';
  /** the words after the name ('done', '→ dark-mode'); the hover card's state line; '' for a name alone */
  readonly words: string;
  /** the hover card's title, under its state (its row's title); '' but on hover */
  readonly title: string;
  /** '?' pink (asks you) or '✗' red (failed): shown without a name, always */
  readonly pill: 'asks' | 'failed' | null;
}
