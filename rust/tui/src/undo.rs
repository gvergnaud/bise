//! The composer's undo and redo stacks (editor.rs owns one): the state
//! before each step, grouped the way editors do. A run of typed chars is
//! one step per word, a run of backspaces one step, a run of voice deltas
//! one step, a run of history recalls one step; a paste, a word or line
//! delete, a cut, a quote or a completion is a step on its own. A state
//! keeps the text, the cursor, the selection and the history entry shown,
//! so an undo puts all of them back. A new step drops the redo; sending
//! drops both (the editor starts over).
//!
//! Pure: no keys, no terminal. The key map is `editor::action` (ctrl+z,
//! ctrl+shift+z, cmd+z, ctrl+/); ctrl+y redoes in input.rs.

/// An editor state an undo or a redo goes back to.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Snap {
    pub(crate) text: String,
    pub(crate) cursor: usize,
    /// the other end of the selection
    pub(crate) anchor: Option<usize>,
    /// the history entry shown (None = the user's own draft)
    pub(crate) hist: Option<usize>,
}

/// What a step groups: consecutive edits of one kind merge (not `Other`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Typing,
    Deleting,
    Voice,
    Recall,
    Other,
}

/// The most steps kept: the oldest go first.
const CAP: usize = 200;

#[derive(Debug, Default, Clone)]
pub(crate) struct Steps {
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last: Option<Kind>,
}

impl Steps {
    /// Before an edit of `kind`: keeps `before` as a new step, unless the
    /// edit continues the current group (same kind, not `Other`, not a
    /// typed word start). Drops the redo either way.
    pub(crate) fn record(&mut self, kind: Kind, word_start: bool, before: impl FnOnce() -> Snap) {
        let merge = self.last == Some(kind) && kind != Kind::Other && !word_start;
        if !merge {
            self.push(before());
        }
        self.redo.clear();
        self.last = Some(kind);
    }

    /// A step on its own (`before`, the state it undoes to); ends the group.
    pub(crate) fn push(&mut self, before: Snap) {
        self.undo.push(before);
        if self.undo.len() > CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.last = None;
    }

    /// Ends the current group (a move, a click, a pause in the voice).
    pub(crate) fn end_group(&mut self) {
        self.last = None;
    }

    /// The group the next edit continues (a replaced selection types on).
    pub(crate) fn continue_as(&mut self, kind: Kind) {
        self.last = Some(kind);
    }

    /// The state to go back to; `now` waits for a redo.
    pub(crate) fn undo(&mut self, now: Snap) -> Option<Snap> {
        let s = self.undo.pop()?;
        self.redo.push(now);
        self.last = None;
        Some(s)
    }

    /// The state an undo left; `now` waits for an undo again.
    pub(crate) fn redo(&mut self, now: Snap) -> Option<Snap> {
        let s = self.redo.pop()?;
        self.undo.push(now);
        self.last = None;
        Some(s)
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Every kept state (the voice chip leaves them all, editor.rs).
    pub(crate) fn states_mut(&mut self) -> impl Iterator<Item = &mut Snap> {
        self.undo.iter_mut().chain(self.redo.iter_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: &str) -> Snap {
        Snap { text: t.into(), cursor: t.chars().count(), ..Default::default() }
    }

    #[test]
    fn a_kind_merges_until_the_group_ends() {
        let mut st = Steps::default();
        st.record(Kind::Typing, false, || s(""));
        st.record(Kind::Typing, false, || s("a"));
        st.record(Kind::Deleting, false, || s("ab"));
        st.end_group();
        st.record(Kind::Deleting, false, || s("a"));
        st.record(Kind::Other, false, || s(""));
        st.record(Kind::Other, false, || s("x"));
        let undone: Vec<_> = std::iter::from_fn(|| st.undo(s("now")).map(|x| x.text)).collect();
        assert_eq!(undone, ["x", "", "a", "ab", ""]);
    }

    #[test]
    fn redo_replays_and_a_new_step_drops_it() {
        let mut st = Steps::default();
        st.push(s(""));
        assert_eq!(st.undo(s("a")).unwrap().text, "");
        assert!(st.can_redo());
        assert_eq!(st.redo(s("")).unwrap().text, "a");
        assert_eq!(st.undo(s("a")).unwrap().text, "");
        st.record(Kind::Typing, false, || s(""));
        assert!(!st.can_redo());
    }

    #[test]
    fn keeps_the_last_two_hundred() {
        let mut st = Steps::default();
        for i in 0..250 {
            st.push(s(&i.to_string()));
        }
        assert_eq!(std::iter::from_fn(|| st.undo(s("n"))).count(), CAP);
    }
}
