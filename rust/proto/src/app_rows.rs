//! The rows the desktop core's own lines carry ([`crate::core_ev::CoreEv`],
//! [`crate::app_cmd::AppCmd`]): the sidebar's projects, accounts, plugins,
//! computer use, roles, the composer's pick lists and ⌘K's index.

use crate::rows::{Agent, ArtLink, Artifact, Card, CardBatch, CardPage, Opt, ReportKind, ScheduledTask};
use crate::{Pos, Project};
use serde::{Deserialize, Serialize};

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
    /// its hub couldn't start, in plain words (the cause and its log);
    /// absent: it runs, or nothing tried yet. The core keeps retrying,
    /// backing off; the row says it at once, never a silent loop
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down: Option<String>,
    /// its pulse (the H1 sidebar, rows::pulse): a held hub's own
    /// `hub/pulse`, else its `view.json`'s (a stopped hub's last one).
    /// New artifacts and unread are counted against `seen.<project>`
    /// (rows::pulse's header); absent: none known yet
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulse: Option<crate::rows::Pulse>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum OpenWhat {
    Thread,
    Review,
    Artifact,
    Worktree,
    /// an MCP Apps view: `agent` its agent, `id` its call (docs/mcp-apps.md C9)
    View,
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
    /// a plan signed in: which one ("Plus", "Pro"); none known: absent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// L5: the catalog provider it sets up (a model row's `provider_id`)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
    /// a key found for it: where (never the key)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<KeyFrom>,
    /// where its keys are made (the key field's "get one" link); "" none
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub keys_url: String,
    /// a key of a coding plan (GLM, Kimi, MiniMax): the first run lists
    /// them under their own row
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub coding: bool,
    /// its provider has no base URL (none built in, none in config.toml,
    /// the env or a .env: the foundry proxy): the key field asks for it
    /// too, and `provider_use` carries it
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub needs_url: bool,
}

/// Where a provider's key was found (bise_catalog::auth::From).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum KeyFrom {
    /// an environment variable (its name)
    Env { var: String },
    /// bise's auth.json
    AuthFile,
    /// an old `.env` file (its path, ~ for his home)
    EnvFile { path: String },
    #[serde(other)]
    Unknown,
}

/// Where a `provider_use` check is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Checking,
    /// it answered: the key is saved, main's model written if it had to be
    Works,
    Failed,
    #[serde(other)]
    Unknown,
}

/// Why a check failed (the TUI's keycheck::Why), each with its own fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CheckWhy {
    /// the provider refused the key (a plan: its sign-in expired)
    WrongKey,
    /// the key works, the account can't pay (a plan: its limit)
    NoCredit,
    /// the provider doesn't know the model
    Model,
    /// the key may not use the model (a plan: plan use is off)
    NoAccess,
    /// no answer, or the provider's own trouble (`said`: the reason)
    Unreachable,
    /// the provider has no base URL, or a setting is wrong (`said`: what to set)
    Config,
    /// no key found nor given (the window asks for one)
    NoKey,
    #[serde(other)]
    Unknown,
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

