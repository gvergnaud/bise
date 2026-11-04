// What his agents made (desktop-screens v6 'artifacts', designer m_11837; the TUI's
// artifacts_screen.rs words): the rows across his projects, their kind's glyph and words, the day
// groups, the filters. Pure. Owner: amb-kit.
import type { Artifact } from '@/proto';
import { hero } from '@/proto/words_hero.gen';

/** One artifact with the project it belongs to (the artifacts event is per project). */
export interface Item {
  readonly project: string;
  /** the project's name as the sidebar says it */
  readonly projectName: string;
  readonly a: Artifact;
}

export type KindFilter = 'page' | 'file' | 'pr' | 'shots';
export const KIND_FILTERS: readonly { id: KindFilter; label: string }[] = [
  { id: 'page', label: 'pages' },
  { id: 'file', label: 'files' },
  { id: 'pr', label: 'PRs' },
  { id: 'shots', label: 'shots' },
];

/** The kind the filters and glyphs know ('shot', 'image' read as shots; a link as a page). */
export function kindOf(kind: string): KindFilter {
  const k = kind.toLowerCase();
  if (k === 'pr') return 'pr';
  if (k === 'shot' || k === 'shots' || k === 'image' || k === 'png') return 'shots';
  if (k === 'page' || k === 'link' || k === 'site' || k === 'url') return 'page';
  return 'file';
}

/** What decides where an artifact opens: its kind, its url, the hub's in-app copy (frame_url), if any. */
export interface Leaving {
  readonly kind: string;
  readonly url?: string | null;
  readonly frame_url?: string | null;
}

/** It has a view in the window: the hub's framed copy, or a page at an http(s) url. Pure. */
export function inApp(a: Leaving): boolean {
  if ((a.frame_url ?? '') !== '') return true;
  return kindOf(a.kind) === 'page' && /^https?:\/\//i.test(a.url ?? '');
}

/**
 * Browser-only (designer m_21714): no view in the window and a url his browser takes (http,
 * https, mailto): a PR, a link. Its rows open the browser at once (leave.ts). Pure.
 */
export function leaves(a: Leaving): boolean {
  return !inApp(a) && /^(https?:\/\/|mailto:)/i.test(a.url ?? '');
}

/** The mark a browser-only row shows in place of its kind and version (designer m_21714). */
export const LEAVES = '↗';

/** The row's glyph, mono: ↗ page, ▤ file, ⇡ PR, ◫ shots. */
export function glyphOf(kind: string): string {
  return { page: '↗', file: '▤', pr: '⇡', shots: '◫' }[kindOf(kind)];
}

/** The kind column: 'page · v2', 'file', 'PR', 'shots' (a version only from v2). */
export function kindWords(a: Artifact): string {
  const k = kindOf(a.kind);
  const word = k === 'pr' ? 'PR' : k === 'shots' ? 'shots' : k;
  return a.version > 1 ? `${word} · v${String(a.version)}` : word;
}

/** Who made it, where: 'perf · storefront' (the project left out when the list is one project's). */
export function madeBy(it: Item, withProject = true): string {
  return withProject && it.projectName !== '' ? `${it.a.agent} · ${it.projectName}` : it.a.agent;
}

const pad = (n: number) => String(n).padStart(2, '0');

/** The local day of a time, as 'YYYY-MM-DD'. */
export function dayOf(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getFullYear())}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

const MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];
const DAYS = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'];

/** A day's group title (ambient's day separators): 'today', 'yesterday', else 'sun 5 oct'. */
export function dayWords(ms: number, now: number): string {
  if (dayOf(ms) === dayOf(now)) return 'today';
  if (dayOf(ms) === dayOf(now - 86_400_000)) return 'yesterday';
  const d = new Date(ms);
  // past a year, the year in place of the day's name: '3 oct 2025' (designer m_45148)
  if (now - ms > 365 * 86_400_000) return `${String(d.getDate())} ${MONTHS[d.getMonth()] ?? ''} ${String(d.getFullYear())}`;
  return `${DAYS[d.getDay()] ?? ''} ${String(d.getDate())} ${MONTHS[d.getMonth()] ?? ''}`;
}

