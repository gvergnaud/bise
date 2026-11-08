// React's side of the store: one hook per slice, on useSyncExternalStore with
// the slice's own key, so a component re-renders only when what it reads
// changed (a new entry never re-renders the other rows or the composer).
// The store comes from StoreContext: the app's one store (bound to
// window.bise.core) by default, a test's own store through the provider.
import { createContext, createElement, useContext, useEffect, useSyncExternalStore, type ReactNode } from 'react';
import type { Account, Agent, FlowMode, Growth, ReleaseEv, SwitchRow, UpdateEv, Artifact, ComputerAgent, Card, Entry, Feature, Found, IndexRow, Job, JsonValue, Merged, Model, PickCommand, Place, PickSkill, Plugin, PluginPreview, PluginsSummary, ProjectRow, RoleRow, ScheduledTask, SetupGauge, Worktree, Folder } from '@/proto';
import type { CorePort, CoreState } from '@/shared/api';
import type { BiseUpdate, Proc } from '@/proto';
import { keep } from '@/app/keep';
import type { AnsweredFrom } from './answered';
import type { AnswerJump } from './store';
import type { Acted } from './acted';
import type { Refused } from './refused';
import type { InboxItem } from '@/inbox/listed';
import { guideKey, prActKey, prDetailKey, prDiffKey, prLogKey, type GuideEv, type PrActState } from './types';
import { artDiffKey, diffErrKey, diffKey, toolOutKey, Store, type ToolOut, type Away, type Dictation, type HubLine, type Diff, type FilesAnswer, type Ev, type EvOf, type Pending, type PrsList, type BranchesList, type VersionsList, type PagesList, type ThreadHits, type ThreadOrder, type VoiceModeState, type TalkShot } from './store';

/**
 * Feeds a store from a core port: its live events, then the state events
 * main kept from before this window loaded (asked after the listener is on:
 * IPC keeps order, so the snapshot is never older than a live event already
 * applied), and the core's state. Returns the unsubscribe.
 */
export function connect(store: Store, port: CorePort): () => void {
  const offEv = port.onEvent((ev) => {
    store.apply(ev);
  });
  const offState = port.onState((s) => {
    store.setCore(s);
  });
  void port.snapshot?.().then((evs) => {
    for (const ev of evs) store.apply(ev);
  });
  void port.state().then((s) => {
    store.setCore(s);
  });
  return () => {
    offEv();
    offState();
  };
}

let appStore: Store | null = null;

/** The app's one store, bound to window.bise.core when there is one (none in a unit test). */
export function getAppStore(): Store {
  if (appStore === null) {
    const port = (globalThis as { bise?: { core?: CorePort } }).bise?.core ?? null;
    // his answers' rows where he answered are the window's own: kept with his drafts and view
    appStore = new Store(port, {
      asides: {
        read: (k) => keep.view(k),
        write: (k, v) => {
          keep.setView(k, v);
        },
      },
    });
    if (port !== null) connect(appStore, port);
  }
  return appStore;
}

const StoreContext = createContext<Store | null>(null);

/** A store for the tree below (tests, fixture replay); without it the app's store is used. */
export function StoreProvider(props: { store: Store; children?: ReactNode }): ReactNode {
  return createElement(StoreContext.Provider, { value: props.store }, props.children);
}

export function useStore(): Store {
  return useContext(StoreContext) ?? getAppStore();
}

function useSlice<T>(key: string, read: (s: Store) => T): T {
  const store = useStore();
  return useSyncExternalStore(
    (cb) => store.subscribe(key, cb),
    () => read(store),
  );
}

