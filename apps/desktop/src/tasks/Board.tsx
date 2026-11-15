// The Board (brief "all › opens the Board as a panel tab"): one column per status in the signed order
// (Needs you · Working · Waiting · Stalled · To review · Done this week · Paused · Ongoing), scrolling
// sideways like Linear; an empty column folds to a strip (its name, 0, a tip). No drag. A card opens
// its Task tab (over the Board, one click back); a stalled card has Restart, a paused one Resume.
// Its top line: 'since you looked: 3 done, 1 needs you, 2 moving' (the last time he opened it, kept;
// his first look counts the last day), on a 40 px head, its 24 px padding the columns' edge.
// Nothing on it: BoardEmpty, no since line. Owner: tasks.
import { useEffect, useRef, useState } from 'react';
import { Press, tipAttrs } from '@/ds';
import { keep } from '@/app/keep';
import { useAgents, useStore } from '@/store';
import type { Task as TaskReq, WorkStatus } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';
import { useNowFor } from '@/app/clock';
import { aboutOf, barRows, crewOf, cutAtWord, goalsOf, saidOf, STATUS_ORDER, type ProjWork } from './model';
import { ago } from '@/agents/model';
import { useAllWorks, useNewestArts } from './hooks';
import { NewDot, openTask, StatusMark } from './Marks';
import { isNew } from './news';
import { markActed, markSeen, useSeenTasks, useSettleActed } from './seen';
import { UndoToast, useUndo } from './UndoToast';
import type { Did } from './undo';
import { lineAt } from './Side';
import { sinceLooked } from './since';
import { BoardEmpty } from './BoardEmpty';

/** a column's head */
function colWord(s: WorkStatus): string {
  switch (s) {
    case 'needs_you':
      return tasks.col_needs_you;
    case 'working':
      return tasks.col_working;
    case 'waiting':
      return tasks.col_waiting;
    case 'stalled':
      return tasks.col_stalled;
    case 'to_review':
      return tasks.col_to_review;
    case 'done':
      return tasks.col_done;
    case 'paused':
      return tasks.col_paused;
    case 'ongoing':
      return tasks.col_ongoing;
    default:
      return tasks.col_unknown;
  }
}

const LOOKED = 'tasks.looked';
const DAY_MS = 24 * 3_600_000;

/** when he last opened this Board (the kept view), 0 never */
function lookedAt(project: string): number {
  const v = keep.view(LOOKED);
  const at = typeof v === 'object' && v !== null ? (v as Record<string, unknown>)[project] : undefined;
  return typeof at === 'number' ? at : 0;
}

function setLooked(project: string, at: number): void {
  const v = keep.view(LOOKED);
  const was = typeof v === 'object' && v !== null ? (v as Record<string, unknown>) : {};
  keep.setView(LOOKED, { ...was, [project]: at });
}

/** a task's gesture on the Board or its tab: through the hub's task_act (sb task's own steps) */
export function useTaskAct(): (project: string, step: 'done' | 'reopen' | 'pause' | 'resume', id: string) => void {
  const req = useTaskReq();
  return (project, step, id) => {
    req(project, { step, id });
  };
}

/** any of sb task's steps (rename, merge...) as his gesture */
export function useTaskReq(): (project: string, req: TaskReq) => void {
  const store = useStore();
  return (project, req) => {
    store.command({ cmd: 'task_act', project, req });
  };
}

function Card({ x, now, fresh, onDid }: { x: ProjWork; now: number; fresh: boolean; onDid: (d: Did) => void }) {
  const doAct = useTaskAct();
  // his gesture on the card: where it goes is his, not news (news.ts); its short Undo (undo.ts)
  const act = (project: string, step: 'resume', id: string): void => {
    markActed(project, x.w);
    doAct(project, step, id);
    onDid({ project, step, row: x.w });
  };
  // its agents as the Task tab lists them: main only when alone or working on it (designer m_64968)
  const agents = useAgents(x.project);
  const crew = crewOf(x.w.agents ?? [], (n) => agents.find((a) => a.name === n)?.status === 'working');
  const goals = goalsOf(x.w);
  const id = `tasks.board.card.${x.project}.${x.w.id}`;
  // its sentence: the newest report of its agents with its age (roma idea 1, designer); nothing
  // said yet: what its status is about ('no agent yet', 'quiet since 14:20')
  const said = saidOf((x.w.agents ?? []).flatMap((n) => {
    const a = agents.find((g) => g.name === n);
    return a === undefined ? [] : [a];
  }));
  const line = lineAt(x, now);
  const sentence = said?.text ?? (aboutOf(line) !== '' ? aboutOf(line) : line);
  return (
    <div className="tk-card" data-st={x.w.status} data-testid={id} role="button" tabIndex={0} onClick={() => {
      openTask(x.project, x.w.id);
    }} onKeyDown={(e) => {
      if (e.key === 'Enter') openTask(x.project, x.w.id);
    }} {...tipAttrs(x.w.name)}>
      <span className="tk-card-name">
        <StatusMark status={x.w.status} />
        <span className="tk-card-title">{cutAtWord(x.w.name, 40)}</span>
        {/* new since he looked: the ink dot (designer: never blue, never pink) */}
        <NewDot on={fresh} testId={`${id}.new`} />
      </span>
      <span className="tk-card-what" data-testid={`${id}.said`} {...tipAttrs(sentence)}>
        <span className="tk-card-said">{cutAtWord(sentence, 46)}</span>
        {said?.at != null && <span className="tk-card-age">{ago(said.at, now)}</span>}
      </span>
      {(goals !== null || crew.length > 0) && (
        <span className="tk-card-foot">
          {goals !== null && <span {...tipAttrs(tasks.goals_tip(goals))}>{tasks.goals_of(goals)}</span>}
          {crew.length > 0 && <span>{crew.join(', ')}</span>}
        </span>
      )}
      {x.w.status === 'stalled' && (
        // a stalled card's Restart, on the card (signed; tasks-ux m_64959)
        <span className="tk-card-acts">
          <Press look="b-btn sm" onClick={(e) => {
            e.stopPropagation();
            act(x.project, 'resume', x.w.id);
          }} data-testid={`${id}.restart`}>{tasks.act_restart}</Press>
        </span>
      )}
      {x.w.status === 'paused' && (
        <span className="tk-card-acts">
          <Press look="b-btn sm" onClick={(e) => {
            e.stopPropagation();
            act(x.project, 'resume', x.w.id);
          }} data-testid={`${id}.resume`}>{tasks.act_resume}</Press>
        </span>
      )}
    </div>
  );
}

