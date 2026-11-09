//! Tool rows (G3): the TUI's rule, the fold's too (moved out of lines.rs
//! unchanged, to keep it under 1000 lines).

use super::{Obs, Rec, ToolViewRef};

/// What a line does to a thread's tool rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolMove {
    /// `tool_started`: a running row of its own; a row exists from here
    Start(u32),
    /// `tool #<id> <name> : <args>`: its name and args (as on the wire)
    /// go on the newest row with that id; none: nothing
    Info { id: u32, name: String, args: String },
    /// `tool_intent`: on the newest row with that id
    Intent { id: u32, text: String },
    /// `tool_account`: the account it runs as, on the newest row with
    /// that id
    Account { id: u32, label: String },
    /// `tool_code`: on the newest row with that id
    Code { id: u32, code: String },
    /// `tool_result`: its output on the newest row with that id
    Result { id: u32, ok: bool, preview: String },
    /// `tool_finished`: the running row with that id ends, ok or not;
    /// none running: an ended row of its own
    Finish { id: u32, ok: bool },
    /// `tool_view`: a view on the newest row with that id; none: nothing
    View { id: u32, view: ToolViewRef },
}

/// The tool move of a line, if any.
///
/// Outside the wire rule on purpose (lib.rs; architect m_14688): since
/// P4d0 a call line with no `tool_started` before it makes no tool row, in
/// the hub's fold as in the TUI. That drops a phantom row ('0.0s', never
/// run) for every reader alike: a fix, not a field changing meaning, so
/// tests/released.rs's fold law covers message lines only.
pub fn tool_move(rec: &Rec) -> Option<ToolMove> {
    Some(match rec.clone() {
        Rec::Obs(Obs::ToolStarted(id)) => ToolMove::Start(id),
        Rec::Obs(Obs::ToolFinished { id, ok }) => ToolMove::Finish { id, ok },
        Rec::Tool { id, name, args } => ToolMove::Info { id, name, args },
        Rec::ToolIntent { id, text } => ToolMove::Intent { id, text },
        Rec::ToolAccount { id, label } => ToolMove::Account { id, label },
        Rec::ToolCode { id, code } => ToolMove::Code { id, code },
        Rec::ToolResult { id, ok, preview } => ToolMove::Result { id, ok, preview },
        Rec::ToolView { id, view } => ToolMove::View { id, view },
        _ => return None,
    })
}