export const useProjects = (): readonly ProjectRow[] => useSlice('projects', (s) => s.projects());
/** Why this core can start no hub (its runtime is missing); null: it can. */
export const useBroken = (): string | null => useSlice('broken', (s) => s.broken());
/** update-flow: a newer bise for the machine (the status pill's); none: null */
export const useBiseUpdate = (): BiseUpdate | null => useSlice('bise_update', (s) => s.biseUpdate());
export const useAgents = (project: string): readonly Agent[] => useSlice(`agents:${project}`, (s) => s.agents(project));
/** A project's open cards: its `projects` row's, the one copy every view and count reads. */
export const useCards = (project: string): readonly Card[] => useSlice('projects', (s) => s.cards(project));
export const useVoiceAnswer = (): EvOf<'voice_answer'> | null => useSlice('voice_answer', (s) => s.voiceAnswer());
export const useNotice = (project: string): HubLine | null => useSlice(`notice:${project}`, (s) => s.notice(project));
/** bar V8/W21: the approvals mode, checker and rules of a project's hub; null until it said. */
export const useApprovals = (project: string): EvOf<'approvals'> | null => useSlice(`approvals:${project}`, (s) => s.approvals(project));
export const usePrefs = (): JsonValue | null => useSlice('prefs', (s) => s.prefs());
export const useAccounts = (): readonly Account[] => useSlice('accounts', (s) => s.accounts());
/** Claude Code is signed in with a Claude plan (presence only, the accounts event). */
export const useClaudePlan = (): boolean => useSlice('claude_plan', (s) => s.claudePlan());
/** V14: the sign-in open in his browser, {id, url} (copy the link), null at its end. */
export const useSigning = (): { readonly id: string; readonly url: string } | null => useSlice('signing', (s) => s.signing());
/** provider_use's last answer (checking, then works or failed), null before any. */
export const useProviderCheck = (): EvOf<'provider_check'> | null => useSlice('provider_check', (s) => s.providerCheck());
export const useFound = (): readonly Found[] => useSlice('found', (s) => s.found());
export const useComputerUse = (): EvOf<'computer_use'> | null => useSlice('computer_use', (s) => s.computerUse());
export const usePlugins = (project?: string): readonly Plugin[] | null => useSlice(`plugins:${project ?? ''}`, (s) => s.plugins(project));
/** the plugins in numbers (the sidebar's row, the place's band); null until the core sent them */
export const usePluginsSummary = (project?: string): PluginsSummary | null => useSlice(`plugins_summary:${project ?? ''}`, (s) => s.pluginsSummary(project));
/** what a source offers to add (`mistral`, a marketplace); null until it was read */
export const useBrowse = (source: string): readonly Plugin[] | null => useSlice(`browse:${source}`, (s) => s.browse(source));
export const useBrowseSources = (): readonly string[] => useSlice('browse_sources', (s) => s.browseSources());
/** the add sheet's preview waiting for his click */
export const usePluginPreview = (): { readonly token: string; readonly preview: PluginPreview } | null => useSlice('plugin_preview', (s) => s.pluginPreview());
export const useArtifacts = (project: string): readonly Artifact[] => useSlice(`artifacts:${project}`, (s) => s.artifacts(project));
/** the library's folders, the same for every project (artifacts/folders.ts reads them) */
export const useFolders = (): readonly Folder[] => useSlice('library:folders', (s) => s.folders());
export const useScheduled = (project: string): readonly ScheduledTask[] => useSlice(`scheduled:${project}`, (s) => s.scheduled(project));
export const useWorktrees = (project: string): readonly Worktree[] => useSlice(`worktrees:${project}`, (s) => s.worktrees(project));
/** a project's processes (HubEv processes): running, then ended in the last 24 h; none until its hub said */
export const useProcesses = (project: string): readonly Proc[] => useSlice(`processes:${project}`, (s) => s.processes(project));
export const useMerged = (project: string): readonly Merged[] => useSlice(`merged:${project}`, (s) => s.merged(project));
/** a project's branches (ask with store.askBranches); null before the answer */
export const useBranches = (project: string): BranchesList | null => useSlice(`branches:${project}`, (s) => s.branches(project));
export const useVersions = (project: string): VersionsList | null => useSlice(`versions:${project}`, (s) => s.versions(project));
export const useFlow = (project: string): FlowMode | null => useSlice(`flow:${project}`, (s) => s.flow(project));
export const usePages = (project: string): PagesList | null => useSlice(`pages:${project}`, (s) => s.pages(project));
/** /update's build in bise's source tree (amb-feed m_16785): 'updating bise…', then its end */
export const useHubUpdate = (project: string): UpdateEv | null => useSlice(`update:${project}`, (s) => s.hubUpdate(project));
/** the hub's newest version switch (/reload, /restart, /version): null before the hub said one */
export const useHubSwitch = (project: string): SwitchRow | null => useSlice(`switch:${project}`, (s) => s.switchRow(project));
/** les bims: how much the project has done (hub/growth): null before the hub said */
export const useGrowth = (project: string): Growth | null => useSlice(`growth:${project}`, (s) => s.growth(project));
export const useRelease = (project: string): ReleaseEv | null => useSlice(`release:${project}`, (s) => s.release(project));
export const usePrs = (project: string): PrsList | null => useSlice(`prs:${project}`, (s) => s.prs(project));
export const useModels = (project: string): readonly Model[] => useSlice(`models:${project}`, (s) => s.models(project));
export const useFeatures = (project: string): readonly Feature[] => useSlice(`features:${project}`, (s) => s.features(project));
export const usePlaces = (project: string): readonly Place[] => useSlice(`places:${project}`, (s) => s.places(project));
export const useJobs = (project: string): readonly Job[] => useSlice(`jobs:${project}`, (s) => s.jobs(project));
export const useArtifactDiff = (project: string, id: string, version: number): Diff | null =>
  useSlice(artDiffKey(project, id, version), (s) => s.artifactDiff(project, id, version));
export const useDiff = (project: string, agent: string, commit?: string): Diff | null =>
  useSlice(diffKey(project, agent, commit), (s) => s.diff(project, agent, commit));
