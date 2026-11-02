// A bim's body at time t (pure, no three): the per-kind poses of the spec's §4, the §5 transitions,
// his paper plane's catch, and where the bim stands under reduced motion (fades, never a walk).
// bim.ts applies a Rig to the meshes; index.ts sums the Motion. Every still pose reads without
// motion (asleep: eyes closed, head down; idle: sunk in the bean bag; failed: slumped; in line: the
// box). Loops (typing, hot's sweat, the '?' hops, eating at the island) are 'loop' at LOOP_FPS;
// reduced motion has none. restingSince says when a bim came to rest in its still pose: the resting
// bims' rare idle moves (render/idle.ts) start from there.
import type { Activity, ActivityKind, Doing, Motion, Pose, Pt, UseHow } from '../model';
import { walkTime } from '../avoid';
import { NAP_UP, SIT, SIT_S, sitBeat, toWorld } from '../sit';
import { BEAT, PLANE_CATCH, REACH_S, awayOf } from './catch';
import { FLAT, NAP_DZ, SINK_DROP, SLING_LIFT, type NapRest } from './nap';
import { MAIN_K, keysLean, onKeys } from './laptop';

/** rotations in radians: [x, y, z] */
export type Rot = readonly [number, number, number];

export interface Props {
  readonly crate: boolean;
  readonly mug: boolean;
  readonly orange: boolean;
  readonly pine: boolean;
  /** a sheet held in both hands: main's paper, an urgent ask's form to sign */
  readonly sheet: boolean;
  /** that sheet laid flat on the desk before it (main's paper while it catches his plane) */
  readonly sheetDown?: boolean;
  /** the thumb up on its right fist (the catch's thumbs up) */
  readonly thumb?: boolean;
  readonly skate: boolean;
  readonly glasses: boolean;
  /** the grey cloud over the laptop (failed), 0..1 */
  readonly cloud: number;
  /** hot: the sweat and steam loop's phase, null when not hot */
  readonly hot: number | null;
  /** what its turn does (Activity.doing, spec §4 [t]): a marker (think), a newspaper (search), the big
   * screen beside its desk (computer), a magnifier (image), a poster (publish), the shredder (compact) */
  readonly marker: boolean;
  readonly newspaper: boolean;
  readonly bigScreen: boolean;
  readonly magnifier: boolean;
  readonly poster: boolean;
  readonly shredder: number | null;
  /** build: the progress bar over its head, 0..1 */
  readonly progress: number | null;
  /** the eggs' props (render/egg.ts): m2's coin rising over its head 0..1, o5's golden pineapple, m6's box held up */
  readonly coin: number | null;
  readonly golden: boolean;
  readonly crateUp: boolean;
  /** a poke's turbo typing: its sweat's phase, drops flicking off at the temples (no steam); absent: none */
  readonly flick?: number | null;
  /** the box's place [y, z] in its arms (a poke's juggle); absent: its own */
  readonly crateAt?: readonly [number, number];
  /** the fruit in hand eaten so far (snack): 0 whole .. 1 gone */
  readonly eaten: number;
  /** a climber's fall onto the mat (render/climb.ts): its chalk's puff, 0..1 through it; absent: none */
  readonly chalk?: number;
}

export interface Rig {
  /** where the bim's group stands: the pose's point plus an offset (asks steps aside), and its way */
  readonly x: number;
  readonly z: number;
  readonly dir: number;
  /** the body's lift and rotation, the head's rotation */
  readonly y: number;
  readonly body: Rot;
  readonly head: Rot;
  readonly armL: Rot;
  readonly armR: Rot;
  readonly legL: number;
  readonly legR: number;
  /** 1 open, 0.15 closed */
  readonly eyes: number;
  readonly mouth: 'smile' | 'laugh' | 'oh';
  readonly props: Props;
  /** its desk's laptop, 0 off .. 1 lit */
  readonly screen: number;
  /** its desk lamp */
  readonly lamp: boolean;
  /** 0 its colours .. 1 grey (stopped) */
  readonly grey: number;
  /** 0 gone .. 1 shown (reduced motion's fades) */
  readonly opacity: number;
  readonly motion: Motion;
  /** the whole bim off the floor (a skater on a rail, up a ramp; skate.ts); absent: on it */
  readonly lift?: number;
  /** the whole bim tilted along its way: pitch (nose up < 0) and roll (a ramp's wall, a carve) */
  readonly tilt?: readonly [number, number];
  /**
   * a skater's board's own tilt on the surface under it (pitch, roll): its tilt, but on a steep wall
   * the bim stands back from it (render/skate.ts wallRide) and its board keeps the wall's; absent: its tilt
   */
  readonly boardTilt?: readonly [number, number];
  /** its board's place and turn in the bim's frame (a flip, kicked up into the hand); absent: under its feet */
  readonly board?: { readonly x: number; readonly y: number; readonly z: number; readonly rot: Rot };
  /** his pokes (render/poke.ts): the boop's squash (k 1 flat .. -0.25 stretched) about a height
   * (the floor, a seat); the chair spin's chair, rolled back and turned with its bim; how far the
   * poke lifts its head, so its labels and pills stay where the state's pose has them */
  readonly squash?: { readonly k: number; readonly at: number };
  readonly seat?: { readonly back: number; readonly turn: number };
  readonly anchor?: number;
}

export interface RigInput {
  readonly pose: Pose;
  readonly t: number;
  readonly main: boolean;
  /** the bim's own loop phase (palette.outfitOf) */
  readonly phase: number;
  readonly reduced: boolean;
  /** the bed at a lounge spot (render/nap.ts napRests: its shape, the height its napper rests at); absent: the old fixed recline */
  readonly bedAt?: (p: Pt) => NapRest | undefined;
}

