// The store's shapes: the typed event union, TAGS (every tag the store applies,
// checked complete at compile time), the slices' list types, and the key and
// compare helpers. Pure, no state. Split out of store.ts (pure move). Owner:
// amb-core (the hub slices with amb-win).
import type { AwayProject, CoreEv, FindHit, HubEv, InboxRecord, Mode, Part, Pr, PickFile, PickFolder, BranchRow, Page, VersionItem } from '@/proto';
export type Ev = HubEv | CoreEv;
export type EvOf<T extends Ev['ev']> = Extract<Ev, { ev: T }>;

/** Every typed tag the store applies; the compiler checks it names each tag of HubEv | CoreEv, no more. */
export const TAGS = {
  welcome: true,
  agents: true,
  cards: true,
  thread: true,
  thread_hits: true,
  entry: true,
  typing: true,
  partial: true,
  artifacts: true,
  // the library (artifacts-folders): a hub's folders after an agent's edit, the core's after his;
  // a hub's answer to artifacts/move and to artifacts/library (the terminal's reads)
  folders: true,
  library_folders: true,
  artifacts_moved: true,
  library: true,
  scheduled: true,
  worktrees: true,
  merged: true,
  features: true,
  prs: true,
  // the answer to his Merge or Close on a PR's tab (pr/act): its request's result, never pushed
  pr_acted: true,
  // a PR tab's detail and a check's log (pr/detail, pr/checkLog): results, never pushed
  pr_detail: true,
  pr_check_log: true,
  models: true,
  tool_out: true,
  // the answer to queued_take (turn/unqueue): his taken-back words, to the asking window (main 34c35175)
  queued_taken: true,
  // the answer to log (log/read): a page of an agent's raw session, read through onEvent by the
  // window's /debug screen (main 0c227ae3); `hits` with a find
  log: true,
  // the answer to completion (log/completion): a request of the agent's ring, /debug completion
  completion: true,
  // the answer to log_body (log/body): a cut /debug body whole, its `of` echoed (log/whole.ts)
  log_body: true,
  diff: true,
  route: true,
  route_done: true,
  jobs: true,
  job_end: true,
  followed_end: true,
  activity: true,
  approvals: true,
  notice: true,
  error: true,
  projects: true,
  open: true,
  prefs: true,
  accounts: true,
  found: true,
  away_summary: true,
  plugins: true,
  plugins_summary: true,
  // a source's plugins to add (the Mistral catalog, a marketplace)
  browse: true,
  plugin_preview: true,
  computer_use: true,
  dictation: true,
  voice_mode: true,
  voice_answer: true,
  level: true,
  heard: true,
  shot: true,
  commands: true,
  skills: true,
  files: true,
  roles: true,
  // settings · Setup's gauge (page models-setup): known; its page reads it
  setup_gauge: true,
  signing: true,
  provider_check: true,
  app_update: true,
  // update-flow: a newer bise for the machine (the status pill's), folded by the core
  machine_update: true,
  // a project's own line (hub/bise_update): the core folds it into machine_update
  bise_update: true,
  hub_refused: true,
  index: true,
  setup_first: true,
  keychain: true,
  computer_live: true,
  computer_peek: true,
  refused: true,
  // client-protocol's newer HubEv tags (main 0c3be6b6), applied since amb-win's (amb-feed m_16785):
  // state per project (branches, versions, flow, pages, release, update), moments read through
  // onEvent (page_changed, card_open, focused)
  branches: true,
  card_open: true,
  // MCP Apps C9: a terminal's `app ↗`, read through onEvent (the views host opens its tab)
  app_open: true,
  flow: true,
  focused: true,
  page_changed: true,
  pages: true,
  release: true,
  update: true,
  versions: true,
  // inbox-history pick A (inbox-record d22e2c0ff): a page of a hub's answered and withdrawn
  // records, and each new one as it closes (the inbox screen, inbox-screen's src/inbox)
  inbox_history: true,
  inbox_recorded: true,
  // cost-pause (main 09b6d551): the hub's pause state and its usage sums; recognised only, the
  // settings pause/usage screen draws them later (amb-home)
  paused: true,
  usage: true,
  // the hub's version switch (/reload, /restart; architect m_20576): one status line under the
  // title bar, words from Rust's switch_line (amb-home draws it)
  switch: true,
  growth: true,
  // the H1 sidebar: a project's pulse (rows::Pulse); known, kept nowhere: the core folds it into
  // ProjectRow.pulse (apply.ts)
  pulse: true,
  // running processes (procs-hub's proto, docs/processes.md): known; procs-desktop applies them
  processes: true,
  proc_out: true,
  // the core can't start any hub (its runtime is missing): Rust's words, drawn by CoreLine
  broken: true,
  // the core's capsule and agents-panel lines, typed in CoreEv (docs/issues/24): known, not
  // applied (no window reads them; 'sent' is also the window-local echo, echo.ts reads its own shape)
  state: true,
  main: true,
  hub: true,
  sent: true,
  agent_sent: true,
  // main's ipc.ts gives it to its window as 'sent' or 'send_refused' (never seen raw here)
  send_answered: true,
  word: true,
  phase: true,
  page: true,
  agent_entry: true,
  agent_history: true,
  agent_preview: true,
  agent_queued: true,
  agent_typing: true,
  fake_voice: true,
} as const satisfies Record<Ev['ev'], true>;

