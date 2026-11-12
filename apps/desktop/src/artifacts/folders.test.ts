import { describe, expect, it } from 'vitest';
import type { Artifact } from '@/proto';
import { library } from '@/proto/words_library.gen';
import {
  atOfPath, atPath, chipsOf, countsOf, countWords, coverOf, crumbOf, folderLine, itemsAt, libraryFacts, libraryOf,
  moveRows, parseQuery, pathOf, projectOfTyped, rememberMove, searchItems, titlePath, type Folder,
} from './folders';
import { cardRows, folderRows } from './Menus';
import { moveKeys, walkOf } from './MovePicker';
import { dragKeys, dragPayload, draggedKeys } from './drag';
import { folderCmd, moveCmd } from './ops';
import { scopeRows } from './Scope';
import type { Item } from './model';

const NOW = Date.UTC(2026, 9, 24, 18, 0);
const art = (id: string, over: Partial<Artifact> = {}): Artifact =>
  ({ id, kind: 'file', title: id, agent: 'amb-home', url: '', at_ms: NOW, version: 1, new: false, ...over });
const item = (project: string, a: Artifact): Item => ({ project, projectName: project, a });
// the app the hub puts on a file's row (bend_plugins::opens' BUILTIN): its icon is the card's kind
const app = (type: string, icon: string) => ({ opener: { plugin: 'media', tool: icon, app: 'Preview', type, type_title: type, icon, builtin: true } });

const desk: Folder = { id: 'f-desk', name: 'desktop app', by: 'main', named_by_you: false };
const view: Folder = { id: 'f-view', name: 'agent view', parent: 'f-desk', by: 'main', named_by_you: false };
const hero: Folder = { id: 'f-hero', name: 'page heroes', parent: 'f-desk', by: 'main', named_by_you: false };
const rue: Folder = { id: 'f-rue', name: '36 rue de Montreuil', by: 'you', named_by_you: true };
const FOLDERS = [desk, view, hero, rue];

const ITEMS: Item[] = [
  item('harness', art('step2', { folder: 'f-view', at_ms: NOW - 1000, new: true, path: '/x/step2.png', ...app('*.png', 'image') })),
  item('harness', art('fixes', { folder: 'f-view', at_ms: NOW - 2000, agent: 'amb-win', path: '/x/fixes.png', ...app('*.png', 'image') })),
  item('harness', art('hero-shots', { folder: 'f-hero', at_ms: NOW - 3000, kind: 'page', agent: 'ds-hero' })),
  item('ai-docs', art('dpe', { folder: 'f-rue', at_ms: NOW - 4000, path: '/x/dpe.md', ...app('*.md', 'doc') })),
  item('paint', art('offre', { folder: 'f-rue', at_ms: NOW - 5000, path: '/x/offre.pdf', ...app('*.pdf', 'doc') })),
  item('harness', art('loose', { at_ms: NOW - 6000, agent: 'you', by: 'you' })),
  // a folder gone from the list (deleted): reads as unfiled
  item('harness', art('orphan', { folder: 'f-gone', at_ms: NOW - 7000 })),
];

