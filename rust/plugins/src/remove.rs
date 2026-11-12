//! Remove a plugin he added (`/plugins remove`, `bise plugins remove`,
//! the desktop's `plugin_remove`): only a plugin of his plugins folder
//! (`<user root>/<name>`, what the add menu installs). A built-in, a
//! repo's own plugin, a Mistral connector (disconnected on Mistral's
//! side), a skills folder or a part is refused, typed ([`Refused`]).
//!
//! Order, so a half-removed plugin never resolves and a failure leaves
//! the whole plugin or nothing of it:
//!
//! 1. its folder renamed into the add's staging folder
//!    ([`add::staging_dir`]: a dot folder is never a plugin; a crash
//!    after this step is swept with the stale previews);
//! 2. its remote servers' logins, through the logout path
//!    ([`login::logout`]: revoked first when the server can; kept when
//!    another plugin calls the same server), while his saved `${VAR}`s
//!    still expand their URLs;
//! 3. its secrets forgotten ([`secrets::remove`]); a failure renames the
//!    folder back (only its logins are gone: he logs in again);
//! 4. then, best effort: its ids out of `plugins.json`
//!    ([`state::forget`]), its servers' statuses ([`status::forget`]),
//!    and the folder deleted.
//!
//! Live sessions need nothing more: each bridge's next poll
//! ([`crate::bridge::POLL`]) sees the plugin gone and retires its
//! servers (a call gets [`crate::bridge::off_error`]'s words), no
//! `/reload`.

use std::path::{Path, PathBuf};

use crate::add::{self, Ctx};
use crate::id::PluginId;
use crate::login::{self, Target};
use crate::resolve::{self, Plugin, Roots, Scope};
use crate::{oauth, remote, secrets, state, status};

/// Why a remove can't go on. The apps pick their words from the kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoveError {
    /// not a plugin of his plugins folder
    Refused { id: String, why: Refused },
    /// no plugin of that name anywhere
    NotFound { name: String },
    /// the secrets could not be forgotten: the plugin is back in place
    Secrets { why: String },
    Io { why: String },
}

/// What a refused id is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// bise's own (the app root's `plugins/`)
    BuiltIn,
    /// the workspace's own (`<repo>/.agents/plugins`): the repo's to remove
    Workspace,
    /// a Mistral connector: disconnected on Mistral's side
    Connector,
    /// a skills folder
    Skills,
    /// a part of a plugin: turned off, not removed
    Part,
}

impl std::fmt::Display for RemoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoveError::Refused { id, why } => match why {
                Refused::BuiltIn => write!(f, "{id} comes with bise: turn it off instead"),
                Refused::Workspace => write!(f, "{id} is the repo's own plugin: remove it from the repo"),
                Refused::Connector => write!(f, "{id} is a Mistral connector: disconnect it on Mistral's side, or turn it off"),
                Refused::Skills => write!(f, "{id} is a skills folder: turn it off instead"),
                Refused::Part => write!(f, "{id} is a part of a plugin: turn it off instead"),
            },
            RemoveError::NotFound { name } => write!(f, "no plugin named {name} in your plugins"),
            RemoveError::Secrets { why } => write!(f, "couldn't forget its saved secrets ({why}); nothing was removed"),
            RemoveError::Io { why } => write!(f, "{why}"),
        }
    }
}

