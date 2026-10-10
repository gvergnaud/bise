//! The hub's decisions now run in Bend (`hub/*.bend`, the `sb-core`
//! process): this module is the Rust side of the link. `Hub::handle`
//! turns one input into a JSON line for sb-core, answers its git queries
//! through `Env`, and turns the effects it returns into `Effect`s. It
//! keeps a read-only mirror of the durable state (for the views: board,
//! snapshot, contexts, prompts), fed only by the journal events and the
//! runtime changes sb-core emits.
//!
//! `sb every`'s timers are sb-core's too (bend/hub/timers.bend): their
//! journal lines replay with the rest, the tick decides the wakes. Here
//! only the request's checks and words, the first wake's time
//! (`every_set`), the `every_wake` need (wake texts, a daily timer's next
//! time on the local clock, every.rs), the ◷ lines of a set or ended
//! timer, and the mirror `st.timers` from the view.

use crate::board;
use crate::model::*;
use crate::prompts;
use crate::role;
use crate::router::{self, UserCmd};
use crate::util::{clip, clip_tail, one_line, wire_escape};
use crate::wire::{self, Wire};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};


pub type ClientId = u64;
pub type Token = u64;

/// What a worktree drop would lose (RFC 0002 §5.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loss {
    pub dirty: usize,
    pub unpushed: usize,
}

impl Loss {
    pub fn any(&self) -> bool {
        self.dirty > 0 || self.unpushed > 0
    }
}

/// The side of the world the core may query synchronously.
pub trait Env {
    fn now(&self) -> u64;
    fn is_git(&self) -> bool;
    /// RFC 0002 §4.1: a worktree on `sb/<name>` for this task.
    fn worktree_create(&mut self, name: &str, with_changes: bool) -> Result<Workspace, String>;
    fn worktree_loss(&mut self, ws: &Workspace) -> Loss;
    /// RFC 0002 §5.2-5.3: save if needed, then remove. Answers the ref.
    fn worktree_drop(
        &mut self,
        name: &str,
        ws: &Workspace,
        loss: &Loss,
    ) -> Result<Option<String>, String>;
    /// RFC 0002 §5.6.
    fn worktree_restore(
        &mut self,
        name: &str,
        ws: &Workspace,
        snapshot: Option<&str>,
    ) -> Result<Workspace, String>;
    /// pr-design §6.4: the PR of `branch` is merged with `head` its head:
    /// a tip at `head` loses nothing at a drop (no backup, RFC 0002
    /// §5.1's squash case).
    fn pr_merged(&mut self, _branch: &str, _head: &str) {}
    /// dev-flow §5.1: a worktree on `sb/<name>` from the tip of feature
    /// `feature` (a registered one), landing on it.
    fn worktree_feature(&mut self, _name: &str, feature: &str) -> Result<Workspace, String> {
        Err(format!("no feature {}", feature))
    }
    /// bise desktop S2 (bise's home hub): the hub id of the registered
    /// project `name` (its name or its id), never this hub's own.
    fn project_hub(&self, name: &str) -> Result<String, String> {
        Err(format!("no project {}", name))
    }
}

/// A request of the `sb` CLI (RFC 0003 §4, RFC 0001 §7.2, §7.5).
#[derive(Clone, Debug, PartialEq)]
pub enum AgentReq {
    List,
    /// `sb tasks`: the detailed state of every task.
    Tasks,
    Send {
        to: String,
        text: String,
        expect_reply: bool,
        reply_to: Option<u64>,
        /// `--mode queued`: delivered only as a new turn.
        queued: bool,
        /// `--why`: main's reason when it answers a task for the user
        /// (shown in the `answered` line of main's feed).
        why: String,
        /// `--model`/`--effort` with the text (issue #4): the task moves
        /// to that model from its next turn, then the message goes.
        switch: Option<(String, String)>,
    },
    Wait {
        msg: u64,
        timeout_s: u64,
    },
    Ask {
        to: String,
        text: String,
        timeout_s: u64,
    },
    Status {
        status: Declared,
        note: String,
    },
    /// `sb worktree <path>|none` (BISE-136): the agent works in a private
    /// git worktree of its own (`gate.sh new`), or no longer ("" = none).
    /// A view-only fact: never sent to sb-core, never journaled.
    Worktree {
        path: String,
    },
    /// `sb flow [pr|trunk]` (main, dev-flow §2): the repo's flow and the
    /// question to ask, or the user's answer saved. The daemon's
    /// (`Effect::Flow`): never sent to sb-core.
    Flow {
        set: Option<crate::flow::FlowMode>,
    },
    Report {
        kind: String,
        summary: String,
        decisions: Vec<String>,
        /// S10: `--step n/m` (n, m)
        step: Option<(u32, u32)>,
        /// emitter 5: `--result` (proto's DiffResult, checked by the CLI)
        result: Option<Value>,
    },
    /// S10: `sb follow <agent> [--off]` (main only, sb-core decides).
    Follow {
        agent: String,
        on: bool,
        /// J: the hub that asked (bise's home hub, through its xfollow op;
        /// "" this hub's own main): the job's end goes back there
        hub: String,
    },
    Spawn {
        name: String,
        brief: Brief,
        worktree: bool,
        with_changes: bool,
        /// `--place <agent>|<branch>` (dev-flow §3.1): join that worktree;
        /// "" = the shared folder, or a new worktree with `worktree`.
        place: String,
        /// `--feature <name>` (dev-flow §5.1): a new worktree from the
        /// feature's tip, landing on it. "" = none.
        feature: String,
        /// `--model`, `--effort`, `--profile` (issue #4): all "" = the
        /// agents default, as before.
        ask: bise_catalog::spawn::Ask,
    },
    /// `sb send <task> --model <id> [--effort <e>]` without a text
    /// (issue #4): the task's model from its next turn.
    Switch {
        to: String,
        model: String,
        effort: String,
    },
    /// `sb feature new|sync|ready|merge|drop|list [<name>]` (dev-flow
    /// §5.1): the daemon's (`Effect::Feature`), git off the hub's loop.
    Feature {
        op: String,
        name: String,
    },
    /// `sb move <agent> new|shared|<agent>|<branch>` (dev-flow §3.1).
    Move {
        agent: String,
        place: String,
    },
    /// `sb land [--here] [--add <path>]... "<message>"` (dev-flow §5):
    /// run by the daemon, off the hub's loop (`Effect::Land`).
    Land {
        here: bool,
        message: String,
        add: Vec<String>,
    },
    Interrupt {
        agent: String,
    },
    Stop {
        agent: String,
        reason: String,
    },
    Drop {
        agent: String,
    },
    Card {
        text: String,
        for_msg: Option<u64>,
    },
    /// `sb close N ["note"]`: refused on a card of the user's inbox
    /// (BISE-299: only the user closes those).
    Close {
        card: u64,
        note: String,
    },
    /// `sb card --withdraw N "why"`: main takes back its own card
    /// (BISE-299).
    Withdraw {
        card: u64,
        why: String,
    },
    /// `sb rename <task> <new-name>`: the same rules as `/rename`.
    Rename {
        agent: String,
        new_name: String,
    },
    /// `sb restore <task>`: only on the user's explicit request.
    Restore {
        agent: String,
    },
    /// `sb isolate <task>`: only on the user's explicit request.
    Isolate {
        agent: String,
    },
    /// `sb every` (docs/ambient-roadmap.md B): the hub's timers (every.rs);
    /// the Rust side's, never sent to sb-core.
    Every(EveryReq),
    /// `sb project send <project> --input m_<n>` (bise's main, desktop S2):
    /// the user's message `msg` to main, forwarded by reference.
    ProjectSend {
        project: String,
        msg: u64,
    },
    /// `sb project ask <project> "<question>"`: bise's own question.
    ProjectAsk {
        project: String,
        text: String,
    },
}

/// `sb every`'s three forms.
#[derive(Clone, Debug, PartialEq)]
pub enum EveryReq {
    List,
    Stop(u64),
    /// `--show <id>`: its line and the words it sends
    Show(u64),
    /// `to` "": the caller; `name` None: the hub names it (every_name.rs)
    Add { to: String, text: String, sched: crate::every::Sched, until_ms: Option<u64>, times: Option<u64>, page: Option<String>, name: Option<String> },
}

/// `m_12` or `12`.
pub fn parse_msg_id(s: &str) -> Option<u64> {
    s.trim().trim_start_matches("m_").parse().ok()
}

