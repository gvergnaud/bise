//! An agent's `sb` request on agent.sock (docs/issues/17; issue 37 #2):
//! one entry point, and its sender checked by sb-core before anything runs.
//! The judge read who sent it at its accept ([`super::accept::Asker`]);
//! sb-core says whether that sender may speak as the request's `from`
//! (bend/hub/peer.bend `speaker`, laws in bend/laws/peer.bend):
//!
//! - a request sb-core runs ([`core_served`]: an `AgentReq`, no attachment
//!   or card link to read first) carries its sender in `Input::Agent`, and
//!   sb-core refuses it inside that same step (fx `spoke` alone); one with
//!   files or a link takes `speaks` first, then sb-core checks it again;
//! - every other one (inspect, history/show, page_*, taste/people,
//!   artifact, project_list, a cross-hub follow or read, `version`, an
//!   unknown cmd, and a request with `--attach` or a card link) first
//!   steps `Input::Speaks` and runs only on its `spoke {ok}`
//!   ([`Shell::spoke`]).
//!
//! No path serves an agent request without sb-core's ok: while sb-core
//! boots or replays its journal, these requests wait for it too (fail
//! closed). A refusal: the words sb-core gave, and one hub.log line with
//! the peer's pid and program.

use super::accept::{Asker, SockName};
use super::{history, log_line, write_json, xhub, xread, Shell, Token};
use crate::core::{AgentReq, Effect, Input};
use serde_json::{json, Value};
use std::os::unix::net::UnixStream;

/// Whether sb-core runs `req` itself, checking its sender in that step:
/// not one the shell serves (the file reads, pages, keeps, artifacts,
/// project_list), not a cross-hub follow (`follows`), and nothing the
/// shell reads before the step (`--attach`'s files, `sb card`'s page,
/// artifact or open links). The rest take the `speaks` step first.
pub(super) fn core_served(req: &AgentReq, follows: bool) -> bool {
    match req {
        AgentReq::Inspect(_)
        | AgentReq::Show { .. }
        | AgentReq::History(_)
        | AgentReq::PagePublish(_)
        | AgentReq::PageStart { .. }
        | AgentReq::PageTick { .. }
        | AgentReq::PageWaiting
        | AgentReq::PageList
        | AgentReq::PageNotes { .. }
        | AgentReq::Taste(_)
        | AgentReq::People(_)
        | AgentReq::Artifact(_)
        | AgentReq::ProjectList
        | AgentReq::Unknown { .. } => false,
        AgentReq::Follow { .. } => !follows,
        AgentReq::Send { attach, .. } | AgentReq::Spawn { attach, .. } => attach.is_empty(),
        AgentReq::Card { page, artifact, open, .. } => page.is_none() && artifact.is_none() && open.is_empty(),
        _ => true,
    }
}

impl Shell {
    /// May `asker` speak as `from` (sb-core's `speaks` step, its fx
    /// `spoke`)? Err: the line `sb` prints, and one hub.log line.
    pub(super) fn spoke(&mut self, token: Token, from: &str, cmd: &str, asker: &Asker) -> Result<(), String> {
        let ask = Input::Speaks { token, from: from.into(), cmd: cmd.into(), by: asker.by() };
        let fx = self.hub.handle(ask, &mut self.env);
        let said = fx.iter().find_map(|e| match e {
            Effect::Spoke { ok, why, .. } => Some((*ok, why.clone())),
            _ => None,
        });
        self.step_fx(fx);
        match said {
            Some((true, _)) => Ok(()),
            // no answer (an older or broken sb-core): refused, fail closed
            Some((false, why)) => Err(self.refused(asker, cmd, why)),
            None => Err(self.refused(asker, cmd, "the hub could not check who sent this request: refused".into())),
        }
    }

    fn refused(&self, asker: &Asker, cmd: &str, why: String) -> String {
        log_line(&self.opts.paths, &format!("agent request refused on {} ({cmd}): {}: {why}", asker.sock.name(), asker.peer));
        why
    }

