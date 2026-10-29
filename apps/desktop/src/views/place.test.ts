// The views layer's model: a frame's box and clip over its anchor, which anchor it takes, and which
// live views go past MAX_LIVE.
import { describe, expect, it } from 'vitest';
import { evict, MAX_LIVE, pick, placeOf, samePlace, Z } from './place';

const col = { x: 0, y: 100, w: 600, h: 400 };

describe('placeOf', () => {
  it('a card inside its column: its box, no clip, z 2', () => {
    const p = placeOf('thread', { x: 20, y: 200, w: 560, h: 160 }, [col]);
    expect(p).toEqual({ slot: 'thread', box: { x: 20, y: 200, w: 560, h: 160 }, inset: [0, 0, 0, 0], z: Z.thread, seen: { x: 20, y: 200, w: 560, h: 160 } });
  });

  it('a card scrolled half under the column top is clipped there, and what shows is told', () => {
    const p = placeOf('thread', { x: 20, y: 40, w: 560, h: 160 }, [col]);
    expect(p?.inset).toEqual([60, 0, 0, 0]);
    expect(p?.seen).toEqual({ x: 20, y: 100, w: 560, h: 100 });
  });

  it('clipped by every box that clips it (the column and the window)', () => {
    const p = placeOf('tab', { x: 500, y: 450, w: 300, h: 200 }, [col, { x: 0, y: 0, w: 700, h: 600 }]);
    expect(p?.inset).toEqual([0, 200, 150, 0]);
    expect(p?.z).toBe(Z.tab);
  });

  it('nothing shows: null (scrolled out, no size)', () => {
    expect(placeOf('thread', { x: 20, y: 600, w: 560, h: 160 }, [col])).toBeNull();
    expect(placeOf('thread', { x: 20, y: 200, w: 0, h: 160 }, [col])).toBeNull();
  });
});

describe('pick', () => {
  const thread = { box: { x: 20, y: 200, w: 560, h: 160 }, clips: [col] };
  const tab = { box: { x: 700, y: 52, w: 640, h: 700 }, clips: [] };
  it('inline: over its card; fullscreen: over its tab, never back on the card', () => {
    expect(pick('inline', { thread, tab })?.slot).toBe('thread');
    expect(pick('fullscreen', { thread, tab })?.slot).toBe('tab');
    expect(pick('fullscreen', { thread })).toBeNull();
    expect(pick('inline', { tab })).toBeNull();
  });

  it('the same place twice is the same (no style written)', () => {
    expect(samePlace(pick('inline', { thread }), pick('inline', { thread }))).toBe(true);
    expect(samePlace(pick('inline', { thread }), pick('fullscreen', { tab }))).toBe(false);
    expect(samePlace(null, null)).toBe(true);
  });
});

describe('evict', () => {
  it('past MAX_LIVE, the hidden ones shown longest ago go; a showing one never', () => {
    const live = new Map<string, number>(Array.from({ length: MAX_LIVE + 2 }, (_, i) => [`v${String(i)}`, i]));
    expect(evict(live, new Set())).toEqual(['v0', 'v1']);
    expect(evict(live, new Set(['v0']))).toEqual(['v1', 'v2']);
    expect(evict(new Map([['a', 1]]), new Set())).toEqual([]);
  });
});
