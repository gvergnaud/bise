//! The typed records of an agent's transcript, the hub's lines: a new
//! line is `sb <kind> :: <json>` ([`Feed::line`], two colons), read back
//! with serde ([`Feed::read_rest`]); every older text line
//! (`sb <kind> : <text>`, one colon and a space) is read in one place,
//! `thread/legacy.rs`. Two colons make the new form one prefix test: no
//! old writer ever wrote ` :: ` right after the kind. Transcripts are never
//! rewritten (their positions are ids, a rollback reads them), so both
//! forms stay readable. DRAFT until the first writer lands (issue 25,
//! chunk F sha 2): a name here may still change; from then on the wire
//! rule binds.
//!
//! One variant per kind the hub writes today (sb-core's `line` effects
//! and the Rust side's own lines), its fields what
//! [`crate::thread::lines::Hub`] reads from the text now, so the fold's
//! change is mechanical. The tag is `"sb"` (`kind` is a field of two
//! records); the line carries the kind in its prefix and the JSON
//! without it ([`Feed::line`], [`Feed::read`]).
//!
//! `v` (always 1 for now) only on the records whose shape grew before or
//! may change: card, approval, pr, landed, scheduled, wake.

use super::core_kinds;
use serde_json::Value;

core_kinds! {
    /// One hub record of a transcript.
    pub enum Feed, tag "sb" {
        /// his message (`via`: the view that brought it, `bise` when bise
        /// routed his words here; none on older lines)
        You = "you" => RYou { text: Body, via: OptStrOmit },
        /// the id of the `you` record right before it (same step)
        YouId = "you-id" => RYouId { id: Nat },
        /// the parts of the `you` record before it (msg-parts, a
        /// `Vec<parts::Part>`)
        YouParts = "you-parts" => RYouParts { parts: BodyJson },
        /// the parts of the route record right before it
        RouteParts = "route-parts" => RRouteParts { parts: Json },
        /// bise's home hub: his message `msg` went to the project `to`
        /// (hub id; `name` its name) as out `xid`, not to main
        Routed = "routed" => RRouted { to: Str, name: Str, xid: Nat, msg: Nat },
        /// the parts of message `id` the feed's owner received (send-parts:
        /// `sb send --attach`), after its msg-in; the fold puts them on
        /// the entry of that id
        MsgParts = "msg-parts" => RMsgParts { id: Nat, parts: BodyJson },
        /// the parts of message `id` the feed's owner sent, after its sent
        /// line; likewise by id
        SentParts = "sent-parts" => RSentParts { id: Nat, parts: BodyJson },
        /// the agent read these steered messages here, mid-turn
        Steered = "steered" => RSteered { ids: Nats },
        /// the agent's runtime received these steered messages
        SteerRx = "steer-rx" => RSteerRx { ids: Nats },
        /// his fn context (S9), the FnContext JSON
        Context = "context" => RContext { context: Json },
        /// his message could not reach `to` (stopped, dropped, archived)
        Undelivered = "undelivered" => RUndelivered { to: Str, text: Body },
        /// what the feed's owner received (`id` none: older lines);
        /// `view`: the plugin whose MCP App view wrote it (from `view`)
        MsgIn = "msg-in" => RMsgIn { from: Str, id: OptNatOmit, body: Body, view: OptStrOmit },
        /// a message between two others (`id` none: before BISE-110)
        Msg = "msg" => RMsg { from: Str, to: Str, id: OptNatOmit, body: Body },
        /// what the feed's owner sent (`ask`: it awaits a reply)
        Sent = "sent" => RSent { to: Str, id: Nat, ask: Bool, body: Body },
        /// an agent writing to the user
        MsgYou = "msg-you" => RMsgYou { from: Str, body: Str },
        /// main answered an agent for the user
        Answered = "answered" => RAnswered { agent: Str, question: Body, answer: Body, why: Str },
        /// an attention card opened
        Card = "card" => RCard { v: Ver, id: Nat, kind: Str, agent: Str, body: Str },
        /// the approvals gate of the running call: `check`, `card`, `done`
        Gate = "gate" => RGate { step: Str },
        /// a gate's card answered: `how` allowed|outside|outside-always|no
        Approval = "approval" => RApproval { v: Ver, how: Str, who: Str, what: Str, note: Str },
        /// a card closed (`res`: answered, closed, …)
        CardClosed = "card-closed" => RCardClosed { id: Nat, res: Str },
        /// his words routed to `who`: his answer to card `card`, or (none)
        /// his message to that agent. `msg`: the message his answer
        /// became (the asker's thread folds its msg-in into this row);
        /// `pick_n`/`pick`: an option's number and its words when his
        /// answer was one of the card's options (both or neither)
        Route = "route" => RRoute { who: Str, card: OptNatOmit, said: Str, msg: OptNatOmit, pick_n: OptNatOmit, pick: OptStrOmit },
        /// pr-news: `state` news|done|failing
        Pr = "pr" => RPr { v: Ver, state: Str, number: Nat, url: Str, text: Str },
        /// an artifact registered, its `version`
        Artifact = "artifact" => RArtifact { id: Str, agent: Str, title: Str, kind: Str, version: Nat },
        /// a land: `files` changed, `add`/`del` lines
        Landed = "landed" => RLanded { v: Ver, agent: Str, target: Str, from: Str, sha: Str, files: Nat, add: Nat, del: Nat },
        /// a task created (`main → new task @x : objective`)
        Spawn = "spawn" => RSpawn { text: Str },
        /// computer use (design §7.3)
        Computer = "computer" => RComputer { text: Str },
        Direct = "direct" => RDirect { text: Str },
        Info = "info" => RInfo { text: Str },
        Warn = "warn" => RWarn { text: Str },
        /// a scheduled task set or ended (`thread::Scheduled`'s JSON)
        Scheduled = "scheduled" => RScheduled { v: Ver, timer: Json },
        /// an interrupt (sb-core writes it in the interrupt's step)
        Stopped = "stopped" => RStopped { text: Str },
        /// a turn /pause cut (cost-pause, pause.bend): `‖ paused · its turn stopped`
        Paused = "paused" => RPaused { text: Str },
        /// event-wake: a watch ended (`thread::WakeFold`'s JSON)
        Wake = "wake" => RWake { v: Ver, watch: Json },
        /// the answer he had read as it streamed in, when its turn failed
        Kept = "kept" => RKept { text: Str },
    }
}

