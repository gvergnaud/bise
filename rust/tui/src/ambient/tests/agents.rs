//! Round 10: the agents view (identity10 #data, ambient-lead m_7026)
//! (moved out of ambient/tests.rs, architect m_13367: a pure move, no
//! test changed).

use super::*;


fn agents_state(perf: &str) -> Value {
    json!({"ev": "state",
        "agents": [
            {"name": "main", "main": true, "status": "idle"},
            {"name": "perf", "status": perf, "objective": "make the e2e fast\nmore", "note": "profiling the hub", "report": "", "created_ms": 5},
            {"name": "old", "status": "archived", "objective": "an old fix", "report": "fixed in 0.4", "report_ms": 7},
        ],
        "cards": [{"id": 9, "kind": "question", "agent": "perf", "text": "which bench?\n1. cold\n2. warm"}],
        "pages": [{"id": "perf-notes", "title": "Perf notes", "agent": "perf", "version": 2, "url": "http://p/perf-notes", "at_ms": 3}]})
}

#[test]
fn agents_rows_carry_status_title_purpose_since_waits_and_archived() {
    let mut t = T::new();
    t.ready();
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    let rows = st["agents"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "main is not a row, an archived agent is: {rows:?}");
    let (perf, old) = (&rows[0], &rows[1]);
    assert_eq!(perf["status"], "working");
    assert_eq!(perf["title"], "profiling the hub");
    assert_eq!(perf["purpose"], "make the e2e fast");
    assert_eq!(perf["waits"], 1);
    assert_eq!(perf["archived"], false);
    assert_eq!(perf["followed"], false);
    assert_eq!(perf["since_ms"], 5, "first sight: its birth");
    assert_eq!((old["status"].as_str(), old["archived"].as_bool(), old["title"].as_str()), (Some("done"), Some(true), Some("fixed in 0.4")));
    assert_eq!(old["since_ms"], 7);
    // the status changes: since moves; follow shows on the row
    t.hub.say(agents_state("idle"));
    t.until(|o| has(o, "state"));
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    assert_eq!(st["agents"][0]["status"], "idle");
    assert!(st["agents"][0]["since_ms"].as_u64().unwrap() > 1_000_000);
    t.cmd(Cmd::Follow { agent: "perf".into(), on: true });
    let st = t.take().into_iter().find(|v| v["ev"] == "state").unwrap();
    assert_eq!(st["agents"][0]["followed"], true);
}

