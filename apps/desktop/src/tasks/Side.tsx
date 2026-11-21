// The right bar's Tasks section (brief "Right bar"): under Agents, above Artifacts. One row per open
// task in the Board's order (its mark, the pink dot when it needs him; its name; its done-when n/m),
// then one fold line for the ongoing ones and the ones done this week (it opens the Board, where they
// sit at the right end). Head: `Tasks 9 · 2 need you` and `all ›` (the Board as a panel tab). A row
// opens its Task tab; its name takes the row up to its n/m, cut at a word to its measured room (ds
// WordCut). None open or folded: no section (OuterList); only ongoing or done: 'Nothing in progress'
// and the fold. Owner: tasks.
import { PrStateIcon, Press, tipAttrs, WordCut } from '@/ds';
import { OuterHead, OuterList } from '@/app/side3d/OuterCard';
import { Empty } from '@/app/side3d/Flyout';
import { groupParts } from '@/app/side3d/lists';
import { tasks } from '@/proto/words_tasks.gen';
import { dayWords, timeOf } from '@/artifacts/model';
import { useState } from 'react';
import { barRows, DONE_SHOWN, goalsOf, listRows, needsHim, type Kept, type ProjWork, type prSet } from './model';
import { NewDot, openBoard, openTask, TaskCheck, TaskMark } from './Marks';
import { useTaskAct, useTaskReq } from './Board';
import { BatchToast, UndoToast, useBatchUndo, useUndo } from './UndoToast';
import { useNewestArts, useTaskPrs } from './hooks';
import { isNew } from './news';
import { useSeenTasks, useSettleActed } from './seen';
import { whatOf } from './what';

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

/** A row of the todo list. `check`: the right bar's rows have his box in the mark slot (his m_66588,
 *  designer m_66969), its own button beside the row's (never a button in a button); without it
 *  (the coin card's few rows) the status mark. */
function TaskRow({ x, now, group, fresh, check, prs }: { x: ProjWork; now: number; group: string | undefined; fresh: boolean; check?: { checked: boolean; onToggle: () => void }; prs?: ReturnType<typeof prSet> | undefined }) {
  const id = `app.outer.tasks.${x.project}.${x.w.id}`;
  // the name takes the row up to its n/m, cut at a word to its measured room (designer m_66328)
  const g = groupParts(group, x.w.name);
  const goals = goalsOf(x.w);
  const line = lineAt(x, now);
  // every done-when item ticked, still open: an ink box ready to check (main m_66590)
  const ready = check !== undefined && !check.checked && goals !== null && goals.done === goals.all;
  return (
    <div className="s3-orow tk-orow" data-testid={id} data-checked={check?.checked === true || undefined}>
      {check !== undefined && <TaskCheck status={x.w.status} checked={check.checked} ready={ready} goals={goals} onToggle={check.onToggle} testId={`${id}.box`} />}
      <Press look="s3-orow-open" aria-label={x.w.name} {...tipAttrs(`${x.w.name} · ${line}`, undefined, 'left')} onClick={() => {
        openTask(x.project, x.w.id);
      }} data-row="">
        <span className="s3-orow-mark">
          {/* a live row's mark says its status line on hover (his rule: a compact mark explains itself; ui-review m_67794) */}
          {check === undefined && <TaskMark status={x.w.status} tip={line} testId={`${id}.mark`} />}
        </span>
        <span className="s3-orow-name tk-oname" data-testid={`${id}.name`}>{g.project !== '' && <span className="s3-group">{g.project}</span>}<WordCut className="tk-ocut" text={g.name} /></span>
        <NewDot on={fresh} testId={`${id}.new`} />
        {/* every PR merged: one 12 px merged mark in place of its n/m, the set in its tip; before that
            the n/m stays, the set in its tip too (designer m_66721) */}
        {prs?.allMerged === true ? (
          <span className="tk-goals tk-merged" {...tipAttrs(prsTip(prs, goals))} data-testid={`${id}.merged`}><PrStateIcon state="merged" size={12} /></span>
        ) : goals !== null && <span className="tk-goals" {...tipAttrs(prs === undefined ? tasks.goals_tip(goals) : prsTip(prs, goals))} data-testid={`${id}.goals`}>{tasks.goals_of(goals)}</span>}
      </Press>
    </div>
  );
}

