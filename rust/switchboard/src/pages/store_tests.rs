//! The page store's laws (moved out of store.rs as is).

use super::*;
use serde_json::json;

/// Law (architect m_21543): his words on a drafts card reach the asker
/// as a page note, the card's line quoted; the model reads the text.
#[test]
fn words_on_a_drafts_card_are_a_page_note() {
    let d = temp();
    let s = Store::new(&d);
    s.publish(&publish("t1", Some("rel"), Some("release replies")), vec![], 10, &|_| false).unwrap();
    let m = s.meta("rel").unwrap();
    let (text, part) = batch_words(&m, "http://p/rel", 7, "2 drafts wait for you · release replies", "2 drafts wait for you · release replies\nreplies to Lucas and Marc", "t1", "Ou sont les drafts?");
    assert_eq!(text, "on the drafts notification \"2 drafts wait for you · release replies\" of your page \"release replies\" (http://p/rel), the user says: Ou sont les drafts?");
    assert_eq!(bise_proto::parts::render(std::slice::from_ref(&part)), text, "the model's text, byte for byte");
    let bise_proto::parts::Part::Page { to, page, title, version, notes, .. } = part else { panic!("{part:?}") };
    assert_eq!((to.as_str(), page.as_str(), title.as_str(), version), ("t1", "rel", "release replies", 1));
    assert_eq!(
        notes,
        vec![bise_proto::parts::PageNote {
            block: String::new(),
            kind: "note".into(),
            text: "Ou sont les drafts?".into(),
            quote: "2 drafts wait for you · release replies".into(),
            card: 7,
            asked: "2 drafts wait for you · release replies\nreplies to Lucas and Marc".into(),
            parts: Vec::new(),
        }]
    );
    let _ = std::fs::remove_dir_all(d);
}

fn temp() -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!("sb-pages-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn publish(agent: &str, id: Option<&str>, title: Option<&str>) -> Publish {
    Publish { agent: agent.into(), id: id.map(String::from), title: title.map(String::from), html: "<section data-kit=\"prose\" data-id=\"p1\"><p>hi</p></section>".into(), ..Publish::default() }
}

#[test]
fn ids_and_slugs() {
    assert!(valid_id("weekly-update") && valid_id("a") && !valid_id("") && !valid_id("A") && !valid_id("../x"));
    assert!(!valid_id(&"a".repeat(41)));
    assert_eq!(slug("Weekly update: Slack & Linear!"), "weekly-update-slack-linear");
    assert_eq!(title_of("weekly-update"), "weekly update");
}