#[test]
fn agents_cmds_parse() {
    let p = |s: &str| Cmd::parse(s);
    assert_eq!(p(r#"{"cmd":"agent_preview","agent":"@perf"}"#), Ok(Cmd::AgentPreview { agent: "perf".into() }));
    assert_eq!(p(r#"{"cmd":"agent_history","agent":"perf"}"#), Ok(Cmd::AgentHistory { agent: "perf".into(), before: None, limit: 60 }));
    assert_eq!(p(r#"{"cmd":"agent_history","agent":"perf","before":40,"limit":20}"#), Ok(Cmd::AgentHistory { agent: "perf".into(), before: Some(40), limit: 20 }));
    assert_eq!(p(r#"{"cmd":"agent_send","agent":"perf","text":"hi","mode":"queued"}"#), Ok(Cmd::AgentSend { agent: "perf".into(), text: "hi".into(), queued: true }));
    assert_eq!(p(r#"{"cmd":"agent_send","agent":"perf","text":"hi","mode":"now"}"#), Ok(Cmd::AgentSend { agent: "perf".into(), text: "hi".into(), queued: false }));
    assert_eq!(p(r#"{"cmd":"archive","agent":"perf","stop_first":true}"#), Ok(Cmd::Archive { agent: "perf".into(), stop_first: true }));
    assert_eq!(p(r#"{"cmd":"unarchive","agent":"perf"}"#), Ok(Cmd::Unarchive { agent: "perf".into() }));
    assert_eq!(p(r#"{"cmd":"follow","agent":"perf","on":false}"#), Ok(Cmd::Follow { agent: "perf".into(), on: false }));
    assert!(p(r#"{"cmd":"stop"}"#).is_err(), "no agent: the app's bug");
}

/// perf's feed: his message, a reply, three tool calls, a report.
fn perf_lines() -> Vec<Line> {
    let l = |pos: u64, line: &str| (pos, 1_000 + pos, line.to_string());
    vec![
        l(1, "sb you : make it fast"),
        l(2, "  obs: turn_started"),
        l(3, "  obs: assistant: <think>hmm</think>on it"),
        l(4, "  obs: tool_started #1"),
        l(4, "tool #1 bash : cargo test -q"),
        l(5, "tool_intent #1 : running the tests"),
        l(6, "tool_result #1 ok : 3 failed"),
        l(7, "  obs: tool_started #2"),
        l(7, "tool #2 read_file : {\"path\":\"a.rs\"}"),
        l(8, "  obs: tool_started #3"),
        l(8, "tool #3 bash : sb land \"fast\""),
        l(9, "tool_intent #3 : landing the fix"),
        l(10, "sb msg-in : ambient-lead m_3 : nice\\nsecond line"),
        l(11, "  obs: tool_started #4"),
        l(11, "tool #4 bash : sb report done \"the e2e takes 40 s\""),
        l(12, "  obs: turn_done: completed"),
    ]
}

const PROJECT: &str = super::fake_hub::HOME;

/// The hub's fold of perf's lines (bise_proto::thread, what the hub
/// sends): card 9 open, perf-notes published.
fn hub_fold(lines: &[Line]) -> Vec<Entry> {
    let pages = |id: &str| {
        (id == "perf-notes").then(|| PageRef { id: id.into(), title: "Perf notes".into(), v: Some(2), url: "http://p/perf-notes".into() })
    };
    thread::fold(lines, &Ctx { open_cards: &[9], page: &pages, provider: &crate::models::provider_name, width: &unicode_width::UnicodeWidthStr::width, offset: &|_| 0, attached: &bise_proto::thread::Attached::plain })
}

/// The hub's typed hello answered (the target's project).
fn welcome(t: &mut T) {
    t.hub.say(json!({"ev": "welcome", "project": PROJECT, "proto": 1, "workspace": "/w", "name": "w"}));
}

fn thread_ev(agent: &str, entries: &[Entry], before: Option<u64>, more: bool) -> Value {
    json!({"ev": "thread", "project": PROJECT, "agent": agent, "entries": entries, "before": before, "more": more})
}

fn entry_ev(agent: &str, e: &Entry) -> Value {
    json!({"ev": "entry", "project": PROJECT, "agent": agent, "entry": e})
}

#[test]
fn a_preview_shows_now_the_last_actions_what_waits_the_report_and_pages() {
    let mut t = T::new();
    t.ready();
    welcome(&mut t);
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    t.take();
    t.cmd(Cmd::AgentPreview { agent: "perf".into() });
    assert_eq!(t.hub.next(), json!({"cmd": "subscribe", "agent": "perf", "limit": 8, "project": PROJECT}));
    t.hub.say(thread_ev("perf", &hub_fold(&perf_lines()), None, false));
    t.until(|o| has(o, "agent_preview"));
    let p = t.take().into_iter().find(|v| v["ev"] == "agent_preview").unwrap();
    let acts: Vec<(&str, &str)> = p["actions"].as_array().unwrap().iter().map(|a| (a["kind"].as_str().unwrap(), a["text"].as_str().unwrap())).collect();
    assert_eq!(acts, [("message", "on it"), ("tool", "running the tests"), ("tool", "read_file: {\"path\":\"a.rs\"}"), ("land", "landing the fix"), ("report", "the e2e takes 40 s")]);
    assert_eq!(p["waiting"], json!([{"card_id": 9, "question": "which bench?"}]));
    assert_eq!(p["last_report"], json!({"kind": "done", "text": "the e2e takes 40 s", "at_ms": 1011}));
    assert_eq!(p["pages"], json!([{"id": "perf-notes", "title": "Perf notes", "v": 2, "url": "http://p/perf-notes"}]));
    assert_eq!(p["now"], "profiling the hub");
    // its next step re-sends the preview: now is that step
    t.hub.say(json!({"ev": "typing", "project": PROJECT, "agent": "perf", "text": "listing the bench files"}));
    t.until(|o| o.iter().any(|v| v["ev"] == "agent_preview" && v["now"] == "listing the bench files"));
    // another agent selected: its entries send nothing; perf's thread
    // stays subscribed, the capsule's (its words to him, core/home.rs)
    t.cmd(Cmd::AgentPreview { agent: "old".into() });
    assert_eq!(t.hub.next(), json!({"cmd": "subscribe", "agent": "old", "limit": 8, "project": PROJECT}));
    t.take();
    let mut lines = perf_lines();
    lines.push((13, 1_013, "  obs: assistant: done".into()));
    t.hub.say(entry_ev("perf", hub_fold(&lines).last().unwrap()));
    t.sync();
    assert!(!t.take().iter().any(|v| v["ev"] == "agent_preview" && v["agent"] == "perf"));
}

#[test]
fn a_history_shows_the_hubs_entries_older_and_follows_live() {
    let mut t = T::new();
    t.ready();
    welcome(&mut t);
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    t.take();
    t.cmd(Cmd::AgentHistory { agent: "perf".into(), before: None, limit: 60 });
    assert_eq!(t.hub.next(), json!({"cmd": "subscribe", "agent": "perf", "limit": 60, "project": PROJECT}));
    let mut lines = perf_lines();
    lines.push((13, 1_013, "sb you : and the cold bench?".into()));
    t.hub.say(thread_ev("perf", &hub_fold(&lines), None, false));
    t.until(|o| has(o, "agent_history"));
    let h = t.take().into_iter().find(|v| v["ev"] == "agent_history").unwrap();
    let kinds: Vec<&str> = h["entries"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["you", "agent", "tools", "from-agent", "report", "you", "card"], "from_agent is the panel's from-agent");
    let e = &h["entries"];
    assert_eq!((e[0]["pos"].as_u64(), e[0]["at_ms"].as_u64(), e[0]["text"].as_str()), (Some(1), Some(1001), Some("make it fast")));
    assert_eq!(e[1]["text"], "on it", "no thinking");
    assert_eq!((e[2]["pos"].as_u64(), e[2]["tools"]["count"].as_u64(), e[2]["text"].as_str()), (Some(4), Some(3), Some("read 1 file, ran 2 commands")), "the hub's counted summary");
    assert_eq!((e[3]["text"].as_str(), e[3]["from"].as_str()), (Some("nice\nsecond line"), Some("ambient-lead")));
    assert_eq!(e[4]["report"]["kind"], "done");
    assert_eq!(e[6]["card"], json!({"id": 9, "question": "which bench?", "options": [{"n": 1, "label": "cold"}, {"n": 2, "label": "warm"}], "answered": false}));
    assert_eq!((h["before"].clone(), h["more"].clone()), (Value::Null, json!(false)));
    // live: the hub's changed entries and the step pass to the panel
    lines.push((14, 1_014, "  obs: tool_started #6".into()));
    lines.push((14, 1_014, "tool #6 bash : sb page publish notes.html --id perf-notes".into()));
    lines.push((15, 1_015, "  obs: tool_started #7".into()));
    lines.push((15, 1_015, "tool #7 bash : ls".into()));
    lines.push((16, 1_016, "tool_intent #7 : listing".into()));
    for e in hub_fold(&lines).iter().filter(|e| e.pos >= 14) {
        t.hub.say(entry_ev("perf", e));
    }
    t.hub.say(json!({"ev": "typing", "project": PROJECT, "agent": "perf", "text": "listing"}));
    t.until(|o| o.iter().any(|v| v == &json!({"ev": "agent_typing", "agent": "perf", "text": "listing"})));
    let out = t.take();
    let page = out.iter().find(|v| v["ev"] == "agent_entry" && v["entry"]["kind"] == "page").unwrap();
    assert_eq!(page["entry"]["page"], json!({"id": "perf-notes", "title": "Perf notes", "v": 2, "url": "http://p/perf-notes"}));
    assert!(out.iter().any(|v| v["ev"] == "agent_entry" && v["entry"]["tools"]["items"][0]["text"] == "listing"));
    // the same entry again (a replay): nothing
    t.hub.say(entry_ev("perf", hub_fold(&lines).last().unwrap()));
    // the card answered (the state has no card now): only its entry says so
    t.hub.say(json!({"ev": "state", "agents": agents_state("working")["agents"], "cards": []}));
    t.until(|o| o.iter().any(|v| v["ev"] == "agent_entry" && v["entry"]["card"]["answered"] == true));
    let out = t.take();
    assert_eq!(out.iter().filter(|v| v["ev"] == "agent_entry").count(), 1, "the replayed entry sends nothing: {out:?}");
    // older entries: before the first pos, none at once; else a page of the hub
    t.cmd(Cmd::AgentHistory { agent: "perf".into(), before: Some(1), limit: 60 });
    assert_eq!(t.take().into_iter().find(|v| v["ev"] == "agent_history").unwrap()["entries"], json!([]));
    t.cmd(Cmd::AgentHistory { agent: "perf".into(), before: Some(11), limit: 2 });
    assert_eq!(t.hub.next(), json!({"cmd": "page", "agent": "perf", "before": 11, "limit": 2, "project": PROJECT}));
    let older: Vec<Line> = perf_lines().into_iter().take(13).skip(2).collect();
    let (entries, before, more) = thread::page(&older, &[], &Ctx { open_cards: &[], page: &|_: &str| None, provider: &crate::models::provider_name, width: &unicode_width::UnicodeWidthStr::width, offset: &|_| 0, attached: &bise_proto::thread::Attached::plain }, 2);
    t.hub.say(thread_ev("perf", &entries, before, more));
    t.until(|o| has(o, "agent_history"));
    let h = t.take().into_iter().find(|v| v["ev"] == "agent_history").unwrap();
    let kinds: Vec<&str> = h["entries"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["tools", "from-agent"], "the hub's page, as the panel's");
    assert_eq!((h["before"].as_u64(), h["more"].as_bool()), (Some(4), Some(true)));
    // the panel closes: no more entries (its thread stays the capsule's)
    t.cmd(Cmd::AgentUnwatch { agent: "perf".into() });
    lines.push((17, 1_017, "  obs: assistant: bye".into()));
    t.hub.say(entry_ev("perf", hub_fold(&lines).last().unwrap()));
    t.sync();
    assert!(!has(&t.take(), "agent_entry"));
}

#[test]
fn a_reconnection_subscribes_again_and_sends_only_what_changed() {
    let mut t = T::new();
    t.ready();
    welcome(&mut t);
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    t.cmd(Cmd::AgentHistory { agent: "perf".into(), before: None, limit: 60 });
    assert_eq!(t.hub.next()["cmd"], "subscribe");
    let mut lines = perf_lines();
    t.hub.say(thread_ev("perf", &hub_fold(&lines), None, false));
    t.until(|o| has(o, "agent_history"));
    t.take();
    // the hub restarts: the core says hello again; its welcome resubscribes,
    // never re-sends a command
    t.hub.w.shutdown(std::net::Shutdown::Both).unwrap();
    t.hub = t.ends.recv_timeout(Duration::from_secs(3)).expect("reconnected");
    t.until(|o| o.iter().any(|v| v["ev"] == "hub" && v["up"] == true));
    t.take();
    welcome(&mut t);
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    assert_eq!(t.hub.next(), json!({"cmd": "subscribe", "agent": "perf", "limit": 60, "project": PROJECT}));
    lines.push((13, 1_013, "  obs: assistant: back".into()));
    t.hub.say(thread_ev("perf", &hub_fold(&lines), None, false));
    t.until(|o| has(o, "agent_entry"));
    t.sync();
    let out = t.take();
    assert!(!has(&out, "agent_history"), "no new page for an open panel: {out:?}");
    let pos: Vec<u64> = out.iter().filter(|v| v["ev"] == "agent_entry").map(|v| v["entry"]["pos"].as_u64().unwrap()).collect();
    assert_eq!(pos, [13], "only the new entry");
}

#[test]
fn acts_send_now_queued_stop_archive_unarchive() {
    let mut t = T::new();
    t.ready();
    t.hub.say(agents_state("working"));
    t.until(|o| has(o, "state"));
    t.take();
    t.cmd(Cmd::AgentSend { agent: "perf".into(), text: " look at the cold bench ".into(), queued: false });
    assert_eq!(t.hub.sent(), json!({"cmd": "send", "agent": "perf", "text": "look at the cold bench", "via": "ambient"}));
    assert!(t.take().contains(&json!({"ev": "sent", "agent": "perf", "mode": "now"})));
    // queued while it works: the hub holds it (sb-core), never the core
    t.cmd(Cmd::AgentSend { agent: "perf".into(), text: "then the warm one".into(), queued: true });
    assert_eq!(t.hub.sent(), json!({"cmd": "send", "agent": "perf", "text": "then the warm one", "via": "ambient", "mode": "queued"}));
    let out = t.take();
    assert!(out.contains(&json!({"ev": "agent_queued", "agent": "perf", "text": "then the warm one"})));
    assert!(!out.iter().any(|v| v["ev"] == "phase"), "never main's orb");
    t.hub.say(agents_state("idle"));
    t.until(|o| has(o, "state"));
    t.cmd(Cmd::Stop { agent: "perf".into() });
    // turn/interrupt (the fake end gives a request back as its HubCmd)
    let stop = t.hub.next();
    assert_eq!((stop["cmd"].as_str(), stop["agent"].as_str()), (Some("stop"), Some("perf")), "nothing resent at idle: {stop}");
    t.cmd(Cmd::Archive { agent: "perf".into(), stop_first: true });
    assert_eq!(t.hub.sent(), json!({"cmd": "archive", "agent": "perf", "force": true}));
    t.cmd(Cmd::Archive { agent: "perf".into(), stop_first: false });
    assert_eq!(t.hub.sent(), json!({"cmd": "archive", "agent": "perf", "force": false}));
    t.cmd(Cmd::Unarchive { agent: "old".into() });
    assert_eq!(t.hub.sent(), json!({"cmd": "unarchive", "agent": "old"}));
    t.cmd(Cmd::AgentSend { agent: "nobody".into(), text: "hi".into(), queued: false });
    assert!(has(&t.take(), "error"));
}

#[test]
fn a_preview_of_an_agent_the_hub_does_not_know_comes_at_once() {
    // the QA scenes inject agents into the page only (m_7106): the core
    // still answers, empty, so the bar never waits
    let mut t = T::new();
    t.ready();
    t.cmd(Cmd::AgentPreview { agent: "cookies".into() });
    let p = t.take().into_iter().find(|v| v["ev"] == "agent_preview").expect("a preview at once");
    assert_eq!(p, json!({"ev": "agent_preview", "agent": "cookies", "now": "", "actions": [], "waiting": [], "last_report": null, "pages": []}));
    // asked again (the page asks once, the app may relaunch the page): again
    t.cmd(Cmd::AgentPreview { agent: "cookies".into() });
    assert!(has(&t.take(), "agent_preview"));
}
