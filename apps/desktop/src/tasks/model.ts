// The tasks' pure model for the desktop: the order of the statuses (the Board's columns, signed by
// designer and tasks-ux), the right bar's rows (open first, then a fold for the ongoing ones and the
// ones done this week), its counts, a row's done-when count, a name cut at a word. Pure. Owner: tasks.
import type { Artifact, WorkRow, WorkStatus, WorkTomb } from '@/proto';

/** A task with its project (the bar lists every project's on bise's own). */
export interface ProjWork {
  readonly project: string;
  readonly projectName: string;
  readonly w: WorkRow;
}

/** The Board's column order (signed): Needs you · Working · Waiting · Stalled · To review · Done this week · Paused · Ongoing. */
export const STATUS_ORDER: readonly WorkStatus[] = ['needs_you', 'working', 'waiting', 'stalled', 'to_review', 'done', 'paused', 'ongoing', 'unknown'];

/** a status's rank in the bar's open rows: the Board's order, Paused right after To review (Done and Ongoing fold) */
const OPEN_RANK: Readonly<Record<WorkStatus, number>> = {
  needs_you: 0,
  working: 1,
  waiting: 2,
  stalled: 3,
  to_review: 4,
  paused: 5,
  unknown: 6,
  done: 7,
  ongoing: 8,
};

/** done this week: the bar's fold keeps them, older ones are tombs already (the hub folds them at 7 days) */
const WEEK_MS = 7 * 24 * 3_600_000;

/** Open: not done and not ongoing (an ongoing task is never done, it folds with the done ones). */
export const isOpen = (w: Pick<WorkRow, 'status'>): boolean => w.status !== 'done' && w.status !== 'ongoing';

/** What waits on him: its status is Needs you. */
export const needsHim = (w: Pick<WorkRow, 'status'>): boolean => w.status === 'needs_you';

/** Newest fact first, then the name: a stable order inside a status. */
function byLast(a: ProjWork, b: ProjWork): number {
  return b.w.last_ms - a.w.last_ms || a.w.name.localeCompare(b.w.name);
}

/** The bar's rows: the open ones in the status order; then the fold, ongoing ones then the done this week. Pure. */
export function barRows(all: readonly ProjWork[], now: number): { readonly open: readonly ProjWork[]; readonly ongoing: readonly ProjWork[]; readonly done: readonly ProjWork[] } {
  const open = all.filter((x) => isOpen(x.w)).sort((a, b) => OPEN_RANK[a.w.status] - OPEN_RANK[b.w.status] || byLast(a, b));
  const ongoing = all.filter((x) => x.w.status === 'ongoing').sort(byLast);
  const done = all
    .filter((x) => x.w.status === 'done' && now - (x.w.done?.at_ms ?? x.w.last_ms) < WEEK_MS)
    .sort((a, b) => (b.w.done?.at_ms ?? b.w.last_ms) - (a.w.done?.at_ms ?? a.w.last_ms));
  return { open, ongoing, done };
}

/** A row's done-when count: ticked of all; null when it has none. Pure. */
export function goalsOf(w: Pick<WorkRow, 'goals'>): { readonly done: number; readonly all: number } | null {
  const g = w.goals ?? [];
  if (g.length === 0) return null;
  return { done: g.filter((x) => x.done === true).length, all: g.length };
}

/** A task id as a link reads it: a merged one's, the task it went into (its tombs, followed to the end). Pure. */
export function resolveMerged(tombs: readonly Pick<WorkTomb, 'id' | 'merged_into'>[], id: string): string {
  const into = new Map(tombs.flatMap((t) => (t.merged_into == null ? [] : [[t.id, t.merged_into] as const])));
  let at = id;
  // a chain of merges ends; a loop (never written by the hub) stops after as many steps as tombs
  for (let n = 0; n <= into.size; n++) {
    const next = into.get(at);
    if (next === undefined) return at;
    at = next;
  }
  return at;
}

/** A name cut at a word with '…' when longer than `max` letters (the whole name goes in its tip). Pure. */
export function cutAtWord(name: string, max: number): string {
  if (name.length <= max) return name;
  const head = name.slice(0, max);
  const at = head.lastIndexOf(' ');
  return `${(at > 8 ? head.slice(0, at) : head).trimEnd()}…`;
}

/** The artifacts that serve task `id` (a merged task's read as the one it went into), archived ones out,
 *  newest first; `mine` the ones for him (an older hub's, none said: his), else all of them. Pure. */
export function artsOf<A extends Pick<Artifact, 'works' | 'at_ms' | 'archived' | 'audience'>>(arts: readonly A[], id: string, tombs: readonly Pick<WorkTomb, 'id' | 'merged_into'>[]): { readonly all: readonly A[]; readonly mine: readonly A[] } {
  const all = arts.filter((a) => a.archived !== true && (a.works ?? []).some((w) => resolveMerged(tombs, w) === id)).sort((a, b) => b.at_ms - a.at_ms);
  const his = all.filter((a) => a.audience !== 'agents');
  return { all, mine: his.length > 0 ? his : all };
}
