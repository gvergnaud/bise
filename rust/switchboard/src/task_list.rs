//! tasks (docs/tasks.md): `sb task list` and `sb task show`, main's
//! `<open_tasks>`, and the `works` event's rows: the hub's reads of the
//! works sb-core's view gave (`State.works`, `State.tombs`).
//! The status and what it is about are sb-core's (wstatus.bend); this
//! file only puts them in words, never decides one.

use crate::every::{hh_mm, past_label, when_label};
use crate::model::State;
use bise_proto::core::state::WorkKind;
use bise_proto::core::view::ViewWork as VWork;
use bise_proto::rows::{WorkAbout, WorkDone, WorkGoal, WorkRow, WorkStatus, WorkTomb};

/// A work's status and what it is about, one pattern for all: `<status>
/// · <what>` (tasks-ux m_64508: never blank; m_64563: the status word
/// always there, the Board's and the Task tab's pill read the same). The
/// pick is the wire row's (`rows::WorkRow::what`, words::tasks), the one
/// the window reads too (architect m_64809).
pub fn what(w: &VWork, now: u64) -> String {
    row(w).what(now, |t| past_label(t, now), |t| when_label(t, now))
}

/// The wire row of a work (the `works` event): its facts and the status
/// sb-core computed.
pub fn row(w: &VWork) -> WorkRow {
    let x = &w.work;
    WorkRow {
        id: x.id.clone(),
        name: x.name.clone(),
        why: x.why.clone(),
        goals: x.goals.iter().map(|g| WorkGoal { text: g.text.clone(), done: g.done, by: g.by.clone() }).collect(),
        ongoing: x.kind == WorkKind::Ongoing,
        origin: x.origin,
        by: x.by.clone(),
        created_ms: x.created_ms,
        paused_ms: x.paused,
        until_ms: x.until,
        done: x.done.as_ref().map(|f| WorkDone { at_ms: f.at, by: f.by.clone() }),
        merged_into: x.merged_into.clone(),
        named_by_you: x.named_by_you,
        agents: x.agents.clone(),
        last_ms: x.last,
        status: WorkStatus::of_word(&w.status),
        about: WorkAbout::of_word(&w.why),
        who: w.who.clone(),
        at_ms: w.at,
        needs: w.needs,
        cards: w.cards.clone(),
        // joined at the snapshot, which has the places and the cards (core_snapshot::with_reasons)
        pr: None,
        page: None,
    }
}

/// The `works` event's lists: every work sb-core keeps, oldest first, and
/// the folded ones' tombs.
pub fn rows(st: &State) -> (Vec<WorkRow>, Vec<WorkTomb>) {
    let mut ws: Vec<&VWork> = st.works.values().collect();
    ws.sort_by_key(|w| (w.work.created_ms, w.work.id.clone()));
    let tombs = st
        .tombs
        .values()
        .map(|t| WorkTomb { id: t.id.clone(), name: t.name.clone(), done_ms: t.done, merged_into: t.merged_into.clone() })
        .collect();
    (ws.into_iter().map(row).collect(), tombs)
}

/// `today`, `yesterday`, `3 days ago`
fn days_ago(at: u64, now: u64) -> String {
    use bise_proto::words::tasks;
    match now.saturating_sub(at) / (24 * 3_600_000) {
        0 => tasks::today.into(),
        1 => tasks::yesterday.into(),
        n => tasks::days_ago(n),
    }
}

/// The order of `sb task list`, the right bar's (his pick B on page tasks-true): in progress
/// first (needs you, working), then to review (finished, his OK left), then what waits, done last.
fn rank(status: &str) -> u8 {
    match status {
        "needs_you" => 0,
        "working" => 1,
        "to_review" => 2,
        "waiting" => 3,
        "stalled" => 4,
        "paused" => 5,
        "ongoing" => 6,
        _ => 7,
    }
}

/// One line a work: `id · name · what · agents`.
fn line(w: &VWork, now: u64) -> String {
    let agents = if w.work.agents.is_empty() {
        String::new()
    } else {
        format!(" · {}", w.work.agents.join(", "))
    };
    format!(
        "{} · {} · {}{}",
        w.work.id,
        w.work.name,
        what(w, now),
        agents
    )
}

/// The lines of the works whose status `keep` takes, what needs him
/// first; never a merged one.
fn lines(st: &State, keep: impl Fn(&str) -> bool, now: u64) -> Vec<String> {
    let mut ws: Vec<&VWork> = st
        .works
        .values()
        .filter(|w| w.work.merged_into.is_none() && keep(&w.status))
        .collect();
    ws.sort_by_key(|w| (rank(&w.status), w.work.created_ms));
    ws.iter().map(|w| line(w, now)).collect()
}

