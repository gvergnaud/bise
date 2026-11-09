//! The window's setup commands on the real core (bise desktop S11, core
//! side; plan reviewed by architect m_9130): his preferences, his model
//! accounts, the repos found in a folder, his projects list. This file has
//! the port the core calls (all I/O: [`SetupPorts`]), its live filling
//! ([`live`]) and the pure parts; core/setup.rs is the glue.
//!
//! The logic stays with its owners: `bise_home::prefs` (set_dotted, the
//! window's keys, excluded_apps), `bise_home::projects` (the registry's
//! only writer), `bise_catalog::auth_cli` (statuses, keys, the sign-in
//! flow, run as `bise auth login <id>`), devflow's `[flow] mode` (the
//! binary hands its writer in: bend-tui doesn't link switchboard).
//! A key never goes out in an event or a log line ([`redact`]).

use bise_home::projects::Row;
use bise_proto::app_rows::{Account, Found, Plugin, RoleRow};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::UNIX_EPOCH;

/// How deep `found_scan` looks under its folder.
pub const DEPTH: usize = 2;
/// The most repos one `found` lists.
pub const MAX_FOUND: usize = 50;

/// A job that ends later, back on the core's tick.
#[derive(Debug, PartialEq)]
pub enum Done {
    /// a sign-in opened its link (V14: `auth login --events`' open)
    Signing { id: String, url: String },
    /// a folder walked: its repos
    Scanned { dir: PathBuf, entries: Vec<Entry> },
    /// his sign-in ended (Err: its last line)
    SignedIn { id: String, res: Result<(), String> },
    /// a plugin server's login ended (P.1; Err: its reason)
    LoggedIn { name: String, project: Option<String>, res: Result<(), String> },
    /// computer use's setup check answered (P.2, once a second while its
    /// page is open)
    CuCheck(Value),
    /// a computer-use fix went wrong later (its line)
    CuSaid(String),
    /// the desktop app's release channel read (bar S.6,
    /// core/app_update.rs): its `desktop.json` (Err: why not), the
    /// channel's base URL, this Mac's target
    Manifest { text: Result<String, String>, base: String, target: String },
    /// a `provider_use` check ended (its seq: a newer one for the same
    /// provider drops it)
    Used { seq: u64, used: crate::firstkey::Used },
    /// N1: a /keychain move ended: its lines (item names and counts,
    /// never a secret) and whether it failed
    Keychain { lines: Vec<String>, failed: bool },
    /// a plugin to add built in the staging folder (a git clone may have
    /// run), its preview next (Err: why not)
    Staged { res: Result<StagedPlugin, String> },
    /// his projects read for what they name (the band's whys)
    Signals { inputs: Vec<bend_plugins::signals::Inputs> },
    /// a marketplace added and read: its plugins as Browse rows
    Market { source: String, res: Result<Vec<Plugin>, String> },
    /// a Mistral connector's connect said a step (`said`) or ended
    /// (`res`); the list is sent again either way
    Connecting { id: String, project: Option<String>, said: Option<String>, res: Option<Result<(), String>> },
    /// the Mistral catalog fetched again (Err: its problem)
    Refreshed { project: Option<String>, res: Result<(), String> },
    /// a change to one of his accounts ended (`cmd`: the command's name,
    /// for its error); the list is sent again either way
    AccountChanged { cmd: &'static str, project: Option<String>, res: Result<(), String> },
    /// the servers of a plugin he just added were tried once: their
    /// states are in the status files, the list is sent again
    Tried,
    /// settings · Setup's live test of a model ended, on the generation
    /// of its provider's key it ran with (Err: why, the provider's words)
    Tested { model: String, generation: u64, res: Result<(), (bise_proto::app_rows::CheckWhy, String)>, ms: u64 },
}

/// A staged plugin on its way back to the core (compared by its token).
#[derive(Debug)]
pub struct StagedPlugin(pub bend_plugins::add::Staged);

impl PartialEq for StagedPlugin {
    fn eq(&self, other: &Self) -> bool {
        self.0.token() == other.0.token()
    }
}

/// A repo the walk found.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub last_ms: u64,
}

/// A change to the projects list (`key`: the project's folder).
#[derive(Clone, Debug, PartialEq)]
pub enum RegistryOp {
    Remove(PathBuf),
    Move(PathBuf, usize),
    Rename(PathBuf, String),
}

