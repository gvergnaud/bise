//! usage step 2 (architect m_53623 b): the hub counts sb-core's journal
//! effects, live only. A hub that restarts replays its journal and boots:
//! nothing it already counted comes back as an effect, so nothing counts
//! twice. Out of core_tests.rs (its line limit).

use super::super::*;
use crate::usage_count::Counter;

/// The journal effects of `fx` through a fresh counter: its counts.
fn counted(fx: &[Effect]) -> Vec<(bise_usage::Day, bise_usage::Tally)> {
    let mut c = Counter::new(0);
    for e in fx {
        if let Effect::Journal(ev) = e {
            c.journal(ev, 0);
        }
    }
    c.due(0)
}

#[test]
fn a_boot_replay_counts_nothing_twice() {
    let mut t = T::new();
    let mut live = Vec::new();
    live.extend(t.user(MAIN, "/new -w fix: the invoice parser"));
    t.spawn_task("docs");
    live.extend(t.user("docs", "and the README too"));
    live.extend(t.go(Input::ReplReady { agent: "fix".into() }));
    let first = counted(&live);
    assert!(first.first().is_some_and(|(_, tally)| tally.counts.contains_key(&bise_usage::CountKey::UserMessage)), "the live run counts: {first:?}");
    // the hub restarts on that journal
    let journal = t.journal.borrow().clone();
    t.hub = Hub::new("/w");
    assert!(t.hub.replay(&journal).is_empty());
    let mut boot = t.go(Input::Boot);
    for a in [MAIN, "fix", "docs"] {
        boot.extend(t.go(Input::ReplReady { agent: a.into() }));
    }
    assert_eq!(counted(&boot), vec![], "a replay is no effect: {boot:?}");
}
