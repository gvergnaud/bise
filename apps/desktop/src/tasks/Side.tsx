// The right bar's Tasks section (brief "Right bar"): under Agents, above Artifacts. One row per open
// task in the Board's order (its mark, the pink dot when it needs him; its name; its done-when n/m),
// then one fold line for the ongoing ones and the ones done this week (it opens the Board, where they
// sit at the right end). Head: `Tasks 9 · 2 need you` and `all ›` (the Board as a panel tab). A row
// opens its Task tab. None ever: 'No tasks yet · Ask bise'; only ongoing or done: 'Nothing in
// progress' and the fold. Owner: tasks.
import { Press, tipAttrs } from '@/ds';
import { OuterHead, OuterList } from '@/app/side3d/OuterCard';
import { Empty } from '@/app/side3d/Flyout';
import { groupParts } from '@/app/side3d/lists';
import { tasks } from '@/proto/words_tasks.gen';
import { dayWords, timeOf } from '@/artifacts/model';
import { barRows, cutAtWord, goalsOf, needsHim, type ProjWork } from './model';
import { NewDot, openBoard, openTask, StatusMark } from './Marks';
import { useNewestArts } from './hooks';
import { isNew } from './news';
import { useSeenTasks, useSettleActed } from './seen';
import { whatOf } from './what';
import { askBise } from './BoardEmpty';
import { useRouter } from '@/app/router';

/** how a past and a coming time read in a row's line: the hour, a date past a day */
export function clock(ms: number, now: number): string {
  // 24 h like the rest of the app ('14:20'); another day: 'yesterday 14:20', 'sun 5 oct 14:20' (designer m_64958)
  const day = dayWords(ms, now);
  return day === 'today' ? timeOf(ms) : `${day} ${timeOf(ms)}`;
}

/** A task's line at `now` (what.ts), its times as the bar writes them. */
export function lineAt(x: ProjWork, now: number): string {
  return whatOf(x.w, now, (ms) => clock(ms, now), (ms) => clock(ms, now));
}

function TaskRow({ x, now, group, fresh }: { x: ProjWork; now: number; group: string | undefined; fresh: boolean }) {
  const id = `app.outer.tasks.${x.project}.${x.w.id}`;
  const g = groupParts(group, cutAtWord(x.w.name, 26));
  const goals = goalsOf(x.w);
  const line = lineAt(x, now);
  return (
    <div className="s3-orow tk-orow" data-testid={id}>
      <Press look="s3-orow-open" aria-label={x.w.name} {...tipAttrs(`${x.w.name} · ${line}`, undefined, 'left')} onClick={() => {
        openTask(x.project, x.w.id);
      }} data-row="">
        <span className="s3-orow-mark">
          <StatusMark status={x.w.status} testId={`${id}.mark`} />
        </span>
        <span className="s3-orow-name" data-testid={`${id}.name`}>{g.project !== '' && <span className="s3-group">{g.project}</span>}{g.name}</span>
        <NewDot on={fresh} testId={`${id}.new`} />
        <span className="s3-orow-gap" aria-hidden="true" />
        {goals !== null && <span className="tk-goals" {...tipAttrs(tasks.goals_tip(goals))} data-testid={`${id}.goals`}>{tasks.goals_of(goals)}</span>}
      </Press>
    </div>
  );
}

/** The section: `scope` the bar's project ('' every project's: bise's own); `groupOf` a project's group words. */
export function OuterTasks({ all, scope, now, groupOf }: { all: readonly ProjWork[]; scope: string; now: number; groupOf: (project: string) => string | undefined }) {
  const router = useRouter();
  const { seen, looked } = useSeenTasks();
  const arts = useNewestArts(scope);
  useSettleActed(all);
  const { open, ongoing, done } = barRows(all, now);
  const needs = open.filter((x) => needsHim(x.w)).length;
  const rest = ongoing.length + done.length;
  return (
    <OuterList id="tasks" name={tasks.section} count={String(open.length)} {...(needs > 0 ? { loud: tasks.head_needs({ n: needs }) } : {})}
      onAll={() => {
        openBoard(scope);
      }}
      total={open.length} empty={all.length === 0 ? (
        // none ever: one quiet line and its way in (tasks-ux m_65124)
        <Empty>
          {tasks.none_yet} · <Press look="tk-more" onClick={() => {
            askBise(router, scope);
          }} data-testid="app.outer.tasks.ask">{tasks.ask_bise}</Press>
        </Empty>
      ) : <Empty>{tasks.none_open}</Empty>}
      rows={(n) => open.slice(0, n).map((x) => <TaskRow key={`${x.project}/${x.w.id}`} x={x} now={now} group={groupOf(x.project)} fresh={isNew(x.w, seen(x.project, x.w.id), arts.get(`${x.project}/${x.w.id}`) ?? 0, looked)} />)}
      after={rest > 0 && (
        <Press look="tk-fold" data-testid="app.outer.tasks.rest" onClick={() => {
          openBoard(scope);
        }}>
          <StatusMark status="ongoing" /> {tasks.fold_rest({ ongoing: ongoing.length, done: done.length })}
        </Press>
      )} />
  );
}

/** At widths where the right bar folds to coins: the Tasks head in the agents coin's card, under its
 *  agents, as the wide bar has it ('Tasks 7 · 1 needs you', all › opens the Board; designer m_65859,
 *  tasks-ux m_65804: the Board reachable at every width); its first `rows` open tasks under it, none
 *  when the card has no room. */
export function FloatTasks({ all, scope, now, groupOf, rows }: { all: readonly ProjWork[]; scope: string; now: number; groupOf: (project: string) => string | undefined; rows: number }) {
  const { seen, looked } = useSeenTasks();
  const arts = useNewestArts(scope);
  const { open } = barRows(all, now);
  const needs = open.filter((x) => needsHim(x.w)).length;
  if (all.length === 0) return null;
  return (
    <div className="tk-float" data-testid="app.flyout.agents.tasks">
      <OuterHead id="tasks" name={tasks.section} count={String(open.length)} {...(needs > 0 ? { loud: tasks.head_needs({ n: needs }) } : {})} onAll={() => {
        openBoard(scope);
      }} />
      {open.slice(0, rows).map((x) => <TaskRow key={`${x.project}/${x.w.id}`} x={x} now={now} group={groupOf(x.project)} fresh={isNew(x.w, seen(x.project, x.w.id), arts.get(`${x.project}/${x.w.id}`) ?? 0, looked)} />)}
    </div>
  );
}
