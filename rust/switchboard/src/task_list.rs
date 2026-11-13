//! tasks (docs/tasks.md): `sb task list` and `sb task show`, the hub's
//! reads of the works sb-core's view gave (`State.works`, `State.tombs`).
//! The status and what it is about are sb-core's (wstatus.bend); this
//! file only puts them in words, never decides one.

use crate::every::{hh_mm, past_label, when_label};
use crate::model::State;
use bise_proto::core::view::ViewWork as VWork;

/// A work's status and what it is about, one pattern for all: `<status>
/// · <what>` (tasks-ux m_64508: never blank; m_64563: the status word
/// always there, the Board's and the Task tab's pill read the same).
pub fn what(w: &VWork, now: u64) -> String {
    let past = || w.at.map(|t| past_label(t, now)).unwrap_or_default();
    let next = || w.at.map(|t| when_label(t, now)).unwrap_or_default();
    match (w.status.as_str(), w.why.as_str()) {
        ("needs_you", _) => format!("needs you · {} question{}", w.needs, if w.needs == 1 { "" } else { "s" }),
        ("working", _) => format!("working · {}", w.who),
        ("waiting", "reply") => format!("waiting · {}'s reply", w.who),
        ("waiting", "timer") => format!("waiting · wakes at {}", next()),
        ("waiting", "watch") => format!("waiting · {}'s watch", w.who),
        ("waiting", _) => format!("waiting · quiet since {}", past()),
        ("stalled", "crash") => format!("stalled · {} stopped at {}", w.who, past()),
        ("stalled", _) => format!("stalled · quiet since {}", past()),
        ("paused", "until") => format!("paused · until {}", next()),
        ("paused", _) => "paused · by you".into(),
        ("done", _) => match &w.work.done {
            Some(f) => format!("done · {}", days_ago(f.at, now)),
            None => "done".into(),
        },
        (s, _) => s.replace('_', " "),
    }
}

/// `today`, `yesterday`, `3 days ago`
fn days_ago(at: u64, now: u64) -> String {
    match now.saturating_sub(at) / (24 * 3_600_000) {
        0 => "today".into(),
        1 => "yesterday".into(),
        n => format!("{} days ago", n),
    }
}

/// The order of `sb task list`: what needs him first, done last.
fn rank(status: &str) -> u8 {
    match status {
        "needs_you" => 0,
        "stalled" => 1,
        "to_review" => 2,
        "working" => 3,
        "waiting" => 4,
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

/// `sb task list`: the open works (`all`: with the done and ongoing ones,
/// and how many were folded).
pub fn list(st: &State, all: bool, now: u64) -> String {
    let mut ws: Vec<&VWork> = st
        .works
        .values()
        .filter(|w| w.work.merged_into.is_none())
        .filter(|w| all || !matches!(w.status.as_str(), "done" | "ongoing"))
        .collect();
    ws.sort_by_key(|w| (rank(&w.status), w.work.created_ms));
    let mut out: Vec<String> = ws.iter().map(|w| line(w, now)).collect();
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