export const NO_PROPS: Props = {
  crate: false, mug: false, orange: false, pine: false, sheet: false,
  skate: false, glasses: false, cloud: 0, hot: null,
  marker: false, newspaper: false, bigScreen: false, magnifier: false, poster: false, shredder: null, progress: null,
  coin: null, golden: false, crateUp: false, eaten: 0,
};

const Z: Rot = [0, 0, 0];

/** a pose before placement: the body's parts only */
interface Body {
  readonly y: number;
  readonly body: Rot;
  readonly head: Rot;
  readonly armL: Rot;
  readonly armR: Rot;
  readonly legL: number;
  readonly legR: number;
  readonly eyes: number;
  readonly mouth: Rig['mouth'];
  readonly props: Props;
  readonly screen: number;
  readonly lamp: boolean;
  readonly grey: number;
  /** offset from the spot, in the bim's own frame (x to its left/right, z ahead) */
  readonly dx: number;
  readonly dz: number;
  /** a fixed way to face (the lounge, the camera), else the pose's */
  readonly face: number | null;
}

const STAND: Body = {
  y: 0, body: Z, head: Z, armL: Z, armR: Z, legL: 0, legR: 0, eyes: 1, mouth: 'smile',
  props: NO_PROPS, screen: 0, lamp: true, grey: 0, dx: 0, dz: 0, face: null,
};

const SEATED = { y: -0.05, legL: -1.45, legR: -1.45 } as const;
/** just sat down (sit.ts): seated, its hands on its lap, before its activity's pose */
const SITTING: Body = { ...STAND, ...SEATED };

/** the kinds that type at a lit screen (a loop) */
const TYPING: ReadonlySet<ActivityKind> = new Set(['work', 'hot', 'main']);

/** the loop phase of time t: frozen under reduced motion so a loop pose holds still */
const loopT = (t: number, reduced: boolean): number => (reduced ? 0 : t);

/** what doing kinds loop (a moving hand, a growing bar): typing's siblings; the others are still */
const DOING_LOOPS: ReadonlySet<Doing> = new Set(['think', 'computer', 'compact', 'build', 'edit', 'tool', 'unknown']);

/** typing: how far the body leans in so its hands land on its keys (an agent, main: his longer arms lean less), how high a hand lifts off them at a tap */
const TYPE_LEAN = { agent: keysLean(1, SEATED.y), main: keysLean(MAIN_K, SEATED.y) } as const;
const TAP = 0.018;

/** a working bim (work, hot, main) by what its turn does; none or edit/tool: typing at its desk */
function working(k: ActivityKind, act: Activity, lt: number, ph: number, main: boolean): Body {
  const hot = k === 'hot';
  const base = { ...NO_PROPS, hot: hot ? (lt * 0.9 + ph) % 1 : null };
  const mouth = hot ? ('oh' as const) : ('smile' as const);
  if (act.at === 'object' && act.use !== undefined) return using(act.use.how, lt, ph, mouth, base);
  switch (act.doing) {
    case 'think':
      if (act.at === 'board') {
        // at the whiteboard, writing equations
        return {
          ...STAND, mouth, armL: [-0.3, 0, 0], head: [-0.15, 0, 0],
          armR: [-2.4 + Math.sin(lt * 6 + ph) * 0.2, 0, 0.2 + Math.sin(lt * 3 + ph) * 0.25],
          props: { ...base, marker: true },
        };
      }
      // thinking at its desk: chin on the hand, eyes up
      return { ...STAND, ...SEATED, mouth, armR: [-2.2, 0, -0.5], armL: [-1.2, 0, 0], head: [-0.25, 0.2 + Math.sin(lt * 1.5 + ph) * 0.1, 0], screen: 0.6, props: base };
    case 'search':
      // reads a newspaper at its desk
      return { ...STAND, ...SEATED, mouth, armL: [-1.3, 0, 0.35], armR: [-1.3, 0, -0.35], head: [0.2, 0, 0], screen: 0.5, props: { ...base, newspaper: true } };
    case 'computer':
      // standing by the big screen beside its desk, a mouse in hand
      return {
        ...STAND, dx: -0.75, dz: 0.1, face: 0, mouth, head: [0.05, -0.5, 0],
        armR: [-1.0 + Math.sin(lt * 4 + ph) * 0.08, 0, Math.sin(lt * 3 + ph) * 0.1], armL: Z,
        screen: 0.4, props: { ...base, bigScreen: true },
      };
    case 'image':
      // a magnifier up to its eye
      return { ...STAND, ...SEATED, mouth: 'oh', armR: [-2.0, 0, -0.3], armL: [-1.2, 0, 0], head: [0.1, 0, 0], screen: 0.9, props: { ...base, magnifier: true } };
    case 'publish':
      // a poster held up, on its way to the back wall
      return { ...STAND, mouth: 'laugh', eyes: 0.3, armL: [-2.6, 0, 0.2], armR: [-2.6, 0, -0.2], props: { ...base, poster: true } };
    case 'compact':
      // tidies its desk: the pile of papers goes to the shredder beside it
      return {
        ...STAND, dx: 0.6, face: 0, mouth, armL: [-1.1, 0, 0], armR: [-1.1, 0, 0], head: [0.35, 0, 0],
        screen: 0.4, props: { ...base, shredder: (lt * 0.8 + ph) % 1 },
      };
    case 'build':
      // waits standing in front of its screen, arms crossed, a progress bar over its head
      return {
        ...STAND, dz: -0.15, mouth, armL: [-1.2, 0, 0.9], armR: [-1.2, 0, -0.9], head: [0.1, 0, 0],
        screen: 0.9, props: { ...base, progress: ((lt + ph * 3) % 12) / 12 },
      };
    case 'edit':
    case 'tool':
    case 'unknown':
    case undefined: {
      // the hands on the keys (laptop.ts onKeys), each tapping in turn, the body leaning in a little
      // to reach them, the legs as seated (Gabriel m_49096: they went through the desk's edge)
      const sp = hot ? 22 : 12;
      const s = main ? MAIN_K : 1;
      const lean = main ? TYPE_LEAN.main : TYPE_LEAN.agent;
      const tap = (a: number): number => Math.max(0, Math.sin(lt * sp + ph + a)) * TAP;
      return {
        ...STAND, ...SEATED,
        legL: SEATED.legL - lean,
        legR: SEATED.legR - lean,
        armL: onKeys(-1, s, SEATED.y, lean, tap(0)),
        armR: onKeys(1, s, SEATED.y, lean, tap(Math.PI)),
        head: [0.15 - lean + Math.sin(lt * 2 + ph) * 0.04, Math.sin(lt * 0.7 + ph) * 0.15, 0],
        body: [lean, 0, hot ? Math.sin(lt * 30) * 0.015 : 0],
        mouth,
        screen: 0.9,
        props: base,
      };
    }
  }
}