/** A row's time, '16:02': its day group already says the day. */
export function timeOf(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** A time out of any day group (the open header, the versions): '14:02' today, else 'yesterday 14:02', 'sun 5 oct 14:02'. */
export function whenWords(ms: number, now: number): string {
  const day = dayWords(ms, now);
  return day === 'today' ? timeOf(ms) : `${day} ${timeOf(ms)}`;
}

export interface Filter {
  /** a project id; '' all */
  project: string;
  /** an agent's name; '' all */
  agent: string;
  /** the kinds shown; empty: all */
  kinds: readonly KindFilter[];
  /** words in the title, the agent or the project */
  q: string;
}

export const ALL: Filter = { project: '', agent: '', kinds: [], q: '' };

/** Archived by him or an agent (art-archive's archived_at): out of the default list, behind 'N archived'. */
export const isArchived = (a: Artifact): boolean => a.archived_at != null;

/** The hero's facts line (designer m_39021, m_39330): how many, and how many he hasn't opened, '41 artifacts · 3 new'. Pure. */
export function heroFacts(live: readonly Item[]): string {
  if (live.length === 0) return hero.nothing_yet;
  const fresh = live.filter((it) => it.a.new).length;
  const n = hero.artifacts_n({ n: live.length });
  return fresh === 0 ? n : `${n} · ${hero.new_ones({ n: fresh })}`;
}

/** The list's items and how many archived ones it leaves behind (the TUI's faint '2 archived' row). Pure. */
export function splitArchived(items: readonly Item[]): { live: Item[]; archived: Item[] } {
  const live: Item[] = [];
  const archived: Item[] = [];
  for (const it of items) (isArchived(it.a) ? archived : live).push(it);
  return { live, archived };
}

/** The items the filter keeps, newest first. */
export function filterItems(items: readonly Item[], f: Filter): Item[] {
  const words = f.q.toLowerCase().split(/\s+/).filter((w) => w !== '');
  return items
    .filter((it) => f.project === '' || it.project === f.project)
    .filter((it) => f.agent === '' || it.a.agent === f.agent)
    .filter((it) => f.kinds.length === 0 || f.kinds.includes(kindOf(it.a.kind)))
    .filter((it) => {
      const hay = `${it.a.title} ${it.a.agent} ${it.projectName}`.toLowerCase();
      return words.every((w) => hay.includes(w));
    })
    .sort((x, y) => y.a.at_ms - x.a.at_ms);
}

/** The items by day, newest day first: [{title, items}]. */
export function byDay(items: readonly Item[], now: number): { day: string; title: string; items: Item[] }[] {
  const out: { day: string; title: string; items: Item[] }[] = [];
  for (const it of items) {
    const day = dayOf(it.a.at_ms);
    const last = out.at(-1);
    if (last?.day === day) last.items.push(it);
    else out.push({ day, title: dayWords(it.a.at_ms, now), items: [it] });
  }
  return out;
}

/** The agents of the items, by name, for the 'all agents ⌄' menu. */
export function agentsOf(items: readonly Item[]): string[] {
  return [...new Set(items.map((it) => it.a.agent).filter((a) => a !== ''))].sort();
}

/** The key of an item (its project and id): the route's and the list's. */
export const keyOf = (it: Item): string => `${it.project}/${it.a.id}`;

/** The artifact's file on disk (amb-feed 2f4ce9c9: file targets only), for show in Finder and quick look, else null. Pure. */
export function pathOf(a: Artifact): string | null {
  return typeof a.path === 'string' && a.path !== '' ? a.path : null;
}

/**
 * A patch kept as an artifact (a .diff or .patch file, a feature's diff: try-diff-open): the panel
 * draws its files with the review's diff (the hub parses it, `diff` with `artifact`), never a frame.
 * Pure.
 */
export function isDiff(a: Artifact): boolean {
  const file = pathOf(a) ?? a.copy ?? '';
  return /\.(diff|patch)$/iu.test(file);
}

/**
 * What 'show in Finder' selects (art-save): the file itself while it is there, else bise's copy
 * of the version shown (the current one's when that version kept none), else null (no item).
 * Never a bise page's or a link's. Pure.
 */
export function revealPathOf(a: Artifact, shown = a.version): string | null {
  const path = pathOf(a);
  if (path === null) return null;
  if (a.gone !== true) return path;
  const copy = a.versions?.find((x) => x.v === shown)?.copy ?? a.copy;
  return copy ?? null;
}

/** The last part of a path, its trailing slashes left out. Pure. */
const fileName = (p: string): string => p.replace(/\/+$/u, '').split('/').pop() ?? '';

/**
 * What 'save a copy…' copies (art-save): the bytes of version `shown`, bise's copy of it first
 * (what the panel frames), else the file itself when `shown` is the current version and it is
 * still there; named after the file the agent made. null: no item (a bise page: its html needs
 * the pages server, his browser saves it; a link; an older version bise didn't keep). Pure.
 */
export function saveOf(a: Artifact, shown = a.version): { path: string; name: string } | null {
  const path = pathOf(a);
  if (path === null) return null;
  const ver = a.versions?.find((x) => x.v === shown);
  const current = shown === a.version;
  const copy = ver?.copy ?? (current ? a.copy : null) ?? null;
  const src = copy ?? (current && a.gone !== true ? path : null);
  if (src === null) return null;
  const target = ver?.target ?? (current ? path : '');
  const name = fileName(target) || fileName(src);
  return { path: src, name };
}

/**
 * The versions popover's rows, newest first: 'v2  the current one  14:02', 'v1  <its note>  13:20'
 * (the hub's versions, oldest first, note = the TUI's words, left out when empty; from the live
 * artifacts event, never the ⌘K index).
 */
export function versions(a: Artifact, now = Date.now()): { v: number; words: string; when?: string }[] {
  const known = a.versions ?? [];
  return Array.from({ length: Math.max(1, a.version) }, (_, i) => {
    const v = Math.max(1, a.version) - i;
    const k = known.find((x) => x.v === v);
    const at = k?.at_ms ?? (i === 0 ? a.at_ms : undefined);
    const words = i === 0 ? 'the current one' : (k?.note ?? '');
    return k === undefined && i > 0 ? { v, words } : { v, words, ...(at === undefined ? {} : { when: whenWords(at, now) }) };
  });
}
