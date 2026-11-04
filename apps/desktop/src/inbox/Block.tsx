// The inbox block (b2, the user's card #476 'b + the native notification'; designer's mocks on page
// inbox-native v2): ONE inbox, every project's open items, as the TUI's, mounted once by the shell
// right above the composer under every screen (amb-win's b1: ThreadScreen; off a thread, the floating composer); in a
// project's view only that project's items, on bise's view and the inbox screen every project's
// (scope.ts, his inbox-scope ask; an explicit open of another project's item widens it). Folded it is
// 'inbox · 3 waiting  ⌃1–3 open', a line per item up to 3 (fewer in a short window), then '+ N more ·
// ? who · words' (the TUI's box, card_draw.rs); ⌃n, a click, ⌘I (openInbox) or a notification
// click (openInbox({project, card})) opens that item IN PLACE (the block grows, nothing floats over
// what he reads): its options, 1-9 pick, ←→ walk them, ⌘⏎ the one the keys are on, ⌘⌫ close, ↑↓ the
// next item, esc folds. It tells the composer under it what his words do (onAnswer): a question takes
// them as its answer ('↳ answering search · …', 'esc to perf instead'); every other item takes free
// text too and closes (his rule m_44704, model.ts freeWords): a tool call's approval is refused with
// his text as the reason, any other item ('to perf · the merge waits for 1 or 2') steers its agent.
// Owner: amb-home.
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import type { Card, ProjectRow } from '@/proto';
import type { NotifyAuth } from '@/shared/api';
import { Act, Dot, Kbd, Row, RowMark } from '@/ds';
import { actedKeys, useActed, useInboxItems, useProjects, useRefusedList, useStore, useVoiceAnswer, type AnswerAt } from '@/store';
import { shownName } from '@/agents/model';
import { inbox } from '@/proto/words_inbox.gen';
import { common } from '@/proto/words_common.gen';
import { age, dotOf, folds, InboxCard, RefusedMark } from './InboxCard';
import { useMenuAction } from '@/app/menu';
import { cardKey } from './order';
import { isUpdate } from '@/shared/update';
import { askedBiseUpdate } from '@/app/UpdateLine';
import { freeWords, markOf, pickWaits, takesWords } from './model';
import { withRefused, type InboxItem } from './listed';
import { cardKeyOf, fitRows, openOnKey, rowWords, STRIP_KEYS, STRIP_ROWS, stripKey } from './stripRows';
import { ArtWords } from './artWords';
import { InboxZero, useInboxZero } from './Zero';
import { pruneCardDrafts } from './drafts';
import { keep } from '@/app/keep';
import { useModHeld } from '@/app/held';
import { answerTip } from '@/app/tipwords';
import './inbox.css';
import { aboutOf, chipOpen } from './about';
import { inboxScope, inScope, othersWaiting } from './scope';
import { useRouter } from '@/app/router';

/** What the composer under the block does with his words while an item is open. */
export interface Answering {
  readonly project: string;
  readonly card: number;
  /** the label line's left: '↳ answering search · reindex tonight…', 'to perf · the merge waits for 1 or 2' */
  readonly label: string;
  /** the label line's right ('esc to perf instead'); none: the composer's own words */
  readonly end?: string;
  /** a question his words answer (composer-target A): the field's words ('your answer to search, then ⏎')
   *  and the pink border's tip; none: the screen's own placeholder, no pink */
  readonly placeholder?: string;
  readonly tip?: string;
  /** his words answer this card (store.answer); null: they go as a plain message (never an approval) */
  readonly answer: number | null;
  /** who asked the card his words answer (inbox picks (3): his row where he is names it) */
  readonly asker?: string;
  /** the hub closed the open card while his words were typed: they stay in its draft and go to
   *  `asker` as a plain message (card-draft-keep) */
  readonly closed?: boolean;
  /** an item only its options decide is open (model.ts freeWords): his words refuse a tool call's
   *  approval ('deny', the reason its agent reads), else steer `asker` ('steer'); either way it closes */
  readonly free?: 'deny' | 'steer';
}

/** Where the block is: the screen's project and agent (none: bise's view, settings, review…). */
export interface Here {
  readonly project: string | null;
  readonly agent: string | null;
}

export interface InboxBlockProps {
  readonly here: Here;
  readonly onAnswer: (a: Answering | null) => void;
  /** now, for the ages (a test pins it) */
  readonly now?: number;
  /**
   * Only these items (task-panel m_20455: the artifact view docks only its maker's questions). It
   * filters the list before anything else (head count, ↑↓, ⌃n, the first item ⌘I opens), and bise's
   * notifications ask stays out. Absent: every project's items.
   */
  readonly only?: (c: Card) => boolean;
  /**
   * 'open-only': at rest it shows nothing, only an item he opens (⏎, ⌘I): the inbox screen, whose list
   * already shows the waiting items right above it (designer m_25639). 'line': at rest one 32 px line,
   * the count and the newest item cut with …; a click or ⌃1-3 opens one, never a hover (the dock
   * under a page off the thread: designer m_41569, ui-review-app #1). Absent: folded as signed.
   */
  readonly rest?: 'open-only' | 'line';
}

// ---- openInbox: ⌘I and a notification click (amb-win's keys and router call it; no route change)

type OpenAt = { readonly project: string; readonly card: number } | undefined;
const openers = new Set<(at: OpenAt) => boolean>();

