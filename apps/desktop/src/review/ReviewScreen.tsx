// The review (bar A.6): an agent's change, or one commit already on main (route commit), file by
// file, under its own head (bise fait mac §9: no banner; its words through the catalog, the floating
// composer's room at the foot). Owner: nm-pages-b (amb-win's before). The old window's S7 rules, kept: a merged
// change or a commit from 'merged today' is read-only ('merged · e0f3df5
// · 14:05', no merge, no ask for changes); a merge happens only by his
// click on the agent's open merge card (the hub's one /answer path).
import { useEffect, useRef } from 'react';
import { Button, Head } from '@/ds';
import { useCards, useDiff, useMerged, useStore } from '@/store';
import { clock } from '@/thread/entry';
import { useRouter } from '@/app/router';
import { usePageColumn } from '@/app/pagecol';
import { useKeptScroll } from '@/app/keep';
import { MAIN } from '@/app/route';
import { pages } from '@/proto/words_pages.gen';
import { headLine, mergeCardOf, mergedSha, totals } from './model';
import { DiffFiles } from './DiffFiles';
import './review.css';

export function ReviewScreen({ project, agent, commit }: { project: string; agent: string; commit?: string }) {
  useKeptScroll(`review:${project}/${agent}/${commit ?? ''}`, () => document.querySelector<HTMLElement>('[data-testid="review.files"]'));
  const store = useStore();
  const router = useRouter();
  const diff = useDiff(project, agent, commit);
  const merged = useMerged(project);
  const cards = useCards(project);

  // the diff once on open (the hub answers `diff`), the merged list fresh
  useEffect(() => {
    store.command(commit === undefined ? { cmd: 'diff', project, agent } : { cmd: 'diff', project, agent, commit });
    store.command({ cmd: 'merged', project });
  }, [store, project, agent, commit]);

  const screen = useRef<HTMLElement>(null);
  const col = usePageColumn(screen);
  const target = commit === undefined ? agent : MAIN;
  const from = commit === undefined ? `${agent} vs main` : `commit ${commit.slice(0, 7)}`;

  const done = mergedSha(diff, commit) !== undefined;
  const card = done ? undefined : mergeCardOf(cards, agent);
  const title = commit === undefined ? agent : pages.review_commit({ sha: commit.slice(0, 7) });
  const line = headLine(diff, { commit, merged, clock });

  return (
    <section ref={screen} style={col} className="rv-screen" data-testid="review.screen" data-project={project} data-agent={agent}>
      <Head
        title={title}
        sub={[line, diff && diff.files.length > 0 ? totals(diff) : ''].filter(Boolean).join(' · ')}
        dot={done ? 'ok' : undefined}
        end={
          <>
            {card !== undefined && (
              <Button
                kind="pri"
                onClick={() => {
                  store.do({ i: 'answer', project, card: card.id, reply: '1' });
                }}
                testId="review.merge"
              >
                {pages.review_merge}
              </Button>
            )}
            <Button
              kind="ghost"
              onClick={() => {
                router.go({ screen: 'thread', project, agent: commit === undefined ? agent : MAIN });
              }}
              testId="review.thread"
            >
              {pages.review_thread}
            </Button>
          </>
        }
      />
      <DiffFiles
        project={project}
        diff={diff}
        loading={loadingWords(agent, commit)}
        none={commit === undefined ? pages.review_no_changes : pages.review_no_commit_changes}
        to={target}
        from={from}
        onEscape={() => {
          router.back();
        }}
      />
    </section>
  );
}

/** What the review says while its diff comes: 'loading perf's changes…' (main is bise), a commit's. Pure. */
export function loadingWords(agent: string, commit: string | undefined): string {
  if (commit !== undefined) return pages.review_loading_commit({ sha: commit.slice(0, 7) });
  return pages.review_loading({ agent: agent === MAIN ? 'bise' : agent });
}
