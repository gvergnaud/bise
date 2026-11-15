// Move to task… (F9; spec tasks-ux m_65409, look designer m_65406, hub architect m_65565): his
// gesture that puts an artifact or an inbox card on another task, adds an artifact to one more, or
// takes it off one. The hub has it (task_move: exactly the new works, his for good); this file says
// which tasks the picker offers, in which order, what each pick makes of the thing's works, and the
// one ask open now (the native menu's pick opens it where he clicked; MoveHost draws it). Pure but
// the ask's small store. Owner: tasks.
import { useSyncExternalStore } from 'react';
import type { WorkRow } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';
import { cutAtWord, STATUS_ORDER } from './model';

/** What moves: an artifact (or a page) by its id, or an inbox card by its number. */
export type Thing =
  | { readonly kind: 'art'; readonly project: string; readonly id: string; readonly page: boolean; readonly title: string; readonly works: readonly string[] }
  | { readonly kind: 'card'; readonly project: string; readonly card: number; readonly title: string; readonly works: readonly string[] };

/** Move: onto the picked task alone. Also: the picked task added (artifacts only). */
export type Mode = 'move' | 'also';

/** The 'No task' row's id (artifacts only): out of every task. */
export const NO_TASK = '';

/** How long a done task stays offered (artifacts only, faded at the end): a week. */
const DONE_LATELY_MS = 7 * 86_400_000;

/** The picker's rows: the project's open tasks (the ones he opened lately first, then the Board's
 *  order), the current ones flagged; done lately at the end for an artifact (a card can't wait on
 *  him in a done task). Pure. */
export function offered(thing: Thing, works: readonly WorkRow[], lookedAt: (id: string) => number, now: number): { readonly open: readonly WorkRow[]; readonly done: readonly WorkRow[] } {
  const rank = (w: WorkRow): number => STATUS_ORDER.indexOf(w.status);
  const open = works
    .filter((w) => w.status !== 'done' && w.merged_into == null)
    .slice()
    .sort((a, b) => lookedAt(b.id) - lookedAt(a.id) || rank(a) - rank(b) || b.created_ms - a.created_ms);
  const done = thing.kind === 'art' ? works.filter((w) => w.status === 'done' && w.merged_into == null && now - (w.done?.at_ms ?? 0) < DONE_LATELY_MS) : [];
  return { open, done };
}

/** The rows whose name holds every word he typed (case aside). Pure. */
export function filtered(rows: readonly WorkRow[], q: string): readonly WorkRow[] {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  return words.length === 0 ? rows : rows.filter((w) => words.every((x) => w.name.toLowerCase().includes(x)));
}

/** The works after his pick: Move → exactly that task ('No task': none), Also → that one added. Pure. */
export function worksAfter(prev: readonly string[], pick: string, mode: Mode): string[] {
  if (pick === NO_TASK) return [];
  if (mode === 'also') return prev.includes(pick) ? [...prev] : [...prev, pick];
  return [pick];
}

/** The works after 'Remove from <task>'. Pure. */
export function worksWithout(prev: readonly string[], id: string): string[] {
  return prev.filter((w) => w !== id);
}

/** The most letters of a task's name in the toast and the menu ('Speed up the signup flow…'). */
export const NAME_MAX = 25;

/** The toast's words: '<thing> moved to <task>', 'added to', 'removed from' (designer m_65406 5:
 *  the thing first, no ✓, the task's name cut at a word). Pure. */
export function movedWords(what: 'moved' | 'added' | 'removed' | 'none', title: string, task: string): string {
  const t = cutAtWord(task, NAME_MAX);
  switch (what) {
    case 'moved':
      return tasks.move_done({ what: title, task: t });
    case 'added':
      return tasks.move_added({ what: title, task: t });
    case 'removed':
      return tasks.move_removed({ what: title, task: t });
    case 'none':
      return tasks.move_none_done({ what: title });
  }
}

// ---- the ask open now: the native menu's pick opens the picker where he clicked ----

/** The picker asked for: what moves, how, and where it opens (the pointer of the click that
 *  opened the menu). */
export interface Ask {
  readonly thing: Thing;
  readonly mode: Mode;
  readonly x: number;
  readonly y: number;
}

/** What he did, for the toast's 6 s Undo: the thing, its works before (Undo sends them back,
 *  exactly), and the words. */
export interface Done {
  readonly thing: Thing;
  readonly before: readonly string[];
  readonly words: string;
  readonly at: number;
}

/** Where he last pressed or right-clicked (MoveHost keeps it): the picker opens there, by the ⋯ or
 *  the row his native menu came from. */
export const lastAt = { x: 0, y: 0 };

let ask: Ask | null = null;
let done: Done | null = null;
const subs = new Set<() => void>();
const tell = (): void => {
  for (const f of subs) f();
};
const subscribe = (cb: () => void): (() => void) => {
  subs.add(cb);
  return () => subs.delete(cb);
};

/** Opens the picker for `thing` (null closes it). */
export function askMove(a: Ask | null): void {
  ask = a;
  tell();
}

/** The toast for his move (null: gone). */
export function tellMoved(d: Done | null): void {
  done = d;
  tell();
}

export function useMoveAsk(): Ask | null {
  return useSyncExternalStore(subscribe, () => ask);
}

export function useMoved(): Done | null {
  return useSyncExternalStore(subscribe, () => done);
}
