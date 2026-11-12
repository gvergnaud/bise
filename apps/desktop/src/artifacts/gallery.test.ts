import { describe, expect, it } from 'vitest';
import type { Artifact } from '@/proto';
import { cardsOf, chipsOf, colsFor, gridStep, lineOf, lineTip, listKind, listName, ownerOf, rowsOf, sheetItems, sheetLine, splitTitle, whenOf, type Card, type Stack } from './gallery';
import type { Item } from './model';
import { library } from '@/proto/words_library.gen';

const NOW = new Date(2026, 9, 22, 19, 0).getTime();
const at = (daysAgo: number, h: number, m: number) => new Date(2026, 9, 22 - daysAgo, h, m).getTime();
const TURN = at(0, 14, 0);
type A = Artifact;
const art = (id: string, extra: Partial<A> = {}): A => ({ id, kind: 'file', title: id, agent: 'amb-home', version: 1, at_ms: at(0, 14, 2), url: '', new: false, ...extra });
const item = (a: A, project = 'harness-1', projectName = 'harness'): Item => ({ project, projectName, a });
const shot = (n: number, words: string, extra: Partial<A> = {}) =>
  item(art(`s${String(n)}`, { kind: 'shot', title: `agent view step 2 · ${words}`, turn: TURN, path: `/x/${words.replaceAll(' · ', '-')}.png`, at_ms: at(0, 14, 2) + 100 - n, ...extra }));
const keys = (cards: readonly Card[]) => cards.map((c) => (c.kind === 'one' ? c.it.a.id : `[${c.title} ${String(c.items.length)}]`));

describe('the cards: one agent, one turn, one title prefix make one stack', () => {
  const shots = [shot(1, 'dark · 1400 · perf'), shot(2, 'dark · 800 · perf'), shot(3, 'light · 1400 · perf'), shot(4, 'light · 800 · search')];

  it('stacks a turn\'s shots where its newest is, keeps the rest as cards', () => {
    const notes = item(art('notes', { at_ms: at(0, 16, 0) }));
    const older = item(art('older', { at_ms: at(0, 9, 0) }));
    expect(keys(cardsOf([notes, ...shots, older]))).toEqual(['notes', '[agent view step 2 4]', 'older']);
  });

  it('never stacks across agents, turns, prefixes, projects, nor without a turn; one alone is a card', () => {
    const other = [
      shot(1, 'a · x'),
      shot(2, 'b · y', { agent: 'architect' }),
      shot(3, 'c · z', { turn: TURN + 1 }),
      item(art('s4', { title: 'agent view step 3 · w', turn: TURN })),
      item(art('s5', { title: 'agent view step 2 · v' })),
      { ...shot(6, 'u'), project: 'ai-docs-2' },
    ];
    expect(keys(cardsOf(other))).toEqual(['s1', 's2', 's3', 's4', 's5', 's6']);
  });

  it('splits a title at its first separator, else drops a trailing number', () => {
    expect(splitTitle('agent view step 2 · dark · 1400')).toEqual({ head: 'agent view step 2', rest: 'dark · 1400' });
    expect(splitTitle('poof variant 12')).toEqual({ head: 'poof variant', rest: 'poof variant 12' });
    expect(splitTitle('DPE 36 rue de Montreuil')).toEqual({ head: 'DPE 36 rue de Montreuil', rest: '' });
  });

  it('a stack opens as a sheet: its line, chips from its names (some share them, not all), one chip keeps its items', () => {
    const s = cardsOf(shots)[0] as Stack;
    expect(sheetLine(s, NOW)).toBe('amb-home saved 4 shots in one turn · today 14:02');
    expect(chipsOf(s)).toEqual(['dark', '1400', 'perf', '800', 'light']);
    expect(sheetItems(s, 'dark').map((it) => it.a.id)).toEqual(['s1', 's2']);
    expect(sheetItems(s, null)).toHaveLength(4);
    expect(listName(s)).toBe('agent view step 2 · 4 shots');
    // an agent's 'file' that is a png is a shot too (the shots' fixture saves pngs as files)
    const files = cardsOf(shots.map((it) => ({ ...it, a: { ...it.a, kind: 'file' } })))[0] as Stack;
    expect(sheetLine(files, NOW)).toBe('amb-home saved 4 shots in one turn · today 14:02');
  });
});

