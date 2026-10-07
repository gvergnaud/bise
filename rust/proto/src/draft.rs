//! Shapes with no producer yet (architect's S3a review, change A): their
//! fixtures feed the window's fake core now, but their names may still
//! move until their stream emits them, outside v1's "never rename" rule.
//! When a stream ships one, it moves to [`crate::hub`] (or stays here for
//! the core's own messages) and its fixture joins the frozen ones.
//!
//! - [`DraftHubCmd`]: a send with quoted lines (S7's artifacts, diffs,
//!   worktrees and dev servers, S2's routing "→ project" and S10's
//!   followed jobs shipped: hub.rs);
//! - [`CoreEv`] (core -> app: the projects list, an app-side open) and
//!   [`AppCmd`] (app -> core: open, project_add): never a hub's (S3b);
//! - [`ProjectView`]: S1's `hubs/<id>/view.json` (amb-hub writes it).

use crate::rows::{Agent, Artifact, Card, ScheduledTask};
use crate::{decode, parse, Pos, Project};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Selected lines of a thread quoted in a message (❝).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Quote {
    pub agent: String,
    pub pos: Pos,
    pub text: String,
}

/// One of his messages queued for an agent until its turn ends (sb-core
/// holds it, amb-hub m_8281): an [`Agent`] row's `queued`, draft until the
/// hub's mirror sends them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Queued {
    pub id: u64,
    pub text: String,
    pub created_ms: u64,
}

/// Hub commands whose stream hasn't shipped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum DraftHubCmd {
    /// a send with quoted lines (the window's ❝)
    SendQuote { project: Project, agent: String, text: String, quote: Quote },
    #[serde(skip)]
    Unknown { tag: String, raw: Value },
}

impl DraftHubCmd {
    pub const TAGS: &'static [&'static str] = &["send_quote"];

    pub fn decode(line: &str) -> Result<DraftHubCmd, String> {
        decode(parse(line)?, "cmd", Self::TAGS, |tag, raw| DraftHubCmd::Unknown { tag, raw })
    }
}

/// One project in the window's sidebar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectRow {
    pub project: Project,
    pub name: String,
    pub path: String,
    /// its checkout's current branch (none: not a git repo, or detached)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// a git repo (bise's home is not)
    pub git: bool,
    /// bise's own (the home workspace)
    pub home: bool,
    pub order: u32,
    pub agents: u32,
    pub working: u32,
    pub waits: u32,
    /// its hub runs (the sidebar's dot)
    #[serde(default)]
    pub running: bool,
    /// its folder is gone (moved or deleted)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub missing: bool,
    /// his open cards (`waits` of them): a held hub's own `cards`, else
    /// its `view.json`'s, so bise's "waiting for you" lists and answers
    /// the cards of a project it doesn't hold (lead m_8769)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<Card>,
    /// its GitHub remote's web page (`https://github.com/acme/engine`):
    /// the review opens `<web>/commit/<sha>`; absent: no GitHub remote
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum OpenWhat {
    Thread,
    Review,
    Artifact,
    Worktree,
}

/// A model account of his (S11's settings, amb-win m_8382).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Account {
    /// chatgpt, claude, anthropic-key, ...
    pub id: String,
    pub label: String,
    /// subscription | key
    pub kind: String,
    /// signed_in | signed_out | expired
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub who: Option<String>,
    /// L5: the catalog provider it sets up (a model row's `provider_id`)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
}

/// A folder found by `found_scan` that could be a project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Found {
    pub path: String,
    pub name: String,
    pub git: bool,
    pub last_ms: u64,
    /// already one of his projects
    pub known: bool,
}

/// An agent plugin of his (P.1, the TUI's `/plugins`): bend_plugins'
/// resolution of a workspace, user and built-in plugins included.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Plugin {
    pub name: String,
    /// loaded | disabled | shadowed | invalid
    pub state: String,
    /// built-in | user | workspace
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what: Option<String>,
    /// its remote MCP servers that log in (none: nothing to log in to)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logins: Vec<PluginLogin>,
}

/// A plugin's remote MCP server that logs in through his browser.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginLogin {
    /// what `plugin_login` takes (its id, or plugin/server)
    pub name: String,
    pub host: String,
    /// needs | in | out | pending (its browser is open)
    pub state: String,
    /// its tools, once a session connected it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<u32>,
}

/// A step of computer use's setup (P.2, the TUI's `/computer-use`
/// screen as data: rust/tui/src/computer_use.rs rows).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CuRow {
    /// browser | extension | live_test | accessibility | screen_recording
    pub id: String,
    pub label: String,
    /// done | waits | checking | failed | not_yet
    pub state: String,
    pub detail: String,
    /// the lines under it while it waits on him
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub help: Vec<String>,
    /// what its button does ("open chrome://extensions"); none: no button
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// what `computer_use {act: fix, fix}` takes for that button
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    /// under "for apps" (the helper app's permissions)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub apps: bool,
}

