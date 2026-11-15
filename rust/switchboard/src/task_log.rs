//! A task's Log (roma idea 3; architect m_65317): who did what and why,
//! newest first: its agents' `sb report`s with their decisions, his
//! answers to its questions, its lands, his gestures (pause, done, move,
//! rename...), agents joining. A read, never a row kept per step: built
//! when the Task tab or `sb task show --log` asks, from facts journaled
//! already, filtered by the task's works:
//! - report: `reported`, its `report.works` (the reporter's `--task` or
//!   turn's works at the report, written since this change; an older line
//!   names none and gives no line);
//! - answer: `card_closed` with his words or pick, of a card whose works
//!   (its `card_opened`, then his `card_moved`) hold the task;
//! - gesture: `work_changed` by `user`, `art_moved` / `card_moved` onto it
//!   (in) or off it (out, from works that held it);
//! - join: an agent's first `work_turn` with the task, and `work_created`'s
//!   maker;
//! - land: the land records (`lands.jsonl`) whose `works` hold it.
//!
//! Every line is read into its typed record (the shapes of bise_proto's
//! journal table, core/journal.rs) before it counts; a substring test on
//! the task's id is only the speed filter. Pure here: the lines come in
//! newest first ([`Scan`]), the file walk is the shell's
//! (daemon/task_log.rs), off the hub's loop.

use crate::land_record::LandRecord;
use bise_proto::core::state::{Card, Work};
use bise_proto::rows::{LogGesture, LogKind, LogLine};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// The lines a Log gives when the ask names no limit.
pub const LIMIT: usize = 50;

/// `work_changed {id, op, by, at}`, its op's keys inline (WOp.journal).
#[derive(Deserialize)]
struct Changed {
    id: String,
    op: String,
    by: String,
    at: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    into: String,
    #[serde(default)]
    why: String,
    #[serde(default)]
    n: u64,
    /// a Restart's: whom it acted on (absent on an older line)
    #[serde(default)]
    restored: Vec<String>,
    #[serde(default)]
    noted: Vec<String>,
}

/// `work_turn {turn}` (WTurn.journal)
#[derive(Deserialize)]
struct Turn {
    turn: TurnRec,
}
#[derive(Deserialize)]
struct TurnRec {
    agent: String,
    works: Vec<String>,
    at: u64,
}

/// `reported {name, report}`
#[derive(Deserialize)]
struct Reported {
    name: String,
    report: Rep,
}

/// the report a `reported` line carries (sb-core's report record, opaque
/// to the journal codec: Report's keys, every one but `kind` and
/// `summary` defaulted as an older line may lack them)
#[derive(Deserialize)]
struct Rep {
    #[serde(default)]
    at_ms: u64,
    kind: String,
    summary: String,
    #[serde(default)]
    decisions: Vec<String>,
    #[serde(default)]
    auto: bool,
    #[serde(default)]
    works: Vec<String>,
}

/// `card_opened {card}`
#[derive(Deserialize)]
struct Opened {
    card: Card,
}

/// `card_closed {id, resolution, ans}`, his answer's keys inline (Ans.journal)
#[derive(Deserialize)]
struct Closed {
    id: u64,
    #[serde(default)]
    said: String,
    #[serde(default)]
    pick: String,
    #[serde(default)]
    via: String,
}

/// `card_moved {moved: {card, works}}`, `art_moved {moved: {art, works, page}}`
#[derive(Deserialize)]
struct MovedLine {
    moved: MovedRec,
}
#[derive(Deserialize)]
struct MovedRec {
    #[serde(default)]
    card: Option<u64>,
    #[serde(default)]
    art: String,
    works: Vec<String>,
    /// main on his word (`sb task move`); "" him (an older line too)
    #[serde(default)]
    by: String,
}

/// Who a move's line names: him, or main on his word (architect m_65565:
/// never "you" for main's).
fn mover(by: String) -> String {
    if by.is_empty() {
        "user".into()
    } else {
        by
    }
}

