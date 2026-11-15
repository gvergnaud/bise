// The Task tab (brief "Task tab", content approved by him as drawn; designer's look rules: 384 wide):
// its status pill, name, project · agents · started; his origin (a link to his message); why;
// done-when; Needs you answered in place (its cards' options); Now (its line); Agents (live dot,
// archived greyed); Artifacts (3 newest for him, 'All n in Library →' on task:<id>); Changes (its agents' branches); a paused task's line 'this task is paused · Resume' (tasks-ux); footer Mark done
// · Pause/Resume (Reopen when done, Restart when stalled). A merged or folded one says where it went.
// Gestures go through the hub's task_act. Owner: tasks.
import { useEffect, useRef, useState } from 'react';
import { ArtChip, Dropdown, Input, Kbd, Press, Tabs, tipAttrs } from '@/ds';
import { TaskLog } from './TaskLog';
import { dayWords, glyphOf, timeOf } from '@/artifacts/model';
import { age } from '@/inbox/InboxCard';
import { inbox } from '@/proto/words_inbox.gen';
import { sentenceCase } from '@/ds/case';
import type { Entry } from '@/proto';
import { threadKey, useAgents, useArtifacts, useCards, useProjects, useStore, useThreadOrder } from '@/store';
import { openTab } from '@/app/panel/tabs';
import { presetLibraryQuery } from '@/artifacts/preset';
import { changeDoor } from '@/review/openChange';
import { useRouter } from '@/app/router';
import { MAIN } from '@/app/route';
import { useNowFor } from '@/app/clock';
import { askButtons, pickIntent } from '@/app/side3d/model';
import { tasks } from '@/proto/words_tasks.gen';
import { useWorks } from '@/store';
import { aboutOf, artsOf, crewOf, cutAtWord, doingOf, firstLine, goalsOf, isOpen, nowOf } from './model';
import { openTask, StatusMark, statusWord } from './Marks';
import { useTaskAct, useTaskReq } from './Board';
import { lineAt } from './Side';
import { markLooked, markSeen } from './seen';
import { UndoToast, useUndo } from './UndoToast';
import type { Did } from './undo';

/** His message's words by its id among a thread's entries; null when this window doesn't hold it. Pure. */
export function originText(entries: readonly (Pick<Entry, 'kind' | 'msg_id' | 'text'> | undefined)[], id: number): string | null {
  const e = entries.find((x) => x !== undefined && x.kind === 'you' && x.msg_id === id);
  const text = e?.text.trim() ?? '';
  return text === '' ? null : text;
}