/// Remove `id` (a plugin of his plugins folder) and everything bise
/// keeps for it. `workspace`: the repo whose own plugins are refused by
/// name (none: bise's home).
pub fn remove(ctx: &Ctx, workspace: Option<&Path>, id: &PluginId) -> Result<(), RemoveError> {
    let refused = |why| Err(RemoveError::Refused { id: id.to_string(), why });
    let name = match id {
        PluginId::Plugin { name } => name.clone(),
        PluginId::Mistral { .. } => return refused(Refused::Connector),
        PluginId::Skills { .. } => return refused(Refused::Skills),
        PluginId::Part { .. } => return refused(Refused::Part),
    };
    let Some(p) = his(ctx, &name) else {
        return match elsewhere(ctx, workspace, &name) {
            Some(Scope::BuiltIn) => refused(Refused::BuiltIn),
            Some(Scope::Workspace) => refused(Refused::Workspace),
            _ => Err(RemoveError::NotFound { name }),
        };
    };
    let logins = logins_to_drop(ctx, workspace, &p);
    // 1. out of the roots at once
    let gone = add::staging_dir(&ctx.user_root).join(format!("rm-{}", add::token())).join(&name);
    let io = |e: std::io::Error| RemoveError::Io { why: e.to_string() };
    std::fs::create_dir_all(gone.parent().unwrap_or(&ctx.user_root)).map_err(io)?;
    std::fs::rename(&p.root, &gone).map_err(io)?;
    // 2. its logins, while its saved ${VAR}s still expand their URLs
    for t in &logins {
        let _ = login::logout(t, &ctx.oauth_dir, None);
    }
    // 3. its secrets, or the plugin back
    if let Err(e) = secrets::remove(&ctx.secrets_dir, id) {
        let back = std::fs::rename(&gone, &p.root);
        drop_temp(&gone);
        return Err(match back {
            Ok(()) => RemoveError::Secrets { why: e.to_string() },
            Err(b) => RemoveError::Io { why: format!("{e}; and the folder could not be put back: {b}") },
        });
    }
    // 4. the rest, best effort
    let _ = state::forget(&ctx.state, id);
    status::forget(&ctx.status_dir, &p.name);
    drop_temp(&gone);
    Ok(())
}

/// The plugin `name` of his plugins folder (an invalid one too: a broken
/// plugin is removable).
fn his(ctx: &Ctx, name: &str) -> Option<Plugin> {
    let roots = Roots { builtin: None, user: Some(ctx.user_root.clone()), workspace: None, data: ctx.data.clone(), documents: ctx.documents.clone(), state: Default::default() };
    resolve::resolve(&roots).plugins.into_iter().find(|p| p.name == name && p.scope == Scope::User && p.root.starts_with(&ctx.user_root))
}

/// The workspace's plugins root, as [`Roots::standard`] makes it.
fn workspace_root(workspace: Option<&Path>) -> Option<PathBuf> {
    workspace.map(|w| w.join(".agents").join("plugins"))
}

/// Where else a plugin `name` is (built-in, the workspace's).
fn elsewhere(ctx: &Ctx, workspace: Option<&Path>, name: &str) -> Option<Scope> {
    let roots = Roots { builtin: ctx.builtin.clone(), user: None, workspace: workspace_root(workspace), data: ctx.data.clone(), documents: ctx.documents.clone(), state: Default::default() };
    resolve::resolve(&roots).plugins.into_iter().find(|p| p.name == name).map(|p| p.scope)
}

/// `p`'s remote servers whose login goes with it: one that may log in,
/// and that no other plugin calls (the tokens are the server's, shared).
/// Its URL's `${VAR}`s expand by [`secrets::lookup`]'s rule.
fn logins_to_drop(ctx: &Ctx, workspace: Option<&Path>, p: &Plugin) -> Vec<Target> {
    let target = |o: &Plugin, s: &resolve::HttpServer| Target { plugin: o.name.clone(), server: s.clone(), name: s.id.clone(), env_dir: Some(ctx.secrets_dir.clone()) };
    let resource = |t: &Target| {
        let env = secrets::lookup(t.env_dir.as_deref(), &PluginId::plugin(t.plugin.as_str()));
        remote::target(&t.server, &env).ok().map(|(u, _)| oauth::resource_of(&u))
    };
    let roots = Roots { builtin: ctx.builtin.clone(), user: Some(ctx.user_root.clone()), workspace: workspace_root(workspace), data: ctx.data.clone(), documents: ctx.documents.clone(), state: Default::default() };
    let others: Vec<String> = resolve::resolve(&roots)
        .plugins
        .iter()
        .filter(|o| o.root != p.root)
        .flat_map(|o| o.remotes.iter().filter_map(|s| resource(&target(o, s))).collect::<Vec<_>>())
        .collect();
    p.remotes
        .iter()
        .filter(|s| s.may_login())
        .map(|s| target(p, s))
        .filter(|t| resource(t).is_some_and(|r| !others.contains(&r)))
        .collect()
}

/// The renamed folder deleted, with its `rm-<token>` parent.
fn drop_temp(gone: &Path) {
    let parent: Option<PathBuf> = gone.parent().map(Path::to_path_buf);
    let _ = std::fs::remove_dir_all(gone);
    if let Some(d) = parent {
        let _ = std::fs::remove_dir(d);
    }
}

#[cfg(test)]
#[path = "remove_tests.rs"]
mod tests;
