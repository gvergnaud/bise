// The tasks' pure model for the desktop: the order of the statuses (the Board's columns, signed by
// designer and tasks-ux), the right bar's rows (open first, then a fold for the ongoing ones and the
// ones done this week), its counts, a row's done-when count, a name cut at a word. Pure. Owner: tasks.
import type { WorkRow, WorkStatus } from '@/proto';

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

/** A name cut at a word with '…' when longer than `max` letters (the whole name goes in its tip). Pure. */
export function cutAtWord(name: string, max: number): string {
  if (name.length <= max) return name;
  const head = name.slice(0, max);
  const at = head.lastIndexOf(' ');
  return `${(at > 8 ? head.slice(0, at) : head).trimEnd()}…`;
}
