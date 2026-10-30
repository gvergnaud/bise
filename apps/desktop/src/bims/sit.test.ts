// The sitting law (Gabriel: « les agents passent a travers leur chaise pour s'assoir »; bims
// m_32740): on every map and every desk, a bim walking to its chair, sitting down, getting up and
// walking off never has a part of its body (its hoodie, its legs, its feet) inside its chair (the
// seat, the backrest), but the seat it sits on. The real timeline and the real rigs (render/pose.ts),
// sampled every 20 ms; the body and the chair as render/bim.ts and render/furniture.ts build them.
import { describe, expect, it } from 'vitest';
import { agent, mainAgent } from './fixtures';
import { layoutOf } from './maps';
import { MAP_IDS } from './maps/ids';
import type { Activity, Office, Spot } from './model';
import { rigOf, type Rig } from './render/pose';
import { CHAIR, chairSpots, hopBeat, pullRoom, seatWaits, SIT, SIT_S, sitBeat, sidePoints, STAND_AT, standBeat, toLocal } from './sit';
import { makePath } from './paths';
import { newTimeline } from './timeline';

const near = (a: { x: number; z: number }, b: { x: number; z: number }): boolean => Math.hypot(a.x - b.x, a.z - b.z) < 1e-4;

function office(map: (typeof MAP_IDS)[number], n: number): Office {
  return layoutOf([mainAgent(), ...Array.from({ length: n - 1 }, (_, i) => agent(`a${String(i + 1)}`))], map);
}

/** a 3D box in the chair's spot frame (u across, v along the desk's way) */
interface Box3 {
  readonly u0: number;
  readonly u1: number;
  readonly v0: number;
  readonly v1: number;
  readonly y0: number;
  readonly y1: number;
}

/** the chair (furniture.ts chair: the seat 0.42 square 0.08 thick under CHAIR.seatTop, the backrest 0.07 thick from just under it to CHAIR.backTop, 0.2 behind its middle), slid back `back` */
function chairParts(back: number): { seat: Box3; rest: Box3 } {
  const c = -CHAIR.dz - back;
  return {
    seat: { u0: -0.21, u1: 0.21, v0: c - 0.21, v1: c + CHAIR.front, y0: CHAIR.seatTop - 0.08, y1: CHAIR.seatTop },
    rest: { u0: -0.21, u1: 0.21, v0: c - 0.235, v1: c - 0.165, y0: CHAIR.seatTop - 0.02, y1: CHAIR.backTop },
  };
}

/** a body part as a ball (centre in the spot frame, its height, its radius) or the hoodie (a capsule: an upright cylinder) */
type Part = { kind: 'ball'; name: string; u: number; v: number; y: number; r: number } | { kind: 'hoodie'; u: number; v: number; y0: number; y1: number; r: number };

/** the bim's parts from its rig (bim.ts: the hoodie's capsule 0.21 at 0.5, legs from 0.3 at x ±0.1, a foot 0.15 x 0.08 x 0.24) in the spot's frame */
function parts(r: Rig, spot: Spot, main: boolean): Part[] {
  const s = main ? 1.18 : 1;
  const out: Part[] = [];
  const c = Math.cos(r.dir);
  const sn = Math.sin(r.dir);
  const lift = r.lift ?? 0;
  const at = (bx: number, by: number, bz: number): { u: number; v: number; y: number } => {
    const l = toLocal(spot, { x: r.x + bx * c + bz * sn, z: r.z - bx * sn + bz * c });
    return { ...l, y: lift + by };
  };
  const mid = at(0, 0, 0);
  out.push({ kind: 'hoodie', u: mid.u, v: mid.v, y0: lift + r.y + (0.5 - 0.11) * s, y1: lift + r.y + (0.5 + 0.11) * s, r: 0.21 * s });
  // the capsule's round ends: a ball each
  out.push({ kind: 'ball', name: 'hoodie bottom', ...at(0, r.y + (0.5 - 0.11) * s, 0), r: 0.21 * s });
  for (const [sx, a] of [[-0.1, r.legL], [0.1, r.legR]] as const) {
    const rot = (y: number, z: number): [number, number] => [y * Math.cos(a) - z * Math.sin(a), y * Math.sin(a) + z * Math.cos(a)];
    for (const d of [0, -0.08, -0.16, -0.22]) {
      const [y, z] = rot(d, 0);
      out.push({ kind: 'ball', name: 'leg', ...at(sx * s, r.y + (0.3 + y) * s, z * s), r: 0.075 * s });
    }
    for (const fy of [-0.04, 0.04]) for (const fz of [-0.08, 0.04, 0.16]) {
      const [y, z] = rot(-0.27 + fy, fz);
      out.push({ kind: 'ball', name: 'foot', ...at(sx * s, r.y + (0.3 + y) * s, z * s), r: 0.04 * s });
    }
  }
  return out;
}

