//! sb-core's inputs: what the hub's Rust side asks it to decide, one JSON
//! line each (`{"t": <tag>, …}`), and [`Step`], the envelope the hub adds
//! (its clock, whether the workspace is git, the answers to the step's
//! queries so far).
//!
//! One variant per tag of sb-core's dispatch (`dispatch` in
//! bend/hub/core.bend, through the generated gen/in.bend: `card_close` is
//! its own tag and the Bend constructor `IConfirmClose`). Fields are
//! today's: a field sb-core reads with `C.get_s` is `Str` (missing reads
//! ""), `C.get_nat` `Nat`, `C.get_b` `Bool`, `C.opt_str`/`C.opt_nat`
//! `OptStr`/`OptNat` (or their `Omit` twins when the hub writes nothing
//! for none), a value it keeps whole `Json`.

use super::core_kinds;
use serde_json::Value;

core_kinds! {
    /// One input of sb-core (its dispatch's tags).
    pub enum In, tag "t" {
        /// an agent's `sb` request (`token` 0: the hub's own, no reply
        /// awaited); `req` is the request as the agent sent it; `who`,
        /// `dir`, `sock`: its sender, as for `speaks` (`hub`: the hub's own)
        Req = "req" => IReq { token: Nat, from: Str, req: Json, who: Str, dir: StrOmit, sock: StrOmit },
        /// a request the hub serves itself (inspect, history, page_*,
        /// artifact...: no `req` step): may its sender say `from`? (issue
        /// 37 #2, bend/hub/peer.bend); fx `spoke` answers. `who`, `dir`:
        /// the judge's fact on the sender (outside, hub, agent, other_hub,
        /// gone; "" reads gone), `sock` the socket it came on
        Speaks = "speaks" => ISpeaks { token: Nat, from: Str, cmd: Str, who: Str, dir: StrOmit, sock: StrOmit },
        /// the waits' timeouts, `sb every`'s timers and `sb wake`'s max
        Tick = "tick" => ITick {},
        /// an agent's turn ended (`leftover`: steering its runtime never
        /// read; `held`: stopped on an expired sign-in, not over)
        Idle = "idle" => IIdle { agent: Str, leftover: Bool, held: Bool },
        TurnStarted = "turn_started" => ITurnStarted { agent: Str },
        /// the runtime received steered messages
        SteerRx = "steer_rx" => ISteerRx { agent: Str },
        /// the model read steered messages
        Steered = "steered" => ISteered { agent: Str },
        /// an agent's written answer
        Assistant = "assistant" => IAssistant { agent: Str, text: Str },
        /// an agent edited a file
        Touch = "touch" => ITouch { agent: Str, path: Str },
        /// an agent's REPL is up
        Ready = "ready" => IReady { agent: Str },
        /// (`stalled`: its start made no progress; `loaded`: the machine
        /// was at load x2+ then; `row`: that REPL's stalls in a row, the
        /// shell's count, repl_start::Starts. An older core ignores them:
        /// crashed, it counts as before; hub/stall.bend). `stopped`: it
        /// ended by SIGTERM, SIGINT or SIGHUP, a request to stop (from the
        /// hub's stop, `sb restart`, a teardown, a `kill`), not a crash;
        /// `row` then counts its stops too until it connects
        Exited = "exited" => IExited { agent: Str, crashed: Bool, reason: Str, stalled: Bool, loaded: Bool, row: Nat, stopped: Bool },
        /// the hub started: sb-core's state is the replayed journal
        Boot = "boot" => IBoot {},
        /// his message to the agent in view (`parts`: msg-parts JSON, only
        /// when he sent typed parts). `from`: the thread he typed it in when
        /// not `focus`'s (at-start: his `@agent …` from another thread), ""
        /// none: sb-core leaves one route row there
        Say = "say" => ISay { focus: Str, text: Str, queued: Bool, parts: StrOmit, from: StrOmit },
        /// his `@agent` message
        Route = "route" => IRoute { target: Str, text: Str, focus: Str, queued: Bool, parts: StrOmit },
        /// `/new`: `brief` is the Brief as JSON, `brief_text` its body
        New = "new" => INew { base: Str, brief: Json, brief_text: Str, objective: Str, worktree: Bool, with_changes: Bool },
        Drop = "drop" => IDrop { name: Str, force: Bool },
        Restore = "restore" => IRestore { name: Str },
        Isolate = "isolate" => IIsolate { name: Str },
        /// hub-loop: the shell made (or failed to make) the worktree of
        /// task `name` (fx `worktree_create`): `ws` its workspace, null
        /// when `err` says why not
        Placed = "placed" => IPlaced { name: Str, ws: Json, err: Str },
        /// hub-loop: the shell removed (or failed to remove) the worktree
        /// of task `name` (fx `worktree_drop`): `snapshot` the ref of its
        /// saved work (none: nothing to save), `err` git's words
        Unplaced = "unplaced" => IUnplaced { name: Str, snapshot: OptStr, err: Str },
        /// `sb feature retire <name>` (main's): the feature's tip is in
        /// the trunk (the hub checked); its live agents land on the trunk
        /// from now on. `kept`: the branch's name after the retire
        /// (`<name>-retired`), `main`: the trunk's short name, for the
        /// words
        FeatureRetire = "feature_retire" => IFeatureRetire { name: Str, kept: Str, main: Str },
        Rename = "rename" => IRename { name: Str, new_name: Str, valid: Bool },
        /// his answer to a card
        Answer = "answer" => IAnswer { card: Nat, text: Str, parts: StrOmit },
        Interrupt = "interrupt" => IInterrupt { agent: Str },
        /// his queued message taken back (HubCmd queued_take)
        Unqueue = "unqueue" => IUnqueue { id: Nat },
        /// his queued message said now (HubCmd queued_steer)
        SteerQueued = "steer_queued" => ISteerQueued { id: Nat },
        Passthrough = "passthrough" => IPassthrough { focus: Str, line: Str },
        /// a client's view moved (`note`, `direct`: what he decided
        /// directly with the agent he leaves, RFC 0001 §7.4)
        Focus = "focus" => IFocus { focus: Str, note: OptStr, direct: Str },
        /// `/close N`
        Close = "close" => IClose { card: Nat },
        /// tests, and a restarted sb-core: an agent's REPL state
        ForceRun = "force_run" => IForceRun { agent: Str, run: Str },
        /// the approvals gate: a card opens, closes
        ConfirmOpen = "confirm_open" => IConfirmOpen { agent: Str, text: Str },
        ConfirmClose = "confirm_close" => IConfirmClose { card: Nat, res: Str },
        /// an MCP App view's call that needs his ok, outside the agent's
        /// turn (docs/mcp-apps.md, C5): a `confirm` card placed
        /// `view:<agent>/<call>` (shown only), its `ask` (the tool and its
        /// arguments, the shell's canonical JSON) typed on the card
        /// (`Card.view`); closed as timed out at its due, at a stop and at
        /// boot; its id comes back as fx `view_card`
        ViewConfirmOpen = "view_confirm_open" => IViewConfirmOpen { agent: Str, call: Str, text: Str, ask: Str },
        /// an MCP App view's `ui/message` (C5): one message from the
        /// fixed sender `view`, its plugin typed (`Msg.view`), queued to
        /// `agent` only; never his words
        ViewMessage = "view_message" => IViewMessage { agent: Str, plugin: Str, text: Str },
        /// an MCP App view's `ui/update-model-context` (C5, review d5):
        /// kept for `agent` (the last one per view `call`, never
        /// journaled: lost at a restart) and joined before his next
        /// message to it, once; `text` its model text (the shell's
        /// `parts::render` of that one part), `part` its
        /// `Part::ViewContext`; dropped when the agent stops
        ViewContext = "view_context" => IViewContext { agent: Str, call: Str, text: Str, part: Json },
        /// pr-news: GitHub's news to the agent that owns the PR
        Forge = "forge" => IForge { to: Str, text: Str },
        /// a check still fails after two tries: a card for him
        ForgeCard = "forge_card" => IForgeCard { agent: Str, text: Str },
        /// a hub item about a place (pr-design §6.3)
        CardOpen = "card_open" => ICardOpen { kind: Str, agent: Str, text: Str, place: OptStr, pr: OptNat },
        /// a hub item closed: the same step as `confirm_close`
        CardClose = "card_close" => IConfirmClose { card: Nat, res: Str },
        /// a message of the hub itself (from `bise`) to an agent
        HubMsg = "hub_msg" => IHubMsg { to: Str, text: Str },
        /// land-claims: the claims of `place`'s agents whose file has no
        /// change there now (the shell's `git status`, after a land in the
        /// shared folder and at boot): each agent's claims on them end
        ClaimsClean = "claims_clean" => IClaimsClean { place: Str, paths: Strs },
        /// a page question's card answered on its page (wake-deliver): the
        /// card closes `answered`; its asker hears it when that is not
        /// `agent`, the agent the pick went to (reach.bend)
        /// `block`: the page's question block (the inbox's record)
        PageAnswered = "page_answered" => IPageAnswered { card: Nat, agent: Str, page: Str, text: Str, block: StrOmit },
        /// a page's own card (plan-ok, architect m_26549, m_26571): a plan
        /// an agent proposes (kind "plan"). Its asker is the first live
        /// agent of `agents` (a plan's data-agent, then the page's agent),
        /// else main (laws page_card_asker_*); its answer is the hub's
        /// page path. `key`: what it asks on its page ('plan:<block>'),
        /// for the record. `about`: the page as an artifact (the shell's
        /// ArtLink), stored on the card opened as is, never read
        PageCard = "page_card" => IPageCard { agents: Strs, kind: Str, text: Str, key: Str, about: OptJsonOmit },
        /// the shell appended card `card`'s route line (his answer) to
        /// `agent`'s transcript at `pos`: journaled once per card closed
        /// with his answer (law answer_pos_once)
        AnswerPos = "answer_pos" => IAnswerPos { card: Nat, agent: Str, pos: Nat },
        /// `sb every`: the hub checked the request and computed `next_ms`;
        /// one of `every_ms`, `daily_min`; `name` only when given
        EverySet = "every_set" => IEverySet {
            agent: Str, by: Str, text: Str, next_ms: Nat, name: StrOmit,
            every_ms: OptNatOmit, daily_min: OptNatOmit, until_ms: OptNatOmit, times: OptNatOmit, page: OptStrOmit,
        },
        /// `wake`: what bise tells the agent when he stopped it ("" none)
        EveryStop = "every_stop" => IEveryStop { id: Nat, why: Str, wake: Str },
        EveryRun = "every_run" => IEveryRun { id: Nat, text: Str },
        EveryName = "every_name" => IEveryName { id: Nat, name: Str },
        /// bise's cross-hub messages (S2): a project hub receives one
        XIn = "x_in" => IXIn { token: Nat, hub: Str, xid: Nat, kind: Str, text: Str, name: Str, parts: StrOmit },
        XAck = "x_ack" => IXAck { xid: Nat },
        XFail = "x_fail" => IXFail { xid: Nat },
        XReply = "x_reply" => IXReply { xid: Nat, name: Str, text: Str },
        /// his words to the agent in view, before they are said (issue 37
        /// #1): sb-core holds them for a project when he addressed one
        /// (`@<project> …`, xhub.bend `addressed`), and answers fx
        /// `route_asked` either way. `text` goes whole to the project
        /// (`@<project>` included); `context`: his fn context's JSON text
        /// ("" none); `projects`:
        /// the registered projects `{id, name}` (bise's home hub only,
        /// [] elsewhere)
        RouteAsk = "route_ask" => IRouteAsk { focus: Str, text: Str, via: Str, queued: Bool, context: Str, parts: StrOmit, projects: Jsons },
        RouteCorrect = "route_correct" => IRouteCorrect { rid: Nat, to: Str, name: Str },
        RouteCancel = "route_cancel" => IRouteCancel { rid: Nat },
        /// S10: follow a task
        Follow = "follow" => IFollow { agent: Str, on: Bool },
        /// `sb wake`: `spec` is the watch's spec as the hub wrote it;
        /// `then`: `--then` (Some true: a pause, the agent resumes its
        /// work at the end) or `--alarm` (Some false); none: sb-core's
        /// default by the spec's kind (wakes.bend `resumes_of`)
        WakeSet = "wake_set" => IWakeSet { agent: Str, spec: Json, max_text: Str, max_ms: Nat, quiet: Bool, then: OptBoolOmit },
        /// `tell`: his stop's words for the watch's agent (one message
        /// from bise); none: the agent's own `sb wake --stop`
        WakeStop = "wake_stop" => IWakeStop { id: Nat, why: Str, tell: StrOmit },
        WakeHit = "wake_hit" => IWakeHit { id: Nat, text: Str },
        /// cost-pause: his pause and resume of the whole project
        /// (hub/pause.bend; the client socket only)
        Pause = "pause" => IPause {},
        Resume = "resume" => IResume {},
    }
}

