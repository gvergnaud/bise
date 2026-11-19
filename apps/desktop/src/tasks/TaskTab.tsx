// The Task tab (brief "Task tab", content approved by him as drawn; designer's look rules: 384 wide):
// its status pill, name, project · agents · started; his origin (a link to his message); why;
// done-when; Needs you answered in place (its cards' options); Now (its line); Agents (live dot,
// archived greyed); Artifacts (3 newest for him, 'All n in Library →' on task:<id>, its PRs never); Changes (its PRs
// with GitHub's marks, then its agents' branches with no PR, then their lands: his pick on page tasks-fresh); a paused task's line 'this task is paused · Resume' (tasks-ux); footer Mark done
// · Pause/Resume (Reopen when done, Restart when stalled). A merged or folded one says where it went.
// Gestures go through the hub's task_act. Owner: tasks.
import { useEffect, useRef, useState } from 'react';
import { AppIcon, ArtChip, Dropdown, Field, IconButton, Input, PickerPop, PrReviewIcon, PrStateIcon, Press, Seg, tipAttrs, WordCut } from '@/ds';
import { atOf, MenuAnchor, type MenuAt } from '@/artifacts/Menus';
import { TaskLog } from './TaskLog';
import { dayWords, glyphOf, timeOf } from '@/artifacts/model';
import { sentenceCase } from '@/ds/case';
import type { Entry } from '@/proto';
import { threadKey, useAgents, useArtifacts, useCards, useProjects, useStore, useTaskLog, useThreadOrder } from '@/store';
import { openTab } from '@/app/panel/tabs';
import { presetLibraryQuery } from '@/artifacts/preset';
import { changeDoor } from '@/review/openChange';
import { openPrOf } from '@/review/model';
import { forgeWord } from '@/review/prwords';
import { PartImageChip } from '@/thread/ImageGone';
import { partImageWords } from '@/thread/YouBody';
import { chipName, storeImage } from '@/thread/parts';
import { useRouter } from '@/app/router';
import { MAIN } from '@/app/route';
import { useNowFor } from '@/app/clock';
import { signOf } from '@/app/side';
import { barSign } from '@/app/barrows';
import { tasks } from '@/proto/words_tasks.gen';
import { useWorks } from '@/store';
import { aboutOf, agentLineOf, agentStateOf, artsOf, crewOf, goalsOf, isOpen, nowOfTask, originOf, prsOf, reportsOf, soloOn, type Origin, type TaskPr } from './model';
import { openTask, pausedWords, TaskMark, statusWord } from './Marks';
import { useTaskAct, useTaskReq } from './Board';
import { clock, lineAt } from './Side';
import { markLooked, markSeen } from './seen';
import { UndoToast, useUndo } from './UndoToast';
import { flipTo, shownStatus, type Did, type Flip } from './undo';
import { ArtMoveMore } from './MoveMenus';
import { Question } from './Question';

/** His message by its id among a thread's entries, as the tab quotes it (his words from its parts,
 *  his images: originOf); null when this window doesn't hold it or he wrote no words. Pure. */
export function originIn(entries: readonly (Pick<Entry, 'kind' | 'msg_id' | 'text' | 'parts'> | undefined)[], id: number): Origin | null {
  const e = entries.find((x) => x !== undefined && x.kind === 'you' && x.msg_id === id);
  return e === undefined ? null : originOf(e);
}

/** an agent's state on this task in words (agentStateOf) */
const STATE_WORD = { archived: tasks.agent_archived, working: tasks.agent_live, waits: tasks.agent_waits, idle: tasks.agent_idle } as const;

/** a PR row's state words: 'open · changes requested', 'merged 13:17', and 'as of 14:20' when the
 *  forge answers late (its last known state); the review rides with an open one (designer m_66712) */