/** the right bar's tip on a task with PRs: '5 PRs · 2 open · 1 draft · 1 merged · 0/1 done when' */
function prsTip(p: ReturnType<typeof prSet>, goals: { readonly done: number; readonly all: number } | null): string {
  // a zero part left out, never '0 open'; the done-when count last (designer m_67612)
  const parts = [tasks.prs_n({ n: p.n }), p.open > 0 ? tasks.prs_open({ n: p.open }) : '', p.draft > 0 ? tasks.prs_draft({ n: p.draft }) : '', p.merged > 0 ? tasks.prs_merged({ n: p.merged }) : '', goals === null ? '' : tasks.prs_goals(goals)];
  return parts.filter((x) => x !== '').join(' · ');
}

/** `k` kept unchecked at its place (an Undo of his box). Pure. */
function uncheck(m: ReadonlyMap<string, Kept>, k: string): ReadonlyMap<string, Kept> {
  const v = m.get(k);
  return v === undefined ? m : new Map(m).set(k, { ...v, checked: false });
}

/** The section: `scope` the bar's project ('' every project's: bise's own); `groupOf` a project's group words.
 *  His pick B on page tasks-true (tasks-ux m_67726, designer m_67727): in progress first (needs you,
 *  working), then To check (finished, his OK left) under one 'Mark all done' row, then what waits;
 *  the box only where his click is safe (To check, done); the last done ones struck through. */
