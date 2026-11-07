// The library's folders, read side (artifacts-folders brief part 4, page artifacts-gallery v7): the
// one folder list every project shares (~/.bise/library/folders.json, art-store's; it comes on each
// project's artifacts event) joined with each artifact's resolved folder id. Where he is (all
// artifacts, recent, unfiled, a theme, a task's folder), the column's rows and counts ('14 of 74'
// under a project filter), the pink dot, the headers and crumbs, the hero's facts, the tiles'
// covers, the m picker's rows, the search across folders (grouped by folder, 4 a group, kind chips)
// and its nearest match. Pure: the screen (Screen.tsx) and its parts call these. Owner: art-folders.
import { iconKindOf, sentenceCase, type IconKind } from '@/ds';
import { library } from '@/proto/words_library.gen';
import { hero } from '@/proto/words_hero.gen';
import type { Folder } from '@/proto';
import type { Item } from './model';
import { prSettled } from './pr';

/** One folder of the library (bise_proto Folder): a theme (no parent) or a task folder in one, two levels at most. */
export type { Folder } from '@/proto';

/** The folder an artifact is in (the hub resolves merges; a folder gone from the list reads as unfiled). */
export function folderIdOf(it: Item, known: ReadonlyMap<string, Folder>): string | null {
  const id = it.a.folder ?? null;
  return id !== null && known.has(id) ? id : null;
}

/** Where he is on the page. */
export type At =
  | { readonly at: 'all' }
  | { readonly at: 'recent' }
  | { readonly at: 'unfiled' }
  | { readonly at: 'folder'; readonly id: string }
  /** the archived ones, every folder's ('a' toggles it; an artifact gone from the list links here, gone.ts) */
  | { readonly at: 'archived' };

export const AT_ALL: At = { at: 'all' };

/** The route's tail for a place ('', 'recent', 'unfiled', 'archived', 'f/<id>') and back. Pure. */
export function atPath(at: At): string {
  return at.at === 'all' ? '' : at.at === 'folder' ? `f/${at.id}` : at.at;
}
export function atOfPath(p: string): At {
  if (p === 'recent' || p === 'unfiled' || p === 'archived') return { at: p };
  if (p.startsWith('f/') && p.length > 2) return { at: 'folder', id: p.slice(2) };
  return AT_ALL;
}

/** The library: the folders by id, their themes and tasks, each item's folder. Built once per render. */
export interface Library {
  readonly byId: ReadonlyMap<string, Folder>;
  /** the themes, newest work first, then by name */
  readonly themes: readonly Folder[];
  /** a theme's task folders, newest work first */
  readonly tasks: ReadonlyMap<string, readonly Folder[]>;
  /** each item's folder id, null unfiled (by keyOf) */
  readonly folderOf: (it: Item) => string | null;
  /** the newest artifact's time in a folder (a theme counts its tasks'), 0 none */
  readonly latest: ReadonlyMap<string, number>;
}

/** The library of these folders and items (every project's: the counts' totals). Pure. */
export function libraryOf(folders: readonly Folder[], items: readonly Item[]): Library {
  const byId = new Map(folders.map((f) => [f.id, f]));
  // a parent that is not a theme (or gone): the folder reads as a theme (two levels at most)
  const themeOf = (f: Folder): string | null => {
    const p = f.parent ?? null;
    if (p === null) return null;
    const pf = byId.get(p);
    return pf !== undefined && (pf.parent ?? null) === null ? p : null;
  };
  const folderOf = (it: Item) => folderIdOf(it, byId);
  const latest = new Map<string, number>();
  const bump = (id: string, ms: number) => {
    if ((latest.get(id) ?? 0) < ms) latest.set(id, ms);
  };
  for (const it of items) {
    const id = folderOf(it);
    if (id === null) continue;
    bump(id, it.a.at_ms);
    const f = byId.get(id);
    const t = f === undefined ? null : themeOf(f);
    if (t !== null) bump(t, it.a.at_ms);
  }
  const order = (x: Folder, y: Folder) => (latest.get(y.id) ?? 0) - (latest.get(x.id) ?? 0) || x.name.localeCompare(y.name);
  const themes = folders.filter((f) => themeOf(f) === null).sort(order);
  const tasks = new Map<string, Folder[]>();
  for (const f of folders) {
    const t = themeOf(f);
    if (t === null) continue;
    const list = tasks.get(t) ?? [];
    list.push(f);
    tasks.set(t, list);
  }
  for (const list of tasks.values()) list.sort(order);
  // the parent the reads go by: a folder whose parent is not a theme is one
  const fixed = new Map([...byId].map(([id, f]): [string, Folder] => {
    if (themeOf(f) === (f.parent ?? null)) return [id, f];
    return [id, { id: f.id, name: f.name, by: f.by, named_by_you: f.named_by_you }];
  }));
  return { byId: fixed, themes, tasks, folderOf, latest };
}