fn jstr(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn jstrs(v: &Value, k: &str) -> Vec<String> {
    v.get(k)
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

impl AgentReq {
    pub fn from_json(v: &Value) -> Result<AgentReq, String> {
        let cmd = jstr(v, "cmd");
        let timeout = v.get("timeout_s").and_then(|x| x.as_u64()).unwrap_or(20);
        let req = match cmd.as_str() {
            "list" => AgentReq::List,
            "tasks" => AgentReq::Tasks,
            "send" => AgentReq::Send {
                to: jstr(v, "to"),
                text: jstr(v, "text"),
                expect_reply: v
                    .get("expect_reply")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false),
                reply_to: v
                    .get("reply_to")
                    .and_then(|x| x.as_str())
                    .and_then(parse_msg_id),
                queued: match jstr(v, "mode").as_str() {
                    "" | "steer" => false,
                    "queued" => true,
                    m => return Err(format!("unknown mode: {} (steer|queued)", m)),
                },
                why: jstr(v, "why"),
                switch: (v.get("model").is_some() || v.get("effort").is_some())
                    .then(|| (jstr(v, "model"), jstr(v, "effort"))),
            },
            "wait" => AgentReq::Wait {
                msg: parse_msg_id(&jstr(v, "msg")).ok_or("invalid message id")?,
                timeout_s: timeout,
            },
            "ask" => AgentReq::Ask {
                to: jstr(v, "to"),
                text: jstr(v, "text"),
                timeout_s: timeout,
            },
            "status" => AgentReq::Status {
                status: match jstr(v, "status").as_str() {
                    "working" => Declared::Working,
                    "done" => Declared::Done,
                    "blocked" => Declared::Blocked,
                    s => return Err(format!("unknown status: {} (working|done|blocked)", s)),
                },
                note: jstr(v, "note"),
            },
            "flow" => AgentReq::Flow {
                set: match jstr(v, "set").trim() {
                    "" => None,
                    m => Some(crate::devflow::parse_mode(m).map_err(|_| "usage: sb flow [pr|trunk]".to_string())?),
                },
            },
            "worktree" => AgentReq::Worktree {
                path: match jstr(v, "path").trim() {
                    "" | "none" => String::new(),
                    p if p.starts_with('/') => p.trim_end_matches('/').to_string(),
                    p => return Err(format!("sb worktree: an absolute path or none, not {}", p)),
                },
            },
            "report" => {
                let kind = jstr(v, "kind");
                if !["progress", "done", "failed", "blocked"].contains(&kind.as_str()) {
                    return Err(format!(
                        "unknown report kind: {} (progress|done|failed|blocked)",
                        kind
                    ));
                }
                let n = |k: &str| v.get(k).and_then(Value::as_u64).and_then(|x| u32::try_from(x).ok());
                AgentReq::Report {
                    kind,
                    summary: jstr(v, "summary"),
                    decisions: jstrs(v, "decisions"),
                    step: n("step").zip(n("of")),
                    result: v.get("result").filter(|r| r.is_object()).cloned(),
                }
            }
            "follow" => AgentReq::Follow {
                agent: jstr(v, "agent").trim_start_matches('@').to_string(),
                on: !v.get("off").and_then(Value::as_bool).unwrap_or(false),
                hub: String::new(),
            },
            "spawn" => AgentReq::Spawn {
                name: jstr(v, "name"),
                brief: Brief {
                    objective: jstr(v, "objective"),
                    context: jstr(v, "context"),
                    constraints: jstrs(v, "constraints"),
                    done_when: Some(jstr(v, "done_when")).filter(|s| !s.is_empty()),
                    report_format: Some(jstr(v, "report_format")).filter(|s| !s.is_empty()),
                },
                worktree: v.get("worktree").and_then(|x| x.as_bool()).unwrap_or(false),
                with_changes: v
                    .get("with_changes")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false),
                place: jstr(v, "place"),
                feature: jstr(v, "feature"),
                ask: bise_catalog::spawn::Ask {
                    model: jstr(v, "model"),
                    effort: jstr(v, "effort"),
                    profile: jstr(v, "profile"),
                },
            },
            "switch" => AgentReq::Switch {
                to: jstr(v, "to"),
                model: jstr(v, "model"),
                effort: jstr(v, "effort"),
            },
            "feature" => AgentReq::Feature {
                op: jstr(v, "step"),
                name: jstr(v, "name"),
            },
            "move" => AgentReq::Move {
                agent: jstr(v, "agent"),
                place: jstr(v, "place"),
            },
            "land" => AgentReq::Land {
                here: v.get("here").and_then(|x| x.as_bool()).unwrap_or(false),
                message: jstr(v, "message"),
                add: v
                    .get("add")
                    .and_then(|x| x.as_array())
                    .map(|xs| xs.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                    .unwrap_or_default(),
            },
            "interrupt" => AgentReq::Interrupt {
                agent: jstr(v, "agent"),
            },
            "stop" => AgentReq::Stop {
                agent: jstr(v, "agent"),
                reason: jstr(v, "reason"),
            },
            "drop" => AgentReq::Drop {
                agent: jstr(v, "agent"),
            },
            "card" => AgentReq::Card {
                text: jstr(v, "text"),
                for_msg: v.get("for").and_then(|x| x.as_str()).and_then(parse_msg_id),
            },
            "close" => AgentReq::Close {
                card: v
                    .get("card")
                    .and_then(|x| x.as_u64())
                    .ok_or("usage: sb close <card> [\"<note>\"]")?,
                note: jstr(v, "note"),
            },
            "withdraw" => AgentReq::Withdraw {
                card: v
                    .get("card")
                    .and_then(|x| x.as_u64())
                    .ok_or("usage: sb card --withdraw <card> \"<why>\"")?,
                why: jstr(v, "why"),
            },
            "rename" => AgentReq::Rename {
                agent: jstr(v, "agent"),
                new_name: jstr(v, "new_name"),
            },
            "restore" => AgentReq::Restore {
                agent: jstr(v, "agent"),
            },
            "isolate" => AgentReq::Isolate {
                agent: jstr(v, "agent"),
            },
            "every" => AgentReq::Every(match jstr(v, "step").as_str() {
                "" | "list" => EveryReq::List,
                "stop" => EveryReq::Stop(v["id"].as_u64().ok_or("usage: sb every --stop <id>")?),
                "show" => EveryReq::Show(v["id"].as_u64().ok_or("usage: sb every --show <id>")?),
                "add" => EveryReq::Add {
                    to: jstr(v, "to").trim_start_matches('@').to_string(),
                    text: jstr(v, "text"),
                    sched: match (v["every_ms"].as_u64(), v["daily_min"].as_u64()) {
                        (Some(p), _) => crate::every::Sched::Every(p),
                        (None, Some(m)) if m < 24 * 60 => crate::every::Sched::Daily(m as u32),
                        _ => return Err("sb every: every_ms or daily_min".into()),
                    },
                    until_ms: v["until_ms"].as_u64(),
                    times: v["times"].as_u64(),
                    page: v["page"].as_str().filter(|p| !p.is_empty()).map(String::from),
                    name: v["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).map(String::from),
                },
                o => return Err(format!("sb every: unknown step {}", o)),
            }),
            "project_send" => AgentReq::ProjectSend {
                project: jstr(v, "project"),
                msg: parse_msg_id(&jstr(v, "msg")).ok_or("usage: sb project send <project> --input m_<n>")?,
            },
            "project_ask" => AgentReq::ProjectAsk {
                project: jstr(v, "project"),
                text: jstr(v, "text"),
            },
            other => return Err(format!("unknown command: {}", other)),
        };
        Ok(req)
    }
}

#[derive(Clone, Debug)]
pub enum Input {
    /// The daemon started: spawn every live agent.
    Boot,
    /// The REPL of `agent` is connected and idle.
    ReplReady {
        agent: String,
    },
    ReplLine {
        agent: String,
        line: String,
    },
    /// `--- idle`. `leftover`: steering written during the turn was still
    /// in the file (never read by the runtime).
    ReplIdle {
        agent: String,
        leftover: bool,
    },
    ReplExited {
        agent: String,
        crashed: bool,
        reason: String,
    },
    ClientHello {
        client: ClientId,
    },
    ClientInput {
        client: ClientId,
        focus: String,
        text: String,
        /// The window's "send queued": a plain text or `@agent` text waits
        /// for the end of the agent's turn (sb-core's queued-mode path).
        queued: bool,
    },
    ClientFocus {
        client: ClientId,
        focus: String,
    },
    ClientGone {
        client: ClientId,
    },
    ClientConfirm {
        client: ClientId,
        id: u64,
        yes: bool,
    },
    ClientInterrupt {
        client: ClientId,
        agent: String,
    },
    Agent {
        token: Token,
        from: String,
        req: AgentReq,
    },
    Tick,
    /// `/scheduled`'s stop: the user stopped timer `id`; its agent hears
    /// it from bise, once (`why`: words after the id, may be empty).
    EveryStop {
        id: u64,
        why: String,
    },
    /// `/scheduled`'s run now: timer `id` wakes its agent at once,
    /// outside its count.
    EveryRun {
        id: u64,
    },
    /// The approvals gate (approvals-design.md §9): a `confirm` card for
    /// `agent`'s waiting call, in the user's inbox.
    ConfirmOpen {
        agent: String,
        text: String,
    },
    /// Its calls are gone (interrupted): the card closes unanswered.
    ConfirmClose {
        card: u64,
        res: String,
    },
    /// The end of a role-line call (BISE-126) for the task in `dir`,
    /// asked with `key`: the new line, or None (failed: the old one stays).
    RoleLine {
        dir: String,
        key: String,
        line: Option<String>,
    },
    /// The end of a timer-name call (every_name.rs): the model's reply,
    /// or None (no model, or it failed: the plain fallback names it).
    TimerName {
        id: u64,
        reply: Option<String>,
    },
    /// An answer of the PR poller (`forge::poll`, its own thread).
    Prs(crate::forge::poll::Report),
    /// `gh pr merge`'s answer for the ready-to-merge item `card`
    /// (`Effect::Merge`, the daemon's thread): Err is gh's reason.
    Merged {
        card: u64,
        place: String,
        number: u64,
        res: Result<(), String>,
    },
    /// A feature step ended (dev-flow §5.1, the daemon's thread).
    Feature(FeatureDone),
    /// update-card: a check of the release channel (the daemon's, at
    /// the start and every hour, or `/update`).
    Release(update_card::ReleaseCheck),
    /// expired-ux: the ChatGPT sign-in is back (the daemon reads
    /// auth.json): the `signin` item closes, its agents go on.
    SignedIn,
    /// update-card: the end of `1` on an update item (`Effect::Update`):
    /// Ok, the switch started; Err, why it failed (nothing changed).
    /// desktop S2, a project hub: bise's message (`xin` on hub.sock) from
    /// bise's hub `hub`; `token` answers the caller (ack or error).
    XIn {
        token: Token,
        hub: String,
        xid: u64,
        kind: String,
        text: String,
        /// the sender's project name (J: a followed task's end, kind
        /// job_end, is said from `@<name>`; "" from bise's home hub)
        name: String,
    },
    /// desktop S2, bise's home hub: the project hub has outbox entry `xid`.
    XAck {
        xid: u64,
    },
    /// Its delivery failed (the hub away, a refusal): a retry later.
    XFail {
        xid: u64,
    },
    /// The project `name` answered it (`xreply` on hub.sock).
    XReply {
        xid: u64,
        name: String,
        text: String,
    },
    /// desktop S2 step 2, bise's home hub: his words to bise's main that
    /// route.rs sends to project `to` (a hub id, `name` its name, `why`
    /// the guess's reason, `via` his view): held 2 s, then forwarded.
    RouteHold {
        text: String,
        via: String,
        to: String,
        name: String,
        why: String,
        /// his fn context (FnContext JSON), as the input op had it: sb-core
        /// carries it as text to the project, which renders it there
        /// (fn_context::with_context); never on the home hub
        context: Option<Value>,
    },
    /// A held route goes to `to` instead (the shell checked it in the
    /// registry; `bise`: a cancel). `client`: the typed connection that
    /// asked, which gets sb-core's notice when the route ended already.
    RouteCorrect {
        client: Option<ClientId>,
        rid: u64,
        to: String,
        name: String,
    },
    /// A held route doesn't go: his words reach main as today.
    RouteCancel {
        client: Option<ClientId>,
        rid: u64,
    },
    /// S2 step 5: BISE_ROUTE_MODEL picked project `to` (checked in the
    /// registry by the shell, `name` its name) for the unclear route
    /// `rid`; sb-core holds it 2 s for `to` if it still waits, else
    /// nothing.
    RoutePick {
        rid: u64,
        to: String,
        name: String,
        why: String,
    },
    /// S10: follow a task (the typed `follow`; `client` gets the refusal).
    Follow {
        client: Option<ClientId>,
        agent: String,
        on: bool,
    },
    /// A typed command of a window (bise-proto `new`, `rename`, `model`,
    /// `effort`): the same handlers as the TUI's line, its fields never
    /// parsed again (core_user.rs `user_cmd`); `focus` the agent it's for.
    UserCmd {
        client: ClientId,
        focus: String,
        cmd: UserCmd,
    },
    Updated {
        card: u64,
        version: String,
        res: Result<(), String>,
    },
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)] // short-lived, one list per input
pub enum Effect {
    /// Append to the journal (already applied to the state). The event is
    /// sb-core's JSON as is: the Rust side never decodes it (hub/codec.bend
    /// does), so a new event kind needs no Rust change.
    Journal(Value),
    Spawn {
        agent: String,
        resume: bool,
        crash_note: Option<String>,
    },
    Kill {
        agent: String,
    },
    /// A new turn: `say <text>` on the REPL socket (the agent is idle).
    Say {
        agent: String,
        text: String,
    },
    /// Steering: appended to the steering file (the agent is busy).
    Steer {
        agent: String,
        text: String,
    },
    /// A raw REPL command line (`/compact`), the agent is idle.
    Passthrough {
        agent: String,
        line: String,
    },
    /// `by`: who asked, written in the interrupt flag so the runtime's
    /// text names them ("user": the TUI; "main" or a task: `sb
    /// interrupt`; "bise": the hub itself).
    Interrupt {
        agent: String,
        by: String,
    },
    /// Rewrite the agent's BEND_CONTEXT_FILE.
    Context {
        agent: String,
        text: String,
    },
    /// A synthetic line in the agent's feed (`sb <kind> : <text>`).
    Line {
        agent: String,
        line: String,
    },
    /// The answer to an `sb` request.
    Reply {
        token: Token,
        body: Value,
    },
    /// `sb land` (dev-flow §5): the daemon runs it in a thread, in line
    /// on its land queue, then answers `token`.
    Land {
        token: Token,
        job: Box<crate::land::Job>,
    },
    ToClient {
        client: ClientId,
        body: Value,
    },
    Renamed {
        old: String,
        new: String,
    },
    /// The state changed: broadcast a snapshot.
    State,
    /// Ask a small model for a task's role line (BISE-126), off the hub's
    /// loop; the answer comes back as `Input::RoleLine`.
    AskRole {
        dir: String,
        key: String,
        request: String,
    },
    /// Ask a small model for timer `id`'s name (every_name.rs), off the
    /// hub's loop; the answer comes back as `Input::TimerName`.
    AskTimerName {
        id: u64,
        request: String,
    },
    /// The user answered a `confirm` card (approvals-design.md §9): the
    /// daemon writes the verdict to the waiting calls' gates.
    Confirm {
        card: u64,
        agent: String,
        text: String,
    },
    /// Issue #4: the answer to a spawn that asked for a model (`--model`,
    /// `--effort`, `--profile`): the daemon picks what runs (the agents
    /// default when the ask cannot), writes the task's choice file before
    /// its first call, says it in the answer `body` (`model`) and, for a
    /// fallback, in a line of the task's thread.
    SpawnModel {
        token: Token,
        agent: String,
        ask: bise_catalog::spawn::Ask,
        body: Value,
    },
    /// Issue #4: `sb send <task> --model <id> [--effort <e>]`: the task's
    /// model from its next turn (a line in its thread says who moved it);
    /// `token`: the request to answer (None: a message goes with it, its
    /// own answer is the reply).
    Switch {
        token: Option<Token>,
        from: String,
        to: String,
        model: String,
        effort: String,
    },
    /// Issue #4: the provider refused the model a task was spawned on
    /// before it ever answered (`why`: the runtime's refusal line): the
    /// daemon moves it to the agents default, says so in its thread and
    /// to main, and starts its turn again.
    ModelRefused {
        agent: String,
        why: String,
    },
    /// `/model`, `/reasoning` (BISE-135): the daemon checks the words
    /// against the catalog, writes the agent's choice (and config.toml
    /// for `default`), answers the client and broadcasts the state. No
    /// model and no effort: it says what the agent runs with.
    Choose {
        client: ClientId,
        agent: String,
        model: Option<String>,
        effort: Option<String>,
        default: bool,
    },
    /// `/flow` and `sb flow` (dev-flow §2, §7): the daemon reads the
    /// config and the detection, saves a switch, and answers the client
    /// (`client`) or the agent's request (`token`).
    Flow {
        client: Option<ClientId>,
        token: Option<Token>,
        set: Option<crate::flow::FlowMode>,
    },
    /// A PR event (pr-design §10): the daemon logs it (pr-news routes
    /// them, wave 3). The journaled ones also come as `Journal`.
    Pr(crate::forge::PrEvent),
    /// pr-design §6.3: the user's `1` on a ready-to-merge item. The
    /// daemon runs `gh pr merge` on a thread (the head pinned) and
    /// answers with `Input::Merged`.
    Merge {
        card: u64,
        place: String,
        number: u64,
        head: String,
        method: crate::place::MergeMethod,
    },
    /// dev-flow §5.1: a feature step (`sb feature <op> <name>`, or the
    /// user's answer on a feature item: `try`, `diff`, `later`, `keep`,
    /// `merge`, `drop`), run by the daemon in a thread; it answers
    /// `token` when an agent asked, then comes back as
    /// `Input::Feature`.
    Feature {
        token: Option<Token>,
        op: String,
        name: String,
        /// The feature's agents (live), each with its worktree.
        agents: Vec<(String, String)>,
    },
    /// update-card: the user's `1` on an update item: the daemon installs
    /// release `id` (`bise update` when needed), switches onto it, and
    /// answers with `Input::Updated` (`version`: its name, for the words).
    Update {
        card: u64,
        id: String,
        version: String,
    },
    /// update-card: `2 later`: the daemon keeps the id, no item again for it.
    UpdateLater {
        id: String,
    },
    /// desktop S2, bise's home hub: deliver outbox entry `xid` to the
    /// project hub `project` (a hub id; started when stopped), on a
    /// thread; it comes back as `Input::XAck` or `Input::XFail`.
    XDeliver {
        xid: u64,
        project: String,
        kind: String,
        text: String,
        /// a routed message's fn context (FnContext JSON text, "" none)
        context: String,
    },
    /// desktop S2, a project hub: main's answer to bise's message `xid`
    /// goes back to bise's hub `hub` (`xreply` on its hub.sock).
    XReply {
        hub: String,
        xid: u64,
        text: String,
    },
    /// desktop S2 step 2: a route held (or corrected: same `rid`), for
    /// the typed clients (`route`).
    Route {
        rid: u64,
        to: String,
        name: String,
        text: String,
        due_ms: u64,
        why: String,
    },
    /// S2 step 5: his unclear words, held as route `rid` with no target
    /// (1.5 s), wait for BISE_ROUTE_MODEL's pick (`route_ask`): the shell
    /// asks it and steps `Input::RoutePick`, or nothing (then they go
    /// plain to bise's main).
    RouteAsk {
        rid: u64,
        text: String,
    },
    /// Its end: `state` "sent" (`xid` its delivery) or "cancelled".
    RouteDone {
        rid: u64,
        state: String,
        to: String,
        xid: Option<u64>,
    },
    /// A feature merged into the trunk (T1 run 5 step 9): today's merged
    /// list goes again to the typed connections, as after a land (a
    /// feature merge writes no `landed` line).
    MergedChanged,
    /// S10: a followed task ended (done or failed), once: his notification.
    /// `key`: the job's (its end's time here), the same in what goes back
    /// to a hub that asked (J), so a window hearing both shows one line.
    JobEnd {
        agent: String,
        state: String,
        label: String,
        summary: String,
        key: u64,
    },
    /// J, bise's home hub: a task it follows in another project ended
    /// (`project`: that hub's id); bise's main has its line already.
    FollowedEnd {
        project: String,
        agent: String,
        key: u64,
        state: String,
        label: String,
        summary: String,
    },
}

/// What a feature step did (dev-flow §5.1), for the hub: a card to open
/// or close, agents to archive, a line for main's feed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeatureDone {
    pub name: String,
    /// (kind, text): open this item (place `feature:<name>`), closing the
    /// feature's other open items first.
    pub open: Option<(String, String)>,
    /// Close the feature's open items (with this result).
    pub close: Option<String>,
    pub archive: Vec<String>,
    /// (kind, text) for main's feed: `info`, `warn`.
    pub line: Option<(String, String)>,
}

/// The sb-core executable when the app root has none: `sb-core` next to
/// the executable; a debug build (the tests) also tries the root of the
/// repository it was built from. No build-machine path in a release build
/// (BISE-163, packaging.md C4). The hub gets its sb-core from the harness
/// (`daemon::Opts::core_bin`: the app root's), never from the environment:
/// an inherited `SB_CORE_BIN` once ran a stale sb-core on every version
/// after it (5d136677).
pub fn default_core_bin() -> std::path::PathBuf {
    let next_to_exe = std::env::current_exe()
        .ok()
        .map(|e| std::fs::canonicalize(&e).unwrap_or(e))
        .and_then(|e| e.parent().map(|d| d.join("sb-core")));
    #[cfg(debug_assertions)]
    {
        let dev = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sb-core");
        if !next_to_exe.as_ref().is_some_and(|p| p.exists()) {
            return dev;
        }
    }
    next_to_exe.unwrap_or_else(|| "sb-core".into())
}

/// One sb-core process and its connection.
pub struct CoreLink {
    child: Child,
    _out: BufReader<ChildStdout>,
    w: TcpStream,
    r: BufReader<TcpStream>,
}

