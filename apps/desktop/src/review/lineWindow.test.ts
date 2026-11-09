// lineWindow (code-viewer-width): the lines of a long file drawn around the view, in steps, a
// wrapped line counted for its rows (designer m_57777). Mutations: lineWindow returning
// { from: 0, to: total } turns the window cases but the no-layout one red; lineRows returning 1
// turns the wrap cases red; lineRows splitting anywhere (no words) turns the words case red.
import { describe, expect, it } from 'vitest';
import { lineRows, lineWindow, NO_LAYOUT_ROWS, ROW_STEP, rowStarts } from './lineWindow';

const rowH = 20;
const view = 800; // 40 rows
const flat = (n: number) => rowStarts(Array.from({ length: n }, () => 1));

describe('lineWindow', () => {
  it('at the top of a 30k-line file: the screen and one more, in steps, never the whole file', () => {
    expect(lineWindow({ top: 0, view, rowH, starts: flat(30_000) })).toEqual({ from: 0, to: 2 * ROW_STEP });
  });

  it('scrolled into it: a screen above and below the view, on step edges', () => {
    const s = lineWindow({ top: 10_000 * rowH, view, rowH, starts: flat(30_000) });
    expect(s.from % ROW_STEP).toBe(0);
    expect(s.from).toBeLessThanOrEqual(10_000 - 40);
    expect(s.to).toBeGreaterThanOrEqual(10_000 + 80);
    expect(s.to - s.from).toBeLessThan(300);
  });

  it('wrapped lines above the view move the window up: the lines there, not the rows', () => {
    // 1000 lines of 3 rows each: row 1500 is line 500
    const starts = rowStarts(Array.from({ length: 3000 }, (_, i) => (i < 1000 ? 3 : 1)));
    const s = lineWindow({ top: 1500 * rowH, view, rowH, starts });
    expect(s.from).toBeLessThanOrEqual(500 - 40 / 3);
    expect(s.to).toBeGreaterThan(500);
    expect(s.to).toBeLessThan(1000);
  });

  it('a file far below or far above the view draws nothing', () => {
    expect(lineWindow({ top: -50 * view, view, rowH, starts: flat(500) })).toEqual({ from: 0, to: 0 });
    expect(lineWindow({ top: 50 * view, view, rowH, starts: flat(500) })).toEqual({ from: 0, to: 0 });
  });

  it('a small file in view: all its lines', () => {
    expect(lineWindow({ top: 0, view, rowH, starts: flat(12) })).toEqual({ from: 0, to: 12 });
  });

  it('no layout to measure: the first lines', () => {
    expect(lineWindow({ top: 0, view: 0, rowH: Number.NaN, starts: flat(30_000) })).toEqual({ from: 0, to: NO_LAYOUT_ROWS });
  });
});

describe('lineRows', () => {
  it('a line that fits is one row; a 300-column one wraps, its next rows at its indent', () => {
    expect(lineRows('const a = 1;', 80)).toBe(1);
    expect(lineRows('x'.repeat(80), 80)).toBe(1);
    expect(lineRows('x'.repeat(300), 80)).toBe(4); // 80 + 80 + 80 + 60
    // 4 spaces of indent: the next rows hold 76 columns: 80 + 76 + 76 + 68
    expect(lineRows(`    ${'x'.repeat(296)}`, 80)).toBe(4);
    expect(lineRows(`    ${'x'.repeat(306)}`, 80)).toBe(5);
  });

  it('words wrap at their spaces first (designer m_57796): a word that does not fit starts the next row', () => {
    // 27 columns at 10 a row: 3 rows split anywhere, 4 by words
    expect(lineRows('aaaaaa bbbbbb cccccc dddddd', 10)).toBe(4);
    // 'host-' | '1.example.' | 'com': after the hyphen, then the rest split as it must
    expect(lineRows('host-1.example.com', 10)).toBe(3);
  });

  it('a tab goes to its stop; an indent over half the cell counts as half', () => {
    expect(lineRows(`\t${'x'.repeat(72)}`, 80)).toBe(1);
    expect(lineRows(`${' '.repeat(70)}${'x'.repeat(60)}`, 80)).toBe(3); // 130 columns: 80, then 40 a row
  });

  it("a hunk's head, or no width measured: one row", () => {
    expect(lineRows('@@ '.repeat(100), 80, true)).toBe(1);
    expect(lineRows('x'.repeat(300), 0)).toBe(1);
  });
});
