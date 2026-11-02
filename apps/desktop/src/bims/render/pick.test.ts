import { describe, expect, it } from 'vitest';
import { pickAt, pickObjectAt, type ObjTarget, type Target } from './pick';

const bim = (name: string, x: number, y: number, desk?: { x: number; y: number }): Target => ({
  name,
  head: { x, y },
  feet: { x, y: y + 30 },
  desk: desk ? { ...desk, r: 20 } : null,
});

describe('pick', () => {
  it('picks the bim whose body is under the point', () => {
    const ts = [bim('a', 50, 50), bim('b', 120, 50)];
    expect(pickAt(ts, 52, 65)).toBe('a');
    expect(pickAt(ts, 118, 79)).toBe('b');
  });

  it('picks the nearest of two close bims', () => {
    expect(pickAt([bim('a', 50, 50), bim('b', 62, 50)], 59, 60)).toBe('b');
  });

  it('falls back to the desk under the point when no body is near', () => {
    const ts: Target[] = [{ name: 'away', head: null, feet: null, desk: { x: 200, y: 100, r: 20 } }, bim('a', 50, 50)];
    expect(pickAt(ts, 205, 110)).toBe('away');
  });

  it('answers null on nothing', () => {
    expect(pickAt([bim('a', 50, 50, { x: 50, y: 90 })], 250, 250)).toBeNull();
  });
});

// his plugins' objects (Gabriel m_51985: hovering one names its plugin): each object's drawn box on the canvas
const box = (key: string, left: number, top: number, right: number, bottom: number): ObjTarget => ({ key, left, top, right, bottom });

describe('pickObjectAt', () => {
  it('picks the object whose drawn box holds the point', () => {
    const ts = [box('gmail', 10, 10, 40, 50), box('slack', 100, 20, 140, 60)];
    expect(pickObjectAt(ts, 20, 30)).toBe('gmail');
    expect(pickObjectAt(ts, 139, 59)).toBe('slack');
  });

  it('of two boxes that hold it, picks the front one (its foot lower on the canvas: nearer the camera)', () => {
    const ts = [box('back', 0, 0, 60, 50), box('front', 20, 20, 80, 70)];
    expect(pickObjectAt(ts, 30, 30)).toBe('front');
    expect(pickObjectAt([...ts].reverse(), 30, 30)).toBe('front');
  });

  it('answers null between the objects', () => {
    expect(pickObjectAt([box('gmail', 10, 10, 40, 50)], 41, 30)).toBeNull();
    expect(pickObjectAt([], 0, 0)).toBeNull();
  });
});
