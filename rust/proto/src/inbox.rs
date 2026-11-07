//! The inbox's history (inbox-record, architect m_24257): one record per
//! card he answered or that was withdrawn, read from sb-core's journal.
//!
//! - [`journal`]: the typed reader of the journal's card lines
//!   (`card_opened`, `card_closed` with his answer, `card_answer_pos`),
//!   declared in `core::journal::JOURNAL` (a test keeps both in step);
//! - [`Index`]: the fold of those lines into [`InboxRecord`]s, pure. The
//!   hub runs it at its replay and on each line sb-core journals; the
//!   desktop core runs it on a stopped project's journal. Same lines, same
//!   records;
//! - [`Index::page`]: the paged, filtered list (`inbox/history`), newest
//!   first. Paging by `before`, a card id: ids only grow, so the cards
//!   before one are the older ones;
//! - [`words`]: the row's words (where he answered, a withdrawn reason, a
//!   day's head), so the terminal and the desktop say the same.
//!
//! History is a read model, not a decision: sb-core keeps no closed card
//! (issue 21); it only writes the lines.

pub mod journal;
pub mod words;

#[cfg(test)]
mod tests;

use crate::Project;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// What became of a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum InboxState {
    /// he answered it
    Answered,
    /// it closed without him: taken back, its agent stopped, closed
    Withdrawn,
    #[serde(other)]
    Unknown,
}

/// The card's kind, as the hub opened it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum InboxKind {
    /// an agent's question (`sb card`, a page question)
    Question,
    /// main asks to drop a task
    Drop,
    /// a tool call waiting for his yes
    Confirm,
    /// one of the hub's items (a merge, a feature, an update, a sign-in)
    Item,
    #[serde(other)]
    Unknown,
}

impl InboxKind {
    pub fn of(kind: &str) -> InboxKind {
        match kind {
            "question" => InboxKind::Question,
            "drop" => InboxKind::Drop,
            "confirm" => InboxKind::Confirm,
            "merge" | "feature_try" | "feature_merge" | "signin" => InboxKind::Item,
            _ => InboxKind::Unknown,
        }
    }
}

/// Where he answered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Where {
    /// the inbox (the terminal's or the desktop's)
    Inbox,
    /// his words to the asker in its own thread
    Thread { agent: String },
    /// a bise page's question
    Page {
        page: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        block: String,
    },
    #[serde(other)]
    Unknown,
}

/// Where his answer sits in a thread: the transcript position of the
/// line it became (`thread/page`'s `pos`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AnswerAt {
    pub agent: String,
    pub pos: u64,
}

/// One answered or withdrawn card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InboxRecord {
    /// the card's id in its hub
    pub card: u64,
    pub project: Project,
    /// the agent that asked
    pub asker: String,
    pub kind: InboxKind,
    /// the question without its options
    pub question: String,
    /// its options' words, in order ([] for free words)
    pub options: Vec<String>,
    pub state: InboxState,
    /// answered: his words, the option's words when he picked one by
    /// its number (never the digit alone)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
    /// the option's number when he picked one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick_n: Option<u64>,
    /// answered: where
    #[serde(default, rename = "where", skip_serializing_if = "Option::is_none")]
    pub at: Option<Where>,
    /// withdrawn: why, as the hub said it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// when the card opened
    pub asked_ms: u64,
    /// when it closed
    pub at_ms: u64,
    /// his answer's line in the asker's thread; absent: none was written
    /// (an approval, a page's answer) or a journal older than it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<AnswerAt>,
    /// the row's second line, from [`words::second_line`] (`you: avif ·
    /// in the inbox`, `withdrawn: moot`): the terminal and the desktop
    /// draw these words, never their own
    #[serde(default)]
    pub line2: String,
    /// what the question was about (its card's link as it opened: `sb
    /// notify --artifact`, a page's question): the desktop's chip under
    /// it; none for a card without one, or journaled before cards kept it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<crate::rows::ArtLink>,
}

/// Which records a list wants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum StateFilter {
    #[default]
    All,
    Answered,
    Withdrawn,
}

/// A list's filters and page (`inbox/history`'s params).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    /// only this asker's cards
    pub agent: Option<String>,
    pub state: StateFilter,
    /// every word in the question, his words, the asker or the reason
    /// (any case)
    pub words: Option<String>,
    /// only cards before this id (the next page)
    pub before: Option<u64>,
    pub limit: usize,
}

/// `inbox/history`'s params: [`Query`] on the wire, every field optional.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HistoryParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<StateFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<String>,
    /// a card id: only the older ones (the next page)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<u64>,
    /// at most [`PAGE_MAX`]; none: [`PAGE`]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

impl HistoryParams {
    pub fn query(&self) -> Query {
        Query {
            agent: self.agent.clone().filter(|a| !a.is_empty()),
            state: self.state.unwrap_or_default(),
            words: self.words.clone().filter(|w| !w.trim().is_empty()),
            before: self.before,
            limit: self.limit.unwrap_or(0) as usize,
        }
    }
}

/// The default and the largest page.
pub const PAGE: usize = 50;
pub const PAGE_MAX: usize = 200;

/// One page of the history.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InboxPage {
    /// newest first
    pub items: Vec<InboxRecord>,
    /// older records match too: ask again with `before` the last card
    pub more: bool,
    /// every record this hub has (the filters left aside): the head's
    /// `41 answered`
    pub answered: u64,
    pub withdrawn: u64,
}

/// An open card, until it closes: what its record needs.
#[derive(Clone, Debug, PartialEq)]
struct Open {
    kind: String,
    agent: String,
    text: String,
    created_ms: u64,
    about: Option<crate::rows::ArtLink>,
}

