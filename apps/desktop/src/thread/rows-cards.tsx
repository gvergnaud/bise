// A thread's card rows: cards and their answers, approvals, 'you answered' rows,
// image chips, and one agent's message to another. Owner: amb-win. Split out of
// rows.tsx (pure move).
import { useContext, useEffect, useState, type ReactNode } from 'react';
import { useFold } from './folds';
import { Act, Answered, ArtName, CardLine, Chip, Marker, Press, RowMark, type AppIconName } from '@/ds';
import { openInbox } from '@/inbox/Block';
import { ArtWords } from '@/inbox/artWords';
import type { Entry, EntryCard } from '@/proto';
import { common, thread } from '@/proto/words.gen';
import { RouterContext } from '@/app/router';
import { answeredFromWords, answerPos, approvalTarget, cardTarget, openAnswer, type AnswerAt } from './answerlink';
import { useAnsweredFrom, useEntry, useProjects, useStore, useThreadOrder } from '@/store';
import { Markdown } from './Markdown';
import { clock, lastOfRun, reportStatus } from './entry';
import { shownName } from '@/agents/model';
import { YouBubble } from './rows-messages';
import { PartChips, partImageWords } from './YouBody';
import { labelN } from '@/composer/partsback';
import type { Part } from '@/proto';
import { NewsLine, ReportRow } from './rows-news';
import type { RowCtx } from './rows';
import { drawnParts, pageAnswer, pageOf, youView } from './parts';
import { forwardLine, forwardOf, forwardTip, type Forward } from './forward';
/** The hub's words about him in the 3rd person, said to him: 'he showed it' -> 'you showed it'. Pure. */
export function toYou(words: string): string {
  return words.replace(/\bhe\b/gu, 'you').replace(/\bHe\b/gu, 'You').replace(/\bhis\b/gu, 'your').replace(/\bhim\b/gu, 'you');
}

/** Main answered an agent's question for him: the question, the answer, and why, dim. */
export function AnsweredRow({ e }: { e: Entry }) {
  const a = e.answered;
  if (a == null) return <Marker>{e.text}</Marker>;
  // the hub's why 'he answered': his own answer, never 'he' (designer m_21561): 'you answered <agent>'
  const mine = a.why === 'he answered';
  return (
    <div className="th-answered" data-testid={`thread.answered.${e.pos}`}>
      <NewsLine mark="check" tone="plain" testId={`thread.answered.${e.pos}.head`}>
        {mine ? thread.you_answered_q({ agent: a.agent, question: a.question }) : thread.main_answered_q({ main: shownName('main'), agent: a.agent, question: a.question })}
        <b>{a.answer}</b>
      </NewsLine>
      {a.why !== '' && !mine && <div className="th-news-note">{toYou(a.why)}</div>}
      <ImageChips images={a.images ?? []} testId={`thread.answered.${e.pos}`} />
    </div>
  );
}

/**
 * R36: the images he pasted with an answer, each the one image chip of the window (the composer's,
 * the spotlight's, ⌘I's): its 16 px thumbnail read from his source path and its label ('[Image #1]');
 * never the hub's marker or a path in the words.
 */
export function ImageChips({ images, testId }: { images: readonly { name: string; path: string }[]; testId: string }) {
  if (images.length === 0) return null;
  return (
    <div className="th-images">
      {images.map((img, i) => (
        <ImageChip key={`${img.path}#${String(i)}`} path={img.path} i={i} testId={`${testId}.image.${String(i + 1)}`} />
      ))}
    </div>
  );
}

/**
 * One image he sent, the signed part chip everywhere (his bubble's, the composer's, the spotlight's;
 * designer m_18135, m_21561): 'image 1' in ink, 1 px line, its 16 px thumbnail or the no-file glyph;
 * never the '[Image #n]' tag in the accent.
 */
export function ImageChip({ path, i, testId }: { path?: string; i: number; testId: string }) {
  // 'image <n>', its order in this answer: never the store's file name (a sha .png on real data, R36)
  return (
    <Chip kind="part" of="image" {...(path === undefined ? {} : { title: path })} thumb={<Thumb {...(path === undefined ? {} : { path })} />} testId={testId}>
      {partImageWords('', i + 1)}
    </Chip>
  );
}

