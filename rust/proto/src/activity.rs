//! `hub/activity` (les bims, architect m_27581): who messages whom and
//! who reports, hub-wide, for every typed client connection. The hub
//! makes it from sb-core's own journal effects (a message sent, a report
//! made: switchboard `activity.rs`); a boot replay makes none.
//!
//! What never leaves: a message's body, a report's text, a tool's args.
//! Kinds, names, ids and times only.

use crate::rows::ReportKind;
use serde::{Deserialize, Serialize};

/// One end of a message: the user, an agent, or one of the hub's pseudo
/// senders. Typed, so no client compares a name with "you" (a client
/// says "you" from its words catalog).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "who", rename_all = "snake_case")]
pub enum Party {
    /// the user (from any client)
    You,
    /// an agent, by its name
    Agent { name: String },
    /// bise itself (the hub's own notes to an agent)
    Bise,
    /// GitHub's news about a PR (pr-news)
    Github,
    /// an MCP App view's `ui/message` (sb-core's sender `view`; its
    /// plugin is on the message, never in this word)
    View,
    /// a kind this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

impl Party {
    /// sb-core's word for a message's end (`model.bend`'s USER, HUB,
    /// GITHUB, VIEW, else an agent's name).
    pub fn of_word(w: &str) -> Party {
        match w {
            "user" => Party::You,
            "switchboard" | "bise" => Party::Bise,
            "github" => Party::Github,
            "view" => Party::View,
            name => Party::Agent { name: name.to_string() },
        }
    }
}

/// How a message was sent (`sb send --mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum MsgMode {
    /// it reaches a busy recipient at once, mid-turn
    Steer,
    /// it waits for the recipient's turn to end
    Queued,
    /// a mode this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

/// One activity event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActivityEv {
    /// a message was sent (its id is the `m_<id>` agents see)
    Message { id: u64, from: Party, to: Party, mode: MsgMode, expects_reply: bool, at_ms: u64 },
    /// an agent reported (`sb report`, or a failed turn)
    Report { agent: String, report: ReportKind, at_ms: u64 },
    /// a kind this version doesn't know (a newer hub)
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_wire_shape() {
        let m = ActivityEv::Message {
            id: 7,
            from: Party::You,
            to: Party::Agent { name: "main".into() },
            mode: MsgMode::Steer,
            expects_reply: false,
            at_ms: 1,
        };
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            json!({"kind": "message", "id": 7, "from": {"who": "you"}, "to": {"who": "agent", "name": "main"},
                   "mode": "steer", "expects_reply": false, "at_ms": 1})
        );
        let r = ActivityEv::Report { agent: "docs".into(), report: ReportKind::Done, at_ms: 2 };
        assert_eq!(serde_json::to_value(&r).unwrap(), json!({"kind": "report", "agent": "docs", "report": "done", "at_ms": 2}));
    }

    #[test]
    fn newer_kinds_read_as_unknown() {
        assert_eq!(serde_json::from_value::<ActivityEv>(json!({"kind": "dance", "x": 1})).unwrap(), ActivityEv::Unknown);
        assert_eq!(serde_json::from_value::<Party>(json!({"who": "a_robot"})).unwrap(), Party::Unknown);
        assert_eq!(serde_json::from_value::<MsgMode>(json!("later")).unwrap(), MsgMode::Unknown);
    }

    #[test]
    fn sb_cores_words_are_typed() {
        assert_eq!(Party::of_word("user"), Party::You);
        assert_eq!(Party::of_word("switchboard"), Party::Bise);
        assert_eq!(Party::of_word("github"), Party::Github);
        assert_eq!(Party::of_word("view"), Party::View);
        assert_eq!(Party::of_word("you"), Party::Agent { name: "you".into() }, "an agent may be named you");
    }
}