/// main's `<open_tasks>` (board.rs): the works it may put new work on,
/// the ongoing ones too (a standing order takes work), never a done one.
pub fn open_lines(st: &State, now: u64) -> Vec<String> {
    lines(st, |s| s != "done", now)
}

/// `sb task list`: the open works (`all`: with the done and ongoing ones,
/// and how many were folded).
pub fn list(st: &State, all: bool, now: u64) -> String {
    let mut out = lines(st, |s| all || !matches!(s, "done" | "ongoing"), now);
    if out.is_empty() {
        out.push(if all {
            "no task yet".into()
        } else {
            "no open task (sb task list --all: the done ones too)".into()
        });
    }
    if all && !st.tombs.is_empty() {
        out.push(format!(
            "{} done more than 7 days ago (sb task show <id>, sb history)",
            st.tombs.len()
        ));
    }
    out.join("\n")
}

/// `sb task list --check`'s bars (architect m_66621 point 5): an open task
/// with this many agents or more looks over-linked
pub const CHECK_AGENTS: usize = 8;
/// ...or working this long with nothing new on it
pub const CHECK_QUIET_MS: u64 = 24 * 3_600_000;

/// Why an open work looks over-linked, none when it doesn't. Pure.
pub fn over_linked(w: &VWork, now: u64) -> Option<String> {
    if w.work.merged_into.is_some() || matches!(w.status.as_str(), "done" | "ongoing") {
        return None;
    }
    let n = w.work.agents.len();
    if n >= CHECK_AGENTS {
        return Some(format!("{n} agents"));
    }
    if w.status == "working" && now.saturating_sub(w.work.last) >= CHECK_QUIET_MS {
        return Some(format!("working, nothing new since {}", past_label(w.work.last, now)));
    }
    None
}

/// `sb task list --check`: the open tasks that look over-linked, one line
/// each with why and the cleanup (`sb task unlink`); for agents and the
/// gates, never a label in his UI.
pub fn check(st: &State, now: u64) -> String {
    let mut ws: Vec<&VWork> = st.works.values().collect();
    ws.sort_by_key(|w| w.work.created_ms);
    let out: Vec<String> = ws.iter().filter_map(|w| over_linked(w, now).map(|why| format!("{} · {} · {} · {}", w.work.id, w.work.name, why, w.work.agents.join(", ")))).collect();
    if out.is_empty() {
        return "no open task looks over-linked".into();
    }
    format!("{}\n(on his word: sb task unlink <id> <agent> for an agent that never worked on it)", out.join("\n"))
}

/// `sb task show <id>`: the work whole (a merged one: the one it went
/// into), a folded one's tomb, or why not.
pub fn show(st: &State, id: &str, now: u64) -> Result<String, String> {
    let mut id = id.to_string();
    let mut head = Vec::new();
    // a merge chain ends: each hop is a work sb-core keeps, ids unique
    for _ in 0..st.works.len() {
        match st.works.get(&id).and_then(|w| w.work.merged_into.clone()) {
            Some(into) => {
                head.push(format!("{} was merged into {}", id, into));
                id = into;
            }
            None => break,
        }
    }
    if let Some(w) = st.works.get(&id) {
        head.extend(card(w, now));
        return Ok(head.join("\n"));
    }
    if let Some(t) = st.tombs.get(&id) {
        head.push(format!("{} · {} · done · {}, folded (sb history finds the rest)", t.id, t.name, days_ago(t.done, now)));
        return Ok(head.join("\n"));
    }
    Err(format!("no task {} (sb task list --all)", id))
}

fn card(w: &VWork, now: u64) -> Vec<String> {
    let x = &w.work;
    let mut out = vec![
        format!("{} · {}", x.id, x.name),
        format!("status: {}", what(w, now)),
    ];
    if !x.why.is_empty() {
        out.push(format!("why: {}", x.why));
    }
    if !x.goals.is_empty() {
        out.push("done when:".into());
        for (i, g) in x.goals.iter().enumerate() {
            let by = if g.done && !g.by.is_empty() {
                format!(" ({})", g.by)
            } else {
                String::new()
            };
            out.push(format!(
                "  {}. [{}] {}{}",
                i + 1,
                if g.done { "x" } else { " " },
                g.text,
                by
            ));
        }
    }
    if !x.agents.is_empty() {
        out.push(format!("agents: {}", x.agents.join(", ")));
    }
    if !w.cards.is_empty() {
        let cs: Vec<String> = w.cards.iter().map(|c| format!("#{}", c)).collect();
        out.push(format!("his cards: {}", cs.join(", ")));
    }
    if let Some(m) = x.origin {
        out.push(format!("from his message m_{}", m));
    }
    out.push(format!("made by {} at {}", x.by, hh_mm(x.created_ms)));
    if let Some(f) = &x.done {
        out.push(format!("done {} by {}", past_label(f.at, now), f.by));
    }
    out
}