describe('the library', () => {
  const lib = libraryOf(FOLDERS, ITEMS);

  it('has its themes, newest work first, and their task folders', () => {
    expect(lib.themes.map((f) => f.name)).toEqual(['desktop app', '36 rue de Montreuil']);
    expect(lib.tasks.get('f-desk')?.map((f) => f.name)).toEqual(['agent view', 'page heroes']);
  });

  it('reads a folder missing from the list as unfiled (deleting a folder rewrites no artifact)', () => {
    expect(itemsAt(lib, ITEMS, { at: 'unfiled' }).map((it) => it.a.id)).toEqual(['loose', 'orphan']);
  });

  it('a theme holds its tasks\' artifacts', () => {
    expect(itemsAt(lib, ITEMS, { at: 'folder', id: 'f-desk' }).map((it) => it.a.id)).toEqual(['step2', 'fixes', 'hero-shots']);
    expect(itemsAt(lib, ITEMS, { at: 'folder', id: 'f-view' }).map((it) => it.a.id)).toEqual(['step2', 'fixes']);
  });

  it('a project filter says "14 of 74" on the folders it leaves part of', () => {
    const shown = ITEMS.filter((it) => it.project === 'ai-docs');
    const counts = countsOf(lib, ITEMS, shown);
    expect(countWords(counts.get('f-rue'))).toBe('1 of 2');
    expect(countWords(counts.get('f-desk'))).toBe('0 of 3');
    expect(countWords(countsOf(lib, ITEMS, ITEMS).get('f-desk'))).toBe('3');
  });

  it('counts the new ones per folder, its theme too (the pink dot)', () => {
    const counts = countsOf(lib, ITEMS, ITEMS);
    expect(counts.get('f-view')?.fresh).toBe(1);
    expect(counts.get('f-desk')?.fresh).toBe(1);
    expect(counts.get('f-rue')?.fresh).toBe(0);
  });

  it('says its facts: from how many projects, in how many folders, new, unfiled', () => {
    expect(libraryFacts(lib, ITEMS, '')).toBe('7 from 3 projects, in 2 folders · 1 new · 2 unfiled');
    expect(libraryFacts(lib, ITEMS.filter((it) => it.project === 'ai-docs'), 'ai-docs')).toBe('1 from ai-docs, in 1 folder');
    expect(libraryFacts(lib, [], '')).toBe('nothing yet');
  });

  it("never says 'in 0 folders' while folders exist: nothing filed, the facts leave the folders out (ui-review m_45919)", () => {
    const loose = ITEMS.filter((it) => it.a.id === 'loose');
    expect(libraryFacts(lib, loose, 'harness')).toBe(`${library.count_from_alone({ n: 1, from: 'harness' })} · ${library.unfiled_n({ n: 1 })}`);
    expect(libraryFacts(libraryOf([], loose), loose, '')).toBe(`${library.count_from_alone({ n: 1, from: library.projects_n({ n: 1 }) })} · ${library.unfiled_n({ n: 1 })}`);
  });

  it("a theme's line says 1 task, not '1 tasks' (ui-review m_45919)", () => {
    const docs: Folder = { id: 'f-docs', name: 'docs', by: 'main', named_by_you: false };
    const pricing: Folder = { id: 'f-pricing', name: 'pricing', parent: 'f-docs', by: 'main', named_by_you: false };
    const one = libraryOf([docs, pricing], []);
    expect(folderLine(one, 'f-docs', [])).toBe(`${library.theme_count({ n: 0, tasks: library.tasks_n({ n: 1 }) })} · ${library.theme_filed}`);
    expect(library.tasks_n({ n: 1 })).toBe('1 task');
  });

  it('names paths, crumbs and the title bar', () => {
    expect(pathOf(lib, 'f-view')).toBe('desktop app / agent view');
    expect(crumbOf(lib, { at: 'folder', id: 'f-view' }).map((c) => c.name)).toEqual(['desktop app', 'agent view']);
    expect(titlePath(lib, { at: 'folder', id: 'f-view' }, '')).toBe('artifacts / desktop app / agent view');
    expect(titlePath(lib, { at: 'unfiled' }, '')).toBe('artifacts / Unfiled');
    // searching: the search, whatever place was open
    expect(titlePath(lib, { at: 'folder', id: 'f-view' }, 'dark')).toBe(`artifacts / ${library.crumb_search({ q: 'dark' })}`);
  });

  it('says a folder\'s line: a theme its tasks, a task its agent and the others', () => {
    expect(folderLine(lib, 'f-desk', itemsAt(lib, ITEMS, { at: 'folder', id: 'f-desk' }))).toBe('3 artifacts from 2 tasks · filed by main when each task started');
    expect(folderLine(lib, 'f-view', itemsAt(lib, ITEMS, { at: 'folder', id: 'f-view' }))).toBe("2 · amb-home's work, and 1 more agent");
    expect(folderLine(lib, 'f-hero', itemsAt(lib, ITEMS, { at: 'folder', id: 'f-hero' }))).toBe("1 · ds-hero's work");
  });

  it('covers a big tile with its newest three', () => {
    expect(coverOf(lib, ITEMS, 'f-desk').map((it) => it.a.id)).toEqual(['step2', 'fixes', 'hero-shots']);
  });

  it('keeps two levels at most: a folder under a task folder reads as a theme', () => {
    const deep: Folder = { id: 'f-deep', name: 'too deep', parent: 'f-view', by: 'main', named_by_you: false };
    const l = libraryOf([...FOLDERS, deep], ITEMS);
    expect(l.themes.map((f) => f.id)).toContain('f-deep');
    expect(pathOf(l, 'f-deep')).toBe('too deep');
  });

  it('routes a place and back', () => {
    for (const at of [{ at: 'all' }, { at: 'recent' }, { at: 'unfiled' }, { at: 'archived' }, { at: 'folder', id: 'f-view' }] as const) {
      expect(atOfPath(atPath(at))).toEqual(at);
    }
  });
});

describe('the m picker', () => {
  const lib = libraryOf(FOLDERS, ITEMS);

  it('shows the recent folders first with digits, then every theme, and leaves out the one it is in', () => {
    const rows = moveRows(lib, '', ['f-hero', 'f-view', 'f-rue'], 'f-view');
    expect(rows.recent.map((r) => [r.name, r.where, r.digit])).toEqual([['page heroes', 'desktop app', 1], ['36 rue de Montreuil', '', 2]]);
    expect(rows.all.map((r) => r.name)).toEqual(['desktop app', '36 rue de Montreuil', 'Unfiled']);
    expect(rows.all[0]?.opens).toBe(true);
    expect(rows.make).toBeNull();
  });

  it('filters by what he types and offers to make it when no folder has that name', () => {
    const rows = moveRows(lib, 'pa', [], null);
    expect(rows.all.map((r) => r.name)).toEqual(['page heroes']);
    expect(rows.make).toBe('pa');
    expect(moveRows(lib, 'page heroes', [], null).make).toBeNull();
  });

  it('opens a theme onto its tasks', () => {
    expect(moveRows(lib, '', [], null, 'f-desk').all.map((r) => [r.name, r.digit])).toEqual([['agent view', 1], ['page heroes', 2]]);
  });

  it('walks its rows and says its keys on one line', () => {
    const rows = moveRows(lib, 'pa', [], null);
    expect(walkOf(rows)).toEqual(['a:f-hero', 'new']);
    expect(moveKeys(3, true, false)).toBe('⏎ move   1-3 pick   → open   esc back');
    expect(moveKeys(1, false, true)).toBe('⏎ move   1 pick   ⌘⏎ new folder   esc back');
  });

  it('remembers the last folders, newest first, no repeats', () => {
    expect(rememberMove(['a', 'b', 'c', 'd'], 'c')).toEqual(['c', 'a', 'b', 'd']);
    expect(rememberMove(['a', 'b', 'c', 'd'], 'e')).toEqual(['e', 'a', 'b', 'c']);
  });
});

