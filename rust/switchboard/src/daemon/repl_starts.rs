//! The hub's side of REPL starts (the decisions: crate::repl_start).
//! Every start takes a slot: a fresh spawn waits in `start_queue` until
//! one is free (`admit_starts`, at each request and tick), a switch or a
//! recycle is admitted by `switch_admitted` from what is left. Each phase
//! of a start is progress (`Msg::ReplStartStep`, `ReplSpawned`, the old
//! process's exit in a switch); `check_starts` ends a start without
//! progress for `still_limit_ms` at the load now (crate::load): its
//! process killed, a `ReplGone` that says whether the stall reports to
//! probation (a death always does).

use super::{
    adopt, adoptable, busy_at, free_port, hash_keys, kill_pid, log_line, plugins_fingerprint,
    prompt_is_stale, skill_roots, skills_fingerprint, stall_for_tests, supervise, write_logged, Msg, PromptInputs,
    Shell, PROMPT_PLUGINS_FILE,
};
use crate::repl_start::{admit, Cand, Kind};
use crate::util::now_ms;
use std::path::PathBuf;
use std::process::Command;

/// A fresh spawn waiting for a slot.
#[derive(Debug)]
pub(super) struct Queued {
    pub(super) name: String,
    pub(super) gen: u64,
    pub(super) resume: bool,
    pub(super) crash_note: Option<String>,
    pub(super) port: Option<u16>,
    pub(super) asked_ms: u64,
    /// sb-core's `wait_load` (hub/stall.bend): admitted only at a load
    /// under x2
    pub(super) wait_load: bool,
}

/// Why a REPL is gone (probation reads it; sb-core the stall's facts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Gone {
    /// It exited, crashed or could not be run.
    Died,
    /// Its start made no progress (crate::repl_start): `loaded` at load
    /// x2+, `row` that REPL's stalls in a row.
    Stalled { reports: bool, loaded: bool, row: u32 },
    /// It ended by SIGTERM, SIGINT or SIGHUP: someone asked it to stop
    /// (repl::gone_of). sb-core decides what it counts (hub/stall.bend).
    Stopped,
}

impl Gone {
    /// A reason for a version on probation to roll back (a stop is not).
    pub(super) fn reports(self) -> bool {
        match self {
            Gone::Died => true,
            Gone::Stalled { reports, .. } => reports,
            Gone::Stopped => false,
        }
    }
}

impl Shell {
    /// The user's input waits on this agent: main, a client's focus, a
    /// message queued for it, a stall restarted (it goes first).
    fn urgent(&self, name: &str, dir: &str) -> bool {
        name == crate::model::MAIN
            || self.hub.focused().contains(name)
            || crate::board::queued_count(&self.hub.st, name) > 0
            || self.starts.goes_first(dir)
    }

    /// A fresh spawn of `name` waits for a slot (generation `gen`).
    pub(super) fn queue_start(&mut self, dir: &str, q: Queued) {
        self.start_queue.insert(dir.to_string(), q);
        self.admit_starts();
    }

    /// The queued spawns that get a free slot, in order; one sb-core
    /// said waits for the machine (`wait_load`) only at a load under x2.
    pub(super) fn admit_starts(&mut self) {
        if self.start_queue.is_empty() {
            return;
        }
        let loaded = self.start_queue.values().any(|q| q.wait_load) && crate::load::is_loaded(crate::load::factor());
        let cands: Vec<Cand> = self
            .start_queue
            .iter()
            .filter(|(_, q)| !(q.wait_load && loaded))
            .map(|(dir, q)| Cand {
                dir: dir.clone(),
                kind: Kind::Start,
                urgent: self.urgent(&q.name, dir),
                asked_ms: q.asked_ms,
                main: q.name == crate::model::MAIN,
            })
            .collect();
        let free = self.starts.free();
        for c in admit(cands, free) {
            let Some(q) = self.start_queue.remove(&c.dir) else { continue };
            // killed or asked again meanwhile: not this start
            if self.gens.get(&c.dir) != Some(&q.gen) {
                continue;
            }
            self.starts.begin(&c.dir, Kind::Start, now_ms());
            self.spawn_fresh(&q.name, q.gen, q.resume, q.crash_note, q.port);
        }
    }