/// One row of the Plugins list (the TUI's `/plugins`, the app's
/// plugins place): a real plugin, a Mistral connector, a local MCP server
/// or a skills folder, with its parts. Built by
/// `bend_plugins::catalog::rows`, its one builder. `id` is a
/// `bend_plugins::id::PluginId` as written (`docs`, `mistral:gmail`,
/// `skills:personal`), never split by a reader.
///
/// The wire rule (lib.rs): `name`, `state`, `scope`, `what` and `logins`
/// keep the meaning v2026.10.2-28 gave them (the released app reads
/// them); the unified list's facts came on top, each with a default, so a
/// released line still reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Plugin {
    /// its id as a string (a real plugin's: its name)
    #[serde(default)]
    pub id: String,
    /// what `plugin_set` and the released app take: a real plugin's name,
    /// else its id
    pub name: String,
    #[serde(default)]
    pub kind: PluginKind,
    pub scope: PluginScope,
    /// what he reads (`Gmail`, `Personal skills`, `docs`)
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what: Option<String>,
    /// its icon's name or address (none: the kind's own)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// a real plugin's manifest version (`0.1.0`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// where it lives on his Mac: a plugin's root, a skills folder
    /// (absolute; none for a connector)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub state: PluginState,
    /// turned on (plugins.json: everything on but the opt-in plugins)
    #[serde(default)]
    pub on: bool,
    /// the composer's `@` menu offers it for a turn (at-plugins, architect
    /// m_53238): `bend_plugins::resolve::Plugin::pickable`, the one rule
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub pickable: bool,
    /// a pickable plugin that waits on him (a login, a server that
    /// failed): the `@` menu's row says so (none: it works)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick_waits: Option<PickWaits>,
    /// its line for him in the `@` menu: its manifest's
    /// `dev.bise.tagline` (`what` is written for the model); none: the
    /// menu says `what`'s first sentence
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,
    /// not his to turn off (the `skills:bise` folder: pages need it)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub locked: bool,
    /// what its button does (none: no button)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<PluginAction>,
    /// a Mistral connector's own login (it is its own server): pending
    /// while its connect runs (no button then), in when ready, out when
    /// not connected; none for every other kind
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<LoginState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PluginPart>,
    /// what to try it with: its manifest's `dev.bise.prompts` (at most
    /// 3, one line each); a click puts one in the composer, never sends it
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<String>,
    /// what he may give it (its manifest's `dev.bise.settings`: an API
    /// key), each set with `plugin_setting`; never a value, only `set`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settings: Vec<PluginSetting>,
    /// its MCP server runs on Node and none (18 or newer) is on his Mac:
    /// the row's own state says 'needs Node 18 or newer' beside its switch,
    /// which still turns it on (architect m_51637)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub needs_node: bool,
    /// the released app's (before desktop-v2026.10.16) remote MCP servers
    /// that log in: kept on the wire for the released law, always empty
    /// now; a server's login is its `mcp` part's `login`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logins: Vec<PluginLogin>,
}

/// One of a plugin's settings (`bend_plugins::resolve::Setting`): a field
/// of the plugins page. The value is never on the wire, not even a
/// non-secret one: `set` says whether he gave one (his MCP secrets, or
/// the environment). `label` and `help` are the plugin's own words
/// (third-party text: drawn escaped, like its description).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginSetting {
    /// the `${VAR}` its mcp.json reads, what `plugin_setting` takes
    pub var: String,
    pub label: String,
    /// typed as a password
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub secret: bool,
    /// the plugin cannot work without it
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// an example of a value: the empty field's placeholder
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// a value is given
    #[serde(default)]
    pub set: bool,
}

/// A plugin's remote MCP server that logs in through his browser (the
/// released row's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginLogin {
    /// what `plugin_login` takes (its id, or plugin/server)
    pub name: String,
    pub host: String,
    pub state: LoginState,
    /// its tools, once a session connected it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<u32>,
}

/// What a Plugins row is (its tag in the list).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// a folder with a plugin.json
    #[default]
    Plugin,
    /// a connector of his Mistral account
    Mistral,
    /// a local MCP server (a plugin folder bise wrote for it)
    Mcp,
    /// a folder of loose skills
    Skills,
    /// a newer core's value
    #[serde(other)]
    Unknown,
}

/// Where a Plugins row comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum PluginScope {
    /// shipped with bise
    BuiltIn,
    /// his (`~/.agents/...`, `~/.vibe/skills`)
    User,
    /// this project's (`<repo>/.agents/...`)
    Workspace,
    /// his Mistral account
    Mistral,
    /// a newer core's value
    #[serde(other)]
    Unknown,
}

/// Whether a Plugins row loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// it loads (when on)
    Loaded,
    /// turned off
    Disabled,
    /// a plugin of the same name wins (workspace over user over built-in)
    Shadowed,
    /// its manifest is wrong (the diagnostics say why)
    Invalid,
    /// not connected yet: a connector in Browse
    Available,
    /// a newer core's value
    #[serde(other)]
    Unknown,
}

