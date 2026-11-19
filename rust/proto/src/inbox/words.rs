//! The history's words (architect m_24257): one place for what the
//! terminal and the desktop say on a row, pure. The caller passes the
//! UTC offset (seconds east) and its now: this crate has no clock. The
//! words themselves are the catalog's ([`crate::words::inbox`]); the
//! choices (which word for which state) are here.

use super::{InboxRecord, InboxState, Where};
use crate::thread::when;
use crate::words::inbox;

/// `in the inbox`, `on a page`, `in perf's thread`.
pub fn where_words(w: &Where) -> String {
    match w {
        Where::Inbox | Where::Unknown => inbox::where_inbox.into(),
        Where::Thread { agent } => inbox::where_thread(agent),
        Where::Page { .. } => inbox::where_page.into(),
    }
}

/// The row's second line: `you: <his words> · in the inbox`, or
/// `withdrawn: <why>`.
pub fn second_line(r: &InboxRecord) -> String {
    match r.state {
        InboxState::Answered => {
            let said = r.said.as_deref().map(one_line).unwrap_or_default();
            match &r.at {
                Some(w) => inbox::answered_at(said, where_words(w)),
                None => inbox::answered(said),
            }
        }
        _ => inbox::withdrawn(reason(r.reason.as_deref().unwrap_or(""))),
    }
}

/// The hub's resolution as the row says it: `withdrawn by main: <why>`
/// says `<why>`, a stopped agent `its agent stopped`, a close without him
/// `you closed it`.
pub fn reason(res: &str) -> String {
    let res = res.trim();
    if let Some(why) = res.strip_prefix("withdrawn by main:").or_else(|| res.strip_prefix("withdrawn:")) {
        return why.trim().to_string();
    }
    match res {
        "agent stopped" => inbox::reason_agent_stopped.into(),
        "agent archived" => inbox::reason_agent_archived.into(),
        "closed" => inbox::reason_closed.into(),
        "interrupted" => inbox::reason_interrupted.into(),
        "" | "closed by main" => inbox::reason_closed_by_main.into(),
        r => r.strip_prefix("main: ").unwrap_or(r).to_string(),
    }
}

/// A day's head: `today`, `yesterday`, `oct 12`, `dec 30 2024`.
pub fn day_head(ms: u64, now: u64, off: i32) -> String {
    let (y, m, d, ..) = when::civil(ms, off);
    let (ny, nm, nd, ..) = when::civil(now, off);
    let (yy, ym, yd, ..) = when::civil(now.saturating_sub(86_400_000), off);
    if (y, m, d) == (ny, nm, nd) {
        inbox::day_today.into()
    } else if (y, m, d) == (yy, ym, yd) {
        inbox::day_yesterday.into()
    } else {
        let t = when::day_and_time(ms, off, now, off);
        // `sep 28 18:02` -> `sep 28`; `dec 30 2024 18:02` -> `dec 30 2024`
        t.rsplit_once(' ').map(|(day, _)| day.to_string()).unwrap_or(t)
    }
}

/// The same local day.
pub fn same_day(a: u64, b: u64, off: i32) -> bool {
    let (x, y) = (when::civil(a, off), when::civil(b, off));
    (x.0, x.1, x.2) == (y.0, y.1, y.2)
}

/// `14:02`, local.
pub fn hour(ms: u64, off: i32) -> String {
    let (.., h, mi) = when::civil(ms, off);
    format!("{h:02}:{mi:02}")
}

/// The words a search looks for: lowercased, split on spaces.
pub fn terms(q: &str) -> Vec<String> {
    q.split_whitespace().map(str::to_lowercase).collect()
}

/// Every term in the question, his words, the asker or the reason.
pub fn matches(r: &InboxRecord, terms: &[String]) -> bool {
    if terms.is_empty() {
        return true;
    }
    let hay = [r.question.as_str(), r.said.as_deref().unwrap_or(""), r.asker.as_str(), r.reason.as_deref().unwrap_or("")].join("\n").to_lowercase();
    terms.iter().all(|t| hay.contains(t.as_str()))
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
