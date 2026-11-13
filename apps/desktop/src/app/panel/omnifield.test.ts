// The omnibox's field, key by key (omnifield.ts; page newtab-omnibox-flow, designer m_64584): type,
// Tab, ⇧Tab, →/End, ⌫ over the inline, ↑/↓, ⏎, ⌘⏎, esc, ⇧⌫, on a fake web history.
import { describe, expect, it } from 'vitest';
import type { WebHistory, WebPage } from '@/shared/api';
import { omniHits, omniRows } from './omni';
import { clampAt, fieldOf, onInput, onKey, shownOf, type OmniField, type OmniKeyIn } from './omnifield';

const NOW = 1_800_000_000_000;
const page = (url: string, n: number): WebPage => ({ url, n, visits: Array.from({ length: n }, () => ({ at: NOW, typed: false })) });
const WEB: WebHistory = { keep: true, pages: [page('https://github.com/gvergnaud/bise', 3), page('https://gist.github.com/x', 1), page('https://news.ycombinator.com/', 1)] };

/** The list and the field's text for `f`. */
function view(f: OmniField) {
  const hits = omniHits(f.typed, WEB, [], new Set(), NOW);
  const rows = omniRows(f.typed, hits);
  return { hits, rows, shown: shownOf(f, rows, hits) };
}
const k = (key: string, o: Partial<OmniKeyIn> = {}): OmniKeyIn => ({ key, shift: false, mod: false, ...o });
function press(f: OmniField, key: OmniKeyIn) {
  const v = view(f);
  return onKey(f, key, v.rows, v.hits);
}

describe('typing', () => {
  it('a letter: row 1 the prefix hit, its rest inline after his words', () => {
    const f = onInput('gi', 'insert');
    expect(view(f).shown).toEqual({ value: 'github.com/gvergnaud/bise', inline: 'thub.com/gvergnaud/bise' });
    expect(f.at).toBe(0);
  });
  it('⌫ over the inline (a delete), a paste, IME: no inline until a letter', () => {
    expect(view(onInput('gi', 'other')).shown).toEqual({ value: 'gi', inline: '' });
    expect(view(onInput('git', 'insert')).shown.inline).toBe('hub.com/gvergnaud/bise');
  });
  it('no prefix hit: no inline, row 1 is what he typed', () => {
    const f = onInput('ycomb', 'insert');
    expect(view(f).shown).toEqual({ value: 'ycomb', inline: '' });
    expect(view(f).rows[0]?.kind).toBe('search');
  });
});

describe('Tab, ⇧Tab, →/End', () => {
  it('Tab takes the inline; ⇧Tab gives his words back with it', () => {
    const f = onInput('gi', 'insert');
    const t = press(f, k('Tab'));
    expect(t?.f).toEqual({ typed: 'github.com/gvergnaud/bise', at: 0, held: false, before: 'gi' });
    const back = press(t?.f ?? f, k('Tab', { shift: true }));
    expect(back?.f).toEqual(fieldOf('gi'));
    expect(view(back?.f ?? f).shown.inline).toBe('thub.com/gvergnaud/bise');
  });
  it('Tab on a walked row takes its text', () => {
    const f = { ...onInput('gi', 'insert'), at: 2 };
    expect(view(f).rows[2]).toMatchObject({ kind: 'web', page: { url: 'https://gist.github.com/x' } });
    expect(press(f, k('Tab'))?.f.typed).toBe('gist.github.com/x');
  });
  it('nothing to take: Tab stays in the field with words, moves focus with none; ⇧Tab with nothing to undo moves focus', () => {
    expect(press(onInput('ycomb', 'insert'), k('Tab'))?.f).toEqual(onInput('ycomb', 'insert'));
    expect(press(fieldOf(''), k('Tab'))).toBeNull();
    expect(press(onInput('gi', 'insert'), k('Tab', { shift: true }))).toBeNull();
  });
  it('→ and End take the inline; with no inline they are the field’s', () => {
    for (const key of ['ArrowRight', 'End']) {
      expect(press(onInput('gi', 'insert'), k(key))?.f.typed, key).toBe('github.com/gvergnaud/bise');
      expect(press(onInput('ycomb', 'insert'), k(key)), key).toBeNull();
    }
    expect(press(onInput('gi', 'insert'), k('ArrowRight', { shift: true }))).toBeNull();
  });
});

describe('↑/↓, esc', () => {
  it('↓ walks, the field shows the row; ↑ back to row 1 brings his words and the inline back; no wrap', () => {
    let f = onInput('gi', 'insert');
    const n = view(f).rows.length;
    f = press(f, k('ArrowDown'))?.f ?? f;
    expect(f.at).toBe(1);
    expect(view(f).shown).toEqual({ value: 'gi', inline: '' });
    f = press(f, k('ArrowDown'))?.f ?? f;
    expect(view(f).shown.value).toBe('gist.github.com/x');
    for (let i = 0; i < 5; i++) f = press(f, k('ArrowDown'))?.f ?? f;
    expect(f.at).toBe(n - 1);
    for (let i = 0; i < 5; i++) f = press(f, k('ArrowUp'))?.f ?? f;
    expect(f.at).toBe(0);
    expect(view(f).shown.inline).toBe('thub.com/gvergnaud/bise');
  });
  it('esc: first drops a walk or the inline (his words alone, row 1); then it is the launcher’s', () => {
    const walked = { ...onInput('gi', 'insert'), at: 2 };
    const e1 = press(walked, k('Escape'));
    expect(e1?.f).toMatchObject({ at: 0, held: true, typed: 'gi' });
    expect(view(e1?.f ?? walked).shown).toEqual({ value: 'gi', inline: '' });
    expect(press(e1?.f ?? walked, k('Escape'))).toBeNull();
    expect(press(onInput('gi', 'insert'), k('Escape'))?.f.held).toBe(true);
  });
});

describe('⏎, ⌘⏎, ⇧⌫', () => {
  it('⏎ opens, ⌘⏎ opens behind', () => {
    expect(press(onInput('gi', 'insert'), k('Enter'))?.do).toBe('open');
    expect(press(onInput('gi', 'insert'), k('Enter', { mod: true }))?.do).toBe('open-behind');
  });
  it('⇧⌫ on a walked page removes it; on row 1, an artifact, or without ⇧: the field’s', () => {
    const f = { ...onInput('gi', 'insert'), at: 2 };
    expect(press(f, k('Backspace', { shift: true }))?.do).toBe('remove');
    expect(press(f, k('Delete', { shift: true }))?.do).toBe('remove');
    expect(press(f, k('Backspace'))).toBeNull();
    expect(press(onInput('gi', 'insert'), k('Backspace', { shift: true }))).toBeNull();
  });
  it('the highlight stays inside a list that shrank', () => {
    expect(clampAt({ ...fieldOf('x'), at: 4 }, 2).at).toBe(1);
    expect(clampAt({ ...fieldOf('x'), at: 1 }, 2).at).toBe(1);
  });
});
