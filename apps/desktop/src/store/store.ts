// The one store of bise desktop: fed by the core's typed events (bise-proto's
// HubEv | CoreEv, src/proto), read by React through hooks.ts. Pure: no React,
// no Electron; the core is a CorePort (window.bise.core, or a fake in tests).
//
// The rule it keeps (amb-win m_9478): a change re-renders only who reads it.
// Every slice has its own key and listeners; an object a slice gives out is
// replaced only when that slice changed, so useSyncExternalStore sees the
// same reference otherwise. A thread is a key `${project}/${agent}`: its
// order (positions), its entries by position (a `thread` page merges, an
// `entry` replaces only that entry), typing, and his pending sends. Its order
// may hold one detached window around a far entry (jumpTo / newerPage, the gap
// to the tail: window.ts).
//
// The store is becoming a pure state machine slice by slice (fast-try p4,
// machine/): a slice is step(state, facts, input) -> [state, outputs], and
// this class is its shell: it holds the state, stamps the clock, runs the
// outputs at one edge (stepIn: a kept list is put, told and written, a command
// goes to the core) and tells the listeners. HubFacts (machine/types.ts) holds
// the hubs' projects rows once, read by every slice. Moved so far: the cards
// slice (machine/cards.ts: his answers and closes in flight and how they
// settle); his gestures on it come in as store.do(intent).
//
// Unknown tags (a newer core, the capsule's older events) are ignored at
// runtime: only the tags of TAGS are applied, and TAGS is checked complete
// against the generated unions at compile time. The store's shapes, TAGS and
// its key and compare helpers live in types.ts (re-exported here).
import type {
  Entry,
  Mode,
  Part,
} from "@/proto";
import { cardRefusedOf, type Acted } from "./acted";
import type { Refused } from "./refused";
import { cardsInit, inboxView, stepCards, type AnswerJump, type CardsState } from "./machine/cards";
import { pendingOf, sendingNow, sendsInit, stepSends, type SendsState } from "./machine/sends";
import type { InboxItem } from "@/inbox/listed";
import { factsOf, type Did, type Effect, type Input, type Intent, type Output, type WinEv } from "./machine/types";
import type { CoreState } from "@/shared/api";
import {
  type Ev,
  type EvOf,
  type FilesAnswer,
  type HubLine,
  type Pending,
  type ThreadOrder,
  sameJson,
  sameOrder,
  sameStrings,
  threadKey,
  EMPTY_ORDER,
  typed,
} from "./types";
import { applyTyped } from "./apply";
import { answers, listsLive, merged, planJump, planNewer } from "./window";
import { trim } from "./trim";
import { mergeArchived } from "./archived";
import { sendRefusedOf, sentOf } from "./echo";
import type { Thread } from "./base";
import { cardAnsweredOf, cardKeyOf, type AnsweredFrom } from "./answered";
import { StoreVoice } from "./voice";

export { DICTATE_STOP_WAIT_MS } from "./voice";
export type { StoreOptions } from "./base";

export type { AnswerJump } from "./machine/cards";

export class Store extends StoreVoice {
  /** the cards slice (machine/cards.ts): his answers and closes in flight, how they settled; his kept lists read back */
  private cardsState: CardsState = cardsInit(this.asideKeep === null ? null : (k) => this.asideKeep?.read(k));
  /** the sends slice (machine/sends.ts): his messages on their way, how they settled */
  private sendsState: SendsState = sendsInit();
  /** the inbox's view as last read, and what it was made of (inboxItems()) */
  private inboxMemo: { rows: unknown; acted: unknown; refused: unknown; items: readonly InboxItem[] } | null = null;
  /** the app's edge for the machine's effects (onEffect) */
  private readonly effects = new Set<(e: Effect) => void>();

  // ---- the core's events

  /** Applies one raw event from the core; false when it isn't one the store knows. */
  apply(raw: unknown): boolean {
    // A1: the window-local 'sent' (main's ipc.ts): his send `cid` has its message id. Always a known event.
    const sent = sentOf(raw);
    if (sent !== null) return this.settled({ ev: "sent", ...sent });
    const late = sendRefusedOf(raw);
    if (late !== null) return this.settled({ ev: "send_refused", ...late });
    // main's window-local answers to this window's own card requests (machine/types.ts WinEv)
    const done = cardAnsweredOf(raw);
    if (done !== null) return this.settled({ ev: "card_answered", ...done });
    const refused = cardRefusedOf(raw);
    if (refused !== null) return this.settled({ ev: "card_refused", ...refused });
    const ev = typed(raw);
    if (ev === null) return false;
    this.applyTyped(ev);
    for (const fn of [...this.evListeners]) fn(ev);
    return true;
  }

