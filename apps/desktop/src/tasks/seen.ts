// What he saw of each task (roma idea 2; architect: a desktop view, its one owner this module, no
// hub state): one Seen per `<project>/<id>` in the kept view `tasks.seen`, so a reload and a
// relaunch keep his dots. Marked when a Task tab opens and closes, when the Board closes (every
// task it showed), and as his gesture (news.ts's `acted`). This module is the one writer.
// Owner: tasks.
import { useEffect, useSyncExternalStore } from 'react';
import { keep } from '@/app/keep';
import type { WorkRow } from '@/proto';
import { seenActed, seenNow, settled, type Seen } from './news';
import type { ProjWork } from './model';

const KEY = 'tasks.seen';

type All = Readonly<Record<string, Seen>>;

const subs = new Set<() => void>();
let cache: All | null = null;

function read(): All {
  if (cache !== null) return cache;
  const v = keep.view(KEY);
  cache = typeof v === 'object' && v !== null ? (v as All) : {};
  return cache;
}

function write(next: All): void {
  cache = next;
  keep.setView(KEY, next);
  for (const f of subs) f();
}

const key = (project: string, id: string): string => `${project}/${id}`;

/** Marks tasks seen now (a Task tab, the Board's close). */
export function markSeen(rows: readonly ProjWork[], now: number = Date.now()): void {
  if (rows.length === 0) return;
  const all = { ...read() };
  for (const x of rows) all[key(x.project, x.w.id)] = seenNow(x.w, now);
  write(all);
}

/** His gesture on a task: what it goes to is his, not news. */
export function markActed(project: string, w: Pick<WorkRow, 'id' | 'status' | 'cards'>, now: number = Date.now()): void {
  write({ ...read(), [key(project, w.id)]: seenActed(w, now) });
}

/** The seen records, re-read when one is written. */
export function useSeenTasks(): (project: string, id: string) => Seen | undefined {
  const all = useSyncExternalStore(
    (cb) => {
      subs.add(cb);
      return () => {
        subs.delete(cb);
      };
    },
    read,
  );
  return (project, id) => all[key(project, id)];
}

/** Settles his gestures once the rows show where they went (news.ts's `settled`). */
export function useSettleActed(rows: readonly ProjWork[]): void {
  useEffect(() => {
    const all = read();
    let next: Record<string, Seen> | null = null;
    for (const x of rows) {
      const s = settled(all[key(x.project, x.w.id)], x.w);
      if (s === null) continue;
      next ??= { ...all };
      next[key(x.project, x.w.id)] = s;
    }
    if (next !== null) write(next);
  }, [rows]);
}