/// One project's part of `away_summary` (only a project where something
/// happened while he was away).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AwayProject {
    pub project: Project,
    pub done: u32,
    pub questions: u32,
    pub failed: u32,
}

/// A role's model (V17, config.toml's `[roles]`, the TUI's `/models`
/// screen as data): `kind` chat or voice; `source`: `config` (set in
/// config.toml), `env` (an env var sets it), `default` (unset: bise picks,
/// or `follows` that role's model).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RoleRow {
    pub role: String,
    pub name: String,
    pub about: String,
    pub kind: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follows: Option<String>,
}

/// How a slash command runs (C, architect m_10724): `hub` (his typed
/// line goes to the hub's own parser, HubCmd slash), `core` (a window
/// core command), `window` (the window's own screen or state: the window
/// alone knows whether it is built).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Runs {
    Hub,
    Core,
    Window,
}

/// One value a command's argument can take, as its popup shows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PickChoice {
    pub value: String,
    pub desc: String,
}

/// What completes one argument of a command (the TUI's commands.rs
/// `Arg`): fixed words, a list the window has (live agents, archived ones,
/// open cards, branches, models, efforts, plugins), or free text (`text`
/// required, `note` optional).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PickArg {
    Words { words: Vec<PickChoice> },
    Agent,
    Archived,
    Card,
    Branch,
    Model,
    Effort,
    Plugin,
    Text,
    Note,
}

/// A slash command, as the TUI's `/` popup lists it (commands.rs
/// COMMANDS: same name, desc, order, args). `runnable`: for `hub` and
/// `core`, a typed path exists today; for `window`, always false here
/// (the window owns its screens' readiness and combines the two).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PickCommand {
    pub name: String,
    pub desc: String,
    pub args: Vec<PickArg>,
    pub runs: Runs,
    pub runnable: bool,
}

/// A skill of a workspace, as the TUI's `$` popup lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PickSkill {
    pub name: String,
    pub desc: String,
}

/// A file or folder for an `@` query, ranked as the TUI ranks it
/// (files.rs): `path` relative to the workspace (or as typed for
/// `@../`, `@~/`, `@/`), `protected`: macOS asks once it is entered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PickFile {
    pub path: String,
    pub dir: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub protected: bool,
}

/// The folder an `@folder/` query browses, its own row (⏎ inserts a
/// reference to the folder); `locked`: no access to it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PickFolder {
    pub path: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
}

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
    Accounts { items: Vec<Account> },
    /// draft (V14): a sign-in started: the link it opened in his browser
    /// (the window copies it); once per sign-in, its end as before
    /// (accounts, or error {cmd: sign_in})
    Signing { id: String, url: String },
    /// the answer to `found_scan`
    Found { dir: String, items: Vec<Found> },
    /// the answer to `away_back` (S9, amb-mac m_9038): what happened while
    /// he was away, since `since_ms`: agents that turned done or failed,
    /// his cards (questions and approvals) that came; `projects`: each
    /// project where one of those happened, in his projects' order
    AwaySummary { since_ms: u64, done: u32, questions: u32, failed: u32, projects: Vec<AwayProject> },
    /// his agent plugins for `project`'s workspace (none: bise's home),
    /// the answer to `plugins` and to each change; a change applies to
    /// the sessions that start after it (the TUI's `/reload`)
    Plugins {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        items: Vec<Plugin>,
    },
    /// computer use (P.2): on (the built-in `computer` plugin), its setup's
    /// steps while the window's page checks them (none until a check
    /// came), ready (browser, extension and live test done), `said`: what
    /// went wrong with a fix, `flash`: what a fix just did
    ComputerUse {
        on: bool,
        ready: bool,
        rows: Vec<CuRow>,
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
    /// window went, the core stopped; `note`: how long, how many turns)
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
    },
    /// bar S.6: a newer desktop app on bise's release channel than the one
    /// running (`app_version`), read from `latest.json` by
    /// bise_home::release (its one reader, the one version rule): `url` is
    /// the zip's, absolute; the app downloads it, checks `sha256` and its
    /// signature, and swaps itself only on his click. Sent once per
    /// version, never for the same build or an older one
    AppUpdate { version: String, url: String, sha256: String },
    /// a project's hub refused this core's connection (its words): the
    /// core never reconnects to it until he retries (`hub_retry`) or a
    /// new core starts; commands to it get error {kind: hub_refused}
    HubRefused { project: Project, error: String },
    /// ⌘K's index (his unified search): the projects this core does NOT
    /// hold, read from each one's `view.json`, never by starting its hub;
    /// sent on `index`, then on change at most every 5 s (a held project
    /// is never in it: its live events say the same)
    Index { projects: Vec<IndexRow> },
    #[serde(skip)]
    Unknown { tag: String, raw: Value },
}