export function OuterTasks({ all, scope, now, groupOf }: { all: readonly ProjWork[]; scope: string; now: number; groupOf: (project: string) => string | undefined }) {
  const { seen, looked } = useSeenTasks();
  const arts = useNewestArts(scope);
  const prs = useTaskPrs(scope);
  useSettleActed(all);
  const { open, ongoing, done } = barRows(all, now);
  // his box: Mark done or Reopen, the row kept at its place while the list is in view (designer m_66969)
  const [kept, setKept] = useState<ReadonlyMap<string, Kept>>(new Map());
  // the struck rows he reopened: unchecked in place, in sight, until the hub's row is open again (ui-review m_67804)
  const [reopened, setReopened] = useState<ReadonlySet<string>>(new Set());
  const [did, offer] = useUndo();
  const [batch, offerBatch] = useBatchUndo();
  const act = useTaskAct();
  const req = useTaskReq();
  const key = (x: ProjWork) => `${x.project}/${x.w.id}`;
  const listed = listRows(open, kept);
  const inList = new Set(listed.map((r) => key(r.x)));
  // the last done ones, struck through, after the open ones; the rest fold
  // (always in sight under the open ones and their '+N more', never behind it: ui-review m_67794)
  const struck = done.filter((x) => !inList.has(key(x))).slice(0, DONE_SHOWN);
  const rows = listed;
  const shown = new Set([...rows.map((r) => key(r.x)), ...struck.map(key)]);
  const needs = open.filter((x) => needsHim(x.w)).length;
  const toCheck = listed.filter((r) => !r.checked && r.x.w.status === 'to_review').map((r) => r.x);
  const rest = ongoing.length + done.filter((x) => !shown.has(key(x))).length;
  const toggle = (x: ProjWork, at: number, checked: boolean) => {
    setKept((m) => new Map(m).set(key(x), { x, at, checked: !checked }));
    offerBatch(null);
    if (checked) {
      act(x.project, 'reopen', x.w.id);
      offer(null);
    } else {
      act(x.project, 'done', x.w.id);
      offer({ project: x.project, step: 'done', row: x.w });
    }
  };
  // one gesture, one Undo: every To check task done, each kept checked at its place
  const doneAll = () => {
    setKept((m) => {
      const n = new Map(m);
      for (const x of toCheck) n.set(key(x), { x, at: rows.findIndex((r) => key(r.x) === key(x)), checked: true });
      return n;
    });
    for (const x of toCheck) act(x.project, 'done', x.w.id);
    offer(null);
    offerBatch(toCheck);
  };
  const unticked = toCheck.filter((x) => {
    const g = goalsOf(x.w);
    return g !== null && g.done < g.all;
  }).length;
  const firstCheck = toCheck.length > 1 ? rows.findIndex((r) => key(r.x) === key(toCheck[0] as ProjWork)) : -1;
  return (
    <OuterList id="tasks" name={tasks.section} count={String(open.length)} {...(needs > 0 ? { loud: tasks.head_needs({ n: needs }) } : {})}
      onAll={() => {
        openBoard(scope);
      }}
      // no open task and nothing in the fold: no section (OuterList; his m_66323, designer m_66328)
      total={rows.length} empty={<Empty>{tasks.none_open}</Empty>}
      rows={(n) => rows.slice(0, n).flatMap((r, i) => {
        // the box only where his click is safe: a finished task (To check) or a done one; a live
        // task shows its mark, closed from its Task tab (tasks-ux m_67726)
        // one he just acted on keeps its box until the hub answers (a Reopen too; ui-review m_67794)
        const boxed = r.checked || r.x.w.status === 'to_review' || kept.has(key(r.x));
        const row = (
          <TaskRow key={key(r.x)} x={r.x} now={now} group={groupOf(r.x.project)} prs={prs.get(key(r.x))} fresh={!r.checked && isNew(r.x.w, seen(r.x.project, r.x.w.id), arts.get(key(r.x)) ?? 0, looked)}
            {...(boxed ? { check: { checked: r.checked, onToggle: () => {
              toggle(r.x, i, r.checked);
            } } } : {})} />
        );
        return i === firstCheck ? [<BatchRow key="batch" n={toCheck.length} unticked={unticked} onClick={doneAll} />, row] : [row];
      })}
      after={(struck.length > 0 || rest > 0 || did !== null || batch !== null) && (
        <>
          {struck.map((x) => (
            <TaskRow key={key(x)} x={x} now={now} group={groupOf(x.project)} prs={prs.get(key(x))} fresh={false} check={{ checked: !reopened.has(key(x)), onToggle: () => {
              const was = !reopened.has(key(x));
              setReopened((m) => {
                const n = new Set(m);
                if (was) n.add(key(x));
                else n.delete(key(x));
                return n;
              });
              offerBatch(null);
              act(x.project, was ? 'reopen' : 'done', x.w.id);
              offer(was ? null : { project: x.project, step: 'done', row: x.w });
            } }} />
          ))}
          <UndoToast did={did} works={all.filter((x) => x.project === did?.project).map((x) => x.w)} name={did?.row.name ?? ''} send={req} done={() => {
            // his Undo sent the Reopen: the row stays at its place, unchecked
            if (did !== null) setKept((m) => uncheck(m, `${did.project}/${did.row.id}`));
            offer(null);
          }} testId="app.outer.tasks.toast" bar />
          <BatchToast rows={batch} send={req} done={() => {
            // his Undo reopened them all: each stays at its place, unchecked
            if (batch !== null) setKept((m) => batch.reduce((acc, x) => uncheck(acc, key(x)), m));
            offerBatch(null);
          }} testId="app.outer.tasks.batch-toast" />
          {rest > 0 && (
            <Press look="tk-fold" data-testid="app.outer.tasks.rest" onClick={() => {
              openBoard(scope);
            }}>
              <TaskMark status="ongoing" /> {tasks.fold_rest({ ongoing: ongoing.length, done: rest - ongoing.length })}
            </Press>
          )}
        </>
      )} />
  );
}

/** The row heading To check when 2+ wait for his OK: how many, and Mark all done (one Undo). */
function BatchRow({ n, unticked, onClick }: { n: number; unticked: number; onClick: () => void }) {
  return (
    <div className="tk-batch" data-testid="app.outer.tasks.check-all">
      <span className="tk-batch-n">{tasks.to_check({ n })}</span>
      <Press look="tk-batch-act" {...tipAttrs(tasks.act_done_all_tip({ n, unticked }), undefined, 'left')} onClick={onClick} data-testid="app.outer.tasks.check-all.act">{tasks.act_done_all}</Press>
    </div>
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
