// The hero film's score (page bims-film v11: Gabriel's scenario A, « de zéro à l'app », the shop):
// kiln, a pottery studio's empty office in the middle of a calm town (harness and acme-web live
// quietly around it, no shot goes there). He writes to bise in the feed (bottom-left); each line
// folds into a plane that dives to kiln's main, and an agent pops per task. checkout visits cart and
// asks him a question; order-test finds it a bug (no ✗: nobody failed); 'review it for me' and a
// reviewer stamps ✓. The crates land, the shop rises from the last one on localhost:5173 and a mug
// is bought; then the page and the town fold into cubes and the entry wave builds it again: the loop's
// seam is the entry itself, kiln empty again. Each bim's life is a list of [time, state] changes (the
// state at T is the last change at or before T, else its life's last one); his lines, the agents'
// visits, the bubbles, the eggs and the director's shots are timed the same way. Data only: world.ts
// reads it into the app's rows, director.ts into the camera, feed.ts into the feed. Owner: hero-town.
import type { DoingKind } from '@/proto';
import type { EggKind } from '../bims/eggs';
import type { MapId } from '../bims/maps/ids';
import type { HerHair } from '../bims/render/palette';

/** the loop, s: the fold ends at FILM_S and the entry wave is 0:00 */
export const FILM_S = 68.6;

/** What a bim does from a change on (world.ts turns it into the app's row, or its office life). */
export type Mode =
  | { readonly k: 'gone' }
  | { readonly k: 'work'; readonly doing?: DoingKind }
  /** idle: in a bean bag */
  | { readonly k: 'idle' }
  /** idle, a trip to the island: its fruit (0 the pineapple), its trip number */
  | { readonly k: 'snack'; readonly fruit: number; readonly trip: number }
  /** asleep at its desk on a watch */
  | { readonly k: 'sleep' }
  /** asks you: the '?' pill */
  | { readonly k: 'asks' }
  /** failed: the '✗' pill */
  | { readonly k: 'failed' }
  /** waits on another agent */
  | { readonly k: 'wait'; readonly on: string }
  /** in main's land queue at that rank, its box carried */
  | { readonly k: 'landing'; readonly rank: number }
  /** done: on the terrace */
  | { readonly k: 'done' }
  /** main at work */
  | { readonly k: 'main' }
  /** main idle: reads the paper */
  | { readonly k: 'mainidle' };

export type Change = readonly [at: number, mode: Mode];

export interface Bim {
  readonly name: string;
  readonly main?: boolean;
  readonly life: readonly Change[];
  /**
   * its cut and hat in the film over its name's (palette.ts herHair): the hero's look only, so an
   * agent of that name in his own projects keeps its own; its clothes, skin and colours stay its name's
   */
  readonly head?: HerHair;
}

export interface Cast {
  readonly id: string;
  readonly name: string;
  readonly map: MapId;
  /** its growth stage (growth.ts STAGE_AT): never changes in the film (an office never shrinks) */
  readonly stage: number;
  readonly bims: readonly Bim[];
}

/** his plane's flight (bims-plane 382a3ddee): its length, s, and its loops; it leaves from the feed */
export interface Flight {
  readonly dur: number;
  readonly loops: 0 | 1 | 2;
}

/** a message: from you (a paper plane to the agent, off the feed) or from an agent to another (a visit) */
export interface Note {
  readonly at: number;
  readonly office: string;
  /** an agent's name, or 'you' (his paper plane) */
  readonly from: string;
  readonly to: string;
  /** his plane's flight (unset: the app's) */
  readonly flight?: Flight;
}

/** an egg forced on a bim from `at` for `dur` s */
export interface Egg {
  readonly at: number;
  readonly dur: number;
  readonly office: string;
  readonly bim: string;
  readonly kind: EggKind;
}

/** a bim's words over its head (bims-film v9: one at a time, 2-5 words, the labels' look, BUBBLE_S) */
export interface Bubble {
  readonly at: number;
  readonly office: string;
  readonly bim: string;
  readonly words: string;
}

/**
 * A line of the feed (the app's composer, bottom-left): his (typed from `at`, folded into a plane at
 * `plane`), bise's, or bise's waiting line (the app's '? who · words', its choices). `tick`: when it
 * takes its ✓ (its crate is in). Lines leave the feed at the fold.
 */
export type Line =
  | { readonly at: number; readonly who: 'you'; readonly words: string; readonly plane?: number; readonly tick?: number }
  | { readonly at: number; readonly who: 'bise'; readonly words: string; readonly tick?: number }
  | {
      readonly at: number;
      readonly who: 'ask';
      readonly bim: string;
      readonly words: string;
      /** the answer's field ('answering checkout'), or numbered choices, the picked one at `picked` */
      readonly field?: string;
      readonly choices?: readonly string[];
      readonly pick?: { readonly at: number; readonly n: number };
      /** answered: it leaves the feed's waiting look */
      readonly until: number;
    };

