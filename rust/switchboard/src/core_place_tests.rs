//! Where an agent works (BISE-136, wt-sign): `sb worktree`, the bash
//! fallback, and main, who always works in the workspace.

use super::*;

/// BISE-136: `sb worktree <path>` (gate.sh new) puts the private worktree
/// in the snapshot and `sb tasks`; `none` takes it back. Before any
/// `sb worktree`, a bash call that starts in a linked worktree (a `.git`
/// file) is the fallback; after one, the fallback no longer guesses.
#[test]
fn an_agent_says_where_it_works() {
    let mut t = T::new();
    t.spawn_task("docs");
    let place = |t: &mut T| {
        t.hub.snapshot(0).agent("docs").place.clone()
    };
    assert!(place(&mut t).is_none());
    // the fallback: a linked worktree has a .git file
    let wt = std::env::temp_dir().join(format!("sb-place-{}", std::process::id()));
    std::fs::create_dir_all(&wt).unwrap();
    std::fs::write(wt.join(".git"), "gitdir: /x").unwrap();
    let wt = wt.to_string_lossy().to_string();
    let bash = |t: &mut T, cmd: &str| {
        t.go(Input::ReplLine {
            agent: "docs".into(),
            line: format!("tool #1 bash : {}", json!({"arg": cmd})),
        })
    };
    bash(&mut t, "cd /tmp && ls");
    assert!(place(&mut t).is_none(), "not a linked worktree");
    let fx = bash(&mut t, &format!("cd {} && cargo test", wt));
    assert_eq!(place(&mut t), Some(wt.clone()));
    assert!(fx.contains(&Effect::State), "the views learn it");
    // wt-sign (Benjamin's 'ψ ven-bots' on main's line): main looking into
    // a task's worktree is no move; main works in the workspace
    t.go(Input::ReplLine { agent: MAIN.into(), line: format!("tool #2 bash : {}", json!({"arg": format!("cd {} && git log", wt)})) });
    let snap = t.hub.snapshot(0);
    let main = snap.agent(MAIN);
    assert!(main.place.is_none() && main.place_id == "shared", "{main:?}");
    // told: the fallback stops guessing
    let (tok, fx) = t.req("docs", AgentReq::Worktree { path: "/tmp/docs-wt".into() });
    assert!(fx.contains(&Effect::Reply { token: tok, body: json!({"ok": true, "path": "/tmp/docs-wt"}) }), "{:?}", fx);
    assert_eq!(place(&mut t).as_deref(), Some("/tmp/docs-wt"));
    let (_, fx) = t.req(MAIN, AgentReq::Tasks);
    assert!(format!("{:?}", fx).contains("private worktree /tmp/docs-wt"), "{:?}", fx);
    bash(&mut t, &format!("cd {} && cargo test", wt));
    assert_eq!(place(&mut t).as_deref(), Some("/tmp/docs-wt"));
    // none: its own workspace again
    t.req("docs", AgentReq::Worktree { path: String::new() });
    assert!(place(&mut t).is_none());
    // the CLI's path: an absolute path or none (a relative one refused)
    let r = |p: &str| crate::core::pure::worktree_path(p);
    assert_eq!(r("none"), Ok(String::new()));
    assert_eq!(r("/tmp/x-wt/"), Ok("/tmp/x-wt".into()));
    assert!(r("x-wt").is_err());
    let (tok, fx) = t.req("docs", AgentReq::Worktree { path: "x-wt".into() });
    assert!(fx.iter().any(|e| matches!(e, Effect::Reply { token, body } if *token == tok && body["ok"] == false)), "{:?}", fx);
    // qa-explore B: main works in the workspace, it cannot mark itself
    let (tok, fx) = t.req(MAIN, AgentReq::Worktree { path: "/tmp/docs-wt".into() });
    assert!(fx.iter().any(|e| matches!(e, Effect::Reply { token, body } if *token == tok && body["ok"] == false)), "{:?}", fx);
    let _ = std::fs::remove_dir_all(&wt);
}

