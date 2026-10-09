//! Scheduled tasks (site/m/timers): what `sb every` sets, as the user
//! reads it. hub/scheduled carries them (its items, the active ones, then
//! its ended ones of the week: sb/state_rows.rs, P4c-4b); the feeds carry their ◷ lines (the hub's
//! `scheduled : <json>` when one is set or ends, and each run, a
//! `msg-in` from bise whose text is the wake the agent reads). The words
//! are designer's (site/m/timers 'v1 and the words'): "scheduled task"
//! for one, "scheduled" for the lines and the screen, "run" for a wake.
//! The full screen is `scheduled_screen.rs`.

use crate::wire::Ev;

// one parser and one set of words with the hub's thread fold (batch 3b):
// bise_proto::thread::scheduled; the TUI passes its own clock
pub(crate) use bise_proto::thread::scheduled::{clip, Task};

/// A time to come on the real clock: `14:22`, `tomorrow 07:30`.
pub(crate) fn ahead(ms: u64, now: u64) -> String {
    bise_proto::thread::scheduled::ahead(ms, now, &crate::when::offset_at)
}

/// How many so far, or until when, on the real clock: `2 of 6`, `until 18:00`.
pub(crate) fn so_far(t: &Task, now: u64) -> String {
    t.so_far(now, &crate::when::offset_at)
}

/// `in 1m`, `in 1h 5m`, `in 2d` (`now` when due).
pub(crate) fn countdown(ms: u64, now: u64) -> String {
    let s = ms.saturating_sub(now) / 1000;
    let (d, h, m) = (s / 86_400, s % 86_400 / 3600, s % 3600 / 60);
    match (d, h, m) {
        (0, 0, 0) => "now".into(),
        (0, 0, m) => format!("in {m}m"),
        (0, h, 0) => format!("in {h}h"),
        (0, h, m) => format!("in {h}h {m}m"),
        (d, _, _) => format!("in {d}d"),
    }
}

fn ev(l: bise_proto::thread::Scheduled) -> Ev {
    Ev::Scheduled { head: l.head, words: l.words, open: false }
}

/// The hub's `scheduled : <json>` line: one set (`ev: set`) or ended
/// (`ev: end`); main's copy of another agent's (`in: main`) names it.
pub(crate) fn hub_line(raw: &str) -> Option<Ev> {
    hub_line_at(raw, crate::when::now_ms())
}

pub(crate) fn hub_line_at(raw: &str, now: u64) -> Option<Ev> {
    bise_proto::thread::scheduled::hub_line(raw, now, &crate::when::offset_at).map(ev)
}

/// The hub's own sender (its id `switchboard`, or `bise` as shown).
pub(crate) fn is_hub(from: &str) -> bool {
    bise_proto::thread::lines::is_hub_sender(from)
}

/// A run: the wake an agent reads from bise as its ◷ line
/// (bise_proto::thread::scheduled::run_line). None: not a wake.
pub(crate) fn run_line(text: &str) -> Option<Ev> {
    bise_proto::thread::scheduled::run_line(text).map(ev)
}

/// The note bise sends an agent when you stop its scheduled task: for
/// the agent only (its ◷ ended line says it in the thread).
pub(crate) fn is_stop_note(text: &str) -> bool {
    bise_proto::thread::lines::is_stop_note(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_790_000_000_000;

    /// The TUI's ◷ lines are bise_proto's words (the words' own tests are
    /// there): a run and a set line come out as `Ev::Scheduled`.
    #[test]
    fn the_lines_are_the_shared_words() {
        let run = "timer #48 \"build check\" (every 2m, 2/6, set by answer-line): check the build\n(stop it: sb every --stop 48)";
        assert!(matches!(run_line(run), Some(Ev::Scheduled { head, words, open: false }) if head == "build check · 2 of 6" && words == "check the build"));
        let set = r#"{"ev":"set","id":3,"agent":"a","by":"a","label":"every 2m","text":"x","next_ms":1790000120000}"#;
        assert!(matches!(hub_line_at(set, NOW), Some(Ev::Scheduled { head, .. }) if head.starts_with("a scheduled x · every 2m · next ")));
        assert!(run_line("hello").is_none());
        assert!(is_stop_note("the user stopped timer #3 (x): don't set it again unless they ask"));
    }

    #[test]
    fn countdowns() {
        assert_eq!(countdown(NOW + 61_000, NOW), "in 1m");
        assert_eq!(countdown(NOW + 3_900_000, NOW), "in 1h 5m");
        assert_eq!(countdown(NOW, NOW), "now");
    }
}