impl CoreLink {
    /// Start sb-core on a free port. A port taken meanwhile (parallel
    /// hubs) is retried on another one.
    pub fn start(bin: &Path) -> std::io::Result<CoreLink> {
        let mut last = None;
        for _ in 0..5 {
            match CoreLink::try_start(bin) {
                Ok(l) => return Ok(l),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap())
    }

    fn try_start(bin: &Path) -> std::io::Result<CoreLink> {
        let port = std::net::TcpListener::bind("127.0.0.1:0")?
            .local_addr()?
            .port();
        let mut cmd = Command::new(bin);
        bise_home::env::for_child(bise_home::env::Child::Core, [("SB_CORE_PORT", port.to_string())]).apply(&mut cmd);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // it lives as long as the hub: a pipe of a concurrent spawn must
        // not stay open in it (BISE-291)
        crate::procs::no_leaked_fds(&mut cmd);
        let mut child = cmd.spawn()?;
        let mut out = BufReader::new(child.stdout.take().expect("stdout"));
        let mut banner = String::new();
        out.read_line(&mut banner)?;
        let conn = if banner.starts_with("sb-core on") {
            TcpStream::connect(("127.0.0.1", port))
        } else {
            Err(std::io::Error::other(format!("sb-core did not start: {:?}", banner)))
        };
        let w = match conn {
            Ok(w) => w,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        w.set_nodelay(true)?;
        let r = BufReader::new(w.try_clone()?);
        Ok(CoreLink {
            child,
            _out: out,
            w,
            r,
        })
    }

    /// One input line, one answer line; an error when sb-core is gone
    /// (or answers something that is not JSON).
    pub fn call(&mut self, v: &Value) -> std::io::Result<Value> {
        let mut line = v.to_string();
        line.push('\n');
        self.w.write_all(line.as_bytes())?;
        let mut back = String::new();
        if self.r.read_line(&mut back)? == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "its connection closed"));
        }
        serde_json::from_str(&back)
            .map_err(|e| std::io::Error::other(format!("a bad answer {:?}: {}", clip(&back, 200), e)))
    }

    /// Tests only: kill sb-core under the hub.
    #[cfg(test)]
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for CoreLink {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Clone, Debug)]
struct ClientView {
    focus: String,
    since_ms: u64,
    sent: Vec<String>,
}

pub struct Hub {
    /// The mirror of sb-core's durable state (plus the runtime fields it
    /// reports), for the views.
    pub st: State,
    pub workspace: String,
    clients: BTreeMap<ClientId, ClientView>,
    /// Drop confirmations pending on a client: id -> (client, task).
    confirms: BTreeMap<u64, (ClientId, String)>,
    next_confirm: u64,
    /// (time, text) of recent assistant messages, for direct-exchange
    /// excerpts.
    recent: BTreeMap<String, VecDeque<(u64, String)>>,
    contexts: BTreeMap<String, String>,
    /// The last thing each agent did (from its REPL lines, for the views).
    activity: BTreeMap<String, (u64, String)>,
    /// The role line of each task, by dir (BISE-126).
    roles: BTreeMap<String, Role>,
    /// The timers' name calls (every_name.rs); runtime only.
    timer_names: crate::every_name::Asker,
    /// BISE-136: the private worktree each agent works in, by dir (a
    /// rename keeps it); runtime only, like `activity`.
    places: BTreeMap<String, Place>,
    /// Agents whose call waits on a `confirm` card (approvals-design.md
    /// §10): shown `waiting` on `you`; runtime only, like `activity`.
    on_you: BTreeSet<String>,
    /// dev-flow §3.1: the PR of each place, by place id (pr-hub, wave 2;
    /// empty until then) and the held line of a place (the land queue's
    /// `waits to land · 2nd`, set by the daemon). Runtime only.
    pub prs: BTreeMap<String, crate::place::PrSnapshot>,
    pub lids: BTreeMap<String, String>,
    /// The repo's `[flow] mode` (None: not set), for the views' held
    /// header (`lands via PRs` / `lands on main`); the daemon reads it.
    pub flow: Option<crate::flow::FlowMode>,
    /// dev-flow §5.1, set by the daemon: each feature's held lid by place
    /// id (`feature · 14 commits · not tried`), and the ones whose try
    /// build builds or is on trial (the `Δ`). Runtime only.
    pub feature_lids: BTreeMap<String, String>,
    pub trying: BTreeSet<String>,
    /// desktop S2, set by the daemon: this is bise's home hub, so main
    /// reads every user message with its id (`prompts::user_message`).
    pub user_ids: bool,
    /// pr-hub (pr-design §7-§10), runtime but `known`: what the journal
    /// says of each place's PR (its number; `pr_*` lines, read back by
    /// `replay`); `pr_lids`: the held line of a worktree with no PR yet
    /// (`no PR yet · 2 commits`; the land queue's `lids` win);
    /// `pr_ok_ms`: when the forge last answered for a place; `pr_late`:
    /// its last ask failed (the boxes go faint); `pr_done`: places whose
    /// merged PR was cleaned up (or kept, said once).
    pr_known: BTreeMap<String, crate::forge::Known>,
    pr_lids: BTreeMap<String, String>,
    pr_ok_ms: BTreeMap<String, u64>,
    pub pr_late: Option<crate::forge::ForgeError>,
    pr_done: BTreeSet<String>,
    /// pr-news (pr-design §6): who owns each PR, what was passed on, the
    /// checks' tries (forge/news.rs); `pr_bots`: the repo's `[pr]
    /// trusted_bots` (the daemon reads config.toml).
    pr_news: crate::forge::news::News,
    pub pr_bots: Vec<String>,
    /// pr-merge (pr-design §6.3): the ready-to-merge items' runtime side
    /// (merge.rs).
    merges: merge::Merges,
    /// update-card: the new-release item's runtime side (update_card.rs).
    updates: update_card::Updates,
    /// expired-ux: the agents waiting for the ChatGPT sign-in
    /// (signin_card.rs).
    signin: signin_card::SignIn,
    dirty: bool,
    link: CoreLink,
    /// The sb-core it runs (and starts again after a death).
    core_bin: PathBuf,
    /// How to bring sb-core back when it dies (the daemon's; none: a
    /// death panics, as at boot and in most tests).
    revive: Option<Revive>,
    /// Lines for main's feed from a revival, out at the next input.
    revived: Vec<String>,
    /// Who asked for the interrupt sb-core is deciding on (the user, an
    /// agent); none: the hub's own (`Effect::Interrupt::by`).
    interrupt_by: Option<String>,
    /// Issue #4, set by the daemon from the choice files: each agent's
    /// model for `sb list`, `sb tasks` and the roster.
    pub models: board::Models,
    /// Issue #4: the tasks on a model asked at their spawn (or by `sb
    /// send --model`) that has not answered a turn yet: a provider that
    /// refuses it moves them to the agents default (`Effect::ModelRefused`).
    /// Set by the daemon, cleared at the agent's first turn end.
    pub model_trial: BTreeSet<String>,
    /// The spawns that asked for a model, by request token: their answer
    /// waits for the daemon's pick (`Effect::SpawnModel`).
    spawn_asks: BTreeMap<Token, bise_catalog::spawn::Ask>,
}

/// What the hub needs to restart a dead sb-core (BISE-292): the journal
/// (the durable state, replayed into the new one) and hub.log.
pub struct Revive {
    pub journal: Box<dyn FnMut() -> Vec<Value>>,
    pub log: Box<dyn Fn(&str)>,
    /// When the last restarts happened (ms): a crash loop gives up.
    times: Vec<u64>,
}

impl Revive {
    pub fn new(journal: Box<dyn FnMut() -> Vec<Value>>, log: Box<dyn Fn(&str)>) -> Revive {
        Revive { journal, log, times: Vec::new() }
    }
}

/// More restarts than this in `REVIVE_WINDOW_MS`: sb-core dies on its
/// state, not by accident; the hub stops (as before BISE-292).
/// A PR the forge has not answered about for this long shows as stale
/// (pr-design §7; pr-hub sets it from its cadence).
pub const PR_STALE_MS: u64 = 10 * 60 * 1000;

const REVIVE_LIMIT: usize = 3;
/// Journal events per `replay_many` line at boot (~50 KB). sb-core's
/// parse of a line grows faster than its length: on a 37.7k-event
/// journal, 500 per line replayed in 17 s, 5000 in 23 s, 100 in 12 s.
const REPLAY_BATCH: usize = 100;
const REVIVE_WINDOW_MS: u64 = 60_000;

type Fx = Vec<Effect>;

/// Where an agent said it works (BISE-136), or where its bash went.
#[derive(Clone, Debug, Default)]
struct Place {
    /// The private worktree; "" = the agent's own workspace.
    path: String,
    /// It came from `sb worktree` (gate.sh): the bash fallback no
    /// longer guesses for this agent.
    told: bool,
    /// The branch checked out there, as the PR poller last read it
    /// (None: detached, as `gate.sh new` makes it, or not read yet).
    branch: Option<String>,
}

/// BISE-136 fallback: the linked git worktree a bash call starts in
/// (`cd /tmp/x-wt && …`), when it is not `own` (the agent's workspace).
/// A linked worktree has a `.git` file (the main checkout a directory).
fn bash_worktree(args: &str, own: &str) -> Option<String> {
    // the bare command, or the JSON of a call with a description (BISE-223)
    let cmd = match serde_json::from_str::<Value>(args) {
        Ok(v) => v["arg"].as_str()?.to_string(),
        Err(_) => args.to_string(),
    };
    let rest = cmd.trim_start().strip_prefix("cd ")?;
    let path = rest.split(|c: char| c.is_whitespace() || c == ';' || c == '&' || c == '|').next()?;
    let path = path.trim_matches(|c| c == '"' || c == '\'').trim_end_matches('/');
    if !path.starts_with('/') || path == own.trim_end_matches('/') {
        return None;
    }
    std::path::Path::new(path).join(".git").is_file().then(|| path.to_string())
}

/// A task's role line (BISE-126) and its calls.
#[derive(Clone, Debug, Default)]
struct Role {
    /// The last line a model gave; None: the objective's first sentence.
    line: Option<String>,
    /// What `line` was made from (role::key).
    key: String,
    /// The key of the call in flight.
    asking: Option<String>,
    /// A turn ended during the call: look again when it is over.
    pending: bool,
    /// The last failed call: no new one for ROLE_RETRY_MS.
    failed_ms: Option<u64>,
}

/// After a failed role-line call, the next one waits this long.
const ROLE_RETRY_MS: u64 = 5 * 60 * 1000;

fn line(agent: &str, kind: &str, text: &str) -> Effect {
    Effect::Line {
        agent: agent.to_string(),
        line: format!("sb {} : {}", kind, wire_escape(text)),
    }
}

/// The separator of the fields of a hub line (C2: `answered`).
pub const FIELD_SEP: &str = " : ";

/// A field of a multi-field hub line: a `" : "` inside it becomes
/// `" \\: "`, so the reader splits on the real separators only (the TUI's
/// `parse_hub_line` undoes it).
pub fn field_escape(s: &str) -> String {
    s.replace(FIELD_SEP, " \\: ")
}

/// The text of a hub line made of fields (C2 `answered`: agent,
/// question, answer, why).
pub fn join_fields(fields: &[String]) -> String {
    fields.iter().map(|f| field_escape(f)).collect::<Vec<_>>().join(FIELD_SEP)
}

fn notice(client: ClientId, text: &str) -> Effect {
    Effect::ToClient {
        client,
        body: json!({"ev": "notice", "text": text}),
    }
}

fn parse<T: serde::de::DeserializeOwned>(x: &Value) -> Option<T> {
    T::deserialize(x).ok()
}

fn run_of(s: &str) -> Run {
    match s {
        "starting" => Run::Starting,
        "idle" => Run::Idle,
        "busy" => Run::Busy,
        _ => Run::Down,
    }
}

impl Hub {
    /// The tests' hub: their sb-core is the test setting `SB_CORE_BIN`
    /// (the gate's cache file), else [`default_core_bin`].
    #[cfg(test)]
    pub fn new(workspace: &str) -> Hub {
        let bin = bise_home::env::test_setting("SB_CORE_BIN").map(PathBuf::from).unwrap_or_else(default_core_bin);
        // a test on an sb-core of other sources lies both ways (core_fresh.rs)
        crate::core_fresh::check(&bin);
        Hub::with_core(workspace, bin)
    }

    /// A hub whose decisions run in the sb-core at `core_bin`.
    pub fn with_core(workspace: &str, core_bin: PathBuf) -> Hub {
        let mut link = CoreLink::start(&core_bin).unwrap_or_else(|e| {
            panic!("sb-core not found ({}): {}", core_bin.display(), e)
        });
        link.call(&json!({"t": "init", "workspace": workspace}))
            .unwrap_or_else(|e| panic!("sb-core: {}", e));
        let mut hub = Hub {
            st: State::new(workspace),
            workspace: workspace.to_string(),
            clients: BTreeMap::new(),
            confirms: BTreeMap::new(),
            next_confirm: 1,
            recent: BTreeMap::new(),
            contexts: BTreeMap::new(),
            activity: BTreeMap::new(),
            roles: BTreeMap::new(),
            timer_names: Default::default(),
            places: BTreeMap::new(),
            on_you: BTreeSet::new(),
            prs: BTreeMap::new(),
            lids: BTreeMap::new(),
            flow: None,
            feature_lids: BTreeMap::new(),
            trying: BTreeSet::new(),
            user_ids: false,
            pr_known: BTreeMap::new(),
            pr_lids: BTreeMap::new(),
            pr_ok_ms: BTreeMap::new(),
            pr_late: None,
            pr_done: BTreeSet::new(),
            pr_news: Default::default(),
            pr_bots: Vec::new(),
            merges: merge::Merges::default(),
            updates: update_card::Updates::default(),
            signin: signin_card::SignIn::default(),
            dirty: false,
            link,
            core_bin,
            revive: None,
            revived: Vec::new(),
            interrupt_by: None,
            models: BTreeMap::new(),
            model_trial: BTreeSet::new(),
            spawn_asks: BTreeMap::new(),
        };
        hub.view_all();
        hub
    }

    /// Tests only: put an agent's REPL in a given state (never sent by
    /// the daemon).
    #[cfg(test)]
    pub fn force_run(&mut self, agent: &str, run: Run) {
        let r = match run {
            Run::Down => "down",
            Run::Starting => "starting",
            Run::Idle => "idle",
            Run::Busy => "busy",
        };
        if let Some(out) = self.call(&json!({"t": "force_run", "agent": agent, "run": r})) {
            self.load_view(&out["view"]);
        }
    }

    /// Rebuild the durable state from the journal: sb-core replays it,
    /// then sends the whole state. Returns the events sb-core did not
    /// apply (a kind it does not know: the journal was written by a newer
    /// hub before a /version rollback), for the caller to log.
    /// Batches of REPLAY_BATCH events (`replay_many`): one event per call
    /// was quadratic in the messages (a 20k-event journal took ~18 s at
    /// boot, past the switcher's wait).
    pub fn replay(&mut self, events: &[Value]) -> Vec<Value> {
        self.replay_with(events, &mut |_, _| {})
    }

    /// `replay`, telling `progress(done, total)` after each batch (the
    /// boot watch, daemon/boot.rs: a long replay is progress, not a hang).
    pub fn replay_with(&mut self, events: &[Value], progress: &mut dyn FnMut(usize, usize)) -> Vec<Value> {
        let mut skipped = Vec::new();
        let mut core: Vec<&Value> = Vec::new();
        for ev in events {
            // the hub's own lines (the PR numbers): never sb-core's
            if crate::forge::is_pr_line(ev) {
                crate::forge::read_line(&mut self.pr_known, ev);
                self.pr_news.read_journal(ev);
                continue;
            }
            core.push(ev);
        }
        let (mut done, total) = (0, core.len());
        for batch in core.chunks(REPLAY_BATCH) {
            let out = self.raw(&json!({"t": "replay_many", "evs": batch}));
            for i in out["skipped"].as_array().into_iter().flatten().filter_map(Value::as_u64) {
                if let Some(ev) = batch.get(i as usize) {
                    skipped.push((*ev).clone());
                }
            }
            done += batch.len();
            progress(done, total);
        }
        self.view_all();
        skipped
    }

