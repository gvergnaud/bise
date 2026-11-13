// A thread's rows: one entry, one send not confirmed yet. Owner: amb-win.
//
// Each row reads its own data with the store's per-key hooks (useEntry,
// usePending), so a new or replaced entry re-renders that row alone; the
// list above it only holds keys. Memo'd on their props (strings, numbers).
// This file dispatches an entry to its row; the row families live next to it:
// rows-messages (his bubbles, sends, queued, the route hold), rows-tools (tool
// calls, thinking), rows-cards (cards, approvals, answers, agent-to-agent),
// rows-news (landed, PRs, artifacts, scheduled, notices, briefs, not delivered).
import { memo, type ReactNode } from 'react';
import { Marker, Msg, RowMark } from '@/ds';
import type { Entry } from '@/proto';
import { thread } from '@/proto/words.gen';
import { useEntry } from '@/store';
import { Answer } from './Markdown';
import { GROUP_GAP_MS, THOUGHT_MS, answeredCardOf, repeatedReport, briefOf, clock, dayLabel, groupHead, lastOfRun, sameDay, sideOf, dropAuthor, reportStatus, thoughtNothing, thoughtShows, whoOf } from './entry';
import { useToday } from '@/app/clock';
import { useIsCurrent } from './findmark';
import { useVoiceLit } from '@/voicemode';
import { AgentNote, AnsweredRow, ApprovalRow, CardAnsweredRow, CardRow, PageAnswerRow, ToAgentRow } from './rows-cards';
import { pageOf } from './parts';
import { shownName } from '@/agents/model';
import { BODY_KINDS, NAMELESS_KINDS, NOTE_KINDS, QUIET_KINDS, QUIET_REPORT, ROW_KINDS, sameRoute } from './tiers';
import { YouRow } from './rows-messages';
import { BriefRow, CompactedRow, LandedRow, MadeChip, MadeRow, NotDeliveredRow, NoticeMark, NoticeRow, PrRow, ReportRow, ScheduledRow, WakeRow } from './rows-news';
import { ThinkingLine, thoughtFor, ToolsRow } from './rows-tools';
/** What a row needs of its thread. */
export interface RowCtx {
  readonly tkey: string;
  readonly project: string;
  readonly agent: string;
  /** bise's own main (the pearl, 'bise') */
  readonly bise: boolean;
  /** a link's click; `mod`: ⌘ held (WebTab: his browser, page newtab-browser) */
  readonly onLink: (url: string, mod?: boolean) => void;
  /** an artifact named in a row (its chip, an `artifact:` link, a note's page): its view */
  readonly onArtifact: (id: string) => void;
  /** a file path named in an answer, relative to the agent's folder (M4, B14) */
  readonly onPath?: (path: string, line?: number) => void;
}