/**
 * a working bim at the plugin object its call uses (uses.ts), standing and facing it, by how it
 * uses it (bims-plugins m_52119: a few poses, the object's own move does the rest): it types at a
 * machine, reads off a shelf, looks at a globe or a sky, plays a game, tends a plant or a pet
 */
function using(how: UseHow, lt: number, ph: number, mouth: Rig['mouth'], props: Props): Body {
  const s = (f: number, a = 0): number => Math.sin(lt * f + ph + a);
  switch (how) {
    case 'types':
      // both hands on its keys, tapping in turn, head down to them
      return { ...STAND, mouth, armL: [-1.25 + Math.max(0, s(12)) * 0.12, 0, 0.12], armR: [-1.25 + Math.max(0, s(12, Math.PI)) * 0.12, 0, -0.12], head: [0.3, 0, 0], body: [0.04, 0, 0], props };
    case 'reads':
      // a hand up to its shelf, the other at its side, head going along the spines
      return { ...STAND, mouth, armR: [-2.1 + s(1.2) * 0.1, 0, -0.15], armL: [-0.2, 0, 0.1], head: [-0.12, s(0.8) * 0.3, 0], props };
    case 'looks':
      // hands behind its back, chin up, looking about
      return { ...STAND, mouth: 'oh', armL: [0.45, 0, 0.25], armR: [0.45, 0, -0.25], head: [-0.25, s(0.6) * 0.25, 0], props };
    case 'plays':
      // both hands out at it, quick, a bob in its knees
      return { ...STAND, mouth: 'laugh', armL: [-1.4 + s(9) * 0.18, 0, 0.2], armR: [-1.4 + s(9, 1.7) * 0.18, 0, -0.2], body: [0.08, 0, s(4) * 0.04], head: [0.15, 0, 0], y: -Math.abs(s(4)) * 0.02, props };
    case 'tends':
      // leans in, a hand out patting it gently
      return { ...STAND, mouth, armR: [-1.0 + s(3) * 0.15, 0, -0.1], armL: [-0.3, 0, 0.1], body: [0.08, 0, 0], head: [0.3, 0, 0], props };
  }
}

/** whether a working bim's pose loops (typing and its siblings) or holds still (reading, a poster) */
export function workLoops(act: Activity): boolean {
  // at a plugin object it uses, it loops (its pose, and the object's move every USE_EVERY s)
  return act.at === 'object' || act.doing === undefined || DOING_LOOPS.has(act.doing) || act.k === 'hot';
}

/**
 * napping, reclined: sunk in a bean bag, on a hammock's cloth, facing the room, hands folded on its
 * belly (arms out read fallen, and stuck out of the bed: Gabriel, bims-clip; bims m_33943). Its
 * footprint is maps/plan.ts NAP_BED's. With no bed known (a test's bim): at the loft's bean bag.
 */
const napping: Body = {
  ...STAND, y: 0.22, dz: NAP_DZ.sink, body: [-0.9, 0, 0], legL: -0.4, legR: -0.4, armL: [-0.7, 0, 0.75], armR: [-0.7, 0, -0.75],
  eyes: 0.15, mouth: 'oh', face: 0,
};

/**
 * Napping on its bed (bims-nap: they lay in the air, 20 cm over a mat, the back hanging at 52° over a
 * cushion): flat (a mat, a cushion, a cot, a bench), on its back, back and heels on the bed's top,
 * its head up on the pillow and tilted to the room (render/nap.ts FLAT); a bean bag, sunk in it;
 * a hammock, on its cloth.
 */
function nappingOn(r: NapRest | undefined): Body {
  if (!r) return napping;
  if (r.shape === 'sink') return { ...napping, y: r.top - SINK_DROP };
  if (r.shape === 'sling') return { ...napping, y: r.top + SLING_LIFT };
  return {
    ...napping, y: r.top + FLAT.lift, dz: NAP_DZ.flat, body: [FLAT.body, 0, 0], head: [FLAT.head, 0, 0], legL: FLAT.legs, legR: FLAT.legs,
  };
}

/** sat on its flat bed's edge, before it lies back (and after it sits up): its seat on the top, legs out */
function perched(r: NapRest): Body {
  return { ...STAND, ...SEATED, y: r.top - 0.18, dz: NAP_DZ.flat, armL: [-0.5, 0, 0.3], armR: [-0.5, 0, -0.3], face: 0, eyes: 0.5 };
}

