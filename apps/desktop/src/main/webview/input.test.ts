// What the window and a page's keys give a web tab (webview/input.ts; architect m_64682): main
// checks every call, and owns only its verbs' keys.
import { describe, expect, it } from 'vitest';
import { ENTRIES_MAX, keyIn, keyOf, rectIn, startIn, webPartitionDir, zoomStep } from './input';
import { standInPaths } from '../userdata';

const key = (code: string, o: Partial<{ meta: boolean; control: boolean; alt: boolean; shift: boolean; key: string; type: 'keyDown' | 'keyUp' }> = {}) =>
  ({ type: 'keyDown' as const, key: code, code, meta: false, control: false, alt: false, shift: false, ...o });

describe('keys in a page', () => {
  it("main's verbs: back, forward, reload, zoom; esc stops a load only", () => {
    expect(keyOf(key('BracketLeft', { meta: true }), false, true)).toEqual({ verb: 'back' });
    expect(keyOf(key('BracketRight', { meta: true }), false, true)).toEqual({ verb: 'forward' });
    expect(keyOf(key('KeyR', { meta: true }), false, true)).toEqual({ verb: 'reload' });
    expect(keyOf(key('Equal', { meta: true }), false, true)).toEqual({ verb: 'zoom-in' });
    expect(keyOf(key('Equal', { meta: true, shift: true }), false, true)).toEqual({ verb: 'zoom-in' });
    expect(keyOf(key('Minus', { meta: true }), false, true)).toEqual({ verb: 'zoom-out' });
    expect(keyOf(key('Digit0', { meta: true }), false, true)).toEqual({ verb: 'zoom-reset' });
    expect(keyOf(key('Escape', { key: 'Escape' }), true, true)).toEqual({ verb: 'stop' });
    expect(keyOf(key('Escape', { key: 'Escape' }), false, true)).toBeNull();
  });
  it("the panel's own keys go to the window: ⌘T ⌘L ⌘P, ⇧⌘[ ⇧⌘], ⌥⌘…, ⌃`, ⌃⇧T", () => {
    for (const k of [key('KeyT', { meta: true }), key('KeyL', { meta: true }), key('KeyP', { meta: true }), key('BracketRight', { meta: true, shift: true }), key('KeyB', { meta: true, alt: true }), key('Backquote', { control: true }), key('KeyT', { control: true, shift: true })]) {
      expect(keyOf(k, false, true), k.code).toEqual({ window: true });
    }
  });
  it("the page keeps its own: typing, ⌘C ⌘V ⌘A ⌘Z, arrows, a key up", () => {
    for (const k of [key('KeyA'), key('KeyC', { meta: true }), key('KeyV', { meta: true }), key('KeyA', { meta: true }), key('KeyZ', { meta: true }), key('ArrowLeft', { meta: true }), key('KeyT', { meta: true, type: 'keyUp' })]) {
      expect(keyOf(k, false, true), k.code).toBeNull();
    }
  });
  it('off a Mac: ⌃ is the key', () => {
    expect(keyOf(key('KeyR', { control: true }), false, false)).toEqual({ verb: 'reload' });
    expect(keyOf(key('KeyR', { meta: true }), false, false)).toBeNull();
  });
});

describe('zoom levels', () => {
  it("Chrome's steps, ends held, 0 back to 100 %", () => {
    expect(zoomStep(1, 1)).toBe(1.1);
    expect(zoomStep(1, -1)).toBe(0.9);
    expect(zoomStep(1.05, 1)).toBe(1.1);
    expect(zoomStep(3, 1)).toBe(3);
    expect(zoomStep(0.5, -1)).toBe(0.5);
    expect(zoomStep(2, 0)).toBe(1);
  });
});

describe("the window's calls, checked", () => {
  it('a tab key', () => {
    expect(keyIn('acme|web:1')).toBe('acme|web:1');
    for (const x of ['', 3, null, 'x'.repeat(201)]) expect(keyIn(x)).toBeNull();
  });
  it('a rect: finite numbers, a size, visible a boolean', () => {
    expect(rectIn({ x: 1, y: 2, w: 3, h: 4, visible: true })).toEqual({ x: 1, y: 2, w: 3, h: 4, visible: true });
    for (const r of [null, {}, { x: 1, y: 2, w: -1, h: 4, visible: true }, { x: Number.NaN, y: 2, w: 3, h: 4, visible: true }, { x: 1, y: 2, w: 3, h: 4, visible: 'yes' }]) expect(rectIn(r)).toBeNull();
  });
  it('a start: a URL, or entries of URL and title (form data never), capped', () => {
    expect(startIn({ url: 'https://a.com/', typed: true })).toEqual({ url: 'https://a.com/', typed: true });
    expect(startIn({ entries: [{ url: 'https://a.com/', title: 'A', pageState: 'secret' }, { nope: 1 }], index: 0 })).toEqual({ entries: [{ url: 'https://a.com/', title: 'A' }], index: 0 });
    const many = Array.from({ length: ENTRIES_MAX + 5 }, (_, i) => ({ url: `https://a.com/${String(i)}`, title: '' }));
    const s = startIn({ entries: many, index: 3 });
    expect(s !== null && 'entries' in s ? s.entries.length : 0).toBe(ENTRIES_MAX);
    for (const x of [null, {}, { entries: 'x', index: 0 }]) expect(startIn(x)).toBeNull();
  });
});

describe("the web tabs' cookies stay in a stand-in's own folders (rule 3)", () => {
  it('under its sessionData, never his real Library', () => {
    const REAL = '/Users/gabriel';
    for (const env of [{ HOME: `${REAL}/.bise/gate/w/h` }, { HOME: REAL, BISE_HOME: `${REAL}/.bise/try/desktop` }]) {
      const p = standInPaths(env, REAL, 'bise');
      if (p === null) throw new Error('a stand-in');
      expect(webPartitionDir(p.sessionData).startsWith(p.sessionData)).toBe(true);
      expect(webPartitionDir(p.sessionData).startsWith(`${REAL}/Library`)).toBe(false);
    }
  });
});
