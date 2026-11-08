//! The mirror of sb-core's view (core_view.rs), on a real sb-core.

use super::*;

/// idle-cpu: an idle hub with a big history stays cheap. The daemon
/// ticks every 500 ms; a tick's view sent every agent not archived
/// (~127 KB for 20 agents on a real hub, ~10% CPU in sb-core and as much
/// in the hub to parse it). Now an idle tick sends no agent, no message
/// walk, no order, cards or notes: a few hundred bytes whatever the
/// history, and 20 of them cost less than one full view. A step that
/// changes an agent's runtime still sends that agent, and the mirror
/// keeps the others.
#[test]
fn an_idle_tick_on_a_big_history_is_small_and_cheap() {
    let ws = json!({"mode": "shared", "path": "/w", "branch": null, "base_commit": null, "dropped": false});
    let long = "fake context line. ".repeat(200);
    let mut ev = Vec::new();
    for i in 0..340u64 {
        let name = format!("t{}", i);
        ev.push(json!({"type": "task_created", "name": name, "parent": "main", "ws": ws, "at_ms": i,
            "brief": {"objective": format!("fake objective {}", i), "context": long}}));
        ev.push(json!({"type": "reported", "name": name, "report": {"at_ms": i, "kind": "progress", "summary": long, "decisions": []}}));
        if i >= 40 {
            ev.push(json!({"type": "lifecycle", "name": name, "lifecycle": "archived", "reason": "drop"}));
        }
    }
    for id in 1..=5000u64 {
        ev.push(json!({"type": "message_sent", "msg": {"id": id, "thread": id, "from": "main", "to": format!("t{}", id % 40),
            "reply_to": null, "expect_reply": false, "auto": false, "text": format!("fake {}", id), "created_ms": 1000 + id, "plain": true}}));
        ev.push(json!({"type": "message_state", "id": id, "state": "delivered"}));
    }
    ev.push(json!({"type": "card_opened", "card": {"id": 1, "agent": "t1", "kind": "question", "text": "fake card", "for_msg": null, "created_ms": 1}}));
    ev.push(json!({"type": "main_note", "text": "a fake note", "at_ms": 1}));
    let mut h = Hub::new("/w");
    assert!(h.replay(&ev).is_empty());
    for i in 0..40u64 {
        h.force_run(&format!("t{}", i), Run::Idle);
    }
    let order = h.st.order.clone();
    let (cards, notes) = (h.st.cards.len(), h.st.main_notes.clone());
    assert_eq!(order.len(), 341);
    let t0 = std::time::Instant::now();
    let full = h.raw(&json!({"t": "view_all"}));
    let full_time = t0.elapsed();
    let t0 = std::time::Instant::now();
    let mut biggest = 0;
    for i in 0..20u64 {
        let out = h.link.call(&json!({"t": "tick", "now": 1_000_000 + i * 500, "git": true, "ans": []})).unwrap();
        biggest = biggest.max(out.to_string().len());
        assert_eq!(out["view"]["agents"], json!([]), "an idle tick sends no agent");
        assert_eq!(out["view"]["msgs"], json!([]));
        assert!(out["view"].get("order").is_none() && out["view"].get("cards").is_none(), "{}", out);
        h.load_view(&out["view"]);
    }
    let ticks_time = t0.elapsed();
    assert!(biggest < 400, "an idle tick answers {} bytes", biggest);
    assert!(full.to_string().len() > 1_000_000, "the history is big: {}", full.to_string().len());
    assert!(ticks_time < full_time, "20 idle ticks {:?}, one full view {:?}", ticks_time, full_time);
    // the mirror kept everything the ticks left out
    assert_eq!(h.st.order, order);
    assert_eq!((h.st.cards.len(), &h.st.main_notes), (cards, &notes));
    assert_eq!(h.st.agents.len(), 341);
    assert_eq!(h.st.agents["t3"].run, Run::Idle);
    assert!(h.st.agents["t3"].brief.context.starts_with("fake context"));
    // a runtime change is sent: that agent only
    let out = h.raw(&json!({"t": "force_run", "agent": "t3", "run": "busy", "now": 1_020_000, "git": true, "ans": []}));
    let sent: Vec<&str> = out["view"]["agents"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(sent, vec!["t3"]);
    h.load_view(&out["view"]);
    assert_eq!(h.st.agents["t3"].run, Run::Busy);
    assert_eq!(h.st.agents["t4"].run, Run::Idle);
    assert_eq!(h.st.agents.len(), 341);
}

/// view-all-perf: view_all sends an old message that owes nothing as a
/// cold row without its body (`cold`), the newest 256, a question not
/// settled and a queued one whole; the mirror keeps the cold one's row
/// (`peek`). No row has words (c6): the journal has them, whole or cold.
#[test]
fn view_all_sends_old_messages_cold_and_the_journal_has_every_ones_words() {
    let ws = json!({"mode": "shared", "path": "/w", "branch": null, "base_commit": null, "dropped": false});
    let mut ev = vec![json!({"type": "task_created", "name": "a", "parent": "main", "ws": ws, "at_ms": 0, "brief": {"objective": "fake"}})];
    for id in 1..=400u64 {
        let ask = id == 2;
        ev.push(json!({"type": "message_sent", "msg": {"id": id, "thread": id, "from": "main", "to": "a",
            "reply_to": null, "expect_reply": ask, "auto": false, "text": format!("fake {}", id), "created_ms": 1000 + id, "plain": false,
            "parts": if id == 1 { "[{\"type\":\"text\",\"text\":\"fake 1\"}]" } else { "" }}}));
        ev.push(json!({"type": "message_state", "id": id, "state": if id == 3 { json!({"state": "queued", "since_ms": 1})} else { json!("delivered") }}));
    }
    let mut h = Hub::new("/w");
    assert!(h.replay(&ev).is_empty());
    let full = h.raw(&json!({"t": "view_all"}));
    let rows = full["view"]["msgs"].as_array().unwrap();
    let colds = full["view"]["cold"].as_array().unwrap();
    let cold: Vec<u64> = colds.iter().map(|c| ColdMsg::decode(c).unwrap().id).collect();
    // ids 1..=144 are older than the newest 256; 2 is a question, 3 queued
    assert_eq!(cold, (1..=144).filter(|i| *i != 2 && *i != 3).collect::<Vec<_>>());
    assert_eq!(rows.len(), 400 - cold.len());
    assert_eq!(colds[0], json!([1, 1, "main", "a", null, false, 1001, false, {"state": "delivered"}, false]));
    assert_eq!(h.st.msg(1), None);
    assert_eq!(h.st.peek(1).map(|m| (m.to, m.created_ms)), Some(("a", 1001)));
    assert!(h.st.msg(2).is_some() && h.st.msg(145).is_some());
    assert_eq!(h.st.whole_msgs().count(), 400 - cold.len());
    // no row carries words: a whole one's keys are its header
    assert!(rows.iter().all(|r| r["msg"].get("text").is_none() && r["msg"].get("parts").is_none()), "{}", rows[0]);
    assert_eq!(rows.iter().find(|r| r["msg"]["id"] == 145).unwrap()["msg"]["has_parts"], false);
    // the words of every one, whole or cold, from the journal
    let w = h.st.words(1).unwrap();
    assert_eq!((w.text.as_str(), w.parts.as_str()), ("fake 1", "[{\"type\":\"text\",\"text\":\"fake 1\"}]"));
    assert_eq!(h.st.text(2), "fake 2");
    assert_eq!(h.st.text(145), "fake 145");
    assert_eq!(h.st.text(401), bise_proto::core::body::Missing(401).text());
}

/// view-all-perf: a question card's note cites main's message with its
/// words even when view_all sent it cold (c6: read in the journal).
#[test]
fn a_card_note_on_a_cold_message_carries_its_words() {
    let ws = json!({"mode": "shared", "path": "/w", "branch": null, "base_commit": null, "dropped": false});
    let mut ev = vec![json!({"type": "task_created", "name": "a", "parent": "main", "ws": ws, "at_ms": 0, "brief": {"objective": "fake"}})];
    ev.push(json!({"type": "card_opened", "card": {"id": 1, "agent": "a", "kind": "question", "text": "fake question", "for_msg": 999, "created_ms": 1000}}));
    for id in 1..=300u64 {
        let (from, to) = if id == 1 { ("main", "a") } else { ("a", "main") };
        ev.push(json!({"type": "message_sent", "msg": {"id": id, "thread": id, "from": from, "to": to,
            "reply_to": null, "expect_reply": false, "auto": false, "text": format!("fake {}", id), "created_ms": 1000 + id, "plain": false}}));
        ev.push(json!({"type": "message_state", "id": id, "state": "delivered"}));
    }
    let mut h = Hub::new("/w");
    assert!(h.replay(&ev).is_empty());
    assert_eq!(h.st.msg(1), None, "main's message is cold after view_all");
    let snap = h.snapshot(2000);
    let card = snap.cards.iter().find(|c| c.id == 1).expect("the card");
    assert_eq!(card.note.as_deref(), Some("@main wrote to @a since then (m_1): fake 1"));
}

/// The view's fixture lines (bise-proto's fixtures/core_view.jsonl, one
/// real view per shape) load into a hub's mirror: every agent row and
/// message of a full view is there, and a step's partial view keeps the
/// rest of the mirror.
#[test]
fn every_fixture_view_loads_into_the_mirror() {
    let text = include_str!("../../proto/fixtures/core_view.jsonl");
    let mut h = Hub::new("/w");
    for l in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(l).unwrap();
        let before: BTreeSet<String> = h.st.agents.keys().cloned().collect();
        h.load_view(&v);
        let names: Vec<&str> = v["agents"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
        for n in &names {
            assert!(h.st.agents.contains_key(*n), "{n} loaded from {l}");
        }
        // a partial view without agents keeps the mirror's, minus the
        // ones its order no longer names (a rename; the fixture's views
        // come from different hubs)
        if v["all_agents"] == json!(false) && names.is_empty() {
            let want: BTreeSet<String> = match v["order"].as_array() {
                Some(o) => before.into_iter().filter(|n| o.iter().any(|x| x == n.as_str())).collect(),
                None => before,
            };
            assert_eq!(h.st.agents.keys().cloned().collect::<BTreeSet<_>>(), want, "a partial view keeps the mirror: {l}");
        }
        assert_eq!(h.st.next_msg, v["next_msg"].as_u64().unwrap());
    }
}

/// debt-audit #1 (issue 35): the hub's State copies these view keys by
/// hand, one load line each in core_view.rs (`self.st.<field> = ...`);
/// everything else of sb-core's view comes in as a generated type
/// (agents: ViewAgent whole; cards, messages: core::state's). The list
/// only shrinks: a new hand copy is red (state-schema's follow-up takes
/// these to zero with the view's scalars as one record).
const HAND_MIRRORS: &[&str] = &["order", "main_notes", "next_msg", "next_card", "paused"];

#[test]
fn the_hand_mirrors_of_the_view_only_shrink() {
    let src = include_str!("core_view.rs");
    let typed = ["agents", "cards"];
    let mut copied: Vec<&str> = src
        .lines()
        .filter_map(|l| l.trim().strip_prefix("self.st."))
        .filter_map(|l| l.split_once(" = ").map(|(f, _)| f))
        .filter(|f| !typed.contains(f))
        .collect();
    copied.sort();
    copied.dedup();
    let mut want = HAND_MIRRORS.to_vec();
    want.sort();
    assert_eq!(copied, want, "State's hand copies of sb-core's view (HAND_MIRRORS may only shrink)");
}
