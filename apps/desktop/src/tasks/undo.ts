// Undo on his task gestures (roma idea 4; architect m_65317): a short 'Undo' after Pause, Resume and
// Mark done that sends the inverse gesture, a new journaled one, never a rewind: Pause ↔ Resume (a
// Resume undone pauses again with its old end if it is still to come, else with none), Mark done →
// Reopen while the task isn't folded. Merge keeps its confirm and gets none. Pure. Owner: tasks.
import type { Task as TaskReq, WorkRow, WorkStatus } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';

/** How long the Undo stays (designer: short). */
export const UNDO_MS = 6_000;

/** His gesture, as the toast keeps it: the step and the row he acted on (its state before). */
export interface Did {
  readonly project: string;
  readonly step: 'pause' | 'resume' | 'done';
  readonly row: Pick<WorkRow, 'id' | 'name' | 'status' | 'until_ms'>;
}

/** His Pause, Resume or Mark done shown at once (tasks-ux m_65598): the pill and the footer flip
 *  with the toast, before the hub's row comes back. */
export interface Flip {
  readonly id: string;
  readonly from: WorkStatus;
  readonly to: WorkStatus;
  readonly at: number;
}

/** How long a flip waits for the hub's row (a refused step falls back to the row's status). */
export const FLIP_MS = 5_000;

/** The status a step makes, null a step that flips nothing (Reopen, Restart: the hub says). Pure. */
export function flipTo(step: string): WorkStatus | null {
  return step === 'pause' ? 'paused' : step === 'resume' ? 'working' : step === 'done' ? 'done' : null;
}

/** The status to show: the flip's while the hub's row still says what it said before (and the
 *  flip is fresh), else the row's own. Pure. */
export function shownStatus(row: Pick<WorkRow, 'id' | 'status'>, f: Flip | null, now: number): WorkStatus {
  return f !== null && f.id === row.id && row.status === f.from && now - f.at < FLIP_MS ? f.to : row.status;
}

/** The gesture that undoes `d` at `now`; null: none (a resume of a task that wasn't paused, a
 *  Restart of a stalled one, has nothing to put back). Pure. */
export function inverse(d: Did, now: number): TaskReq | null {
  const id = d.row.id;
  switch (d.step) {
    case 'pause':
      return { step: 'resume', id };
    case 'resume': {
      if (d.row.status !== 'paused') return null;
      const until = d.row.until_ms ?? null;
      return until !== null && until > now ? { step: 'pause', id, until_ms: until } : { step: 'pause', id };
    }
    case 'done':
      return { step: 'reopen', id };
  }
}

/** The toast's words for `d`: 'Paused "Write the pricing page"'. Pure. */
export function didWords(d: Did, name: string): string {
  switch (d.step) {
    case 'pause':
      return tasks.undo_paused({ name });
    case 'resume':
      return tasks.undo_resumed({ name });
    case 'done':
      return tasks.undo_done({ name });
  }
}

/** Undo still offered for `d`: Mark done's only while the task is in the works (a folded one is a
 *  tomb; reopening it is not his Undo). Pure. */
export function undoable(d: Did, works: readonly Pick<WorkRow, 'id'>[]): boolean {
  return works.some((w) => w.id === d.row.id);
}