    /// An agent request (agent.sock's `agent` op).
    /// An agent request (agent.sock's `agent` op), decoded once
    /// (`bise_proto::sb::Req`). One it cannot read (unknown, malformed)
    /// takes the `speaks` step too before its error.
    pub(super) fn agent_new(&mut self, token: Token, mut stream: UnixStream, v: Value, asker: &Asker) {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let (from, cmd) = (s("from"), s("cmd"));
        // its sb gave up before the loop took it (a busy hub): not run
        // late, or a send it will retry is delivered twice (hub-stall)
        if super::outbox::peer_gone(&stream) {
            log_line(&self.opts.paths, &format!("sb {cmd} from {from}: its sb gave up before the hub took it: not run"));
            return;
        }
        let req = AgentReq::decode_known(v);
        if let Ok(r) = &req {
            let follows = matches!(r, AgentReq::Follow { agent, .. } if self.wants_follow(agent));
            if core_served(r, follows) {
                let req = req.expect("checked Ok");
                return self.core_step(token, stream, from, &cmd, req, asker, None);
            }
        }
        if let Err(e) = self.spoke(token, &from, &cmd, asker) {
            write_json(&mut stream, &json!({"ok": false, "error": e}));
            return;
        }
        match req {
            Ok(req) => self.shell_request(token, stream, from, req, asker),
            Err(e) => {
                write_json(&mut stream, &json!({"ok": false, "error": e}));
            }
        }
    }

    /// `req`'s sb-core step: its sender rides it, and sb-core refuses it
    /// first (fx spoke alone) when it may not speak as `from`; then a
    /// card's link (`sb card --page`) on the card it opened.
    #[allow(clippy::too_many_arguments)]
    fn core_step(&mut self, token: Token, stream: UnixStream, from: String, cmd: &str, req: AgentReq, asker: &Asker, link: Option<crate::card_link::CardLink>) {
        let before: Vec<u64> = self.hub.st.cards.keys().copied().collect();
        self.replies.insert(token, stream);
        let fx = self.hub.handle(Input::Agent { token, from, req, by: asker.by() }, &mut self.env);
        let refused = fx.iter().find_map(|e| match e {
            Effect::Spoke { ok: false, why, .. } => Some(why.clone()),
            _ => None,
        });
        self.step_fx(fx);
        if let Some(why) = refused {
            let why = self.refused(asker, cmd, why);
            if let Some(mut stream) = self.replies.remove(&token) {
                write_json(&mut stream, &json!({"ok": false, "error": why}));
            }
            return;
        }
        if let Some(l) = link {
            self.link_new_card(&before, l);
        }
    }

    /// A request the shell serves (its sender's ok given: [`Shell::spoke`]);
    /// an `AgentReq` with files or a card link to read first goes on to
    /// sb-core, which checks its sender again.
    fn shell_request(&mut self, token: Token, mut stream: UnixStream, from: String, req: AgentReq, asker: &Asker) {
        let body = match req {
            // another project's thread (S2 C, xread.rs): `<p>/<agent>`
            AgentReq::Inspect(q) if xread::elsewhere(&q.agent) => self.xinspect(&q),
            AgentReq::Show { msg: Some(n), context, .. } => self.show_msg(n, context),
            AgentReq::Show { agent, pos, context, .. } if xread::elsewhere(&agent) => self.xshow(&agent, pos, context),
            AgentReq::History(h) if xread::wanted(&h) => self.xsearch(&h),
            // S10: bise's `sb follow <p>/<agent>`: on that project's hub
            AgentReq::Follow { agent, on, .. } if self.wants_follow(&agent) => return self.xfollow(stream, &from, &agent, on),
            AgentReq::Inspect(q) => self.inspect(&from, &q),
            AgentReq::Show { agent, pos, context, .. } => self.search(&history::Look::show(&agent, pos, context)),
            AgentReq::History(h) => self.search(&history::Look::history(&h)),
            r @ (AgentReq::PagePublish(_) | AgentReq::PageStart { .. } | AgentReq::PageTick { .. } | AgentReq::PageWaiting | AgentReq::PageList | AgentReq::PageNotes { .. }) => self.page_op(&from, r),
            AgentReq::Taste(k) => self.keeps_op("taste", &k),
            AgentReq::People(k) => self.keeps_op("people", &k),
            AgentReq::Artifact(a) => self.artifact_cmd(&from, a),
            AgentReq::ProjectList => xhub::list_body(&self.opts.home),
            req => return self.core_files(token, stream, from, req, asker),
        };
        write_json(&mut stream, &body);
    }