/** lying down on its bed (f 0..1, its arrival's beat), or getting up (1..0): on a flat bed it sits on the edge first, never pops into the lie (bims m_35286) */
function lieDown(r: NapRest | undefined, f: number): Body {
  const lie = nappingOn(r);
  const stand: Body = { ...STAND, face: 0, dz: lie.dz };
  if (!r || r.shape !== 'flat') return blend(stand, lie, ease(f));
  const sit = perched(r);
  return f < 0.4 ? blend(stand, sit, ease(f / 0.4)) : blend(sit, lie, ease((f - 0.4) / 0.6));
}

/** lying down and getting up (s): the beat of its arrival on a bed, of its leaving it (sit.ts NAP_UP: the timeline waits there that long) */
export const NAP_DOWN = NAP_UP;

/**
 * asleep at its desk: head down on the arms in front of the laptop, eyes closed, screen off. Leaning
 * less than the head bows: at 0.5 the head went through the lid (Gabriel, bims-fix; laptop.ts
 * headClearance, pose.test.ts)
 */
const asleep: Body = {
  ...STAND, ...SEATED, body: [0.3, 0, 0], head: [0.6, 0, 0], armL: [-1.4, 0, 0], armR: [-1.4, 0, 0],
  eyes: 0.15, mouth: 'oh',
};

/**
 * Eating at the island (Gabriel and Lucas, bims-life: a bim at the bar stays a while and eats): a
 * loop from its arrival (`stay`, s). It reaches into the bowl and picks a fruit (PICK_S), then BITES
 * times: the fruit up to its mouth, a bite (the fruit shrinks), down to its chest, chewing; then the
 * next fruit. A mug (fruit 2) is sipped, never smaller. Reduced motion: chewing, a fruit in hand.
 */
export const PICK_S = 1.6;
export const BITE_S = 2.6;
export const BITES = 4;
export const FRUIT_S = PICK_S + BITES * BITE_S;

const lerp3 = (a: Rot, b: Rot, f: number): Rot => [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f];
const HOLD: Rot = [-1.3, 0, -0.2];
const MOUTH: Rot = [-2.25, 0, -0.45];
const BOWL: Rot = [-0.75, 0, 0.1];

/**
 * An idle trip at a workshop corner's nook (timeline.ts nookFor): on the sofa or the drum stool,
 * seated, leaning back, a foot tapping; at the maker bench or by the lift, standing, chin on the
 * hand, the head turning slowly (think).
 */
function hanging(sits: boolean, lt: number, ph: number): Body {
  if (sits) {
    return {
      ...STAND, ...SEATED, body: [-0.12, 0, 0], armL: [-0.5, 0, 0.25], armR: [-0.5, 0, -0.25],
      head: [-0.05, Math.sin(lt * 0.6 + ph) * 0.2, 0], legR: SEATED.legR + Math.max(0, Math.sin(lt * 4 + ph)) * 0.12, eyes: 0.7,
    };
  }
  return { ...STAND, armR: [-2.2, 0, -0.5], armL: [-1.2, 0, 0], head: [-0.25, 0.2 + Math.sin(lt * 1.5 + ph) * 0.15, 0] };
}

function eating(fruit: number, stay: number, ph: number): Body {
  const n = Math.floor(stay / FRUIT_S);
  const u = stay - n * FRUIT_S;
  const mug = fruit === 2;
  const kind = mug ? 2 : (fruit + n) % 2;
  const held = (eaten: number, on: boolean): Props => ({ ...NO_PROPS, pine: on && kind === 0, orange: on && kind === 1, mug: on && mug, eaten: mug ? 0 : eaten });
  if (u < PICK_S) {
    // into the bowl, eyes on it; the fruit in hand once picked, back up to its chest
    const f = u / PICK_S;
    const down = f < 0.5 ? ease(f * 2) : 1 - ease((f - 0.5) * 2);
    return {
      ...STAND, body: [0.22 * down, 0, 0], head: [0.1 + 0.3 * down, 0, 0],
      armR: f < 0.5 ? lerp3(HOLD, BOWL, ease(f * 2)) : lerp3(BOWL, HOLD, ease((f - 0.5) * 2)),
      armL: [-0.15, 0, 0], mouth: 'smile', props: held(0, f >= 0.5),
    };
  }
  const b = Math.floor((u - PICK_S) / BITE_S);
  const v = u - PICK_S - b * BITE_S;
  const before = b / BITES;
  const after = (b + 1) / BITES;
  if (v < 0.5) {
    // up to its mouth, mouth open
    return { ...STAND, armR: lerp3(HOLD, MOUTH, ease(v / 0.5)), head: [-0.05, 0, 0], mouth: 'oh', props: held(before, true) };
  }
  if (v < 0.75) {
    // the bite: a nod into it, the fruit a bite smaller
    return { ...STAND, armR: MOUTH, head: [0.15, 0, 0], mouth: 'smile', eyes: 0.6, props: held(after, after < 1) };
  }
  if (v < 1.15) {
    return { ...STAND, armR: lerp3(MOUTH, HOLD, ease((v - 0.75) / 0.4)), head: [0.05, 0, 0], mouth: 'smile', props: held(after, after < 1) };
  }
  // chewing: the mouth goes, the head bobs, eyes half shut with pleasure
  const c = Math.sin((v - 1.15) * Math.PI * 6 + ph);
  return {
    ...STAND, armR: HOLD, head: [0.04 + c * 0.04, 0, Math.sin((v - 1.15) * 2 + ph) * 0.05],
    mouth: c > 0 ? 'oh' : 'smile', eyes: 0.4, props: held(after, after < 1),
  };
}

