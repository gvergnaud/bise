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
    /// a decision of its agents' newest reports
    ctx_decided(text) = "Decided: {text}";
    /// the task is paused by him
    ctx_paused = "Paused by the user: its messages wait until he resumes it.";
    /// his words that started it, quoted
    ctx_said(text) = "He asked: “{text}”";
    /// the head of what waits on him now
    ctx_waits = "Waiting on the user now (never ask him again):";
    /// one of its open questions or picks: its kind and words
    ctx_wait(kind, text) = "- {kind}: {text}";
    /// the count of its items waiting on his pages
    ctx_page_waits(n) = "- on his pages, items waiting: {n}";
    /// the head of its agents
    ctx_agents = "Its agents:";
    /// one of its agents and what it says it does now
    ctx_agent(who, note) = "- {who}: {note}";
    /// one of its agents that says nothing now
    ctx_agent_quiet(who) = "- {who}";
    /// the last line: where the rest is
    ctx_more(id) = "More: sb task show {id} --log";
}
