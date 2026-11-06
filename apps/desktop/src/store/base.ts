// The store's base (store.ts's split, ambient-lead m_22445): its slices, threads and listeners,
// the core's port, and the one way out to the core (command). Reads in reads.ts, voice in voice.ts,
// the events and his actions in store.ts.
import type { ArchivedAgent, Entry } from "@/proto";
import type { CoreCmd, CorePort } from "@/shared/api";
import { artDiffKey, diffErrKey, diffKey, type Ev, type FilesAnswer, type ThreadOrder } from "./types";
import type { Ask } from "./window";
import { NO_FACTS, type HubFacts } from "./machine/types";

export const EMPTY: readonly never[] = [];

export interface Thread {
  view: ThreadOrder;
  byPos: Map<number, Entry>;
  typing: string;
  ask?: Ask; // the window's page on its way (window.ts)
  /** subscribed, its newest page not in yet: what the view holds may be old (thread-loading) */
  syncing?: boolean;
}

export type Listener = () => void;

/**
 * Where the window keeps its own facts about his answers (where he made them: answered.ts; the
 * cards he acted on: acted.ts; the refused ones: refused.ts) across a reload and a relaunch: they are
 * the window's own, no hub has them. The app passes its keep (hooks.ts); tests a map or nothing.
 */
export interface AsideKeep {
  read(key: string): unknown;
  write(key: string, v: unknown): void;
}

export interface StoreOptions {
  readonly now?: () => number;
  readonly asides?: AsideKeep;
}

export class StoreBase {
  protected readonly listeners = new Map<string, Set<Listener>>();
  protected readonly threads = new Map<string, Thread>();
  /** @internal for apply.ts */ readonly slices = new Map<string, unknown>();
  protected readonly evListeners = new Set<(ev: Ev) => void>();
  /** what the hubs said that several slices read (machine/types.ts): one copy, as it came */
  /** @internal for store.ts */ facts: HubFacts = NO_FACTS;
  /** the window's one counter of request ids: a send's cid (machine/sends.ts takes the next), a /restart's, a dictation's */
  protected cid = 0;
  /** R8: when this window last sent a /model (a setup_first after it is his; a replayed one isn't) */
  protected modelSentAt = Number.NEGATIVE_INFINITY;
  /** the cids of this window's /restart lines: the hub's notice that answers one is a restart's */
  /** @internal for apply.ts */ readonly restartCids = new Set<number>();
  /** tool_out asks in flight (by key), and each project's last one (an error names no call) */
  /** @internal for apply.ts */ readonly toolAsks = new Set<string>();
  /** @internal for apply.ts */ readonly toolAsk = new Map<string, string>();
  /** each project's last queued_take (its error carries no id: it answers that one) */
  /** @internal for apply.ts */ readonly takeAsk = new Map<string, string>();
  /**
   * the diff asks this window sent with a `req` (none today: the review asks without one); a diff
   * that answers another `req` (a terminal's `/diff` the hub broadcasts) is for the view that
   * asked, never the review's diff of the same agent (lead m_16528)
   */
  /** @internal for apply.ts */ readonly diffReqs = new Set<number>();
  /**
   * each project's last diff ask from this window (its slice key): a refused `diff` (main's
   * onRefused, an `error` with cmd 'diff') lands on that view, never stays 'loading…' (pr-changes-blank)
   */
  /** @internal for apply.ts */ readonly diffAsked = new Map<string, string>();
  /** the dictations this window started and not over yet (bar I) */
  /** @internal for apply.ts */ readonly dictating = new Set<string>();
  /** the `@` queries on their way: rid -> its project and resolve; each project's newest rid */
  protected rid = 0;
  protected readonly asked = new Map<
    number,
    { project: string; resolve: (a: FilesAnswer | null) => void }
  >();
  protected readonly newestRid = new Map<string, number>();
  /** agents-light: each project's archived light list as its hub last sent it (store/archived.ts) */
  protected readonly archivedKept = new Map<string, readonly ArchivedAgent[]>();
  /** each project's newest ⌘F over a thread (findInThread): its thread and rid, for a refusal */
  /** @internal for apply.ts */ readonly finding = new Map<string, { key: string; rid: number }>();
  /** commands asked from this core (said again when a core comes up) */
  protected commandsAsked = false;
  /** the last shown set sent (null: none yet); said again to a core that comes back up */
  protected shownSet: readonly string[] | null = null;
  /** the threads this window follows (subscribeThread), said again to a core that comes back up */
  protected readonly subs = new Map<
    string,
    { project: string; agent: string; limit: number | undefined }
  >();
  protected readonly now: () => number;

