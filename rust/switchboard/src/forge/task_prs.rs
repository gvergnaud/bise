//! His pick on page tasks-fresh (architect m_66713 point 3, m_67541): the
//! linked PRs of each task, counted for sb-core's `pr_states` input. A
//! task's PRs are the PR artifacts whose works hold it; each counts by its
//! last state on the forge (`Meta::pr_last`): open, a draft or changes
//! requested as open, merged as merged, closed-unmerged and unknown not
//! at all. Pure: the daemon reads the store (daemon/pr_states.rs).

use bise_proto::rows::PrState;
use std::collections::BTreeMap;

/// The input's three lists, one entry per task with a PR counted, by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub works: Vec<String>,
    pub open: Vec<u64>,
    pub merged: Vec<u64>,
}

/// Count `prs` (each PR's works and last state) per task.
pub fn counts<'a>(prs: impl IntoIterator<Item = (&'a [String], PrState)>) -> Counts {
    let mut by: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    for (works, state) in prs {
        let (o, m) = match state {
            PrState::Open | PrState::Draft => (1, 0),
            PrState::Merged => (0, 1),
            PrState::Closed | PrState::Unknown => continue,
        };
        for w in works {
            let e = by.entry(w.as_str()).or_default();
            (e.0, e.1) = (e.0 + o, e.1 + m);
        }
    }
    let mut c = Counts::default();
    for (w, (o, m)) in by {
        c.works.push(w.to_string());
        c.open.push(o);
        c.merged.push(m);
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    /// a PR on two tasks counts on both; a draft is open; a closed one and
    /// a task with only closed ones count nothing
    #[test]
    fn each_task_counts_its_open_and_merged_prs() {
        let (a, ab, b, c) = (ws(&["a"]), ws(&["a", "b"]), ws(&["b"]), ws(&["c"]));
        let got = counts([
            (&a[..], PrState::Merged),
            (&ab[..], PrState::Merged),
            (&b[..], PrState::Draft),
            (&c[..], PrState::Closed),
            (&a[..], PrState::Closed),
        ]);
        assert_eq!(got, Counts { works: ws(&["a", "b"]), open: vec![0, 1], merged: vec![2, 1] });
        assert_eq!(counts([]), Counts::default());
    }
}
