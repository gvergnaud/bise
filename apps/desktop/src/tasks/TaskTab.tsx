// The Task tab (brief "Task tab", content approved by him as drawn; designer's look rules: 384 wide):
// its status pill, name, project · agents · started; his origin (a link to his message); why;
// done-when; Needs you answered in place (its cards' options); Now (its line); Agents (live dot,
// archived greyed); a paused task's line 'this task is paused · Resume' (tasks-ux); footer Mark done
// · Pause/Resume (Reopen when done, Restart when stalled). A merged or folded one says where it went.
// Gestures go through the hub's task_act. Owner: tasks.
import { Press, tipAttrs } from '@/ds';
import { sentenceCase } from '@/ds/case';
import { useAgents, useCards, useProjects, useStore } from '@/store';
import { useRouter } from '@/app/router';
import { MAIN } from '@/app/route';
import { useNowFor } from '@/app/clock';
import { OptButton } from '@/app/side3d/FlyoutAgents';
import { askButtons, pickIntent } from '@/app/side3d/model';
import { tasks } from '@/proto/words_tasks.gen';
import { useWorks } from '@/store';
import { goalsOf } from './model';
import { openTask, StatusMark, statusWord } from './Marks';
import { lineAt } from './Side';
import { useTaskAct } from './Board';

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
  const store = useStore();
  const router = useRouter();
  const act = useTaskAct();
  const now = useNowFor(() => false);
  const w = works.works.find((x) => x.id === id);
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
  const t = `tasks.tab.${w.id}`;
  return (
    <div className="tk-tab" data-testid={t}>
      <header className="tk-tab-head">
        <span className="tk-pill" data-st={w.status} data-testid={`${t}.status`}>
          <StatusMark status={w.status} />
          {sentenceCase(statusWord(w.status))}
        </span>
        <h2 className="tk-tab-name">{w.name}</h2>
        <p className="tk-meta">{tasks.tab_meta({ project: x.projectName, agents: tasks.tab_agents({ n: names.length }), when: started(w.created_ms, now) })}</p>
      </header>

      {w.origin != null && (
        <Press look="tk-origin" {...tipAttrs(tasks.origin_tip)} onClick={() => {
          router.go({ screen: 'thread', project, agent: MAIN });
        }} data-testid={`${t}.origin`}>{tasks.origin_open}</Press>
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
      </footer>
    </div>
  );
}
