// The sidebar's rows: 'Projects', one row per project in his order (Settings › Projects), no agent
// under it: the agents of a project live on the right side, his pinned ones first (no 'Pinned'
// here, his m_52954). A project row is Gabriel's H1 (round 3, page
// sidebar-projects, designer m_44783): its name, the inbox pill or its age, one plain sentence and
// its week (projectfacts.ts says them); a quiet one is one line. With ⌘ held, its ⌘ digit in the
// pill's place (⌘1-9 opens it, keys.ts).
// Pure: Sidebar.tsx draws them. Owner: r3-projects this round (nm-frame's, archived).
import type { Agent, ProjectRow } from '@/proto';
import { liveOf } from '@/agents/model';
import { projectFacts, type ProjectFacts } from './projectfacts';

/** A project's row in the sidebar. */
export interface SideRow {
  readonly p: ProjectRow;
  readonly on: boolean;
  readonly facts: ProjectFacts;
  readonly digit?: number;
}

/** His projects in his order, bise's own out (the rail's ':*' is it). Pure. */
export function projectsOf(rows: readonly ProjectRow[]): ProjectRow[] {
  return rows.filter((p) => !p.home).sort((a, b) => a.order - b.order || a.name.localeCompare(b.name));
}

/** How many of a project's live agents work now. Pure. */
export function workingOf(agents: readonly Agent[]): number {
  return liveOf(agents).filter((a) => a.status === 'working').length;
}

/** A project's row: `waits` is the inbox's own count for it (inbox/waiting useProjectWaiting), `index` its place in projectsOf (⌘ index+1, up to 9). Pure. */
export function projectRowOf(c: {
  readonly p: ProjectRow;
  readonly agents: readonly Agent[];
  readonly waits: number;
  readonly index: number;
  /** the project in view */
  readonly on: boolean;
  /** ⌘ is held: the row shows its digit */
  readonly cmd: boolean;
  /** when he last looked at it (seen.ts) */
  readonly seen?: number | undefined;
  readonly now: number;
}): SideRow {
  const facts = projectFacts({ p: c.p, name: c.p.name, working: workingOf(c.agents), waits: c.waits, seen: c.seen, now: c.now });
  const digit = c.cmd && c.index < 9 ? c.index + 1 : undefined;
  return { p: c.p, on: c.on, facts, ...(digit === undefined ? {} : { digit }) };
}
