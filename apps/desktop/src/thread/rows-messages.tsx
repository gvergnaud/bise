// A thread's message rows: his bubbles (their delivery marks; their body, quotes, words, chips and
// fold, is YouBody.tsx, the same while sending and once sent), his sends not confirmed yet (a queued one is the composer's drawer until it is said), the route hold and
// the agent's typing line. Owner: amb-win. Split out of rows.tsx (pure move).
import { memo, useContext, useEffect, useState, type ReactNode } from 'react';
import { RouterContext } from '@/app/router';
import { TaskChipsOf } from '@/tasks';
import { Button, Chip, Press } from '@/ds';
import type { Entry } from '@/proto';
import { common, thread } from '@/proto/words.gen';
import { useAgents, useCoreState, useHubRefused, usePending, useProjects, useStore } from '@/store';
import { refusedLead } from '@/app/Refused';
import { useHoldOf, type Hold } from './routes';
import { chipOf, clock, deliveryMark, lastOfRun, stripContext } from './entry';
import { shownName } from '@/agents/model';
import { saidFor, useSaidMs } from '@/voicemode';
import type { RowCtx } from './rows';
import { drawnParts, viaFn, youView } from './parts';
import { YouBody } from './YouBody';
import { rowLayout } from './rows';
import { useToday } from '@/app/clock';
import type { Pending } from '@/store';
/**
 * His words on the right in a soft bubble (designer's messages-right v1; the look lives in
 * thread.css '.th-you'): no 'you' line, the side says who spoke. A dim head over it only when it
 * says something ('to perf', '✓ you answered gift-ui'); inside, a quote, then his words, then what
 * rides with them; under it, its time (always under the last bubble of a run, on hover for the
 * others) or its state ('sending…', 'not sent…') in the time's place.
 */
export function YouBubble({
  head,
  children,
  under,
  timeAlways = true,
  state,
  testId,
}: {
  head?: ReactNode;
  children?: ReactNode;
  under?: ReactNode;
  /** false: the time shows on hover only (a bubble inside a run) */
  timeAlways?: boolean;
  state?: 'sending' | 'failed';
  testId?: string;
}) {
  return (
    <div className="th-you" data-state={state} data-testid={testId}>
      {head != null && head !== '' && (
        <div className="th-you-head" data-testid={testId === undefined ? undefined : `${testId}.head`}>
          {head}
        </div>
      )}
      {children != null && <div className="b-text th-bubble">{children}</div>}
      {under != null && under !== '' && (
        <div className={timeAlways ? 'th-you-under' : 'th-you-under th-hover'} data-testid={testId === undefined ? undefined : `${testId}.under`}>
          {under}
        </div>
      )}
    </div>
  );
}

/**
 * His words typed in this thread that went to another agent (at-start, designer m_34049): one quiet
 * row where he sent them, '→ sent to architect · <his words>', the name a link to its thread. The
 * message itself is in that agent's thread.
 */
function WentRow({ e, ctx, name }: { e: Entry; ctx: RowCtx; name: string }) {
  const router = useContext(RouterContext);
  const drawn = drawnParts(e.parts);
  const text = drawn !== null ? youView(drawn).words : stripContext(e.text);
  const pos = String(e.pos);
  return (
    <div className="th-went" title={thread.went_agent_tip({ name: shownName(name) })} data-testid={`thread.you.${pos}`} data-went="agent">
      {'→ '}
      {thread.sent_to({ name: '' })}
      <span
        className="th-answer-link"
        role="link"
        onClick={(ev) => {
          if (!(window.getSelection()?.isCollapsed ?? true)) return;
          ev.stopPropagation();
          router?.go({ screen: 'thread', project: ctx.project, agent: name });
        }}
        data-testid={`thread.you.${pos}.went`}
      >
        {shownName(name)}
      </span>
      {` · ${text}`}
    </div>
  );
}