#[test]
fn publish_versions_owner_and_takeover() {
    let d = temp();
    let s = Store::new(&d);
    let none = |_: &str| false;
    let (m, _) = s.publish(&publish("t1", None, Some("Weekly update")), vec![], 10, &none).unwrap();
    assert_eq!((m.id.as_str(), m.title.as_str(), m.agent.as_str(), m.version()), ("weekly-update", "Weekly update", "t1", 1));
    let (m, _) = s.publish(&publish("t1", Some("weekly-update"), None), vec![], 20, &none).unwrap();
    assert_eq!((m.version(), m.title.as_str()), (2, "Weekly update"));
    assert!(s.html("weekly-update", 1).is_some() && s.html("weekly-update", 2).is_some());
    // another agent's page
    let e = s.publish(&publish("t2", Some("weekly-update"), None), vec![], 30, &none).unwrap_err();
    assert!(e.contains("t1's page"), "{e}");
    // main takes over a gone agent's page
    let (m, _) = s.publish(&publish("main", Some("weekly-update"), None), vec![], 40, &|a| a == "t1").unwrap();
    assert_eq!((m.agent.as_str(), m.version()), ("main", 3));
    assert!(s.publish(&publish("t1", None, None), vec![], 50, &none).is_err());
    assert_eq!(s.list().len(), 1);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn notes_draft_send_and_outcomes() {
    let d = temp();
    let s = Store::new(&d);
    s.publish(&publish("t1", Some("w"), None), vec![], 10, &|_| false).unwrap();
    let notes = s
        .set_drafts(&Target::Page("w".into()), &[json!({"block": "t1", "quote": "Revenue", "kind": "note", "text": "use the March numbers"}), json!({"block": "p2", "kind": "drop", "x": 1})], 1, 11)
        .unwrap();
    assert_eq!(notes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), vec!["n1", "n2"]);
    assert_eq!(notes[1].extra.get("x"), Some(&json!(1)), "the layer's own fields are kept");
    // the whole draft list again: n1 edited, n2 left out (deleted), an edit added
    s.set_drafts(&Target::Page("w".into()), &[json!({"id": "n1", "block": "t1", "quote": "Revenue", "kind": "note", "text": "use March"}), json!({"block": "p4", "kind": "edit", "before": "Q2", "after": "Q3"})], 1, 12)
        .unwrap();
    let sent = s.send(&Target::Page("w".into()), 13).unwrap();
    assert_eq!(sent.iter().map(|n| (n.id.as_str(), n.status.as_str())).collect::<Vec<_>>(), vec![("n1", "sent"), ("n2", "sent")]);
    assert!(s.send(&Target::Page("w".into()), 14).unwrap().is_empty(), "nothing left to send");
    // a sent note can't be edited as a draft
    assert!(s.set_drafts(&Target::Page("w".into()), &[json!({"id": "n1", "text": "x"})], 1, 15).is_err());
    // an empty list clears the drafts, never the sent ones
    assert_eq!(s.set_drafts(&Target::Page("w".into()), &[], 1, 16).unwrap().len(), 2);
    let m = s.meta("w").unwrap();
    let msg = notes_message(&m, "http://127.0.0.1:47100/p/w", &sent);
    assert!(msg.starts_with("you sent 2 notes on your page \"w\" (w v1, http://127.0.0.1:47100/p/w):\n1. on \"Revenue\" (t1): use March\n2. on p4, you changed \"Q2\" → \"Q3\"\nanswer with: sb page publish <file> --id w --notes-done n1,n2 (or --note-answer n1=\"…\")\n<page-notes page=\"w\" version=\"1\">[{"), "{msg}");
    // the next version's outcomes
    let (_, unknown) = s
        .publish(&Publish { done: vec!["n1".into(), "n9".into()], answers: vec![("n2".into(), "kept: Marc asked".into())], ..publish("t1", Some("w"), None) }, vec![], 20, &|_| false)
        .unwrap();
    assert_eq!(unknown, vec!["n9"]);
    let notes = s.notes("w");
    assert_eq!(notes.iter().map(|n| n.status.as_str()).collect::<Vec<_>>(), vec!["done", "answered"]);
    assert_eq!(notes[1].answer.as_deref(), Some("kept: Marc asked"));
    let _ = std::fs::remove_dir_all(d);
}