describe("the line under a card: its app, who made it, when; a PR's or a link's says only what the place does not (density rule 1)", () => {
  // the hub's opener for a png (bise's own type until its step lands: docs/plugin-apps.md A3)
  const opener = { plugin: 'media', tool: 'open_media', app: 'Preview', type: '*.png', type_title: 'PNG image', builtin: true };
  const mine = item(art('step', { at_ms: at(0, 14, 2), path: '/x/step.png', opener }));
  const theirs = item(art('target', { agent: 'agent-target', at_ms: at(0, 9, 12), path: '/x/t.png', opener }));
  const his = item(art('offre', { by: 'you', agent: 'main', at_ms: at(0, 10, 0), path: '/x/offre.png', opener }), 'ai-docs-2', 'ai-docs');
  // a link: no file, no app (its ↗ says where it opens)
  const link = (id: string, extra: Partial<A>) => item(art(id, { kind: 'link', url: `https://example.com/${id}`, ...extra }));
  const mineL = link('step', { at_ms: at(0, 14, 2) });
  const theirsL = link('target', { agent: 'agent-target', at_ms: at(0, 9, 12) });
  const oldL = link('folds', { agent: 'amb-win', at_ms: at(9, 11, 0) });
  const hisL = item(art('offre', { kind: 'link', url: 'https://example.com/offre', by: 'you', agent: 'main', at_ms: at(0, 10, 0) }), 'ai-docs-2', 'ai-docs');
  const one = (it: Item): Card => ({ kind: 'one', key: it.a.id, it });
  const folderOf = (it: Item) => (it.a.id === 'step' ? 'desktop app / agent view' : '');

  it("a file, in every place: 'Preview · amb-home · 14:02', 'you' for his own (designer m_61626)", () => {
    const folder = { kind: 'folder' as const, owner: 'amb-home' };
    expect(lineOf(one(mine), folder, NOW)).toBe('Preview · amb-home · 14:02');
    expect(lineOf(one(theirs), { kind: 'recent', folderOf }, NOW)).toBe('Preview · agent-target · 09:12');
    expect(lineOf(one(his), { kind: 'unfiled' }, NOW)).toBe('Preview · you · 10:00');
  });

  it("no app opens it, path or not: 'opens on your Mac · 14:02', without who (designer m_61626)", () => {
    expect(lineOf(one(item(art('zip', { path: '/x/logs.zip' }))), { kind: 'unfiled' }, NOW)).toBe(`${library.opens_mac} · 14:02`);
    expect(lineOf(one(item(art('zip2', { url: '/Users/me/w/logs.zip' }))), { kind: 'unfiled' }, NOW)).toBe(`${library.opens_mac} · 14:02`);
  });

  it("a link in a task's folder: the time alone, another agent's name first, a date for an older day", () => {
    const place = { kind: 'folder' as const, owner: 'amb-home' };
    expect(lineOf(one(mineL), place, NOW)).toBe('14:02');
    expect(lineOf(one(theirsL), place, NOW)).toBe('agent-target · 09:12');
    expect(lineOf(one(oldL), place, NOW)).toBe('amb-win · oct 13');
  });

  it('a link in recent: the folder and the time; unfiled: who and the time; the project only in its tip', () => {
    expect(lineOf(one(mineL), { kind: 'recent', folderOf }, NOW)).toBe('desktop app / agent view · 14:02');
    expect(lineOf(one(hisL), { kind: 'unfiled' }, NOW)).toBe('you · 10:00');
    expect(lineTip(one(his))).toBe('added by you in ai-docs');
    expect(lineTip(one(theirs))).toBe('made by agent-target in harness');
    expect(whenOf(at(1, 23, 59), NOW)).toBe('oct 21');
  });

  it("a folder's owner is the agent that made most of it", () => {
    expect(ownerOf([mine, theirs, item(art('x'))])).toBe('amb-home');
    expect(ownerOf([])).toBeUndefined();
  });

  it("the list's kind in words: the list says it, the card never", () => {
    expect(listKind(one(item(art('a', { path: '/x/a.png' }))))).toBe('image');
    expect(listKind(one(item(art('r', { path: '/x/review.md' }))))).toBe('doc · md');
    expect(listKind(one(item(art('p', { kind: 'page' }))))).toBe('page');
    expect(listKind(one(item(art('pr', { kind: 'pr', url: 'https://github.com/x/y/pull/3' }))))).toBe('PR');
    // designer m_40850: a 3D model says so, any other file its word and extension, never a bare one
    expect(listKind(one(item(art('m', { path: '/x/office.glb' }))))).toBe('3D · glb');
    expect(listKind(one(item(art('d', { path: '/x/bise.dmg' }))))).toBe('file · dmg');
  });
});

describe('the grid: day groups, cols across, the move keys', () => {
  const day = (n: number, d: number) => Array.from({ length: n }, (_, i) => item(art(`d${String(d)}-${String(i)}`, { at_ms: at(d, 12, 0) - i })));
  const cards = cardsOf([...day(5, 0), ...day(2, 9)]);
  const rows = rowsOf(cards, 4, NOW);

  it('cuts each day into rows of cols cards under its title', () => {
    expect(rows.map((r) => (r.kind === 'day' ? r.title : r.cards.length))).toEqual(['today', 4, 1, 'tue 13 oct', 2]);
    expect(rowsOf(cards, 4, NOW, false).map((r) => (r.kind === 'day' ? r.title : r.cards.length))).toEqual([4, 3]);
  });

  it('3 across at 800, 4 at 1100, 5 at 1400', () => {
    expect([800, 1100, 1400].map(colsFor)).toEqual([3, 4, 5]);
  });

  it('←→ one card across rows and days, ↑↓ one row in its column (the shorter row\'s last), clamped', () => {
    const k = (id: string) => `harness-1/${id}`;
    const step = (from: string | null, dir: Parameters<typeof gridStep>[2]) => gridStep(rows, from === null ? null : k(from), dir);
    expect(step('d0-3', 'right')).toBe(k('d0-4'));
    expect(step('d0-4', 'right')).toBe(k('d9-0'));
    expect(step('d0-0', 'left')).toBe(k('d0-0'));
    expect(step('d0-2', 'down')).toBe(k('d0-4'));
    expect(step('d0-4', 'down')).toBe(k('d9-0'));
    expect(step('d9-1', 'up')).toBe(k('d0-4'));
    expect(step('d0-1', 'up')).toBe(k('d0-1'));
    expect(step(null, 'down')).toBe(k('d0-0'));
    // two full rows: ↓ keeps the column
    const full = rowsOf(cardsOf(day(8, 0)), 4, NOW);
    expect(gridStep(full, k('d0-2'), 'down')).toBe(k('d0-6'));
    expect(gridStep(full, k('d0-7'), 'up')).toBe(k('d0-3'));
  });
});