/**
 * Open the block in place: on that card (a notification's), else on its first item. True when a
 * block opened one; false: nothing waits (or no block is mounted), the caller says so ('/inbox').
 */
export function openInbox(at?: { project: string; card: number }): boolean {
  let opened = false;
  for (const f of [...openers]) opened = f(at) || opened;
  return opened;
}

/** The blocks with an item open now: the inbox is in front, its ⌘⏎ wins over the artifact's (artifacts/front.ts). */
const openBlocks = new Set<symbol>();
export const inboxItemOpen = (): boolean => openBlocks.size > 0;

/** Hear openInbox (the block itself, a test): returns the unsubscribe. */
export function onOpenInbox(f: (at: OpenAt) => boolean): () => void {
  openers.add(f);
  return () => {
    openers.delete(f);
  };
}

// ---- the notifications' permission row (amb-mac's window.bise.notify, 97a5939a; designer m_14211)

type Notify = { state: () => Promise<NotifyAuth>; onState: (f: (s: NotifyAuth) => void) => () => void; ask: () => Promise<NotifyAuth> };
const notifyApi = (): Notify | null => (globalThis as { bise?: { notify?: Notify } }).bise?.notify ?? null;

/** 'not now' twice and the row never comes back; once, it waits for the next launch (designer). */
const NOT_NOW_KEY = 'bise.notify.notnow';
const NOT_NOW_LAUNCH = 'bise.notify.notnow.launch';
const notNowCount = (): number => Number(localStorage.getItem(NOT_NOW_KEY) ?? '0') || 0;

/** Whether the permission row shows: macOS never asked, an item waits, 'not now' not said this launch nor twice. Pure. */
export function asksNotify(auth: NotifyAuth | null, items: number, notNow: number, thisLaunch: boolean): boolean {
  return auth === 'never' && items > 0 && notNow < 2 && !thisLaunch;
}

/**
 * macOS's notifications permission as amb-mac's window.bise.notify tells it, live (null: not known yet,
 * or no notify api: a component test, an older shell), and a setter for ask()'s answer. Settings reads it too.
 */
export function useNotifyAuth(): [NotifyAuth | null, (s: NotifyAuth) => void] {
  const [auth, setAuth] = useState<NotifyAuth | null>(null);
  useEffect(() => {
    const api = notifyApi();
    if (api === null) return;
    let live = true;
    void api.state().then((s) => {
      if (live) setAuth(s);
    });
    const off = api.onState((s) => {
      setAuth(s);
    });
    return () => {
      live = false;
      off();
    };
  }, []);
  return [auth, setAuth];
}

/**
 * The permission ask is one ordinary item from bise (H10, designer m_19835: every question for him
 * stays above the composer, and it never has a line of its own): '? bise · let questions reach you as
 * macOS notifications?', 1 turn on · 2 not now. Asked once: either answer and it is gone. Never the hub's.
 */
export const NOTIFY_ID = -1;
export function notifyCard(home: string, since: number): Card {
  return {
    id: NOTIFY_ID,
    project: home,
    kind: 'question',
    agent: 'bise',
    question: inbox.notify_question,
    options: [
      { n: 1, label: inbox.notify_on },
      { n: 2, label: inbox.notify_not_now },
    ],
    urgent: false,
    since_ms: since,
  };
}
const isNotify = (c: Card): boolean => c.id === NOTIFY_ID;

/** Whether the block lists bise's notifications ask now: the inbox screen lists and counts it too (one inbox, one number; designer m_25639). */
export function useNotifyAsks(items: number): boolean {
  return useNotifyRow(items).show;
}

function useNotifyRow(items: number): { show: boolean; on: () => void; later: () => void } {
  const [auth, setAuth] = useNotifyAuth();
  const [said, setSaid] = useState(() => sessionStorage.getItem(NOT_NOW_LAUNCH) === '1');
  return {
    show: asksNotify(auth, items, notNowCount(), said),
    on: () => {
      setSaid(true);
      void notifyApi()?.ask().then(setAuth);
    },
    // asked once (designer m_19835): 'not now' is for good
    later: () => {
      localStorage.setItem(NOT_NOW_KEY, '2');
      sessionStorage.setItem(NOT_NOW_LAUNCH, '1');
      setSaid(true);
    },
  };
}

/** The count the block in `here` says in its head ('N waiting'): the same items in the same scope,
 *  bise's notifications ask too where every project's show (the empty home's line says this
 *  number, designer m_43577: one count, one word). */
export function useWaitingCount(here: Here): number {
  const rows = useProjects();
  const all = useInboxItems();
  const home = rows.find((r) => r.home)?.project ?? '';
  const scope = inboxScope(here, home);
  const n = all.filter((c) => inScope(scope, c)).length;
  const notify = useNotifyRow(n);
  return n + (notify.show && scope === null ? 1 : 0);
}

// ---- the pure parts

/** The thread he answers from: the screen's, or bise's view's (its home project's main); none elsewhere. Pure. */
export function answerAt(here: Here, home: string): AnswerAt | null {
  if (here.project !== null && here.agent !== null) return { project: here.project, agent: here.agent };
  if (here.project === null && here.agent === null && home !== '') return { project: home, agent: 'main' };
  return null;
}

