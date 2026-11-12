//! The laws of artifacts_marks: his word first, opens only forward, the
//! section of each fact, the backfill's rules.

use super::*;

fn kept_by(m: &Marks, id: &str) -> Option<String> {
    m.kept(id).map(|k| k.by.clone())
}

#[test]
fn his_keep_wins_over_an_agents() {
    let mut m = Marks::default();
    assert_eq!(keep(&mut m, "a", Who::Agent("learnings"), 1), Outcome::Changed);
    assert_eq!(kept_by(&m, "a").as_deref(), Some("learnings"));
    // his keep takes it over
    assert_eq!(keep(&mut m, "a", Who::You, 2), Outcome::Changed);
    assert_eq!(kept_by(&m, "a").as_deref(), Some("you"));
    // an agent's keep never writes over his
    assert_eq!(keep(&mut m, "a", Who::Agent("perf"), 3), Outcome::His);
    assert_eq!(m.kept("a"), Some(&Kept { by: "you".into(), at_ms: 2 }));
}

#[test]
fn keep_twice_is_keep_once() {
    let mut m = Marks::default();
    keep(&mut m, "a", Who::You, 1);
    let once = m.clone();
    assert_eq!(keep(&mut m, "a", Who::You, 9), Outcome::Same);
    assert_eq!(m, once);
    keep(&mut m, "b", Who::Agent("x"), 1);
    let once = m.clone();
    // a second agent's keep: still kept by the first, nothing written
    assert_eq!(keep(&mut m, "b", Who::Agent("y"), 9), Outcome::Same);
    assert_eq!(m, once);
}

#[test]
fn an_agent_never_lets_his_keep_go() {
    let mut m = Marks::default();
    keep(&mut m, "a", Who::You, 1);
    assert_eq!(unkeep(&mut m, "a", Who::Agent("learnings")), Outcome::His);
    assert!(m.kept("a").is_some());
    // he lets go of any keep, his or an agent's
    assert_eq!(unkeep(&mut m, "a", Who::You), Outcome::Changed);
    assert!(m.kept("a").is_none());
    keep(&mut m, "b", Who::Agent("learnings"), 1);
    assert_eq!(unkeep(&mut m, "b", Who::You), Outcome::Changed);
    assert!(m.kept("b").is_none());
    // an agent lets an agent's keep go; not kept: nothing to do
    keep(&mut m, "c", Who::Agent("learnings"), 1);
    assert_eq!(unkeep(&mut m, "c", Who::Agent("perf")), Outcome::Changed);
    assert_eq!(unkeep(&mut m, "c", Who::Agent("perf")), Outcome::Same);
    assert_eq!(unkeep(&mut m, "nope", Who::You), Outcome::Same);
}

#[test]
fn a_mark_left_empty_goes_from_the_file() {
    let mut m = Marks::default();
    keep(&mut m, "a", Who::You, 1);
    unkeep(&mut m, "a", Who::You);
    assert!(m.marks.is_empty());
    // an agent's refused keep leaves no empty entry
    unkeep(&mut m, "z", Who::Agent("x"));
    assert!(m.marks.is_empty());
}

#[test]
fn an_open_only_moves_forward() {
    let mut m = Marks::default();
    assert!(opened(&mut m, "a", 10, Some(2)));
    assert!(!opened(&mut m, "a", 5, Some(1)));
    assert!(!opened(&mut m, "a", 10, None));
    assert_eq!((m.opened_ms("a"), m.seen_v("a")), (Some(10), Some(2)));
    assert!(opened(&mut m, "a", 11, None));
    assert_eq!((m.opened_ms("a"), m.seen_v("a")), (Some(11), Some(2)));
    // a newer version seen in an open at the same time still moves
    assert!(opened(&mut m, "a", 11, Some(3)));
    assert_eq!(m.seen_v("a"), Some(3));
    // opens and keeps live side by side
    keep(&mut m, "a", Who::You, 12);
    unkeep(&mut m, "a", Who::You);
    assert_eq!(m.opened_ms("a"), Some(11));
}