    /// Of the idle REPLs to switch or recycle (name, dir), the ones that
    /// get a slot now (the others: a later tick), each taking it.
    pub(super) fn switch_admitted(&mut self, stale: Vec<(String, String)>) -> Vec<(String, String)> {
        let cands: Vec<Cand> = stale
            .iter()
            .map(|(name, dir)| {
                let recycle = self.recycle.is_due(dir) && !self.reload_repls.contains(dir);
                let kind = if recycle { Kind::Recycle } else { Kind::Switch };
                Cand { dir: dir.clone(), kind, urgent: self.urgent(name, dir), asked_ms: 0, main: name == crate::model::MAIN }
            })
            .collect();
        let free = self.starts.free();
        let now = now_ms();
        let mut out = Vec::new();
        for c in admit(cands, free) {
            if let Some(n) = stale.iter().find(|(_, d)| *d == c.dir) {
                self.starts.begin(&c.dir, c.kind, now);
                out.push(n.clone());
            }
        }
        out
    }

    /// A phase of `dir`'s start is done (generation `gen`: a killed
    /// one's phases are not progress).
    pub(super) fn start_step(&mut self, dir: &str, gen: u64) {
        if self.gens.get(dir) == Some(&gen) {
            self.starts.progress(dir, now_ms());
        }
    }

    /// `dir`'s new process connected: its start is done, its slot free.
    pub(super) fn start_connected(&mut self, dir: &str) {
        self.switch_spawned.remove(dir);
        self.starts.connected(dir);
    }

    /// `dir`'s live REPL is gone: the old process of a switch exits as
    /// asked (its slot stays for the new one: progress); any other end
    /// frees the slot. Returns its row of stops with no connect between
    /// (a `Stopped` end, +1; 0 otherwise).
    pub(super) fn start_gone(&mut self, dir: &str, cause: Gone) -> u32 {
        if self.switching.contains_key(dir) && !self.switch_spawned.contains(dir) && cause == Gone::Died {
            self.starts.progress(dir, now_ms());
        } else if cause == Gone::Stopped {
            return self.starts.stopped(dir);
        } else {
            self.starts.ended(dir);
        }
        0
    }

    /// The starts without progress: killed and gone (a crash, restarted
    /// first); then the queue gets the freed slots.
    ///
    /// Judged at `at_ms`, when the tick that asks was sent (hub-stall):
    /// every progress message sent before it is applied by now (the
    /// inbox keeps their order), so a loop that was busy never fails a
    /// start whose progress still waited behind it. A stalled one is gone
    /// at once (`repl_gone`): a later message of its generation (spawned,
    /// connected) finds it dead and kills its process, never an orphan
    /// that got the brief and shares the agent's session.
    pub(super) fn check_starts(&mut self, at_ms: u64) {
        // the limit scales with the load (crate::repl_start)
        let factor = crate::load::factor();
        for s in self.starts.stalled(at_ms, factor) {
            let reason = format!(
                "its REPL made no progress in {} s while starting (load x{:.1}, {} at once at most, the rest wait)",
                s.secs,
                factor,
                crate::repl_start::MAX_IN_FLIGHT
            );
            log_line(&self.opts.paths, &format!("repl {} not started: {}", s.dir, reason));
            if let Some((_, pid)) = self.pids.get(&s.dir) {
                kill_pid(*pid);
            }
            if let Some(gen) = self.gens.get(&s.dir).copied() {
                let _ = self.repl_gone(s.dir, gen, reason, Gone::Stalled { reports: s.reports, loaded: s.loaded, row: s.row });
            }
        }
        self.admit_starts();
    }
}

