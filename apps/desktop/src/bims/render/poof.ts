// A newcomer's poof (Gabriel's review, main m_38961: « un petit nuage de fumée ou un peu de glitter
// [...] faut que ça reste joli, pas kitsch »; designer m_39015 picked 'both' of three): as a bim pops
// in (office-scene.ts appearOut), a ring of soft paper-white clay puffs rings out from its feet and
// shrinks away, and four small gold sparkles twinkle out over it. Every piece grows and shrinks (no
// see-through material: no sorting, one draw call per kind). Pure: where each piece is and its size
// at s seconds, from its bim's feet; poofs.ts draws them. Under reduced motion there is none (the
// caller never starts one). Owner: bim-pop.
import { AZ } from './camera';

/** a poof's whole length, s (longer than the bim's own pop, APPEAR_S: the cloud clears after it) */
export const POOF_S = 0.9;

/** one piece at a time: offset from the bim's feet, office units; `r` its size (0: gone); `spin` radians */
export interface PoofBit {
  readonly kind: 'puff' | 'spark';
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly r: number;
  readonly spin: number;
}

/** the puffs: how many, the ring's start and end radius, a ball's radius, its grow time, its life (s) */
export const PUFF = { n: 8, r0: 0.16, r1: 0.6, size: 0.17, grow: 0.12, life: [0.55, 0.8] as const } as const;
/** the sparkles: how many, a star's half length, each one's life, the gap between two, the first's delay, their height */
export const SPARK = { n: 4, size: 0.15, life: 0.5, gap: 0.07, from: 0.08, y: 0.75 } as const;
/** designer m_39015: a puff's ball at least this wide on screen (css px), its ring at least ~1.5 bims wide */
export const PUFF_MIN_PX = 6;
/**
 * how far the far camera may grow the puffs: a little (designer m_49905: at the tile's fit they read
 * as grey dust over the bim's body; the glitter is the sparkles' now, the puffs stay at its feet)
 */
const BOOST_MAX = 1.3;
/** the puffs stay under the bim's waist (its feet's cloud): a ball's top at most this high */
export const PUFF_TOP = 0.45;
/**
 * his m_49844 (« la même animation d'apparition que dans la hero section, le glitter, comme par
 * magie »): the hero's close shots show the sparkles; at the tile's fit (~6 px a unit) they were a
 * pixel. A star's half length at least this on screen (css px), grown at most SPARK_MAX, and spread
 * out at most SPREAD_MAX with it (the glitter round the bim, not on it)
 */
export const SPARK_MIN_PX = 4;
const SPARK_MAX = 4.5;
/**
 * designer m_49952: at the fit the 4 stars sat on each other (one clump): their ring round its head
 * grows so the nearest star is at least SPARK_RING_PX from it on screen, at most SPREAD_MAX times
 * its close-up size (the hero's, ×1, unchanged)
 */
export const SPARK_RING_PX = 14;
const SPREAD_MAX = 8;
/** far out (spread past 1), one star after another: each in this long after the last, living this long (designer m_49952) */
export const SPARK_FAR = { life: 0.3, gap: 0.08 } as const;
/** the ring's nearest radius close up, office units (sparks: far) */
const RING_IN = 0.45;
/** the floor's way toward the camera (camera.ts AZ: x cos a, z sin a), and the arc of the ring the far stars use round it */
const FRONT = Math.atan2(Math.cos(AZ), Math.sin(AZ));
const FRONT_ARC = Math.PI - 0.6;
/** the far stars' slots on that arc, in their order in time */
const FAR_SLOTS = [1, 3, 0, 2] as const;

/** How much the sparkles grow at this camera (`scale`: office units per css px): 1 close up, more far out. Pure. */
export function sparkBoost(scale: number): number {
  return Math.min(SPARK_MAX, Math.max(1, (SPARK_MIN_PX * scale) / SPARK.size));
}

/** How much wider their ring round the head is at this camera: 1 close up, more far out (SPARK_RING_PX). Pure. */
export function sparkSpread(scale: number): number {
  return Math.min(SPREAD_MAX, Math.max(1, (SPARK_RING_PX * scale) / RING_IN));
}

const clamp01 = (x: number): number => Math.min(1, Math.max(0, x));
const expoOut = (x: number): number => (x >= 1 ? 1 : 1 - 2 ** (-10 * x));
const backOut = (x: number): number => {
  const c = 1.7;
  const y = clamp01(x) - 1;
  return 1 + (c + 1) * y ** 3 + c * y * y;
};

/**
 * How much the puffs grow at this camera (`scale`: office units per css px): 1 close up (today's
 * size), more far out so a ball stays PUFF_MIN_PX wide on screen, at most BOOST_MAX. Pure.
 */