export function YouRow({ e, ctx, next }: { e: Entry; ctx: RowCtx; next: Entry | undefined }) {
  if (e.went?.agent === true) return <WentRow e={e} ctx={ctx} name={e.went.name} />;
  // the task his message started, under it (a task's origin is his message's id: WorkRow.origin)
  const id = e.msg_id ?? null;
  return (
    <>
      <YouMessage e={e} ctx={ctx} next={next} />
      {id !== null && (
        <div className="tk-under-you">
          <TaskChipsOf project={ctx.project} of={{ origin: id }} testId={`thread.you.${String(e.pos)}.task`} />
        </div>
      )}
    </>
  );
}

function YouMessage({ e, ctx, next }: { e: Entry; ctx: RowCtx; next: Entry | undefined }) {
  // msg-parts: his message comes from its typed parts, never from Entry.text (the model's templated
  // text; the fold gives every 'you' entry its parts since msg-parts sha c). No parts (an older
  // hub): the text drawn plain, never parsed for tags (architect's delta review D1); a part of a
  // kind this window doesn't know (a newer hub's): drawn from its text, never empty. The fn context
  // block the hub appended never shows: a chip from the typed context instead
  const drawn = drawnParts(e.parts);
  const text = drawn !== null ? youView(drawn).words : stripContext(e.text);
  const chip = e.context != null ? chipOf(e.context) : null;
  // in bise's or a main's thread, a message that went to someone else says so; to the owner, nothing
  const to = e.to != null && e.to !== '' && e.to !== ctx.agent ? common.to_agent({ agent: e.to }) : undefined;
  // a steer: the hub's receipt says where the agent read it (the list placed it there), with
  // 'steered · perf read it mid-turn' under it (amb-feed 9633cd2a; no more guessing from neighbors)
  const steered = e.steered_at != null;
  // his turn by voice (designer's voice-window page (1)): 'said · 0:06' under it, how long he talked
  const saidMs = useSaidMs(ctx.project, ctx.agent, text);
  // the delivery mark after the time or the 'said' line (designer's messages-right v3); a steered
  // one's 'steered · … read it mid-turn' line is its ✓✓, so it gets none
  const mark = steered ? null : deliveryMark(e.delivery, shownName(to !== undefined && e.to != null ? e.to : ctx.agent));
  // H12 (designer m_19832): no time under his bubbles, only on hover; the steer, the said line and
  // the delivery mark stay
  const words = steered ? thread.steered({ agent: shownName(ctx.agent) }) : saidMs !== null ? saidFor(saidMs) : undefined;
  // typed with fn space (designer m_22714): 'from fn' first on the row, always; the time after it
  // keeps H12's hover rule (its ' · ' with it). Read from the hub's hint part, never its words
  const fromFn = drawn !== null && viaFn(drawn, saidMs !== null);
  // routed by bise (route-name, designer m_25074): in bise's main, 'sent to harness' (main never
  // got it: no mark), its name a link to harness's main thread (a history step); in the project's
  // thread, 'via bise' (his own words, bise brought them). Always shown, after 'from fn'
  const router = useContext(RouterContext);
  const went = e.went;
  const routed: ReactNode =
    went != null ? (
      <>
        {thread.sent_to({ name: '' })}
        <span
          className="th-answer-link"
          role="link"
          onClick={(ev) => {
            if (!(window.getSelection()?.isCollapsed ?? true)) return;
            ev.stopPropagation();
            router?.go({ screen: 'thread', project: went.to, agent: 'main' });
          }}
          data-testid={`thread.you.${String(e.pos)}.went`}
        >
          {went.name}
        </span>
      </>
    ) : e.via === 'bise' ? (
      thread.via_bise
    ) : undefined;
  const time =
    e.at_ms > 0 ? (
      <span className="th-you-time" data-testid={`thread.you.${String(e.pos)}.time`}>
        {fromFn || routed !== undefined ? ` · ${clock(e.at_ms)}` : clock(e.at_ms)}
      </span>
    ) : undefined;
  const lead =
    words !== undefined ? (
      words
    ) : fromFn || routed !== undefined ? (
      <>
        <span
          className="th-you-via"
          title={went != null ? thread.went_tip({ name: went.name }) : routed !== undefined ? thread.via_bise_tip : thread.from_fn_tip}
          data-testid={`thread.you.${String(e.pos)}.via`}
        >
          {fromFn && thread.from_fn}
          {fromFn && routed !== undefined && ' · '}
          {routed}
        </span>
        {time}
      </>
    ) : (
      time
    );
  const under =
    mark === null ? (
      lead
    ) : (
      <>
        {lead}
        {lead !== undefined && ' '}
        <span className="th-mark" data-tone={mark.tone} title={mark.tip} aria-label={mark.tip} data-testid={`thread.you.${String(e.pos)}.mark`}>
          {mark.glyph}
        </span>
      </>
    );
  return (
    <YouBubble head={to} under={under} timeAlways={words !== undefined || fromFn || routed !== undefined || (mark !== null && lastOfRun(e, next))} testId={`thread.you.${String(e.pos)}`}>
      <YouBody
        parts={drawn}
        plain={text}
        ctx={ctx}
        id={`thread.you.${String(e.pos)}`}
        foldPos={e.pos}
        extra={
          chip !== null && (
            <div className="b-ctx">
              <ContextChip chip={chip} ctx={ctx} pos={e.pos} />
            </div>
          )
        }
      />
    </YouBubble>
  );
}

