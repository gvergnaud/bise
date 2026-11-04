import { describe, expect, it } from 'vitest';
import type { Item } from '@/artifacts/model';
import type { Agent } from '@/proto';
import { FOUND_MAX, NO_ROW, PICK_MAX, enterRow, firstAt, hasChanges, latestArtifacts, launchEnter, launchMatches, launchRows, looksLikeUrl, openRows, recentIn, stepAt, walkAt } from './launch';
import { RECENT_MAX } from './model';
import { changesInView, panelKeyOf, PANEL_KEYS } from './keys';

const item = (id: string, title: string, at: number, kind = 'page', agent = 'perf') =>
  ({ project: 'acme', projectName: 'acme', a: { id, title, kind, agent, at_ms: at, version: 1, url: '', new: false } }) as unknown as Item;

describe("the launcher's search (New tab, mockup 3)", () => {
  const items = [item('a', 'Perf notes', 1), item('b', 'Launch deck', 3, 'deck', 'docs'), item('c', 'perf bench', 2)];
  it('finds by every word in the title, the kind or the maker, newest first', () => {
    expect(launchMatches(items, 'perf').map((x) => x.a.id)).toEqual(['c', 'a']);
    expect(launchMatches(items, 'PERF notes').map((x) => x.a.id)).toEqual(['a']);
    expect(launchMatches(items, 'deck docs').map((x) => x.a.id)).toEqual(['b']);
  });
  it('finds nothing for no words, and caps the list', () => {
    expect(launchMatches(items, '  ')).toEqual([]);
    const many = Array.from({ length: 20 }, (_, i) => item(String(i), 'perf', i));
    expect(launchMatches(many, 'perf')).toHaveLength(FOUND_MAX);
  });
  it('tells an address from a search', () => {
    for (const u of ['localhost:5173', 'https://bise.dev', 'bise.dev/docs', 'http://127.0.0.1:4000/x']) expect(looksLikeUrl(u), u).toBe(true);
    for (const q of ['perf notes', 'perf', '', 'launch deck v2']) expect(looksLikeUrl(q), q).toBe(false);
  });
  it("the Artifacts picker (⌘P): the 10 newest with no words, else what they find", () => {
    const many = Array.from({ length: 14 }, (_, i) => item(String(i), i % 2 === 0 ? 'perf' : 'deck', i, 'page', 'docs'));
    expect(latestArtifacts(many, '').map((x) => x.a.id)).toEqual(['13', '12', '11', '10', '9', '8', '7', '6', '5', '4']);
    expect(latestArtifacts(many, '')).toHaveLength(PICK_MAX);
    expect(latestArtifacts(many, 'perf').map((x) => x.a.id)).toEqual(['12', '10', '8', '6', '4', '2', '0']);
  });
  it('⏎ opens the highlighted artifact, else an address in his browser, else nothing', () => {
    const found = launchMatches(items, 'perf');
    expect(launchEnter(found, 1, 'perf')).toEqual({ artifact: found[1] });
    expect(launchEnter(found, 9, 'perf')).toEqual({ artifact: found[0] });
    expect(launchEnter([], 0, 'bise.dev/docs')).toEqual({ url: 'https://bise.dev/docs' });
    expect(launchEnter([], 0, 'http://localhost:5173')).toEqual({ url: 'http://localhost:5173' });
    expect(launchEnter([], 0, 'perf notes')).toBeUndefined();
  });
  it('↑/↓ walk the rows and stop at the ends', () => {
    expect(stepAt(0, 3, 1)).toBe(1);
    expect(stepAt(2, 3, 1)).toBe(2);
    expect(stepAt(0, 3, -1)).toBe(0);
    expect(stepAt(0, 0, 1)).toBe(0);
  });
});

