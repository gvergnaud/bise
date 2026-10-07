//! The window's setup commands (bise desktop S11, core side; amb-home's
//! file in amb-core's core, seams agreed m_9134): `prefs_set`,
//! `sign_in`, `key_set`, `key_remove`, `found_scan`, `project_add`,
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
use bise_proto::draft::AppCmd;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;

/// The commands this file answers (never a hub's, never another of the
/// core's own).
pub const TAGS: &[&str] = &[
    "prefs_set",
    "sign_in",
    "sign_in_cancel",
    "key_set",
    "key_remove",
    "found_scan",
    "project_add",
    "project_remove",
    "project_move",
    "project_rename",
    "plugins",
    "plugin_set",
    "plugin_login",
    "plugin_logout",
    "computer_use",
    "setup_check",
    "role_set",
    "app_version",
    "setup_first_cancel",
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
pub(crate) fn cu_idle(last_check: Instant, now: Instant) -> bool {
    now.saturating_duration_since(last_check) >= CU_IDLE
}

pub(crate) struct Setup {
    pub(super) ports: SetupPorts,
    pub(super) tx: Sender<Done>,
    rx: Receiver<Done>,
    /// the provider whose sign-in runs (one at a time)
    signing: Option<String>,
    /// the plugin servers whose login runs (their browser is open)
    logins: Vec<String>,
    /// computer use's page while it is open
    cu: Option<CuLive>,
    /// the desktop app's update check (bar S.6)
    pub(super) update: super::app_update::UpdateCheck,
    /// R8: his `/model` line held until its provider works
    held: Option<Held>,
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
        self.setup = Some(Setup { ports, tx, rx, signing: None, logins: Vec::new(), cu: None, update: Default::default(), held: None });
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
    }

    /// Each role's model and effort (V17).
    fn emit_roles(&mut self) {
        let Some(s) = &self.setup else { return };
        let items = (s.ports.roles)();
        self.emit(json!({"ev": "roles", "items": items}));
    }

    fn emit_prefs(&mut self) {
        let Some(s) = &self.setup else { return };
        let prefs = bise_home::prefs::for_window((s.ports.prefs)());
        self.emit(json!({"ev": "prefs", "prefs": prefs}));
    }

    fn emit_accounts(&mut self) {
        let Some(s) = &self.setup else { return };
        let items = (s.ports.accounts)();
        self.emit(json!({"ev": "accounts", "items": items}));
        self.replay_held();
    }

    fn setup_error(&mut self, cmd: &str, text: &str) {
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
                }
                ("prefs_set", res)
            }
            AppCmd::KeySet { id, key } => {
                let res = (p.key_set)(&id, &key);
                if res.is_ok() {
                    self.emit_accounts();
                }
                ("key_set", res)
            }
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
                self.poll_projects(true);
                ("project_add", res)
            }
            AppCmd::ProjectRemove { project } => {
                let res = self.registry(&project, RegistryOp::Remove);
                if res.is_ok() {
                    self.drop_held(Some(&project));
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
            AppCmd::RoleSet { role, model, effort } => {
                let res = (p.role_set)(&role, model.as_deref(), effort.as_deref());
                if res.is_ok() {
                    self.emit_roles();
                }
                ("role_set", res)
            }
            AppCmd::SetupCheck => {
                self.app_start();
                self.poll_projects(true);
                ("setup_check", Ok(()))
            }
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

    /// `plugins` for that workspace (its logins still open marked pending).
    fn emit_plugins(&mut self, project: Option<String>) -> Result<(), String> {
        let ws = self.workspace_of(project.as_deref())?;
        let s = self.setup.as_ref().ok_or("this core has no setup")?;
        let items = (s.ports.plugins)(&ws, &s.logins);
        let mut ev = json!({"ev": "plugins", "items": items});
        if let Some(p) = project {
            ev["project"] = json!(p);
        }
        self.emit(ev);
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
