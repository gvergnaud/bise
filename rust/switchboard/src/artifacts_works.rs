//! tasks (docs/tasks.md; architect m_64425, m_64740): the works an
//! artifact serves, on its Meta. The single writer is this type: its
//! fields are private, so the compiler is the guard, and its only
//! mutators are the two writes sb-core's facts make.
//!
//! - [`ArtWorks::stamp`], at the door (an agent's add or new version,
//!   `daemon/art.rs`): the union of what it served with the works the
//!   agent's turn serves (sb-core's view, `turn_works`). A union on each
//!   new version is the rule: a doc made under A and revised under B
//!   serves both. His own add stamps nothing.
//! - [`ArtWorks::moved_to`], from sb-core's `art_works` effect (his "Move
//!   to task…", journaled as `art_moved`): exactly his works, and the
//!   artifact is his from then: no stamp changes it again (law
//!   moved_by_you_sticks). Twice the same: the same.
//!
//! sb-core keeps no artifact-to-works map: the Meta is the one owner of
//! an artifact's current works.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtWorks {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    works: Vec<String>,
    /// he moved it: its works are his, never stamped again
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    moved: bool,
}

impl ArtWorks {
    /// The works it serves, in the order they came.
    pub fn works(&self) -> &[String] {
        &self.works
    }

    /// He moved it (its works are his).
    pub fn moved(&self) -> bool {
        self.moved
    }

    /// The door's stamp: `turn`'s works added, once each; none once he
    /// moved it.
    pub fn stamp(&mut self, turn: &[String]) {
        if self.moved {
            return;
        }
        for w in turn {
            if !self.works.contains(w) {
                self.works.push(w.clone());
            }
        }
    }

    /// His move: exactly `works` (none: out of every task), his for good.
    pub fn moved_to(&mut self, works: Vec<String>) {
        self.works = works;
        self.moved = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn each_new_version_adds_its_turns_works_once() {
        let mut a = ArtWorks::default();
        a.stamp(&w(&["login"]));
        a.stamp(&w(&["pricing", "login"]));
        assert_eq!(a.works(), w(&["login", "pricing"]).as_slice(), "made under login, revised under pricing: both");
        a.stamp(&[]);
        assert_eq!(a.works().len(), 2, "a turn with no work stamps nothing");
    }

    #[test]
    fn his_move_sticks_and_twice_is_the_same() {
        let mut a = ArtWorks::default();
        a.stamp(&w(&["login"]));
        a.moved_to(w(&["pricing"]));
        a.stamp(&w(&["login", "digest"]));
        assert_eq!((a.works(), a.moved()), (w(&["pricing"]).as_slice(), true), "law moved_by_you_sticks");
        let once = a.clone();
        a.moved_to(w(&["pricing"]));
        assert_eq!(a, once);
    }

    #[test]
    fn an_old_meta_reads_no_works_and_a_plain_one_writes_none() {
        let a: ArtWorks = serde_json::from_str("{}").unwrap();
        assert_eq!(a, ArtWorks::default());
        assert_eq!(serde_json::to_string(&a).unwrap(), "{}");
    }
}
