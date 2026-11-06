//! pr-integration rules 2 and 4: a PR the hub follows is an artifact of
//! kind `pr`, made by the hub, keyed by the PR's link (`forge::pr_link`:
//! `sb artifact add <url>` lands on the same one), by the place's agent,
//! its title the forge's. Merged or closed: the hub archives it;
//! reopened: back, but only when the hub archived it itself (his own
//! archive stays).
//!
//! Pure: what the store holds for the PR in, the store's ops out. The
//! daemon applies them (daemon/pr_arts.rs) each time the hub's snapshot
//! of a PR changes, so they are a sync, not an event log: the same
//! snapshot again, or a hub restart, asks for nothing.

use crate::place::{PrSnapshot, PrState};

/// What the store holds for a PR (its artifact, found by its source).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Have {
    pub id: String,
    pub title: String,
    /// in the store's archived set
    pub archived: bool,
    /// the hub archived it at the PR's end ([`crate::artifacts::Meta::pr_ended`])
    pub pr_ended: bool,
}

/// One write for the daemon to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreOp {
    /// A new artifact for the PR (`url`: its key, `agent` its maker);
    /// `ended`: the PR is already merged or closed, archive it at once.
    Add { url: String, title: Option<String>, agent: String, ended: bool },
    /// The forge's title (the add's `PR #6` until the forge said it, or
    /// a title edited on the forge).
    Retitle { id: String, title: String },
    /// Merged or closed: archived, as the hub's.
    Archive { id: String },
    /// Reopened: back in the list (the hub's archive only).
    Unarchive { id: String },
}

fn ended(s: PrState) -> bool {
    matches!(s, PrState::Merged | PrState::Closed)
}

/// The ops that make the store say what the forge says about `pr`.
/// `agent`: the place's first agent (none: no live agent there, then no
/// new artifact; one that exists is still kept in step).
pub fn sync(pr: &PrSnapshot, agent: Option<&str>, have: Option<&Have>) -> Vec<StoreOp> {
    let title = pr.facts.title.trim();
    let Some(h) = have else {
        let (Some(agent), Some(link)) = (agent, super::pr_link(&pr.url)) else { return Vec::new() };
        return vec![StoreOp::Add {
            url: link.url,
            title: (!title.is_empty()).then(|| title.to_string()),
            agent: agent.to_string(),
            ended: ended(pr.state),
        }];
    };
    let mut out = Vec::new();
    if !title.is_empty() && h.title != title {
        out.push(StoreOp::Retitle { id: h.id.clone(), title: title.to_string() });
    }
    // once per end (architect m_54244): a merged PR he unarchived by
    // hand stays out of the archive; a reopen clears `pr_ended`, so a
    // second end archives it again
    match (ended(pr.state), h.archived) {
        (true, false) if !h.pr_ended => out.push(StoreOp::Archive { id: h.id.clone() }),
        // reopened: back in the list, the end forgotten (also when he had
        // unarchived it already: the next end archives it again)
        (false, _) if h.pr_ended => out.push(StoreOp::Unarchive { id: h.id.clone() }),
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::tests::pr;

    fn have(archived: bool, pr_ended: bool) -> Have {
        Have { id: "pr-7".into(), title: "faster signup".into(), archived, pr_ended }
    }

    fn titled(n: u64, s: PrState) -> PrSnapshot {
        let mut p = pr(n, s);
        p.facts.title = "faster signup".into();
        p
    }

    #[test]
    fn a_new_pr_is_added_by_its_place_s_agent() {
        let p = titled(7, PrState::Open);
        assert_eq!(
            sync(&p, Some("perf"), None),
            [StoreOp::Add { url: "https://github.com/o/r/pull/7".into(), title: Some("faster signup".into()), agent: "perf".into(), ended: false }]
        );
        // no live agent: not added
        assert!(sync(&p, None, None).is_empty());
        // first seen merged: added archived
        assert!(matches!(&sync(&titled(7, PrState::Merged), Some("perf"), None)[..], [StoreOp::Add { ended: true, .. }]));
        // the forge said no title yet: the store's default
        assert!(matches!(&sync(&pr(7, PrState::Open), Some("perf"), None)[..], [StoreOp::Add { title: None, .. }]));
    }

    #[test]
    fn the_title_follows_the_forge() {
        let mut p = titled(7, PrState::Open);
        p.facts.title = "faster signup, again".into();
        assert_eq!(sync(&p, Some("perf"), Some(&have(false, false))), [StoreOp::Retitle { id: "pr-7".into(), title: "faster signup, again".into() }]);
        // no title from the forge: the store's stays
        assert!(sync(&pr(7, PrState::Open), Some("perf"), Some(&have(false, false))).is_empty());
    }

    #[test]
    fn archived_by_its_end_back_when_reopened_never_his_own_archive() {
        let id = || "pr-7".to_string();
        for s in [PrState::Merged, PrState::Closed] {
            assert_eq!(sync(&titled(7, s), Some("perf"), Some(&have(false, false))), [StoreOp::Archive { id: id() }]);
            // archived already (his or the hub's): nothing
            assert!(sync(&titled(7, s), Some("perf"), Some(&have(true, false))).is_empty());
        }
        assert_eq!(sync(&titled(7, PrState::Open), Some("perf"), Some(&have(true, true))), [StoreOp::Unarchive { id: id() }]);
        // archived once per end: he unarchived a merged PR, it stays out
        assert!(sync(&titled(7, PrState::Merged), Some("perf"), Some(&have(false, true))).is_empty());
        // ... until it is reopened: the end is forgotten
        assert_eq!(sync(&titled(7, PrState::Open), Some("perf"), Some(&have(false, true))), [StoreOp::Unarchive { id: id() }]);
        // he archived an open PR himself: it stays archived
        assert!(sync(&titled(7, PrState::Open), Some("perf"), Some(&have(true, false))).is_empty());
        assert!(sync(&titled(7, PrState::Draft), Some("perf"), Some(&have(true, false))).is_empty());
    }

    /// architect m_54087: a restart's first answer, or the same snapshot
    /// again, writes nothing once the store is in step.
    #[test]
    fn in_step_asks_nothing() {
        for (s, h) in [(PrState::Open, have(false, false)), (PrState::Draft, have(false, false)), (PrState::Merged, have(true, true)), (PrState::Closed, have(true, false))] {
            assert!(sync(&titled(7, s), Some("perf"), Some(&h)).is_empty(), "{s:?}");
        }
    }
}
