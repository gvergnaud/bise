import { describe, expect, it } from 'vitest';
import { vitestWorkers } from './test-workers';

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