  private settled(ev: WinEv): true {
    this.stepIn({ at_ms: this.now(), in: "win", ev });
    return true;
  }

  private applyTyped(ev: Ev): void {
    applyTyped(this, ev);
  }

  /** The core went up or down: down fails every send still on its way ('not sent · retry'). */
  setCore(state: CoreState): void {
    if (state === this.facts.core) return;
    this.facts = { ...this.facts, core: state };
    this.emit("core");
    // a new core knows nothing of the window: what it shows is said again
    if (state === "up") this.sayAgain();
    if (state !== "down") return;
    this.commandsAsked = false;
    // no page comes from a core that is down (the screen says the core is down); asked again when it is up
    for (const [key, t] of this.threads) this.setSyncing(key, t, false);
    for (const a of this.asked.values()) a.resolve(null);
    this.asked.clear();
    // every send still on its way fails (machine/sends.ts)
    this.stepIn({ at_ms: this.now(), in: "win", ev: { ev: "core", state } });
  }

  /**
   * A (re)started core knows nothing of this window (qa-native m_9736): what it
   * shows, then every thread it follows, are said again (their pages come back
   * and merge: an entry already held is replaced only if it changed).
   */
  private sayAgain(): void {
    if (this.shownSet !== null)
      this.command({ cmd: "shown", projects: [...this.shownSet] });
    if (this.slices.has("commands") || this.commandsAsked) {
      this.commandsAsked = false;
      this.askCommands();
    }
    for (const s of this.subs.values()) {
      // a window's page asked of the old core never comes
      const key = threadKey(s.project, s.agent);
      const t = this.thread(key);
      delete t.ask;
      this.setSyncing(key, t, true);
      this.command({
        cmd: "subscribe",
        project: s.project,
        agent: s.agent,
        ...(s.limit === undefined ? {} : { limit: s.limit }),
      });
    }
  }

  /** @internal for apply.ts */ thread(key: string): Thread {
    let t = this.threads.get(key);
    if (t === undefined) {
      t = { view: EMPTY_ORDER, byPos: new Map(), typing: "" };
      this.threads.set(key, t);
    }
    return t;
  }

  /** @internal for apply.ts */ page(ev: EvOf<"thread">): void {
    const key = threadKey(ev.project, ev.agent);
    const t = this.thread(key);
    for (const e of ev.entries) this.putEntry(key, t, e);
    // his message can come back in a page, not as an entry event: its row goes the same way (machine/sends.ts)
    this.stepIn({ at_ms: this.now(), in: "ev", ev });
    // a page of older entries keeps the newer ones; its before/more are the oldest page's; the
    // window's page (a jump's, a newer one's) moves the gap (window.ts)
    const pos = ev.entries.map((e) => e.pos);
    const ask = t.ask !== undefined && answers(t.ask, t.view, pos) ? t.ask : null;
    if (ask !== null) delete t.ask;
    this.setView(key, t, merged(t.view, pos, ev.before, ev.more, ask));
    // the first page after a subscribe that answers no window ask is its newest one: the view is
    // current (the hub answers a connection's commands in order)
    if (ask === null) this.setSyncing(key, t, false);
  }

  /** The thread's newest page is on its way, or came (thread-loading: the list's cue). */
  private setSyncing(key: string, t: Thread, on: boolean): void {
    if ((t.syncing === true) === on) return;
    t.syncing = on;
    this.emit(`sync:${key}`);
  }

  /** @internal for apply.ts: a subscribe the hub refused (no such agent): its threads wait for nothing. */
  subscribeFailed(project: string): void {
    for (const s of this.subs.values()) {
      if (s.project !== project) continue;
      const key = threadKey(s.project, s.agent);
      this.setSyncing(key, this.thread(key), false);
    }
  }

