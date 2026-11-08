//! The window's setup commands (bise desktop S11, core side; amb-home's
//! file in amb-core's core, seams agreed m_9134): `prefs_set`,
//! `sign_in`, `provider_use`, `key_remove`, `found_scan`, `project_add`,
//! `project_remove`, `project_move`, `project_rename` (proto::draft's
//! `AppCmd`), answered by `prefs`, `accounts`, `found`, the projects list
//! read again, or `error {cmd, text}`; and the agent plugins (P.1):
//! `plugins`, `plugin_set`, `plugin_login`, `plugin_logout`, answered by
//! `plugins` (the TUI's /plugins functions, crate::plugins); computer use
//! (P.2): `computer_use {act}` answered by `computer_use` (the TUI's
//! /computer-use functions: its setup check runs on its own thread only
//! while the window's page is open, stops at `leave` or [`CU_IDLE`] after
//! the last `check`, never on the tick); and `setup_check` (S.8, check
//! again: prefs, accounts and the projects list sent again, read fresh).
//! All I/O through [`crate::ambient::setup::SetupPorts`]; the jobs that end later (a walk,
//! a sign-in) come back on the tick. A key is never in an event.

use super::Core;
use crate::ambient::setup::{rank, CuBusy, Done, RegistryOp, SetupPorts};
use bise_proto::app_cmd::{AppCmd, TelemetryAct};
use bise_proto::app_rows::CheckState;
use bise_proto::core_ev::CoreEv;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;

/// How often his projects are read again for what they name (the band's
/// whys; architect m_41049).
pub(crate) const SIGNALS_EVERY: std::time::Duration = std::time::Duration::from_secs(3600);

/// A check's end: what it found, and how long the call took (ms).
pub(crate) type CheckEnd<'a> = (Result<&'a crate::firstkey::Works, &'a crate::firstkey::Failed>, u64);

/// A `provider_check` event (bise_proto's CoreEv, draft): the key never
/// in it. `end`: none while it checks.
pub(crate) fn provider_check(id: &str, name: &str, state: CheckState, model: &str, pasted: bool, end: Option<CheckEnd<'_>>) -> Value {
    let works = end.and_then(|(r, _)| r.ok());
    let failed = end.and_then(|(r, _)| r.err());
    let ms = end.map(|(_, ms)| ms);
    let ev = CoreEv::ProviderCheck {
        id: id.to_string(),
        name: name.to_string(),
        state,
        model: model.to_string(),
        label: if model.is_empty() { String::new() } else { bise_catalog::names::long_name(model) },
        pasted,
        answer: works.map(|w| w.answer.clone()).filter(|a| !a.is_empty()),
        main: works.map(|w| w.main.clone()).filter(|m| !m.is_empty()),
        lines: works.map(|w| w.lines.clone()).unwrap_or_default(),
        why: failed.map(|f| f.why),
        said: failed.map(|f| f.said.clone()).filter(|t| !t.is_empty()),
        url: failed.and_then(|f| f.url.clone()),
        ms,
    };
    serde_json::to_value(ev).unwrap_or_else(|_| json!({}))
}

/// The commands this file answers (never a hub's, never another of the
/// core's own).
pub const TAGS: &[&str] = &[
    "prefs_set",
    "sign_in",
    "sign_in_cancel",
    "provider_use",
    "key_remove",
    "found_scan",
    "project_add",
    "project_remove",
    "project_move",
    "project_rename",
    "plugins",
    "plugin_set",
    "plugin_grant",
    "plugin_setting",
    "plugin_login",
    "plugin_logout",
    "plugin_connect",
    "plugins_refresh",
    "plugin_add",
    "plugin_add_confirm",
    "plugin_add_cancel",
    "plugin_remove",
    "computer_use",
    "setup_check",
    "setup_test",
    "helper_perms",
    "role_set",
    "role_set_many",
    "app_version",
    "setup_first_cancel",
    "keychain",
    "telemetry",
];

/// Computer use's setup check stops this long after the page's last
/// `check` (a lost `leave`, a hidden window: it never polls forever).
pub(crate) const CU_IDLE: std::time::Duration = std::time::Duration::from_secs(120);

/// Computer use's page is open: its poll, what runs, what it last said.
struct CuLive {
    stop: Arc<AtomicBool>,
    last_check: Instant,
    check: Value,
    busy: CuBusy,
    said: Option<String>,
    flash: Option<String>,
    /// the live test ran by itself once (never again on its own)
    auto_ran: bool,
    /// the last event sent (only a change goes out)
    sent: Value,
}