    fn view_all(&mut self) {
        if let Some(out) = self.call(&json!({"t": "view_all"})) {
            self.load_view(&out["view"]);
        }
    }

    /// The agents a client is looking at (a REPL start for them goes
    /// first, daemon/repl_starts.rs).
    pub fn focused(&self) -> BTreeSet<String> {
        self.clients.values().map(|c| c.focus.clone()).collect()
    }

    /// sb-core's pid (a stuck boot stops it with the hub).
    pub fn core_pid(&self) -> u32 {
        self.link.child.id()
    }

    /// Restart sb-core when it dies (see `Revive`); the daemon sets it
    /// once the journal is replayed.
    pub fn set_revive(&mut self, r: Revive) {
        self.revive = Some(r);
    }

    /// Tests only: kill sb-core under the hub.
    #[cfg(test)]
    pub fn kill_core(&mut self) {
        self.link.kill();
    }

    /// One call to sb-core, with no revival: its death panics.
    fn raw(&mut self, v: &Value) -> Value {
        self.link.call(v).unwrap_or_else(|e| panic!("sb-core: {}", e))
    }

    /// One call to sb-core. It died: restart it on the journal and run
    /// the input again, once; dead again on it, the input is dropped
    /// (None) and sb-core restarted once more (BISE-292).
    fn call(&mut self, v: &Value) -> Option<Value> {
        let e = match self.link.call(v) {
            Ok(out) => return Some(out),
            Err(e) => e,
        };
        self.restart_core(&e.to_string());
        match self.link.call(v) {
            Ok(out) => Some(out),
            Err(e) => {
                let what = v["t"].as_str().unwrap_or("?").to_string();
                self.restart_core(&e.to_string());
                self.revived.push(format!("sb-core stopped twice on the same input ({}): that input was dropped", what));
                None
            }
        }
    }

    /// A new sb-core with the durable state of the journal and the REPL
    /// states of the old one (the REPLs themselves never stopped).
    fn restart_core(&mut self, why: &str) {
        let Some(r) = self.revive.as_mut() else {
            panic!("sb-core: {}", why);
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        r.times.retain(|t| now.saturating_sub(*t) < REVIVE_WINDOW_MS);
        if r.times.len() >= REVIVE_LIMIT {
            (r.log)(&format!("sb-core stopped ({}): {} restarts in {} s, the hub stops", why, REVIVE_LIMIT, REVIVE_WINDOW_MS / 1000));
            panic!("sb-core: {} ({} restarts in {} s)", why, REVIVE_LIMIT, REVIVE_WINDOW_MS / 1000);
        }
        r.times.push(now);
        (r.log)(&format!("sb-core stopped ({}): restarting it on the journal", why));
        let events = (r.journal)();
        self.link = CoreLink::start(&self.core_bin).unwrap_or_else(|e| panic!("sb-core: {} (and it cannot restart: {})", why, e));
        let runs: Vec<(String, Run)> = self
            .st
            .agents
            .values()
            .filter(|a| a.run != Run::Down)
            .map(|a| (a.name.clone(), a.run))
            .collect();
        self.raw(&json!({"t": "init", "workspace": self.workspace}));
        let skipped = self.replay(&events).len();
        for (agent, run) in runs {
            let r = match run {
                Run::Down => "down",
                Run::Starting => "starting",
                Run::Idle => "idle",
                Run::Busy => "busy",
            };
            self.raw(&json!({"t": "force_run", "agent": agent, "run": r}));
        }
        let out = self.raw(&json!({"t": "view_all"}));
        self.load_view(&out["view"]);
        if let Some(r) = self.revive.as_ref() {
            (r.log)(&format!("sb-core restarted: {} journal events replayed ({} skipped)", events.len(), skipped));
        }
        self.revived.push(format!(
            "sb-core, the hub's state machine, stopped ({}): restarted, its state rebuilt from the journal. the agents kept running",
            why
        ));
    }

    /// Store the state sb-core sent (it is the only source of truth).
    fn load_view(&mut self, v: &Value) {
        let mut agents = BTreeMap::new();
        for a in v["agents"].as_array().into_iter().flatten() {
            let name = jstr(a, "name");
            let declared = a["declared"]
                .as_object()
                .and_then(|d| parse(&d["status"]).map(|st| (st, jstr(&a["declared"], "note"))));
            let agent = Agent {
                name: name.clone(),
                dir: jstr(a, "dir"),
                is_main: a["is_main"].as_bool().unwrap_or(false),
                parent: a["parent"].as_str().map(|x| x.to_string()),
                brief: parse(&a["brief"]).unwrap_or_default(),
                created_ms: a["created_ms"].as_u64().unwrap_or(0),
                ws: parse(&a["ws"]).unwrap_or_else(|| panic!("sb-core: bad ws {}", a["ws"])),
                lifecycle: parse(&a["lifecycle"]).unwrap_or(Lifecycle::Active),
                failure: a["failure"].as_str().map(|x| x.to_string()),
                declared,
                last_report: parse(&a["last_report"]),
                aliases: parse(&a["aliases"]).unwrap_or_default(),
                files: parse(&a["files"]).unwrap_or_default(),
                snapshot_ref: a["snapshot_ref"].as_str().map(|x| x.to_string()),
                follow: a["follow"].as_bool().unwrap_or(false),
                run: run_of(&jstr(a, "run")),
                waiting: a["waiting"].as_bool().unwrap_or(false),
                waiting_on: a["waiting_on"].as_str().map(|x| x.to_string()),
                turn_started_ms: a["turn_ms"].as_u64(),
                turns_ended: a["turns"].as_u64().unwrap_or(0),
                activity: self.activity.get(&name).cloned(),
                place: None,
                place_branch: None,
            };
            let mut agent = agent;
            agent.place = self.place_of(&agent);
            agent.place_branch = self.place_branch_of(&agent);
            if self.on_you.contains(&agent.name) {
                agent.waiting = true;
                agent.waiting_on = Some("you".into());
            }
            agents.insert(name, agent);
        }
        // idle-cpu: the order, the cards and the notes change only by an
        // event: a step with none leaves them out (view.bend `durable_kvs`)
        if let Some(order) = v.get("order") {
            self.st.order = parse(order).unwrap_or_default();
        }
        // hub-lag, idle-cpu: a step's view leaves out the agents it did
        // not change (view.bend `changed`; an idle tick sends none): keep
        // them as they are, minus the ones gone from the order (a rename)
        if v["all_agents"].as_bool() == Some(false) {
            let order: BTreeSet<&String> = self.st.order.iter().collect();
            for (name, a) in std::mem::take(&mut self.st.agents) {
                if order.contains(&name) && !agents.contains_key(&name) {
                    agents.insert(name, a);
                }
            }
        }
        self.st.agents = agents;
        if v["all_msgs"].as_bool() == Some(true) {
            self.st.msgs.clear();
            self.st.msg_state.clear();
            self.st.settled.clear();
        }
        for r in v["msgs"].as_array().into_iter().flatten() {
            let Some(m) = parse(&r["msg"]) else { continue };
            let m: Msg = m;
            if let Some(state) = parse(&r["state"]) {
                self.st.msg_state.insert(m.id, state);
            }
            if r["settled"].as_bool() == Some(true) {
                self.st.settled.insert(m.id);
            } else {
                self.st.settled.remove(&m.id);
            }
            self.st.msgs.insert(m.id, m);
        }
        if let Some(cards) = v.get("cards") {
            self.st.cards = cards
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| parse(c).map(|c: Card| (c.id, c)))
                .collect();
        }
        if let Some(notes) = v.get("notes") {
            self.st.main_notes = parse(notes).unwrap_or_default();
        }
        // sb every's timers: only in a view whose step changed them
        if let Some(t) = v.get("timers") {
            self.st.timers.load_live(t);
        }
        if let Some(t) = v.get("timers_ended") {
            self.st.timers.load_ended(t);
        }
        self.st.next_msg = v["next_msg"].as_u64().unwrap_or(1);
        self.st.next_card = v["next_card"].as_u64().unwrap_or(1);
    }

    /// The agent's last activity, for the views (not a decision).
    fn set_activity(&mut self, agent: &str, now: u64, what: String) {
        if let Some(a) = self.st.agents.get_mut(agent) {
            a.activity = Some((now, what.clone()));
            self.activity.insert(agent.to_string(), (now, what));
        }
    }
    /// The approvals gate (approvals-design.md §10): `agent`'s call waits
    /// on the user (a card), or no longer does. The views say it at once.
    pub fn set_on_you(&mut self, agent: &str, on: bool) {
        let changed = if on { self.on_you.insert(agent.to_string()) } else { self.on_you.remove(agent) };
        if let Some(a) = self.st.agents.get_mut(agent).filter(|_| changed) {
            a.waiting = on;
            a.waiting_on = on.then(|| "you".to_string());
        }
    }

    /// BISE-136: the private worktree `a` works in (None: its own
    /// workspace, shared checkout or hub worktree).
    /// `sb move <agent> <place>`: the target's ws (None: a new worktree,
    /// /isolate's path). An agent alone in its worktree does not leave
    /// it: the folder would be left behind with no one (drop it instead).
    fn move_target(&self, agent: &str, place: &str) -> Result<Option<Workspace>, String> {
        let name = self.st.resolve(agent).ok_or_else(|| format!("no agent named {}", agent))?;
        let a = &self.st.agents[&name];
        let target = match place {
            "new" => None,
            crate::place::SHARED => self.st.agents.get(MAIN).map(|m| m.ws.clone()),
            p => Some(crate::place::find_worktree(&self.st, p)?),
        };
        if a.ws.mode == Mode::Worktree && !a.ws.dropped {
            let id = a.ws.place_id(&a.dir);
            let others = crate::place::places(&self.st, &BTreeMap::new())
                .into_iter()
                .find(|p| p.id == id)
                .is_some_and(|p| p.agents.iter().any(|n| *n != name));
            let same = target.as_ref().is_some_and(|w| w.place_id(&a.dir) == id);
            if !others && !same {
                return Err(format!(
                    "@{} is alone in its worktree: moving it would leave the folder behind; drop it instead",
                    name
                ));
            }
        }
        Ok(target)
    }

    /// A feature step ended (dev-flow §5.1): its items replaced or
    /// closed, its agents archived (a merge, a drop), main's feed told.
    fn feature_done(&mut self, fx: &mut Fx, env: &mut dyn Env, d: FeatureDone) {
        let place = crate::feature::place_id(&d.name);
        if d.open.is_some() || d.close.is_some() {
            let open: Vec<u64> = self
                .st
                .open_cards()
                .filter(|c| c.place.as_deref() == Some(place.as_str()))
                .map(|c| c.id)
                .collect();
            let res = d.close.clone().unwrap_or_else(|| "replaced".into());
            for id in open {
                self.close_card(fx, env, id, &res);
            }
        }
        if let Some((kind, text)) = &d.open {
            self.open_card(
                fx,
                env,
                merge::HubItem { kind, agent: MAIN, text, place: &place, pr: None },
            );
        }
        for a in &d.archive {
            self.core(fx, env, None, json!({"t": "drop", "name": a, "force": true}));
        }
        if let Some((kind, text)) = &d.line {
            fx.push(line(MAIN, kind, text));
        }
        // the merge reached the trunk: the changes tab's merged today again
        if d.close.as_deref() == Some("merged") {
            fx.push(Effect::MergedChanged);
        }
        self.dirty = true;
    }

    /// The user's number on a feature item (pr-merge's choice cards,
    /// dev-flow §5.1): `feature_try` 1 try it / 2 show the diff / 3 not
    /// yet; `feature_merge` 1 merge / 2 keep working / 3 drop the branch
    /// (the TUI asked "drop it?" once more before sending 3). The card
    /// stays open until the step ends (`feature_done` closes or replaces
    /// it). A number out of range: nothing.
    fn feature_choice(&mut self, fx: &mut Fx, kind: &str, place: Option<&str>, choice: &str) {
        let Some(name) = place.and_then(crate::feature::of_place) else { return };
        let op = match (kind, choice.trim()) {
            (crate::feature::TRY, "1") => "try",
            (crate::feature::TRY, "2") => "diff",
            (crate::feature::TRY, "3") => "later",
            (crate::feature::MERGE, "1") => "merge",
            (crate::feature::MERGE, "2") => "keep",
            (crate::feature::MERGE, "3") => "drop",
            _ => return,
        };
        let agents = self.feature_agents(name);
        fx.push(Effect::Feature { token: None, op: op.into(), name: name.into(), agents });
    }

    /// The live agents of feature `name` with their worktrees (dev-flow
    /// §5.1), in the hub's order.
    pub fn feature_agents(&self, name: &str) -> Vec<(String, String)> {
        self.st
            .order
            .iter()
            .filter_map(|n| self.st.agents.get(n))
            .filter(|a| a.lifecycle != Lifecycle::Archived && a.ws.feature() == Some(name))
            .map(|a| (a.name.clone(), a.ws.path.clone()))
            .collect()
    }

    /// `sb spawn --feature <f>` (dev-flow §5.1): the task's name as sb-core
    /// will pick it (the next free one, `fix-2`), then its worktree from
    /// the feature's tip, made now so sb-core takes it as the place to
    /// join.
    fn feature_worktree(&self, env: &mut dyn Env, name: Option<&str>, brief: &Brief, feature: &str) -> Result<Workspace, String> {
        if self.flow == Some(crate::flow::FlowMode::Pr) {
            return Err("this repo ships through pull requests: a feature is a PR's branch here (`--place new`)".into());
        }
        let base = new_task(name, brief, true, false)?["base"].as_str().unwrap_or_default().to_string();
        let free = unique_name(&self.st, &base);
        env.worktree_feature(&free, feature)
    }

    /// `sb land` (dev-flow §5): what the land needs from the state. The
    /// daemon adds the repo's flow and runs it (`land::run`).
    fn land_job(&self, from: &str, here: bool, message: &str, add: Vec<String>) -> Result<crate::land::Job, String> {
        let name = self.st.resolve(from).ok_or_else(|| format!("unknown agent: {}", from))?;
        let a = &self.st.agents[&name];
        let place = a.ws.place_id(&a.dir);
        let others = self
            .st
            .agents
            .values()
            .filter(|b| b.name != name && b.lifecycle != Lifecycle::Archived)
            .filter(|b| b.ws.place_id(&b.dir) == place)
            .map(|b| (b.name.clone(), b.files.iter().cloned().collect()))
            .collect();
        Ok(crate::land::Job {
            agent: name.clone(),
            here,
            message: message.trim().to_string(),
            worktree: a.ws.mode == Mode::Worktree,
            place,
            dir: std::path::PathBuf::from(&a.ws.path),
            shared: std::path::PathBuf::from(&self.workspace),
            files: a.files.iter().cloned().collect(),
            others,
            add,
            since_ms: a.created_ms,
            flow: crate::flow::FlowConfig::default(),
            // a feature's agent lands on the feature (dev-flow §5.1)
            onto: a.ws.feature().map(|f| format!("refs/heads/{}", f)),
        })
    }

    /// `sb every`'s timers (the state's `timers`, the pages' `watch`).
    pub fn timers(&self) -> &crate::every::Timers {
        &self.st.timers
    }

