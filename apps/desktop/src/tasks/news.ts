// 'new since you looked' (roma idea 2, designer: our ink dot, never blue, never pink): a task whose
// status, artifacts or questions changed since he last saw it, on a Board card or the right bar's
// row. What he saw is a Seen per task (seen.ts keeps them in his view state, architect: no hub
// state); this file only compares. Pure. Owner: tasks.
import type { WorkRow, WorkStatus } from '@/proto';

/** What he saw of a task: when, its status, its open questions. `acted`: his own gesture changed
 *  it from that status (Pause, Resume on a card): the next status he didn't see is his, not news. */
export interface Seen {
  readonly at: number;
  readonly status: WorkStatus;
  readonly cards: readonly number[];
  readonly acted?: true;
}

/** The record of a look at `w` now. Pure. */
export function seenNow(w: Pick<WorkRow, 'status' | 'cards'>, at: number): Seen {
  return { at, status: w.status, cards: [...(w.cards ?? [])] };
}

/** His gesture on `w`: the status it leaves is seen, the one it goes to is his. Pure. */
export function seenActed(w: Pick<WorkRow, 'status' | 'cards'>, at: number): Seen {
  return { ...seenNow(w, at), acted: true };
}

/** A seen record after his gesture landed: once the row left the status he acted on, that new
 *  status is the one he saw (null: nothing to settle). Pure. */
export function settled(s: Seen | undefined, w: Pick<WorkRow, 'status' | 'cards'>): Seen | null {
  if (s?.acted !== true || w.status === s.status) return null;
  return { at: s.at, status: w.status, cards: s.cards };
}

/** The dot: what changed since `s` (never seen: no dot, nothing to compare). `newestArt` the newest
 *  artifact of the task (0 none). A question he answered (a card gone) is not news; a new one is. Pure. */
export function isNew(w: Pick<WorkRow, 'status' | 'cards'>, s: Seen | undefined, newestArt: number): boolean {
  if (s === undefined) return false;
  if (s.acted !== true && w.status !== s.status) return true;
  if (newestArt > s.at) return true;
  const was = new Set(s.cards);
  return (w.cards ?? []).some((c) => !was.has(c));
}