/** a kind's pose at rest, at loop time lt (lt is 0 under reduced motion); `stay`: s since it arrived (its own loops from there) */
function rest(act: Activity, lt: number, ph: number, stay = lt, bed?: NapRest, main = false): Body {
  const k = act.k;
  const urgent = act.urgent === true;
  const fruit = act.fruit ?? 0;
  switch (k) {
    case 'work':
    case 'main':
    case 'hot':
      return working(k, act, lt, ph, main);
    case 'starting':
      // at the door: looks around, a little wave
      return { ...STAND, head: [0, 0.35, 0], armR: [0, 0, 2.4], face: Math.PI };
    case 'nap':
      // no bean bag free at this stage (timeline.ts bedFor): it naps at its desk, as one asleep
      return act.at === 'desk' ? asleep : nappingOn(bed);
    case 'sleep':
    case 'waitagent':
      return asleep;
    case 'snack':
      return act.at === 'nook' ? hanging(act.sits === true, lt, ph) : eating(fruit, stay, ph);
    case 'asks': {
      // standing by its desk, hand up, hops (faster and with a sheet when urgent)
      const sp = urgent ? 5.2 : 2.6;
      return {
        ...STAND, dx: 0.65, dz: 0.15, face: 0,
        y: Math.abs(Math.sin(lt * sp + ph)) * 0.08,
        armR: [0, 0, 2.7 + Math.sin(lt * 5 + ph) * 0.25],
        armL: urgent ? [-1.3, 0, 0] : Z,
        mouth: 'oh', screen: 0.6,
        props: urgent ? { ...NO_PROPS, sheet: true } : NO_PROPS,
      };
    }
    case 'blocked':
      return { ...STAND, ...SEATED, armL: [-1.2, 0, 0.9], armR: [-1.2, 0, -0.9], head: [0.25, 0, 0], mouth: 'oh', screen: 0.4 };
    case 'failed':
      // slumped on its desk, a grey cloud over the laptop
      return {
        ...STAND, ...SEATED, body: [0.35, 0, 0], head: [0.7, 0, 0], armL: [-0.2, 0, 0], armR: [-0.2, 0, 0],
        eyes: 0.3, mouth: 'oh', props: { ...NO_PROPS, cloud: 1 },
      };
    case 'landing':
      return { ...STAND, armL: [-1.3, 0, 0], armR: [-1.3, 0, 0], props: { ...NO_PROPS, crate: true } };
    case 'done':
      // no deck chair at its spot (timeline.ts chairFor): it stands there, a mug in hand, sunglasses;
      // facing the view or turned to a neighbour (rigOf: act.turn), sipping (the mug at its mouth) or laughing
      if (act.standing) {
        const props = { ...NO_PROPS, mug: true, glasses: true };
        if (act.sip) return { ...STAND, armR: MOUTH, head: [-0.12, 0, 0], mouth: 'oh', eyes: 0.6, props };
        return { ...STAND, armR: [-1.5, 0, 0], head: [-0.1, 0, 0.06], mouth: 'laugh', eyes: 0.3, props };
      }
      // on a deck chair (stages.ts deckChair), sunglasses: its seat under it, its back on the
      // backrest's tilt (DECK.tilt), its legs down along the seat
      return {
        ...STAND, y: 0.26, dz: 0.33, body: [-1.0, 0, 0], legL: -0.2, legR: -0.2, armL: [0, 0, -2.6], armR: [0, 0, 2.6],
        props: { ...NO_PROPS, glasses: true },
      };
    case 'stopped':
      return { ...STAND, ...SEATED, armL: [-0.9, 0, 0], armR: [-0.9, 0, 0], head: [0.1, 0, 0], lamp: false, grey: 1 };
    case 'mainidle':
      // reads the paper
      return { ...STAND, ...SEATED, armL: [-1.3, 0, 0.35], armR: [-1.3, 0, -0.35], head: [0.25, 0, 0], props: { ...NO_PROPS, sheet: true } };
    // the hub's events (bims-facts): a coffee at the island, a visit to a busy bim's desk, a report to main
    case 'coffee':
      return { ...STAND, armR: [-1.5, 0, 0], head: [-0.1, 0, 0.06], mouth: 'laugh', eyes: 0.3, props: { ...NO_PROPS, mug: true } };
    case 'visit':
      return { ...STAND, armR: [-1.2, 0, -0.3], head: [0.1, 0, 0] };
    case 'report':
      return { ...STAND, armL: [-1.3, 0, 0], armR: [-1.3, 0, 0], props: { ...NO_PROPS, sheet: true } };
  }
}

/** the §5 transition's length into `to` from `from`, seconds (0: none, a plain blend of 0.6) */
export function transitionOf(from: ActivityKind | null, to: ActivityKind): number {
  if (from === null || from === to) return 0;
  if (to === 'sleep' || to === 'waitagent') return 1.5;
  if (to === 'failed') return 1.2;
  if (to === 'stopped') return 0.5;
  if (from === 'sleep' || from === 'waitagent' || from === 'failed' || from === 'stopped') return 1.0;
  return 0.6;
}

/** his plane: the flight (its zigzag and loop, plane.ts), then the catch (catch.ts: its beats, PLANE_CATCH) */
export const PLANE_FLIGHT = 3.0;

const ease = (x: number): number => (x <= 0 ? 0 : x >= 1 ? 1 : x * x * (3 - 2 * x));
const mix = (a: number, b: number, f: number): number => a + (b - a) * f;
const mixR = (a: Rot, b: Rot, f: number): Rot => [mix(a[0], b[0], f), mix(a[1], b[1], f), mix(a[2], b[2], f)];

