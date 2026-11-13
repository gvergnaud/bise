//! The desktop core's events to the app besides the hubs' (S3b,
//! `core/<tag>` on its stdio: [`crate::app_rpc`]).

use crate::app_rows::*;
use crate::hub::Mode;
use crate::rows::{Page, ScheduledTask};
use crate::thread::{Entry, PageRef};
use crate::{decode, parse, Project};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What the core sends the app besides the hubs' events (S3b).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum CoreEv {
    /// every registered project (bise's first), on change
    Projects { projects: Vec<ProjectRow> },
    /// open this in the window (a capsule's or a card's ⏎)
    Open {
        project: Project,
        what: OpenWhat,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// his preferences (`bise_home::prefs`, its one owner): kept as JSON
    /// here until that module types them
    Prefs { prefs: Value },
    /// `claude_plan`: Claude Code is signed in with a Claude plan
    /// (bise_catalog::detect, presence only): the first run points at
    /// its monthly API credits
    Accounts {
        items: Vec<Account>,
        #[serde(default)]
        claude_plan: bool,
    },
    /// draft (V14): a sign-in started: the link it opened in his browser
    /// (the window copies it); once per sign-in, its end as before
    /// (accounts, or error {cmd: sign_in})
    Signing { id: String, url: String },
    /// draft (architect m_28532): the answer to `provider_use`: checking,
    /// then works (`model` answered `answer`; `main`: main's model now;
    /// `lines`: what each role runs, in the catalog's words) or failed
    /// (`why`, the provider's own words in `said`, the page that fixes
    /// it in `url`: its keys or its billing)
    ProviderCheck {
        id: String,
        name: String,
        state: CheckState,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        model: String,
        /// the model's name for people, as its chip says it (`opus 5.5`)
        #[serde(default, skip_serializing_if = "String::is_empty")]
        label: String,
        /// the key checked was pasted (not found, not a sign-in)
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pasted: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        answer: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        main: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        lines: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        why: Option<CheckWhy>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        said: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        /// how long the one real call took, its end (works or failed): the
        /// first launch's '✓ it said: “…” · 1.2 s' (desktop-onboarding v4)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ms: Option<u64>,
    },
    /// the answer to `found_scan`, whole each time: the repos of `dir`
    /// (his home) and of each guarded folder he looked in; `guarded`: the
    /// folders macOS guards (Documents, Desktop, Downloads...) the scan did
    /// not enter and he has not looked in yet (his click sends
    /// `found_scan {dir}`); `looked`: the ones he did
    Found {
        dir: String,
        items: Vec<Found>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        guarded: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        looked: Vec<Looked>,
    },
    /// the answer to `away_back` (S9, amb-mac m_9038): what happened while
    /// he was away, since `since_ms`: agents that turned done or failed,
    /// his cards (questions and approvals) that came; `projects`: each
    /// project where one of those happened, in his projects' order
    AwaySummary { since_ms: u64, done: u32, questions: u32, failed: u32, projects: Vec<AwayProject> },
    /// the Plugins list for `project`'s workspace (none: bise's home),
    /// whole, the answer to `plugins` and to each plugin command
    /// (`bend_plugins::catalog::rows`)
    Plugins {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        items: Vec<Plugin>,
        /// why his Mistral connectors may be missing or old (none: fine)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connectors: Option<ConnectorsProblem>,
        /// Settings › Plugins › File types (docs/plugin-apps.md A3): every
        /// type an on plugin's app opens, by type key
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        file_types: Vec<crate::rows::FileType>,
    },
    /// what a source offers to add: `source` is `mistral` (the connectors
    /// not connected yet) or a marketplace's address
    Browse {
        source: String,
        items: Vec<Plugin>,
        /// `mistral`'s: why the list may be missing or old
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connectors: Option<ConnectorsProblem>,
    },
    /// the answer to `plugin_add`: what the plugin will run, shown before
    /// his click; `plugin_add_confirm {token}` adds it, `plugin_add_cancel`
    /// drops it (nothing of it ran)
    PluginPreview { token: String, preview: PluginPreview },
    /// the Plugins list of `project`'s workspace (none: bise's home) in a
    /// few numbers (`bend_plugins::catalog::summary`): the sidebar's
    /// plugins row and the place's band; at the core's start and only
    /// when it changes
    PluginsSummary {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        summary: PluginsSummary,
    },
    /// computer use (P.2): on (the built-in `computer` plugin), its setup's
    /// steps while the window's page checks them (none until a check
    /// came), ready (browser, extension and live test done), `said`: what
    /// went wrong with a fix, `flash`: what a fix just did
    ComputerUse {
        on: bool,
        ready: bool,
        rows: Vec<CuRow>,
        /// the browser profiles with the extension (the Profiles group,
        /// shown from two), the default marked; none until a check came
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        profiles: Vec<CuProfile>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        said: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        flash: Option<String>,
    },
    /// a voice's loudness, 0..1, ~30 Hz while it sounds, then a 0: `you`
    /// (the mic: his talk or a dictation, for a meter) or `main` (main's
    /// spoken answer). No words, to every window
    Level { who: String, v: f64 },
    /// fn's talk (`talk_start`): what bise has heard so far, the WHOLE
    /// text each time (`final` false, ~as the words come), then all of it
    /// once (`final` true) when the talk ends; to every window (the
    /// capsule's talk view draws it). Never journaled, never in a log
    Heard {
        text: String,
        #[serde(rename = "final")]
        is_final: bool,
    },
    /// fn's talk: the still of his front window the talk will send
    /// (ambient-lead m_21912, amb-web m_21935): `file` its stored path in
    /// the image store, `app` and `title` the front app and window; once
    /// it is stored, then `file: null` when it goes (he dropped it with
    /// `shot_drop`, the talk sent or cancelled it). To every window (the
    /// capsule's talk view shows it). Never journaled, never in a log.
    /// `id`: this still's number, the one a spotlight send names
    /// (`send {shot: id}`) and a `shot_drop {id}` drops; none with
    /// `file: null`
    Shot {
        #[serde(default)]
        file: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    /// a dictation's words (bar I, `dictate_start {id}`), only to the
    /// window that started it (the app's main routes it by `id`), never
    /// to main, never journaled, never in a log. `text` is the WHOLE text
    /// so far (replace, never append): a dropped or late partial can't
    /// duplicate words; the composer's text is the last event's. Partials
    /// have `final: false`; exactly one `final: true` ends it (stop, the
    /// time limit, fn's talk, `cancelled` with an empty text, or `error`:
    /// the start refused (the mic is busy, no mic, no voice model) or the
    /// listener failed, with the words heard so far)
    /// each role's model and effort (V17): at the start, after each
    /// `role_set`, at `setup_check`
    Roles { items: Vec<RoleRow> },
    /// settings · Setup's features and gauge (page models-setup), computed
    /// in the core (bise_catalog::features): with `roles`, and after each
    /// `setup_test` result
    SetupGauge { gauge: SetupGauge },
    /// the artifacts library's folders after his `folders` or
    /// `folders_move` (the hubs' `artifacts` follow): `made` the folder a
    /// make made or found, `notice` the toast's words (not `folders`: the
    /// hub's answer has that tag)
    LibraryFolders {
        folders: Vec<crate::rows::Folder>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        made: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        notice: Option<String>,
    },
    /// the slash commands (answer to `commands`, and once at the start):
    /// the TUI's `/` list as data
    Commands { items: Vec<PickCommand> },
    /// a workspace's skills (answer to `skills {project}`): the TUI's `$`
    Skills { project: Project, items: Vec<PickSkill> },
    /// an `@` query's files (answer to `files {project, q, rid}`), ranked
    /// by the TUI's own files.rs: `partial` while the workspace's first
    /// walk runs (one more answer with the same rid follows), `folder`:
    /// the browsed folder's own row (`@src/`)
    Files {
        project: Project,
        rid: u64,
        q: String,
        items: Vec<PickFile>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<PickFolder>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        partial: bool,
    },
    /// bar N14: his talk answered the card in view (`card_in_view`): the
    /// window shows `line` on that card for `ms` (esc undoes, nothing
    /// sent), then answers option `n` (1-based) through the hub's one
    /// answer path; the words did not go to main
    VoiceAnswer { project: Project, card: u64, n: u32, line: String, ms: u64 },
    /// voice mode with the agent in view (bar V12/W24/L21/N13, the
    /// TUI's ctrl+r twice): its state, on each change, only to the window
    /// that turned it on (the app's main routes it), never journaled,
    /// never in a log. `state`: listening, hearing (he talks, `heard`:
    /// his words so far), sending, thinking (the agent works), speaking
    /// (`said`: the sentence being said), muted, typing, failed (`fail`:
    /// why, voice mode stays on). `on: false` is its last (he left, his
    /// window went, the core stopped; `note`: how long, how many turns).
    /// `lit`: the agent message being said and how far, while it speaks
    /// and after (a cut too), until his next turn
    VoiceMode {
        project: Project,
        agent: String,
        on: bool,
        state: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        heard: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        said: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        muted: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fail: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lit: Option<VoiceLit>,
    },
    Dictation {
        id: String,
        text: String,
        #[serde(rename = "final")]
        is_final: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        cancelled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// on a final that failed, timed out or came back empty: the core
        /// keeps this take's sound (memory only, its last 3 takes, 10 min
        /// each) and `dictate_retry {id}` transcribes it again
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        kept: bool,
        /// on a final with words: one line about them (the voice model
        /// is gone, another of its provider's heard them), said like an
        /// error but the words land
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// bar S.6: a newer desktop app on bise's release channel than the one
    /// running (`app_version`), read from `latest.json` by
    /// bise_home::release (its one reader, the one version rule): `url` is
    /// the zip's, absolute; the app downloads it, checks `sha256` and its
    /// signature, and swaps itself only on his click. Sent once per
    /// version, never for the same build or an older one
    AppUpdate { version: String, url: String, sha256: String },
    /// update-flow: a newer bise for the machine, folded from every
    /// project's `hub/bise_update` (rows::bise_update::machine): the
    /// status pill's; None: nothing to show. Sent when it changes. Its own
    /// tag: a CoreEv never shares one with a HubEv (app_rpc's law)
    MachineUpdate {
        #[serde(default)]
        update: Option<crate::rows::BiseUpdate>,
    },
    /// a project's hub refused this core's connection (its words): the
    /// core never reconnects to it until he retries (`hub_retry`) or a
    /// new core starts; commands to it get error {kind: hub_refused}
    HubRefused { project: Project, error: String },
    /// ⌘K's index (his unified search): the projects this core does NOT
    /// hold, read from each one's `view.json`, never by starting its hub;
    /// sent on `index`, then on change at most every 5 s (a held project
    /// is never in it: its live events say the same)
    Index { projects: Vec<IndexRow> },
    /// R8 (the TUI's sb.rs /model): his `/model <model>` in `agent`'s
    /// view needs `provider`'s key: the line is held, not sent; the window
    /// opens that provider's setup (`name` for people). It runs once the
    /// provider works (its `cid` answered as any slash), or is dropped by
    /// `setup_first_cancel`, the window that sent it closing, the project
    /// removed, or the core restarting
    SetupFirst {
        project: Project,
        agent: String,
        provider: String,
        name: String,
        model: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cid: Option<u64>,
    },
    /// S29: every agent using computer use on this machine now (all of
    /// them, each with its project when its hub is one of his): sent on
    /// change (state.json read at most every 500 ms) and once at start
    ComputerLive { items: Vec<ComputerAgent> },
    /// S29: the answer to `computer {act: peek, key}`: the still, or why
    /// none (`not_found`: it drives nothing now or he took over;
    /// `too_soon`: one per 2 s per agent; `no_broker`: computer use
    /// isn't running)
    ComputerPeek {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shot: Option<PeekShot>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        why: Option<String>,
    },
    /// N1 (the TUI's /keychain): where his keys and sign-ins are kept,
    /// `keychain` (the macOS keychain) or `file` (files in ~/.bise), and
    /// the last move's lines (item names and counts, never a secret);
    /// `moving` while a move runs on its own thread (a locked keychain
    /// asks ~2 s an item). Sent at the core's start, on `keychain`, and at
    /// a move's start and end
    Keychain {
        store: String,
        moving: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        lines: Vec<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        failed: bool,
    },
    /// this core can't start any hub (its runtime is missing): why, in
    /// plain words (the binary's `approot::missing_words`), once after
    /// `initialized`. It still answers; every hub the window asks for is
    /// down with the same words, never a silent retry
    Broken { reason: String },
    /// the main hub as the capsule draws it, on each change: his agents'
    /// rows (main aside), his cards with their options, the pages, the
    /// pages' address, the standing orders (live, then lately ended)
    State {
        agents: Vec<StateAgent>,
        cards: Vec<StateCard>,
        pages: Vec<Page>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages_url: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        timers: Vec<ScheduledTask>,
    },
    /// main's words for him: its answer to `turn` (his talk or send), or
    /// an agent writing to him (`from`: who)
    Main {
        text: String,
        turn: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
    },
    /// the main hub's connection: up or down (`workspace`: bise's home);
    /// `project` and `note` while bise there is being updated
    Hub {
        up: bool,
        workspace: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// his talk's or send's words went to main: `voice` (said, so the
    /// answer is said too), `shot` (with the talk's still)
    Sent { text: String, voice: bool, shot: bool },
    /// his `agent_send` reached the agent's hub, `now` or `queued`
    AgentSent { agent: String, mode: Mode },
    /// his send `cid` the core answered late (its hub said nothing in 30 s,
    /// error reason "late"), settled at last (queue-drawer-fix, architect
    /// m_34889): its hub's `SendResult`, or none when it went (its
    /// connection lost, or 10 min passed). Main's ipc.ts gives it to the
    /// window that sent it, as its window-local 'sent' or 'send_refused'.
    SendAnswered {
        project: Project,
        agent: String,
        cid: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<crate::rpc::SendResult>,
    },
    /// main's spoken answer of `turn`: the word `i` (0-based) is being
    /// said now
    Word { turn: u64, i: u64 },
    /// the capsule's phase (listening, sending, thinking, speaking, idle,
    /// failed, …), `line`: its words; `page`: a note talk on that page
    Phase {
        phase: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<PhasePage>,
    },
    /// a page of the main hub changed (the hub's row), `front`: it answers
    /// what he waited for, so the capsule brings it forward
    Page {
        #[serde(flatten)]
        page: Page,
        front: bool,
    },
    /// a history panel's new or changed entry (the hub's thread entry;
    /// an open card not in the thread is one at `CARD_POS + id`)
    AgentEntry { agent: String, entry: Box<Entry> },
    /// a history panel's page: the newest (it opened) or the ones before
    /// a pos; `before`: the next older page's pos, none at the start
    AgentHistory { agent: String, entries: Vec<Entry>, before: Option<u64>, more: bool },
    /// an agent's preview (`agent_preview`), again when it changes
    AgentPreview {
        agent: String,
        now: String,
        actions: Vec<PreviewAction>,
        waiting: Vec<PreviewWait>,
        last_report: Option<PreviewReport>,
        pages: Vec<PageRef>,
    },
    /// a queued `agent_send` waits for the agent's turn to end
    AgentQueued { agent: String, text: String },
    /// what the agent shows it is doing now (its typing line), while its
    /// history panel is open
    AgentTyping { agent: String, text: String },
    /// harness A: the fake voice is on (no mic, no sound), its words read
    /// from `words` or `fake_words`
    FakeVoice {
        on: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        words: Option<String>,
    },
    /// his usage counts as `bise telemetry show` prints them (page
    /// usage-metrics-plan, step 6: Settings › Privacy, "See what's
    /// sent"). Sent only in reply to `telemetry {act}`, never at start,
    /// never broadcast: it carries the install id, so it is never logged
    /// (architect m_56355)
    Telemetry {
        /// his `telemetry.share`, whatever else turns it off
        share: bool,
        /// why nothing is shared now (none: it shares)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        off: Option<TelemetryOff>,
        /// the id in use (none: drawn at the first send)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        id: Option<String>,
        /// delete requests waiting to be sent
        #[serde(default, skip_serializing_if = "is_zero")]
        pending: u32,
        /// the next packets, each as its JSON
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(optional, as = "Option<Vec<String>>"))]
        next: Vec<String>,
        /// those sent the last 7 days
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(optional, as = "Option<Vec<TelemetrySent>>"))]
        sent: Vec<TelemetrySent>,
        /// what the act it answers did (none: a show)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        done: Option<TelemetryDone>,
    },
    #[serde(skip)]
    Unknown { tag: String, raw: Value },
}

