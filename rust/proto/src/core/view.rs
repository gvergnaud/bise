//! sb-core's view (issue 21, D3): the state the hub's mirror reads after
//! each step (a step's answer `view`, `view_all`'s), as sb-core writes it
//! (bend/hub/view.bend, through the encoders tests/core_gen.rs writes in
//! `bend/hub/gen/view.bend`).
//!
//! Untagged records: [`View`], its [`ViewAgent`] rows and [`ViewMsg`]
//! records. An agent row's opaque JSON is typed here (its brief, ws,
//! report: core::state's Rust-only records, read leniently, see
//! [`super::lenient`]); a message and its state, the cards, timers and
//! watches stay `Json`, read by the hub into core::state's generated
//! types.
//!
//! A step's view carries only what the step changed (view.bend
//! `changed`, `durable_kvs`, `timers_kvs`, `wakes_kvs`): the agents it
//! changed (`all_agents` false), the messages it touched (`all_msgs`
//! false), and the order, cards, notes, timers and watches only when an
//! event changed them (absent otherwise: the hub keeps its own).
//!
//! sb-core never decodes a view, so View has no Bend decoder and no round
//! law: it is proven Rust-side (the table test in tests.rs, the fixture
//! fixtures/core_view.jsonl, one view per shape).

use super::core_records;
use super::state::{Brief, Declared, Lifecycle, Report, Run, Workspace};
use serde::{Deserialize, Serialize};

core_records! {
    /// The records of sb-core's view, for the Bend encoders.
    pub const VIEW = "View" {
        /// One agent's row: its durable record, its runtime (`run`,
        /// `waiting`, `turn_ms`, `turns`) and who it waits on.
        ViewAgent = "agent" => VAgent {
            name: Str,
            dir: Str,
            is_main: Bool,
            parent: OptStr,
            /// the task's brief (null: none)
            brief: Json(Brief),
            created_ms: Nat,
            /// its workspace
            ws: Json(Workspace),
            /// `active`, `archived`, ...
            lifecycle: Str(Lifecycle),
            failure: OptStr,
            /// `{status, note}` from `sb status`, null when none
            declared: OptJson(Declaration),
            /// its last `sb report` (null: none)
            last_report: OptJson(Report),
            aliases: Strs,
            files: Strs,
            snapshot_ref: OptStr,
            follow: Bool,
            /// `down`, `starting`, `idle`, `busy`
            run: Str(Run),
            waiting: Bool,
            /// the recipient of the message its `sb wait` waits on
            waiting_on: OptStr,
            /// when its turn started (null: no turn)
            turn_ms: OptNat,
            turns: Nat,
        },
        /// One message with its state, whether its question is settled,
        /// and what sb-core says of it (model.bend's predicates, the ones
        /// its steps use): `queued` waits for delivery, `owes` was
        /// delivered and expects a reply not given yet. The hub's
        /// Waiting reads these, never a predicate of its own (hot-view
        /// sha 2b).
        ViewMsg = "msg" => VMsg { msg: Json, state: Json, settled: Bool, queued: Bool, owes: Bool },
        /// The view. A missing `all_agents` reads true (every agent), a
        /// missing `next_msg`/`next_card` 1, as the hub read them before.
        View = "view" => VView {
            agents: Recs(ViewAgent),
            /// false: `agents` are the ones the step changed, keep the others
            all_agents: Bool = true,
            msgs: Recs(ViewMsg),
            /// true: `msgs` are every message (drop the ones not in it)
            all_msgs: Bool,
            /// view-all-perf: `view_all`'s cold messages, each a
            /// [`ColdMsg`] row without its body (sb-core decides which,
            /// view.bend `ships_whole`); `msgs` holds the others, whole.
            /// The hub asks the link's `msg_bodies` for a cold one's body.
            /// Absent in a step's view and from an older sb-core.
            cold: OptJsonsOmit,
            next_msg: Nat = 1,
            next_card: Nat = 1,
            /// cost-pause: since when the project is paused (null: it runs)
            paused: OptNat,
            order: OptStrsOmit,
            cards: OptJsonsOmit,
            notes: OptStrsOmit,
            /// `sb every`'s live timers
            timers: OptJsonsOmit,
            timers_ended: OptJsonsOmit,
            /// `sb wake`'s live watches
            wakes: OptJsonsOmit,
            wakes_ended: OptJsonsOmit,
        },
    }
}

impl View {
    /// sb-core's view: an error when a field doesn't fit (a bug of
    /// sb-core's encoder; the hub logs it and keeps its mirror).
    pub fn decode(v: &serde_json::Value) -> Result<View, String> {
        <View as serde::Deserialize>::deserialize(v).map_err(|e| format!("view: {e}"))
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("a view always serializes")
    }
}

/// view-all-perf: one cold message of `view_all` ([`View::cold`]): its
/// row without its body, as sb-core writes it (view.bend `cold_json`),
/// one array `[id, thread, from, to, reply_to, expect_reply, created_ms,
/// plain, state, settled]` (a 54k-message view was ~14 MB of keys as
/// objects). Never queued nor owing a reply: sb-core sends those whole.
#[derive(Clone, Debug, PartialEq)]
pub struct ColdMsg {
    pub id: u64,
    pub thread: u64,
    pub from: String,
    pub to: String,
    pub reply_to: Option<u64>,
    pub expect_reply: bool,
    pub created_ms: u64,
    pub plain: bool,
    /// its delivery state, as a message row's `state`
    pub state: serde_json::Value,
    pub settled: bool,
}

type ColdRow = (u64, u64, String, String, Option<u64>, bool, u64, bool, serde_json::Value, bool);

impl ColdMsg {
    /// A row that doesn't fit is an error (a bug of sb-core's encoder).
    pub fn decode(v: &serde_json::Value) -> Result<ColdMsg, String> {
        let (id, thread, from, to, reply_to, expect_reply, created_ms, plain, state, settled) =
            <ColdRow as serde::Deserialize>::deserialize(v).map_err(|e| format!("cold msg: {e}"))?;
        Ok(ColdMsg { id, thread, from, to, reply_to, expect_reply, created_ms, plain, state, settled })
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::json!([self.id, self.thread, self.from, self.to, self.reply_to, self.expect_reply, self.created_ms, self.plain, self.state, self.settled])
    }
}

/// An agent's declared status as its view row writes it (`sb status`:
/// view.bend's `{status, note}` from its Agent's `declared` and `note`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declaration {
    pub status: Declared,
    #[serde(default)]
    pub note: String,
}