function blend(a: Body, b: Body, f: number): Body {
  const half = f >= 0.5;
  return {
    y: mix(a.y, b.y, f), body: mixR(a.body, b.body, f), head: mixR(a.head, b.head, f),
    armL: mixR(a.armL, b.armL, f), armR: mixR(a.armR, b.armR, f), legL: mix(a.legL, b.legL, f), legR: mix(a.legR, b.legR, f),
    eyes: mix(a.eyes, b.eyes, f), mouth: half ? b.mouth : a.mouth,
    props: { ...(half ? b.props : a.props), cloud: mix(a.props.cloud, b.props.cloud, f) },
    screen: mix(a.screen, b.screen, f), lamp: half ? b.lamp : a.lamp, grey: mix(a.grey, b.grey, f),
    dx: mix(a.dx, b.dx, f), dz: mix(a.dz, b.dz, f), face: half ? b.face : a.face,
  };
}

/** walking (or skating, or carrying the box) at time t */
function walking(k: ActivityKind, t: number): Body {
  if (k === 'done') {
    return {
      ...STAND, y: 0.1, legL: 0.2, legR: -0.3, armL: [0, 0, -1.1], armR: [0, 0, 1.1], body: [0, 0.5, 0],
      mouth: 'laugh', eyes: 0.3, props: { ...NO_PROPS, skate: true, glasses: true },
    };
  }
  const w = t * 9;
  const carry = k === 'landing';
  return {
    ...STAND,
    y: Math.abs(Math.sin(w)) * 0.05,
    legL: Math.sin(w) * 0.6, legR: -Math.sin(w) * 0.6,
    armL: [carry ? -1.3 : -Math.sin(w) * 0.5, 0, 0], armR: [carry ? -1.3 : Math.sin(w) * 0.5, 0, 0],
    props: carry ? { ...NO_PROPS, crate: true } : NO_PROPS,
  };
}

/** the special beats on top of a transition's blend (f: 0..1 through it) */
function beat(from: ActivityKind | null, to: ActivityKind, f: number, b: Body): Body {
  const bump = Math.sin(Math.PI * f);
  if ((to === 'sleep' || to === 'waitagent') && f < 0.6) {
    // a yawn and a stretch before the head goes down
    return { ...b, armL: [0, 0, -2.6 * bump], armR: [0, 0, 2.6 * bump], mouth: 'oh', eyes: 0.6 };
  }
  if ((from === 'sleep' || from === 'waitagent') && TYPING.has(to)) {
    // woken with a start: a little jump
    return { ...b, y: b.y + 0.18 * bump, eyes: 1 };
  }
  if (to === 'failed') return { ...b, armL: [-2.4 * bump, 0, 0.4], armR: [-2.4 * bump, 0, -0.4] };
  if (from === 'stopped') return { ...b, armL: [0, 0, -2.2 * bump], armR: [0, 0, 2.2 * bump] };
  return b;
}

/** the right arm up and ahead, open to the plane (catch.ts REACH_S); the arm holding it in; the read's; to the pocket at its hip; laying it on the desk */
const ARM = {
  reach: [-2.3, 0, 0.25] as Rot,
  give: [-2.0, 0, 0.2] as Rot,
  hold: [-1.5, 0, -0.25] as Rot,
  read: [-1.35, 0, -0.35] as Rot,
  pocket: [-0.2, 0, 0] as Rot,
  desk: [-1.0, 0, 0.3] as Rot,
  thumbs: [-1.75, 0, -0.15] as Rot,
} as const;

/**
 * His plane coming into the bim's hand, s seconds after it left (dur: its flight's length): catch.ts's
 * beats. It looks up and raises its arm REACH_S before it lands, the hand gives a little on the
 * impact, brings it in, it reads (head down), puts it away (the desk when it sits at one, else its
 * pocket), a short thumbs up. The plane itself is office-scene's, at the hand: never a prop here.
 */
function caught(b: Body, s: number, dur: number, desk: boolean, thumbs: boolean): Body {
  const c = s - dur;
  // a sheet already in its hands (main's idle paper): laid on the desk as the arm rises, back after the catch (designer m_33640, bims m_33641)
  const paper = b.props.sheet;
  if (c < -REACH_S - (paper ? PAPER_S : 0) || c > PLANE_CATCH) return b;
  const r = catchBody(b, c, desk, thumbs);
  return paper && c < PLANE_CATCH - 0.05 ? { ...r, props: { ...r.props, sheet: false, sheetDown: true } } : r;
}

/** the paper's laying down, before the reach, s */
const PAPER_S = 0.1;

function catchBody(b: Body, c: number, desk: boolean, thumbs: boolean): Body {
  if (c < -REACH_S) return b;
  if (c < 0) {
    const f = ease((c + REACH_S) / REACH_S);
    return { ...blend(b, { ...b, armR: ARM.reach, head: [-0.35, 0, 0] }, f), eyes: 1, mouth: 'oh' };
  }
  const look = { ...b, eyes: 1, mouth: 'smile' as const };
  if (c < BEAT.give) return { ...look, armR: mixR(ARM.reach, ARM.give, ease(c / BEAT.give)), head: [-0.35, 0, 0] };
  if (c < BEAT.bring) {
    const f = ease((c - BEAT.give) / (BEAT.bring - BEAT.give));
    return { ...look, armR: mixR(ARM.give, ARM.hold, f), head: mixR([-0.35, 0, 0], [0.2, 0, 0], f) };
  }
  if (c < BEAT.read) {
    const f = ease((c - BEAT.bring) / 0.2);
    return { ...look, armR: mixR(ARM.hold, ARM.read, f), head: mixR([0.2, 0, 0], [0.35, 0, 0], f) };
  }
  const away = desk ? ARM.desk : ARM.pocket;
  if (c < BEAT.away) {
    const f = ease((c - BEAT.read) / (BEAT.away - BEAT.read));
    return { ...look, armR: mixR(ARM.read, away, f), head: mixR([0.35, 0, 0], b.head, f) };
  }
  // an egg follows (the hero's hop): no thumbs up, straight back to its pose
  if (!thumbs) return blend({ ...b, armR: away }, b, ease((c - BEAT.away) / (PLANE_CATCH - BEAT.away)));
  // the thumbs up, short: the fist up in front at shoulder height, its thumb up (bims m_33641: never an arm pointing sideways), then back to its pose
  if (c < BEAT.thumbs) return { ...b, armR: mixR(away, ARM.thumbs, ease((c - BEAT.away) / 0.12)), mouth: 'laugh', eyes: 0.3, props: { ...b.props, thumb: true } };
  return blend({ ...b, armR: ARM.thumbs }, b, ease((c - BEAT.thumbs) / (PLANE_CATCH - BEAT.thumbs)));
}

