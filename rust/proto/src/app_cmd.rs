//! The app's commands to its desktop core itself, never forwarded to a hub
//! (`core/<tag>` on its stdio: [`crate::app_rpc`]).

use crate::app_rows::*;
use crate::hub::Mode;
use crate::{decode, parse, Project};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An artifact a `folders_move` moves: its project, its id, its folder
/// before (the window's row's).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FolderItem {
    pub project: Project,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub from: Option<String>,
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
    /// `order`: the 0-based index in the registry's list, 0 = first after
    /// home; past the end = last (bise_home::projects::move_to; architect
    /// m_49117)
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
    /// draft (architect m_28532): use provider `id` (the first run's
    /// pick, settings' key): one tiny live call with `key` (none: the key
    /// found for it, or its sign-in: the ChatGPT plan, OpenRouter's
    /// minted key), answered by `provider_check` checking, then works or
    /// failed. The key is saved only once it works; main's model is
    /// written then when main has none or can't run. A newer one for the
    /// same id drops the older's answer. `base_url`: for a provider
    /// with none (the foundry proxy: an account's `needs_url`), used for
    /// the check and written to config.toml (`[providers.<id>]
    /// base_url`) once it works (Isidor, 28 oct: a key with no URL left
    /// no model)
    ProviderUse {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
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
    /// update-flow: his click on the status pill's `update`: the core
    /// sends `bise/update` to a connected hub (the machine installs the
    /// release once, every project switches to it); answered before the
    /// window swaps its own app
    BiseUpdate {},
    /// the plugins of `project`'s workspace (none: bise's home): `plugins`
    Plugins {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
    },
    /// turn a plugin or a part on or off (plugins.json, bend_plugins' one
    /// writer): `plugins` again. `name`: any PluginId as written (`docs`,
    /// `mistral:gmail#tool:send_email`); a bare name is that plugin, as
    /// the released app sends it (the wire rule: never renamed)
    PluginSet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        name: String,
        on: bool,
    },
    /// grant an MCP App's view its permissions (docs/mcp-apps.md C8:
    /// the add screen's 'its views' step, the plugins page's VIEWS
    /// group): `plugins.json`'s `view_grants` through bend_plugins' one
    /// writer, then `plugins` again. `plugin`: the plugin's name, `uri`:
    /// the view's `ui://` uri, `perms`: what he ticked (none: asked,
    /// nothing granted)
    PluginGrant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        plugin: String,
        uri: String,
        #[serde(default)]
        perms: Vec<String>,
    },
    /// give or forget one of a plugin's settings (its `dev.bise.settings`,
    /// the plugins page's field): `value` none or empty forgets it. Kept
    /// in his MCP secrets by bend_plugins' one writer (the add menu's);
    /// a var the plugin does not declare is an error and writes nothing.
    /// Its servers restart with it (new calls); then `plugins` again
    PluginSetting {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        name: String,
        var: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
    /// connect a Mistral connector (`mistral:<name>`): opens his browser
    /// on Mistral's page (his click only, never a test), then the catalog
    /// is read again until it is ready: `plugins` at each change
    PluginConnect {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        id: String,
    },
    /// read the Mistral catalog again now: `plugins` and `browse`
    PluginsRefresh {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
    },
    /// add a plugin (the add menu): `plugins` again at its end
    PluginAdd {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<Project>,
        add: PluginAddKind,
    },
    /// his click on a `plugin_preview`: the plugin goes into
    /// `~/.agents/plugins`, then `plugins` again; an unknown token is an
    /// error
    PluginAddConfirm {
        token: String,
        /// the values he typed for the preview's `secrets` (a name left
        /// out stays `${NAME}`): written to bise's secrets before the
        /// plugin goes in, never anywhere else
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        secrets: Vec<SecretValue>,
    },
    /// the preview closed without adding: its staged folder goes
    PluginAddCancel { token: String },
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
    /// remove a plugin of his plugins folder with its saved secrets,
    /// logins and on/off ids (after the UI's own confirm), then
    /// `plugins` again; a built-in, a repo's own plugin, a connector, a
    /// skills folder or a part is refused (`bend_plugins::remove`)
    PluginRemove {
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
    /// settings · Setup's live test (page models-setup, his click only:
    /// Test everything, Test again): one real call per model, each model
    /// once even when it serves several roles; `models` none = every
    /// model behind the gauge. Answered by `setup_gauge` (its `testing`,
    /// then each result as it ends)
    SetupTest {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        models: Option<Vec<String>>,
    },
    /// the helper's permissions, relayed by Electron's main (it owns the
    /// helper) at each helper `perms` and again after a core (re)start: a
    /// fact the core keeps for settings · Setup (none: not known yet)
    HelperPerms {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mic: Option<crate::helper::Grant>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        accessibility: Option<crate::helper::Grant>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        screen: Option<crate::helper::Grant>,
    },
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
    /// his change of the artifacts library's folders (one list for every
    /// project, `bise_home::library`, written here): `library_folders` answers;
    /// an undo of a move sends the artifacts back through their hubs
    Folders { op: crate::rows::FolderOp },
    /// his move of artifacts (any projects) to a library folder (none:
    /// unfiled): each project's `artifacts_move` (its hub started if
    /// needed); `from` each one's folder before, the undo slot's
    FoldersMove {
        items: Vec<FolderItem>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        folder: Option<String>,
    },
    /// the window's running screen (all of bise) is shown (`on`) or left:
    /// while on, the core asks every project hub that runs for its
    /// `processes` (watched: again every 2 s) and holds those hubs, never
    /// starting one (a stopped hub killed its processes); off, the window
    /// gone or a new core: the holds and the watches go (architect m_38378).
    /// `by`: which holder (none: the running screen); a process's panel is
    /// `panel:<project>/<agent>/<id>` with its `project`, so it watches that
    /// hub only and the screen's off never ends its watch (architect
    /// m_47406)
    ProcessesAll {
        on: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        by: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        project: Option<String>,
    },
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
    /// the talk's still is dropped (his ✕ on it, ambient-lead m_21912):
    /// the core forgets it, says `shot {file: null}`, and the talk goes on
    /// with his words only. `id`: only that still (the spotlight closed
    /// on it), never a newer one; none: whatever still is pending
    ShotDrop {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
    },
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
    /// a `kept` take transcribed again from its sound, with a fresh
    /// listener: one final `dictation` for that id, no partials; an id
    /// the core no longer keeps: a final with error 'nothing to try again.'
    DictateRetry { id: String },
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
    /// R14: he took `path` (as `files` answered it) from a workspace's
    /// `@` list: it ranks first in that list's next answers (the TUI's
    /// memory, per workspace, in the core's memory). A folder is browsed,
    /// never picked
    FilePicked { project: Project, path: String },
    /// R8: the setup a `setup_first` opened closed without the provider
    /// working: the held `/model` line is dropped
    SetupFirstCancel,
    /// N1 (the TUI's /keychain [on|off]): `on` moves his keys and sign-ins
    /// to the macOS keychain, `off` back to files in ~/.bise, none just
    /// asks where they are; answered by `keychain` (a second move while
    /// one runs: `error {cmd: keychain}`, never queued)
    Keychain {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<String>,
    },
    /// S29: his act on an agent's computer use, by its `computer_live`
    /// key: `take` (he takes over: it pauses), `hand_back` (it goes on),
    /// `stop` (as the TUI's /stop), `peek` (a still, `max_width` px wide
    /// at most, answered by `computer_peek`). A key not in the last
    /// `computer_live`, or of no project of his: `error {cmd: computer}`
    Computer {
        act: String,
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_width: Option<u32>,
    },
    /// fn's talk: the still the helper took of his front window (`path`
    /// in the shots folder), sent with the talk's words
    Shot {
        path: String,
        #[serde(default)]
        app: String,
        #[serde(default)]
        title: String,
    },
    /// his words to main (the capsule, the spotlight; its `files` list
    /// went with the hub's in issue 31: a drop rides as parts)
    Send {
        #[serde(default)]
        text: String,
    },
    /// fn space's typed text sent with tab: main starts an agent on it
    /// (only main starts agents; ambient-lead m_6513)
    Start {
        #[serde(default)]
        text: String,
    },
    /// his answer to the main hub's card `card`
    Answer {
        card: u64,
        #[serde(default)]
        reply: String,
    },
    /// main's voice stops (its words stay)
    Hush,
    /// he talks over main: its voice stops and his talk starts
    CutIn,
    /// a quiet moment (the app's read: a call, a shared screen, full
    /// screen): text only, no voice
    /// (docs/ambient-app.md §4)
    Quiet {
        #[serde(default)]
        on: bool,
    },
    /// the settings' voice answers (amb-mac m_5327): off, main's answers
    /// are text only; on (the default), as usual
    Voice {
        #[serde(default = "crate::yes")]
        on: bool,
    },
    /// the menu bar's "stop watching" (amb-mac m_5475): standing order
    /// `id` stops (the hub's scheduled/stop)
    EveryStop { id: u64 },
    /// harness A: the words the fake voice hears at the next talk (only
    /// with the fake voice on; an error otherwise)
    FakeWords {
        #[serde(default)]
        text: String,
    },
    /// an agent's preview (now, its last actions, what waits on him, its
    /// last report, its pages), sent again on its events while it stays
    /// selected (round 10)
    AgentPreview { agent: String },
    /// an agent's history panel: the newest entries (`before` none: the
    /// panel opens and its live entries follow) or the ones before a pos;
    /// `limit`: 60 if none, 1..200
    AgentHistory {
        agent: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u32>,
    },
    /// the history panel closed: no more live entries
    AgentUnwatch { agent: String },
    /// his words to an agent: `now` (steered into its turn, the default)
    /// or `queued` (held by the core until the agent is free)
    AgentSend {
        agent: String,
        #[serde(default)]
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<Mode>,
    },
    /// follow an agent's events (on, the default) or not
    Follow {
        agent: String,
        #[serde(default = "crate::yes")]
        on: bool,
    },
    /// stop an agent's turn (the hub's interrupt; the window asks first)
    Stop { agent: String },
    /// `/archive <agent>`; `stop_first`: `--force`, a working agent
    Archive {
        agent: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        stop_first: bool,
    },
    /// `/restore <agent>`: back in his list
    Unarchive { agent: String },
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

/// What the Plugins add menu adds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginAddKind {
    /// an MCP server: a command, or a remote server's address; `secrets`:
    /// the names of its env and header values he typed in (their values go
    /// to the secrets store, never on this line)
    Mcp {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        secrets: Vec<String>,
    },
    /// another agent's MCP servers, found on disk
    Import { from: ImportFrom },
    /// a plugin folder on his Mac
    Folder { path: String },
    /// a plugin in a git repo (`subdir`: its folder in the repo)
    Git {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subdir: Option<String>,
    },
    /// a marketplace (a git repo with a marketplace.json)
    Marketplace { url: String },
    /// make a plugin with an agent: his words, a request to main
    Agent { words: String },
}

/// A secret he typed, by its name (an env var's, a header's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SecretValue {
    pub name: String,
    pub value: Secret,
}