    /// `sb every` (every.rs): set, list or stop a timer; the reply. The
    /// request's checks and words are here; the timer is sb-core's
    /// (`every_set`, `every_stop` inputs, hub/timers.bend).
    fn timer_req(&mut self, fx: &mut Fx, env: &mut dyn Env, from: &str, r: EveryReq) -> Value {
        let now = env.now();
        let Some(by) = self.st.resolve(from) else {
            return json!({"ok": false, "error": format!("unknown agent: {}", from)});
        };
        match r {
            EveryReq::List => json!({"ok": true, "text": self.st.timers.list(now)}),
            EveryReq::Show(id) => match self.st.timers.map.get(&id) {
                Some(t) => json!({"ok": true, "text": crate::every::show(t, now)}),
                None => json!({"ok": false, "error": format!("no timer #{} (`sb every` lists them)", id)}),
            },
            EveryReq::Stop(id) => {
                let no = json!({"ok": false, "error": format!("no timer #{} (`sb every` lists them)", id)});
                if !self.st.timers.map.contains_key(&id) {
                    return no;
                }
                self.core(fx, env, None, json!({"t": "every_stop", "id": id, "why": format!("stopped by {}", by), "wake": ""}));
                if self.st.timers.map.contains_key(&id) {
                    return no;
                }
                json!({"ok": true, "text": format!("timer #{} stopped", id)})
            },
            EveryReq::Add { to, text, sched, until_ms, times, page, name } => {
                let to = if to.is_empty() { by.clone() } else { to };
                let agent = match self.st.resolve(&to) {
                    Some(a) if self.st.agents[&a].lifecycle == Lifecycle::Active => a,
                    _ => return json!({"ok": false, "error": format!("sb every: no active agent @{}", to)}),
                };
                if matches!(sched, crate::every::Sched::Every(p) if p < crate::every::min_ms(bise_home::env::test_setting("SB_EVERY_MIN_MS").as_deref())) {
                    return json!({"ok": false, "error": "sb every: at least 1m between two wakes"});
                }
                if text.trim().is_empty() {
                    return json!({"ok": false, "error": "sb every: the message is empty"});
                }
                if until_ms.is_some_and(|u| u <= now) {
                    return json!({"ok": false, "error": "sb every: --until is already past"});
                }
                if times == Some(0) {
                    return json!({"ok": false, "error": "sb every: --times is at least 1"});
                }
                let n = crate::every::New { agent, by, text: text.trim().to_string(), sched, until_ms, times, page, name };
                // sb-core gives the id: its every_set journal line's
                let k = fx.len();
                self.core(fx, env, None, n.input(now));
                let set_id = |e: &Effect| match e {
                    Effect::Journal(j) if j["type"] == "every_set" => j["id"].as_u64(),
                    _ => None,
                };
                let Some(id) = fx[k..].iter().find_map(set_id) else {
                    return json!({"ok": false, "error": format!("sb every: no active agent @{}", to)});
                };
                let line = self.st.timers.list(now).lines().find(|l| l.starts_with(&format!("#{} ", id))).unwrap_or_default().to_string();
                json!({"ok": true, "id": id, "text": format!("timer set: {} (stop it: sb every --stop {})", line, id)})
            }
        }
    }

    /// A timer's line in the feeds (`sb scheduled : <json>`, the TUI's ◷
    /// line): its agent's feed, and main's when main set it for another
    /// agent (main's thread shows the timers main sets, never another
    /// agent's wakes). `ev`: the timer's state json with `ev` set/end.
    fn timer_lines(&self, fx: &mut Fx, agent: &str, by: &str, ev: &Value) {
        fx.push(line(agent, "scheduled", &ev.to_string()));
        if by == MAIN && agent != MAIN {
            // main's copy says so: its ended line names the agent
            let mut m = ev.clone();
            m["in"] = json!(MAIN);
            fx.push(line(MAIN, "scheduled", &m.to_string()));
        }
    }

    /// The ◷ line of a timer sb-core just set or ended (its `every_set` or
    /// `every_stop`; the step's view, loaded first, has the timer).
    fn timer_journal_lines(&self, fx: &mut Fx, ev: &Value) {
        let id = ev["id"].as_u64().unwrap_or(0);
        let (t, mut v, what) = match ev["type"].as_str() {
            Some("every_set" | "every_name") if crate::every_name::set_line_now(ev) => match self.st.timers.map.get(&id) {
                Some(t) => (t, t.json(), "set"),
                None => return,
            },
            Some("every_stop") => match self.st.timers.ended.iter().rev().find(|e| e.timer.id == id) {
                Some(e) => (&e.timer, e.json(), "end"),
                None => return,
            },
            // a refused wake (fire_is_a_message): tried again in a minute
            Some("every_fired") if ev["undelivered"] == true => {
                if let Some(t) = self.st.timers.map.get(&id) {
                    let w = format!("timer #{id}: its wake did not reach @{}; tried again in a minute", t.agent);
                    fx.push(line(MAIN, "warn", &w));
                }
                return;
            }
            _ => return,
        };
        v["ev"] = json!(what);
        self.timer_lines(fx, &t.agent, &t.by, &v);
    }

    /// `/scheduled`'s stop (the user): the timer ends, its agent hears it
    /// from bise once in the same sb-core step, the ◷ line says so.
    fn timer_stop_by_user(&mut self, fx: &mut Fx, env: &mut dyn Env, id: u64, why: &str) {
        let Some(t) = self.st.timers.map.get(&id) else { return };
        let wake = crate::every::stop_note(id, &t.text, why);
        self.core(fx, env, None, json!({"t": "every_stop", "id": id, "why": "stopped by the user", "wake": wake}));
    }

    /// `/scheduled`'s run now: one wake of timer `id` at once, outside its
    /// count, through sb-core's only wake path (timer_fire: never a second
    /// wake while one from bise is queued).
    fn timer_run_now(&mut self, fx: &mut Fx, env: &mut dyn Env, id: u64) {
        if let Some(text) = self.st.timers.run_text(id, env.now()) {
            self.core(fx, env, None, json!({"t": "every_run", "id": id, "text": text}));
        }
    }

    fn place_of(&self, a: &Agent) -> Option<String> {
        let p = &self.places.get(&a.dir)?.path;
        (!p.is_empty() && *p != a.ws.path).then(|| p.clone())
    }

    /// The branch checked out in `a`'s private worktree, as the PR
    /// poller last read it (None: detached, or none read yet).
    fn place_branch_of(&self, a: &Agent) -> Option<String> {
        self.place_of(a)?;
        self.places.get(&a.dir)?.branch.clone()
    }

    /// BISE-136: the PR poller read what private worktree `path` has
    /// checked out (`branch` "": detached). A change: its agents' places
    /// carry it (the views, the next plan asks the forge about it).
    fn private_branch_in(&mut self, path: &str, branch: &str) {
        let branch = (!branch.is_empty()).then(|| branch.to_string());
        let mut changed = false;
        for p in self.places.values_mut().filter(|p| p.path == path) {
            if p.branch != branch {
                p.branch = branch.clone();
                changed = true;
            }
        }
        if !changed {
            return;
        }
        for a in self.st.agents.values_mut().filter(|a| a.place.as_deref() == Some(path)) {
            a.place_branch = branch.clone();
        }
        self.dirty = true;
    }

    /// `id` is a private worktree with no branch checked out (as far as
    /// the poller read).
    fn detached_private(&self, id: &str) -> bool {
        crate::place::private_path(id).is_some_and(|path| {
            let mut ps = self.places.values().filter(|p| p.path == path).peekable();
            ps.peek().is_some() && ps.all(|p| p.branch.is_none())
        })
    }

    /// Record where `agent` works; `told`: from `sb worktree` (else the
    /// bash fallback, ignored once the agent told).
    fn set_place(&mut self, agent: &str, path: String, told: bool) {
        let Some(dir) = self.st.agents.get(agent).map(|a| a.dir.clone()) else {
            return;
        };
        if !told && self.places.get(&dir).is_some_and(|p| p.told || p.path == path) {
            return;
        }
        // the same folder keeps the branch read; another is read anew
        let branch = self.places.get(&dir).filter(|p| p.path == path).and_then(|p| p.branch.clone());
        self.places.insert(dir, Place { path, told, branch });
        if let Some(a) = self.st.agents.get(agent).cloned() {
            let (place, branch) = (self.place_of(&a), self.place_branch_of(&a));
            if let Some(a) = self.st.agents.get_mut(agent) {
                a.place = place;
                a.place_branch = branch;
            }
        }
        self.dirty = true;
    }

    /// A client is attached (the PR poller's slow cadence when none).
    pub fn has_clients(&self) -> bool {
        !self.clients.is_empty()
    }

    /// The branches the PR poller follows: each worktree place's.
    pub fn pr_watches(&self) -> Vec<crate::forge::poll::Watch> {
        crate::place::places(&self.st, &self.prs)
            .into_iter()
            .filter(|p| p.kind == crate::place::PlaceKind::Worktree)
            .filter_map(|p| {
                // BISE-136: a private worktree is followed by what it has
                // checked out (its branch "" while detached or not read)
                let head = crate::place::private_path(&p.id).is_some();
                let branch = if head { p.branch.unwrap_or_default() } else { p.branch? };
                Some(crate::forge::poll::Watch { branch, place: p.id, path: p.path, base: p.base, head })
            })
            .collect()
    }

    /// An answer of the PR poller (pr-design §7-§10): the snapshots by
    /// place, the events (journaled when they carry the number), the
    /// held line of a place with no PR yet, a merged PR's cleanup.
    fn prs_in(&mut self, fx: &mut Fx, env: &mut dyn Env, r: crate::forge::poll::Report) {
        use crate::forge::{self, PrEvent};
        // BISE-136: what each private worktree has checked out first (its
        // place's branch, the PR below is matched against it)
        for l in &r.local {
            if let Some(path) = crate::place::private_path(&l.place) {
                self.private_branch_in(path, &l.branch);
            }
        }
        let places = crate::place::places(&self.st, &self.prs);
        let place = |id: &str| places.iter().find(|p| p.id == id && p.kind == crate::place::PlaceKind::Worktree);
        for l in &r.local {
            if place(&l.place).is_none() {
                continue;
            }
            // designer's call 8: a detached private worktree with no
            // commit of its own has nothing to say (no held line)
            let detached = crate::place::private_path(&l.place).is_some() && l.branch.is_empty();
            match l.commits {
                Some(0) if detached => {
                    if self.pr_lids.remove(&l.place).is_some() {
                        self.dirty = true;
                    }
                }
                Some(n) => {
                    let lid = forge::no_pr_lid(n);
                    if self.pr_lids.get(&l.place) != Some(&lid) {
                        self.pr_lids.insert(l.place.clone(), lid);
                        self.dirty = true;
                    }
                }
                None => {}
            }
        }
        let prs = match r.prs {
            None => return,
            Some(Err(e)) => {
                if self.pr_late.is_none() {
                    fx.push(Effect::Pr(PrEvent::Unreachable { error: e.clone() }));
                    self.dirty = true;
                }
                self.gh_off(fx, &e);
                self.pr_late = Some(e);
                return;
            }
            Some(Ok(prs)) => prs,
        };
        if self.pr_late.take().is_some() {
            self.dirty = true;
        }
        for l in &r.local {
            let Some(p) = place(&l.place) else { continue };
            let known = self.pr_known.get(&l.place).copied();
            let new = prs.iter().find(|pr| pr.branch == l.branch).filter(|pr| forge::belongs(pr, known, l.tip.as_deref()));
            for e in forge::diff(&l.place, self.prs.get(&l.place), new, known) {
                // the repo's first PR (the journal knew none): its line says more
                let first = self.pr_known.is_empty();
                if let Some(j) = forge::journal_line(&e) {
                    forge::read_line(&mut self.pr_known, &j);
                    fx.push(Effect::Journal(j));
                }
                let at = forge::news::At { place: &p.id, branch: &l.branch, agents: &p.agents };
                let outs = self.pr_news.event(&e, Some(&at), first);
                self.news_out(fx, env, outs);
                fx.push(Effect::Pr(e));
            }
            self.pr_ok_ms.insert(l.place.clone(), r.at_ms);
            match new {
                Some(pr) => {
                    if self.prs.get(&l.place) != Some(pr) {
                        self.prs.insert(l.place.clone(), pr.clone());
                        self.dirty = true;
                    }
                }
                None => {
                    if self.prs.remove(&l.place).is_some() {
                        self.dirty = true;
                    }
                }
            }
            // a private worktree is its agent's (gate.sh done removes
            // it): a merge archives no one there
            let private = crate::place::private_path(&l.place).is_some();
            if let Some(pr) = new.filter(|pr| pr.state == crate::place::PrState::Merged && !private) {
                let agents = p.agents.clone();
                self.pr_cleanup(fx, env, l, pr, &agents);
            }
        }
        // pr-news: the reviews, comments and failing checks of the PRs
        // that changed, to the agent that owns each
        for (branch, act) in &r.activity {
            let Some(l) = r.local.iter().find(|l| &l.branch == branch) else { continue };
            let (Some(p), Some(pr)) = (place(&l.place), self.prs.get(&l.place).cloned()) else { continue };
            let at = forge::news::At { place: &p.id, branch: &l.branch, agents: &p.agents };
            let outs = self.pr_news.activity(&at, &pr, act, &self.pr_bots);
            self.news_out(fx, env, outs);
        }
        // the places gone (dropped): their PR goes with them
        let ids: BTreeSet<&str> = places.iter().map(|p| p.id.as_str()).collect();
        self.pr_news.retain(&ids);
        self.prs.retain(|k, _| ids.contains(k.as_str()));
        self.pr_lids.retain(|k, _| ids.contains(k.as_str()));
        self.pr_ok_ms.retain(|k, _| ids.contains(k.as_str()));
        // pr-merge: the ready-to-merge items against the PRs now
        self.merge_sync(fx, env);
    }

    /// pr-news' decisions, carried out: a message from `github` (sb-core's
    /// `forge`, a pseudo sender like a peer), a line in main's feed (`sb
    /// pr : tone : number : url : text`), a question card for the user
    /// (`forge_card`: its answer goes to the agent), a journal line.
    fn news_out(&mut self, fx: &mut Fx, env: &mut dyn Env, outs: Vec<crate::forge::news::Out>) {
        use crate::forge::news::Out;
        for o in outs {
            match o {
                Out::Tell { to, text } => self.core(fx, env, None, json!({"t": "forge", "to": to, "text": text})),
                Out::Line { tone, number, url, text } => fx.push(line(
                    MAIN,
                    "pr",
                    &join_fields(&[tone.as_str().to_string(), number.to_string(), url, text]),
                )),
                Out::Card { agent, text } => {
                    self.core(fx, env, None, json!({"t": "forge_card", "agent": agent, "text": text}))
                }
                Out::Journal(j) => fx.push(Effect::Journal(j)),
            }
        }
    }

    /// pr-design §6.4: a merged PR archives its place's agents and
    /// removes the worktree, with no backup when nothing would be lost
    /// (the PR's head is the branch's tip, no changed file). Else the
    /// place stays, and main's thread says why, once.
    fn pr_cleanup(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        l: &crate::forge::poll::Local,
        pr: &crate::place::PrSnapshot,
        agents: &[String],
    ) {
        if self.pr_done.contains(&l.place) {
            return;
        }
        let Some(dirty) = l.dirty else { return };
        self.pr_done.insert(l.place.clone());
        let names = agents.iter().map(|a| format!("@{}", a)).collect::<Vec<_>>().join(", ");
        if dirty || l.tip.as_deref() != Some(pr.head_oid.as_str()) {
            if !agents.is_empty() {
                let why = if dirty { "changed files" } else { "commits after the PR's head" };
                fx.push(line(
                    MAIN,
                    "warn",
                    &format!(
                        "#{} ({}) is merged, but its worktree has {}: {} stays. /archive it when its work is not needed",
                        pr.number, l.branch, why, names
                    ),
                ));
            }
            return;
        }
        env.pr_merged(&l.branch, &pr.head_oid);
        for a in agents {
            self.core(fx, env, None, json!({"t": "drop", "name": a, "force": true}));
        }
    }

