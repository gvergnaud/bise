//! One message of the hub's loop (`daemon::run`), executed on the shell:
//! the match that was `run`'s loop body, moved out so `run` stays short.

use super::repl::kill_pid;
use super::{log_line, version_allowed, write_json, Msg, Repl, Shell, RESUME_TEXT};
use crate::core::Input;
use crate::model::MAIN;
use crate::util::wire_escape;
use serde_json::json;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

/// What the loop does after a message.
pub(super) enum Flow {
    /// The next message.
    Go,
    /// The hub stops; `keep`: its REPLs stay for the next hub.
    Stop { keep: bool },
}

impl Shell {
    /// Run one message. `tick_queued`: the ticker's flag (one tick in the
    /// queue at most), cleared when a tick is taken.
    pub(super) fn dispatch(&mut self, m: Msg, tick_queued: &AtomicBool) -> Flow {
        match m {
            Msg::In(i) => {
                let tick = matches!(i, Input::Tick);
                if tick {
                    tick_queued.store(false, Ordering::Release);
                    self.flush_offsets();
                }
                self.step(i);
                if tick {
                    self.page_waits_check();
                    self.check_starts();
                    self.plugins_changed(false);
                    self.switch_idle_repls();
                    self.announce_update();
                    self.release_check(None);
                    self.plan_prs();
                    self.idle_check();
                    // computer use (design §7.3): each stop, one line in main's feed
                    for l in self.cu.poll() {
                        self.feed(crate::model::MAIN, &format!("sb computer : {}", crate::util::wire_escape(&l)));
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
                        self.feed(
                            &name,
                            "sb info : its turn was interrupted by a restart — it continues where it left off",
                        );
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
            Msg::ReplGone {
                dir,
                gen,
                reason,
                cause,
            } => {
                // a killed generation is not live anymore: its exit is
                // expected; any exit of the live one is a crash (the hub
                // never asks a REPL to quit)
                if self.gens.get(&dir) != Some(&gen) {
                    return Flow::Go;
                }
                self.start_gone(&dir, cause);
                self.gens.remove(&dir);
                self.repls.remove(&dir);
                self.gate_forget(&dir);
                self.pids.remove(&dir);
                // its writer's lock goes with it (a respawn resumes the log)
                self.recorders.remove(&dir);
                if self.switching.contains_key(&dir) && !self.switch_spawned.contains(&dir) {
                    // the reload a switch asked for: the same session on
                    // this hub's binary, the same port
                    let port = self.ports.get(&dir).copied();
                    if let Some(name) = self.agent_by_dir(&dir).map(|a| a.name.clone()) {
                        self.restored.insert(dir.clone());
                        self.switch_spawned.insert(dir.clone());
                        self.spawn_on(&name, true, None, port);
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
                if let Some(name) = self.agent_by_dir(&dir).map(|a| a.name.clone()) {
                    self.step(Input::ReplExited {
                        agent: name,
                        crashed: true,
                        reason,
                    });
                }
            }
            Msg::Page(m) => self.page_msg(m),
            Msg::ClientNew { id, stream } => self.client_hello(id, stream),
            Msg::ClientLine { id, v } => self.client_line(id, v),
            Msg::ClientGone { id } => {
                if self.clients.remove(&id).is_some() {
                    self.step(Input::ClientGone { client: id });
                }
            }
            Msg::AgentNew { token, stream, v } => self.agent_request(token, stream, v),
            Msg::XHub { token, stream, v } => self.xhub_op(token, stream, v),
            Msg::Version { mut stream, v } => {
                let from = v.get("from").and_then(|x| x.as_str()).unwrap_or("");
                let what = v.get("do").and_then(|x| x.as_str()).unwrap_or("");
                match version_allowed(from, what) {
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
                let kind = if kind == "warn" { "warn" } else { "info" };
                self.feed(MAIN, &format!("sb {} : {}", kind, wire_escape(&text)));
                self.broadcast_versions();
            }
            Msg::Release { client, v } => self.release_event(client, v),
            Msg::Update(v) => self.update_event(v),
            Msg::Changes { name, v } => self.on_changes(name, v),
            Msg::RoutePick { rid, pick } => self.route_picked(rid, pick),
            Msg::ToClient { id, v } => {
                if let Some(c) = self.clients.get_mut(&id) {
                    write_json(c, &v);
                }
            }
            Msg::RoleLine { dir, key, line } => self.step(Input::RoleLine { dir, key, line }),
            Msg::GateChecked { dir, n, req, out } => self.on_checked(&dir, &n, *req, out),
            Msg::BuildEnded { rev } => {
                self.building.remove(&rev);
                self.broadcast_versions();
            }
            Msg::Land { line } => {
                if let Some((kind, text)) = line {
                    self.feed(MAIN, &format!("sb {} : {}", kind, wire_escape(&text)));
                    // a land moved main or a feature: the features' facts again
                    self.refresh_features();
                    if kind == "landed" {
                        self.merged_all();
                    }
                }
                let snap = self.snapshot();
                self.broadcast(&snap);
            }
            Msg::Shutdown { keep } => return Flow::Stop { keep },
        }
        Flow::Go
    }
}