/** A diff's guide by the diff's hash (none: no hash, an older hub); null until the hub says. */
export const useGuide = (project: string, hash: string | null): GuideEv | null =>
  useSlice(hash === null ? 'guide:none' : guideKey(project, hash), (s) => (hash === null ? null : s.guide(project, hash)));
/** A PR's diff by its link (askPrDiff); null until the hub answers. */
export const usePrDiff = (project: string, url: string): Diff | null =>
  useSlice(prDiffKey(project, url), (s) => s.prDiff(project, url));
/** A PR tab's detail (askPrDetail): the last answer, a detail or a typed error; null before one. */
export const usePrDetail = (project: string, url: string): EvOf<'pr_detail'> | null =>
  useSlice(prDetailKey(project, url), (s) => s.prDetail(project, url));
/** A check's log (askPrCheckLog): null while it comes. */
export const usePrCheckLog = (project: string, url: string, check: string): EvOf<'pr_check_log'> | null =>
  useSlice(prLogKey(project, url, check), (s) => s.prCheckLog(project, url, check));
/** His last Merge or Close on that PR's tab: on its way, then the hub's answer; null before one. */
export const usePrActed = (project: string, url: string): PrActState | null =>
  useSlice(prActKey(project, url), (s) => s.prActed(project, url));
/** The hub's words when it refused that diff (`diffKey` or `artDiffKey`); null while none. */
export const useDiffError = (key: string): string | null =>
  useSlice(diffErrKey(key), (s) => (s.slices.get(diffErrKey(key)) as string | null | undefined) ?? null);
/** A tool call's real output once its open row asked (askToolOut), or the hub's refusal; null until then. */
export const useToolOut = (project: string, agent: string, pos: number): ToolOut | null =>
  useSlice(toolOutKey(project, agent, pos), (s) => s.toolOut(project, agent, pos));
export const useAway = (): Away | null => useSlice('away', (s) => s.away());
/** Each role's model and effort (V17), at the core's start, after each role_set, at setup_check. */
export const useRoles = (): readonly RoleRow[] => useSlice('roles', (s) => s.roles());
/** settings · Setup's gauge (page models-setup): the core's rows, drawn as they come. */
export const useSetupGauge = (): SetupGauge | null => useSlice('setup_gauge', (s) => s.setupGauge());
/** The `/` commands (the TUI's list; asked from the core on first use). */
export function useCommands(): readonly PickCommand[] {
  const store = useStore();
  useEffect(() => {
    store.askCommands();
  }, [store]);
  return useSlice('commands', (s) => s.commands());
}
/** A workspace's `$` skills (the TUI's list; read again each time a screen mounts it). */
export function useSkills(project: string): readonly PickSkill[] {
  const store = useStore();
  useEffect(() => {
    if (project !== '') store.askSkills(project);
  }, [store, project]);
  return useSlice(`skills:${project}`, (s) => s.skills(project));
}
/** The newest `@` answer of a project (store.files asks; a partial answer is followed by the full one). */
export const useFiles = (project: string): FilesAnswer | null => useSlice(`files:${project}`, (s) => s.filesNow(project));
/** fn's talk: what bise heard so far (the whole text; final once the talk ended), null before a talk. */
export const useHeard = (): { readonly text: string; readonly final: boolean } | null => useSlice('heard', (s) => s.heard());
/** fn's talk: the still of his front window it will send, null: none, dropped or gone (ambient-lead m_21912). */
export const useShot = (): TalkShot | null => useSlice('shot', (s) => s.shot());
/** A voice's loudness now, 0..1 ('you': the mic during a talk, a dictation or voice mode; 'main': its spoken answer; 'agent': voice mode's agent speaking). */
export const useLevel = (who: 'you' | 'main' | 'agent'): number => useSlice(`level:${who}`, (s) => s.level(who));
/** A dictation this window started (store.dictateStart()): its whole text so far, final, cancelled, error. */
export const useDictation = (id: string | null): Dictation | null => useSlice(`dictation:${id ?? ''}`, (s) => (id === null ? null : s.dictation(id)));
/** Why this project's hub refused this core (its words; useStore().hubRetry(project) tries again), or null. */
export const useHubRefused = (project: string): string | null => useSlice(`hub_refused:${project}`, (s) => s.hubRefused(project));
/** Voice mode's state in this window (null: never on here): state, heard, said, muted, note, fail. */
export const useVoiceMode = (): VoiceModeState | null => useSlice('voice_mode', (s) => s.voiceModeNow());
/** ⌘K's index (store.askIndex() once): every project the core doesn't hold, from its files, as of written_ms. */
export const useIndex = (): readonly IndexRow[] => useSlice('index', (s) => s.index());
/** R8: the /model the core holds until its provider works (open that provider's setup; setup_first_cancel drops it). */
export const useSetupFirst = (): EvOf<'setup_first'> | null => useSlice('setup_first', (s) => s.setupFirst());
/** N1: where his keys and sign-ins are kept (the core's keychain), and the last move's lines. */
export const useKeychain = (): EvOf<'keychain'> | null => useSlice('keychain', (s) => s.keychain());
/** usage-metrics-plan step 6: his usage counts (the core's telemetry, after a telemetry {act}); none yet: null. */
export const useTelemetry = (): EvOf<'telemetry'> | null => useSlice('telemetry', (s) => s.telemetry());
export const useCoreState = (): CoreState => useSlice('core', (s) => s.core());
/** S29: every agent using computer use now (key, project when his, agent, driving, place, paused, stopped); act with useStore().computer(act, key). */
export const useComputerLive = (): readonly ComputerAgent[] => useSlice('computer_live', (s) => s.computerLive());

