//! His last messages in a thread (`thread/marks`, msg-dots): the desktop's
//! strip of ticks beside a thread, one per message of his, needs where his
//! last ones are without holding the thread. Which entry is his and the words
//! a tick shows; the hub's transcript walker (the one ⌘F's `thread/find`
//! runs, backward here) applies it from the transcript's end. Pure.
//!
//! His message is an [`EntryKind::You`] entry, by its line's type: a task's
//! brief comes as a message from main (`sb msg-in`, a `from_agent` entry),
//! never his.

use super::{Entry, EntryKind, Pos};

/// Marks one `thread_marks` carries at most.
pub const MAX_MARKS: usize = 50;
/// A mark's words at most (chars): the strip's bubble shows three lines of them.
pub const TEXT_MAX: usize = 200;

/// One message of his: its entry's pos, when he sent it, his words cut short.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Mark {
    pub pos: Pos,
    pub at_ms: u64,
    pub text: String,
}

/// His words on one line (runs of whitespace folded), at most [`TEXT_MAX`]
/// chars, cut at a word with '…' when longer.
pub fn short(text: &str) -> String {
    let one = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= TEXT_MAX {
        return one;
    }
    let cut: String = one.chars().take(TEXT_MAX).collect();
    let at_word = cut.rfind(' ').filter(|&i| i > TEXT_MAX / 2).map_or(cut.as_str(), |i| &cut[..i]);
    format!("{}…", at_word.trim_end())
}

/// The entry's mark when it is a message of his.
pub fn mark_of(e: &Entry) -> Option<Mark> {
    (e.kind == EntryKind::You).then(|| Mark { pos: e.pos, at_ms: e.at_ms, text: short(&e.text) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thread::{fold, Attached, Ctx, Line};

    fn folded(lines: &[Line]) -> Vec<Entry> {
        let page = |_: &str| None;
        let ctx = Ctx { open_cards: &[], page: &page, provider: &|id: &str, _: &str| id.to_string(), width: &|s: &str| s.chars().count(), offset: &|_| 0, attached: &Attached::plain };
        fold(lines, &ctx)
    }

    #[test]
    fn his_messages_only_and_a_brief_is_mains() {
        let lines: Vec<Line> = vec![
            (1, 10, "sb msg-in :: {\"body\":\"# Task `x`\\n\\nObjective: go\",\"from\":\"main\",\"id\":\"m_1\"}".into()),
            (2, 20, "sb you : make it\n".into()),
            (3, 30, "  obs: assistant: on it".into()),
            (4, 40, "sb msg-in : peer m_3 : nice".into()),
            (5, 50, "sb you : and faster".into()),
        ];
        let marks: Vec<Mark> = folded(&lines).iter().filter_map(mark_of).collect();
        assert_eq!(marks, vec![Mark { pos: 2, at_ms: 20, text: "make it".into() }, Mark { pos: 5, at_ms: 50, text: "and faster".into() }]);
    }

    #[test]
    fn short_folds_whitespace_and_cuts_at_a_word() {
        assert_eq!(short("a\n\n  b\tc"), "a b c");
        let s = short(&"word ".repeat(100));
        assert!(s.ends_with("word…"), "{s}");
        assert!(s.chars().count() <= TEXT_MAX + 1);
        assert_eq!(short(&"x".repeat(300)).chars().count(), TEXT_MAX + 1);
    }
}