/// page-notes: a note on 3 rows of a k-diff reaches the agent with
/// the block, the tab, the lines' numbers and text, and the note.
#[test]
fn a_note_on_a_diffs_lines_quotes_the_block_the_numbers_and_the_text() {
    let d = temp();
    let s = Store::new(&d);
    s.publish(&publish("t1", Some("w"), None), vec![], 10, &|_| false).unwrap();
    let lines = json!([{"old": 34, "text": "Skills contain"}, {"old": 35, "text": ""}, {"new": 34, "text": "Skills below"}]);
    s.set_drafts(&Target::Page("w".into()), &[json!({"block": "diff", "quote": "Skills contain Skills below", "kind": "note", "text": "keep the old line", "lines": lines, "part": "main"})], 1, 11).unwrap();
    let sent = s.send(&Target::Page("w".into()), 12).unwrap();
    let msg = notes_message(&s.meta("w").unwrap(), "http://127.0.0.1:47100/p/w", &sent);
    assert!(
        msg.contains("1. on diff main, old 34–35 → new 34: keep the old line\n     −34       │ Skills contain\n     −35       │ \n           +34 │ Skills below\nanswer with:"),
        "{msg}"
    );
    assert!(msg.contains("\"lines\":[{"), "the JSON keeps the lines: {msg}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn went_parses_and_keeps_one_entry_per_kind_and_ref() {
    let w = Went::parse("gmail-draft:r-123@mail=https://mail.google.com/mail/#drafts?compose=r-123").unwrap();
    assert_eq!((w.kind.as_str(), w.reference.as_str(), w.block.as_deref()), ("gmail-draft", "r-123", Some("mail")));
    assert_eq!(w.url, "https://mail.google.com/mail/#drafts?compose=r-123");
    assert!(Went::parse("kit-draft:42=https://app.kit.com/b/42").unwrap().block.is_none());
    for bad in ["gmail-draft", "gmail-draft:r1", ":r1=https://x", "Gmail:r1=https://x", "gmail-draft:r1=file:///etc", "gmail-draft:r 1=https://x"] {
        assert!(Went::parse(bad).is_err(), "{bad}");
    }
    let d = temp();
    let s = Store::new(&d);
    let pw = |went: Vec<Went>| Publish { went, ..publish("t1", Some("mail"), None) };
    let (m, _) = s.publish(&pw(vec![w.clone()]), vec![], 10, &|_| false).unwrap();
    assert_eq!((m.went.len(), m.went[0].at), (1, 10));
    // no --went: kept; the same kind and ref: replaced; another: added
    let (m, _) = s.publish(&pw(vec![]), vec![], 20, &|_| false).unwrap();
    assert_eq!(m.went.len(), 1);
    let w2 = Went { url: "https://mail.google.com/x".into(), ..w.clone() };
    let k = Went::parse("kit-draft:42=https://app.kit.com/b/42").unwrap();
    let (m, _) = s.publish(&pw(vec![w2, k]), vec![], 30, &|_| false).unwrap();
    assert_eq!(m.went.iter().map(|x| (x.kind.as_str(), x.url.as_str(), x.at)).collect::<Vec<_>>(), vec![("gmail-draft", "https://mail.google.com/x", 30), ("kit-draft", "https://app.kit.com/b/42", 30)]);
    let v = serde_json::to_value(s.meta("mail").unwrap()).unwrap();
    assert_eq!(v["went"][0]["ref"], "r-123", "the meta says ref");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn start_notes_go_to_main_not_in_the_agents_message() {
    let d = temp();
    let s = Store::new(&d);
    s.publish(&publish("t1", Some("bugs"), None), vec![], 10, &|_| false).unwrap();
    s.set_drafts(&Target::Page("bugs".into()), &[json!({"block": "b2", "quote": "login fails on Safari", "kind": "start"}), json!({"block": "b3", "kind": "start", "text": "this one first"})], 1, 11).unwrap();
    let sent = s.send(&Target::Page("bugs".into()), 12).unwrap();
    let m = s.meta("bugs").unwrap();
    assert_eq!(notes_message(&m, "u", &sent), "", "only start notes: nothing for the page's agent");
    let starts = s.route_starts("bugs", &|_| true, "main").unwrap();
    assert_eq!(starts.len(), 2);
    assert!(s.route_starts("bugs", &|_| true, "main").unwrap().is_empty(), "routed once");
    let msg = start_message(&m, "http://127.0.0.1:47100/p/bugs", &starts);
    assert!(msg.starts_with("you asked for an agent on these items of the page \"bugs\" (bugs v1, http://127.0.0.1:47100/p/bugs):\n1. item b2 \"login fails on Safari\"\n2. item b3: this one first\n"), "{msg}");
    assert!(msg.contains("ask t1 (the page's agent) to publish it again with data-agent=") && msg.contains("<page-start page=\"bugs\" version=\"1\">[{"), "{msg}");
    // a start note among others: the agent's message has the others only
    s.set_drafts(&Target::Page("bugs".into()), &[json!({"block": "b4", "kind": "start"}), json!({"block": "p1", "kind": "note", "text": "shorter"})], 1, 13).unwrap();
    let sent = s.send(&Target::Page("bugs".into()), 14).unwrap();
    let text = notes_message(&m, "u", &sent);
    assert!(text.starts_with("you sent 1 note on your page") && text.contains("shorter") && !text.contains("b4"), "{text}");
    let _ = std::fs::remove_dir_all(d);
}

/// page-answer-row (architect m_20833): the message's page part renders
/// as the model's text, and carries his pick with the question's card
/// and words, his note with what it is on, an edit's old and new words.
#[test]
fn a_page_message_carries_his_notes_typed() {
    let d = temp();
    let s = Store::new(&d);
    s.publish(&publish("t1", Some("w"), Some("efforts")), vec![], 10, &|_| false).unwrap();
    let m = s.meta("w").unwrap();
    let q = super::super::questions::Question { block: "q-live".into(), text: "Est-ce que bise lit les listes ?".into(), card: 516, ..Default::default() };
    let pick = s.add_pick("w", "q-live", "la table maintenant", &[], 1, 11).unwrap();
    s.set_drafts(&Target::Page("w".into()), &[json!({"block": "p1", "kind": "note", "text": "shorter", "quote": "Revenue grew"}), json!({"block": "p2", "kind": "edit", "before": "Q2", "after": "Q3"})], 1, 12).unwrap();
    let sent = s.send(&Target::Page("w".into()), 13).unwrap();
    let mut all = vec![pick];
    all.extend(sent);
    let text = format!("{}\n\n[from the page: answer on the page; one short line at most]", notes_message(&m, "u", &all));
    let part = page_part(&m, std::slice::from_ref(&q), &all, "main", &text);
    assert_eq!(bise_proto::parts::render(std::slice::from_ref(&part)), text, "the model's text, byte for byte");
    let bise_proto::parts::Part::Page { to, page, title, version, notes, .. } = part else { panic!("{part:?}") };
    assert_eq!((to.as_str(), page.as_str(), title.as_str(), version), ("main", "w", "efforts", 1));
    let n = |block: &str, kind: &str, text: &str, quote: &str, card: u64, asked: &str| bise_proto::parts::PageNote { block: block.into(), kind: kind.into(), text: text.into(), quote: quote.into(), card, asked: asked.into(), parts: Vec::new() };
    assert_eq!(
        notes,
        vec![
            n("q-live", "pick", "la table maintenant", "", 516, "Est-ce que bise lit les listes ?"),
            n("p1", "note", "shorter", "Revenue grew", 0, ""),
            n("p2", "edit", "Q3", "Q2", 0, ""),
        ]
    );
    let _ = std::fs::remove_dir_all(d);
}

/// card-image-parts (architect m_35268): a card answered with an image
/// and words keeps his parts on the pick note; its text is his words
/// alone; the model's line renders the parts (the image's marker, so the
/// model gets it); the wire's note carries them for the chips.
#[test]
fn a_pick_with_an_image_keeps_his_parts() {
    use bise_proto::parts::Part;
    let d = temp();
    let s = Store::new(&d);
    s.publish(&publish("t1", Some("w"), Some("efforts")), vec![], 10, &|_| false).unwrap();
    let m = s.meta("w").unwrap();
    let img = Part::Image { name: "[Image #1]".into(), source: "/u/shot.png".into(), mime: "image/png".into(), b64_file: "/h/images/ab.b64".into(), caption: String::new() };
    let parts = vec![img.clone(), Part::Text { text: " les murs ne sont pas réalistes".into() }];
    let pick = s.add_pick("w", "q-live", "les murs ne sont pas réalistes", &parts, 1, 11).unwrap();
    assert_eq!(s.notes("w")[0].parts, parts, "stored with the note");
    let text = notes_message(&m, "u", std::slice::from_ref(&pick));
    let marker = bise_proto::parts::render(std::slice::from_ref(&img));
    assert!(text.contains(&format!("pick: {marker} les murs")), "{text}");
    let q = super::super::questions::Question { block: "q-live".into(), text: "quel plan ?".into(), card: 687, ..Default::default() };
    let Part::Page { notes, .. } = page_part(&m, std::slice::from_ref(&q), &[pick], "bims", &text) else { panic!() };
    assert_eq!(notes[0].text, "les murs ne sont pas réalistes", "his words, never a marker");
    assert_eq!(notes[0].parts, parts);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn data_agents_are_read_from_the_fragment() {
    let h = r#"<section data-kit="review" data-id="r"><ul><li data-id="a" data-agent="fix-login">x</li><li data-agent='docs'>y</li><li data-agent="fix-login">z</li><li data-agent="">w</li></ul></section>"#;
    assert_eq!(data_agents_of(h), vec!["fix-login", "docs"]);
    assert!(data_agents_of("<p>no agent</p>").is_empty());
}

#[test]
fn a_page_waits_for_review_until_the_users_go() {
    let d = temp();
    let s = Store::new(&d);
    let block = |id: &str, kit: &str| Block { id: id.into(), kit: kit.into(), hash: "h".into() };
    // a plain page never holds drafts back
    s.publish(&publish("t1", Some("plain"), None), vec![block("p1", "prose")], 10, &|_| false).unwrap();
    assert_eq!(s.pending_review("t1"), None);
    // a review: held until an approve, a send, or words asking for drafts
    s.publish(&Publish { title: Some("replies".into()), ..publish("t1", Some("replies"), None) }, vec![block("r", "review"), block("m", "email")], 20, &|_| false).unwrap();
    assert_eq!(s.pending_review("t1").as_deref(), Some("\"replies\""));
    assert_eq!(s.pending_review("t2"), None, "another agent's page");
    s.set_drafts(&Target::Page("replies".into()), &[json!({"block": "m", "kind": "note", "text": "shorter"})], 1, 21).unwrap();
    s.send(&Target::Page("replies".into()), 22).unwrap();
    assert!(s.pending_review("t1").is_some(), "a note is not his go");
    s.set_drafts(&Target::Page("replies".into()), &[json!({"block": "m", "kind": "voice", "text": "ok put it in my drafts"})], 1, 23).unwrap();
    assert!(s.pending_review("t1").is_some(), "a draft note is not said yet");
    s.send(&Target::Page("replies".into()), 24).unwrap();
    assert_eq!(s.pending_review("t1"), None, "his word: drafts run free");
    // an open question holds too, until answered
    s.publish(&publish("t3", Some("plan"), None), vec![block("q1", "question")], 30, &|_| false).unwrap();
    s.save_questions("plan", &[super::super::questions::Question { block: "q1".into(), text: "which venue?".into(), options: vec![], card: 4, ..Default::default() }]).unwrap();
    assert!(s.pending_review("t3").is_some());
    s.save_questions("plan", &[super::super::questions::Question { block: "q1".into(), text: "which venue?".into(), options: vec![], card: 4, reply: Some("Les Pins".into()), ..Default::default() }]).unwrap();
    assert_eq!(s.pending_review("t3"), None);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn taste_is_counted_and_only_on_the_publish_that_said_it() {
    assert_eq!(taste_rules("# my taste\n- short\n* no emoji\n  - lowercase\ntext - not one\n-nope\n"), 3);
    assert_eq!(taste_rules(""), 0);
    let d = temp();
    let s = Store::new(&d);
    let (m, _) = s.publish(&Publish { taste: Some(4), ..publish("t1", Some("w"), None) }, vec![], 10, &|_| false).unwrap();
    assert_eq!(m.taste, Some(json!({"rules": 4})));
    assert_eq!(serde_json::to_value(s.meta("w").unwrap()).unwrap()["taste"], json!({"rules": 4}));
    let (m, _) = s.publish(&publish("t1", Some("w"), None), vec![], 20, &|_| false).unwrap();
    assert_eq!(m.taste, None, "a publish without --taste: gone");
    let _ = std::fs::remove_dir_all(d);
}

/// Law (cause 4 of page tasks-true, architect m_67725): a page's waits
/// count for its latest version's works, never for every task one of its
/// versions served; his move wins; a version from before the stamp reads
/// the union.
#[test]
fn page_waits_count_for_its_latest_versions_works() {
    let w = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let v = |works: Option<&[&str]>| Version { works: works.map(w), ..Default::default() };
    let mut m = Meta::default();
    m.works.stamp(&w(&["old", "new"]));
    m.versions = vec![v(Some(&["old"])), v(Some(&["new"]))];
    assert_eq!(m.waits_works(), w(&["new"]), "the latest version's works, not the union");
    m.versions.push(v(None));
    assert_eq!(m.waits_works(), w(&["old", "new"]), "a version from before the stamp: the union");
    m.versions.push(v(Some(&["new"])));
    m.works.moved_to(w(&["his"]));
    assert_eq!(m.waits_works(), w(&["his"]), "his move wins");
}
