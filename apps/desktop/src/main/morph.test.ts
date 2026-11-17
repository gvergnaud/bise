import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { cubicBezier, EASE_OUT, frameAt, MORPH_MS, retarget, startMorph } from './morph';

const from = { x: 436, y: 170, width: 640, height: 600 };
const to = { x: 196, y: 170, width: 1120, height: 700 };

describe('the capsule frame morph', () => {
  it("moves on the page's own curve: ds/tokens.css's --ease-out", () => {
    const css = readFileSync(join(__dirname, '../ds/tokens.css'), 'utf8');
    const m = /--ease-out:\s*cubic-bezier\(([^)]*)\)/u.exec(css);
    expect(m?.[1]?.split(',').map((s) => Number(s.trim()))).toEqual([...EASE_OUT]);
  });

  it('the curve starts at 0, ends at 1, and is ahead of linear (an ease out)', () => {
    const f = cubicBezier(...EASE_OUT);
    expect(f(0)).toBe(0);
    expect(f(1)).toBe(1);
    expect(f(0.5)).toBeGreaterThan(0.75);
    // a known point: CSS's linear cubic-bezier(0, 0, 1, 1) is the identity
    expect(cubicBezier(0, 0, 1, 1)(0.3)).toBeCloseTo(0.3, 4);
  });

  it('open: from where it was at t=0, past half way at mid time, exactly the end at 220 ms', () => {
    const m = startMorph(from, to, 1000, 'open');
    expect(m.ms).toBe(MORPH_MS.open);
    expect(frameAt(m, 1000)).toEqual({ rect: from, done: false });
    const mid = frameAt(m, 1110).rect;
    // both edges move: the left one left, the right one right, the same 240 px each way
    expect(mid.x).toBeLessThan(from.x - 120);
    expect(mid.x + mid.width).toBeGreaterThan(from.x + from.width + 120);
    expect(frameAt(m, 1220)).toEqual({ rect: to, done: true });
    expect(frameAt(m, 5000)).toEqual({ rect: to, done: true });
  });

  it('close: 160 ms', () => {
    const m = startMorph(to, from, 0, 'close');
    expect(frameAt(m, 159).done).toBe(false);
    expect(frameAt(m, 160)).toEqual({ rect: from, done: true });
  });

  it('a new size mid-move keeps the time and goes to the new end', () => {
    const m = startMorph(from, to, 0, 'open');
    const taller = { ...to, height: 760 };
    const r = retarget(m, taller);
    expect(r.start).toBe(0);
    expect(frameAt(r, 220).rect).toEqual(taller);
    expect(frameAt(r, 110).rect.x).toBe(frameAt(m, 110).rect.x);
  });
});
