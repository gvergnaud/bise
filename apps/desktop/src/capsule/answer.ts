// bise's answer to his ask in the spotlight and the corner card, from what main's thread says now
// (page spotlight-reponse v2, 'la base'): its live partial, its typing, the newest agent entry
// after his ask, and whether main's turn still runs. A gap in the stream (a tool call, a partial
// cleared) never ends the answer: while main works, what came stays with its '∿', and the whole
// entry takes its place when it lands. Pure; CapsuleRoot only feeds it.
import type { SpotAnswer } from '@/spotlight';

export interface AnswerIn {
  /** the live partial's text (null: none, or cleared) */
  readonly partial: string | null;
  /** the thread's typing line ('': none) */
  readonly typing: string;
  /** the newest agent entry's text after his ask (undefined: none yet) */
  readonly reply: string | undefined;
  /** main's turn still runs (its status is working) */
  readonly working: boolean;
  /** what the answer shows now */
  readonly shown: SpotAnswer | null;
}

/** The answer to show, or null: keep what shows (nothing came yet). Pure. */
export function answerOf(i: AnswerIn): SpotAnswer | null {
  // the turn is over and its entry is here: that is the answer, whatever partial was left (a
  // clear that never came; ui-review m_57911)
  if (!i.working && i.reply !== undefined) return { text: i.reply, done: true };
  if (i.partial !== null && i.partial !== '') return { text: i.partial, done: false };
  if (i.typing !== '') return { text: i.typing, done: false };
  const kept = i.shown !== null && i.shown.note !== true && i.shown.text !== '' ? i.shown.text : undefined;
  const text = i.reply ?? kept;
  if (text === undefined) return null;
  return { text, done: !i.working };
}
