//! Small pure helpers of core.rs's handlers (moved out of core.rs to keep
//! it under its line ratchet, architect m_27675): the direct-exchange
//! note, the `sb` commands that need git, a message id's and a JSON
//! field's reading. No hub state: arguments in, words out.

use super::AgentReq;
use crate::util::{clip_tail, one_line};
use serde_json::Value;

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

/// RFC 0001 §7.4: what main (`note`) and the user's own feed (`direct`)
/// read after the user talked directly to `task`: the messages sent,
/// and the end of the task's reply since.
pub(super) fn direct_exchange(task: &str, sent: &[String], reply: &str) -> (String, String) {
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

/// The `sb` commands that need git: off in a workspace without it (the
/// home workspace, docs/ambient-pages.md §5.1). Their name, else None.
pub(super) fn git_only(req: &AgentReq) -> Option<&'static str> {
    match req {
        AgentReq::Land { .. } => Some("land"),
        AgentReq::Feature { .. } => Some("feature"),
        AgentReq::Flow { .. } => Some("flow"),
        _ => None,
    }
}

/// sb-core's `force_run` input: an agent's REPL state (a restarted
/// sb-core gets it bare, without the step's envelope).
pub(super) fn force_run_input(agent: &str, run: crate::model::Run) -> serde_json::Value {
    use crate::model::Run;
    use bise_proto::core::{input as ci, In};
    let run = match run {
        Run::Down => "down",
        Run::Starting => "starting",
        Run::Idle => "idle",
        Run::Busy => "busy",
    };
    In::ForceRun(ci::ForceRun { agent: agent.into(), run: run.into() }).to_value()
}

/// `sb flow [pr|trunk]`'s `set`: "" asks, else devflow's mode (its one
/// parser).
pub(super) fn flow_set(set: &str) -> Result<Option<crate::flow::FlowMode>, String> {
    match set.trim() {
        "" => Ok(None),
        m => crate::devflow::parse_mode(m).map(Some).map_err(|_| "usage: sb flow [pr|trunk]".to_string()),
    }
}

/// `sb worktree <path>|none`: "" or "none" is none, else an absolute
/// path, its trailing `/` taken off.
pub(super) fn worktree_path(path: &str) -> Result<String, String> {
    match path.trim() {
        "" | "none" => Ok(String::new()),
        p if p.starts_with('/') => Ok(p.trim_end_matches('/').to_string()),
        p => Err(format!("sb worktree: an absolute path or none, not {}", p)),
    }
}

pub(super) fn jstr(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// The one line `sb <cmd>` answers in a workspace without git.
pub(crate) fn not_in_a_repo(cmd: &str) -> String {
    format!("sb {}: not in a repo (this workspace has no git): files stay as you saved them, nothing to land", cmd)
}

/// Issue 37 #2: an agent request core.rs turns into sb-core's `req` step
/// with nothing done before it (no reply, effect or state change), so
/// sb-core checks its sender in that step. Anything else (core.rs answers
/// it itself, or acts first) takes the `speaks` step before; a variant
/// not listed here does too: forgetting one costs a step, never a check.
pub(super) fn rides_req(req: &AgentReq) -> bool {
    match req {
        AgentReq::Send { switch, .. } => switch.is_none(),
        AgentReq::Spawn { ask, feature, .. } => ask.is_empty() && feature.is_empty(),
        AgentReq::Wait { .. }
        | AgentReq::Ask { .. }
        | AgentReq::Status { .. }
        | AgentReq::Report { .. }
        | AgentReq::Follow { .. }
        | AgentReq::Move { .. }
        | AgentReq::Stop { .. }
        | AgentReq::Card { .. }
        | AgentReq::Close { .. }
        | AgentReq::Withdraw { .. }
        | AgentReq::Rename { .. }
        | AgentReq::Restore { .. }
        | AgentReq::Isolate { .. }
        | AgentReq::ProjectSend { .. }
        | AgentReq::ProjectAsk { .. } => true,
        _ => false,
    }
}
