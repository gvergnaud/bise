// The inbox screen's list (designer's inbox-history pick A, m_24212 (2)-(6)): what waits on him
// first, then what he answered or what was withdrawn, by day, read-only. Pure: the rows, their
// words, the filters and where a past row goes (his answer in a thread, or the page at its block).
// The records are inbox-record's (bise_proto::inbox, one per answered or withdrawn card); a row's
// second line is the record's own words (line2, words::second_line), so the terminal and the window
// agree (architect m_24257); the day heads are words::day_head's. Owner: inbox-screen.
import type { Card } from '@/proto';
import type { InboxRecord, Where } from '@/proto/inbox';
import { inbox } from '@/proto/words_inbox.gen';
import { hero } from '@/proto/words_hero.gen';
import { shownName } from '@/agents/model';
import { dayOf } from '@/artifacts/model';
import { sentenceCase } from '@/ds/case';
import { age } from './InboxCard';
import { idOf } from './id';

export type { InboxRecord, Where as InboxWhere } from '@/proto/inbox';

export type StateFilter = 'all' | 'waiting' | 'answered' | 'withdrawn';
export const STATES: readonly StateFilter[] = ['all', 'waiting', 'answered', 'withdrawn'];

export interface Filter {
  /** a project id; '' all */
  readonly project: string;
  /** an asker; '' all */
  readonly agent: string;
  readonly state: StateFilter;
  /** words in the question, the asker, the project, his answer or the reason */
  readonly q: string;
}

export const ALL: Filter = { project: '', agent: '', state: 'all', q: '' };

/** A row that waits on him: an open card. */
export interface WaitRow {
  readonly key: string;
  readonly card: Card;
  /** '?' a question or an approval, '¶' a page's item */
  readonly glyph: '?' | '¶';
  readonly title: string;
  readonly question: string;
  readonly project: string;
}

/** A past row: an answered or withdrawn record. */
export interface PastRow {
  readonly key: string;
  readonly r: InboxRecord;
  readonly glyph: '✓' | '–';
}

export interface Day {
  readonly day: string;
  readonly title: string;
  readonly rows: readonly PastRow[];
}

export interface Lists {
  readonly waiting: readonly WaitRow[];
  readonly days: readonly Day[];
  /** every row's key in the order they show (the list keys' order) */
  readonly order: readonly string[];
}

export const waitKey = (c: { project: string; id: number }): string => `w:${c.project}/${String(c.id)}`;
export const pastKey = (r: { project: string; card: number }): string => `p:${r.project}/${String(r.card)}`;

const words = (q: string): string[] => q.toLowerCase().split(/\s+/u).filter((w) => w !== '');
const hasAll = (hay: string, ws: readonly string[]): boolean => {
  const h = hay.toLowerCase();
  return ws.every((w) => h.includes(w));
};

/** The askers he can filter on, from both lists, sorted. Pure. */
export function askersOf(cards: readonly Card[], records: readonly InboxRecord[]): string[] {
  return [...new Set([...cards.map((c) => c.agent), ...records.map((r) => r.asker)])].sort();
}

/**
 * The hero's facts line (designer m_39021): how many wait (the accent's part) and the oldest's age,
 * '3 waiting · the oldest 2 h ago'; nothing waits: its words alone (no accent). Pure.
 */
export function heroFacts(cards: readonly Pick<Card, 'since_ms'>[], now: number): { wait?: string; rest: string } {
  if (cards.length === 0) return { rest: hero.nothing_waits };
  const oldest = Math.min(...cards.map((c) => c.since_ms));
  return { wait: hero.waiting({ n: cards.length }), rest: hero.oldest({ age: age(now - oldest) }) };
}

/** The rows the filter keeps: what waits (in the hub's order, given), then the past by day, newest first. Pure. */
export function listsOf(cards: readonly Card[], records: readonly InboxRecord[], names: Readonly<Record<string, string>>, f: Filter, now: number): Lists {
  const pname = (p: string) => names[p] ?? p;
  // '567' or '#567' that is a notification's number: that row alone (notif-id, designer m_27855)
  const id = idOf(f.q);
  if (id !== null && (cards.some((c) => c.id === id) || records.some((r) => r.card === id))) {
    return listsOf(cards.filter((c) => c.id === id), records.filter((r) => r.card === id), names, { ...f, q: '' }, now);
  }
  const ws = words(f.q);
  const keepWait = f.state === 'all' || f.state === 'waiting';
  const waiting: WaitRow[] = keepWait
    ? cards
        .filter((c) => (f.project === '' || c.project === f.project) && (f.agent === '' || c.agent === f.agent))
        .filter((c) => ws.length === 0 || hasAll(`${c.question} ${shownName(c.agent)} ${c.agent} ${pname(c.project)}`, ws))
        .map((c) => ({ key: waitKey(c), card: c, glyph: c.page != null ? '¶' : '?', title: shownName(c.agent), question: c.question, project: c.project }))
    : [];
  const past = records
    .filter((r) => f.state === 'all' || (f.state === 'answered' && r.state === 'answered') || (f.state === 'withdrawn' && r.state !== 'answered'))
    .filter((r) => (f.project === '' || r.project === f.project) && (f.agent === '' || r.asker === f.agent))
    .filter((r) => ws.length === 0 || hasAll(`${r.question} ${shownName(r.asker)} ${r.asker} ${pname(r.project)} ${r.said ?? ''} ${r.reason ?? ''}`, ws))
    .slice()
    .sort((a, b) => closedMs(b) - closedMs(a) || b.card - a.card);
  const days: Day[] = [];
  for (const r of past) {
    const ms = closedMs(r);
    const d = ms > 0 ? dayOf(ms) : inbox.day_earlier;
    const row: PastRow = { key: pastKey(r), r, glyph: r.state === 'answered' ? '✓' : '–' };
    const last = days[days.length - 1];
    if (last !== undefined && last.day === d) (last.rows as PastRow[]).push(row);
    // a group label: sentence case (designer m_43052); the day word itself is the TUI's too
    else days.push({ day: d, title: sentenceCase(dayHead(ms, now)), rows: [row] });
  }
  return { waiting, days, order: [...waiting.map((w) => w.key), ...days.flatMap((d) => d.rows.map((r) => r.key))] };
}