impl Shell {
    /// Start the REPL of `name` on a supervisor thread. `port`: the port of the process it replaces (a switch keeps the
    /// port: background commands and steer files are keyed by it).
    pub(super) fn spawn_on(&mut self, name: &str, resume: bool, crash_note: Option<String>, port: Option<u16>, wait_load: bool) {
        // it reads context.txt at its first call (daemon/contexts.rs)
        self.contexts_now();
        let Some(a) = self.hub.st.agents.get(name).cloned() else {
            return;
        };
        let dir = a.dir.clone();
        let adir = self.opts.paths.agent_dir(&dir);
        let _ = std::fs::create_dir_all(&adir);
        // its temp folder and the harness's own files (approvals-design.md
        // §7.1): nothing in /tmp
        let (tmp, run) = (self.opts.paths.agent_tmp(&dir), self.opts.paths.agent_run(&dir));
        if let Err(e) = crate::tools_env::make_agent_dirs(&tmp, &run) {
            log_line(&self.opts.paths, &format!("{}: cannot create {}: {}", a.name, tmp.display(), e));
        }
        // the approvals mode, read by the runtime before each gated call
        self.write_mode_file(&dir);
        let tmp_s = tmp.to_string_lossy().into_owned();
        let role = self.role_of(&a, &tmp_s);
        write_logged(&self.opts.paths, &adir.join("role.md"), &role);
        if !adir.join("context.txt").exists() {
            let _ = std::fs::write(adir.join("context.txt"), "");
        }
        let gen = self.next_gen;
        self.next_gen += 1;
        self.gens.insert(dir.clone(), gen);
        if self.booting && resume {
            if let Some(r) = adoptable(&adir) {
                log_line(&self.opts.paths, &format!("adopting the REPL of {} (pid {}, port {})", a.name, r.pid, r.port));
                self.pids.insert(dir.clone(), (gen, r.pid));
                if !self.reload_id.is_empty() {
                    self.reload_repls.insert(dir.clone());
                }
                self.bins.insert(dir.clone(), r.bin.clone());
                self.ports.insert(dir.clone(), r.port);
                self.attach_session(&a, &dir, &adir);
                let tx = self.tx.clone();
                let paths = self.opts.paths.clone();
                std::thread::spawn(move || adopt(r, dir, gen, adir, tx, paths));
                return;
            }
            // not adoptable: dead. Was it in the middle of a turn?
            let wire = adir.join("wire.log");
            let len = std::fs::metadata(&wire).map(|m| m.len()).unwrap_or(0);
            if len > 0 && busy_at(&wire, len) {
                log_line(&self.opts.paths, &format!("{}: its REPL died mid-turn, the turn resumes", a.name));
                self.resume_turn.insert(dir.clone());
            }
        }
        // a fresh process, in a free slot; a switch's new process holds
        // the slot of its reload
        if !self.starts.in_flight(&dir) {
            let q = Queued { name: a.name.clone(), gen, resume, crash_note, port, asked_ms: now_ms(), wait_load };
            return self.queue_start(&dir, q);
        }
        self.spawn_fresh(&a.name, gen, resume, crash_note, port);
    }

