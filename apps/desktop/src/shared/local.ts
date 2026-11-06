// The window-local events main sends the one window that asked (src/main/core/ipc.ts), built from
// the core's answer to that window's request: 'card_answered' (card/answer's result), 'sent' (a
// send's message id), 'card_refused' (an answer's or a close's error). Not the core's protocol:
// the store reads them (src/store echo.ts, answered.ts, acted.ts). Pure, so ipc.ts and the nightly
// invariant check on recorded cores (tests/nightly) build them the same way. Owner: amb-core.
import type { AnswerResult, CardAnswer, SendReason, SendResult } from '../proto';

export type LocalEv =
  | { readonly ev: 'card_answered'; readonly project: unknown; readonly card: unknown; readonly answer: CardAnswer | null }
  | { readonly ev: 'sent'; readonly project: unknown; readonly agent: unknown; readonly cid: unknown; readonly msg_id: number | null; readonly reason: SendReason | null }
  | { readonly ev: 'card_refused'; readonly project: unknown; readonly card: unknown; readonly text: string }
  | { readonly ev: 'send_refused'; readonly project: unknown; readonly agent: unknown; readonly cid: unknown; readonly reason: SendRefusal };

/**
 * Why the core answered his send with an error (queue-drawer-fix, architect m_34889), from the
 * error's typed `data.reason`, never its words: 'late' (its hub said nothing in 30 s: it may still
 * make his message, the core keeps the request and says its end as 'send_answered'), 'gone' (its
 * hub's connection went, or the core gave up after 10 min: it may or may not have it), 'refused'
 * (the hub's own refusal: its error event with the cid settles the row).
 */
export type SendRefusal = 'late' | 'gone' | 'refused';

/**
 * The core answered `cmd` with `result`: the window's event, 'card_answered' for an answer
 * (card/answer's result) or 'sent' for a send (turn/send's message id); null for a close taken
 * (the rows say it) and any other command.
 */
export function answeredEv(cmd: Readonly<Record<string, unknown>>, result: unknown): LocalEv | null {
  // a close taken: the hub's rows drop the card, nothing more to say
  if (cmd['cmd'] === 'close') return null;
  if (cmd['cmd'] === 'answer') {
    // the step closed no card (or a page took it) answers {}: answer null, no row
    const answer = (result as AnswerResult | null)?.answer ?? null;
    return { ev: 'card_answered', project: cmd['project'], card: cmd['card'], answer };
  }
  if (cmd['cmd'] === 'send') {
    // turn/send's result (bise-proto rpc SendResult): his message's id and why none was made; an
    // older hub answers {}: both null, the row goes on it (echo.ts answered)
    const r = result as SendResult | null;
    return { ev: 'sent', project: cmd['project'], agent: cmd['agent'], cid: cmd['cid'], msg_id: r?.msg_id ?? null, reason: r?.reason ?? null };
  }
  return null;
}

/** The core answered his send with `error`: the window's 'send_refused' with its typed reason; null for any other command. */
export function sendRefusedEv(cmd: Readonly<Record<string, unknown>>, error: { readonly data?: unknown }): LocalEv | null {
  if (cmd['cmd'] !== 'send') return null;
  const why = (error.data as { reason?: unknown } | null | undefined)?.reason;
  const reason: SendRefusal = why === 'late' || why === 'gone' ? why : 'refused';
  return { ev: 'send_refused', project: cmd['project'], agent: cmd['agent'], cid: cmd['cid'], reason };
}

/**
 * A refused `diff` (no such agent, git failed): the hub's typed `error` with its cmd tag, so the
 * view that asked it shows why with a retry (pr-changes-blank). Null for any other command.
 */
export function diffRefusedEv(cmd: Readonly<Record<string, unknown>>, error: { readonly message: string }): Record<string, unknown> | null {
  if (cmd['cmd'] !== 'diff') return null;
  return { ev: 'error', project: cmd['project'], cmd: 'diff', text: error.message };
}

/**
 * The core's 'send_answered' (CoreEv::SendAnswered: a send it answered late, settled by its hub at
 * last): the window's 'sent' with the hub's result, or 'send_refused' gone when it never answered.
 * Null when `ev` isn't one.
 */
export function lateAnsweredEv(ev: Readonly<Record<string, unknown>>): LocalEv | null {
  if (ev['ev'] !== 'send_answered') return null;
  const send = { cmd: 'send', project: ev['project'], agent: ev['agent'], cid: ev['cid'] };
  const result = ev['result'];
  if (result === undefined || result === null) return { ev: 'send_refused', project: send.project, agent: send.agent, cid: send.cid, reason: 'gone' };
  return answeredEv(send, result);
}

/** The core or its hub refused his answer or close of a card: that card's event, with the words. */
export function refusedEv(project: unknown, card: unknown, message: string): LocalEv {
  return { ev: 'card_refused', project, card, text: message };
}

/** A core gone before it answered his answer or close: the card is his again, with these words. */
export const RESTARTED_WORDS = "bise restarted before your answer was confirmed: if the card is still here, answer it again.";