  private setView(key: string, t: Thread, v: ThreadOrder): void {
    const o = t.view;
    const g = (x: ThreadOrder) => (x.gap === undefined ? "" : `${String(x.gap.lo)}/${String(x.gap.hi)}/${String(x.gap.from)}`);
    if (o.loaded && o.before === v.before && o.more === v.more && g(o) === g(v) && sameOrder(o.order, v.order)) return;
    t.view = v;
    this.emit(`order:${key}`);
  }

  /** @internal for apply.ts */ entryEv(ev: EvOf<"entry">): void {
    const key = threadKey(ev.project, ev.agent);
    const t = this.thread(key);
    const known = t.byPos.has(ev.entry.pos);
    this.putEntry(key, t, ev.entry);
    // a live entry lands in the tail; one that falls in the window's gap waits for its page
    if (!known && listsLive(t.view, ev.entry.pos)) {
      const order = [...t.view.order, ev.entry.pos].sort((a, b) => a - b);
      this.setView(key, t, { ...t.view, order, loaded: true });
    }
    // his message landed: its row goes (machine/sends.ts)
    if (ev.entry.kind === "you") this.stepIn({ at_ms: this.now(), in: "ev", ev });
  }

  private putEntry(key: string, t: Thread, e: Entry): void {
    const old = t.byPos.get(e.pos);
    if (old !== undefined && sameJson(old, e)) return;
    t.byPos.set(e.pos, e);
    this.emit(`entry:${key}:${String(e.pos)}`);
    // a steer's receipt came (amb-feed 9633cd2a: the entry is re-sent with steered_at): the rows
    // move (the window shows it where the agent read it), so the order's readers recompute
    if (
      old !== undefined &&
      t.view.loaded &&
      (old.steered_at ?? null) !== (e.steered_at ?? null)
    ) {
      t.view = { ...t.view };
      this.emit(`order:${key}`);
    }
  }

  /**
   * One input through the cards slice (machine/cards.ts), then its outputs at the edge, in order:
   * a kept list is put (its listeners told) and written to his asides, a command goes to the core;
   * a window move or a window.bise call goes to the app's edge (onEffect, app/edge.ts); a new jump
   * is told last, then the inbox's view when its rows, acted or refused changed.
   */
  private stepIn(input: Input, rowsCame = false): readonly Output[] {
    const was = this.cardsState;
    const [next, cardOuts] = stepCards(was, this.facts, input);
    this.cardsState = next;
    const sendsWas = this.sendsState;
    const [sends, sendOuts] = stepSends(sendsWas, this.facts, input);
    this.sendsState = sends;
    // the window's one counter of request ids moves past the one a send took (machine/types.ts Input)
    if (sends.cid > this.cid) this.cid = sends.cid;
    // a thread whose rows changed is told before its send goes, as the rows show at once
    if (sends.threads !== sendsWas.threads)
      for (const [key, t] of Object.entries(sends.threads)) if (t.pending !== pendingOf(sendsWas, key)) this.emit(`pending:${key}`);
    const outs = sendOuts.length === 0 ? cardOuts : [...cardOuts, ...sendOuts];
    for (const o of outs) {
      switch (o.out) {
        case "cmd":
          this.command(o.cmd);
          break;
        case "keep":
          this.emit(o.key);
          this.asideKeep?.write(o.key, o.value);
          break;
        case "nav":
        case "fx":
          for (const fn of [...this.effects]) fn(o);
          break;
      }
    }
    if (next.jump !== was.jump) this.emit("answer_jump");
    if (rowsCame || next.acted !== was.acted || next.refused !== was.refused) this.emit("inbox_items");
    return outs;
  }
  /** The app's edge (app/edge.ts) runs the machine's window moves and window.bise calls; a test records them. */
  onEffect(fn: (e: Effect) => void): () => void {
    this.effects.add(fn);
    return () => {
      this.effects.delete(fn);
    };
  }
  /** The inbox's one view (machine/cards.ts inboxView); the same array until its rows, acted or refused change. */
  inboxItems(): readonly InboxItem[] {
    const s = this.cardsState;
    const m = this.inboxMemo;
    if (m !== null && m.rows === this.facts.rows && m.acted === s.acted && m.refused === s.refused) return m.items;
    const items = inboxView(s, this.facts);
    this.inboxMemo = { rows: this.facts.rows, acted: s.acted, refused: s.refused, items };
    return items;
  }
  /** His gesture (machine/types.ts Intent), through the machine. */
  do(intent: Intent): Did {
    const cid = this.cid + 1;
    const outs = this.stepIn({ at_ms: this.now(), in: "do", intent, cid });
    // what it did, from its outputs: a view folds the card only when his answer went (architect m_29867)
    const answered = outs.some((o) => o.out === "cmd" && o.cmd.cmd === "answer");
    // a send or a resend made a row under the next request id, sent or failed at once (core down)
    return this.sendsState.cid === cid ? { answered, cid } : { answered };
  }
  /**
   * The cards he answered or closed in this window that the hub's rows still list (acted.ts):
   * every view leaves them out. Kept across a reload, as his asides.
   */
  acted(): readonly Acted[] {
    return this.cardsState.acted;
  }
  /**
   * A page's item kept open across the views (notif-artifact, designer m_26776): the inbox block that
   * opens the page beside, or unmounts with that item open, sets it; the block that mounts reads it
   * and clears it once its list holds the item. A view slice: the window's own, never the hub's.
   */
  keepOpen(at: { readonly project: string; readonly card: number } | undefined): void {
    this.slices.set("reopen", at);
  }
  keptOpen(): { readonly project: string; readonly card: number } | undefined {
    return this.slice<{ readonly project: string; readonly card: number } | undefined>("reopen", undefined);
  }
  /** @internal for apply.ts: an event the hubs' facts keep (machine/types.ts factsOf); its slice key told */
  factsCame(ev: Ev, key: string): void {
    this.facts = factsOf(this.facts, ev);
    this.emit(key);
  }
  /** @internal for apply.ts: a projects event: the hub's rows, then the cards its rows no longer list leave the set */
  rowsCame(ev: EvOf<"projects">): void {
    this.facts = factsOf(this.facts, ev);
    this.emit("projects");
    this.stepIn({ at_ms: this.now(), in: "ev", ev }, true);
  }
  /**
   * The cards whose answer or close was refused in this window (refused.ts): each view that draws
   * the card says 'not sent: <words> · try again' on it. Kept across a reload, as his asides.
   */
  refused(): readonly Refused[] {
    return this.cardsState.refused;
  }

