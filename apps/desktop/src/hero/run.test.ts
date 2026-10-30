// The film replays: its first loop's every bim (where it stands, what it does, its egg) is its
// second loop's, frame for frame, and the score's beats land on the poses they were written for.
import { describe, expect, it } from 'vitest';
import { FILM_S, KILN, NOTES, REVIEWER } from './cast';
import { advance, FEED_X, makeRuns, stepOf } from './run';
import { ORIGIN } from './world';

/** every bim of every office at each 0.25 s of film time, from world time `from` for one loop */
function play(from: number, runs = makeRuns()): Map<number, string> {
  const out = new Map<number, string>();
  for (let k = 0; k < FILM_S * 4; k++) {
    const t = from + k / 4;
    const row: string[] = [];
    for (const r of runs) {
      advance(r, t, false, false);
      for (const f of stepOf(r, false, false)?.played ?? []) {
        row.push(`${r.cast.id}/${f.name}:${f.pose.seg.act.k}:${f.pose.x.toFixed(3)},${f.pose.z.toFixed(3)}:${f.egg?.kind ?? ''}`);
      }
    }
    out.set(k, row.join('|'));
  }
  return out;
}

describe('the hero film', () => {
  it('plays the same loop twice: every bim where it was, doing what it did', () => {
    const runs = makeRuns();
    const one = play(ORIGIN, runs);
    const two = play(ORIGIN + FILM_S, runs);
    const diff = [...one].filter(([k, v]) => two.get(k) !== v).map(([k]) => k / 4);
    expect(diff).toEqual([]);
  });

  it('flies every plane of his to its catch: a newer one never removes it (designer m_32877)', () => {
    const runs = makeRuns();
    const missing: string[] = [];
    // every 0.25 s of each flight, in film order (a run only goes forward)
    const checks = NOTES.flatMap((n) => {
      const fl = n.flight;
      if (n.from !== 'you' || fl === undefined) return [];
      return Array.from({ length: Math.ceil((fl.dur - 0.05) / 0.25) }, (_, k) => ({ n, dur: fl.dur, s: 0.05 + k * 0.25 }));
    }).sort((a, b) => a.n.at + a.s - (b.n.at + b.s));
    for (const { n, dur, s } of checks) {
      for (const r of runs) advance(r, ORIGIN + n.at + s, false, false);
      const r = runs.find((x) => x.cast.id === n.office);
      const f = r ? stepOf(r, false, false)?.played.find((x) => x.name === n.to) : undefined;
      const all = [...(f?.pose.aloft ?? []), ...(f?.pose.plane ? [f.pose.plane] : [])];
      if (!all.some((p) => Math.abs(p.since - s) < 0.06 && p.dur === dur && p.glide === true)) missing.push(`${n.to}'s plane of ${String(n.at)} at +${s.toFixed(2)} s`);
    }
    expect(missing).toEqual([]);
  });

  it('lands its beats in the office brains (bims-film v15): paper planes, pops at the desk, visits, the queue, the stamp', () => {
    const runs = makeRuns();
    const at = (T: number, name: string) => {
      for (const r of runs) advance(r, ORIGIN + T, false, false);
      const r = runs.find((x) => x.cast.id === KILN);
      return r ? stepOf(r, false, false)?.played.find((f) => f.name === name) : undefined;
    };
    // paper planes from his send button: 2.5 s the first (it leaves at 4.8, caught at 7.3), 2.8 s the next, gliding, never a loop
    expect(at(5, 'main')?.pose.plane).toMatchObject({ dur: 2.5, loops: 0, startX: FEED_X, glide: true });
    expect(at(8.5, 'main')?.pose.plane).toMatchObject({ dur: 2.8, loops: 0, glide: true });
    // an agent pops at its desk, never walking in by the door
    const catalog = at(10.75, 'catalog');
    expect(catalog?.pose.moving).toBe(false);
    expect(catalog?.pose.seg.act.at).toBe('desk');
    // the visits: there before their bubbles
    expect(at(19.1, 'checkout')?.pose.seg.act.k).toBe('visit');
    expect(at(43.2, 'order-test')?.pose.seg.act.k).toBe('visit');
    expect(at(44.1, 'order-test')?.egg?.kind).toBe('hop');
    // its visit over before checkout gets up for its crate (designer m_50704: its head brushed checkout's)
    expect(at(44.6, 'order-test')?.pose.seg.act.k).not.toBe('visit');
    // checkout's crate at main by 0:52.1, every other one by 0:56.5
    expect(at(52.1, 'checkout')).toMatchObject({ pose: { moving: false, seg: { act: { k: 'landing' } } } });
    for (const n of ['catalog', 'cart', 'emails', 'theme', 'order-test']) expect(at(56.5, n)).toMatchObject({ pose: { moving: false, seg: { act: { k: 'landing' } } } });
    // the reviewer beside the first crate, its stamp; main's crate over its head
    const review = at(58, REVIEWER);
    expect(review?.egg?.kind).toBe('stamp');
    expect(review?.pose.moving).toBe(false);
    expect(at(60, 'main')?.egg?.kind).toBe('itemget');
  });
});
