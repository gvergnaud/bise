// The library's sections (his pick D+C+A+B on page artifacts-signal v3; designer's
// artifacts-signal-options v3, words and rules designer m_60689): For you (Kept ◆, then Open work),
// with Done ▸ and Between agents ▸ one click away. Which section an artifact reads in is the hub's
// one decision (its row's `section`, artifacts_marks.rs): this module never re-decides it, it
// splits the items a place already holds, counts them and orders each section by its last touch
// (`touched_ms`, newest first, then id; architect m_60685). Pure, kept apart from the drawing
// (SectionRows.tsx) so the app filter on plugin-apps composes after it: items → itemsIn → app facet.
// Owner: artifacts-keep.
import type { Artifact } from '@/proto';
import type { Item } from './model';

/** The hub's sections. */
export type SectionId = 'kept' | 'open' | 'done' | 'agents';

/** What the gallery shows: For you (Kept and Open work), or one of the two lists behind it. */
export type SectionView = 'you' | 'done' | 'agents';

/** The sections in the order they are drawn. */
export const SECTIONS: readonly SectionId[] = ['kept', 'open', 'done', 'agents'];

/** The sections each view shows, in order. */
const SHOWN: Record<SectionView, readonly SectionId[]> = { you: ['kept', 'open'], done: ['done'], agents: ['agents'] };

/** An artifact's section as the hub sent it; a row from an older hub (none) reads as open work. */
export function sectionOf(a: Pick<Artifact, 'section'>): SectionId {
  const s = a.section;
  return s === 'kept' || s === 'done' || s === 'agents' ? s : 'open';
}

/** Its last touch: the hub's `touched_ms`, else its newest version's time (an older hub). */
export const touchedOf = (a: Pick<Artifact, 'touched_ms' | 'at_ms'>): number => a.touched_ms ?? a.at_ms;

/** Newest touch first, then id: one order for every client (architect m_60685). */
export function byTouched(x: Item, y: Item): number {
  const d = touchedOf(y.a) - touchedOf(x.a);
  if (d !== 0) return d;
  return x.a.id < y.a.id ? -1 : x.a.id > y.a.id ? 1 : 0;
}

/** How many items each section holds. */
export function sectionCounts(items: readonly Item[]): Record<SectionId, number> {
  const n: Record<SectionId, number> = { kept: 0, open: 0, done: 0, agents: 0 };
  for (const it of items) n[sectionOf(it.a)] += 1;
  return n;
}

/** One section's items, newest touch first. */
export interface Group {
  readonly id: SectionId;
  readonly items: readonly Item[];
}

/**
 * The groups a view shows: For you is Kept then Open work, Done and Between agents one list each;
 * `all` (a search) every section with something in it. An empty section is left out. Pure.
 */
export function groupsOf(items: readonly Item[], view: SectionView | 'all'): Group[] {
  const ids = view === 'all' ? SECTIONS : SHOWN[view];
  return ids
    .map((id) => ({ id, items: items.filter((it) => sectionOf(it.a) === id).sort(byTouched) }))
    .filter((g) => g.items.length > 0);
}

/** The items a view shows, in its order (its groups one after the other). Pure. */
export function itemsIn(items: readonly Item[], view: SectionView | 'all'): Item[] {
  return groupsOf(items, view).flatMap((g) => g.items);
}

/** He kept it (◆ in ink); an agent's keep is ◆ in text-3 with its name on the card's line. */
export const keptByHim = (a: Pick<Artifact, 'kept'>): boolean => a.kept?.by === 'you';

/** A view's place in the route's tail, before the place's own ('done', 'agents/f/<id>'); For you adds nothing. Pure. */
export function sectionPath(view: SectionView, place: string): string {
  if (view === 'you') return place;
  return place === '' ? view : `${view}/${place}`;
}

/** The route's tail as a view and the place's own tail (sectionPath's inverse). Pure. */
export function splitSectionPath(p: string): { view: SectionView; rest: string } {
  for (const v of ['done', 'agents'] as const) {
    if (p === v) return { view: v, rest: '' };
    if (p.startsWith(`${v}/`)) return { view: v, rest: p.slice(v.length + 1) };
  }
  return { view: 'you', rest: p };
}

/**
 * From agents' foot line (designer m_62438): the bytes of the agents' files' copies in these
 * projects, each its hub's count (`agents_bytes`); none when no hub said one (an older hub). Pure.
 */
/** The foot line's size: '512 B', '2.1 KB', '14.0 MB', '1.3 GB' (log/model's shortBytes stops at MB: the TUI's). Pure. */
export function sizeWords(n: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'] as const;
  let v = n;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u += 1;
  }
  return u === 0 ? `${String(n)} B` : `${v.toFixed(1)} ${units[u] ?? 'TB'}`;
}

export function bytesIn(projects: readonly string[], read: (project: string) => number | undefined): number | undefined {
  let sum: number | undefined;
  for (const p of projects) {
    const n = read(p);
    if (n !== undefined) sum = (sum ?? 0) + n;
  }
  return sum;
}