export function puffBoost(scale: number): number {
  const ball = 2 * PUFF.size * 0.75;
  return Math.min(BOOST_MAX, Math.max(1, (PUFF_MIN_PX * scale) / ball));
}

/** a small seeded stream in [0, 1) (mulberry32): the same poof for the same bim, every frame */
function stream(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const GONE = (kind: PoofBit['kind']): PoofBit => ({ kind, x: 0, y: 0, z: 0, r: 0, spin: 0 });

/** the puffs: a ring from the feet, out and up a little, each grows fast and shrinks away */
function puffs(rnd: () => number, s: number, boost: number): PoofBit[] {
  const out: PoofBit[] = [];
  const turn = rnd() * Math.PI * 2;
  // the ring as wide at any camera (designer m_49905: spread ×1)
  const wide = 1;
  for (let i = 0; i < PUFF.n; i++) {
    const a = turn + (i / PUFF.n) * Math.PI * 2 + (rnd() - 0.5) * 0.5;
    const life = PUFF.life[0] + rnd() * (PUFF.life[1] - PUFF.life[0]);
    const big = 0.75 + rnd() * 0.5;
    // every third one a little higher, all of them at its feet: the newcomer is seen arriving over them
    const lift = (i % 3 === 0 ? 0.12 : 0.04) + rnd() * 0.06;
    const u = s / life;
    if (u < 0 || u >= 1) { out.push(GONE('puff')); continue; }
    const go = expoOut(u * 1.6);
    const rad = (PUFF.r0 + (PUFF.r1 - PUFF.r0) * go) * wide;
    const grow = backOut(s / PUFF.grow);
    const fade = 1 - clamp01((u - 0.35) / 0.65) ** 2;
    const r = Math.max(0, Math.min(PUFF_TOP / 2, PUFF.size * boost * big * grow * fade));
    // its top under PUFF_TOP: never over the bim's body
    const y = Math.min(Math.max(lift, r * 0.8) + 0.06 * go, PUFF_TOP - r);
    out.push({ kind: 'puff', x: Math.cos(a) * rad, y, z: Math.sin(a) * rad, r, spin: 0 });
  }
  return out;
}

/** the sparkles: out from the bim's middle, one after another, each twinkles once (in, a turn, out) */
function sparks(rnd: () => number, s: number, boost = 1, spread = 1): PoofBit[] {
  const out: PoofBit[] = [];
  // far out: one by one (SPARK_FAR), each already out on its ring as it twinkles in
  const one = spread > 1;
  const life = one ? SPARK_FAR.life : SPARK.life;
  const gap = one ? SPARK_FAR.gap : SPARK.gap;
  const turn = rnd() * Math.PI * 2;
  for (let i = 0; i < SPARK.n; i++) {
    const jitter = (rnd() - 0.5) * 0.7;
    // far out: on the ring's half toward the camera, never over the head on screen, where the bims'
    // pills and labels stand (designer m_50022: the top star went under the '?' pills); one after
    // another on slots apart (FAR_SLOTS), never two neighbours in a row
    const a = one ? FRONT - FRONT_ARC / 2 + ((FAR_SLOTS[i % FAR_SLOTS.length] ?? 0) + 0.5 + jitter * 0.3) * (FRONT_ARC / SPARK.n) : turn + (i / SPARK.n) * Math.PI * 2 + jitter;
    const far = RING_IN + rnd() * 0.3;
    const up = 0.15 + rnd() * 0.45;
    const big = 0.7 + rnd() * 0.5;
    const u = (s - SPARK.from - i * gap) / life;
    if (u < 0 || u >= 1) { out.push(GONE('spark')); continue; }
    const go = one ? 0.85 + 0.15 * expoOut(u) : expoOut(u);
    // a twinkle: quick in, a short hold, out
    const tw = Math.sin(Math.PI * clamp01(u) ** 0.7);
    // at its head's height (designer m_49905), a little over it as it twinkles out
    out.push({ kind: 'spark', x: Math.cos(a) * far * spread * go, y: SPARK.y + up * 0.35 * go, z: Math.sin(a) * far * spread * go, r: Math.max(0, SPARK.size * boost * big * tw), spin: a + u * 2.2 });
  }
  return out;
}

/**
 * A poof's pieces at s seconds from its start: PUFF.n puffs then SPARK.n sparkles, at any s (gone
 * ones r 0). `boost`: puffBoost of the camera; `spark`: sparkBoost's; `spread`: sparkSpread's. Pure.
 */
export function poofAt(seed: number, s: number, boost = 1, spark = 1, spread = 1): PoofBit[] {
  const rnd = stream(seed);
  return [...puffs(rnd, s, boost), ...sparks(rnd, s, spark, spread)];
}