impl CoreEv {
    pub const TAGS: &'static [&'static str] = &["projects", "open", "prefs", "accounts", "signing", "found", "away_summary", "plugins", "computer_use", "level", "heard", "dictation", "voice_mode", "voice_answer", "commands", "skills", "files", "roles", "app_update", "hub_refused", "index"];

    pub fn decode(line: &str) -> Result<CoreEv, String> {
        decode(parse(line)?, "ev", Self::TAGS, |tag, raw| CoreEv::Unknown { tag, raw })
    }
}

/// What the app asks the core itself (never forwarded to a hub).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum AppCmd {
    Open {
        project: Project,
        what: OpenWhat,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    /// add a folder to his projects (S1's registry); `land`: the add
    /// sheet's 'agents land their work', the repo's flow mode (true trunk,
    /// false pr, absent: devflow decides; architect m_9130)
    ProjectAdd {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        land: Option<bool>,
    },
    ProjectRemove { project: Project },
    ProjectMove { project: Project, order: u32 },
    ProjectRename { project: Project, name: String },
    /// one preference by its dotted key (`quiet.call`)
    PrefsSet { key: String, value: Value },
    /// opens his browser for that account, then `accounts` again
    SignIn { id: String },
    /// draft (V14): ends the open sign-in of that account (its browser
    /// page and local listener go); its end is error {cmd: sign_in,
    /// text: '<id>: cancelled'}
    SignInCancel { id: String },
    KeySet { id: String, key: String },
    KeyRemove { id: String },
    /// folders that could be projects (none: his home folder, depth 2)
    FoundScan {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dir: Option<String>,
    },
    /// he's back at his Mac after `away_ms` (the app's read): the core
    /// answers `away_summary`
    AwayBack { away_ms: u64 },
    /// bar S.6: the running app's own build (its VERSION's `id` and
    /// `built`; none in a dev run): the core checks bise's release channel
    /// now and hourly, and says `app_update` when a newer app is there
    AppVersion {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        built: Option<String>,
    },
    /// the plugins of `project`'s workspace (none: bise's home): `plugins`
    Plugins {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
    },
    /// turn a plugin on or off (plugins.json, bend_plugins' one writer)
    PluginSet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        name: String,
        on: bool,
    },
    /// log in to a plugin's remote MCP server: opens his browser (his
    /// click only, never a test), `plugins` again at its end
    PluginLogin {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        name: String,
    },
    PluginLogout {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        name: String,
    },
    /// a role's model (V17, config.toml's `[roles]`, written by the same
    /// locked writer as the TUI's roles screen): `model` none = it
    /// follows its fallback again, `effort` none = the model's default;
    /// a bad role or an unknown model is an error and writes nothing
    RoleSet {
        role: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effort: Option<String>,
    },
    /// check again (S.8, the TUI's `/setup`): `prefs`, `accounts` and
    /// `projects` come again, read fresh
    SetupCheck,
    /// computer use (P.2): `act` on | off | uninstall | check (its page is
    /// open: the setup check runs, once a second, until leave or 2 min
    /// without a check) | leave | fix (`fix`: a row's fix, his click only)
    ComputerUse {
        act: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fix: Option<String>,
    },
    /// the projects the window shows now (replaces the set; on load and on
    /// each route change): the core holds their hubs and sends their
    /// `projects`/`agents`/`cards` only after the first one (core/hubs.rs)
    Shown { projects: Vec<Project> },
    /// fn held (the helper's talk): the core opens the mic and starts a
    /// talk to main; `page`: a note talk on that bise page instead, its
    /// words to the page, never to main (docs/ambient-pages.md §4.1)
    TalkStart {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<TalkPage>,
    },
    /// fn released: the talk's words go
    TalkEnd,
    /// the talk is dropped, nothing sent
    TalkCancel,
    /// the composer's mic (bar I): the core opens the mic and dictates
    /// into the window that sent this, never to main, never journaled;
    /// `id`: the window's short-lived id, echoed on each `dictation`. One
    /// mic user at a time: refused while a talk or another dictation
    /// runs; fn (talk_start) ends a dictation first
    DictateStart { id: String },
    /// the words so far stay: the mic goes off, one final `dictation`
    DictateStop { id: String },
    /// dropped: the mic goes off, one final `dictation` with `cancelled`
    DictateCancel { id: String },
    /// voice mode (draft, the TUI's ctrl+r twice): `on` with `agent` of
    /// `project`: the core opens the mic and the speaker (voicemode's own
    /// controller), his turns go to that agent (never main unless main is
    /// in view), its answers are said aloud (nothing aloud while quiet).
    /// Another agent while on: it follows him there; `on: false` leaves.
    /// One mic owner: refused while a talk or a dictation runs; while on,
    /// fn and dictation are refused. Its state comes as `voice_mode`
    VoiceMode { project: Project, agent: String, on: bool },
    /// his mic off (m) or on again; the agent still speaks
    VoiceMute { on: bool },
    /// he types (tab): the mic waits; his typed send is the window's
    /// normal `send`, then `voice_type {on: false}`: its answer is said
    VoiceType { on: bool },
    /// send what he said now (the TUI's space tap)
    VoiceSend,
    /// cut the agent off (its voice stops, its turn is stopped)
    VoiceCut,
    /// try a refused project's hub again (his action, `hub_refused`); a
    /// project that isn't refused: nothing
    HubRetry { project: Project },
    /// bar N14: the card in front of him in the focused window (the ⌘I
    /// inbox's current item), or none (both absent: the window blurred,
    /// closed or left the inbox); fn's talk answers it by voice when its
    /// words name an option (`voice_answer`), else they go to main
    CardInView {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        card: Option<u64>,
    },
    /// the slash commands: `commands` answers
    Commands,
    /// ⌘K opened: `index` answers, then again on change (at most every 5 s)
    Index,
    /// a workspace's skills: `skills` answers
    Skills { project: Project },
    /// an `@` query on a workspace's files: `files` answers with this
    /// `rid` (a newer rid's answer wins; `limit`: 50 if none)
    Files {
        project: Project,
        q: String,
        rid: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u32>,
    },
    #[serde(skip)]
    Unknown { tag: String, raw: Value },
}