/// Writes the repo's flow mode (`true`: trunk, `false`: pr).
pub type FlowFn = Box<dyn Fn(&Path, bool) -> Result<(), String>>;
pub type WriteFn = Box<dyn Fn(&Value) -> Result<(), String>>;
pub type IdFn = Box<dyn Fn(&str) -> Result<(), String>>;
/// (the model and its role, its provider's generation, the channel of its end)
pub type ModelTestFn = Box<dyn Fn(super::model_test::ModelTest, u64, Sender<Done>)>;
/// (provider, the key pasted or none, its seq, the channel of its end) ->
/// (the provider's name, the model it is checked with), at once; the
/// check's end comes as `Done::Used`
pub type ProviderUseFn = Box<dyn Fn(&str, Option<String>, Option<String>, u64, Sender<Done>) -> (String, String)>;
pub type SignInFn = Box<dyn Fn(&str, Sender<Done>) -> Result<(), String>>;
pub type PathFn = Box<dyn Fn(&Path) -> Result<(), String>>;
/// Reads the desktop app's release channel he follows (stable or beta) on
/// its own thread, `Done::Manifest` at its end.
pub type ManifestFn = Box<dyn Fn(bise_home::release::DesktopChannel, Sender<Done>)>;
/// A workspace's plugins (`pending`: the logins whose browser is open).
pub type PluginsFn = Box<dyn Fn(&Path, &[String]) -> Vec<Plugin>>;
pub type PluginSetFn = Box<dyn Fn(&str, bool) -> Result<(), String>>;
/// a view's grant: plugin, uri, permissions (docs/mcp-apps.md C8)
pub type PluginGrantFn = Box<dyn Fn(&str, &str, &[String]) -> Result<(), String>>;
pub type PluginSettingFn = Box<dyn Fn(&Path, &str, &str, Option<&str>) -> Result<(), String>>;
/// (workspace, server name, the project it was asked for, the channel of its end)
pub type PluginLoginFn = Box<dyn Fn(&Path, &str, Option<String>, Sender<Done>) -> Result<(), String>>;
pub type PluginLogoutFn = Box<dyn Fn(&Path, &str) -> Result<(), String>>;
/// Builds what he adds in the staging folder on its own thread (a git
/// clone), `Done::Staged` at its end; nothing of the plugin runs
pub type PluginStageFn = Box<dyn Fn(bend_plugins::add::Request, Sender<Done>)>;
/// Adds a marketplace to his list and reads it on its own thread (a git
/// clone), `Done::Market` at its end
pub type MarketFn = Box<dyn Fn(String, Sender<Done>)>;
/// (each project's folder and name, the channel): what they name
/// (bend_plugins::signals::gather, names only) on its own thread,
/// `Done::Signals` at its end
pub type SignalsFn = Box<dyn Fn(Vec<(PathBuf, String)>, Sender<Done>)>;
/// (the plugin he just added, where adds read and write, the channel):
/// its MCP servers tried once on their own thread
/// (bend_plugins::add::try_servers), `Done::Tried` at its end
pub type PluginTryFn = Box<dyn Fn(PathBuf, bend_plugins::add::Ctx, Sender<Done>)>;
/// (connector id, the project it was asked for, the channel): its connect
/// on its own thread (his browser, then a poll up to 5 min),
/// `Done::Connecting` at each step and at its end (never run by a test)
pub type PluginConnectFn = Box<dyn Fn(&str, Option<String>, Sender<Done>) -> Result<(), String>>;
/// (connector id, the account's connection name, the project, the
/// channel): that account's login on its own thread (his browser, then a
/// poll up to 5 min), `Done::Connecting` at each step and at its end
/// (never run by a test)
pub type PluginAccountConnectFn = Box<dyn Fn(&str, &str, Option<String>, Sender<Done>) -> Result<(), String>>;
/// (connector id, the account's connection name, the change, the
/// project, the channel): the change on Mistral then the catalog read
/// again, on its own thread, `Done::AccountChanged` at its end
pub type PluginAccountChangeFn = Box<dyn Fn(&str, &str, bend_plugins::connectors::AccountChange, Option<String>, Sender<Done>)>;
/// (the project it was asked for, the channel): the catalog fetched on
/// its own thread, `Done::Refreshed` at its end
pub type PluginsRefreshFn = Box<dyn Fn(Option<String>, Sender<Done>)>;
/// (role, model, effort): config.toml's `[roles]` through the one locked writer
pub type RoleSetFn = Box<dyn Fn(&str, Option<&str>, Option<&str>) -> Result<(), String>>;
/// (roles, model): one model for those roles, one write
pub type RoleSetManyFn = Box<dyn Fn(&[String], &str) -> Result<(), String>>;
/// What runs now from computer use's page (the live test, a helper that reopens).
pub type CuBusy = std::sync::Arc<std::sync::Mutex<crate::computer_use::Busy>>;
/// The poll's start: its channel and its stop flag.
pub type CuPollFn = Box<dyn Fn(Sender<Done>, std::sync::Arc<std::sync::atomic::AtomicBool>)>;
/// (the last setup check, the fix, what runs, the channel of a later failure) -> its flash
pub type CuFixFn = Box<dyn Fn(&Value, &str, CuBusy, Sender<Done>) -> Option<String>>;
/// (default | forget, the browser profile) -> done, or why not
pub type CuProfileFn = Box<dyn Fn(&str, &bise_proto::app_rows::CuProfileKey) -> Result<(), String>>;