/** the shot sizes (bims-film): the town, one office (tight: closer), a bim followed, a close-up, closest */
export type Size = 'wide' | 'office' | 'tight' | 'follow' | 'close' | 'closest';

/** what a shot frames: the town, an office, a spot of an office, a bim (its name shown), two bims (no name) */
export type Subject =
  | { readonly k: 'town' }
  | { readonly k: 'office'; readonly office: string }
  | { readonly k: 'spot'; readonly office: string; readonly spot: 'island' | 'lounge' | 'park' | 'main' }
  | { readonly k: 'bim'; readonly office: string; readonly name: string }
  | { readonly k: 'pair'; readonly office: string; readonly names: readonly [string, string] };

export interface Shot {
  /** when the action starts: the camera leaves LEAD_S before (director.ts) */
  readonly at: number;
  readonly size: Size;
  readonly subject: Subject;
  /** its glide in, s, past the director's own */
  readonly glide?: number;
  /** how long before `at` its camera leaves, s, past the director's LEAD_S */
  readonly lead?: number;
  /** where its subject sits, fractions of the frame, past the director's safe point */
  readonly frame?: { readonly x: number; readonly y: number };
  /** this much farther than its size (a slight pull back for a plane on its way), 1 none */
  readonly back?: number;
}

const work: Mode = { k: 'work', doing: 'edit' };
const think: Mode = { k: 'work', doing: 'think' };
const gone: Mode = { k: 'gone' };
const idle: Mode = { k: 'idle' };
const landing = (rank: number): Mode => ({ k: 'landing', rank });

export const KILN = 'kiln';
export const HARNESS = 'harness';
export const ACME = 'acme-web';

/** the shop's page (kiln-app): it rises from main's crate, the fold takes it and the town, the wave is 0:00 */
export const PAGE = { at: 60.7, fold: 67.4 } as const;
/** the agent that reviews checkout's crate ('review it for me'): it pops beside it in main's queue (run.ts reviewSpot) */
export const REVIEWER = 'review';
/** reduced motion's still (bims-film v11): kiln full, every line ticked, the shop up with its cart open (kiln-app: 2.0 s in) */
export const REDUCED_T = PAGE.at + 2;
/** the fold: from the edge to the middle, FOLD_S long, ending at FILM_S */
export const FOLD_S = FILM_S - PAGE.fold;
/** a bubble's time over its speaker, s */
export const BUBBLE_S = 1.8;

/** paper planes, tossed then gliding (designer's hero-fix-2 §2, bims-film v14): 3 s the first, 2.8 s the next, never a loop */
// the first leaves 0.5 s later than it did (his line held to be read, then sent: main m_36302), its catch at 0:07.3 kept
const FIRST: Flight = { dur: 2.5, loops: 0 };
const SHORT: Flight = { dur: 2.8, loops: 0 };

/** checkout's head in the film (hero-hair: his pick of designer's four cuts) */
export const CHECKOUT_HEAD: HerHair = { cut: 'buns', hat: 'bare' };

