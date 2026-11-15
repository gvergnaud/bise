// The sends slice of the window's machine (fast-try p4 slice 2, architect m_30501): his messages
// on their way and how they settle, as step(state, facts, input) -> [state, outputs]. What it holds
// is the window's own: each thread's rows of his messages not echoed yet (sending, or failed with
// why) and the ids of his messages that landed before their send's answer. How a row settles is
// the hub's to say, read with echo.ts's rules, never one of its own: turn/send's result gives the
// row its message id (A1, main's window-local 'sent'), his 'you' entry with that id (or a page
// holding it) takes it away, the send's error with its cid fails it (refused) or takes it away
// (undelivered: the hub's entry stands in), and the core going down (HubFacts.core) fails every
// row still on its way. A send while the core is down sends nothing and fails at once.
// Kept as today, with its end: 'sends-oldest' (an error without a cid fails the project's oldest
// row on its way: no path without a cid was found, architect m_30504). Its deletion, with feature
// test 7 rewritten (architect m_30540), waits until fast-try's p4 resumes (paused after slice 2 by
// Gabriel's word: no new refactors).
// An input that changes nothing gives the same state back and no output. Pure. Owner: amb-core.
import type { Entry } from "@/proto";
import { common } from "@/proto/words.gen";
import { answered, echoOf, keepLanded, type SendRefused, type Sent } from "../echo";
import { threadKey, type Ev, type EvOf, type Pending } from "../types";
import type { HubFacts, Input, Intent, Output } from "./types";

/** One thread's sends: its rows of his messages not echoed yet, and his ids that landed first. */
export interface ThreadSends {
  readonly pending: readonly Pending[];
  /** A1: his message ids that landed before their send's answer (the last 32, echo.ts keepLanded) */
  readonly landed?: readonly number[];
}

export interface SendsState {
  /** by thread key (`${project}/${agent}`), in the order he first sent to each */
  readonly threads: Readonly<Record<string, ThreadSends>>;
  /** the last request id a send took from the window's counter (Input's `cid`): the shell moves its counter to it */
  readonly cid: number;
}

const NONE: readonly never[] = [];
const NO_ROWS: readonly Pending[] = [];

export const sendsInit = (): SendsState => ({ threads: {}, cid: 0 });

/** Thread `key`'s rows of his messages not echoed yet, oldest first (the same array until they change). */
export function pendingOf(s: SendsState, key: string): readonly Pending[] {
  return s.threads[key]?.pending ?? NO_ROWS;
}

/** A message of his is still on its way (a row sending, or late): the app's update waits for it. */
export function sendingNow(s: SendsState): boolean {
  return Object.values(s.threads).some((t) => t.pending.some(onItsWay));
}

/** A row not settled yet: sending, or late (the core gave up waiting, its hub may still make it). */
const onItsWay = (p: Pending): boolean => p.state === "sending" || p.state === "late";

type Step = readonly [SendsState, readonly Output[]];

/** One input. `f`: the facts, already updated with this input's event or the core's state. Pure. */
export function stepSends(s: SendsState, f: HubFacts, input: Input): Step {
  switch (input.in) {
    case "ev":
      return evCame(s, input.ev);
    case "win":
      if (input.ev.ev === "sent") return [sentCame(s, input.ev), NONE];
      if (input.ev.ev === "send_refused") return [sendRefused(s, input.ev, input.at_ms), NONE];
      if (input.ev.ev === "core") return [input.ev.state === "down" ? failAll(s, common.not_sent, input.at_ms) : s, NONE];
      return [s, NONE];
    case "do":
      return does(s, f, input.intent, input.at_ms, input.cid);
  }
}

function withThread(s: SendsState, key: string, t: ThreadSends): SendsState {
  return { ...s, threads: { ...s.threads, [key]: t } };
}

function rowsOf(s: SendsState, key: string): ThreadSends {
  return s.threads[key] ?? { pending: NO_ROWS };
}

