// The store's shapes (generated from rust/proto) mapped to the search's own view types: the one
// place a protocol change reaches the search. Pure.
// The agents (archived too), cards and artifacts of the projects the core holds (live slices);
// the projects it does not hold from amb-core's index (2926a991: their view.json, as of
// written_ms); the scheduled tasks from amb-feed's typed list (284bd5c6) for a held project, from
// the index row's scheduled (26b2400b, its live timers as of written_ms) for the others.
import type { Agent, Artifact, Card, IndexRow, ProjectRow, ScheduledTask, WorkRow, WorkTomb } from '@/proto';
import { leaves, LEAVES } from '@/artifacts/model';
import { howOften } from '@/scheduled/model';
import type { SearchData, SearchScheduled } from './rank';

export interface StoreView {
  readonly projects: readonly ProjectRow[];
  readonly agents: (project: string) => readonly Agent[];
  readonly artifacts: (project: string) => readonly Artifact[];
  readonly cards: (project: string) => readonly Card[];
  readonly scheduled?: (project: string) => readonly ScheduledTask[];
  /** a project's tasks and its folded ones (HubEv works) */
  readonly works?: (project: string) => { readonly works: readonly WorkRow[]; readonly tombs: readonly WorkTomb[] };
  /** a task's line (tasks/what.ts at render's now) */
  readonly taskLine?: (project: string, w: WorkRow) => string;
  /** amb-core's index: the projects the core does not hold (store.index()). */
  readonly index?: readonly IndexRow[];
}

/** An artifact's dim words: 'page · v2', 'image'; a browser-only one (a PR, a link) '↗' (designer m_21714). */
const whatOf = (a: Artifact): string => (leaves(a) ? LEAVES : a.version > 1 ? `${a.kind} · v${String(a.version)}` : a.kind);

/** A scheduled task as the search reads it (its words, the agent it wakes, when next). */
const scheduledOf = (project: string, t: ScheduledTask): SearchScheduled => ({
  project,
  id: t.id,
  agent: t.agent,
  ...(t.name ? { name: t.name } : {}),
  words: t.words,
  every: howOften(t),
  ...(t.next_ms == null ? {} : { next_ms: t.next_ms }),
});

/** Everything the search reads, bise's home first, then his projects in his order. */
export function searchDataOf(v: StoreView): SearchData {
  const listed = [...v.projects].sort((a, b) => Number(b.home) - Number(a.home) || a.order - b.order);
  const ids = new Set(listed.map((p) => p.project));
  const idx = new Map((v.index ?? []).map((r) => [r.project, r]));
  // a project the index knows but his list doesn't (yet): searched too, named by its id
  const projects = [
    ...listed.map((p) => ({ id: p.project, name: p.name, home: p.home, cards: p.cards ?? [], working: p.working })),
    ...(v.index ?? []).filter((r) => !ids.has(r.project)).map((r) => ({ id: r.project, name: r.project, home: false, cards: [] as Card[], working: 0 })),
  ];
  // a held project's live slice wins; else what the index read from its files
  const agentsOf = (p: string): readonly Agent[] => {
    const live = v.agents(p);
    return live.length > 0 ? live : (idx.get(p)?.agents ?? []);
  };
  const artifactsOf = (p: string): readonly Artifact[] => {
    const live = v.artifacts(p);
    return live.length > 0 ? live : (idx.get(p)?.artifacts ?? []);
  };
  const scheduledOfP = (p: string): readonly ScheduledTask[] => {
    const live = v.scheduled?.(p) ?? [];
    return live.length > 0 ? live : (idx.get(p)?.scheduled ?? []);
  };
  return {
    projects: projects.map((p) => ({ id: p.id, name: p.name, home: p.home })),
    agents: projects.flatMap((p) => {
      const cards = v.cards(p.id);
      const open = cards.length > 0 ? cards : p.cards;
      return agentsOf(p.id).map((a) => {
        const ask = open.find((c) => c.agent === a.name)?.question;
        return {
          project: p.id,
          name: a.name,
          main: a.main,
          archived: a.archived,
          status: a.status,
          title: a.title,
          purpose: a.purpose,
          waits: a.waits,
          since_ms: a.since_ms,
          ...(ask === undefined ? {} : { ask }),
          ...(a.main ? { atWork: p.working } : {}),
        };
      });
    }),
    artifacts: projects.flatMap((p) =>
      artifactsOf(p.id).map((a) => ({ project: p.id, id: a.id, title: a.title, kind: a.kind, what: whatOf(a), agent: a.agent, at_ms: a.at_ms })),
    ),
    scheduled: projects.flatMap((p) => scheduledOfP(p.id).map((t) => scheduledOf(p.id, t))),
    // the tasks, then the folded done ones (a merged one is found through the task it went into)
    tasks: projects.flatMap((p) => {
      const w = v.works?.(p.id);
      if (w === undefined) return [];
      return [
        ...w.works.map((x) => ({ project: p.id, id: x.id, name: x.name, status: x.status, what: v.taskLine?.(p.id, x) ?? x.status, at_ms: x.last_ms })),
        ...w.tombs.filter((t) => (t.merged_into ?? null) === null).map((t) => ({ project: p.id, id: t.id, name: t.name, status: 'done' as const, what: 'done', at_ms: t.done_ms })),
      ];
    }),
  };
}
