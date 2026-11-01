//! The unified Plugins list (docs/plugins.md, "Contract"): one pure
//! builder, [`rows`], of the wire rows (`bise_proto::app_rows::Plugin`)
//! that the TUI and the desktop's core both show, each in its own process
//! (no hub.sock method: the state is his files). Every input is read by
//! its owner first ([`Inputs`]); `rows` reads only the MCP servers' last
//! status files and the login tokens, by the folders it is given.
//!
//! The rows, in order: bise's own skills (`skills:bise`, locked on), the
//! real plugins (built-in, user, workspace: the resolver's), his Mistral
//! connectors that are ready, the skills folders (personal, this
//! project's, Vibe's), then the connectors not connected yet (state
//! `available`, a `connect` or `set_up` button: Browse). A plugin folder
//! bise wrote for an MCP server ([`crate::import::MARK`]) is kind `mcp`.
//! A row's `on` and each part's come from `plugins.json`
//! ([`StateV2::is_on`]); an MCP server's status, tool count, tools and
//! error from its last status file ([`crate::status`]); a remote one's
//! login from its tokens. A connector with no display name gets its
//! name as people read it ([`crate::title::display_name`]), once here,
//! so every reader shows the same title. No secret reaches a row: a server shows its
//! command's name and package or its URL's host, never an argument, a
//! header, an env value or a URL path.

use crate::id::{PartKind as IdPart, PluginId, SkillsScope};
use crate::model::{ConnectorAuth, ConnectorEntry, SkillEntry, StateV2};
use crate::resolve::{self, HttpServer, StdioServer};
use crate::title::display_name;
use crate::{connectors, status};
use bise_proto::app_rows::{ConnectorsProblem, LoginState, PartKind, Plugin, PluginAction, PluginKind, PluginPart, PluginScope, PluginSetting, PluginState};
use bise_proto::words::settings;
use std::path::{Path, PathBuf};

mod summary;
pub mod views;
pub use summary::{summary, Market, USEFUL};

/// What the list is built from.
pub struct Inputs<'a> {
    /// the real plugins, built-in, user and workspace (`resolve::resolve`)
    pub resolution: &'a resolve::Resolution,
    /// his Mistral connectors (`connectors::cached` / `connectors::load`)
    pub catalog: &'a connectors::Catalog,
    /// the skills of the skills folders, off ones too (the skills
    /// scanner); a plugin's own skills come from `resolution`
    pub skills: &'a [SkillEntry],
    /// the on/off file (`state::read`)
    pub state: &'a StateV2,
    /// the views' grants (`state::grants`, the same file)
    pub grants: &'a crate::state::Grants,
    /// each MCP server's last state (`status::dir`)
    pub status_dir: Option<&'a Path>,
    /// the remote servers' login tokens (`oauth::store_dir`)
    pub oauth_dir: Option<&'a Path>,
    /// the logins open in his browser, by name (a remote server's), and
    /// the connectors whose connect runs, by id (`mistral:gmail`)
    pub pending_logins: &'a [String],
    /// his MCP secrets (`Home::mcp_secrets_dir`): whether each setting
    /// of a plugin is given (`set`), never its value
    pub secrets_dir: Option<&'a Path>,
}

/// At most this many `prompts` per row (a longer manifest list is cut).
pub const MAX_PROMPTS: usize = 3;

/// The Plugins list: real plugins, connectors, MCP plugins and skills
/// folders, each with its parts and whether it is on.
pub fn rows(inputs: &Inputs) -> Vec<Plugin> {
    let mut out = Vec::new();
    out.extend(skills_row(inputs, SkillsScope::Bise));
    let names = login_names(inputs.resolution);
    out.extend(inputs.resolution.plugins.iter().map(|p| plugin_row(inputs, p, &names)));
    let (ready, browse): (Vec<&ConnectorEntry>, Vec<&ConnectorEntry>) =
        inputs.catalog.connectors.iter().partition(|c| c.auth == ConnectorAuth::Ready);
    out.extend(ready.into_iter().map(|c| connector_row(inputs.state, c)));
    for scope in [SkillsScope::Personal, SkillsScope::Workspace, SkillsScope::Vibe] {
        out.extend(skills_row(inputs, scope));
    }
    out.extend(browse.into_iter().map(|c| browse_row(c, inputs.pending_logins)));
    out
}

