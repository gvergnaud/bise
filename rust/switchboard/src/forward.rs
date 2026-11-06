//! msg-forward (architect m_41072): the shell's half of `sb send/spawn
//! --forward m_<n>` and of `sb show m_<n>`. sb-core decides the forward
//! (bend/hub/forward.bend: the door, the text, the parts, `Msg.fwd`);
//! here, only what the shell knows:
//!
//! - [`keys`]: the request's `fwd` and `fwd_at` for sb-core, the
//!   original's time formatted from the same id ("" when the mirror lacks
//!   it: the header leaves the date out, sb-core's door still refuses an
//!   unknown id);
//! - [`msg_pos`]: where message n sits in its recipient's thread, from
//!   the transcript's lines as a stream (never the whole file in memory).

use crate::model::State;
use bise_proto::thread::lines::{read, Hub, Rec};
use serde_json::{json, Value};

/// `fwd` and `fwd_at` on sb-core's `send` / `spawn` input; none: unchanged
/// (an older core reads the same line as before).
pub fn keys(st: &State, q: &mut Value, fwd: Option<u64>) {
    let Some(n) = fwd else { return };
    q["fwd"] = json!(n);
    q["fwd_at"] = json!(st.peek(n).map(|m| crate::search::fmt_utc(m.created_ms)).unwrap_or_default());
}

/// The position (from 1, as `transcript.rs`) of message n's entry in a
/// transcript: its `msg-in` line, or his `you` line (the one before its
/// `you-id`). Lines are `<ms>\t<line>`.
pub fn msg_pos<I: Iterator<Item = String>>(lines: I, n: u64) -> Option<usize> {
    let id = format!("m_{n}");
    let mut prev_you: Option<usize> = None;
    for (i, l) in lines.enumerate() {
        let pos = i + 1;
        let line = l.split_once('\t').map_or(l.as_str(), |x| x.1);
        match read(line) {
            Rec::Hub(Hub::MsgIn { id: ref got, .. }) if *got == id => return Some(pos),
            Rec::Hub(Hub::YouId(got)) if got == n => return Some(prev_you.unwrap_or(pos)),
            Rec::Hub(Hub::You(..)) => prev_you = Some(pos),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "forward_tests.rs"]
mod tests;