    /// A fresh REPL process of `name` (generation `gen`), in its slot.
    fn spawn_fresh(&mut self, name: &str, gen: u64, resume: bool, crash_note: Option<String>, port: Option<u16>) {
        let Some(a) = self.hub.st.agents.get(name).cloned() else {
            return;
        };
        let dir = a.dir.clone();
        let adir = self.opts.paths.agent_dir(&dir);
        let (tmp, run) = (self.opts.paths.agent_tmp(&dir), self.opts.paths.agent_run(&dir));
        // a fresh process: a fresh wire log, once the old one's events
        // the hub never took are in the session log (architect m_18556)
        let caught = self.catch_up_wire(&dir, &adir);
        if caught > 0 {
            log_line(&self.opts.paths, &format!("session log of {}: {} events of its last REPL recorded before the restart", name, caught));
        }
        write_logged(&self.opts.paths, &adir.join("wire.log"), "");
        write_logged(&self.opts.paths, &adir.join("wire.offset"), "0");
        let _ = std::fs::remove_file(adir.join("repl.json"));
        let port = match port.map(Ok).unwrap_or_else(free_port) {
            Ok(p) => p,
            Err(e) => {
                let reason = format!("no free port: {}", e);
                let _ = self.tx.send(Msg::ReplGone { dir, gen, reason, cause: Gone::Died });
                return;
            }
        };
        let (session, cont) = self.prepare_session(&a, &dir, &adir, resume);
        let mut cmd = Command::new(&self.opts.repl_bin);
        cmd.current_dir(&self.opts.app_root);
        // its whole environment: the hub's minus every internal and test
        // variable (bise_home::env), plus what names this agent
        let mut env = bise_home::env::for_child(
            bise_home::env::Child::Repl,
            [
                ("BEND_REPL_PORT", std::ffi::OsString::from(port.to_string())),
                // its run dir is the hub's (`Paths::run`): its bridge.json
                // is where the hub looks for it
                ("BEND_RUN_DIR", self.opts.paths.run.clone().into()),
                ("BEND_SESSION_FILE", session.clone().into()),
                ("BEND_EXTRA_PROMPT", adir.join("role.md").into()),
                ("BEND_CONTEXT_FILE", adir.join("context.txt").into()),
                ("BEND_WORKDIR", a.ws.path.clone().into()),
                // the REPL starts its plugins bridge with this binary
                // (`bise plugins serve`, docs/plugins.md)
                ("BEND_HARNESS_BIN", self.opts.exe.clone().into()),
                // the agents' socket, never hub.sock (docs/issues/16)
                ("SB_SOCKET", self.opts.paths.agent_socket().into()),
                ("BEND_WIRE_LOG", adir.join("wire.log").into()),
                ("SB_AGENT", a.name.clone().into()),
                // the hub's home workspace (taste.md, people.md): an agent's
                // `~/bise` is its shell's HOME, not this hub's when the home
                // is elsewhere (a QA hub, amb-tools m_6141)
                ("BISE_HOME_WORKSPACE", crate::paths::home_workspace().into()),
                // which model: config.toml `model`, or `agent_model` (BISE-142)
                ("BISE_ROLE", (if a.is_main { "main" } else { "agent" }).into()),
                // its own model and effort, over both (BISE-135: /model)
                (bise_catalog::CHOICE_ENV, adir.join("choice.toml").into()),
                // RFC 0002 §9: two dev servers must not fight for one port
                ("SB_TASK", a.name.clone().into()),
                // what it starts is its: killed at its stop or /drop (BISE-243)
                (
                    crate::procs::ENV,
                    crate::procs::for_repl(std::env::var(crate::procs::ENV).ok().as_deref(), &self.proc_hub, &dir, now_ms())
                        .into(),
                ),
                (
                    "SB_PORT_OFFSET",
                    self.hub.st.order.iter().position(|n| *n == a.name).unwrap_or(0).to_string().into(),
                ),
            ],
        );
        if let Some(j) = &self.opts.jsrt_bin {
            env.set("BEND_JSRT_BIN", j);
        }
        // the request ring (/debug completion, bise_session::reqlog): the
        // exact body of the agent's last model calls, a few slots in
        // agents/<a>/requests/ the REPL overwrites in turn. Bodies only,
        // no headers: the keys go in headers.
        let req_dir = adir.join("requests");
        if std::fs::create_dir_all(&req_dir).is_ok() {
            env.set("BEND_WIRE_DUMP", format!("{}/", req_dir.display()));
        }
        for (k, v) in crate::tools_env::temp_env(&tmp, &run) {
            env.set(k, v);
        }
        let keys = self.opts.spawn_env.map(|f| f()).unwrap_or_default();
        self.spawn_keys.insert(dir.clone(), hash_keys(&keys));
        let ws = PathBuf::from(&a.ws.path);
        // a worktree task's one cargo target, <task dir>/target (sweep::task_target)
        if let Some(t) = crate::sweep::task_target(&self.opts.paths.worktrees, &ws) {
            env.set("CARGO_TARGET_DIR", t);
        }
        // the desktop's rules, read once at this start like the prompt's
        // (role_of): bise-pages (prompts/skills-all) only where they're on
        let desktop = bise_home::projects::desktop_for(&self.opts.paths.workspace);
        // its skill folders reach its bridge (bend_plugins::skills, the
        // session's skills index), never guessed there
        let roots = skill_roots(&ws, &self.opts.app_root, a.is_main, desktop);
        let (k, v) = bend_plugins::skills::Root::env(&roots);
        env.set(k, v);
        let spawned = PromptInputs {
            plugins: plugins_fingerprint(&ws),
            skills: skills_fingerprint(&self.opts.paths, &roots),
            ws,
            main: a.is_main,
            desktop,
        };
        let fp = spawned.combined();
        self.spawn_plugins.insert(dir.clone(), spawned);
        for (k, v) in keys {
            match v {
                Some(v) => env.set(k, v),
                None => env.unset(k),
            };
        }
        // its own session: what it starts is found by session id too,
        // even a macOS binary whose environment is hidden (BISE-243)
        crate::procs::own_session(&mut cmd);
        if resume && cont && session.exists() {
            env.set("BEND_CONTINUE", "1");
            // its plugins changed since its prompt was built: the restored
            // session takes this start's prompt (runtime/persist.bend
            // with_cfg), else it never learns of a new plugin
            if prompt_is_stale(&adir, fp) {
                env.set("BEND_FRESH_PROMPT", "1");
            }
        }
        let _ = std::fs::write(adir.join(PROMPT_PLUGINS_FILE), fp.to_string());
        if let Some(n) = crash_note {
            env.set("BEND_CRASH_NOTE", n);
        }
        self.bins.insert(dir.clone(), self.opts.repl_bin.clone());
        self.ports.insert(dir.clone(), port);
        let tx = self.tx.clone();
        let (paths, home) = (self.opts.paths.clone(), self.opts.home.clone());
        let workdir = PathBuf::from(&a.ws.path);
        std::thread::spawn(move || {
            stall_for_tests();
            // sb, the hub's PATH, the user's login-shell PATH, the
            // standard dirs; the model is told once whether rg and git
            // are there (BISE-166), then which plugins it has and what
            // for. Here, off the hub's loop: the first spawn may wait for
            // the login shell (read once, at most 3 s)
            let agent_path = crate::tools_env::hub_agent_path(&paths);
            env.set("BEND_TOOLS_NOTE", crate::tools_env::session_note(&agent_path, &workdir)).set("PATH", agent_path);
            // each phase is progress (the hub's start watch)
            let step = || tx.send(Msg::ReplStartStep { dir: dir.clone(), gen }).is_ok();
            step();
            // the AGENTS.md files of its working folder (a task: its
            // worktree's), read again at each start and /reload (BISE-232)
            env.set(
                crate::agents_md::ENV,
                crate::agents_md::write_for(&workdir, &home, &adir.join("agents-md.md")),
            );
            step();
            slow_spawn_for_tests();
            env.apply(&mut cmd);
            supervise(cmd, dir, gen, adir, port, tx, paths)
        });
    }
}

/// The hub's starts: the idle limit is START_STILL_MS, or in a test
/// `SB_START_STILL_MS` (ms; times the load factor all the same).
pub(super) fn new_starts() -> crate::repl_start::Starts {
    match bise_home::env::test_setting("SB_START_STILL_MS").and_then(|s| s.trim().parse::<u64>().ok()) {
        Some(ms) => crate::repl_start::Starts::with_still_ms(ms),
        None => Default::default(),
    }
}

/// Tests only: `SB_SLOW_SPAWN=<file>` holding a number of ms: each REPL
/// start of this hub waits that long before its process is spawned, with
/// no progress (a slow machine; a very long one: a version whose REPLs
/// never start). Read at each start: the test writes it at the switch.
pub(super) fn slow_spawn_for_tests() {
    let Some(f) = bise_home::env::test_setting("SB_SLOW_SPAWN") else {
        return;
    };
    if let Some(ms) = std::fs::read_to_string(f).ok().and_then(|s| s.trim().parse::<u64>().ok()) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}