    /// An sb-core request with something to read first: its `--attach`
    /// parts made (daemon/attach.rs), a notification's links checked
    /// before it opens (daemon/page_cards.rs).
    fn core_files(&mut self, token: Token, mut stream: UnixStream, from: String, req: AgentReq, asker: &Asker) {
        let cmd = req.tag().to_string();
        let checked = self.attached(&from, req).and_then(|req| self.card_page_link(&req).map(|link| (req, link)));
        match checked {
            Ok((req, link)) => self.core_step(token, stream, from, &cmd, req, asker, link),
            Err(e) => {
                write_json(&mut stream, &json!({"ok": false, "error": e}));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Env, Hub, Loss};
    use crate::model::Workspace;
    use crate::peer::{Sender, Sock, Who};

    struct NoEnv;

    impl Env for NoEnv {
        fn now(&self) -> u64 {
            1_000_000
        }
        fn is_git(&mut self) -> bool {
            false
        }
        fn worktree_create(&mut self, _: &str, _: bool) -> Result<Workspace, String> {
            Err("no worktree here".into())
        }
        fn worktree_loss(&mut self, _: &Workspace) -> Loss {
            Loss::default()
        }
        fn worktree_drop(&mut self, _: &str, _: &Workspace, _: &Loss) -> Result<Option<String>, String> {
            Err("no worktree here".into())
        }
        fn worktree_restore(&mut self, _: &str, _: &Workspace, _: Option<&str>) -> Result<Workspace, String> {
            Err("no worktree here".into())
        }
    }

    /// A hub with main and task t1 (its folder t1).
    fn hub() -> (Hub, NoEnv) {
        let (mut h, mut env) = (Hub::new("/w"), NoEnv);
        let spawn = json!({"cmd": "spawn", "name": "t1", "objective": "o", "worktree": false});
        h.handle(Input::Agent { token: 1, from: "main".into(), req: AgentReq::decode(spawn).expect("spawn"), by: Sender::hub() }, &mut env);
        (h, env)
    }

    fn spoke(fx: &[Effect]) -> Vec<(bool, String)> {
        fx.iter().filter_map(|e| match e { Effect::Spoke { ok, why, .. } => Some((*ok, why.clone())), _ => None }).collect()
    }

    /// issue 37 #2 (law req_refused_says_only_why): an agent request its
    /// sender may not make is refused in its own step, by fx spoke alone:
    /// no reply, no journal line, nothing delivered.
    #[test]
    fn a_request_as_another_is_refused_in_its_own_step() {
        let (mut h, mut env) = hub();
        let t1 = Sender::of(&Who::Agent("t1".into()), Sock::Agent);
        let send = AgentReq::decode(json!({"cmd": "send", "to": "t1", "text": "go"})).expect("send");
        let fx = h.handle(Input::Agent { token: 7, from: "main".into(), req: send, by: t1.clone() }, &mut env);
        assert_eq!(spoke(&fx), vec![(false, "this request says it is from main, but it comes from agent t1's process: an agent speaks only as itself (SB_AGENT is not for changing)".to_string())]);
        assert_eq!(fx.len(), 1, "fx spoke alone: {fx:?}");
        let list = AgentReq::decode(json!({"cmd": "list"})).expect("list");
        let fx = h.handle(Input::Agent { token: 8, from: "t1".into(), req: list, by: t1 }, &mut env);
        assert!(spoke(&fx).is_empty() && fx.iter().any(|e| matches!(e, Effect::Reply { token: 8, .. })), "as itself: served {fx:?}");
        // an agent request that came on hub.sock (law agent_req_only_on_agent_sock)
        let list = AgentReq::decode(json!({"cmd": "list"})).expect("list");
        let fx = h.handle(Input::Agent { token: 9, from: "main".into(), req: list, by: Sender::of(&Who::Outside, Sock::Client) }, &mut env);
        assert_eq!(spoke(&fx), vec![(false, "an agent request is served on agent.sock only".to_string())]);
    }

    /// The variant's name, by an exhaustive match: a new `AgentReq`
    /// variant does not compile here until it has a fixture below.
    fn kind(r: &AgentReq) -> &'static str {
        match r {
            AgentReq::List => "list",
            AgentReq::Tasks => "tasks",
            AgentReq::Send { .. } => "send",
            AgentReq::Wait { .. } => "wait",
            AgentReq::Ask { .. } => "ask",
            AgentReq::Status { .. } => "status",
            AgentReq::Worktree { .. } => "worktree",
            AgentReq::Flow { .. } => "flow",
            AgentReq::Report { .. } => "report",
            AgentReq::Follow { .. } => "follow",
            AgentReq::Spawn { .. } => "spawn",
            AgentReq::Switch { .. } => "switch",
            AgentReq::Feature { .. } => "feature",
            AgentReq::Move { .. } => "move",
            AgentReq::Land { .. } => "land",
            AgentReq::Interrupt { .. } => "interrupt",
            AgentReq::Stop { .. } => "stop",
            AgentReq::Drop { .. } => "drop",
            AgentReq::Card { .. } => "card",
            AgentReq::Close { .. } => "close",
            AgentReq::Withdraw { .. } => "withdraw",
            AgentReq::Rename { .. } => "rename",
            AgentReq::Restore { .. } => "restore",
            AgentReq::Isolate { .. } => "isolate",
            AgentReq::Every(_) => "every",
            AgentReq::Wake(_) => "wake",
            AgentReq::ProjectSend { .. } => "project_send",
            AgentReq::ProjectAsk { .. } => "project_ask",
            AgentReq::ProjectList => "project_list",
            AgentReq::Inspect(_) => "inspect",
            AgentReq::History(_) => "history",
            AgentReq::Show { .. } => "show",
            AgentReq::Taste(_) => "taste",
            AgentReq::People(_) => "people",
            AgentReq::Artifact(_) => "artifact",
            AgentReq::PagePublish(_) => "page_publish",
            AgentReq::PageStart { .. } => "page_start",
            AgentReq::PageWaiting => "page_waiting",
            AgentReq::PageTick { .. } => "page_tick",
            AgentReq::PageList => "page_list",
            AgentReq::PageNotes { .. } => "page_notes",
            AgentReq::Unknown { .. } => "unknown",
        }
    }

    /// architect m_35671: rides_req skips the `speaks` step, so it is
    /// proved here, not trusted. Every variant (the conditional ones both
    /// ways), sent by another project's agent: the outcome is fx spoke's
    /// refusal alone: no reply, journal line, notice or other effect, and
    /// the hub's state as it was. A listed variant that one day acts before
    /// sb-core's req step turns this red.
    #[test]
    fn every_request_from_another_hub_is_refused_alone() {
        let reqs = [
            json!({"cmd": "list"}),
            json!({"cmd": "tasks"}),
            json!({"cmd": "send", "to": "t1", "text": "go"}),
            json!({"cmd": "send", "to": "t1", "text": "go", "model": "gpt-5", "effort": "high"}),
            json!({"cmd": "wait", "msg": "m_1"}),
            json!({"cmd": "ask", "to": "t1", "text": "?"}),
            json!({"cmd": "status", "status": "working", "note": "n"}),
            json!({"cmd": "worktree", "path": "/tmp/w"}),
            json!({"cmd": "flow", "set": "trunk"}),
            json!({"cmd": "report", "kind": "progress", "summary": "s"}),
            json!({"cmd": "follow", "agent": "t1"}),
            json!({"cmd": "spawn", "name": "t9", "objective": "o"}),
            json!({"cmd": "spawn", "name": "t9", "objective": "o", "model": "gpt-5"}),
            json!({"cmd": "spawn", "name": "t9", "objective": "o", "feature": "f"}),
            json!({"cmd": "switch", "to": "t1", "model": "gpt-5"}),
            json!({"cmd": "feature", "step": "new", "name": "f"}),
            json!({"cmd": "move", "agent": "t1", "place": "new"}),
            json!({"cmd": "land", "message": "m"}),
            json!({"cmd": "interrupt", "agent": "t1"}),
            json!({"cmd": "stop", "agent": "t1", "reason": "r"}),
            json!({"cmd": "drop", "agent": "t1"}),
            json!({"cmd": "card", "text": "q?"}),
            json!({"cmd": "close", "card": 1}),
            json!({"cmd": "withdraw", "card": 1, "why": "w"}),
            json!({"cmd": "rename", "agent": "t1", "new_name": "t8"}),
            json!({"cmd": "restore", "agent": "t1"}),
            json!({"cmd": "isolate", "agent": "t1"}),
            json!({"cmd": "every", "step": "add", "to": "t1", "text": "x", "every_ms": 60000}),
            json!({"cmd": "wake", "step": "list"}),
            json!({"cmd": "project_send", "project": "shop", "msg": "m_1"}),
            json!({"cmd": "project_ask", "project": "shop", "text": "?"}),
            // the shell's own: the shell steps `speaks` for them, and an
            // `Input::Agent` carrying one is refused alone as well
            json!({"cmd": "project_list"}),
            json!({"cmd": "inspect", "agent": "t1"}),
            json!({"cmd": "history", "query": "x"}),
            json!({"cmd": "show", "agent": "t1", "pos": 1}),
            json!({"cmd": "taste", "step": "list"}),
            json!({"cmd": "people", "step": "list"}),
            json!({"cmd": "artifact", "do": "list"}),
            json!({"cmd": "page_publish", "html": "<p>x</p>"}),
            json!({"cmd": "page_start", "id": "p"}),
            json!({"cmd": "page_waiting"}),
            json!({"cmd": "page_tick", "id": "p", "item": "i"}),
            json!({"cmd": "page_list"}),
            json!({"cmd": "page_notes", "id": "p"}),
            json!({"cmd": "what-is-this"}),
        ];
        let (mut h, mut env) = hub();
        let other = Sender::of(&Who::OtherHub("t1".into()), Sock::Agent);
        let words = "this request comes from agent t1's process (another project's hub): an agent speaks only on its own hub";
        let mut seen = std::collections::BTreeSet::new();
        for (i, v) in reqs.iter().enumerate() {
            let req = AgentReq::decode(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"));
            seen.insert(kind(&req));
            let before = format!("{:?}", h.st);
            let fx = h.handle(Input::Agent { token: 100 + i as u64, from: "t1".into(), req, by: other.clone() }, &mut env);
            assert!(
                matches!(fx.as_slice(), [Effect::Spoke { ok: false, why, .. }] if why == words),
                "{v}: the refusal alone, got {fx:?}"
            );
            assert_eq!(format!("{:?}", h.st), before, "{v}: the hub's state as it was");
        }
        assert_eq!(seen.len(), 42, "a fixture for every variant: {seen:?}");
    }

    /// The shell's own requests ask `speaks` and get one fx spoke.
    #[test]
    fn speaks_answers_once_and_fails_closed() {
        let (mut h, mut env) = hub();
        let mut ask = |from: &str, by: Sender| spoke(&h.handle(Input::Speaks { token: 3, from: from.into(), cmd: "inspect".into(), by }, &mut env));
        let agent = |d: &str| Sender::of(&Who::Agent(d.into()), Sock::Agent);
        assert_eq!(ask("t1", agent("t1")), vec![(true, String::new())]);
        assert_eq!(ask("anything", Sender::of(&Who::Outside, Sock::Agent)), vec![(true, String::new())]);
        assert!(!ask("main", agent("t1"))[0].0);
        assert_eq!(ask("t1", Sender::of(&Who::Gone, Sock::Agent))[0].1, "the hub could not read which process sent this request (gone already?): refused");
        assert_eq!(
            ask("t2", Sender::of(&Who::OtherHub("t2".into()), Sock::Agent))[0].1,
            "this request comes from agent t2's process (another project's hub): an agent speaks only on its own hub"
        );
        assert_eq!(ask("x", agent("nope"))[0].1, "this request comes from the process of agent folder nope, which this hub does not know: refused");
        // a fact sb-core does not know reads gone
        let odd = Sender { who: "admin".into(), dir: String::new(), sock: "agent".into() };
        assert!(!ask("t1", odd)[0].0);
    }

    /// architect m_35407: every cmd the shell serves takes the `speaks`
    /// step (an unknown one too); sb-core's own requests check their
    /// sender in their step, unless the shell reads something first.
    #[test]
    fn every_shell_cmd_waits_for_spoke() {
        let r = |v: Value| AgentReq::decode(v).expect("an sb request");
        let shell = [
            json!({"cmd": "inspect", "agent": "t1"}),
            json!({"cmd": "history", "words": "x"}),
            json!({"cmd": "show", "agent": "t1", "pos": 1}),
            json!({"cmd": "page_waiting"}),
            json!({"cmd": "page_list"}),
            json!({"cmd": "project_list"}),
            json!({"cmd": "what-is-this"}),
        ];
        for v in shell {
            assert!(!core_served(&r(v.clone()), false), "{v} is the shell's");
        }
        assert!(!core_served(&r(json!({"cmd": "follow", "agent": "shop/t1"})), true), "a cross-hub follow");
        let send = json!({"cmd": "send", "to": "main", "text": "hi", "from": "t1"});
        assert!(core_served(&r(send.clone()), false), "a plain send is sb-core's");
        let mut with = send;
        with["attach"] = json!(["/tmp/x.png"]);
        assert!(!core_served(&r(with), false), "--attach: the shell reads the files first");
        assert!(!core_served(&r(json!({"cmd": "card", "text": "q?", "page": "p"})), false), "a card link");
        assert!(core_served(&r(json!({"cmd": "list"})), false));
    }
}