  /** Where he made each answer this window sent (window-local, oldest first; kept across a reload). */
  answersFrom(): readonly AnsweredFrom[] {
    return this.cardsState.answersFrom;
  }
  /** Where he made his answer to card `card` of `project`, when this window sent it. */
  answeredFrom(project: string, card: number): AnsweredFrom | null {
    const k = cardKeyOf(project, card);
    return this.answersFrom().find((a) => a.key === k) ?? null;
  }
  /** The newest jump to an answer the hub took (answer-in-thread A); null before any. */
  answerJump(): AnswerJump | null {
    return this.cardsState.jump;
  }

  /** A message of his is on its way (a pending row still sending): the app's update waits for it (update/moment.ts). */
  sending(): boolean {
    return sendingNow(this.sendsState);
  }
  /** Thread `key`'s rows of his messages not echoed yet (machine/sends.ts), the same array until they change. */
  pending(key: string): readonly Pending[] {
    return pendingOf(this.sendsState, key);
  }

  /** @internal for apply.ts: his send's error (G), with its cid or none (machine/sends.ts) */
  sendFailed(ev: EvOf<"error">): void {
    this.stepIn({ at_ms: this.now(), in: "ev", ev });
  }

  /**
   * An `@` query's rows, ranked by the core (the TUI's files.rs). Resolves
   * with the first answer (partial while the first walk runs: the fuller one
   * then lands in useFiles(project)), or null when a newer query of the
   * project replaced it or the core went down.
   */
  files(
    project: string,
    q: string,
    limit?: number,
  ): Promise<FilesAnswer | null> {
    return this.askFiles(project, q, limit);
  }
  /** R14: he took `path` (as `files` answered it; a file, never a folder he browsed) from `project`'s @ list: it ranks first in that list's next answers. */
  filePicked(project: string, path: string): void {
    this.command({ cmd: "file_picked", project, path });
  }
  private askFiles(
    project: string,
    q: string,
    limit?: number,
  ): Promise<FilesAnswer | null> {
    this.rid += 1;
    const rid = this.rid;
    for (const [r, a] of this.asked) {
      if (a.project !== project) continue;
      this.asked.delete(r);
      a.resolve(null);
    }
    this.newestRid.set(project, rid);
    return new Promise((resolve) => {
      this.asked.set(rid, { project, resolve });
      this.command({
        cmd: "files",
        project,
        q,
        rid,
        ...(limit === undefined ? {} : { limit }),
      });
    });
  }
  /** The newest `@` answer of a project (a partial one, then the full one). */
  filesNow(project: string): FilesAnswer | null {
    return this.slice<FilesAnswer | null>(`files:${project}`, null);
  }