export const EntryRow = memo(function EntryRow({
  ctx,
  pos,
  prev,
  next,
  live,
  still = false,
  named,
  carry = false,
  afterFold = false,
  underFold = false,
  turnLine,
  retries,
}: {
  ctx: RowCtx;
  pos: number;
  prev?: number | undefined;
  next?: number | undefined;
  live?: Entry | undefined;
  still?: boolean;
  /** its name line shows (entry.ts namedRows: one head per author run, never on a tool row, H2) */
  named?: boolean | undefined;
  /** the live row: a turn that opened on its work still owes its name (namedRows' carry) */
  carry?: boolean;
  /** the row right under a folded run: it follows the fold's quiet row, not the hidden note (H1) */
  afterFold?: boolean;
  /** the first message of an open run: 8 px under its fold row, never a new group's 20 (designer m_19912 (2)) */
  underFold?: boolean;
  /** its turn's 'worked for' line (TurnLine.tsx), drawn over it under its day line: the line's 12 is its gap */
  turnLine?: ReactNode;
  /** a retry notice that ends a run of them: each try's words, oldest first (H9, tiers.ts retryRuns) */
  retries?: readonly string[] | undefined;
}) {
  // the live row (live.ts) is this same row drawing the entry it becomes, so
  // the entry takes its place with nothing unmounted (text-appearing v2 (4))
  const stored = useEntry(ctx.tkey, pos);
  const e = live ?? stored;
  const before = useEntry(ctx.tkey, prev ?? -1);
  const afterIt = useEntry(ctx.tkey, next ?? -1);
  const hit = useIsCurrent(pos);
  const today = useToday();
  if (e === undefined) return null;
  const p = prev === undefined ? undefined : before;
  // one 'session restored' row, never the same notice stacked (the TUI's)
  if (e.kind === 'notice' && p?.kind === 'notice' && (p.notice?.text ?? p.text) === (e.notice?.text ?? e.text)) return null;
  const n0 = next === undefined ? undefined : afterIt;
  const { day, pause, head, turn, gap, side, chained } = rowLayout({ prev: p, e, next: n0, ctx, today, afterFold, underFold, underTurn: turnLine !== undefined });
  // H2: the name line opens an author's run on its first row, a tool or a thinking row too
  // (designer m_21404 (3)); the times themselves on hover only
  const shown = named ?? (head || carry);
  const n = next === undefined ? undefined : afterIt;
  return (
    <>
      {day !== null && (
        <div className="th-day" data-testid={`thread.day.${pos}`}>
          <span>{day}</span>
        </div>
      )}
      {pause && (
        <div className="th-pause" data-testid={`thread.pause.${pos}`}>
          · {clock(e.at_ms)} ·
        </div>
      )}
      {turnLine}
      <div className="th-row" data-hit={hit || undefined} data-testid={live === undefined ? `thread.entry.${pos}` : 'thread.stream'} data-kind={e.kind} data-from={e.from ?? undefined} data-head={head} data-side={side} data-turn={turn || undefined} data-gap={gap}>
        <EntryBody e={e} ctx={ctx} head={shown} next={n} live={live !== undefined} still={still} chained={chained} retries={retries} />
      </div>
    </>
  );
});

/** Where a row sits among its neighbors (EntryRow's, and his sending bubble's, PendingRow, so
 *  nothing moves at its echo): its day separator, its '· 14:31 ·' pause, its name line, its turn
 *  and its gap. Pure. */
export function rowLayout({
  prev: p,
  e,
  next: n0,
  ctx,
  today,
  afterFold = false,
  underFold = false,
  underTurn = false,
}: {
  prev: Entry | undefined;
  e: Entry;
  next: Entry | undefined;
  ctx: { agent: string; bise: boolean };
  today: number;
  afterFold?: boolean;
  underFold?: boolean;
  underTurn?: boolean;
}): { day: string | null; pause: boolean; head: boolean; turn: boolean; gap: ReturnType<typeof gapOf> | 'row' | 'flush'; side: 'you' | 'them'; chained: boolean } {
  // ambient m_11777: a day separator between days; a name line opens a group (a new author, 5
  // min of silence); the rows inside show their time on hover in its place
  const day = e.at_ms > 0 && (p === undefined || !sameDay(p.at_ms, e.at_ms)) ? dayLabel(e.at_ms, today) : null;
  // a news or plumbing row has no name line: a new author there opens no group (designer m_19387);
  // nor does a message between agents right after another (an open run: 12 px apart, m_19784 (2))
  // a report the message under it repeats is a quiet row (H8): no group of its own, 4 px from quiet rows
  const quietMe = repeatedReport(e, n0);
  const quietPrev = p !== undefined && repeatedReport(p, e);
  const nameless = p !== undefined && (quietMe || NAMELESS_KINDS.has(e.kind) || (NOTE_KINDS.has(e.kind) && NOTE_KINDS.has(p.kind)));
  const head = groupHead(p, e, ctx) && !nameless && !underFold;
  // same sender → same receiver as the message before: one chip for both (m_19784 (2))
  const chained = sameRoute(p, e, ctx.agent);
  // his on the right, the rest on the left (messages-right v1); a change of side opens more room
  const side = sideOf(e);
  // under a turn's 'worked for' line (turns.ts): the line opened the turn and keeps its 12 after
  const turn = !underTurn && p !== undefined && day === null && sideOf(p) !== side;
  const gap = underTurn && day === null ? 'flush' : underFold ? 'row' : p === undefined || day !== null || turn || head ? undefined : gapOf(afterFold ? { kind: 'fold' } : quietPrev ? { kind: QUIET_REPORT } : p, quietMe ? { kind: QUIET_REPORT } : e);
  // H12: after 5 min of silence a faint '· 14:31 ·' (the TUI's)
  const pause = day === null && pauseOf(p, e);
  return { day, pause, head, turn, gap, side, chained };
}