export const CAST: readonly Cast[] = [
  {
    id: KILN,
    name: 'kiln',
    map: 'workshop',
    stage: 6,
    bims: [
      // alone at 0:00 (it pops with the wave), his first plane in its hands at 0:07.3
      { name: 'main', main: true, life: [[0, { k: 'mainidle' }], [7.3, { k: 'main' }]] },
      // one agent per line, popping at its desk 0.3 s after main catches it (bims-film v14: cart and
      // checkout from the same line); the crates leave from 0:44.6 (checkout's), the others from 0:45,
      // ~0.6 s apart: the walks overlap (bims-film v15: each gets up round its chair first, bims-sit)
      { name: 'catalog', life: [[0, gone], [10.7, work], [45, landing(2)]] },
      { name: 'cart', life: [[0, gone], [12.2, work], [45.7, landing(3)]] },
      // visits cart (0:15.5), asks him (0:26.8), his answer at 0:32.8: at its desk it reads it and lays
      // it there (to 0:34.55), its '?' goes as it hops, it sits and types fast (bims-film v16); its crate
      // first (0:44.6, there at 0:52)
      // its hair (Gabriel m_46420: not the 'long' its name draws): the film's, bare-headed
      { name: 'checkout', head: CHECKOUT_HEAD, life: [[0, gone], [12.5, work], [26.8, { k: 'asks' }], [34.55, work], [44.6, landing(1)]] },
      { name: 'emails', life: [[0, gone], [13.7, work], [46, landing(4)]] },
      { name: 'theme', life: [[0, gone], [39.1, work], [47.2, landing(6)]] },
      // finds checkout a bug (0:38.1, its visit over by 0:44.0: gone before checkout gets up at 0:44.6,
      // its head brushed checkout's as it rose, designer m_50704), no ✗; 5th in line (from its desk the 6th place has no clear way: it never left)
      { name: 'order-test', life: [[0, gone], [37.6, work], [47.8, landing(5)]] },
      // 'review it for me': pops beside checkout's crate, reads, stamps ✓ (0:58.9)
      { name: REVIEWER, life: [[0, gone], [56.7, work]] },
    ],
  },
  {
    id: HARNESS,
    name: 'harness',
    map: 'loft',
    stage: 13,
    bims: [
      { name: 'main', main: true, life: [[0, { k: 'main' }]] },
      { name: 'signup-flow', life: [[0, work]] },
      { name: 'dark-mode', life: [[0, think]] },
      { name: 'docs-sync', life: [[0, idle], [24, { k: 'snack', fruit: 1, trip: 1 }], [38, idle]] },
      { name: 'perf-audit', life: [[0, { k: 'sleep' }]] },
    ],
  },
  {
    id: ACME,
    name: 'acme-web',
    map: 'riad',
    stage: 9,
    bims: [
      { name: 'main', main: true, life: [[0, { k: 'main' }]] },
      { name: 'api-tests', life: [[0, work]] },
      { name: 'invoice-pdf', life: [[0, think]] },
      { name: 'search', life: [[0, idle], [16, { k: 'snack', fruit: 2, trip: 1 }], [30, idle]] },
    ],
  },
];

/** a bim's head set by the cast (Bim.head) in an office, the town's TownOpts.heads; undefined: its name's own. Pure. */
export function headOf(office: string, name: string): HerHair | undefined {
  return CAST.find((c) => c.id === office)?.bims.find((b) => b.name === name)?.head;
}

/** his lines' planes leave the feed when the line folds (FEED's `plane`), the agents' visits */
export const NOTES: readonly Note[] = [
  { at: 4.8, office: KILN, from: 'you', to: 'main', flight: FIRST },
  // at most two in the air, on different curves (bims-film v14)
  { at: 7.6, office: KILN, from: 'you', to: 'main', flight: SHORT },
  { at: 9.1, office: KILN, from: 'you', to: 'main', flight: SHORT },
  { at: 10.6, office: KILN, from: 'you', to: 'main', flight: SHORT },
  // checkout goes to cart's desk (the app's visit)
  { at: 15.5, office: KILN, from: 'checkout', to: 'cart' },
  // his answer, into checkout's hands at 0:32.8
  { at: 30, office: KILN, from: 'you', to: 'checkout', flight: SHORT },
  { at: 36, office: KILN, from: 'you', to: 'main', flight: SHORT },
  { at: 37.5, office: KILN, from: 'you', to: 'main', flight: SHORT },
  // 0.5 s after it pops: a message in the step that makes the bim is played before it exists, and lost
  { at: 38.1, office: KILN, from: 'order-test', to: 'checkout' },
  // 'review it for me'
  { at: 53.6, office: KILN, from: 'you', to: 'main', flight: SHORT },
];

/** the feed (bims-film v14), in its order */
export const FEED: readonly Line[] = [
  { at: 3, who: 'you', words: 'build a shop for kiln, my pottery studio', plane: 4.8, tick: 60.2 },
  // each ✓ when its crate is in main's queue (the office brains' arrivals)
  // one line in his field at a time: typed, held to be read, sent (its button folds into its plane)
  { at: 5.6, who: 'you', words: 'a grid of pots with prices', plane: 7.6, tick: 53.5 },
  { at: 7.6, who: 'you', words: 'a cart and card payments', plane: 9.1, tick: 54.9 },
  { at: 9.2, who: 'you', words: 'email the receipt', plane: 10.6, tick: 55.1 },
  { at: 14, who: 'bise', words: 'on it: 4 agents in kiln' },
  { at: 27.3, who: 'ask', bim: 'checkout', words: 'ship worldwide or EU only?', field: 'answering checkout', until: 30 },
  { at: 29, who: 'you', words: 'EU only', plane: 30 },
  { at: 34.5, who: 'you', words: 'make it warm, like paper', plane: 36, tick: 56.2 },
  { at: 35.8, who: 'you', words: 'test the whole checkout before you ship', plane: 37.5, tick: 56.3 },
  { at: 52, who: 'ask', bim: 'checkout', words: 'ship it, or review it first?', choices: ['ship it', 'review it for me'], pick: { at: 53.2, n: 2 }, until: 53.6 },
  { at: 60.2, who: 'bise', words: 'all done. it runs on localhost:5173', tick: 60.2 },
];

