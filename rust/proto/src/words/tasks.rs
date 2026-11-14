//! The tasks' words (see [`super`]: the rules, how to read them): a
//! task's `<status> · <what>` line (tasks-ux m_64508, m_64563: never
//! blank, the status word always first), read the same by the desktop's
//! right bar, Board and Task tab and by `sb task list` (architect m_64809:
//! one text source; the pick is `rows::WorkRow::what`, pinned on both
//! sides by fixtures/work_what.jsonl). Times (`14:20`, `yesterday 14:20`,
//! `tomorrow 07:30`) stay Rust/TS fns, filled in `at`.

crate::words! { tasks:
    // ---- a status alone (no fact to add)
    /// his open cards or page questions carry it
    status_needs_you = "needs you";
    /// one of its agents is in a turn
    status_working = "working";
    /// on something not him
    status_waiting = "waiting";
    /// nothing moves, or an agent of it crashed
    status_stalled = "stalled";
    /// its agents are done, a land or a look is left to him
    status_to_review = "to review";
    /// done, by his word
    status_done = "done";
    /// never done
    status_ongoing = "ongoing";
    /// a status a newer hub knows and this window doesn't: drawn as quiet
    status_unknown = "quiet";

    // ---- a status and what it is about
    /// how many of his questions wait on it
    needs_questions(n) = one "needs you · {n} question" | "needs you · {n} questions";
    /// the agent in a turn of it
    working_who(who) = "working · {who}";
    /// it waits for an agent's reply
    waiting_reply(who) = "waiting · {who}'s reply";
    /// it waits for a timer (`at`: its time)
    waiting_timer(at) = "waiting · wakes at {at}";
    /// it waits for an agent's watch (a build, a pid)
    waiting_watch(who) = "waiting · {who}'s watch";
    /// nothing named: since its last fact
    waiting_quiet(at) = "waiting · quiet since {at}";
    /// an agent of it crashed, and when
    stalled_crash(who, at) = "stalled · {who} stopped at {at}";
    /// nothing moves for 2 h, since when
    stalled_quiet(at) = "stalled · quiet since {at}";
    /// paused until a time, when it resumes by itself
    paused_until(at) = "paused · until {at}";
    /// paused with no end: he resumes it
    paused_by_you = "paused · by you";
    /// done, and when (`today`, `yesterday`, `3 days ago`)
    done_when(when) = "done · {when}";

    // ---- how long ago a task was done
    /// done less than a day ago
    today = "today";
    /// done a day ago
    yesterday = "yesterday";
    /// 2 days or more
    days_ago(n) = one "{n} day ago" | "{n} days ago";

    // ---- the right bar's Tasks section (brief: under Agents, above Artifacts)
    /// the section's name
    section = "Tasks";
    /// the head's loud part: how many tasks wait on him
    head_needs(n) = one "{n} needs you" | "{n} need you";
    /// no open task in the scope
    none_open = "No open task";
    /// a row's done-when count, ticked of all
    goals_of(done, all) = "{done}/{all}";
    /// a row's done-when count's tip
    goals_tip(done, all) = "done when: {done} of {all} ticked";
    /// the fold at the section's end: never-done tasks and the ones done this week
    fold_rest(ongoing, done) = "{ongoing} ongoing · {done} done lately";
    /// a chip's tip: the task's name and its line, then what the click does
    chip_tip(name, what) = "{name} · {what} · open the task";
    /// the `+N` after a chip: its tip lists the other tasks
    chips_more(n) = "+{n}";

    // ---- the Board (a panel tab: the Tasks section's 'all ›')
    /// the Board tab's title
    board_title = "Tasks";
    /// a column's head: his questions wait there
    col_needs_you = "Needs you";
    /// a column's head
    col_working = "Working";
    /// a column's head
    col_waiting = "Waiting";
    /// a column's head
    col_stalled = "Stalled";
    /// a column's head
    col_to_review = "To review";
    /// a column's head: done in the last 7 days
    col_done = "Done this week";
    /// a column's head
    col_paused = "Paused";
    /// a column's head: never done
    col_ongoing = "Ongoing";
    /// a status a newer hub knows: its own column at the end
    col_unknown = "Other";
    /// an empty column's strip tip
    col_empty(col) = "{col}: no task";
    /// the Board's top line, its start: what was done since he last opened it
    since_done(n) = "since you looked: {n} done";
    /// the since line's pink part: what waits on him now
    since_needs(n) = one "{n} needs you" | "{n} need you";
    /// the since line's end: what moved since his look
    since_moving(n) = "{n} moving";
    /// the Board with no task at all
    board_empty = "No task yet. main makes one when you ask for something that takes more than a turn.";

    // ---- the Task tab
    /// its meta line: the project, its agents, when it started
    tab_meta(project, agents, when) = "{project} · {agents} · started {when}";
    /// its meta line's agents part
    tab_agents(n) = one "{n} agent" | "{n} agents";
    /// the quoted origin's tip
    origin_tip = "your message that started it · open it in the thread";
    /// the origin line when its words are not here
    origin_open = "your message that started it";
    /// a section head
    sec_done_when = "Done when";
    /// a section head
    sec_needs = "Needs you";
    /// a section head
    sec_now = "Now";
    /// a section head
    sec_agents = "Agents";
    /// an agent of it in a turn
    agent_live = "in a turn";
    /// an agent of it, idle
    agent_idle = "idle";
    /// an agent of it, archived: its thread stays readable
    agent_archived = "archived";
    /// a goal ticked by an agent's report
    goal_by(by) = "ticked by {by}";
    /// the footer's buttons
    act_done = "Mark done";
    /// the footer's buttons
    act_pause = "Pause";
    /// the footer's buttons
    act_resume = "Resume";
    /// a stalled task's button
    act_restart = "Restart";
    /// a done task's button
    act_reopen = "Reopen";
    /// the footer's ⋯: its tip
    act_more = "More: Rename, Merge into…";
    /// the ⋯ menu: rename the task (his name: an agent never renames it after)
    act_rename = "Rename";
    /// the merge picker's head
    merge_head = "Merge into";
    /// the merge picker with no other open task
    merge_none = "No other open task";
    /// the footer's '+ Agent' (tasks-ux m_64875)
    act_agent = "+ Agent";
    /// its tip
    agent_tip = "ask bise for one more agent on this task";
    /// its field's placeholder, in place of the footer
    agent_ask = "What should it do?";
    /// his message's lead for main: only for the human reading the thread
    /// (architect m_64887: nothing keys on it; main reads the task from its turn)
    agent_lead(name, words) = "new agent on {name}: {words}";
    /// the line under Agents until the new agent's row comes
    agent_asked = "asked bise for an agent · just now";
    /// the task gone (merged, folded, unknown here)
    tab_gone = "This task is not here anymore.";
    /// a merged task's tab: where it went
    tab_merged(name) = "Merged into {name}.";
}
