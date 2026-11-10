// A thread's news rows: landed, PRs, artifacts, scheduled tasks, compaction,
// notices, a task's brief, a handoff, a message not delivered. Owner: amb-win.
// Split out of rows.tsx (pure move).
import { useOpenChange } from '@/review/openChange';
import { useArtifactOpener, useOpenArtifact } from './openArtifact';
import { useRef, useState, type ReactNode } from 'react';
import { Act, AppIcon, ArtName, BriefLine, Button, Fold, iconKindOf, KindIcon, Marker, Press, RowMark, SchedHead, type AppIconName, type IconKind } from '@/ds';
import type { Entry } from '@/proto';
import { thread } from '@/proto/words.gen';
import { useAgents, useArtifacts, useStore } from '@/store';
import { leaves, LEAVES } from '@/artifacts/leave';
import { useFold } from './folds';
import { useFoldHint } from './foldHint';
import { Markdown } from './Markdown';
import { clock, handoffOf, repeatsReport, reportBody, schedHead } from './entry';
import { wakeLine, wakeTail } from './wake';
import { useRouter } from '@/app/router';
import { useCut } from '@/app/side3d/Flyout';
import type { RowCtx } from './rows';
import { openPr } from '@/review/openPr';
import { retryOf, retryWords, typedRetry, type Retry } from './tiers';
/** A news line's mark: what it means, never its color alone. */
export function NewsLine({ mark, tone, testId, children }: { mark: AppIconName; tone: 'ok' | 'err' | 'plain' | 'dim'; testId: string; children: ReactNode }) {
  return (
    <div className={`th-news ${tone}`} data-testid={testId} data-tone={tone}>
      <RowMark icon={mark} tone={tone} />
      <span className="th-news-text">{children}</span>
    </div>
  );
}

/** The machine's own bits of a line in mono (designer m_19832: a sha, a path, a number pair like '+12 −2'), the words sans. Pure. */
export function machineParts(text: string): { text: string; mono: boolean }[] {
  const out: { text: string; mono: boolean }[] = [];
  const re = /[+−-]\d+(?:\s+[+−-]\d+)*/gu;
  let at = 0;
  for (const m of text.matchAll(re)) {
    const i = m.index;
    if (i > at) out.push({ text: text.slice(at, i), mono: false });
    out.push({ text: m[0], mono: true });
    at = i + m[0].length;
  }
  if (at < text.length) out.push({ text: text.slice(at), mono: false });
  return out;
}

/** Words with their machine bits in mono. */
function Machine({ text }: { text: string }) {
  return (
    <>
      {machineParts(text).map((p, i) =>
        p.mono ? (
          <span key={i} className="th-mono">
            {p.text}
          </span>
        ) : (
          p.text
        ),
      )}
    </>
  );
}

/** '✓ perf landed on main · a1b2c3d · 3 files +42 −18', quiet as a wake (H5): the sha opens the commit. */
export function LandedRow({ e, ctx }: { e: Entry; ctx: RowCtx }) {
  const open = useOpenChange(ctx.project);
  const l = e.landed;
  if (l == null) return <Marker>{e.text}</Marker>;
  return (
    <NewsLine mark="check" tone="plain" testId={`thread.landed.${e.pos}`}>
      {l.agent} landed on {l.target} ·{' '}
      <Act
        kind="ref"
        onClick={() => {
          open({ commit: l.sha });
        }}
        testId={`thread.landed.${e.pos}.sha`}
      >
        <span className="th-mono">{l.sha.slice(0, 7)}</span>
      </Act>
      {e.text !== '' && (
        <>
          {' · '}
          <Machine text={e.text} />
        </>
      )}
    </NewsLine>
  );
}

/** A PR's news: its number opens it; failing / done / news said by its mark and words. */
export function PrRow({ e, ctx }: { e: Entry; ctx: RowCtx }) {
  const p = e.pr;
  if (p == null) return <Marker>{e.text}</Marker>;
  // the TUI's PR mark (theme::G_PR '↑', render.rs pr_lines), toned by its state: failing in the
  // error tone, done dim (designer m_19784 (3)); the words after it say the checks, never color alone
  const tone = p.state === 'failing' ? 'err' : p.state === 'done' ? 'dim' : 'plain';
  const mark = 'branch';
  return (
    <NewsLine mark={mark} tone={tone} testId={`thread.pr.${e.pos}`}>
      <Act
        kind="ref"
        onClick={(ev) => {
          // its tab in the panel (⌘: the forge's page), as every PR door (review/openPr.ts)
          openPr(ctx.project, p.url, ev);
        }}
        testId={`thread.pr.${e.pos}.link`}
      >
        PR #{p.number}
      </Act>{' '}
      · {p.text}
    </NewsLine>
  );
}