/** A folder's theme: itself when it is one. */
export function themeIdOf(lib: Library, id: string): string {
  return lib.byId.get(id)?.parent ?? id;
}

/** Is the item in this folder: in it, or in one of its tasks when it is a theme. Pure. */
export function inFolder(lib: Library, it: Item, id: string): boolean {
  const f = lib.folderOf(it);
  return f !== null && (f === id || lib.byId.get(f)?.parent === id);
}

/** The items of a place, newest first ('recent': the newest across folders, 40 at most). Pure. */
export function itemsAt(lib: Library, items: readonly Item[], at: At): Item[] {
  const sorted = [...items].sort((x, y) => y.a.at_ms - x.a.at_ms);
  switch (at.at) {
    case 'all':
    case 'archived':
      // archived: the page hands the archived ones in (Screen.tsx splitArchived)
      return sorted;
    case 'recent':
      return sorted.slice(0, RECENT_N);
    case 'unfiled':
      return sorted.filter((it) => lib.folderOf(it) === null);
    case 'folder':
      return sorted.filter((it) => inFolder(lib, it, at.id));
  }
}

/** How many cards 'recent' shows (all artifacts' recent row shows the first ones). */
export const RECENT_N = 40;

/** A folder's count: shown under the filter, and its total ('14 of 74'), and how many new. */
export interface Count {
  readonly n: number;
  readonly total: number;
  readonly fresh: number;
}

/** Every folder's count, plus 'unfiled' and 'all' (keys: folder id, 'unfiled', 'all'). Pure. */
export function countsOf(lib: Library, all: readonly Item[], shown: readonly Item[]): ReadonlyMap<string, Count> {
  const out = new Map<string, { n: number; total: number; fresh: number }>();
  const get = (k: string) => {
    let c = out.get(k);
    if (c === undefined) {
      c = { n: 0, total: 0, fresh: 0 };
      out.set(k, c);
    }
    return c;
  };
  const shownKeys = new Set(shown.map((it) => `${it.project}/${it.a.id}`));
  const add = (k: string, it: Item, isShown: boolean) => {
    const c = get(k);
    c.total += 1;
    if (isShown) {
      c.n += 1;
      if (it.a.new) c.fresh += 1;
    }
  };
  for (const f of lib.byId.keys()) get(f);
  get('unfiled');
  get('all');
  for (const it of all) {
    const isShown = shownKeys.has(`${it.project}/${it.a.id}`);
    add('all', it, isShown);
    const id = lib.folderOf(it);
    if (id === null) {
      add('unfiled', it, isShown);
      continue;
    }
    add(id, it, isShown);
    const p = lib.byId.get(id)?.parent ?? null;
    if (p !== null) add(p, it, isShown);
  }
  return out;
}

/** A count's words: '74', or '14 of 74' when a project filter leaves some out. Pure. */
export function countWords(c: Count | undefined): string {
  if (c === undefined) return '0';
  return c.n === c.total ? String(c.total) : library.of_total({ n: c.n, total: c.total });
}

/** A folder's name, '' when it is gone. */
export function nameOf(lib: Library, id: string): string {
  return lib.byId.get(id)?.name ?? '';
}

/** A folder's path, 'desktop app / agent view'. Pure. */
export function pathOf(lib: Library, id: string): string {
  const f = lib.byId.get(id);
  if (f === undefined) return '';
  const p = f.parent ?? null;
  return p === null ? f.name : `${nameOf(lib, p)} / ${f.name}`;
}