/// What a Plugins row's button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PluginAction {
    /// connect a Mistral connector (`plugin_connect`: his browser, or one
    /// click)
    Connect,
    /// set it up on Mistral's page (credentials)
    SetUp,
    /// log in to a remote MCP server (`plugin_login`)
    LogIn,
    /// a newer core's value
    #[serde(other)]
    Unknown,
}

/// A part of a plugin, each turned on or off by its `id` (a
/// `bend_plugins::id::PluginId` part: `docs#skill:docs`,
/// `linear#mcp:linear`, `mistral:gmail#tool:send_email`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginPart {
    pub id: String,
    pub kind: PartKind,
    /// its own name (a remote server's: what `plugin_login` takes)
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what: Option<String>,
    pub on: bool,
    /// how it is, in his words (`connected · 23 tools`, `needs a login`,
    /// `✗ exited at start: …`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// a remote MCP server that logs in: its login, as data (the buttons
    /// read this, never `status`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<LoginState>,
    /// an MCP server that failed: why, in one line (`exited at start:
    /// DATABASE_URL is not set`). Some marks the failure; the text is for
    /// display only (no reader matches on it, nor on `status`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// an MCP server's tools (none: not listed yet, or not a server)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<PluginPart>,
    /// how many tools an MCP server has, once a session connected it
    /// (known before its `tools` are listed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_count: Option<u32>,
    /// a skill's `SKILL.md`, where the catalog found it (its realpath for
    /// a plugin's skill); none for every other kind, and from a hub before
    /// it. The desktop's skill tab reads that file and lists its folder
    /// through main, fenced to the paths of the rows it got (nm-pages-b,
    /// architect m_43316)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// an MCP App's view (kind `view`): its uri, what it declares and
    /// what he granted (docs/mcp-apps.md C8); none for every other kind
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<PartView>,
}

/// A view part's facts (MCP Apps, docs/mcp-apps.md C8): the plugins
/// page's VIEWS group draws its grant ticks from them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PartView {
    /// its `ui://` uri
    pub uri: String,
    /// the server that serves it (none: a Vibe static view)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// the permissions it declares (`camera`, `microphone`, `midi`,
    /// `clipboardWrite`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared: Vec<String>,
    /// the origins it reaches, as declared (its CSP's domains, every kind)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domains: Vec<String>,
    /// what he granted it (`plugins.json`'s `view_grants`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub granted: Vec<String>,
    /// he was asked (at the add or on its page): false is 'not granted'
    #[serde(default)]
    pub asked: bool,
}

/// What a plugin will run on his Mac once added, shown before his click
/// (`core_ev::PluginPreview`, `bend_plugins::add::stage`): nothing of it
/// runs before `plugin_add_confirm`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginPreview {
    /// the plugin's name, its folder in `~/.agents/plugins`
    pub name: String,
    /// where it comes from, in his words (`git github.com/acme/plugins`,
    /// `Claude Code (~/.claude.json)`)
    pub from: String,
    /// every MCP server it starts or calls
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<PluginRun>,
    /// its skills' names
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    /// the names of the values kept in bise's secrets (never a value)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<String>,
    /// what the source had that bise leaves out (`linear: oauth_resource`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dropped: Vec<String>,
    /// what else to know (`it shadows the built-in docs`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// it replaces his plugin of that name (a new import of the same
    /// source)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<String>,
    /// its views known before it runs (its Vibe static views): each with
    /// its domains and the permissions it asks, ticked on the add screen
    /// (docs/mcp-apps.md C8); a server's views come once it runs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<PluginPart>,
}

/// One MCP server of a [`PluginPreview`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginRun {
    pub server: String,
    pub transport: RunTransport,
    /// a local server's program and arguments, exactly (never one shell
    /// string)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
    /// a remote server's host (its path may hold a key)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

/// How a [`PluginRun`] is reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RunTransport {
    /// a program on his Mac
    Stdio,
    /// Streamable HTTP
    Http,
    /// the legacy HTTP+SSE transport
    Sse,
    /// a newer core's value
    #[serde(other)]
    Unknown,
}

