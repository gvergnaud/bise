//! The runtime's observations as typed records (issue 38, sha 2a): a new
//! line is `obs <kind> :: <json>` ([`ObsRec::line`]), read back with serde
//! ([`ObsRec::read_rest`]); the older text line (`  obs: <kind>: <text>`)
//! is read in one place, `thread/legacy.rs`. Like [`crate::core::Feed`]: two
//! colons make the new form one prefix test, transcripts are never
//! rewritten, so both forms stay readable.
//!
//! The reader ships one release before the writers (issue 25's rule,
//! architect m_23413): until sha 2b the runtime still writes the text form
//! and the generated Bend encoders (`bend/core/gen/obs.bend`) are compiled
//! but not called. DRAFT until then: a name here may still change; from
//! 2b on the wire rule binds.
//!
//! A record's JSON keys are sorted (serde_json's map, as [`ObsRec::line`]
//! and `Feed::line` write them); the generated Bend encoders write them in
//! that order too, and bend/proofs/obs.bend proves each one equal to its
//! line of `fixtures/obs_lines.jsonl`. 2b's runtime writers keep that order.
//!
//! One record per kind the runtime writes (bend/core/obs.bend `show_obs`
//! and the hand writers of bend/runtime); the view the fold and the TUI
//! read stays [`crate::thread::lines::Obs`] ([`ObsRec::view`]).

use crate::core::core_kinds;
use crate::thread::lines::{Obs, TurnEnd};
use serde_json::Value;

core_kinds! {
    /// One observation of the runtime.
    pub enum ObsRec, tag "obs" {
        TurnStarted = "turn_started" => OTurnStarted {},
        /// the reply's text as on the wire, its `<think>` parts in it; ""
        /// for a tool-call-only reply
        Assistant = "assistant" => OAssistant { text: Str },
        ToolStarted = "tool_started" => OToolStarted { call: Nat },
        ToolFinished = "tool_finished" => OToolFinished { call: Nat, ok: Bool },
        /// plumbing nobody shows
        ToolCommitted = "tool_result_committed" => OToolCommitted { call: Nat },
        SteerReceived = "steering_received" => OSteerReceived { text: Str },
        SteerRejected = "steering_rejected" => OSteerRejected { reason: Str },
        Steered = "steered" => OSteered { text: Str },
        NotifReceived = "notification_received" => ONotifReceived { text: Str },
        NotifDelivered = "notification_delivered" => ONotifDelivered { text: Str },
        /// a provider call retried: attempt `n` of `of`, its cause, the wait
        ProviderRetry = "provider_retry" => OProviderRetry { n: Nat, of: Nat, why: Str, delay_ms: Nat },
        /// a provider call waits for the network (no name, no connection):
        /// no attempt spent; `waited_ms` so far in this outage, the next
        /// pause, its cause. Written when a wait starts and every 5 min
        /// (bend/runtime/provider-pure.bend wait_says), not at each pause.
        NetworkWait = "network_wait" => ONetworkWait { waited_ms: Nat, delay_ms: Nat, why: Str },
        /// the parent respawned the process after a crash (its note)
        HarnessRestarted = "harness_restarted" => OHarnessRestarted { note: Str },
        CandidateDiscarded = "candidate_discarded" => OCandidateDiscarded { cause: Str },
        /// `compaction` its number, `auto` or manual
        CompactionStarted = "compaction_started" => OCompactionStarted { compaction: Nat, auto: Bool },
        CompactionFailed = "context_compaction_failed" => OCompactionFailed { code: Str },
        /// the summary's preview (one line, 40 chars and `...`)
        CompactionDone = "compaction_done" => OCompactionDone { summary: Str },
        /// a resumed session: its message count
        SessionRestored = "session_restored" => OSessionRestored { messages: Nat },
        /// a plan's swap, once per session: the id asked, the one run
        ModelSwapped = "model_swapped" => OModelSwapped { from: Str, to: Str },
        /// the last model call's tokens: `input` every one it read (cached
        /// ones included), `output` the ones it wrote
        Usage = "usage" => OUsage { model: Str, input: Nat, output: Nat, cache_read: Nat, cache_write: Nat },
        NullIteration = "null_iteration" => ONullIteration {},
        /// how the turn ended: `end` completed|interrupted|failed; `why`
        /// the failure, or who interrupted it
        TurnDone = "turn_done" => OTurnDone { end: Str, why: Str },
        TurnStalled = "turn_stalled" => OTurnStalled { why: Str },
        /// /reload's words (`refused`: not between turns)
        Reload = "reload" => OReload { refused: Bool, text: Str },
    }
}

/// Between a new line's kind and its JSON (an old line has `: `).
pub const SEP: &str = " :: ";