/** A place's crumb parts after 'artifacts': [] all, ['recent'], ['unfiled'], ['desktop app', 'agent view']. Pure. */
export function crumbOf(lib: Library, at: At): { name: string; at: At }[] {
  switch (at.at) {
    case 'all':
    case 'archived':
      return [];
    case 'recent':
      return [{ name: library.recent, at }];
    case 'unfiled':
      return [{ name: sentenceCase(library.unfiled), at }];
    case 'folder': {
      const f = lib.byId.get(at.id);
      if (f === undefined) return [];
      const p = f.parent ?? null;
      const theme = p === null ? [] : [{ name: nameOf(lib, p), at: { at: 'folder', id: p } as At }];
      return [...theme, { name: f.name, at }];
    }
  }
}

/** The title bar's path: 'artifacts / desktop app / agent view'. Pure. */
export function titlePath(lib: Library, at: At, searched: string): string {
  const parts = searched !== '' ? [library.crumb_search({ q: searched })] : crumbOf(lib, at).map((c) => c.name);
  return [library.crumb_root, ...parts].join(' / ');
}

/**
 * The hero's facts line: '893 from 6 projects, in 9 folders · 12 new · 23 unfiled'; filtered to one
 * project: '33 from ai-docs, in 3 folders · 1 new'. Pure.
 */
export function libraryFacts(lib: Library, live: readonly Item[], projectName: string): string {
  if (live.length === 0) return hero.nothing_yet;
  const projects = new Set(live.map((it) => it.project)).size;
  const used = new Set<string>();
  for (const it of live) {
    const id = lib.folderOf(it);
    if (id === null) continue;
    used.add(themeIdOf(lib, id));
  }
  const folders = projectName === '' ? lib.themes.length : used.size;
  const from = projectName === '' ? library.projects_n({ n: projects }) : projectName;
  // nothing of it in a folder: no 'in 0 folders' (the column has folders, the page says unfiled)
  const parts = [folders === 0 ? library.count_from_alone({ n: live.length, from }) : library.count_from({ n: live.length, from, inside: library.in_folders({ n: folders }) })];
  const fresh = live.filter((it) => it.a.new).length;
  if (fresh > 0) parts.push(hero.new_ones({ n: fresh }));
  const unfiled = live.filter((it) => lib.folderOf(it) === null).length;
  if (unfiled > 0) parts.push(library.unfiled_n({ n: unfiled }));
  return parts.join(' · ');
}

/** A folder header's line: a theme '412 artifacts from 15 tasks · filed by main…', a task '48 · amb-home's work, and 2 more agents', his own 'a folder you made'. Pure. */
export function folderLine(lib: Library, id: string, items: readonly Item[]): string {
  const f = lib.byId.get(id);
  if (f === undefined) return '';
  const isTheme = (f.parent ?? null) === null;
  if (isTheme && (lib.tasks.get(id)?.length ?? 0) > 0) {
    const tasks = lib.tasks.get(id)?.length ?? 0;
    const head = library.theme_count({ n: items.length, tasks: library.tasks_n({ n: tasks }) });
    return f.by === 'you' ? head : `${head} · ${library.theme_filed}`;
  }
  if (items.length === 0) return f.by === 'you' ? library.made_by_you : '';
  const byAgent = new Map<string, number>();
  for (const it of items) byAgent.set(it.a.agent, (byAgent.get(it.a.agent) ?? 0) + 1);
  const ranked = [...byAgent].sort((x, y) => y[1] - x[1] || x[0].localeCompare(y[0]));
  const top = ranked[0]?.[0] ?? '';
  const more = ranked.length - 1;
  const who = top === 'you' ? library.made_by_you : more > 0 ? `${library.work_of({ agent: top })}, ${library.and_more_agents({ n: more })}` : library.work_of({ agent: top });
  return `${String(items.length)} · ${who}`;
}