/** The core's or a hub's event: his echo (an entry, or a page's entries) or his send's error. */
function evCame(s: SendsState, ev: Ev): Step {
  switch (ev.ev) {
    case "entry":
      return ev.entry.kind === "you" ? [landed(s, threadKey(ev.project, ev.agent), ev.entry), NONE] : [s, NONE];
    case "thread": {
      // his message can come back in a page (a thread he opened after sending, a reconnection's
      // page): its row goes the same way (ambient-lead m_20138)
      const key = threadKey(ev.project, ev.agent);
      if (rowsOf(s, key).pending.length === 0) return [s, NONE];
      let next = s;
      for (const e of ev.entries) if (e.kind === "you") next = landed(next, key, e);
      return [next, NONE];
    }
    case "error":
      return [sendFailed(s, ev), NONE];
    default:
      return [s, NONE];
  }
}

/**
 * His message landed: its row goes, by its id (A1, echo.ts). No row, or a lost row taken by its
 * words: its id is kept, so a send answered with it later leaves at once (review-refactor m_30778).
 */
function landed(s: SendsState, key: string, e: Entry): SendsState {
  const t = rowsOf(s, key);
  const i = echoOf(t.pending, e);
  const ids = i < 0 || t.pending[i]?.msgId !== e.msg_id ? keepLanded(t.landed, e.msg_id) : t.landed;
  const kept: ThreadSends = ids === t.landed ? t : { ...t, ...(ids === undefined ? {} : { landed: ids }) };
  if (i < 0) return kept === t ? s : withThread(s, key, kept);
  return withThread(s, key, { ...kept, pending: t.pending.filter((_, j) => j !== i) });
}

/** A1: turn/send's result for his send `cid` (echo.ts answered). */
function sentCame(s: SendsState, sent: Sent): SendsState {
  const key = threadKey(sent.project, sent.agent);
  const t = s.threads[key];
  const next = t && answered(t.pending, sent, new Set(t.landed));
  return t === undefined || next == null ? s : withThread(s, key, { ...t, pending: next });
}

/**
 * The core answered his send `cid` with an error (queue-drawer-fix, architect m_34889). Late: its
 * hub said nothing in 30 s and may still make it: the row turns late (drawn sending, no retry),
 * its end comes as the core's late 'sent' (then his echo by its id) or 'send_refused' gone. Gone:
 * its hub's connection went (or 10 min passed): 'not sent', with lostAt as for a core gone (echo.ts
 * takes it by its words from an entry written by then). Refused: the hub's own error event with
 * the cid settles it (sendFailed). Only a row with no message id yet.
 */
function sendRefused(s: SendsState, r: SendRefused, at: number): SendsState {
  if (r.reason === "refused") return s;
  const key = threadKey(r.project, r.agent);
  const t = s.threads[key];
  const p = t?.pending.find((x) => x.cid === r.cid);
  if (t === undefined || p === undefined || !onItsWay(p) || typeof p.msgId === "number") return s;
  const next: Pending = r.reason === "late" ? { ...p, state: "late" } : { ...p, state: "failed", why: common.not_sent, lostAt: at };
  if (next.state === p.state) return s;
  return withThread(s, key, { ...t, pending: t.pending.map((x) => (x.cid === r.cid ? next : x)) });
}

/**
 * G: his send failed. With its cid: refused, his row shows 'not delivered' with retry; undelivered,
 * the hub wrote its undelivered entry in the thread, which stands in for his row (gone). Without a
 * cid (an older core, shim 'sends-oldest'): the oldest row on its way of that project fails.
 */
function sendFailed(s: SendsState, ev: EvOf<"error">): SendsState {
  if (ev.cmd !== "send") return s;
  if (ev.cid === undefined || ev.cid === null) return failOldest(s, ev.project ?? null, ev.text);
  const cid = ev.cid;
  const reason = ev.reason ?? "refused";
  for (const [key, t] of Object.entries(s.threads)) {
    if (!t.pending.some((p) => p.cid === cid)) continue;
    const pending =
      reason === "undelivered"
        ? t.pending.filter((p) => p.cid !== cid)
        : t.pending.map((p) => (p.cid === cid ? { ...p, state: "failed" as const, why: ev.text } : p));
    return withThread(s, key, { ...t, pending });
  }
  return s;
}

