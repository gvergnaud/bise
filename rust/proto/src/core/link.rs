//! The link's own requests (`{"t": <tag>, …}`, like an input): main.bend
//! answers them before any dispatch (its `tag_of`), so they are not
//! sb-core inputs and have no envelope. Any other tag is a step (an
//! [`super::In`]): its Bend fallback is `TStep`.

use super::{core_kinds, Field, FieldKind, Fx, Kind, Table};

core_kinds! {
    /// One request of the hub to the sb-core process itself.
    pub enum Link, tag "t" {
        /// reset the hub's state (a new sb-core, a restart)
        Init = "init" => TInit { workspace: Str },
        /// one journal event (answers `{"skipped": true}` for a kind it
        /// does not know)
        Replay = "replay" => TReplay { ev: Json },
        /// a batch of journal events at boot (answers the skipped indexes)
        ReplayMany = "replay_many" => TReplayMany { evs: Jsons },
        /// answers `{}`
        Ping = "ping" => TPing {},
        /// the whole view (its cold messages without their body: `whole`
        /// false, view-all-perf)
        ViewAll = "view_all" => TView {},
        /// the messages `ids`, whole: answered like `view_all` (`{"view":
        /// …}`), a view with only those messages (a step's view of them:
        /// no agent, no order). The hub asks for the bodies `view_all`
        /// left out, at most [`MSG_BODIES_MAX`] ids a call.
        MsgBodies = "msg_bodies" => TMsgBodies { ids: Nats },
        /// the durable state as its photo (issue 36), answered `{"st":
        /// <St photo>}`; read-only, refused while an env query is pending
        Photo = "photo" => TPhoto {},
        /// the durable state from a photo, the runtime fresh (issue 36):
        /// answers `{}`; a photo that is not an object changes nothing and
        /// answers the error
        RestorePhoto = "restore_photo" => TRestorePhoto { st: Json },
    }
}

/// The most ids one `msg_bodies` asks for: sb-core walks its cold
/// messages down to the oldest id asked, each one checked against the
/// ids, so a batch stays small.
pub const MSG_BODIES_MAX: usize = 256;

/// sb-core's answer line to one link request or step (main.bend's
/// `answer`, `replay`, `replay_many`, `handle`). Untagged: its keys say
/// which it is, tried in this order (a step's answer has a `view` too).
/// Each shape is a row of [`ANSWER`], from which tests/core_gen.rs writes
/// sb-core's encoder (`bend/hub/gen/link.bend`'s `answer_<tag>`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Answer {
    Step(AStep),
    Need(ANeed),
    Photo(APhoto),
    View(AView),
    Skipped(ASkipped),
    Error(AError),
    Done(ADone),
    /// a shape this side doesn't know (a newer sb-core): kept whole
    Unknown(serde_json::Value),
}

/// A committed step: its effects in order, whether the durable state
/// changed (the hub saves it), the view of what the step changed.
/// `msg`: the id of his message the step made (core.bend's `said`: an
/// input of his makes it when it arrives, queued too), the id his
/// you-id line carries when it is delivered (A1, architect m_21450:
/// `turn/send`'s result hands it to the asker); none when the step made
/// none, and from an older sb-core.
/// `ans_*`: his answer to a card in this step (inbox picks (3),
/// architect m_22636), from the one answer that wrote its route line
/// (core.bend answer_line): the card, who asked, what he said, the card's
/// question, his pick (an option's words). card, who, said and ask all
/// set or all absent, pick only with them (laws in bend/laws/threads.bend);
/// none from an older sb-core.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AStep {
    #[serde(with = "fx_list")]
    pub fx: Vec<Fx>,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default)]
    pub view: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ans_card: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ans_who: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ans_said: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ans_ask: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ans_pick: Option<String>,
}

/// A git answer is missing: the hub runs the query and sends the same
/// input again with its answer appended to `ans` (nothing was committed).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ANeed {
    pub need: serde_json::Value,
}