/// Between a new line's kind and its JSON (an old line has ` : `).
pub const SEP: &str = " :: ";

impl Feed {
    /// Its transcript line: `sb <kind> :: <json without the tag>`.
    pub fn line(&self) -> String {
        let mut v = self.to_value();
        if let Value::Object(m) = &mut v {
            m.remove("sb");
        }
        format!("sb {}{SEP}{}", self.tag(), v)
    }

    /// A hub line after its `sb `, read when it is the new form
    /// (`<kind> :: <json>`): its kind and the record (an error when the
    /// JSON doesn't fit the kind: a bug of the writer); None for an older
    /// text line (the legacy reader's).
    pub fn read_rest(rest: &str) -> Option<(&str, Result<Feed, String>)> {
        let (kind, json) = rest.split_once(SEP)?;
        if kind.is_empty() || kind.contains(' ') {
            return None;
        }
        Some((kind, Feed::read(kind, json)))
    }

    /// An info line.
    pub fn info(text: impl Into<String>) -> Feed {
        Feed::Info(Info { text: text.into() })
    }

    /// A warning line.
    pub fn warn(text: impl Into<String>) -> Feed {
        Feed::Warn(Warn { text: text.into() })
    }

    /// A kind and its JSON (without the tag) read back.
    pub fn read(kind: &str, json: &str) -> Result<Feed, String> {
        let mut m = match serde_json::from_str::<Value>(json) {
            Ok(Value::Object(m)) => m,
            Ok(_) => return Err(format!("sb {kind}: not a JSON object")),
            Err(e) => return Err(format!("sb {kind}: {e}")),
        };
        m.insert("sb".into(), Value::String(kind.to_string()));
        Feed::decode(Value::Object(m))
    }
}
