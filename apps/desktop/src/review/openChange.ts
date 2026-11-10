// One home per change (page pr-guided V2): an agent's change, a branch's or a commit's opens in the
// panel as the guided review, the centre on its agent's thread; the review screen is gone from the
// centre. An agent with an open PR opens that PR (its guide is the same diff's), a branch an agent
// works on opens that agent's change. Every door goes through here: ⌘D, /diff, the ψ link, Changes'
// rows, the side's marks, a try card's 'show the diff', the agent menu's Changes. Owner: pr-artifacts.
import { useCallback } from 'react';
import type { Agent, Place, Worktree } from '@/proto';
import type { Entity } from '@/shared/entity';
import type { Route } from '@/app/route';
import { useRouter, type Router } from '@/app/router';
import { openTab } from '@/app/panel/tabs';
import { useStore } from '@/store';
import { openPrOf } from './model';

/** A change to read: an agent's live one, a branch's against the trunk, or a commit on main. */
export type Change = { readonly agent: string } | { readonly branch: string } | { readonly commit: string };

/** What the hub tells of the project: who works where, and their PRs. */
export interface ChangeFacts {
  readonly agents: readonly Agent[];
  readonly places: readonly Place[];
  readonly worktrees: readonly Worktree[];
}

/** The agent whose worktree is on that branch, if any. Pure. */
export function agentOfBranch(items: readonly { branch: string; agent?: string | null }[], branch: string): string | undefined {
  return items.find((w) => w.branch === branch && w.agent != null && w.agent !== '')?.agent ?? undefined;
}

/**
 * Where a change opens: its tab in the panel, and the centre's thread (null: the centre stays where
 * it is: a branch no agent works on, a commit). Pure.
 */
export function changeDoor(project: string, c: Change, f: ChangeFacts): { readonly tab: Entity; readonly centre: Route | null } {
  if ('commit' in c) return { tab: { kind: 'commit', project, sha: c.commit }, centre: null };
  const agent = 'agent' in c ? c.agent : agentOfBranch(f.worktrees, c.branch);
  if (agent === undefined) return { tab: { kind: 'branch', project, branch: 'branch' in c ? c.branch : '' }, centre: null };
  const pr = openPrOf(agent, f.agents, f.places);
  const tab: Entity = pr === null ? { kind: 'diff', project, agent } : { kind: 'pr', project, url: pr.url };
  return { tab, centre: { screen: 'thread', project, agent } };
}

/** The store's reads the door needs (the Store has them). */
export interface ChangeStore {
  agents(project: string): readonly Agent[];
  places(project: string): readonly Place[];
  worktrees(project: string): readonly Worktree[];
  command(c: { cmd: 'worktrees'; project: string }): void;
}

/** Opens a change where it lives: its tab in the panel, its agent's thread in the centre. */
export function openChange(store: ChangeStore, router: Pick<Router, 'go'> | null, project: string, c: Change): void {
  // who works on which branch: fresh for the next door
  if ('branch' in c) store.command({ cmd: 'worktrees', project });
  const d = changeDoor(project, c, { agents: store.agents(project), places: store.places(project), worktrees: store.worktrees(project) });
  openTab(d.tab);
  if (d.centre !== null) router?.go(d.centre);
}

/** openChange for a component: the store and router of its tree. */
export function useOpenChange(project: string): (c: Change) => void {
  const router = useRouter();
  const store = useStore();
  return useCallback((c: Change) => { openChange(store, router, project, c); }, [router, store, project]);
}
