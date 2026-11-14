// The Task tab (brief "Task tab", content approved by him as drawn; designer's look rules: 384 wide):
// its status pill, name, project · agents · started; his origin (a link to his message); why;
// done-when; Needs you answered in place (its cards' options); Now (its line); Agents (live dot,
// archived greyed); Artifacts (3 newest for him, 'All n in Library →' on task:<id>); Changes (its agents' branches); a paused task's line 'this task is paused · Resume' (tasks-ux); footer Mark done
// · Pause/Resume (Reopen when done, Restart when stalled). A merged or folded one says where it went.
// Gestures go through the hub's task_act. Owner: tasks.
import { useState } from 'react';
import { Dropdown, Input, Press, tipAttrs } from '@/ds';
import { sentenceCase } from '@/ds/case';
import type { Entry } from '@/proto';
import { threadKey, useAgents, useArtifacts, useCards, useProjects, useStore, useThreadOrder } from '@/store';
import { openTab } from '@/app/panel/tabs';
import { presetLibraryQuery } from '@/artifacts/preset';
import { changeDoor } from '@/review/openChange';
import { useRouter } from '@/app/router';
import { MAIN } from '@/app/route';
import { useNowFor } from '@/app/clock';
import { OptButton } from '@/app/side3d/FlyoutAgents';
import { askButtons, pickIntent } from '@/app/side3d/model';
import { tasks } from '@/proto/words_tasks.gen';
import { useWorks } from '@/store';
import { artsOf, cutAtWord, goalsOf, isOpen } from './model';
import { openTask, StatusMark, statusWord } from './Marks';
import { lineAt } from './Side';
import { useTaskAct, useTaskReq } from './Board';

/** His message's words by its id among a thread's entries; null when this window doesn't hold it. Pure. */
export function originText(entries: readonly (Pick<Entry, 'kind' | 'msg_id' | 'text'> | undefined)[], id: number): string | null {
  const e = entries.find((x) => x !== undefined && x.kind === 'you' && x.msg_id === id);
  const text = e?.text.trim() ?? '';
  return text === '' ? null : text;
}

/** a start's words: today's hour, else the date */
function started(ms: number, now: number): string {
  const d = new Date(ms);
  return now - ms < 20 * 3_600_000 ? d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : d.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

export function TaskTab({ project, id }: { project: string; id: string }) {
  const works = useWorks(project);
  const projects = useProjects();
  const agents = useAgents(project);
  const cards = useCards(project);
  const allArts = useArtifacts(project);
  const store = useStore();
  const router = useRouter();
  const act = useTaskAct();
  const req = useTaskReq();
  const now = useNowFor(() => false);
  const [renaming, setRenaming] = useState(false);
  // '+ Agent': the field open; asked: how many agents it had when he asked (its line goes when one more comes)
  const [asking, setAsking] = useState(false);
  const [asked, setAsked] = useState<{ readonly n: number } | null>(null);
  const w = works.works.find((x) => x.id === id);
  // his message that started it, quoted from main's thread when this window holds it (by its id)
  const mainKey = threadKey(project, MAIN);
  const order = useThreadOrder(mainKey);
  const origin = w?.origin == null ? null : originText(order.order.map((p) => store.entry(mainKey, p)), w.origin);
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
  const arts = artsOf(allArts, w.id, works.tombs);
  // its agents that work on a branch of their own (a worktree)
  const changes = names.flatMap((n) => {
    const a = live(n);
    return a?.branch == null || a.branch === '' ? [] : [{ name: n, branch: a.branch }];
  });
  return (
    <div className="tk-tab" data-testid={t}>
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
        <p className="tk-meta">{tasks.tab_meta({ project: x.projectName, agents: tasks.tab_agents({ n: names.length }), when: started(w.created_ms, now) })}</p>
      </header>

      {w.origin != null && (
        <Press look="tk-origin" {...tipAttrs(tasks.origin_tip)} onClick={() => {
          router.go({ screen: 'thread', project, agent: MAIN });
        }} data-testid={`${t}.origin`}>{origin === null ? tasks.origin_open : `“${cutAtWord(origin, 160)}”`}</Press>
      )}
      {(w.why ?? '') !== '' && <p className="tk-why">{w.why}</p>}

      {w.status === 'paused' && (
        <p className="tk-paused" data-testid={`${t}.paused`}>
          <StatusMark status="paused" /> {lineAt(x, now)}
          <Press look="b-btn sm" onClick={() => {
            act(project, 'resume', w.id);
          }} data-testid={`${t}.resume.line`}>{tasks.act_resume}</Press>
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
              <span className="tk-q-opts">
                {askButtons(c).opts.map((o) => (
                  <OptButton key={o.n} label={o.label} onPick={() => store.do(pickIntent(c, o.n, null))} testId={`${t}.q.${String(c.id)}.${String(o.n)}`} />
                ))}
              </span>
            </div>
          ))}
        </section>
      )}

      <section className="tk-sec">
        <h3 className="tk-h">{tasks.sec_now}</h3>
        <p className="tk-why" data-testid={`${t}.now`}>{lineAt(x, now)}</p>
      </section>

      {names.length > 0 && (
        <section className="tk-sec">
          <h3 className="tk-h">{tasks.sec_agents}</h3>
          <ul className="tk-agents">
            {names.map((n) => {
              const a = live(n);
              const on = a?.status === 'working';
              const state = a === undefined ? tasks.agent_archived : on ? tasks.agent_live : tasks.agent_idle;
              return (
                <li key={n} role="link" tabIndex={0} data-archived={a === undefined || undefined} onClick={() => {
                  router.go({ screen: 'thread', project, agent: n });
                }} onKeyDown={(e) => {
                  if (e.key === 'Enter') router.go({ screen: 'thread', project, agent: n });
                }} data-testid={`${t}.agent.${n}`}>
                  <span className="tk-live" data-on={on || undefined} />
                  <span className="tk-agent">{n}</span>
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
                <Press look="tk-row" {...tipAttrs(a.title)} onClick={() => {
                  openTab({ kind: 'artifact', project, id: a.id });
                }} data-testid={`${t}.art.${a.id}`}>
                  <span className="tk-row-name">{a.title}</span>
                  <span className="tk-agent-state">{a.agent}</span>
                </Press>
              </li>
            ))}
          </ul>
          <Press look="tk-origin" onClick={() => {
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
        <p className="tk-meta" data-testid={`${t}.asked`}>{tasks.agent_asked}</p>
      )}

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
              setAsked({ n: names.length });
              setAsking(false);
            }} />
        </footer>
      ) : (
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
          w.status !== 'done' && (
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
      )}
    </div>
  );
}