/// Why bise shares no usage counts now (bise_usage's `Off`, by name).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TelemetryOff {
    /// bise doesn't send usage yet (the global switch, until step 9)
    NotYet,
    /// `BISE_TELEMETRY=0`
    Env,
    /// `DO_NOT_TRACK=1`
    DoNotTrack,
    Ci,
    TestHome,
    Offscreen,
    DevBuild,
    /// he turned it off
    Pref,
    /// his note was not shown yet
    NoteNotSeen,
    #[serde(other)]
    Unknown,
}

/// A packet sent the last 7 days: when, the day it counted, its JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TelemetrySent {
    pub on: String,
    pub day: String,
    pub json: String,
}

/// What a `telemetry {act}` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TelemetryDone {
    /// a new id from the next send
    Reset,
    /// deleted here; the server's delete request goes at the next send
    Asked,
    /// deleted here; the server never got anything from this id
    Here,
    #[serde(other)]
    Unknown,
}

impl CoreEv {
    pub const TAGS: &'static [&'static str] = &["projects", "open", "prefs", "accounts", "signing", "provider_check", "found", "away_summary", "plugins", "browse", "plugin_preview", "plugins_summary", "computer_use", "level", "heard", "shot", "dictation", "voice_mode", "voice_answer", "commands", "skills", "files", "roles", "setup_gauge", "library_folders", "app_update", "machine_update", "hub_refused", "index", "setup_first", "computer_live", "computer_peek", "keychain", "broken", "state", "main", "hub", "sent", "agent_sent", "send_answered", "word", "phase", "page", "agent_entry", "agent_history", "agent_preview", "agent_queued", "agent_typing", "fake_voice", "telemetry"];

    pub fn decode(line: &str) -> Result<CoreEv, String> {
        decode(parse(line)?, "ev", Self::TAGS, |tag, raw| CoreEv::Unknown { tag, raw })
    }
}