/// One input as the link carries it: the input's own fields and the
/// envelope sb-core's `start` reads (`now`, `git`, `ans`), in one object.
/// (A restarted sb-core gets its agents' `force_run` bare, without it:
/// `Hub::raw`; sb-core reads 0, false, none.)
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub input: In,
    /// the hub's clock for the input and its re-sends (ms)
    pub now: u64,
    /// the workspace is a git repository
    pub git: bool,
    /// the answers to the step's queries so far (a `need` replays it)
    pub ans: Vec<Value>,
}

impl Step {
    /// The envelope fields, in the order the hub writes them.
    pub const FIELDS: &'static [super::Field] = &[
        super::Field::new("now", super::FieldKind::Nat),
        super::Field::new("git", super::FieldKind::Bool),
        super::Field::new("ans", super::FieldKind::Jsons),
    ];

    /// The input's object with the envelope's fields set.
    pub fn to_value(&self) -> Value {
        let mut v = self.input.to_value();
        if let Value::Object(m) = &mut v {
            m.insert("now".into(), Value::from(self.now));
            m.insert("git".into(), Value::Bool(self.git));
            m.insert("ans".into(), Value::Array(self.ans.clone()));
        }
        v
    }

    pub fn encode(&self) -> String {
        self.to_value().to_string()
    }

    /// One link line: the envelope read with sb-core's defaults (0, false,
    /// none), the rest as an [`In`].
    pub fn decode(v: Value) -> Result<Step, String> {
        let Value::Object(mut m) = v else { return Err("not an object".into()) };
        let now = m.remove("now").and_then(|n| n.as_u64()).unwrap_or(0);
        let git = m.remove("git").and_then(|g| g.as_bool()).unwrap_or(false);
        let ans = match m.remove("ans") {
            Some(Value::Array(a)) => a,
            _ => Vec::new(),
        };
        Ok(Step { input: In::decode(Value::Object(m))?, now, git, ans })
    }
}

impl In {
    /// The input as a step with this envelope.
    pub fn step(self, now: u64, git: bool, ans: Vec<Value>) -> Step {
        Step { input: self, now, git, ans }
    }
}