#[test]
fn the_file_keeps_unknown_keys_and_reads_old_ones() {
    let raw = r#"{"v":1,"later":true,"marks":{"a":{"kept":{"by":"you","at_ms":3},"pin":1}}}"#;
    let m: Marks = serde_json::from_str(raw).unwrap();
    assert_eq!(m.kept("a").map(|k| k.by.as_str()), Some("you"));
    let back = serde_json::to_value(&m).unwrap();
    assert_eq!(back["later"], true);
    assert_eq!(back["marks"]["a"]["pin"], 1);
    // no `v`: this version; a newer one is never written over
    let none: Marks = serde_json::from_str("{}").unwrap();
    assert!(none.writable() && none.backfilled.is_none());
    let newer: Marks = serde_json::from_str(r#"{"v":2}"#).unwrap();
    assert!(!newer.writable());
}

#[test]
fn the_audience_at_an_add() {
    use Audience::*;
    assert_eq!(audience_at_add(None, "perf", "image", false), Agents);
    assert_eq!(audience_at_add(None, "perf", "image", true), You);
    assert_eq!(audience_at_add(None, "you", "doc", false), You, "his own add");
    assert_eq!(audience_at_add(None, "perf", "pr", false), You, "a PR waits on his merge");
    assert_eq!(audience_at_add(None, crate::model::MAIN, "doc", false), You, "main is the agent he talks to");
    // a next version keeps it; --for-you raises, never lowers
    assert_eq!(audience_at_add(Some(You), "perf", "doc", false), You);
    assert_eq!(audience_at_add(Some(Agents), "perf", "doc", true), You);
    assert_eq!(audience_at_add(Some(Agents), "perf", "doc", false), Agents);
}

#[test]
fn done_is_its_task_or_its_pr_ending_unless_it_waits_on_him() {
    assert!(!done(Ending::default()));
    assert!(done(Ending { agent_archived: true, ..Default::default() }));
    assert!(done(Ending { archived: true, ..Default::default() }));
    assert!(done(Ending { pr_ended: true, ..Default::default() }));
    assert!(!done(Ending { agent_archived: true, pr_ended: true, archived: true, waiting: true }));
}

#[test]
fn the_section_of_each_fact() {
    use Audience::{Agents as Theirs, You};
    use Section::{Agents, Done, Kept, Open};
    for (aud, kept, done, want) in [
        (You, true, true, Kept),
        (Theirs, true, false, Kept),
        (Theirs, false, false, Agents),
        (Theirs, false, true, Agents),
        (You, false, true, Done),
        (You, false, false, Open),
    ] {
        assert_eq!(section(aud, kept, done), want, "{aud:?} kept={kept} done={done}");
    }
    // every section has its word, read by the clients
    assert_eq!([Kept, Open, Done, Agents].map(Section::word), ["kept", "open", "done", "agents"]);
}

#[test]
fn touched_is_the_last_of_its_version_his_open_and_a_note() {
    assert_eq!(touched(5, None, None), 5);
    assert_eq!(touched(5, Some(9), None), 9);
    assert_eq!(touched(5, Some(3), Some(7)), 7);
}

#[test]
fn the_backfill_reads_the_audits_words() {
    use Audience::*;
    for (title, kind, by, want) in [
        ("bise-pitch-investisseurs.pptx", "slides", "pitch", You),
        ("readme film · final cut", "video", "readme-film", You),
        ("les bims · prototype", "site", "bims", You),
        ("note for Benjamin", "doc", "main", You),
        ("agent-view-dark-1400-search", "image", "ui-review", Agents),
        ("3ce74eb9a quick pass shots", "folder", "fast-loop", Agents),
        ("film QA sheet", "image", "readme-film", Agents),
        ("film shot 3", "image", "readme-film", Agents),
        ("brief: land-all (issue 12)", "doc", "fast-loop", Agents),
        ("plugin apps contract", "doc", "apps-lead", Agents),
        ("anything", "pr", "perf", You),
        ("anything", "doc", "you", You),
    ] {
        assert_eq!(classify(title, kind, by), want, "{title}");
    }
}

#[test]
fn the_backfill_keeps_what_outlives_its_task() {
    assert!(evergreen("learnings · carnet", "learnings"));
    assert!(evergreen("changelog", "changelog"));
    assert!(evergreen("prd: bise apps", "prd-bise-apps"));
    assert!(evergreen("the ds", "ds-page"));
    assert!(!evergreen("dsp notes", "x"));
    assert!(!evergreen("agent view fixes", "agent-view-fixes"));
}
