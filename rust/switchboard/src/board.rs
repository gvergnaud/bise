//! The state views every agent reads: the task board of main (RFC 0001
//! §8.2), the agent threads summary (RFC 0003 §9), the group roster of a
//! task, and the `sb list` answer. All built without an LLM, from the
//! state alone.
//!
//! The contexts are built here and kept by contexts.rs, which rebuilds
//! only those whose inputs changed; the snapshot is built at each change.
//! What they share is built once ([`Waiting`]: one pass over the
//! messages; [`Group`]: the roster's rows), never once per agent. Per
//! agent, a long-lived hub (470 agents, 14,500 messages) spent its whole
//! loop there and a client's hello never got its turn.

use crate::model::{Agent, Lifecycle, Mode, Msg, Peek, Row, State, Status, MAIN, USER};
use crate::prompts::relation;
use crate::util::{age, clip, one_line};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

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
    if let Some(d) = a.declared() {
        if !d.note.is_empty() {
            s.push_str(&format!("  note: {}", clip(&one_line(&d.note), 80)));
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

/// `/agents`: the board of the live agents the user reads (an archived
/// one stays under `/archived`, with one line that says how many), then
/// the open notifications.
pub fn user_board(st: &State, now: u64) -> String {
    let (live, archived): (Vec<&Agent>, Vec<&Agent>) =
        st.tasks().partition(|a| a.lifecycle != Lifecycle::Archived);
    let mut lines: Vec<String> = live.iter().map(|a| task_line(a, now)).collect();
    if lines.is_empty() {
        lines.push(if archived.is_empty() { "no agents yet" } else { "no live agents" }.into());
    }
    if !archived.is_empty() {
        lines.push(format!("{} archived: /archived", archived.len()));
    }
    lines.extend(st.open_cards().map(|c| {
        format!("notification #{} {} @{}: {}", c.id, c.kind, c.agent, clip(&one_line(&c.text), 100))
    }));
    lines.join("\n")
}

/// The first message of each of the `limit` newest threads of
/// [`agent_threads`]: the bodies it shows (the hub fetches the cold
/// ones first, view-all-perf).
pub fn thread_firsts(st: &State, limit: usize) -> Vec<u64> {
    st.threads().newest(limit).filter_map(|(_, ids)| ids.first().copied()).collect()
}

/// Threads between agents that the user is not part of, most recent
/// first (RFC 0003 §9): the `limit` newest of [`Threads`], rendered.
pub fn agent_threads(st: &State, limit: usize) -> Vec<String> {
    st.threads()
        .newest(limit)
        .map(|(t, ids)| {
            let ms: Vec<Peek> = ids.iter().filter_map(|id| st.peek(*id)).collect();
            let parties = parties_of(&ms);
            let open = ms.iter().any(|m| m.expect_reply && !st.settled().contains(&m.id));
            // the first message's words: the hub fetched the body of a
            // cold one before the build ([`thread_firsts`]); one still
            // cold (sb-core didn't answer) is shown without them
            let first = st.msg(ms[0].id).map(|m| format!("  \"{}\"", clip(&one_line(&m.text), 60))).unwrap_or_default();
            format!(
                "t_{} {}  {} message{}  {}{}",
                t,
                // the hub's own id `switchboard` reads `bise`
                parties.iter().map(|p| crate::prompts::shown_sender(p)).collect::<Vec<_>>().join(" ↔ "),
                ms.len(),
                if ms.len() > 1 { "s" } else { "" },
                if open { "open" } else { "answered" },
                first
            )
        })
        .collect()
}

/// The threads of main's `<agent_threads>`, kept as the messages arrive
/// (issue 21, hot-view sha 2a): [`State::put_msg`] calls [`Threads::put`]
/// with each message, so [`agent_threads`] reads the newest few instead
/// of grouping every message at each build (31.6 ms per user line on his
/// journal copy: 19,474 messages in 16,243 threads).
///
/// A thread is shown when no message of it is the user's (from or to) or
/// plain, and its parties are not just main and one other (main's own
/// conversations are in its thread already); newest first by its last
/// message's time, equal times by thread id.
#[derive(Clone, Debug, Default)]
pub struct Threads {
    /// each thread's message ids, oldest first
    ids: BTreeMap<u64, Vec<u64>>,
    /// the shown threads, newest first
    shown: BTreeSet<(Reverse<u64>, u64)>,
    /// a shown thread's place in `shown`
    key: BTreeMap<u64, Reverse<u64>>,
}

impl Threads {
    /// Message `id` of `thread` was written (`msgs`: every message, it
    /// included): the thread is judged again from its messages.
    pub fn put(&mut self, msgs: &BTreeMap<u64, Row>, thread: u64, id: u64) {
        let ids = self.ids.entry(thread).or_default();
        if let Err(at) = ids.binary_search(&id) {
            ids.insert(at, id);
        }
        self.judge(msgs, thread);
    }

    /// Message `id` left `thread` (a row rewritten with another thread).
    pub fn take(&mut self, msgs: &BTreeMap<u64, Row>, thread: u64, id: u64) {
        if let Some(ids) = self.ids.get_mut(&thread) {
            ids.retain(|x| *x != id);
        }
        self.judge(msgs, thread);
    }

    fn judge(&mut self, msgs: &BTreeMap<u64, Row>, thread: u64) {
        if let Some(k) = self.key.remove(&thread) {
            self.shown.remove(&(k, thread));
        }
        let ms: Vec<Peek> = self.ids.get(&thread).into_iter().flatten().filter_map(|id| msgs.get(id).map(Row::peek)).collect();
        let Some(last) = ms.last() else { return };
        let users = ms.iter().any(|m| m.from == USER || m.to == USER || m.plain);
        let parties = parties_of(&ms);
        if users || (parties.contains(&MAIN) && parties.len() == 2) {
            return;
        }
        let k = Reverse(last.created_ms);
        self.shown.insert((k, thread));
        self.key.insert(thread, k);
    }

    /// The `limit` newest shown threads: (thread, its message ids).
    pub fn newest(&self, limit: usize) -> impl Iterator<Item = (u64, &[u64])> + '_ {
        self.shown.iter().take(limit).map(|(_, t)| (*t, self.ids[t].as_slice()))
    }
}

/// The senders and recipients of a thread, in order of appearance.
fn parties_of<'a>(ms: &[Peek<'a>]) -> Vec<&'a str> {
    let mut p: Vec<&str> = Vec::new();
    for x in ms.iter().flat_map(|m| [m.from, m.to]) {
        if !p.contains(&x) {
            p.push(x);
        }
    }
    p
}

