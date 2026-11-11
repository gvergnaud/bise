//! The hub's loop (`Shell::serve`, called by `daemon::run`) and one
//! message of it executed on the shell (`Shell::dispatch`, the match that
//! was `run`'s loop body, moved out so `run` stays short).

use super::inbox::{Inbox, Next};
use super::repl::kill_pid;
use super::{log_line, version_allowed, write_json, Msg, Repl, Shell, RESUME_TEXT};
use crate::core::Input;
use crate::model::MAIN;
use bise_proto::core::Feed;
use crate::util::{now_ms, wire_escape};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Receiver;

/// What the loop does after a message.
pub(super) enum Flow {
    /// The next message.
    Go,
    /// The hub stops; `keep`: its REPLs stay for the next hub.
    Stop { keep: bool },
}

/// The ticker's flag (hub-lag: one tick in the queue at most) and when
/// that tick was sent: the starts are judged at that time (hub-stall,
/// `check_starts`), not when a busy loop reaches the tick.
#[derive(Default)]
pub(super) struct TickFlag {
    pub(super) queued: AtomicBool,
    pub(super) sent_ms: AtomicU64,
}

impl TickFlag {
    /// The ticker's turn: false when a tick waits already; else marked
    /// queued and sent now (the caller sends it).
    pub(super) fn take(&self) -> bool {
        if self.queued.swap(true, Ordering::AcqRel) {
            return false;
        }
        self.sent_ms.store(now_ms(), Ordering::Release);
        true
    }
}

impl Shell {
    /// The hub's loop, until a stop (its `keep`) or every sender gone:
    /// the clients' lane first (daemon/inbox.rs: a hello never waits
    /// behind the REPLs' lines), the state to the clients when its gate
    /// allows (daemon/state_gate.rs: at most once per 100 ms, the last
    /// change always sent, a pending one flushed at the stop).
    pub(super) fn serve(&mut self, rx: Receiver<Msg>, tick_queued: &TickFlag) -> bool {
        let mut inbox = Inbox::new(rx, Msg::is_urgent);
        let keep = loop {
            // the pending contexts, at most every CONTEXT_MS (daemon/contexts.rs)
            self.contexts_tick();
            // a client's message waiting goes first (inbox push_now)
            if self.state_gate.due(now_ms()) && inbox.push_now() {
                // watched like a message: a slow push says so in hub.log
                self.loop_watch.on("state push".into());
                self.push_state();
                if let Some(l) = self.loop_watch.idle() {
                    log_line(&self.opts.paths, &l);
                }
            }
            let wait = [self.state_gate.wait(now_ms()), self.context_gate.wait(now_ms())].into_iter().flatten().min();
            let m = match inbox.next(wait) {
                Next::Msg(m) => m,
                Next::Idle => continue,
                Next::Closed => break false,
            };
            self.loop_watch.on(m.label());
            // the photo's moment: nothing else waiting behind this message
            self.photos.quiet = inbox.is_empty();
            let flow = self.dispatch(m, tick_queued);
            let flow = self.core_gone(flow);
            if let Some(l) = self.loop_watch.idle() {
                log_line(&self.opts.paths, &l);
            }
            if let Flow::Stop { keep } = flow {
                break keep;
            }
        };
        if self.state_gate.pending() {
            self.push_state();
        }
        // a clean stop: the day's counts so far
        self.usage_write(now_ms());
        keep
    }

    /// sb-core could not be revived (Hub::gave_up): the hub stops, its
    /// REPLs kept for the next hub (nothing of their work lost); one
    /// hub.log line, and `run` exits non-zero (Shell::exit).
    fn core_gone(&self, flow: Flow) -> Flow {
        let Some(why) = self.hub.gave_up() else { return flow };
        log_line(&self.opts.paths, &format!("sb-core could not be revived ({}): stopping", why));
        Flow::Stop { keep: true }
    }

    /// The end of `run`: an error when sb-core stopped the hub (a
    /// non-zero exit for whoever launched it).
    pub(super) fn exit(&self) -> std::io::Result<()> {
        match self.hub.gave_up() {
            Some(why) => Err(std::io::Error::other(format!("sb-core could not be revived: {}", why))),
            None => Ok(()),
        }
    }

