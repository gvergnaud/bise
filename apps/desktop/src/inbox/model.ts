// The ⌘I inbox's state and keys, pure (no React, no store): which item is
// current, which option the keyboard is on, what was answered here. The
// screen (Inbox.tsx) draws it and runs the acts. Owner: amb-home.
//
// The keys are the TUI's card view (rust/tui/src/sb/cards.rs, its key
// handler): ↑↓ the previous / next item on an empty field, ctrl+p / ctrl+n
// with words in it too (its step(app, ±1, true)), ←→ walk the current item's
// options (none on open: a reflex ⏎ never answers), ⏎ picks the one the
// keyboard is on, or sends his typed words (a question only: an approval
// is answered by one of its options, never words), 1-9 pick at once on an
// empty field, ⌘⌫ / ctrl+x close it without answering, esc goes back.
// "done" and "overlap" take ⏎ with no words (the TUI's no_words).
//
// An item answered or closed here leaves the list at once and stays out
// until the core's next rows no longer have it (the TUI's `answered` set):
// the hub's snapshot lags the act by a moment.
import type { Card } from '@/proto';
import { CHOICE_KINDS as CHOICE_KIND_LIST } from '@/proto/app_methods.gen';
import { inbox } from '@/proto/words_inbox.gen';
import { cardKey, sortCards } from './order';

export interface InboxState {
  /** the current item's key (none: the top one) */
  readonly cur: string | null;
  /** the option the keyboard is on (none on open) */
  readonly hl: number | null;
  /** answered or closed here, still in the core's rows */
  readonly gone: ReadonlySet<string>;
}

export const START: InboxState = { cur: null, hl: null, gone: new Set() };

export type Key =
  | { k: 'up' | 'down' | 'prev' | 'next' | 'left' | 'right' | 'enter' | 'close' | 'esc' }
  | { k: 'digit'; n: number }
  | { k: 'pick'; n: number }
  | { k: 'select'; key: string };

export type Act =
  | { t: 'answer'; project: string; card: number; reply: string }
  | { t: 'close'; project: string; card: number }
  | { t: 'back' };

/** The items he sees, in the TUI's order, the ones acted on here left out. */
export function visible(cards: readonly Card[], s: InboxState): Card[] {
  return sortCards(cards).filter((c) => !s.gone.has(cardKey(c)));
}

/** Forget the acted-on keys the core no longer lists (call on each new rows). */
export function prune(s: InboxState, cards: readonly Card[]): InboxState {
  const live = new Set(cards.map(cardKey));
  const gone = new Set([...s.gone].filter((k) => live.has(k)));
  return gone.size === s.gone.size ? s : { ...s, gone };
}

/** The current item: the one he moved to, else the top one. */
export function current(list: readonly Card[], s: InboxState): Card | undefined {
  return list.find((c) => cardKey(c) === s.cur) ?? list[0];
}

/** The hub's items with numbered options about a place (bise_proto::rows::CHOICE_KINDS, generated): a
 *  digit answers them; typed words go to main, never as their answer. */
const CHOICE_KINDS: ReadonlySet<string> = new Set(CHOICE_KIND_LIST);

/** Words answer it (a question), not only its options: never an approval or a choice item, which
 *  only 1/2 (a click, ⌘⏎) decides (ambient m_8773: a typed line never merges anything). */
export function takesWords(c: Pick<Card, 'approval' | 'kind'>): boolean {
  return c.approval !== true && !CHOICE_KINDS.has(c.kind);
}

/**
 * An item's one mark, its row's and its card head's alike (designer m_45259, m_45255): 'ask', a
 * '?' in the accent, for anything that asks him (a question, an approval, a merge, a try, 'it
 * failed: retry?'); 'fail', a ✗ in err, for a failure that only reports and asks nothing; null
 * for a report that asks nothing else (done: its dot). No '!': an approval's words say it. Pure.
 */
export function markOf(c: Pick<Card, 'kind' | 'options'>): 'ask' | 'fail' | null {
  const asks = c.options.length > 0;
  if ((c.kind === 'failed' || c.kind === 'restart') && !asks) return 'fail';
  if (c.kind === 'done' && !asks) return null;
  return 'ask';
}

/**
 * What his free text does to an open item (his rule m_44704: every item takes free text instead of
 * an option, and it closes it): a question's answer; a tool call's approval ('confirm'): its
 * refusal, his text the reason the agent reads (the hub's `deny: <why>`, the TUI's Enter::Deny);
 * any other item (a merge, a try, an update, a sign-in, a batch): a steer to its agent, and the
 * item closes. Pure.
 */
export function freeWords(c: Pick<Card, 'approval' | 'kind'>): 'answer' | 'deny' | 'steer' {
  if (takesWords(c)) return 'answer';
  return c.kind === 'confirm' ? 'deny' : 'steer';
}

/** '1', '1 or 2', '1 to 3': the digits that answer n options, in words. Pure. */
export function digitWords(n: number): string {
  return n <= 1 ? '1' : n === 2 ? inbox.digits_two : inbox.digits_to({ n });
}

/** What a docked item that only its options decide waits for, for the composer's label ('the merge
 *  waits for 1 or 2'); none for an item his words answer. Pure. */
