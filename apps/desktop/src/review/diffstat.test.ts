// A PR's diffstat (diffstat.ts): the totals over every file, GitHub's five squares.
import { describe, expect, it } from 'vitest';
import { diffStat } from './diffstat';

describe('a diffstat', () => {
  it('sums every file, a cut one too, and rounds each share down (his GitHub shot: +438 −151)', () => {
    const s = diffStat([{ add: 400, del: 100 }, { add: 38, del: 51 }, { add: 0, del: 0 }]);
    expect([s.add, s.del]).toEqual([438, 151]);
    expect(s.squares).toEqual(['add', 'add', 'add', 'del', 'none']);
  });

  it('only added lines: five green; only removed: five red', () => {
    expect(diffStat([{ add: 20, del: 0 }]).squares).toEqual(['add', 'add', 'add', 'add', 'add']);
    expect(diffStat([{ add: 0, del: 3 }]).squares).toEqual(['del', 'del', 'del', 'del', 'del']);
  });

  it('as many added as removed: two of each, one neutral', () => {
    expect(diffStat([{ add: 7, del: 7 }]).squares).toEqual(['add', 'add', 'del', 'del', 'none']);
  });

  it('no file, or no line changed (a binary): zero and zero, five neutral', () => {
    expect(diffStat([])).toEqual({ add: 0, del: 0, squares: ['none', 'none', 'none', 'none', 'none'] });
    expect(diffStat([{ add: 0, del: 0 }]).squares).toEqual(['none', 'none', 'none', 'none', 'none']);
  });
});