/// `work_created {work}`
#[derive(Deserialize)]
struct Created {
    work: Work,
}

/// `lifecycle {name, lifecycle, reason, works}`: `works` the turn's a
/// stop or a failure broke (absent before the stamp, or none: no Break)
#[derive(Deserialize)]
struct Life {
    name: String,
    lifecycle: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    works: Vec<String>,
}

/// One artifact or page as the Log reads it (the stores' metas, read by
/// the shell off the loop): each version with the works it was made
/// under (`None`: a version from before the stamp, read with `works`,
/// the Meta's current ones; architect m_65507).
#[derive(Clone, Debug, Default)]
pub struct ArtLog {
    pub art: String,
    pub title: String,
    pub page: bool,
    pub works: Vec<String>,
    /// (version, at_ms, who made it, its works)
    pub versions: Vec<(u64, u64, String, Option<Vec<String>>)>,
}

/// One journal fact that may concern the task, in the journal's order.
enum Fact {
    Created(Work),
    Changed(Changed),
    Turn(TurnRec),
    Reported(String, Rep),
    Opened(Card),
    Closed(Closed, u64),
    CardMoved(u64, Vec<String>, u64, String),
    ArtMoved(String, Vec<String>, u64, String),
    Life(Life, u64),
}

#[derive(Deserialize)]
struct Head {
    #[serde(rename = "type")]
    kind: String,
}

/// A time a line carries, cheaply: the first `"at":`, `"at_ms":` or
/// `"created":` number in it (a message's, a change's, a report's). The
/// lines with none (a card's close, a move) take their newer
/// neighbour's: the hub writes them in the step that wrote it.
fn probe_time(line: &str) -> Option<u64> {
    ["\"at_ms\":", "\"at\":", "\"created\":", "\"created_ms\":"].iter().find_map(|k| {
        let i = line.find(k)? + k.len();
        let digits: String = line[i..].trim_start().chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    })
}

/// The task's facts, walked newest first: feed every journal line from
/// the end with [`Scan::line`] while it says to go on, then [`Scan::log`].
pub struct Scan {
    ids: HashSet<String>,
    quoted: Vec<String>,
    /// the facts met, newest first
    facts: Vec<Fact>,
    /// the newer neighbour's time (for a line that carries none)
    newer: u64,
    /// ids whose `work_created` is still ahead (older)
    births: HashSet<String>,
}

impl Scan {
    /// The scan of task `ids` (the task, then the ones merged into it).
    pub fn new(ids: &[String]) -> Scan {
        Scan {
            ids: ids.iter().cloned().collect(),
            quoted: ids.iter().map(|i| format!("\"{}\"", i)).collect(),
            facts: Vec::new(),
            newer: 0,
            births: ids.iter().cloned().collect(),
        }
    }

    /// One line, newer than the next one fed. False: every id's birth is
    /// behind, stop.
    pub fn line(&mut self, line: &str) -> bool {
        let named = self.quoted.iter().any(|q| line.contains(q.as_str()));
        let maybe = named || line.contains("\"card_closed\"") || line.contains("\"card_moved\"") || line.contains("\"art_moved\"");
        let time = if maybe { probe_time(line) } else { None };
        if maybe {
            if let Some(f) = self.fact(line, time.unwrap_or(self.newer)) {
                if let Fact::Created(w) = &f {
                    self.births.remove(&w.id);
                }
                self.facts.push(f);
            }
        }
        // the neighbour's time for the next (older) line that has none
        if let Some(t) = time.or_else(|| probe_time(line)) {
            self.newer = t;
        }
        !self.births.is_empty()
    }