fn scope_of(s: resolve::Scope) -> PluginScope {
    match s {
        resolve::Scope::BuiltIn => PluginScope::BuiltIn,
        resolve::Scope::User => PluginScope::User,
        resolve::Scope::Workspace => PluginScope::Workspace,
    }
}

fn state_of(s: resolve::State) -> PluginState {
    match s {
        resolve::State::Loaded => PluginState::Loaded,
        resolve::State::Disabled => PluginState::Disabled,
        resolve::State::Shadowed => PluginState::Shadowed,
        resolve::State::Invalid => PluginState::Invalid,
    }
}

/// A plugin's `dev.bise.prompts`, cut to [`MAX_PROMPTS`], blank ones out.
pub fn prompts(list: &[String]) -> Vec<String> {
    list.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).take(MAX_PROMPTS).map(String::from).collect()
}

/// The name `/plugins login` and `plugin_login` take for each remote
/// server that logs in: its id, or `plugin/id` when two loaded plugins
/// have a server of that id (the same rule as `login::targets`).
fn login_names(res: &resolve::Resolution) -> Vec<(String, String, String)> {
    let all: Vec<(&str, &str)> =
        res.loaded().flat_map(|p| p.remotes.iter().filter(|s| s.may_login()).map(move |s| (p.name.as_str(), s.id.as_str()))).collect();
    all.iter()
        .map(|(p, s)| {
            let twice = all.iter().filter(|(_, o)| o == s).count() > 1;
            (p.to_string(), s.to_string(), if twice { format!("{p}/{s}") } else { s.to_string() })
        })
        .collect()
}

fn plugin_row(i: &Inputs, p: &resolve::Plugin, names: &[(String, String, String)]) -> Plugin {
    let id = PluginId::plugin(&p.name);
    let on = p.state == resolve::State::Loaded;
    let mcp = p.description.as_deref().is_some_and(crate::import::is_bise_mcp);
    let mut parts = Vec::new();
    let bare = |s: &resolve::Skill| s.name.strip_prefix(&format!("{}:", p.namespace)).unwrap_or(&s.name).to_string();
    for (s, s_on) in p.skills.iter().map(|s| (s, true)).chain(p.off_skills.iter().map(|s| (s, false))) {
        let pid = id.part(IdPart::Skill, None, bare(s));
        parts.push(PluginPart { on: on && s_on && i.state.is_on(&pid, false), path: Some(s.path.to_string_lossy().into_owned()), ..part(&pid, PartKind::Skill, bare(s), Some(&s.description)) });
    }
    let login_name = |server: &str| names.iter().find(|(pl, s, _)| *pl == p.name && s == server).map(|(_, _, n)| n.clone());
    for s in &p.servers {
        parts.push(stdio_part(i, &id, p, s, on));
    }
    for s in &p.remotes {
        parts.push(remote_part(i, &id, p, s, on, login_name(&s.id)));
    }
    // VIEWS (MCP Apps, docs/mcp-apps.md C8): its views and his grants
    parts.extend(views::parts(&p.name, &p.views, i.grants, on));
    for off in &p.off_servers {
        if let PluginId::Part { name, .. } = off {
            parts.push(PluginPart { on: false, ..part(off, PartKind::Mcp, name.clone(), None) });
        }
    }
    let what = if mcp {
        p.servers.first().map(run_line).or_else(|| p.remotes.first().map(HttpServer::host))
    } else {
        p.description.clone().filter(|d| !d.trim().is_empty())
    };
    let needs = parts.iter().any(|x| x.login == Some(LoginState::Needs));
    Plugin {
        id: id.to_string(),
        name: p.name.clone(),
        kind: if mcp { PluginKind::Mcp } else { PluginKind::Plugin },
        scope: scope_of(p.scope),
        title: p.name.clone(),
        what,
        icon: None,
        version: p.version.clone(),
        path: Some(p.root.to_string_lossy().into_owned()),
        state: state_of(p.state),
        on,
        locked: false,
        action: (on && needs).then_some(PluginAction::LogIn),
        login: None,
        prompts: prompts(&p.prompts),
        settings: settings_of(p, i.secrets_dir),
        logins: Vec::new(),
        parts,
    }
}

