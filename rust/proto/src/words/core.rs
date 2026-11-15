//! sb-core's words (see [`super`]; architect m_65509): what the hub
//! writes into an agent's prompt itself, generated into
//! `bend/hub/gen/words.bend` (rust/proto/tests/core_gen.rs: one def per
//! word, its holes as String arguments) so sb-core never spells English
//! inline. sb-core's older inline words (GO_ON, the pause notes,
//! TASKS_OFF, task_err's, `[report: `) move here one at a time. No plural
//! form here: the generator refuses one.

crate::words! { core:
    // ---- a task's context for an agent that joins it (roma idea 5)
    /// its first line: the task's id and name
    ctx_head(id, name) = "## Task `{id}`: {name}";
    /// under the head: what this block is (the hub's, not a message to answer)
    ctx_intro = "You now serve this task. What it is, from the hub (not a message to answer):";
    /// why he wants it
    ctx_why(why) = "Why: {why}";
    /// the head of its done-when list
    ctx_done_when = "Done when:";
    /// a done-when item ticked
    ctx_goal_done(text) = "- [x] {text}";
    /// a done-when item still open
    ctx_goal_open(text) = "- [ ] {text}";
    /// the head of its agents' newest reports
    ctx_reports = "Its agents' newest reports:";
    /// one agent's newest report: its kind and summary
    ctx_report(who, kind, text) = "- {who} ({kind}): {text}";
    /// a decision of that report, under it
    ctx_decided(text) = "  decided: {text}";
    /// the task is paused by him
    ctx_paused = "Paused by the user: its messages wait until he resumes it.";
}
