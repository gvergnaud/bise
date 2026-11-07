//! drop-card (Gabriel: « des boutons avec un 1 pour yes et 2 pour no »):
//! main's drop of a task with work to save opens a card with two options,
//! `1 archive it` / `2 keep it` (bend/hub/place.bend drop_card). Option 1
//! archives exactly like `yes` (work saved, /restore brings it back),
//! option 2 keeps it like `no`; typed yes/no still answer it, and a digit
//! on a card opened before its options keeps the agent (law
//! drop_option_* in bend/laws/inbox.bend). Out of core_tests.rs (its
//! line limit).

use super::super::*;

/// @fix, a worktree task with 1 changed file, and the card main's drop opened.
fn drop_card(t: &mut T) -> (u64, String) {
    t.user(MAIN, "/new -w fix: x");
    t.go(Input::ReplReady { agent: "fix".into() });
    t.go(Input::ReplIdle { agent: "fix".into(), leftover: false });
    t.env.loss = Loss { dirty: 1, unpushed: 0 };
    let (tok, fx) = t.req(MAIN, AgentReq::Drop { agent: "fix".into() });
    assert_eq!(reply(&fx, tok).unwrap()["dropped"], false);
    let c = t.hub.st.cards.values().find(|c| c.kind == "drop").unwrap();
    (c.id, c.text.clone())
}

fn answer(t: &mut T, card: u64, text: &str) -> Vec<Effect> {
    t.go(Input::UserCmd { client: 1, focus: MAIN.into(), cmd: UserCmd::Answer { card, text: text.into(), parts: String::new() } })
}

/// Card `card`'s close with his answer: its resolution, pick number and words.
fn closed(t: &T, card: u64) -> Vec<(Option<String>, Option<u64>, Option<String>)> {
    let s = |v: &Value| v.as_str().map(String::from);
    t.journal.borrow().iter().filter(|j| j["type"] == "card_closed" && j["id"] == card).map(|j| (s(&j["resolution"]), j["pick_n"].as_u64(), s(&j["pick"]))).collect()
}

fn pick(res: &str, n: u64, words: &str) -> (Option<String>, Option<u64>, Option<String>) {
    (Some(res.into()), Some(n), Some(words.into()))
}

#[test]
fn a_drop_card_shows_two_options() {
    let mut t = T::new();
    let (_, text) = drop_card(&mut t);
    let (q, opts) = bise_proto::rows::question(&text);
    let labels: Vec<&str> = opts.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(labels, ["archive it", "keep it"], "{text}");
    assert_eq!(q, "main suggests it. its 1 changed file is kept; /restore brings it back.");
    assert!(!text.contains("answer yes or no"), "{text}");
}

#[test]
fn option_1_archives_like_yes_and_restore_brings_it_back() {
    let mut t = T::new();
    let (card, _) = drop_card(&mut t);
    let fx = answer(&mut t, card, "1");
    assert!(fx.contains(&Effect::Kill { agent: "fix".into() }), "{fx:?}");
    assert_eq!(t.status("fix"), Status::Archived);
    assert_eq!(t.env.dropped, ["fix"]);
    assert!(t.hub.st.agents["fix"].snapshot_ref.is_some(), "its work is saved");
    let c = closed(&t, card);
    assert!(c.contains(&pick("accepted", 1, "archive it")), "{c:?}");
    let fx = t.user(MAIN, "/restore fix");
    assert!(fx.iter().any(|e| matches!(e, Effect::Spawn { agent, resume: true, .. } if agent == "fix")), "{fx:?}");
    // its saved work is used: no snapshot left
    assert!(t.hub.st.agents["fix"].snapshot_ref.is_none());
}

// issue 32 3b: a restore whose worktree git could not make again (law
// restore_err_keeps_archived): back archived in its dropped worktree, its
// saved work kept, git's words in main's feed, no REPL
#[test]
fn a_restore_git_refuses_keeps_it_archived_with_its_work() {
    let mut t = T::new();
    let (card, _) = drop_card(&mut t);
    answer(&mut t, card, "1");
    let snap = t.hub.st.agents["fix"].snapshot_ref.clone();
    assert!(snap.is_some());
    t.env.restore_err = Some("fatal: 'sb/fix' is already checked out".into());
    let fx = t.user(MAIN, "/restore fix");
    assert!(!fx.iter().any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == "fix")), "{fx:?}");
    assert_eq!(t.status("fix"), Status::Archived);
    let a = &t.hub.st.agents["fix"];
    assert!(a.ws.dropped, "its dropped ws back");
    assert_eq!(a.snapshot_ref, snap, "its saved work kept");
    assert!(has_line(&fx, MAIN, "@fix could not be restored: its worktree was not made again: fatal: 'sb/fix' is already checked out"), "{fx:?}");
    // git fixed: the next /restore brings it back
    t.env.restore_err = None;
    let fx = t.user(MAIN, "/restore fix");
    assert!(fx.iter().any(|e| matches!(e, Effect::Spawn { agent, resume: true, .. } if agent == "fix")), "{fx:?}");
}

#[test]
fn option_2_keeps_it_like_no() {
    let mut t = T::new();
    let (card, _) = drop_card(&mut t);
    let fx = answer(&mut t, card, "2");
    assert!(!fx.contains(&Effect::Kill { agent: "fix".into() }), "{fx:?}");
    assert_ne!(t.status("fix"), Status::Archived);
    assert!(t.env.dropped.is_empty());
    assert_eq!(closed(&t, card), [pick("refused", 2, "keep it")]);
}

#[test]
fn typed_yes_and_no_still_answer_it() {
    for (said, archived) in [("yes", true), ("oui", true), ("no", false), ("plus tard", false)] {
        let mut t = T::new();
        let (card, _) = drop_card(&mut t);
        answer(&mut t, card, said);
        assert_eq!(t.status("fix") == Status::Archived, archived, "{said}");
    }
}

/// A card opened before its options (an older hub's text): a digit is no
/// option there, so it keeps the agent; its yes still archives.
#[test]
fn a_digit_on_an_old_drop_card_keeps_the_agent() {
    for (said, archived) in [("1", false), ("yes", true)] {
        let mut t = T::new();
        let (card, _) = drop_card(&mut t);
        let old = "main suggests: archive @fix? 1 changed file and 0 unpushed commit will be saved (/restore). (answer yes or no)";
        assert!(bise_proto::rows::question(old).1.is_empty(), "{old}");
        // the hub restarts on a journal whose card has the old words
        let mut journal = t.journal.borrow().clone();
        for j in journal.iter_mut().filter(|j| j["type"] == "card_opened" && j["card"]["id"] == card) {
            j["card"]["text"] = json!(old);
        }
        t.hub = Hub::new("/w");
        assert!(t.hub.replay(&journal).is_empty());
        t.go(Input::Boot);
        t.go(Input::ReplReady { agent: MAIN.into() });
        t.go(Input::ReplReady { agent: "fix".into() });
        t.go(Input::ReplIdle { agent: "fix".into(), leftover: false });
        assert_eq!(t.hub.st.cards[&card].text, old);
        answer(&mut t, card, said);
        assert_eq!(t.status("fix") == Status::Archived, archived, "{said}");
    }
}