/// The Plugins list in a few numbers, for the sidebar's plugins row and
/// the place's 'connect your tools' band (`core_ev::PluginsSummary`).
/// Built by `bend_plugins::catalog::summary` from the same rows, its one
/// builder: the window adds nothing up, it draws what it gets.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginsSummary {
    /// rows on and loading (a plugin, a ready connector, a server, a
    /// skills folder)
    pub on: u32,
    /// rows he turned off
    pub off: u32,
    /// on rows that wait on him: a login to do, a server that failed
    /// (typed fields only: a part's `login: needs`, its `error`)
    pub waiting: u32,
    /// his Mistral connectors logged in
    pub connected: u32,
    /// every Mistral connector, connected or not
    pub connectors: u32,
    /// what to suggest, waiting ones first (login, error), then mistral,
    /// import, workspace, marketplace
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggest: Vec<PluginSuggest>,
}

/// One suggestion of a [`PluginsSummary`]. `n` is the full count; `ids`
/// (PluginId strings) and `titles` (what he reads, same order) are capped
/// at [`SUGGEST_CAP`], so `n >= ids.len() == titles.len()`. `whys` says
/// why bise proposes each one: empty (an older core) or `titles`' length,
/// in the same order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PluginSuggest {
    pub kind: SuggestKind,
    pub n: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub titles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub whys: Vec<Why>,
    /// a marketplace item's source ('openai/plugins'), the line names it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// At most this many `ids` and `titles` per [`PluginSuggest`].
pub const SUGGEST_CAP: usize = 5;

/// At most this many places in a [`Why`]'s `where`.
pub const WHERE_CAP: usize = 3;

/// Why bise proposes one suggestion, in his words (built by
/// `bend_plugins::catalog::summary`; the window draws it, decides nothing).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Why {
    /// the line under the title ('a SENTRY_DSN in 2 of your projects')
    pub text: String,
    pub source: WhySource,
    /// where it was seen (project names), at most [`WHERE_CAP`]; only
    /// for `seen`
    #[serde(default, rename = "where", skip_serializing_if = "Vec::is_empty")]
    pub places: Vec<String>,
}

/// Where a [`Why`] comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum WhySource {
    /// something bise saw on this Mac (a project's remote, file or env
    /// name; an app's config; a login or a server that waits)
    Seen,
    /// a short generic why for that tool (the words catalog)
    Generic,
    /// the start of its own description
    Description,
    /// a newer core's value
    #[serde(other)]
    Unknown,
}

/// What a [`PluginSuggest`] proposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum SuggestKind {
    /// Mistral connectors not connected yet (`n`: all of them; `ids`: the
    /// most useful first)
    Mistral,
    /// MCP servers found in other apps' configs (`n`: all their servers;
    /// `ids`: the plugin each app's import makes, `titles`: the apps)
    Import,
    /// on rows whose login waits on him
    Login,
    /// on rows with a server that failed
    Error,
    /// this project's own plugins and skills (`n`: its skills and
    /// plugins), only for a project
    Workspace,
    /// an added marketplace's plugins not added yet, one item per source
    Marketplace,
    /// a newer core's value
    #[serde(other)]
    Unknown,
}

/// What a plugin part is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    Skill,
    Mcp,
    Tool,
    /// an MCP App's view (its `view` facts)
    View,
    /// a newer core's value
    #[serde(other)]
    Unknown,
}

/// Why his Mistral connectors may be missing or old (the connectors'
/// cache, `bend_plugins::connectors::Problem`): the window says so from
/// the kind, never from `why`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConnectorsProblem {
    /// no Mistral key: his account isn't connected
    NoKey,
    /// the keychain is locked: the key is there, unread
    Locked,
    /// the last fetch failed (`why`: its words, display only); the list is
    /// the last good one
    Fetch { why: String },
    /// a newer core's value
    #[serde(other)]
    Unknown,
}

/// What a pickable plugin waits on before it works (at-plugins, designer
/// m_53299): the `@` menu says it in place of its description, and a
/// pick still makes its chip. bend_plugins' summary rules (typed fields
/// only: a part's `login: needs`, its `error`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PickWaits {
    /// a server of it needs his login
    Login,
    /// a server of it failed to start
    Failed,
    /// a newer core's value
    #[serde(other)]
    Unknown,
}

