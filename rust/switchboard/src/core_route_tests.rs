//! His `@agent` words routed: to a busy task, and typed in another
//! thread (at-start's trace); his `@<project>` words on bise's home hub
//! (route_ask, xhub.bend `addressed`) and the routing hold. Out of
//! core_tests.rs (its line limit).

use super::*;

/// `h`'s step: its journal events kept as the daemon's append keeps them
/// (T::go's, for a hub driven alone): the mirror reads its words there.
fn step(h: &mut Hub, env: &mut FakeEnv, i: Input) -> Vec<Effect> {
    let fx = h.handle(i, env);
    for e in &fx {
        if let Effect::Journal(ev) = e {
            h.st.bodies.keep(ev);
        }
    }
    fx
}

#[test]
fn at_task_to_a_busy_task_steers_and_answers_at_turn_end() {
    let mut t = T::new();
    t.spawn_task("docs");
    // docs is still in its first turn
    let fx = t.user(MAIN, "@docs où en es-tu ?");
    let s = steer_to(&fx, "docs").unwrap();
    assert!(s.starts_with("<user_message id=\"m_2\" via=\"main\">"), "{}", s);
    t.go(Input::ReplLine {
        agent: "docs".into(),
        line: "  obs: assistant: à mi-chemin".into(),
    });
    let fx = t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    assert!(has_rec(&fx, MAIN, json!({"sb": "msg-you", "from": "docs", "body": "à mi-chemin"})), "{:?}", fx);
    // main still gets its own automatic reply to the brief
    assert!(t.hub.st.unanswered_for("docs").is_empty());
}

/// at-start: his words for docs typed in main's thread leave one route row
/// there with the message's id (the window's '→ sent to docs'); none from
/// docs's own thread, an unknown thread, or no `from`; the message is the
/// same either way (laws say_from_*).
#[test]
fn a_say_typed_in_another_thread_leaves_one_route_row_there() {
    let mut t = T::new();
    t.spawn_task("docs");
    let fx = t.user_from("docs", "fais une map", MAIN);
    let rows = recs_of(&fx, MAIN, "route");
    assert_eq!(rows.len(), 1, "{fx:?}");
    assert_eq!((&rows[0]["who"], &rows[0]["said"], rows[0].get("card")), (&json!("docs"), &json!("fais une map"), None));
    let id = rows[0]["msg"].as_u64().expect("the message's id");
    assert!(t.hub.st.msgs().values().any(|m| m.id == id && m.to == "docs" && t.hub.st.text(m.id) == "fais une map"), "the message itself went to docs");
    assert!(recs_of(&fx, "docs", "route").is_empty(), "never in docs's own feed");
    for from in ["", "docs", "nobody"] {
        let fx = t.user_from("docs", "encore", from);
        assert!(recs_of(&fx, MAIN, "route").is_empty() && recs_of(&fx, "docs", "route").is_empty(), "from {from:?}: {fx:?}");
    }
}

// ---- desktop S2 step 2: the routing hold (core.bend's `routing hold`) ----

fn routes(fx: &[Effect]) -> Vec<(u64, String, String, u64)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Route { rid, to, name, due_ms, .. } => Some((*rid, to.clone(), name.clone(), *due_ms)),
            _ => None,
        })
        .collect()
}

fn route_dones(fx: &[Effect]) -> Vec<(u64, String, String, Option<u64>)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::RouteDone { rid, state, to, xid, .. } => Some((*rid, state.clone(), to.clone(), *xid)),
            _ => None,
        })
        .collect()
}

/// His `@shop …` to main on bise's home hub (projects shop and docs),
/// his words go whole.
fn ask_with(t: &mut T, text: &str, context: Option<Value>, parts: &str) -> Vec<Effect> {
    let projects = vec![("shop-1".to_string(), "shop".to_string()), ("docs-2".to_string(), "docs".to_string())];
    t.go(Input::RouteAsk { focus: String::new(), text: text.into(), via: String::new(), queued: false, context, parts: parts.into(), projects })
}

