// What he pinned (bise fait mac, decision d5, architect m_42051; every kind his m_49100, architect
// m_49104): an agent, an artifact, a scheduled task or a process, pinned from the right side's rows
// (a hover pin in the row's end) or its menu, or from the tab it is open in (its hover pin left of
// the tab's ×, panel/TabPin.tsx, and the tab's menu; tab-pin, his m_49983, m_50763), first in its list there
// and on its page. Desktop view
// state that lasts across windows and restarts, kept in the app's prefs through prefs_set, one key
// per kind and project hub id (pinKeyOf), each the pinned ids in the order he pinned them:
// `pins.<project>` the agents' names (the left sidebar's Pinned reads them too), `pins_art.<project>`
// artifact ids, `pins_sched.<project>` timer ids and watches' (watchPinId), `pins_proc.<project>` '<agent>/<process id>'
// (procPinId: a process's id carries its start, so a pin is that run and ends with it). Ids are the
// rows' own, never rebuilt. A pin whose thing left its list is not shown (a pinned row is only ever
// a row that is there), and the next write of that kind and project drops it, only once that
// project's list of that kind came (liveIds): never before its first event. An archived artifact
// stays in the hub's list, so its pin is kept and comes back when it comes back. Pinned rows go
// first by `pinFirst`, a stable partition (no client sorts on its own: each group keeps the hub's
// order, agents' ⌥N follow it, agents/model numbersOf). This module is the one writer (`pinThing`,
// `pin`) and the reader (`usePins`, `usePinned`). Owner: nm-frame.
import { useMemo } from 'react';
import type { Agent, JsonValue } from '@/proto';
import type { Entity } from '@/shared/entity';
import { getAppStore, usePrefs, type Store } from '@/store';
import { getPref, type Prefs } from '@/settings/prefs';

/** What he can pin. */
export type PinKind = 'agent' | 'artifact' | 'schedule' | 'process';

/** One pinned agent: its project's hub id and its name. */
export interface Pin {
  readonly project: string;
  readonly agent: string;
}

/** Each kind's prefs root; its project's list is `<root>.<project>`. */
const ROOTS: Record<PinKind, string> = { agent: 'pins', artifact: 'pins_art', schedule: 'pins_sched', process: 'pins_proc' };

/** The prefs key of a project's pins of one kind. */
export const pinKeyOf = (kind: PinKind, project: string): string => `${ROOTS[kind]}.${project}`;

/** The prefs key of a project's pinned agents. */
export const pinsKey = (project: string): string => pinKeyOf('agent', project);

/** A process's pin id: its agent and its id (which carries its start: one run). */
export const procPinId = (agent: string, id: string): string => `${agent}/${id}`;

/** A watch's pin id (an agent's sb wake, listed with the timers): its wake id, apart from the timers' ids. */
export const watchPinId = (id: number): string => `watch:${String(id)}`;

/** One thing's pin: its kind, its project and its pin id. */
export interface PinRef {
  readonly kind: PinKind;
  readonly project: string;
  readonly id: string;
}

/** The pin of a thing he can open (a row's, a menu's, a tab's); none for what has no pin (a PR, a commit, a project). Pure. */
export function pinRefOf(e: Entity): PinRef | undefined {
  switch (e.kind) {
    case 'agent':
      return { kind: 'agent', project: e.project, id: e.agent };
    case 'artifact':
      return { kind: 'artifact', project: e.project, id: e.id };
    case 'process':
      return { kind: 'process', project: e.project, id: procPinId(e.agent, e.id) };
    case 'scheduled':
      return { kind: 'schedule', project: e.project, id: String(e.id) };
    default:
      return undefined;
  }
}

/** A project's pinned ids from its prefs value: strings only, each once. Pure. */
export function namesOf(v: JsonValue | undefined): string[] {
  if (!Array.isArray(v)) return [];
  const out: string[] = [];
  for (const x of v) if (typeof x === 'string' && x !== '' && !out.includes(x)) out.push(x);
  return out;
}

/** One kind's pins, project → its ids in pin order. */
export type PinSet = ReadonlyMap<string, readonly string[]>;