/**
 * A chip's 16 px image slot: his file's thumbnail, or, when there's no file (an older hub's raw
 * line) or it's gone, the same slot with a faint image glyph, so every chip reads the same
 * (designer m_15943).
 */
export function Thumb({ path, onGone }: { path?: string; onGone?: () => void }) {
  const [gone, setGone] = useState(false);
  useEffect(() => {
    if (gone) onGone?.();
  }, [gone, onGone]);
  if (path == null || path === '' || gone) {
    return (
      <span className="thumb none" aria-hidden="true">
        <svg viewBox="0 0 16 16" width="16" height="16"><rect x="2.5" y="3.5" width="11" height="9" rx="1.5" fill="none" stroke="currentColor" /><circle cx="6" cy="6.5" r="1" fill="currentColor" /><path d="M3 11.5l3.5-3 2.5 2 2-1.5 2.5 2" fill="none" stroke="currentColor" /></svg>
      </span>
    );
  }
  return <img className="thumb" src={fileUrl(path)} alt="" onError={() => { setGone(true); }} />;
}

/** An answer's n-th image chip label: '[Image #n]', whatever the file is called. Pure. */
export function imageLabel(i: number): string {
  return `[Image #${String(i + 1)}]`;
}

/** A local file's URL for an img (the window's CSP allows file:). Pure. */
export function fileUrl(path: string): string {
  return `file://${path.split('/').map(encodeURIComponent).join('/')}`;
}

/**
 * His answer to main's own card, in the same order as every '✓ you answered' row (designer
 * m_15420): line 1 what was asked, line 2 his words, line 3 a chip for each '[Image #n]' he sent.
 */