  constructor(
    protected readonly port: Pick<CorePort, "send"> | null = null,
    opts: StoreOptions = {},
  ) {
    this.now = opts.now ?? Date.now;
    this.asideKeep = opts.asides ?? null;
  }
  /** @internal for store.ts */ readonly asideKeep: AsideKeep | null;

  // ---- subscriptions

  subscribe(key: string, fn: Listener): () => void {
    let set = this.listeners.get(key);
    if (set === undefined) {
      set = new Set();
      this.listeners.set(key, set);
    }
    set.add(fn);
    return () => {
      set.delete(fn);
    };
  }

  /** Every applied event, after the store changed (one-shot things: open, route, job_end, error). */
  onEvent(fn: (ev: Ev) => void): () => void {
    this.evListeners.add(fn);
    return () => {
      this.evListeners.delete(fn);
    };
  }

  /** @internal for apply.ts */ emit(key: string): void {
    const set = this.listeners.get(key);
    if (set === undefined) return;
    for (const fn of [...set]) fn();
  }

  /** @internal for apply.ts */ put(key: string, value: unknown): void {
    this.slices.set(key, value);
    this.emit(key);
  }

  protected slice<T>(key: string, empty: T): T {
    return (this.slices.get(key) as T | undefined) ?? empty;
  }

  // ---- actions (typed commands to the core)

  command(cmd: CoreCmd): void {
    // a new talk starts with nothing heard and no still
    if (cmd.cmd === "talk_start" && this.slices.get("heard") != null)
      this.put("heard", null);
    if (cmd.cmd === "talk_start" && this.slices.get("shot") != null)
      this.put("shot", null);
    if (cmd.cmd === "slash" && cmd.line.trim().startsWith("/model "))
      this.modelSentAt = this.now();
    // a review's or a .diff artifact's ask: a refusal comes back for it; asked again, its last one goes
    if (cmd.cmd === "diff" && cmd.req == null) {
      const key = cmd.artifact != null ? artDiffKey(cmd.project, cmd.artifact, cmd.version ?? 0) : cmd.agent != null ? diffKey(cmd.project, cmd.agent, cmd.commit ?? undefined) : null;
      if (key !== null) {
        this.diffAsked.set(cmd.project, key);
        if (this.slices.get(diffErrKey(key)) != null) this.put(diffErrKey(key), null);
      }
    }
    // his own /restart: its cid comes back on the hub's line, so the row knows it answers a restart
    // (ambient-lead m_22833) without reading the hub's words
    if (cmd.cmd === "slash" && /^\/restart(\s|$)/u.test(cmd.line.trim())) {
      this.cid += 1;
      const cid = cmd.cid ?? this.cid;
      this.restartCids.add(cid);
      this.port?.send({ ...cmd, cid });
      return;
    }
    this.port?.send(cmd);
  }

  /** R8: this window sent a /model in the last `ms` (then forgets it: one setup_first per send). */
  takeModelSent(ms: number): boolean {
    const mine = this.now() - this.modelSentAt <= ms;
    this.modelSentAt = Number.NEGATIVE_INFINITY;
    return mine;
  }
}