export function pickWaits(c: Pick<Card, 'approval' | 'kind' | 'options'>): string | null {
  if (takesWords(c) || c.options.length === 0) return null;
  // the item's own verb ('the merge'), else 'the approval' (designer m_26683)
  const noun = c.kind === 'merge' || c.kind === 'feature_merge' ? inbox.noun_merge : c.kind === 'review' ? inbox.noun_review : c.kind === 'feature_try' ? inbox.noun_try : c.kind === 'signin' ? inbox.noun_signin : c.approval === true ? inbox.noun_approval : c.kind;
  return inbox.waits_for({ noun, digits: digitWords(c.options.length) });
}

/** ⏎ with nothing typed and no option answers it (done, overlap). */
export function noWords(c: Card): boolean {
  return c.kind === 'done' || c.kind === 'overlap';
}

/**
 * One key on the inbox. `text`: his typed words in the current item's
 * field (trimmed by the caller's ⏎ only). The arrows and the digits act
 * only on an empty field (with text they belong to the text).
 */
export function step(s: InboxState, list: readonly Card[], key: Key, text: string): { s: InboxState; act?: Act } {
  const c = current(list, s);
  const empty = text.trim() === '';
  switch (key.k) {
    case 'esc':
      return { s, act: { t: 'back' } };
    case 'select':
      return { s: { ...s, cur: key.key, hl: null } };
    case 'up':
    case 'down':
    case 'prev':
    case 'next': {
      // ↑↓ belong to his words when there are some; ctrl+p/n never do
      if (!c || (!empty && (key.k === 'up' || key.k === 'down'))) return { s };
      const i = list.indexOf(c) + (key.k === 'down' || key.k === 'next' ? 1 : -1);
      // the TUI's step(): ↑↓ stop at the ends, ctrl+n / ctrl+p wrap around (sb/cards.rs:849)
      const wrap = key.k === 'next' || key.k === 'prev';
      const to = list[wrap ? (i + list.length) % list.length : Math.max(0, Math.min(list.length - 1, i))];
      return { s: { ...s, cur: to ? cardKey(to) : s.cur, hl: to === c ? s.hl : null } };
    }
    case 'left':
    case 'right': {
      if (!c || !empty || c.options.length === 0) return { s };
      const ns = c.options.map((o) => o.n);
      const at = s.hl === null ? -1 : ns.indexOf(s.hl);
      const i = s.hl === null ? (key.k === 'right' ? 0 : ns.length - 1) : at + (key.k === 'right' ? 1 : -1);
      return { s: { ...s, hl: ns[Math.max(0, Math.min(ns.length - 1, i))] ?? null } };
    }
    case 'digit':
      if (!c || !empty || !c.options.some((o) => o.n === key.n)) return { s };
      return answer(s, list, c, String(key.n));
    case 'pick':
      if (!c || !c.options.some((o) => o.n === key.n)) return { s };
      return answer(s, list, c, String(key.n));
    case 'enter':
      if (!c) return { s };
      if (!empty && takesWords(c)) return answer(s, list, c, text.trim());
      if (s.hl !== null) return answer(s, list, c, String(s.hl));
      if (noWords(c)) return answer(s, list, c, '');
      return { s };
    case 'close':
      if (!c) return { s };
      return { s: leave(s, list, c), act: { t: 'close', project: c.project, card: c.id } };
  }
}

function answer(s: InboxState, list: readonly Card[], c: Card, reply: string): { s: InboxState; act: Act } {
  return { s: leave(s, list, c), act: { t: 'answer', project: c.project, card: c.id, reply } };
}

/** It leaves the list: the next one becomes current (the previous at the end). */
function leave(s: InboxState, list: readonly Card[], c: Card): InboxState {
  const i = list.indexOf(c);
  const next = list[i + 1] ?? list[i - 1];
  const gone = new Set(s.gone);
  gone.add(cardKey(c));
  return { cur: next ? cardKey(next) : null, hl: null, gone };
}

/**
 * A gated call (the hub's `confirm` card: 'wants to run', '| <command>', 'reason: …', 'always: …')
 * comes with no options: they are the TUI's (sb/cards.rs confirm_shape): allow, always allow <rules>
 * here (only when the hub names rules), no; 'no' is always 3, a hard rule has no 2. The hub's gate
 * reads 1 / 2 / 3 (daemon/gate.rs answer_of). It is answered by an option, never words. Pure.
 */
export function withConfirmOptions(c: Card): Card {
  if (c.kind !== 'confirm' || c.options.length > 0) return c;
  const lines = c.question.split('\n');
  const head = lines[0] ?? '';
  const always = lines.flatMap((l) => (l.startsWith('always: ') ? [l.slice('always: '.length)] : []));
  const body = lines.flatMap((l) => (l.startsWith('|') ? [l.replace(/^\| ?/, '')] : []));
  const rerun = head.endsWith('outside the sandbox');
  const rules = always.join(', ');
  const alwaysLabel = !rerun ? inbox.always_allow({ rules }) : rules === body.join('\n').trim() ? inbox.always_outside : inbox.always_outside_rules({ rules });
  const options = [
    { n: 1, label: rerun ? inbox.rerun : inbox.allow },
    ...(always.length > 0 ? [{ n: 2, label: alwaysLabel }] : []),
    { n: 3, label: inbox.no },
  ];
  return { ...c, options, approval: true };
}