/// A new line's prefix (an old one is `  obs: `).
pub const PREFIX: &str = "obs ";

impl ObsRec {
    /// Its transcript line: `obs <kind> :: <json without the tag>`.
    pub fn line(&self) -> String {
        let mut v = self.to_value();
        if let Value::Object(m) = &mut v {
            m.remove("obs");
        }
        format!("{PREFIX}{}{SEP}{}", self.tag(), v)
    }

    /// A line after its `obs `, read when it is the new form
    /// (`<kind> :: <json>`): its kind and the record (an error when the
    /// JSON doesn't fit the kind: a bug of the writer); None for anything
    /// else.
    pub fn read_rest(rest: &str) -> Option<(&str, Result<ObsRec, String>)> {
        let (kind, json) = rest.split_once(SEP)?;
        if kind.is_empty() || kind.contains(' ') {
            return None;
        }
        Some((kind, ObsRec::read(kind, json)))
    }

    /// A kind and its JSON (without the tag) read back.
    pub fn read(kind: &str, json: &str) -> Result<ObsRec, String> {
        let mut m = match serde_json::from_str::<Value>(json) {
            Ok(Value::Object(m)) => m,
            Ok(_) => return Err(format!("obs {kind}: not a JSON object")),
            Err(e) => return Err(format!("obs {kind}: {e}")),
        };
        m.insert("obs".into(), Value::String(kind.to_string()));
        ObsRec::decode(Value::Object(m))
    }

    /// The record as the view [`Obs`] (what the fold, the hub and the
    /// TUI match on), the same as the text form's.
    pub fn view(self) -> Obs {
        match self {
            ObsRec::TurnStarted(_) => Obs::TurnStarted,
            ObsRec::Assistant(r) => Obs::Assistant(r.text),
            ObsRec::ToolStarted(r) => u32::try_from(r.call).map(Obs::ToolStarted).unwrap_or(Obs::Plumbing),
            ObsRec::ToolFinished(r) => u32::try_from(r.call).map(|id| Obs::ToolFinished { id, ok: r.ok }).unwrap_or(Obs::Plumbing),
            ObsRec::ToolCommitted(_) => Obs::Plumbing,
            ObsRec::SteerReceived(r) => Obs::SteeringReceived(r.text),
            ObsRec::SteerRejected(r) => Obs::Other(format!("steering_rejected: {}", r.reason)),
            ObsRec::Steered(r) => Obs::Steered(r.text),
            ObsRec::NotifReceived(r) => Obs::NotificationReceived(r.text),
            ObsRec::NotifDelivered(r) => Obs::NotificationDelivered(r.text),
            ObsRec::ProviderRetry(r) => Obs::ProviderRetry(format!("{}/{} · {} · retry in {}s", r.n, r.of, r.why, r.delay_ms / 1000)),
            ObsRec::NetworkWait(r) => Obs::NetworkWait { waited_ms: r.waited_ms, delay_ms: r.delay_ms, why: r.why },
            ObsRec::HarnessRestarted(r) => Obs::HarnessRestarted(r.note),
            ObsRec::CandidateDiscarded(r) => Obs::CandidateDiscarded(r.cause),
            ObsRec::CompactionStarted(_) => Obs::CompactionStarted,
            ObsRec::CompactionFailed(r) => Obs::CompactionFailed(r.code),
            ObsRec::CompactionDone(r) => Obs::CompactionDone(r.summary),
            ObsRec::SessionRestored(r) => Obs::SessionRestored(format!("{} messages", r.messages)),
            ObsRec::ModelSwapped(r) => Obs::ModelSwapped { from: r.from, to: r.to },
            ObsRec::Usage(r) => Obs::Usage(format!(
                "model={} in={} out={} cache_read={} cache_write={}",
                r.model, r.input, r.output, r.cache_read, r.cache_write
            )),
            ObsRec::NullIteration(_) => Obs::NullIteration,
            ObsRec::TurnDone(r) => Obs::TurnDone(match r.end.as_str() {
                "completed" => TurnEnd::Completed,
                "interrupted" => TurnEnd::Interrupted { by: Some(r.why).filter(|w| !w.is_empty()) },
                "failed" => TurnEnd::Failed(r.why),
                _ => TurnEnd::Other(r.end),
            }),
            ObsRec::TurnStalled(r) => Obs::TurnStalled(r.why),
            ObsRec::Reload(r) => Obs::Other(format!("reload{} {}", if r.refused { " refused:" } else { ":" }, r.text)),
            ObsRec::Unknown { tag, raw } => Obs::Other(format!("{tag}: {raw}")),
        }
    }
}

#[cfg(test)]
mod tests;
