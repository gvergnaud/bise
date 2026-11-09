// lineWindow (code-viewer-width): the rows of a long file drawn around the view, in steps.
// Mutation: lineWindow returning { from: 0, to: total } turns each case but the no-layout one red.
import { describe, expect, it } from 'vitest';
import { lineWindow, NO_LAYOUT_ROWS, ROW_STEP } from './lineWindow';

const rowH = 20;
const view = 800; // 40 rows

describe('lineWindow', () => {
  it('at the top of a 30k-line file: the screen and one more, in steps, never the whole file', () => {
    expect(lineWindow({ top: 0, view, rowH, total: 30_000 })).toEqual({ from: 0, to: 2 * ROW_STEP });
  });

  it('scrolled into it: a screen above and below the view, on step edges', () => {
    const s = lineWindow({ top: 10_000 * rowH, view, rowH, total: 30_000 });
    expect(s.from % ROW_STEP).toBe(0);
    expect(s.from).toBeLessThanOrEqual(10_000 - 40);
    expect(s.to).toBeGreaterThanOrEqual(10_000 + 80);
    expect(s.to - s.from).toBeLessThan(300);
  });

  it('a file far below or far above the view draws nothing', () => {
    expect(lineWindow({ top: -50 * view, view, rowH, total: 500 })).toEqual({ from: 0, to: 0 });
    expect(lineWindow({ top: 50 * view, view, rowH, total: 500 })).toEqual({ from: 0, to: 0 });
  });

  it('a small file in view: all its lines', () => {
    expect(lineWindow({ top: 0, view, rowH, total: 12 })).toEqual({ from: 0, to: 12 });
  });

  it('no layout to measure: the first rows', () => {
    expect(lineWindow({ top: 0, view: 0, rowH: Number.NaN, total: 30_000 })).toEqual({ from: 0, to: NO_LAYOUT_ROWS });
  });
});