/// A secret's text: its `Debug` and `Display` print `•••`, so a log, a
/// debug ring or a panic never shows it; the wire carries it as a plain
/// string. Read it with [`Secret::expose`], only to store it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Secret {
        Secret(s.into())
    }

    /// The text itself: only for the secrets store.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("•••")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("•••")
    }
}

/// Whose MCP servers an import reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ImportFrom {
    Claude,
    Codex,
    Cursor,
}

impl AppCmd {
    /// Gone tags never come back with another meaning: `fn_context` (the
    /// fn context with his selection, removed for the spotlight's ❝,
    /// architect m_53938; decodes as Unknown, the core refuses it).
    pub const TAGS: &'static [&'static str] = &[
        "open",
        "project_add",
        "project_remove",
        "project_move",
        "project_rename",
        "prefs_set",
        "sign_in",
        "sign_in_cancel",
        "provider_use",
        "key_remove",
        "found_scan",
        "away_back",
        "app_version",
        "bise_update",
        "plugins",
        "plugin_set",
        "plugin_grant",
        "plugin_setting",
        "plugin_connect",
        "plugins_refresh",
        "plugin_add",
        "plugin_add_confirm",
        "plugin_add_cancel",
        "plugin_login",
        "plugin_logout",
        "plugin_remove",
        "computer_use",
        "shown",
        "folders",
        "folders_move",
        "processes_all",
        "talk_start",
        "talk_end",
        "talk_cancel",
        "shot_drop",
        "dictate_start",
        "dictate_stop",
        "dictate_cancel",
        "dictate_retry",
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
        "file_picked",
        "setup_first_cancel",
        "keychain",
        "setup_check",
        "setup_test",
        "helper_perms",
        "role_set",
        "computer",
        "shot",
        "send",
        "start",
        "answer",
        "hush",
        "cut_in",
        "quiet",
        "voice",
        "every_stop",
        "fake_words",
        "agent_preview",
        "agent_history",
        "agent_unwatch",
        "agent_send",
        "follow",
        "stop",
        "archive",
        "unarchive",
    ];

    pub fn decode(line: &str) -> Result<AppCmd, String> {
        decode(parse(line)?, "cmd", Self::TAGS, |tag, raw| AppCmd::Unknown { tag, raw })
    }
}