function failOldest(s: SendsState, project: string | null, why: string): SendsState {
  let oldest: { key: string; t: ThreadSends; p: Pending } | null = null;
  for (const [key, t] of Object.entries(s.threads))
    for (const p of t.pending) {
      if (p.state !== "sending" || (project !== null && p.project !== project)) continue;
      if (oldest === null || p.cid < oldest.p.cid) oldest = { key, t, p };
    }
  if (oldest === null) return s;
  const { key, t, p } = oldest;
  return withThread(s, key, { ...t, pending: t.pending.map((x) => (x.cid === p.cid ? { ...x, state: "failed" as const, why } : x)) });
}

/**
 * The core went down at `at`: every row still on its way fails ('not sent · retry'). One not
 * answered yet may have reached the hub by then: lostAt (echo.ts takes it by its words, send-reason).
 */
function failAll(s: SendsState, why: string, at: number): SendsState {
  let next = s;
  for (const [key, t] of Object.entries(s.threads)) {
    if (!t.pending.some(onItsWay)) continue;
    const pending = t.pending.map((p) =>
      !onItsWay(p) ? p : { ...p, state: "failed" as const, why, ...(typeof p.msgId === "number" ? {} : { lostAt: at }) },
    );
    next = withThread(next, key, { ...t, pending });
  }
  return next;
}

/** His gesture on his messages. */
function does(s: SendsState, f: HubFacts, intent: Intent, at: number, cid: number): Step {
  switch (intent.i) {
    case "send":
      return send(s, f, intent, at, cid);
    case "resend": {
      // the same words, mode and parts, as a new row under a new cid
      const t = s.threads[intent.key];
      const p = t?.pending.find((x) => x.cid === intent.cid && x.state === "failed");
      if (t === undefined || p === undefined) return [s, NONE];
      const without = withThread(s, intent.key, { ...t, pending: t.pending.filter((x) => x.cid !== intent.cid) });
      return send(
        without,
        f,
        {
          i: "send",
          project: p.project,
          agent: p.agent,
          text: p.text,
          mode: p.mode,
          ...(p.parts === undefined ? {} : { parts: p.parts }),
          ...(p.from === undefined ? {} : { from: p.from }),
          ...(p.works === undefined ? {} : { works: p.works }),
        },
        at,
        cid,
      );
    }
    case "dismiss": {
      const t = s.threads[intent.key];
      if (t === undefined || !t.pending.some((x) => x.cid === intent.cid)) return [s, NONE];
      return [withThread(s, intent.key, { ...t, pending: t.pending.filter((x) => x.cid !== intent.cid) }), NONE];
    }
    default:
      return [s, NONE];
  }
}

/** His words to an agent: a row sending at once, then the send with its cid; the core down: it fails, nothing goes. */
function send(s: SendsState, f: HubFacts, a: Extract<Intent, { i: "send" }>, at: number, cid: number): Step {
  const key = threadKey(a.project, a.agent);
  const mode = a.mode ?? "now";
  // msg-parts: his message typed (quotes, words, pastes); the hub renders the model's text from them
  const parts = a.parts !== undefined && a.parts.length > 0 ? [...a.parts] : undefined;
  const p: Pending = {
    cid,
    project: a.project,
    agent: a.agent,
    text: a.text,
    mode,
    ...(parts === undefined ? {} : { parts }),
    ...(a.from === undefined ? {} : { from: a.from }),
    ...(a.works === undefined || a.works.length === 0 ? {} : { works: [...a.works] }),
    at_ms: at,
    state: "sending",
  };
  const t = rowsOf(s, key);
  const next = { ...withThread(s, key, { ...t, pending: [...t.pending, p] }), cid };
  if (f.core === "down") return [failOldest(next, a.project, common.not_sent), NONE];
  // G: the cid goes along (echoed on its error); with parts, no text: the hub renders it (m_22248)
  return [
    next,
    [
      {
        out: "cmd",
        cmd: {
          cmd: "send",
          project: a.project,
          agent: a.agent,
          text: parts === undefined ? a.text : "",
          mode,
          ...(parts === undefined ? {} : { parts }),
          ...(a.via === undefined ? {} : { via: a.via }),
          ...(a.from === undefined ? {} : { from: a.from }),
          ...(a.shot === undefined ? {} : { shot: a.shot }),
          ...(a.works === undefined || a.works.length === 0 ? {} : { works: [...a.works] }),
          cid,
        },
      },
    ],
  ];
}