/** over the visits once they are there (bims-film v14) */
export const BUBBLES: readonly Bubble[] = [
  { at: 19.1, office: KILN, bim: 'checkout', words: 'cart line format?' },
  { at: 21.1, office: KILN, bim: 'cart', words: 'id, qty, price' },
  { at: 42.2, office: KILN, bim: 'order-test', words: 'a mug sold twice' },
];

export const EGGS: readonly Egg[] = [
  // after his answer's catch and read, its plane put away (render/catch.ts: 1.75 s on): no thumbs up, this hop
  { at: 34.55, dur: 0.6, office: KILN, bim: 'checkout', kind: 'hop' },
  { at: 43.8, dur: 0.6, office: KILN, bim: 'order-test', kind: 'hop' },
  // the reviewer reads the crate's sheet (main's paper) a second, then stamps it: ✓ at 0:58.9
  { at: 57.5, dur: 1.8, office: KILN, bim: REVIEWER, kind: 'stamp' },
  // main lifts the last crate over its head, three notes
  { at: 59.7, dur: 2.5, office: KILN, bim: 'main', kind: 'itemget' },
];

/** the ✓ the reviewer stamps on checkout's crate: from then until the fold */
export const STAMP = { at: 58.9, office: KILN, on: 'checkout' } as const;

const bim = (office: string, name: string): Subject => ({ k: 'bim', office, name });
const pair = (office: string, a: string, b: string): Subject => ({ k: 'pair', office, names: [a, b] });
const kiln: Subject = { k: 'office', office: KILN };
/** where the catcher sits while a plane from the feed (bottom right, run.ts FEED_X) flies to it, up left of it (designer m_32742): its path across the frame */
export const FLIGHT_FRAME = { x: 0.52, y: 0.3 } as const;

/** no cut: every move a glide (expo in-out); the camera leaves ~1 s before the action (bims-film v9) */
export const SHOTS: readonly Shot[] = [
  // the entry wave (0-2.4) seen wide, then the glide to kiln (film.ts entryCam): there by 0:02.8;
  // no lead (the fold's wide holds to the loop's end)
  { at: 0, size: 'office', subject: kiln, lead: 0 },
  // the first plane: its path held at FOLLOW, main up right, the feed's corner below left, so the
  // plane crosses the frame at ~30-40 px (bims m_32000, designer m_31992); back to the OFFICE on its catch (0:07.3)
  { at: 4.3, size: 'follow', subject: bim(KILN, 'main'), frame: FLIGHT_FRAME },
  { at: 7.6, size: 'office', subject: kiln, lead: 0.3 },
  { at: 15.5, size: 'follow', subject: bim(KILN, 'checkout') },
  { at: 19, size: 'follow', subject: pair(KILN, 'checkout', 'cart') },
  { at: 23, size: 'follow', subject: bim(KILN, 'checkout') },
  { at: 26.8, size: 'close', subject: bim(KILN, 'checkout') },
  // his answer's plane: the same framing on checkout, then checkout in the middle once it caught it
  { at: 29, size: 'follow', subject: bim(KILN, 'checkout'), frame: FLIGHT_FRAME },
  { at: 32.8, size: 'follow', subject: bim(KILN, 'checkout'), lead: 0 },
  { at: 35.5, size: 'office', subject: kiln },
  { at: 38.2, size: 'follow', subject: bim(KILN, 'order-test') },
  { at: 41.6, size: 'follow', subject: pair(KILN, 'order-test', 'checkout') },
  // checkout lifts its crate, then the parade to main's queue seen whole
  { at: 44.6, size: 'follow', subject: bim(KILN, 'checkout') },
  { at: 47, size: 'office', subject: kiln },
  // checkout at the queue: '? checkout · ship it, or review it first?', the reviewer, its stamp
  { at: 52, size: 'tight', subject: { k: 'spot', office: KILN, spot: 'main' } },
  { at: 59.7, size: 'follow', subject: bim(KILN, 'main') },
  // the shop rises from the crate: a little farther, the bims around live on
  { at: PAGE.at, size: 'follow', subject: bim(KILN, 'main'), back: 1.4 },
  // the fold: back to the wide the wave starts from
  { at: PAGE.fold, size: 'wide', subject: { k: 'town' }, lead: 0, glide: FOLD_S },
];