/// A remote MCP server's login.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LoginState {
    /// it said it needs one
    Needs,
    /// logged in
    In,
    /// not logged in
    Out,
    /// its browser is open
    Pending,
    /// a newer core's value
    #[serde(other)]
    Unknown,
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

/// An agent that uses computer use now (S29, `computer_live`): one entry
/// of the machine's state.json (`bise_computer_use::state::agents`),
/// idle agents absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ComputerAgent {
    /// `<hub>.<dir>` (docs/issues/18): what `computer {act, key}` takes
    pub key: String,
    /// its project; none: its hub is none of his registered projects (an
    /// agent of another bise: shown, never acted on)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<Project>,
    pub agent: String,
    /// the app or browser it drives ("Chrome", "Figma")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driving: Option<String>,
    /// where in it (a site, a window title)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since_ms: Option<u64>,
    /// he took over (`take`): it waits until `hand_back`
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub paused: bool,
    /// stopped (`stop`, the TUI's /stop): it asks before it starts again
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub stopped: bool,
    /// its line under its head, from `bise_computer_use::live::line` (the
    /// TUI's status row draws the same): how to colour it, the mark, the
    /// words (`driving Chrome · amazon.fr`). No age in them: the window
    /// adds ` · 2 min` from `since_ms` to a driving line, and builds no
    /// other word.
    pub kind: LiveKind,
    pub mark: String,
    pub words: String,
}

/// How a computer-use line is coloured (`bise_computer_use::live::Kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum LiveKind {
    /// it drives an app now (the working colour)
    Driving,
    /// he took the wheel: it waits on him (accent)
    Paused,
    /// stopped until he writes to it (dim)
    Stopped,
}

/// A still of what an agent drives (`computer_peek`): inline, never
/// stored (no file, no image store, no log).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PeekShot {
    /// base64
    pub data: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub at_ms: u64,
}

/// How far voice mode got saying an agent's message (`voice_mode`'s `lit`,
/// amb-web m_16805): the window inks `text[..said]` of that thread entry
/// and leaves the rest faint ('you cut in' when `cut`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VoiceLit {
    /// the agent's thread entry being said (or last said)
    pub pos: Pos,
    /// UTF-16 code units of the entry's text said so far, as JS slices
    /// it: `text.slice(0, said)` (the end of the last said word; 0: none
    /// yet)
    pub said: u32,
    /// he cut in: the rest stays unsaid
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cut: bool,
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

/// One of the features settings · Setup counts (page models-setup, pick
/// B): what he gets, each tied to a role or a permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FeatureId {
    /// talk to bise (main's model)
    Talk,
    /// the agents main starts
    Agents,
    /// titles and summaries (the small jobs model)
    Small,
    /// images and computer use (main's model reads images)
    Vision,
    /// dictation and fn (the model that listens, the mic)
    Dictation,
    /// voice mode (listens, speaks, main answers)
    VoiceMode,
    /// auto approvals (the checker)
    Approvals,
    #[serde(other)]
    Unknown,
}

/// A model's job behind a feature: the config's roles (bise_catalog::roles)
/// and voice mode's voice (`tts`, `[voice_mode] tts_model`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RoleId {
    Main,
    Agents,
    Small,
    Voice,
    Classify,
    Tts,
    #[serde(other)]
    Unknown,
}

/// ✓ it works, ✗ it failed (required, or set up and broken: waits on
/// him), ○ off (optional, never set up, or the model can't).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FeatureState {
    On,
    Failed,
    Off,
    #[serde(other)]
    Unknown,
}

/// Where a role's model comes from: config.toml, an env var (`var`), or
/// bise's default (it may follow another role).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PartSource {
    Config,
    Env,
    Default,
    #[serde(other)]
    Unknown,
}

