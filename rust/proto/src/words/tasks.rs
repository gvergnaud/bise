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
    /// needs you, a card of it on a page: that page (tasks-ux: Waiting is never blank)
    needs_page(page) = "needs you · a pick on page {page}";
    /// to review, its agents' place has an open PR: its number
    review_pr(n) = "to review · PR #{n}";
    /// the agent in a turn of it
    working_who(who) = "working · {who}";
    /// it waits for an agent's reply
    waiting_reply(who) = "waiting · {who}'s reply";
    /// it waits for a timer (`at`: its time)
    waiting_timer(at) = "waiting · wakes at {at}";
    /// it waits for an agent's watch (a build, a pid)
    waiting_watch(who) = "waiting · {who}'s watch";
    /// waiting, the task has no agent yet (tasks-ux m_65131: never a blank Waiting)
    waiting_no_agent = "waiting · no agent yet";
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
    /// no open task in the scope, only ongoing or done ones (its fold line follows)
    none_open = "Nothing in progress";
    /// the right bar's Tasks section when the scope has none at all, done or not ('· Ask bise' follows)
    none_yet = "No tasks yet";
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
    /// the Board's top line, its start; its parts follow, the zero ones left out (tasks-ux m_64959)
    since_head = "since you looked: ";
    /// the since line's part: what was done since he last opened it
    since_done(n) = "{n} done";
    /// the since line when every part is zero
    since_none = "since you looked: nothing new";
    /// the since line's pink part: what waits on him now
    since_needs(n) = one "{n} needs you" | "{n} need you";
    /// the since line's end: the tasks working now
    since_moving(n) = "{n} moving";
    /// the Board with no task at all: its title (designer m_65123, tasks-ux m_65124: 'bise', never 'main' or 'a turn')
    board_empty = "No tasks yet";
    /// its 3 steps: 1
    board_step_ask = "You ask bise for work.";
    /// its 3 steps: 2
    board_step_shows = "It shows up here as a task: its agents, its questions, where it stands.";
    /// its 3 steps: 3
    board_step_moves = "It moves column to column until it's done.";
    /// its one action, and the right bar's: his composer, on main's thread
    ask_bise = "Ask bise";
    /// the Board with tasks only in the past (done more than a week ago): 'No tasks yet' would lie
    board_past = "Nothing in progress";
    /// its detail: where they went
    board_past_more = "Done tasks stay in search (⌘K).";

    // ---- the Task tab
    /// its meta line: the project, its agents, when it started
    tab_meta(project, agents, when) = "{project} · {agents} · started {when}";
    /// its meta line's agents part
    tab_agents(n) = one "{n} agent" | "{n} agents";
    /// its meta line's middle with no agent on it yet (never '0 agents')
    tab_no_agents = "no agent yet";
    /// its Agents section with none yet: how one comes
    agents_none = "No agent on it yet. + Agent asks bise for one.";
    /// the quoted origin's tip
    origin_tip = "your message that started it · open it in the thread";
    /// a section head
    sec_done_when = "Done when";
    /// a section head
    sec_needs = "Needs you";
    /// a section head: the newest report or status note of its agents
    sec_now = "Now";
    /// the paused line under its name (tasks-ux m_64959): since when, its Resume in the footer
    paused_since(ago) = "paused by you · {ago} ago";
    /// a section head
    sec_agents = "Agents";
    /// a section head: the 3 newest artifacts for him that serve it
    sec_artifacts = "Artifacts";
    /// the link under them: the Library filtered on this task (`task:<id>`)
    arts_all(n) = "All {n} in Library →";
    /// a section head: its agents' branches (a PR opens instead when one is open)
    sec_changes = "Changes";
    /// a change row's tip
    change_tip = "open its changes";
    /// an agent of it in a turn
    agent_live = "working";
    /// an agent of it, idle
    agent_idle = "idle";
    /// an agent of it waiting on his answer (tasks-ux m_64967)
    agent_waits = "waits for you";
    /// the typed answer's placeholder on a question with no options
    q_type = "type your answer, ⏎ sends it";
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
    /// the short Undo after his gesture (roma idea 4): its button
    undo = "Undo";
    /// the Undo's line after Pause: the name first, like our other toasts (designer m_65397)
    undo_paused(name) = "{name} paused";
    /// the Undo's line after Resume
    undo_resumed(name) = "{name} resumed";
    /// the Undo's line after Mark done
    undo_done(name) = "{name} marked done";
    /// the ink dot's label and tip (roma idea 2): its status, artifacts or questions changed
    new_since_looked = "new since you looked";

    // ---- the task's Log (roma idea 3): who did what and why, newest first
    /// the Task tab's first pane: what the task is now
    tab_overview = "Overview";
    /// the Task tab's second pane: its Log
    tab_log = "Log";
    /// the Log with no line yet
    log_none = "Nothing yet. Its agents' reports, your answers and what lands show up here.";
    /// the Log while the hub reads it
    log_loading = "reading its Log…";
    /// the Log the hub couldn't read
    log_failed(why) = "its Log can't be read: {why}";
    /// an agent's `sb report` (kind: progress, done, blocked, failed)
    log_report(who, kind, text) = "{who} reported {kind}: {text}";
    /// a `--decision` of that report
    log_decision(text) = "decided: {text}";
    /// his answer to one of its questions
    log_answer(text) = "you answered: {text}";
    /// a land of its agents: its commits, onto which ref, its tip
    log_land(who, n, target, sha) = one "{who} landed {n} commit on {target} ({sha})" | "{who} landed {n} commits on {target} ({sha})";
    /// an agent's first turn on it
    log_join(who) = "{who} joined";
    /// his gesture: Pause
    log_paused = "you paused it";
    /// his gesture: Resume
    log_resumed = "you resumed it";
    /// his gesture: Mark done
    log_done = "you marked it done";
    /// his gesture: Reopen
    log_reopened = "you reopened it";
    /// his gesture: Rename, its new name
    log_renamed(name) = "you renamed it {name}";
    /// his gesture: Merge into, its target
    log_merged(into) = "you merged it into {into}";
    /// his gesture: its why set
    log_why(text) = "you set its why: {text}";
    /// his gesture: a done-when item added
    log_goal(text) = "you added done-when: {text}";
    /// his gesture: done-when item n ticked
    log_ticked(n) = "you ticked done-when {n}";
    /// his gesture: done-when item n unticked
    log_unticked(n) = "you unticked done-when {n}";
    /// his gesture: made a task that never ends
    log_ongoing = "you made it ongoing";
    /// he moved an artifact, a page or a card onto it
    log_moved_in(what) = "you moved {what} here";
    /// he moved an artifact, a page or a card off it
    log_moved_out(what) = "you moved {what} away";
    /// a moved card (no title to name it)
    log_a_card = "a question";
    /// one of its agents asked him (a question, a pick, a block)
    log_asked(who, text) = "{who} asked: {text}";
    /// one of its agents made a version of an artifact or a page
    log_made(who, title, v) = "{who} made {title} v{v}";
    /// back-to-back versions of one artifact, folded into one line
    log_made_range(who, title, from, to) = "{who} made {title} v{from}→v{to}";
    /// one of its agents failed in its turn, and why
    log_crashed(who, why) = "{who} stopped (crash): {why}";
    /// one of its agents was stopped in its turn, and why
    log_stopped(who, why) = "{who} was stopped: {why}";
    /// his gesture: Restart of a stalled task
    log_restarted = "you restarted it";
    /// main's Restart (sb task restart), never said as his
    log_restarted_by(who) = "{who} restarted it";
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
    /// that line when main answered instead (it asks him something): a link to its reply
    agent_answered = "bise answered · see its reply →";
    /// the task gone (merged, folded, unknown here)
    tab_gone = "This task is not here anymore.";
    /// a merged task's tab: where it went
    tab_merged(name) = "Merged into {name}.";
    /// the inbox's folded group under 'Waiting for you': the questions of
    /// tasks he paused (they wait until he resumes one)
    inbox_paused = "On paused tasks";
    /// the thread's inbox strip head: the questions of paused tasks, apart (a link to the inbox page)
    block_paused(n) = "{n} on paused tasks";
    /// its fold's tip while folded
    inbox_paused_show = "show the questions of the tasks you paused";
    /// its fold's tip while open
    inbox_paused_hide = "fold them away";
}