/** how deep a part goes into a box (<= 0: clear) */
function depth(p: Part, b: Box3): number {
  const du = Math.max(b.u0 - p.u, 0, p.u - b.u1);
  const dv = Math.max(b.v0 - p.v, 0, p.v - b.v1);
  const flat = p.r - Math.hypot(du, dv);
  if (p.kind === 'hoodie') return Math.min(flat, p.y1 - b.y0, b.y1 - p.y0);
  const dy = Math.max(b.y0 - p.y, 0, p.y - b.y1);
  return p.r - Math.hypot(du, dv, dy);
}

/**
 * sat on it: its legs a seated bim's, its middle over the seat (where a seated bim's is, its chair's
 * middle CHAIR.dz behind it, or the seat sliding back from under it as it gets up: sit.ts standBeat)
 */
function onSeat(r: Rig, spot: Spot): boolean {
  const l = toLocal(spot, r);
  const back = r.seat?.back ?? 0;
  const mid = -CHAIR.dz - back;
  return Math.abs(l.u) < 0.015 && l.v >= mid - 0.015 && l.v <= mid + CHAIR.half && r.legL < -1.2;
}

/**
 * main's hoodie is 1.18 times a bim's, wider than a chair's room (made for 0.21): seated, its back
 * touches the backrest by that much (as it always has). Never more, and no other bim's at all.
 */
const excess = (main: boolean): number => (main ? 0.21 * 0.18 : 0) + 1e-3;

/** one event-loop turn: vitest's worker reads its RPC replies only between macrotasks (walklaw.test.ts) */
const turn = (): Promise<void> => new Promise((done) => { setTimeout(done, 0); });

/** what crosses the chair in this rig: '' when nothing */
function crossing(r: Rig, spot: Spot, main: boolean): string {
  const { seat, rest } = chairParts(r.seat?.back ?? 0);
  const sat = onSeat(r, spot);
  for (const p of parts(r, spot, main)) {
    const name = p.kind === 'hoodie' ? 'hoodie' : p.name;
    const dr = depth(p, rest);
    if (dr > excess(main)) return `${name} in the backrest by ${dr.toFixed(3)}`;
    const ds = depth(p, seat);
    if (!sat && ds > 1e-3) return `${name} in the seat by ${ds.toFixed(3)}`;
  }
  return '';
}

