// The thread's quiet line when its agent's PR has its guided review ready (page pr-guided V2,
// architect m_58302/m_58377): 'PR #412 · guided review ready · 3 parts ›', a click opens the PR's
// panel on it. A status, never a log entry: drawn at the thread's foot from the store's guide for
// that PR (the hub names what each guide answer is for), gone when the PR closes or its head moves
// until the new head's guide is ready. The thread asks it once per PR head, run false: the hub
// answers from what it keeps and never starts a call for it. Owner: pr-artifacts.
import { useEffect, useMemo } from 'react';
import { Press } from '@/ds';
import type { GuideTarget } from '@/proto';
import { pages } from '@/proto/words_pages.gen';
import { useAgents, useGuideFor, usePlaces, useStore } from '@/store';
import { MAIN } from '@/app/route';
import { openPrOf } from './model';
import { useOpenPr } from './openPr';
import './review.css';

export function GuideLine({ project, agent }: { project: string; agent: string }) {
  const store = useStore();
  const openPr = useOpenPr();
  const pr = openPrOf(agent === MAIN ? undefined : agent, useAgents(project), usePlaces(project));
  const url = pr?.url ?? null;
  const head = pr?.head_oid ?? '';
  const target = useMemo((): GuideTarget | null => (url === null ? null : { kind: 'pr', url }), [url]);
  // once per PR head: a push is a new diff, its guide asked again (it replaces the old answer)
  useEffect(() => {
    if (target !== null) store.command({ cmd: 'guide', project, target });
  }, [store, project, target, head]);
  const ev = useGuideFor(project, target);
  if (pr === null || ev?.state !== 'ready' || ev.guide == null) return null;
  return (
    <div className="th-notice info th-hubnote" role="status" data-testid="thread.guide">
      <Press look="rv-guide-go" onClick={(e) => { openPr(project, pr.url, e); }} data-testid="thread.guide.open">
        {pages.review_guide_line({ tag: `#${String(pr.number)}`, n: ev.guide.parts.length })} ›
      </Press>
    </div>
  );
}
