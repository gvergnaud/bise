import { describe, expect, it } from 'vitest';
import { ENGAGED_MS, UsageClock } from './clock';

const MIN = 60_000;

describe('the usage clock', () => {
  it('counts open, focused and engaged minutes from fake events', () => {
    const c = new UsageClock(0);
    // 2 min open in the back
    c.on('focus', 2 * MIN);
    // focused; he types at 3 min: engaged from 3 to 4 min
    c.on('input', 3 * MIN);
    c.on('blur', 6 * MIN);
    expect(c.take(10 * MIN)).toEqual({ open_s: 600, focused_s: 240, engaged_s: ENGAGED_MS / 1000 });
  });

  it('counts nothing while the Mac sleeps or is locked', () => {
    const c = new UsageClock(0);
    c.on('focus', 0);
    c.on('input', 0);
    c.on('suspend', 30_000);
    c.on('resume', 30 * MIN);
    c.on('lock', 31 * MIN);
    c.on('unlock', 40 * MIN);
    // awake 0-30 s, 30-31 min and 40-41 min: the input ended its engaged minute at 60 s, asleep after 30 s
    expect(c.take(41 * MIN)).toEqual({ open_s: 150, focused_s: 150, engaged_s: 30 });
  });

  it('an input keeps it engaged for a minute after it, no more', () => {
    const c = new UsageClock(0);
    c.on('focus', 0);
    c.on('input', 10_000);
    c.on('input', 50_000);
    expect(c.take(5 * MIN).engaged_s).toBe(100);
  });

  it('keeps the part of a second for the next take: nothing lost, nothing twice', () => {
    const c = new UsageClock(0);
    c.on('focus', 0);
    expect(c.take(1_500)).toEqual({ open_s: 1, focused_s: 1, engaged_s: 0 });
    expect(c.take(3_000)).toEqual({ open_s: 2, focused_s: 2, engaged_s: 0 });
    // a clock that goes back counts nothing
    expect(c.take(2_000)).toEqual({ open_s: 0, focused_s: 0, engaged_s: 0 });
  });
});