/// `p`'s settings as rows: each `set` when the environment or his saved
/// secrets ([`crate::secrets::lookup`], the bridge's one rule) give its
/// var a non-empty value. The value itself never leaves here.
pub fn settings_of(p: &resolve::Plugin, secrets_dir: Option<&Path>) -> Vec<PluginSetting> {
    if p.settings.is_empty() {
        return Vec::new();
    }
    let vars = crate::secrets::lookup(secrets_dir, &PluginId::plugin(&p.name));
    p.settings
        .iter()
        .map(|s| PluginSetting {
            var: s.var.clone(),
            label: s.label.clone(),
            secret: s.secret,
            required: s.required,
            help: s.help.clone(),
            placeholder: s.placeholder.clone(),
            set: vars(&s.var).is_some_and(|v| !v.is_empty()),
        })
        .collect()
}

/// A part with nothing known yet but its id, kind, name and words.
fn part(id: &PluginId, kind: PartKind, name: String, what: Option<&str>) -> PluginPart {
    PluginPart {
        id: id.to_string(),
        kind,
        name,
        what: what.map(str::trim).filter(|d| !d.is_empty()).map(String::from),
        on: true,
        status: None,
        login: None,
        error: None,
        tools: Vec::new(),
        tool_count: None,
        path: None,
        view: None,
    }
}

/// How a stdio server runs, without a secret: its command's name and its
/// first plain argument (`npx @playwright/mcp`); a flag, an address, a
/// `k=v`, a path, a script (a space in it) or a `user:secret` is left out.
pub fn run_line(s: &StdioServer) -> String {
    let cmd = Path::new(&s.command).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let plain = |a: &&String| {
        !a.is_empty() && !a.starts_with(['-', '/', '.', '~']) && !a.contains(['=', ':', ' ', '\t', '\n'])
    };
    match s.args.iter().find(plain) {
        Some(a) => format!("{cmd} {a}"),
        None => cmd,
    }
}

/// An MCP server's tools from its last status, each on or off.
fn tools_of(owner: &PluginId, server: &str, last: Option<&status::Status>, server_on: bool, st: &StateV2) -> Vec<PluginPart> {
    let Some(last) = last else { return Vec::new() };
    last.listed
        .iter()
        .map(|(name, desc)| {
            let id = owner.part(IdPart::Tool, Some(server), name.clone());
            PluginPart { on: server_on && st.is_on(&id, false), ..part(&id, PartKind::Tool, name.clone(), Some(desc)) }
        })
        .collect()
}

/// What its last status says: the words, the tool count, the error.
fn said(last: Option<&status::Status>) -> (Option<String>, Option<u32>, Option<String>) {
    match last {
        None => (None, None, None),
        Some(s) if s.login => (Some(settings::plugins_part_needs_login.into()), None, None),
        Some(s) => match &s.tools {
            // listed, not running: a state, never an error
            Ok(n) if s.idle => (Some(settings::plugins_listed(*n)), Some(*n as u32), None),
            Ok(n) => (Some(settings::plugins_connected(*n)), Some(*n as u32), None),
            Err(e) => (Some(settings::plugins_failed(e)), None, Some(e.clone())),
        },
    }
}