/** a start's words: today's hour, else the date */
function started(ms: number, now: number): string {
  const d = new Date(ms);
  // 24 h ('14:20') today, else the day ('yesterday', 'sun 5 oct'), as the rest of the app
  const day = dayWords(ms, now);
  return day === 'today' ? timeOf(d.getTime()) : day;
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
  const now = useNowFor(() => false);
  // the short Undo after Pause, Resume, Mark done (roma idea 4)
  const [did, offer] = useUndo();
  const act = (p: string, step: 'done' | 'reopen' | 'pause' | 'resume', tid: string): void => {
    const row = works.works.find((x) => x.id === tid);
    doAct(p, step, tid);
    offer(row !== undefined && step !== 'reopen' ? ({ project: p, step, row } satisfies Did) : null);
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
  const [pane, setPane] = useState<'overview' | 'log'>('overview');
  // '+ Agent': the field open; asked: how many agents it had when he asked (its line goes when one more comes)
  const [asking, setAsking] = useState(false);
  const [asked, setAsked] = useState<{ readonly n: number; readonly at: number } | null>(null);
  const w = works.works.find((x) => x.id === id);
  // his message that started it, quoted from main's thread when this window holds it (by its id)
  const mainKey = threadKey(project, MAIN);
  const order = useThreadOrder(mainKey);
  const origin = w?.origin == null ? null : originText(order.order.map((p) => store.entry(mainKey, p)), w.origin);
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
  const names = w.agents ?? [];
  const mine = new Set(w.cards ?? []);
  const asks = cards.filter((c) => mine.has(c.id));
  const live = (name: string) => agents.find((a) => a.name === name);
  // the tasks it can merge into: the other open ones of its project
  const others = works.works.filter((o) => o.id !== w.id && isOpen(o));
  const t = `tasks.tab.${w.id}`;
  // main starts the work, it is not on the task: listed only when alone or in a turn for it
  const crew = crewOf(names, (n) => live(n)?.status === 'working');
  const said = nowOf(names.flatMap((n) => {
    const a = live(n);
    return a === undefined ? [] : [a];
  }));
  const nowLine = said ?? aboutOf(lineAt(x, now));
  const arts = artsOf(allArts, w.id, works.tombs);
  // its agents that work on a branch of their own (a worktree)
  const changes = names.flatMap((n) => {
    const a = live(n);
    return a?.branch == null || a.branch === '' ? [] : [{ name: n, branch: a.branch }];
  });
  return (
    <div className="tk-tab" data-testid={t}>
      {/* the body scrolls above the footer, never under it (tasks-ux m_64967) */}
      <div className="tk-body">
      <header className="tk-tab-head">
        <span className="tk-pill" data-st={w.status} data-testid={`${t}.status`}>
          <StatusMark status={w.status} />
          {sentenceCase(statusWord(w.status))}
        </span>
        {renaming ? (
          <Input look="tk-tab-name tk-rename" defaultValue={w.name} autoFocus aria-label={tasks.act_rename} data-testid={`${t}.rename`}
            onKeyDown={(e) => {
              if (e.key === 'Escape') setRenaming(false);
              if (e.key !== 'Enter') return;
              const name = e.currentTarget.value.trim();
              if (name !== '' && name !== w.name) req(project, { step: 'rename', id: w.id, name });
              setRenaming(false);
            }}
            onBlur={() => {
              setRenaming(false);
            }} />
        ) : (
          <h2 className="tk-tab-name">{w.name}</h2>
        )}
        <p className="tk-meta">{tasks.tab_meta({ project: x.projectName, agents: names.length === 0 ? tasks.tab_no_agents : tasks.tab_agents({ n: names.length }), when: started(w.created_ms, now) })}</p>
      </header>

      {/* roma idea 3: its Log beside what it is now */}
      <Tabs testId={`${t}.panes`} onPick={(p) => {
        setPane(p === 'log' ? 'log' : 'overview');
      }} items={[
        { id: 'overview', text: tasks.tab_overview, on: pane === 'overview' },
        { id: 'log', text: tasks.tab_log, on: pane === 'log' },
      ]} />

      {pane === 'log' ? <TaskLog project={project} id={w.id} last={w.last_ms} now={now} testId={`${t}.log`} /> : (<>

      {origin !== null && (
        // his message itself, its first line cut at a word, linked to its place in main's thread (no origin: no block)
        <Press look="tk-origin" {...tipAttrs(tasks.origin_tip)} onClick={() => {
          router.go({ screen: 'thread', project, agent: MAIN });
        }} data-testid={`${t}.origin`}>{`“${cutAtWord(firstLine(origin), 160)}”`}</Press>
      )}
      {(w.why ?? '') !== '' && <p className="tk-why">{w.why}</p>}

      {w.status === 'paused' && (
        // one Resume, the footer's (tasks-ux m_64959): the line says since when
        <p className="tk-paused" data-testid={`${t}.paused`}>
          <StatusMark status="paused" /> {tasks.paused_since({ ago: age(now - (w.paused_ms ?? now)) })}
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
                <span className="tk-check">{g.done === true ? '✓' : '○'}</span>
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
            <div key={c.id} className="tk-q" data-testid={`${t}.q.${String(c.id)}`}>
              <p className="tk-q-text">{c.question}</p>
              {/* the inbox's keys (1-9) on its options, then his own words (tasks-ux m_64959) */}
              <ol className="tk-q-opts">
                {askButtons(c).opts.map((o) => (
                  <li key={o.n}>
                    <Press look="tk-q-opt" onClick={() => store.do(pickIntent(c, o.n, null))} data-testid={`${t}.q.${String(c.id)}.${String(o.n)}`}>
                      <Kbd>{String(o.n)}</Kbd>
                      <span className="tk-q-opt-w">{o.label}</span>
                    </Press>
                  </li>
                ))}
              </ol>
              <Input look="tk-q-type" placeholder={askButtons(c).opts.length > 0 ? inbox.hint_or_answer : tasks.q_type} aria-label={c.question} data-testid={`${t}.q.${String(c.id)}.type`}
                onKeyDown={(e) => {
                  const words = e.currentTarget.value.trim();
                  if (e.key === 'Enter' && words !== '') store.do({ i: 'answer', project: c.project, card: c.id, reply: words });
                  else if (e.key >= '1' && e.key <= '9' && e.currentTarget.value === '' && askButtons(c).opts.some((o) => String(o.n) === e.key)) {
                    e.preventDefault();
                    store.do(pickIntent(c, Number(e.key), null));
                  }
                }} />
            </div>
          ))}
        </section>
      )}

      {nowLine !== '' && (
        // the newest report or status note of its agents; none said: what the status is about, past the
        // pill's own word ('no agent yet', 'quiet since 14:20'): never a bare pill (tasks-ux m_64959, m_65131)
        <section className="tk-sec">
          <h3 className="tk-h">{tasks.sec_now}</h3>
          <p className="tk-why" data-testid={`${t}.now`}>{nowLine}</p>
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
              const state = a === undefined ? tasks.agent_archived : on ? tasks.agent_live : a.waits > 0 ? tasks.agent_waits : tasks.agent_idle;
              const doing = a === undefined ? '' : doingOf(a);
              return (
                <li key={n} role="link" tabIndex={0} data-archived={a === undefined || undefined} onClick={() => {
                  router.go({ screen: 'thread', project, agent: n });
                }} onKeyDown={(e) => {
                  if (e.key === 'Enter') router.go({ screen: 'thread', project, agent: n });
                }} data-testid={`${t}.agent.${n}`}>
                  <span className="tk-live" data-on={on || undefined} />
                  <span className="tk-agent">{n}</span>
                  {doing !== '' && <span className="tk-agent-doing">· {doing}</span>}
                  <span className="tk-agent-state">{state}</span>
                </li>
              );
            })}
          </ul>
        </section>
      )}

      {arts.all.length > 0 && (
        // the 3 newest for him, then the Library on this task (`task:<id>`, brief "Task tab")
        <section className="tk-sec" data-testid={`${t}.arts`}>
          <h3 className="tk-h">{tasks.sec_artifacts} <span className="tk-count">{arts.all.length}</span></h3>
          <ul className="tk-rows">
            {arts.mine.slice(0, 3).map((a) => (
              <li key={a.id}>
                {/* an artifact is a pink chip, everywhere (taste 13); its kind at the right, text-3 */}
                <ArtChip title={a.title} glyph={glyphOf(a.kind)} keep={24} onClick={() => {
                  openTab({ kind: 'artifact', project, id: a.id });
                }} testId={`${t}.art.${a.id}`} />
                <span className="tk-agent-state">{a.kind}</span>
              </li>
            ))}
          </ul>
          <Press look="tk-more" onClick={() => {
            presetLibraryQuery(`task:${w.id}`);
            router.go({ screen: 'artifacts', project });
          }} data-testid={`${t}.arts.all`}>{tasks.arts_all({ n: arts.all.length })}</Press>
        </section>
      )}

      {changes.length > 0 && (
        // its agents' branches; a PR opens instead when one is open (review/openChange changeDoor)
        <section className="tk-sec" data-testid={`${t}.changes`}>
          <h3 className="tk-h">{tasks.sec_changes}</h3>
          <ul className="tk-rows">
            {changes.map((a) => (
              <li key={a.name}>
                <Press look="tk-row" {...tipAttrs(tasks.change_tip)} onClick={() => {
                  const door = changeDoor(project, { agent: a.name }, { agents, places: store.places(project), worktrees: store.worktrees(project) });
                  openTab(door.tab);
                }} data-testid={`${t}.change.${a.name}`}>
                  <span className="tk-row-name">{a.branch}</span>
                  <span className="tk-agent-state">{a.name}</span>
                </Press>
              </li>
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
          <Input look="tk-rename" autoFocus placeholder={tasks.agent_ask} aria-label={tasks.agent_tip} data-testid={`${t}.agent.ask`}
            onKeyDown={(e) => {
              if (e.key === 'Escape') setAsking(false);
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
      <UndoToast did={did} works={works.works} name={did?.row.name ?? w.name} send={req} done={() => {
        offer(null);
      }} testId={`${t}.toast`} />
      <footer className="tk-foot">
        {w.status === 'stalled' && (
          <Press look="b-btn sm pri" onClick={() => {
            act(project, 'resume', w.id);
          }} data-testid={`${t}.restart`}>{tasks.act_restart}</Press>
        )}
        {w.status === 'done' ? (
          <Press look="b-btn sm" onClick={() => {
            act(project, 'reopen', w.id);
          }} data-testid={`${t}.reopen`}>{tasks.act_reopen}</Press>
        ) : (
          w.status !== 'ongoing' && (
            <Press look="b-btn sm" onClick={() => {
              act(project, 'done', w.id);
            }} data-testid={`${t}.done`}>{tasks.act_done}</Press>
          )
        )}
        {w.status === 'paused' ? (
          <Press look="b-btn sm" onClick={() => {
            act(project, 'resume', w.id);
          }} data-testid={`${t}.resume`}>{tasks.act_resume}</Press>
        ) : (
          // no agent on it: Pause would pause nothing (tasks-ux m_65124)
          w.status !== 'done' && names.length > 0 && (
            <Press look="b-btn sm" onClick={() => {
              act(project, 'pause', w.id);
            }} data-testid={`${t}.pause`}>{tasks.act_pause}</Press>
          )
        )}
        {w.status !== 'done' && (
          <Press look="b-btn sm" {...tipAttrs(tasks.agent_tip)} onClick={() => {
            setAsking(true);
          }} data-testid={`${t}.agent.add`}>{tasks.act_agent}</Press>
        )}
        <Dropdown look="pop" label="⋯" name={tasks.act_more} tip={tasks.act_more} testId={`${t}.more`}
          groups={[
            { id: 'acts', rows: [{ id: 'rename', name: tasks.act_rename, testId: `${t}.more.rename` }] },
            {
              id: 'merge',
              head: tasks.merge_head,
              rows: others.length === 0
                ? [{ id: 'none', name: tasks.merge_none, disabled: true }]
                : others.map((o) => ({ id: `merge:${o.id}`, name: o.name, glyph: <StatusMark status={o.status} />, testId: `${t}.more.merge.${o.id}` })),
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
