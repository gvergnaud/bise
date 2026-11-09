// Every read of the store (store.ts's split, ambient-lead m_22445): stable references between
// changes, one slice each.
import type {
  Account,
  SwitchRow,
  Growth,
  Card,
  ComputerAgent,
  Agent,
  Artifact,
  Folder,
  Entry,
  Found,
  IndexRow,
  Job,
  JsonValue,
  Feature,
  Place,
  Merged,
  Model,
  PickCommand,
  RoleRow,
  SetupGauge,
  PickSkill,
  Plugin,
  PluginPreview,
  PluginsSummary,
  ProjectRow,
  ScheduledTask,
  Worktree,
  FlowMode,
  ReleaseEv,
  UpdateEv,
} from "@/proto";
import type { CoreState } from "@/shared/api";
import type { BiseUpdate, Proc } from "@/proto";
import {
  type Away,
  type BranchesList,
  type Dictation,
  type Diff,
  diffKey,
  diffErrKey,
  artDiffKey,
  branchDiffKey,
  guideKey,
  type GuideEv,
  prActKey,
  type PrActState,
  prDetailKey,
  prDiffKey,
  prLogKey,
  type EvOf,
  type HubLine,
  type InboxList,
  type PagesList,
  type PrsList,
  type QueuedTake,
  sameJson,
  type TalkShot,
  takeKey,
  EMPTY_ORDER,
  type ThreadHits,
  type ThreadOrder,
  type ToolOut,
  toolOutKey,
  type VersionsList,
  type VoiceModeState,
} from "./types";
import { EMPTY, StoreBase } from "./base";

export class StoreReads extends StoreBase {
  // ---- reads (stable references between changes)