/// Computer use's I/O (P.2): the TUI's /computer-use functions
/// (crate::computer_use), a fake in tests.
pub struct CuPorts {
    /// the built-in `computer` plugin is on
    pub is_on: Box<dyn Fn() -> bool>,
    pub set_on: Box<dyn Fn(bool)>,
    /// off (true: uninstall too): the line to show
    pub off: Box<dyn Fn(bool) -> String>,
    /// one setup check a second on a thread until the flag, each as `Done::CuCheck`
    pub poll: CuPollFn,
    /// a row's fix (his click only, never a test's real one): its flash;
    /// a later failure as `Done::CuSaid`
    pub fix: CuFixFn,
    /// the live test, once at a time
    pub live_test: Box<dyn Fn(&CuBusy)>,
    /// a browser profile made the default or forgotten (`default` |
    /// `forget`, his click only): the broker's (`bise computer-use
    /// default`); the next check shows it
    pub profile: CuProfileFn,
}

/// Everything the setup commands read or write.
pub struct SetupPorts {
    /// `prefs.json`'s object (None: no file)
    pub prefs: Box<dyn Fn() -> Option<Value>>,
    pub write_prefs: WriteFn,
    pub accounts: Box<dyn Fn() -> Vec<Account>>,
    /// Claude Code is signed in with a Claude plan (bise_catalog::detect,
    /// presence only): the accounts event says so
    pub claude_plan: Box<dyn Fn() -> bool>,
    pub provider_use: ProviderUseFn,
    pub key_remove: IdFn,
    /// only a provider that signs in; its browser flow, `Done::SignedIn`
    /// at its end (never run by a test)
    pub sign_in: SignInFn,
    /// ends the open sign-in of that account (V14): its process group,
    /// so its local callback listener goes too; Err when none is open
    pub sign_in_cancel: IdFn,
    /// walks a folder on its own thread, `Done::Scanned` at its end
    pub scan: Box<dyn Fn(PathBuf, Sender<Done>)>,
    /// his home folder (`found_scan` without a folder)
    pub home_dir: PathBuf,
    /// the registry's rows, home first
    pub rows: Box<dyn Fn() -> Vec<Row>>,
    pub add: PathFn,
    pub registry: Box<dyn Fn(RegistryOp) -> Result<(), String>>,
    pub flow: FlowFn,
    /// a workspace's agent plugins (`pending`: logins whose browser is open)
    pub plugins: PluginsFn,
    /// plugins.json through bend_plugins' one writer
    pub plugin_set: PluginSetFn,
    /// a view's grant in plugins.json, through bend_plugins' one writer
    pub plugin_grant: PluginGrantFn,
    /// a plugin's setting in his MCP secrets, through bend_plugins' one
    /// writer: (workspace, plugin, var, value or none to forget)
    pub plugin_setting: PluginSettingFn,
    /// a plugin server's browser login, `Done::LoggedIn` at its end (never
    /// run by a test)
    pub plugin_login: PluginLoginFn,
    pub plugin_logout: PluginLogoutFn,
    /// the add menu (bend_plugins::add): a plugin staged, then held by
    /// the core until his click (`plugin_preview`)
    pub plugin_stage: PluginStageFn,
    pub plugin_market: MarketFn,
    /// what his projects name, for the band's whys (never on a send path)
    pub signals: SignalsFn,
    /// where an add reads and writes (`add::Ctx::standard`, a temp one in
    /// tests): what a confirmed preview installs into
    pub add_ctx: Box<dyn Fn() -> bend_plugins::add::Ctx>,
    /// right after his confirm installed a plugin (never before)
    pub plugin_try: PluginTryFn,
    pub plugin_connect: PluginConnectFn,
    pub plugin_account_connect: PluginAccountConnectFn,
    pub plugin_account_change: PluginAccountChangeFn,
    pub plugins_refresh: PluginsRefreshFn,
    /// why his Mistral connectors may be missing or old (their cache's)
    pub connectors_problem: Box<dyn Fn() -> Option<bise_proto::app_rows::ConnectorsProblem>>,
    pub cu: CuPorts,
    /// each role's model (V17, [`role_rows`] of config.toml's setup)
    pub roles: Box<dyn Fn() -> Vec<RoleRow>>,
    /// what settings · Setup's gauge reads (bise_catalog::features): the
    /// setup with the keys, and voice mode's `tts_model` when he set one
    pub gauge_input: Box<dyn Fn() -> (bise_catalog::Setup, Option<String>)>,
    /// one live test of a model (his click only), on its own thread,
    /// `Done::Tested` at its end with the generation given
    pub model_test: ModelTestFn,
    /// checks.json's text, each model's last test from any screen
    /// (bise_catalog::checks; "" when none)
    pub checks: Box<dyn Fn() -> String>,
    /// a test's result into checks.json, so the terminal's /models says it too
    pub record_check: Box<dyn Fn(&bise_catalog::features::Test)>,
    /// a role's model and effort, checked ([`check_role_set`]) then
    /// written by bise_catalog's roles::save_role
    pub role_set: RoleSetFn,
    /// one model for several roles (the news line): each checked, then
    /// one write by roles::save_roles, all of them or none
    pub role_set_many: RoleSetManyFn,
    /// the desktop app's release channel (stable or beta), for its updates
    /// (bar S.6): the binary wires its fetch; none here (a core without
    /// it never offers an update)
    pub manifest: ManifestFn,
    /// this core's own build time (its app root's VERSION `built=`, found
    /// the way bise finds its app root everywhere: the binary wires
    /// approot::locate); empty: a dev build, none here
    pub own_built: Box<dyn Fn() -> String>,
    /// N1 (the TUI's /keychain): where the secrets are and the move
    pub keychain: KeychainPort,
    /// his usage counts' acts and view (step 6): bise_usage::control
    /// with this core's build (the binary wires its app root's; a dev
    /// build's here)
    pub telemetry: super::telemetry::TelemetryFn,
}