/** What an agent made: a card that opens it (its page, else the project's artifacts). */
export function MadeRow({ e, ctx }: { e: Entry; ctx: RowCtx }) {
  const artifacts = useArtifacts(ctx.project);
  const goArtifact = useArtifactOpener();
  const openId = useOpenArtifact();
  const m = e.made;
  if (m == null) return <Marker>{e.text}</Marker>;
  // a browser-only one (a PR, a link): 'opens in your browser' in place of its kind and version (designer m_21714)
  const known = artifacts.find((x) => x.id === m.id);
  const out = leaves(known ?? m);
  const meta = out ? LEAVES : [m.kind_word, m.v > 1 ? `v${String(m.v)}` : ''].filter(Boolean).join(' · ');
  return (
    <MadeChip
      kind={iconKindOf(known ?? m)}
      title={m.title}
      meta={meta}
      open={openId === m.id}
      onClick={() => {
        // its tab in the panel (bise fait mac); a browser-only one his browser
        goArtifact(ctx.project, m.id, m);
      }}
      testId={`thread.artifact.${String(e.pos)}`}
    />
  );
}

/** An artifact in the thread (QA §4): the pink chip, its kind's 14 line icon (ds KindIcon, no glyph:
 *  ui-review m_43792) and its title, then its kind and version dim after it. One line, 22 high: a
 *  long title is cut with … and its tip holds it whole, only while cut (his chip-wrap screenshot:
 *  the wrapped words spilled out of the box; designer m_46988's rule for a cut title). */
export function MadeChip({ kind, title, meta, open = false, onClick, testId }: { kind: IconKind; title: string; meta: string; open?: boolean; onClick: () => void; testId: string }) {
  const box = useRef<HTMLDivElement>(null);
  const cut = useCut(box);
  return (
    <div ref={box} className="th-made" data-open={open || undefined}>
      <ArtName onClick={onClick} testId={testId} tip={cut ? title : undefined}>
        <KindIcon kind={kind} size={14} />
        <span className="th-made-title">{title}</span>
      </ArtName>
      {meta !== '' && <span className="th-made-meta">{meta}</span>}
    </div>
  );
}

/**
 * A scheduled task (sb every) set, run or ended, as the TUI's faint '◷ head ▸' row (wire.rs
 * Scheduled): the head is the hub's words as sent; the words it sends open under it (none: no ▸).
 */
export function ScheduledRow({ e, ctx }: { e: Entry; ctx: RowCtx }) {
  const [open, setOpen] = useFold(false, e.pos);
  const s = e.scheduled;
  const head = s?.head ?? e.text;
  const words = s?.words ?? '';
  // one line (H3, designer m_19832): '◷ bise scheduled the p99 check · every 30 min · next 20:11 ▸',
  // cut with …; '▸' only when it has words to open
  return (
    <div className="th-sched" data-testid={`thread.scheduled.${e.pos}`} data-id={s?.id}>
      <div className="th-sched-line">
        <RowMark icon="scheduled" />
        {words === '' ? (
          <SchedHead>{schedHead(head, ctx.agent === 'main' ? null : ctx.agent)}</SchedHead>
        ) : (
          <SchedHead
            open={open}
            onClick={() => {
              setOpen((o) => !o);
            }}
            testId={`thread.scheduled.${e.pos}.head`}
          >
            {schedHead(head, ctx.agent === 'main' ? null : ctx.agent)}
<span className="th-sched-chev" data-open={open || undefined} aria-hidden="true">
              <AppIcon name="right" size={14} />
            </span>
          </SchedHead>
        )}
      </div>
      {open && words !== '' && (
        <div className="th-sched-words" data-testid={`thread.scheduled.${e.pos}.words`}>
          {words}
        </div>
      )}
    </div>
  );
}

/**
 * The summary its context was compacted to: one folded line; a click or ⌃O opens the whole summary
 * as markdown (the hub sends it whole; an older transcript's 40-char preview shows as it came).
 */