fn hold(t: &mut T, text: &str) -> Vec<Effect> {
    ask_with(t, &format!("@shop {text}"), None, "")
}

fn asked(fx: &[Effect]) -> Vec<bool> {
    fx.iter().filter_map(|e| match e { Effect::RouteAsked { routed } => Some(*routed), _ => None }).collect()
}

// issue 37 #1: sb-core decides the route and answers each ask once; held
// iff he addressed a registered project that no agent of this hub is named
// like (laws route_ask_*, addressed_*; their instances: bend/laws/hub.bend)
#[test]
fn a_route_ask_is_answered_once_and_holds_only_his_address() {
    let mut t = T::new();
    let fx = ask_with(&mut t, "  @Shop: why does p99 rise", None, "");
    assert_eq!(asked(&fx), vec![true]);
    assert_eq!(routes(&fx), vec![(1, "shop-1".into(), "shop".into(), t.env.now + 2000)]);
    let why: Vec<&str> = fx.iter().filter_map(|e| match e { Effect::Route { why, .. } => Some(why.as_str()), _ => None }).collect();
    assert_eq!(why, vec!["you wrote @shop"]);
    for text in ["je les voie pas dans shop non plus", "tell @shop to fix it", "@shop", "@shop   ", "@shp fix it", "@ shop fix it", "/answer 3 yes", "see https://github.com/acme/shop/pull/3"] {
        let mut t = T::new();
        let n = t.hub.st.msgs().len();
        let fx = ask_with(&mut t, text, None, "");
        assert_eq!(asked(&fx), vec![false], "{text}");
        assert!(routes(&fx).is_empty() && t.journal.borrow().is_empty(), "{text}: no hold, no journal line");
        assert_eq!(t.hub.st.msgs().len(), n, "{text}: nothing said: the shell steps his input next");
    }
    // queued, to another agent, fn space's 'start an agent', off home
    let projects = || vec![("shop-1".to_string(), "shop".to_string())];
    let go = |focus: &str, via: &str, queued: bool, projects: Vec<(String, String)>| {
        let mut t = T::new();
        let text = "@shop fix it".to_string();
        asked(&t.go(Input::RouteAsk { focus: focus.into(), text, via: via.into(), queued, context: None, parts: String::new(), projects }))
    };
    assert_eq!(go(MAIN, "capsule", false, projects()), vec![true]);
    assert_eq!(go(MAIN, "", true, projects()), vec![false], "queued");
    assert_eq!(go("docs", "", false, projects()), vec![false], "to another agent");
    assert_eq!(go(MAIN, "capsule-start", false, projects()), vec![false], "start an agent");
    assert_eq!(go(MAIN, "", false, Vec::new()), vec![false], "not bise's home hub");
}

// an agent of this hub named like the project: his `@name` is for it
#[test]
fn an_agent_named_like_a_project_wins_the_address() {
    let mut t = T::new();
    t.spawn_task("shop");
    let fx = hold(&mut t, "fix the checkout");
    assert_eq!(asked(&fx), vec![false]);
    assert!(routes(&fx).is_empty());
}

// architect m_34720 (3.): the address is read from the head of his words:
// a pasted 200 KB text costs what a short one does
#[test]
fn a_route_ask_reads_only_the_head_of_a_long_text() {
    let long = format!("@shop {}", "the p99 rose again. ".repeat(10_000));
    assert!(long.len() >= 200_000);
    let mut t = T::new();
    let t0 = std::time::Instant::now();
    let fx = ask_with(&mut t, &long, None, "");
    let routed_ms = t0.elapsed();
    assert_eq!(asked(&fx), vec![true]);
    let plain = format!("no address here {}", "the p99 rose again. ".repeat(10_000));
    let mut t = T::new();
    let t0 = std::time::Instant::now();
    assert_eq!(asked(&ask_with(&mut t, &plain, None, "")), vec![false]);
    assert!(t0.elapsed() < routed_ms * 4 + std::time::Duration::from_millis(500), "{:?} vs {:?}", t0.elapsed(), routed_ms);
}

