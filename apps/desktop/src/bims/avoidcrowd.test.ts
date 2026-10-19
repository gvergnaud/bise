// les bims: a crowd coming in never freezes the page (avoid-stall). bims/mapsb's crowd step, in
// a test: the office fixture's bims, then 16 or 19 new agents at once on greenhouse, campus and
// nordic. Before, one walk's search graph grew to 21,889 nodes (rings round bims standing close
// together cutting each other) and one update ran for minutes: the page's main thread stalled
// and the right bar's buttons took no click. Each update's CPU time stays under UPDATE_MS: the
// walk law's own updates (20 bims) take 65 ms at most, the crowd's 300 ms at most at load 30.
import { describe, expect, it } from 'vitest';
import { activities } from './activity';
import { crowd, office } from './fixtures';
import { layoutOf } from './maps';
import { newTimeline } from './timeline';

/** one update's CPU time at most, ms */
const UPDATE_MS = 750;

describe('a crowd coming in (avoid-stall): every update stays short', () => {
  for (const map of ['greenhouse', 'campus', 'nordic'] as const) {
    for (const n of [16, 19]) {
      it(`${map}: ${String(n + 1)} agents at once`, () => {
        const tl = newTimeline();
        let o = layoutOf(office.agents, map);
        let t = 1000;
        tl.update(activities(office, t), o, t, []);
        t += 5;
        const s = crowd(n);
        o = layoutOf(s.agents, map, o);
        const slow: string[] = [];
        for (let k = 0; k < 8; k++, t += 1) {
          const c = process.cpuUsage();
          tl.update(activities(s, t), o, t, []);
          const ms = process.cpuUsage(c).user / 1000;
          if (ms > UPDATE_MS) slow.push(`update ${String(k)}: ${ms.toFixed(0)} ms`);
        }
        expect(slow).toEqual([]);
        expect(tl.names(t).length).toBeGreaterThan(0);
      }, 30_000);
    }
  }
});