/// N1: his keys and sign-ins, in the macOS keychain or in files. `now`
/// says where (`keychain` or `file`); `switch(to_keychain, tx)` moves
/// them on its own thread and sends `Done::Keychain` at its end. The live
/// one is bise_catalog's secrets_cli::switch (the TUI's /keychain); a
/// test's is a fake (never /usr/bin/security).
pub struct KeychainPort {
    pub now: Box<dyn Fn() -> String>,
    pub switch: Box<dyn Fn(bool, Sender<Done>)>,
}

/// The keychain port bise uses: bise_catalog's secrets_cli, as the TUI's
/// keychain.rs (auth.json and the MCP logins; off macOS its line says so).
pub fn live_keychain() -> KeychainPort {
    // the TUI's own paths and setting (architect m_17309: one builder, crate::keychain's)
    use crate::keychain::paths;
    KeychainPort {
        now: Box::new(|| store_word(crate::keychain::now()).into()),
        switch: Box::new(|on, tx| {
            std::thread::spawn(move || {
                let to = if on { bise_secrets::Store::Keychain } else { bise_secrets::Store::File };
                // a panic in the move still ends it (amb-core m_17305): else the core stays
                // 'moving' and refuses every later /keychain until a restart
                let done = std::panic::catch_unwind(|| bise_catalog::secrets_cli::switch(&paths(), &bend_plugins::oauth::store_dir(), Some(to), cfg!(target_os = "macos")));
                let d = match done {
                    Ok(o) => Done::Keychain { lines: o.lines, failed: o.failed },
                    Err(why) => {
                        let why = why.downcast_ref::<&str>().map(|s| (*s).to_string()).or_else(|| why.downcast_ref::<String>().cloned()).unwrap_or_else(|| "an error".into());
                        Done::Keychain { lines: vec![format!("the move stopped: {why}")], failed: true }
                    }
                };
                let _ = tx.send(d);
            });
        }),
    }
}

/// The event's word for where the secrets are.
pub fn store_word(s: bise_secrets::Store) -> &'static str {
    match s {
        bise_secrets::Store::Keychain => "keychain",
        _ => "file",
    }
}

/// Each role's model and effort as the TUI's `/models` screen reads
/// them (V17): the role's words, its model, its effort, where they come
/// from (`config`, `env`, or `default`: bise picks, or it follows a role).
pub fn role_rows(setup: &bise_catalog::Setup) -> Vec<RoleRow> {
    use bise_catalog::roles::{Kind, Source, ROLES};
    ROLES
        .iter()
        .map(|r| {
            let (model, from) = setup.role_model(r.id);
            let (source, follows) = match from {
                Source::Picked => ("config", None),
                Source::Env(_) => ("env", None),
                Source::SameAs(to) => ("default", Some(to.to_string())),
                Source::Auto | Source::None => ("default", None),
            };
            RoleRow {
                role: r.id.into(),
                name: r.name.into(),
                about: r.about.into(),
                kind: if r.kind == Kind::Voice { "voice" } else { "chat" }.into(),
                model,
                effort: Some(setup.role_effort(r.id).to_string()).filter(|e| !e.is_empty()),
                source: source.into(),
                follows,
            }
        })
        .collect()
}

/// A `role_set` the TUI's roles screen would take: a known role, a model
/// the catalog knows (a listed or unlisted model of a known provider, or
/// an alias), an effort that model takes. Err: what's wrong, nothing written.
pub fn check_role_set(setup: &bise_catalog::Setup, role: &str, model: Option<&str>, effort: Option<&str>) -> Result<(), String> {
    use bise_catalog::roles::Kind;
    let r = bise_catalog::roles::role(role).ok_or_else(|| format!("there's no role {role}"))?;
    let c = &setup.catalog;
    let Some(m) = model.map(str::trim).filter(|m| !m.is_empty()) else {
        return match effort {
            Some(_) => Err("an effort needs a model".into()),
            None => Ok(()),
        };
    };
    let target = c.aliases.iter().find(|(a, _)| a == m).map(|(_, to)| to.clone()).unwrap_or_else(|| m.to_string());
    let res = c.resolve(&target);
    if res.known == bise_catalog::Known::NoProvider {
        return Err(format!("bise doesn't know the model {m}"));
    }
    if let Some(e) = effort.filter(|e| !e.is_empty()) {
        if r.kind == Kind::Voice {
            return Err(format!("the {} role takes no effort", r.name));
        }
        let efforts = res.efforts();
        if !efforts.iter().any(|x| x == e) {
            return Err(format!("{m} takes no effort {e}"));
        }
    }
    Ok(())
}