/// `photo`: the durable state, its photo shape (St.photo_json).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct APhoto {
    pub st: serde_json::Value,
}

/// `view_all`: the whole view.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AView {
    pub view: serde_json::Value,
}

/// `replay`: `true`, a journal event of a kind this sb-core doesn't know
/// (written by a newer hub before a rollback), not applied;
/// `replay_many`: the indexes in the batch of those.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ASkipped {
    pub skipped: Skipped,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Skipped {
    One(bool),
    At(Vec<u64>),
}

/// A line that is not JSON (`json: <why>`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AError {
    pub error: String,
}

/// `{}`: init, ping, an applied replay.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ADone {}

impl Answer {
    /// One answer line (any JSON object is an answer: `Unknown` at worst).
    pub fn decode_line(line: &str) -> Result<Answer, String> {
        serde_json::from_value(crate::parse(line)?).map_err(|e| format!("answer: {e}"))
    }

    pub fn to_value(&self) -> serde_json::Value {
        match self {
            Answer::Unknown(v) => v.clone(),
            a => serde_json::to_value(a).expect("an answer always serializes"),
        }
    }
}

/// An answer's `fx`, which never fails (architect m_18746): a failed list
/// would make the untagged `Answer` fall through to `Unknown` and the hub
/// drop every effect of the step, its journal included. Each item goes
/// through [`Fx::decode`]; a tag this side doesn't know, or a known tag
/// whose fields don't decode, or an item without a tag, is `Fx::Unknown`
/// (the hub logs it), and costs only that item. Not a list: no effects.
mod fx_list {
    use super::Fx;
    use serde::{Deserialize, Deserializer, Serializer};
    use serde_json::Value;

    pub fn serialize<S: Serializer>(fx: &[Fx], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(fx.iter().map(Fx::to_value))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Fx>, D::Error> {
        let Value::Array(vals) = Value::deserialize(d)? else { return Ok(Vec::new()) };
        Ok(vals.into_iter().map(item).collect())
    }

    fn item(v: Value) -> Fx {
        Fx::decode(v.clone()).unwrap_or_else(|_| Fx::Unknown {
            tag: v.get("fx").and_then(Value::as_str).unwrap_or_default().to_string(),
            raw: v,
        })
    }
}

/// The shapes of [`Answer`], for the Bend encoders (tag: the encoder's
/// name after `answer_`; no tag key on the wire). `skipped` has two
/// shapes, one row each.
pub const ANSWER: Table = Table {
    name: "Answer",
    tag_key: "",
    kinds: &[
        Kind { tag: "step", rust: "Step", bend: "AStep", fields: &[Field::new("fx", FieldKind::Jsons), Field::new("dirty", FieldKind::Bool), Field::new("view", FieldKind::Json), Field::new("msg", FieldKind::OptNatOmit),
            Field::new("ans_card", FieldKind::OptNatOmit), Field::new("ans_who", FieldKind::OptStrOmit), Field::new("ans_said", FieldKind::OptStrOmit), Field::new("ans_ask", FieldKind::OptStrOmit), Field::new("ans_pick", FieldKind::OptStrOmit)] },
        Kind { tag: "need", rust: "Need", bend: "ANeed", fields: &[Field::new("need", FieldKind::Json)] },
        Kind { tag: "photo", rust: "Photo", bend: "APhoto", fields: &[Field::new("st", FieldKind::Json)] },
        Kind { tag: "view", rust: "View", bend: "AView", fields: &[Field::new("view", FieldKind::Json)] },
        Kind { tag: "skipped", rust: "Skipped", bend: "ASkipped", fields: &[Field::new("skipped", FieldKind::Bool)] },
        Kind { tag: "skipped_at", rust: "Skipped", bend: "ASkippedAt", fields: &[Field::new("skipped", FieldKind::Nats)] },
        Kind { tag: "error", rust: "Error", bend: "AError", fields: &[Field::new("error", FieldKind::Str)] },
        Kind { tag: "done", rust: "Done", bend: "ADone", fields: &[] },
    ],
};
