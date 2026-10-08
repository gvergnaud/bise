//! The typed commands' arms (architect review 10: moved out of
//! `daemon/proto.rs`, no behavior change): what each `HubCmd` does once
//! `proto_cmd` decoded it, checked hello and its project. Each reaches
//! the same paths as the TUI's ops; a refusal is its `error`.

use super::*;
use bise_proto::hub::Mode;
use bise_proto::thread::Line;
use crate::proto_view::slash::Slash;
use crate::router::UserCmd;

/// Entries in a `thread` page when the client gives no limit, and at most.
const LIMIT: usize = 40;
const MAX_LIMIT: usize = 200;
/// Transcript lines read for one page (a long tools run folds into one
/// entry: the page may hold fewer entries than asked).
const LINES: usize = 600;

fn lines_of(raw: Vec<(usize, Option<u64>, String)>) -> Vec<Line> {
    raw.into_iter().map(|(pos, ts, l)| (pos as u64, ts.unwrap_or(0), l)).collect()
}

impl Shell {
    /// `route_correct`'s `to`, checked in the registry before sb-core is
    /// stepped (architect m_8883): "bise" keeps his words here (a cancel),
    /// a known project gives its name, anything else an error.
    fn route_to(&self, to: &str) -> Result<Option<(String, String)>, String> {
        if to == "bise" {
            return Ok(None);
        }
        let home = bise_home::Home::from_env();
        let rows = bise_home::projects::list(&home, &crate::paths::home_workspace());
        let hub = bise_home::projects::target(&rows, to, &self.project())?;
        let name = rows.iter().find(|r| r.id == hub).map_or_else(|| hub.clone(), |r| r.name.clone());
        Ok(Some((hub, name)))
    }