pub use crate::firstkey::{account_of, accounts};

/// The repos to show: newest first, at most [`MAX_FOUND`], each flagged
/// when already one of his projects.
pub fn rank(mut entries: Vec<Entry>, known: &[PathBuf]) -> Vec<Found> {
    entries.sort_by(|a, b| b.last_ms.cmp(&a.last_ms).then_with(|| a.name.cmp(&b.name)));
    entries.truncate(MAX_FOUND);
    entries
        .into_iter()
        .map(|e| Found {
            known: known.iter().any(|k| k == &e.path),
            path: e.path.to_string_lossy().into_owned(),
            name: e.name,
            git: true,
            last_ms: e.last_ms,
        })
        .collect()
}

/// The folders a walk never enters.
fn skipped(name: &str) -> bool {
    name.starts_with('.') || ["Library", "node_modules", "target", "Applications"].contains(&name)
}

/// The git repos under `dir`, `depth` levels down (a repo's own folders
/// are not entered; a worktree, whose `.git` is a file, is not a repo
/// here; `skip`: bise's own folders). No git process: the last commit's
/// time is `.git/logs/HEAD`'s (else `.git/HEAD`'s) modification time.
pub fn walk(dir: &Path, depth: usize, skip: &[PathBuf]) -> Vec<Entry> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if skipped(&name) || !e.file_type().is_ok_and(|t| t.is_dir()) || skip.iter().any(|s| s == &path) {
            continue;
        }
        let git = path.join(".git");
        if git.is_dir() {
            let t = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
            let at = t(git.join("logs/HEAD")).or_else(|| t(git.join("HEAD"))).or_else(|| t(path.clone()));
            let last_ms = at.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
            out.push(Entry { path: bise_home::projects::canonical(&path), name, last_ms });
        } else if depth > 1 {
            out.extend(walk(&path, depth - 1, skip));
        }
    }
    out
}

/// A stdin line fit for a log: a `provider_use`'s key is never written.
pub fn redact(line: &str) -> String {
    match serde_json::from_str::<Value>(line) {
        Ok(mut v) if v.get("cmd").and_then(Value::as_str) == Some("provider_use") && v.get("key").is_some() => {
            v["key"] = Value::from("…");
            v.to_string()
        }
        Ok(_) => line.to_string(),
        Err(_) if line.contains("provider_use") => "(a provider_use line that isn't JSON)".into(),
        Err(_) => line.to_string(),
    }
}

/// The live ports: his `~/.bise` (or `$BISE_HOME`), his home folder, the
/// catalog's key store, `bise auth login` for a sign-in; `flow` from the
/// binary (devflow's writer).
/// The open sign-ins (V14): id -> (its process group, cancelled).
type SignIns = std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, (u32, std::sync::Arc<std::sync::atomic::AtomicBool>)>>>;

/// What one line of `bise auth login <id> --events` says (architect m_10942:
/// typed lines, never the terminal's prose).
#[derive(Debug, PartialEq)]
pub enum SignEv {
    Open(String),
    Done,
    Error(String),
}

/// One `--events` line, or None (not one of them).
pub fn sign_ev(line: &str) -> Option<SignEv> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    match s("ev")?.as_str() {
        "open" => s("url").filter(|u| u.starts_with("http://") || u.starts_with("https://")).map(SignEv::Open),
        "done" => Some(SignEv::Done),
        "error" => Some(SignEv::Error(s("text").unwrap_or_else(|| "the sign-in failed".into()))),
        _ => None,
    }
}