/**
 * Folded, the block is its head and this many item lines, then a '+ N more' line: the TUI's box
 * (card_draw.rs STRIP_ROWS; his inbox-stack ask, back from H10's one line); fewer in a short window
 * (fitRows, designer m_26234).
 */
export const BLOCK_ROWS = STRIP_ROWS;

const cut = (s: string, max: number): string => {
  const one = s.replace(/\s+/gu, ' ').trim();
  return Array.from(one).length > max ? `${Array.from(one).slice(0, max - 1).join('').trimEnd()}…` : one;
};

/** The composer's head while his words answer a question: '↳ answering' (never cut). */
export const ANSWERING_HEAD = '↳';

/** A question on one line, cut after a whole word within `max` characters with one '…' (designer
 *  m_24325: never mid-word; a word longer than `max` alone: cut there). Pure. */
export function cutWords(s: string, max: number): string {
  const one = s.replace(/\s+/gu, ' ').trim();
  if (Array.from(one).length <= max) return one;
  const words = one.split(' ');
  let head = '';
  for (const w of words) {
    const next = head === '' ? w : `${head} ${w}`;
    if (Array.from(next).length > max - 1) break;
    head = next;
  }
  head = head.replace(/[\s·,:;]+$/u, '');
  return head === '' ? cut(one, max) : `${head}…`;
}

/**
 * What his words do while `c` is open, seen from `here`. An item of another project than the
 * screen's names its agent and project (designer m_19631: 'answering p99 · telemetry · …', 'to p99
 * · telemetry · the merge waits for 1 or 2'); the screen's own project and bise's view: as before.
 * `pname`: the item's project as the block shows it (none: its id). Pure.
 */
export function answeringOf(c: Card, here: Here, pname?: string): Answering {
  const to = here.agent === null ? 'bise' : shownName(here.agent);
  // the permission ask is bise's own, never the hub's: only its two options answer it
  if (isNotify(c)) return { project: c.project, card: c.id, label: inbox.notify_waits({ to }), answer: null };
  // the update card is bise's own (update-dot): never another project's
  const other = here.project !== null && here.project !== c.project && !isUpdate(c) ? `${shownName(c.agent)} · ${pname ?? c.project}` : null;
  if (takesWords(c)) {
    // composer-target A (designer m_26531): '↳ answering search · <question>' (the tab cuts it after
    // a whole word), 'esc to bise instead', the field names who gets his words, the border's tip
    // on the asker's own thread there is no esc end (designer m_26776, m_26818)
    const who = shownName(c.agent);
    const label = `${ANSWERING_HEAD} ${inbox.answering({ who: other ?? who })} · ${cutWords(c.question, 40)}`;
    const end = other === null && who === to ? {} : { end: inbox.esc_instead({ to }) };
    return { project: c.project, card: c.id, label, ...end, answer: c.id, asker: c.agent, placeholder: inbox.answer_placeholder({ who, key: '⏎' }), tip: answerTip(who, to) };
  }
  // an approval or a choice: its options decide it, and his free text closes it too (his rule
  // m_44704, freeWords): a tool call's approval refused with his text as the reason, any other item
  // a steer to its asker; the label says what waits ('to perf · the merge waits for 1 or 2'), the
  // field what his words do; esc names the screen's own addressee when it is someone else
  const waits = pickWaits(c) ?? inbox.waits_options;
  const asker = shownName(c.agent);
  const same = other === null && asker === to;
  const free = freeWords(c) === 'deny' ? 'deny' : 'steer';
  const placeholder = free === 'deny' ? inbox.deny_placeholder({ who: asker }) : inbox.steer_placeholder({ who: asker });
  return { project: c.project, card: c.id, label: inbox.to_waits({ who: other ?? asker, waits }), ...(same ? {} : { end: inbox.esc_instead({ to }) }), answer: null, asker: c.agent, free, placeholder };
}

/**
 * What his words do while `c` is open after the hub closed it (withdrawn, answered via main, its
 * agent stopped) with his words typed: a plain message to its asker; the tab leaves the accent,
 * nothing waits on him (designer m_24087: 'to perf · the question is closed', 'esc to bise
 * instead'). Pure.
 */
export function closedAnswering(c: Card, here: Here, pname?: string): Answering {
  const to = here.agent === null ? 'bise' : shownName(here.agent);
  const other = here.project !== null && here.project !== c.project && !isUpdate(c) ? `${shownName(c.agent)} · ${pname ?? c.project}` : shownName(c.agent);
  return { project: c.project, card: c.id, label: inbox.to_closed({ who: other }), end: inbox.esc_instead({ to }), answer: null, asker: c.agent, closed: true };
}

/**
 * The open card `c` left the hub's rows: it stays, closed, when it took words, he did not act on it
 * here (`gone`: answered or closed in this window fold as before) and the composer under the block
 * holds his words (its field still shows that card's draft).
 */
function keepsWords(c: Card, gone: ReadonlySet<string>): boolean {
  if (gone.has(cardKey(c)) || isNotify(c) || !takesWords(c)) return false;
  return (document.querySelector('[data-testid="composer.field"]')?.textContent ?? '').trim() !== '';
}

// ---- leaveClosed: his words to a closed card went (the composer's send): the block lets it go
const leavers = new Set<() => void>();

/** The composer sent his words from the closed card: it leaves the block. */
export function leaveClosed(): void {
  for (const f of [...leavers]) f();
}