// his words wait 2 s on the timers' clock, then go once: his message to
// main is recorded (an id, never a turn: law route_fire_makes_no_turn),
// forwarded by the outbox, and main gets a note with the id
#[test]
fn a_held_route_goes_at_two_seconds_once_with_no_turn() {
    let mut t = T::new();
    let now = t.env.now;
    let n = t.hub.st.msgs().len();
    let fx = hold(&mut t, "why does p99 rise");
    assert_eq!(routes(&fx), vec![(1, "shop-1".into(), "shop".into(), now + 2000)]);
    assert!(xdelivers(&fx).is_empty() && say_to(&fx, MAIN).is_none() && steer_to(&fx, MAIN).is_none());
    assert_eq!(t.hub.st.msgs().len(), n, "no message while held");
    t.env.now += 1_500;
    assert!(xdelivers(&t.go(Input::Tick)).is_empty());
    t.env.now += 600;
    let fx = t.go(Input::Tick);
    assert_eq!(xdelivers(&fx), vec![(1, "shop-1".into(), "input".into(), "@shop why does p99 rise".into())]);
    assert_eq!(route_dones(&fx), vec![(1, "sent".into(), "shop-1".into(), Some(1))]);
    assert!(say_to(&fx, MAIN).is_none() && steer_to(&fx, MAIN).is_none(), "no turn: {:?}", fx);
    let his = last_msg(&t);
    let m = &t.hub.st.msgs()[&his];
    assert_eq!((m.from.as_str(), m.to.as_str(), t.hub.st.text(his).as_str()), (crate::model::USER, MAIN, "@shop why does p99 rise"));
    let note = t.hub.st.main_notes.last().expect("a note for main");
    assert!(note.contains(&format!("m_{his}")) && note.contains("shop"), "{note}");
    // forwarded already: main can't send it a second time
    let (tok, fx) = t.req(MAIN, AgentReq::ProjectSend { project: "shop".into(), msg: his });
    assert!(reply(&fx, tok).unwrap()["error"].as_str().unwrap().contains("already forwarded"), "{:?}", fx);
    // once: no second fire, and a late correction or cancel changes nothing
    t.env.now += 10_000;
    assert!(xdelivers(&t.go(Input::Tick)).is_empty());
    let fx = t.go(Input::RouteCorrect { client: None, rid: 1, to: "docs-2".into(), name: "docs".into() });
    assert!(routes(&fx).is_empty() && xdelivers(&fx).is_empty());
    let fx = t.go(Input::RouteCancel { client: None, rid: 1 });
    assert!(route_dones(&fx).is_empty() && say_to(&fx, MAIN).is_none());
    // the typed connection that asked hears why nothing happened
    for input in [Input::RouteCancel { client: Some(5), rid: 1 }, Input::RouteCorrect { client: Some(5), rid: 1, to: "docs-2".into(), name: "docs".into() }] {
        let fx = t.go(input);
        assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { client: 5, body } if body["ev"] == "notice"
            && body["text"].as_str().unwrap_or("").contains("route 1 already went or was cancelled"))), "{:?}", fx);
    }
}