/** '◫ with Chrome · localhost:4801/sett…': a url opens in his browser, a file in his editor; a shot alone isn't kept. */
function ContextChip({ chip, ctx, pos }: { chip: NonNullable<ReturnType<typeof chipOf>>; ctx: RowCtx; pos: number }) {
  const open = chip.open;
  const run =
    open === null
      ? undefined
      : 'url' in open
        ? () => {
            ctx.onLink(open.url);
          }
        : () => {
            void (window as Partial<Window>).bise?.openPath(open.file, {});
          };
  return (
    <Press look="th-ctx" disabled={run === undefined} title={chip.tip} onClick={run} data-testid={`thread.context.${String(pos)}`}>
      {chip.label}
    </Press>
  );
}

/**
 * His send before its entry lands: shown at once (bar ⛔3), dimmed while
 * sending; failed, one line with retry and dismiss (never a send with no
 * trace). His words stay here and in the composer's history.
 */
export const PendingRow = memo(function PendingRow({ ctx, cid, after }: { ctx: RowCtx; cid: number; after?: Entry | undefined }) {
  const store = useStore();
  const pending = usePending(ctx.tkey);
  const i = pending.findIndex((x) => x.cid === cid);
  const p = pending[i];
  const hold = useHoldOf(store, ctx.tkey, cid);
  const connecting = useConnecting(ctx.project);
  const today = useToday();
  if (p === undefined) return null;
  // its place among its neighbors as its entry will have it (rows.tsx rowLayout): the row the
  // list ends on (`after`), else his send before it; nothing moves at its echo (bubble-parts)
  const before = i > 0 ? pending[i - 1] : undefined;
  const at = rowLayout({ prev: before !== undefined ? pendingEntry(before) : after, e: pendingEntry(p), next: undefined, ctx, today });
  // a routed send: its route's state is the row's (lead m_9854)
  const failed = p.state === 'failed' && hold?.state !== 'sent';
  const state = hold !== undefined && hold.state !== 'cancelled' ? hold.state : p.state;
  // under his bubble, in its time's place (messages-right v1): 'sending…' until it lands, the
  // reason it wasn't sent in --c-err with 'try again' (his words stay in the bubble), where it went
  // the store's own 'not sent' (store.ts): no why to add
  const why = p.why !== undefined && p.why !== common.not_sent ? p.why : '';
  const under =
    hold?.state === 'held' ? undefined : hold?.state === 'sent' ? thread.sent_to({ name: hold.name }) : failed ? (
      <span className="th-you-fail">
        <span className="th-you-err">{why === '' ? thread.not_sent : thread.not_sent_because({ why })}</span>
        <Press
          look="th-you-act"
          onClick={() => {
            store.retry(ctx.tkey, cid);
          }}
          data-testid={`thread.retry.${cid}`}
        >
          {common.try_again}
        </Press>
        <Press
          look="th-you-act"
          onClick={() => {
            store.dismiss(ctx.tkey, cid);
          }}
          data-testid={`thread.dismiss.${cid}`}
        >
          {thread.drop}
        </Press>
      </span>
    ) : (
      (connecting ?? thread.sending)
    );
  return (
    <>
    {at.day !== null && (
      <div className="th-day" data-testid={`thread.sending.${cid}.day`}>
        <span>{at.day}</span>
      </div>
    )}
    {at.pause && (
      <div className="th-pause" data-testid={`thread.sending.${cid}.pause`}>
        · {clock(p.at_ms)} ·
      </div>
    )}
    <div className={failed ? 'th-row th-pending th-failed' : hold?.state === 'sent' ? 'th-row th-routed' : 'th-row th-pending'} data-testid={`thread.pending.${cid}`} data-state={state} data-side="you" data-head={at.head} data-turn={at.turn || undefined} data-gap={at.gap}>
      <YouBubble under={under} state={failed ? 'failed' : 'sending'}>
        {/* the same body as once it landed, from the same parts (bubble-parts): nothing moves at its echo */}
        <YouBody parts={drawnParts(p.parts) ?? [{ kind: 'text', text: p.text }]} plain={p.text} ctx={ctx} id={`thread.sending.${String(cid)}`} />
      </YouBubble>
      {hold?.state === 'held' && <HoldLine hold={hold} />}
    </div>
    </>
  );
});