/** A raw event from the core as a typed one, or null (not an object, or a tag the store doesn't know). */
/** fn's talk's still (core `shot`, ambient-lead m_21912): its stored file, the front app and window. */
export interface TalkShot {
  readonly file: string;
  /** its number: the spotlight's send names it (`shot`), its close drops only it */
  readonly id: number | null;
  readonly app: string | null;
  readonly title: string | null;
}

/** A project's branches, the hub's answer to branches (the /diff branch picker; amb-feed m_16785). */
export interface BranchesList {
  readonly base: string;
  readonly rows: readonly BranchRow[];
}

/** bise's versions as its hub sees them (/version: current, the list, dev/installed). */
export interface VersionsList {
  readonly current: string;
  readonly dev: boolean;
  readonly installed: boolean;
  readonly items: readonly VersionItem[];
}

/** His pages, newest first, and how many of his promises are late (never a card). */
export interface PagesList {
  readonly items: readonly Page[];
  readonly overdue: number;
}

/** A project's PR list (the prs event without its tag). */
/** A hub's answered and withdrawn records (inbox/history), newest first, with its totals. */
export interface InboxList {
  readonly items: readonly InboxRecord[];
  /** older records match too (ask again with `before` the last card) */
  readonly more: boolean;
  readonly answered: number;
  readonly withdrawn: number;
}

export interface PrsList {
  readonly head: string;
  readonly items: readonly Pr[];
  /** the hub's words when none is open */
  readonly none: string | null;
}

export function typed(raw: unknown): Ev | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const tag = (raw as { ev?: unknown }).ev;
  if (typeof tag !== 'string' || !Object.hasOwn(TAGS, tag)) return null;
  return raw as Ev;
}

export const threadKey = (project: string, agent: string): string => `${project}/${agent}`;

/** The hub's last line to this window for a project: a plain line (a notice: /flow, a confirm's no) or an error (a refused slash line: his words to fix). */
export interface HubLine {
  readonly text: string;
  readonly level: 'note' | 'error';
  /** the answer to this window's own /restart (its cid): the row adds the app's version when it is not the hub's */
  readonly restart?: true;
  /** a /restart's line once the hub said hello again: its words are over, only the app's staged build still shows */
  readonly back?: true;
}

/** His send, shown at once, until its 'you' entry lands (FIFO by text) or it fails. */
export interface Pending {
  readonly cid: number;
  readonly project: string;
  readonly agent: string;
  readonly text: string;
  readonly mode: Mode;
  /** msg-parts: his message typed, sent with it (a retry sends them again) */
  readonly parts?: readonly Part[];
  /** the thread he typed it in when not `agent`'s (at-start): a retry leaves its trace there too */
  readonly from?: string;
  readonly at_ms: number;
  /** late: the core gave up waiting for its hub's answer (30 s), which may still make it: drawn as
   *  sending, no retry; its end comes as the core's late 'sent' (queue-drawer-fix) */
  readonly state: 'sending' | 'late' | 'failed';
  readonly why?: string;
  /** A1: the message id the hub answered the send with (echo.ts); absent: not answered yet (a
   *  send answered with no message has no row: send-reason) */
  readonly msgId?: number;
  /** when the core went down before its answer (the window's ms): 'not sent', yet it may have
   *  reached the hub before that (echo.ts echoOf takes it by its words, from an entry written by
   *  then; by its cid once docs/issues/33 is done) */
  readonly lostAt?: number;
}

/**
 * The hub's answer to ⌘F over a whole thread (`thread_hits`, store.findInThread): every entry with
 * a match up to line `upto`, oldest first. `hits` null: this hub has no thread/find (older), the
 * window searches what it has loaded.
 */
export interface ThreadHits {
  readonly rid: number;
  readonly hits: readonly FindHit[] | null;
  readonly upto: number;
}

/** A thread's list, as the list draws it: replaced only when its order, before or more change. */
export interface ThreadOrder {
  readonly order: readonly number[];
  readonly before: number | null;
  readonly more: boolean;
  /** a first page arrived */
  readonly loaded: boolean;
  /**
   * A detached window around a far entry is held (window.ts): the positions between `lo` (its
   * newest) and `hi` (the tail's oldest) are not; the list draws its gap row there, and pages
   * newer from `from` until it meets the tail. Absent: one run.
   */
  readonly gap?: { readonly lo: number; readonly hi: number; readonly from: number };
}

/** A thread not loaded yet. */
export const EMPTY_ORDER: ThreadOrder = {
  order: [],
  before: null,
  more: false,
  loaded: false,
};