/// The bise page a note talk is about (`talk_start {page}`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TalkPage {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl AppCmd {
    pub const TAGS: &'static [&'static str] = &[
        "open",
        "project_add",
        "project_remove",
        "project_move",
        "project_rename",
        "prefs_set",
        "sign_in",
        "sign_in_cancel",
        "key_set",
        "key_remove",
        "found_scan",
        "away_back",
        "app_version",
        "plugins",
        "plugin_set",
        "plugin_login",
        "plugin_logout",
        "computer_use",
        "shown",
        "talk_start",
        "talk_end",
        "talk_cancel",
        "dictate_start",
        "dictate_stop",
        "dictate_cancel",
        "voice_mode",
        "voice_mute",
        "voice_type",
        "voice_send",
        "voice_cut",
        "hub_retry",
        "card_in_view",
        "commands",
        "index",
        "skills",
        "files",
        "setup_check",
        "role_set",
    ];

    pub fn decode(line: &str) -> Result<AppCmd, String> {
        decode(parse(line)?, "cmd", Self::TAGS, |tag, raw| AppCmd::Unknown { tag, raw })
    }
}

/// `hubs/<id>/view.json` (S1, written only by that hub): what the sidebar
/// and bise read of a project without its hub.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectView {
    pub v: u32,
    pub project: Project,
    pub written_ms: u64,
    /// set at a clean stop, absent while it runs (readers tell a running
    /// hub by its pid or a ping, never by a flag here)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_ms: Option<u64>,
    pub last_activity_ms: u64,
    pub agents: Vec<Agent>,
    pub cards: Vec<Card>,
    /// ⌘K's index (architect m_11910): its artifacts as the typed
    /// `artifacts` event gives them (the same builder), the newest
    /// [`VIEW_ARTIFACTS`] first; `artifacts_total`: how many it has
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub artifacts_total: u32,
    /// ⌘K's index: its live scheduled tasks, as the typed `scheduled`
    /// event gives them (the same builder, proto_view::scheduled)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scheduled: Vec<ScheduledTask>,
}

/// How many artifacts a `view.json` keeps (it is rewritten on every
/// change, so it stays small); the newest first.
pub const VIEW_ARTIFACTS: usize = 50;

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// A project of ⌘K's index (CoreEv `index`): what its `view.json` says,
/// as of `written_ms` (none: no view yet), and whether its hub runs now
/// (`up`: a stopped hub fires no timers, so the window never shows a
/// past next run as coming).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IndexRow {
    pub project: Project,
    pub up: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_ms: Option<u64>,
    pub agents: Vec<Agent>,
    pub artifacts: Vec<Artifact>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub artifacts_total: u32,
    /// its live scheduled tasks as of `written_ms` (a stopped hub runs
    /// none: a `next_ms` in the past is not coming)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scheduled: Vec<ScheduledTask>,
}