/** An item's project as its row and its card say it: bise's home is 'bise', else its name. Pure. */
export function projectWord(row: Pick<ProjectRow, 'name' | 'home'> | undefined, project: string): string {
  return row?.home === true ? 'bise' : (row?.name ?? project);
}

/** The block's head: 'N waiting', ' · K projects' once more than one project waits. Pure. */
export function headWords(list: readonly Pick<Card, 'project'>[]): string {
  const projects = new Set(list.map((c) => c.project)).size;
  return projects > 1 ? inbox.block_head_projects({ n: list.length, projects }) : common.count_waiting({ n: list.length });
}

/** An item's sign on its line (model markOf): '?' in the accent when it asks him, ✗ in err for a
 *  failure that asks nothing, else its dot; its 14 line icon, never a text glyph (ui-review m_43949). */
function SignMark({ c }: { c: Card }): ReactNode {
  const m = markOf(c);
  if (m === 'ask') return <RowMark icon="ask" tone="accent" label="question" />;
  if (m === 'fail') return <RowMark icon="close" tone="err" label="failed" />;
  return <Dot state={dotOf(c)} />;
}

/** The last line, the TUI's more_row: '+ 2 more · ? perf · which bench?' (`next`: the first of them),
 *  its sign drawn as the row's line icon, never a text glyph (ui-review m_43978; the TUI keeps its '?'). Pure. */
export function moreWords(more: number, next: Card): ReactNode {
  const w = rowWords(next);
  const what = w.what === '' ? '' : ` · ${w.cmd ? '$ ' : ''}${w.what}`;
  // the catalog's words with a slot where the sign goes: the words stay the catalog's, the icon sits in them
  const slot = '\u{E000}';
  const [before = '', after = ''] = inbox.more_row({ n: more, first: `${slot}${shownName(w.who)}${what}` }).split(slot);
  return (
    <>
      {before}
      {markOf(next) !== null && <SignMark c={next} />}
      {after}
    </>
  );
}

/** A field he is typing in: its keys are his, not the block's. */
function typing(t: EventTarget | null): boolean {
  if ((t instanceof HTMLTextAreaElement || t instanceof HTMLInputElement) && t.value !== '') return true;
  return t instanceof HTMLElement && t.isContentEditable && t.textContent.trim() !== '';
}

