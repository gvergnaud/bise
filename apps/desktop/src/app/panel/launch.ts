// src/app/panel · the launcher's pure parts: which artifacts his words find (every word in the
// title, the kind or the maker's name, newest first), the Artifacts picker's list (the 10 newest,
// or what the field finds), what ⏎ opens, the highlighted row's ↑/↓ walk, whether the agent in view
// has changes (its rows' diff counts: the Changes row and ⌥⌘D only then), and which rows Open
// shows. A New tab's rows at the keys (his m_46444, designer m_46449): Open, New, then Recent (the
// project in front's only); ↑/↓ walk them from the field, ⏎ opens the highlighted one. As he types,
// the field is the omnibox's (omni.ts, Omnibox.tsx). Owner: nm-panel.
import type { Item } from '@/artifacts/model';
import type { Agent } from '@/proto';
import type { Entity } from '@/shared/entity';
import type { MakeOpt } from './make';
import { RECENT_MAX } from './model';

/** How many found artifacts the launcher lists. */
export const FOUND_MAX = 8;
/** How many artifacts the Artifacts picker lists (⌘P). */
export const PICK_MAX = 10;

/** The artifacts whose title, kind or maker hold every word of `q` (case aside), newest first. Pure. */
export function launchMatches(items: readonly Item[], q: string, max = FOUND_MAX): readonly Item[] {
  const words = q.toLowerCase().split(/\s+/u).filter((x) => x !== '');
  if (words.length === 0) return [];
  return items
    .filter((x) => {
      const hay = `${x.a.title} ${x.a.kind} ${x.a.agent}`.toLowerCase();
      return words.every((wd) => hay.includes(wd));
    })
    .toSorted((a, b) => b.a.at_ms - a.a.at_ms)
    .slice(0, max);
}

/** The Artifacts picker: the PICK_MAX newest, or those `q` finds. Pure. */
export function latestArtifacts(items: readonly Item[], q: string, max = PICK_MAX): readonly Item[] {
  if (q.trim() === '') return items.toSorted((a, b) => b.a.at_ms - a.a.at_ms).slice(0, max);
  return launchMatches(items, q, max);
}

/** The agent `name` has changes to show (its rows' diff: a file at least). Pure. */
export function hasChanges(agents: readonly Agent[], name: string): boolean {
  return (agents.find((a) => a.name === name)?.changes?.files ?? 0) > 0;
}

/** An Open row of the launcher. */
export type OpenRow = 'artifacts' | 'terminal' | 'changes' | 'browser' | 'office';

/** Open's rows: Changes only when the agent in view has some (r2 #2), Office with a project. Pure. */
export function openRows(o: { readonly changes: boolean; readonly office: boolean }): readonly OpenRow[] {
  return (['artifacts', 'terminal', 'changes', 'browser', 'office'] as const).filter((r) => (r === 'changes' ? o.changes : r === 'office' ? o.office : true));
}

/** Recent of the project in front: its own things, and those of no project (a plugin, a skill), RECENT_MAX at most (his m_46444). Pure. */
export function recentIn(recent: readonly Entity[], project: string, max = RECENT_MAX): readonly Entity[] {
  return recent.filter((e) => !('project' in e) || e.project === project).slice(0, max);
}

/** A row of a New tab that ↑/↓ walk and ⏎ opens: an Open row, a New › make, a Recent thing, a picker's artifact. */
export type LaunchRow =
  | { readonly kind: 'open'; readonly row: OpenRow }
  | { readonly kind: 'recent'; readonly e: Entity }
  | { readonly kind: 'found'; readonly item: Item }
  /** New › (make.ts, docs/plugin-apps.md A6b): one type an on app makes; a pick asks the hub's file/new */
  | { readonly kind: 'new'; readonly make: MakeOpt };

/** His words filter the rows (or the tab is the Artifacts picker): the found list shows, not Open and Recent. Pure. */
export function isListing(q: string, picker: boolean): boolean {
  return picker || q.trim() !== '';
}

/** The tab's rows top to bottom: Open, New, then Recent; the picker: the artifacts it found (his words in the New tab's field are the omnibox's: omni.ts). Pure. */
export function launchRows(o: {
  readonly picker: boolean;
  readonly found: readonly Item[];
  readonly open: readonly OpenRow[];
  readonly recent: readonly Entity[];
  /** New ›'s rows (make.ts newRows); the Artifacts picker has none */
  readonly makes?: readonly MakeOpt[];
}): readonly LaunchRow[] {
  if (o.picker) return o.found.map((item) => ({ kind: 'found' as const, item }));
  return [...o.open.map((row) => ({ kind: 'open' as const, row })), ...(o.makes ?? []).map((make) => ({ kind: 'new' as const, make })), ...o.recent.map((e) => ({ kind: 'recent' as const, e }))];
}

/** No row highlighted: the field has the keys alone. */
export const NO_ROW = -1;

/** Where the highlight starts: none on Open/Recent (designer m_46449 b), the first row of a filtered list or the picker. Pure. */
export function firstAt(q: string, picker: boolean): number {
  return isListing(q, picker) ? 0 : NO_ROW;
}

/** ↑ (-1) and ↓ (+1) over `n` rows from `at`: ↓ from the field enters the first row, ↑ on it gives the field back, no wrap (designer m_46449 c). Pure. */
export function walkAt(at: number, n: number, dir: 1 | -1): number {
  if (n === 0) return NO_ROW;
  if (dir === 1) return Math.min(n - 1, Math.max(NO_ROW, at) + 1);
  return at <= 0 ? NO_ROW : Math.min(n - 1, at - 1);
}

/** What ⏎ opens: the highlighted row; none on the picker's list: its first; none on Open/Recent: nothing. Pure. */
export function enterRow(rows: readonly LaunchRow[], at: number, listing: boolean): LaunchRow | undefined {
  return rows[at] ?? (listing ? rows[0] : undefined);
}