/// The fold of the journal's card lines.
#[derive(Clone, Debug, Default)]
pub struct Index {
    project: Project,
    open: BTreeMap<u64, Open>,
    done: BTreeMap<u64, InboxRecord>,
    /// the latest time a line read so far carried (ms): a close without `ts` is then
    last_ms: u64,
}

/// The time a journal line carries, in ms; 0: none. Its own `ts`, `at` or
/// `at_ms`, else its card's `created_ms` or its report's `at_ms`. Pure.
fn line_ms(line: &Value) -> u64 {
    let ms = |v: Option<&Value>, k: &str| v.and_then(|v| v.get(k)).and_then(Value::as_u64);
    ms(Some(line), "ts")
        .or_else(|| ms(Some(line), "at_ms"))
        .or_else(|| ms(Some(line), "at"))
        .or_else(|| ms(line.get("card"), "created_ms"))
        .or_else(|| ms(line.get("report"), "at_ms"))
        .unwrap_or(0)
}

impl Index {
    pub fn new(project: Project) -> Index {
        Index { project, ..Index::default() }
    }

    /// One journal line. A close's time is its line's `ts` when the hub
    /// stamped one; sb-core writes `card_closed` without any time, so
    /// else the journal's latest time before it (any line's `ts`, `at`,
    /// `at_ms`, a card's `created_ms`, a report's `at_ms`), never before
    /// the card opened: never 0, which read as jan 1 1970. The card
    /// whose record it made or changed, if any.
    pub fn read(&mut self, line: &Value) -> Option<u64> {
        let ts = line.get("ts").and_then(Value::as_u64);
        self.last_ms = self.last_ms.max(line_ms(line));
        match journal::CardLine::of(line)? {
            journal::CardLine::Opened(o) => {
                let c = o.card;
                self.open.insert(c.id, Open { kind: c.kind, agent: c.agent, text: c.text, created_ms: c.created_ms, about: c.about });
                None
            }
            journal::CardLine::Closed(c) => {
                let o = self.open.remove(&c.id)?;
                let at = ts.unwrap_or(self.last_ms).max(o.created_ms);
                let r = record(&self.project, c, o, at);
                let id = r.card;
                self.done.insert(id, r);
                Some(id)
            }
            journal::CardLine::AnswerPos(p) => {
                let r = self.done.get_mut(&p.card)?;
                if r.thread.is_some() || r.state != InboxState::Answered {
                    return None;
                }
                r.thread = Some(AnswerAt { agent: p.agent, pos: p.pos });
                Some(p.card)
            }
        }
    }

    /// Every line of a journal, in order.
    pub fn read_all<'a>(&mut self, lines: impl IntoIterator<Item = &'a Value>) {
        for l in lines {
            self.read(l);
        }
    }

    /// The record of card `id`.
    pub fn get(&self, id: u64) -> Option<&InboxRecord> {
        self.done.get(&id)
    }

    pub fn len(&self) -> usize {
        self.done.len()
    }

    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }

    /// One page of the records `q` wants, newest first.
    pub fn page(&self, q: &Query) -> InboxPage {
        let limit = if q.limit == 0 { PAGE } else { q.limit.min(PAGE_MAX) };
        let words = q.words.as_deref().map(words::terms).unwrap_or_default();
        let upto = q.before.unwrap_or(u64::MAX);
        let mut fit = self.done.range(..upto).rev().map(|(_, r)| r).filter(|r| wanted(r, q, &words));
        let items: Vec<InboxRecord> = fit.by_ref().take(limit).cloned().collect();
        let more = fit.next().is_some();
        let count = |s| self.done.values().filter(|r| r.state == s).count() as u64;
        InboxPage { items, more, answered: count(InboxState::Answered), withdrawn: count(InboxState::Withdrawn) }
    }
}

fn wanted(r: &InboxRecord, q: &Query, terms: &[String]) -> bool {
    let state = match q.state {
        StateFilter::All => true,
        StateFilter::Answered => r.state == InboxState::Answered,
        StateFilter::Withdrawn => r.state == InboxState::Withdrawn,
    };
    state && q.agent.as_deref().is_none_or(|a| r.asker == a) && words::matches(r, terms)
}

/// A closed card's record: answered when the close carries his answer.
fn record(project: &Project, c: journal::CardClosed, o: Open, ts: u64) -> InboxRecord {
    let (question, options) = crate::rows::split_choices(&o.text);
    let answered = !c.via.is_empty();
    let at = answered.then(|| match c.via.as_str() {
        "inbox" => Where::Inbox,
        "thread" => Where::Thread { agent: o.agent.clone() },
        "page" => Where::Page { page: c.page.clone(), block: c.block.clone() },
        _ => Where::Unknown,
    });
    // his words: the option's when he picked one by its number
    let said = answered.then(|| if c.pick.is_empty() { c.said.clone() } else { c.pick.clone() });
    let mut r = InboxRecord {
        card: c.id,
        project: project.clone(),
        asker: o.agent,
        kind: InboxKind::of(&o.kind),
        question: question.trim().to_string(),
        options,
        state: if answered { InboxState::Answered } else { InboxState::Withdrawn },
        said,
        pick_n: c.pick_n.filter(|_| answered && !c.pick.is_empty()),
        at,
        reason: (!answered).then_some(c.resolution),
        asked_ms: o.created_ms,
        at_ms: ts,
        thread: None,
        line2: String::new(),
        about: o.about,
    };
    r.line2 = words::second_line(&r);
    r
}