/** His send as the entry its echo will be, for its place in the list (rowLayout). Pure. */
export function pendingEntry(p: Pending): Entry {
  return { pos: -1, at_ms: p.at_ms, kind: 'you', text: p.text, ...(p.parts === undefined ? {} : { parts: [...p.parts] }) };
}

/**
 * The wait before a send can go, named (lead m_10108): while the core
 * starts or the project's hub hasn't said who it is yet (no agents from it:
 * its welcome comes first), 'connecting to bise…'; none once connected.
 */
function useConnecting(project: string): string | undefined {
  const core = useCoreState();
  const agents = useAgents(project);
  const row = useProjects().find((p) => p.project === project);
  // its hub refused this core: say so, never an endless 'connecting…' (lead m_11347; the
  // hub's words and the retry are on the line over the composer)
  const refused = useHubRefused(project) !== null;
  const name = row === undefined || row.home ? 'bise' : row.name;
  if (refused) return refusedLead(name);
  if (core === 'up' && agents.length > 0) return undefined;
  return thread.connecting({ name });
}

/** Seconds left to change a held route, ticking while it shows. */
function useLeft(until: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => {
      setNow(Date.now());
    }, 200);
    return () => {
      clearInterval(t);
    };
  }, []);
  return Math.max(0, Math.ceil((until - now) / 1000));
}

/**
 * 'to shop · 2 s to change': bise holds his words for a project's main.
 * Keep it with bise (a click here or esc: route_cancel) or pick another
 * project (route_correct); the hub ends it with route_done.
 */
function HoldLine({ hold }: { hold: Hold }) {
  const store = useStore();
  const left = useLeft(hold.until);
  const others = useProjects().filter((p) => !p.home && p.project !== hold.to);
  return (
    <div className="th-hold" data-testid="thread.route" role="status" title={hold.why}>
      <span className="th-hold-to">
        {thread.hold_line({ name: hold.name, left })}
      </span>
      <Button
        kind="ghost"
        onClick={() => {
          store.command({ cmd: 'route_cancel', project: hold.home, rid: hold.rid });
        }}
        testId="thread.route.keep"
      >
        {thread.route_keep}
      </Button>
      {others.map((o) => (
        <Chip
          key={o.project}
          onClick={() => {
            store.command({ cmd: 'route_correct', project: hold.home, rid: hold.rid, to: o.project });
          }}
          testId={`thread.route.to.${o.project}`}
        >
          {o.name}
        </Chip>
      ))}
    </div>
  );
}