  projects(): readonly ProjectRow[] {
    return this.facts.rows;
  }
  agents(project: string): readonly Agent[] {
    return this.slice(`agents:${project}`, EMPTY);
  }
  /**
   * The project's open cards: its `projects` row's (HubFacts.rows, his and bise's own by id), the
   * one copy the inbox and every count read too (inbox-sync). The same array until they change
   * (a reader subscribes to 'projects').
   */
  cards(project: string): readonly Card[] {
    const now = this.facts.rows.find((r) => r.project === project)?.cards ?? EMPTY;
    const was = this.cardsOf.get(project);
    if (was !== undefined && (was === now || sameJson(was, now))) return was;
    this.cardsOf.set(project, now);
    return now;
  }
  /** cards()'s last answer per project, so a new rows event with the same cards keeps the reference */
  private readonly cardsOf = new Map<string, readonly Card[]>();
  /** the last answer by voice the core heard (bar N14), null: none */
  voiceAnswer(): EvOf<"voice_answer"> | null {
    return this.slice<EvOf<"voice_answer"> | null>("voice_answer", null);
  }
  /** The card in front of him in this focused window (bar N14), or none: fn's talk can answer it by voice. */
  cardInView(project: string | null, card: number | null): void {
    this.command(
      project === null || card === null
        ? { cmd: "card_in_view" }
        : { cmd: "card_in_view", project, card },
    );
  }
  /** the hub's last line to this window: a notice (a no's '@x stays: nothing archived', /flow) or a refused slash line */
  notice(project: string): HubLine | null {
    return this.slice<HubLine | null>(`notice:${project}`, null);
  }
  /** how tool calls are approved on this machine and the repo's saved rules (bar V8/W21); null until the hub said */
  approvals(project: string): EvOf<"approvals"> | null {
    return this.slice<EvOf<"approvals"> | null>(`approvals:${project}`, null);
  }
  prefs(): JsonValue | null {
    return this.slice<JsonValue | null>("prefs", null);
  }
  /** V14: the sign-in open in his browser (its link, to copy), until its end (accounts or error). */
  signing(): { readonly id: string; readonly url: string } | null {
    return this.slice<{ readonly id: string; readonly url: string } | null>(
      "signing",
      null,
    );
  }
  /** provider_use's last answer (checking, works, failed); null before any. */
  providerCheck(): EvOf<"provider_check"> | null {
    return this.slice<EvOf<"provider_check"> | null>("provider_check", null);
  }
  accounts(): readonly Account[] {
    return this.slice("accounts", EMPTY);
  }
  /** Claude Code is signed in with a Claude plan (the accounts event): the first run points at its API credits */
  claudePlan(): boolean {
    return this.slice<boolean>("claude_plan", false);
  }
  found(): readonly Found[] {
    return this.slice("found", EMPTY);
  }
  /** a workspace's plugins in numbers (none: bise's home's); null until the core sent them */
  pluginsSummary(project?: string): PluginsSummary | null {
    return this.slice<PluginsSummary | null>(`plugins_summary:${project ?? ""}`, null);
  }
  /** what a source offers to add (`mistral`, a marketplace's address); null until it was read */
  browse(source: string): readonly Plugin[] | null {
    return this.slice<readonly Plugin[] | null>(`browse:${source}`, null);
  }
  /** the marketplaces read so far, in the order they came */
  browseSources(): readonly string[] {
    return this.slice<readonly string[]>("browse_sources", EMPTY);
  }
  /** the add sheet's preview waiting for his click; null when none */
  pluginPreview(): { readonly token: string; readonly preview: PluginPreview } | null {
    return this.slice<{ readonly token: string; readonly preview: PluginPreview } | null>("plugin_preview", null);
  }
  /** computer use's state (P.2); null until the core sent it */
  computerUse(): EvOf<"computer_use"> | null {
    return this.slice<EvOf<"computer_use"> | null>("computer_use", null);
  }
  /** S29: every agent using computer use now (each with its project when it is one of his) */
  computerLive(): readonly ComputerAgent[] {
    return this.slice("computer_live", EMPTY);
  }
  /** S29: the latest peek answer for an agent's key (a still, or why none); null before one */
  peek(key: string): EvOf<"computer_peek"> | null {
    return this.slice<EvOf<"computer_peek"> | null>(`peek:${key}`, null);
  }
  /** S29: his act on an agent's computer use by its key (take, hand_back, stop, peek); the rows follow state.json, never optimistic */
  computer(
    act: "take" | "hand_back" | "stop" | "peek",
    key: string,
    maxWidth?: number,
  ): void {
    this.command({
      cmd: "computer",
      act,
      key,
      ...(maxWidth === undefined ? {} : { max_width: maxWidth }),
    });
  }
  /** S29: forgets an agent's last still (its row unmounted: no image kept) */
  dropPeek(key: string): void {
    if (this.slices.get(`peek:${key}`) != null) this.put(`peek:${key}`, null);
  }
  /** a workspace's agent plugins (none: bise's home's); null until the core sent them */
  plugins(project?: string): readonly Plugin[] | null {
    return this.slice<readonly Plugin[] | null>(
      `plugins:${project ?? ""}`,
      null,
    );
  }
  artifacts(project: string): readonly Artifact[] {
    return this.facts.artifacts[project] ?? EMPTY;
  }
  /** the project's artifacts event came (its list is the hub's, even empty) */
  artifactsCame(project: string): boolean {
    return this.facts.artifacts[project] !== undefined;
  }
  /** the library's folders, shared by every project (the newest artifacts event's); none until a hub said */
  folders(): readonly Folder[] {
    return this.slice<readonly Folder[]>("library:folders", EMPTY);
  }
  worktrees(project: string): readonly Worktree[] {
    return this.slice(`worktrees:${project}`, EMPTY);
  }
  /** the project's live scheduled tasks (sb every), whole each time; none until the hub said */
  scheduled(project: string): readonly ScheduledTask[] {
    return this.slice(`scheduled:${project}`, EMPTY);
  }
  /** the project's scheduled event came (its list is the hub's, even empty) */
  scheduledCame(project: string): boolean {
    return this.slices.has(`scheduled:${project}`);
  }
  /** the processes its agents left running, then the ones that ended in the last 24 h; none until the hub said */
  processes(project: string): readonly Proc[] {
    return this.slice(`processes:${project}`, EMPTY);
  }
  /** the project's processes event came (its list is the hub's, even empty) */
  processesCame(project: string): boolean {
    return this.slices.has(`processes:${project}`);
  }
  merged(project: string): readonly Merged[] {
    return this.slice(`merged:${project}`, EMPTY);
  }
  /** a project's features (amb-feed 5f436473: at hello, on change, on HubCmd features) */
  features(project: string): readonly Feature[] {
    return this.slice(`features:${project}`, EMPTY);
  }
  /** a project's worktrees and their PRs, with hub/agents (the TUI's places): none until the hub said */
  places(project: string): readonly Place[] {
    return this.slice(`places:${project}`, EMPTY);
  }
  /** the models a project's agents can run, the TUI's /model list (amb-feed 7dfd1c74) */
  models(project: string): readonly Model[] {
    return this.slice(`models:${project}`, EMPTY);
  }
  /** a project's open PRs as the TUI's /prs (amb-feed 66faffe3): its head, rows, and the 'none' words; null until the hub sent them */
  /** a project's branches (the answer to branches); null before one */
  branches(project: string): BranchesList | null {
    return this.slice<BranchesList | null>(`branches:${project}`, null);
  }
  /** bise's versions (state at hello, again after versions) */
  versions(project: string): VersionsList | null {
    return this.slice<VersionsList | null>(`versions:${project}`, null);
  }
  /** the repo's flow mode: null before the hub said, or when it is not set */
  flow(project: string): FlowMode | null {
    return this.slice<FlowMode | null>(`flow:${project}`, null);
  }
  /** the hub's newest version switch (its phase, to/from, why it rolled back, at_ms); null: none said */
  switchRow(project: string): SwitchRow | null {
    return this.slice<SwitchRow | null>(`switch:${project}`, null);
  }
  /** les bims: how much the project has done (hub/growth); null before the hub said */
  growth(project: string): Growth | null {
    return this.slice<Growth | null>(`growth:${project}`, null);
  }
  /** his pages and the late promises' count; null before the hub said */
  pages(project: string): PagesList | null {
    return this.slice<PagesList | null>(`pages:${project}`, null);
  }
  /** /update's build in bise's source tree, its latest step; null when none ran */
  hubUpdate(project: string): UpdateEv | null {
    return this.slice<UpdateEv | null>(`update:${project}`, null);
  }
  /** a /release-bise run (dev builds), its latest step; null when none ran */
  release(project: string): ReleaseEv | null {
    return this.slice<ReleaseEv | null>(`release:${project}`, null);
  }
  /** ask a project's branches (the answer: branches) */
  askBranches(project: string): void {
    this.command({ cmd: "branches", project });
  }
  /** ask bise's versions again (the answer: versions) */
  askVersions(project: string): void {
    this.command({ cmd: "versions", project });
  }
  /** a hub's answered and withdrawn records (inbox/history); null before it said */
  inbox(project: string): InboxList | null {
    return this.slice<InboxList | null>(`inbox:${project}`, null);
  }
  prs(project: string): PrsList | null {
    return this.slice<PrsList | null>(`prs:${project}`, null);
  }
  jobs(project: string): readonly Job[] {
    return this.slice(`jobs:${project}`, EMPTY);
  }
  /**
   * A tool call's real output (amb-feed dece0fe9: up to 4 KB from the session log, `cut` when
   * longer or only the 200-char preview was found), or the hub's refusal; null until asked and
   * answered. Ask with askToolOut on open: the answer goes to the asking connection only.
   */
  toolOut(project: string, agent: string, pos: number): ToolOut | null {
    return this.slice<ToolOut | null>(toolOutKey(project, agent, pos), null);
  }
  /** Ask the hub for a call's output once (an open row asks; a second open reads what came). */
  askToolOut(project: string, agent: string, pos: number): void {
    const key = toolOutKey(project, agent, pos);
    if (this.slices.get(key) != null || this.toolAsks.has(key)) return;
    this.toolAsks.add(key);
    this.toolAsk.set(project, key);
    this.port?.send({ cmd: "tool_out", project, agent, pos });
  }
  /** His queued message taken back (queued_take): its words, or the hub's refusal; null until then. */
  queuedTaken(project: string, agent: string, id: number): QueuedTake | null {
    return this.slice<QueuedTake | null>(takeKey(project, agent, id), null);
  }
  /**
   * Take back a message the hub holds until the agent's turn ends (amb-feed 34c35175: 'edit' =
   * take + his words back in the composer, 'drop' = take). The answer is queued_taken (its words),
   * or an error with the hub's words (already delivered, no longer waiting); the row leaves
   * Agent.queued at the next agents event.
   */
  queuedTake(project: string, agent: string, id: number): void {
    const key = takeKey(project, agent, id);
    if (this.slices.get(key) != null) this.put(key, null);
    this.takeAsk.set(project, key);
    this.command({ cmd: "queued_take", project, agent, id });
  }
  /**
   * Send a queued message now (the drawer's 'send now', architect m_25139): the hub steers it into
   * the agent's running turn with its own id; the row leaves Agent.queued at the next agents event.
   * A refusal (delivered, taken, the agent down) is the hub's words on the line over the composer.
   */
  queuedSteer(project: string, agent: string, id: number): void {
    this.command({ cmd: "queued_steer", project, agent, id });
  }
  /** A `.diff` artifact's diff at `version` (null until the hub answers). */
  artifactDiff(project: string, id: string, version: number): Diff | null {
    return this.slice<Diff | null>(artDiffKey(project, id, version), null);
  }
  /** A new req for a PR tab's ask, the newest for `key`: an older one's answer is dropped. */
  private prReq(key: string): number {
    this.cid += 1;
    this.prReqs.set(this.cid, key);
    this.prLatest.set(key, this.cid);
    return this.cid;
  }
  /** A PR tab's detail, asked again: the last one stays until the new one comes. */
  askPrDetail(project: string, url: string): void {
    const req = this.prReq(prDetailKey(project, url));
    this.command({ cmd: "pr_detail", project, url, req });
  }
  prDetail(project: string, url: string): EvOf<"pr_detail"> | null {
    return this.slice<EvOf<"pr_detail"> | null>(prDetailKey(project, url), null);
  }
  /** A failing check's last lines ('Ask <agent> to fix'): null while they come. */
  askPrCheckLog(project: string, url: string, check: string): void {
    const key = prLogKey(project, url, check);
    const req = this.prReq(key);
    this.put(key, null);
    this.command({ cmd: "pr_check_log", project, url, check, req });
  }
  prCheckLog(project: string, url: string, check: string): EvOf<"pr_check_log"> | null {
    return this.slice<EvOf<"pr_check_log"> | null>(prLogKey(project, url, check), null);
  }
  /** His Merge or Close on a PR's tab, at the head he saw: on its way until the hub answers. */
  prAct(project: string, url: string, head: string, act: PrActState["act"]): void {
    const key = prActKey(project, url);
    const req = this.prReq(key);
    this.put(key, { act, done: false } satisfies PrActState);
    this.command({ cmd: "pr_act", project, url, head, act, req });
  }
  prActed(project: string, url: string): PrActState | null {
    return this.slice<PrActState | null>(prActKey(project, url), null);
  }
  /** A PR's diff (the hub picks its place's checkout or the forge's): known by its req. */
  askPrDiff(project: string, url: string): void {
    const key = prDiffKey(project, url);
    const req = this.prReq(key);
    this.diffAsked.set(project, key);
    if (this.slices.get(diffErrKey(key)) != null) this.put(diffErrKey(key), null);
    this.command({ cmd: "diff", project, pr_url: url, req });
  }
  prDiff(project: string, url: string): Diff | null {
    return this.slice<Diff | null>(prDiffKey(project, url), null);
  }
  /** A diff's guide (its state, the guide once written) by the diff's hash; null before the hub says. */
  guide(project: string, hash: string): GuideEv | null {
    return this.slice<GuideEv | null>(guideKey(project, hash), null);
  }
  /** A branch's latest diff against the trunk (branchDiffKey). */
  branchDiff(project: string, branch: string): Diff | null {
    return this.slice<Diff | null>(branchDiffKey(project, branch), null);
  }
  /** The latest diff of an agent's change, or of one commit (`commit`). */
  diff(project: string, agent: string, commit?: string): Diff | null {
    return this.slice<Diff | null>(diffKey(project, agent, commit), null);
  }
  away(): Away | null {
    return this.slice<Away | null>("away", null);
  }
  core(): CoreState {
    return this.facts.core;
  }
  threadOrder(key: string): ThreadOrder {
    return this.threads.get(key)?.view ?? EMPTY_ORDER;
  }
  /** Subscribed, its newest page not in yet: the thread may have newer entries than it shows (thread-loading). */
  threadSyncing(key: string): boolean {
    return this.threads.get(key)?.syncing === true;
  }
  /** What it shows is the core's copy from before (boot-speed): its cue shows at once. */
  threadCopy(key: string): boolean {
    return this.threads.get(key)?.cached === true;
  }
  /** The hub's newest answer to ⌘F in thread `key` (findInThread); hits null: the hub can't search (older hub). */
  threadHits(key: string): ThreadHits | null {
    return this.slice<ThreadHits | null>(`hits:${key}`, null);
  }
  entry(key: string, pos: number): Entry | undefined {
    return this.threads.get(key)?.byPos.get(pos);
  }
  typing(key: string): string {
    return this.threads.get(key)?.typing ?? "";
  }
  /** Each role's model and effort (V17; set with command({cmd: 'role_set', role, model?, effort?})). */
  roles(): readonly RoleRow[] {
    return this.slice("roles", EMPTY);
  }
  /** settings · Setup's gauge (the core's bise_catalog::features; null until it comes). */
  setupGauge(): SetupGauge | null {
    return this.slice<SetupGauge | null>("setup_gauge", null);
  }
  /** The `/` list (the TUI's commands.rs), asked once from each core. */
  commands(): readonly PickCommand[] {
    return this.slice("commands", EMPTY);
  }
  askCommands(): void {
    if (this.commandsAsked) return;
    this.commandsAsked = true;
    this.command({ cmd: "commands" });
  }
  /** A workspace's `$` list (the TUI's skills.rs); askSkills reads it again. */
  skills(project: string): readonly PickSkill[] {
    return this.slice(`skills:${project}`, EMPTY);
  }
  askSkills(project: string): void {
    this.command({ cmd: "skills", project });
  }
  /** R8: the last /model the core holds until its provider works (amb-home's setup opens on it), or null. */
  setupFirst(): EvOf<"setup_first"> | null {
    return this.slice<EvOf<"setup_first"> | null>("setup_first", null);
  }
  /** Why this core can start no hub (CoreEv broken: its runtime is missing); null: it can. */
  broken(): string | null {
    return this.slice<string | null>("broken", null);
  }

