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
}