  /** @internal for apply.ts */ agentsCame(ev: EvOf<"agents">): void {
    // agents-light: the live rows and the archived list (kept when the event has none), one slice
    const m = mergeArchived(ev.agents, ev.archived, this.archivedKept.get(ev.project) ?? []);
    this.archivedKept.set(ev.project, m.kept);
    this.put(`agents:${ev.project}`, m.rows);
  }

  /** @internal for apply.ts */ filesAnswer(ev: EvOf<"files">): void {
    // an older query's late answer never replaces a newer one's
    if (ev.rid !== this.newestRid.get(ev.project)) return;
    const a: FilesAnswer = {
      q: ev.q,
      items: ev.items,
      folder: ev.folder ?? null,
      partial: ev.partial ?? false,
    };
    this.put(`files:${ev.project}`, a);
    const asked = this.asked.get(ev.rid);
    if (asked === undefined) return;
    this.asked.delete(ev.rid);
    asked.resolve(a);
  }

  /**
   * His words to an agent: shown at once as pending, then sent. Returns its cid.
   * His images and files go as parts (the hub stores each image once).
   */
  send(a: {
    project: string;
    agent: string;
    text: string;
    mode?: Mode;
    parts?: readonly Part[];
    /** where it came from, for the hub's hint ('capsule': the spotlight) */
    via?: string;
    /** the still the spotlight showed (the core's shot id): the core puts it first */
    shot?: number;
    /** the thread he typed it in when not `agent`'s (at-start): the hub leaves one route row there */
    from?: string;
  }): number {
    // Sugar over store.do (machine/sends.ts): its 8 callers move to store.do and it goes when
    // fast-try's p4 resumes (paused after slice 2 by Gabriel's word: no new refactors)
    return this.do({ i: "send", ...a }).cid ?? 0;
  }

  /** A failed send goes again (same words, same mode), as a new pending row. Sugar over store.do, goes with send. */
  retry(key: string, cid: number): void {
    this.do({ i: "resend", key, cid });
  }

  /** Drops a failed send (he gave up on it). Sugar over store.do, goes with send. */
  dismiss(key: string, cid: number): void {
    this.do({ i: "dismiss", key, cid });
  }

  /**
   * The projects the window shows now (on load and each route change, amb-win
   * m_9717): the core holds their hubs and sends their rows, agents and cards
   * only after the first one. Sent only when the set changed.
   */
  shown(projects: readonly string[]): void {
    const set = [...new Set(projects)];
    if (this.shownSet !== null && sameStrings(this.shownSet, set)) return;
    this.shownSet = set;
    this.command({ cmd: "shown", projects: set });
  }

