//! The state views every agent reads: the task board of main (RFC 0001
//! §8.2), the agent threads summary (RFC 0003 §9), the group roster of a
//! task, and the `sb list` answer. All built without an LLM, from the
//! state alone.
//!
//! The hub rebuilds every agent's context at each input, and the
//! snapshot at each change: what they share is built once per input
//! ([`Waiting`]: one pass over the messages; [`Group`]: the roster's
//! rows), never once per agent. Per agent, a long-lived hub (470 agents,
//! 14,500 messages) spent its whole loop there and a client's hello
//! never got its turn.

use crate::model::{Agent, Mode, Msg, State, Status, MAIN, USER};
use crate::prompts::relation;
use crate::util::{age, clip, one_line};
use std::collections::BTreeMap;

/// Each agent's model (issue #4), by name: its short tag (`opus·hi`, the
/// TUI panel's) for `sb list` and the roster, and its `model:` line for
/// `sb tasks` (`openai/gpt-9 · low (profile fast)`). The daemon fills it
/// from the agents' choice files; an agent missing from it shows none.
pub type Models = BTreeMap<String, (String, String)>;

fn ws_label(a: &Agent) -> String {
    match a.ws.mode {
        Mode::Worktree if a.ws.dropped => " [worktree dropped]".to_string(),
        Mode::Worktree => format!(" [worktree {}]", a.ws.branch.clone().unwrap_or_default()),
        Mode::Shared => a.place.as_ref().map(|p| format!(" [private worktree {}]", p)).unwrap_or_default(),
    }
}

/// One task board line.
pub fn task_line(a: &Agent, now: u64) -> String {
    let st = a.status();
    let mut s = format!(
        "{:<24} {:<9} {:>4}  \"{}\"{}",
        a.name,
        st.as_str(),
        age(a.created_ms, now),
        clip(&a.description(), 70),
        ws_label(a)
    );
    if let Some((_, note)) = &a.declared {
        if !note.is_empty() {
            s.push_str(&format!("  note: {}", clip(&one_line(note), 80)));
        }
    }
    if let Some(r) = &a.last_report {
        s.push_str(&format!(
            "  {} report {} ago: \"{}\"",
            if r.auto { "last turn" } else { r.kind.as_str() },
            age(r.at_ms, now),
            clip(&one_line(&r.summary), 160)
        ));
    }
    if matches!(st, Status::Working | Status::Waiting) {
        if let Some((t, what)) = &a.activity {
            s.push_str(&format!(
                "  now: {} ({} ago)",
                clip(&one_line(what), 100),
                age(*t, now)
            ));
        }
    }
    if st == Status::Failed {
        if let Some(f) = &a.failure {
            s.push_str(&format!("  failure: {}", clip(&one_line(f), 120)));
        }
    }
    s
}

/// `/agents`: the board of the agents the user reads, then the open cards.
pub fn user_board(st: &State, now: u64) -> String {
    let mut lines: Vec<String> = st.tasks().map(|a| task_line(a, now)).collect();
    if lines.is_empty() {
        lines.push("no agents yet".into());
    }
    lines.extend(st.open_cards().map(|c| {
        format!("card #{} {} @{}: {}", c.id, c.kind, c.agent, clip(&one_line(&c.text), 100))
    }));
    lines.join("\n")
}

/// Threads between agents that the user is not part of, most recent
/// first (RFC 0003 §9).
pub fn agent_threads(st: &State, limit: usize) -> Vec<String> {
    let mut threads: BTreeMap<u64, Vec<&crate::model::Msg>> = BTreeMap::new();
    for m in st.msgs.values() {
        threads.entry(m.thread).or_default().push(m);
    }
    let mut shown: Vec<(u64, u64, Vec<&crate::model::Msg>, Vec<&str>)> = threads
        .into_iter()
        .filter(|(_, ms)| !ms.iter().any(|m| m.from == USER || m.to == USER || m.plain))
        .map(|(t, ms)| {
            let parties = parties_of(&ms);
            let last = ms.last().map(|m| m.created_ms).unwrap_or(0);
            (last, t, ms, parties)
        })
        // main's own conversations are already in its thread
        .filter(|(_, _, _, p)| !(p.contains(&MAIN) && p.len() == 2))
        .collect();
    // stable: equal times keep the thread order; only the shown ones
    // are rendered
    shown.sort_by_key(|x| std::cmp::Reverse(x.0));
    shown
        .into_iter()
        .take(limit)
        .map(|(_, t, ms, parties)| {
            let open = ms
                .iter()
                .any(|m| m.expect_reply && !st.settled.contains(&m.id));
            format!(
                "t_{} {}  {} message{}  {}  \"{}\"",
                t,
                // the hub's own id `switchboard` reads `bise`
                parties.iter().map(|p| crate::prompts::shown_sender(p)).collect::<Vec<_>>().join(" ↔ "),
                ms.len(),
                if ms.len() > 1 { "s" } else { "" },
                if open { "open" } else { "answered" },
                clip(&one_line(&ms[0].text), 60)
            )
        })
        .collect()
}

