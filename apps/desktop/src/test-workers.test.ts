import { describe, expect, it } from 'vitest';
import { vitestTimeout, vitestWorkers } from './test-workers';

describe('a test timeout stretched by the load (the gate gives the factor, VITEST_LOAD_FACTOR)', () => {
  it('an idle Mac: exactly the 5 s written, never looser', () => {
    expect(vitestTimeout(5000, '1.00', 1)).toBe(5000);
    expect(vitestTimeout(5000, '0.30', 1)).toBe(5000);
  });

  it("the gate's factor wins over the load measured now: one factor for the whole gate", () => {
    expect(vitestTimeout(5000, '2.50', 1)).toBe(12_500);
    expect(vitestTimeout(5000, '1.00', 3.7)).toBe(5000);
  });

  it('at most 4x (load 127 on 12 cpus: 20 s, then red)', () => {
    expect(vitestTimeout(5000, '10.58', 1)).toBe(20_000);
    expect(vitestTimeout(5000, undefined, 10.58)).toBe(20_000);
  });

  it('no factor, or one that is no number: the load measured now', () => {
    expect(vitestTimeout(5000, undefined, 1.3)).toBe(6500);
    expect(vitestTimeout(5000, '', 2)).toBe(10_000);
    expect(vitestTimeout(5000, 'x', 1)).toBe(5000);
  });
});

describe('vitest workers from heavy-run’s share (HEAVY_CPUS)', () => {
  it('outside heavy-run (no HEAVY_CPUS): half the cpus, at most 4, at least 1', () => {
    expect(vitestWorkers(12, undefined)).toBe(4);
    expect(vitestWorkers(4, undefined)).toBe(2);
    expect(vitestWorkers(1, undefined)).toBe(1);
  });

  it('a heavy job alone or with 2 others on 12 cpus: 4; 4 jobs: 3; 5 jobs (the 11-11 reds): 2', () => {
    expect(vitestWorkers(12, '12')).toBe(4);
    expect(vitestWorkers(12, '4')).toBe(4);
    expect(vitestWorkers(12, '3')).toBe(3);
    expect(vitestWorkers(12, '2')).toBe(2);
  });

  it('never under 2 in heavy-run, even when the jobs outnumber the cpus', () => {
    expect(vitestWorkers(12, '1')).toBe(2);
    expect(vitestWorkers(12, '0')).toBe(2);
  });

  it('a HEAVY_CPUS that is no whole number counts as outside heavy-run', () => {
    expect(vitestWorkers(12, '')).toBe(4);
    expect(vitestWorkers(12, 'x')).toBe(4);
  });
});
