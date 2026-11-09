// A thread's tool rows (one summary line per row, open to each call's state, time, error line,
// code and output) and its thinking rows. Owner: nm-thread. Split out of rows.tsx (pure move).
import { useContext, useEffect } from 'react';
import { AppIcon, Fold, Press, tipAttrs } from '@/ds';
import type { Entry } from '@/proto';
import { thread } from '@/proto/words.gen';
import { running } from '@/proto/words_running.gen';
import { useProcesses, useStore, useToolOut } from '@/store';
import { RouterContext } from '@/app/router';
import { useNow } from '@/app/clock';
import { shortAge } from '@/agents/model';
import { isRunning, procOfCall } from '@/running/model';
import { liveTime, useLiveStep, useSecondsSince } from './live-step';
import { useFold } from './folds';
import { useFoldHint } from './foldHint';
import { thinkingWords } from './thinking';
import { editCounts, toolLines, toolsWords, type ToolLine } from './entry';
import { Code, known } from './Code';
import type { RowCtx } from './rows';
import { Pulse } from '@/working/Signs';
/**
 * The line under a cut output (designer m_15928): '… 4 KB of 18 KB shown' when the hub knows the
 * whole size (bytes, the cap's unit; shown = the out's UTF-8 bytes), else '… cut after 4 KB'. Pure.
 */
export function cutWords(out: string, total?: number): string {
  const kb = (b: number): string => `${String(Math.max(1, Math.round(b / 1024)))} KB`;
  if (total == null || total <= 0) return thread.out_cut;
  return thread.out_shown({ shown: kb(new TextEncoder().encode(out).length), total: kb(total) });
}

/** A thinking row's head, as the TUI's words::thought_for: 'thought for 3.2s', 'thought' (time unknown). Pure. */
export function thoughtFor(ms: number): string {
  if (ms <= 0) return thread.thought;
  // words::think_time's format
  const s = Math.floor(ms / 1000);
  const dur = ms < 10_000 ? `${String(s)}.${String(Math.floor((ms % 1000) / 100))}s` : ms < 60_000 ? `${String(s)}s` : `${String(Math.floor(ms / 60_000))}m${String(Math.floor((ms % 60_000) / 1000))}s`;
  return thread.thought_for({ dur });
}

/** What the model thought: one folded line, open to its words (⌘E opens them all). */
export function ThinkingLine({ pos, head, body: raw }: { pos: number; head: string; body: string }) {
  const body = thinkingWords(raw);
  const [open, setOpen] = useFold(false, pos);
  const hint = useFoldHint();
  const toggle = () => {
    setOpen((o) => !o);
  };
  return (
    <div className="th-think" data-testid={`thread.think.${pos}`}>
      <Fold open={open} onToggle={body === '' ? undefined : toggle} hint={body === '' ? undefined : hint} testId={`thread.think.${pos}.head`}>
        {head}
      </Fold>
      {open && body !== '' && <div className="th-think-body">{body}</div>}
    </div>
  );
}

/**
 * Its tool calls in a row (QA §4, mockup 1): one summary line, 28 high, 13 text-3, the last
 * command's description there is or else the hub's counted words (toolsWords), and the edits' lines ('edited 3 files +42 −18 ›', the + in --c-ok, the − in
 * --c-err), its › opening every step (a click, ⏎, ⌘E). What must show folded shows under it: a
 * failed call (R11, the TUI's ✗ row) and the call at work with its '∿ 15 s' (S9); a call the model
 * described is drawn once, by the growing fold at the thread's end (live-step.tsx).
 */