describe('search across folders', () => {
  const lib = libraryOf(FOLDERS, ITEMS);

  it('reads names, folders, agents and kinds, and in:<project>', () => {
    expect(parseQuery('dark in:ai-docs  shots')).toEqual({ project: 'ai-docs', words: ['dark', 'shots'] });
    expect(searchItems(lib, ITEMS, ['agent', 'view']).map((it) => it.a.id)).toEqual(['step2', 'fixes']);
    expect(searchItems(lib, ITEMS, ['amb-win']).map((it) => it.a.id)).toEqual(['fixes']);
    expect(searchItems(lib, ITEMS, ['pages']).map((it) => it.a.id)).toEqual(['hero-shots']);
  });

  it('counts its kind chips, the most first', () => {
    expect(chipsOf(ITEMS)).toEqual([{ kind: 'files', n: 2 }, { kind: 'docs', n: 2 }, { kind: 'shots', n: 2 }, { kind: 'pages', n: 1 }].sort((x, y) => y.n - x.n || x.kind.localeCompare(y.kind)));
  });

  it('takes a typed project by its id or name, or the one it starts', () => {
    const ps = [{ project: 'ai-docs-1a', name: 'ai-docs' }, { project: 'harness-2b', name: 'harness' }];
    expect(projectOfTyped(ps, 'ai-docs')).toBe('ai-docs-1a');
    expect(projectOfTyped(ps, 'har')).toBe('harness-2b');
    expect(projectOfTyped(ps, 'x')).toBeNull();
  });
});

describe('the scope, the menus, the drag, the commands', () => {
  it('lists the projects with their counts, the most first', () => {
    expect(scopeRows(ITEMS).map((r) => [r.name, r.n])).toEqual([['harness', 5], ['ai-docs', 1], ['paint', 1]]);
  });

  it('a card menu has its keys; several picked: what applies to all', () => {
    expect(cardRows({ finder: true, several: 1 }).map((r) => `${r.name} ${r.key}`)).toEqual([
      'Open ⏎', 'Quick look space', 'Versions v', 'Put in a message @', 'Copy link ⌘C', 'Show in Finder r', 'Move to… m', 'Archive ⌫',
    ]);
    expect(cardRows({ finder: false, several: 1 }).some((r) => r.id === 'finder')).toBe(false);
    expect(cardRows({ finder: true, several: 3 }).map((r) => r.id)).toEqual(['move', 'put', 'archive']);
  });

  it('a folder menu offers a folder inside only on a theme (two levels at most)', () => {
    expect(folderRows(true).map((r) => r.name)).toEqual(['Rename', 'New folder inside', 'Merge into…', 'Delete folder…']);
    expect(folderRows(false).map((r) => r.name)).toEqual(['Rename', 'Merge into…', 'Delete folder…']);
  });

  it('drags the picked ones when the card is one of them, else itself', () => {
    expect(draggedKeys('a', new Set(['a', 'b']))).toEqual(['a', 'b']);
    expect(draggedKeys('c', new Set(['a', 'b']))).toEqual(['c']);
    expect(dragKeys(dragPayload(['h/a', 'h/b']))).toEqual(['h/a', 'h/b']);
    expect(dragKeys('not json')).toEqual([]);
  });

  it('sends a move with each artifact\'s folder before (the undo), and the folder edits as the core takes them', () => {
    const lib = libraryOf(FOLDERS, ITEMS);
    const two = ITEMS.filter((it) => ['step2', 'loose'].includes(it.a.id));
    expect(moveCmd(two, 'f-hero', lib.folderOf)).toEqual({
      cmd: 'folders_move', folder: 'f-hero', items: [{ project: 'harness', id: 'step2', from: 'f-view' }, { project: 'harness', id: 'loose' }],
    });
    expect(moveCmd(two, null, lib.folderOf)).toEqual({ cmd: 'folders_move', items: [{ project: 'harness', id: 'step2', from: 'f-view' }, { project: 'harness', id: 'loose' }] });
    expect(folderCmd({ make: { name: 'offre', parent: 'f-rue' } })).toEqual({ cmd: 'folders', op: { make: { name: 'offre', parent: 'f-rue' } } });
    expect(folderCmd('undo')).toEqual({ cmd: 'folders', op: 'undo' });
  });
});