/// `bise auth login <id> --events` in its own process group; its lines to
/// the core (`Done::Signing` at its open, `Done::SignedIn` at its end).
fn sign_in_run(id: &str, tx: Sender<Done>, open: &SignIns) -> Result<(), String> {
    use std::io::{BufRead, Read};
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = std::process::Command::new(exe)
        .args(["auth", "login", id, "--events"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Ok(mut m) = open.lock() {
        m.insert(id.to_string(), (child.id(), cancelled.clone()));
    }
    let (id, open) = (id.to_string(), open.clone());
    std::thread::spawn(move || {
        let mut end: Option<Result<(), String>> = None;
        if let Some(out) = child.stdout.take() {
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                match sign_ev(&line) {
                    Some(SignEv::Open(url)) => {
                        let _ = tx.send(Done::Signing { id: id.clone(), url });
                    }
                    Some(SignEv::Done) => end = Some(Ok(())),
                    Some(SignEv::Error(e)) => end = Some(Err(e)),
                    None => {}
                }
            }
        }
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        let ok = child.wait().is_ok_and(|s| s.success());
        if let Ok(mut m) = open.lock() {
            m.remove(&id);
        }
        let res = if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            Err("cancelled".to_string())
        } else {
            match end {
                Some(r) => r,
                None if ok => Ok(()),
                None => Err(err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the sign-in failed").to_string()),
            }
        };
        let _ = tx.send(Done::SignedIn { id, res });
    });
    Ok(())
}

/// Ends `id`'s open sign-in: its whole process group (the local callback
/// listener with it); its thread then reports it cancelled.
fn sign_in_cancel(id: &str, open: &SignIns) -> Result<(), String> {
    let pg = match open.lock() {
        Ok(m) => m.get(id).map(|(pg, c)| {
            c.store(true, std::sync::atomic::Ordering::SeqCst);
            *pg
        }),
        Err(_) => None,
    };
    let Some(pg) = pg else { return Err(format!("no sign-in open for {id}")) };
    std::process::Command::new("kill")
        .args(["-TERM", &format!("-{pg}")])
        .status()
        .map_err(|e| e.to_string())
        .and_then(|s| if s.success() { Ok(()) } else { Err(format!("cannot end the sign-in of {id}")) })
}

pub fn live(flow: FlowFn) -> SetupPorts {
    use bise_catalog::auth_cli;
    let home = bise_home::Home::from_env;
    let auth_paths = move || bise_catalog::auth_cli::Paths::of(&home());
    let env = |k: &str| std::env::var(k).ok();
    // the open sign-ins: their process group and whether it was cancelled
    let open: SignIns = Default::default();
    let open2 = open.clone();
    SetupPorts {
        prefs: Box::new(move || std::fs::read_to_string(home().prefs_file()).ok().and_then(|t| serde_json::from_str(&t).ok())),
        // the whole file, tmp + rename (bise_home's Slot)
        write_prefs: Box::new(move |v| bise_home::Slot::file(home().prefs_file()).set(v.clone()).map_err(|e| e.to_string())),
        accounts: Box::new(move || accounts(&home(), &env)),
        claude_plan: Box::new(move || bise_catalog::detect::claude_plan(home().user_home(), &env)),
        // the key step's one path (firstkey.rs): checked live on its own
        // thread, saved only once it works
        provider_use: Box::new(move |id, key, base_url, seq, tx| {
            let h = home();
            let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
            let said = crate::firstkey::model_for(&h, &env, id);
            let id = id.to_string();
            std::thread::spawn(move || {
                let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
                let used = crate::firstkey::use_provider(&h, &env, &id, key, base_url, &crate::firstkey::live_ports());
                let _ = tx.send(Done::Used { seq, used });
            });
            said
        }),
        key_remove: Box::new(move |id| {
            let paths = auth_paths();
            let setup = bise_catalog::Setup::load(&paths.config);
            auth_cli::logout(&paths, id, &env, &setup.catalog).map(|_| ())
        }),
        sign_in: Box::new(move |id, tx| {
            let setup = bise_catalog::Setup::load(&auth_paths().config);
            // the ChatGPT plan, or OpenRouter's login that mints a key
            if !setup.catalog.provider(id).is_some_and(|p| p.signs_in() || p.id == bise_catalog::openrouter_login::ID) {
                return Err(format!("{id} has no sign-in"));
            }
            sign_in_run(id, tx, &open)
        }),
        sign_in_cancel: Box::new(move |id| sign_in_cancel(id, &open2)),
        scan: Box::new(move |dir, tx| {
            let h = home();
            let skip = vec![h.root().to_path_buf(), bise_home::projects::canonical(&home_workspace())];
            std::thread::spawn(move || {
                let entries = walk(&dir, DEPTH, &skip);
                let _ = tx.send(Done::Scanned { dir, entries });
            });
        }),
        home_dir: home().user_home().to_path_buf(),
        rows: Box::new(move || bise_home::projects::list(&home(), &home_workspace())),
        add: Box::new(move |path| {
            let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
            bise_home::projects::add_path(&home(), &home_workspace(), path, None, now).map(|_| ())
        }),
        registry: Box::new(move |op| {
            use bise_home::projects;
            let key = |p: &Path| p.to_string_lossy().into_owned();
            projects::update(&home(), |l| match &op {
                RegistryOp::Remove(p) => projects::remove(l, &key(p)).map(|_| ()),
                RegistryOp::Move(p, i) => projects::move_to(l, &key(p), *i),
                RegistryOp::Rename(p, n) => projects::rename(l, &key(p), n),
            })
        }),
        flow,
        // the TUI's /plugins functions (crate::plugins), the one code path
        plugins: Box::new(crate::plugins::rows),
        plugin_set: Box::new(|name, on| crate::plugins::set(name, on).map(|_| ())),
        plugin_grant: Box::new(|plugin, uri, perms| crate::plugins::grant(plugin, uri, perms).map(|_| ())),
        plugin_setting: Box::new(crate::plugins::setting),
        roles: Box::new(move || role_rows(&bise_catalog::Setup::load(&home().config_file()))),
        gauge_input: Box::new(move || {
            let file = home().config_file();
            let text = std::fs::read_to_string(&file).ok();
            (bise_catalog::Setup::load(&file), bise_catalog::voice::VoiceModeKeys::from_text(text.as_deref()).tts_model)
        }),
        model_test: Box::new(move |t, generation, tx| {
            let h = home();
            std::thread::spawn(move || {
                let t0 = std::time::Instant::now();
                let env = |k: &str| std::env::var(k).ok();
                let ports = super::model_test::Ports { check: crate::keycheck::check_answer, token: |p| bise_catalog::chatgpt::access_token(p).map_err(|e| e.to_string()) };
                let res = super::model_test::run(&h, &env, &t, &ports);
                let ms = u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX);
                let _ = tx.send(Done::Tested { model: t.model, generation, res, ms });
            });
        }),
        checks: Box::new(move || std::fs::read_to_string(home().checks_file()).unwrap_or_default()),
        record_check: Box::new(move |t| {
            if let Err(e) = bise_catalog::checks::record(&home().checks_file(), t, t.at_ms) {
                eprintln!("bise: {e}");
            }
        }),
        role_set: Box::new(move |role, model, effort| {
            let file = home().config_file();
            check_role_set(&bise_catalog::Setup::load(&file), role, model, effort)?;
            // the TUI's roles screen's own writer (locked, tmp + rename)
            bise_catalog::roles::save_role(&file, role, model, effort)
        }),
        role_set_many: Box::new(move |roles, model| {
            let file = home().config_file();
            let setup = bise_catalog::Setup::load(&file);
            // every role checked first: one refused, nothing written
            for r in roles {
                check_role_set(&setup, r, Some(model), None)?;
            }
            let ids: Vec<&str> = roles.iter().map(String::as_str).collect();
            bise_catalog::roles::save_roles(&file, &ids, model)
        }),
        plugin_login: Box::new(|ws, name, project, tx| {
            let ts = bend_plugins::login::targets(ws);
            let t = bend_plugins::login::find(&ts, name)?.clone();
            std::thread::spawn(move || {
                let (secrets, sd) = (bend_plugins::oauth::store_dir(), bend_plugins::status::dir());
                let open = |u: &str| bend_plugins::oauth::open_browser(u).map_err(|e| e.to_string());
                let r = bend_plugins::login::run(&t, &secrets, Some(&sd), &open, bend_plugins::login::WAIT, None, None);
                let _ = tx.send(Done::LoggedIn { name: t.name.clone(), project, res: r.map(|_| ()) });
            });
            Ok(())
        }),
        plugin_logout: Box::new(|ws, name| crate::plugins::logout(ws, name).map(|_| ())),
        plugin_stage: Box::new(|req, tx| {
            std::thread::spawn(move || {
                let res = bend_plugins::add::stage(&req, &bend_plugins::add::Ctx::standard());
                let _ = tx.send(Done::Staged { res: res.map(StagedPlugin).map_err(|e| e.to_string()) });
            });
        }),
        plugin_market: Box::new(|url, tx| {
            std::thread::spawn(move || {
                use bend_plugins::marketplace;
                let h = bise_home::Home::from_env();
                let res = marketplace::add(&h.marketplaces(), &url)
                    .and_then(|(_, url)| marketplace::read(&h.marketplace_cache(), &url, true))
                    .map(|m| marketplace::rows(&m))
                    .map_err(|e| e.to_string());
                let _ = tx.send(Done::Market { source: url, res });
            });
        }),
        signals: Box::new(|projects, tx| {
            std::thread::spawn(move || {
                let inputs = projects.iter().map(|(path, name)| bend_plugins::signals::gather(path, name)).collect();
                let _ = tx.send(Done::Signals { inputs });
            });
        }),
        add_ctx: Box::new(bend_plugins::add::Ctx::standard),
        plugin_try: Box::new(|dir, ctx, tx| {
            std::thread::spawn(move || {
                bend_plugins::add::try_servers(&dir, &ctx);
                let _ = tx.send(Done::Tried);
            });
        }),
        // the TUI's own threads (crate::plugins), never the core loop
        plugin_connect: Box::new(|id, project, tx| {
            let (tx2, p2, id1, id2) = (tx.clone(), project.clone(), id.to_string(), id.to_string());
            crate::plugins::connect(
                id,
                move |l| drop(tx.send(Done::Connecting { id: id1.clone(), project: project.clone(), said: Some(l), res: None })),
                move |r| drop(tx2.send(Done::Connecting { id: id2, project: p2, said: None, res: Some(r) })),
            )
        }),
        plugin_account_connect: Box::new(|id, account, project, tx| {
            let (tx2, p2, id1, id2) = (tx.clone(), project.clone(), id.to_string(), id.to_string());
            crate::plugins::connect_account(
                id,
                account,
                move |l| drop(tx.send(Done::Connecting { id: id1.clone(), project: project.clone(), said: Some(l), res: None })),
                move |r| drop(tx2.send(Done::Connecting { id: id2, project: p2, said: None, res: Some(r) })),
            )
        }),
        plugin_account_change: Box::new(|id, account, change, project, tx| {
            let cmd = crate::plugins::change_cmd(&change);
            crate::plugins::change_account(id, account, change, move |res| drop(tx.send(Done::AccountChanged { cmd, project, res })));
        }),
        connectors_problem: Box::new(crate::plugins::connectors_problem),
        plugins_refresh: Box::new(|project, tx| {
            crate::plugins::refresh(move |e| drop(tx.send(Done::Refreshed { project, res: e.map_or(Ok(()), Err) })));
        }),
        // the harness binary sets bise update's fetch (crate harness)
        manifest: Box::new(|_, _| {}),
        own_built: Box::new(String::new),
        keychain: live_keychain(),
        telemetry: super::telemetry::live(bise_usage::Build::this("unknown", bise_usage::Channel::Dev)),
        cu: CuPorts {
            is_on: Box::new(crate::computer_use::is_on),
            set_on: Box::new(crate::computer_use::set_on),
            off: Box::new(crate::computer_use::turn_off),
            poll: Box::new(|tx, stop| {
                crate::computer_use::poll(move |v| drop(tx.send(Done::CuCheck(v))), stop);
            }),
            fix: Box::new(|check, fix, busy, tx| crate::computer_use::fix(check, fix, busy, move |t| drop(tx.send(Done::CuSaid(t))))),
            live_test: Box::new(crate::computer_use::live_test),
            profile: Box::new(crate::computer_use::profile_act),
        },
    }
}

