// An agent's links in its thread's head (his m_57202: « à côté de son nom dans le header des liens
// utiles comme celui de la PR »; designer m_57254): its open PR first, then its worktree's branch,
// then its newest page. Each from what the hub already sent: the agent's place (barplaces.ts, the
// sidebar's own mark) and the project's artifacts. Nothing for an agent in the shared checkout with
// no PR and no page. Pure. Owner: nm-frame.
import type { Agent, Artifact, Card, Place } from '@/proto';
import { asksMerge, livePr, placeLabel, placeOf, tipString, tipWords, worktreeParts, type MarkTone } from './barplaces';

/** Its open PR: the sidebar's tint and words, its link for the PR tab (⌘: the forge's page). */
export interface PrLink {
  readonly n: number;
  readonly url: string;
  /** err when its checks fail, acc when a card asks him to merge it, dim a draft or a late forge, else mid */
  readonly tone: MarkTone;
  readonly fails: boolean;
  readonly merge: boolean;
  /** the sidebar mark's words: '#415 · checks fail: e2e/login · ψ worktree on sb/checkout' */
  readonly tip: string;
}

/** Its worktree: the branch (else the folder) it shows, the worktree's words for its tip, the branch screen's key. */
export interface BranchLink {
  readonly label: string;
  readonly tip: string;
  /** the branch screen's branch; null: a worktree with no branch (detached), no screen to open */
  readonly branch: string | null;
}

/** Its newest page: the artifact's tab. */
export interface PageLink {
  readonly id: string;
  readonly title: string;
}

export interface AgentLinks {
  readonly pr: PrLink | null;
  readonly branch: BranchLink | null;
  readonly page: PageLink | null;
}

/** The PR's tone, as the sidebar's mark (barplaces.ts markOf): its checks failing and his merge first. Pure. */
function prTone(fails: boolean, merge: boolean, dim: boolean): MarkTone {
  if (fails && !dim) return 'err';
  if (merge && !dim) return 'acc';
  return dim ? 'dim' : 'mid';
}

/**
 * The links of `a` (undefined, main, archived: none). `places`, `cards`, `arts`: its project's.
 * `home`: folded to '~' in the worktree's tip. Pure.
 */
export function agentLinksOf(a: Agent | undefined, places: readonly Place[], cards: readonly Card[], arts: readonly Artifact[], home?: string): AgentLinks {
  if (a === undefined || a.main || a.archived) return { pr: null, branch: null, page: null };
  const place = placeOf(a, places);
  const pr = place === undefined ? null : livePr(place);
  const fails = pr?.checks === 'fail';
  const merge = place !== undefined && asksMerge(place, cards);
  const dim = pr !== null && (pr.stale_ms != null || pr.state === 'draft');
  const page = arts.filter((x) => x.kind === 'page' && x.agent === a.name).reduce<Artifact | null>((m, x) => (m === null || x.at_ms > m.at_ms ? x : m), null);
  return {
    pr: pr === null || pr.url === '' ? null : { n: pr.number, url: pr.url, tone: prTone(fails, merge, dim), fails, merge, tip: place === undefined ? '' : tipWords(place, home) },
    branch: place === undefined ? null : { label: placeLabel(place), tip: tipString(worktreeParts(place, home)), branch: place.branch != null && place.branch !== '' ? place.branch : null },
    page: page === null ? null : { id: page.id, title: page.title },
  };
}

/** Nothing to show. Pure. */
export const noLinks = (l: AgentLinks): boolean => l.pr === null && l.branch === null && l.page === null;