    /// The held lines (the land queue's first, else `no PR yet`) and the
    /// stale PRs' ages, by place id, for the views.
    fn pr_views(&self, now: u64) -> (BTreeMap<String, String>, BTreeMap<String, u64>) {
        let mut lids = self.lids.clone();
        // trunk flow: a branch lands on main, it never waits for a PR
        let no_pr = if self.flow == Some(crate::flow::FlowMode::Trunk) { &BTreeMap::new() } else { &self.pr_lids };
        for (id, l) in no_pr {
            // a detached private worktree (BISE-136) has no branch to ask
            // the forge about: its count shows without an answer
            let asked = self.pr_ok_ms.contains_key(id) || self.detached_private(id);
            if !self.prs.contains_key(id) && asked {
                lids.entry(id.clone()).or_insert_with(|| l.clone());
            }
        }
        let stale = self
            .prs
            .keys()
            .filter_map(|id| {
                let age = now.saturating_sub(*self.pr_ok_ms.get(id)?);
                (self.pr_late.is_some() || age > PR_STALE_MS).then(|| (id.clone(), age))
            })
            .collect();
        (lids, stale)
    }

    /// The client snapshot (agents, cards) for the views.
    pub fn snapshot(&self, now: u64) -> Value {
        let waiting = board::Waiting::of(&self.st);
        let agents: Vec<Value> = self
            .st
            .order
            .iter()
            .filter_map(|n| self.st.agents.get(n))
            .map(|a| {
                json!({
                    "name": a.name,
                    "main": a.is_main,
                    "status": a.status().as_str(),
                    "objective": a.description(),
                    // what it is doing now, one line (BISE-126); "" for main
                    "role": self.role_line(a),
                    "parent": a.parent,
                    "mode": match a.ws.mode { Mode::Worktree => "worktree", Mode::Shared => "shared" },
                    "path": a.ws.path,
                    "branch": a.ws.branch,
                    "dropped": a.ws.dropped,
                    // BISE-136: a private worktree (gate.sh new), else null
                    "place": a.place,
                    // dev-flow §3.1: the id of the place it is in (`places`)
                    // (BISE-136: its private worktree's, `pt:<path>`; a
                    // feature's agent: its feature's, dev-flow §5.1)
                    "place_id": crate::place::view_id(a),
                    "created_ms": a.created_ms,
                    // its folder under agents/ and its former names (S2 C:
                    // view.json carries them for sb history --project)
                    "dir": a.dir,
                    "aliases": a.aliases,
                    "note": a.declared.as_ref().map(|(_, n)| n.clone()).unwrap_or_default(),
                    "report": a.last_report.as_ref().map(|r| clip(&one_line(&r.summary), 200)),
                    // when it last reported (an archived task: about when it stopped)
                    "report_ms": a.last_report.as_ref().map(|r| r.at_ms),
                    // S10: followed, and its progress step (`--step n/m`)
                    "follow": a.follow,
                    "report_kind": a.last_report.as_ref().map(|r| r.kind.clone()),
                    "step": a.last_report.as_ref().and_then(|r| r.step),
                    "of": a.last_report.as_ref().and_then(|r| r.of),
                    "queued": waiting.queued_count(&a.name),
                    // the user's queued inputs waiting for the turn's end
                    // (the window's "send queued"): [{id, text, created_ms}]
                    "queued_inputs": waiting.queued_inputs(&a.name),
                    // BISE-299: main's inbox, the agents' questions waiting
                    // for main (the user sees a quiet count, never asked)
                    "inbox": if a.is_main { waiting.unanswered(&a.name).len() } else { 0 },
                    "turn_ms": a.turn_started_ms.map(|t| now.saturating_sub(t)),
                    // issue 22: its ended turns, from the same sb-core
                    // view as its status (the agents row's `turns`)
                    "turns": a.turns_ended,
                    // who it waits on (`sb wait` / `sb ask`), for `waits {name}`
                    "waiting_on": a.waiting_on.as_ref().filter(|_| a.waiting),
                })
            })
            .collect();
        let cards: Vec<Value> = self
            .st
            .open_cards()
            .map(|c| {
                json!({
                    "id": c.id,
                    "kind": c.kind,
                    "agent": c.agent,
                    "text": c.text,
                    "for_msg": c.for_msg,
                    "age_ms": now.saturating_sub(c.created_ms),
                    "note": match c.kind.as_str() {
                        "merge" => self.merge_note(c),
                        update_card::KIND => self.update_note(c),
                        signin_card::KIND => self.signin_note(),
                        _ => self.card_note(c),
                    },
                    // expired-ux: the agents that wait for the sign-in
                    // (⏎ in their thread signs in)
                    "waiting": if c.kind == signin_card::KIND { Some(self.signin_waiting()) } else { None },
                    // update-card: the release page its `3` opens
                    "link": if c.kind == update_card::KIND { self.update_link(c) } else { None },
                    // a hub item's place and PR (pr-merge): the TUI ties
                    // it to the place's box and opens its link
                    "place": c.place,
                    "pr": c.pr,
                })
            })
            .collect();
        let places = crate::place::places(&self.st, &self.prs);
        let (mut lids, stale) = self.pr_views(now);
        // a feature's lid: the land queue's (`landing`) wins
        for (id, l) in &self.feature_lids {
            lids.entry(id.clone()).or_insert_with(|| l.clone());
        }
        let places = crate::place::views(&places, &lids, &stale, &self.trying);
        json!({"ev": "state", "agents": agents, "cards": cards, "places": places,
               "flow": self.flow.map(|f| f.as_str())})
    }

    /// A question card whose asker heard from main since, without a
    /// reply to the question: maybe answered another way (the card
    /// stays open, the view says so).
    fn card_note(&self, c: &Card) -> Option<String> {
        if c.kind != "question" || c.agent == MAIN {
            return None;
        }
        let m = self
            .st
            .msgs
            .values()
            .rev()
            .take_while(|m| m.created_ms >= c.created_ms)
            .find(|m| m.from == MAIN && m.to == c.agent && m.reply_to != c.for_msg)?;
        Some(format!(
            "@main wrote to @{} since then (m_{}): {}",
            c.agent,
            m.id,
            clip(&one_line(&m.text), 120)
        ))
    }


    /// The role line the views show (BISE-126): the model's, else the
    /// objective's first sentence; none for main.
    pub fn role_line(&self, a: &Agent) -> String {
        if a.is_main {
            return String::new();
        }
        match self.roles.get(&a.dir).and_then(|r| r.line.clone()) {
            Some(l) => l,
            None => role::default_line(&a.brief.objective),
        }
    }

    /// A line saved by an earlier hub (`<agent dir>/role.json`).
    pub fn load_role(&mut self, dir: &str, line: String, key: String) {
        let r = self.roles.entry(dir.to_string()).or_default();
        r.line = Some(line);
        r.key = key;
    }

    /// A task's turn ended: one call for a new line when what it is made
    /// of changed, none while one is in flight (it looks again at its
    /// end), none for a while after a failure. Never main, never a task
    /// that is not active.
    fn ask_role(&mut self, fx: &mut Fx, now: u64, agent: &str) {
        let Some(a) = self.st.agents.get(agent) else { return };
        if a.is_main || a.lifecycle != Lifecycle::Active {
            return;
        }
        let report = a.last_report.as_ref().map(|r| r.summary.clone()).unwrap_or_default();
        let note = a.declared.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();
        let key = role::key(&a.brief.objective, &report, &note);
        let current = self.role_line(a);
        let (objective, dir) = (a.brief.objective.clone(), a.dir.clone());
        let r = self.roles.entry(dir.clone()).or_default();
        if r.asking.is_some() {
            r.pending = true;
            return;
        }
        if r.key == key || r.failed_ms.is_some_and(|t| now.saturating_sub(t) < ROLE_RETRY_MS) {
            return;
        }
        r.asking = Some(key.clone());
        fx.push(Effect::AskRole {
            dir,
            key,
            request: role::request(&objective, &report, &note, &current),
        });
    }

    fn role_answer(&mut self, fx: &mut Fx, now: u64, dir: &str, key: String, line: Option<String>) {
        let r = self.roles.entry(dir.to_string()).or_default();
        r.asking = None;
        match line {
            Some(l) => {
                if r.line.as_deref() != Some(l.as_str()) {
                    self.dirty = true;
                }
                r.line = Some(l);
                r.key = key;
                r.failed_ms = None;
            }
            None => r.failed_ms = Some(now),
        }
        if std::mem::take(&mut r.pending) {
            if let Some(name) = self.st.agents.values().find(|a| a.dir == dir).map(|a| a.name.clone()) {
                self.ask_role(fx, now, &name);
            }
        }
    }

    pub fn handle(&mut self, input: Input, env: &mut dyn Env) -> Fx {
        let mut fx = Fx::new();
        match input {
            Input::Boot => self.core(&mut fx, env, None, json!({"t": "boot"})),
            Input::ReplReady { agent } => {
                self.core(&mut fx, env, None, json!({"t": "ready", "agent": agent}))
            }
            Input::ReplLine { agent, line } => self.repl_line(&mut fx, env, &agent, &line),
            Input::ReplIdle { agent, leftover } => {
                self.core(
                    &mut fx,
                    env,
                    None,
                    // expired-ux: a turn that stopped on the expired
                    // ChatGPT sign-in is held, not over: no automatic
                    // reply (`(no written reply…)` woke its parent, on the
                    // same plan, into a turn that failed too)
                    json!({"t": "idle", "agent": agent, "leftover": leftover,
                        "held": self.signin.stopped.contains(&agent)}),
                );
                self.ask_role(&mut fx, env.now(), &agent);
            }
            Input::RoleLine { dir, key, line } => self.role_answer(&mut fx, env.now(), &dir, key, line),
            Input::TimerName { id, reply } => {
                self.timer_names.done(id);
                if let Some(t) = self.st.timers.map.get(&id) {
                    let name = crate::every_name::name_of(reply.as_deref(), &t.text, &t.agent);
                    self.core(&mut fx, env, None, json!({"t": "every_name", "id": id, "name": name}));
                }
            }
            Input::Prs(r) => self.prs_in(&mut fx, env, r),
            Input::Feature(d) => self.feature_done(&mut fx, env, d),
            Input::ReplExited {
                agent,
                crashed,
                reason,
            } => self.core(
                &mut fx,
                env,
                None,
                json!({"t": "exited", "agent": agent, "crashed": crashed, "reason": reason}),
            ),
            Input::ClientHello { client } => {
                self.clients.insert(
                    client,
                    ClientView {
                        focus: MAIN.to_string(),
                        since_ms: env.now(),
                        sent: Vec::new(),
                    },
                );
            }
            Input::ClientInput {
                client,
                focus,
                text,
                queued,
            } => self.user_input(&mut fx, env, client, &focus, &text, queued),
            Input::UserCmd { client, focus, cmd } => {
                let focus = self.focus_of(client, &focus);
                self.user_cmd(&mut fx, env, client, focus, cmd, false)
            }
            Input::ClientFocus { client, focus } => self.set_focus(&mut fx, env, client, &focus),
            Input::ClientGone { client } => {
                self.set_focus(&mut fx, env, client, MAIN);
                self.clients.remove(&client);
            }
            Input::ClientConfirm { client, id, yes } => self.confirm(&mut fx, env, client, id, yes),
            Input::ClientInterrupt { client, agent } => {
                self.interrupt_by = Some("user".into());
                self.core(&mut fx, env, Some(client), json!({"t": "interrupt", "agent": agent}));
                self.interrupt_by = None;
            }
            Input::Agent { token, from, req } => self.agent_req(&mut fx, env, token, &from, req),
            // the waits' timeouts, then sb every's timers (sb-core decides)
            Input::Tick => self.core(&mut fx, env, None, json!({"t": "tick"})),
            Input::EveryStop { id, why } => self.timer_stop_by_user(&mut fx, env, id, &why),
            Input::EveryRun { id } => self.timer_run_now(&mut fx, env, id),
            Input::ConfirmOpen { agent, text } => {
                self.core(&mut fx, env, None, json!({"t": "confirm_open", "agent": agent, "text": text}))
            }
            Input::ConfirmClose { card, res } => {
                self.core(&mut fx, env, None, json!({"t": "confirm_close", "card": card, "res": res}))
            }
            Input::Merged { card, place, number, res } => self.merged(&mut fx, env, card, &place, number, res),
            Input::Release(c) => self.release_in(&mut fx, env, c),
            Input::SignedIn => self.signed_in(&mut fx, env),
            Input::Updated { card, version, res } => self.updated(&mut fx, env, card, &version, res),
            Input::XIn { token, hub, xid, kind, text, name } => {
                let i = json!({"t": "x_in", "token": token, "hub": hub, "xid": xid, "kind": kind, "text": text, "name": name});
                self.core(&mut fx, env, None, i)
            }
            Input::XAck { xid } => self.core(&mut fx, env, None, json!({"t": "x_ack", "xid": xid})),
            Input::XFail { xid } => self.core(&mut fx, env, None, json!({"t": "x_fail", "xid": xid})),
            Input::XReply { xid, name, text } => {
                self.core(&mut fx, env, None, json!({"t": "x_reply", "xid": xid, "name": name, "text": text}))
            }
            Input::RouteHold { text, via, to, name, why, context } => {
                let context = context.filter(|c| !c.is_null()).map_or(String::new(), |c| c.to_string());
                let i = json!({"t": "route_hold", "text": text, "via": via, "to": to, "name": name, "why": why, "context": context});
                self.core(&mut fx, env, None, i)
            }
            Input::RouteCorrect { client, rid, to, name } => {
                self.core(&mut fx, env, client, json!({"t": "route_correct", "rid": rid, "to": to, "name": name}))
            }
            Input::RouteCancel { client, rid } => self.core(&mut fx, env, client, json!({"t": "route_cancel", "rid": rid})),
            Input::RoutePick { rid, to, name, why } => {
                self.core(&mut fx, env, None, json!({"t": "route_pick", "rid": rid, "to": to, "name": name, "why": why}))
            }
            Input::Follow { client, agent, on } => self.core(&mut fx, env, client, json!({"t": "follow", "agent": agent, "on": on})),
        }
        self.refresh_contexts(&mut fx, env.now());
        if self.dirty {
            fx.push(Effect::State);
            self.dirty = false;
        }
        fx
    }

    /// Run one input in sb-core: answer its git queries (the input is
    /// replayed with the answers), then apply the effects it returns.
    fn core(&mut self, fx: &mut Fx, env: &mut dyn Env, client: Option<ClientId>, input: Value) {
        let mut input = input;
        // a number sb-core cannot read fails its whole input: refuse it
        // here, naming it, to whoever sent it (crate::core_num)
        if let Some(n) = crate::core_num::too_big(&input) {
            let e = format!("refused: {} is too large a number (at most {})", n, crate::core_num::MAX);
            match (input["t"].as_str(), input["token"].as_u64(), client) {
                (Some("req"), Some(token), _) => fx.push(Effect::Reply { token, body: json!({"ok": false, "error": e}) }),
                (_, _, Some(c)) => fx.push(notice(c, &e)),
                _ => fx.push(line(MAIN, "warn", &e)),
            }
            return;
        }
        // one now for the input and its re-sends (a need replays the step)
        let now = env.now();
        input["now"] = json!(now);
        input["git"] = json!(env.is_git());
        let mut ans: Vec<Value> = Vec::new();
        loop {
            input["ans"] = Value::Array(ans.clone());
            let out = self.call(&input);
            for l in std::mem::take(&mut self.revived) {
                fx.push(line(MAIN, "warn", &l));
            }
            let Some(out) = out else { return };
            if let Some(q) = out.get("need") {
                // sb every's wakes: their words and the local clock
                if q["q"] == "every_wake" {
                    ans.push(self.st.timers.wakes(q, now));
                    continue;
                }
                let a = self.query(env, q);
                ans.push(a);
                continue;
            }
            if let Some(e) = out.get("error") {
                panic!("sb-core: {}", e);
            }
            if out.get("dirty").and_then(|d| d.as_bool()) == Some(true) {
                self.dirty = true;
            }
            // the state after the step first: the deliveries render from it
            self.load_view(&out["view"]);
            for f in out.get("fx").and_then(|x| x.as_array()).into_iter().flatten() {
                self.effect(fx, env, client, f);
            }
            // a timer without a name: its small-model call (every_name.rs)
            if let Some((id, request)) = self.timer_names.next(&self.st.timers) {
                fx.push(Effect::AskTimerName { id, request });
            }
            return;
        }
    }