/// The senders and recipients of a thread, in order of appearance.
fn parties_of<'a>(ms: &[&'a crate::model::Msg]) -> Vec<&'a str> {
    let mut p: Vec<&str> = Vec::new();
    for x in ms.iter().flat_map(|m| [m.from.as_str(), m.to.as_str()]) {
        if !p.contains(&x) {
            p.push(x);
        }
    }
    p
}

/// The block appended to every model request of main.
pub fn main_context(st: &State, now: u64, waiting: &Waiting) -> String {
    let mut s = String::from(
        "<bise_state>\nLive state injected by bise before this call (not a user message).\n<task_board>\n",
    );
    let tasks: Vec<&Agent> = st
        .tasks()
        .filter(|a| a.status() != Status::Archived)
        .collect();
    if tasks.is_empty() {
        s.push_str("(no task)\n");
    }
    for a in &tasks {
        s.push_str(&task_line(a, now));
        s.push('\n');
    }
    let archived = st
        .tasks()
        .filter(|a| a.status() == Status::Archived)
        .count();
    if archived > 0 {
        s.push_str(&format!(
            "({} archived task{})\n",
            archived,
            if archived > 1 { "s" } else { "" }
        ));
    }
    s.push_str("</task_board>\n");
    push_block(&mut s, "agent_threads", &agent_threads(st, 8));
    let cards: Vec<String> = st
        .open_cards()
        .map(|c| {
            let for_msg = c
                .for_msg
                .map(|m| format!(" (answers m_{})", m))
                .unwrap_or_default();
            format!(
                "#{} {} @{}: \"{}\"{}",
                c.id,
                c.kind,
                c.agent,
                clip(&one_line(&c.text), 100),
                for_msg
            )
        })
        .collect();
    push_block(&mut s, "open_cards", &cards);
    push_block(&mut s, "questions_for_you", &questions_for(waiting.unanswered(MAIN)));
    s.push_str("</bise_state>");
    s
}

