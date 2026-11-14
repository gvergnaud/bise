// The right bar's Tasks section (brief "Right bar"): under Agents, above Artifacts. One row per open
// task in the Board's order (its mark, its name, its done-when n/m, a pink dot when it needs him),
// then one fold line for the ongoing ones and the ones done this week (it opens the Board, where they
// sit at the right end). Head: `Tasks 9 · 2 need you` and `all ›` (the Board as a panel tab). A row
// opens its Task tab. Owner: tasks.
import { Press, tipAttrs } from '@/ds';
import { OuterList } from '@/app/side3d/OuterCard';
import { Empty } from '@/app/side3d/Flyout';
import { groupParts } from '@/app/side3d/lists';
import { tasks } from '@/proto/words_tasks.gen';
import { barRows, cutAtWord, goalsOf, needsHim, type ProjWork } from './model';
import { openBoard, openTask, StatusMark } from './Marks';
import { whatOf } from './what';

/** how a past and a coming time read in a row's line: the hour, a date past a day */
function clock(ms: number, now: number): string {
  const d = new Date(ms);
  const hm = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  return Math.abs(now - ms) < 20 * 3_600_000 ? hm : `${d.toLocaleDateString([], { weekday: 'short' })} ${hm}`;
}

/** A task's line at `now` (what.ts), its times as the bar writes them. */
export function lineAt(x: ProjWork, now: number): string {
  return whatOf(x.w, now, (ms) => clock(ms, now), (ms) => clock(ms, now));
}

function TaskRow({ x, now, group }: { x: ProjWork; now: number; group: string | undefined }) {
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
        <span className="s3-orow-gap" aria-hidden="true" />
        {goals !== null && <span className="tk-goals" {...tipAttrs(tasks.goals_tip(goals))} data-testid={`${id}.goals`}>{tasks.goals_of(goals)}</span>}
        {needsHim(x.w) && <span className="tk-needs" data-testid={`${id}.needs`} />}
      </Press>
    </div>
  );
}

/** The section: `scope` the bar's project ('' every project's: bise's own); `groupOf` a project's group words. */
export function OuterTasks({ all, scope, now, groupOf }: { all: readonly ProjWork[]; scope: string; now: number; groupOf: (project: string) => string | undefined }) {
  const { open, ongoing, done } = barRows(all, now);
  const needs = open.filter((x) => needsHim(x.w)).length;
  const rest = ongoing.length + done.length;
  return (
    <OuterList id="tasks" name={tasks.section} count={String(open.length)} {...(needs > 0 ? { loud: tasks.head_needs({ n: needs }) } : {})}
      onAll={() => {
        openBoard(scope);
      }}
      total={open.length} empty={<Empty>{tasks.none_open}</Empty>}
      rows={(n) => open.slice(0, n).map((x) => <TaskRow key={`${x.project}/${x.w.id}`} x={x} now={now} group={groupOf(x.project)} />)}
      after={rest > 0 && (
        <Press look="tk-fold" data-testid="app.outer.tasks.rest" onClick={() => {
          openBoard(scope);
        }}>
          <StatusMark status="ongoing" /> {tasks.fold_rest({ ongoing: ongoing.length, done: done.length })}
        </Press>
      )} />
  );
}
