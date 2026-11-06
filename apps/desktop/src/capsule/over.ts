// What the capsule says when the core ends his fn talk without a send (amb-core m_21857: his fn
// stuck on 'bise is thinking…'), from the core's events. Pure: CapsuleRoot dispatches the result.

import { W } from "./Capsule";
import { CORNER_LINGER_MS, type CapEvent, type View } from "./machine";

/** the 'over' line of a card his words answered stays this long, then the capsule goes */
export const OVER_FADE_MS = 1500;
/** 'i didn't hear anything' and the plain errors stay this long (designer m_22394) */
export const OVER_PLAIN_MS = 4000;

/**
 * How long the corner card stays before it goes by itself, the time held while the pointer is on
 * it (designer m_22394): an ended answer and a line with an action ('settings › voice')
 * CORNER_LINGER_MS, a plain 'over' line OVER_PLAIN_MS; null: it stays (a live talk, a streaming
 * answer) or has its own time (a card answered, OVER_FADE_MS). Pure.
 */
export function lingerOf(v: View, answerDone: boolean): number | null {
  if (v.kind === "answer") return answerDone ? CORNER_LINGER_MS : null;
  if (v.kind !== "talk" || v.phase !== "over" || v.fades === true) return null;
  return v.link !== undefined ? CORNER_LINGER_MS : OVER_PLAIN_MS;
}

/**
 * The core's own errors the capsule says in the app's words (designer m_21945: the capsule isn't
 * the terminal, and in the app the global main IS bise); every other error passes through.
 */
const REWORDED: Readonly<Record<string, Omit<Extract<CapEvent, { ev: "talk_over" }>, "ev">>> = {
  "voice needs a key. /voice picks one.": { said: W.needsVoiceKey, link: { label: W.voiceSettings, route: "settings/dictation" } },
  "main didn't get it: bise isn't reachable. try again in a moment.": { said: W.notReached },
};

type TalkOver = Extract<CapEvent, { ev: "talk_over" }>;

/** a final 'heard' with no words ends the talk: nothing heard */
export function heardOver(text: string, final: boolean): TalkOver | null {
  return final && text.trim() === "" ? { ev: "talk_over", said: W.heardNothing } : null;
}

/** his words answered a card of `name` (voice_answer): said, then the capsule goes */
export function answeredOver(name: string): TalkOver {
  return { ev: "talk_over", said: W.answered(name), fades: true };
}

/** an `error` with no project (the core's own, a talk's failure) ends the talk with its reason */
export function errorOver(project: string | null | undefined, cmd: string | null | undefined, text: string): TalkOver | null {
  if (project != null || (cmd != null && cmd !== "talk_start")) return null;
  return { ev: "talk_over", ...(REWORDED[text] ?? { said: text }) };
}