/**
 * The room above a row inside one turn (designer m_19359, m_19387, m_19832 H1): 'tight' between two
 * quiet rows (news, wake, scheduled, new agent, the fold: 4 px), 'row' between tool rows or a tool
 * row and a quiet one (8 px), 'block' between two messages of an open run or two of its replies
 * (12 px), 'switch' from body text or a message to a dim row or back (12 px), none otherwise. Pure.
 */
export function gapOf(prev: { readonly kind: string }, e: { readonly kind: string }): 'tight' | 'row' | 'block' | 'switch' | undefined {
  if (NOTE_KINDS.has(prev.kind) && NOTE_KINDS.has(e.kind)) return 'block';
  const qa = QUIET_KINDS.has(prev.kind);
  const qb = QUIET_KINDS.has(e.kind);
  if (qa && qb) return 'tight';
  // a run of quiet rows: 12 from a message or prose, 8 from the tool rows of its turn
  if (qa !== qb) return TOOL_KINDS.has(qa ? e.kind : prev.kind) ? 'row' : 'switch';
  const a = ROW_KINDS.has(prev.kind);
  const b = ROW_KINDS.has(e.kind);
  if (a && b) return 'row';
  if (a !== b && BODY_KINDS.has(a ? e.kind : prev.kind)) return 'switch';
  // two replies of one run (no name line, no hover line any more: H12): 12 apart
  if (prev.kind === 'agent' && e.kind === 'agent') return 'block';
  return undefined;
}

/** The rows of a turn's work: never a name line over them (H2). */
const TOOL_KINDS: ReadonlySet<string> = new Set(['tools', 'thinking']);

/** The faint mark after 5 min of silence, as the TUI's ('· 14:31 ·', H12). Pure. */
export function pauseOf(prev: Pick<Entry, 'at_ms'> | undefined, e: Pick<Entry, 'at_ms'>): boolean {
  return prev !== undefined && prev.at_ms > 0 && e.at_ms > 0 && e.at_ms - prev.at_ms > GROUP_GAP_MS;
}