describe('the sitting law (bims-sit): never through the chair', () => {
  it('every desk on every map, 1 to 20 bims: its way in comes round its chair, which has room to slide back', async () => {
    const bad: string[] = [];
    for (const map of MAP_IDS) {
      for (let n = 1; n <= 20; n++) {
        const o = office(map, n);
        for (const s of chairSpots(o)) {
          const { G } = sidePoints(s, 1);
          const legs = o.walks.filter(([a, b]) => near(a, s) || near(b, s));
          const ok = legs.length > 0 && legs.every(([a, b]) => near(near(a, s) ? b : a, G));
          if (!ok) bad.push(`${map}/${String(n)} (${s.x.toFixed(2)},${s.z.toFixed(2)}): no way round its chair`);
          if (!pullRoom(o, s)) bad.push(`${map}/${String(n)} (${s.x.toFixed(2)},${s.z.toFixed(2)}): no room to slide its chair back`);
        }
        await turn();
      }
    }
    expect(bad.slice(0, 10)).toEqual([]);
  }, 120_000);

  it('the hop: the chair slides in only under a bim high enough, and lands it seated', () => {
    expect(hopBeat(0)).toMatchObject({ u: 0, v: SIT.gap, y: 0, legs: 0, back: SIT.pull, seated: false });
    expect(hopBeat(1)).toMatchObject({ u: 0, y: 0, legs: 1, seated: true });
    expect(hopBeat(1).v).toBeCloseTo(0, 9);
    expect(hopBeat(1).back).toBeCloseTo(0, 9);
    for (let h = 0; h <= 1; h += 0.01) {
      const b = hopBeat(h);
      const front = -CHAIR.dz - b.back + CHAIR.front;
      // the seat's front under the bim's back: it is up (its hoodie over the seat) or sat on it
      if (front > b.v - 0.21) expect(b.y >= SIT.hop - 0.02 || b.seated, `h=${h.toFixed(2)}`).toBe(true);
    }
  });

  it('the get-up: the chair pushed back before the bim rises, never rising inside it (his m_50679)', () => {
    expect(standBeat(0)).toMatchObject({ u: 0, y: 0, legs: 1, back: 0, seated: true });
    expect(standBeat(1)).toMatchObject({ u: 0, v: SIT.gap, y: 0, legs: 0, back: SIT.pull, seated: false });
    expect(standBeat(1, 0).v).toBe(0);
    let last = -1;
    for (let s = 0; s <= 1.0001; s += 0.01) {
      const b = standBeat(s);
      // the chair only goes back, never in under it again
      expect(b.back).toBeGreaterThanOrEqual(last - 1e-9);
      last = b.back;
      // the seat's front clears its standing legs (a body's radius, 0.15) before they are half down
      const front = -CHAIR.dz - b.back + CHAIR.front;
      if (b.legs < 0.5) expect(front, `s=${s.toFixed(2)}`).toBeLessThanOrEqual(b.v - 0.15);
    }
    // it still sits a moment as the chair starts back: the push comes first
    expect(standBeat(STAND_AT.rise).legs).toBe(1);
    expect(standBeat(STAND_AT.rise).back).toBeGreaterThan(0.2);
  });

  it('a sit and a get-up add their beats as waits on the walk', () => {
    const spot: Spot = { x: 0, z: 0, wz: -0.65, ax: 0, face: 0 };
    const { S, G } = sidePoints(spot, 1);
    const path = makePath([{ x: 3, z: -0.65 }, { x: 0.5, z: -0.65 }, S, G, spot]);
    const sit = { in: { spot, side: 1 as const, standing: false } };
    const w = seatWaits(path, sit);
    expect(w.map((x) => x.s)).toEqual([SIT_S.pull, SIT_S.hop]);
    const seg = { path, speed: 2.4, waits: w, sit };
    expect(sitBeat(seg, 0)).toBeNull();
    const reach = (path.len - Math.abs(SIT.gap) - SIT.side) / 2.4;
    expect(sitBeat(seg, reach + 0.01)?.beat).toMatchObject({ u: SIT.side, facing: true });
  });

  // one test per map and bim count: the 3 counts of a map in one test took over 5 s at gate load
  // (load ~60, bims-vitest); the same loops and samples, each count under its own timeout
  for (const map of MAP_IDS) {
    it.each([1, 5, 12])(`${map}, %i bims: every bim sits down at its desk and gets up without a part of it in its chair`, (n) => {
      const bad: string[] = [];
      {
        const o = office(map, n);
        const names = ['main', ...Array.from({ length: n - 1 }, (_, i) => `a${String(i + 1)}`)];
        for (const who of names) {
          const main = who === 'main';
          const at: Activity['at'] = main ? 'main' : 'desk';
          const spot = main ? o.mainDesk : o.desks.get(who);
          if (!spot) continue;
          const tl = newTimeline();
          const acts = new Map<string, Activity>(names.map((x) => [x, x === 'main' ? { k: 'main', at: 'main' } : { k: 'work', at: 'desk' }]));
          let t = 1000;
          tl.update(new Map(acts), o, t, []);
          // seated at work -> a snack (gets up) -> work (sits) -> asks, standing (steps in) -> a snack (steps out) -> asleep at its desk
          const plan: Activity[] = [
            { k: 'snack', at: 'island', key: 1 },
            main ? { k: 'mainidle', at } : { k: 'work', at },
            main ? { k: 'main', at, doing: 'build' } : { k: 'asks', at },
            { k: 'snack', at: 'island', key: 2 },
            main ? { k: 'main', at } : { k: 'sleep', at },
          ];
          for (const a of plan) {
            acts.set(who, a);
            tl.update(new Map(acts), o, t, []);
            const end = t + 9;
            let said = false;
            for (; t < end; t += 0.02) {
              const pose = tl.poseAt(who, t);
              if (!pose) continue;
              const r = rigOf({ pose, t, main, phase: 0, reduced: false });
              const why = crossing(r, spot, main);
              if (why && !said) {
                bad.push(`${map}/${String(n)} ${who} ${a.k} +${(t - pose.seg.t).toFixed(2)}s: ${why}`);
                said = true;
              }
            }
          }
        }
      }
      expect(bad.slice(0, 6)).toEqual([]);
    });
  }
});