export function prWords(p: TaskPr, now: number): string {
  const when = p.at_ms === null ? '' : started(p.at_ms, now);
  const state = p.state === 'merged' ? (when === '' ? tasks.pr_merged : tasks.pr_merged_at({ when })) : p.state === 'closed' ? tasks.pr_closed : p.state === 'draft' ? tasks.pr_draft : tasks.pr_open;
  const review = p.state === 'open' || p.state === 'draft' ? (p.review === 'changes' ? tasks.pr_changes : p.review === 'approved' ? tasks.pr_approved : '') : '';
  const late = p.stale && when !== '' && p.state !== 'merged' ? tasks.pr_as_of({ when }) : '';
  return [state, review, late].filter((x) => x !== '').join(' · ');
}

/** a start's words: today's hour, else the date */
function started(ms: number, now: number): string {
  const d = new Date(ms);
  // 24 h ('14:20') today, else the day ('yesterday', 'sun 5 oct'), as the rest of the app
  const day = dayWords(ms, now);
  return day === 'today' ? timeOf(d.getTime()) : day;
}

/** A Done when item's mark: a ring, or TaskMark's done ✓; the ✓ draws once when it flips while
 *  the tab is open, never on load (tasks-shine batch 5, designer m_67174). */
function GoalCheck({ done, testId }: { done: boolean; testId: string }) {
  const [was, setWas] = useState(done);
  const [drew, setDrew] = useState(false);
  if (done !== was) {
    setWas(done);
    setDrew(done);
  }
  return (
    <span className="tk-check" data-drew={drew || undefined} data-testid={testId}>
      <svg viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
        {done ? <path d="M3.25 6.25l1.9 1.9 3.6-4.1" pathLength={1} /> : <circle cx="6" cy="6" r="3.5" />}
      </svg>
    </span>
  );
}

