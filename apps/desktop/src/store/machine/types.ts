// The window's state machine's shapes (fast-try p4, architect m_29689): every slice is
// step(state, facts, input) -> [state, outputs], pure. Input: one of the core's or a hub's events
// (bise_proto's CoreEv | HubEv), one of main's window-local answers to this window's own requests,
// or one of his gestures (an Intent), stamped with the shell's clock. Output: a command to the core,
// a write to his kept asides, as data; the shell (store.ts) runs them at one edge, in order.
// HubFacts: what the hubs said that several slices read (their last `projects` rows), held once,
// as it came, owned by no slice. Pure, no state. Owner: amb-core.
import type { Artifact, Mode, Part, ProjectRow } from "@/proto";
import type { OptAct } from "@/proto/rows";
import type { BiseApi, CoreCmd, CoreState } from "@/shared/api";
import type { SendRefused, Sent } from "../echo";
import type { Acted } from "../acted";
import type { AnswerFrom, AnsweredFrom, CardAnswered } from "../answered";
import type { Refused } from "../refused";
import type { Ev } from "../types";

/**
 * Main's window-local events (main/core/ipc.ts), the settling of this window's own card requests:
 * card/answer's result (its `answer` is bise_proto's CardAnswer) and the error to a card/answer or
 * card/close request (the RPC error's words).
 */
export type WinEv =
  | ({ readonly ev: "card_answered" } & CardAnswered)
  | { readonly ev: "card_refused"; readonly project: string; readonly card: number; readonly text: string }
  /** turn/send's result (A1): his send `cid` has its message id (null: the hub made none) */
  | ({ readonly ev: "sent" } & Sent)
  /** the core answered his send `cid` with an error of its own (late, gone) or the hub's (queue-drawer-fix) */
  | ({ readonly ev: "send_refused" } & SendRefused)
  /** main's connection to the core changed (CorePort.onState); the facts hold the new state already */
  | { readonly ev: "core"; readonly state: CoreState };

/** His gestures. */
export type Intent =
  | {
      readonly i: "answer";
      readonly project: string;
      readonly card: number;
      readonly reply: string;
      readonly parts?: readonly Part[];
      readonly from?: AnswerFrom;
    }
  | { readonly i: "close"; readonly project: string; readonly card: number }
  /** 'try again' on a refused card: what he sent, sent again */
  | { readonly i: "retry_card"; readonly project: string; readonly card: number }
  /**
   * his option `n` of a card: its act when the hub's row gives it one (nothing sent), else his answer;
   * `tab`: an artifact it opens goes in the panel's tab only (picked on the inbox page: his list stays)
   */
  | { readonly i: "pick"; readonly project: string; readonly card: number; readonly n: number; readonly from?: AnswerFrom; readonly tab?: true }
  /** an act the hub gave (a page's chip, a page answer's jump): open it, nothing sent */
  | {
      readonly i: "open";
      readonly project: string;
      readonly act: OptAct;
      /** in the panel's tab only, the window stays where it is (a card's chip: its card stays open to answer) */
      readonly tab?: true;
    }
  /** his words to an agent: shown at once as sending, then sent (none while the core is down) */
  | {
      readonly i: "send";
      readonly project: string;
      readonly agent: string;
      readonly text: string;
      readonly mode?: Mode;
      /** his images and files go as parts (the hub stores each image once) */
      readonly parts?: readonly Part[];
      /** the window it came from, for the hub's hint ('capsule': the spotlight) */
      readonly via?: string;
      /** the thread he typed it in when not `agent`'s (at-start: his leading `@agent`): the hub leaves one route row there */
      readonly from?: string;
      /** the still the spotlight showed (the core's shot id): the core puts it first, as an image part */
      readonly shot?: number;
    }
  /** 'try again' on his failed message of thread `key`: the same words, mode and parts, under a new cid */
  | { readonly i: "resend"; readonly key: string; readonly cid: number }
  /** he drops his message `cid` of thread `key` (its row shows 'dismiss' when it failed) */
  | { readonly i: "dismiss"; readonly key: string; readonly cid: number };

/**
 * One input to the machine; `at_ms` is the shell's clock when it came (step never reads one).
 * A gesture also carries `cid`, the next of the window's one counter of request ids (the shell's,
 * shared with its /restart and dictation ids): a step that makes a request uses it and says so.
 */
export type Input = { readonly at_ms: number } & (
  | { readonly in: "ev"; readonly ev: Ev }
  | { readonly in: "win"; readonly ev: WinEv }
  | { readonly in: "do"; readonly intent: Intent; readonly cid: number }
);

/** A write to his kept asides (kept across a reload), typed per key. */
export type Keep =
  | { readonly key: "acted"; readonly value: readonly Acted[] }
  | { readonly key: "refused"; readonly value: readonly Refused[] }
  | { readonly key: "answered-from"; readonly value: readonly AnsweredFrom[] };

/** The window's methods an effect may call (preload.ts's window.bise, typed by BiseApi). */
export type FxName = "openExternal";
/** An effect as data: one of window.bise's methods and its arguments, typed from BiseApi. */
export type Fx = { [K in FxName]: { readonly fx: K; readonly args: Parameters<BiseApi[K]> } }[FxName];

/** Where the window goes: an artifact's view beside the thread, at a block (`at`) or its top. */
export interface ArtifactAt {
  readonly project: string;
  readonly artifact: string;
  readonly at: string | null;
}

/** What a step asks the shell to do, in order. */
export type Output =
  | { readonly out: "cmd"; readonly cmd: CoreCmd }
  | ({ readonly out: "keep" } & Keep)
  /** to the artifact's view beside its maker's thread; `tab`: in the panel's tab, the route stays */
  | { readonly out: "nav"; readonly to: ArtifactAt; readonly tab?: true }
  | { readonly out: "fx"; readonly fx: Fx };

/**
 * What store.do(intent) did, read from its step (architect m_30501 (5)), one record for every intent:
 * `answered`: his answer went to the hub (answer, pick without an act, retry_card of an answer): a
 * view folds the card only then. `cid`: the request id of the message a send or a resend made
 * (sent, or failed at once while the core is down); absent for every other intent.
 */
export interface Did {
  readonly answered: boolean;
  readonly cid?: number;
}

/** The outputs the app's edge runs (app/edge.ts): the window's moves and window.bise's calls. */
export type Effect = Extract<Output, { out: "nav" | "fx" }>;

/** What the hubs said that several slices read, as it came, and the window's connection to its core. */
export interface HubFacts {
  /** every project's row, the last `projects` event's own array */
  readonly rows: readonly ProjectRow[];
  /** each project's artifacts, its last `artifacts` event's own items */
  readonly artifacts: Readonly<Partial<Record<string, readonly Artifact[]>>>;
  /** main's connection to the core (CorePort.onState), the one copy (architect m_30501): down until it says up */
  readonly core: CoreState;
}

export const NO_FACTS: HubFacts = { rows: [], artifacts: {}, core: "down" };

/** The facts after one of the core's or a hub's events: the same object when it names none. Pure. */
export function factsOf(f: HubFacts, ev: Ev): HubFacts {
  if (ev.ev === "projects") return { ...f, rows: ev.projects };
  if (ev.ev === "artifacts") return { ...f, artifacts: { ...f.artifacts, [ev.project]: ev.items } };
  return f;
}