    /// A line read into its typed record: None when it is not one the Log
    /// reads, or its typed fields don't name the task.
    fn fact(&self, line: &str, at: u64) -> Option<Fact> {
        let has = |ws: &[String]| ws.iter().any(|w| self.ids.contains(w));
        parse(line, at)?.ok().filter(|f| match f {
            Fact::Created(w) => self.ids.contains(&w.id),
            Fact::Changed(c) => self.ids.contains(&c.id),
            Fact::Turn(t) => has(&t.works),
            Fact::Reported(_, r) => !r.auto && has(&r.works),
            Fact::Life(l, _) => matches!(l.lifecycle.as_str(), "failed" | "stopped") && has(&l.works),
            // a card's or a move's works are read with the ones before it
            Fact::Opened(_) | Fact::Closed(..) | Fact::CardMoved(..) | Fact::ArtMoved(..) => true,
        })
    }

    /// The Log: the facts in the journal's order made lines, newest first,
    /// with the lands of `lands` that served the task; at most `limit`.
    pub fn log(self, lands: &[LandRecord], made: &[ArtLog], limit: usize) -> Vec<LogLine> {
        let ids = self.ids;
        let has = |ws: &[String]| ws.iter().any(|w| ids.contains(w));
        let mut out: Vec<LogLine> = Vec::new();
        let mut cards: HashMap<u64, Vec<String>> = HashMap::new();
        // a question's line, by its card: his answer folds under it
        let mut asked: HashMap<u64, usize> = HashMap::new();
        let mut arts: HashMap<String, Vec<String>> = HashMap::new();
        let mut joined: HashSet<String> = HashSet::new();
        let mut join = |out: &mut Vec<LogLine>, who: &str, at: u64| {
            if !who.is_empty() && who != "user" && joined.insert(who.to_string()) {
                out.push(LogLine { at_ms: at, kind: LogKind::Join, who: who.to_string(), ..LogLine::default() });
            }
        };
        // the newest time met so far: a line with none and no newer
        // neighbour (the journal's last) takes it
        let mut last = 0u64;
        for f in self.facts.into_iter().rev() {
            match &f {
                Fact::Created(w) => last = last.max(w.created_ms),
                Fact::Turn(t) => last = last.max(t.at),
                Fact::Changed(c) => last = last.max(c.at),
                Fact::Reported(_, r) => last = last.max(r.at_ms),
                _ => {}
            }
            let seen = last;
            let when = move |at: u64| if at == 0 { seen } else { at };
            match f {
                Fact::Created(w) => join(&mut out, &w.by, w.created_ms),
                Fact::Turn(t) => join(&mut out, &t.agent, t.at),
                Fact::Changed(c) => {
                    // his gestures; a Restart main made too, said as main's
                    // (architect m_65555: never "you")
                    if c.by != "user" && !(c.op == "restart" && c.by == crate::model::MAIN) {
                        continue;
                    }
                    let (g, text) = match c.op.as_str() {
                        "restart" => (LogGesture::Restart, String::new()),
                        "pause" => (LogGesture::Pause, String::new()),
                        "resume" => (LogGesture::Resume, String::new()),
                        "done" => (LogGesture::Done, String::new()),
                        "reopen" => (LogGesture::Reopen, String::new()),
                        "rename" => (LogGesture::Rename, c.name),
                        "merge" => (LogGesture::Merge, c.into),
                        "why" => (LogGesture::Why, c.why),
                        "goal" => (LogGesture::Goal, c.text),
                        "tick" => (LogGesture::Tick, c.n.to_string()),
                        "untick" => (LogGesture::Untick, c.n.to_string()),
                        "ongoing" => (LogGesture::Ongoing, String::new()),
                        _ => continue,
                    };
                    out.push(LogLine {
                        at_ms: c.at,
                        kind: LogKind::Gesture,
                        who: c.by,
                        text,
                        gesture: Some(g),
                        restored: c.restored,
                        noted: c.noted,
                        ..LogLine::default()
                    });
                }
                Fact::Reported(name, r) => out.push(LogLine {
                    at_ms: r.at_ms,
                    kind: LogKind::Report,
                    who: name,
                    text: r.summary.trim().to_string(),
                    report: r.kind,
                    decisions: r.decisions,
                    ..LogLine::default()
                }),
                Fact::Opened(c) => {
                    // its agents' question to him, opened with the task's works
                    if has(&c.works) && !c.text.trim().is_empty() {
                        asked.insert(c.id, out.len());
                        out.push(LogLine {
                            at_ms: when(c.created_ms),
                            kind: LogKind::Question,
                            who: c.agent.clone(),
                            text: c.text.trim().to_string(),
                            ask: c.kind.clone(),
                            card: Some(c.id),
                            ..LogLine::default()
                        });
                    }
                    cards.insert(c.id, c.works);
                }
                Fact::Closed(c, at) => {
                    let words = if c.said.trim().is_empty() { c.pick.trim().to_string() } else { c.said.trim().to_string() };
                    // his answer: his words or pick, through the inbox, a thread or a page
                    if c.via.is_empty() || words.is_empty() || !cards.get(&c.id).is_some_and(|w| has(w)) {
                        continue;
                    }
                    // under its question when the Log has it (one entry);
                    // a card moved onto the task after it was asked: alone
                    match asked.get(&c.id) {
                        Some(&i) => out[i].answer = Some(words),
                        None => out.push(LogLine { at_ms: when(at), kind: LogKind::Answer, who: "user".into(), text: words, card: Some(c.id), ..LogLine::default() }),
                    }
                }
                Fact::Life(l, at) => out.push(LogLine {
                    at_ms: when(at),
                    kind: LogKind::Break,
                    who: l.name,
                    text: l.reason.unwrap_or_default().trim().to_string(),
                    life: l.lifecycle,
                    ..LogLine::default()
                }),
                Fact::CardMoved(card, works, at, by) => {
                    let was = cards.insert(card, works.clone()).unwrap_or_default();
                    if let Some(g) = moved(has(&was), has(&works)) {
                        out.push(LogLine { at_ms: when(at), kind: LogKind::Gesture, who: mover(by), gesture: Some(g), card: Some(card), ..LogLine::default() });
                    }
                }
                Fact::ArtMoved(art, works, at, by) => {
                    let was = arts.insert(art.clone(), works.clone()).unwrap_or_default();
                    if let Some(g) = moved(has(&was), has(&works)) {
                        out.push(LogLine { at_ms: when(at), kind: LogKind::Gesture, who: mover(by), text: art, gesture: Some(g), ..LogLine::default() });
                    }
                }
            }
        }
        for l in lands.iter().filter(|l| has(&l.works)) {
            out.push(LogLine {
                at_ms: l.at_ms,
                kind: LogKind::Land,
                who: l.agent.clone(),
                sha: Some(l.tip.clone()),
                commits: l.commits.len() as u64,
                target: l.target.clone(),
                ..LogLine::default()
            });
        }
        // each version made under it (its own stamp; an older one: the
        // Meta's works); his move is its own gesture line above
        for a in made {
            for (v, at, who, works) in &a.versions {
                if has(works.as_deref().unwrap_or(&a.works)) {
                    out.push(LogLine { at_ms: *at, kind: LogKind::Artifact, who: who.clone(), text: a.title.clone(), art: a.art.clone(), v: *v, page: a.page, ..LogLine::default() });
                }
            }
        }
        // newest first; a tie keeps the journal's order, newest first too
        out.reverse();
        out.sort_by_key(|l| std::cmp::Reverse(l.at_ms));
        out.truncate(limit);
        out
    }
}