/** How often a shown row asks for a new still (the core allows one per 2 s per agent). */
export const PEEK_EVERY_MS = 2000;

/**
 * S29: the latest still of what an agent drives, asked now and every
 * PEEK_EVERY_MS while the row is mounted, `on` and the window visible;
 * never stored: only the newest answer stays in memory, dropped when the
 * row goes. null key: asks nothing.
 */
export function usePeek(key: string | null, opts: { on?: boolean; maxWidth?: number } = {}): EvOf<'computer_peek'> | null {
  const store = useStore();
  const on = opts.on ?? true;
  const { maxWidth } = opts;
  useEffect(() => {
    if (key === null || !on) return undefined;
    const doc = (globalThis as { document?: Document }).document;
    const ask = (): void => {
      if (doc?.visibilityState !== 'hidden') store.computer('peek', key, maxWidth);
    };
    ask();
    const timer = setInterval(ask, PEEK_EVERY_MS);
    return () => {
      clearInterval(timer);
      store.dropPeek(key);
    };
  }, [store, key, on, maxWidth]);
  return useSlice(`peek:${key ?? ''}`, (s) => (key === null ? null : s.peek(key)));
}

/** A thread's order, before and more (key: threadKey(project, agent)). */
export const useThreadOrder = (key: string): ThreadOrder => useSlice(`order:${key}`, (s) => s.threadOrder(key));
/** Its newest page is on its way (subscribed, not in yet): what it shows may be old (thread-loading). */
export const useThreadSyncing = (key: string): boolean => useSlice(`sync:${key}`, (s) => s.threadSyncing(key));
/** What it shows is the core's copy of its last page, from before the hub answered (boot-speed). */
export const useThreadCopy = (key: string): boolean => useSlice(`sync:${key}`, (s) => s.threadCopy(key));
/** The hub's newest answer to ⌘F in a thread (store.findInThread); null before one. */
export const useThreadHits = (key: string): ThreadHits | null => useSlice(`hits:${key}`, (s) => s.threadHits(key));
/** One entry: re-renders only when that entry is replaced. */
export const useEntry = (key: string, pos: number): Entry | undefined => useSlice(`entry:${key}:${String(pos)}`, (s) => s.entry(key, pos));
export const useTyping = (key: string): string => useSlice(`typing:${key}`, (s) => s.typing(key));
/** His sends on their way or failed, oldest first. */
export const usePending = (key: string): readonly Pending[] => useSlice(`pending:${key}`, (s) => s.pending(key));
/** Where he made his answer to card `card` of `project` (window-local, answered.ts); null when this window didn't send it. */
export const useAnsweredFrom = (project: string, card: number): AnsweredFrom | null => useSlice('answered-from', (s) => s.answeredFrom(project, card));
/** The newest jump to an answer the hub took (answer-in-thread A); null before any. */
export const useAnswerJump = (): AnswerJump | null => useSlice('answer_jump', (s) => s.answerJump());
/** The cards he answered or closed in this window that the hub's rows still list: every view leaves them out (acted.ts). */
export const useActed = (): readonly Acted[] => useSlice('acted', (s) => s.acted());
/** The inbox's one view: every project's open items he hasn't acted on here, each with its refusal (machine/cards.ts inboxView). */
export const useInboxItems = (): readonly InboxItem[] => useSlice('inbox_items', (s) => s.inboxItems());
/**
 * The refusals of his answers and closes in this window (refused.ts): the thread's strip and the
 * capsule make their items with them (inbox/listed.ts withRefused) until their own screens' views.
 */
export const useRefusedList = (): readonly Refused[] => useSlice('refused', (s) => s.refused());

/** Calls `fn` on every applied event (one-shot things: open, route, job_end, error). Keep `fn` stable. */
export function useCoreEvent(fn: (ev: Ev) => void): void {
  const store = useStore();
  useEffect(() => store.onEvent(fn), [store, fn]);
}