/// The poll has run long enough without a `check`.
/// His projects read for what they name (the band's whys): the first
/// time, then at most every [`SIGNALS_EVERY`]; on the port's thread, the
/// summary meanwhile from the last evidence.
fn ask_signals(s: &mut Setup, now: Instant) {
    if s.signals_at.is_some_and(|t| now.saturating_duration_since(t) < SIGNALS_EVERY) {
        return;
    }
    s.signals_at = Some(now);
    let projects = (s.ports.rows)().into_iter().filter(|r| !r.home).map(|r| (r.path, r.name)).collect();
    (s.ports.signals)(projects, s.tx.clone());
}

pub(crate) fn cu_idle(last_check: Instant, now: Instant) -> bool {
    now.saturating_duration_since(last_check) >= CU_IDLE
}

pub(crate) struct Setup {
    pub(super) ports: SetupPorts,
    pub(super) tx: Sender<Done>,
    rx: Receiver<Done>,
    /// the provider whose sign-in runs (one at a time)
    signing: Option<String>,
    /// the plugin servers whose login runs (their browser is open) and
    /// the connectors whose connect runs (by id): their rows say pending
    logins: Vec<String>,
    /// the add menu's previews waiting for his click (dropped with the
    /// core: their staged folders go)
    staged: bend_plugins::add::Held,
    /// the other apps' MCP configs (`add::found`): read once with the rows,
    /// again after a refresh or an add, never at each send
    found: Option<Vec<bend_plugins::add::Found>>,
    /// what his projects name (`signals::reasons`, the band's whys) and
    /// when they were last asked for: read again at most every
    /// [`SIGNALS_EVERY`], on the port's thread, never at a send
    evidence: Vec<bend_plugins::signals::Evidence>,
    signals_at: Option<Instant>,
    /// the marketplaces read so far, by source (their last `browse`)
    markets: Vec<(String, Vec<bise_proto::app_rows::Plugin>)>,
    /// the last `plugins_summary` sent, per project (only a change goes out)
    summaries: std::collections::HashMap<Option<String>, bise_proto::app_rows::PluginsSummary>,
    /// computer use's page while it is open
    cu: Option<CuLive>,
    /// the desktop app's update check (bar S.6)
    pub(super) update: super::app_update::UpdateCheck,
    /// R8: his `/model` line held until its provider works
    held: Option<Held>,
    /// N1: a /keychain move runs (a second one is refused, never queued)
    keychain_moving: bool,
    /// N1: the last move's lines and whether it failed
    keychain_said: (Vec<String>, bool),
    /// `provider_use`: the last seq given, and each provider's newest
    /// (an older check's end is dropped, architect m_28532)
    use_seq: u64,
    /// the check running per provider: its seq and when it started (its end says how long)
    using: std::collections::HashMap<String, (u64, Instant)>,
    /// settings · Setup's gauge: its tests, generations, the helper's perms
    pub(super) gauge: super::gauge::GaugeLive,
}

/// R8 (the TUI's sb.rs /model, architect m_12214): a `/model <model>`
/// whose provider has no key, held (never sent) until that provider
/// works; dropped by `setup_first_cancel` (the window's own, or Electron
/// main's when the window that sent it closes), its project removed, or
/// the core restarting (memory only). One at a time: a newer replaces it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Held {
    project: String,
    agent: String,
    line: String,
    cid: Option<u64>,
    model: String,
}

/// What the window's typed slash does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hold {
    /// held: the window opens the provider's setup (`setup_first`)
    Held,
    /// not a `/model` that needs a key: sent to the hub as is
    Forward,
}

impl Core {
    /// The setup's ports (the binary's live ones, or a test's).
    pub fn set_setup(&mut self, ports: SetupPorts) {
        let (tx, rx) = channel();
        self.setup = Some(Setup { ports, tx, rx, signing: None, logins: Vec::new(), staged: Default::default(), found: None, evidence: Vec::new(), signals_at: None, markets: Vec::new(), summaries: Default::default(), cu: None, update: Default::default(), held: None, keychain_moving: false, keychain_said: (Vec::new(), false), use_seq: 0, using: Default::default(), gauge: Default::default() });
    }

    /// R8: his `/model <model>` in `agent`'s view, before it goes to the
    /// hub: a model whose provider has no key is held and the window opens
    /// that provider's setup; anything else goes on (models::model_needs_key,
    /// the TUI's one rule).
    pub(super) fn hold_if_keyless(&mut self, project: &str, agent: &str, line: &str, cid: Option<u64>) -> Hold {
        let Some(s) = self.setup.as_mut() else { return Hold::Forward };
        let Some((provider, model)) = crate::models::model_needs_key(line) else { return Hold::Forward };
        s.held = Some(Held { project: project.into(), agent: agent.into(), line: line.into(), cid, model: model.clone() });
        let name = crate::models::provider_name(&provider, "");
        let mut ev = json!({"ev": "setup_first", "project": project, "agent": agent, "provider": provider, "name": name, "model": model});
        if let Some(c) = cid {
            ev["cid"] = json!(c);
        }
        self.emit(ev);
        Hold::Held
    }