export function CompactedRow({ e, ctx }: { e: Entry; ctx: RowCtx }) {
  const [open, setOpen] = useFold(false, e.pos);
  const hint = useFoldHint();
  const toggle = () => {
    setOpen((o) => !o);
  };
  return (
    <div className="th-compact" data-testid={`thread.compacted.${e.pos}`}>
      <Fold open={open} onToggle={e.text === '' ? undefined : toggle} hint={e.text === '' ? undefined : hint} testId={`thread.compacted.${e.pos}.head`}>
        {thread.compacted}
      </Fold>
      {open && e.text !== '' && (
        <div className="th-compact-body" data-testid={`thread.compacted.${e.pos}.body`}>
          <Markdown text={e.text} onLink={ctx.onLink} onArtifact={ctx.onArtifact} onPath={ctx.onPath} />
        </div>
      )}
    </div>
  );
}

/**
 * event-wake (designer's page): a watch's end, the hub's own line ('· cargo test ended · rc 101 ·
 * after 3m10s'), dim, only a non-zero rc in the error tone; '–' when he stopped waiting; its last
 * output lines folded ('▸ its last 3 lines', ⌘E / a click opens them, mono).
 */
export function WakeRow({ e, ctx }: { e: Entry; ctx: RowCtx }) {
  const [open, setOpen] = useFold(false, e.pos);
  const hint = useFoldHint();
  const line = wakeLine(e);
  // another agent's wake, main's copy of it (amb-feed 11e5fbbc wake.agent): tier 4, dim, never red,
  // named for whom (designer m_19383: nothing addressed to another agent in ink)
  const other = e.wake?.agent != null && e.wake.agent !== '' && e.wake.agent !== ctx.agent ? e.wake.agent : null;
  const mark = line.mark;
  const parts = other === null ? line.parts : [{ text: thread.wake_of({ agent: other, head: line.parts[0]?.text ?? '' }), err: false }, ...line.parts.slice(1).map((p) => ({ ...p, err: false }))];
  const tail = e.wake?.tail ?? [];
  const words = wakeTail(tail.length);
  return (
    <div className="th-wake" data-for={other ?? undefined} data-testid={`thread.wake.${String(e.pos)}`}>
      <div className="th-wake-head">
        {/* a failed command ('rc 101') reads as one: the alert icon in err, not a 2 px dot (ui-review m_43900) */}
        {mark === '–' ? <RowMark icon="fold" /> : parts.some((p) => p.err) ? <RowMark icon="alert" tone="err" label="error" /> : <RowMark icon="dot" />}
        {parts.map((p, i) => (
          <span key={i} className={p.err ? 'th-wake-err' : undefined} data-err={p.err || undefined}>
            {/* a flex item drops its leading space: the one before '·' is kept unbreakable */}
            {i > 0 && '\u00a0· '}
            {p.text}
          </span>
        ))}
        {words !== null && (
          <Fold
            open={open}
            onToggle={() => {
              setOpen((o) => !o);
            }}
            hint={hint}
            testId={`thread.wake.${String(e.pos)}.fold`}
          >
            {words}
          </Fold>
        )}
      </div>
      {open && tail.length > 0 && <pre className="th-wake-out">{tail.join('\n')}</pre>}
    </div>
  );
}

/** A notice's mark by level (the TUI's: info dim, warn ▲, err ✗), as line icons (ui-review m_43792): the tone is said by the icon and words, never by color alone. */
export const NOTICE_MARK = { info: null, warn: 'alert', err: 'close', unknown: null } as const satisfies Record<string, AppIconName | null>;

/** A notice's mark, its level's icon and tone, named for VoiceOver. An info notice ('this repo lands on
 *  main (trunk)') has no mark, its 16 column kept empty: a dot read as a stray '·' (ui-review m_43949). */
export function NoticeMark({ level }: { level: keyof typeof NOTICE_MARK }) {
  const icon = NOTICE_MARK[level];
  if (icon === null) return <span className="b-rowmark" aria-hidden="true" />;
  return <RowMark icon={icon} tone={level === 'err' ? 'err' : 'plain'} label={level === 'err' ? 'error' : 'warning'} />;
}

