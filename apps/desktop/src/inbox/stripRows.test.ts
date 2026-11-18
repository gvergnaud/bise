import { describe, expect, it } from 'vitest';
import type { Card } from '@/proto';
import { cardKeyOf, fitRows, openOnKey, rowWords, scrollToShow, stripKey } from './stripRows';

const card = (id: number, rank: number, project = 'acme', question = `q${String(id)}`): Card => ({
  id,
  project,
  kind: 'question',
  agent: `t${String(id)}`,
  question,
  options: [],
  urgent: false,
  since_ms: id,
  rank,
});

const key = (code: string, mods: Partial<Record<'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey', boolean>> = {}) => ({
  code,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe("the inbox block's rows (W7/K24)", () => {
  it('ctrl+N (ctrl alone, by code) opens row N, as the TUI', () => {
    expect(stripKey(key('Digit2', { ctrlKey: true }))).toBe(2);
    expect(stripKey(key('Digit0', { ctrlKey: true }))).toBeUndefined();
    expect(stripKey(key('Digit2'))).toBeUndefined();
    expect(stripKey(key('Digit2', { ctrlKey: true, metaKey: true }))).toBeUndefined();
    expect(stripKey(key('Digit2', { ctrlKey: true, shiftKey: true }))).toBeUndefined();
  });

  it("a row's words as the TUI's box_row: who, '$ ' and a command's lines, a patch's counts, the first line with words, the note", () => {
    const c = card(7, 0);
    expect(rowWords({ ...c, question: '\n  npm publish?\nit is 2.5.0', note: 'answered\n another way' })).toEqual({ who: 't7', cmd: false, what: 'npm publish?', note: 'answered another way' });
    expect(rowWords({ ...c, kind: 'approval', question: 'npm publish' })).toEqual({ who: 't7', cmd: true, what: 'npm publish', note: '' });
    expect(rowWords({ ...c, kind: 'approval', question: 'make test\n  --all\n\nthe gate' }).what).toBe('make test · 2 lines');
    const patch = 'diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n+x\n-y\n+z\n+++ b/b.rs\n+w';
    expect(rowWords({ ...c, kind: 'approval', question: patch })).toEqual({ who: 't7', cmd: false, what: 'a.rs +1 file · +3 −1', note: '' });
    // the agent alone on every row, whatever the kind (designer m_26234: the sign says what it is)
    expect(rowWords({ ...c, kind: 'feature_merge', question: 'merge sb/perf into main?' }).who).toBe('t7');
  });

  it('the box is up to 5 rows tall, the 5th cut in half when more scroll under it (option A, designer m_66505)', () => {
    // 820 high, 200 px of head and composer, 28 px rows: the thread keeps far more than 40%
    expect(fitRows({ win: 820, fixed: 200, row: 28, items: 2 })).toBe(2);
    expect(fitRows({ win: 820, fixed: 200, row: 28, items: 5 })).toBe(5);
    expect(fitRows({ win: 820, fixed: 200, row: 28, items: 6 })).toBe(4.5);
    expect(fitRows({ win: 820, fixed: 200, row: 28, items: 12 })).toBe(4.5);
  });

  it('a short window gives rows back, 5 → 1, so the thread keeps 40% of it; one whole row at least, never a lone half', () => {
    // 500 high (the thread wants 200): 100 px for the box, 3.5 rows (98) fit, 4.5 (126) do not
    expect(fitRows({ win: 500, fixed: 200, row: 28, items: 12 })).toBe(3.5);
    // 4 items, no cut: 3.5 rows fit where 4 did not
    expect(fitRows({ win: 500, fixed: 200, row: 28, items: 4 })).toBe(3.5);
    expect(fitRows({ win: 450, fixed: 200, row: 28, items: 12 })).toBe(2.5);
    expect(fitRows({ win: 400, fixed: 200, row: 28, items: 12 })).toBe(1);
    expect(fitRows({ win: 300, fixed: 280, row: 28, items: 12 })).toBe(1);
    // nothing waits: no row
    expect(fitRows({ win: 820, fixed: 200, row: 28, items: 0 })).toBe(0);
  });
});

describe('the box scrolls the open item into view (scrollToShow)', () => {
  it('as little as it can: in view it stays, above its top, under its bottom; taller than the box its top', () => {
    // a 126 px box (4.5 rows of 28) scrolled to 56
    expect(scrollToShow({ scrollTop: 56, view: 126, top: 84, height: 28 })).toBe(56);
    expect(scrollToShow({ scrollTop: 56, view: 126, top: 28, height: 28 })).toBe(28);
    expect(scrollToShow({ scrollTop: 56, view: 126, top: 196, height: 28 })).toBe(98);
    expect(scrollToShow({ scrollTop: 0, view: 126, top: 84, height: 200 })).toBe(84);
  });
});

describe("the keys on an open card (cardKeyOf)", () => {
  it('digits that are options pick, ←→ walk, ⌘⌫ closes, esc puts off', () => {
    const k = (k_: string, mods: Partial<Record<'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey', boolean>> = {}) => ({ key: k_, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...mods });
    const c: Card = { ...card(1, 0), options: [{ n: 1, label: "a" }, { n: 2, label: "b" }] };
    expect(cardKeyOf(k('2'), c, null)).toEqual({ t: 'pick', n: 2 });
    expect(cardKeyOf(k('3'), c, null)).toBeNull();
    expect(cardKeyOf(k('ArrowRight'), c, null)).toEqual({ t: 'on', n: 1 });
    expect(cardKeyOf(k('ArrowRight'), c, 2)).toEqual({ t: 'on', n: 2 });
    expect(cardKeyOf(k('ArrowLeft'), c, null)).toEqual({ t: 'on', n: 2 });
    expect(cardKeyOf(k('Backspace', { metaKey: true }), c, null)).toEqual({ t: 'close' });
    expect(cardKeyOf(k('Escape'), c, null)).toEqual({ t: 'later' });
    expect(cardKeyOf(k('2', { ctrlKey: true }), c, null)).toBeNull();
  });

  it('⌘O opens what a card is about, and only on a card about something (openOnKey)', () => {
    const k = (k_: string, mods: Partial<Record<'metaKey' | 'ctrlKey' | 'altKey', boolean>> = {}) => ({ key: k_, metaKey: false, ctrlKey: false, altKey: false, ...mods });
    const about: Card = { ...card(1, 0), artifact: { id: 'commode', kind: 'image', title: 'commode' } };
    expect(openOnKey(k('o', { metaKey: true }), about)).toBe(true);
    expect(openOnKey(k('O', { metaKey: true }), about)).toBe(true);
    expect(openOnKey(k('o'), about)).toBe(false);
    expect(openOnKey(k('o', { metaKey: true, ctrlKey: true }), about)).toBe(false);
    expect(openOnKey(k('o', { metaKey: true }), { ...card(1, 0), artifact: null, page: null })).toBe(false);
  });
});