export function TaskTab({ project, id }: { project: string; id: string }) {
  const works = useWorks(project);
  const projects = useProjects();
  const agents = useAgents(project);
  const cards = useCards(project);
  const allArts = useArtifacts(project);
  const store = useStore();
  const router = useRouter();
  const doAct = useTaskAct();
  const req = useTaskReq();
  // his Pause, Resume, Mark done shown at once: the pill and the footer flip with the toast (tasks-ux
  // m_65598); the clock to the second while a flip waits for the hub's row
  const [flip, setFlip] = useState<Flip | null>(null);
  const now = useNowFor(() => flip !== null);
  // the short Undo after Pause, Resume, Mark done (roma idea 4)
  const [did, offer] = useUndo();
  const act = (p: string, step: 'done' | 'reopen' | 'pause' | 'resume' | 'restart', tid: string): void => {
    const row = works.works.find((x) => x.id === tid);
    const to = flipTo(step);
    setFlip(row !== undefined && to !== null ? { id: tid, from: row.status, to, at: Date.now() } : null);
    doAct(p, step, tid);
    // Reopen and Restart have nothing to put back: no Undo
    offer(row !== undefined && step !== 'reopen' && step !== 'restart' ? ({ project: p, step, row } satisfies Did) : null);
  };
  // an agent row's ⋯ › Not on this task (tasks-ux m_66626): his gesture, the agent leaves the
  // list until it acts for the task again; its Undo puts it back (WRemoveAgent / WAddAgent)
  const [agentMenu, setAgentMenu] = useState<{ agent: string; at: MenuAt } | null>(null);
  const unlink = (agent: string): void => {
    const row = works.works.find((x) => x.id === id);
    req(project, { step: 'unlink', id, agent });
    offer(row === undefined ? null : ({ project, step: 'unlink', row, agent } satisfies Did));
  };
  // he sees this task while its tab is open: seen when it opens, as it changes, and when it goes
  // (the dot, news.ts)
  const seenRow = works.works.find((x) => x.id === id);
  const seenRef = useRef(seenRow);
  useEffect(() => {
    seenRef.current = seenRow;
    if (seenRow !== undefined) markSeen([{ project, projectName: project, w: seenRow }]);
  }, [project, seenRow]);
  useEffect(() => {
    markLooked();
  }, [project, id]);
  useEffect(() => () => {
    const r = seenRef.current;
    if (r !== undefined) markSeen([{ project, projectName: project, w: r }]);
  }, [project]);
  const [renaming, setRenaming] = useState(false);
  // the rename field takes the focus without scrolling the tab (designer m_65980): typing scrolls
  // only its own text, never the body (tasks-ui-qa m_65973)
  const renameField = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (renaming) renameField.current?.focus({ preventScroll: true });
  }, [renaming]);
  const [pane, setPane] = useState<'overview' | 'log'>('overview');
  // '+ Agent': the field open; asked: how many agents it had when he asked (its line goes when one more comes)
  const [asking, setAsking] = useState(false);
  const [asked, setAsked] = useState<{ readonly n: number; readonly at: number } | null>(null);
  const hubRow = works.works.find((x) => x.id === id);
  const w = hubRow === undefined ? undefined : { ...hubRow, status: shownStatus(hubRow, flip, now) };
  // his message that started it, quoted from main's thread when this window holds it (by its id)
  const mainKey = threadKey(project, MAIN);
  const order = useThreadOrder(mainKey);
  const origin = w?.origin == null ? null : originIn(order.order.map((p) => store.entry(mainKey, p)), w.origin);
  // its Log, asked as the Log pane does: what its agents reported on THIS task (Now, their lines)
  const log = useTaskLog(project, id);
  const lastMs = hubRow?.last_ms ?? 0;
  useEffect(() => {
    store.askTaskLog(project, id);
  }, [store, project, id, lastMs]);
  const reports = reportsOf(log?.lines ?? []);
  // after his '+ Agent' words: main's reply in its thread (an entry of main's after he asked)
  const replied = asked !== null && order.order.some((p) => {
    const e = store.entry(mainKey, p);
    return e !== undefined && e.kind === 'agent' && e.at_ms > asked.at;
  });
  if (w === undefined) {
    const tomb = works.tombs.find((x) => x.id === id);
    const into = tomb?.merged_into ?? null;
    const target = into === null ? undefined : (works.works.find((x) => x.id === into) ?? works.tombs.find((x) => x.id === into));
    return (
      <div className="tk-tab" data-testid="tasks.tab.gone">
        <p className="tk-gone">
          {target !== undefined ? (
            <Press look="tk-origin" onClick={() => {
              openTask(project, target.id);
            }}>{tasks.tab_merged({ name: target.name })}</Press>
          ) : tasks.tab_gone}
        </p>
      </div>
    );
  }
  const x = { project, projectName: projects.find((p) => p.project === project)?.name ?? project, w };
  const goals = goalsOf(w);
  // Mark done turns primary once every Done when item is ✓, never beside a stalled task's Restart
  const doneReady = goals !== null && goals.done === goals.all && w.status !== 'stalled';
  const names = w.agents ?? [];
  const mine = new Set(w.cards ?? []);
  const asks = cards.filter((c) => mine.has(c.id));
  const live = (name: string) => agents.find((a) => a.name === name);
  // the tasks it can merge into: the other open ones of its project
  const others = works.works.filter((o) => o.id !== w.id && isOpen(o));
  const t = `tasks.tab.${w.id}`;
  // main starts the work, it is not on the task: listed only when alone or in a turn for it
  const crew = crewOf(names, (n) => live(n)?.status === 'working');
  // what an agent says here is about THIS task: its reports on it, its note only when it serves no other
  const solo = soloOn(works.works, w.id, names);
  const said = nowOfTask(reports, names.flatMap((n) => {
    const a = live(n);
    return a === undefined || !solo.has(n) ? [] : [a];
  }));
  // paused: its line under the meta says the pause; Now keeps what was happening, never a bare 'until …' (tasks-ux m_65804)
  // done: the pill says it, never a bare 'yesterday' (tasks-ui-qa m_65864 T19)
  // Needs you: its block says it, never '1 question' again under it (tasks-ux m_65871)
  // working: the pill and its Agents rows say who, never a bare name as Now (tasks-ui-qa m_65946 N3)
  const nowLine = said ?? (w.status === 'paused' || w.status === 'done' || w.status === 'needs_you' || w.status === 'working' ? '' : aboutOf(lineAt(x, now)));
  const arts = artsOf(allArts, w.id, works.tombs);
  // its agents that work on a branch of their own (a worktree): the branch, its PR, its newest land
  const places = store.places(project);
  const lands = (log?.lines ?? []).filter((l) => l.kind === 'land' && (l.sha ?? '') !== '');
  // his pick on page tasks-fresh (designer m_66722): Changes is the PR list. Its PRs first (the PR
  // artifacts that serve it, open, merged, then closed), then its agents' branches with no PR of
  // its own here, then the lands of its agents with no branch: nothing twice
  const prs = prsOf(allArts, w.id, works.tombs);
  const prArts = new Map(allArts.filter((a) => a.pr != null).map((a) => [a.id, a]));
  const listed = new Set(prs.map((p) => p.tag));
  const changes = names.flatMap((n) => {
    const a = live(n);
    if (a?.branch == null || a.branch === '') return [];
    const pr = openPrOf(n, agents, places);
    if (pr !== null && listed.has(`#${String(pr.number)}`)) return [];
    const sha = lands.find((l) => l.who === n)?.sha ?? '';
    return [{ name: n, branch: a.branch, pr: pr === null ? tasks.change_no_pr : tasks.change_pr({ n: pr.number, state: pr.state }), sha: sha.slice(0, 9) }];
  });
  const branched = new Set(names.filter((n) => (live(n)?.branch ?? '') !== ''));
  const landed = names.flatMap((n) => {
    const sha = branched.has(n) ? '' : (lands.find((l) => l.who === n)?.sha ?? '');
    return sha === '' ? [] : [{ name: n, sha: sha.slice(0, 9) }];
  });
  return (
    <div className="tk-tab" data-testid={t}>
      {/* its head (pill, title, meta, Overview | Log) out of the body's scroll box: the rename field
          lives in a box that never scrolls, so the browser's caret reveal has nothing to move
          (designer m_65990, tasks-ui-qa m_65985), and the tabs stay in reach on a long task */}
      <div className="tk-top">
      <header className="tk-tab-head">
        <span className="tk-pill" data-st={w.status} data-testid={`${t}.status`}>
          <TaskMark status={w.status} />
          {sentenceCase(statusWord(w.status))}
        </span>
        {renaming ? (
          <Field ref={renameField} data-esc-own="" defaultValue={w.name} aria-label={tasks.act_rename} testId={`${t}.rename`}
            onKeyDown={(e) => {
              // esc closes the field only, never the sheet around the tab: data-esc-own tells app/escape.ts
              // the field takes it first (tasks-ui-qa m_65864, m_65884 T20)
              if (e.key === 'Escape') {
                e.stopPropagation();
                setRenaming(false);
              }
              if (e.key !== 'Enter') return;
              const name = e.currentTarget.value.trim();
              if (name !== '' && name !== w.name) req(project, { step: 'rename', id: w.id, name });
              setRenaming(false);
            }}
            onBlur={() => {
              setRenaming(false);
            }} />
        ) : (
          // two lines at most, the whole name in its tip (designer m_65780)
          <h2 className="tk-tab-name" {...tipAttrs(w.name)}>{w.name}</h2>
        )}
        {/* the agents it lists, never main hidden by crewOf (tasks-ux m_65775) */}
        <p className="tk-meta">{tasks.tab_meta({ project: x.projectName, agents: crew.length === 0 ? tasks.tab_no_agents : tasks.tab_agents({ n: crew.length }), when: started(w.created_ms, now) })}</p>
      </header>

      {/* roma idea 3: its Log beside what it is now; the tab's own sections: the ds underline Seg (designer m_65783) */}
      <Seg testId={`${t}.panes`} on={pane === 'log' ? 1 : 0} onPick={(i) => {
        setPane(i === 1 ? 'log' : 'overview');
      }} items={[tasks.tab_overview, tasks.tab_log]} />
      </div>

      {/* the body scrolls above the footer, never under it (tasks-ux m_64967) */}
      <div className="tk-body">
      {pane === 'log' ? <TaskLog project={project} id={w.id} last={w.last_ms} now={now} testId={`${t}.log`} /> : (<>

      {origin !== null && (
        // his message from its parts (never the model's tags; his m_65766): his images as the
        // thread's image chips, then his first line, linked to its place in main's thread
        <div className="tk-origin" data-testid={`${t}.origin`}>
          {origin.images.map((img, i) => (
            <PartImageChip key={`${img.path}#${String(i)}`} path={img.path} store={storeImage(img.b64, img.mime)} lit={false} on={{}} testId={`${t}.origin.image.${String(i + 1)}`}>
              {/* the thread's own chip words: its file name, else 'image 1', never '[Image #1]' (designer m_65870) */}
              {partImageWords(chipName(img.name), i + 1)}
            </PartImageChip>
          ))}
          <Press look="tk-origin-words" {...tipAttrs(tasks.origin_tip)} onClick={() => {
            router.go({ screen: 'thread', project, agent: MAIN });
          }} data-testid={`${t}.origin.words`}>{`“${origin.words}”`}</Press>
        </div>
      )}
      {(w.why ?? '') !== '' && <p className="tk-why">{w.why}</p>}

      {w.status === 'paused' && (
        // one Resume, the footer's (tasks-ux m_64959): the line says since when
        <p className="tk-paused" data-testid={`${t}.paused`}>
          {/* until a time when it resumes by itself, else since when (designer m_65808: text-3) */}
          <TaskMark status="paused" /> {w.until_ms != null && w.until_ms > now ? tasks.paused_line_until({ at: clock(w.until_ms, now) }) : pausedWords(now - (w.paused_ms ?? now))}
        </p>
      )}

      {goals !== null && (
        <section className="tk-sec">
          <h3 className="tk-h">
            {tasks.sec_done_when} <span className="tk-count">{tasks.goals_of(goals)}</span>
          </h3>
          <ul className="tk-checks">
            {(w.goals ?? []).map((g, i) => (
              <li key={`${String(i)}.${g.text}`} data-done={g.done === true || undefined} {...((g.by ?? '') !== '' ? tipAttrs(tasks.goal_by({ by: g.by ?? '' })) : {})}>
                <GoalCheck done={g.done === true} testId={`${t}.goal.${String(i)}.check`} />
                {g.text}
              </li>
            ))}
          </ul>
        </section>
      )}

      {asks.length > 0 && (
        <section className="tk-sec">
          <h3 className="tk-h">{tasks.sec_needs}</h3>
          {asks.map((c) => (
            <Question key={c.id} c={c} project={project} testId={`${t}.q.${String(c.id)}`} />
          ))}
        </section>
      )}

      {nowLine !== '' && (
        // its agents' newest report on it, first sentence, else a note of an agent on it alone; none
        // said: what the status is about, past the pill's own word ('no agent yet', 'quiet since 14:20'):
        // never a bare pill (tasks-ux m_64959, m_65131, m_65775); two lines, the whole in its tip
        <section className="tk-sec">
          <h3 className="tk-h">{tasks.sec_now}</h3>
          <p className="tk-now" {...tipAttrs(nowLine)} data-testid={`${t}.now`}>{nowLine}</p>
        </section>
      )}

      {crew.length === 0 && (
        // none yet (a task main named before it started any): how one comes, never a blank under the meta
        <section className="tk-sec" data-testid={`${t}.agents.none`}>
          <h3 className="tk-h">{tasks.sec_agents}</h3>
          <p className="tk-none">{tasks.agents_none}</p>
        </section>
      )}

      {crew.length > 0 && (
        <section className="tk-sec">
          <h3 className="tk-h">{tasks.sec_agents}</h3>
          <ul className="tk-agents">
            {crew.map((n) => {
              const a = live(n);
              const on = a?.status === 'working';
              // its state and line on THIS task (tasks-ux m_65775): waits for him only on a card of this task
              const state = STATE_WORD[agentStateOf(a, asks.some((c) => c.agent === n))];
              const doing = agentLineOf(n, reports, a, solo.has(n));
              // an archived one (no live row) draws the bar's at-rest sign
              const face = a === undefined ? barSign({ g: '–', tone: 'dim', end: '', endTone: 'dim', count: 0, title: '' }) : barSign(signOf(a, now));
              return (
                <li key={n} role="link" tabIndex={0} data-archived={a === undefined || undefined} onClick={() => {
                  router.go({ screen: 'thread', project, agent: n });
                }} onKeyDown={(e) => {
                  // the row's own Enter, never its ⋯'s
                  if (e.key === 'Enter' && e.target === e.currentTarget) router.go({ screen: 'thread', project, agent: n });
                }} data-testid={`${t}.agent.${n}`}>
                  {/* one line (tasks-shine batch 4, designer m_66679): its face, the right bar's own sign
                      (app/barrows barSign) · name · its line (cut, whole in its tip) · live dot + state */}
                  <span className={`s3-arow-g ${face.tone}`} aria-hidden="true" data-testid={`${t}.agent.${n}.face`}>{face.g}</span>
                  <span className="tk-agent">{n}</span>
                  <span className="tk-agent-doing" {...(doing === '' ? {} : tipAttrs(doing))}>{doing}</span>
                  <span className="tk-agent-end">
                    <span className="tk-live" data-on={on || undefined} />
                    <span className="tk-agent-state">{state}</span>
                  </span>
                  {/* its ⋯ on hover or focus, as the artifact rows' (tasks-ux m_66626) */}
                  <IconButton size="row" label={tasks.agent_more} onClick={(e) => {
                    e.stopPropagation();
                    setAgentMenu({ agent: n, at: atOf(e.currentTarget) });
                  }} testId={`${t}.agent.${n}.more`}><AppIcon name="more" size={16} /></IconButton>
                </li>
              );
            })}
          </ul>
        </section>
      )}

      {arts.all.length > 0 && (
        // the 3 newest for him in one wrapping row (the proto's), the Library on this task at the
        // head's right (`task:<id>`, brief "Task tab"; designer m_65808)
        <section className="tk-sec" data-testid={`${t}.arts`}>
          <h3 className="tk-h">
            {tasks.sec_artifacts}
            <Press look="tk-more tk-h-end" onClick={() => {
              presetLibraryQuery(`task:${w.id}`);
              router.go({ screen: 'artifacts', project });
            }} data-testid={`${t}.arts.all`}>{tasks.arts_all({ n: arts.all.length })}</Press>
          </h3>
          <ul className="tk-arts">
            {arts.mine.slice(0, 3).map((a) => (
              <li key={a.id}>
                {/* an artifact is a pink chip, everywhere (taste 13); its glyph says its kind */}
                <ArtChip title={a.title} glyph={glyphOf(a.kind)} keep={24} tip={`${a.title} · ${a.kind}`} onClick={() => {
                  openTab({ kind: 'artifact', project, id: a.id });
                }} testId={`${t}.art.${a.id}`} />
                {/* Move to task… (F9, tasks-ux m_65591): its ⋯ on the chip's hover */}
                <ArtMoveMore project={project} a={a} testId={`${t}.art.${a.id}.more`} />
              </li>
            ))}
          </ul>
        </section>
      )}

      {prs.length + changes.length + landed.length > 0 && (
        // its PRs, then its agents' branches (a PR opens instead when one is open: review/openChange
        // changeDoor), then its lands (designer m_66722)
        <section className="tk-sec" data-testid={`${t}.changes`}>
          <h3 className="tk-h">{tasks.sec_changes}</h3>
          <ul className="tk-rows">
            {prs.map((p) => {
              const a = prArts.get(p.id);
              return (
                <li key={p.id} data-st={p.state} data-testid={`${t}.pr.${p.id}`}>
                  <Press look="tk-row tk-pr" {...tipAttrs(p.title)} onClick={() => {
                    openTab({ kind: 'artifact', project, id: p.id });
                  }} data-testid={`${t}.pr.${p.id}.open`}>
                    {/* GitHub's icon in its colour · '#924' text-3 · the title cut at a word · its state words + review (designer m_66712) */}
                    <PrStateIcon state={p.state} testId={`${t}.pr.${p.id}.icon`} />
                    <span className="tk-pr-tag">{p.tag}</span>
                    <WordCut className="tk-row-name tk-pr-title" text={p.title} />
                    <span className="tk-pr-state" data-testid={`${t}.pr.${p.id}.state`}>{prWords(p, now)}</span>
                    <PrReviewIcon review={p.review} testId={`${t}.pr.${p.id}.review`} />
                  </Press>
                  {/* Move to task… (tasks-ux m_65591): its ⋯ on the row's hover */}
                  {a !== undefined && <ArtMoveMore project={project} a={a} out={{ url: a.url, forge: forgeWord(a.pr?.status?.provider) }} testId={`${t}.pr.${p.id}.more`} />}
                </li>
              );
            })}
            {changes.map((a) => (
              <li key={a.name}>
                <Press look="tk-row" {...tipAttrs(tasks.change_tip)} onClick={() => {
                  const door = changeDoor(project, { agent: a.name }, { agents, places, worktrees: store.worktrees(project) });
                  openTab(door.tab);
                }} data-testid={`${t}.change.${a.name}`}>
                  {/* the branch in mono · its PR or 'no PR yet' · its newest land (designer m_65783) */}
                  <span className="tk-row-name tk-mono">{a.branch}</span>
                  <span className="tk-change-pr">· {a.pr}</span>
                  {a.sha !== '' && <span className="tk-change-sha tk-mono">· {a.sha}</span>}
                </Press>
              </li>
            ))}
            {landed.map((l) => (
              // a land of an agent with no branch: its sha, mono, text-3
              <li key={`land.${l.name}`} className="tk-landed tk-mono" data-testid={`${t}.landed.${l.name}`}>{l.sha} · {l.name}</li>
            ))}
          </ul>
        </section>
      )}

      {asked !== null && names.length <= asked.n && (
        // main answered instead of spawning (it asks him something): the line says so and links to its reply (tasks-ux m_64875)
        replied ? (
          <Press look="tk-more" onClick={() => {
            router.go({ screen: 'thread', project, agent: MAIN });
          }} data-testid={`${t}.answered`}>{tasks.agent_answered}</Press>
        ) : (
          <p className="tk-meta" data-testid={`${t}.asked`}>{tasks.agent_asked}</p>
        )
      )}
      </>)}

      </div>

      {asking ? (
        // '+ Agent' (tasks-ux m_64875): his words for main in place of the footer; esc closes, an empty ⏎ does nothing
        <footer className="tk-foot">
          <Input data-esc-own="" look="tk-rename" autoFocus placeholder={tasks.agent_ask} aria-label={tasks.agent_tip} data-testid={`${t}.agent.ask`}
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                e.stopPropagation();
                setAsking(false);
              }
              if (e.key !== 'Enter') return;
              const words = e.currentTarget.value.trim();
              if (words === '') return;
              // his message to main, carrying the task (architect m_64887): an unknown task refuses it, his row stays
              store.send({ project, agent: MAIN, text: tasks.agent_lead({ name: w.name, words }), works: [w.id] });
              setAsked({ n: names.length, at: Date.now() });
              setAsking(false);
            }} />
        </footer>
      ) : (
      <>
      {agentMenu !== null && (
        <AgentMenu at={agentMenu.at} onUnlink={() => {
          unlink(agentMenu.agent);
        }} onClose={() => {
          setAgentMenu(null);
        }} testId={`${t}.agent.menu`} />
      )}
      <UndoToast did={did} works={works.works} name={did?.row.name ?? w.name} send={req} done={() => {
        offer(null);
      }} testId={`${t}.toast`} />
      <footer className="tk-foot">
        {w.status === 'stalled' && (
          <Press look="b-btn sm pri" onClick={() => {
            act(project, 'restart', w.id);
          }} data-testid={`${t}.restart`}>{tasks.act_restart}</Press>
        )}
        {/* a calmer footer (tasks-shine batch 4, designer m_66679): Pause and + Agent quiet (ghost),
            Mark done at the right before ⋯, primary only once every Done when item is ✓ and no
            Restart shows: one primary at a time */}
        {w.status === 'paused' ? (
          <Press look="b-btn sm" onClick={() => {
            act(project, 'resume', w.id);
          }} data-testid={`${t}.resume`}>{tasks.act_resume}</Press>
        ) : (
          // no agent on it: Pause would pause nothing (tasks-ux m_65124)
          w.status !== 'done' && names.length > 0 && (
            <Press look="b-btn sm ghost" onClick={() => {
              act(project, 'pause', w.id);
            }} data-testid={`${t}.pause`}>{tasks.act_pause}</Press>
          )
        )}
        {w.status !== 'done' && (
          <Press look="b-btn sm ghost" {...tipAttrs(tasks.agent_tip)} onClick={() => {
            setAsking(true);
          }} data-testid={`${t}.agent.add`}>{tasks.act_agent}</Press>
        )}
        {w.status === 'done' ? (
          <Press look="b-btn sm tk-foot-end" onClick={() => {
            act(project, 'reopen', w.id);
          }} data-testid={`${t}.reopen`}>{tasks.act_reopen}</Press>
        ) : (
          w.status !== 'ongoing' && (
            <Press look={doneReady ? 'b-btn sm pri tk-foot-end' : 'b-btn sm tk-foot-end'} onClick={() => {
              act(project, 'done', w.id);
            }} data-testid={`${t}.done`}>{tasks.act_done}</Press>
          )
        )}
        <Dropdown look="more" extra="tk-foot-more" label="⋯" name={tasks.act_more} tip={tasks.act_more_move} testId={`${t}.more`}
          groups={[
            { id: 'acts', rows: [{ id: 'rename', name: tasks.act_rename, testId: `${t}.more.rename` }] },
            {
              id: 'merge',
              head: tasks.merge_head,
              rows: others.length === 0
                ? [{ id: 'none', name: tasks.merge_none, disabled: true }]
                : others.map((o) => ({ id: `merge:${o.id}`, name: o.name, glyph: <TaskMark status={o.status} />, testId: `${t}.more.merge.${o.id}` })),
            },
          ]}
          onPick={(pick) => {
            if (pick === 'rename') setRenaming(true);
            else if (pick.startsWith('merge:')) req(project, { step: 'merge', id: w.id, into: pick.slice('merge:'.length) });
          }} />
      </footer>
      </>
      )}
    </div>
  );
}

/** An agent row's menu: Not on this task (the ds PickerPop, from its ⋯). */
function AgentMenu({ at, onUnlink, onClose, testId }: { at: MenuAt; onUnlink: () => void; onClose: () => void; testId: string }) {
  const anchor = useRef<HTMLSpanElement>(null);
  return (
    <>
      <MenuAnchor at={at} anchor={anchor} />
      <PickerPop anchor={anchor} align="start" groups={[{ id: 'agent', rows: [{ id: 'unlink', name: tasks.agent_unlink, testId: `${testId}.unlink` }] }]}
        onClose={onClose} onPick={() => {
          onClose();
          onUnlink();
        }} testId={testId} />
    </>
  );
}