/** where the bim stands under reduced motion: no walk, a fade out at the start then in at the end */
export const FADE = 0.8;
export function placeOf(pose: Pose, reduced: boolean): { x: number; z: number; opacity: number; fading: boolean } {
  const { path } = pose.seg;
  if (!reduced || path.len < 0.01 || pose.seg.from === null) return { x: pose.x, z: pose.z, opacity: 1, fading: false };
  const f = Math.min(FADE, path.len / pose.seg.speed);
  const s = pose.since;
  const a = path.pts[0];
  const b = path.pts[path.pts.length - 1];
  if (!a || !b) return { x: pose.x, z: pose.z, opacity: 1, fading: false };
  if (s < f / 2) return { x: a.x, z: a.z, opacity: 1 - s / (f / 2), fading: true };
  if (s < f) return { x: b.x, z: b.z, opacity: (s - f / 2) / (f / 2), fading: true };
  return { x: b.x, z: b.z, opacity: 1, fading: false };
}

/** a pose's beats at time t: its place, its walk, its bed, its transition (dur) and the time past its arrival (u) */
function timing(inp: Pick<RigInput, 'pose' | 'reduced' | 'bedAt'>) {
  const { pose, reduced } = inp;
  const { seg } = pose;
  const act = seg.act;
  const k: ActivityKind = act.k;
  const place = placeOf(pose, reduced);
  const walkT = seg.path.len > 0.01 && seg.from !== null ? walkTime(seg) : 0;
  const arrived = reduced ? !place.fading : !pose.moving;
  // its bed (a napper in the lounge), the one it gets up from (its walk starts there)
  const bed = k === 'nap' && act.at === 'lounge' ? inp.bedAt?.(seg.spot) : undefined;
  const left = seg.from === 'nap' && seg.path.len > 0.01 ? inp.bedAt?.(seg.path.pts[0] ?? seg.spot) : undefined;
  const down = bed !== undefined && walkT > 0;
  const dur = reduced ? 0 : down ? NAP_DOWN : transitionOf(seg.from, k);
  // at its desk without a walk: its activity's pose after the hop
  const hopT = !reduced && (seg.sit?.up || seg.sit?.down) ? SIT_S.hop : 0;
  const u = pose.since - (reduced ? Math.min(FADE, walkT) : walkT) - hopT;
  // its sit at its desk or its get-up (sit.ts): beside its chair, the chair out, a side step, the hop; never through it
  const sat = !reduced ? sitBeat(seg, pose.since) : null;
  return { act, k, place, walkT, arrived, bed, left, down, dur, hopT, u, sat };
}

/**
 * When the bim came to rest in its state's still pose (t, s), or null while anything else moves it:
 * its walk, its sit or get-up, its lie-down, a transition, a wait, a plane to catch (render/idle.ts:
 * its idle moves start only then). Motion only: reduced motion is always null.
 */
export function restingSince(inp: Pick<RigInput, 'pose' | 't' | 'reduced' | 'bedAt'>): number | null {
  if (inp.reduced) return null;
  const { pose } = inp;
  const { arrived, left, dur, u, sat } = timing(inp);
  if (!arrived || sat || pose.waiting === true || u < dur) return null;
  if (left && pose.since < NAP_DOWN) return null;
  const planes = pose.plane ? [...(pose.aloft ?? []), pose.plane] : [];
  if (planes.some((p) => p.since < (p.dur ?? PLANE_FLIGHT) + PLANE_CATCH)) return null;
  return inp.t - (u - dur);
}