/** The agent most of a folder's work is by ('' none): the cards leave its name out (density rule 1). Pure. */
export function mainAgentOf(items: readonly Item[]): string {
  const byAgent = new Map<string, number>();
  for (const it of items) byAgent.set(it.a.agent, (byAgent.get(it.a.agent) ?? 0) + 1);
  return [...byAgent].sort((x, y) => y[1] - x[1] || x[0].localeCompare(y[0]))[0]?.[0] ?? '';
}

/** A big tile's cover: its newest three artifacts. Pure. */
export function coverOf(lib: Library, items: readonly Item[], id: string): Item[] {
  const out: Item[] = [];
  for (const it of items) {
    if (out.length === 3) break;
    if (inFolder(lib, it, id)) out.push(it);
  }
  return out;
}

// ---- the m picker

/** One row of the move-to picker. */
export interface MoveRow {
  /** a folder id; 'unfiled' takes it out of its folder; 'new' makes the typed folder */
  readonly id: string;
  readonly name: string;
  /** its theme's name, at the row's right ('desktop app') */
  readonly where: string;
  /** 1-9: its digit */
  readonly digit?: number;
  /** a theme with tasks: → opens it */
  readonly opens?: boolean;
}

/** The picker's groups: typed nothing: recent (the last folders he moved to, 3 at most), then every theme; typed: the matches, then '+ new folder "pa"'. Pure. */
export function moveRows(lib: Library, typed: string, recentIds: readonly string[], from: string | null, inside: string | null = null): { recent: MoveRow[]; all: MoveRow[]; make: string | null } {
  const q = typed.trim().toLowerCase();
  const row = (f: Folder): MoveRow => {
    const p = f.parent ?? null;
    const base = { id: f.id, name: f.name, where: p === null ? '' : nameOf(lib, p) };
    return (lib.tasks.get(f.id)?.length ?? 0) > 0 ? { ...base, opens: true } : base;
  };
  const number = (rows: MoveRow[], start: number): MoveRow[] => rows.map((r, i) => (start + i <= 9 ? { ...r, digit: start + i } : r));
  if (q === '') {
    const recent = recentIds.filter((id) => id !== from && lib.byId.has(id)).slice(0, 3).map((id) => row(lib.byId.get(id) as Folder));
    const list = inside === null ? lib.themes : (lib.tasks.get(inside) ?? []);
    const all: MoveRow[] = list.filter((f) => f.id !== from).map(row);
    if (inside === null && from !== null) all.push({ id: 'unfiled', name: sentenceCase(library.unfiled), where: '' });
    const r = number(recent, 1);
    return { recent: r, all: inside === null ? all : number(all, r.length + 1), make: null };
  }
  const hits = [...lib.byId.values()]
    .filter((f) => f.id !== from && f.name.toLowerCase().includes(q))
    .sort((x, y) => Number(!x.name.toLowerCase().startsWith(q)) - Number(!y.name.toLowerCase().startsWith(q)) || (lib.latest.get(y.id) ?? 0) - (lib.latest.get(x.id) ?? 0));
  const exact = [...lib.byId.values()].some((f) => f.name.toLowerCase() === q);
  return { recent: [], all: number(hits.map(row), 1), make: exact ? null : typed.trim() };
}

/** The last folders he moved to, newest first, without repeats (4 kept). Pure. */
export function rememberMove(recent: readonly string[], id: string): string[] {
  return [id, ...recent.filter((x) => x !== id)].slice(0, 4);
}

// ---- search across folders

/** The kind chips the search and a folder show: a word and its icon kinds. */
export type ChipKind = 'shots' | 'pages' | 'docs' | 'sheets' | 'slides' | 'videos' | 'sounds' | '3d' | 'prs' | 'links' | 'files';
const CHIP_OF: Readonly<Record<IconKind, ChipKind>> = {
  image: 'shots', page: 'pages', doc: 'docs', sheet: 'sheets', deck: 'slides', video: 'videos', audio: 'sounds', '3d': '3d',
  link: 'links', pr: 'prs', release: 'links', file: 'files',
};
const CHIP_WORD: Readonly<Record<ChipKind, string>> = {
  shots: library.kind_shots, pages: library.kind_pages, docs: library.kind_docs, sheets: library.kind_sheets, slides: library.kind_slides,
  videos: library.kind_videos, sounds: library.kind_sounds, '3d': library.kind_3d, prs: library.kind_prs, links: library.kind_links,
  files: library.kind_files,
};