/// The one thing that fixes or turns on a feature. Words are the
/// window's (its words catalog); the core sends only what they name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum SetupFix {
    /// add a key for that provider (`url`: where its keys are made)
    AddKey {
        provider: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        url: String,
    },
    /// sign in again (a plan whose sign-in expired)
    SignIn { provider: String },
    /// add credit (`url`: its billing page, else its keys page)
    Credit {
        provider: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        url: String,
    },
    /// pick another model for that role (the model is gone or refused)
    PickModel { role: RoleId },
    /// an env var wins over the setting: remove it (where it is set is
    /// his shell's; the var name is bise_home::env's)
    RemoveEnv { var: String },
    /// allow the microphone
    AllowMic,
    /// turn that role on (the checker set to "off")
    TurnOn { role: RoleId },
    /// test again (unreachable: the network, the provider's own trouble)
    TestAgain,
    #[serde(other)]
    Unknown,
}

/// One model behind a feature, with its last test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FeaturePart {
    pub role: RoleId,
    /// "provider/id"; "" none (voice mode's voice when no provider speaks)
    pub model: String,
    /// its provider's name for people ("Mistral")
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
    pub source: PartSource,
    /// the env var that set it (`source` env)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,
    /// unset: the role it follows
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follows: Option<RoleId>,
    pub state: FeatureState,
    /// failed: why (no key, a test's verdict)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<CheckWhy>,
    /// the provider's own words of its last failed test
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
    /// its last test took this long (the whole check, never "latency")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    /// its last test's end, ms since the epoch (none: never tested since
    /// its key changed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tested_at: Option<u64>,
}

/// One feature of settings · Setup, computed in the core
/// (bise_catalog::features), never in the window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FeatureRow {
    pub id: FeatureId,
    pub state: FeatureState,
    /// the models it needs, in the order they act
    pub parts: Vec<FeaturePart>,
    /// failed or off: what fixes it or turns it on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<SetupFix>,
    /// the mic refused (dictation, voice mode): the helper's fact
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mic_denied: bool,
    /// on, but better with this (the checker on Jev): never a failure,
    /// never `next`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub better: Option<SetupFix>,
}

/// The action that unlocks the most: a failure before a feature that is
/// off; between two, the one that fixes more features.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetupNext {
    pub fix: SetupFix,
    pub unlocks: Vec<FeatureId>,
}

/// settings · Setup's gauge: `on` of `of` features work (5/7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SetupGauge {
    pub rows: Vec<FeatureRow>,
    pub on: u32,
    pub of: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<SetupNext>,
    /// models under test now (Test everything, Test again)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub testing: Vec<String>,
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

/// What completes one argument of a command (the catalog's commands.rs
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
/// COMMANDS, the catalog: same name, desc, order, args). `runnable`: for `hub` and
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

pub(crate) fn is_zero(n: &u32) -> bool {
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
    /// les bims: how much it has done, as its `view.json` says (none: no
    /// view, or one written before growth)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub growth: Option<crate::rows::Growth>,
}

/// An agent's row in the capsule's `state` (main aside): `status` the
/// row's word (idle, working, waiting, done, failed, blocked), `since_ms`
/// when the core saw it change, `waits`: his open cards from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StateAgent {
    pub name: String,
    pub status: String,
    pub title: String,
    pub purpose: String,
    pub since_ms: u64,
    pub waits: u32,
    pub followed: bool,
    pub archived: bool,
    pub note: String,
    pub report: String,
    pub working: bool,
    pub turn_ms: Option<u64>,
}

/// One of his cards in the capsule's `state`: its words without the
/// options it lists, its options, `urgent` (it asks even on a call),
/// `label` (main's own question), the page block it is about, the batch
/// of drafts it stands for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StateCard {
    pub id: u64,
    pub kind: String,
    pub agent: String,
    pub text: String,
    pub options: Vec<Opt>,
    pub urgent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<CardPage>,
    /// what it is about, any artifact, pages included (card-links)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ArtLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<CardBatch>,
}

/// The page a note talk is about, as the capsule's `phase` says it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PhasePage {
    pub id: String,
    pub title: String,
}

/// One of an agent's last actions in its preview: `kind` tool, land,
/// message, report or page, one line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PreviewAction {
    pub at_ms: u64,
    pub kind: String,
    pub text: String,
}

/// A card of the agent's that waits on him, in its preview.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PreviewWait {
    pub card_id: u64,
    pub question: String,
}

/// An agent's last report in its preview.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PreviewReport {
    pub kind: ReportKind,
    pub text: String,
    pub at_ms: u64,
}
