//! The typed commands' arms (architect review 10: moved out of
//! `daemon/proto.rs`, no behavior change): what each `HubCmd` does once
//! `proto_cmd` decoded it, checked hello and its project. Each reaches
//! the same paths as the TUI's ops; a refusal is its `error`. Since
//! client-protocol step 3 every op the terminal sends has its command
//! here (scheduled_run, artifacts_add, branches, focus, the version
//! actions, release_plan/run, diff by branch/pr/range), each calling the
//! op's own handler, and `slash` takes any line he typed: his words and
//! `@route`s go `send`'s path (`proto_input`).

use super::super::capsule::Took;
use super::super::page_cards::Page;
use super::super::processes::Ask;
use super::*;
use bise_proto::hub::SendOpts;
use bise_proto::log::{BodyOf, WHOLE_CAP};
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
/// A `slash` line whose parts aren't its words (msg-parts): what the
/// model would read and what clients draw would differ.
const PARTS_NOT_THE_LINE: &str = "this line isn't its parts' text: update bise";

/// A send's text beside its parts that isn't their render: refused with
/// its cid (words::composer::refused_text_beside_parts), the model and
/// his thread would read two messages. Only a window before
/// desktop-v2026.10.1 (8ed6bf275) sent his words with the chips' labels
/// beside the parts; the log was their shim until issue 31.
fn text_beside_parts(text: &str, parts: &[bise_proto::parts::Part]) -> bool {
    !text.is_empty() && text != bise_proto::parts::render(parts)
}

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
                let lines = lines_of(self.page_lines(&path, None, LINES));
                let facts = self.facts();
                self.setup();
                let setup = &self.setup;
                let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset, attached: &crate::attached::split };
                // the newest page: nothing after it yet
                let (mut entries, before, more) = pthread::page(&lines, &[], &ctx, limit.map_or(LIMIT, |l| (l as usize).clamp(1, MAX_LIMIT)));
                let live = Live::start(lines, &entries);
                self.expire_views(&dir, &mut entries);
                if let Some(c) = self.proto.conns.get_mut(&id) {
                    c.subs.insert(agent.clone(), live);
                }
                // its answer streaming in, if one is: the whole text once,
                // at off 0; the deltas follow
                let partial = self.partials.get(&agent).map(|p| (p.call.clone(), p.text())).filter(|(_, t)| !t.is_empty());
                self.proto_send(id, &HubEv::Thread { project: project.clone(), agent: agent.clone(), entries, before, more });
                if let Some((call, text)) = partial {
                    self.proto_send(id, &HubEv::Partial { project, agent, call, off: 0, text });
                }
            }
            HubCmd::Unsubscribe { agent, .. } => {
                if let Some(c) = self.proto.conns.get_mut(&id) {
                    c.subs.remove(&agent);
                }
            }
            HubCmd::Page { agent, before, limit, .. } => {
                let Some(dir) = self.dir_of(&agent) else { return self.proto_error(id, &tag, &format!("no agent {agent}")) };
                let path = self.transcript(&dir);
                let lines = lines_of(self.page_lines(&path, Some(before as usize), LINES));
                // its last turn's steered receipts may sit after it
                let ahead = lines_of(self.ahead_lines(&path, before as usize, LINES));
                let facts = self.facts();
                self.setup();
                let setup = &self.setup;
                let ctx = Ctx { open_cards: &facts.open, page: &|p: &str| facts.page(p), provider: &|i: &str, k: &str| provider_name(setup, i, k), width: &width, offset: &offset, attached: &crate::attached::split };
                let (mut entries, before, more) = pthread::page(&lines, &ahead, &ctx, limit.map_or(LIMIT, |l| (l as usize).clamp(1, MAX_LIMIT)));
                self.expire_views(&dir, &mut entries);
                self.proto_send(id, &HubEv::Thread { project, agent, entries, before, more });
            }
            HubCmd::Find { agent, needle, rid, .. } => self.proto_find(id, &tag, project, agent, needle, rid),
            HubCmd::Send { agent, text, opts, cid, parts, .. } if parts.is_empty() => self.proto_input(id, &tag, agent, text, opts, cid, String::new()),
            // msg-parts: the model's text from his parts, the parts kept
            // on his message (a part this hub doesn't know: refused)
            HubCmd::Send { agent, text, opts, cid, parts, .. } => {
                if text_beside_parts(&text, &parts) {
                    return self.proto_error_cid(id, &tag, bise_proto::words::composer::refused_text_beside_parts, cid, cid.map(|_| "refused"));
                }
                match crate::parts_in::said(&parts, &crate::parts_in::Door { store: &bise_home::Home::from_env().images_dir(), put: &crate::daemon::fn_context::store_image, exists: &crate::daemon::fn_context::file_exists, known: &self.artifact_known() }) {
                    Ok(s) => {
                        self.artifact_notes_said(&s.parts);
                        self.proto_input(id, &tag, agent, s.text, opts, cid, s.parts)
                    }
                    Err(e) => self.proto_error_cid(id, &tag, &e, cid, cid.map(|_| "refused")),
                }
            }
            HubCmd::Answer { card, reply, parts, .. } => {
                let (reply, parts) = match parts.is_empty() {
                    true => (reply, String::new()),
                    false => match crate::parts_in::said(&parts, &crate::parts_in::Door { store: &bise_home::Home::from_env().images_dir(), put: &crate::daemon::fn_context::store_image, exists: &crate::daemon::fn_context::file_exists, known: &self.artifact_known() }) {
                        Ok(s) => {
                            self.artifact_notes_said(&s.parts);
                            (s.text, s.parts)
                        }
                        Err(e) => return self.proto_error(id, &tag, &e),
                    },
                };
                // a page's card first (daemon/page_cards.rs, as the input
                // op had it): a step ticked, a drafts or replaced batch
                // card answered there; a question's digit becomes its
                // option's words, and the page hears it (P3b). His typed
                // parts go with it, his words alone in the line: the pick
                // note keeps his structure, never the rendered markers
                // (card-image-parts, architect m_35268)
                let typed: Vec<bise_proto::parts::Part> = serde_json::from_str(&parts).unwrap_or_default();
                let words = bise_proto::parts::words(&typed);
                let line = if typed.is_empty() || words.trim().is_empty() { reply.trim() } else { words.trim() };
                let reply = match self.page_first(id, &format!("/answer {card} {line}"), &typed) {
                    Page::Done => return self.page_answers(),
                    Page::Reply(l) => l.splitn(3, ' ').nth(2).unwrap_or_default().to_string(),
                    Page::Not => reply,
                };
                // a card that isn't open is sb-core's to decide (withdrawn-
                // answer, architect m_30039): one main withdrew while he
                // answered reaches main, any other is refused with
                // "notification #N isn't open" (core.bend answer.late)
                // the reply as he wrote it, never read as a command (architect
                // m_8366): the TUI's /answer handler with its fields
                let text = reply.trim().to_string();
                if text.is_empty() {
                    return self.proto_error(id, &tag, "an answer needs words or an option's number");
                }
                // inbox picks (3): his answer as sb-core wrote its route line
                // comes back as the result; an older sb-core's: none ({})
                self.hub.answered = None;
                self.step_typed(id, &tag, Input::UserCmd { client: id, focus: MAIN.into(), cmd: UserCmd::Answer { card, text, parts } });
                if let Some(a) = self.hub.answered.take() {
                    let r = bise_proto::rpc::AnswerResult { answer: Some(a) };
                    self.rpc_set_result(serde_json::to_value(r).unwrap_or_default());
                }
                self.page_answers();
            }
            // the TUI's /close handler with its field (Input::UserCmd, no
            // text; step_typed: a card already gone comes back as error)
            HubCmd::Close { card, .. } => {
                if !self.hub.st.open_cards().any(|c| c.id == card) {
                    return self.proto_error(id, &tag, &format!("notification #{card} isn't open"));
                }
                self.step_typed(id, &tag, Input::UserCmd { client: id, focus: MAIN.into(), cmd: UserCmd::Close { card } });
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
            // diff/read's asks (agent, branch, pr, range, artifact), checked here
            HubCmd::Diff { project, agent, commit, branch, pr, range, req, artifact, version } => {
                let ask = DiffAsk { agent, commit, branch, pr, range, req, artifact, version };
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
                    let _ = tx.send(Msg::Typed { to: super::super::rpc::Typed::Answer(id), v });
                });
            }
            // the window's /log: a page of the agent's raw session (log.rs,
            // bise_session::history's rows, redacted there), read on a
            // thread, to this client only (issue 16: his raw sessions)
            HubCmd::Log { project, agent, before, find } => {
                let Some(dir) = self.dir_of(&agent) else { return self.proto_error(id, &tag, &format!("no agent {agent}")) };
                let adir = self.opts.paths.agent_dir(&dir);
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let home = bise_home::Home::from_env();
                    let redact = bise_session::Redactor::from_home(&home.auth_file(), &home.env_files());
                    let got = super::super::session_log::session_of(&adir)
                        .ok_or_else(|| format!("{agent} has no session log yet"))
                        .and_then(|s| super::log::answer(&home.sessions_dir().join(s), &home.blobs_dir(), &redact, before, find.as_deref()));
                    let v = match got {
                        Ok((items, more, hits)) => HubEv::Log { project, agent, items, more, hits }.to_value(),
                        Err(text) => HubEv::Error { project: Some(project), cmd: Some(tag), text, cid: None, reason: None, kind: None }.to_value(),
                    };
                    let _ = tx.send(Msg::Typed { to: super::super::rpc::Typed::Answer(id), v });
                });
            }
            // /debug completion: a request of the agent's ring as rows
            // (completion.rs), another family's through the runtime's
            // render; read on a thread, to this client only (issue 16)
            HubCmd::Completion(mut ask) => {
                if ask.project.is_empty() {
                    ask.project = self.project();
                }
                let Some(dir) = self.dir_of(&ask.agent) else { return self.proto_error(id, &tag, &format!("no agent {}", ask.agent)) };
                let req_dir = self.opts.paths.agent_dir(&dir).join("requests");
                let setup = self.setup().clone();
                let models: Vec<String> = bise_catalog::picks::picks(&setup, &|_| true).into_iter().map(|p| p.value).filter(|v| v.contains('/')).collect();
                let (repl, root) = (self.opts.repl_bin.clone(), self.opts.app_root.clone());
                let keys = self.opts.spawn_env.map(|f| f()).unwrap_or_default();
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let home = bise_home::Home::from_env();
                    let redact = bise_session::Redactor::from_home(&home.auth_file(), &home.env_files());
                    let render = |wire: &std::path::Path, model: &str| super::completion::render(&repl, &root, wire, model, &keys);
                    let project = ask.project.clone();
                    let v = match super::completion::answer(&ask, &req_dir, &home.images_dir(), &models, &render, &redact) {
                        Ok(c) => HubEv::Completion(Box::new(c)).to_value(),
                        Err(text) => HubEv::Error { project: Some(project), cmd: Some(tag), text, cid: None, reason: None, kind: None }.to_value(),
                    };
                    let _ = tx.send(Msg::Typed { to: super::super::rpc::Typed::Answer(id), v });
                });
            }
            // /debug's 'show more' (debug-more): one cut body whole, from
            // the session log (a history row) or the ring (a completion
            // block); read on a thread, to this client only (issue 16)
            HubCmd::LogBody { project, agent, of } => {
                let Some(dir) = self.dir_of(&agent) else { return self.proto_error(id, &tag, &format!("no agent {agent}")) };
                let adir = self.opts.paths.agent_dir(&dir);
                let ring = (adir.join("requests"), self.opts.repl_bin.clone(), self.opts.app_root.clone(), self.opts.spawn_env.map(|f| f()).unwrap_or_default());
                let models: Vec<String> = match &of {
                    BodyOf::Block { provider: Some(_), .. } => bise_catalog::picks::picks(self.setup(), &|_| true).into_iter().map(|p| p.value).filter(|v| v.contains('/')).collect(),
                    _ => Vec::new(),
                };
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let home = bise_home::Home::from_env();
                    let redact = bise_session::Redactor::from_home(&home.auth_file(), &home.env_files());
                    let got = match &of {
                        BodyOf::Row { pos } => super::super::session_log::session_of(&adir)
                            .ok_or_else(|| format!("{agent} has no session log yet"))
                            .and_then(|s| super::log::whole(&home.sessions_dir().join(s), &home.blobs_dir(), &redact, *pos, WHOLE_CAP)),
                        BodyOf::Block { slot, req, provider, raw, row, block } => {
                            let (req_dir, repl, root, keys) = &ring;
                            let render = |wire: &std::path::Path, model: &str| super::completion::render(repl, root, wire, model, keys);
                            let ask = super::completion::BlockAsk { slot: *slot, req: *req, provider: provider.as_deref(), raw: *raw, row: *row, block: *block };
                            super::completion::block(&ask, req_dir, &home.images_dir(), &models, &render, &redact, WHOLE_CAP)
                        }
                        BodyOf::Unknown => Err("this bise can't read that body: update bise".into()),
                    };
                    let v = match got {
                        Ok((text, cut, total)) => HubEv::LogBody { project, agent, of, text, cut, total }.to_value(),
                        Err(text) => HubEv::Error { project: Some(project), cmd: Some(tag), text, cid: None, reason: None, kind: None }.to_value(),
                    };
                    let _ = tx.send(Msg::Typed { to: super::super::rpc::Typed::Answer(id), v });
                });
            }
            // his queued message taken back (issue 16: this socket's
            // clients only, never an agent's op): sb-core's unqueue decides
            // (Rejected{"taken_back"}, or its notice as this command's
            // error); taken, its words go back to this connection
            HubCmd::QueuedTake { project, agent, id: msg } => {
                use crate::model::MsgState;
                let queued = |sh: &Self| matches!(sh.hub.st.msg_state().get(&msg), Some(MsgState::Queued { .. }));
                let was = queued(self);
                let (text, parts) = self.hub.st.msgs().get(&msg).map(|m| (m.text.clone(), m.parts_list())).unwrap_or_default();
                self.step_typed(id, &tag, Input::ClientUnqueue { client: id, id: msg });
                let taken = matches!(self.hub.st.msg_state().get(&msg), Some(MsgState::Rejected { error }) if error == "taken_back");
                if was && taken {
                    self.proto_send(id, &HubEv::QueuedTaken { project, agent, id: msg, text, parts });
                }
            }
            // his queued message said now (the window's "send now"; this
            // socket's clients only): sb-core's steer_queued decides; a
            // refusal is its notice as this command's error
            HubCmd::QueuedSteer { id: msg, .. } => self.step_typed(id, &tag, Input::ClientSteerQueued { client: id, id: msg }),
            // git in a thread (art.rs, the `branches` op's scan), to this client
            HubCmd::Branches { .. } => self.branches_scan(id),
            // git in a thread (worktrees.rs), to this client
            HubCmd::Worktrees { .. } => self.worktrees_typed(Typed::Answer(id)),
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
            // the inbox's history (daemon/answers.rs): memory only
            HubCmd::InboxHistory { q, .. } => {
                let page = self.inbox_history_ev(&q);
                self.proto_send(id, &page);
            }
            // cost-pause: what the calls cost (daemon/usage.rs: the first
            // read counts the transcripts on a thread, `counting` meanwhile)
            HubCmd::Usage { .. } => {
                let ev = self.usage_ev();
                self.proto_send(id, &ev);
            }
            // cost-pause: his pause and resume of the whole project; what it
            // did ('paused: 2 turns stopped…') is the result, not an error
            HubCmd::Pause { .. } => self.said_step(id, &tag, UserCmd::Pause),
            HubCmd::Resume { .. } => self.said_step(id, &tag, UserCmd::Resume),
            // MCP Apps C9: a terminal's `app ↗`: a UI hint to his desktop
            // app, forwarded as is (no sb-core: no message, nothing kept)
            HubCmd::AppOpen { what, agent, id: thing, .. } => self.app_open(what, agent, thing),
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
            // his stop of a watch (an alarm's 'stop watching'): its line
            // says `stopped`, its agent hears it once (core_wake.rs)
            HubCmd::WakeStop { id: watch, .. } => {
                if !self.hub.wakes().live.contains_key(&watch) {
                    return self.proto_error(id, &tag, &format!("no watch #{watch} (it ended)"));
                }
                self.step_typed(id, &tag, Input::WakeStop { id: watch });
            }
            // his run now, the TUI's /scheduled r
            HubCmd::ScheduledRun { id: timer, .. } => {
                if !self.hub.timers().map.contains_key(&timer) {
                    return self.proto_error(id, &tag, &format!("no scheduled task #{timer}"));
                }
                self.step_typed(id, &tag, Input::EveryRun { id: timer });
            }
            // the `artifacts` op's add (art_add): its words are the result
            HubCmd::ArtifactsAdd { agent, target, title, folder, .. } => {
                let title = title.filter(|t| !t.trim().is_empty());
                match self.art_add(&agent, &target, title, folder) {
                    Ok(text) => self.proto_send(id, &HubEv::Notice { project, cmd: Some(tag.clone()), text, cid: None }),
                    Err(e) => self.proto_error(id, &tag, &e),
                }
                self.artifacts_refresh(true);
            }
            // artifacts in folders (daemon/art_folders.rs)
            HubCmd::ArtifactsMove { ids, folder, to, keep_undo, .. } => self.art_move_typed(id, &tag, ids, folder, to, !keep_undo),
            HubCmd::ArtifactsFolders { op, .. } => self.art_folders_typed(id, &tag, op),
            HubCmd::ArtifactsLibrary { .. } => self.art_library_typed(id),
            // `bise stop` (client::stop): the `stop_hub` op's path; its
            // `{}` goes out before the loop ends
            HubCmd::StopHub { keep_agents, .. } => {
                self.view_stopped();
                let _ = self.tx.send(Msg::Shutdown { keep: keep_agents });
            }
            // a note talk's words to its page (the `page_voice` op's path)
            HubCmd::PageVoice { page, phase, text, .. } => self.page_voice(&page, &phase, &text),
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
            // update-flow: his click on the pill or the line (a client
            // socket's only: agents have no op for it)
            HubCmd::BiseUpdate { .. } => self.step_typed(id, &tag, Input::UpdateNow),
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
            HubCmd::Slash { agent, line, cid, opts, parts, .. } => {
                // msg-parts: the line is his parts' text after its verb
                // (`say `, `steer `), so the model reads what it read
                // before; a line that isn't refuses (never a command)
                if !parts.is_empty() {
                    let refused = cid.map(|_| "refused");
                    let said = crate::parts_in::said(&parts, &crate::parts_in::Door { store: &bise_home::Home::from_env().images_dir(), put: &crate::daemon::fn_context::store_image, exists: &crate::daemon::fn_context::file_exists, known: &self.artifact_known() });
                    if let Ok(s) = &said {
                        self.artifact_notes_said(&s.parts);
                    }
                    return match said {
                        // the line is the text of the parts he sent; the
                        // model gets the hub's (a file gone at send time
                        // is marked missing there, file-chips)
                        Ok(s) => match line.strip_suffix(bise_proto::parts::render(&parts).as_str()) {
                            Some(verb @ ("" | "say " | "steer ")) if !line.trim_start().starts_with('/') => {
                                let text = format!("{verb}{}", s.text);
                                self.proto_input(id, &tag, agent, text, opts, cid, s.parts)
                            }
                            _ => self.proto_error_cid(id, &tag, PARTS_NOT_THE_LINE, cid, refused),
                        },
                        Err(e) => self.proto_error_cid(id, &tag, &e, cid, refused),
                    };
                }
                if !line.trim_start().starts_with('/') {
                    return self.proto_input(id, &tag, agent, line, opts, cid, String::new());
                }
                if opts.queued() {
                    return self.proto_error_cid(id, &tag, QUEUED_COMMAND, cid, cid.map(|_| "refused"));
                }
                self.proto_slash(id, &tag, agent, &line, cid)
            }
            // running processes (docs/processes.md): the worker scans,
            // reads and stops off the loop, and answers this client
            HubCmd::Processes { watch, .. } => self.procs.ask(Ask::List { to: Typed::Answer(id), watch: watch.map(|on| (id, on)) }),
            HubCmd::ProcOut { agent, id: pid, from, .. } => self.procs.ask(Ask::Out { client: id, agent, id: pid, from }),
            HubCmd::ProcFollow { agent, id: pid, on, .. } => self.procs.ask(Ask::Follow { client: id, agent, id: pid, on }),
            // his only (issue 16: a client's op, agents' connections never
            // reach this arm): one process, or all of the agent's
            HubCmd::ProcStop { agent, id: pid, all, .. } => match (pid, all) {
                (Some(pid), false) => self.procs.ask(Ask::Stop { client: id, agent, id: Some(pid) }),
                (None, true) => self.procs.ask(Ask::Stop { client: id, agent, id: None }),
                _ => self.proto_error(id, &tag, "proc_stop takes a process id or all, one of the two"),
            },
            // mcp-apps (C5): answered by their id (daemon/rpc.rs, views.rs)
            HubCmd::ViewResource { .. } | HubCmd::ViewCall { .. } | HubCmd::ViewMessage { .. } | HubCmd::ViewContext { .. } | HubCmd::PluginViews { .. } => {}
            HubCmd::Unknown { .. } => {}
        }
    }

    /// His words to `agent` (`send`, and `slash`'s line that isn't a
    /// command): the `input` op's path with its fields, its notices back
    /// as errors with `cid`.
    /// `parts`: his parts' JSON (msg-parts, `parts_in::said`; "" none),
    /// carried on the op to sb-core's message.
    #[allow(clippy::too_many_arguments)]
    fn proto_input(&mut self, id: ClientId, tag: &str, agent: String, text: String, opts: SendOpts, cid: Option<u64>, parts: String) {
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
        // G: its notices (an agent gone: BISE-86's undelivered
        // line) come back with its cid
        let mut op = json!({"op": "input", "focus": agent, "text": text, "queued": queued, "context": opts.context});
        if opts.voice {
            op["voice"] = json!(true);
        }
        if let Some(via) = opts.via {
            op["via"] = json!(via);
        }
        // at-start: the thread he typed it in (sb-core's trace there)
        if let Some(from) = opts.from.filter(|f| *f != agent) {
            op["from"] = json!(from);
        }
        if !parts.is_empty() {
            op["parts"] = json!(parts);
        }
        // BISE-266: a key saved since a REPL started reaches it before
        // this message does; then the input handler (daemon/capsule.rs:
        // a page's answer, a step, a route or a plain input). The wire's
        // `input` op is gone (client-protocol P3b): this is its one door
        // A1: the id sb-core gives his message comes back typed with its
        // step (AStep.msg -> Hub::said), never read from a line; a send
        // answers it as its result (`command/run` answers its words)
        // send-reason: no message, its reason (proto/send.rs)
        self.hub.said = None;
        let mut took = Took::Stepped;
        self.stepping(id, tag, cid, |sh| {
            sh.keys_changed();
            took = sh.input(id, &op);
        });
        let said = self.hub.said.take();
        if tag == "send" {
            let r = super::send::send_result(took, said);
            self.rpc_set_result(serde_json::to_value(r).unwrap_or_default());
        }
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
            // the board's words as the result (sb-core's own fn, architect
            // m_13450), the rows to a connection that reads them
            Slash::Agents => {
                if self.rpc.init(id) || self.proto.has(id) {
                    let snap = self.snapshot();
                    let (agents, _) = self.proto_rows(&snap);
                    let agents = self.agents_for(id, &snap, &agents);
                    self.proto_send(id, &agents);
                }
                let text = crate::board::user_board(&self.hub.st, now_ms());
                self.proto_send(id, &HubEv::Notice { project, cmd: Some(tag.to_string()), text, cid });
            }
            // no PR: its words as the result; else the rows as the prs
            // event to this connection (the terminal's prs arm reads it)
            Slash::Prs => match self.prs_ev() {
                HubEv::Prs { none: Some(text), .. } => self.proto_send(id, &HubEv::Notice { project, cmd: Some(tag.to_string()), text, cid }),
                prs => self.proto_send(id, &prs),
            },
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
                match self.art_add(&agent, &target, None, None) {
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
            // `/answer N …` on a page's card takes the page's path first
            // (the input op's, P3b): a step ticked there, a question's
            // digit as its option's words, and the page hears it
            Slash::Step(cmd) => {
                let cmd = match self.page_first(id, line, &[]) {
                    Page::Done => return self.page_answers(),
                    Page::Reply(l) => crate::router::parse(&l, &agent),
                    Page::Not => cmd,
                };
                self.stepping(id, tag, cid, |sh| sh.step(Input::UserCmd { client: id, focus: agent, cmd }));
                self.page_answers();
            }
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

    /// /stop's other half (C6, m_3893): his words to an agent that
    /// computer use holds (stopped, or paused because he took the wheel)
    /// are its go-ahead, so it is resumed before they reach sb-core
    /// (`crate::computer_use::resume_if_held`, on its own thread, never
    /// waited on). The agent is the one the router sends his words to
    /// (`router::parse`: `To` its target, `Say` the focus); a command
    /// resumes nothing. Computer use off: state.json is not read. Called
    /// from `step_input`, where both doors meet (turn/send and
    /// command/run's line, proto_input → input): the clients no longer
    /// do it.
    pub(crate) fn resume_computer_use(&self, focus: &str, text: &str) {
        if !crate::computer_use::is_on() {
            return;
        }
        let to = match crate::router::parse(text, focus) {
            UserCmd::To { target, .. } => target,
            UserCmd::Say(_) => focus.to_string(),
            _ => return,
        };
        let Some(dir) = self.hub.st.resolve(&to).and_then(|n| self.dir_of(&n)) else { return };
        let key = bise_computer_use::who::key(&self.opts.paths.proc_hub(), &dir);
        crate::computer_use::resume_if_held(&bise_computer_use::paths::Paths::from_env(), &key);
    }
}