describe("the New tab's rows (his m_46444, designer m_46449)", () => {
  const items = [item('a', 'Perf notes', 1), item('c', 'perf bench', 2)];
  const open = ['artifacts', 'terminal'] as const;
  const recent = [{ kind: 'office' as const, project: 'acme' }];
  it('Open then Recent with no words; an address first, then the matches, as he types', () => {
    expect(launchRows({ q: '', picker: false, found: [], open, recent })).toEqual([
      { kind: 'open', row: 'artifacts' },
      { kind: 'open', row: 'terminal' },
      { kind: 'recent', e: recent[0] },
    ]);
    const found = launchMatches(items, 'perf');
    expect(launchRows({ q: 'perf', picker: false, found, open, recent }).map((r) => r.kind)).toEqual(['found', 'found']);
    expect(launchRows({ q: 'bise.dev', picker: false, found: [], open, recent })).toEqual([{ kind: 'url', url: 'https://bise.dev' }]);
    // the picker searches artifacts only
    expect(launchRows({ q: 'bise.dev', picker: true, found: [], open, recent })).toEqual([]);
  });
  it('nothing highlighted with an empty field; the first row as he types, and in the picker', () => {
    expect(firstAt('', false)).toBe(NO_ROW);
    expect(firstAt('  ', false)).toBe(NO_ROW);
    expect(firstAt('perf', false)).toBe(0);
    expect(firstAt('', true)).toBe(0);
  });
  it('↓ from the field enters the first row, ↑ on it gives the field back, no wrap at the end', () => {
    expect(walkAt(NO_ROW, 3, 1)).toBe(0);
    expect(walkAt(0, 3, 1)).toBe(1);
    expect(walkAt(2, 3, 1)).toBe(2);
    expect(walkAt(0, 3, -1)).toBe(NO_ROW);
    expect(walkAt(NO_ROW, 3, -1)).toBe(NO_ROW);
    expect(walkAt(NO_ROW, 0, 1)).toBe(NO_ROW);
  });
  it('⏎: the highlighted row; none highlighted: the first of a filtered list, nothing on Open/Recent', () => {
    const rows = launchRows({ q: '', picker: false, found: [], open, recent });
    expect(enterRow(rows, 1, false)).toEqual({ kind: 'open', row: 'terminal' });
    expect(enterRow(rows, NO_ROW, false)).toBeUndefined();
    const typed = launchRows({ q: 'perf', picker: false, found: launchMatches(items, 'perf'), open, recent });
    expect(enterRow(typed, NO_ROW, true)).toEqual(typed[0]);
  });
  it("Recent: only the front project's things (and those of no project), eight at most", () => {
    const e = (project: string, id: string) => ({ kind: 'artifact' as const, project, id });
    const list = [e('acme', 'a'), e('home', 'b'), { kind: 'office' as const, project: 'home' }, { kind: 'plugin' as const, id: 'slack' }, e('acme', 'c')];
    expect(recentIn(list, 'acme')).toEqual([e('acme', 'a'), { kind: 'plugin', id: 'slack' }, e('acme', 'c')]);
    expect(recentIn(list, 'home')).toEqual([e('home', 'b'), { kind: 'office', project: 'home' }, { kind: 'plugin', id: 'slack' }]);
    const many = Array.from({ length: 20 }, (_, i) => e('acme', String(i)));
    expect(recentIn(many, 'acme')).toHaveLength(RECENT_MAX);
  });
});

const agent = (name: string, files?: number) => ({ name, main: name === 'main', status: 'idle', ...(files === undefined ? {} : { changes: { add: 1, del: 0, files } }) }) as unknown as Agent;

describe("Open's rows (r2-launcher)", () => {
  it('Changes only when the agent in view has a diff, Office with a project; no side thread', () => {
    expect(openRows({ changes: true, office: true })).toEqual(['artifacts', 'terminal', 'changes', 'browser', 'office']);
    expect(openRows({ changes: false, office: true })).toEqual(['artifacts', 'terminal', 'browser', 'office']);
    expect(openRows({ changes: false, office: false })).toEqual(['artifacts', 'terminal', 'browser']);
  });
  it("an agent has changes from its rows' diff count", () => {
    const rows = [agent('perf', 3), agent('docs', 0), agent('main')];
    expect(hasChanges(rows, 'perf')).toBe(true);
    expect(hasChanges(rows, 'docs')).toBe(false);
    expect(hasChanges(rows, 'main')).toBe(false);
    expect(hasChanges(rows, 'gone')).toBe(false);
  });
  it('the agent in view with changes: a thread or its review, none on a page', () => {
    const agents = () => [agent('perf', 3), agent('docs')];
    expect(changesInView({ route: { screen: 'thread', project: 'acme', agent: 'perf' }, home: undefined, agents })).toEqual({ project: 'acme', agent: 'perf' });
    expect(changesInView({ route: { screen: 'thread', project: 'acme', agent: 'docs' }, home: undefined, agents })).toBeUndefined();
    expect(changesInView({ route: { screen: 'inbox' }, home: undefined, agents })).toBeUndefined();
  });
});