function EntryBody({ e, ctx, head, next, live, still, chained = false, retries }: { e: Entry; ctx: RowCtx; head: boolean; next: Entry | undefined; live: boolean; still: boolean; chained?: boolean; retries?: readonly string[] | undefined }) {
  // the hub's brief opens a task's thread: one quiet row, the brief folded under it (designer; the TUI's is_brief)
  const brief = e.kind === 'you' || e.kind === 'from_agent' || e.kind === 'agent' ? briefOf(e.text) : null;
  if (brief !== null) return <BriefRow pos={e.pos} name={brief.name} objective={brief.objective} body={brief.body} ctx={ctx} />;
  // his answer to main's own card, as the hub still sends it (a raw line from 'switchboard'):
  // the '✓ you answered' row, never the raw line (designer m_15195, ambient-lead m_15382)
  const answeredCard = e.kind === 'from_agent' || e.kind === 'agent' ? answeredCardOf(e.text) : null;
  if (answeredCard !== null) return <CardAnsweredRow pos={e.pos} a={answeredCard} at={e.at_ms} timeAlways={lastOfRun(e, next)} />;
  switch (e.kind) {
    case 'tools':
      return (
        <WorkHead e={e} ctx={ctx} head={head}>
          <ToolsRow e={e} ctx={ctx} />
        </WorkHead>
      );
    case 'card':
      return <CardRow e={e} ctx={ctx} />;
    case 'page':
      return e.page ? (
        <MadeChip
          kind="page"
          title={e.page.title}
          meta={[e.page.v == null ? '' : `v${String(e.page.v)}`, clock(e.at_ms)].filter(Boolean).join(' · ')}
          onClick={() => {
            if (e.page) ctx.onArtifact(e.page.id);
          }}
          testId={`thread.page.${String(e.pos)}`}
        />
      ) : (
        <Marker>{e.text}</Marker>
      );
    case 'stopped':
      // ⛔6: the hub's typed fact of a stop (amb-core 515aa58d), at its pos
      return (
        <div className="th-stopped" data-testid={`thread.stopped.${String(e.pos)}`}>
          <Marker>
            <RowMark icon="stop" tone="plain" />
            {e.text === '' ? 'stopped' : e.text}
            {e.at_ms > 0 ? ` · ${clock(e.at_ms)}` : ''}
          </Marker>
        </div>
      );
    case 'report':
      // a status line, never the '[report: …]' bracket form (designer D14)
    {
      const line = reportStatus(e.text, e.report?.kind) ?? e.text;
      // only what waits on him takes the accent: every done or failed line is dim (designer m_13926)
      return <ReportRow pos={e.pos} line={line} text={e.text} next={next} ctx={ctx} waits={line.startsWith('blocked')} />;
    }
    case 'you':
      // what he did on a page (his pick, note, tick): its answer row, never the hub's template
      return pageOf(e.parts) !== null ? <PageAnswerRow e={e} ctx={ctx} next={next} /> : <YouRow e={e} ctx={ctx} next={next} />;
    case 'to_agent':
      return <ToAgentRow e={e} ctx={ctx} chained={chained} />;
    // amb-feed's batch 2 (b8cb4b1e): the lines the hub's fold used to drop
    case 'thinking':
      // nothing thought under 1 s shows nothing (ambient m_11777, the TUI's)
      // (the hub's own head words 'thought for 0.0s' too)
      if (thoughtNothing(e)) return null;
      return (
        <WorkHead e={e} ctx={ctx} head={head}>
          <ThinkingLine pos={e.pos} head={e.text === '' ? thoughtFor(e.thinking?.ms ?? 0) : e.text} body={e.thinking?.text ?? ''} />
        </WorkHead>
      );
    case 'compacting':
      return (
        <div className="th-compact" data-testid={`thread.compacting.${e.pos}`}>
          <Marker>{thread.compacting_context}</Marker>
        </div>
      );
    case 'compacted':
      return <CompactedRow e={e} ctx={ctx} />;
    // R12: a failed turn is its own row with why, in the TUI's words (proto words::turn_failed: '✗ turn
    // failed: provider 500', a missing key's line; a plan line, its own words, a ▲), never a silent stop
    case 'turn_failed': {
      const level = e.turn_failed != null && e.text === e.turn_failed.why ? 'warn' : 'err';
      return (
        <div className={`th-notice ${level}`} data-level={level} data-testid={`thread.failed.${String(e.pos)}`}>
          <NoticeMark level={level} />
          <span>{e.text}</span>
        </div>
      );
    }
    case 'notice':
      return <NoticeRow e={e} ctx={ctx} retries={retries} />;
    case 'not_delivered':
      return <NotDeliveredRow e={e} ctx={ctx} />;
    // amb-feed's batch 3a (5d4189cb): the hub's news lines, typed
    case 'landed':
      return <LandedRow e={e} ctx={ctx} />;
    case 'pr':
      return <PrRow e={e} ctx={ctx} />;
    case 'artifact':
      return <MadeRow e={e} ctx={ctx} />;
    case 'answered':
      return <AnsweredRow e={e} />;
    case 'approval':
      return <ApprovalRow e={e} next={next} ctx={ctx} />;
    // batch 3b (6e9d68f8): a scheduled task's ◷ line
    case 'scheduled':
      return <ScheduledRow e={e} ctx={ctx} />;
    // event-wake: a watch's end, the hub's line with its folded output
    case 'wake':
      return <WakeRow e={e} ctx={ctx} />;
    case 'from_agent': {
      // tier 4 (designer m_19387): a message from another agent, never body prose; its report to
      // this agent stays a status line (the default's report branch below)
      const report = reportStatus(e.text);
      const from = shownName(e.from ?? thread.an_agent);
      if (report !== null) {
        return <ReportRow pos={e.pos} line={(e.from ?? ctx.agent) !== ctx.agent ? `${from}: ${report}` : report} text={e.text} next={next} ctx={ctx} waits={report.startsWith('blocked')} />;
      }
      const to = shownName(e.to != null && e.to !== '' ? e.to : ctx.agent);
      return <AgentNote pos={e.pos} from={from} to={to} text={dropAuthor(e.text, e.from ?? '')} parts={e.parts} ctx={ctx} testId={`thread.from.${String(e.pos)}`} chip={!chained} />;
    }
    case 'unknown':
      // a kind from a newer hub: its words, plain and dim
      return (
        <div className="th-unknown" data-testid={`thread.unknown.${e.pos}`}>
          {e.text}
        </div>
      );
    default: {
      const who = whoOf(e, ctx);
      // an agent's '[report: blocked] …' message: its status line (designer D14, the TUI's report_parts)
      const report = reportStatus(e.text);
      if (report !== null) {
        return <ReportRow pos={e.pos} line={who.from === 'agent' && (e.from ?? ctx.agent) !== ctx.agent ? `${who.name}: ${report}` : report} text={e.text} next={next} ctx={ctx} waits={report.startsWith('blocked')} />;
      }
      return <AgentMsg e={e} ctx={ctx} who={who} head={head} live={live} still={still} />;
    }
  }
}