    /// A git query of sb-core (RFC 0002), on the workspace the mirror
    /// knows for the task.
    fn query(&mut self, env: &mut dyn Env, q: &Value) -> Value {
        let name = jstr(q, "name");
        let ws: Option<Workspace> = serde_json::from_value(q["ws"].clone()).ok();
        let snap = q["snapshot_ref"].as_str().map(|x| x.to_string());
        let res = |r: Result<Value, String>| match r {
            Ok(v) => json!({"ok": v}),
            Err(e) => json!({"err": e}),
        };
        match jstr(q, "q").as_str() {
            "worktree_create" => {
                let wc = q.get("with_changes").and_then(|x| x.as_bool()).unwrap_or(false);
                res(env.worktree_create(&name, wc).map(|w| json!(w)))
            }
            "worktree_loss" => {
                let l = ws.map(|w| env.worktree_loss(&w)).unwrap_or_default();
                json!({"dirty": l.dirty, "unpushed": l.unpushed})
            }
            "worktree_drop" => {
                let n = |k: &str| q.get(k).and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let loss = Loss {
                    dirty: n("dirty"),
                    unpushed: n("unpushed"),
                };
                match ws {
                    Some(w) => res(env.worktree_drop(&name, &w, &loss).map(|r| json!(r))),
                    None => json!({"err": "no agent"}),
                }
            }
            "worktree_restore" => match ws {
                Some(w) => res(env
                    .worktree_restore(&name, &w, snap.as_deref())
                    .map(|w| json!(w))),
                None => json!({"err": "no agent"}),
            },
            other => json!({"err": format!("unknown query: {}", other)}),
        }
    }

    /// One effect of sb-core.
    fn effect(&mut self, fx: &mut Fx, env: &mut dyn Env, client: Option<ClientId>, f: &Value) {
        let agent = jstr(f, "agent");
        match jstr(f, "fx").as_str() {
            "journal" => {
                assert!(f["ev"].is_object(), "sb-core: bad event {}", f["ev"]);
                fx.push(Effect::Journal(f["ev"].clone()));
                // a timer set or ended (sb every): its ◷ lines
                self.timer_journal_lines(fx, &f["ev"]);
            }
            // the runtime state comes with the view
            "rt" => {}
            "spawn" => fx.push(Effect::Spawn {
                agent,
                resume: f["resume"].as_bool().unwrap_or(false),
                crash_note: f["crash_note"].as_str().map(|x| x.to_string()),
            }),
            "kill" => fx.push(Effect::Kill { agent }),
            "say" => fx.push(Effect::Say {
                agent,
                text: jstr(f, "text"),
            }),
            "passthrough" => fx.push(Effect::Passthrough {
                agent,
                line: jstr(f, "line"),
            }),
            "interrupt" => {
                let by = self.interrupt_by.clone().unwrap_or_else(|| "bise".into());
                fx.push(Effect::Interrupt { agent, by })
            }
            "line" => {
                // C2: a multi-field line (`answered`) comes as `fields`
                let text = match f.get("fields") {
                    Some(_) => join_fields(&jstrs(f, "fields")),
                    None => jstr(f, "text"),
                };
                fx.push(line(&agent, &jstr(f, "kind"), &text))
            }
            "reply" => {
                let token = f["token"].as_u64().unwrap_or(0);
                let body = f["body"].clone();
                match self.spawn_asks.remove(&token) {
                    // issue #4: a spawn that asked for a model: the daemon
                    // picks it, writes the task's choice, then answers
                    Some(ask) if body["ok"] == json!(true) && body["name"].is_string() => {
                        fx.push(Effect::SpawnModel { token, agent: jstr(&body, "name"), ask, body })
                    }
                    _ => fx.push(Effect::Reply { token, body }),
                }
            }
            "deliver" => self.deliver(fx, env, f),
            "xdeliver" => fx.push(Effect::XDeliver {
                xid: f["xid"].as_u64().unwrap_or(0),
                project: jstr(f, "project"),
                kind: jstr(f, "kind"),
                text: jstr(f, "text"),
                context: jstr(f, "context"),
            }),
            "xreply" => fx.push(Effect::XReply { hub: jstr(f, "hub"), xid: f["xid"].as_u64().unwrap_or(0), text: jstr(f, "text") }),
            "route" => fx.push(Effect::Route {
                rid: f["rid"].as_u64().unwrap_or(0),
                to: jstr(f, "to"),
                name: jstr(f, "name"),
                text: jstr(f, "text"),
                due_ms: f["due_ms"].as_u64().unwrap_or(0),
                why: jstr(f, "why"),
            }),
            "route_ask" => fx.push(Effect::RouteAsk { rid: f["rid"].as_u64().unwrap_or(0), text: jstr(f, "text") }),
            "job_end" => fx.push(Effect::JobEnd {
                agent: jstr(f, "agent"),
                state: jstr(f, "state"),
                label: jstr(f, "label"),
                summary: jstr(f, "summary"),
                key: f["key"].as_u64().unwrap_or(0),
            }),
            "followed_end" => fx.push(Effect::FollowedEnd {
                project: jstr(f, "project"),
                agent: jstr(f, "agent"),
                key: f["key"].as_u64().unwrap_or(0),
                state: jstr(f, "state"),
                label: jstr(f, "label"),
                summary: jstr(f, "summary"),
            }),
            "route_done" => fx.push(Effect::RouteDone {
                rid: f["rid"].as_u64().unwrap_or(0),
                state: jstr(f, "state"),
                to: jstr(f, "to"),
                xid: f["xid"].as_u64(),
            }),
            // not the client's `confirm` (a yes/no in the status row): a card's
            // the user's digit on a hub item (merge.rs)
            "card_choice" => self.card_choice(fx, env, f),
            "confirm" if f.get("card").is_some() => fx.push(Effect::Confirm {
                card: f["card"].as_u64().unwrap_or(0),
                agent,
                text: jstr(f, "text"),
            }),
            other => self.client_effect(fx, client, other, f),
        }
    }

    /// The effects on the client that typed the input.
    fn client_effect(&mut self, fx: &mut Fx, client: Option<ClientId>, kind: &str, f: &Value) {
        match kind {
            "notice" => {
                if let Some(c) = client {
                    fx.push(notice(c, &jstr(f, "text")));
                }
            }
            "confirm" => {
                if let Some(c) = client {
                    let id = self.next_confirm;
                    self.next_confirm += 1;
                    self.confirms.insert(id, (c, jstr(f, "name")));
                    fx.push(Effect::ToClient {
                        client: c,
                        body: json!({"ev": "confirm", "id": id, "text": jstr(f, "text")}),
                    });
                }
            }
            "focus_main" => {
                let name = jstr(f, "name");
                for (c, v) in self.clients.iter() {
                    if v.focus == name {
                        fx.push(Effect::ToClient {
                            client: *c,
                            body: json!({"ev": "focus", "focus": MAIN}),
                        });
                    }
                }
            }
            "user_sent" => {
                if let Some(v) = client.and_then(|c| self.clients.get_mut(&c)) {
                    v.sent.push(jstr(f, "text"));
                }
            }
            "renamed" => {
                let (old, new) = (jstr(f, "old"), jstr(f, "new"));
                for v in self.clients.values_mut().filter(|v| v.focus == old) {
                    v.focus = new.clone();
                }
                fx.push(Effect::Renamed { old, new });
            }
            // a newer sb-core (a version switch in flight) may know
            // effects this hub does not: skipped and logged, never a crash
            other => eprintln!("sb-core: unknown effect {} (skipped): {}", other, f),
        }
    }