fn stdio_part(i: &Inputs, owner: &PluginId, p: &resolve::Plugin, s: &StdioServer, plugin_on: bool) -> PluginPart {
    let id = owner.part(IdPart::Mcp, None, s.id.clone());
    let on = plugin_on && i.state.is_on(&id, false);
    let last = i.status_dir.and_then(|d| status::read(d, &p.name, &s.id));
    let (status, tool_count, error) = said(last.as_ref());
    PluginPart {
        on,
        status,
        error,
        tool_count,
        tools: tools_of(owner, &s.id, last.as_ref(), on, i.state),
        ..part(&id, PartKind::Mcp, s.id.clone(), Some(&run_line(s)))
    }
}

fn remote_part(i: &Inputs, owner: &PluginId, p: &resolve::Plugin, s: &HttpServer, plugin_on: bool, login_name: Option<String>) -> PluginPart {
    let id = owner.part(IdPart::Mcp, None, s.id.clone());
    let on = plugin_on && i.state.is_on(&id, false);
    let last = i.status_dir.and_then(|d| status::read(d, &p.name, &s.id));
    let (mut status, tool_count, error) = said(last.as_ref());
    let login = login_name.as_ref().map(|name| {
        if i.pending_logins.contains(name) {
            LoginState::Pending
        } else if last.as_ref().is_some_and(|l| l.login) {
            LoginState::Needs
        } else if i.oauth_dir.is_some_and(|d| logged_in(p, s, d)) {
            LoginState::In
        } else {
            LoginState::Out
        }
    });
    status = match login {
        Some(LoginState::Pending) => Some(settings::plugins_login_pending.into()),
        Some(LoginState::In) => Some(tool_count.map_or_else(|| settings::plugins_logged_in.to_string(), |n| settings::plugins_logged_in_tools(n as usize))),
        Some(LoginState::Out) if tool_count.is_none() && error.is_none() => Some(settings::plugins_not_logged_in.into()),
        _ => status,
    };
    PluginPart {
        on,
        status,
        login,
        error: error.filter(|_| login != Some(LoginState::Needs)),
        tool_count,
        tools: tools_of(owner, &s.id, last.as_ref(), on, i.state),
        ..part(&id, PartKind::Mcp, login_name.unwrap_or_else(|| s.id.clone()), Some(&s.host()))
    }
}

fn logged_in(p: &resolve::Plugin, s: &HttpServer, oauth_dir: &Path) -> bool {
    let env_dir = Some(bise_home::Home::from_env().mcp_secrets_dir());
    let t = crate::login::Target { plugin: p.name.clone(), server: s.clone(), name: s.id.clone(), env_dir };
    crate::login::logged_in(&t, oauth_dir)
}

fn connector_scope(c: &ConnectorEntry) -> PluginScope {
    if connectors::BUILTINS.contains(&c.name.as_str()) {
        PluginScope::BuiltIn
    } else {
        PluginScope::Mistral
    }
}

/// A connector that is ready: a plugin whose parts are its tools.
fn connector_row(st: &StateV2, c: &ConnectorEntry) -> Plugin {
    let id = PluginId::mistral(&c.name);
    let on = st.is_on(&id, false);
    let parts = c
        .tools
        .iter()
        .map(|t| {
            let pid = id.part(IdPart::Tool, None, t.name.clone());
            PluginPart { on: on && st.is_on(&pid, false), ..part(&pid, PartKind::Tool, t.name.clone(), Some(&t.description)) }
        })
        .collect();
    Plugin {
        on,
        state: if on { PluginState::Loaded } else { PluginState::Disabled },
        login: Some(LoginState::In),
        parts,
        ..connector_base(c, id)
    }
}

/// A connector not connected yet: a Browse item with its button (none
/// when bise can't connect it, nor while its connect runs: `pending`).
fn browse_row(c: &ConnectorEntry, pending: &[String]) -> Plugin {
    let id = PluginId::mistral(&c.name);
    if pending.contains(&id.to_string()) {
        return Plugin { state: PluginState::Available, login: Some(LoginState::Pending), ..connector_base(c, id) };
    }
    // Unknown: no auth_action bise knows, not connectable from bise (no button)
    let action = match c.auth {
        ConnectorAuth::CredentialsSetup => Some(PluginAction::SetUp),
        ConnectorAuth::OAuth | ConnectorAuth::NoAuthConnect => Some(PluginAction::Connect),
        ConnectorAuth::Ready | ConnectorAuth::Unknown => None,
    };
    Plugin { state: PluginState::Available, action, login: Some(LoginState::Out), ..connector_base(c, id) }
}