/// brief-feature-merge: his merge, try or drop that did not go through
/// (a conflict, a red check, a race): his item closes once, `handed to
/// main`, no item opens, and main gets exactly one message, the brief.
/// An agent's own `sb feature` that failed (no handoff, a feed line)
/// sends nothing and touches no item.
#[test]
fn a_failed_feature_answer_is_handed_to_main_once() {
    for (kind, op) in [("feature_merge", "merge"), ("feature_try", "try"), ("feature_merge", "drop")] {
        let mut t = T::new();
        let place = crate::feature::place_id("bims");
        let text = "merge bims into main?\n3 commits.\n\n1. merge\n2. keep working\n3. drop the branch";
        let open = ci::CardOpen { kind: kind.into(), agent: MAIN.into(), text: text.into(), place: Some(place.clone()), pr: None };
        t.hub.core(&mut Vec::new(), &mut t.env, None, In::CardOpen(open));
        let id = *t.hub.st.cards.keys().max().unwrap();
        let msgs = |t: &T| t.hub.st.msgs().values().filter(|m| m.to == MAIN).map(|m| t.hub.st.text(m.id)).collect::<Vec<_>>();
        let before = msgs(&t).len();
        // an agent's own step failed: its stream said it; nothing here
        let line = Some(Feed::warn("bims: sync did not go through: x"));
        t.go(Input::Feature(FeatureDone { name: "bims".into(), line, ..FeatureDone::default() }));
        assert!(t.hub.st.cards.contains_key(&id), "{op}: the item stays");
        assert_eq!(msgs(&t).len(), before, "{op}: no message");
        // his answer failed
        let brief = format!("the user picked '{op}' on feature bims at ...");
        let fx = t.go(Input::Feature(FeatureDone { name: "bims".into(), handoff: Some(brief.clone()), ..FeatureDone::default() }));
        assert!(!t.hub.st.cards.contains_key(&id), "{op}: the item closed");
        assert_eq!(t.hub.st.open_cards().count(), 0, "{op}: nothing opens again");
        let got = msgs(&t);
        assert_eq!(got.len(), before + 1, "{op}: one message: {got:?}");
        assert!(got.iter().any(|m| m.contains(&brief)), "{op}: the brief: {got:?}");
        let closed = recs_of(&fx, MAIN, "card-closed");
        assert_eq!(closed.len(), 1, "{op}: closed once: {closed:?}");
        assert_eq!((closed[0]["id"].as_u64(), closed[0]["res"].as_str()), (Some(id), Some(crate::feature::HANDED)), "{op}: {closed:?}");
    }
}

// moved from core_tests.rs (hub-loop: its line count)
#[test]
fn worktrees_need_git() {
    let mut t = T::new();
    t.env.git = false;
    let fx = t.user(MAIN, "/new -w x: y");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("not a git repository"))));
    assert!(!t.hub.st.agents.contains_key("x"));
}

