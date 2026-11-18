// The inbox block's rows (Block.tsx; the thread's own strip, Strip.tsx, went with the dock in bise
// fait mac), pure: how many rows fit, a row's words, the ctrl+N that opens row N, and the keys on
// an open card. Owner: nm-composer.
//
// The TUI's (rust/tui/src/sb/card_draw.rs): a row per item numbered in the inbox's order (row N is
// what ctrl+N opens), at most 3 rows then '+ n more'. The desktop moved ahead of it (his pick A on
// page inbox-scroll, designer m_66505): its block holds every item in a box that scrolls in place,
// up to BLOCK_ROWS rows tall. Not ported: approvals that came together sharing a row (order.ts says why).
import type { Card } from '@/proto';
import { inbox } from '@/proto/words_inbox.gen';
import { aboutOf } from './about';

/** The block's box is at most this many rows tall; more items scroll in it (designer m_66505). */
export const BLOCK_ROWS = 5;

/** ctrl+1 to ctrl+9 open rows 1 to 9; rows 10+ have no number, a click opens them. */
export const STRIP_KEYS = 9;

/** The block + the composer leave the thread at least this share of the window (designer m_26234). */
export const THREAD_MIN = 0.4;

export interface Fit {
  /** the window's height */
  readonly win: number;
  /** the folded block's height and what is under it, its box of rows left out (its head, the composer…) */
  readonly fixed: number;
  /** one row's height */
  readonly row: number;
  /** the items waiting */
  readonly items: number;
}

/**
 * How many rows tall the folded block's box is, the tallest that leaves the thread THREAD_MIN of the
 * window: every item when they fit in BLOCK_ROWS, else 4.5, 3.5, 2.5, 1.5 (the last row cut in half,
 * the cue that more scroll under it), then one whole row, never a lone half (designer m_26234,
 * m_66505). Pure.
 */
export function fitRows(f: Fit): number {
  if (f.items <= 1) return f.items;
  const whole = f.items <= BLOCK_ROWS ? [f.items] : [];
  const cut = Array.from({ length: BLOCK_ROWS - 1 }, (_, i) => BLOCK_ROWS - i - 0.5).filter((l) => l < f.items);
  const fits = (lines: number) => f.win - (f.fixed + lines * f.row) >= THREAD_MIN * f.win;
  return [...whole, ...cut].find(fits) ?? 1;
}

export interface InBox {
  /** the box's scrollTop and its visible height */
  readonly scrollTop: number;
  readonly view: number;
  /** the item's top in the box's content and its height */
  readonly top: number;
  readonly height: number;
}

/**
 * The box's scrollTop that shows the item, moved as little as it can (nearest): the same when it is
 * in view, its top at the box's top when it is above, its bottom at the box's bottom when it is under
 * (a card taller than the box: its top). Never the page's scroll (scrollIntoView moves every scrolling
 * ancestor, the thread's too). Pure.
 */
export function scrollToShow(b: InBox): number {
  if (b.top < b.scrollTop || b.height > b.view) return b.top;
  if (b.top + b.height > b.scrollTop + b.view) return b.top + b.height - b.view;
  return b.scrollTop;
}

export interface CtrlDigit {
  readonly code: string;
  readonly metaKey: boolean;
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
}

/** ctrl+1-9 (ctrl alone, read by `code`): the row it opens; none: not the strip's key. */
export function stripKey(e: CtrlDigit): number | undefined {
  if (!e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return undefined;
  const m = /^Digit([1-9])$/.exec(e.code);
  return m ? Number(m[1]) : undefined;
}

export interface RowWords {
  /** the agent alone, on every row: its sign says what it is (designer m_26234) */
  readonly who: string;
  /** a tool call's command: '$ ' before it, in accent */
  readonly cmd: boolean;
  /** its first line ('npm publish', 'a.rs +1 file · +3 −1', 'make test · 3 lines'), '' when none */
  readonly what: string;
  /** the hub's second line ('approved · checks pass'), faint after it; '' when none */
  readonly note: string;
}

/**
 * A row's words as the TUI's box_row and its shape (rust/tui/src/sb/card_draw.rs, cards.rs): the
 * agent (never the TUI's 't3 failed' title: designer m_26234), then a tool call's command (`$ npm publish`, `· n lines` when it has more), a patch's first file and
 * its +/− counts, else the first line that has words; the hub's note after it. Pure.
 */
export function rowWords(c: Card): RowWords {
  const who = c.agent;
  const note = (c.note ?? '').replace(/\s+/gu, ' ').trim();
  const nl = String.fromCharCode(10);
  if (c.kind !== 'approval') {
    const first = c.question.split(nl).map((l) => l.trim()).find((l) => l !== '') ?? '';
    // a newer bise (update-dot, designer m_46058): bise asks 'v2026.10.3 is ready'
    const what = c.kind === 'update' ? first.replace(/^bise /u, '') : first;
    return { who, cmd: false, what, note };
  }
  // an approval: the command (or a patch) up to the first blank line, then its reason
  const head = c.question.replace(/^\n+|\n+$/gu, '').split(nl + nl)[0] ?? '';
  const lines = head.split(nl);
  if (head.startsWith('diff --git') || head.startsWith('--- ')) {
    const files = lines.filter((l) => l.startsWith('+++ ')).map((l) => l.slice(4).trim().replace(/^b\//u, ''));
    const plus = lines.filter((l) => l.startsWith('+') && !l.startsWith('+++')).length;
    const minus = lines.filter((l) => l.startsWith('-') && !l.startsWith('---')).length;
    const more = files.length > 1 ? ` ${inbox.more_files({ n: files.length - 1 })} ·` : '';
    return { who, cmd: false, what: `${files[0] ?? inbox.a_file}${more} +${String(plus)} −${String(minus)}`, note };
  }
  const first = (lines[0] ?? '').trim();
  return { who, cmd: first !== '', what: lines.length > 1 ? inbox.cmd_lines({ first, n: lines.length }) : first, note };
}

type CardKey = { t: 'pick' | 'on'; n: number } | { t: 'close' } | { t: 'later' };

/** ⌘O on an open card about an artifact: open it beside (notif-artifact), his words typed or not
 * (designer m_47680: ⌘O types nothing). Pure. */
export function openOnKey(e: Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey' | 'altKey'>, c: Pick<Card, 'artifact' | 'page'>): boolean {
  return e.metaKey && !e.ctrlKey && !e.altKey && e.key.toLowerCase() === 'o' && aboutOf(c) != null;
}

/** A key on the open card (no words typed): its act, or none. Pure. */
export function cardKeyOf(e: Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>, c: Card, on: number | null): CardKey | null {
  if (e.key === 'Escape') return { t: 'later' };
  if (e.metaKey && !e.ctrlKey && !e.altKey && e.key === 'Backspace') return { t: 'close' };
  if (e.metaKey || e.ctrlKey || e.altKey) return null;
  const ns = c.options.map((o) => o.n);
  if (/^[1-9]$/.test(e.key) && ns.includes(Number(e.key))) return { t: 'pick', n: Number(e.key) };
  if ((e.key === 'ArrowRight' || e.key === 'ArrowLeft') && ns.length > 0) {
    const at = on === null ? -1 : ns.indexOf(on);
    const i = on === null ? (e.key === 'ArrowRight' ? 0 : ns.length - 1) : at + (e.key === 'ArrowRight' ? 1 : -1);
    return { t: 'on', n: ns[Math.max(0, Math.min(ns.length - 1, i))] ?? ns[0] ?? 1 };
  }
  return null;
}