/// The block appended to every model request of main.
/// `threads`: [`agent_threads`]`(st, 8)`; `questions`: [`questions_for`]
/// main's unanswered messages (both built from the messages alone, so
/// contexts.rs keeps them until the messages change).
pub fn main_context(st: &State, now: u64, threads: &[String], questions: &[String]) -> String {
    let mut s = format!(
        "<{}>\nLive state injected by bise before this call (not a user message).\n<task_board>\n",
        bise_proto::envelope::BISE_STATE
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
    push_block(&mut s, "agent_threads", threads);
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
    push_block(&mut s, "open_notifications", &cards);
    push_block(&mut s, "questions_for_you", questions);
    s.push_str(&format!("</{}>", bise_proto::envelope::BISE_STATE));
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
            .turn_ms
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
        if let Some(d) = a.declared() {
            b.push(format!(
                "declared: {:?}{}",
                d.status,
                if d.note.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", clip(&one_line(&d.note), 200))
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
        // whole ones only: sb-core sends every question not settled whole
        // (view.bend `ships_whole`)
        for m in st
            .whole_msgs()
            .filter(|m| m.from == a.name && m.expect_reply && !st.settled().contains(&m.id))
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
                "open notification #{} {}: {}",
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
                    .declared()
                    .filter(|d| !d.note.is_empty())
                    .map(|d| format!(" — {}", clip(&one_line(&d.note), 60)))
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

    /// What [`Group::seen_by`] reads besides the viewer: each row's
    /// agent, its parent and its row (two equal lists, equal rosters).
    pub fn rows(&self) -> Vec<(String, Option<String>, String)> {
        self.rows.iter().map(|(a, rest)| (a.name.clone(), a.parent.clone(), rest.clone())).collect()
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
/// `questions`: [`questions_for`] its unanswered messages.
pub fn task_context(name: &str, group: &Group, questions: &[String]) -> String {
    let mut s = format!(
        "<{}>\nLive state injected by bise before this call (not a user message). You are `{}`.\n<group>\n",
        bise_proto::envelope::BISE_STATE,
        name
    );
    for l in group.seen_by(name) {
        s.push_str(&l);
        s.push('\n');
    }
    s.push_str("</group>\n");
    push_block(&mut s, "questions_for_you", questions);
    s.push_str(&format!("</{}>", bise_proto::envelope::BISE_STATE));
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
pub fn questions_for(unanswered: &[&Msg]) -> Vec<String> {
    unanswered
        .iter()
        .map(|m| format!("m_{} from {}: \"{}\"", m.id, m.from, clip(&one_line(&m.text), 80)))
        .collect()
}

/// Queued messages, for the board shown to the user.
pub fn queued_count(st: &State, name: &str) -> usize {
    st.hot().filter(|(m, h)| h.queued && m.to == name).count()
}

/// What waits for each agent, from one pass over the hot messages (what
/// sb-core's view says of each, `State::hot`): those queued for it, and
/// the delivered ones it owes a reply to. Oldest first, as in `st.msgs()`.
pub struct Waiting<'a> {
    queued: BTreeMap<&'a str, Vec<&'a Msg>>,
    unanswered: BTreeMap<&'a str, Vec<&'a Msg>>,
}

impl<'a> Waiting<'a> {
    pub fn of(st: &'a State) -> Waiting<'a> {
        let mut w = Waiting { queued: BTreeMap::new(), unanswered: BTreeMap::new() };
        for (m, h) in st.hot() {
            if h.queued {
                w.queued.entry(m.to.as_str()).or_default().push(m);
            }
            if h.owes {
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
    pub fn queued_inputs(&self, name: &str) -> Vec<bise_proto::draft::Queued> {
        self.queued
            .get(name)
            .into_iter()
            .flatten()
            .filter(|m| m.queued && m.from == USER)
            // his parts (msg-parts): a client edits it back from them
            .map(|m| bise_proto::draft::Queued { id: m.id, text: m.text.clone(), created_ms: m.created_ms, parts: m.parts_list() })
            .collect()
    }

    /// The delivered messages to `name` that expect a reply it has not
    /// given (`State::unanswered_for`).
    pub fn unanswered(&self, name: &str) -> &[&'a Msg] {
        self.unanswered.get(name).map_or(&[], Vec::as_slice)
    }

    /// Each agent's `<questions_for_you>` lines ([`questions_for`]), for
    /// every agent that owes a reply.
    pub fn questions(&self) -> BTreeMap<String, Vec<String>> {
        self.unanswered.iter().map(|(n, ms)| (n.to_string(), questions_for(ms))).collect()
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

    /// issue 23: his queued row carries his parts (msg-parts), so a client
    /// edits it back with its chips; a message with none has no `parts`.
    #[test]
    fn a_queued_row_carries_his_parts() {
        let mut st = state();
        let parts = r#"[{"kind":"paste","n":1,"text":"long"},{"kind":"text","text":" why?"}]"#;
        let m = |id: u64, parts: &str| Msg {
            id,
            thread: id,
            from: USER.into(),
            to: "docs".into(),
            text: format!("m{id}"),
            created_ms: id,
            plain: true,
            queued: true,
            parts: parts.into(),
            ..Default::default()
        };
        let busy = || Some(MsgState::Queued { reason: "busy".into() });
        st.put_msg_judged(m(1, parts), busy(), false);
        st.put_msg_judged(m(2, ""), busy(), false);
        let rows = Waiting::of(&st).queued_inputs("docs");
        assert_eq!(serde_json::to_value(&rows[0].parts).unwrap(), serde_json::from_str::<serde_json::Value>(parts).unwrap());
        assert!(rows[1].parts.is_empty(), "{:?}", rows[1]);
    }

    #[test]
    fn the_board_lists_every_live_task() {
        let st = state();
        let c = main_context(&st, 12 * 60_000, &agent_threads(&st, 8), &[]);
        assert!(c.contains("auth-fix"), "{}", c);
        assert!(c.contains("\"Doc API v2\""), "{}", c);
        assert!(c.contains("12m"), "{}", c);
        assert!(c.starts_with("<bise_state>") && c.ends_with("</bise_state>"));
    }

    #[test]
    fn user_board_lists_live_agents_only() {
        use crate::model::{Declaration, Declared, Lifecycle};
        let mut st = state();
        st.test_task("old", "Ancienne tâche");
        st.test_task("kid", "Sous-tâche vivante");
        st.agents.get_mut("old").unwrap().lifecycle = Lifecycle::Archived;
        st.agents.get_mut("kid").unwrap().parent = Some("old".into());
        st.agents.get_mut("docs").unwrap().declared = Some(Declaration { status: Declared::Done, note: String::new() });
        let b = user_board(&st, 60_000);
        assert!(!b.contains("old ") && !b.contains("Ancienne"), "{}", b);
        assert!(b.contains("auth-fix") && b.contains("docs") && b.contains("kid"), "{}", b);
        assert!(b.contains("1 archived: /archived"), "{}", b);
        // only archived ones: none live, the count still says where they are
        for n in ["auth-fix", "docs", "kid"] {
            st.agents.get_mut(n).unwrap().lifecycle = Lifecycle::Archived;
        }
        let b = user_board(&st, 60_000);
        assert!(b.starts_with("no live agents\n4 archived: /archived"), "{}", b);
        assert_eq!(user_board(&State::new("/w"), 0), "no agents yet");
    }

    #[test]
    fn peer_threads_are_summarized_for_main() {
        let mut st = state();
        st.put_msg_judged(
            Msg {
                id: 1,
                thread: 1,
                from: "docs".into(),
                to: "auth-fix".into(),
                expect_reply: true,
                text: "v1 ou v2 ?".into(),
                created_ms: 1,
                ..Default::default()
            },
            Some(MsgState::Queued { reason: "new".into() }),
            false,
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
            expect_reply,
            text: format!("m{id}"),
            created_ms: id,
            queued,
            ..Default::default()
        }
    }

    /// The walk agent_threads made before [`Threads`]: every message
    /// grouped by thread at each call (the reference the index equals).
    fn threads_walk(st: &State, limit: usize) -> Vec<(u64, Vec<u64>)> {
        let mut threads: BTreeMap<u64, Vec<Peek>> = BTreeMap::new();
        for m in st.peeks() {
            threads.entry(m.thread).or_default().push(m);
        }
        let mut shown: Vec<(u64, u64, Vec<u64>)> = threads
            .into_iter()
            .filter(|(_, ms)| !ms.iter().any(|m| m.from == USER || m.to == USER || m.plain))
            .filter(|(_, ms)| {
                let p = parties_of(ms);
                !(p.contains(&MAIN) && p.len() == 2)
            })
            .map(|(t, ms)| (ms.last().unwrap().created_ms, t, ms.iter().map(|m| m.id).collect()))
            .collect();
        shown.sort_by_key(|x| std::cmp::Reverse(x.0));
        shown.into_iter().take(limit).map(|(_, t, ids)| (t, ids)).collect()
    }

    /// hot-view sha 2a: the kept threads are the full walk's, message by
    /// message: agents' threads, the user's and plain ones (never shown),
    /// main's own pairs (never shown), a third party joining main's pair
    /// (shown), equal times (thread order), a question settled by a reply
    /// re-sent with the reply (`reply_to`), a row re-sent unchanged, and a
    /// clear (a full view).
    #[test]
    fn the_kept_threads_are_the_full_walk() {
        let mut st = state();
        let names = [MAIN, "auth-fix", "docs", "perf"];
        let check = |st: &State| {
            for limit in [0, 1, 3, 8, 100] {
                let kept: Vec<(u64, Vec<u64>)> = st.threads().newest(limit).map(|(t, ids)| (t, ids.to_vec())).collect();
                assert_eq!(kept, threads_walk(st, limit), "limit {limit}");
            }
        };
        for id in 1..=120u64 {
            let (from, mut to) = (names[(id % 4) as usize], names[((id * 3 + 1) % 4) as usize]);
            if id % 23 == 0 {
                to = USER;
            }
            let mut m = msg(id, from, to, id % 3 == 0, false);
            m.thread = id % 17 + 1;
            m.created_ms = id / 4; // equal times
            m.plain = id % 29 == 0;
            m.reply_to = (id > 10 && id % 7 == 0).then(|| id - 7);
            st.put_msg_judged(m.clone(), Some(MsgState::Delivered), false);
            if let Some(q) = m.reply_to {
                // the view re-sends the question a reply settled
                let q = st.msgs()[&q].clone();
                st.put_msg_judged(q, None, true);
            }
            if id % 11 == 0 {
                st.put_msg_judged(m, None, false);
            }
            check(&st);
        }
        assert!(st.threads().newest(100).count() > 3, "some threads are shown");
        assert!(st.threads().newest(100).count() < 17, "some are not");
        // a row rewritten with another thread leaves the old one
        let mut m = st.msgs()[&40].clone();
        m.thread = 99;
        st.put_msg_judged(m, None, false);
        check(&st);
        st.clear_msgs();
        assert_eq!(st.threads().newest(8).count(), 0);
        check(&st);
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
            st.put_msg_judged(msg(id, from, to, id % 2 == 0, id % 5 == 0), Some(states[(id % 3) as usize].clone()), id == 4);
        }
        let w = Waiting::of(&st);
        for n in names {
            assert_eq!(w.queued_count(n), queued_count(&st, n), "{n}");
            let ids = |ms: &[&Msg]| ms.iter().map(|m| m.id).collect::<Vec<_>>();
            assert_eq!(ids(w.unanswered(n)), ids(&st.unanswered_for(n)), "{n}");
            let inputs: Vec<bise_proto::draft::Queued> = st
                .msgs()
                .values()
                .filter(|m| m.to == n && m.queued && m.from == USER && st.is_queued(m))
                .map(|m| bise_proto::draft::Queued { id: m.id, text: m.text.clone(), created_ms: m.created_ms, parts: Vec::new() })
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
        st.put_msg_judged(msg(1, "auth-fix", "docs", true, false), Some(MsgState::Delivered), false);
        let c = task_context("docs", &Group::of(&st, 0, &Models::new()), &questions_for(Waiting::of(&st).unanswered("docs")));
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
