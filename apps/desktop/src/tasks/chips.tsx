// The task chips where a thing serves a task (brief "Chips everywhere"): under his message, in an
// agent's header, on an inbox card, on an artifact's frame and Library row, on a page's frame. Each
// reads one source (architect m_64865): an agent's from WorkRow.agents, a card's from WorkRow.cards,
// his message's from WorkRow.origin (main's thread), an artifact's from its row's `works`. Open
// tasks first, newest first; a done one still shows (its mark says it). Owner: tasks.
import { useMemo } from 'react';
import { useWorks } from '@/store';
import { useNowFor } from '@/app/clock';
import type { WorkRow } from '@/proto';
import { isOpen } from './model';
import { lineAt } from './Side';
import { TaskChips } from './Marks';

/** open ones first, then the newest fact: the chip in clear is the one he most likely means. Pure. */
export function chipOrder(list: readonly WorkRow[]): WorkRow[] {
  return [...list].sort((a, b) => Number(isOpen(b)) - Number(isOpen(a)) || b.last_ms - a.last_ms);
}

/** What a thing points at: an agent by name, a card by id, his message by its msg_id (WorkRow.origin
 *  is the id sb-core gave it, bend/hub/work.bend `origin`, matched on Entry.msg_id; never its pos), or task ids. */
export type ChipsOf =
  | { readonly agent: string }
  | { readonly card: number }
  | { readonly origin: number }
  | { readonly ids: readonly string[] };

/** The tasks a thing serves, in chip order. Pure. */
export function tasksOf(works: readonly WorkRow[], of: ChipsOf): WorkRow[] {
  if ('agent' in of) return chipOrder(works.filter((w) => (w.agents ?? []).includes(of.agent) && (isOpen(w) || w.status === 'done')));
  if ('card' in of) return chipOrder(works.filter((w) => (w.cards ?? []).includes(of.card)));
  if ('origin' in of) return chipOrder(works.filter((w) => w.origin === of.origin));
  const ids = new Set(of.ids);
  return chipOrder(works.filter((w) => ids.has(w.id)));
}

/** The chips of a thing of `project`; nothing when it serves no task. */
export function TaskChipsOf({ project, of, testId }: { project: string; of: ChipsOf; testId?: string }) {
  const works = useWorks(project);
  const now = useNowFor(() => false);
  const key = JSON.stringify(of);
  // eslint-disable-next-line react-hooks/exhaustive-deps -- `key` is `of` by value
  const list = useMemo(() => tasksOf(works.works, of), [works, key]);
  if (list.length === 0) return null;
  const byId = new Map(list.map((w) => [w.id, w]));
  const lineOf = (id: string): string => {
    const w = byId.get(id);
    return w === undefined ? '' : lineAt({ project, projectName: project, w }, now);
  };
  return <TaskChips project={project} list={list} lineOf={lineOf} {...(testId === undefined ? {} : { testId })} />;
}