export function ToolsRow({ e, ctx }: { e: Entry; ctx: RowCtx }) {
  const [open, setOpen] = useFold(false, e.pos);
  const items = e.tools?.items ?? [];
  const step = useLiveStep(ctx.tkey, ctx.project, ctx.agent);
  const live = step?.entry === e.pos ? step : undefined;
  const runFor = useSecondsSince(live?.at);
  const t = toolLines(items);
  const said = live !== undefined && live.intent !== '' ? live.pos : undefined;
  const all = [...t.lines, ...t.rest].filter((l) => l.pos !== said).sort((x, y) => x.pos - y.pos);
  const shown = open ? all : all.filter((l) => l.state === 'err' || l.pos === live?.pos);
  const counts = editCounts(items);
  // the last command's description (a group's count after it), else the hub's counted words (his m_49672)
  const line = toolsWords(items, e.tools?.summary !== undefined && e.tools.summary !== '' ? e.tools.summary : e.text, said);
  const id = `thread.tools.${String(e.pos)}`;
  const toggle = () => {
    setOpen((o) => !o);
  };
  return (
    <div className="th-tools" data-testid={`thread.toolrow.${String(e.pos)}`}>
      <Press look="th-ran" aria-expanded={open} onClick={toggle} data-testid={id}>
        <AppIcon name={counts === null ? 'terminal' : 'file'} size={14} />
        {/* a group's ' · 3 commands' wraps out of sight before its description is cut (designer m_49693) */}
        <span className="th-ran-said">
          <span className="th-ran-words">{line.words}</span>
          {line.count !== '' && (
            <span className="th-ran-n" data-testid={`${id}.n`}>
              {` · ${line.count}`}
            </span>
          )}
        </span>
        {counts !== null && (
          <span className="th-ran-counts" data-testid={`${id}.counts`}>
            <span className="th-ran-add">+{String(counts.add)}</span> <span className="th-ran-del">−{String(counts.del)}</span>
          </span>
        )}
        <span className="th-ran-chev">
          <AppIcon name="right" size={14} />
        </span>
      </Press>
      {shown.length > 0 && (
        <div className="th-steps">
          {shown.map((l) => (
            <ToolLineRow key={l.pos} l={l} live={live?.pos === l.pos} runFor={runFor} project={ctx.project} agent={ctx.agent} />
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * One call's line (R11/S9, the TUI's tool row): its mark and words; on the right its state ('∿ 12 s'
 * while it runs, '✓ 0.3 s' dim, '✗ exit 1 · 2.1 s' in the error tone, never by color alone); a
 * failed call's first error line under it; a click (or ⌘E, the TUI's ctrl+o) opens its code and
 * output.
 */
function ToolLineRow({ l, live, runFor, project, agent }: { l: ToolLine; live: boolean; runFor: number; project: string; agent: string }) {
  const [open, setOpen] = useFold(false);
  const store = useStore();
  // the call's real output (amb-feed dece0fe9): asked once when it opens, its 200-char preview until then
  const full = useToolOut(project, agent, l.pos);
  const finished = !live && (l.state === 'ok' || l.state === 'err');
  const opens = l.code !== '' || l.out !== '' || finished;
  useEffect(() => {
    if (open && finished) store.askToolOut(project, agent, l.pos);
  }, [open, finished, store, project, agent, l.pos]);
  const out = full !== null && 'out' in full && full.out !== '' ? full.out : l.out;
  const id = `thread.tool.${String(l.pos)}`;
  const state = live ? 'run' : l.state;
  // a command it left running in the background: '∿ 14 min' at its end, a click opens its output
  // on the running screen (designer's running-procs)
  // none where a row is drawn outside the app (a preview, a test): the row then opens nothing
  const router = useContext(RouterContext);
  const procs = useProcesses(project);
  // the minute clock: its age as the bar's
  const now = useNow();
  const proc = !live && l.mark === '$' ? procOfCall(procs, agent, l.code) : null;
  const bg = proc !== null && isRunning(proc) ? proc : null;
  if (bg !== null && router !== null) {
    const go = () => {
      router.go({ screen: 'running', open: { project, agent, id: bg.id } });
    };
    return (
      <div className="th-tool" data-state="run">
        <div className="th-tool-line" data-testid={id} data-bg={bg.id} role="button" tabIndex={0} onClick={go} onKeyDown={(e) => {
          if (e.key !== ' ' && e.key !== 'Enter') return;
          e.preventDefault();
          go();
        }}>
          <span className="th-tool-mark">{l.mark}</span> <span className={l.said ? 'th-tool-text said' : 'th-tool-text'}>{l.text}</span>
          {/* its tip, like every compact mark (designer m_38604) */}
          <span className="th-tool-time" data-testid={`${id}.bg`} {...tipAttrs(running.tip_thread_row)}>{`∿ ${shortAge(now - bg.started_ms)}`}</span>
        </div>
      </div>
    );
  }
  return (
    <div className="th-tool" data-state={state || undefined}>
      <div
        className={live ? 'th-tool-line live' : 'th-tool-line'}
        data-testid={id}
        data-live={live || undefined}
        role={opens ? 'button' : undefined}
        tabIndex={opens ? 0 : undefined}
        aria-expanded={opens ? open : undefined}
        onClick={
          opens
            ? () => {
                setOpen((o) => !o);
              }
            : undefined
        }
        onKeyDown={
          opens
            ? (e) => {
                if (e.key !== ' ' && e.key !== 'Enter') return;
                e.preventDefault();
                setOpen((o) => !o);
              }
            : undefined
        }
      >
        {/* a read has no mark: no empty slot either, so its words start on the edge (designer m_21012) */}
        {l.mark !== '' && <span className="th-tool-mark">{l.mark}</span>}{' '}
        <span className={l.said ? 'th-tool-text said' : 'th-tool-text'}>{l.text}</span>
        {/* the account it ran as, after its words like the TUI's 'gmail.search · perso' (multi-accounts piece 5) */}
        {l.account !== '' && (
          <span className="th-tool-count" data-testid={`${id}.account`}>
            {`· ${l.account}`}
          </span>
        )}
        {l.counts !== '' && (
          <span className="th-tool-count" data-testid={`${id}.count`}>
            {l.counts}
          </span>
        )}
        {live ? (
          // '$ cmd   ∿ 15 s' (designer's desktop-working v2 (4), the TUI's running tool row)
          <span className="th-tool-time" data-testid={`${id}.time`}>
            <Pulse kind="tool" testId={`${id}.pulse`} /> {liveTime(runFor)}
          </span>
        ) : (
          l.end !== '' && (
            <span className={l.state === 'err' ? 'th-tool-time err' : 'th-tool-time'} data-testid={`${id}.end`}>
              {l.end}
            </span>
          )
        )}
      </div>
      {/* open, the box shows the error: the red line above it goes (designer m_14103) */}
      {l.err !== '' && !(open && opens) && (
        <div className="th-tool-err" data-testid={`${id}.err`}>
          {l.err}
        </div>
      )}
      {/* an empty box never shows: only with its code, its output or the hub's words about it */}
      {open && opens && (l.code !== '' || out !== '' || full !== null) && (
        <div className="th-tool-open" data-testid={`${id}.open`}>
          {/* the source it ran, decoded and in its colours (the TUI's box, toolbox.rs): never its args' JSON */}
          {l.code !== '' && (
            <pre className="th-tool-code" data-testid={`${id}.code`} data-lang={l.lang || undefined}>
              {known(l.lang) ? <Code text={l.code} lang={l.lang} /> : l.code}
            </pre>
          )}
          {out !== '' && (
            <pre className="th-tool-out" data-testid={`${id}.out`}>
              {out}
            </pre>
          )}
          {full !== null && 'cut' in full && full.cut && (
            <div className="th-tool-cut" data-testid={`${id}.cut`}>
              {cutWords(full.out, full.total)}
            </div>
          )}
          {full !== null && 'error' in full && out === '' && (
            <div className="th-tool-cut" data-testid={`${id}.none`}>
              {full.error}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