/// rows233B (desk-cut-15 m_31808): `sb spawn b --place a` while a's
/// worktree is still being made joins a's place: b gets no worktree of
/// its own (no `worktree_create` for b), no REPL before a's is made, then
/// a's path and branch; a's failure fails b too, with git's words.
#[test]
fn a_join_while_placing_waits_for_its_place() {
    for fails in [false, true] {
        let mut t = T::new();
        let spawn = |t: &mut T, name: &str, place: &str| {
            t.token += 1;
            let req = AgentReq::Spawn {
                name: name.into(),
                brief: Brief { objective: format!("objective of {}", name), ..Brief::default() },
                worktree: place == "new",
                with_changes: false,
                place: if place == "new" { String::new() } else { place.into() },
                feature: String::new(),
                ask: Default::default(),
                parts: String::new(),
                attach: Vec::new(),
                fwd: None,
            };
            // the hub alone: the shell's worker has not answered yet
            t.hub.handle(Input::Agent { token: t.token, from: MAIN.into(), req, by: crate::peer::Sender::hub() }, &mut t.env)
        };
        let creates = |fx: &[Effect]| fx.iter().filter(|e| matches!(e, Effect::Place(crate::place_job::Job::Create { .. }))).count();
        let fx = spawn(&mut t, "a", "new");
        assert_eq!(creates(&fx), 1, "{:?}", fx);
        assert!(t.hub.st.agents["a"].ws.placing());
        let fx = spawn(&mut t, "b", "a");
        assert_eq!(creates(&fx), 0, "b joins a's place, no worktree of its own: {:?}", fx);
        assert!(!fx.iter().any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == "b")), "no REPL before placed: {:?}", fx);
        assert!(t.hub.st.agents["b"].ws.placing());
        if fails {
            let fx = t.go(Input::Placed { name: "a".into(), ws: Value::Null, err: "fatal: no space left".into() });
            for n in ["a", "b"] {
                let ag = &t.hub.st.agents[n];
                assert_eq!(ag.lifecycle, Lifecycle::Failed, "{n}: {:?}", ag.lifecycle);
                let told = format!("@{n} could not start: its worktree was not made: fatal: no space left");
                assert!(format!("{:?}", fx).contains(&told), "{told}: {:?}", fx);
            }
            assert!(t.env.dropped.is_empty());
            continue;
        }
        let made = crate::place_job::placed_now(&mut t.env, &Effect::Place(crate::place_job::Job::Create { name: "a".into(), with_changes: false, feature: String::new() })).unwrap();
        let fx = t.go(made);
        assert!(t.env.dropped.is_empty());
        let (wa, wb) = (t.hub.st.agents["a"].ws.clone(), t.hub.st.agents["b"].ws.clone());
        assert_eq!(wa, wb, "one place, the same ws");
        assert_eq!((wb.path.as_str(), wb.branch.as_deref(), wb.place.as_deref()), ("/state/worktrees/a", Some("sb/a"), Some("wt:a")));
        assert!(!wb.placing());
        for n in ["a", "b"] {
            assert!(fx.iter().any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == n)), "{n} starts: {:?}", fx);
        }
        assert_eq!(t.hub.snapshot(0).places[0].agents, ["a", "b"]);
    }
}

/// The home workspace (docs/ambient-pages.md §5.1): land, feature and
/// flow say "not in a repo" in one line, nothing goes to git.
#[test]
fn land_feature_and_flow_say_not_in_a_repo_without_git() {
    let mut t = T::new();
    t.env.git = false;
    let reqs = [
        ("land", AgentReq::Land { here: true, message: "x".into(), add: Vec::new() }),
        ("feature", AgentReq::Feature { op: "new".into(), name: "foo".into() }),
        ("feature", AgentReq::Feature { op: "list".into(), name: String::new() }),
        ("flow", AgentReq::Flow { set: String::new() }),
    ];
    for (cmd, req) in reqs {
        let (tok, fx) = t.req(MAIN, req);
        let r = reply(&fx, tok).unwrap();
        assert_eq!(r["ok"], false);
        let e = r["error"].as_str().unwrap();
        assert!(e.starts_with(&format!("sb {}: not in a repo", cmd)) && !e.contains('\n'), "{}", e);
        assert!(!fx.iter().any(|e| matches!(e, Effect::Land { .. } | Effect::Feature { .. } | Effect::Flow { .. })));
    }
    // with git: the land goes on to the daemon
    t.env.git = true;
    let (_, fx) = t.req(MAIN, AgentReq::Land { here: true, message: "x".into(), add: Vec::new() });
    assert!(fx.iter().any(|e| matches!(e, Effect::Land { .. })));
}