/** The Board of `project` ('' every project's). */
export function BoardTab({ project }: { project: string }) {
  const all = useAllWorks(project);
  const now = useNowFor(() => false);
  const box = useRef<HTMLDivElement>(null);
  // the since line counts from his last look before this one; this look is kept when the tab goes
  const [since] = useState(() => lookedAt(project));
  // what he saw of each task (the dots): every task it shows is seen when the Board goes
  const seen = useSeenTasks();
  const arts = useNewestArts(project);
  useSettleActed(all);
  const shownRef = useRef<readonly ProjWork[]>([]);
  const [did, offer] = useUndo();
  const req = useTaskReq();
  useEffect(() => () => {
    setLooked(project, Date.now());
    markSeen(shownRef.current);
  }, [project]);
  // sideways with the wheel too (a trackpad scrolls both ways by itself)
  useEffect(() => {
    const el = box.current;
    if (el === null) return;
    const on = (e: WheelEvent): void => {
      if (Math.abs(e.deltaY) > Math.abs(e.deltaX) && e.target instanceof Element && e.target.closest('.tk-col-cards') === null) {
        el.scrollLeft += e.deltaY;
        e.preventDefault();
      }
    };
    el.addEventListener('wheel', on, { passive: false });
    return () => {
      el.removeEventListener('wheel', on);
    };
  }, []);
  const { done } = barRows(all, now);
  const shown = [...all.filter((x) => x.w.status !== 'done'), ...done];
  useEffect(() => {
    shownRef.current = shown;
  });
  // his first look counts the last day (designer m_64839: the line is always there)
  const n = sinceLooked(all, since > 0 ? since : now - DAY_MS);
  // nothing on it: no since line (nothing to compare), the empty says what a task is (m_65104)
  if (shown.length === 0)
    return (
      <div className="tk-board" data-testid="tasks.board">
        <BoardEmpty project={project} past={all.length > 0} />
      </div>
    );
  return (
    <div className="tk-board" data-testid="tasks.board">
      <header className="tk-board-head">
        <p className="tk-since" data-testid="tasks.board.since">
          {/* the zero parts left out: '1 needs you, 3 moving' (tasks-ux m_64959) */}
          {n.done + n.needs + n.moving === 0 ? tasks.since_none : (
            <>
              {tasks.since_head}
              {[
                n.done > 0 ? <span key="d">{tasks.since_done({ n: n.done })}</span> : null,
                n.needs > 0 ? <span key="n" className="tk-pink">{tasks.since_needs({ n: n.needs })}</span> : null,
                n.moving > 0 ? <span key="m">{tasks.since_moving({ n: n.moving })}</span> : null,
              ].filter((x) => x !== null).flatMap((x, i) => (i === 0 ? [x] : [', ', x]))}
            </>
          )}
        </p>
      </header>
      <div className="tk-cols" ref={box}>
          {STATUS_ORDER.filter((s) => s !== 'unknown' || shown.some((x) => x.w.status === 'unknown')).map((s) => {
            const list = shown.filter((x) => x.w.status === s).sort((a, b) => b.w.last_ms - a.w.last_ms);
            const label = colWord(s);
            if (list.length === 0)
              return (
                <div key={s} className="tk-col empty" data-testid={`tasks.board.col.${s}`} {...tipAttrs(tasks.col_empty({ col: label }))}>
                  <span className="tk-col-strip">
                    <StatusMark status={s} /> {label} <span className="tk-dim">0</span>
                  </span>
                </div>
              );
            return (
              <div key={s} className="tk-col" data-testid={`tasks.board.col.${s}`}>
                <h3 className="tk-col-head">
                  <StatusMark status={s} />
                  {label}
                  <span className={s === 'needs_you' ? 'tk-pink' : 'tk-dim'}>{list.length}</span>
                </h3>
                <div className="tk-col-cards">
                  {list.map((x) => (
                    <Card key={`${x.project}/${x.w.id}`} x={x} now={now} fresh={isNew(x.w, seen(x.project, x.w.id), arts.get(`${x.project}/${x.w.id}`) ?? 0)} onDid={offer} />
                  ))}
                </div>
              </div>
            );
          })}
      </div>
      <UndoToast did={did} works={all.filter((x) => x.project === did?.project).map((x) => x.w)} name={did?.row.name ?? ''} send={req} done={() => {
        offer(null);
      }} testId="tasks.board.toast" />
    </div>
  );
}