/// The short status that precedes each user message to main: one line
/// per task that is not archived. Empty when there is no such task.
pub fn status_block(st: &State, now: u64) -> String {
    let lines: Vec<String> = st
        .tasks()
        .filter(|a| a.status() != Status::Archived)
        .map(|a| {
            let mut l = format!(
                "{} {} {}",
                a.name,
                a.status().as_str(),
                age(a.created_ms, now)
            );
            if let Some(b) =
                a.ws.branch
                    .as_ref()
                    .filter(|_| a.ws.mode == Mode::Worktree && !a.ws.dropped)
            {
                l.push_str(&format!(" [{}]", b));
            }
            if let Some(p) = &a.place {
                l.push_str(&format!(" [worktree {}]", p));
            }
            match a.status() {
                Status::Working | Status::Waiting => {
                    if let Some((t, what)) = &a.activity {
                        l.push_str(&format!(
                            " — now: {} ({} ago)",
                            clip(&one_line(what), 70),
                            age(*t, now)
                        ));
                    }
                }
                _ => {
                    if let Some(r) = &a.last_report {
                        l.push_str(&format!(" — last: \"{}\"", clip(&one_line(&r.summary), 90)));
                    }
                }
            }
            if a.status() == Status::Failed {
                if let Some(f) = &a.failure {
                    l.push_str(&format!(" — failure: {}", clip(&one_line(f), 90)));
                }
            }
            l
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!("<task_status>\n{}\n</task_status>", lines.join("\n"))
}

/// `sb tasks`: every task in detail, for main (and anyone).
pub fn tasks_detail(st: &State, now: u64, models: &Models) -> String {
    let mut out: Vec<String> = Vec::new();
    for a in st.tasks() {
        let mut b = vec![format!(
            "## {} — {}{} (created {} ago, by {})",
            a.name,
            a.status().as_str(),
            match a.ws.mode {
                Mode::Worktree if a.ws.dropped => " — worktree dropped".to_string(),
                Mode::Worktree => format!(
                    " — worktree {} on {}",
                    a.ws.path,
                    a.ws.branch.clone().unwrap_or_default()
                ),
                Mode::Shared => match &a.place {
                    Some(p) => format!(" — private worktree {} (commits to the shared branch)", p),
                    None => " — shared workspace".to_string(),
                },
            },
            age(a.created_ms, now),
            a.parent.clone().unwrap_or_default()
        )];
        if let Some((_, line)) = models.get(&a.name) {
            b.push(format!("model: {}", line));
        }
        b.push(format!(
            "objective: {}",
            clip(&one_line(&a.brief.objective), 300)
        ));
        if let Some(t) = a
            .turn_started_ms
            .filter(|_| matches!(a.status(), Status::Working | Status::Waiting))
        {
            b.push(format!("current turn: started {} ago", age(t, now)));
        }
        if let Some((t, what)) = &a.activity {
            b.push(format!(
                "last activity ({} ago): {}",
                age(*t, now),
                clip(&one_line(what), 200)
            ));
        }
        if let Some((d, note)) = &a.declared {
            b.push(format!(
                "declared: {:?}{}",
                d,
                if note.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", clip(&one_line(note), 200))
                }
            ));
        }
        if let Some(r) = &a.last_report {
            b.push(format!(
                "last report ({}, {} ago): {}",
                if r.auto {
                    "end of turn"
                } else {
                    r.kind.as_str()
                },
                age(r.at_ms, now),
                clip(&one_line(&r.summary), 500)
            ));
        }
        if let Some(f) = &a.failure {
            b.push(format!("failure: {}", clip(&one_line(f), 300)));
        }
        let q = queued_count(st, &a.name);
        if q > 0 {
            b.push(format!("messages waiting for delivery: {}", q));
        }
        for m in st.unanswered_for(&a.name) {
            b.push(format!(
                "owes a reply to {} (m_{}): \"{}\"",
                m.from,
                m.id,
                clip(&one_line(&m.text), 120)
            ));
        }
        for m in st
            .msgs
            .values()
            .filter(|m| m.from == a.name && m.expect_reply && !st.settled.contains(&m.id))
        {
            b.push(format!(
                "waits for a reply from {} (m_{}): \"{}\"",
                m.to,
                m.id,
                clip(&one_line(&m.text), 120)
            ));
        }
        if !a.files.is_empty() {
            b.push(format!(
                "files changed: {}",
                a.files
                    .iter()
                    .take(10)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        for c in st.open_cards().filter(|c| c.agent == a.name) {
            b.push(format!(
                "open card #{} {}: {}",
                c.id,
                c.kind,
                clip(&one_line(&c.text), 150)
            ));
        }
        out.push(b.join("\n"));
    }
    if out.is_empty() {
        "no agents yet".to_string()
    } else {
        out.join("\n\n")
    }
}

/// The group roster, as `name` sees it (`sb list`, and the context of a
/// task).
pub fn roster(st: &State, viewer: &str, now: u64, models: &Models) -> Vec<String> {
    Group::of(st, now, models).seen_by(viewer)
}

/// The roster's rows but the viewer's column, built once for all the
/// agents that read it (each task's context has the whole roster).
pub struct Group<'a> {
    st: &'a State,
    /// Each agent the roster lists, in order, and its row after the
    /// relation column.
    rows: Vec<(&'a Agent, String)>,
}

impl<'a> Group<'a> {
    pub fn of(st: &'a State, now: u64, models: &Models) -> Group<'a> {
        let width = models.values().map(|(t, _)| t.chars().count().min(18)).max().unwrap_or(0);
        let rows = st
            .order
            .iter()
            .filter_map(|n| st.agents.get(n))
            .filter(|a| a.status() != Status::Archived)
            .map(|a| {
                let note = a
                    .declared
                    .as_ref()
                    .filter(|(_, n)| !n.is_empty())
                    .map(|(_, n)| format!(" — {}", clip(&one_line(n), 60)))
                    .unwrap_or_default();
                // its model's tag, a column of its own (issue #4): as wide as
                // the longest tag (at most 18), a name never cut mid-word
                let model = models.get(&a.name).map(|(t, _)| format!("{:<w$} ", clip(t, 18), w = width)).unwrap_or_default();
                let rest = format!(
                    "{:<9} {:>4}  {}{}{}",
                    a.status().as_str(),
                    age(if a.is_main { now } else { a.created_ms }, now),
                    model,
                    clip(&a.description(), 70),
                    note
                );
                (a, rest)
            })
            .collect();
        Group { st, rows }
    }

    /// The roster as `viewer` sees it: who is itself, its parent, its
    /// children, its peers.
    pub fn seen_by(&self, viewer: &str) -> Vec<String> {
        let viewer_parent = self.st.agents.get(viewer).and_then(|a| a.parent.as_deref());
        self.rows
            .iter()
            .map(|(a, rest)| {
                let rel = if a.name == viewer {
                    "self"
                } else {
                    match relation(&a.name, viewer, a.parent.as_deref(), viewer_parent) {
                        "parent" => "parent",
                        "child" => "child",
                        _ => "peer",
                    }
                };
                format!("{:<24} {:<6} {}", a.name, rel, rest)
            })
            .collect()
    }
}

/// The block appended to every model request of a task.
pub fn task_context(name: &str, group: &Group, waiting: &Waiting) -> String {
    let mut s = format!(
        "<bise_state>\nLive state injected by bise before this call (not a user message). You are `{}`.\n<group>\n",
        name
    );
    for l in group.seen_by(name) {
        s.push_str(&l);
        s.push('\n');
    }
    s.push_str("</group>\n");
    push_block(&mut s, "questions_for_you", &questions_for(waiting.unanswered(name)));
    s.push_str("</bise_state>");
    s
}

/// `<tag>` + one line each + `</tag>`, nothing when there is no line.
fn push_block(s: &mut String, tag: &str, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    s.push_str(&format!("<{}>\n", tag));
    for l in lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str(&format!("</{}>\n", tag));
}

/// The delivered messages still waiting for their recipient's reply.
fn questions_for(unanswered: &[&Msg]) -> Vec<String> {
    unanswered
        .iter()
        .map(|m| format!("m_{} from {}: \"{}\"", m.id, m.from, clip(&one_line(&m.text), 80)))
        .collect()
}

/// Queued messages, for the board shown to the user.
pub fn queued_count(st: &State, name: &str) -> usize {
    st.msgs.values().filter(|m| m.to == name && st.is_queued(m)).count()
}

/// What waits for each agent, from one pass over the messages: those
/// queued for it, and the delivered ones it owes a reply to. Oldest
/// first, as in `st.msgs`.
pub struct Waiting<'a> {
    queued: BTreeMap<&'a str, Vec<&'a Msg>>,
    unanswered: BTreeMap<&'a str, Vec<&'a Msg>>,
}

impl<'a> Waiting<'a> {
    pub fn of(st: &'a State) -> Waiting<'a> {
        let mut w = Waiting { queued: BTreeMap::new(), unanswered: BTreeMap::new() };
        for m in st.msgs.values() {
            if st.is_queued(m) {
                w.queued.entry(m.to.as_str()).or_default().push(m);
            }
            if st.owes_reply(m) {
                w.unanswered.entry(m.to.as_str()).or_default().push(m);
            }
        }
        w
    }

    /// How many messages wait for delivery to `name` ([`queued_count`]).
    pub fn queued_count(&self, name: &str) -> usize {
        self.queued.get(name).map_or(0, Vec::len)
    }

    /// The user's queued inputs still waiting for `name`'s turn to end
    /// (the window's "send queued"), oldest first: `{id, text,
    /// created_ms}` each, the snapshot's `queued_inputs` (bise-proto's
    /// `Agent.queued`).
    pub fn queued_inputs(&self, name: &str) -> Vec<serde_json::Value> {
        self.queued
            .get(name)
            .into_iter()
            .flatten()
            .filter(|m| m.queued && m.from == USER)
            .map(|m| serde_json::json!({"id": m.id, "text": m.text, "created_ms": m.created_ms}))
            .collect()
    }

    /// The delivered messages to `name` that expect a reply it has not
    /// given (`State::unanswered_for`).
    pub fn unanswered(&self, name: &str) -> &[&'a Msg] {
        self.unanswered.get(name).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Msg, MsgState};

    fn state() -> State {
        let mut st = State::new("/w");
        for (n, obj) in [
            ("auth-fix", "Corriger le login Safari"),
            ("docs", "Doc API v2"),
        ] {
            st.test_task(n, obj);
        }
        st
    }

    #[test]
    fn the_board_lists_every_live_task() {
        let st = state();
        let c = main_context(&st, 12 * 60_000, &Waiting::of(&st));
        assert!(c.contains("auth-fix"), "{}", c);
        assert!(c.contains("\"Doc API v2\""), "{}", c);
        assert!(c.contains("12m"), "{}", c);
        assert!(c.starts_with("<bise_state>") && c.ends_with("</bise_state>"));
    }

    #[test]
    fn peer_threads_are_summarized_for_main() {
        let mut st = state();
        st.msg_state.insert(
            1,
            MsgState::Queued {
                reason: "new".into(),
            },
        );
        st.msgs.insert(
            1,
            Msg {
                id: 1,
                thread: 1,
                from: "docs".into(),
                to: "auth-fix".into(),
                reply_to: None,
                expect_reply: true,
                auto: false,
                text: "v1 ou v2 ?".into(),
                created_ms: 1,
                plain: false,
                queued: false,
                via: None,
            },
        );
        let t = agent_threads(&st, 8);
        assert_eq!(t.len(), 1);
        assert!(
            t[0].contains("docs ↔ auth-fix") && t[0].contains("open"),
            "{}",
            t[0]
        );
    }

    fn msg(id: u64, from: &str, to: &str, expect_reply: bool, queued: bool) -> Msg {
        Msg {
            id,
            thread: id,
            from: from.into(),
            to: to.into(),
            reply_to: None,
            expect_reply,
            auto: false,
            text: format!("m{id}"),
            created_ms: id,
            plain: false,
            queued,
            via: None,
        }
    }

    #[test]
    fn waiting_is_the_per_agent_scans_in_one_pass() {
        let mut st = state();
        let states = [
            MsgState::Delivered,
            MsgState::Queued { reason: "busy".into() },
            MsgState::Delivered,
        ];
        let names = [MAIN, "auth-fix", "docs", USER];
        for id in 1..=60u64 {
            let (from, to) = (names[(id % 4) as usize], names[((id + 1) % 4) as usize]);
            st.msgs.insert(id, msg(id, from, to, id % 2 == 0, id % 5 == 0));
            st.msg_state.insert(id, states[(id % 3) as usize].clone());
        }
        st.settled.insert(4);
        let w = Waiting::of(&st);
        for n in names {
            assert_eq!(w.queued_count(n), queued_count(&st, n), "{n}");
            let ids = |ms: &[&Msg]| ms.iter().map(|m| m.id).collect::<Vec<_>>();
            assert_eq!(ids(w.unanswered(n)), ids(&st.unanswered_for(n)), "{n}");
            let inputs: Vec<serde_json::Value> = st
                .msgs
                .values()
                .filter(|m| m.to == n && m.queued && m.from == USER && st.is_queued(m))
                .map(|m| serde_json::json!({"id": m.id, "text": m.text, "created_ms": m.created_ms}))
                .collect();
            assert_eq!(w.queued_inputs(n), inputs, "{n}");
        }
        // none of the three lists is empty everywhere
        assert!(names.iter().map(|n| w.queued_count(n)).sum::<usize>() > 0);
        assert!(names.iter().any(|n| !w.unanswered(n).is_empty()));
        assert!(names.iter().any(|n| !w.queued_inputs(n).is_empty()));
    }

    #[test]
    fn a_task_context_has_the_roster_it_sees_and_its_questions() {
        let mut st = state();
        st.msgs.insert(1, msg(1, "auth-fix", "docs", true, false));
        st.msg_state.insert(1, MsgState::Delivered);
        let c = task_context("docs", &Group::of(&st, 0, &Models::new()), &Waiting::of(&st));
        let mut want = String::from("<group>\n");
        for l in roster(&st, "docs", 0, &Models::new()) {
            want.push_str(&l);
            want.push('\n');
        }
        assert!(c.contains(&want), "{c}");
        assert!(c.contains("<questions_for_you>\nm_1 from auth-fix: \"m1\"\n</questions_for_you>"), "{c}");
        let r = roster(&st, "docs", 0, &Models::new());
        assert_eq!(r[0], format!("{:<24} {:<6} {:<9} {:>4}  {}", MAIN, "parent", st.agents[MAIN].status().as_str(), "0s", st.agents[MAIN].description()));
    }

    #[test]
    fn a_task_sees_its_relations() {
        let st = state();
        let r = roster(&st, "docs", 0, &Models::new());
        assert!(
            r.iter()
                .any(|l| l.starts_with("main") && l.contains("parent")),
            "{:?}",
            r
        );
        assert!(
            r.iter()
                .any(|l| l.starts_with("auth-fix") && l.contains("peer")),
            "{:?}",
            r
        );
        assert!(
            r.iter()
                .any(|l| l.starts_with("docs") && l.contains("self")),
            "{:?}",
            r
        );
    }
}