/** the bim at time t: its body, its place, and how the loop must run for it */
export function rigOf(inp: RigInput): Rig {
  const { pose, t, phase, reduced } = inp;
  const { seg } = pose;
  const { act, k, place, walkT, arrived, bed, left, down, dur, hopT, u, sat } = timing(inp);
  const lt = loopT(t, reduced);
  // a loop of its own from its arrival (eating); reduced motion holds it chewing, a fruit in hand
  const target = rest(act, lt, phase, reduced ? PICK_S + BITE_S - 0.2 : Math.max(0, u - dur), bed, inp.main);

  const sitIn = seg.sit?.in;

  let b: Body;
  let motion: Motion;
  if (left && !reduced && pose.since < NAP_DOWN) {
    // up from its bed before it walks off (sit.ts NAP_UP: the timeline waits there that long)
    b = lieDown(left, 1 - pose.since / NAP_DOWN);
    motion = 'walk';
  } else if (sat) {
    const { beat } = sat;
    const step = beat.step ? walking(k, t) : STAND;
    const legs = SEATED.legL * beat.legs;
    b = {
      ...step, y: beat.y + SEATED.y * beat.legs + (beat.step ? step.y : 0),
      legL: beat.step ? step.legL : legs, legR: beat.step ? step.legR : legs,
      face: beat.facing ? sat.at.spot.face : null,
    };
    motion = 'walk';
  } else if (pose.waiting === true && !reduced) {
    // it stops to let another bim pass, or stands beside its spot (avoid.ts): standing still, as it walks
    b = walking(k, 0);
    // its head turns to the bim it waits for (head only, bims m_31438)
    if (pose.look !== undefined) {
      const turn = Math.atan2(Math.sin(pose.look - pose.dir), Math.cos(pose.look - pose.dir));
      b = { ...b, head: [b.head[0], Math.max(-1.1, Math.min(1.1, turn)), b.head[2]] };
    }
    motion = pose.moving ? 'walk' : 'still';
  } else if (!arrived && !reduced) {
    b = walking(k, t);
    motion = 'walk';
  } else if (!arrived) {
    b = target; // reduced: the end pose, fading in or out
    motion = 'walk';
  } else if (u < dur && down) {
    // on its bed: it sits on the edge, then lies back
    b = lieDown(bed, u / dur);
    motion = 'walk';
  } else if (u < dur) {
    // sat down on its walk's end (sit.ts): from seated
    const from = hopT > 0 ? (seg.sit?.down ? SITTING : STAND) : walkT > 0 ? (sitIn && !sitIn.standing && !reduced ? SITTING : STAND) : seg.from === null ? target : rest({ k: seg.from, at: act.at }, lt, phase, lt, undefined, inp.main);
    const f = ease(u / dur);
    b = beat(seg.from, k, u / dur, blend(from, target, f));
    motion = 'walk';
  } else {
    b = target;
    motion = !reduced && ((TYPING.has(k) && workLoops(act)) || k === 'asks' || k === 'snack') ? 'loop' : 'still';
  }

  if (pose.plane && !reduced) {
    // the newest plane whose reach has begun: a newer one takes the arm, the older's sheet goes away (catch.ts cut)
    const all = [...(pose.aloft ?? []), pose.plane];
    const on = all.findLast((p) => p.since >= (p.dur ?? PLANE_FLIGHT) - REACH_S && p.since < (p.dur ?? PLANE_FLIGHT) + PLANE_CATCH);
    if (all.some((p) => p.since < (p.dur ?? PLANE_FLIGHT) + PLANE_CATCH)) motion = 'walk';
    if (arrived && on) b = caught(b, on.since, on.dur ?? PLANE_FLIGHT, awayOf(act) === 'desk', on.thumbs !== false);
  } else if (pose.plane && arrived) {
    // reduced motion: no flight, the open sheet read in its hand (catch.ts heldAt), then put away
    if (pose.plane.since < (pose.plane.dur ?? PLANE_FLIGHT) + BEAT.read) b = { ...b, armR: ARM.read, head: [0.35, 0, 0] };
  }

  // a standing done bim turned to its neighbour once there (timeline.ts chats)
  const dir = b.face ?? pose.dir + (arrived && !sat ? (act.turn ?? 0) : 0);
  // its chair: slid as the sit has it; left out while it stands at its desk
  const out = (sitIn?.standing === true && arrived) || seg.sit?.up !== undefined || seg.sit?.still !== undefined;
  const back = sat ? sat.beat.back : out ? SIT.pull : null;
  // the offset is in the bim's frame: z ahead along its way, x to its side; standing by its desk
  // (asks, the big screen), in its desk's: beside it on a turned desk too, never in its chair (sit.test.ts)
  const frame = out && !sat ? seg.spot.face : dir;
  const c = Math.cos(frame);
  const sn = Math.sin(frame);
  const here = sat ? toWorld(sat.at.spot, sat.beat.u, sat.beat.v) : place;
  const x = here.x + b.dx * c + b.dz * sn;
  const z = here.z - b.dx * sn + b.dz * c;
  const rig: Rig = {
    x, z, dir, y: b.y, body: b.body, head: b.head, armL: b.armL, armR: b.armR, legL: b.legL, legR: b.legR,
    eyes: b.eyes, mouth: b.mouth, props: b.props, screen: b.screen, lamp: b.lamp, grey: b.grey,
    opacity: place.opacity, motion, ...(back !== null && back > 1e-4 ? { seat: { back, turn: 0 } } : {}),
  };
  // to the park or back from it (skate.ts): its board in its hand; back, it kicks it up first
  const carry = !arrived && !reduced && (seg.pastime?.kind === 'skate' || seg.fromSkate === true);
  return carry ? carrying(rig, seg.fromSkate === true ? pose.since : Infinity) : rig;
}

/** the kick up (s): the board flips from the floor into the hand */
export const KICK_S = 0.4;

/** walking with its board under its arm; `since` < KICK_S: the kick up from the floor */
function carrying(r: Rig, since: number): Rig {
  const hand = { x: 0.3, y: 0.62, z: 0.05, rot: [0, 0, Math.PI / 2] as Rot };
  const k = since / KICK_S;
  const board = k < 1 ? { x: hand.x * k, y: 0.08 + (hand.y - 0.08) * k, z: 0.35 * (1 - k), rot: [Math.PI * 2 * k, 0, (Math.PI / 2) * k] as Rot } : hand;
  return { ...r, armR: [-0.4, 0, 0.35], props: { ...r.props, skate: true }, board };
}

/** the loudest of several motions: 'walk' over 'skate' over 'idle' over 'blend' over 'loop' over 'still' */
export function louder(a: Motion, b: Motion): Motion {
  if (a === 'walk' || b === 'walk') return 'walk';
  if (a === 'skate' || b === 'skate') return 'skate';
  if (a === 'idle' || b === 'idle') return 'idle';
  if (a === 'blend' || b === 'blend') return 'blend';
  if (a === 'loop' || b === 'loop') return 'loop';
  return 'still';
}