// The block never folds, opens or changes height when he scrolls the thread (designer m_21505,
// ambient-lead m_21498): only his hand (a click, esc, ctrl+o) or its content changes it. The H10
// one-line form while he read up the thread (m_19835) is gone: the layout jumped under him.
export function InboxBlock({ here, onAnswer, now: pinned, only, rest }: InboxBlockProps) {
  const cmd = useModHeld('Meta');
  // ctrl held (the TUI's card_draw.rs number_style): each row's number in accent, ctrl+N opens it
  const ctrl = useModHeld('Control');
  const rows = useProjects();
  const store = useStore();
  // the cards he answered or closed, from any view: the store's one set (acted.ts), never this block's
  const acted = useActed();
  const gone = useMemo(() => actedKeys(acted), [acted]);
  const [open, setOpen] = useState<string | null>(null);
  const [on, setOn] = useState<number | null>(null);
  // the keys read the open item and the option they're on from here, written at once by each key:
  // a second key before React renders (a fast repeat, a quick ↓↑) sees the first one's move (amb-mac m_22532)
  const keyAt = useRef<{ open: string | null; on: number | null }>({ open, on });
  const [opened] = useState(() => Date.now());
  const now = pinned ?? opened;
  // an explicit ask ('/inbox', ⌘I, a notification) for an item the `only` filter leaves out widens
  // the block to every project's items until he folds it (ghost-badge: '/inbox' beside an artifact
  // did nothing while another project's item waited)
  const [wide, setWide] = useState(false);
  // the inbox's one view, from the store (machine/cards.ts inboxView): each item with its refusal
  const all = useInboxItems();
  const home = rows.find((r) => r.home)?.project ?? '';
  // a project's view lists that project's items; bise's view and the inbox screen every project's (scope.ts)
  const scope = inboxScope(here, home, rest);
  const real = useMemo(
    () => (wide ? all : all.filter((c) => inScope(scope, c) && (only === undefined || only(c)))),
    [all, only, wide, scope],
  );
  const notify = useNotifyRow(real.length);
  // bise's own notifications ask is no project's: only where every project's items show
  const asks = notify.show && only === undefined && (scope === null || wide);
  // scoped to a project: its rows leave their project out, the head says what waits elsewhere (designer m_32874)
  const scoped = scope !== null && !wide;
  const others = scoped ? othersWaiting(all, scope) : 0;
  const router = useRouter();
  const list = useMemo((): readonly InboxItem[] => (asks ? [...real, { ...notifyCard(home, opened), refused: null }] : real), [real, asks, home, opened]);
  const names = useMemo(() => new Map(rows.map((r) => [r.project, r])), [rows]);
  // Benjamin's lost answer (card-draft-keep): a new card never moves the open one; the open one
  // closed by the hub (withdrawn, answered via main, its agent stopped) while his words are typed
  // stays open, closed, his words in its draft, until he sends them (to its asker) or leaves it
  const [closedAt, setClosed] = useState<InboxItem | null>(null);
  // the kept closed card is drawn with its live refusal, as every item (listed.ts withRefused): the
  // one it had when the hub dropped it is gone with it (refusedPrune), never frozen on the card
  const refusedNow = useRefusedList();
  const closed = useMemo(() => (closedAt === null ? null : (withRefused([closedAt], refusedNow)[0] ?? null)), [closedAt, refusedNow]);
  const [seen, setSeen] = useState<InboxItem | null>(null);
  const listed = list.find((c) => cardKey(c) === open);
  // decided while rendering, before the composer hears a thing: an effect would tell it 'none'
  // for one commit, and the swap and the prune would take his words with them
  if (listed !== undefined && seen !== listed) setSeen(listed);
  if (listed === undefined && open !== null && seen !== null && cardKey(seen) === open && closed === null && keepsWords(seen, gone)) setClosed(seen);
  const openCard = listed ?? (closed !== null && cardKey(closed) === open ? closed : undefined);
  const isOpen = openCard !== undefined;
  useEffect(() => {
    if (!isOpen) return;
    const me = Symbol('block');
    openBlocks.add(me);
    return () => {
      openBlocks.delete(me);
    };
  }, [isOpen]);
  useEffect(() => {
    const f = () => {
      setClosed(null);
      keyAt.current = { open: null, on: null };
      setOpen(null);
      setOn(null);
    };
    leavers.add(f);
    return () => {
      leavers.delete(f);
    };
  }, []);
  // inbox zero (desktop-delight TOUCH 3): his answer to the last item, ':* nothing waits for you' for 2 s
  // scoped to a project, 'nothing waits for you' only when nothing waits in any project either
  const zero = useInboxZero(list.length, acted.length) && all.length === 0;

  // a card answered or closed anywhere takes its half answer with it (drafts.ts; 2b); the closed
  // one in view keeps it until he sends it or leaves
  const keptKey = closed !== null && cardKey(closed) === open ? cardKey(closed) : null;
  const liveKeys = [...rows.flatMap((r) => (r.cards ?? []).map(cardKey)), ...(keptKey === null ? [] : [keptKey])].join(' ');
  useEffect(() => {
    pruneCardDrafts(keep, new Set(liveKeys === '' ? [] : liveKeys.split(' ')));
  }, [liveKeys]);

  // bar N14 (moved here from the ⌘I screen, one inbox): his talk answered the open item (the card
  // in view the core decides on): its heard line shows on it for the event's ms (esc undoes,
  // nothing sent), then the answer goes as the digit key's would. `done`: the event answered or
  // undone (none shows again); the one already there when the block mounts is old, never acted on.
  const voice = useVoiceAnswer();
  const [done, setDone] = useState(() => voice);
  const heard = voice !== null && voice !== done && openCard !== undefined && openCard.project === voice.project && openCard.id === voice.card ? voice : null;
  const answerHeard = useRef<(project: string, card: number, n: number) => void>(() => undefined);
  useEffect(() => {
    if (heard === null) return;
    const t = setTimeout(() => {
      setDone(heard);
      answerHeard.current(heard.project, heard.card, heard.n);
    }, heard.ms);
    return () => {
      clearTimeout(t);
    };
  }, [heard]);
  useEffect(() => {
    if (heard === null) return;
    const esc = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      e.stopImmediatePropagation();
      setDone(heard);
    };
    // first in line: esc undoes the heard answer before it folds the block
    window.addEventListener('keydown', esc, true);
    return () => {
      window.removeEventListener('keydown', esc, true);
    };
  }, [heard]);

  // ctrl+o (and ⌘E), focus anywhere, the composer's too: the window's capture listener sends
  // 'fold-all' (app/keys.ts, the TUI's ctrl+o); the open item's folded lines open whole, again fold
  // them (bar I8, qa-native f640424d); another item opens folded
  const [whole, setWhole] = useState<string | null>(null);
  useMenuAction('fold-all', () => {
    if (openCard !== undefined && folds(openCard.question)) setWhole((w) => (w === open ? null : open));
  });

  const openTo = (k: string | null) => {
    keyAt.current = { open: k, on: null };
    // leaving the closed card: it goes (the composer keeps his words, ThreadComposer)
    if (closed !== null && k !== cardKey(closed)) setClosed(null);
    setOpen(k);
    setOn(null);
    setWhole(null);
    if (k === null) setWide(false);
  };
  const onTo = (n: number) => {
    keyAt.current = { ...keyAt.current, on: n };
    setOn(n);
  };
  // the item he opened beside its page stays open in the block the artifact view mounts
  useEffect(() => {
    const at = store.keptOpen();
    const c = at === undefined ? undefined : list.find((x) => x.project === at.project && x.id === at.card);
    if (c === undefined) return;
    store.keepOpen(undefined);
    // eslint-disable-next-line react-hooks/set-state-in-effect -- set by the view that unmounted in this commit: known only after it
    openTo(cardKey(c));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- openTo is this render's; the list is the trigger
  }, [list]);
  // and when that view closes, the block that mounts back keeps a page's item open too
  const openNow = useRef<Card | undefined>(undefined);
  useEffect(() => {
    openNow.current = openCard;
  });
  useEffect(
    () => () => {
      const c = openNow.current;
      if (c != null && aboutOf(c) != null) store.keepOpen({ project: c.project, card: c.id });
    },
    [store],
  );

  // ⌘I, '/inbox' and a notification click: that card, else the first item; one the filter leaves
  // out widens the block (wide). False: nothing waits anywhere, the caller says so
  const latest = useRef({ list, all });
  useEffect(() => {
    latest.current = { list, all };
  });
  useEffect(() => {
    const f = (at: OpenAt): boolean => {
      const { list: l, all: every } = latest.current;
      const find = (xs: readonly Card[]) => (at !== undefined ? xs.find((c) => c.project === at.project && c.id === at.card) : xs[0]);
      const inView = find(l);
      const want = inView ?? find(every);
      if (want === undefined) return false;
      if (inView === undefined) setWide(true);
      keyAt.current = { open: cardKey(want), on: null };
      setOpen(cardKey(want));
      setOn(null);
      return true;
    };
    openers.add(f);
    return () => {
      openers.delete(f);
    };
  }, []);

  // what his words do: told on each change, and none once the block goes
  const { project: hp, agent: ha } = here;
  const isClosed = openCard !== undefined && openCard === closed;
  const told = openCard === undefined ? null : (isClosed ? closedAnswering : answeringOf)(openCard, { project: hp, agent: ha }, projectWord(names.get(openCard.project), openCard.project));
  const tellKey = told === null ? '' : `${told.project}/${String(told.card)}/${told.label}/${told.end ?? ''}/${String(told.answer)}/${String(told.closed === true)}/${told.asker ?? ''}`;
  // told in the layout phase (ghost-badge): an item he opens swaps the composer to its draft in the
  // same task as his click (an update from a layout effect is sync, so is the field's swap after
  // it); told from a passive effect, the swap came a render later, and a key typed in between went
  // to the thread's draft and left the field (bisected to e2428cd09, which only made it slower)
  const tell = useRef(onAnswer);
  useLayoutEffect(() => {
    tell.current = onAnswer;
  });
  useLayoutEffect(() => {
    tell.current(told);
    // told is derived from tellKey: it changes exactly when tellKey does
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tellKey]);
  useEffect(
    () => () => {
      tell.current(null);
    },
    [],
  );

  // bar N14: the open item is the card in front of him (fn's talk answers it)
  const openProject = openCard === undefined || isNotify(openCard) ? null : openCard.project;
  const openId = openCard === undefined || isNotify(openCard) ? null : openCard.id;
  useEffect(() => {
    store.cardInView(openProject !== null && openId !== null && document.hasFocus() ? openProject : null, openProject !== null && openId !== null && document.hasFocus() ? openId : null);
    return () => {
      store.cardInView(null, null);
    };
  }, [store, openProject, openId]);

  // an item answered or closed leaves every view at once: store.answer / store.close keep it out
  // until the hub's rows drop it (acted.ts); here it only folds
  const leave = () => {
    openTo(null);
  };
  // a card linked to a page (notif-artifact, designer m_26560): its chip or ⌘O opens it in the panel's
  // tab at its block, and only that (his m_47572): the thread stays, this card stays open as it is
  // (its composer and his words too), nothing sent
  const openPage = (c: Card) => {
    const intent = chipOpen(c);
    if (intent !== null) store.do(intent);
  };
  const pick = (c: Card, n: number) => {
    // a closed card picks nothing: its options are dim
    if (c === closed) return;
    if (isNotify(c)) {
      openTo(null);
      if (n === 1) notify.on();
      else notify.later();
      return;
    }
    // the store's machine decides (machine/cards.ts pick): an option the window does itself, as the
    // hub's row says (a drafts card's '1 review' opens its page beside the thread, a merge's '2 open
    // it on GitHub' its PR, a sign-in's '1' the app's own sign-in) sends nothing and the card stays;
    // else his answer, with where he is (inbox picks 3). The block folds only when it answered.
    // '1 update now' on the update card: the app's own update goes with the machine's (UpdateLine.tsx)
    if (isUpdate(c) && n === 1) askedBiseUpdate();
    // on the inbox page, an option that opens an artifact opens it in the panel's tab only: his list
    // stays (designer m_53016, a try card's '2 show the diff')
    const tab = router.get().screen === 'inbox' ? ({ tab: true } as const) : {};
    const { answered } = store.do({ i: 'pick', project: c.project, card: c.id, n, from: { at: answerAt(here, home) }, ...tab });
    if (answered) leave();
  };
  const close = (c: Card) => {
    // closed already: it just goes, his words to the thread's composer (ThreadComposer)
    if (c === closed) {
      openTo(null);
      return;
    }
    if (isNotify(c)) {
      openTo(null);
      notify.later();
      return;
    }
    leave();
    store.do({ i: 'close', project: c.project, card: c.id });
  };
  useEffect(() => {
    answerHeard.current = (project, card, n) => {
      const c = list.find((x) => x.project === project && x.id === card);
      if (c !== undefined) pick(c, n);
    };
  });

  // ⌃1-9 open item n wherever the focus is; the open item has the keys while no words are typed
  // (1-9, ←→, ⌘⌫, esc, ↑↓); ⌘⏎ picks the option the keys are on even with words typed
  // one listener for the block's life: it calls the last render's handler, which reads the open item
  // and the option from `keyAt` (a key before the next render sees the last key's move, amb-mac m_22532)
  const keyNow = useRef<(e: KeyboardEvent) => void>(() => undefined);
  useEffect(() => {
    keyNow.current = (e: KeyboardEvent) => {
      const n = stripKey(e);
      if (n !== undefined) {
        const c = n <= STRIP_KEYS ? list[n - 1] : undefined;
        if (c === undefined) return;
        e.preventDefault();
        e.stopPropagation();
        openTo(cardKey(c));
        return;
      }
      const { open, on } = keyAt.current;
      const openCard = list.find((c) => cardKey(c) === open) ?? (closed !== null && cardKey(closed) === open ? closed : undefined);
      if (openCard === undefined) return;
      if (e.key === 'Enter' && e.metaKey && !e.ctrlKey && !e.altKey && on !== null) {
        e.preventDefault();
        e.stopPropagation();
        pick(openCard, on);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        openTo(null);
        return;
      }
      // ctrl+n / ctrl+p: the next / previous item, wrapping, even with his words in the field (the TUI's cards.rs step(.., true))
      if ((e.key === 'n' || e.key === 'p') && e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
        const i = list.findIndex((c) => cardKey(c) === open);
        const next = list[(i + (e.key === 'n' ? 1 : -1) + list.length) % list.length];
        if (next === undefined) return;
        e.preventDefault();
        e.stopPropagation();
        openTo(cardKey(next));
        return;
      }
      // ⌘O: what it is about in the panel, even with his words in the answer field (designer m_47680):
      // the card says it there, and ⌘O types nothing
      if (openOnKey(e, openCard)) {
        e.preventDefault();
        e.stopPropagation();
        openPage(openCard);
        return;
      }
      if (typing(e.target)) return;
      // ↑↓ and ctrl+↑↓: the previous / next item, no wrap (the TUI's arrow_mods: none or ctrl)
      if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && !e.metaKey && !e.altKey && !e.shiftKey) {
        const i = list.findIndex((c) => cardKey(c) === open);
        const next = list[Math.max(0, Math.min(list.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))];
        if (next === undefined) return;
        e.preventDefault();
        e.stopPropagation();
        openTo(cardKey(next));
        return;
      }
      const k = cardKeyOf(e, openCard, on);
      if (k === null) return;
      e.preventDefault();
      e.stopPropagation();
      if (k.t === 'later') openTo(null);
      else if (k.t === 'on') onTo(k.n);
      else if (k.t === 'pick') pick(openCard, k.n);
      else close(openCard);
    };
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      keyNow.current(e);
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
    };
  }, []);

  // a short window (designer m_26234): folded, the rows give way 3 → 2 → 1 so the thread keeps
  // THREAD_MIN of the window; measured on the folded block (the thread list's top, and the block's top
  // to the window's bottom, its rows left out), again when the window or what holds the block (the
  // composer growing) changes size.
  // An item opened keeps the count it had folded. Never a scroll inside the block.
  const section = useRef<HTMLElement | null>(null);
  const [rowsFit, setRowsFit] = useState(BLOCK_ROWS);
  const folded = openCard === undefined;
  const items = list.length;
  useLayoutEffect(() => {
    const el = section.current;
    if (el === null || !folded) return;
    const measure = () => {
      const rows = el.querySelectorAll(':scope > .b-row');
      const row = rows[0]?.getBoundingClientRect().height ?? 0;
      if (row <= 0) return;
      const win = window.innerHeight;
      // above the thread: the top bar, the job line (none outside a thread)
      const above = document.querySelector('[data-testid="thread.list"]')?.getBoundingClientRect().top ?? 0;
      setRowsFit(fitRows({ win, fixed: above + win - el.getBoundingClientRect().top - rows.length * row, row, items }));
    };
    measure();
    const ro = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    if (el.parentElement !== null) ro?.observe(el.parentElement);
    window.addEventListener('resize', measure);
    return () => {
      ro?.disconnect();
      window.removeEventListener('resize', measure);
    };
  }, [folded, items]);

  if (list.length === 0 && !isClosed) return zero ? <InboxZero /> : null;
  if (rest === 'open-only' && openCard === undefined) return null;
  if (rest === 'line' && openCard === undefined && !isClosed) {
    // list holds one item at least here (the empty list returned above)
    const newest = list.reduce((a, c) => (c.since_ms > a.since_ms ? c : a));
    const w = rowWords(newest);
    return (
      <section className="inbox-strip inbox-block inbox-line" aria-label={inbox.head} data-testid="inbox.block" data-rest="line" data-count={list.length} data-ctrl={ctrl || undefined}>
        <Row
          lead={<span className="strip-title">{inbox.head}</span>}
          name={
            <span className="line-what">
              <span className="strip-count">{headWords(list)}</span>
              {' · '}
              <span className="strip-who">{shownName(w.who)}</span> {w.what}
            </span>
          }
          end={<span className="strip-keys"><Kbd>{list.length > 1 ? `⌃1–${String(Math.min(list.length, STRIP_KEYS))}` : '⌃1'}</Kbd> {inbox.key_open}</span>}
          onClick={() => {
            openTo(cardKey(newest));
          }}
          testId="inbox.block.line"
        />
      </section>
    );
  }
  const openAt = openCard === undefined ? -1 : list.indexOf(openCard);
  // the closed card on top, open; the rows under it keep their ⌃N numbers
  const shown = [...(isClosed ? [openCard] : []), ...list.filter((_, i) => i < rowsFit || i === openAt)];
  const more = list.length - shown.length + (isClosed ? 1 : 0);
  // '+ N more' walks on past the open item, then wraps: never back and forth between two
  const next = list.find((_, i) => i >= rowsFit && i > openAt) ?? list.find((_, i) => i >= rowsFit && i !== openAt);
  const keys = list.length > 1 ? `⌃1–${String(Math.min(list.length, STRIP_KEYS))}` : '⌃1';
  return (
    <section ref={section} className="inbox-strip inbox-block" aria-label={inbox.head} data-testid="inbox.block" data-rows={rowsFit} data-open={openCard !== undefined || undefined} data-count={list.length} data-ctrl={ctrl || undefined}>
      <div className="strip-title">
        {inbox.head}{' '}
        <span className="strip-count">
          {headWords(list)}
          {others > 0 && (
            // what waits in his other projects: ' · ' plain, then the link to the inbox screen
            // (designer m_32874, m_33443: only the words are the link, one space each side of the dot)
            <>
              {' · '}
              <span className="strip-others">
                <Act
                  testId="inbox.block.others"
                  onClick={() => {
                    router.go({ screen: 'inbox' });
                  }}
                >
                  {inbox.block_head_others({ n: others })}
                </Act>
              </span>
            </>
          )}
        </span>
        <span className="strip-keys" data-testid="inbox.block.keys">
          {cmd ? (
            // ⌘ held: the inbox's own ⌘ key on its head (amb-win's held.ts, designer m_21510)
            <span className="app-keyhint" data-testid="inbox.block.hint">
              <Kbd>⌘I</Kbd>
            </span>
          ) : openCard !== undefined ? (
            <>
              <Kbd>↑↓</Kbd> {inbox.key_next} <Kbd>esc</Kbd> {inbox.key_fold}
            </>
          ) : (
            <>
              <Kbd>{keys}</Kbd> {inbox.key_open}
            </>
          )}
        </span>
      </div>
      {shown.map((c) => {
        const n = list.indexOf(c) + 1;
        const row = names.get(c.project);
        if (c === openCard) {
          return (
            // the card his words answer wears the composer's pink as a 2 px rail (composer-target A)
            <div key={cardKey(c)} className="inbox-block-open" data-testid={`inbox.block.open.${String(n)}`} data-answering={(told?.answer != null && told.closed !== true) || undefined}>
              <InboxCard
                card={c}
                // the update card is bise's own (update-dot), wherever it came from: no project
                {...(isUpdate(c) ? { home: true } : { project: projectWord(row, c.project) })}
                now={now}
                on={on}
                full={whole === cardKey(c)}
                {...(heard !== null && heard.project === c.project && heard.card === c.id
                  ? {
                      note: (
                        <span className="inbox-heard" data-testid="inbox.heard">
                          {inbox.heard_undo({ line: heard.line })}
                        </span>
                      ),
                    }
                  : {})}
                acts={{
                  pick: (o) => {
                    pick(c, o);
                  },
                  close: () => {
                    close(c);
                  },
                  later: () => {
                    openTo(null);
                  },
                  ...(aboutOf(c) != null ? { openPage: () => { openPage(c); } } : {}),
                }}
                testId={isNotify(c) ? 'inbox.block.notify' : `inbox.block.card.${cardKey(c)}`}
                {...(c === closed
                  ? {
                      closed: true,
                      keys: (
                        <span data-testid="inbox.block.closed.keys">
                          <Kbd>⏎</Kbd> {inbox.key_send_message} <Kbd>esc</Kbd> {inbox.key_back}
                        </span>
                      ),
                    }
                  : {})}
              />
            </div>
          );
        }
        const w = rowWords(c);
        return (
          // raw-controls (designer m_18343): the ds's Row as a button, never a bare button
          <Row
            key={cardKey(c)}
            lead={
              <>
                <span className="strip-n">{n}</span>
                <SignMark c={c} />
              </>
            }
            name={
              <>
                <span className="strip-who">{shownName(w.who)}</span>
                {w.what !== '' && (
                  <span className="strip-what">
                    {w.cmd && <span className="strip-cmd">$ </span>}
                    <ArtWords text={w.what} project={c.project} testId={`inbox.block.row.${String(n)}`} />
                    {w.note !== '' && <span className="strip-note">{` ${w.note}`}</span>}
                  </span>
                )}
                {!isNotify(c) && <RefusedMark card={c} />}
              </>
            }
            end={
              <>
                {/* the age alone: no ids in the UI (designer m_43892) */}
                {scoped || isUpdate(c) ? age(now - c.since_ms) : `${projectWord(row, c.project)} · ${age(now - c.since_ms)}`}
              </>
            }
            onClick={() => {
              openTo(cardKey(c));
            }}
            testId={`inbox.block.row.${String(n)}`}
          />
        );
      })}
      {more > 0 && next !== undefined && (
        // the TUI's more_row: '+ 2 more · ? perf · which bench?', faint; a click opens the first of
        // them, past the open item (never back and forth between two)
        <Row
          name={<span className="strip-more">{moreWords(more, next)}</span>}
          onClick={() => {
            // the walk's keys (↑↓) are the block's: the row never keeps a focus ring after the click
            if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
            openTo(cardKey(next));
          }}
          testId="inbox.block.more"
        />
      )}
    </section>
  );
}