/// inbox-reappear (his #780): a feature item he answered with a step
/// (merge, try, drop) says `running` while that step runs, so no client
/// shows it to him again meanwhile; a land or an agent's sync on the same
/// feature is not his answer; the step's end (done, or the shell's
/// refusal: a busy feature) ends it, and the item it opens runs nothing.
#[test]
fn a_feature_item_runs_his_answer_until_its_step_ends() {
    let mut t = T::new();
    let place = crate::feature::place_id("cu");
    let open = |t: &mut T, kind: &str| {
        let text = "merge cu into main?\nyou tried abc: x\n3 commits.\n\n1. merge\n2. keep working\n3. drop the branch";
        let open = ci::CardOpen { kind: kind.into(), agent: MAIN.into(), text: text.into(), place: Some(place.clone()), pr: None };
        t.hub.core(&mut Vec::new(), &mut t.env, None, In::CardOpen(open));
        *t.hub.st.cards.keys().max().unwrap()
    };
    let running = |t: &mut T, id: u64| t.hub.snapshot(0).cards.iter().find(|c| c.id == id).map(|c| c.running);
    let id = open(&mut t, "feature_merge");
    assert_eq!(running(&mut t, id), Some(false), "not answered yet");
    // an agent's own step on the feature (a sync, a land's turn): not his answer
    t.go(Input::Feature(FeatureDone { name: "cu".into(), ..FeatureDone::default() }));
    assert_eq!(running(&mut t, id), Some(false));
    // `2 keep working` runs no step of his
    t.user(MAIN, &format!("/answer {} 2", id));
    assert_eq!(running(&mut t, id), Some(false), "keep working");
    let fx = t.user(MAIN, &format!("/answer {} 1", id));
    assert!(fx.iter().any(|e| matches!(e, Effect::Feature { op, .. } if op == "merge")), "{fx:?}");
    assert_eq!(running(&mut t, id), Some(true), "his merge runs");
    let rows = crate::proto_view::cards(&t.hub.snapshot(0), "p", 0, |_| true).0;
    assert!(rows.iter().any(|c| c.id == id && c.running), "the wire says it: {rows:?}");
    // another feature's step ends: still his merge
    t.go(Input::Feature(FeatureDone { name: "other".into(), ..FeatureDone::default() }));
    assert_eq!(running(&mut t, id), Some(true));
    // it fails: handed to main, the item closed in the same step
    t.go(Input::Feature(FeatureDone { name: "cu".into(), handoff: Some("brief".into()), ..FeatureDone::default() }));
    assert_eq!(running(&mut t, id), None, "closed");
    assert!(t.hub.running_cards().is_empty(), "nothing runs");
    // a try: running until its step ends with the merge item
    let try_id = open(&mut t, "feature_try");
    // `2 show the diff` is a client's act (it opens the diff artifact):
    // the hub runs nothing for it and the item stays (try-diff-open)
    let fx = t.user(MAIN, &format!("/answer {} 2", try_id));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Feature { .. })), "{fx:?}");
    assert_eq!(running(&mut t, try_id), Some(false), "still open, nothing runs");
    // the row says it: its option 2 opens the diff kept (the shell's
    // snapshot_feature_acts), 1 and 3 answer
    let mut snap = t.hub.snapshot(0);
    let c = snap.cards.iter_mut().find(|c| c.id == try_id).unwrap();
    c.opens.extend(crate::feature::try_acts(&c.kind, "cu", Some("cu-diff")));
    let row = crate::proto_view::cards(&snap, "p", 0, |_| true).0.into_iter().find(|c| c.id == try_id).unwrap();
    let act = crate::feature::try_acts(crate::feature::TRY, "cu", Some("cu-diff")).map(|(_, a)| a);
    let acts: Vec<(u32, bool)> = row.options.iter().map(|o| (o.n, o.act.is_some() && o.act == act)).collect();
    assert_eq!(acts, vec![(1, false), (2, true), (3, false)]);
    t.user(MAIN, &format!("/answer {} 1", try_id));
    assert_eq!(running(&mut t, try_id), Some(true), "its try builds");
    t.go(Input::Feature(FeatureDone { name: "cu".into(), close: Some("replaced".into()), ..FeatureDone::default() }));
    assert!(t.hub.running_cards().is_empty(), "nothing runs");
}