/** Where he gave it: 'in the inbox', 'on a page', "in perf's thread" (words::where_words: the detail's, and what 800 cuts off a row). Pure. */
export function whereWords(w: Where): string {
  switch (w.kind) {
    case 'thread':
      return inbox.where_thread({ agent: w.agent });
    case 'page':
      return inbox.where_page;
    default:
      return inbox.where_inbox;
  }
}

/** A time the hub never knew: 0, or a few seconds after the epoch (a test world's 2000 ms, an old
 *  record's seconds read as ms). Before 2001 bise did not exist: such a time is no date at all. */
const KNOWN_FROM_MS = 1_000_000_000_000;
const known = (ms: number): number => (ms >= KNOWN_FROM_MS ? ms : 0);

/** When it closed: its at_ms, else when it was asked (an older hub's record without a close time);
 *  0: neither known (its day reads 'earlier', never jan 1 1970). Pure. */
export function closedMs(r: InboxRecord): number {
  return known(r.at_ms) || known(r.asked_ms);
}

const MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];

/** A day's head (words::day_head): 'today', 'yesterday', 'oct 12', 'dec 30 2024'. Pure (local time). */
export function dayHead(ms: number, now: number): string {
  if (known(ms) === 0) return inbox.day_earlier;
  if (dayOf(ms) === dayOf(now)) return inbox.day_today;
  if (dayOf(ms) === dayOf(now - 86_400_000)) return inbox.day_yesterday;
  const d = new Date(ms);
  const md = `${MONTHS[d.getMonth()] ?? ''} ${String(d.getDate())}`;
  return d.getFullYear() === new Date(now).getFullYear() ? md : `${md} ${String(d.getFullYear())}`;
}

/** His answer in words, on one line: what he wrote, the option's words when he picked one; never the digit. Pure. */
export function saidWords(r: InboxRecord): string {
  const said = (r.said ?? '').split(/\s+/u).filter((w) => w !== '').join(' ');
  if (said !== '' && !/^\d+$/u.test(said)) return said;
  const opt = r.pick_n == null ? undefined : r.options[r.pick_n - 1];
  return opt ?? said;
}

/** A waiting row's second line: 'storefront · 12 min', 'bise · page · 1 d' (its #id after it). Pure. */
export function waitEnd(w: WaitRow, names: Readonly<Record<string, string>>, now: number): string {
  const a = age(now - w.card.since_ms);
  const p = names[w.project] ?? w.project;
  return w.glyph === '¶' ? inbox.wait_end_page({ project: p, age: a }) : `${p} · ${a}`;
}

/**
 * Where a past row goes (his ask, m_24212 (5)): a page answer opens the page at its block; else
 * the thread that received his answer, at his answer. A withdrawn item has no answer: the asker's
 * thread. Pure.
 */
export type Jump =
  | { kind: 'thread'; project: string; agent: string; card: number; pos?: number }
  | { kind: 'page'; project: string; page: string; block?: string };

export function jumpOf(r: InboxRecord): Jump {
  const w = r.where;
  if (r.state === 'answered' && w?.kind === 'page') {
    return { kind: 'page', project: r.project, page: w.page, ...(w.block === undefined ? {} : { block: w.block }) };
  }
  const agent = r.thread?.agent ?? (w?.kind === 'thread' ? w.agent : r.asker);
  return { kind: 'thread', project: r.project, agent, card: r.card, ...(r.thread == null ? {} : { pos: r.thread.pos }) };
}


/** Merge each project's records (each hub's own list), one per card, newest first. Pure. */
export function mergeRecords(lists: readonly (readonly InboxRecord[])[]): InboxRecord[] {
  const by = new Map<string, InboxRecord>();
  for (const l of lists) for (const r of l) by.set(pastKey(r), r);
  return [...by.values()].sort((a, b) => closedMs(b) - closedMs(a) || b.card - a.card);
}