// route-name (his screenshot: routed bubbles stuck, read as never sent):
// bise's main thread gets his words where he wrote them, their id, and
// where they went by that id (law route_fire_says_where); the window's
// route_done carries the same id
#[test]
fn a_fired_route_says_where_it_went_in_bises_main_thread() {
    let mut t = T::new();
    let projects = vec![("shop-1".to_string(), "shop".to_string())];
    let text = "@shop why does p99 rise".to_string();
    t.go(Input::RouteAsk { focus: String::new(), text, via: "capsule".into(), queued: false, context: None, parts: String::new(), projects });
    t.env.now += 2_100;
    let fx = t.go(Input::Tick);
    let his = last_msg(&t);
    let main_lines: Vec<&str> = fx.iter().filter_map(|e| match e {
        Effect::Line { agent, line } if agent == MAIN => Some(line.as_str()),
        _ => None,
    }).collect();
    let want = [
        r#"sb you :: {"text":"@shop why does p99 rise","via":"capsule"}"#.to_string(),
        format!(r#"sb you-id :: {{"id":{his}}}"#),
        format!(r#"sb routed :: {{"msg":{his},"name":"shop","to":"shop-1","xid":1}}"#),
    ];
    let at: Vec<usize> = want.iter().map(|w| main_lines.iter().position(|l| l == w).unwrap_or_else(|| panic!("{w} in {main_lines:?}"))).collect();
    assert!(at.windows(2).all(|p| p[0] < p[1]), "in order: {main_lines:?}");
    assert!(fx.iter().any(|e| matches!(e, Effect::RouteDone { state, msg: Some(m), .. } if state == "sent" && *m == his)), "{:?}", fx);
    // a cancel became no message of a route: no id on its end
    let mut t = T::new();
    hold(&mut t, "never mind");
    let fx = t.go(Input::RouteCancel { client: None, rid: 1 });
    assert!(fx.iter().any(|e| matches!(e, Effect::RouteDone { state, msg: None, .. } if state == "cancelled")), "{:?}", fx);
}

// a correction changes the project, not the time; `bise` is a cancel
#[test]
fn a_corrected_route_goes_to_the_new_project_at_the_same_time() {
    let mut t = T::new();
    let now = t.env.now;
    hold(&mut t, "update the docs");
    t.env.now += 1_000;
    let fx = t.go(Input::RouteCorrect { client: None, rid: 1, to: "docs-2".into(), name: "docs".into() });
    assert_eq!(routes(&fx), vec![(1, "docs-2".into(), "docs".into(), now + 2000)]);
    t.env.now += 1_100;
    let fx = t.go(Input::Tick);
    assert_eq!(xdelivers(&fx), vec![(1, "docs-2".into(), "input".into(), "@shop update the docs".into())]);
    assert_eq!(route_dones(&fx), vec![(1, "sent".into(), "docs-2".into(), Some(1))]);
}

// a cancel: no delivery ever, his words reach bise's main as today
#[test]
fn a_cancelled_route_is_said_to_main_and_never_sent() {
    for how in ["cancel", "correct to bise"] {
        let mut t = T::new();
        hold(&mut t, "never mind, for you");
        let fx = match how {
            "cancel" => t.go(Input::RouteCancel { client: None, rid: 1 }),
            _ => t.go(Input::RouteCorrect { client: None, rid: 1, to: "bise".into(), name: "bise".into() }),
        };
        assert_eq!(route_dones(&fx), vec![(1, "cancelled".into(), "shop-1".into(), None)], "{how}");
        let said = say_to(&fx, MAIN).or_else(|| steer_to(&fx, MAIN)).expect("main hears his words");
        assert!(said.contains("never mind, for you"), "{how}: {said}");
        t.env.now += 5_000;
        assert!(xdelivers(&t.go(Input::Tick)).is_empty(), "{how}");
    }
}

// his fn context rides the route to the project's delivery, kept across a
// restart; the routed message on the home hub keeps his words alone
#[test]
fn a_routed_message_carries_his_context_to_the_project() {
    let mut t = T::new();
    let ctx = json!({"app": "Safari", "url": "https://grafana.shop.test/d/p99"});
    ask_with(&mut t, "@shop why does p99 rise", Some(ctx.clone()), "");
    let journal = t.journal.borrow().clone();
    let mut h = Hub::new("/w");
    h.replay(&journal);
    let mut env = FakeEnv::new();
    env.now = t.env.now + 2_500;
    let fx = step(&mut h, &mut env, Input::Tick);
    let sent: Vec<(String, String)> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::XDeliver { text, context, .. } => Some((text.clone(), context.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(sent.len(), 1, "{:?}", fx);
    assert_eq!(sent[0].0, "@shop why does p99 rise");
    assert_eq!(serde_json::from_str::<Value>(&sent[0].1).unwrap(), ctx);
    let his = *h.st.msgs().keys().max().unwrap();
    assert_eq!(h.st.text(his), "@shop why does p99 rise", "no context on the home hub");
    // a forward or an ask carries none
    let (_, fx) = t.req(MAIN, AgentReq::ProjectAsk { project: "shop".into(), text: "status?".into() });
    assert!(fx.iter().any(|e| matches!(e, Effect::XDeliver { context, .. } if context.is_empty())), "{:?}", fx);
}

// R41 (6): his parts ride the route (kept across a restart) to the
// project's delivery, his message at home keeps them, a cancel says them
// to main with them, and the project's message keeps what came
#[test]
fn a_routed_message_carries_his_parts_to_the_project() {
    let parts = r#"[{"kind":"text","text":"look "},{"kind":"image","name":"[Image #1]","source":"/d/s.png","mime":"image/png","b64_file":"/h/images/ab.b64"}]"#;
    let mut t = T::new();
    ask_with(&mut t, "@shop look", None, parts);
    let journal = t.journal.borrow().clone();
    let mut h = Hub::new("/w");
    h.replay(&journal);
    let mut env = FakeEnv::new();
    env.now = t.env.now + 2_500;
    let fx = step(&mut h, &mut env, Input::Tick);
    let sent: Vec<String> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::XDeliver { parts, .. } => Some(parts.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(sent, vec![parts.to_string()], "{:?}", fx);
    let his = *h.st.msgs().keys().max().unwrap();
    assert_eq!(h.st.words(his).unwrap().parts, parts, "his message at home keeps them");
    // a cancel: his words to main with his parts
    let mut t = T::new();
    ask_with(&mut t, "@shop look", None, parts);
    t.go(Input::RouteCancel { client: None, rid: 1 });
    let his = last_msg(&t);
    let w = t.hub.st.words(his).unwrap();
    assert_eq!((w.text.as_str(), w.parts.as_str()), ("@shop look", parts));
    // no parts: the forward carries none (the out JSON has no key)
    let mut t = T::new();
    hold(&mut t, "why does p99 rise");
    t.env.now += 2_100;
    let fx = t.go(Input::Tick);
    assert!(fx.iter().any(|e| matches!(e, Effect::XDeliver { parts, .. } if parts.is_empty())), "{:?}", fx);
    assert!(!t.journal.borrow().iter().any(|l| l.to_string().contains("\"parts\"")), "no parts key in the journal");
    // the project: his forwarded message keeps them; bise's question none
    let mut t = T::new();
    t.go(Input::XIn { token: 900, hub: "home-1".into(), xid: 7, kind: "input".into(), text: "look".into(), name: String::new(), parts: parts.into() });
    let his = last_msg(&t);
    assert_eq!(t.hub.st.words(his).unwrap().parts, parts);
    t.go(Input::XIn { token: 901, hub: "home-1".into(), xid: 8, kind: "ask".into(), text: "status?".into(), name: String::new(), parts: parts.into() });
    let q = last_msg(&t);
    assert!(t.hub.st.words(q).unwrap().parts.is_empty(), "bise's own question has no parts");
    assert!(!t.hub.st.msgs()[&q].has_parts);
}

// a restart mid-hold: the act is in the journal, it fires once after
#[test]
fn a_route_held_across_a_restart_goes_once() {
    let mut t = T::new();
    hold(&mut t, "after the restart");
    let journal = t.journal.borrow().clone();
    let mut h = Hub::new("/w");
    h.replay(&journal);
    let mut env = FakeEnv::new();
    env.now = t.env.now;
    assert!(xdelivers(&h.handle(Input::Boot, &mut env)).is_empty());
    env.now += 2_500;
    assert_eq!(xdelivers(&h.handle(Input::Tick, &mut env)).len(), 1);
    env.now += 2_500;
    assert!(xdelivers(&h.handle(Input::Tick, &mut env)).is_empty());
}