/**
 * The Pull requests filter's two sides (pr-integration A): open & draft by default, merged &
 * closed one click away. The settled ones are archived (the hub archives a PR at its end), so
 * the side reads every PR of the place, archived or not (artifacts/pr.ts prSettled). Pure.
 */
export type PrSide = 'open' | 'settled';
export function prSideItems(items: readonly Item[], side: PrSide): Item[] {
  return items.filter((it) => chipOf(it) === 'prs' && prSettled(it.a) === (side === 'settled'));
}
export const chipOf = (it: Item): ChipKind => CHIP_OF[iconKindOf(it.a)];
export const chipWord = (k: ChipKind): string => CHIP_WORD[k];

/** The chips of these items, the most first: [{kind, n}]. Pure. */
export function chipsOf(items: readonly Item[]): { kind: ChipKind; n: number }[] {
  const n = new Map<ChipKind, number>();
  for (const it of items) n.set(chipOf(it), (n.get(chipOf(it)) ?? 0) + 1);
  return [...n].map(([kind, c]) => ({ kind, n: c })).sort((x, y) => y.n - x.n || x.kind.localeCompare(y.kind));
}

/** The search's words and its 'in:<project>' (the scope typed). Pure. */
export function parseQuery(q: string): { project: string | null; words: string[] } {
  let project: string | null = null;
  const words: string[] = [];
  for (const t of q.toLowerCase().split(/\s+/u)) {
    if (t === '') continue;
    if (t.startsWith('in:') && t.length > 3) project = t.slice(3);
    else words.push(t);
  }
  return { project, words };
}

/** What a search reads of an item: its name, its folder's path, its agent, its kind words. */
function hayOf(lib: Library, it: Item): string {
  const id = lib.folderOf(it);
  const folder = id === null ? library.unfiled : pathOf(lib, id);
  return `${it.a.title} ${folder} ${it.a.agent} ${chipWord(chipOf(it))} ${it.a.kind}`.toLowerCase();
}

/** The items whose words hold every word of the search. Pure. */
export function searchItems(lib: Library, items: readonly Item[], words: readonly string[]): Item[] {
  if (words.length === 0) return [...items];
  return items.filter((it) => {
    const hay = hayOf(lib, it);
    return words.every((x) => hay.includes(x));
  });
}

/** The project a typed 'in:ai' means: an exact id or name, else the one that starts so. Pure. */
export function projectOfTyped(projects: readonly { project: string; name: string }[], typed: string): string | null {
  const t = typed.toLowerCase();
  const exact = projects.find((p) => p.project.toLowerCase() === t || p.name.toLowerCase() === t);
  if (exact !== undefined) return exact.project;
  const start = projects.filter((p) => p.project.toLowerCase().startsWith(t) || p.name.toLowerCase().startsWith(t));
  return start.length === 1 ? (start[0]?.project ?? null) : null;
}

/** What the library shows for a project scope ('' every project): the hero's facts, then each
 *  folder of the column, theme then its tasks, with its count ('14 of 74') and its new ones, then
 *  unfiled's. The screen draws these from the same functions; a feature test reads them. Pure. */
export interface LibraryView {
  readonly facts: string;
  readonly folders: readonly { readonly path: string; readonly count: string; readonly fresh: number }[];
  readonly unfiled: string;
}
export function libraryView(folders: readonly Folder[], all: readonly Item[], project: string, projectName: string): LibraryView {
  const lib = libraryOf(folders, all);
  const live = all.filter((it) => it.a.archived_at == null);
  const scoped = project === '' ? live : live.filter((it) => it.project === project);
  const counts = countsOf(lib, live, scoped);
  const row = (f: Folder) => ({ path: pathOf(lib, f.id), count: countWords(counts.get(f.id)), fresh: counts.get(f.id)?.fresh ?? 0 });
  return {
    facts: libraryFacts(lib, scoped, projectName),
    folders: lib.themes.flatMap((t) => [row(t), ...(lib.tasks.get(t.id) ?? []).map(row)]),
    unfiled: countWords(counts.get('unfiled')),
  };
}