export function CardAnsweredRow({ pos, a, at, timeAlways }: { pos: number; a: { card: number; question: string; words: string }; at: number; timeAlways: boolean }) {
  const images = a.words.match(/\[Image #\d+\]/gu) ?? [];
  const words = withoutImageMarks(a.words);
  const id = `thread.cardanswer.${String(pos)}`;
  // something he said: on his side, '✓ you answered main · <question>' dim over his bubble
  // (messages-right); one head for every answered card, terminal and window (designer m_20985)
  return (
    <div className="th-approval" data-card={a.card}>
      <YouBubble
        head={<AnsweredHead ok>{thread.answered_card({ who: shownName('main'), question: a.question })}</AnsweredHead>}
        under={at > 0 ? clock(at) : undefined}
        timeAlways={timeAlways}
        testId={id}
      >
        {words === '' && images.length === 0 ? undefined : (
          <>
            {words !== '' && <div data-testid={`${id}.words`}>{words}</div>}
            {images.length > 0 && (
              <div className="th-images">
                {images.map((_, i) => (
                  <ImageChip key={i} i={i} testId={`${id}.image.${String(i + 1)}`} />
                ))}
              </div>
            )}
          </>
        )}
      </YouBubble>
    </div>
  );
}

/**
 * What he did on a page (page-answer-row, architect m_20833): the hub's message to the agent
 * carries a typed page part; the row is '✓ you answered main · <what the card asked>' over his
 * words, the TUI's row word for word (bise-proto words::page_answer), never the hub's template.
 */
export function PageAnswerRow({ e, ctx, next }: { e: Entry; ctx: RowCtx; next: Entry | undefined }) {
  const p = pageOf(e.parts);
  if (p === null) return <Marker>{e.text}</Marker>;
  // the agent's name as the thread draws its author (a project's main is 'bise'; designer m_21035)
  const a = pageAnswer({ ...p, to: shownName(p.to) });
  // one answer's typed parts (card-image-parts): his images as his own bubble's chips, never a tag
  const only = p.notes.length === 1 ? p.notes[0] : undefined;
  const images = only?.parts != null && only.parts.length > 0 ? youView(only.parts).images : [];
  // the page's name pink and clickable, its view (desktop-agents-bar §5)
  const on =
    a.on === '' ? null : (
      <>
        {' '}
        <ArtName
          onClick={() => {
            ctx.onArtifact(p.page);
          }}
          testId={`thread.pageanswer.${String(e.pos)}.page`}
        >
          {a.on}
        </ArtName>
      </>
    );
  const head = (
    <>
      {a.head}
      {on}
      {a.asked !== '' ? ` · ${a.asked}` : ''}
    </>
  );
  return (
    <div className="th-approval" data-ok>
      <YouBubble head={<AnsweredHead ok>{head}</AnsweredHead>} under={e.at_ms > 0 ? clock(e.at_ms) : undefined} timeAlways={lastOfRun(e, next)} testId={`thread.pageanswer.${String(e.pos)}`}>
        {a.words === '' && images.length === 0 ? undefined : (
          <>
            {a.words !== '' && (
              <div className="th-you-words" data-testid={`thread.pageanswer.${String(e.pos)}.words`}>
                {a.words}
              </div>
            )}
            <ImageChips images={images} testId={`thread.pageanswer.${String(e.pos)}`} />
          </>
        )}
      </YouBubble>
    </div>
  );
}

/** '✓ you answered gift-ui': the dim head over his answer's bubble, the mark saying it with no color alone. */
/** The mark of his answer, a line icon (ui-review m_43792): check allowed, close said no, a dot when the hub doesn't say. Pure. */
export function answeredMark(ok: boolean | null | undefined): AppIconName {
  return ok === true ? 'check' : ok === false ? 'close' : 'dot';
}

function AnsweredHead({ ok, children }: { ok: boolean | null | undefined; children: ReactNode }) {
  // ✓ only when the hub says allowed, ✕ only when it says no, a neutral '·' when it doesn't say
  // (the real core leaves ok empty today: an allowed answer must never read ✕; lead m_16314)
  const mark = answeredMark(ok);
  return (
    <>
      <span className={ok === true ? 'th-you-mark ok' : 'th-you-mark'} data-ok={ok === true ? 'yes' : ok === false ? 'no' : 'unknown'}>
        <RowMark icon={mark} />
      </span>
      {children}
    </>
  );
}

/**
 * The hub's approval text split into its head and his words: 'you answered main: this one' ->
 * head 'you answered main', words 'this one'; a text with no words is all head, and so is a
 * decision ('you said no to perf: rm -rf target': its tail is what he decided on, not his words;
 * his note goes in the bubble), word for word as the TUI says it (lead m_18803). Pure.
 */
export function approvalParts(text: string): { head: string; words: string } {
  const m = /^(you answered [^:]+?):\s*([\s\S]+)$/u.exec(text.trim());
  if (m === null) return { head: text.trim(), words: '' };
  return { head: (m[1] ?? '').trim(), words: (m[2] ?? '').trim() };
}

/** The head with the name the thread draws: 'you answered main' -> 'you answered bise' (designer m_21658). Pure. */
export function shownHead(head: string): string {
  return head.replace(/^(you answered )(\S+)/u, (_, lead: string, who: string) => lead + shownName(who));
}

/**
 * His words without the '[Image #n]' marks the hub kept in them: the chips under the row are the
 * images, so the words never say them a second time (designer m_15891). Pure.
 */
export function withoutImageMarks(words: string): string {
  return words.replace(/\[Image #\d+\]/gu, ' ').replace(/\s+/gu, ' ').replace(/\s+([:,.])/gu, '$1').trim();
}

/**
 * The line under his answer to a card: 'answered from bise · 09:55' (its tip says it whole) when
 * this window sent it from another thread (answer-in-thread A, designer m_24628), else the time.
 */
function AnsweredUnder({ project, here, card, at, testId }: { project: string; here: { project: string; agent: string }; card: number | null | undefined; at: number; testId: string }) {
  const rows = useProjects();
  const made = useAnsweredFrom(project, card ?? -1);
  const time = clock(at);
  const home = rows.find((r) => r.home)?.project ?? '';
  const said = made === null ? null : answeredFromWords(made.from, here, home, (p) => rows.find((r) => r.project === p)?.name ?? p, time === '' ? clock(made.at_ms) : time);
  if (said === null) return time === '' ? null : <>{time}</>;
  return (
    <span data-tip={said.tip} data-testid={testId}>
      {said.words}
    </span>
  );
}

/** His answer to an approval, folded: allowed or said no, and his words under it. */
export function ApprovalRow({ e, next, ctx }: { e: Entry; next: Entry | undefined; ctx: RowCtx }) {
  const a = e.approval;
  if (a == null) return <Marker>{e.text}</Marker>;
  // answered-link: his answer to another agent's card (main's thread): the head opens it there
  const at = approvalTarget(ctx.project, ctx.agent, e);
  // msg-parts: his answer's own parts (the note's), when the hub sent them: his words and images
  // from them, never from the templated text (no tag shows); the head stays the hub's words
  const drawn = drawnParts(e.parts);
  const view = drawn !== null ? youView(drawn) : null;
  // his images ride only in its parts since main 9ae8e01f (ApprovalFold.images is gone)
  const images = view?.images ?? [];
  // the chips draw his images: their marks leave the words (once, never twice)
  const chips = images.length > 0;
  const say = (s: string): string => (chips ? withoutImageMarks(s).replace(/:$/u, '') : s);
  const parts = approvalParts(say(a.text));
  const note = view !== null ? view.words : a.note == null ? '' : say(a.note);
  // his words (after the head's ':' or the note), then the image chips under them, in his bubble;
  // with parts, the words are the parts' alone (the head's tail is the same answer, templated)
  const words = view !== null ? note : [parts.words, note].filter((w) => w !== '').join('\n');
  return (
    <div className="th-approval" data-ok={a.ok}>
      <YouBubble
        head={
          at === null ? (
            <AnsweredHead ok={a.ok}>{shownHead(parts.head)}</AnsweredHead>
          ) : (
            <AnswerLink at={at} here={ctx.agent} testId={`thread.approval.${e.pos}.link`}>
              <AnsweredHead ok={a.ok}>{shownHead(parts.head)}</AnsweredHead>
            </AnswerLink>
          )
        }
        under={<AnsweredUnder project={ctx.project} here={{ project: ctx.project, agent: ctx.agent }} card={a.card} at={e.at_ms} testId={`thread.approval.${e.pos}.from`} />}
        timeAlways={lastOfRun(e, next) || a.card != null}
        testId={`thread.approval.${e.pos}`}
      >
        {words === '' && !chips ? undefined : (
          <>
            {words !== '' && <div className="th-you-words" data-testid={`thread.approval.${e.pos}.words`}>{words}</div>}
            <ImageChips images={images} testId={`thread.approval.${e.pos}`} />
          </>
        )}
      </YouBubble>
    </div>
  );
}

/**
 * What this agent sent another (amb-feed eb752406, bar A.3): '→ main' and
 * its words, indented as an exchange; one that waits for the reply is its
 * question (the TUI's '✉ gift-ui → main'). When main opened a card for it,
 * that card follows with only its options and says it waits for him; with
 * no card, the question waits for main (ambient m_10689: one copy of it).
 */
export function ToAgentRow({ e, ctx, chained = false }: { e: Entry; ctx: RowCtx; chained?: boolean }) {
  const to = e.to ?? 'main';
  const next = useNeighbor(ctx.tkey, e.pos, 1);
  const carded = e.asks === true && next !== undefined && cardFollows(e, next);
  // its report to bise ('[report: blocked] …'): a status line, never the tag (designer D14 in agent
  // threads, cb1f2b44); a blocked one the question card right under it already says: nothing
  const report = reportStatus(e.text);
  if (report !== null) {
    if (carded) return null;
    return <ReportRow pos={e.pos} line={report} text={e.text} next={next} ctx={ctx} waits={report.startsWith('blocked')} />;
  }
  return (
    <AgentNote
      pos={e.pos}
      from={shownName(ctx.agent)}
      to={shownName(to)}
      text={e.text}
      parts={e.parts}
      ctx={ctx}
      testId={`thread.to.${String(e.pos)}`}
      chip={!chained || e.asks === true}
      tag={
        e.asks === true && !carded ? (
          <span className="th-waits main" data-testid={`thread.asks.${String(e.pos)}`}>
            waits for {shownName(to)}
          </span>
        ) : undefined
      }
    />
  );
}

/** How many lines of an agent's message show folded (designer's desktop-notices (a): 2). */
const NOTE_LINES = 2;
/** Past this many characters the two folded lines are likely cut: '… ▸' says there is more. */
const NOTE_CHARS = 160;

/**
 * A message between agents (designer's thread tiers m_19387, tier 4): never body prose in the thread,
 * whoever wrote it. One dim mono row: the ✉ chip 'sender → receiver' (the sender in ink, the rest
 * text-3), then at most two rows of its words at +16 px, then '… ▸' when there is more; a click on
 * the chip or on '… ▸' opens it whole (markdown), again folds it. `chip` false: the message before
 * went the same way and its chip says it for both (designer m_19784 (2)); the words still open it.
 */
export function AgentNote({ pos, from, to, text: said, parts, ctx, testId, tag, chip = true }: { pos: number; from: string; to: string; text: string; parts?: Entry['parts']; ctx: RowCtx; testId: string; tag?: ReactNode; chip?: boolean }) {
  // send-parts: an agent's attachments (`sb send --attach`) are his message's chips under its
  // words, never their markers; a message without any: its text as ever
  const drawn = drawnParts(parts);
  // msg-forward (designer m_41546): a forward's note is its words; the original goes quoted under
  // its '↪' line (ForwardBody), never the header the model reads
  const fwd = forwardOf(drawn);
  const view = drawn !== null && fwd === null ? youView(drawn) : null;
  const attached = view !== null && (view.images.length > 0 || view.files.length > 0);
  const text = fwd !== null ? youView(fwd.note).words : attached ? view.words : said;
  // ctrl+o / ⌘E open and close it with every fold; a click opens it in place (desktop-notices (c))
  const [open, setOpen] = useFold(false, pos);
  const lines = text.split('\n').filter((l) => l.trim() !== '');
  const more = lines.length > NOTE_LINES || text.length > NOTE_CHARS;
  const toggle = () => {
    setOpen((o) => !o);
  };
  return (
    <div className="th-note" data-open={open || undefined} data-fwd={fwd !== null || undefined} data-testid={testId} data-pos={pos}>
      {(chip || tag !== undefined) && (
        <div className="th-note-head">
          {chip && (
            <Press look="th-note-chip" aria-expanded={open} onClick={toggle} data-testid={`${testId}.chip`}>
              {/* the spaces read in its text; the flex gap draws them */}
              <span className="th-note-faint" aria-hidden="true">
                ✉
              </span>{' '}
              <span className="th-note-from">{from}</span>{' '}
              <span className="th-note-faint">→</span>{' '}
              <span className="th-note-to">{to}</span>
            </Press>
          )}
          {tag}
        </div>
      )}
      {/* desktop-notices (a): its words sans 13 text-3 at +16, two lines, then '… ▸' when there's more */}
      {text !== '' &&
        (open ? (
          <div className="th-note-text" data-testid={`${testId}.text`}>
            <Markdown text={text} onLink={ctx.onLink} onArtifact={ctx.onArtifact} onPath={ctx.onPath} />
          </div>
        ) : (
          <div className="th-note-line" data-testid={`${testId}.text`}>
            {/* explicit lines: one row each, cut with …; a single paragraph: two wrapped lines. The
                last row ends with '… ▸' when there is more (designer m_20621 (2)) */}
            {(lines.length <= 1 ? [lines[0] ?? text] : lines.slice(0, NOTE_LINES)).map((l, i, shown) => (
              <div key={i} className={lines.length <= 1 ? 'th-note-row para' : 'th-note-row'}>
                <span className="th-note-text">{l}</span>
                {more && i === shown.length - 1 && (
                  <Act onClick={toggle} testId={`${testId}.more`}>
                    … ▸
                  </Act>
                )}
              </div>
            ))}
          </div>
        ))}
      {attached && <PartChips view={view} ctx={ctx} id={`thread.you.${String(pos)}`} named />}
      {fwd !== null && <ForwardBody f={fwd} to={to} open={open} onToggle={toggle} ctx={ctx} pos={pos} testId={testId} />}
    </div>
  );
}

/**
 * A forward's original under its note (msg-forward, designer m_41546): '↪ forwarded from you → main ·
 * 12:26' faint (the id in its tip), then the original quoted: a 2 px faint rule on its left, its
 * words in text-2, its quotes as lines, its pastes, images and files as his bubble's chips. Folded,
 * its words on two lines then '… ▸'; open, whole. The TUI's: rust/tui/src/forwarded.rs.
 */
function ForwardBody({ f, to, open, onToggle, ctx, pos, testId }: { f: Forward; to: string; open: boolean; onToggle: () => void; ctx: RowCtx; pos: number; testId: string }) {
  const view = youView(f.original);
  const words = foldedWords(f.original);
  const lines = words.split('\n').filter((l) => l.trim() !== '');
  const more = lines.length > NOTE_LINES || words.length > NOTE_CHARS;
  const id = `thread.fwd.${String(pos)}`;
  return (
    <div className="th-fwd" data-testid={`${testId}.fwd`}>
      <div className="th-fwd-line" title={forwardTip(f.fwd, to)} data-testid={`${testId}.fwd.line`}>
        {forwardLine(f.fwd, (ms) => clock(ms))}
      </div>
      <div className="th-fwd-orig" data-testid={`${testId}.fwd.original`}>
        {view.quotes.map((q, i) => (
          <div key={i} className="th-fwd-quote" data-testid={`${testId}.fwd.quote.${String(i + 1)}`}>
            ❝ {q.text.split('\n')[0] ?? ''}
            <span className="th-note-faint">{`\u00a0· ${shownName(q.from)}`}</span>
          </div>
        ))}
        {/* its words with each chip's token in place ('▣ image 1'), open or folded alike (designer
            m_41711): never the composer's inline chip, whose thumb is his own bubble's */}
        {words !== '' &&
          (open ? (
            <div className="th-fwd-words" data-testid={`${testId}.fwd.words`}>
              <Markdown text={words} onLink={ctx.onLink} onArtifact={ctx.onArtifact} onPath={ctx.onPath} />
            </div>
          ) : (
            <div className="th-fwd-words folded" data-testid={`${testId}.fwd.words`}>
              <span className="th-fwd-clamp">{lines.slice(0, NOTE_LINES).join('\n')}</span>
              {more && (
                <Act onClick={onToggle} testId={`${testId}.fwd.more`}>
                  … ▸
                </Act>
              )}
            </div>
          ))}
        <PartChips view={view} ctx={ctx} id={id} named />
      </div>
    </div>
  );
}

/**
 * A forward's original folded, on its lines: its words with each chip's token where it stood ('▣
 * image 1', '▯ notes.csv', '▤ 1', a numbered quote's '❝ 2'), one space apart, as his bubble places
 * them (designer m_41661). Pure.
 */
export function foldedWords(parts: readonly Part[]): string {
  let images = 0;
  const token = (p: Part): string => {
    switch (p.kind) {
      case 'text':
        return p.text;
      case 'plugin':
        return `@${p.name}`;
      case 'image':
        images += 1;
        return ` ▣ ${partImageWords(p.name, labelN(p.name) ?? images)} `;
      case 'paste':
        return ` ▤ ${String(p.n)} `;
      case 'file':
        return (p.name ?? '') === '' ? '' : ` ▯ ${p.path.split('/').at(-1) ?? p.path} `;
      case 'quote':
        return p.n == null ? '' : ` ❝ ${String(p.n)} `;
      default:
        return '';
    }
  };
  return parts
    .map(token)
    .join('')
    .replace(/[ \t]{2,}/gu, ' ')
    .replace(/[ \t]+\n/gu, '\n')
    .replace(/\n[ \t]+/gu, '\n')
    .trim();
}

/** A card entry right after the ask it answers (same words): the ask carries the question. Pure. */
export function cardFollows(ask: Entry, card: Entry): boolean {
  return ask.kind === 'to_agent' && ask.asks === true && card.kind === 'card' && card.card != null && card.card.question.trim() === ask.text.trim();
}

/** The entry before (-1) or after (+1) this one in the thread's order. */
function useNeighbor(tkey: string, pos: number, dir: -1 | 1): Entry | undefined {
  const { order } = useThreadOrder(tkey);
  const i = order.indexOf(pos);
  const at = i < 0 ? undefined : order[i + dir];
  return useEntry(tkey, at ?? -1);
}

/** A to_agent row's name: '✉ gift-ui → main' when it asks, else 'gift-ui → main'. Pure. */
export function toAgentName(from: string, to: string, asks: boolean): string {
  return `${asks ? '✉ ' : ''}${from} → ${to}`;
}

/**
 * A card it asked: open, its options answer it, tagged 'waits for you · 1
 * or 2'; answered, a line. Right after the ask it answers, the ask carries
 * the question and the card only its options (one copy of it).
 */
export function CardRow({ e, ctx }: { e: Entry; ctx: RowCtx }) {
  const prev = useNeighbor(ctx.tkey, e.pos, -1);
  const c = e.card;
  if (!c) return <Marker>{e.text}</Marker>;
  const follows = prev !== undefined && cardFollows(prev, e);
  if (c.answered) return <AnsweredCard c={c} ctx={ctx} follows={follows} />;
  // b (his card #476, designer m_15612): the inbox block is the one place to answer; the thread says
  // one dim line that points there, never a second copy with its options. A click opens it there.
  // an artifact link in the question is its pink name (art-chips), a click opens its view, not the inbox
  const testId = `thread.card.${String(c.id)}`;
  return (
    <CardLine
      testId={testId}
      onClick={() => {
        openInbox({ project: ctx.project, card: c.id });
      }}
    >
      <ArtWords text={cardLine(shownName(ctx.agent), c.question, follows, c.kind ?? undefined).replace(/^\? /u, '')} project={ctx.project} testId={testId} />
    </CardLine>
  );
}

/**
 * An answered card: '✓ answered: <question>'. A click opens the thread of the agent that asked it at
 * his answer (answered-link, designer m_24211; the asker's own thread: the jump alone).
 */
function AnsweredCard({ c, ctx, follows }: { c: EntryCard; ctx: RowCtx; follows: boolean }) {
  const store = useStore();
  const { order } = useThreadOrder(ctx.tkey);
  const at = cardTarget(ctx.project, ctx.agent, c);
  // one fact, one row (designer m_24812): his own '✓ you answered' row in this thread says it
  if (answerPos(order, (p) => store.entry(ctx.tkey, p), c.id) !== null) return null;
  const row = (
    <Answered testId={`thread.card.${String(c.id)}`}>
      <RowMark icon="check" />
      {follows ? thread.card_answered : thread.card_answered_q({ question: c.question })}
    </Answered>
  );
  if (at === null) return row;
  return (
    <AnswerLink at={at} here={ctx.agent}>
      {row}
    </AnswerLink>
  );
}

/** A '✓ answered' row as a link to his answer: its tooltip says where it goes. Pure. */
export function answerLinkTitle(at: AnswerAt, here: string): string {
  return at.agent === here ? thread.show_answer : thread.open_in({ thread: thread.from_thread({ name: shownName(at.agent) }) });
}

function AnswerLink({ at, here, children, testId }: { at: AnswerAt; here: string; children: ReactNode; testId?: string }) {
  // no router (a row drawn alone, a test): the row as it is, no link
  const router = useContext(RouterContext);
  if (router === null) return <>{children}</>;
  return (
    <span
      className="th-answer-link"
      role="link"
      title={answerLinkTitle(at, here)}
      data-testid={testId}
      onClick={(ev) => {
        // a drag that selected words isn't a click
        if (!(window.getSelection()?.isCollapsed ?? true)) return;
        ev.stopPropagation();
        openAnswer(router, at);
      }}
    >
      {children}
    </span>
  );
}

/**
 * An open card's line in the thread (designer m_15612): '? bise asks: which branch first? · in the
 * inbox below'; right under the ask it answers (the ask carries the question), '? waits for you · in
 * the inbox below'. Pure.
 */
export function cardLine(who: string, question: string, follows: boolean, kind = 'question'): string {
  if (follows) return thread.card_waits;
  const first = question.split('\n').find((l) => l.trim() !== '')?.trim() ?? '';
  // a plan an agent proposes (plan-ok): its own words, as in the inbox
  const asks = kind === 'plan' ? common.title_plan({ agent: who }) : common.title_asks({ agent: who });
  return first === '' ? thread.card_line({ asks }) : thread.card_line_q({ asks, question: first });
}
