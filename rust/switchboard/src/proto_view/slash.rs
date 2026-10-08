//! A typed `slash` line (architect m_10724, m_10789, m_10798): what the
//! hub does with the TUI router's parse of it. Every notice sb-core gives
//! a step is a refusal (the audit in the sha), so a stepped command's
//! notices come back as its `error`; the hub-side commands that answer
//! with text on success are answered here with a typed event instead
//! (`agents`, `prs`, a `notice`), never an error.

use crate::flow::FlowMode;
use bise_proto::slash::{Artifacts, ARTIFACTS_ADD};
use crate::router::UserCmd;

/// What a slash line does.
#[derive(Debug)]
pub enum Slash {
    /// an `error` with these words (the router's usage, or plain text)
    Refuse(String),
    /// `/tasks`: that connection's `agents` rows
    Agents,
    /// `/prs`: that connection's `prs` event
    Prs,
    /// `/help`: a `notice` with the TUI's help (the window has its own
    /// command list; it never needs to send it)
    Help,
    /// `/flow [pr|trunk]`: the repo's flow, shown or set by the TUI's
    /// writer; its words as a `notice`, a refusal as an `error`
    Flow(Option<FlowMode>),
    /// `/artifacts`: that connection's `artifacts` event (the window has
    /// its own screen for it); `/artifacts add <path or link>`: the
    /// hub's add, the one the TUI's client asks with its `artifacts` op
    /// (the TUI runs `/artifacts` itself: the router never sees it)
    Artifacts(Option<String>),
    /// `/stop <agent>`: the `stop` command's path (its turn stops when it
    /// works; idle, nothing to stop). The computer-use stop the TUI also
    /// fires waits for issue 18's ctl ops (architect m_13028).
    Stop(String),
    /// `/version …`, `/restart …`, `/update`: the daemon's `version` op,
    /// its words as a `notice` (his authority: client socket only)
    Version(bise_proto::slash::Version),
    /// `/approvals [yolo|auto]`: the `approvals` command's path
    Approvals(Option<bise_proto::rows::ApprovalMode>),
    /// everything else: one step of the TUI's own handler
    /// (`Input::UserCmd`), its refusals as `error`s with the cid
    Step(UserCmd),
}

/// The TUI router's parse of a slash line, as what the hub does.
pub fn route(cmd: UserCmd) -> Slash {
    match cmd {
        UserCmd::Invalid(e) => Slash::Refuse(e),
        UserCmd::Say(_) => Slash::Refuse("not a command: send it as a message".into()),
        UserCmd::Tasks => Slash::Agents,
        UserCmd::Prs => Slash::Prs,
        UserCmd::Help => Slash::Help,
        UserCmd::Flow { set } => Slash::Flow(set),
        UserCmd::Stop { name } => Slash::Stop(name),
        UserCmd::Version(v) => Slash::Version(v),
        UserCmd::Approvals { mode } => Slash::Approvals(mode),
        UserCmd::Passthrough(l) => match bise_proto::slash::artifacts(&l) {
            Some(Artifacts::List) => Slash::Artifacts(None),
            Some(Artifacts::Add(target)) => Slash::Artifacts(Some(target)),
            Some(Artifacts::Usage) => Slash::Refuse(ARTIFACTS_ADD.into()),
            None => Slash::Step(UserCmd::Passthrough(l)),
        },
        cmd => Slash::Step(cmd),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::parse;

    fn r(line: &str) -> Slash {
        route(parse(line, "perf"))
    }

    /// Law (architect m_10798): a refusal is an `error`, never a notice;
    /// the hub-side successes that answer with text are typed answers,
    /// never errors; the rest steps the TUI's own handler.
    #[test]
    fn a_slash_line_is_refused_answered_or_stepped() {
        assert!(matches!(r("/rename"), Slash::Refuse(e) if e.starts_with("usage: /rename")));
        assert!(matches!(route(parse("/rename perf", "main")), Slash::Refuse(e) if e.starts_with("usage: /rename")));
        assert!(matches!(r("/rename perf2"), Slash::Step(UserCmd::Rename { .. })));
        // main's core_num (close-panic): the refusal names the bad value, then the usage
        assert!(matches!(r("/answer x"), Slash::Refuse(e) if e.starts_with("not a card number: x") && e.contains("usage: /answer")));
        assert!(matches!(r("hello there"), Slash::Refuse(e) if e.starts_with("not a command")));
        assert!(matches!(r("@ "), Slash::Refuse(_)));
        assert!(matches!(r("/tasks"), Slash::Agents));
        assert!(matches!(r("/prs"), Slash::Prs));
        assert!(matches!(r("/help"), Slash::Help));
        assert!(matches!(r("/flow"), Slash::Flow(None)));
        assert!(matches!(r("/flow pr"), Slash::Flow(Some(_))));
        assert!(matches!(r("/rename perf perf2"), Slash::Step(UserCmd::Rename { .. })));
        assert!(matches!(r("/answer 3 v2"), Slash::Step(UserCmd::Answer { card: 3, .. })));
        assert!(matches!(r("/close 3"), Slash::Step(UserCmd::Close { card: 3 })));
        assert!(matches!(r("/compact"), Slash::Step(UserCmd::Passthrough(_))));
        assert!(matches!(r("@docs hi"), Slash::Step(UserCmd::To { .. })));
        assert!(matches!(r("/artifacts"), Slash::Artifacts(None)));
        assert!(matches!(r("/artifacts add https://x.dev/a b"), Slash::Artifacts(Some(t)) if t == "https://x.dev/a b"));
        assert!(matches!(r("/artifacts add  "), Slash::Refuse(e) if e == ARTIFACTS_ADD));
        assert!(matches!(r("/artifacts addx"), Slash::Artifacts(None)));
        assert!(matches!(r("/bogus"), Slash::Step(UserCmd::Passthrough(_))));
        // R1/R6 (ambient-lead m_12190): the TUI's own commands the window
        // types run on the hub, never 'unknown command'
        assert!(matches!(r("/stop docs"), Slash::Stop(n) if n == "docs"));
        assert!(matches!(r("/stop"), Slash::Refuse(e) if e == bise_proto::slash::STOP_USAGE));
        assert!(matches!(r("/version"), Slash::Version(bise_proto::slash::Version::List)));
        assert!(matches!(r("/restart"), Slash::Version(bise_proto::slash::Version::Restart(_))));
        assert!(matches!(r("/update"), Slash::Version(bise_proto::slash::Version::Update)));
        assert!(matches!(r("/approvals"), Slash::Approvals(None)));
        assert!(matches!(r("/approvals yolo"), Slash::Approvals(Some(bise_proto::rows::ApprovalMode::Yolo))));
        assert!(matches!(r("/approvals x"), Slash::Refuse(e) if e == "/approvals x: yolo or auto"));
    }
}
