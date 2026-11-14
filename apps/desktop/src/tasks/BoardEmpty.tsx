// The Board with nothing to show (his note m_65104: 'not polished'), signed by designer (m_65123: the
// kit's Empty centred in the Board's space, its top at ~40 %, 56ch; the title in ink; three numbered
// steps in text-2 on the title's edge; Ask bise under them) in tasks-ux's words (m_65124: 'bise',
// never 'main' or 'a turn'). No task ever: 'No tasks yet' and the steps; only past ones (done more
// than a week ago, off the Board): 'Nothing in progress' and where they went. No since line above it:
// nothing to compare. Ask bise: his composer on main's thread (bise's own on the every-project Board).
// Owner: tasks.
import { Button, Empty } from '@/ds';
import { useRouter, type Router } from '@/app/router';
import { MAIN } from '@/app/route';
import { tasks } from '@/proto/words_tasks.gen';

const COMPOSER_FIELD = '[data-testid="composer.field"]';

/** Ask bise: main's thread of `project` ('' bise's own), the caret in his composer, no words in it */
export function askBise(router: Router, project: string): void {
  router.go(project === '' ? { screen: 'bise' } : { screen: 'thread', project, agent: MAIN });
  requestAnimationFrame(() => {
    document.querySelector<HTMLElement>(COMPOSER_FIELD)?.focus();
  });
}

export function BoardEmpty({ project, past }: { project: string; past: boolean }) {
  const router = useRouter();
  const ask = (
    <Button testId="tasks.board.empty.ask" onClick={() => {
      askBise(router, project);
    }}>{tasks.ask_bise}</Button>
  );
  return (
    <div className="tk-board-none" data-testid="tasks.board.empty">
      {past ? (
        <Empty title={tasks.board_past} action={ask}>{tasks.board_past_more}</Empty>
      ) : (
        <Empty title={tasks.board_empty} action={ask}>
          {/* in the kit's detail <p>: phrasing content only, so spans as lines */}
          {[tasks.board_step_ask, tasks.board_step_shows, tasks.board_step_moves].map((s, i) => (
            <span key={s} className="tk-step"><span className="tk-dim">{i + 1}</span>{s}</span>
          ))}
        </Empty>
      )}
    </div>
  );
}