/// Why his connectors may be missing or old, for the `plugins` and
/// `browse {mistral}` events (the window words it from the kind).
pub fn problem(catalog: &connectors::Catalog) -> Option<ConnectorsProblem> {
    catalog.error.as_ref().map(|p| match p {
        connectors::Problem::NoKey => ConnectorsProblem::NoKey,
        connectors::Problem::Locked => ConnectorsProblem::Locked,
        connectors::Problem::Fetch(why) => ConnectorsProblem::Fetch { why: why.clone() },
    })
}

fn connector_base(c: &ConnectorEntry, id: PluginId) -> Plugin {
    Plugin {
        id: id.to_string(),
        name: id.to_string(),
        kind: PluginKind::Mistral,
        scope: connector_scope(c),
        title: if c.display_name.trim().is_empty() { display_name(&c.name) } else { c.display_name.clone() },
        what: None,
        icon: None,
        version: None,
        path: None,
        state: PluginState::Available,
        on: false,
        locked: false,
        action: None,
        login: None,
        parts: Vec::new(),
        prompts: Vec::new(),
        settings: Vec::new(),
        logins: Vec::new(),
    }
}

/// The folder a loose skill's `SKILL.md` sits in, two levels up.
fn folder_of(s: &SkillEntry) -> Option<PathBuf> {
    s.path.parent()?.parent().map(Path::to_path_buf)
}

/// A skills folder as one plugin (none when it has no skill).
fn skills_row(i: &Inputs, scope: SkillsScope) -> Option<Plugin> {
    let id = PluginId::Skills { scope };
    let mine: Vec<&SkillEntry> = i.skills.iter().filter(|s| s.owner == id).collect();
    let first = mine.first()?;
    let folder = folder_of(first);
    // <project>/.agents/skills: the project is two folders up
    let project = || {
        folder.as_deref().and_then(|f| f.parent()?.parent()?.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    };
    let (title, what, plugin_scope) = match scope {
        SkillsScope::Personal => (settings::plugins_personal_title.to_string(), settings::plugins_personal_where.to_string(), PluginScope::User),
        SkillsScope::Vibe => (settings::plugins_vibe_title.to_string(), settings::plugins_vibe_where.to_string(), PluginScope::User),
        SkillsScope::Workspace => (settings::plugins_project_title(project()), settings::plugins_project_where(project()), PluginScope::Workspace),
        SkillsScope::Bise => (settings::plugins_bise_title.to_string(), settings::plugins_bise_what.to_string(), PluginScope::BuiltIn),
    };
    let locked = StateV2::locked(&id);
    let on = i.state.is_on(&id, false);
    let parts = mine
        .iter()
        .map(|s| {
            let pid = id.part(IdPart::Skill, None, s.name.clone());
            PluginPart { on: on && i.state.is_on(&pid, false), path: Some(s.path.to_string_lossy().into_owned()), ..part(&pid, PartKind::Skill, s.name.clone(), Some(&s.description)) }
        })
        .collect();
    Some(Plugin {
        id: id.to_string(),
        name: id.to_string(),
        kind: PluginKind::Skills,
        scope: plugin_scope,
        title,
        what: Some(what),
        icon: None,
        version: None,
        path: folder.map(|f| f.to_string_lossy().into_owned()),
        state: if on { PluginState::Loaded } else { PluginState::Disabled },
        on,
        locked,
        action: None,
        login: None,
        parts,
        prompts: Vec::new(),
        settings: Vec::new(),
        logins: Vec::new(),
    })
}

#[cfg(test)]
#[path = "catalog/tests.rs"]
mod tests;
