//! Tests: threads fed to the terminal as the hub feeds them since
//! client-protocol step 4 (P4d): their lines folded by bise_proto's one
//! fold, the first page as `thread/subscribe`'s answer, then every entry
//! a new line made or changed as a `thread/entry` (the hub sends an entry
//! again when it changes), older entries as `thread/page`'s answer. The
//! tests that wrote `line` and `history` events write lines here.

use super::feed_entries;
use super::*;
use bise_proto::thread::{fold, Attached, Ctx, Entry, Line};

/// Lines folded again from an entry start at least this far back: older
/// entries no longer change (as the hub's heads.rs keeps one entry).
const TAIL: usize = 300;

/// The hub's fold of `lines` as the tests' hub has it: no card open, no
/// page known, words as wide as their chars.
pub(crate) fn fold_lines(lines: &[Line]) -> Vec<Entry> {
    let none = |_: &str| None;
    let ctx = Ctx {
        open_cards: &[],
        page: &none,
        provider: &|_: &str, k: &str| k.to_string(),
        width: &|s: &str| unicode_width::UnicodeWidthStr::width(s),
        offset: &|_| 0,
        attached: &Attached::plain,
    };
    fold(lines, &ctx)
}

/// One thread as the hub holds it: its lines, the entries it sent.
#[derive(Default)]
struct Thread {
    lines: Vec<Line>,
    sent: Vec<Entry>,
}

/// The tests' hub: each agent's thread.
#[derive(Default)]
pub(crate) struct Hub {
    threads: HashMap<String, Thread>,
}

impl Hub {
    pub(crate) fn new() -> Hub {
        Hub::default()
    }

    /// `agent`'s thread is subscribed with what it holds now (its newest
    /// entries; `more`: older ones are left for pages).
    pub(crate) fn subscribe(&mut self, app: &mut App, agent: &str, more: bool) {
        let t = self.threads.entry(agent.to_string()).or_default();
        app.sb.subscribed.insert(agent.to_string());
        t.sent = fold_lines(&t.lines);
        feed_entries::page(app, agent, None, t.sent.clone(), more);
    }

    /// Lines `agent`'s thread had before the terminal came (no event):
    /// the next [`Hub::subscribe`] sends them.
    pub(crate) fn had(&mut self, agent: &str, lines: impl IntoIterator<Item = Line>) {
        self.threads.entry(agent.to_string()).or_default().lines.extend(lines);
    }

    /// One more line of `agent`'s thread, after its last one, untimed.
    pub(crate) fn line(&mut self, app: &mut App, agent: &str, line: &str) {
        let pos = self.threads.get(agent).and_then(|t| t.lines.last()).map_or(1, |l| l.0 + 1);
        self.line_at(app, agent, pos, 0, line);
    }

    /// Lines of `agent`'s thread, one at a time.
    pub(crate) fn lines(&mut self, app: &mut App, agent: &str, lines: &[&str]) {
        for l in lines {
            self.line(app, agent, l);
        }
    }

    /// Line `pos` of `agent`'s thread at `ts` (ms, 0 untimed): the entries
    /// it made or changed go to the terminal, live (its thread subscribed
    /// first when it was not).
    pub(crate) fn line_at(&mut self, app: &mut App, agent: &str, pos: u64, ts: u64, line: &str) {
        if !app.sb.subscribed.contains(agent) {
            self.subscribe(app, agent, false);
        }
        let t = self.threads.entry(agent.to_string()).or_default();
        t.lines.push((pos, ts, line.to_string()));
        // fold again from an entry start TAIL lines back
        let back = t.lines.len().saturating_sub(TAIL);
        let from = t.sent.iter().rev().map(|e| e.pos).find(|p| t.lines.get(back).is_some_and(|l| *p <= l.0));
        let start = from.map_or(0, |p| t.lines.iter().position(|l| l.0 >= p).unwrap_or(0));
        let kept = from.map_or(0, |p| t.sent.iter().position(|e| e.pos >= p).unwrap_or(t.sent.len()));
        let tail = fold_lines(&t.lines[start..]);
        let changed: Vec<Entry> = tail.iter().filter(|e| !t.sent[kept..].contains(e)).cloned().collect();
        t.sent.truncate(kept);
        t.sent.extend(tail);
        for e in &changed {
            feed_entries::entry(app, agent, e);
        }
    }

    /// `thread/page`'s answer: the entries of `lines` (older than
    /// `before`, what the terminal asked), `more` when some are older still.
    pub(crate) fn page(&mut self, app: &mut App, agent: &str, before: usize, lines: Vec<Line>, more: bool) {
        feed_entries::page(app, agent, Some(before), fold_lines(&lines), more);
    }
}

/// `agent`'s lines in a fresh hub, one at a time (its thread subscribed
/// at the first): for a test that writes one thread once.
pub(crate) fn lines(app: &mut App, agent: &str, lines: &[&str]) -> Hub {
    let mut hub = Hub::new();
    hub.lines(app, agent, lines);
    hub
}

/// `agents`' threads count as subscribed (their entries are placed), with
/// nothing asked of the hub.
pub(crate) fn subscribed(app: &mut App, agents: &[&str]) {
    app.sb.subscribed.extend(agents.iter().map(|a| a.to_string()));
}

/// The `thread/entry` notification of `line` alone at `pos` in `agent`'s
/// thread (the fuzz: any line, a pos that may be known); a line that
/// folds to no entry is a `line` event, which the terminal skips.
pub(crate) fn entry_note(agent: &str, pos: u64, line: &str) -> String {
    use bise_proto::{hub::HubEv, rpc};
    match fold_lines(&[(pos, 0, line.to_string())]).into_iter().next() {
        Some(e) => {
            let ev = HubEv::Entry { project: "p".into(), agent: agent.into(), entry: Box::new(e) };
            rpc::Message::Notification(rpc::note(&ev, None).expect("an entry note")).to_value().to_string()
        }
        None => json!({"ev": "line", "agent": agent, "line": line}).to_string(),
    }
}