    /// R8: the accounts changed (a key set, a sign-in's end): a held
    /// `/model` whose provider now works goes to the hub as he sent it.
    fn replay_held(&mut self) {
        let Some(h) = self.setup.as_ref().and_then(|s| s.held.clone()) else { return };
        crate::models::forget_keys();
        if crate::models::keyless(&h.model).is_some() {
            return;
        }
        if let Some(s) = self.setup.as_mut() {
            s.held = None;
        }
        let mut v = json!({"cmd": "slash", "project": h.project, "agent": h.agent, "line": h.line});
        if let Some(c) = h.cid {
            v["cid"] = json!(c);
        }
        self.typed_cmd(v);
    }

    /// R8: drop the held `/model` (of `project` only, when given).
    fn drop_held(&mut self, project: Option<&str>) {
        if let Some(s) = self.setup.as_mut() {
            if project.is_none_or(|p| s.held.as_ref().is_some_and(|h| h.project == p)) {
                s.held = None;
            }
        }
    }

    /// What the window needs first: his preferences and his accounts (at
    /// the core's start and at each `shown`: the window opens later).
    pub fn app_start(&mut self) {
        self.emit_prefs();
        self.emit_accounts();
        self.emit_roles();
        self.emit_keychain();
        self.emit_summary_of(None);
    }

    /// N1: where his secrets are, and the last move's lines.
    fn emit_keychain(&mut self) {
        let Some(s) = &self.setup else { return };
        let store = (s.ports.keychain.now)();
        let (lines, failed) = s.keychain_said.clone();
        let mut ev = json!({"ev": "keychain", "store": store, "moving": s.keychain_moving});
        if !lines.is_empty() {
            ev["lines"] = json!(lines);
        }
        if failed {
            ev["failed"] = json!(true);
        }
        self.emit(ev);
    }

    /// N1 (the TUI's /keychain [on|off]): none says where; `on`/`off`
    /// starts the move on its thread (its end on the tick).
    fn keychain(&mut self, to: Option<String>) -> Result<(), String> {
        let on = match to.as_deref() {
            None => {
                self.emit_keychain();
                return Ok(());
            }
            Some("on") => true,
            Some("off") => false,
            Some(_) => return Err("/keychain takes on or off.".into()),
        };
        let Some(s) = self.setup.as_mut() else { return Err("this core has no setup".into()) };
        if s.keychain_moving {
            return Err("a move of your keys is still running: wait for its end".into());
        }
        s.keychain_moving = true;
        s.keychain_said = (Vec::new(), false);
        (s.ports.keychain.switch)(on, s.tx.clone());
        self.emit_keychain();
        Ok(())
    }

    /// His act on his usage counts (step 6): answered by `telemetry`
    /// (only here: it carries the id), and by `prefs` when the act wrote
    /// them. An act this core doesn't know: nothing.
    fn telemetry(&mut self, act: TelemetryAct) -> Result<(), String> {
        let Some(s) = &self.setup else { return Err("this core has no setup".into()) };
        let Some(ev) = (s.ports.telemetry)(act)? else { return Ok(()) };
        if matches!(act, TelemetryAct::On | TelemetryAct::Off | TelemetryAct::NoteSeen) {
            self.emit_prefs();
        }
        self.emit(serde_json::to_value(ev).unwrap_or_else(|_| json!({})));
        Ok(())
    }

    /// Each role's model and effort (V17).
    fn emit_roles(&mut self) {
        let Some(s) = &self.setup else { return };
        let items = (s.ports.roles)();
        self.emit(json!({"ev": "roles", "items": items}));
        self.emit_gauge();
    }

    fn emit_prefs(&mut self) {
        let Some(s) = &self.setup else { return };
        let prefs = bise_home::prefs::for_window((s.ports.prefs)());
        self.emit(json!({"ev": "prefs", "prefs": prefs}));
    }

    fn emit_accounts(&mut self) {
        let Some(s) = &self.setup else { return };
        let items = (s.ports.accounts)();
        let claude_plan = (s.ports.claude_plan)();
        self.emit(json!({"ev": "accounts", "items": items, "claude_plan": claude_plan}));
        self.replay_held();
        self.emit_gauge();
    }

    /// `provider_use` (firstkey.rs): `checking` at once, the check on
    /// its own thread, its end on the tick ([`Self::on_used`]).
    fn provider_use(&mut self, id: String, key: Option<String>, base_url: Option<String>) {
        let Some(s) = self.setup.as_mut() else { return };
        s.use_seq += 1;
        let seq = s.use_seq;
        s.using.insert(id.clone(), (seq, Instant::now()));
        let pasted = key.is_some();
        let (name, model) = (s.ports.provider_use)(&id, key, base_url, seq, s.tx.clone());
        self.emit(provider_check(&id, &name, CheckState::Checking, &model, pasted, None));
    }

