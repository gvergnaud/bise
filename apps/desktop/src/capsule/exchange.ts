// The spotlight's exchange with bise (page spotlight-reponse, Gabriel's pick B): his first ask
// shows its answer under the field; a follow-up opens the thread pane on the right, its rows his
// messages and bise's answers from main's thread since that first ask. It is kept 10 min after
// the spotlight closed (fn space brings it back as he left it), ended by the pane's ✕ or ⌘W.
// Pure: the state, its steps, and the pane's rows; CapsuleRoot holds it, ThreadPane draws it.
import type { Entry } from '@/proto';
import type { SpotAnswer } from '@/spotlight';

/** The exchange is kept this long after the spotlight closed. */
export const KEEP_MS = 10 * 60_000;

export interface Exchange {
  /** main's thread positions after this one are the exchange's */
  readonly after: number;
  /** his asks so far (the second one opens the pane) */
  readonly asks: number;
  /** the thread pane is open */
  readonly pane: boolean;
  /** when the spotlight closed with it (null: open now) */
  readonly closedAt: number | null;
}

/** He asked bise from the spotlight (the left field or the pane): the first ask starts one, a follow-up opens the pane. */
export function asked(x: Exchange | null, after: number): Exchange {
  if (x === null) return { after, asks: 1, pane: false, closedAt: null };
  return { ...x, asks: x.asks + 1, pane: true, closedAt: null };
}

/** The spotlight closed at `now`: kept KEEP_MS. */
export const closed = (x: Exchange | null, now: number): Exchange | null => (x === null ? null : { ...x, closedAt: now });

/** The spotlight opened at `now`: the exchange as he left it, or none once it expired. */
export function opened(x: Exchange | null, now: number): Exchange | null {
  if (x === null) return null;
  if (x.closedAt !== null && now - x.closedAt > KEEP_MS) return null;
  return { ...x, closedAt: null };
}

export type PaneRow =
  | { readonly kind: 'you'; readonly pos: number; readonly text: string }
  | { readonly kind: 'bise'; readonly pos: number; readonly text: string; readonly live: boolean }
  /** asked, no word back yet */
  | { readonly kind: 'wait' }
  /** a line about the answer (dim): it went to another project's thread */
  | { readonly kind: 'note'; readonly text: string };

/**
 * The pane's rows: his messages and bise's answers after `after` (no tool calls, no notices),
 * then the answer being written (`last` live) when it is not an entry yet: its words, or the wait.
 * While main works, its newest answer keeps its ∿. Pure.
 */
export function paneRows(entries: readonly Entry[], after: number, last: SpotAnswer | null): PaneRow[] {
  const rows: PaneRow[] = [];
  for (const e of entries) {
    if (e.pos <= after) continue;
    if (e.kind === 'you') rows.push({ kind: 'you', pos: e.pos, text: e.text });
    else if (e.kind === 'agent') rows.push({ kind: 'bise', pos: e.pos, text: e.text, live: false });
  }
  if (last === null) return rows;
  if (last.note === true) return [...rows, { kind: 'note', text: last.text }];
  if (last.done) return rows;
  const end = rows.at(-1);
  if (last.text === '') return end?.kind === 'bise' ? rows : [...rows, { kind: 'wait' }];
  if (end?.kind === 'bise' && end.text === last.text) return [...rows.slice(0, -1), { ...end, live: true }];
  return [...rows, { kind: 'bise', pos: -1, text: last.text, live: true }];
}

/** The newest answer's words (⌘C with nothing selected in the pane copies them), or null. Pure. */
export function lastAnswer(rows: readonly PaneRow[]): string | null {
  for (let i = rows.length - 1; i >= 0; i--) {
    const r = rows[i];
    if (r?.kind === 'bise') return r.text;
  }
  return null;
}
