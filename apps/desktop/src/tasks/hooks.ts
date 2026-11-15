// The tasks across his projects (or one project's), from the store's works:<project> slices (HubEv
// works; none until a hub said), with a stable snapshot per (ids, names), as useAllScheduled does
// (amb-win m_12088: a shared snapshot loops React's tearing check). Owner: tasks.
import { useSyncExternalStore } from 'react';
import { useProjects, useStore, type Works } from '@/store';
import type { ProjWork } from './model';

const last = new Map<string, { from: readonly Works[]; items: readonly ProjWork[] }>();

/** Every project's tasks (`only`: one project's; '' or none: all of them). */
export function useAllWorks(only?: string): readonly ProjWork[] {
  const store = useStore();
  const projects = useProjects();
  const shown = only === undefined || only === '' ? projects : projects.filter((p) => p.project === only);
  const ids = shown.map((p) => p.project).join('\n');
  const names = shown.map((p) => (p.name !== '' ? p.name : p.project)).join('\n');
  return useSyncExternalStore(
    (cb) => {
      const offs = ids === '' ? [] : ids.split('\n').map((p) => store.subscribe(`works:${p}`, cb));
      return () => {
        for (const off of offs) off();
      };
    },
    () => {
      const list = ids === '' ? [] : ids.split('\n');
      const nm = names.split('\n');
      const from = list.map((p) => store.works(p));
      const key = `${ids}\u0000${names}`;
      const was = last.get(key);
      if (was !== undefined && was.from.length === from.length && was.from.every((x, i) => x === from[i])) return was.items;
      const items: ProjWork[] = [];
      list.forEach((p, i) => {
        for (const w of from[i]?.works ?? []) items.push({ project: p, projectName: nm[i] ?? p, w });
      });
      last.set(key, { from, items });
      return items;
    },
  );
}
