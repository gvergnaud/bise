//! The typed commands' arms (architect review 10: moved out of
//! `daemon/proto.rs`, no behavior change): what each `HubCmd` does once
//! `proto_cmd` decoded it, checked hello and its project. Each reaches
//! the same paths as the TUI's ops; a refusal is its `error`. Since
//! client-protocol step 3 every op the terminal sends has its command
//! here (scheduled_run, artifacts_add, branches, focus, the version
//! actions, release_plan/run, diff by branch/pr/range), each calling the
//! op's own handler, and `slash` takes any line he typed: his words and
//! `@route`s go `send`'s path (`proto_input`).

use super::*;
use bise_proto::hub::SendOpts;
use bise_proto::slash::Version;
use bise_proto::thread::Line;
use crate::proto_view::slash::Slash;
use crate::proto_view::DiffAsk;
use crate::router::UserCmd;

/// Entries in a `thread` page when the client gives no limit, and at most.
const LIMIT: usize = 40;
const MAX_LIMIT: usize = 200;
/// Transcript lines read for one page (a long tools run folds into one
/// entry: the page may hold fewer entries than asked).
const LINES: usize = 600;
/// A queued line that is a command (core.rs user_input's rule).
const QUEUED_COMMAND: &str = "a command can't wait in the queue: send it now";

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
    pub(in crate::daemon) fn proto_run(&mut self, id: ClientId, tag: &str, project: String, cmd: HubCmd) {
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
                let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset, attached: &crate::attached::split };
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
                let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset, attached: &crate::attached::split };
                let (entries, before, more) = pthread::page(&lines, &ctx, limit.map_or(LIMIT, |l| (l as usize).clamp(1, MAX_LIMIT)));
                self.proto_send(id, &HubEv::Thread { project, agent, entries, before, more });
            }
            HubCmd::Send { agent, text, opts, cid, .. } => self.proto_input(id, &tag, agent, text, opts, cid),
            HubCmd::Answer { card, reply, files, .. } => {
                if !self.hub.st.open_cards().any(|c| c.id == card) {
                    return self.proto_error(id, &tag, &format!("card {card} isn't open"));
                }
                // the reply as he wrote it, never read as a command (architect
                // m_8366): the TUI's /answer handler with its fields
                let text = reply.trim().to_string();
                if text.is_empty() && files.is_empty() {
                    return self.proto_error(id, &tag, "an answer needs words or an option's number");
                }
                // R41 (architect m_13737): what he pasted with it, rendered
                // after his words by the send path's one render, so an
                // image reaches the asking agent's model as an image
                let text = if files.is_empty() { text } else { super::fn_context::render_files(&text, &files) };
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
            // bar V8/W21: shift+tab and `/approvals [yolo|auto]` (his command:
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
                self.step(Input::ClientInterrupt { client: id, agent });
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
            HubCmd::ArtifactsSeen { at_ms, .. } => self.artifacts_seen(at_ms),
            // git in a thread (art.rs), the typed answer to this client
            // the `diff` op's asks (agent, branch, pr, range), checked here
            HubCmd::Diff { project, agent, commit, branch, pr, range, req } => {
                let ask = DiffAsk { agent, commit, branch, pr, range, req };
                if let Err(e) = ask.check() {
                    return self.proto_error(id, &tag, &e);
                }
                if let (Some(a), None) = (&ask.agent, &ask.range) {
                    if !known(self, a) {
                        return self.proto_error(id, &tag, &format!("no agent {a}"));
                    }
                }
                self.diff_typed(id, project, ask);
            }
            // a tool row opened: its whole output from the session log
            // (tool_out.rs), files read on a thread, to this client
            HubCmd::ToolOut { project, agent, pos } => {
                let Some(dir) = self.dir_of(&agent) else { return self.proto_error(id, &tag, &format!("no agent {agent}")) };
                let (transcript, adir) = (self.transcript(&dir), self.opts.paths.agent_dir(&dir));
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let home = bise_home::Home::from_env();
                    let (sessions, blobs) = (home.sessions_dir(), home.blobs_dir());
                    let v = match super::tool_out::answer(&transcript, &adir, pos, &super::tool_out::Logs { sessions: &sessions, blobs: &blobs }) {
                        Ok((out, cut, total)) => HubEv::ToolOut { project, agent, pos, out, cut, total }.to_value(),
                        Err(text) => HubEv::Error { project: Some(project), cmd: Some(tag), text, cid: None, reason: None, kind: None }.to_value(),
                    };
                    let _ = tx.send(Msg::ToClient { id, v });
                });
            }
            // git in a thread (art.rs, the `branches` op's scan), to this client
            HubCmd::Branches { .. } => self.branches_scan(id, true),
            // git in a thread (worktrees.rs), to this client
            HubCmd::Worktrees { .. } => self.worktrees_typed(Typed::Answer(id)),
            // jobs and lsof in a thread (dev_servers.rs), to this client
            HubCmd::DevServers { .. } => self.dev_servers_typed(Typed::Answer(id)),
            // git and main's transcript in a thread (merged.rs)
            HubCmd::Merged { .. } => self.merged_typed(Typed::Answer(id)),
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
            // his stop, the TUI's /scheduled x, the desktop's 'stop watching':
            // the agent hears it, its line says `stopped by you`
            HubCmd::ScheduledStop { id: timer, .. } => {
                if !self.hub.timers().map.contains_key(&timer) {
                    return self.proto_error(id, &tag, &format!("no scheduled task #{timer}"));
                }
                self.step_typed(id, &tag, Input::EveryStop { id: timer, why: String::new() });
            }
            // his run now, the TUI's /scheduled r
            HubCmd::ScheduledRun { id: timer, .. } => {
                if !self.hub.timers().map.contains_key(&timer) {
                    return self.proto_error(id, &tag, &format!("no scheduled task #{timer}"));
                }
                self.step_typed(id, &tag, Input::EveryRun { id: timer });
            }
            // the `artifacts` op's add (art_add): its words are the result
            HubCmd::ArtifactsAdd { agent, target, title, .. } => {
                let title = title.filter(|t| !t.trim().is_empty());
                match self.art_add(&agent, &target, title) {
                    Ok(text) => self.proto_send(id, &HubEv::Notice { project, cmd: Some(tag.clone()), text, cid: None }),
                    Err(e) => self.proto_error(id, &tag, &e),
                }
                self.artifacts_refresh(true);
            }
            // client/focus: who he is looking at (the TUI's feed in view)
            HubCmd::Focus { focus, .. } => self.step(Input::ClientFocus { client: id, focus }),
            // `/version`'s picker
            HubCmd::Versions { .. } => {
                let mut v = self.version_items();
                v["project"] = json!(project);
                match HubEv::from_value(v) {
                    Ok(ev) => self.proto_send(id, &ev),
                    Err(e) => self.proto_error(id, &tag, &e),
                }
            }
            // `/version`, `/restart`, `/update`: the `slash` arm's path
            HubCmd::VersionInfo { .. } => self.proto_version(id, &tag, Version::List, None),
            HubCmd::VersionSwitch { to, .. } => self.proto_version(id, &tag, Version::Switch(to), None),
            HubCmd::VersionRollback { .. } => self.proto_version(id, &tag, Version::Rollback, None),
            HubCmd::VersionRestart { to, .. } => self.proto_version(id, &tag, Version::Restart(to.unwrap_or_default()), None),
            HubCmd::VersionUpdate { .. } => self.proto_version(id, &tag, Version::Update, None),
            // `/release-bise`'s start (release.rs): a plan's answer comes
            // from its thread (release_event), a run's steps go to all
            HubCmd::ReleasePlan { dry, .. } => {
                if let Err(e) = self.release_start(id, "plan", "", "", dry) {
                    self.proto_error(id, &tag, &e);
                }
            }
            HubCmd::ReleaseRun { tag: rtag, commit, dry, .. } => {
                if let Err(e) = self.release_start(id, "run", &rtag, &commit, dry) {
                    self.proto_error(id, &tag, &e);
                }
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
            // a command: the router's parse; his words or an @route:
            // `send`'s path (architect m_13089 change 3)
            HubCmd::Slash { agent, line, cid, opts, .. } => {
                if !line.trim_start().starts_with('/') {
                    return self.proto_input(id, &tag, agent, line, opts, cid);
                }
                if opts.queued() {
                    return self.proto_error_cid(id, &tag, QUEUED_COMMAND, cid, cid.map(|_| "refused"));
                }
                self.proto_slash(id, &tag, agent, &line, cid)
            }
            HubCmd::Hello { .. } | HubCmd::Unknown { .. } => {}
        }
    }

    /// His words to `agent` (`send`, and `slash`'s line that isn't a
    /// command): the `input` op's path with its fields, its notices back
    /// as errors with `cid`.
    fn proto_input(&mut self, id: ClientId, tag: &str, agent: String, text: String, opts: SendOpts, cid: Option<u64>) {
        if !self.hub.st.agents.contains_key(&agent) {
            return self.proto_error_cid(id, tag, &format!("no agent {agent}"), cid, cid.map(|_| "refused"));
        }
        // queued: sb-core holds it until the agent's turn ends
        // (amb-hub 7dcb56ab), never the client. A command never
        // waits: the same rule as core.rs user_input's notice
        // ("commands run now, not queued"), early and typed here;
        // change both together
        let queued = opts.queued();
        if queued && text.trim_start().starts_with('/') {
            return self.proto_error_cid(id, tag, QUEUED_COMMAND, cid, cid.map(|_| "refused"));
        }
        // S9: the fn context as a field of the input op, never
        // rendered here (the input arm's one render)
        // item H: his files as a field too, rendered by the same input arm
        // G: its notices (an agent gone: BISE-86's undelivered
        // line) come back with its cid
        let mut op = json!({"op": "input", "focus": agent, "text": text, "queued": queued, "context": opts.context, "files": opts.files});
        if opts.voice {
            op["voice"] = json!(true);
        }
        if let Some(via) = opts.via {
            op["via"] = json!(via);
        }
        self.stepping(id, tag, cid, |sh| sh.client_line(id, op));
    }

    /// `/version`, `/restart`, `/update` (a `slash` line or their own
    /// methods): the daemon's `version_op` (his authority: this
    /// is the client socket, docs/issues/16); its words as a `notice` (the
    /// method's result), `/update`'s from `update_op`.
    fn proto_version(&mut self, id: ClientId, tag: &str, v: Version, cid: Option<u64>) {
        if v == Version::Update {
            return self.update_op(id);
        }
        let (what, to) = v.op();
        let text = self.version_op(&json!({"op": "version", "do": what, "to": to}));
        let project = self.project();
        self.proto_send(id, &HubEv::Notice { project, cmd: Some(tag.to_string()), text, cid });
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
                    self.state_now();
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
                let Some(dir) = self.dir_of(&name) else {
                    return self.proto_error_cid(id, tag, &format!("/stop: no agent named {name}"), cid, refused);
                };
                let working = self.hub.st.agents.get(&name).is_some_and(|a| a.status() == crate::model::Status::Working);
                if working {
                    self.step(Input::ClientInterrupt { client: id, agent: name });
                }
                self.computer_use_stop(&dir);
            }
            // the daemon's `version` op, the TUI's (his authority: this is
            // the client socket, docs/issues/16)
            Slash::Version(v) => self.proto_version(id, tag, v, cid),
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

    /// /stop's computer-use half (computer-use-design §7.3): the agent in
    /// `dir` lets go of Chrome and its apps, through the one call the
    /// TUI's /stop makes (`bise_computer_use::cli::stop_agent`, by its key:
    /// this hub's tag id, the hash of its own socket path, and `dir`), on a
    /// thread, never on the hub's loop. The window hears nothing of it; a
    /// refusal (a hub that is an agent's process) is one log line.
    pub(super) fn computer_use_stop(&self, dir: &str) {
        let key = bise_computer_use::who::key(&self.opts.paths.proc_hub(), dir);
        let paths = self.opts.paths.clone();
        std::thread::spawn(move || {
            if let bise_computer_use::cli::StopOutcome::Refused(why) = bise_computer_use::cli::stop_agent(&bise_computer_use::paths::Paths::from_env(), &key) {
                super::super::log_line(&paths, &format!("/stop {key}: computer use refused: {why}"));
            }
        });
    }
}