/** A run that opens on its work (a tool or a thinking row): its author's name line above that first
 *  row, as over a message (designer m_21404 (3)); with no head, the row alone. */
function WorkHead({ e, ctx, head, children }: { e: Entry; ctx: RowCtx; head: boolean; children: ReactNode }) {
  if (!head) return children;
  const who = whoOf(e, ctx);
  return (
    <Msg from={who.from} name={who.name} indent={who.indent} time={clock(e.at_ms)} head>
      {children}
    </Msg>
  );
}

/** An agent's message; while voice mode says it aloud (designer's voice-window page (1)): the
 *  words said in ink, the rest faint, '∿ speaking' under it while it talks, 'you cut in' when he
 *  did (amb-core's voice_mode.lit); else its answer as always. */
function AgentMsg({ e, ctx, who, head, live, still }: { e: Entry; ctx: RowCtx; who: ReturnType<typeof whoOf>; head: boolean; live: boolean; still: boolean }) {
  const text = dropAuthor(e.text, e.from ?? ctx.agent);
  const lit = useVoiceLit(ctx.project, ctx.agent, e.pos, e.text, text);
  const mark =
    lit?.mark == null ? undefined : (
      <div className="vm-mark" data-testid={`thread.voice.${String(e.pos)}`} data-mark={lit.mark}>
        {lit.mark === 'cut' ? thread.you_cut_in : thread.speaking}
      </div>
    );
  return (
    <Msg from={who.from} name={who.name} indent={who.indent} time={clock(e.at_ms)} head={head} after={mark}>
      {e.thinking != null && thoughtShows(e.thinking) && <ThinkingLine pos={e.pos} head={thoughtFor(e.thinking.ms)} body={e.thinking.text} />}
      {lit !== null && lit.unsaid !== '' ? (
        <div className="b-text vm-lit" data-testid={`thread.lit.${String(e.pos)}`}>
          <span className="vm-said">{lit.said}</span>
          <span className="vm-unsaid">{lit.unsaid}</span>
        </div>
      ) : e.cut === true ? (
        // R40: a reply cut off when its turn failed: the words he read as it streamed, kept at
        // their place and dim (0.7, designer m_18084); the turn's end follows it, '▲ turn
        // interrupted (by main)' dim as the TUI's (rows-news NoticeRow) or the failure's row
        <div className="th-cut" data-testid={`thread.cut.${String(e.pos)}`}>
          <Answer text={text} live={false} still={still} onLink={ctx.onLink} onArtifact={ctx.onArtifact} onPath={ctx.onPath} />
        </div>
      ) : (
        <Answer text={text} live={live} still={still} onLink={ctx.onLink} onArtifact={ctx.onArtifact} onPath={ctx.onPath} />
      )}
    </Msg>
  );
}

/** Thinking under this long shows no row: entry.ts owns it (ambient m_11777). */
export { THOUGHT_MS };

// the row families live in their own files; importers keep reading them here
export { answeredMark, approvalParts, cardFollows, cardLine, fileUrl, imageLabel, toAgentName, withoutImageMarks } from './rows-cards';
export { PendingRow, YouBubble } from './rows-messages';
export { LONG_CHARS, LONG_LINES, foldOf } from './YouBody';
export { NOTICE_MARK } from './rows-news';
export { cutWords, thoughtFor } from './rows-tools';
export { liveTime } from './live-step';