  /** A thread's newest page then its live entries; kept, and asked again of a core that comes back up. */
  subscribeThread(project: string, agent: string, limit?: number): void {
    const key = threadKey(project, agent);
    this.subs.set(key, { project, agent, limit });
    this.setSyncing(key, this.thread(key), true);
    this.command({
      cmd: "subscribe",
      project,
      agent,
      ...(limit === undefined ? {} : { limit }),
    });
  }
  unsubscribeThread(project: string, agent: string): void {
    const key = threadKey(project, agent);
    this.subs.delete(key);
    const t = this.threads.get(key);
    if (t !== undefined) this.setSyncing(key, t, false);
    this.command({ cmd: "unsubscribe", project, agent });
  }
  /** The page before the oldest entry held (nothing when there is no more). */
  olderPage(project: string, agent: string, limit?: number): void {
    const v = this.threadOrder(threadKey(project, agent));
    if (!v.more || v.before === null) return;
    this.command({
      cmd: "page",
      project,
      agent,
      before: v.before,
      ...(limit === undefined ? {} : { limit }),
    });
  }
  /**
   * Bring the entry at `pos` into the view (his answer weeks back, a find's hit): 'held' with the
   * row's pos when the view has it; 'asked' when its page is on its way (the view's window, with a
   * gap row to the tail, window.ts); 'wait' while the thread isn't loaded. Again with the page
   * there: 'held'. One window page on its way at a time: while one is (the window's newer page,
   * another jump's), 'asked' and nothing sent; ask again when the view changes. Replaced before
   * its answer, that page would land in the tail without its gap (find-walk-red: ⌘F's ⏎ while the
   * gap row paged newer, the list jumped to rows no match was in).
   */
  jumpTo(project: string, agent: string, pos: number): { kind: "held"; pos: number } | { kind: "asked" | "wait" } {
    const key = threadKey(project, agent);
    const t = this.thread(key);
    const plan = planJump(t.view, pos);
    if (plan.kind !== "ask") return plan;
    if (t.ask !== undefined) return { kind: "asked" };
    t.ask = plan.ask;
    this.setView(key, t, plan.view);
    this.command({ cmd: "page", project, agent, before: plan.ask.before, limit: plan.limit });
    return { kind: "asked" };
  }
  /** The window's next newer page, toward the tail (its gap row came into view); one at a time. */
  newerPage(project: string, agent: string): void {
    const t = this.thread(threadKey(project, agent));
    if (t.ask !== undefined) return;
    const plan = planNewer(t.view);
    if (plan === null) return;
    t.ask = plan.ask;
    this.command({ cmd: "page", project, agent, before: plan.ask.before, limit: plan.limit });
  }
  /**
   * He reads the entry at `pos` (the list's row under its top edge, as he scrolls): the entries far
   * from it leave the view and the store (trim.ts), the hub pages them back when he scrolls there.
   * Not while a window page is asked.
   */
  reading(project: string, agent: string, pos: number): void {
    const key = threadKey(project, agent);
    const t = this.threads.get(key);
    if (t === undefined || t.ask !== undefined) return;
    const { view, dropped } = trim(t.view, pos);
    if (dropped.length === 0) return;
    for (const p of dropped) t.byPos.delete(p);
    this.setView(key, t, view);
  }
  /**
   * ⌘F over a whole thread (`thread/find`, thread-perf): the hub searches its transcript and its
   * `thread_hits` comes back as the thread's `hits:` slice (useThreadHits). Its rid: an answer for
   * an older one is stale. A hub without the method refuses it: the slice says so (hits null).
   */
  findInThread(project: string, agent: string, needle: string): number {
    this.rid += 1;
    const rid = this.rid;
    this.finding.set(project, { key: threadKey(project, agent), rid });
    this.command({ cmd: "find", project, agent, needle, rid });
    return rid;
  }
  /** The hub's line he saw on the key line is over (he typed, sent, esc, its time): it goes, unless a newer one came. */
  dropNotice(project: string, line: HubLine): void {
    if (this.notice(project) === line) this.put(`notice:${project}`, null);
  }
  stop(project: string, agent: string): void {
    this.command({ cmd: "stop", project, agent });
  }
  /**
   * A file he gives an agent (a drop on its thread, the artifacts list's '+ add' or drop): the
   * hub's do: add (the TUI's /artifacts add) makes it an artifact of `agent`, by you; the window
   * never writes the store. The hub's words come back as a notice, the row in `artifacts`.
   */
  addArtifact(project: string, agent: string, path: string): void {
    this.command({ cmd: "artifacts_add", project, agent, target: path });
  }
}

// the store's shapes and helpers live in types.ts; importers keep reading them here
export {
  TAGS,
  artDiffKey,
  diffErrKey,
  diffKey,
  takeKey,
  threadKey,
  toolOutKey,
  typed,
} from "./types";
export type {
  Away,
  AwayProject,
  BranchesList,
  Dictation,
  Diff,
  Ev,
  EvOf,
  FilesAnswer,
  HubLine,
  PagesList,
  Pending,
  PrsList,
  InboxList,
  QueuedTake,
  TalkShot,
  ThreadHits,
  ThreadOrder,
  ToolOut,
  VersionsList,
  VoiceModeState,
} from "./types";