  /** update-flow: a newer bise for the machine (the core's fold of every hub's); none: null */
  biseUpdate(): BiseUpdate | null {
    return this.slice<BiseUpdate | null>("bise_update", null);
  }

  /** N1: where his keys and sign-ins are (the core's keychain event); none yet: null */
  keychain(): EvOf<"keychain"> | null {
    return this.slice<EvOf<"keychain"> | null>("keychain", null);
  }

  /** usage-metrics-plan step 6: his usage counts (the core's telemetry, in reply to an act); none yet: null */
  telemetry(): EvOf<"telemetry"> | null {
    return this.slice<EvOf<"telemetry"> | null>("telemetry", null);
  }
  /**
   * ⌘K's index (core/index.rs): every project the core doesn't hold now,
   * from its view.json (agents, the newest artifacts and their total), as of
   * written_ms, with up (its hub runs). askIndex() once; it comes again on
   * change, at most every 5 s.
   */
  index(): readonly IndexRow[] {
    return this.slice("index", EMPTY);
  }
  askIndex(): void {
    this.command({ cmd: "index" });
  }
  /** Why a project's hub refused this core (its words), or null. */
  hubRefused(project: string): string | null {
    return this.slice<string | null>(`hub_refused:${project}`, null);
  }
  /** Try a refused project's hub again (his click): the core connects once more. */
  hubRetry(project: string): void {
    this.command({ cmd: "hub_retry", project });
  }
  voiceModeNow(): VoiceModeState | null {
    return this.slice<VoiceModeState | null>("voice_mode", null);
  }
  /** fn's talk: what bise heard so far (the whole text; final once the talk ended), null before a talk. */
  heard(): { readonly text: string; readonly final: boolean } | null {
    return this.slice<{
      readonly text: string;
      readonly final: boolean;
    } | null>("heard", null);
  }
  /** fn's talk: the still of his front window it will send (its stored file), null: none or gone. */
  shot(): TalkShot | null {
    return this.slice<TalkShot | null>("shot", null);
  }
  /** A voice's loudness now, 0..1: 'you' (the mic), 'main' (its spoken answer). */
  level(who: string): number {
    return this.slice(`level:${who}`, 0);
  }
  dictation(id: string): Dictation | null {
    return this.slice<Dictation | null>(`dictation:${id}`, null);
  }
}