/// A journal line of a kind the Log reads, as its typed record: None, a
/// kind it doesn't read; Some(Err), one it reads that doesn't parse (a
/// renamed field: the pin test goes red, architect m_65437). `at`: its
/// time when the line carries none.
fn parse(line: &str, at: u64) -> Option<Result<Fact, serde_json::Error>> {
    let kind = serde_json::from_str::<Head>(line).ok()?.kind;
    let r = match kind.as_str() {
        "work_created" => serde_json::from_str::<Created>(line).map(|c| Fact::Created(c.work)),
        "work_changed" => serde_json::from_str::<Changed>(line).map(Fact::Changed),
        "work_turn" => serde_json::from_str::<Turn>(line).map(|t| Fact::Turn(t.turn)),
        "reported" => serde_json::from_str::<Reported>(line).map(|r| Fact::Reported(r.name, r.report)),
        "card_opened" => serde_json::from_str::<Opened>(line).map(|o| Fact::Opened(o.card)),
        "card_closed" => serde_json::from_str::<Closed>(line).map(|c| Fact::Closed(c, at)),
        // a card's move names its card; an artifact's or a page's its `art`
        "card_moved" => serde_json::from_str::<MovedLine>(line).map(|m| Fact::CardMoved(m.moved.card.unwrap_or_default(), m.moved.works, at, m.moved.by)),
        "art_moved" => serde_json::from_str::<MovedLine>(line).map(|m| Fact::ArtMoved(m.moved.art, m.moved.works, at, m.moved.by)),
        "lifecycle" => serde_json::from_str::<Life>(line).map(|l| Fact::Life(l, at)),
        _ => return None,
    };
    Some(r)
}

