// A change in the panel (page pr-guided V2: one home per change, its review left the centre): an
// agent's live change, a branch's diff against its trunk, or a commit already on main, as the PR's
// tab draws a PR: its title, then, scrolling away over its guided review (DiffFiles, its bar
// sticky), one line of facts ('sb/perf → main · 3 files · +42 −7', a merged one 'merged · e0f3df5 ·
// 14:05') with its acts: Merge into main only by his click on the agent's open merge card (the
// hub's one /answer path), Open on GitHub while the agent has an open PR. A merged change or a
// commit is read-only. A quote or a part's 'Ask' goes to its agent: the change's, the commit's
// lander (main if none), main for a branch nobody works on. Owner: pr-artifacts.
import { useCallback, useEffect, useMemo, type ReactNode } from 'react';
import { Button, Head, Press } from '@/ds';
import { branchDiffKey, diffKey, useAgents, useBranchDiff, useCards, useDiff, useDiffError, useMerged, usePlaces, useStore, useWorktrees, type Diff } from '@/store';
import type { GuideTarget } from '@/proto';
import { clock } from '@/thread/entry';
import { useKeptScroll } from '@/app/keep';
import { MAIN } from '@/app/route';
import { pages } from '@/proto/words_pages.gen';
import { headLine, mergeCardOf, mergedSha, openPrOf, totals } from './model';
import { DiffFiles } from './DiffFiles';
import { agentOfBranch, type Change } from './openChange';
import './review.css';

export function ChangeTab({ project, change }: { project: string; change: Change }) {
  const store = useStore();
  const items = useWorktrees(project);
  const branch = 'branch' in change ? change.branch : null;
  // a branch's tab: who works on it, fresh (an agent's branch is that agent's change)
  useEffect(() => {
    if (branch !== null) store.command({ cmd: 'worktrees', project });
  }, [store, project, branch]);
  if ('commit' in change) return <AgentChange project={project} agent={MAIN} commit={change.commit} />;
  if ('agent' in change) return <AgentChange project={project} agent={change.agent} />;
  const agent = agentOfBranch(items, change.branch);
  if (agent !== undefined) return <AgentChange project={project} agent={agent} />;
  return <BranchChange project={project} branch={change.branch} />;
}

/** An agent's change, or one commit on main (read-only, its quotes to whoever landed it). */
function AgentChange({ project, agent, commit }: { project: string; agent: string; commit?: string }) {
  useKeptScroll(`review:${project}/${agent}/${commit ?? ''}`, () => document.querySelector<HTMLElement>('[data-testid="review.files"]'));
  const store = useStore();
  const diff = useDiff(project, agent, commit);
  const failed = useDiffError(diffKey(project, agent, commit));
  const merged = useMerged(project);
  const cards = useCards(project);
  const agents = useAgents(project);
  const pr = openPrOf(commit === undefined ? agent : undefined, agents, usePlaces(project));

  // the diff once on open (the hub answers `diff`, or refuses it: then its words and a retry), the
  // merged list fresh
  const ask = useCallback(() => {
    store.command(commit === undefined ? { cmd: 'diff', project, agent } : { cmd: 'diff', project, agent, commit });
  }, [store, project, agent, commit]);
  useEffect(() => {
    ask();
    store.command({ cmd: 'merged', project });
  }, [store, project, ask]);

  const done = mergedSha(diff, commit) !== undefined;
  const card = done ? undefined : mergeCardOf(cards, agent);
  // a commit's quotes go to the agent that landed it, while it is here; else to main
  const landed = commit === undefined ? undefined : merged.find((m) => m.sha.startsWith(commit) || commit.startsWith(m.sha));
  const lander = landed?.by;
  const to = commit === undefined ? agent : lander != null && agents.some((a) => a.name === lander) ? lander : MAIN;
  // the title says what changed (designer m_58359): the agent's task line, the commit's subject; its
  // name or sha only when there is no line
  const row = agents.find((a) => a.name === agent);
  const task = row === undefined ? '' : row.title !== '' ? row.title : row.purpose;
  const title = commit === undefined ? (task !== '' ? task : agent) : landed !== undefined && landed.title !== '' ? landed.title : pages.review_commit({ sha: commit.slice(0, 7) });
  const who = commit === undefined && task !== '' ? agent : '';
  // the change its guide is of (page review-guided): the agent's live change, or that commit
  const guide = useMemo((): GuideTarget => (commit === undefined ? { kind: 'agent', name: agent } : { kind: 'commit', agent, sha: commit }), [agent, commit]);
  const acts = (
    <>
      {card !== undefined && (
        <Button kind="ghost" size="sm" onClick={() => { store.do({ i: 'answer', project, card: card.id, reply: '1' }); }} testId="review.merge">
          {pages.review_merge}
        </Button>
      )}
      {pr !== null && (
        <Press look="rv-out" title={pages.review_open_pr_tip({ n: pr.number })} onClick={() => { void (window as Partial<Window>).bise?.openExternal(pr.url); }} data-testid="review.open-pr">
          {pages.review_open_pr}
        </Press>
      )}
    </>
  );
  return (
    <Shell testId="review" project={project} title={title} dot={done}
      diff={diff} line={[who, headLine(diff, { commit, merged, clock })].filter(Boolean).join(' · ')} acts={acts}
      body={(lead) => (
        <DiffFiles project={project} diff={diff} loading={loadingWords(agent, commit)} error={failed} onRetry={ask}
          none={commit === undefined ? pages.review_no_changes : pages.review_no_commit_changes}
          to={to} from={commit === undefined ? `${agent} vs main` : `commit ${commit.slice(0, 7)}`}
          onEscape={() => undefined} guide={guide} lead={lead} />
      )} />
  );
}