    /// Run one message. `tick_queued`: the ticker's flag (one tick in the
    /// queue at most), cleared when a tick is taken.
    pub(super) fn dispatch(&mut self, m: Msg, tick_queued: &TickFlag) -> Flow {
        match m {
            Msg::In(i) => {
                let i = *i;
                let tick = matches!(i, Input::Tick);
                if tick {
                    tick_queued.queued.store(false, Ordering::Release);
                    self.loop_watch.phase("offsets");
                    self.flush_offsets();
                    self.loop_watch.phase("sb-core");
                }
                self.step(i);
                if tick {
                    // the periodic jobs (daemon/tick.rs), each named on a
                    // busy line (daemon/loop_watch.rs)
                    let w = self.loop_watch.clone();
                    let at = super::tick::At { sent_ms: tick_queued.sent_ms.load(Ordering::Acquire) };
                    for (i, job) in super::tick::JOBS.iter().enumerate() {
                        if self.tick.take(i, now_ms()) {
                            w.phase(job.name);
                            (job.run)(self, &at);
                        }
                    }
                }
            }
            Msg::ReplConnected {
                dir,
                gen,
                stream,
                steer,
                interrupt,
                pid,
                adopted,
                busy,
            } => {
                if self.gens.get(&dir) != Some(&gen) {
                    // killed while it was starting
                    kill_pid(pid);
                    return Flow::Go;
                }
                let stream = match super::outbox::of_tcp(&stream, super::outbox::REPL_MAX) {
                    Ok(o) => o,
                    Err(e) => {
                        log_line(&self.opts.paths, &format!("repl {dir}: no writer for it: {e}"));
                        kill_pid(pid);
                        return Flow::Go;
                    }
                };
                if !adopted {
                    let _ = std::fs::write(&steer, "");
                    let _ = std::fs::write(&interrupt, "");
                }
                self.repls.insert(
                    dir.clone(),
                    Repl {
                        stream,
                        steer,
                        interrupt,
                    },
                );
                // its first lines (missed ones, the resume) after the contexts
                self.contexts_now();
                self.start_connected(&dir);
                self.recycle_started(&dir);
                crate::util::timing(&format!("repl connected {} (adopted {})", dir, adopted));
                if let Some(q) = self.switching.remove(&dir) {
                    // a switched REPL: same session, the core never saw
                    // it go; the writes it missed go now
                    log_line(&self.opts.paths, &format!("switched the REPL of {}", dir));
                    if let Some(r) = self.repls.get_mut(&dir) {
                        for l in &q {
                            let _ = r.stream.write_all(l.as_bytes());
                        }
                    }
                    if !q.is_empty() {
                        return Flow::Go;
                    }
                }
                if !adopted && self.resume_turn.remove(&dir) {
                    // its turn was cut: the first turn of the new process
                    // continues it (queued writes of the core come after)
                    if let Some(r) = self.repls.get_mut(&dir) {
                        let _ = r
                            .stream
                            .write_all(format!("say {}\n", wire_escape(RESUME_TEXT)).as_bytes());
                    }
                    if let Some(name) = self.agent_by_dir(&dir).map(|a| a.name.clone()) {
                        self.feed_rec(&name, Feed::info("its turn was interrupted by a restart — it continues where it left off"));
                    }
                }
                if let Some(name) = self.agent_by_dir(&dir).map(|a| a.name.clone()) {
                    if busy {
                        // adopted mid-turn: busy until its `--- idle`
                        self.step(Input::ReplLine {
                            agent: name.clone(),
                            line: "  obs: turn_started".into(),
                        });
                    } else {
                        self.step(Input::ReplReady { agent: name.clone() });
                    }
                    // design §10: a restart keeps the card the REPL waits on
                    if adopted {
                        self.gate_restore(&dir, &name);
                    } else {
                        self.gate_fresh(&name);
                    }
                }
            }
            Msg::ReplStartStep { dir, gen } => self.start_step(&dir, gen),
            Msg::ReplSpawned { dir, gen, pid } => {
                self.start_step(&dir, gen);
                let _ = std::fs::write(
                    self.opts.paths.agent_dir(&dir).join("repl.pid"),
                    pid.to_string(),
                );
                crate::procs::add_sid(&self.opts.paths.agent_dir(&dir).join("repl.sids"), pid);
                if self.gens.get(&dir) == Some(&gen) {
                    self.pids.insert(dir, (gen, pid));
                } else {
                    kill_pid(pid);
                }
            }
            Msg::ReplLine {
                dir,
                gen,
                line,
                offset,
            } => {
                if self.gens.get(&dir) == Some(&gen) {
                    if !self.on_ev_line(&dir, &line, offset) {
                        self.on_repl_line(&dir, &line);
                    }
                    self.offsets.insert(dir, offset);
                }
            }
            Msg::ReplGone { dir, gen, reason, cause } => return self.repl_gone(dir, gen, reason, cause),
            Msg::Page(m) => self.page_msg(m),
            Msg::OlderHello { stream, peer } => self.older_hello(stream, &peer),
            Msg::RpcNew { id, stream, v, peer } => self.rpc_new(id, stream, v, peer),
            Msg::Typed { to, v } => self.typed_msg(to, v),
            Msg::ProcCounts(c) => self.procs_counts(c),
            Msg::ClientLine { id, v } => self.client_line(id, v),
            Msg::ClientGone { id } => {
                self.rpc_gone(id);
                self.procs.ask(super::processes::Ask::Gone(id));
                if self.clients.remove(&id).is_some() {
                    self.step(Input::ClientGone { client: id });
                }
            }
            Msg::AgentNew { token, stream, v, asker } => self.agent_new(token, stream, v, &asker),
            Msg::WakeSeen(s) => self.wake_seen(s),
            Msg::Placed { name, ws, err } => self.step(Input::Placed { name, ws, err }),
            Msg::Unplaced { name, snapshot, err } => self.step(Input::Unplaced { name, snapshot, err }),
            Msg::CommitStyle(style) => self.commit_style = style,
            Msg::ArtBuilt(b) => {
                let (key, view) = *b;
                self.art_built(key, view);
            }
            Msg::XHub { token, stream, v } => self.xhub_op(token, stream, v),
            Msg::Version { mut stream, v, asker } => {
                let from = v.get("from").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let what = v.get("do").and_then(|x| x.as_str()).unwrap_or("").to_string();
                // sb-core says `from` is the sender's (daemon/agent_req.rs, docs/issues/17), then main's ops are main's
                match self.spoke(0, &from, &format!("version {what}"), &asker).and_then(|()| version_allowed(&from, &what)) {
                    Ok(()) => {
                        let text = self.version_op(&v);
                        write_json(&mut stream, &json!({"ok": true, "text": text}));
                    }
                    Err(e) => {
                        write_json(&mut stream, &json!({"ok": false, "error": e}));
                    }
                }
            }
            Msg::Notice { kind, text } => {
                // a switch phase alone (its start, its probation passed): the switch row says it, no thread line
                if kind != crate::switch::SWITCH_KIND {
                    self.feed_rec(MAIN, if kind == "warn" { Feed::warn(text) } else { Feed::info(text) });
                }
                self.broadcast_versions();
                // the switcher's notices mark its phases (versions.json)
                self.switch_moved();
            }
            Msg::Release { client, v } => self.release_event(client, v),
            Msg::Update(v) => self.update_event(v),
            Msg::Changes { name, v } => self.on_changes(name, v),
            Msg::Serve { id, v, serve, root } => self.on_serve(&id, v, serve, root),
            Msg::RoleLine { dir, key, line } => self.step(Input::RoleLine { dir, key, line }),
            Msg::GuideSeen(s) => self.guide_seen(*s),
            Msg::GuideDone { hash, target, got } => self.guide_done(hash, target, got),
            Msg::UsageCounted(w) => {
                let (ledger, week) = *w;
                self.usage_counted(ledger);
                self.pulse_filled(week);
            }
            Msg::TimerName { id, reply } => self.step(Input::TimerName { id, reply }),
            Msg::GateChecked { dir, n, req, out } => self.on_checked(&dir, &n, *req, out),
            Msg::View(step) => step(self),
            Msg::BuildEnded { rev } => {
                self.building.remove(&rev);
                self.broadcast_versions();
            }
            Msg::Land { line } => {
                if let Some(rec) = line {
                    let landed = matches!(rec, Feed::Landed(_));
                    self.feed_rec(MAIN, rec);
                    // a land moved main or a feature: the features' facts again
                    self.refresh_features();
                    if landed {
                        self.usage_count(bise_usage::CountKey::Land);
                        self.merged_all();
                        // main moved: its commit style again, off the loop
                        self.ask_commit_style();
                    }
                }
                self.state_now();
            }
            Msg::Shutdown { keep } => return Flow::Stop { keep },
        }
        Flow::Go
    }