/// The journal kinds the Log reads.
pub const KINDS: [&str; 9] = ["work_created", "work_changed", "work_turn", "reported", "card_opened", "card_closed", "card_moved", "art_moved", "lifecycle"];

/// A line of `KINDS` that the Log's records read (the pin test's).
pub fn reads(line: &str) -> Result<(), String> {
    match parse(line, 0) {
        Some(Ok(_)) => Ok(()),
        Some(Err(e)) => Err(e.to_string()),
        None => Err("not a kind the Log reads".into()),
    }
}

/// A move's gesture: onto the task (in) or off it (out); none when it
/// neither came nor went.
fn moved(was: bool, now: bool) -> Option<LogGesture> {
    match (was, now) {
        (false, true) => Some(LogGesture::MoveIn),
        (true, false) => Some(LogGesture::MoveOut),
        _ => None,
    }
}

/// The Log of task `ids` from journal lines newest first.
pub fn of_lines<'a>(ids: &[String], lines: impl IntoIterator<Item = &'a str>, lands: &[LandRecord], limit: usize) -> Vec<LogLine> {
    let mut s = Scan::new(ids);
    for l in lines {
        if !s.line(l) {
            break;
        }
    }
    s.log(lands, &[], limit)
}

/// Back-to-back versions of one artifact by one agent folded into one
/// line (the window folds the same: "v2→v5"): each line with the oldest
/// version of its run, newest first like the Log.
pub fn fold(lines: &[LogLine]) -> Vec<(&LogLine, u64)> {
    let mut out: Vec<(&LogLine, u64)> = Vec::new();
    for l in lines {
        if let Some((prev, from)) = out.last_mut() {
            if l.kind == LogKind::Artifact && prev.kind == LogKind::Artifact && prev.art == l.art && prev.page == l.page && prev.who == l.who {
                *from = l.v;
                continue;
            }
        }
        out.push((l, l.v));
    }
    out
}

/// `sb task show --log`'s lines: one per fact (back-to-back versions of
/// one artifact folded), newest first, `when` putting a time in words.
pub fn words(lines: &[LogLine], when: impl Fn(u64) -> String) -> Vec<String> {
    use bise_proto::words::tasks;
    fold(lines)
        .into_iter()
        .map(|(l, from)| {
            let w = when(l.at_ms);
            let what = match l.kind {
                LogKind::Question => {
                    let mut s = tasks::log_asked(&l.who, &l.text);
                    if let Some(a) = &l.answer {
                        s.push(char::from(10u8));
                        s.push_str("    ");
                        s.push_str(&tasks::log_answer(a));
                    }
                    s
                }
                LogKind::Artifact if from < l.v => tasks::log_made_range(&l.who, &l.text, from, l.v),
                LogKind::Artifact if l.v > 1 => tasks::log_updated(&l.who, &l.text, l.v),
                LogKind::Artifact => tasks::log_made(&l.who, &l.text),
                LogKind::Break => break_words(l),
                LogKind::Report => {
                    let mut s = tasks::log_report(&l.who, &l.report, &l.text);
                    // its decisions under it, indented
                    for d in &l.decisions {
                        s.push(char::from(10u8));
                        s.push_str("    ");
                        s.push_str(&tasks::log_decision(d));
                    }
                    s
                }
                LogKind::Answer => tasks::log_answer(&l.text),
                LogKind::Land => tasks::log_land(&l.who, l.commits, &l.target, short(l.sha.as_deref().unwrap_or_default())),
                LogKind::Join => tasks::log_join(&l.who),
                LogKind::Gesture => gesture_words(l),
                LogKind::Unknown => String::new(),
            };
            format!("{}  {}", w, what)
        })
        .collect()
}