    /// A check ended: dropped when a newer one for that provider runs;
    /// else works (then the accounts and roles it changed) or failed.
    fn on_used(&mut self, seq: u64, used: crate::firstkey::Used) {
        let Some(s) = self.setup.as_mut() else { return };
        let Some(&(running, t0)) = s.using.get(&used.id) else { return };
        if running != seq {
            return;
        }
        s.using.remove(&used.id);
        let ms = u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX);
        let state = if used.res.is_ok() { CheckState::Works } else { CheckState::Failed };
        if let Some(u) = self.usage.as_mut() {
            u.count(super::usage::of_check(used.res.as_ref().map(drop).map_err(|f| f.why)));
        }
        self.emit(provider_check(&used.id, &used.name, state, &used.model, used.pasted, Some((used.res.as_ref(), ms))));
        if used.res.is_ok() {
            self.emit_accounts();
            self.emit_roles();
        }
    }

    pub(super) fn setup_error(&mut self, cmd: &str, text: &str) {
        self.emit(json!({"ev": "error", "cmd": cmd, "text": text}));
    }

    /// The folder of project `id` (a hub id of the registry, never home).
    fn project_path(&self, id: &str) -> Result<PathBuf, String> {
        let s = self.setup.as_ref().ok_or("this core has no setup")?;
        let rows = (s.ports.rows)();
        match rows.iter().find(|r| r.id == id) {
            Some(r) if r.home => Err("bise's home stays first and can't change".into()),
            Some(r) => Ok(r.path.clone()),
            None => Err(format!("there's no project {id}")),
        }
    }

    /// One of the window's setup commands.
    pub(super) fn app_cmd(&mut self, c: AppCmd) {
        // a preview he never answered goes after add::HELD_TTL
        if let Some(s) = self.setup.as_mut() {
            s.staged.sweep(Instant::now());
        }
        let Some(s) = &self.setup else {
            return self.setup_error("setup", "this core has no setup");
        };
        let p = &s.ports;
        let (cmd, res): (&str, Result<(), String>) = match c {
            AppCmd::PrefsSet { key, value } => {
                let now = (p.prefs)().unwrap_or_else(|| json!({}));
                let res = bise_home::prefs::set_dotted(&now, &key, value).and_then(|v| (p.write_prefs)(&v));
                if res.is_ok() {
                    self.emit_prefs();
                    self.update_pref_set(&key);
                }
                ("prefs_set", res)
            }
            AppCmd::ProviderUse { id, key, base_url } => {
                self.provider_use(id, key, base_url);
                return;
            }
            AppCmd::Keychain { to } => ("keychain", self.keychain(to)),
            AppCmd::Telemetry { act } => ("telemetry", self.telemetry(act)),
            AppCmd::KeyRemove { id } => {
                let res = (p.key_remove)(&id);
                if res.is_ok() {
                    self.emit_accounts();
                }
                ("key_remove", res)
            }
            AppCmd::SignIn { id } => {
                let res = match &s.signing {
                    Some(other) => Err(format!("a sign-in to {other} is still open in your browser")),
                    None => (p.sign_in)(&id, s.tx.clone()),
                };
                if res.is_ok() {
                    if let Some(s) = self.setup.as_mut() {
                        s.signing = Some(id);
                    }
                }
                ("sign_in", res)
            }
            // V14: ends the open sign-in; its end comes as error
            // {cmd: sign_in, text: '<id>: cancelled'} from its thread
            AppCmd::SignInCancel { id } => {
                let res = match &s.signing {
                    Some(open) if *open == id => (p.sign_in_cancel)(&id),
                    _ => Err(format!("no sign-in open for {id}")),
                };
                ("sign_in_cancel", res)
            }
            // bar S.6: the app's own build; the core reads the channel
            AppCmd::AppVersion { id, built } => {
                self.app_version(id, built);
                return;
            }
            // update-flow: his click on the pill's `update`, to one hub
            AppCmd::BiseUpdate {} => {
                let res = match self.bise_update_now() {
                    true => Ok(()),
                    false => Err("no project's bise is running to update it".to_string()),
                };
                ("bise_update", res)
            }
            AppCmd::FoundScan { dir } => {
                let dir = dir.filter(|d| !d.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| p.home_dir.clone());
                (p.scan)(dir, s.tx.clone());
                ("found_scan", Ok(()))
            }
            AppCmd::ProjectAdd { path, land } => {
                let res = (p.add)(Path::new(&path));
                let res = match (res, land) {
                    (Ok(()), Some(trunk)) => {
                        (p.flow)(&bise_home::projects::canonical(Path::new(&path)), trunk).map_err(|e| format!("added; its flow wasn't set: {e}"))
                    }
                    (r, _) => r,
                };
                if let Err(e) = &res {
                    eprintln!("ambient-core: project_add {path}: {e}");
                }
                self.projects_now();
                ("project_add", res)
            }
            AppCmd::ProjectRemove { project } => {
                let res = self.registry(&project, RegistryOp::Remove);
                if res.is_ok() {
                    self.drop_held(Some(&project));
                    self.forget_thread_copy(&project);
                }
                ("project_remove", res)
            }
            AppCmd::SetupFirstCancel => {
                self.drop_held(None);
                return;
            }
            AppCmd::ProjectMove { project, order } => {
                ("project_move", self.registry(&project, |path| RegistryOp::Move(path, order as usize)))
            }
            AppCmd::ProjectRename { project, name } => ("project_rename", self.registry(&project, |path| RegistryOp::Rename(path, name))),
            AppCmd::Plugins { project } => ("plugins", self.emit_plugins(project)),
            AppCmd::PluginSet { project, name, on } => {
                let res = (p.plugin_set)(&name, on);
                let shown = self.emit_plugins(project);
                ("plugin_set", res.and(shown))
            }
            AppCmd::PluginGrant { project, plugin, uri, perms } => {
                let res = (p.plugin_grant)(&plugin, &uri, &perms);
                let shown = self.emit_plugins(project);
                ("plugin_grant", res.and(shown))
            }
            AppCmd::PluginSetting { project, name, var, value } => {
                let res = self.workspace_of(project.as_deref()).and_then(|ws| (p.plugin_setting)(&ws, &name, &var, value.as_deref()));
                let shown = self.emit_plugins(project);
                ("plugin_setting", res.and(shown))
            }
            AppCmd::PluginLogin { project, name } => {
                let res = match self.workspace_of(project.as_deref()) {
                    Ok(_) if s.logins.contains(&name) => Err(format!("the login to {name} is still open in your browser")),
                    Ok(ws) => (p.plugin_login)(&ws, &name, project.clone(), s.tx.clone()),
                    Err(e) => Err(e),
                };
                if res.is_ok() {
                    if let Some(s) = self.setup.as_mut() {
                        s.logins.push(name);
                    }
                    let _ = self.emit_plugins(project);
                }
                ("plugin_login", res)
            }
            AppCmd::PluginLogout { project, name } => {
                let res = self.workspace_of(project.as_deref()).and_then(|ws| (p.plugin_logout)(&ws, &name));
                let shown = self.emit_plugins(project);
                ("plugin_logout", res.and(shown))
            }
            AppCmd::PluginConnect { project, id } => {
                let res = (p.plugin_connect)(&id, project.clone(), s.tx.clone());
                // its row shows login: pending until the connect ends
                if res.is_ok() {
                    if let Some(s) = self.setup.as_mut() {
                        s.logins.push(id);
                    }
                    let _ = self.emit_plugins(project);
                }
                ("plugin_connect", res)
            }
            AppCmd::PluginsRefresh { project } => {
                // the other apps' configs are read again with the next rows
                if let Some(s) = self.setup.as_mut() {
                    s.found = None;
                }
                let Some(s) = self.setup.as_ref() else { return };
                let p = &s.ports;
                (p.plugins_refresh)(project, s.tx.clone());
                ("plugins_refresh", Ok(()))
            }
            AppCmd::PluginAdd { project: _, add } => ("plugin_add", self.plugin_add(add)),
            AppCmd::PluginAddConfirm { token, secrets } => {
                let ctx = (p.add_ctx)();
                let res = match self.setup.as_mut() {
                    Some(s) => {
                        // an import changes what's left to import
                        s.found = None;
                        s.staged.confirm(&token, &secrets, &ctx).map_err(|e| e.to_string())
                    }
                    None => Err("this core has no setup".into()),
                };
                // installed: its servers tried once, after his click only
                let res = res.map(|dir| {
                    if let Some(s) = self.setup.as_ref() {
                        (s.ports.plugin_try)(dir, ctx, s.tx.clone());
                    }
                });
                let shown = self.emit_plugins(None);
                ("plugin_add_confirm", res.and(shown))
            }
            AppCmd::PluginRemove { project, name } => {
                let ctx = (p.add_ctx)();
                let res = self.workspace_of(project.as_deref()).and_then(|ws| {
                    let id = bend_plugins::id::PluginId::parse(&name)?;
                    bend_plugins::remove::remove(&ctx, Some(&ws), &id).map_err(|e| e.to_string())
                });
                let shown = self.emit_plugins(project);
                ("plugin_remove", res.and(shown))
            }
            AppCmd::PluginAddCancel { token } => {
                let res = match self.setup.as_mut() {
                    Some(s) => s.staged.cancel(&token).map_err(|e| e.to_string()),
                    None => Err("this core has no setup".into()),
                };
                ("plugin_add_cancel", res)
            }
            AppCmd::RoleSet { role, model, effort } => {
                // the role's model before, to test it only when it changed (page models-settings)
                let before = (p.gauge_input)().0.role_model(&role).0;
                let res = (p.role_set)(&role, model.as_deref(), effort.as_deref());
                if res.is_ok() {
                    self.emit_roles();
                    if let Err(e) = self.test_changed_role(&role, &before) {
                        self.setup_error("setup_test", &e);
                    }
                }
                ("role_set", res)
            }
            AppCmd::RoleSetMany { roles, model } => {
                // the news line's one act: one write, then the model tested once
                let (setup, _) = (p.gauge_input)();
                let before: Vec<String> = roles.iter().map(|r| setup.role_model(r).0).collect();
                let res = (p.role_set_many)(&roles, &model);
                if res.is_ok() {
                    self.emit_roles();
                    // the first role it moved (one already on the model changed nothing), with its was
                    if let Some((role, was)) = roles.iter().zip(&before).find(|(_, b)| **b != model) {
                        if let Err(e) = self.test_changed_role(role, was) {
                            self.setup_error("setup_test", &e);
                        }
                    }
                }
                ("role_set_many", res)
            }
            AppCmd::SetupCheck => {
                self.app_start();
                self.poll_projects(true);
                ("setup_check", Ok(()))
            }
            AppCmd::SetupTest { models } => ("setup_test", self.setup_test(models)),
            AppCmd::HelperPerms { mic, accessibility, screen } => ("helper_perms", self.helper_perms(mic, accessibility, screen)),
            AppCmd::ComputerUse { act, fix } => ("computer_use", self.computer_use(&act, fix.as_deref())),
            other => return self.setup_error("setup", &format!("not a setup command: {other:?}")),
        };
        if let Err(e) = res {
            self.setup_error(cmd, &e);
        }
    }

    /// One act of computer use's page: on, off, uninstall, check (start
    /// or keep its poll), leave (stop it), fix (a row's button).
    fn computer_use(&mut self, act: &str, fix: Option<&str>) -> Result<(), String> {
        let s = self.setup.as_mut().ok_or("this core has no setup")?;
        match act {
            "on" => (s.ports.cu.set_on)(true),
            "off" | "uninstall" => {
                let line = (s.ports.cu.off)(act == "uninstall");
                if let Some(c) = s.cu.as_mut() {
                    c.flash = Some(line);
                }
            }
            "check" => match s.cu.as_mut() {
                Some(c) => c.last_check = Instant::now(),
                None => {
                    let stop = Arc::new(AtomicBool::new(false));
                    (s.ports.cu.poll)(s.tx.clone(), stop.clone());
                    s.cu = Some(CuLive {
                        stop,
                        last_check: Instant::now(),
                        check: Value::Null,
                        busy: CuBusy::default(),
                        said: None,
                        flash: None,
                        auto_ran: false,
                        sent: Value::Null,
                    });
                }
            },
            "leave" => {
                if let Some(c) = s.cu.take() {
                    c.stop.store(true, Ordering::SeqCst);
                }
                return Ok(());
            }
            "fix" => {
                let f = fix.ok_or("computer_use fix: which fix?")?;
                let c = s.cu.as_mut().ok_or("computer use's page isn't open")?;
                c.said = None;
                c.flash = (s.ports.cu.fix)(&c.check, f, c.busy.clone(), s.tx.clone());
            }
            other => return Err(format!("computer_use: no act {other}")),
        }
        self.emit_cu(true);
        Ok(())
    }

    /// `computer_use` now (`always`: even when nothing changed).
    fn emit_cu(&mut self, always: bool) {
        let Some(s) = self.setup.as_mut() else { return };
        let on = (s.ports.cu.is_on)();
        let (rows, said, flash) = match s.cu.as_mut() {
            Some(c) => {
                crate::computer_use::settle(&mut c.busy.lock().unwrap_or_else(|e| e.into_inner()), &c.check);
                let busy = c.busy.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let rows = crate::computer_use::rows(&c.check, &busy);
                // the live test runs by itself once, as soon as it can (the TUI's rule)
                if !c.auto_ran && rows.iter().any(|r| r.id == "live_test" && r.st == crate::computer_use::St::Waits) {
                    c.auto_ran = true;
                    (s.ports.cu.live_test)(&c.busy);
                }
                (rows, c.said.clone(), c.flash.clone())
            }
            None => (Vec::new(), None, None),
        };
        let ready = crate::computer_use::ready(&rows);
        let rows: Vec<_> = rows.iter().map(crate::computer_use::row_data).collect();
        let mut ev = json!({"ev": "computer_use", "on": on, "ready": ready, "rows": rows});
        if let Some(t) = said {
            ev["said"] = json!(t);
        }
        if let Some(t) = flash {
            ev["flash"] = json!(t);
        }
        if let Some(c) = s.cu.as_mut() {
            if !always && c.sent == ev {
                return;
            }
            c.sent = ev.clone();
        }
        self.emit(ev);
    }

    /// The folder whose plugins a command means: the project's, else bise's home.
    pub(super) fn workspace_of(&self, project: Option<&str>) -> Result<PathBuf, String> {
        let s = self.setup.as_ref().ok_or("this core has no setup")?;
        let rows = (s.ports.rows)();
        let row = match project {
            Some(id) => rows.iter().find(|r| r.id == id).ok_or_else(|| format!("there's no project {id}"))?,
            None => rows.iter().find(|r| r.home).ok_or("bise's home isn't in the projects list")?,
        };
        Ok(row.path.clone())
    }

    /// `plugins` for that workspace (its logins still open marked pending:
    /// bend_plugins::catalog::rows, the one builder), then `browse` of the
    /// Mistral connectors not connected yet (the same rows, `available`).
    fn emit_plugins(&mut self, project: Option<String>) -> Result<(), String> {
        let ws = self.workspace_of(project.as_deref())?;
        let s = self.setup.as_ref().ok_or("this core has no setup")?;
        let rows = (s.ports.plugins)(&ws, &s.logins);
        let connectors = (s.ports.connectors_problem)();
        self.emit_summary(project.clone(), &rows);
        let (browse, items): (Vec<_>, Vec<_>) = rows.into_iter().partition(|r| r.state == bise_proto::app_rows::PluginState::Available);
        self.ev(CoreEv::Plugins { project, items, connectors: connectors.clone() });
        self.ev(CoreEv::Browse { source: MISTRAL.into(), items: browse, connectors });
        Ok(())
    }

    /// `plugins_summary` of that workspace, its rows read now (the core's
    /// start, a marketplace read): only when it changed.
    fn emit_summary_of(&mut self, project: Option<String>) {
        let Ok(ws) = self.workspace_of(project.as_deref()) else { return };
        let Some(s) = self.setup.as_ref() else { return };
        let rows = (s.ports.plugins)(&ws, &s.logins);
        self.emit_summary(project, &rows);
    }

    /// `plugins_summary` from these rows (all of them, Browse's too:
    /// bend_plugins::catalog::summary, the one builder), sent only when it
    /// differs from the last one sent for that project.
    fn emit_summary(&mut self, project: Option<String>, rows: &[bise_proto::app_rows::Plugin]) {
        let Some(s) = self.setup.as_mut() else { return };
        if s.found.is_none() {
            s.found = Some(bend_plugins::add::found(&(s.ports.add_ctx)()));
        }
        ask_signals(s, Instant::now());
        let markets: Vec<bend_plugins::catalog::Market> =
            s.markets.iter().map(|(source, items)| bend_plugins::catalog::Market { source, items }).collect();
        let summary = bend_plugins::catalog::summary(rows, s.found.as_deref().unwrap_or_default(), &markets, &s.evidence, project.is_some());
        if s.summaries.get(&project) == Some(&summary) {
            return;
        }
        s.summaries.insert(project.clone(), summary.clone());
        self.ev(CoreEv::PluginsSummary { project, summary });
    }

    /// The add menu's item: staged off the loop (then a preview), a
    /// marketplace read (then `browse`), or a brief to main.
    fn plugin_add(&mut self, add: bise_proto::app_cmd::PluginAddKind) -> Result<(), String> {
        use bise_proto::app_cmd::PluginAddKind as K;
        let s = self.setup.as_ref().ok_or("this core has no setup")?;
        match add {
            K::Marketplace { url } => (s.ports.plugin_market)(url, s.tx.clone()),
            K::Agent { words } => self.send_via(bend_plugins::add::agent_brief(&words), false, "capsule-start"),
            other => {
                let req = bend_plugins::add::Request::from_wire(&other).ok_or("plugin_add: not something to add")?;
                (s.ports.plugin_stage)(req, s.tx.clone());
            }
        }
        Ok(())
    }

    /// A change to the list for project `id`, then the list read again.
    fn registry(&mut self, id: &str, op: impl FnOnce(PathBuf) -> RegistryOp) -> Result<(), String> {
        let path = self.project_path(id)?;
        let s = self.setup.as_ref().ok_or("this core has no setup")?;
        (s.ports.registry)(op(path))?;
        self.poll_projects(true);
        Ok(())
    }

    /// The jobs that ended: `found` for a walk, `accounts` (or an error)
    /// for a sign-in.
    pub(super) fn setup_tick(&mut self) {
        if let Some(s) = self.setup.as_mut() {
            if s.cu.as_ref().is_some_and(|c| cu_idle(c.last_check, Instant::now())) {
                if let Some(c) = s.cu.take() {
                    c.stop.store(true, Ordering::SeqCst);
                }
            }
        }
        let Some(s) = &self.setup else { return };
        let done: Vec<Done> = s.rx.try_iter().collect();
        for d in done {
            match d {
                Done::Scanned { dir, entries } => {
                    let known: Vec<PathBuf> = self.setup.as_ref().map(|s| (s.ports.rows)().into_iter().map(|r| r.path).collect()).unwrap_or_default();
                    let items = rank(entries, &known);
                    self.emit(json!({"ev": "found", "dir": dir.to_string_lossy(), "items": items}));
                }
                // V14: the link it opened, once per sign-in
                Done::Signing { id, url } => {
                    self.emit(json!({"ev": "signing", "id": id, "url": url}));
                }
                Done::SignedIn { id, res } => {
                    if let Some(s) = self.setup.as_mut() {
                        s.signing = None;
                    }
                    if let Err(e) = res {
                        self.setup_error("sign_in", &format!("{id}: {e}"));
                    }
                    self.emit_accounts();
                }
                Done::Used { seq, used } => self.on_used(seq, used),
                d @ Done::Tested { .. } => self.on_tested(d),
                Done::CuCheck(v) => {
                    if let Some(c) = self.setup.as_mut().and_then(|s| s.cu.as_mut()) {
                        c.check = v;
                    }
                    self.emit_cu(false);
                }
                Done::CuSaid(t) => {
                    if let Some(c) = self.setup.as_mut().and_then(|s| s.cu.as_mut()) {
                        c.said = Some(t);
                    }
                    self.emit_cu(false);
                }
                Done::Manifest { text, base, target } => self.on_manifest(text, &base, &target),
                Done::Keychain { lines, failed } => {
                    if let Some(s) = self.setup.as_mut() {
                        s.keychain_moving = false;
                        s.keychain_said = (lines, failed);
                    }
                    self.emit_keychain();
                }
                Done::Staged { res } => match res {
                    Ok(staged) => {
                        let Some(s) = self.setup.as_mut() else { continue };
                        let (token, preview) = s.staged.hold(staged.0);
                        self.ev(CoreEv::PluginPreview { token, preview });
                    }
                    Err(e) => self.setup_error("plugin_add", &e),
                },
                // a step's line goes to the core's log (no window event
                // carries it yet); its end, the list again
                Done::Connecting { id, project, said, res } => {
                    if let Some(l) = said {
                        eprintln!("ambient-core: plugin_connect: {l}");
                    }
                    match res {
                        Some(Err(e)) => self.setup_error("plugin_connect", &e),
                        Some(Ok(())) => {}
                        None => continue,
                    }
                    if let Some(s) = self.setup.as_mut() {
                        s.logins.retain(|n| *n != id);
                    }
                    let _ = self.emit_plugins(project);
                }
                Done::Tried => {
                    let _ = self.emit_plugins(None);
                }
                Done::Refreshed { project, res } => {
                    if let Err(e) = res {
                        self.setup_error("plugins_refresh", &e);
                    }
                    let _ = self.emit_plugins(project);
                }
                Done::Signals { inputs } => {
                    let projects: Vec<Option<String>> = match self.setup.as_mut() {
                        Some(s) => {
                            s.evidence = bend_plugins::signals::reasons(&inputs);
                            s.summaries.keys().cloned().collect()
                        }
                        None => Vec::new(),
                    };
                    // each summary sent so far, ranked and said again
                    for p in projects {
                        self.emit_summary_of(p);
                    }
                }
                Done::Market { source, res } => match res {
                    Ok(items) => {
                        if let Some(s) = self.setup.as_mut() {
                            s.markets.retain(|(src, _)| *src != source);
                            s.markets.push((source.clone(), items.clone()));
                        }
                        self.ev(CoreEv::Browse { source, items, connectors: None });
                        self.emit_summary_of(None);
                    }
                    Err(e) => self.setup_error("plugin_add", &format!("{source}: {e}")),
                },
                Done::LoggedIn { name, project, res } => {
                    if let Some(s) = self.setup.as_mut() {
                        s.logins.retain(|n| *n != name);
                    }
                    if let Err(e) = res {
                        self.setup_error("plugin_login", &format!("{name}: {e}"));
                    }
                    let _ = self.emit_plugins(project);
                }
            }
        }
        // bar S.6: the release channel again once an hour
        self.update_tick();
    }
}


/// The `browse` source of his Mistral connectors not connected yet.
const MISTRAL: &str = "mistral";


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_computer_use_poll_stops_two_minutes_after_the_last_check() {
        let t0 = Instant::now();
        assert!(!cu_idle(t0, t0 + CU_IDLE - std::time::Duration::from_secs(1)));
        assert!(cu_idle(t0, t0 + CU_IDLE));
        assert!(!cu_idle(t0 + CU_IDLE, t0), "a clock that went back: not idle");
    }
}