    /// `dir`'s REPL of generation `gen` is gone (it exited, or its start
    /// stalled: repl_starts.rs calls this at once, so a later message of
    /// that generation finds it dead and kills its process).
    pub(super) fn repl_gone(&mut self, dir: String, gen: u64, reason: String, cause: super::repl_starts::Gone) -> Flow {
        // a killed generation is not live anymore: its exit is
        // expected; any exit of the live one is a crash (the hub
        // never asks a REPL to quit)
        if self.gens.get(&dir) != Some(&gen) {
            return Flow::Go;
        }
        let stops = self.start_gone(&dir, cause);
        self.gens.remove(&dir);
        self.repls.remove(&dir);
        self.usage_gone(&dir);
        self.gate_forget(&dir);
        self.pids.remove(&dir);
        // a dead REPL's port names no route, even for a moment: a view's
        // call would reach its retiring bridge (architect m_51383); its
        // next process registers its own at its spawn
        let port = self.ports.remove(&dir);
        // its writer's lock goes with it (a respawn resumes the log)
        self.recorders.remove(&dir);
        if self.switching.contains_key(&dir) && !self.switch_spawned.contains(&dir) {
            // the reload a switch asked for: the same session on
            // this hub's binary, the same port
            if let Some(name) = self.agent_by_dir(&dir).map(|a| a.name.clone()) {
                self.restored.insert(dir.clone());
                self.switch_spawned.insert(dir.clone());
                self.spawn_on(&name, true, None, port, false);
                return Flow::Go;
            }
        }
        // the new process of a switch died: a crash like any other
        self.switching.remove(&dir);
        self.switch_spawned.remove(&dir);
        self.restored.remove(&dir);
        // a new version on probation: a REPL that dies is a
        // reason to roll back, a slow start is not (repl_start)
        if cause.reports() {
            crate::switch::report_failure(&self.opts.paths, &format!("the REPL of {} stopped: {}", dir, reason));
        }
        // a stall's or a stop's facts: sb-core decides if it counts
        // (hub/stall.bend)
        let (stalled, loaded, row, stopped) = match cause {
            super::repl_starts::Gone::Stalled { loaded, row, .. } => (true, loaded, u64::from(row), false),
            super::repl_starts::Gone::Stopped => (false, false, u64::from(stops), true),
            super::repl_starts::Gone::Died => (false, false, 0, false),
        };
        if let Some(name) = self.agent_by_dir(&dir).map(|a| a.name.clone()) {
            self.step(Input::ReplExited { agent: name, crashed: true, reason, stalled, loaded, row, stopped });
        }
        Flow::Go
    }
}

impl Shell {
    /// The image store's care on the tick (crate::image_care, D14): the
    /// images this hub's hot messages and open cards name are touched
    /// hourly, the store pruned at most once a day; hub.log says so.
    pub(super) fn image_care(&mut self) {
        let home = self.opts.home.clone();
        let st = &self.hub.st;
        // the hot messages' words, read in the journal (c6)
        let hot: Vec<String> = st.hot().map(|(m, _)| st.text(m.id)).collect();
        let texts = hot.iter().map(String::as_str).chain(st.open_cards().map(|c| c.text.as_str()));
        let paths = self.opts.paths.clone();
        let care = crate::image_care::tick(&self.opts.paths, &home.images_dir(), &home.prefs_file(), texts, now_ms(), move |r| {
            match r {
                Ok(n) => log_line(&paths, &format!("images: pruned {n} file(s) from the store")),
                Err(e) => log_line(&paths, &format!("images: the prune failed: {e}")),
            }
        });
        if let Some(n) = care.touched {
            log_line(&self.opts.paths, &format!("images: touched the {n} image(s) this hub names"));
        }
        if let Some(d) = care.pruning {
            log_line(&self.opts.paths, &format!("images: pruning what is older than {d} day(s)"));
        }
    }
}