const key = (code: string, mods: { meta?: boolean; alt?: boolean; shift?: boolean; ctrl?: boolean } = {}) => ({
  code,
  key: '',
  metaKey: mods.meta ?? false,
  altKey: mods.alt ?? false,
  shiftKey: mods.shift ?? false,
  ctrlKey: mods.ctrl ?? false,
});

describe("the panel's keys (native-mac-qa §7, the launcher's rows)", () => {
  it('⌘T New tab; ⌘P Artifacts; ⌘L Browser; ⌥⌘D Changes; ⌥⌘S nothing (no side thread)', () => {
    expect(panelKeyOf(key('KeyT', { meta: true }), false)).toBe('new-tab');
    expect(panelKeyOf(key('KeyP', { meta: true }), false)).toBe('files');
    expect(panelKeyOf(key('KeyL', { meta: true }), false)).toBe('browser');
    expect(panelKeyOf(key('KeyS', { meta: true, alt: true }), false)).toBeUndefined();
    expect(panelKeyOf(key('KeyD', { meta: true, alt: true }), false)).toBe('changes');
  });
  it('⌘⇧[ ] are the open panel\'s only; ⌘W is the main window\'s open or folded (never the window: cmd-w-artifact), the menu\'s in a window of its own', () => {
    expect(panelKeyOf(key('KeyW', { meta: true }), true)).toBe('close-tab');
    expect(panelKeyOf(key('KeyW', { meta: true }), false)).toBe('close-tab');
    expect(panelKeyOf(key('KeyW', { meta: true }), true, true)).toBeUndefined();
    expect(panelKeyOf(key('BracketRight', { meta: true, shift: true }), true)).toBe('next-tab');
    expect(panelKeyOf(key('BracketLeft', { meta: true, shift: true }), true)).toBe('prev-tab');
    expect(panelKeyOf(key('BracketLeft', { meta: true, shift: true }), false)).toBeUndefined();
  });
  it('⌃⇧T reopens the last closed tab, folded or open (his m_46187, as Chrome); ⇧⌘T is the File menu\'s, never the page\'s (designer m_46216)', () => {
    expect(panelKeyOf(key('KeyT', { ctrl: true, shift: true }), false)).toBe('reopen-tab');
    expect(panelKeyOf(key('KeyT', { ctrl: true, shift: true }), true)).toBe('reopen-tab');
    expect(panelKeyOf(key('KeyT', { meta: true, shift: true }), true)).toBeUndefined();
    expect(panelKeyOf(key('KeyT', { ctrl: true, meta: true, shift: true }), true)).toBeUndefined();
    expect(panelKeyOf(key('KeyT', { meta: true, alt: true, shift: true }), true)).toBeUndefined();
  });
  it('leaves the other keys alone (⌘D the changes screen, ⌘K, a plain letter, ⌃T)', () => {
    expect(panelKeyOf(key('KeyD', { meta: true }), true)).toBeUndefined();
    expect(panelKeyOf(key('KeyK', { meta: true }), true)).toBeUndefined();
    expect(panelKeyOf(key('KeyT'), true)).toBeUndefined();
    expect(panelKeyOf(key('KeyT', { ctrl: true, meta: true }), true)).toBeUndefined();
  });
  it('the rows name the keys they answer to', () => {
    expect(PANEL_KEYS).toEqual({ files: '⌘P', terminal: '⌃`', changes: '⌥⌘D', browser: '⌘L', toggle: '⌥⌘B', close: '⌘W', add: '⌘T', reopen: '⇧⌘T' });
  });
});