/// The home workspace (`~/bise`, `$BISE_HOME_WORKSPACE` in the tests):
/// bise_home's one rule, the hub's too.
fn home_workspace() -> PathBuf {
    bise_home::projects::home_workspace()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("amb-setup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        bise_home::projects::canonical(&d)
    }

    fn status(id: &str, auth: &str, state: &str, email: Option<&str>) -> bise_catalog::auth_cli::Status {
        bise_catalog::auth_cli::Status {
            id: id.into(),
            name: id.to_uppercase(),
            auth: auth.into(),
            state: state.into(),
            from: None,
            email: email.map(String::from),
            plan: None,
            good_until: None,
            main: false,
        }
    }

    #[test]
    fn accounts_say_subscription_or_key_and_one_of_three_states() {
        let a = account_of(&status("chatgpt", "chatgpt", "signed in", Some("c@x")));
        assert_eq!((a.kind.as_str(), a.state.as_str(), a.who.as_deref(), a.label.as_str()), ("subscription", "signed_in", Some("c@x"), "CHATGPT"));
        assert_eq!(account_of(&status("chatgpt", "chatgpt", "expired", Some("c@x"))).state, "expired");
        assert_eq!(account_of(&status("chatgpt", "chatgpt", "signed out", None)).state, "signed_out");
        let k = account_of(&status("anthropic", "api", "ready", None));
        assert_eq!((k.kind.as_str(), k.state.as_str()), ("key", "signed_in"));
        assert_eq!(account_of(&status("mistral", "api", "not set up", None)).state, "signed_out");
    }

    #[test]
    fn found_repos_are_newest_first_capped_and_flag_his_projects() {
        let e = |n: &str, t: u64| Entry { path: PathBuf::from(format!("/h/{n}")), name: n.into(), last_ms: t };
        let f = rank(vec![e("old", 1), e("new", 3), e("mid", 2)], &[PathBuf::from("/h/mid")]);
        assert_eq!(f.iter().map(|f| (f.name.as_str(), f.known)).collect::<Vec<_>>(), vec![("new", false), ("mid", true), ("old", false)]);
        assert!(f.iter().all(|f| f.git));
        let many: Vec<Entry> = (0..80).map(|i| e(&format!("r{i}"), i)).collect();
        let r = rank(many, &[]);
        assert_eq!((r.len(), r[0].name.as_str()), (MAX_FOUND, "r79"));
    }

    #[test]
    fn the_walk_finds_repos_two_levels_down_and_skips_the_rest() {
        let d = scratch("walk");
        for p in ["shop/.git/logs", "code/api/.git", "code/deep/x/.git", ".hidden/.git", "Library/l/.git", "node_modules/n/.git", "notes", "bise/b/.git"] {
            std::fs::create_dir_all(d.join(p)).unwrap();
        }
        std::fs::write(d.join("shop/.git/logs/HEAD"), "x").unwrap();
        std::fs::create_dir_all(d.join("code/wt")).unwrap();
        std::fs::write(d.join("code/wt/.git"), "gitdir: /elsewhere").unwrap();
        let mut names: Vec<String> = walk(&d, DEPTH, &[d.join("bise")]).into_iter().map(|e| e.name).collect();
        names.sort();
        assert_eq!(names, vec!["api", "shop"], "depth 2, no dot folder, Library, node_modules, worktree, skipped folder");
        assert!(walk(&d.join("none"), DEPTH, &[]).is_empty());
        let shop = walk(&d, 1, &[]).into_iter().find(|e| e.name == "shop").unwrap();
        assert!(shop.last_ms > 0 && shop.path == d.join("shop"));
    }

    #[test]
    fn a_key_never_reaches_a_log_line() {
        let l = redact(r#"{"cmd":"provider_use","id":"anthropic","key":"sk-secret"}"#);
        assert!(!l.contains("sk-secret") && l.contains("anthropic"), "{l}");
        assert!(!redact(r#"{"cmd":"provider_use","key":"sk-secret""#).contains("sk-secret"));
        assert_eq!(redact(r#"{"cmd":"found_scan"}"#), r#"{"cmd":"found_scan"}"#);
    }
}