export type Diff = EvOf<'diff'>;
export type Away = EvOf<'away_summary'>;
/** An `@` query's answer (core/picks.rs, the TUI's files.rs ranking). */
export interface FilesAnswer {
  readonly q: string;
  readonly items: readonly PickFile[];
  /** the browsed folder's own row (`@src/`) */
  readonly folder: PickFolder | null;
  /** the workspace's first walk still runs: a fuller answer follows */
  readonly partial: boolean;
}

/**
 * A dictation (bar I): `text` is the whole text so far (replace the
 * composer's dictated part, never append); `final`: over (stopped, the time
 * limit, fn's talk, cancelled, or `error`: refused or failed).
 */
export interface Dictation {
  readonly text: string;
  readonly final: boolean;
  readonly cancelled: boolean;
  readonly error: string | null;
  /** a final with no words or an error whose take the core kept: dictateRetry transcribes it
   *  again, dictateDrop lets it go (amb-core 276074b2) */
  readonly kept?: boolean;
  /** a final's one line about its words (the voice model is gone, another of its provider's
   *  heard them): said like an error, the words land */
  readonly note?: string;
}
/** Voice mode's state (core/voice_mode.rs), only in the window that turned it on. */
export type VoiceModeState = EvOf<'voice_mode'>;
export type { AwayProject };

/** The listener key of a diff slice (hooks.ts subscribes with it). */
/** A tool call's output: its real lines and whether they were cut, or the hub's refusal. */
/** total: the whole output's bytes, only when the hub read the session log (amb-feed a928ab09). */
export type ToolOut = { readonly out: string; readonly cut: boolean; readonly total?: number } | { readonly error: string };

/** The slice key of one call's output (pos is the key: ToolItem.id repeats). Pure. */
/** A queued message taken back: its words, or the hub's refusal. */
export type QueuedTake = { readonly text: string; readonly parts?: readonly Part[] } | { readonly error: string };

export function takeKey(project: string, agent: string, id: number): string {
  return `queued_taken:${project}/${agent}/${String(id)}`;
}

export function toolOutKey(project: string, agent: string, pos: number): string {
  return `tool_out:${project}/${agent}/${String(pos)}`;
}

export function diffKey(project: string, agent: string, commit?: string): string {
  return commit === undefined ? `diff:${project}/${agent}` : `diff:${project}/@${commit}`;
}

/** A `.diff` artifact's diff at its version (the `diff` ask's `artifact`, try-diff-open). */
export function artDiffKey(project: string, id: string, version: number): string {
  return `diff:${project}/#${id}@${String(version)}`;
}

/** The hub's refusal of the diff at `key` (a diffKey or artDiffKey): its words, until asked again. */
export function diffErrKey(key: string): string {
  return `err:${key}`;
}

export function sameStrings(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((x, i) => x === b[i]);
}

export function sameOrder(a: readonly number[], b: readonly number[]): boolean {
  return a.length === b.length && a.every((x, i) => x === b[i]);
}

export function sameJson(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/**
 * His message as sent, from its typed parts, to match a pending send with its echo: the words
 * (spaces collapsed), each paste, quote and file by its content, each image by its name (the hub
 * fills its b64_file and mime on the kept part). Pure.
 */
export function sentParts(parts: readonly Part[]): string {
  return parts
    .map((p) => {
      switch (p.kind) {
        case 'text':
          return p.text;
        case 'image':
          return `\u0000image:${p.name}\u0000`;
        case 'file':
          return `\u0000file:${p.path}\u0000`;
        // a plugin he picked with @ (at-plugins)
        case 'plugin':
          return `\u0000plugin:${p.name}\u0000`;
        // a part of an artifact: which one, which version, where (the hub may add its rev)
        case 'anchor':
          return `\u0000anchor:${p.artifact}:${String(p.version)}:${JSON.stringify(p.anchor)}\u0000`;
        // a forward's header (the hub's only, never in a send of his): the message it names
        case 'forwarded':
          return `\u0000forwarded:${String(p.id)}\u0000`;
        default:
          return `\u0000${p.kind}:${p.text}\u0000`;
      }
    })
    .join('')
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * His words as sent, to match a pending send with its echo: the hub's image markers
 * (`<image name="[Image #1]" path=… mime=… b64=…>`, rust/images marker()) and the composer's
 * '[Image #n]' labels out, and the same for files (a link to an absolute path,
 * `[deck.pptx](/u/deck.pptx)[ (missing)]`, bise_proto parts::file_marker; '[File #n]'), spaces
 * collapsed. Pure.
 */
export function sentWords(text: string): string {
  return text
    .replace(/<image name="[^"]*" path="[^"]*" mime="[^"]*" b64="[^"]*">/g, ' ')
    .replace(/\[(?:\\.|[^\]\\\n])*\]\((?:<(?:\\.|[^>\\\n])*>|\/[^)\n]*)\)(?: \(missing\))?/g, ' ')
    .replace(/\[(?:Image|File) #\d+\]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