export function NoticeRow({ e, ctx, retries }: { e: Entry; ctx: RowCtx; retries?: readonly string[] | undefined }) {
  const level = e.notice?.level ?? 'info';
  const text = e.notice?.text ?? e.text;
  // the hub's typed retry first (dd17152f), else its words (an older hub)
  const retry = e.notice?.retry != null ? typedRetry(e.notice.retry) : retryOf(text);
  if (retry !== null) return <RetryRow pos={e.pos} retry={retry} tries={retries ?? [text]} />;
  // a main's handoff (designer's v6 (3)): '→ perf · product page speed', opens its thread
  const handoff = level === 'info' ? handoffOf(text) : null;
  if (handoff !== null) return <HandoffRow pos={e.pos} to={handoff.agent} brief={handoff.brief} ctx={ctx} />;
  // 'session restored · 14:02', one row centred like a day separator (ambient m_11777)
  if (level === 'info' && text.startsWith(thread.restored)) {
    return (
      <div className="th-day th-restored" data-testid={`thread.notice.${e.pos}`}>
        <span>{e.at_ms > 0 ? thread.restored_at({ at: clock(e.at_ms) }) : thread.restored}</span>
      </div>
    );
  }
  // '▲ turn interrupted' / '▲ turn interrupted by main' (the catalog's words): the TUI draws it dim,
  // a stop he or main asked for, read under a cut reply as stopped, not as a failure
  const interrupted = level === 'warn' && (text === thread.turn_interrupted || text.startsWith(thread.turn_interrupted_by({ by: '' })));
  return (
    <div className={`th-notice ${level}${interrupted ? ' interrupted' : ''}`} data-level={level} data-testid={`thread.notice.${e.pos}`}>
      <NoticeMark level={level} />
      <span>{text}</span>
    </div>
  );
}

/** 'bise started gift-ui · <objective>': the task's brief as one dim row; a click unfolds the whole brief under a rail. */
export function BriefRow({ pos, name, objective, body, ctx }: { pos: number; name: string; objective: string; body: string; ctx: RowCtx }) {
  const [open, setOpen] = useFold(false, pos);
  return (
    <div className="th-brief" data-open={open || undefined} data-testid={`thread.brief.${String(pos)}`}>
      <BriefLine
        open={open}
        onClick={() => {
          setOpen((o) => !o);
        }}
      >
        bise started {name}
        {objective !== '' && ` · ${objective}`}
      </BriefLine>
      {open && (
        <div className="th-brief-body" data-testid={`thread.brief.${String(pos)}.body`}>
          <Markdown text={body} onLink={ctx.onLink} onArtifact={ctx.onArtifact} onPath={ctx.onPath} />
        </div>
      )}
    </div>
  );
}

/** '✚ new agent perf · product page speed' (H11, designer m_19832): a quiet row, its role (else the brief); its name opens its thread. */
function HandoffRow({ pos, to, brief, ctx }: { pos: number; to: string; brief: string; ctx: RowCtx }) {
  const router = useRouter();
  const a = useAgents(ctx.project).find((x) => x.name === to);
  const role = a !== undefined && a.purpose !== '' ? a.purpose : brief;
  return (
    <NewsLine mark="plus" tone="plain" testId={`thread.handoff.${String(pos)}`}>
      new agent{' '}
      <Act
        kind="ref"
        onClick={() => {
          router.go({ screen: 'thread', project: ctx.project, agent: to });
        }}
        testId={`thread.handoff.${String(pos)}.name`}
      >
        {to}
      </Act>
      {role !== '' && ` · ${role}`}
    </NewsLine>
  );
}

/** His message didn't reach the agent (BISE-86): send it again, or drop it from view. */
export function NotDeliveredRow({ e, ctx }: { e: Entry; ctx: RowCtx }) {
  const store = useStore();
  const [gone, setGone] = useState<'sent' | 'dropped' | null>(null);
  const nd = e.not_delivered;
  const to = nd?.to ?? ctx.agent;
  const text = nd?.text ?? e.text;
  if (gone === 'dropped') return null;
  const again = () => {
    store.send({ project: ctx.project, agent: to, text, mode: 'now' });
    setGone('sent');
  };
  const drop = () => {
    setGone('dropped');
  };
  return (
    <div
      className="th-notdelivered"
      data-testid={`thread.notdelivered.${e.pos}`}
      tabIndex={-1}
      onKeyDown={(k) => {
        if (gone !== null) return;
        if (k.key === 'Enter') again();
        else if (k.key === 'Escape') drop();
        else return;
        k.preventDefault();
        k.stopPropagation();
      }}
    >
      <div className="th-notdelivered-head">
        <span aria-label="error">✗</span> {thread.not_delivered({ to })}
      </div>
      <div className="th-notdelivered-text">{text}</div>
      {gone === 'sent' ? (
        <div className="th-waits">{thread.sent_again}</div>
      ) : (
        <div className="th-notdelivered-acts">
          <Button kind="ghost" onClick={again} testId={`thread.notdelivered.${e.pos}.again`}>
            ⏎ {thread.send_again}
          </Button>
          <Button kind="ghost" onClick={drop} testId={`thread.notdelivered.${e.pos}.drop`}>
            esc {thread.drop}
          </Button>
        </div>
      )}
    </div>
  );
}