/** A branch no agent works on: its diff against the trunk, with its guide; a quote goes to main. */
function BranchChange({ project, branch }: { project: string; branch: string }) {
  const store = useStore();
  const diff = useBranchDiff(project, branch);
  const failed = useDiffError(branchDiffKey(project, branch));
  const ask = useCallback(() => {
    store.command({ cmd: 'diff', project, branch });
  }, [store, project, branch]);
  useEffect(ask, [ask]);
  const guide = useMemo((): GuideTarget => ({ kind: 'branch', name: branch }), [branch]);
  return (
    <Shell testId="branch" project={project} title={branch} dot={false}
      diff={diff} line={diff === null ? pages.a_branch : headLine(diff, { merged: [], clock })} acts={null}
      body={(lead) => (
        <DiffFiles project={project} diff={diff} loading={pages.review_loading_diff({ title: branch })} error={failed} onRetry={ask}
          none={pages.review_no_changes} to={MAIN} from={`${branch} vs ${diff?.base || 'main'}`}
          onEscape={() => undefined} testId="branch" guide={guide} lead={lead} />
      )} />
  );
}

/** The tab's frame: its title, then its line of facts and acts scrolling away over the guide. */
function Shell({ testId, project, title, dot, diff, line, acts, body }: {
  testId: string; project: string; title: string; dot: boolean; diff: Diff | null; line: string; acts: ReactNode; body: (lead: ReactNode) => ReactNode;
}) {
  const facts = [line, diff && diff.files.length > 0 ? totals(diff) : ''].filter(Boolean).join(' · ');
  return (
    <section className="rv-tab" data-testid={`${testId}.tab`} data-project={project}>
      <Head title={title} dot={dot ? 'ok' : undefined} />
      {body(
        <p className="rv-lead" data-testid={`${testId}.line`}>
          <span className="rv-lead-facts">{facts}</span>
          {acts}
        </p>,
      )}
    </section>
  );
}

/** What the review says while its diff comes: 'loading perf's changes…' (main is bise), a commit's. Pure. */
export function loadingWords(agent: string, commit: string | undefined): string {
  if (commit !== undefined) return pages.review_loading_commit({ sha: commit.slice(0, 7) });
  return pages.review_loading({ agent: agent === MAIN ? 'bise' : agent });
}