/** One kind's pins in the prefs, project by project (the prefs' key order). Pure. */
export function pinSetOf(prefs: Prefs | null, kind: PinKind): PinSet {
  const all = getPref(prefs, ROOTS[kind]);
  if (all === null || all === undefined || typeof all !== 'object' || Array.isArray(all)) return new Map();
  return new Map(Object.entries(all).map(([project, v]) => [project, namesOf(v)] as const).filter(([, ids]) => ids.length > 0));
}

/** Whether `id` of `project` is in `set`. Pure. */
export const pinnedIn = (set: PinSet, project: string, id: string): boolean => set.get(project)?.includes(id) === true;

/** Every pinned agent, project by project, each project's in pin order. Pure. */
export function pinsOf(prefs: Prefs | null): Pin[] {
  return [...pinSetOf(prefs, 'agent')].flatMap(([project, names]) => names.map((agent) => ({ project, agent })));
}

/** The ids after a pin (`on`) or an unpin, dropping ids no longer in `live` (a rename's old key, a thing gone). Pure. */
export function pinned(names: readonly string[], id: string, on: boolean, live?: readonly string[]): string[] {
  const kept = names.filter((n) => n !== id && (live === undefined || live.includes(n)));
  return on ? [...kept, id] : kept;
}

/** The pins whose agent is still in its project's rows (the sidebar never shows a dead pin). Pure. */
export function livePins(pins: readonly Pin[], agents: (project: string) => readonly Agent[]): Pin[] {
  return pins.filter((p) => agents(p.project).some((a) => a.name === p.agent));
}

/** Whether `agent` of `project` is pinned in `pins`. Pure. */
export const isPinned = (pins: readonly Pin[], project: string, agent: string): boolean =>
  pins.some((p) => p.project === project && p.agent === agent);

/**
 * The pinned rows first, then the others: a stable partition, never a sort (architect m_49104):
 * each group keeps the order `rows` came in (the hub's), and with no pin it is `rows` itself. Pure.
 */
export function pinFirst<T>(rows: readonly T[], isPin: (r: T) => boolean): readonly T[] {
  const on: T[] = [];
  const off: T[] = [];
  for (const r of rows) (isPin(r) ? on : off).push(r);
  return on.length === 0 ? rows : [...on, ...off];
}

/** One kind's pins now, re-read when the prefs change. */
export function usePins(kind: PinKind): PinSet {
  const prefs = usePrefs() as Prefs | null;
  return useMemo(() => pinSetOf(prefs, kind), [prefs, kind]);
}

/** Every pinned agent now, re-read when the prefs change. */
export function usePinned(): readonly Pin[] {
  const prefs = usePrefs() as Prefs | null;
  return useMemo(() => pinsOf(prefs), [prefs]);
}

/** The ids of `project`'s things of `kind` the hub lists now; none until its list came (nothing is dropped before). */
export function liveIds(store: Store, kind: PinKind, project: string): readonly string[] | undefined {
  switch (kind) {
    case 'agent': {
      const names = store.agents(project).map((a) => a.name);
      return names.length === 0 ? undefined : names;
    }
    case 'artifact':
      return store.artifactsCame(project) ? store.artifacts(project).map((a) => a.id) : undefined;
    case 'schedule':
      // the timers, and the agents' watches listed with them (watchPinId)
      return store.scheduledCame(project)
        ? [...store.scheduled(project).map((t) => String(t.id)), ...store.agents(project).flatMap((a) => a.watching.map((w) => watchPinId(w.id)))]
        : undefined;
    case 'process':
      return store.processesCame(project) ? store.processes(project).map((p) => procPinId(p.agent, p.id)) : undefined;
  }
}

/** Pins (`on`) or unpins one thing: one prefs_set of its kind's list in its project. */
export function pinThing(kind: PinKind, project: string, id: string, on: boolean, store: Store = getAppStore()): void {
  const ids = namesOf(getPref(store.prefs() as Prefs | null, pinKeyOf(kind, project)));
  store.command({ cmd: 'prefs_set', key: pinKeyOf(kind, project), value: pinned(ids, id, on, liveIds(store, kind, project)) });
}

/** Pins (`on`) or unpins an agent. */
export function pin(project: string, agent: string, on: boolean): void {
  pinThing('agent', project, agent, on);
}