    /// The text a delivery puts in the agent's context: main's notes, the
    /// task status block, then each message as the recipient reads it.
    fn deliver(&mut self, fx: &mut Fx, env: &mut dyn Env, f: &Value) {
        let agent = jstr(f, "agent");
        let mut parts: Vec<String> = Vec::new();
        let notes: Vec<String> = f["notes"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(|x| x.to_string())).collect())
            .unwrap_or_default();
        if !notes.is_empty() {
            let mut t = String::from("<bise_notes>\n");
            for n in &notes {
                t.push_str(&format!("- {}\n", n));
            }
            t.push_str("</bise_notes>");
            parts.push(t);
        }
        if f["status"].as_bool() == Some(true) {
            let status = board::status_block(&self.st, env.now());
            if !status.is_empty() {
                parts.push(status);
            }
        }
        for m in f["msgs"].as_array().into_iter().flatten() {
            let id = m["id"].as_u64().unwrap_or(0);
            if let Some(msg) = self.st.msgs.get(&id) {
                parts.push(prompts::tagged(msg, &jstr(m, "rel"), self.user_ids && agent == MAIN));
            }
        }
        let text = parts.join("\n\n");
        if jstr(f, "mode") == "steer" {
            fx.push(Effect::Steer { agent, text });
        } else {
            fx.push(Effect::Say { agent, text });
        }
    }

    fn repl_line(&mut self, fx: &mut Fx, env: &mut dyn Env, agent: &str, raw: &str) {
        let now = env.now();
        let t = |kind: &str| json!({"t": kind, "agent": agent});
        match wire::parse(raw) {
            Wire::TurnStarted => self.core(fx, env, None, t("turn_started")),
            Wire::SteeringReceived => self.core(fx, env, None, t("steer_rx")),
            Wire::Steered => self.core(fx, env, None, t("steered")),
            Wire::Assistant(text) if !text.is_empty() => {
                self.set_activity(agent, now, format!("wrote: {}", clip(&one_line(&text), 160)));
                let r = self.recent.entry(agent.to_string()).or_default();
                r.push_back((now, text.clone()));
                if r.len() > 20 {
                    r.pop_front();
                }
                self.core(
                    fx,
                    env,
                    None,
                    json!({"t": "assistant", "agent": agent, "text": text}),
                );
            }
            Wire::Tool { name, args } if name == "edit" || name == "write_file" => {
                // Vibe's edit tools: the runtime's feed line carries
                // {"file_path": …} (bend/core/edit.bend ann_args)
                let path = wire::edit_file(&args);
                if self.st.agents.contains_key(agent) {
                    let verb = if name == "edit" { "edit" } else { "write" };
                    self.set_activity(agent, now, format!("{} {}", verb, path.as_deref().unwrap_or("")));
                    self.dirty = true;
                }
                if let Some(path) = path {
                    self.core(fx, env, None, json!({"t": "touch", "agent": agent, "path": path}));
                }
            }
            Wire::Tool { name, args } if name != "apply_patch" => {
                if let Some(own) = self.st.agents.get(agent).map(|a| a.ws.path.clone()) {
                    self.set_activity(agent, now, format!("{} `{}`", name, clip(&one_line(&args), 120)));
                    self.dirty = true;
                    if let Some(p) = (name == "bash").then(|| bash_worktree(&args, &own)).flatten() {
                        self.set_place(agent, p, false);
                    }
                }
            }
            Wire::Intent(text) => {
                if self.st.agents.contains_key(agent) {
                    self.set_activity(agent, now, clip(&text, 120));
                    self.dirty = true;
                }
            }
            Wire::Tool { args, .. } => {
                let files = wire::patch_files(&args);
                if self.st.agents.contains_key(agent) {
                    self.set_activity(agent, now, format!("apply_patch {}", files.join(", ")));
                    self.dirty = true;
                }
                for path in files {
                    self.core(
                        fx,
                        env,
                        None,
                        json!({"t": "touch", "agent": agent, "path": path}),
                    );
                }
            }
            // BR-007: a task whose turn failed (network down, provider
            // error...) must not go quiet: the failure reaches its parent
            // as a report (board + message), like a report the task wrote.
            // Main's own failures show in main's view (its turn_done line);
            // a stop the user asked for (Ctrl+C) is not news to anyone.
            Wire::TurnDone(t) => {
                // issue #4: the first turn on a model asked at the spawn;
                // refused by its provider, the task moves to the default
                if self.model_trial.remove(agent) {
                    if let Some(why) = model_refusal(&t) {
                        fx.push(Effect::ModelRefused { agent: agent.to_string(), why });
                        return;
                    }
                }
                // expired-ux: the ChatGPT sign-in expired: the user's
                // `signin` item says it, the agent waits for the sign-in
                // (its parent, on the same plan, can't help)
                if self.signin_turn(fx, env, agent, &t) {
                    return;
                }
                if let Some(summary) = failed_turn_report(agent, &t) {
                    if self.st.agents.contains_key(agent) {
                        let q = json!({"cmd": "report", "kind": "turn_failed",
                            "summary": summary, "decisions": []});
                        // token 0: no connection waits for this reply
                        // (connection tokens start at 1)
                        self.core(
                            fx,
                            env,
                            None,
                            json!({"t": "req", "token": 0, "from": agent, "req": q}),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn set_focus(&mut self, fx: &mut Fx, env: &mut dyn Env, client: ClientId, focus: &str) {
        let now = env.now();
        let Some(view) = self.clients.get_mut(&client) else {
            return;
        };
        let prev = std::mem::replace(
            view,
            ClientView {
                focus: focus.to_string(),
                since_ms: now,
                sent: Vec::new(),
            },
        );
        let mut input = json!({"t": "focus", "focus": focus, "note": null, "direct": ""});
        if prev.focus != MAIN && prev.focus != focus && !prev.sent.is_empty() {
            // RFC 0001 §7.4: main learns what the user decided directly
            let reply: String = self
                .recent
                .get(&prev.focus)
                .map(|r| {
                    r.iter()
                        .filter(|(t, _)| *t >= prev.since_ms)
                        .map(|(_, s)| s.clone())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            let (note, direct) = direct_exchange(&prev.focus, &prev.sent, &reply);
            input["note"] = json!(note);
            input["direct"] = json!(direct);
        }
        self.core(fx, env, Some(client), input);
    }

    fn confirm(&mut self, fx: &mut Fx, env: &mut dyn Env, client: ClientId, id: u64, yes: bool) {
        let Some((_, name)) = self.confirms.remove(&id) else {
            return;
        };
        if !yes {
            fx.push(notice(client, &format!("drop of @{} cancelled", name)));
            return;
        }
        self.core(fx, env, Some(client), json!({"t": "confirm_drop", "name": name}));
    }

    fn refresh_contexts(&mut self, fx: &mut Fx, now: u64) {
        let names: Vec<String> = self
            .st
            .agents
            .values()
            .filter(|a| a.lifecycle == Lifecycle::Active)
            .map(|a| a.name.clone())
            .collect();
        // at every input: what the contexts share is built once (board.rs)
        let waiting = board::Waiting::of(&self.st);
        let group = board::Group::of(&self.st, now, &self.models);
        for name in names {
            let text = if name == MAIN {
                board::main_context(&self.st, now, &waiting)
            } else {
                board::task_context(&name, &group, &waiting)
            };
            if self.contexts.get(&name) != Some(&text) {
                self.contexts.insert(name.clone(), text.clone());
                fx.push(Effect::Context { agent: name, text });
            }
        }
    }

    /// Issue #4: who `sb send --model` may move: a task, by main or its
    /// parent. Its name, or the words of the refusal.
    fn switch_target(&self, from: &str, to: &str) -> Result<String, String> {
        let to = self.st.resolve(to.trim_start_matches('@')).ok_or_else(|| format!("unknown agent: {}", to))?;
        let from = self.st.resolve(from).unwrap_or_default();
        let parent = self.st.agents.get(&to).and_then(|a| a.parent.clone()).unwrap_or_default();
        if to == MAIN {
            return Err("sb send --model moves a task; main's model is the user's (/model)".into());
        }
        if from != MAIN && from != parent {
            return Err(format!("sb send --model is main's (or the parent's): ask {}", if parent.is_empty() { MAIN } else { &parent }));
        }
        Ok(to)
    }

    fn agent_req(&mut self, fx: &mut Fx, env: &mut dyn Env, token: Token, from: &str, req: AgentReq) {
        let reply = |fx: &mut Fx, body: Value| fx.push(Effect::Reply { token, body });
        // the home workspace (docs/ambient-pages.md §5.1): no git, so no
        // land, feature or flow; one line that says why, never a git error
        if let Some(cmd) = git_only(&req).filter(|_| !env.is_git()) {
            reply(fx, json!({"ok": false, "error": not_in_a_repo(cmd)}));
            return;
        }
        let q = match req {
            AgentReq::List | AgentReq::Tasks => {
                let Some(from) = self.st.resolve(from) else {
                    reply(fx, json!({"ok": false, "error": format!("unknown agent: {}", from)}));
                    return;
                };
                let text = if req == AgentReq::List {
                    board::roster(&self.st, &from, env.now(), &self.models).join("\n")
                } else {
                    let mut t = board::tasks_detail(&self.st, env.now(), &self.models);
                    // the standing orders (sb every): what wakes whom, when
                    let timers = self.st.timers.list(env.now());
                    if !self.st.timers.map.is_empty() {
                        t.push_str(&format!("\n\n## timers (sb every)\n{}", timers));
                    }
                    t
                };
                reply(fx, json!({"ok": true, "text": text}));
                return;
            }
            AgentReq::Send {
                to,
                text,
                expect_reply,
                reply_to,
                queued,
                why,
                switch,
            } => {
                // issue #4: the model first, so the message's turn runs on it
                if let Some((model, effort)) = switch {
                    match self.switch_target(from, &to) {
                        Ok(to) => fx.push(Effect::Switch { token: None, from: from.to_string(), to, model, effort }),
                        Err(e) => {
                            reply(fx, json!({"ok": false, "error": e}));
                            return;
                        }
                    }
                }
                json!({"cmd": "send", "to": to, "text": text, "expect_reply": expect_reply,
                        "reply_to": reply_to, "queued": queued, "why": why})
            }
            AgentReq::Switch { to, model, effort } => {
                match self.switch_target(from, &to) {
                    Ok(to) => fx.push(Effect::Switch { token: Some(token), from: from.to_string(), to, model, effort }),
                    Err(e) => reply(fx, json!({"ok": false, "error": e})),
                }
                return;
            }
            AgentReq::Wait { msg, timeout_s } => {
                json!({"cmd": "wait", "msg": msg, "timeout_s": timeout_s})
            }
            AgentReq::Ask {
                to,
                text,
                timeout_s,
            } => json!({"cmd": "ask", "to": to, "text": text, "timeout_s": timeout_s}),
            AgentReq::Status { status, note } => {
                json!({"cmd": "status", "status": status, "note": note})
            }
            AgentReq::Flow { set } => {
                if self.st.resolve(from).as_deref() != Some(MAIN) {
                    reply(fx, json!({"ok": false, "error": "sb flow is main's: ask main"}));
                    return;
                }
                fx.push(Effect::Flow { client: None, token: Some(token), set });
                return;
            }
            AgentReq::Feature { op, name } => {
                // merge and drop on the user's go only: main's (dev-flow §5.1)
                let is_main = self.st.resolve(from).as_deref() == Some(MAIN);
                if matches!(op.as_str(), "new" | "merge" | "drop") && !is_main {
                    reply(fx, json!({"ok": false, "error": format!("sb feature {} is main's: ask main", op)}));
                    return;
                }
                let agents = self.feature_agents(&name);
                fx.push(Effect::Feature { token: Some(token), op, name, agents });
                return;
            }
            AgentReq::Worktree { path } => {
                let Some(name) = self.st.resolve(from) else {
                    reply(fx, json!({"ok": false, "error": format!("unknown agent: {}", from)}));
                    return;
                };
                if name == MAIN && !path.is_empty() {
                    let e = "sb worktree: main works in the workspace, not in a worktree";
                    reply(fx, json!({"ok": false, "error": e}));
                    return;
                }
                self.set_place(&name, path.clone(), true);
                reply(fx, json!({"ok": true, "path": path}));
                return;
            }
            AgentReq::Report {
                kind,
                summary,
                decisions,
                step,
                result,
            } => {
                let mut q = json!({"cmd": "report", "kind": kind, "summary": summary, "decisions": decisions});
                if let Some((n, of)) = step {
                    q["step"] = json!(n);
                    q["of"] = json!(of);
                }
                if let Some(r) = result {
                    q["result"] = r;
                }
                q
            }
            AgentReq::Follow { agent, on, hub } => json!({"cmd": "follow", "agent": agent, "off": !on, "hub": hub}),
            AgentReq::Spawn {
                name,
                brief,
                worktree,
                with_changes,
                place,
                feature,
                ask,
            } => {
                // issue #4: the answer waits for the daemon's pick of the model
                if !ask.is_empty() {
                    self.spawn_asks.insert(token, ask);
                }
                let name = Some(name.as_str()).filter(|n| !n.is_empty());
                let join = match (place.as_str(), feature.as_str()) {
                    // dev-flow §5.1: its own worktree, from the feature's tip
                    (_, f) if !f.is_empty() => self.feature_worktree(env, name, &brief, f).map(Some),
                    ("" | "new" | crate::place::SHARED, _) => Ok(None),
                    (p, _) => crate::place::find_worktree(&self.st, p).map(Some),
                };
                match join.and_then(|j| new_task(name, &brief, worktree || place == "new", with_changes).map(|v| (j, v))) {
                    Ok((join, mut v)) => {
                        v["cmd"] = json!("spawn");
                        if let Some(ws) = join {
                            v["join"] = json!(ws);
                            v["worktree"] = json!(false);
                        }
                        v
                    }
                    Err(e) => json!({"cmd": "spawn", "pre_err": e}),
                }
            }
            AgentReq::Move { agent, place } => match self.move_target(&agent, &place) {
                Ok(ws) => json!({"cmd": "move", "agent": agent, "ws": ws}),
                Err(e) => json!({"cmd": "move", "agent": agent, "pre_err": e}),
            },
            AgentReq::Land { here, message, add } => {
                match self.land_job(from, here, &message, add) {
                    Ok(job) => {
                        // pr-news: the PR's news go to the place's last lander
                        if job.worktree {
                            self.pr_news.landed(&job.place, &job.agent);
                        }
                        fx.push(Effect::Land { token, job: Box::new(job) })
                    }
                    Err(e) => reply(fx, json!({"ok": false, "error": e})),
                }
                return;
            }
            AgentReq::Interrupt { agent } => {
                // the flag names the asker: "interrupted by main", not
                // "by the user"
                self.interrupt_by = Some(from.to_string());
                json!({"cmd": "interrupt", "agent": agent})
            }
            AgentReq::Stop { agent, reason } => {
                json!({"cmd": "stop", "agent": agent, "reason": reason})
            }
            AgentReq::Drop { agent } => {
                // a drop the user was asked to confirm waits for his
                // answer: a second drop never acts in his place (pm's C
                // fail 36: main dropped @proxy-fix again once it was idle,
                // while its card still asked him)
                let name = self.st.resolve(&agent).unwrap_or_else(|| agent.clone());
                if let Some(c) = self.st.open_cards().find(|c| c.kind == "drop" && c.agent == name) {
                    let error = format!("@{name}: the user has not answered card #{} (archive it?) yet: wait for his answer", c.id);
                    reply(fx, json!({"ok": false, "error": error}));
                    return;
                }
                json!({"cmd": "drop", "agent": agent})
            }
            AgentReq::Card { text, for_msg } => {
                json!({"cmd": "card", "text": text, "for": for_msg})
            }
            AgentReq::Close { card, note } => json!({"cmd": "close", "card": card, "note": note}),
            AgentReq::Withdraw { card, why } => json!({"cmd": "withdraw", "card": card, "why": why}),
            AgentReq::Rename { agent, new_name } => {
                let valid = router::valid_name(&new_name);
                json!({"cmd": "rename", "agent": agent, "new_name": new_name, "valid": valid})
            }
            AgentReq::Restore { agent } => json!({"cmd": "restore", "agent": agent}),
            AgentReq::Isolate { agent } => json!({"cmd": "isolate", "agent": agent}),
            AgentReq::Every(r) => {
                let body = self.timer_req(fx, env, from, r);
                reply(fx, body);
                return;
            }
            // desktop S2: the project's hub id (sb-core never reads the registry)
            AgentReq::ProjectSend { project, msg } => match env.project_hub(&project) {
                Ok(hub) => json!({"cmd": "project_send", "project": hub, "msg": msg}),
                Err(e) => json!({"cmd": "project_send", "pre_err": e}),
            },
            AgentReq::ProjectAsk { project, text } => match env.project_hub(&project) {
                Ok(hub) => json!({"cmd": "project_ask", "project": hub, "text": text}),
                Err(e) => json!({"cmd": "project_ask", "pre_err": e}),
            },
        };
        self.core(
            fx,
            env,
            None,
            json!({"t": "req", "token": token, "from": from, "req": q}),
        );
    }
}

/// RFC 0001 §7.4: what main (`note`) and the user's own feed (`direct`)
/// read after the user talked directly to `task`: the messages sent,
/// and the end of the task's reply since.
fn direct_exchange(task: &str, sent: &[String], reply: &str) -> (String, String) {
    let quoted: Vec<String> = sent.iter().map(|m| format!("\"{}\"", one_line(m))).collect();
    let n = sent.len();
    let s = if n > 1 { "s" } else { "" };
    let note = format!(
        "The user talked directly to @{} ({} message{}): {}. Last reply of @{}: \"{}\"",
        task,
        n,
        s,
        quoted.join(", "),
        task,
        clip_tail(&one_line(reply), 2000)
    );
    (note, format!("You talked to @{} ({} message{})", task, n, s))
}

/// Issue #4: a failed turn's line that says the provider refused the
/// model or its key (a 400, 401, 403, 404 or 422, not for the request's
/// size): the status's words, `the provider refused it (404)`; None for
/// any other failure (network, rate limit, server error, a stop).
fn model_refusal(turn_done: &str) -> Option<String> {
    let why = turn_done.strip_prefix("failed: ")?;
    // the size's own refusal compacts (core/wire.bend too_large_mark)
    if why.contains("the request is too large") {
        return None;
    }
    let at = why.find(" refused the ")?;
    let rest = &why[at..];
    let open = rest.find('(')?;
    let status: u16 = rest[open + 1..].split(')').next()?.trim().parse().ok()?;
    [400, 401, 403, 404, 422]
        .contains(&status)
        .then(|| format!("the provider refused it ({})", status))
}

/// BR-007: the report a failed turn of a task sends to its parent, or
/// None when there is nothing to report: a completed or interrupted
/// turn, main (its own view shows the failure), or a retry loop someone
/// stopped on purpose. A call an interrupt stopped mid-answer says who
/// asked (the runtime's "interrupted by main" / "by the user") and is a
/// stop, not a failure.
fn failed_turn_report(agent: &str, turn_done: &str) -> Option<String> {
    let why = turn_done.strip_prefix("failed: ")?;
    if agent == "main" || why.starts_with("stopped retrying (interrupted by ") {
        return None;
    }
    if why.starts_with("interrupted by ") {
        return Some(format!("my turn stopped: {} — a new message continues it", why));
    }
    Some(format!(
        "my turn failed: {} — a new message retries it",
        why
    ))
}

/// The checks and texts of a new task the daemon prepares for sb-core
/// (RFC 0001 §7.1): a valid name or the slug of the objective, the brief
/// as the task reads it (sb-core adds the `# Task` header with the final
/// name).
/// The name sb-core gives a new task of base name `base` (hub/core.bend
/// `unique_name`): `base` when free, else `base-2`, `base-3`… cut to 24
/// characters, trailing dashes off before the suffix.
fn unique_name(st: &State, base: &str) -> String {
    let taken = |n: &str| n == MAIN || n == "user" || n == "hub" || st.resolve(n).is_some();
    if !taken(base) {
        return base.to_string();
    }
    (2..1002)
        .map(|i| {
            let suffix = format!("-{}", i);
            let head: String = base.chars().take(24usize.saturating_sub(suffix.len())).collect();
            format!("{}{}", head.trim_end_matches('-'), suffix)
        })
        .find(|c| !taken(c))
        .unwrap_or_else(|| base.to_string())
}

/// The `sb` commands that need git: off in a workspace without it (the
/// home workspace, docs/ambient-pages.md §5.1). Their name, else None.
fn git_only(req: &AgentReq) -> Option<&'static str> {
    match req {
        AgentReq::Land { .. } => Some("land"),
        AgentReq::Feature { .. } => Some("feature"),
        AgentReq::Flow { .. } => Some("flow"),
        _ => None,
    }
}

/// The one line `sb <cmd>` answers in a workspace without git.
pub(crate) fn not_in_a_repo(cmd: &str) -> String {
    format!("sb {}: not in a repo (this workspace has no git): files stay as you saved them, nothing to land", cmd)
}

fn new_task(name: Option<&str>, brief: &Brief, worktree: bool, with_changes: bool) -> Result<Value, String> {
    if brief.objective.trim().is_empty() {
        return Err("empty objective".into());
    }
    let base = match name {
        Some(n) if !n.is_empty() => {
            if !router::valid_name(n) {
                return Err(format!("invalid name: {} ([a-z0-9-], 24 characters max)", n));
            }
            n.to_string()
        }
        _ => router::slug(&brief.objective),
    };
    Ok(json!({"base": base, "brief": brief, "brief_text": prompts::brief_body(brief), "objective": brief.objective,
              "worktree": worktree, "with_changes": with_changes}))
}


pub const HELP: &str = "\
plain text        message to the agent in view (main by default)
@agent text       direct message to an agent, without main (@main from an agent)
/new [-w] [name:] objective   create an agent (-w: its own git worktree, --with-changes: with your changes)
/archive [agent] [--force]    stop an agent and archive it, with its worktree
/restore agent    bring an archived agent back (and its saved worktree)
/isolate agent    give a worktree to an agent that has not changed anything yet
/rename a b       rename an agent (the old name still works)
/answer N text    answer card N
/agents           list the agents and what they do
/interrupt        interrupt the turn of the agent in view
/compact          compact the conversation of the agent in view
/model [m] [default]  the model of the agent in view (default: also config.toml's)
/flow [pr|trunk]  how this repo ships code (PRs or straight to main), why, and switch it
/reasoning [effort]   its reasoning effort (none, low, medium, high, max...)";

#[path = "core_user.rs"]
mod user;

#[path = "merge.rs"]
mod merge;

#[path = "update_card.rs"]
pub mod update_card;

#[path = "signin_card.rs"]
pub mod signin_card;

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