/**
 * An agent's report (tui-parity H8, designer m_19832): one line, never two loud copies. When the
 * message under it says the same words, one quiet row '✓ report ▸' (the message stays the reply);
 * else its status line, cut with …, and '▸ report' when it has more to read. Either opens in place.
 */
export function ReportRow({ pos, line, text, next, ctx, waits }: { pos: number; line: string; text: string; next: Entry | undefined; ctx: RowCtx; waits: boolean }) {
  const [open, setOpen] = useFold(false, pos);
  const body = reportBody(text);
  const dup = next !== undefined && (next.kind === 'agent' || next.kind === 'from_agent' || next.kind === 'to_agent') && repeatsReport(body, next.text);
  const more = dup || body.includes('\n');
  const toggle = () => {
    setOpen((o) => !o);
  };
  const whole = open && body !== '' && (
    <div className="th-report-body" data-testid={`thread.report.${String(pos)}.body`}>
      <Markdown text={body} onLink={ctx.onLink} onArtifact={ctx.onArtifact} onPath={ctx.onPath} />
    </div>
  );
  // the status line's '✓ ' / '✗ ' (reportStatus, the TUI's words) drawn as its line icon
  const mark: AppIconName | null = line.startsWith('✓ ') ? 'check' : line.startsWith('✗ ') ? 'close' : null;
  if (dup) {
    // its words stay (designer, ambient-lead m_20040): '✓ <summary>… ▸ report', sans 13, text-2
    return (
      <div className="th-report" data-dup="true" data-testid={`thread.report.${String(pos)}`}>
        <Press look="th-report-fold" aria-expanded={open} onClick={toggle} data-testid={`thread.report.${String(pos)}.fold`}>
          <RowMark icon={mark ?? 'dot'} tone={mark === 'close' ? 'err' : 'dim'} />
          <span className="th-report-words">{body.split('\n')[0] ?? ''}</span>
          <span className="th-report-more">
            <span className="th-sched-chev" data-open={open || undefined} aria-hidden="true">
              <AppIcon name="right" size={14} />
            </span>
            {thread.report_fold}
          </span>
        </Press>
        {whole}
      </div>
    );
  }
  return (
    <div className="th-report" data-waits={waits || undefined} data-testid={`thread.report.${String(pos)}`}>
      <div className="th-report-line">
        {/* its status mark a line icon (ui-review m_43868), the words after it */}
        <Marker>
          {mark !== null && <RowMark icon={mark} tone={mark === 'close' ? 'err' : 'dim'} />}
          {mark === null ? line : line.slice(2)}
        </Marker>
        {more && (
          <Act onClick={toggle} testId={`thread.report.${String(pos)}.more`}>
            <span className="th-sched-chev" data-open={open || undefined} aria-hidden="true">
              <AppIcon name="right" size={14} />
            </span>
            {thread.report_fold}
          </Act>
        )}
      </div>
      {whole}
    </div>
  );
}

/**
 * A model call's retries as one row that updates in place (tui-parity H9, designer m_19832): '▲ model
 * call failed · retry 3 of 10 in 4 s ▸', the ▲ dim and the words text-2; in --c-err only when the last
 * try failed. Each try's full words open under its ▸.
 */
function RetryRow({ pos, retry, tries }: { pos: number; retry: Retry; tries: readonly string[] }) {
  const [open, setOpen] = useFold(false, pos);
  // red only once the last try failed; a run that went through dims (amb-feed: end 'recovered')
  const done = retry.end === 'recovered' || retry.end === 'unknown';
  const failed = !done && retry.next === undefined;
  return (
    <div className="th-retry" data-failed={failed || undefined} data-done={done || undefined} data-testid={`thread.notice.${String(pos)}`}>
      <Press
        look="th-retry-line"
        aria-expanded={open}
        onClick={() => {
          setOpen((o) => !o);
        }}
        data-testid={`thread.notice.${String(pos)}.retry`}
      >
        <RowMark icon="alert" tone={failed ? 'err' : 'dim'} label={failed ? 'error' : 'warning'} />
        <span className="th-retry-words">{retryWords(retry)}</span>
        <span className="th-sched-chev" data-open={open || undefined} aria-hidden="true">
          <AppIcon name="right" size={14} />
        </span>
      </Press>
      {open && (
        <ol className="th-retry-tries" data-testid={`thread.notice.${String(pos)}.tries`}>
          {tries.map((t, i) => (
            <li key={i}>{t}</li>
          ))}
        </ol>
      )}
    </div>
  );
}
