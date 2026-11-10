// The right bar's scope (his ask, main m_25024): ARTIFACTS and SCHEDULED & WATCHING follow the
// thread in view. bise's thread: every project's, no filter; a project's main (or any project
// screen that is no agent's): that project's; a sub-agent's thread, review or log: only what that
// agent made, its own timers and alarms. AGENTS and ARCHIVED keep the project's (or every
// project's) list. Pure. Owner: amb-win.
import { panel } from '@/proto/words_panel.gen';
import type { Route } from './route';
import { MAIN } from './route';

export type BarScope = { readonly level: 'bise' } | { readonly level: 'project' } | { readonly level: 'agent'; readonly agent: string };

/** The scope of the bar of `project` ('' = bise's thread, every project) on `route`. Pure. */
export function barScope(route: Route, project: string): BarScope {
  if (project === '') return { level: 'bise' };
  switch (route.screen) {
    case 'thread':
    case 'debug':
      return route.project === project && route.agent !== MAIN ? { level: 'agent', agent: route.agent } : { level: 'project' };
    default:
      return { level: 'project' };
  }
}

/** Whether a row of `agent` (who made the artifact, who the task wakes, whose alarm) shows. Pure. */
export function inScope(scope: BarScope, agent: string): boolean {
  return scope.level !== 'agent' || scope.agent === agent;
}

/** A scoped head's words (designer m_25031): a line under the head, their tip, the line at 0 rows. */
export interface ScopeWords {
  readonly words: string;
  readonly tip: string;
  readonly empty: string;
}

/**
 * The words a section head says of its scope (designer m_25031), on a line of their own under it
 * (bar-heads, designer m_26425): on a sub-agent's thread 'by perf'
 * (ARTIFACTS), 'for perf' (SCHEDULED & WATCHING: what wakes it); none on a project's main or bise's
 * thread, the bar's top says it. `projectName`: whose full list 'all ›' opens. Pure.
 */
export function scopeWords(scope: BarScope, section: 'artifacts' | 'scheduled', projectName: string): ScopeWords | null {
  if (scope.level !== 'agent') return null;
  const a = scope.agent;
  return section === 'artifacts'
    ? { words: panel.scope_by({ agent: a }), tip: panel.scope_by_tip({ agent: a, project: projectName }), empty: panel.scope_by_none({ agent: a }) }
    : { words: panel.scope_for({ agent: a }), tip: panel.scope_for_tip({ agent: a, project: projectName }), empty: panel.scope_for_none({ agent: a }) };
}

/**
 * A folded scoped head's count tip (designer m_26425): the scope line folds with the rows, the
 * count says the scope instead ('2 scheduled for perf', '1 artifact by perf'). Pure.
 */
export function scopeCountTip(section: 'artifacts' | 'scheduled', n: number, scope: ScopeWords): string {
  return section === 'scheduled' ? panel.scope_count_scheduled({ n, scope: scope.words }) : panel.scope_count_artifacts({ n, scope: scope.words });
}