/// A break in words: a crash says so once, 'auth stopped (crash): <why>', never 'crash:' twice
/// (designer m_65569); a stop, 'docs was stopped: <why>'.
fn break_words(l: &LogLine) -> String {
    use bise_proto::words::tasks;
    let why = l.text.trim();
    let (crash, why) = match why.strip_prefix("crash:") {
        Some(rest) => (true, rest.trim()),
        None => (l.life == "failed", why),
    };
    if crash {
        tasks::log_crashed(&l.who, why)
    } else {
        tasks::log_stopped(&l.who, why)
    }
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(9)]
}

/// A Restart that names whom it acted on: 'you restarted auth, kb and
/// asked ui to go on' (either part alone when the other is empty).
fn restart_words(who: &str, restored: &str, noted: &str) -> String {
    use bise_proto::words::tasks;
    match (restored.is_empty(), noted.is_empty()) {
        (false, false) => tasks::log_restarted_and_asked(who, restored, noted),
        (false, true) => tasks::log_restarted_whom(who, restored),
        _ => tasks::log_asked_on(who, noted),
    }
}

/// His gesture in words (the window has the same words, words::tasks).
pub fn gesture_words(l: &LogLine) -> String {
    use bise_proto::words::tasks;
    let t = l.text.as_str();
    match l.gesture.unwrap_or_default() {
        LogGesture::Pause => tasks::log_paused.to_string(),
        LogGesture::Resume => tasks::log_resumed.to_string(),
        LogGesture::Done => tasks::log_done.to_string(),
        LogGesture::Reopen => tasks::log_reopened.to_string(),
        LogGesture::Rename => tasks::log_renamed(t),
        LogGesture::Merge => tasks::log_merged(t),
        LogGesture::Why => tasks::log_why(t),
        LogGesture::Goal => tasks::log_goal(t),
        LogGesture::Tick => tasks::log_ticked(t),
        LogGesture::Untick => tasks::log_unticked(t),
        LogGesture::Ongoing => tasks::log_ongoing.to_string(),
        // main's move on his word is said as main's (architect m_65565)
        LogGesture::MoveIn if l.who != "user" => tasks::log_moved_in_by(&l.who, if t.is_empty() { tasks::log_a_card } else { t }),
        LogGesture::MoveOut if l.who != "user" => tasks::log_moved_out_by(&l.who, if t.is_empty() { tasks::log_a_card } else { t }),
        LogGesture::MoveIn => tasks::log_moved_in(if t.is_empty() { tasks::log_a_card } else { t }),
        LogGesture::MoveOut => tasks::log_moved_out(if t.is_empty() { tasks::log_a_card } else { t }),
        // whom it acted on, as its line recorded it (architect m_65602,
        // tasks-ux m_65604); an older line names no one
        LogGesture::Restart if !l.restored.is_empty() || !l.noted.is_empty() => {
            let who = if l.who == "user" { tasks::log_you } else { l.who.as_str() };
            restart_words(who, &l.restored.join(", "), &l.noted.join(", "))
        }
        // main's Restart is said as main's, never his (architect m_65555)
        LogGesture::Restart if l.who != "user" => tasks::log_restarted_by(&l.who),
        LogGesture::Restart => tasks::log_restarted.to_string(),
        LogGesture::Unknown => String::new(),
    }
}

#[cfg(test)]
#[path = "task_log_tests.rs"]
mod tests;