    /// One typed command of connection `id` (said hello, this hub's
    /// `project`): its arm.
    pub(super) fn proto_run(&mut self, id: ClientId, tag: &str, project: String, cmd: HubCmd) {
        let tag = tag.to_string();
        let known = |sh: &Shell, a: &str| sh.hub.st.agents.contains_key(a);
        match cmd {
            HubCmd::Subscribe { agent, limit, .. } => {
                let Some(dir) = self.dir_of(&agent) else { return self.proto_error(id, &tag, &format!("no agent {agent}")) };
                let path = self.transcript(&dir);
                let lines = lines_of(transcript_page(&path, transcript_len(&path) + 1, LINES));
                let facts = self.facts();
                self.setup();
                let setup = &self.setup;
                let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset };
                let (entries, before, more) = pthread::page(&lines, &ctx, limit.map_or(LIMIT, |l| (l as usize).clamp(1, MAX_LIMIT)));
                let live = Live::start(lines, &entries);
                if let Some(c) = self.proto.conns.get_mut(&id) {
                    c.subs.insert(agent.clone(), live);
                }
                self.proto_send(id, &HubEv::Thread { project, agent, entries, before, more });
            }
            HubCmd::Unsubscribe { agent, .. } => {
                if let Some(c) = self.proto.conns.get_mut(&id) {
                    c.subs.remove(&agent);
                }
            }
            HubCmd::Page { agent, before, limit, .. } => {
                let Some(dir) = self.dir_of(&agent) else { return self.proto_error(id, &tag, &format!("no agent {agent}")) };
                let lines = lines_of(transcript_page(&self.transcript(&dir), before as usize, LINES));
                let facts = self.facts();
                self.setup();
                let setup = &self.setup;
                let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset };
                let (entries, before, more) = pthread::page(&lines, &ctx, limit.map_or(LIMIT, |l| (l as usize).clamp(1, MAX_LIMIT)));
                self.proto_send(id, &HubEv::Thread { project, agent, entries, before, more });
            }
            HubCmd::Send { agent, text, mode, context, files, cid, .. } => {
                if !known(self, &agent) {
                    return self.proto_error_cid(id, &tag, &format!("no agent {agent}"), cid, cid.map(|_| "refused"));
                }
                // queued: sb-core holds it until the agent's turn ends
                // (amb-hub 7dcb56ab), never the client. A command never
                // waits: the same rule as core.rs user_input's notice
                // ("commands run now, not queued"), early and typed here;
                // change both together
                let queued = mode == Mode::Queued;
                if queued && text.trim_start().starts_with('/') {
                    return self.proto_error_cid(id, &tag, "a command can't wait in the queue: send it now", cid, cid.map(|_| "refused"));
                }
                // S9: the fn context as a field of the input op, never
                // rendered here (the input arm's one render)
                // item H: his files as a field too, rendered by the same input arm
                // G: its notices (an agent gone: BISE-86's undelivered
                // line) come back with its cid
                let op = json!({"op": "input", "focus": agent, "text": text, "queued": queued, "context": context, "files": files});
                self.stepping(id, &tag, cid, |sh| sh.client_line(id, op));
            }
            HubCmd::Answer { card, reply, .. } => {
                if !self.hub.st.open_cards().any(|c| c.id == card) {
                    return self.proto_error(id, &tag, &format!("card {card} isn't open"));
                }
                // the reply as he wrote it, never read as a command (architect
                // m_8366): the TUI's /answer handler with its fields
                let text = reply.trim().to_string();
                if text.is_empty() {
                    return self.proto_error(id, &tag, "an answer needs words or an option's number");
                }
                self.step_typed(id, &tag, Input::UserCmd { client: id, focus: MAIN.into(), cmd: UserCmd::Answer { card, text } });
            }
            // the TUI's /close handler with its field (Input::UserCmd, no
            // text; step_typed: a card already gone comes back as error)
            HubCmd::Close { card, .. } => {
                if !self.hub.st.open_cards().any(|c| c.id == card) {
                    return self.proto_error(id, &tag, &format!("card {card} isn't open"));
                }
                self.step_typed(id, &tag, Input::UserCmd { client: id, focus: MAIN.into(), cmd: UserCmd::Close { card } });
            }
            // his answer to the hub's yes/no question: sb-core's own
            // confirm path; a no's line comes back as a notice, not an
            // error (an unknown or answered id: sb-core says nothing)
            HubCmd::Confirm { id: cid, yes, .. } => {
                self.stepping(id, &tag, None, |sh| {
                    if let Some(s) = sh.proto.stepping.as_mut() {
                        s.as_notice = true;
                    }
                    sh.step(Input::ClientConfirm { client: id, id: cid, yes });
                });
            }
            // bar V8/W21: the TUI's `approvals` op, typed (his command:
            // docs/issues/16 keeps agents' connections from it). A mode
            // sets it and every connection hears it (with flash); none
            // answers this connection; another word is refused
            HubCmd::Approvals { mode: None, .. } => {
                let ev = self.approvals_ev(false);
                let ev = self.proto_approvals(&ev);
                self.proto_send(id, &ev);
            }
            HubCmd::Approvals { mode: Some(word), .. } => {
                let m = match word.as_str() {
                    "toggle" => Some(self.gates.mode.other()),
                    w => crate::approvals::Mode::parse(w),
                };
                let Some(m) = m else { return self.proto_error(id, &tag, &format!("approvals: {word} is not yolo, auto or toggle")) };
                self.set_mode(m);
                let ev = self.approvals_ev(true);
                self.broadcast(&ev);
            }
            // the rule's fields back to gate.rs's remove_rule (rule_of_json):
            // gone already is this error; done, every connection hears it
            HubCmd::RemoveRule { rule, .. } => {
                let v = serde_json::to_value(&rule).unwrap_or(Value::Null);
                if let Err(e) = self.remove_rule(&v) {
                    self.proto_error(id, &tag, &e);
                }
            }
            HubCmd::Stop { agent, .. } => {
                if !known(self, &agent) {
                    return self.proto_error(id, &tag, &format!("no agent {agent}"));
                }
                self.client_line(id, json!({"op": "interrupt", "agent": agent}));
            }
            HubCmd::Archive { agent, force, .. } => {
                if !known(self, &agent) {
                    return self.proto_error(id, &tag, &format!("no agent {agent}"));
                }
                self.step_typed(id, &tag, Input::UserCmd { client: id, focus: MAIN.into(), cmd: UserCmd::Drop { name: Some(agent), force } });
            }
            HubCmd::Unarchive { agent, .. } => {
                if !known(self, &agent) {
                    return self.proto_error(id, &tag, &format!("no agent {agent}"));
                }
                self.step_typed(id, &tag, Input::UserCmd { client: id, focus: MAIN.into(), cmd: UserCmd::Restore { name: agent } });
            }
            // the TUI's own `seen`: the clock moves, the list comes again
            HubCmd::ArtifactsSeen { .. } => self.artifacts_op(id, &json!({"do": "seen"})),
            // git in a thread (art.rs), the typed answer to this client
            HubCmd::Diff { project, agent, commit } => {
                if !known(self, &agent) {
                    return self.proto_error(id, &tag, &format!("no agent {agent}"));
                }
                if commit.as_deref().is_some_and(|c| !crate::proto_view::is_sha(c)) {
                    return self.proto_error(id, &tag, "a commit is its sha (4 to 40 hex digits)");
                }
                self.diff_typed(id, project, agent, commit);
            }
            // git in a thread (worktrees.rs), to this client
            HubCmd::Worktrees { .. } => self.worktrees_typed(vec![id]),
            // jobs and lsof in a thread (dev_servers.rs), to this client
            HubCmd::DevServers { .. } => self.dev_servers_typed(vec![id]),
            // git and main's transcript in a thread (merged.rs)
            HubCmd::Merged { .. } => self.merged_typed(vec![id]),
            // the registry and the facts the threads read: no git here
            HubCmd::Features { .. } => {
                let feats = self.features_ev();
                self.proto_send(id, &feats);
            }
            // the forge poll's last snapshots (Hub.prs): no network here
            HubCmd::Prs { .. } => {
                let prs = self.prs_ev();
                self.proto_send(id, &prs);
            }
            // the live timers, from the hub's state (no line read)
            HubCmd::Scheduled { .. } => {
                let sched = self.scheduled_ev();
                self.proto_send(id, &sched);
            }
            // his stop, the TUI's /scheduled stop (its `every_stop` op):
            // the agent hears it, its line says `stopped by you`
            HubCmd::ScheduledStop { id: timer, .. } => {
                if !self.hub.timers().map.contains_key(&timer) {
                    return self.proto_error(id, &tag, &format!("no scheduled task #{timer}"));
                }
                self.step_typed(id, &tag, Input::EveryStop { id: timer, why: String::new() });
            }
            // the keys and config read again (a login made elsewhere)
            HubCmd::Models { .. } => {
                let models = self.models_ev();
                self.proto_send(id, &models);
            }
            // bar A.1 and A.5: the TUI's /new, /rename, /model, /reasoning,
            // their fields straight to the same handlers (Input::UserCmd,
            // architect m_10331): a refusal comes back as this error, the
            // change itself in `agents`
            HubCmd::New { name, brief, worktree, with_changes, .. } => {
                let cmd = UserCmd::New { name, brief, worktree, with_changes };
                self.step_typed(id, &tag, Input::UserCmd { client: id, focus: MAIN.into(), cmd });
            }
            HubCmd::Rename { agent, to, .. } => {
                if !known(self, &agent) {
                    return self.proto_error(id, &tag, &format!("no agent {agent}"));
                }
                let cmd = UserCmd::Rename { name: agent, new_name: to };
                self.step_typed(id, &tag, Input::UserCmd { client: id, focus: MAIN.into(), cmd });
            }
            HubCmd::Model { agent, model, default, .. } => {
                if !known(self, &agent) {
                    return self.proto_error(id, &tag, &format!("no agent {agent}"));
                }
                let cmd = UserCmd::Model { model: Some(model), default };
                self.step_typed(id, &tag, Input::UserCmd { client: id, focus: agent, cmd });
            }
            HubCmd::Effort { agent, effort, .. } => {
                if !known(self, &agent) {
                    return self.proto_error(id, &tag, &format!("no agent {agent}"));
                }
                let cmd = UserCmd::Reasoning { effort: Some(effort.to_ascii_lowercase()) };
                self.step_typed(id, &tag, Input::UserCmd { client: id, focus: agent, cmd });
            }
            // the 2 s hold (sb-core's routing hold): a route that ended
            // already gets sb-core's notice, as an error (step_typed); an
            // unknown target never reaches sb-core
            HubCmd::RouteCorrect { rid, to, .. } => match self.route_to(&to) {
                Err(e) => self.proto_error(id, &tag, &e),
                Ok(None) => self.step_typed(id, &tag, Input::RouteCancel { client: Some(id), rid }),
                Ok(Some((to, name))) => self.step_typed(id, &tag, Input::RouteCorrect { client: Some(id), rid, to, name }),
            },
            HubCmd::RouteCancel { rid, .. } => self.step_typed(id, &tag, Input::RouteCancel { client: Some(id), rid }),
            // S10: sb-core refuses main, an archived or unknown agent: the
            // notice comes back as the error (step_typed)
            HubCmd::Follow { agent, on, .. } => self.step_typed(id, &tag, Input::Follow { client: Some(id), agent, on }),
            HubCmd::Slash { agent, line, cid, .. } => self.proto_slash(id, &tag, agent, &line, cid),
            HubCmd::Hello { .. } | HubCmd::Unknown { .. } => {}
        }
    }

    /// A typed `slash` line typed in `agent`'s view: the TUI's router
    /// parses it (`proto_view::slash`), the same handlers run it. A
    /// refusal is an `error` with `cid`; a success shows in what it moves,
    /// or as `agents`/`prs`/`notice` to this connection, never an error.
    fn proto_slash(&mut self, id: ClientId, tag: &str, agent: String, line: &str, cid: Option<u64>) {
        let refused = cid.map(|_| "refused");
        if self.dir_of(&agent).is_none() {
            return self.proto_error_cid(id, tag, &format!("no agent {agent}"), cid, refused);
        }
        let project = self.project();
        match proto_view::slash::route(crate::router::parse(line, &agent)) {
            Slash::Refuse(text) => self.proto_error_cid(id, tag, &text, cid, refused),
            Slash::Agents => {
                let snap = self.snapshot();
                let (agents, _) = self.proto_rows(&snap);
                self.proto_send(id, &agents);
            }
            Slash::Prs => {
                let prs = self.prs_ev();
                self.proto_send(id, &prs);
            }
            Slash::Help => self.proto_send(id, &HubEv::Notice { project, cmd: Some(tag.to_string()), text: crate::core::HELP.to_string(), cid }),
            // the TUI's /flow writer (Effect::Flow's), not a copy
            Slash::Flow(set) => {
                let (ok, text) = self.flow_cmd(set);
                if !ok {
                    return self.proto_error_cid(id, tag, &text, cid, refused);
                }
                self.proto_send(id, &HubEv::Notice { project, cmd: Some(tag.to_string()), text, cid });
                if set.is_some() {
                    let snap = self.snapshot();
                    self.broadcast(&snap);
                }
            }
            Slash::Artifacts(None) => {
                let ev = self.artifacts_ev();
                let arts = self.proto_artifacts(&ev);
                self.proto_send(id, &arts);
            }
            // the TUI's `artifacts` op's add (art_add), in this agent's view
            Slash::Artifacts(Some(target)) => {
                match self.art_add(&agent, &target, None) {
                    Ok(text) => self.proto_send(id, &HubEv::Notice { project, cmd: Some(tag.to_string()), text, cid }),
                    Err(e) => self.proto_error_cid(id, tag, &e, cid, refused),
                }
                self.artifacts_refresh(true);
            }
            // the `stop` command's path: a working agent's turn stops (the
            // TUI's /stop: idle, nothing to stop; the computer-use stop
            // waits for issue 18's ctl ops)
            Slash::Stop(name) => {
                if self.dir_of(&name).is_none() {
                    return self.proto_error_cid(id, tag, &format!("/stop: no agent named {name}"), cid, refused);
                }
                let working = self.hub.st.agents.get(&name).is_some_and(|a| a.status() == crate::model::Status::Working);
                if working {
                    // TODO(client-protocol P3): the old untyped op door, replaced by P3's typed interrupt
                    self.client_line(id, json!({"op": "interrupt", "agent": name}));
                }
            }
            // the daemon's `version` op, the TUI's (his authority: this is
            // the client socket, docs/issues/16)
            Slash::Version(bise_proto::slash::Version::Update) => self.update_op(id),
            Slash::Version(v) => {
                let (what, to) = v.op();
                let text = self.version_op(&json!({"op": "version", "do": what, "to": to}));
                self.proto_send(id, &HubEv::Notice { project, cmd: Some(tag.to_string()), text, cid });
            }
            // the `approvals` command's path (show, or switch and tell all)
            Slash::Approvals(None) => {
                let ev = self.approvals_ev(false);
                let ev = self.proto_approvals(&ev);
                self.proto_send(id, &ev);
            }
            Slash::Approvals(Some(mode)) => {
                use bise_proto::rows::ApprovalMode;
                let m = match mode {
                    ApprovalMode::Yolo => crate::approvals::Mode::Yolo,
                    ApprovalMode::Auto => crate::approvals::Mode::Auto,
                    ApprovalMode::Unknown => return self.proto_error_cid(id, tag, "/approvals: yolo or auto", cid, refused),
                };
                self.set_mode(m);
                let ev = self.approvals_ev(true);
                self.broadcast(&ev);
            }
            Slash::Step(cmd) => self.stepping(id, tag, cid, |sh| sh.step(Input::UserCmd { client: id, focus: agent, cmd })),
        }
    }
}
