// A branch's diff by its name (bar C15/W20/V6, the TUI's '/diff sb/x'; route branch/<p>/<branch>).
// An agent's branch (its worktree's) is that agent's review; any other branch (a feature's: its try
// card's '2 show the diff', code-viewer-width) is its diff against the trunk, the hub's `diff
// {branch}`, drawn by the review's DiffFiles with its guide (GuideTarget branch, architect m_57792).
// Owner: amb-win.
import { useCallback, useEffect, useMemo, useRef } from 'react';
import { usePageColumn } from '@/app/pagecol';
import { Head } from '@/ds';
import { branchDiffKey, useBranchDiff, useDiffError, useStore, useWorktrees } from '@/store';
import { MAIN } from '@/app/route';
import { useRouter } from '@/app/router';
import type { GuideTarget } from '@/proto';
import { DiffFiles } from './DiffFiles';
import { ReviewScreen } from './ReviewScreen';
import { pages } from '@/proto/words_pages.gen';
import './review.css';

/** The agent whose worktree is on that branch, if any. Pure. */
export function agentOfBranch(items: readonly { branch: string; agent?: string | null }[], branch: string): string | undefined {
  return items.find((w) => w.branch === branch && w.agent != null && w.agent !== '')?.agent ?? undefined;
}

export function BranchScreen({ project, branch }: { project: string; branch: string }) {
  const store = useStore();
  const items = useWorktrees(project);
  // the list fresh: who works on which branch
  useEffect(() => {
    store.command({ cmd: 'worktrees', project });
  }, [store, project]);
  const agent = agentOfBranch(items, branch);
  if (agent !== undefined) return <ReviewScreen project={project} agent={agent} />;
  return <BranchDiff project={project} branch={branch} />;
}

/** A branch no agent works on: its diff against the trunk, with its guide; a quote goes to main. */
function BranchDiff({ project, branch }: { project: string; branch: string }) {
  const store = useStore();
  const router = useRouter();
  const screen = useRef<HTMLElement>(null);
  const col = usePageColumn(screen);
  const diff = useBranchDiff(project, branch);
  const failed = useDiffError(branchDiffKey(project, branch));
  const ask = useCallback(() => {
    store.command({ cmd: 'diff', project, branch });
  }, [store, project, branch]);
  useEffect(ask, [ask]);
  const guide = useMemo((): GuideTarget => ({ kind: 'branch', name: branch }), [branch]);
  return (
    <section ref={screen} style={col} className="rv-screen" data-testid="branch.screen" data-project={project} data-branch={branch}>
      <Head title={branch} sub={pages.a_branch} />
      <DiffFiles
        project={project}
        diff={diff}
        loading={pages.review_loading_diff({ title: branch })}
        error={failed}
        onRetry={ask}
        none={pages.review_no_changes}
        to={MAIN}
        from={`${branch} vs ${diff?.base || 'main'}`}
        onEscape={() => {
          router.back();
        }}
        testId="branch"
        guide={guide}
      />
    </section>
  );
}
