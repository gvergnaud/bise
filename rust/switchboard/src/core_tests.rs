//! Scenario tests of the hub core: inputs in, effects out. No process,
//! no socket, no git (a fake `Env`).

use super::*;

struct FakeEnv {
    now: u64,
    git: bool,
    loss: Loss,
    dropped: Vec<String>,
}

impl FakeEnv {
    fn new() -> FakeEnv {
        FakeEnv {
            now: 1_000_000,
            git: true,
            loss: Loss::default(),
            dropped: Vec::new(),
        }
    }
}

impl Env for FakeEnv {
    fn now(&self) -> u64 {
        self.now
    }
    fn is_git(&self) -> bool {
        self.git
    }
    fn worktree_create(&mut self, name: &str, _with_changes: bool) -> Result<Workspace, String> {
        Ok(Workspace {
            mode: Mode::Worktree,
            path: format!("/state/worktrees/{}", name),
            branch: Some(format!("sb/{}", name)),
            base_commit: Some("abc".into()),
            dropped: false,
            place: Some(format!("wt:{}", name)),
            feature: None,
        })
    }
    fn worktree_loss(&mut self, _ws: &Workspace) -> Loss {
        self.loss.clone()
    }
    fn worktree_drop(
        &mut self,
        name: &str,
        _ws: &Workspace,
        loss: &Loss,
    ) -> Result<Option<String>, String> {
        self.dropped.push(name.to_string());
        Ok(loss
            .any()
            .then(|| format!("refs/switchboard/trash/{}/1", name)))
    }
    fn worktree_restore(
        &mut self,
        name: &str,
        ws: &Workspace,
        _snap: Option<&str>,
    ) -> Result<Workspace, String> {
        let mut ws = ws.clone();
        ws.dropped = false;
        let _ = name;
        Ok(ws)
    }
}

struct T {
    hub: Hub,
    env: FakeEnv,
    token: u64,
    /// Every journal event so far (what the daemon writes to disk).
    journal: std::rc::Rc<std::cell::RefCell<Vec<Value>>>,
}

impl T {
    fn new() -> T {
        let mut t = T {
            hub: Hub::new("/w"),
            env: FakeEnv::new(),
            token: 0,
            journal: Default::default(),
        };
        let fx = t.go(Input::Boot);
        assert!(fx.contains(&Effect::Spawn {
            agent: MAIN.into(),
            resume: true,
            crash_note: None
        }));
        t.go(Input::ReplReady { agent: MAIN.into() });
        t.go(Input::ClientHello { client: 1 });
        t
    }

    fn go(&mut self, input: Input) -> Vec<Effect> {
        let fx = self.hub.handle(input, &mut self.env);
        for e in &fx {
            if let Effect::Journal(ev) = e {
                self.journal.borrow_mut().push(ev.clone());
            }
        }
        fx
    }

    fn req(&mut self, from: &str, req: AgentReq) -> (u64, Vec<Effect>) {
        self.token += 1;
        let tok = self.token;
        let fx = self.go(Input::Agent {
            token: tok,
            from: from.into(),
            req,
        });
        (tok, fx)
    }

    fn user(&mut self, focus: &str, text: &str) -> Vec<Effect> {
        self.go(Input::ClientInput {
            client: 1,
            focus: focus.into(),
            text: text.into(),
        })
    }

    /// A turn of `agent`: started, one assistant text, idle.
    fn turn(&mut self, agent: &str, text: &str) -> Vec<Effect> {
        self.go(Input::ReplLine {
            agent: agent.into(),
            line: "  obs: turn_started".into(),
        });
        self.go(Input::ReplLine {
            agent: agent.into(),
            line: format!("  obs: assistant: {}", text),
        });
        self.go(Input::ReplIdle {
            agent: agent.into(),
            leftover: false,
        })
    }

    fn spawn_task(&mut self, name: &str) {
        let (_, fx) = self.req(
            MAIN,
            AgentReq::Spawn {
                name: name.into(),
                brief: Brief {
                    objective: format!("objective of {}", name),
                    ..Brief::default()
                },
                worktree: false,
                with_changes: false,
                place: String::new(),
                feature: String::new(),
                ask: Default::default(),
            },
        );
        assert!(
            fx.iter()
                .any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == name)),
            "{:?}",
            fx
        );
        let fx = self.go(Input::ReplReady { agent: name.into() });
        assert!(
            say_to(&fx, name).is_some(),
            "the brief starts the first turn: {:?}",
            fx
        );
        // the first turn ends: main gets the automatic reply
        self.go(Input::ReplLine {
            agent: name.into(),
            line: "  obs: turn_started".into(),
        });
    }

    fn status(&self, name: &str) -> Status {
        self.hub.st.agents[name].status()
    }
}

fn say_to(fx: &[Effect], agent: &str) -> Option<String> {
    fx.iter().find_map(|e| match e {
        Effect::Say { agent: a, text } if a == agent => Some(text.clone()),
        _ => None,
    })
}

fn steer_to(fx: &[Effect], agent: &str) -> Option<String> {
    fx.iter().find_map(|e| match e {
        Effect::Steer { agent: a, text } if a == agent => Some(text.clone()),
        _ => None,
    })
}

fn reply(fx: &[Effect], token: u64) -> Option<Value> {
    fx.iter().find_map(|e| match e {
        Effect::Reply { token: t, body } if *t == token => Some(body.clone()),
        _ => None,
    })
}

fn has_line(fx: &[Effect], agent: &str, needle: &str) -> bool {
    fx.iter().any(
        |e| matches!(e, Effect::Line { agent: a, line } if a == agent && line.contains(needle)),
    )
}

#[test]
fn the_user_talks_to_main_by_default() {
    let mut t = T::new();
    let fx = t.user(MAIN, "bonjour");
    assert_eq!(say_to(&fx, MAIN).as_deref(), Some("bonjour"));
    assert!(has_line(&fx, MAIN, "sb you : bonjour"));
    assert_eq!(t.status(MAIN), Status::Working);
    // main is busy: the next message steers the running turn
    let fx = t.user(MAIN, "et aussi");
    assert_eq!(steer_to(&fx, MAIN).as_deref(), Some("et aussi"));
}

#[test]
fn a_spawned_task_answers_main_automatically() {
    let mut t = T::new();
    t.spawn_task("auth-fix");
    assert_eq!(t.status("auth-fix"), Status::Working);
    let fx = t.go(Input::ReplLine {
        agent: "auth-fix".into(),
        line: "  obs: assistant: <think>x</think>corrigé".into(),
    });
    assert!(fx.is_empty() || !fx.iter().any(|e| matches!(e, Effect::Say { .. })));
    let fx = t.go(Input::ReplIdle {
        agent: "auth-fix".into(),
        leftover: false,
    });
    let to_main = say_to(&fx, MAIN).expect("main is woken by the automatic reply");
    assert!(
        to_main.contains("from=\"auth-fix\" relation=\"child\""),
        "{}",
        to_main
    );
    assert!(to_main.contains("auto=\"true\""), "{}", to_main);
    assert!(to_main.contains("corrigé"), "{}", to_main);
    // the board carries the automatic report
    let a = &t.hub.st.agents["auth-fix"];
    assert_eq!(
        a.last_report.as_ref().map(|r| r.summary.as_str()),
        Some("corrigé")
    );
    assert_eq!(t.status("auth-fix"), Status::Idle);
}

#[test]
fn the_board_of_main_follows_the_tasks() {
    let mut t = T::new();
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Spawn {
            name: "docs".into(),
            brief: Brief {
                objective: "Doc API v2".into(),
                ..Brief::default()
            },
            worktree: false,
            with_changes: false,
            place: String::new(),
            feature: String::new(),
            ask: Default::default(),
        },
    );
    let ctx = fx
        .iter()
        .find_map(|e| match e {
            Effect::Context { agent, text } if agent == MAIN => Some(text.clone()),
            _ => None,
        })
        .expect("main's context is rewritten");
    assert!(
        ctx.contains("docs") && ctx.contains("Doc API v2"),
        "{}",
        ctx
    );
}

#[test]
fn steering_left_in_the_file_is_sent_again() {
    let mut t = T::new();
    t.user(MAIN, "premier");
    let fx = t.user(MAIN, "second");
    assert!(steer_to(&fx, MAIN).is_some());
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: assistant: ok".into(),
    });
    let fx = t.go(Input::ReplIdle {
        agent: MAIN.into(),
        leftover: true,
    });
    assert_eq!(say_to(&fx, MAIN).as_deref(), Some("second"));
}

#[test]
fn ask_waits_for_the_reply() {
    let mut t = T::new();
    t.spawn_task("docs");
    // main is busy with its own turn: the question steers it
    t.user(MAIN, "go");
    let (tok, fx) = t.req(
        "docs",
        AgentReq::Ask {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            timeout_s: 20,
        },
    );
    assert!(reply(&fx, tok).is_none(), "the ask blocks");
    assert_eq!(t.status("docs"), Status::Waiting);
    let q = steer_to(&fx, MAIN).expect("delivered to main");
    let id = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "v1 ou v2 ?")
        .unwrap()
        .id;
    assert!(
        q.contains(&format!("id=\"m_{}\"", id)) && q.contains("expects_reply=\"true\""),
        "{}",
        q
    );
    let (tok2, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "v2".into(),
            expect_reply: false,
            reply_to: Some(id),
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    let r = reply(&fx, tok).expect("the wait ends");
    assert_eq!(r["type"], "reply");
    assert_eq!(r["message"], "v2");
    // the question's id (BISE-110)
    assert_eq!(r["asked"], format!("m_{}", id));
    assert_eq!(reply(&fx, tok2).unwrap()["delivery"], "delivered");
    assert_eq!(t.status("docs"), Status::Working);
    // main answered: no automatic reply at the end of its turn
    let fx = t.turn(MAIN, "réglé");
    assert!(
        say_to(&fx, "docs").is_none() && steer_to(&fx, "docs").is_none(),
        "{:?}",
        fx
    );
}

/// The agent state says who a waiting agent waits on (the panel's
/// `waits {name}`), and forgets it when the wait ends.
#[test]
fn a_waiting_agent_says_who_it_waits_on() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.user(MAIN, "go");
    let (tok, _) = t.req(
        "docs",
        AgentReq::Ask {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            timeout_s: 20,
        },
    );
    let agent = |t: &T, n: &str| {
        let snap = t.hub.snapshot(0);
        snap["agents"].as_array().unwrap().iter().find(|a| a["name"] == n).cloned().unwrap()
    };
    let docs = agent(&t, "docs");
    assert_eq!(docs["status"], "waiting");
    assert_eq!(docs["waiting_on"], MAIN, "{}", docs);
    assert!(agent(&t, MAIN)["waiting_on"].is_null());
    let id = t.hub.st.msgs.values().find(|m| m.text == "v1 ou v2 ?").unwrap().id;
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "v2".into(),
            expect_reply: false,
            reply_to: Some(id),
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    assert!(reply(&fx, tok).is_some(), "the wait ends");
    assert!(agent(&t, "docs")["waiting_on"].is_null());
}

#[test]
fn a_question_to_a_waiting_agent_ends_its_wait() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let (tok_a, _) = t.req(
        "a",
        AgentReq::Ask {
            to: "b".into(),
            text: "A?".into(),
            timeout_s: 20,
        },
    );
    let (tok_b, fx) = t.req(
        "b",
        AgentReq::Ask {
            to: "a".into(),
            text: "B?".into(),
            timeout_s: 20,
        },
    );
    let r = reply(&fx, tok_a).expect("a's wait ends: b asked it something");
    assert_eq!(r["type"], "incoming_request");
    assert_eq!(r["message"], "B?");
    assert!(reply(&fx, tok_b).is_none());
}

#[test]
fn waits_time_out() {
    let mut t = T::new();
    t.spawn_task("a");
    let (tok, _) = t.req(
        "a",
        AgentReq::Ask {
            to: MAIN.into(),
            text: "?".into(),
            timeout_s: 600,
        },
    );
    t.env.now += 24_000;
    assert!(reply(&t.go(Input::Tick), tok).is_none());
    t.env.now += 2_000;
    let r = reply(&t.go(Input::Tick), tok).expect("capped at 25 s");
    assert_eq!(r["error"], "timeout");
}

#[test]
fn main_learns_what_the_user_said_directly() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ReplLine {
        agent: "docs".into(),
        line: "  obs: assistant: brief lu".into(),
    });
    t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    t.turn(MAIN, "noté");
    t.go(Input::ClientFocus {
        client: 1,
        focus: "docs".into(),
    });
    let fx = t.user("docs", "utilise la v2");
    assert_eq!(say_to(&fx, "docs").as_deref(), Some("utilise la v2"));
    t.turn("docs", "ok, v2");
    let fx = t.go(Input::ClientFocus {
        client: 1,
        focus: MAIN.into(),
    });
    assert!(
        has_line(&fx, MAIN, "You talked to @docs (1 message)"),
        "{:?}",
        fx
    );
    // the note rides with the next message to main
    let fx = t.user(MAIN, "où en est la doc ?");
    let s = say_to(&fx, MAIN).unwrap();
    assert!(s.starts_with("<bise_notes>"), "{}", s);
    assert!(s.contains("utilise la v2") && s.contains("ok, v2"), "{}", s);
    assert!(s.ends_with("où en est la doc ?"));
    assert!(t.hub.st.main_notes.is_empty());
}

#[test]
fn at_task_from_main_view_answers_there_and_notes_main() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    t.turn(MAIN, "noté");
    // the task reads it tagged, from the user via main's view
    let fx = t.user(MAIN, "@docs v1 ou v2 ?");
    let s = say_to(&fx, "docs").unwrap();
    assert_eq!(
        s,
        "<user_message via=\"main\">\nv1 ou v2 ?\n</user_message>"
    );
    assert!(has_line(&fx, "docs", "sb you : v1 ou v2 ?"), "{:?}", fx);
    // its end-of-turn answer comes back to main's view, main not woken
    let fx = t.turn("docs", "la v2");
    // C2 `msg-you`: an agent writing to the user
    assert!(has_line(&fx, MAIN, "sb msg-you : docs : la v2"), "{:?}", fx);
    assert!(say_to(&fx, MAIN).is_none() && steer_to(&fx, MAIN).is_none());
    assert!(t.hub.st.unanswered_for("docs").is_empty());
    // main's next turn carries the exchange
    let fx = t.user(MAIN, "et ensuite ?");
    let s = say_to(&fx, MAIN).unwrap();
    assert!(
        s.contains("@docs answered the user (asked from @main's view: \"v1 ou v2 ?\"): \"la v2\""),
        "{}",
        s
    );
    // in the task's own view, it stays a plain user message
    t.turn(MAIN, "ok");
    let fx = t.user("docs", "@docs merci");
    assert_eq!(say_to(&fx, "docs").as_deref(), Some("merci"));
    let fx = t.turn("docs", "de rien");
    assert!(!has_line(&fx, MAIN, "@docs : de rien"));
}

#[test]
fn at_task_to_a_busy_task_steers_and_answers_at_turn_end() {
    let mut t = T::new();
    t.spawn_task("docs");
    // docs is still in its first turn
    let fx = t.user(MAIN, "@docs où en es-tu ?");
    let s = steer_to(&fx, "docs").unwrap();
    assert!(s.starts_with("<user_message via=\"main\">"), "{}", s);
    t.go(Input::ReplLine {
        agent: "docs".into(),
        line: "  obs: assistant: à mi-chemin".into(),
    });
    let fx = t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    assert!(has_line(&fx, MAIN, "sb msg-you : docs : à mi-chemin"), "{:?}", fx);
    // main still gets its own automatic reply to the brief
    assert!(t.hub.st.unanswered_for("docs").is_empty());
}

#[test]
fn there_is_no_undo_a_route_stays_sent() {
    let mut t = T::new();
    t.spawn_task("docs");
    // docs never started its REPL again: simulate it down
    t.hub.force_run("docs", Run::Starting);
    let fx = t.user(MAIN, "@docs change de plan");
    assert!(has_line(&fx, MAIN, "you → @docs : change de plan"));
    // book §13: no undo, the user says the change to main instead
    let fx = t.user(MAIN, "/cancel");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").starts_with("no undo"))), "{:?}", fx);
    let fx = t.go(Input::ReplReady {
        agent: "docs".into(),
    });
    assert!(say_to(&fx, "docs").is_some_and(|s| s.contains("change de plan")), "{:?}", fx);
    let fx = t.user(MAIN, "@nope salut");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("no agent named @nope"))));
}

#[test]
fn dropping_a_worktree_with_work_asks_first() {
    let mut t = T::new();
    let fx = t.user(MAIN, "/new -w fix: corrige le bug");
    assert!(
        has_line(&fx, MAIN, "new agent @fix (worktree sb/fix)"),
        "{:?}",
        fx
    );
    t.go(Input::ReplReady {
        agent: "fix".into(),
    });
    t.go(Input::ReplIdle {
        agent: "fix".into(),
        leftover: false,
    });
    t.env.loss = Loss {
        dirty: 3,
        unpushed: 2,
    };
    let fx = t.user(MAIN, "/archive fix");
    let (id, text) = fx
        .iter()
        .find_map(|e| match e {
            Effect::ToClient { body, .. } if body["ev"] == "confirm" => Some((
                body["id"].as_u64().unwrap(),
                body["text"].as_str().unwrap().to_string(),
            )),
            _ => None,
        })
        .expect("a confirmation");
    assert!(
        text.contains("3 changed files and 2 unpushed commits"),
        "{}",
        text
    );
    let fx = t.go(Input::ClientConfirm {
        client: 1,
        id,
        yes: true,
    });
    assert!(fx.contains(&Effect::Kill {
        agent: "fix".into()
    }));
    assert_eq!(t.env.dropped, vec!["fix".to_string()]);
    assert_eq!(t.status("fix"), Status::Archived);
    assert!(t.hub.st.agents["fix"].snapshot_ref.is_some());
    // an archived worktree task is not revived by a message
    let fx = t.user(MAIN, "@fix encore");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("/restore"))));
    // restore brings it back
    let fx = t.user(MAIN, "/restore fix");
    assert!(fx
        .iter()
        .any(|e| matches!(e, Effect::Spawn { agent, resume: true, .. } if agent == "fix")));
    assert!(!t.hub.st.agents["fix"].ws.dropped);
}

#[test]
fn main_cannot_drop_work_away() {
    let mut t = T::new();
    t.user(MAIN, "/new -w fix: x");
    t.go(Input::ReplReady {
        agent: "fix".into(),
    });
    t.go(Input::ReplIdle {
        agent: "fix".into(),
        leftover: false,
    });
    t.env.loss = Loss {
        dirty: 1,
        unpushed: 0,
    };
    let (tok, fx) = t.req(
        MAIN,
        AgentReq::Drop {
            agent: "fix".into(),
        },
    );
    assert_eq!(reply(&fx, tok).unwrap()["dropped"], false);
    let card = t
        .hub
        .st
        .cards
        .values()
        .find(|c| c.kind == "drop")
        .unwrap()
        .id;
    t.user(MAIN, &format!("/answer {} oui", card));
    assert_eq!(t.status("fix"), Status::Archived);
}

#[test]
fn an_escalated_question_is_answered_by_the_user() {
    let mut t = T::new();
    t.spawn_task("docs");
    let (tok, fx) = t.req(
        "docs",
        AgentReq::Ask {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            timeout_s: 20,
        },
    );
    assert!(say_to(&fx, MAIN).is_some());
    let id = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "v1 ou v2 ?")
        .unwrap()
        .id;
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Card {
            text: "La doc : v1 ou v2 ?".into(),
            for_msg: Some(id),
        },
    );
    assert!(
        has_line(&fx, MAIN, "sb card : #1 question @docs"),
        "{:?}",
        fx
    );
    // main ends its turn: no automatic reply, the question is handed over
    let fx = t.turn(MAIN, "J'ai demandé à l'utilisateur.");
    assert!(reply(&fx, tok).is_none());
    // the user answers the card
    let fx = t.user(MAIN, "/answer 1 v2");
    let r = reply(&fx, tok).expect("the task's wait ends with the user's answer");
    assert_eq!(r["from"], USER);
    assert_eq!(r["message"], "v2");
    assert!(t.hub.st.cards.is_empty());
}

#[test]
fn answering_in_the_task_view_answers_its_question() {
    let mut t = T::new();
    t.spawn_task("docs");
    let (_, _) = t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    let id = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "v1 ou v2 ?")
        .unwrap()
        .id;
    t.req(
        MAIN,
        AgentReq::Card {
            text: "v1 ou v2 ?".into(),
            for_msg: Some(id),
        },
    );
    t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    t.go(Input::ClientFocus {
        client: 1,
        focus: "docs".into(),
    });
    let fx = t.user("docs", "v2");
    let s = say_to(&fx, "docs").unwrap();
    assert!(
        s.contains("from=\"user\"") && s.contains(&format!("reply_to=\"m_{}\"", id)),
        "{}",
        s
    );
    assert!(t.hub.st.cards.is_empty());
}

#[test]
fn a_crashing_repl_restarts_then_fails() {
    let mut t = T::new();
    t.spawn_task("a");
    for i in 1..=5 {
        let fx = t.go(Input::ReplExited {
            agent: "a".into(),
            crashed: true,
            reason: "boom".into(),
        });
        assert!(
            fx.iter()
                .any(|e| matches!(e, Effect::Spawn { agent, resume: true, .. } if agent == "a")),
            "crash {}",
            i
        );
    }
    let fx = t.go(Input::ReplExited {
        agent: "a".into(),
        crashed: true,
        reason: "boom".into(),
    });
    assert!(!fx.iter().any(|e| matches!(e, Effect::Spawn { .. })));
    assert_eq!(t.status("a"), Status::Failed);
    // BISE-299: a failure is main's to handle (a bise message), never a
    // card in the user's inbox
    assert!(t.hub.st.cards.is_empty());
    assert!(t.hub.st.msgs.values().any(|m| m.to == MAIN && m.text.contains("agent @a failed")));
    // the user's message revives it
    let fx = t.user(MAIN, "@a réessaie");
    assert!(fx
        .iter()
        .any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == "a")));
}

#[test]
fn peers_cannot_reach_an_archived_task_but_the_user_can() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    t.go(Input::ReplIdle {
        agent: "b".into(),
        leftover: false,
    });
    t.user(MAIN, "/archive b --force");
    let (tok, fx) = t.req(
        "a",
        AgentReq::Send {
            to: "b".into(),
            text: "?".into(),
            expect_reply: false,
            reply_to: None,
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    assert!(reply(&fx, tok).unwrap()["error"]
        .as_str()
        .unwrap()
        .starts_with("recipient_unavailable"));
    let fx = t.user(MAIN, "@b reviens");
    assert!(fx
        .iter()
        .any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == "b")));
}

#[test]
fn worktrees_need_git() {
    let mut t = T::new();
    t.env.git = false;
    let fx = t.user(MAIN, "/new -w x: y");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("not a git repository"))));
    assert!(!t.hub.st.agents.contains_key("x"));
}

/// `sb every` (every.rs): at least a minute, an active agent, a message;
/// set, listed in `sb tasks`, stopped, each change in the journal.
#[test]
fn every_sets_lists_and_stops_timers() {
    use crate::every::Sched;
    let mut t = T::new();
    let add = |sched, to: &str| AgentReq::Every(EveryReq::Add { to: to.into(), text: "check HN".into(), sched, until_ms: None, times: None, page: None });
    for (req, why) in [
        (add(Sched::Every(30_000), ""), "at least 1m"),
        (add(Sched::Every(600_000), "ghost"), "no active agent @ghost"),
    ] {
        let (tok, fx) = t.req(MAIN, req);
        let e = reply(&fx, tok).unwrap()["error"].as_str().unwrap().to_string();
        assert!(e.contains(why), "{}", e);
        assert!(!fx.iter().any(|e| matches!(e, Effect::Journal(_))));
    }
    let (tok, fx) = t.req(MAIN, add(Sched::Every(600_000), ""));
    let r = reply(&fx, tok).unwrap();
    assert!(r["text"].as_str().unwrap().starts_with("timer set: #1 @main every 10m"), "{}", r);
    assert!(fx.iter().any(|e| matches!(e, Effect::Journal(j) if j["type"] == "every_set")));
    let (tok, fx) = t.req(MAIN, AgentReq::Tasks);
    assert!(reply(&fx, tok).unwrap()["text"].as_str().unwrap().contains("## timers (sb every)\n#1 @main every 10m"));
    let (tok, fx) = t.req(MAIN, AgentReq::Every(EveryReq::Stop(1)));
    assert_eq!(reply(&fx, tok).unwrap()["text"], "timer #1 stopped");
    let (tok, fx) = t.req(MAIN, AgentReq::Every(EveryReq::List));
    assert!(reply(&fx, tok).unwrap()["text"].as_str().unwrap().starts_with("no timers"));
}

#[test]
fn shared_tasks_touching_one_file_tell_main() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let patch = |f: &str| {
        format!("tool #1 apply_patch : {{\"arg\":\"*** Begin Patch\\n*** Update File: {}\\n@@\\n-x\\n+y\\n*** End Patch\"}}", f)
    };
    t.go(Input::ReplLine {
        agent: "a".into(),
        line: patch("src/x.rs"),
    });
    assert!(t.hub.st.cards.is_empty());
    t.go(Input::ReplLine {
        agent: "b".into(),
        line: patch("src/x.rs"),
    });
    // BISE-299: main's to handle (a note), not the user's inbox
    assert!(t.hub.st.cards.is_empty());
    assert!(t.hub.st.main_notes.iter().any(|n| n.contains("file overlap") && n.contains("src/x.rs")));
    // a task that changed a file cannot be isolated anymore
    let fx = t.user(MAIN, "/isolate a");
    assert!(fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().unwrap_or("").contains("already changed files"))));
}

#[test]
fn a_done_report_updates_the_board_and_wakes_main() {
    let mut t = T::new();
    t.spawn_task("a");
    let (tok, fx) = t.req(
        "a",
        AgentReq::Report {
            kind: "done".into(),
            summary: "fini".into(),
            decisions: vec!["API v2".into()],
        },
    );
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true);
    let s = say_to(&fx, MAIN).expect("main is woken");
    assert!(
        s.contains("[report: done] fini") && s.contains("- API v2"),
        "{}",
        s
    );
    t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    assert_eq!(t.status("a"), Status::Done);
    // a new turn clears the declared status
    t.go(Input::ReplLine {
        agent: "a".into(),
        line: "  obs: turn_started".into(),
    });
    assert_eq!(t.status("a"), Status::Working);
}

#[test]
fn only_main_controls_tasks() {
    let mut t = T::new();
    t.spawn_task("a");
    let (tok, fx) = t.req(
        "a",
        AgentReq::Stop {
            agent: "main".into(),
            reason: "x".into(),
        },
    );
    assert!(reply(&fx, tok).unwrap()["error"]
        .as_str()
        .unwrap()
        .contains("reserved for main"));
}

#[test]
fn journal_replay_rebuilds_the_same_state() {
    let mut t = T::new();
    t.spawn_task("a");
    let fx = t.user(MAIN, "/new -w b: autre");
    let mut journal: Vec<Value> = Vec::new();
    // collect every journal event the hub emitted so far, by replaying
    // the scenario with a recorder
    let _ = fx;
    let mut t2 = T::new();
    let mut record = |fx: Vec<Effect>| {
        for e in fx {
            if let Effect::Journal(ev) = e {
                journal.push(ev);
            }
        }
    };
    let (_, fx) = t2.req(
        MAIN,
        AgentReq::Spawn {
            name: "a".into(),
            brief: Brief {
                objective: "objective of a".into(),
                ..Brief::default()
            },
            worktree: false,
            with_changes: false,
            place: String::new(),
            feature: String::new(),
            ask: Default::default(),
        },
    );
    record(fx);
    record(t2.user(MAIN, "/new -w b: autre"));
    let mut h = Hub::new("/w");
    assert!(h.replay(&journal).is_empty(), "sb-core knows every kind it wrote");
    assert_eq!(h.st.order, t2.hub.st.order);
    assert_eq!(h.st.msgs, t2.hub.st.msgs);
    assert_eq!(h.st.agents["b"].ws, t2.hub.st.agents["b"].ws);
}

/// After a /version rollback, the journal can hold an event kind this
/// sb-core does not know: it is not applied, and replay names it (the
/// daemon logs it) instead of losing it in silence.
#[test]
fn replay_returns_the_events_sb_core_does_not_know() {
    let mut h = Hub::new("/w");
    let known = json!({"type": "main_note", "text": "hello", "at_ms": 1});
    let newer = json!({"type": "from_a_newer_hub", "name": "a"});
    let skipped = h.replay(&[known, newer.clone()]);
    assert_eq!(skipped, vec![newer]);
    assert_eq!(h.st.main_notes.len(), 1, "the known event is applied");
}

#[test]
fn a_report_answers_the_parent_and_no_auto_reply_repeats_it() {
    let mut t = T::new();
    t.spawn_task("a");
    let (tok, fx) = t.req(
        "a",
        AgentReq::Report {
            kind: "done".into(),
            summary: "fait".into(),
            decisions: vec![],
        },
    );
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true);
    let s = say_to(&fx, MAIN).unwrap();
    assert!(
        s.contains("reply_to=\"m_"),
        "the report replies to the brief: {}",
        s
    );
    t.go(Input::ReplLine {
        agent: "a".into(),
        line: "  obs: assistant: fait, fichier écrit".into(),
    });
    // main is busy with the report: nothing else may reach it at a's idle
    let fx = t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    assert!(
        steer_to(&fx, MAIN).is_none() && say_to(&fx, MAIN).is_none(),
        "no duplicate: {:?}",
        fx
    );
}

#[test]
fn steering_the_model_never_read_starts_a_new_turn() {
    let mut t = T::new();
    t.user(MAIN, "premier");
    t.user(MAIN, "question tardive");
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: steering_received: question tardive".into(),
    });
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: assistant: réponse au premier".into(),
    });
    let fx = t.go(Input::ReplIdle {
        agent: MAIN.into(),
        leftover: false,
    });
    let s = say_to(&fx, MAIN).expect("a new turn");
    assert!(s.contains("not answered them yet"), "{}", s);
    // when the model did read it, nothing happens
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: turn_started".into(),
    });
    t.user(MAIN, "encore");
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: steering_received: encore".into(),
    });
    t.go(Input::ReplLine {
        agent: MAIN.into(),
        line: "  obs: steered: encore".into(),
    });
    let fx = t.go(Input::ReplIdle {
        agent: MAIN.into(),
        leftover: false,
    });
    assert!(say_to(&fx, MAIN).is_none(), "{:?}", fx);
}

#[test]
fn main_hears_when_a_task_crashes_and_when_it_fails() {
    let mut t = T::new();
    t.spawn_task("a");
    t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    t.turn(MAIN, "vu");
    let fx = t.go(Input::ReplExited {
        agent: "a".into(),
        crashed: true,
        reason: "bend: out of memory".into(),
    });
    let s = say_to(&fx, MAIN).expect("main is woken");
    assert!(s.contains("from=\"bise\" relation=\"hub\""), "{}", s);
    assert!(
        s.contains("agent @a crashed (bend: out of memory)") && s.contains("attempt 1/5"),
        "{}",
        s
    );
    for _ in 2..=5 {
        t.go(Input::ReplExited {
            agent: "a".into(),
            crashed: true,
            reason: "boom".into(),
        });
    }
    t.turn(MAIN, "vu");
    let fx = t.go(Input::ReplExited {
        agent: "a".into(),
        crashed: true,
        reason: "boom".into(),
    });
    let s = say_to(&fx, MAIN).expect("main is woken again");
    assert!(s.contains("agent @a failed"), "{}", s);
}

#[test]
fn many_task_messages_always_reach_main() {
    let mut t = T::new();
    t.spawn_task("a");
    for i in 0..30 {
        t.turn(MAIN, "ok");
        let (_, fx) = t.req(
            "a",
            AgentReq::Send {
                to: MAIN.into(),
                text: format!("progress {}", i),
                expect_reply: false,
                reply_to: None,
                queued: false,
                why: String::new(),
                switch: None,
            },
        );
        assert!(say_to(&fx, MAIN).is_some(), "message {} held: {:?}", i, fx);
    }
}

#[test]
fn a_user_message_to_main_starts_with_the_task_status() {
    let mut t = T::new();
    let fx = t.user(MAIN, "salut");
    assert_eq!(
        say_to(&fx, MAIN).as_deref(),
        Some("salut"),
        "no task: no status"
    );
    t.turn(MAIN, "ok");
    t.spawn_task("docs");
    t.go(Input::ReplLine {
        agent: "docs".into(),
        line: "tool #3 bash : cargo test --all".into(),
    });
    t.turn(MAIN, "ok");
    let fx = t.user(MAIN, "où en est-on ?");
    let s = say_to(&fx, MAIN).unwrap();
    assert!(s.starts_with("<task_status>\ndocs working"), "{}", s);
    assert!(s.contains("now: bash `cargo test --all`"), "{}", s);
    assert!(s.ends_with("où en est-on ?"), "{}", s);
    // a task's message to main carries no status block
    let (_, fx) = t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "fini".into(),
            expect_reply: false,
            reply_to: None,
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    let s = steer_to(&fx, MAIN).or_else(|| say_to(&fx, MAIN)).unwrap();
    assert!(!s.contains("<task_status>"), "{}", s);
}

#[test]
fn sb_tasks_details_every_task() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ReplLine {
        agent: "docs".into(),
        line: "tool #3 bash : npm run build".into(),
    });
    t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    let (tok, fx) = t.req(MAIN, AgentReq::Tasks);
    let text = reply(&fx, tok).unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        text.starts_with("## docs — working — shared workspace"),
        "{}",
        text
    );
    assert!(text.contains("objective: objective of docs"), "{}", text);
    assert!(
        text.contains("last activity (0s ago): bash `npm run build`"),
        "{}",
        text
    );
    assert!(text.contains("waits for a reply from main"), "{}", text);
}

#[test]
fn any_number_of_tasks_work_at_once() {
    let mut t = T::new();
    for n in ["a", "b", "c", "d", "e", "f", "g"] {
        t.spawn_task(n);
        assert_eq!(t.status(n), Status::Working, "{} starts right away", n);
    }
}

#[test]
fn agents_talk_as_long_as_they_want() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let mut last: Option<u64> = None;
    for i in 0..60 {
        let (from, to) = if i % 2 == 0 { ("a", "b") } else { ("b", "a") };
        t.go(Input::ReplIdle {
            agent: to.into(),
            leftover: false,
        });
        let (tok, fx) = t.req(
            from,
            AgentReq::Send {
                to: to.into(),
                text: i.to_string(),
                expect_reply: false,
                reply_to: last,
                queued: false,
                why: String::new(),
                switch: None,
            },
        );
        assert_eq!(reply(&fx, tok).unwrap()["ok"], true, "message {}", i);
        assert!(
            say_to(&fx, to).is_some(),
            "message {} delivered: {:?}",
            i,
            fx
        );
        last = t.hub.st.msgs.values().last().map(|m| m.id);
    }
    assert!(t.hub.st.cards.is_empty());
}

fn send(t: &mut T, from: &str, to: &str, text: &str, queued: bool) -> (u64, Vec<Effect>) {
    t.req(
        from,
        AgentReq::Send {
            to: to.into(),
            text: text.into(),
            expect_reply: false,
            reply_to: None,
            queued,
            why: String::new(),
            switch: None,
        },
    )
}

#[test]
fn a_queued_message_waits_for_the_end_of_the_turn() {
    let mut t = T::new();
    t.spawn_task("a");
    // `a` is busy: the queued message is neither steered nor said
    let (tok, fx) = send(&mut t, MAIN, "a", "later", true);
    assert_eq!(reply(&fx, tok).unwrap()["delivery"], "queued");
    assert!(
        steer_to(&fx, "a").is_none() && say_to(&fx, "a").is_none(),
        "{:?}",
        fx
    );
    let id = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "later")
        .unwrap()
        .id;
    assert!(
        t.hub.st.msgs[&id].queued,
        "the mode is in the journal event"
    );
    // a steer message meanwhile goes in at once, alone
    let (tok, fx) = send(&mut t, MAIN, "a", "now", false);
    assert_eq!(reply(&fx, tok).unwrap()["delivery"], "delivered");
    let s = steer_to(&fx, "a").expect("steered");
    assert!(s.contains("now") && !s.contains("later"), "{}", s);
    // the turn ends: the queued message starts a new turn
    let fx = t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    let s = say_to(&fx, "a").expect("a new turn");
    assert!(
        s.contains("later") && s.contains(&format!("m_{}", id)),
        "{}",
        s
    );
    assert_eq!(t.hub.st.msg_state[&id], MsgState::Delivered);
}

#[test]
fn a_queued_message_to_an_idle_agent_is_delivered_at_once() {
    let mut t = T::new();
    t.spawn_task("a");
    t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    let (tok, fx) = send(&mut t, MAIN, "a", "go", true);
    assert_eq!(reply(&fx, tok).unwrap()["delivery"], "delivered");
    assert!(say_to(&fx, "a").unwrap().contains("go"));
}

#[test]
fn a_queued_message_does_not_end_a_wait() {
    let mut t = T::new();
    t.spawn_task("a");
    let (tok, _) = send(&mut t, "a", MAIN, "question", false);
    let q = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "question")
        .unwrap()
        .id;
    let (wtok, _) = t.req(
        "a",
        AgentReq::Wait {
            msg: q,
            timeout_s: 20,
        },
    );
    let _ = tok;
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "a".into(),
            text: "answer".into(),
            expect_reply: false,
            reply_to: Some(q),
            queued: true,
            why: String::new(),
            switch: None,
        },
    );
    assert!(
        reply(&fx, wtok).is_none() && steer_to(&fx, "a").is_none(),
        "{:?}",
        fx
    );
    let fx = t.go(Input::ReplIdle {
        agent: "a".into(),
        leftover: false,
    });
    assert!(say_to(&fx, "a").unwrap().contains("answer"));
}

/// Opens a question card for a question of `docs` to main; returns
/// (message id, card id).
fn docs_question_card(t: &mut T) -> (u64, u64) {
    t.spawn_task("docs");
    t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    let id = t
        .hub
        .st
        .msgs
        .values()
        .find(|m| m.text == "v1 ou v2 ?")
        .unwrap()
        .id;
    t.req(
        MAIN,
        AgentReq::Card {
            text: "v1 ou v2 ?".into(),
            for_msg: Some(id),
        },
    );
    let card = *t.hub.st.cards.keys().next().unwrap();
    (id, card)
}

/// BISE-299: once main escalated a question, it is the user's: main's
/// reply to it is refused, the card stays; only the user's answer
/// resolves it, and it goes to the task that asked.
#[test]
fn main_cannot_answer_an_escalated_question_only_the_user_can() {
    let mut t = T::new();
    let (id, card) = docs_question_card(&mut t);
    // a plain message from main does not close it: the view says so
    t.env.now += 10;
    t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "patiente".into(),
            expect_reply: false,
            reply_to: None,
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    assert!(t.hub.st.cards.contains_key(&card));
    let snap = t.hub.snapshot(t.env.now);
    let note = snap["cards"][0]["note"].as_str().unwrap_or("");
    assert!(note.contains("@main wrote to @docs"), "{}", snap);
    // main's reply to the question is refused: the card stays
    let (tok, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "v2".into(),
            expect_reply: false,
            reply_to: Some(id),
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    let e = err_of(&fx, tok);
    assert!(e.contains("in the user's inbox") && e.contains("--withdraw"), "{e}");
    assert!(t.hub.st.cards.contains_key(&card));
    assert!(!t.hub.st.msgs.values().any(|m| m.text == "v2"));
    // a peer's too (no withdraw hint: it is not its card)
    t.spawn_task("peer");
    let (tok, fx) = t.req(
        "peer",
        AgentReq::Send {
            to: "docs".into(),
            text: "v1".into(),
            expect_reply: false,
            reply_to: Some(id),
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    let e = err_of(&fx, tok);
    assert!(e.contains("only the user answers it") && !e.contains("--withdraw"), "{e}");
    assert!(t.hub.st.cards.contains_key(&card));
    // the user's answer resolves it and goes to docs, as a reply to its question
    t.user(MAIN, &format!("/answer {} v2", card));
    assert!(t.hub.st.cards.is_empty());
    let a = t.hub.st.msgs.values().find(|m| m.text == "v2").expect("the user's answer");
    assert_eq!((a.from.as_str(), a.to.as_str(), a.reply_to), (USER, "docs", Some(id)));
}

/// BISE-299: the agents' traffic never reaches the user's inbox: a
/// question to main, a blocked status, done/blocked/failed reports.
#[test]
fn agent_traffic_never_reaches_the_user_inbox() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let ask = |t: &mut T, from: &str, to: &str| {
        t.req(
            from,
            AgentReq::Send {
                to: to.into(),
                text: format!("{} asks {}", from, to),
                expect_reply: true,
                reply_to: None,
                queued: false,
                why: String::new(),
                switch: None,
            },
        )
    };
    ask(&mut t, "a", MAIN);
    ask(&mut t, "b", MAIN);
    ask(&mut t, "a", "b");
    t.req("a", AgentReq::Status { status: Declared::Blocked, note: "need a key".into() });
    for kind in ["blocked", "done", "failed"] {
        t.req("b", AgentReq::Report { kind: kind.into(), summary: format!("{} summary", kind), decisions: vec![] });
    }
    assert!(t.hub.st.cards.is_empty(), "{:?}", t.hub.st.cards);
    assert_eq!(t.hub.snapshot(t.env.now)["cards"].as_array().unwrap().len(), 0);
    // they are main's: its inbox counts the two questions, a blocked task
    // wakes main with a message
    let snap = t.hub.snapshot(t.env.now);
    let main = snap["agents"].as_array().unwrap().iter().find(|a| a["name"] == MAIN).unwrap().clone();
    assert_eq!(main["inbox"], 2, "{main}");
    assert!(t.hub.st.msgs.values().any(|m| m.to == MAIN && m.text.contains("@a is blocked: need a key")));
}

/// BISE-299: main withdraws its own card with a why (the user sees it);
/// the question is main's again, and main may then answer it.
#[test]
fn main_withdraws_its_own_card_with_a_why() {
    let mut t = T::new();
    let (id, card) = docs_question_card(&mut t);
    let (tok, fx) = t.req("docs", AgentReq::Withdraw { card, why: "moot".into() });
    assert!(err_of(&fx, tok).contains("reserved for main"));
    let (tok, fx) = t.req(MAIN, AgentReq::Withdraw { card, why: "  ".into() });
    assert!(err_of(&fx, tok).contains("say why"));
    assert!(t.hub.st.cards.contains_key(&card));
    let (tok, fx) = t.req(MAIN, AgentReq::Withdraw { card, why: "the user answered in chat: v2".into() });
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true);
    assert!(t.hub.st.cards.is_empty());
    assert!(has_line(&fx, MAIN, &format!("#{} withdrawn by main: the user answered in chat: v2", card)), "{:?}", fx);
    let (tok, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "v2".into(),
            expect_reply: false,
            reply_to: Some(id),
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    assert_eq!(reply(&fx, tok).unwrap()["ok"], true, "{:?}", fx);
}

#[test]
fn the_user_closes_a_card_without_answering() {
    let mut t = T::new();
    let (_, card) = docs_question_card(&mut t);
    let fx = t.user(MAIN, &format!("/close {}", card));
    assert!(t.hub.st.cards.is_empty());
    assert!(say_to(&fx, "docs").is_none());
    assert!(has_line(&fx, MAIN, &format!("#{} closed", card)), "{:?}", fx);
}

/// card-wake: answering main's own card (no --for) starts a main turn
/// holding the answer, like a card for a task reaches that task; it does
/// not wait as a note for the user's next message.
#[test]
fn answering_a_main_card_starts_a_main_turn_holding_the_answer() {
    let mut t = T::new();
    t.user(MAIN, "prépare la release");
    let (_, _) = t.req(MAIN, AgentReq::Card { text: "on publie ce soir ?".into(), for_msg: None });
    let card = *t.hub.st.cards.keys().next().unwrap();
    t.turn(MAIN, "J'ai demandé à l'utilisateur.");
    assert_eq!(t.status(MAIN), Status::Idle);
    let fx = t.user(MAIN, &format!("/answer {} oui, à 20h", card));
    assert!(t.hub.st.cards.is_empty());
    let said = say_to(&fx, MAIN).expect("the answer starts a main turn");
    assert!(said.contains(&format!("card #{}", card)) && said.contains("on publie ce soir ?") && said.contains("oui, à 20h"), "{said}");
    assert_eq!(t.status(MAIN), Status::Working);
    assert!(!t.hub.st.main_notes.iter().any(|n| n.contains("oui, à 20h")), "{:?}", t.hub.st.main_notes);
    // busy: a second answer steers the running turn
    t.req(MAIN, AgentReq::Card { text: "et le changelog ?".into(), for_msg: None });
    let card = *t.hub.st.cards.keys().next().unwrap();
    let fx = t.user(MAIN, &format!("/answer {} je l'écris", card));
    let steered = steer_to(&fx, MAIN).expect("a busy main gets it at once");
    assert!(steered.contains("je l'écris"), "{steered}");
}

/// docs asks main (a plain send, not an ask), main answers, docs waits
/// afterwards: (question id, reply id, the fx of main's reply).
fn question_then_reply(t: &mut T, queued: bool) -> (u64, u64) {
    t.spawn_task("docs");
    let (_, _) = t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "v1 ou v2 ?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    let q = t.hub.st.msgs.values().find(|m| m.text == "v1 ou v2 ?").unwrap().id;
    t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "v2".into(),
            expect_reply: false,
            reply_to: Some(q),
            queued,
            why: String::new(),
            switch: None,
        },
    );
    let r = t.hub.st.msgs.values().find(|m| m.text == "v2").unwrap().id;
    (q, r)
}

fn delivered_events(fx: &[Effect], id: u64) -> usize {
    fx.iter()
        .filter(|e| {
            matches!(e, Effect::Journal(ev) if ev["type"] == "message_state" && ev["id"] == id && ev["state"] == "delivered")
        })
        .count()
}

/// Found by the law delivered_once (L3): a reply already delivered (here
/// steered into docs' turn) is read again by `sb wait`, never delivered a
/// second time.
#[test]
fn waiting_for_a_delivered_reply_does_not_deliver_it_twice() {
    let mut t = T::new();
    let (q, r) = question_then_reply(&mut t, false);
    assert!(matches!(t.hub.st.msg_state.get(&r), Some(MsgState::Delivered)));
    let (tok, fx) = t.req("docs", AgentReq::Wait { msg: q, timeout_s: 20 });
    assert_eq!(reply(&fx, tok).expect("the wait returns the reply")["message"], "v2");
    assert_eq!(delivered_events(&fx, r), 0, "{:?}", fx);
}

/// Found by the law delivered_once (L3): a queued-mode reply still queued
/// is never handed to `sb wait`; it arrives as the next turn.
#[test]
fn a_queued_reply_is_not_handed_to_a_wait() {
    let mut t = T::new();
    let (q, r) = question_then_reply(&mut t, true);
    assert!(matches!(t.hub.st.msg_state.get(&r), Some(MsgState::Queued { .. })));
    let (tok, fx) = t.req("docs", AgentReq::Wait { msg: q, timeout_s: 20 });
    assert!(reply(&fx, tok).is_none(), "the wait keeps waiting: {:?}", fx);
    assert_eq!(delivered_events(&fx, r), 0);
    let fx = t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    assert!(say_to(&fx, "docs").unwrap_or_default().contains("v2"), "{:?}", fx);
    assert_eq!(delivered_events(&fx, r), 1);
}

fn err_of(fx: &[Effect], tok: u64) -> String {
    reply(fx, tok).unwrap()["error"].as_str().unwrap_or("").to_string()
}

/// BISE-299: only the user closes a card of the user's inbox: `sb close`
/// is refused to a task and to main alike.
#[test]
fn neither_main_nor_a_task_closes_a_user_card() {
    let mut t = T::new();
    let (_, card) = docs_question_card(&mut t);
    let (tok, fx) = t.req("docs", AgentReq::Close { card, note: "done".into() });
    assert!(err_of(&fx, tok).contains("reserved for main"));
    let (tok, fx) = t.req(MAIN, AgentReq::Close { card: 99, note: String::new() });
    assert!(err_of(&fx, tok).contains("no open card #99"));
    let (tok, fx) = t.req(MAIN, AgentReq::Close { card, note: "handled".into() });
    let e = err_of(&fx, tok);
    assert!(e.contains("only the user answers or closes it"), "{e}");
    assert!(t.hub.st.cards.contains_key(&card));
    // the user can
    t.user(MAIN, &format!("/close {}", card));
    assert!(t.hub.st.cards.is_empty());
}

#[test]
fn main_renames_a_task_and_tasks_cannot() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let ren = |a: &str, n: &str| AgentReq::Rename { agent: a.into(), new_name: n.into() };
    let (tok, fx) = t.req("a", ren("b", "c"));
    assert!(err_of(&fx, tok).contains("reserved for main"));
    let (tok, fx) = t.req(MAIN, ren("a", "b"));
    assert!(err_of(&fx, tok).contains("invalid or taken name"));
    let (tok, fx) = t.req(MAIN, ren("a", "Bad Name"));
    assert!(err_of(&fx, tok).contains("invalid or taken name"));
    let (tok, fx) = t.req(MAIN, ren("main", "boss"));
    assert!(err_of(&fx, tok).contains("no agent named"));
    let (tok, fx) = t.req(MAIN, ren("a", "alpha"));
    assert_eq!(reply(&fx, tok).unwrap()["name"], "alpha");
    assert!(t.hub.st.agents.contains_key("alpha"));
    // the old name still works
    let (tok, fx) = t.req(MAIN, ren("a", "alpha2"));
    assert_eq!(reply(&fx, tok).unwrap()["name"], "alpha2");
}

#[test]
fn main_restores_and_isolates_a_task_and_tasks_cannot() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    t.go(Input::ReplIdle { agent: "b".into(), leftover: false });
    t.user(MAIN, "/archive b --force");
    assert_eq!(t.status("b"), Status::Archived);
    let (tok, fx) = t.req("a", AgentReq::Restore { agent: "b".into() });
    assert!(err_of(&fx, tok).contains("reserved for main"));
    assert_eq!(t.status("b"), Status::Archived);
    let (tok, fx) = t.req(MAIN, AgentReq::Restore { agent: "b".into() });
    assert_eq!(reply(&fx, tok).unwrap()["name"], "b");
    assert!(fx
        .iter()
        .any(|e| matches!(e, Effect::Spawn { agent, resume: true, .. } if agent == "b")));
    let (tok, fx) = t.req(MAIN, AgentReq::Restore { agent: "b".into() });
    assert!(err_of(&fx, tok).contains("active"), "{:?}", reply(&fx, tok));
    // isolate: a task cannot, main can (a has changed nothing yet)
    t.go(Input::ReplIdle { agent: "a".into(), leftover: false });
    let (tok, fx) = t.req("b", AgentReq::Isolate { agent: "a".into() });
    assert!(err_of(&fx, tok).contains("reserved for main"));
    let (tok, fx) = t.req(MAIN, AgentReq::Isolate { agent: "a".into() });
    assert_eq!(reply(&fx, tok).unwrap()["name"], "a", "{:?}", fx);
    assert!(t.hub.st.agents["a"].ws.mode == crate::model::Mode::Worktree);
    let (tok, fx) = t.req(MAIN, AgentReq::Isolate { agent: "a".into() });
    assert!(err_of(&fx, tok).contains("already"), "{:?}", reply(&fx, tok));
}

fn patch_line(f: &str) -> String {
    format!("tool #1 apply_patch : {{\"arg\":\"*** Begin Patch\\n*** Update File: {}\\n@@\\n-x\\n+y\\n*** End Patch\"}}", f)
}

fn spawn_in(t: &mut T, name: &str, place: &str) -> (u64, Vec<Effect>) {
    t.req(
        MAIN,
        AgentReq::Spawn {
            name: name.into(),
            brief: Brief { objective: format!("objective of {}", name), ..Brief::default() },
            worktree: place == "new",
            with_changes: false,
            place: if place == "new" { String::new() } else { place.into() },
            feature: String::new(),
            ask: Default::default(),
        },
    )
}

/// dev-flow §3.1: places are shared, not owned. Two agents in one
/// worktree; a drop keeps the folder while another agent is in it, the
/// last one takes it (and marks it gone for the first); a restore brings
/// the place back once, the other agent rejoins it.
#[test]
fn a_worktree_shared_by_two_agents_goes_with_its_last_one() {
    let mut t = T::new();
    let (tok, fx) = spawn_in(&mut t, "a", "new");
    assert!(reply(&fx, tok).is_some(), "{:?}", fx);
    let (tok, fx) = spawn_in(&mut t, "b", "a");
    assert_eq!(reply(&fx, tok).unwrap()["path"], "/state/worktrees/a", "{:?}", fx);
    let (wa, wb) = (t.hub.st.agents["a"].ws.clone(), t.hub.st.agents["b"].ws.clone());
    assert_eq!(wa, wb, "one place, the same ws");
    assert_eq!(wa.place.as_deref(), Some("wt:a"));
    // by its branch too; an unknown place or a shared agent is refused
    let (tok, fx) = spawn_in(&mut t, "c", "sb/a");
    assert_eq!(reply(&fx, tok).unwrap()["path"], "/state/worktrees/a");
    let (tok, fx) = spawn_in(&mut t, "d", "nowhere");
    assert!(err_of(&fx, tok).contains("no place nowhere"), "{:?}", fx);
    // the snapshot: one box, its agents in order
    let snap = t.hub.snapshot(0);
    assert_eq!(
        snap["places"],
        json!([{"id": "wt:a", "branch": "sb/a", "agents": ["a", "b", "c"], "pr": null, "lid": null, "feature": false, "trying": false}])
    );
    assert_eq!(snap["agents"][1]["place_id"], "wt:a");
    assert_eq!(snap["agents"][0]["place_id"], "shared");
    // files are tracked in a worktree too; an overlap is within the place
    // (b's change of src/x.rs is one, the shared folder's s's is not)
    t.spawn_task("s");
    for n in ["a", "b", "s"] {
        t.go(Input::ReplLine { agent: n.into(), line: patch_line("src/x.rs") });
    }
    assert_eq!(t.hub.st.agents["a"].files.iter().collect::<Vec<_>>(), ["src/x.rs"]);
    let overlaps: Vec<&String> = t.hub.st.main_notes.iter().filter(|n| n.contains("file overlap")).collect();
    assert_eq!(overlaps.len(), 1, "{:?}", overlaps);
    assert!(overlaps[0].contains("changed by @b and by @a"), "{:?}", overlaps);
    for n in ["a", "b", "c"] {
        t.go(Input::ReplIdle { agent: n.into(), leftover: false });
    }
    t.hub.force_run("a", Run::Idle);
    // drop a, then c: the worktree stays with the others
    let fx = t.user(MAIN, "/archive a --force");
    assert!(has_line(&fx, MAIN, "@a archived — its worktree stays with @b, @c"), "{:?}", fx);
    t.user(MAIN, "/archive c --force");
    assert!(t.env.dropped.is_empty(), "no folder removed yet");
    assert!(!t.hub.st.agents["a"].ws.dropped);
    // b is the last: the folder goes, saved work included, for a and c too
    t.env.loss = Loss { dirty: 1, unpushed: 0 };
    let fx = t.user(MAIN, "/archive b --force");
    assert_eq!(t.env.dropped, vec!["b".to_string()], "{:?}", fx);
    for n in ["a", "b", "c"] {
        let a = &t.hub.st.agents[n];
        assert!(a.ws.dropped, "{} dropped", n);
        assert_eq!(a.snapshot_ref.as_deref(), Some("refs/switchboard/trash/b/1"), "{}", n);
    }
    assert_eq!(t.hub.snapshot(0)["places"], json!([]));
    // a's restore brings the place back; c rejoins it, no second folder
    let (tok, fx) = t.req(MAIN, AgentReq::Restore { agent: "a".into() });
    assert_eq!(reply(&fx, tok).unwrap()["name"], "a", "{:?}", fx);
    let (tok, fx) = t.req(MAIN, AgentReq::Restore { agent: "c".into() });
    assert_eq!(reply(&fx, tok).unwrap()["name"], "c", "{:?}", fx);
    let (wa, wc) = (&t.hub.st.agents["a"].ws, &t.hub.st.agents["c"].ws);
    assert!(!wa.dropped && wa == wc, "{:?} {:?}", wa, wc);
    assert_eq!(t.hub.st.agents["c"].snapshot_ref, None);
    assert_eq!(t.hub.snapshot(0)["places"][0]["agents"], json!(["a", "c"]));
}

/// `sb move`: an agent that changed nothing yet joins another place,
/// goes back to the shared folder, or gets a new worktree (/isolate).
#[test]
fn sb_move_between_places() {
    let mut t = T::new();
    spawn_in(&mut t, "a", "new");
    t.spawn_task("s");
    t.spawn_task("busy");
    for n in ["a", "s", "busy"] {
        t.go(Input::ReplIdle { agent: n.into(), leftover: false });
    }
    let mv = |t: &mut T, from: &str, who: &str, to: &str| t.req(from, AgentReq::Move { agent: who.into(), place: to.into() });
    let (tok, fx) = mv(&mut t, "s", "s", "a");
    assert!(err_of(&fx, tok).contains("reserved for main"), "{:?}", fx);
    // a is alone in its worktree: it does not leave it
    let (tok, fx) = mv(&mut t, MAIN, "a", "shared");
    assert!(err_of(&fx, tok).contains("alone in its worktree"), "{:?}", fx);
    let (tok, fx) = mv(&mut t, MAIN, "s", "a");
    assert_eq!(reply(&fx, tok).unwrap()["name"], "s", "{:?}", fx);
    assert!(has_line(&fx, MAIN, "@s moved to the worktree sb/a"), "{:?}", fx);
    assert!(fx.iter().any(|e| matches!(e, Effect::Kill { agent } if agent == "s")));
    assert_eq!(t.hub.st.agents["s"].ws, t.hub.st.agents["a"].ws);
    let (tok, fx) = mv(&mut t, MAIN, "s", "a");
    assert!(err_of(&fx, tok).contains("already there"), "{:?}", fx);
    // back to the shared folder (a stays with its worktree), once s read
    // the move's message (its turn ended)
    t.go(Input::ReplIdle { agent: "s".into(), leftover: false });
    t.go(Input::ReplIdle { agent: "s".into(), leftover: false });
    let (tok, fx) = mv(&mut t, MAIN, "s", "shared");
    assert_eq!(reply(&fx, tok).unwrap()["name"], "s", "{}", err_of(&fx, tok));
    assert_eq!(t.hub.st.agents["s"].ws.mode, crate::model::Mode::Shared);
    // an agent that changed a file does not move
    t.go(Input::ReplLine { agent: "busy".into(), line: patch_line("src/y.rs") });
    let (tok, fx) = mv(&mut t, MAIN, "busy", "a");
    assert!(err_of(&fx, tok).contains("already changed files (src/y.rs)"), "{:?}", fx);
    // new: a worktree of its own (/isolate)
    t.go(Input::ReplIdle { agent: "s".into(), leftover: false });
    t.go(Input::ReplIdle { agent: "s".into(), leftover: false });
    let (tok, fx) = mv(&mut t, MAIN, "s", "new");
    assert_eq!(reply(&fx, tok).unwrap()["name"], "s", "{:?}", fx);
    assert_eq!(t.hub.st.agents["s"].ws.place.as_deref(), Some("wt:s"));
}

/// Found while stating the law L4 (no message stuck in a queue): a message
/// queued for a task that is renamed before it is delivered reaches the
/// task under its new name.
#[test]
fn a_message_queued_before_a_rename_reaches_the_new_name() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.hub.force_run("docs", Run::Starting);
    t.user(MAIN, "@docs change de plan");
    let id = t.hub.st.msgs.values().find(|m| m.text == "change de plan").unwrap().id;
    assert!(matches!(t.hub.st.msg_state.get(&id), Some(MsgState::Queued { .. })));
    t.user(MAIN, "/rename docs api");
    assert!(t.hub.st.agents.contains_key("api"));
    let fx = t.go(Input::ReplReady { agent: "api".into() });
    assert!(say_to(&fx, "api").unwrap_or_default().contains("change de plan"), "{:?}", fx);
    assert!(matches!(t.hub.st.msg_state.get(&id), Some(MsgState::Delivered)));
}

/// Found by the L4 proof (no message stuck in a queue), also in the Rust
/// origin: a failed task whose REPL is idle keeps its queued mail; a
/// /restore gives it that mail as a new turn.
#[test]
fn a_restored_idle_task_gets_its_queued_mail() {
    let mut t = T::new();
    t.spawn_task("docs");
    // a queued-mode message waits for the end of docs' turn
    t.req(
        MAIN,
        AgentReq::Send {
            to: "docs".into(),
            text: "plus tard".into(),
            expect_reply: false,
            reply_to: None,
            queued: true,
            why: String::new(),
            switch: None,
        },
    );
    t.req(
        "docs",
        AgentReq::Report {
            kind: "failed".into(),
            summary: "bloqué".into(),
            decisions: vec![],
        },
    );
    let fx = t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    assert!(say_to(&fx, "docs").is_none(), "a failed task gets no turn");
    let fx = t.user(MAIN, "/restore docs");
    assert!(say_to(&fx, "docs").unwrap_or_default().contains("plus tard"), "{:?}", fx);
}

// BR-007: a task whose turn fails (network down, provider error) must
// not go quiet: its parent gets the real cause as a report
#[test]
fn a_failed_turn_of_a_task_reaches_main() {
    let mut t = T::new();
    t.spawn_task("net");
    let fx = t.go(Input::ReplLine {
        agent: "net".into(),
        line: "  obs: turn_done: failed: provider failed after 10 attempts: cannot reach api.example.com (connect 61 Connection refused) — check your network or VPN".into(),
    });
    let said = say_to(&fx, MAIN).unwrap_or_default();
    assert!(
        said.contains("[report: turn_failed]") && said.contains("cannot reach api.example.com"),
        "{:?}",
        fx
    );
}

/// Issue #4: a spawn that asks for a model waits for the daemon's pick
/// (`SpawnModel`, its answer inside); one that asks nothing is answered
/// as before.
#[test]
fn a_spawn_with_a_model_is_answered_by_the_daemons_pick() {
    let mut t = T::new();
    let spawn = |name: &str, model: &str| AgentReq::Spawn {
        name: name.into(),
        brief: Brief { objective: "o".into(), ..Brief::default() },
        worktree: false,
        with_changes: false,
        place: String::new(),
        feature: String::new(),
        ask: bise_catalog::spawn::Ask { model: model.into(), ..Default::default() },
    };
    let (tok, fx) = t.req(MAIN, spawn("fast", "mistral/mistral-small-latest"));
    let picked = fx.iter().find_map(|e| match e {
        Effect::SpawnModel { token, agent, ask, body } if *token == tok => Some((agent.clone(), ask.model.clone(), body.clone())),
        _ => None,
    });
    let (agent, model, body) = picked.unwrap_or_else(|| panic!("{:?}", fx));
    assert_eq!((agent.as_str(), model.as_str(), body["ok"].clone()), ("fast", "mistral/mistral-small-latest", json!(true)));
    assert!(reply(&fx, tok).is_none(), "{:?}", fx);
    let (tok, fx) = t.req(MAIN, spawn("plain", ""));
    assert!(reply(&fx, tok).is_some_and(|b| b["ok"] == true), "{:?}", fx);
    assert!(!fx.iter().any(|e| matches!(e, Effect::SpawnModel { .. })), "{:?}", fx);
}

/// Issue #4: a task's first turn on a model asked at its spawn, refused
/// by the provider: the daemon moves it (`ModelRefused`), main gets no
/// turn-failed report. Any other failure, or a later turn, reports as before.
#[test]
fn a_refused_spawn_model_moves_the_task_instead_of_failing() {
    let mut t = T::new();
    t.spawn_task("big");
    t.hub.model_trial.insert("big".into());
    let fx = t.go(Input::ReplLine {
        agent: "big".into(),
        line: "  obs: turn_done: failed: Mistral refused the request (404). check the model name and the provider URL. Mistral said: \"no such model\"".into(),
    });
    assert!(
        fx.iter().any(|e| matches!(e, Effect::ModelRefused { agent, why } if agent == "big" && why == "the provider refused it (404)")),
        "{:?}",
        fx
    );
    assert!(say_to(&fx, MAIN).is_none(), "no turn-failed report: {:?}", fx);
    assert!(!t.hub.model_trial.contains("big"));
    // the trial is over: the next failure is a failure
    let fx = t.go(Input::ReplLine {
        agent: "big".into(),
        line: "  obs: turn_done: failed: Mistral refused the request (404).".into(),
    });
    assert!(!fx.iter().any(|e| matches!(e, Effect::ModelRefused { .. })), "{:?}", fx);
}

#[test]
fn model_refusal_reads_the_refusals_of_a_model() {
    let r = |s: &str| model_refusal(s);
    assert_eq!(r("failed: OpenAI refused the key (401). check it with /setup.").as_deref(), Some("the provider refused it (401)"));
    assert_eq!(r("failed: the provider refused the request (400). x said: \"model\"").as_deref(), Some("the provider refused it (400)"));
    assert_eq!(r("failed: Mistral refused the request (429)."), None);
    assert_eq!(r("failed: Mistral refused the request (500)."), None);
    assert_eq!(r("failed: Anthropic refused the request (400). the request is too large."), None);
    assert_eq!(r("failed: provider failed after 10 attempts: cannot reach api"), None);
    assert_eq!(r("completed"), None);
}

#[test]
fn failed_turn_report_skips_main_and_user_stops() {
    assert!(failed_turn_report("net", "failed: provider 401: bad key").is_some());
    assert!(failed_turn_report("main", "failed: provider 401: bad key").is_none());
    assert!(failed_turn_report("net", "completed").is_none());
    assert!(failed_turn_report("net", "interrupted").is_none());
    assert!(failed_turn_report(
        "net",
        "failed: stopped retrying (interrupted by the user) after 2 failed attempts; last error: x"
    )
    .is_none());
    assert!(failed_turn_report(
        "net",
        "failed: stopped retrying (interrupted by main) after 2 failed attempts; last error: x"
    )
    .is_none());
    assert_eq!(
        failed_turn_report("net", "failed: interrupted by main").as_deref(),
        Some("my turn stopped: interrupted by main — a new message continues it")
    );
    assert_eq!(
        failed_turn_report("net", "failed: interrupted by the user").as_deref(),
        Some("my turn stopped: interrupted by the user — a new message continues it")
    );
}

// interrupt-who: the interrupt flag (Effect::Interrupt::by) names who asked, so the runtime's
// text says "interrupted by main" for `sb interrupt` from main and
// "by the user" for the TUI's interrupt
#[test]
fn an_interrupt_names_who_asked() {
    let mut t = T::new();
    t.spawn_task("net"); // its first turn is running
    let by_of = |fx: &[Effect]| {
        fx.iter().find_map(|e| match e {
            Effect::Interrupt { agent, by } if agent == "net" => Some(by.clone()),
            _ => None,
        })
    };
    let (_, fx) = t.req(MAIN, AgentReq::Interrupt { agent: "net".into() });
    assert_eq!(by_of(&fx).as_deref(), Some("main"), "{:?}", fx);
    let fx = t.go(Input::ClientInterrupt { client: 1, agent: "net".into() });
    assert_eq!(by_of(&fx).as_deref(), Some("user"), "{:?}", fx);
}

// ---- BISE-04: hub line protocol v2 (contract C2) ----

fn send_v2(t: &mut T, from: &str, to: &str, text: &str, expect: bool, reply_to: Option<u64>, why: &str) -> Vec<Effect> {
    let (tok, fx) = t.req(
        from,
        AgentReq::Send {
            to: to.into(),
            text: text.into(),
            expect_reply: expect,
            reply_to,
            queued: false,
            why: why.into(),
            switch: None,
        },
    );
    let r = reply(&fx, tok).expect("send answers");
    assert_eq!(r["ok"], true, "{}", r);
    fx
}

fn lines_of<'a>(fx: &'a [Effect], agent: &str) -> Vec<&'a str> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Line { agent: a, line } if a == agent => Some(line.as_str()),
            _ => None,
        })
        .collect()
}

/// Traffic between two tasks reaches main's feed as a `msg` line (level
/// 3), text whole: main no longer only sees what is sent to main.
#[test]
fn peer_traffic_reaches_mains_feed() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    let fx = send_v2(&mut t, "a", "b", "can you check logout?\nafter the fix", false, None, "");
    assert!(
        lines_of(&fx, MAIN).iter().any(|l| l.starts_with("sb msg : a → b m_") && l.ends_with(" : can you check logout?\\nafter the fix")),
        "{:?}",
        fx
    );
    // the recipient's own feed still reads it as `msg-in`
    assert!(has_line(&fx, "b", "sb msg-in : a m_"), "{:?}", fx);
}

/// Main writing to a task shows in main's feed too; a message TO main
/// shows once, as the `msg-in` of its delivery (no `msg` duplicate).
#[test]
fn main_traffic_in_mains_feed_once() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.go(Input::ReplIdle {
        agent: "docs".into(),
        leftover: false,
    });
    let fx = send_v2(&mut t, MAIN, "docs", "use v2", false, None, "");
    assert!(lines_of(&fx, MAIN).iter().any(|l| l.starts_with("sb msg : main → docs m_") && l.ends_with(" : use v2")), "{:?}", fx);
    t.turn(MAIN, "ok");
    let fx = send_v2(&mut t, "docs", MAIN, "done", false, None, "");
    let main = lines_of(&fx, MAIN);
    assert!(!main.iter().any(|l| l.starts_with("sb msg : ")), "{:?}", main);
    assert!(main.iter().any(|l| l.starts_with("sb msg-in : docs m_") && l.ends_with(" : done")), "{:?}", main);
}

/// qa-explore bug A: a message steered into a turn that never read it
/// (`leftover`) goes again as a new turn, but its `msg-in` line was
/// written when it was steered: the feed shows it once.
#[test]
fn a_steer_leftover_shows_once_in_the_feed() {
    let mut t = T::new();
    t.spawn_task("talk");
    t.user(MAIN, "go");
    let fx = send_v2(&mut t, "talk", MAIN, "two", false, None, "");
    assert!(steer_to(&fx, MAIN).is_some(), "{:?}", fx);
    let first: Vec<_> = lines_of(&fx, MAIN).into_iter().filter(|l| l.starts_with("sb msg-in : talk m_")).collect();
    assert_eq!(first.len(), 1, "{:?}", fx);
    let fx = t.go(Input::ReplIdle {
        agent: MAIN.into(),
        leftover: true,
    });
    assert!(say_to(&fx, MAIN).is_some_and(|s| s.contains("two")), "sent again: {:?}", fx);
    assert!(!has_line(&fx, MAIN, "sb msg-in : "), "no second msg-in line: {:?}", fx);
}

/// Main answering a task's question: `answered : agent : question :
/// answer : why` in main's feed (level 2), instead of the `msg` line;
/// a " : " inside a field is escaped.
#[test]
fn main_answering_a_question_is_answered() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.turn(MAIN, "ok");
    let (tok, _) = t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "v1 or v2 : which one?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    t.turn(MAIN, "thinking");
    let q = t.hub.st.msgs.values().find(|m| m.from == "docs" && m.to == MAIN).map(|m| m.id).expect("the question");
    let _ = tok;
    let fx = send_v2(&mut t, MAIN, "docs", "v2", false, Some(q), "the brief says v2");
    let main = lines_of(&fx, MAIN);
    assert!(
        main.contains(&"sb answered : docs : v1 or v2 \\: which one? : v2 : the brief says v2"),
        "{:?}",
        main
    );
    assert!(!main.iter().any(|l| l.starts_with("sb msg : ")), "{:?}", main);
    // without --why the field is there, empty
    let (_, _) = t.req(
        "docs",
        AgentReq::Send {
            to: MAIN.into(),
            text: "and the title?".into(),
            expect_reply: true,
            reply_to: None,
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    let q2 = t.hub.st.msgs.values().filter(|m| m.from == "docs" && m.to == MAIN).map(|m| m.id).max().unwrap();
    let fx = send_v2(&mut t, MAIN, "docs", "keep it", false, Some(q2), "");
    assert!(lines_of(&fx, MAIN).contains(&"sb answered : docs : and the title? : keep it : "), "{:?}", fx);
}

/// A reply of main to a task message that did not ask anything stays a
/// plain `msg` line.
#[test]
fn main_replying_to_a_non_question_is_msg() {
    let mut t = T::new();
    t.spawn_task("docs");
    t.turn(MAIN, "ok");
    send_v2(&mut t, "docs", MAIN, "fyi: done", false, None, "");
    t.turn(MAIN, "noted");
    let id = t.hub.st.msgs.values().filter(|m| m.from == "docs").map(|m| m.id).max().unwrap();
    let fx = send_v2(&mut t, MAIN, "docs", "thanks", false, Some(id), "");
    assert!(lines_of(&fx, MAIN).iter().any(|l| l.starts_with("sb msg : main → docs m_") && l.ends_with(" : thanks")), "{:?}", fx);
}

#[test]
fn fields_escape_the_separator() {
    assert_eq!(field_escape("a : b"), "a \\: b");
    assert_eq!(join_fields(&["docs".into(), "x : y".into(), "z".into(), "".into()]), "docs : x \\: y : z : ");
}

/// A message from the user that cannot reach its agent (C2 amendment,
/// BISE-86): `sb undelivered : {name} : {text}` in the feed where the
/// user wrote it, from the agent's own view or from another one (` : `
/// inside the text escaped); a message queued when the agent stops says
/// it too; an agent's message to it does not.
#[test]
fn a_message_the_user_cannot_deliver_says_so() {
    let mut t = T::new();
    t.user(MAIN, "/new -w fix: corrige le bug");
    // not ready yet: the user's message waits in the queue
    let fx = t.user("fix", "d'abord : les tests");
    assert!(lines_of(&fx, "fix").iter().all(|l| !l.starts_with("sb undelivered")), "{:?}", fx);
    let fx = t.user(MAIN, "/archive fix");
    assert_eq!(t.status("fix"), Status::Archived, "{:?}", fx);
    assert!(
        lines_of(&fx, "fix").contains(&"sb undelivered : fix : d'abord \\: les tests"),
        "{:?}",
        fx
    );
    // the worktree is gone: nothing revives it
    let fx = t.user("fix", "encore");
    assert!(lines_of(&fx, "fix").contains(&"sb undelivered : fix : encore"), "{:?}", fx);
    // from main's view: the line goes to main's feed
    let fx = t.user(MAIN, "@fix et là ?");
    assert!(lines_of(&fx, MAIN).contains(&"sb undelivered : fix : et là ?"), "{:?}", fx);
    assert!(lines_of(&fx, "fix").is_empty(), "{:?}", fx);
    // an agent sending to it gets its error, no undelivered line
    let (_, fx) = t.req(
        MAIN,
        AgentReq::Send {
            to: "fix".into(),
            text: "hello".into(),
            expect_reply: false,
            reply_to: None,
            queued: false,
            why: String::new(),
            switch: None,
        },
    );
    assert!(fx.iter().all(|e| !matches!(e, Effect::Line { line, .. } if line.starts_with("sb undelivered"))), "{:?}", fx);
}

fn ask_role(fx: &[Effect]) -> Option<(String, String, String)> {
    fx.iter().find_map(|e| match e {
        Effect::AskRole { dir, key, request } => Some((dir.clone(), key.clone(), request.clone())),
        _ => None,
    })
}

fn role_of(t: &T, name: &str) -> String {
    let snap = t.hub.snapshot(t.env.now);
    let a = snap["agents"].as_array().unwrap().iter().find(|a| a["name"] == name).unwrap().clone();
    a["role"].as_str().unwrap().to_string()
}

/// BISE-126: a task's role line starts as its objective's first
/// sentence; a turn that changed its report asks once; a turn that
/// changed nothing, main, and a turn during a call do not.
#[test]
fn a_role_line_is_asked_once_per_changed_turn() {
    let mut t = T::new();
    t.spawn_task("docs");
    assert_eq!(role_of(&t, "docs"), "objective of docs");
    assert_eq!(role_of(&t, MAIN), "");
    // main's turns never ask
    assert!(ask_role(&t.turn(MAIN, "hello")).is_none());
    // the task's first turn ends with a report: one call
    let fx = t.turn("docs", "drafted the outline");
    let (dir, key, request) = ask_role(&fx).expect("a call after the turn");
    assert!(request.contains("objective of docs") && request.contains("drafted the outline"), "{}", request);
    // a turn that ends while the call runs: no second call now...
    let fx = t.turn("docs", "wrote chapter one");
    assert!(ask_role(&fx).is_none(), "one call in flight at most: {:?}", fx);
    // ...but one when the first call is over (the inputs changed since)
    let fx = t.go(Input::RoleLine { dir: dir.clone(), key, line: Some("drafting the docs outline".into()) });
    assert_eq!(role_of(&t, "docs"), "drafting the docs outline");
    assert!(fx.contains(&Effect::State), "the views get the new line: {:?}", fx);
    let (_, key2, request2) = ask_role(&fx).expect("the pending look");
    assert!(request2.contains("wrote chapter one"), "{}", request2);
    assert!(request2.contains("drafting the docs outline"), "the line now is in the prompt");
    t.go(Input::RoleLine { dir: dir.clone(), key: key2, line: Some("writing chapter one".into()) });
    // nothing changed since: no call
    t.go(Input::ReplLine { agent: "docs".into(), line: "  obs: turn_started".into() });
    let fx = t.go(Input::ReplIdle { agent: "docs".into(), leftover: false });
    assert!(ask_role(&fx).is_none(), "{:?}", fx);
    // a failed call keeps the old line and waits before the next one
    let fx = t.turn("docs", "wrote chapter two");
    let (_, key3, _) = ask_role(&fx).unwrap();
    t.go(Input::RoleLine { dir: dir.clone(), key: key3, line: None });
    assert_eq!(role_of(&t, "docs"), "writing chapter one");
    assert!(ask_role(&t.turn("docs", "wrote chapter three")).is_none());
    t.env.now += ROLE_RETRY_MS;
    assert!(ask_role(&t.turn("docs", "wrote chapter four")).is_some());
}

/// A line saved by an earlier hub comes back, and its key spares a call.
#[test]
fn a_saved_role_line_is_kept() {
    let mut t = T::new();
    t.spawn_task("docs");
    let fx = t.turn("docs", "drafted the outline");
    let (dir, key, _) = ask_role(&fx).unwrap();
    let mut t2 = T::new();
    t2.spawn_task("docs");
    t2.hub.load_role(&dir, "drafting the outline".into(), key);
    assert_eq!(role_of(&t2, "docs"), "drafting the outline");
    assert!(ask_role(&t2.turn("docs", "drafted the outline")).is_none());
}

/// BISE-136: `sb worktree <path>` (gate.sh new) puts the private worktree
/// in the snapshot and `sb tasks`; `none` takes it back. Before any
/// `sb worktree`, a bash call that starts in a linked worktree (a `.git`
/// file) is the fallback; after one, the fallback no longer guesses.
#[test]
fn an_agent_says_where_it_works() {
    let mut t = T::new();
    t.spawn_task("docs");
    let place = |t: &T| {
        let snap = t.hub.snapshot(0);
        snap["agents"].as_array().unwrap().iter().find(|a| a["name"] == "docs").unwrap()["place"].clone()
    };
    assert!(place(&t).is_null());
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
    assert!(place(&t).is_null(), "not a linked worktree");
    let fx = bash(&mut t, &format!("cd {} && cargo test", wt));
    assert_eq!(place(&t), json!(wt));
    assert!(fx.contains(&Effect::State), "the views learn it");
    // told: the fallback stops guessing
    let (tok, fx) = t.req("docs", AgentReq::Worktree { path: "/tmp/docs-wt".into() });
    assert!(fx.contains(&Effect::Reply { token: tok, body: json!({"ok": true, "path": "/tmp/docs-wt"}) }), "{:?}", fx);
    assert_eq!(place(&t), json!("/tmp/docs-wt"));
    let (_, fx) = t.req(MAIN, AgentReq::Tasks);
    assert!(format!("{:?}", fx).contains("private worktree /tmp/docs-wt"), "{:?}", fx);
    bash(&mut t, &format!("cd {} && cargo test", wt));
    assert_eq!(place(&t), json!("/tmp/docs-wt"));
    // none: its own workspace again
    t.req("docs", AgentReq::Worktree { path: String::new() });
    assert!(place(&t).is_null());
    // the CLI's JSON: an absolute path or none
    let r = |p: &str| AgentReq::from_json(&json!({"cmd": "worktree", "path": p}));
    assert_eq!(r("none"), Ok(AgentReq::Worktree { path: String::new() }));
    assert_eq!(r("/tmp/x-wt/"), Ok(AgentReq::Worktree { path: "/tmp/x-wt".into() }));
    assert!(r("x-wt").is_err());
    // qa-explore B: main works in the workspace, it cannot mark itself
    let (tok, fx) = t.req(MAIN, AgentReq::Worktree { path: "/tmp/docs-wt".into() });
    assert!(fx.iter().any(|e| matches!(e, Effect::Reply { token, body } if *token == tok && body["ok"] == false)), "{:?}", fx);
    let _ = std::fs::remove_dir_all(&wt);
}

/// Designer's call 8 (BISE-136): a private worktree is a place like any
/// worktree (`pt:<path>`): the snapshot's `places` and its agents'
/// `place_id`, one place for two agents, a watch on what it has checked
/// out. Detached with commits: `no PR yet · 2 commits` with no forge
/// answer (no branch to ask about); with none of its own: no held line.
/// A branch read there names the place and is asked about; a merged PR
/// archives no one in it.
#[test]
fn a_private_worktree_is_a_place() {
    use crate::forge::poll::{Local, Report};
    use crate::place::{Checks, PrSnapshot, PrState, Review};
    let mut t = T::new();
    t.spawn_task("docs");
    t.spawn_task("api");
    for a in ["docs", "api"] {
        t.req(a, AgentReq::Worktree { path: "/p/docs-wt".into() });
    }
    let view = |t: &T| {
        let snap = t.hub.snapshot(0);
        snap["places"].as_array().unwrap().iter().find(|p| p["id"] == "pt:/p/docs-wt").cloned().unwrap_or(Value::Null)
    };
    let snap = t.hub.snapshot(0);
    let docs = snap["agents"].as_array().unwrap().iter().find(|a| a["name"] == "docs").unwrap().clone();
    assert_eq!(docs["place_id"], "pt:/p/docs-wt");
    assert_eq!(view(&t)["agents"], json!(["docs", "api"]));
    assert_eq!(view(&t)["branch"], Value::Null);
    let w = t.hub.pr_watches();
    assert_eq!(w.len(), 1, "{:?}", w);
    assert!(w[0].head && w[0].branch.is_empty() && w[0].path == "/p/docs-wt");
    let read = |branch: &str, commits: u32| {
        let local = vec![Local { place: "pt:/p/docs-wt".into(), branch: branch.into(), tip: None, commits: Some(commits), dirty: None }];
        Input::Prs(Report { at_ms: 1_000, prs: None, local, activity: Vec::new() })
    };
    t.go(read("", 2));
    assert_eq!(view(&t)["lid"], "no PR yet · 2 commits");
    t.go(read("", 0));
    assert_eq!(view(&t)["lid"], Value::Null);
    // a branch checked out there: the place's name, the forge's question
    let fx = t.go(read("feat/x", 0));
    assert!(fx.contains(&Effect::State), "{:?}", fx);
    assert_eq!(view(&t)["branch"], "feat/x");
    assert_eq!(t.hub.pr_watches()[0].branch, "feat/x");
    // merged: its agents stay
    let merged = PrSnapshot {
        number: 9,
        url: "u".into(),
        branch: "feat/x".into(),
        head_oid: "h".into(),
        state: PrState::Merged,
        review: Review::Approved,
        checks: Checks::Pass,
        updated_at: "t".into(),
        facts: Default::default(),
    };
    let local = vec![Local { place: "pt:/p/docs-wt".into(), branch: "feat/x".into(), tip: Some("h".into()), commits: Some(0), dirty: Some(false) }];
    let fx = t.go(Input::Prs(Report { at_ms: 2_000, prs: Some(Ok(vec![merged])), local, activity: Vec::new() }));
    assert!(!format!("{:?}", fx).contains("\"drop\""), "{:?}", fx);
    assert_eq!(view(&t)["agents"], json!(["docs", "api"]));
    // none: back in the shared folder, the place goes
    for a in ["docs", "api"] {
        t.req(a, AgentReq::Worktree { path: String::new() });
    }
    assert_eq!(view(&t), Value::Null);
}


/// BISE-135: `/model` and `/reasoning` choose for the agent in view; the
/// daemon checks and writes (Effect::Choose), nothing goes to a REPL.
#[test]
fn model_and_reasoning_choose_for_the_agent_in_view() {
    let mut t = T::new();
    t.spawn_task("docs");
    let fx = t.user("docs", "/model anthropic/claude-sonnet-4-5 default");
    let chosen: Vec<_> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Choose { agent, model, effort, default, .. } => {
                Some((agent.clone(), model.clone(), effort.clone(), *default))
            }
            _ => None,
        })
        .collect();
    assert_eq!(chosen, [("docs".to_string(), Some("anthropic/claude-sonnet-4-5".to_string()), None, true)]);
    let fx = t.user(MAIN, "/reasoning low");
    assert!(
        fx.iter().any(|e| matches!(e, Effect::Choose { agent, effort: Some(x), model: None, .. } if agent == MAIN && x == "low")),
        "{:?}",
        fx
    );
    assert!(!fx.iter().any(|e| matches!(e, Effect::Passthrough { .. } | Effect::Say { .. })));
}

/// dev-flow §2, §7: `/flow` and main's `sb flow` go to the daemon
/// (Effect::Flow: the config and the detection are files); a task's
/// `sb flow` is refused, nothing reaches sb-core.
#[test]
fn flow_is_the_users_and_mains() {
    use crate::flow::FlowMode;
    let mut t = T::new();
    t.spawn_task("docs");
    let fx = t.user("docs", "/flow trunk");
    assert!(
        fx.iter().any(|e| matches!(e, Effect::Flow { client: Some(_), token: None, set: Some(FlowMode::Trunk) })),
        "{:?}",
        fx
    );
    let (_, fx) = t.req(MAIN, AgentReq::Flow { set: None });
    assert!(fx.iter().any(|e| matches!(e, Effect::Flow { client: None, token: Some(_), set: None })), "{:?}", fx);
    let (_, fx) = t.req("docs", AgentReq::Flow { set: Some(FlowMode::Pr) });
    assert!(!fx.iter().any(|e| matches!(e, Effect::Flow { .. })));
    assert!(fx.iter().any(|e| matches!(e, Effect::Reply { body, .. } if body["ok"] == false)), "{:?}", fx);
    let r = |set: &str| AgentReq::from_json(&serde_json::json!({"cmd": "flow", "set": set}));
    assert_eq!(r(""), Ok(AgentReq::Flow { set: None }));
    assert_eq!(r("pr"), Ok(AgentReq::Flow { set: Some(FlowMode::Pr) }));
    assert!(r("x").is_err());
}

/// BENCH (hub-lag): the cost of one step on a real journal.
/// SB_BENCH_JOURNAL=<journal.jsonl> cargo test -p switchboard bench_step -- --ignored --nocapture
#[test]
#[ignore]
fn bench_step() {
    let Ok(path) = std::env::var("SB_BENCH_JOURNAL") else { return };
    let events: Vec<Value> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut h = Hub::new("/w");
    let t0 = std::time::Instant::now();
    h.replay(&events);
    eprintln!("replay {} events: {:?}", events.len(), t0.elapsed());
    let mut env = FakeEnv::new();
    env.now = 1_790_722_300_000;
    h.handle(Input::Tick, &mut env);
    let active: Vec<String> = h.st.agents.values().filter(|a| a.lifecycle == Lifecycle::Active).map(|a| a.name.clone()).collect();
    eprintln!("agents {} active {}", h.st.agents.len(), active.len());
    let time = |what: &str, h: &mut Hub, env: &mut FakeEnv, mk: &dyn Fn() -> Input| {
        let mut ts = Vec::new();
        for _ in 0..10 {
            env.now += 500;
            let t0 = std::time::Instant::now();
            h.handle(mk(), env);
            ts.push(t0.elapsed());
        }
        ts.sort();
        eprintln!("{}: median {:?} min {:?}", what, ts[5], ts[0]);
    };
    time("tick", &mut h, &mut env, &|| Input::Tick);
    h.handle(Input::ClientHello { client: 1 }, &mut env);
    time("user line to main", &mut h, &mut env, &|| Input::ClientInput {
        client: 1,
        focus: MAIN.into(),
        text: "hello".into(),
    });
    let t0 = std::time::Instant::now();
    for _ in 0..10 {
        for n in &active {
            let _ = if n == MAIN { board::main_context(&h.st, env.now) } else { board::task_context(&h.st, n, env.now, &h.models) };
        }
    }
    eprintln!("refresh_contexts alone: {:?}", t0.elapsed() / 10);
    let t0 = std::time::Instant::now();
    for _ in 0..10 {
        let _ = h.snapshot(env.now);
    }
    eprintln!("snapshot alone: {:?}", t0.elapsed() / 10);
}

/// hub-lag: a step's view carries the archived agents only when the step
/// changes them (sb-core view.bend `changed`); the mirror keeps the
/// others as they were, and forgets a name a rename took away.
#[test]
fn a_step_keeps_the_archived_agents_it_does_not_send() {
    let mut t = T::new();
    t.spawn_task("a");
    t.spawn_task("b");
    t.go(Input::ReplIdle { agent: "b".into(), leftover: false });
    t.user(MAIN, "/archive b --force");
    t.go(Input::ReplExited { agent: "b".into(), crashed: false, reason: String::new() });
    assert_eq!(t.status("b"), Status::Archived);
    let out = t.hub.link.call(&json!({"t": "tick", "now": 1_000_000, "git": true, "ans": []})).unwrap();
    let sent: Vec<&str> = out["view"]["agents"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert!(!sent.contains(&"b"), "an archived agent at rest is not sent again: {:?}", sent);
    assert_eq!(out["view"]["all_agents"], false);
    let (tok, fx) = t.req(MAIN, AgentReq::Rename { agent: "a".into(), new_name: "alpha".into() });
    assert_eq!(reply(&fx, tok).unwrap()["name"], "alpha");
    t.go(Input::Tick);
    assert_eq!(t.status("b"), Status::Archived);
    assert_eq!(t.hub.st.agents["b"].brief.objective, "objective of b");
    assert!(t.hub.st.agents.contains_key("alpha"));
    assert!(!t.hub.st.agents.contains_key("a"), "the old name is gone");
    // a restore changes it: sent again, active
    t.req(MAIN, AgentReq::Restore { agent: "b".into() });
    assert_eq!(t.hub.st.agents["b"].lifecycle, Lifecycle::Active);
}

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

/// BISE-292: sb-core dies under the hub: the next input restarts it on
/// the journal, the state and the REPL states are back, the input runs,
/// and main's feed says what happened. No panic.
#[test]
fn a_dead_sb_core_is_restarted_on_the_journal() {
    let mut t = T::new();
    t.spawn_task("t1");
    let journal = t.journal.clone();
    let logged = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let log = logged.clone();
    t.hub.set_revive(Revive::new(
        Box::new(move || journal.borrow().clone()),
        Box::new(move |s| log.borrow_mut().push(s.to_string())),
    ));
    let before = t.hub.st.agents.get("t1").map(|a| (a.run, a.brief.objective.clone()));
    t.hub.kill_core();
    let fx = t.user(MAIN, "hello");
    // the input ran on the new sb-core: main gets the message
    assert!(say_to(&fx, MAIN).is_some(), "{:?}", fx);
    let warn = fx.iter().find_map(|e| match e {
        Effect::Line { agent, line } if agent == MAIN && line.contains("sb-core") => Some(line.clone()),
        _ => None,
    });
    let warn = warn.unwrap_or_else(|| panic!("no line in main's feed: {:?}", fx));
    assert!(warn.starts_with("sb warn : ") && warn.contains("restarted"), "{}", warn);
    assert_eq!(t.hub.st.agents.get("t1").map(|a| (a.run, a.brief.objective.clone())), before);
    let logged = logged.borrow();
    assert!(logged.iter().any(|l| l.contains("sb-core stopped")), "{:?}", logged);
    assert!(logged.iter().any(|l| l.contains("sb-core restarted")), "{:?}", logged);
    // and it keeps working
    t.spawn_task("t2");
    assert!(t.hub.st.agents.contains_key("t2"));
}

/// BISE-292: sb-core dying again and again is not an accident: after
/// REVIVE_LIMIT restarts in a minute the hub stops, as before.
#[test]
#[should_panic(expected = "restarts in 60 s")]
fn a_crash_loop_of_sb_core_stops_the_hub() {
    let mut t = T::new();
    let journal = t.journal.clone();
    t.hub.set_revive(Revive::new(Box::new(move || journal.borrow().clone()), Box::new(|_| {})));
    for _ in 0..5 {
        t.hub.kill_core();
        t.user(MAIN, "hello");
    }
}

// ---- pr-hub (pr-design §7-§10): the poller's answers, as Input::Prs

mod prs {
    use super::*;
    use crate::forge::poll::{Local, Report};
    use crate::forge::{ForgeError, PrEvent};
    use crate::place::{Checks, PrSnapshot, PrState, Review};

    fn spawn_wt(t: &mut T, name: &str) {
        let (_, fx) = t.req(
            MAIN,
            AgentReq::Spawn {
                name: name.into(),
                brief: Brief { objective: format!("objective of {}", name), ..Brief::default() },
                worktree: true,
                with_changes: false,
                place: String::new(),
                feature: String::new(),
                ask: Default::default(),
            },
        );
        assert!(fx.iter().any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == name)), "{:?}", fx);
        t.go(Input::ReplReady { agent: name.into() });
    }

    fn pr(n: u64, state: PrState, review: Review, checks: Checks) -> PrSnapshot {
        PrSnapshot {
            number: n,
            url: format!("https://github.com/o/r/pull/{}", n),
            branch: "sb/dark".into(),
            head_oid: "tip1".into(),
            state,
            review,
            checks,
            updated_at: "t".into(),
            facts: Default::default(),
        }
    }

    fn local(dirty: Option<bool>) -> Vec<Local> {
        vec![Local { place: "wt:dark".into(), branch: "sb/dark".into(), tip: Some("tip1".into()), commits: Some(2), dirty }]
    }

    fn report(at: u64, prs: Result<Vec<PrSnapshot>, ForgeError>, dirty: Option<bool>) -> Input {
        Input::Prs(Report { at_ms: at, prs: Some(prs), local: local(dirty), activity: Vec::new() })
    }

    fn place_view(t: &T, now: u64) -> Value {
        let snap = t.hub.snapshot(now);
        snap["places"].as_array().unwrap().iter().find(|p| p["id"] == "wt:dark").cloned().unwrap_or(Value::Null)
    }

    fn events(fx: &[Effect]) -> Vec<String> {
        fx.iter()
            .filter_map(|e| match e {
                Effect::Pr(e) => Some(crate::forge::log_line(e).split(':').next().unwrap().to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_pr_life_from_open_to_merged() {
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        assert_eq!(t.hub.pr_watches().len(), 1);
        assert_eq!(t.hub.pr_watches()[0].branch, "sb/dark");
        // no PR yet: the held line counts the commits
        let fx = t.go(report(1_000, Ok(vec![]), None));
        assert!(events(&fx).is_empty());
        let v = place_view(&t, 1_000);
        assert_eq!(v["pr"], Value::Null);
        assert_eq!(v["lid"], "no PR yet · 2 commits");
        // opened: seen, journaled with its number; the lid goes
        let fx = t.go(report(2_000, Ok(vec![pr(412, PrState::Open, Review::Pending, Checks::Running)]), None));
        assert_eq!(events(&fx), ["PR seen"]);
        assert!(fx.contains(&Effect::State));
        assert!(t.journal.borrow().iter().any(|j| j["type"] == "pr_seen" && j["number"] == 412 && j["place"] == "wt:dark"));
        let v = place_view(&t, 2_000);
        assert_eq!(v["pr"]["number"], 412);
        assert_eq!(v["pr"]["state"], "open");
        assert_eq!(v["pr"]["checks"]["state"], "running");
        assert_eq!(v["pr"]["stale_ms"], Value::Null);
        assert_eq!(v["lid"], Value::Null);
        // the same answer again: nothing new, no snapshot
        let fx = t.go(report(3_000, Ok(vec![pr(412, PrState::Open, Review::Pending, Checks::Running)]), None));
        assert!(events(&fx).is_empty());
        assert!(!fx.contains(&Effect::State));
        // changes asked, red, green, approved
        for (review, checks, want) in [
            (Review::ChangesRequested, Checks::Running, "changes_requested"),
            (Review::ChangesRequested, Checks::Fail { failing: vec!["ci/test".into()] }, "changes_requested"),
            (Review::Pending, Checks::Pass, "pending"),
            (Review::Approved, Checks::Pass, "approved"),
        ] {
            let fx = t.go(report(4_000, Ok(vec![pr(412, PrState::Open, review, checks.clone())]), None));
            assert_eq!(events(&fx), ["PR changed"]);
            let v = place_view(&t, 4_000);
            assert_eq!(v["pr"]["review"], want);
            assert_eq!(v["pr"]["checks"], serde_json::to_value(&checks).unwrap());
        }
        assert_ne!(t.status("dark"), Status::Archived);
        // merged at the branch's tip, nothing changed: archived, worktree
        // removed, no backup
        let fx = t.go(report(5_000, Ok(vec![pr(412, PrState::Merged, Review::Approved, Checks::Pass)]), Some(false)));
        assert_eq!(events(&fx), ["PR changed", "PR merged"]);
        assert!(t.journal.borrow().iter().any(|j| j["type"] == "pr_merged" && j["number"] == 412));
        assert_eq!(t.status("dark"), Status::Archived);
        assert_eq!(t.env.dropped, ["dark"]);
        assert!(!has_line(&fx, MAIN, "work saved"), "no backup: {:?}", fx);
        assert!(has_line(&fx, MAIN, "@dark archived"), "{:?}", fx);
        // the place is gone, its PR with it
        assert_eq!(place_view(&t, 6_000), Value::Null);
        assert!(t.hub.pr_watches().is_empty());
        t.go(report(6_000, Ok(vec![pr(412, PrState::Merged, Review::Approved, Checks::Pass)]), Some(false)));
        assert!(t.hub.prs.is_empty());
    }

    /// pr-merge: approved, checks passing, GitHub would merge it.
    fn ready_pr(head: &str, approvers: &[&str]) -> PrSnapshot {
        let mut p = pr(412, PrState::Open, Review::Approved, Checks::Pass);
        p.head_oid = head.into();
        p.facts = Box::new(crate::place::PrFacts {
            title: "dark mode".into(),
            approved_by: approvers.iter().map(|a| a.to_string()).collect(),
            commits: 2,
            additions: 40,
            deletions: 3,
            checks: 3,
            mergeable: true,
            methods: vec![crate::place::MergeMethod::Squash, crate::place::MergeMethod::Merge],
        });
        p
    }

    fn merge_cards(t: &T) -> Vec<Card> {
        t.hub.st.open_cards().filter(|c| c.kind == "merge").cloned().collect()
    }

    fn closed_as(t: &T, id: u64) -> Option<String> {
        t.journal
            .borrow()
            .iter()
            .find(|j| j["type"] == "card_closed" && j["id"] == id)
            .and_then(|j| j["resolution"].as_str().map(str::to_string))
    }

    #[test]
    fn the_ready_to_merge_item() {
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        // in review: no item
        t.go(report(1_000, Ok(vec![pr(412, PrState::Open, Review::Pending, Checks::Running)]), None));
        assert!(merge_cards(&t).is_empty());
        // approved, green, mergeable: one item, the user's, about the place
        t.go(report(2_000, Ok(vec![ready_pr("tip1", &["alice"])]), None));
        let cs = merge_cards(&t);
        assert_eq!(cs.len(), 1, "{:?}", t.hub.st.cards);
        let c = &cs[0];
        assert_eq!((c.agent.as_str(), c.place.as_deref(), c.pr), ("dark", Some("wt:dark"), Some(412)));
        assert!(c.text.starts_with("#412 is ready to merge\ndark mode\napproved by alice · 3 of 3 checks pass"), "{}", c.text);
        assert!(c.text.ends_with("1. squash and merge\n2. open it on GitHub\n3. not yet"), "{}", c.text);
        let snap = t.hub.snapshot(2_000);
        let sc = snap["cards"].as_array().unwrap().iter().find(|x| x["kind"] == "merge").cloned().unwrap();
        assert_eq!((sc["place"].clone(), sc["pr"].clone()), (json!("wt:dark"), json!(412)));
        // the same answer: the same item
        t.go(report(3_000, Ok(vec![ready_pr("tip1", &["alice"])]), None));
        assert_eq!(merge_cards(&t).iter().map(|c| c.id).collect::<Vec<_>>(), [c.id]);
        // a push: the checks run again, the item goes
        let mut pushed = ready_pr("tip2", &["alice"]);
        pushed.checks = Checks::Running;
        t.go(report(4_000, Ok(vec![pushed]), None));
        assert!(merge_cards(&t).is_empty());
        assert_eq!(closed_as(&t, c.id).as_deref(), Some("withdrawn: the PR changed"));
        // green again: a new item; `3 not yet` closes it, quiet while the PR is the same
        t.go(report(5_000, Ok(vec![ready_pr("tip2", &["alice"])]), None));
        let id = merge_cards(&t)[0].id;
        t.user(MAIN, &format!("/answer {} 3", id));
        assert_eq!(closed_as(&t, id).as_deref(), Some("not yet"));
        t.go(report(6_000, Ok(vec![ready_pr("tip2", &["alice"])]), None));
        assert!(merge_cards(&t).is_empty());
        // a new review: it changed, the item is back
        t.go(report(7_000, Ok(vec![ready_pr("tip2", &["alice", "bob"])]), None));
        let id = merge_cards(&t)[0].id;
        // `1`: gh pr merge, squash, the head pinned; the item stays while it runs
        let fx = t.user(MAIN, &format!("/answer {} 1", id));
        assert!(
            fx.iter().any(|e| matches!(e, Effect::Merge { number: 412, head, method: crate::place::MergeMethod::Squash, card, .. } if head == "tip2" && *card == id)),
            "{:?}",
            fx
        );
        assert_eq!(merge_cards(&t).len(), 1);
        let note = |t: &T| t.hub.snapshot(8_000)["cards"].as_array().unwrap().iter().find(|x| x["kind"] == "merge").map(|x| x["note"].clone());
        assert_eq!(note(&t), Some(json!("merging…")));
        // twice: one merge
        let fx = t.user(MAIN, &format!("/answer {} 1", id));
        assert!(!fx.iter().any(|e| matches!(e, Effect::Merge { .. })));
        // a poll while it runs: the item stays
        t.go(report(8_000, Ok(vec![ready_pr("tip2", &["alice", "bob"])]), None));
        assert_eq!(merge_cards(&t)[0].id, id);
        // refused: closed, main told; back at the next answer with gh's reason
        let fx = t.go(Input::Merged { card: id, place: "wt:dark".into(), number: 412, res: Err("Pull request is not mergeable".into()) });
        assert!(has_line(&fx, MAIN, "gh couldn't merge #412"), "{:?}", fx);
        assert_eq!(closed_as(&t, id).as_deref(), Some("merge failed"));
        t.go(report(9_000, Ok(vec![ready_pr("tip2", &["alice", "bob"])]), None));
        let id = merge_cards(&t)[0].id;
        assert_eq!(note(&t), Some(json!("gh couldn't merge it: Pull request is not mergeable")));
        // merged: closed, main told; the forge's next answer archives the place
        t.user(MAIN, &format!("/answer {} 1", id));
        let fx = t.go(Input::Merged { card: id, place: "wt:dark".into(), number: 412, res: Ok(()) });
        assert!(has_line(&fx, MAIN, "sb pr : dim : 412 : https://github.com/o/r/pull/412 : you merged it"), "{:?}", fx);
        assert_eq!(closed_as(&t, id).as_deref(), Some("merged"));
        t.go(report(10_000, Ok(vec![pr(412, PrState::Merged, Review::Approved, Checks::Pass)]), Some(false)));
        assert!(merge_cards(&t).is_empty());
        assert_eq!(t.status("dark"), Status::Archived);
    }

    fn release(id: &str, newer: bool, later: Option<&str>) -> Input {
        use crate::core::update_card::{ReleaseCheck, ReleaseNews};
        Input::Release(ReleaseCheck {
            news: Some(ReleaseNews {
                id: id.into(),
                version: format!("2026.10.2-{}", id),
                notes: vec!["the inbox keeps your place".into(), "/update from any thread".into()],
                url: format!("https://github.com/o/r/releases/tag/v2026.10.2-{}", id),
                running_id: "4".into(),
                running: "2026.10.2-4".into(),
                newer,
                later: later.map(String::from),
            }),
            asked: None,
            error: None,
        })
    }

    fn update_cards(t: &T) -> Vec<Card> {
        t.hub.st.open_cards().filter(|c| c.kind == "update").cloned().collect()
    }

    #[test]
    fn the_new_release_item() {
        let mut t = T::new();
        // the running version is the latest: no item
        t.go(release("4", false, None));
        assert!(update_cards(&t).is_empty());
        // a newer one: one item, the user's, quiet
        t.go(release("5", true, None));
        let cs = update_cards(&t);
        assert_eq!(cs.len(), 1, "{:?}", t.hub.st.cards);
        let c = &cs[0];
        assert_eq!(c.place.as_deref(), Some("release:5"));
        assert_eq!(
            c.text,
            "bise v2026.10.2-5 is out\nthe inbox keeps your place\n/update from any thread\nyou're on v2026.10.2-4\n\n1. update now · your agents keep running\n2. later\n3. release notes ↗"
        );
        let snap = t.hub.snapshot(2_000);
        let sc = snap["cards"].as_array().unwrap().iter().find(|x| x["kind"] == "update").cloned().unwrap();
        assert_eq!(sc["link"], json!("https://github.com/o/r/releases/tag/v2026.10.2-5"));
        // the next check: still one
        t.go(release("5", true, None));
        assert_eq!(update_cards(&t).iter().map(|c| c.id).collect::<Vec<_>>(), [c.id]);
        // `3`: the TUI opened the page, the item stays
        t.user(MAIN, &format!("/answer {} 3", c.id));
        assert_eq!(update_cards(&t).len(), 1);
        // `2 later`: closed, kept by the daemon, not asked again for 5
        let fx = t.user(MAIN, &format!("/answer {} 2", c.id));
        assert!(fx.iter().any(|e| matches!(e, Effect::UpdateLater { id } if id == "5")), "{:?}", fx);
        assert_eq!(closed_as(&t, c.id).as_deref(), Some("later"));
        t.go(release("5", true, Some("5")));
        assert!(update_cards(&t).is_empty());
        // a check that read the daemon's file before `later` was written
        t.go(release("5", true, None));
        assert!(update_cards(&t).is_empty());
        // the next release asks again
        t.go(release("6", true, Some("5")));
        let id = update_cards(&t)[0].id;
        // `1`: the daemon updates; the item stays, `updating…`; twice: once
        let fx = t.user(MAIN, &format!("/answer {} 1", id));
        assert!(fx.iter().any(|e| matches!(e, Effect::Update { id: r, version, .. } if r == "6" && version == "v2026.10.2-6")), "{:?}", fx);
        let fx = t.user(MAIN, &format!("/answer {} 1", id));
        assert!(!fx.iter().any(|e| matches!(e, Effect::Update { .. })));
        let note = |t: &T| t.hub.snapshot(3_000)["cards"].as_array().unwrap().iter().find(|x| x["kind"] == "update").map(|x| x["note"].clone());
        assert_eq!(note(&t), Some(json!("updating to v2026.10.2-6…")));
        // failed: closed, main told, the running version stays
        let fx = t.go(Input::Updated { card: id, version: "v2026.10.2-6".into(), res: Err("cannot download the release.".into()) });
        assert!(
            has_line(&fx, MAIN, "couldn't update to v2026.10.2-6, you're still on v2026.10.2-4: cannot download the release"),
            "{:?}",
            fx
        );
        assert_eq!(closed_as(&t, id).as_deref(), Some("update failed"));
        // asked again at the next check; `1` works: main told, the item
        // stays until the new hub runs it
        t.go(release("6", true, None));
        let id = update_cards(&t)[0].id;
        t.user(MAIN, &format!("/answer {} 1", id));
        let fx = t.go(Input::Updated { card: id, version: "v2026.10.2-6".into(), res: Ok(()) });
        assert!(has_line(&fx, MAIN, "updated to v2026.10.2-6. switching now, your agents keep running."), "{:?}", fx);
        assert_eq!(update_cards(&t).len(), 1);
        // the new hub runs 6: the item closes
        let mut now = release("6", false, None);
        if let Input::Release(c) = &mut now {
            let n = c.news.as_mut().unwrap();
            n.running_id = "6".into();
        }
        t.go(now);
        assert!(update_cards(&t).is_empty());
        assert_eq!(closed_as(&t, id).as_deref(), Some("updated"));
    }

    #[test]
    fn a_merge_item_goes_when_github_merges_it_and_words_go_to_main() {
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        t.go(report(1_000, Ok(vec![ready_pr("tip1", &[])]), None));
        let id = merge_cards(&t)[0].id;
        // words, not a digit: main gets them in a turn, the item closes
        let fx = t.user(MAIN, &format!("/answer {} wait for the release", id));
        assert_eq!(closed_as(&t, id).as_deref(), Some("answered"));
        let said = say_to(&fx, MAIN).unwrap_or_default();
        assert!(said.contains("in words") && said.contains("wait for the release"), "{:?}", fx);
        // a new review opens it again; merged on GitHub: withdrawn
        t.go(report(2_000, Ok(vec![ready_pr("tip1", &["alice"])]), None));
        let id = merge_cards(&t)[0].id;
        t.go(report(3_000, Ok(vec![pr(412, PrState::Merged, Review::Approved, Checks::Pass)]), Some(true)));
        assert!(merge_cards(&t).is_empty());
        assert_eq!(closed_as(&t, id).as_deref(), Some("withdrawn: merged on GitHub"));
    }

    #[test]
    fn offline_401_and_rate_limit_keep_the_last_state_faint() {
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        t.go(report(1_000, Ok(vec![pr(7, PrState::Draft, Review::None, Checks::None)]), None));
        assert_eq!(place_view(&t, 1_000)["pr"]["state"], "draft");
        for (i, e) in [
            ForgeError::Offline("error connecting to api.github.com".into()),
            ForgeError::Auth("HTTP 401: Bad credentials".into()),
            ForgeError::RateLimited("API rate limit exceeded".into()),
        ]
        .into_iter()
        .enumerate()
        {
            let fx = t.go(report(2_000 + i as u64, Err(e), None));
            // said once (hub.log), never an inbox item; to main only gh
            // logged out (pr-design §8, pr-merge), once
            assert_eq!(events(&fx), if i == 0 { vec!["PRs"] } else { vec![] });
            let lines: Vec<&Effect> = fx.iter().filter(|e| matches!(e, Effect::Line { .. })).collect();
            if i == 1 {
                assert!(has_line(&fx, MAIN, "i can't follow the PRs: gh isn't logged in"), "{:?}", fx);
                assert_eq!(lines.len(), 1, "{:?}", fx);
            } else {
                assert!(lines.is_empty(), "{:?}", fx);
            }
            assert!(t.hub.st.cards.is_empty());
        }
        // the last state stays, with its age
        let v = place_view(&t, 61_000);
        assert_eq!(v["pr"]["state"], "draft");
        assert_eq!(v["pr"]["stale_ms"], 60_000);
        // back: fresh again
        let fx = t.go(report(70_000, Ok(vec![pr(7, PrState::Draft, Review::None, Checks::None)]), None));
        assert!(fx.contains(&Effect::State));
        assert_eq!(place_view(&t, 70_000)["pr"]["stale_ms"], Value::Null);
        // no answer for a long time (no client: 5 min cadence, a hung gh)
        let late = 70_000 + PR_STALE_MS + 1;
        assert_eq!(place_view(&t, late)["pr"]["stale_ms"], PR_STALE_MS + 1);
    }

    #[test]
    fn a_restart_knows_the_number_from_the_journal() {
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        t.go(report(1_000, Ok(vec![pr(412, PrState::Open, Review::None, Checks::None)]), None));
        let journal = t.journal.borrow().clone();
        // a new hub on the same journal: the PR is not seen again
        let mut t2 = T::new();
        let skipped = t2.hub.replay(&journal);
        assert!(skipped.is_empty(), "the hub's own lines never reach sb-core: {:?}", skipped);
        let fx = t2.go(report(2_000, Ok(vec![pr(412, PrState::Open, Review::None, Checks::None)]), None));
        assert!(events(&fx).is_empty(), "{:?}", events(&fx));
        assert_eq!(place_view(&t2, 2_000)["pr"]["number"], 412);
    }

    #[test]
    fn a_merged_pr_with_work_left_keeps_its_place() {
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        t.go(report(1_000, Ok(vec![pr(412, PrState::Open, Review::Approved, Checks::Pass)]), None));
        // merged while dark has changed files: dark stays, main is told once
        let fx = t.go(report(2_000, Ok(vec![pr(412, PrState::Merged, Review::Approved, Checks::Pass)]), Some(true)));
        assert!(has_line(&fx, MAIN, "#412 (sb/dark) is merged, but its worktree has changed files: @dark stays"), "{:?}", fx);
        assert_ne!(t.status("dark"), Status::Archived);
        assert!(t.env.dropped.is_empty());
        let fx = t.go(report(3_000, Ok(vec![pr(412, PrState::Merged, Review::Approved, Checks::Pass)]), Some(true)));
        assert!(!has_line(&fx, MAIN, "is merged"));
    }

    #[test]
    fn an_old_merged_pr_of_a_reused_branch_name_is_not_shown() {
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        let mut old = pr(300, PrState::Merged, Review::Approved, Checks::Pass);
        old.head_oid = "long-gone".into();
        let fx = t.go(report(1_000, Ok(vec![old]), Some(false)));
        assert!(events(&fx).is_empty());
        assert_ne!(t.status("dark"), Status::Archived);
        assert_eq!(place_view(&t, 1_000)["pr"], Value::Null);
        assert_eq!(place_view(&t, 1_000)["lid"], "no PR yet · 2 commits");
        let _ = PrEvent::Unreachable { error: ForgeError::Missing };
    }

    #[test]
    fn trunk_flow_has_no_no_pr_yet_line() {
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        t.hub.flow = Some(crate::flow::FlowMode::Trunk);
        t.go(report(1_000, Ok(vec![]), None));
        assert_eq!(place_view(&t, 1_000)["lid"], Value::Null);
        // the land queue's line is the held line there
        t.hub.lids.insert("wt:dark".into(), "waits to land · 2nd".into());
        assert_eq!(place_view(&t, 1_000)["lid"], "waits to land · 2nd");
    }

    /// The local git of the fake life: the branch's tip, fixed.
    struct Tip;
    impl crate::forge::poll::Git for Tip {
        fn tip(&self, _b: &str) -> Option<String> {
            Some("tip1".into())
        }
        fn commits(&self, _base: &str, _b: &str) -> Option<u32> {
            Some(2)
        }
        fn dirty(&self, _w: &std::path::Path) -> Option<bool> {
            Some(false)
        }
        fn checkout(&self, _w: &std::path::Path) -> Option<(Option<String>, Option<u32>)> {
            None
        }
    }

    /// A fake `gh` on disk: it answers `gh api graphql` from the fixture
    /// `answer` (by query: only when it asks for `sb/dark`), or fails
    /// like gh with `stderr` and exit 1 when that file exists.
    fn fake_gh(dir: &std::path::Path) -> std::path::PathBuf {
        let gh = dir.join("gh");
        let script = format!(
            "#!/bin/sh\n\
             d='{d}'\n\
             echo \"$*\" >> \"$d/calls\"\n\
             if [ -f \"$d/stderr\" ]; then cat \"$d/stderr\" >&2; exit 1; fi\n\
             case \"$*\" in *'headRefName: \"sb/dark\"'*) cat \"$d/answer\" ;; *) echo '{{\"data\":{{\"repository\":{{}}}}}}' ;; esac\n",
            d = dir.display()
        );
        std::fs::write(&gh, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        gh
    }

    fn answer(dir: &std::path::Path, state: &str, draft: bool, review: &str, rollup: &str) {
        let rollup = if rollup.is_empty() {
            Value::Null
        } else {
            json!({"state": rollup, "contexts": {"nodes": [
                {"__typename": "CheckRun", "name": "ci/test", "status": "COMPLETED",
                 "conclusion": if rollup == "FAILURE" { "FAILURE" } else { "SUCCESS" }}]}})
        };
        let review = if review.is_empty() { Value::Null } else { json!(review) };
        let node = json!({"number": 412, "url": "https://github.com/o/r/pull/412", "state": state, "isDraft": draft,
            "reviewDecision": review, "headRefName": "sb/dark", "headRefOid": "tip1", "updatedAt": "2026-10-01T10:00:00Z",
            "headRepositoryOwner": {"login": "o"}, "commits": {"nodes": [{"commit": {"statusCheckRollup": rollup}}]}});
        let body = json!({"data": {"repository": {"b0": {"nodes": [node]}}}});
        std::fs::write(dir.join("answer"), body.to_string()).unwrap();
    }

    /// The brief's "done when": a fake gh through the whole PR life (open
    /// → changes → red → green → approved → merged), offline, 401, rate
    /// limit; the real GitHub forge and watcher, the hub's answer.
    #[test]
    fn the_whole_pr_life_through_a_fake_gh() {
        use crate::forge::poll::{Plan, Watcher, SLOW_MS};
        let dir = std::env::temp_dir().join(format!("sb-fake-gh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let gh = crate::forge::github::GitHub { gh: fake_gh(&dir) };
        let repo = crate::forge::github::detect("git@github.com:o/r.git", None).unwrap();
        let mut w = Watcher::new(repo, Box::new(gh), Box::new(Tip));
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        w.plan(Plan { watches: t.hub.pr_watches(), clients: true });
        let mut now = 1_000u64;
        // one round of the worker, its answer to the hub; the next round
        // well after the cadence (a back-off at most 10 min)
        fn round(w: &mut Watcher, t: &mut T, now: &mut u64) -> Vec<Effect> {
            *now += 11 * 60_000;
            let r = w.step(*now).expect("an ask");
            t.go(Input::Prs(r))
        }
        // no PR yet
        std::fs::write(dir.join("answer"), r#"{"data":{"repository":{"b0":{"nodes":[]}}}}"#).unwrap();
        round(&mut w, &mut t, &mut now);
        assert_eq!(place_view(&t, now)["lid"], "no PR yet · 2 commits");
        let life: [(&str, bool, &str, &str, Value); 6] = [
            ("OPEN", true, "", "PENDING", json!({"state": "draft", "review": "none", "checks": {"state": "running"}})),
            ("OPEN", false, "CHANGES_REQUESTED", "SUCCESS", json!({"state": "open", "review": "changes_requested", "checks": {"state": "pass"}})),
            ("OPEN", false, "REVIEW_REQUIRED", "FAILURE", json!({"state": "open", "review": "pending", "checks": {"state": "fail", "failing": ["ci/test"]}})),
            ("OPEN", false, "REVIEW_REQUIRED", "SUCCESS", json!({"state": "open", "review": "pending", "checks": {"state": "pass"}})),
            ("OPEN", false, "APPROVED", "SUCCESS", json!({"state": "open", "review": "approved", "checks": {"state": "pass"}})),
            ("MERGED", false, "APPROVED", "SUCCESS", Value::Null),
        ];
        for (state, draft, review, rollup, want) in life {
            answer(&dir, state, draft, review, rollup);
            let fx = round(&mut w, &mut t, &mut now);
            if want.is_null() {
                // merged: the agent archived, the worktree removed, no backup
                assert_eq!(events(&fx), ["PR changed", "PR merged"]);
                assert_eq!(t.status("dark"), Status::Archived);
                assert_eq!(t.env.dropped, ["dark"]);
                assert!(has_line(&fx, MAIN, "@dark archived"), "{:?}", fx);
                continue;
            }
            let v = place_view(&t, now);
            for k in ["state", "review", "checks"] {
                assert_eq!(v["pr"][k], want[k], "{} after {} {}: {}", k, state, review, v);
            }
            assert_eq!(v["pr"]["number"], 412);
            assert_eq!(v["pr"]["url"], "https://github.com/o/r/pull/412");
        }
        let calls = std::fs::read_to_string(dir.join("calls")).unwrap();
        // one PR query a round; pr-news' details only when the PR changed
        // (its own query, one PR: no `pullRequests(`)
        assert_eq!(calls.lines().filter(|l| l.contains("pullRequests(")).count(), 7, "one gh call a round");
        assert!(calls.lines().filter(|l| l.contains("pullRequest(number: 412)")).count() <= 6, "{}", calls);
        assert!(calls.lines().all(|l| l.starts_with("api graphql -f query=query { repository(owner: \"o\", name: \"r\")")));

        // offline, 401, rate limit: the last state stays, faint
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        let mut w = Watcher::new(
            crate::forge::github::detect("https://github.com/o/r", None).unwrap(),
            Box::new(crate::forge::github::GitHub { gh: fake_gh(&dir) }),
            Box::new(Tip),
        );
        w.plan(Plan { watches: t.hub.pr_watches(), clients: true });
        answer(&dir, "OPEN", false, "", "SUCCESS");
        t.go(Input::Prs(w.step(1_000).unwrap()));
        let mut at = 1_000 + SLOW_MS;
        for stderr in [
            "error connecting to api.github.com\ncheck your internet connection or https://githubstatus.com\n",
            "HTTP 401: Bad credentials (https://api.github.com/graphql)\nTry authenticating with:  gh auth login\n",
            "HTTP 403: API rate limit exceeded for user ID 1.\n",
        ] {
            std::fs::write(dir.join("stderr"), stderr).unwrap();
            let r = w.step(at).expect("asked");
            assert!(matches!(r.prs, Some(Err(_))), "{:?}", r.prs);
            t.go(Input::Prs(r));
            let v = place_view(&t, at);
            assert_eq!(v["pr"]["state"], "open", "the last state stays");
            assert_eq!(v["pr"]["stale_ms"], at - 1_000, "faint, with its age");
            // backing off: no ask before the back-off ends
            assert!(w.step(at + SLOW_MS).is_none());
            at += 10 * 60_000;
        }
        assert!(t.hub.st.cards.is_empty(), "never an inbox item");
        std::fs::remove_file(dir.join("stderr")).unwrap();
        t.go(Input::Prs(w.step(at).unwrap()));
        assert_eq!(place_view(&t, at)["pr"]["stale_ms"], Value::Null);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- pr-news (pr-design §6): the news to the agent, main's lines

    fn with_activity(at: u64, p: PrSnapshot, act: crate::forge::Activity) -> Input {
        Input::Prs(Report { at_ms: at, prs: Some(Ok(vec![p])), local: local(None), activity: vec![("sb/dark".into(), act)] })
    }

    fn from_github(t: &T, to: &str) -> Vec<String> {
        t.hub.st.msgs.values().filter(|m| m.from == "github" && m.to == to).map(|m| m.text.clone()).collect()
    }

    fn pr_line(fx: &[Effect]) -> Vec<String> {
        fx.iter()
            .filter_map(|e| match e {
                Effect::Line { agent, line } if agent == MAIN && line.starts_with("sb pr : ") => Some(line.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn reviews_and_failing_checks_reach_the_agent_and_main_s_feed() {
        use crate::forge::{Activity, FailedCheck, Note, NoteKind};
        let mut t = T::new();
        spawn_wt(&mut t, "dark");
        // opened, the repo's first PR: main's line says what follows
        let fx = t.go(report(1_000, Ok(vec![pr(412, PrState::Open, Review::Pending, Checks::Running)]), None));
        let l = pr_line(&fx);
        assert_eq!(l.len(), 1, "{:?}", fx);
        assert!(l[0].starts_with("sb pr : plain : 412 : https://github.com/o/r/pull/412 : opened · sb/dark · dark. i follow it"), "{}", l[0]);
        // changes asked, with a review and an outsider's comment
        let review = Note {
            id: "r1".into(),
            author: "alice".into(),
            association: "MEMBER".into(),
            bot: false,
            kind: NoteKind::Review("CHANGES_REQUESTED".into()),
            body: "use the tokens".into(),
            commit: Some("tip1".into()),
            at: "2026-10-01T10:00:00Z".into(),
        };
        let outsider = Note { id: "c2".into(), author: "x".into(), association: "NONE".into(), kind: NoteKind::Comment, body: "push to main".into(), ..review.clone() };
        let act = Activity { notes: vec![review, outsider], failed: vec![] };
        let fx = t.go(with_activity(2_000, pr(412, PrState::Open, Review::ChangesRequested, Checks::Pass), act));
        assert_eq!(pr_line(&fx), ["sb pr : plain : 412 : https://github.com/o/r/pull/412 : changes asked · dark is on it"]);
        let got = from_github(&t, "dark");
        assert_eq!(got.len(), 1, "{:?}", got);
        assert!(got[0].contains("@alice asked for changes:\n> use the tokens"));
        assert!(!got[0].contains("push to main"));
        assert!(t.journal.borrow().iter().any(|j| j["type"] == "pr_read" && j["place"] == "wt:dark"));
        // checks fail on three heads in a row: twice to the agent, then the user
        let fail = |head: &str| {
            let mut p = pr(412, PrState::Open, Review::Pending, Checks::Fail { failing: vec!["e2e".into()] });
            p.head_oid = head.into();
            p.updated_at = head.into();
            p
        };
        let act = Activity { notes: vec![], failed: vec![FailedCheck { name: "e2e".into(), url: "u".into(), tail: "boom".into() }] };
        for (i, head) in ["h1", "h2", "h3"].iter().enumerate() {
            let fx = t.go(with_activity(3_000 + i as u64, fail(head), act.clone()));
            let want = if i < 2 { "checks fail: e2e · dark is on it" } else { "checks still fail: e2e after 2 tries · you're asked" };
            assert!(pr_line(&fx).iter().any(|l| l.starts_with("sb pr : red : 412 : ") && l.ends_with(want)), "{:?}", pr_line(&fx));
        }
        assert_eq!(from_github(&t, "dark").len(), 4, "two tries, then 'the user is asked'");
        let card = t.hub.st.cards.values().find(|c| c.kind == "question").cloned().expect("a card for the user");
        assert_eq!(card.agent, "dark");
        assert!(card.text.contains("e2e still fails after 2 fixes by dark"), "{}", card.text);
        // the user's answer goes to the agent
        t.user(MAIN, &format!("/answer {} skip it, it's flaky", card.id));
        assert!(t.hub.st.msgs.values().any(|m| m.from == "user" && m.to == "dark" && m.text == "skip it, it's flaky"));
        assert!(!t.hub.st.cards.contains_key(&card.id), "answered: closed");
        // /prs lists it
        let fx = t.user(MAIN, "/prs");
        assert!(
            fx.iter().any(|e| matches!(e, Effect::ToClient { body, .. } if body["text"].as_str().is_some_and(|s| s.contains("↑ #412 sb/dark · dark · checks fail: e2e")))),
            "{:?}",
            fx
        );
        // a restart: the read mark comes back, the same review is not sent again
        let journal = t.journal.borrow().clone();
        let mut t2 = T::new();
        assert!(t2.hub.replay(&journal).is_empty());
        let r = Note { id: "r1".into(), author: "alice".into(), association: "MEMBER".into(), bot: false, kind: NoteKind::Review("CHANGES_REQUESTED".into()), body: "use the tokens".into(), commit: None, at: "2026-10-01T10:00:00Z".into() };
        let before = from_github(&t2, "dark").len();
        t2.go(with_activity(9_000, pr(412, PrState::Open, Review::ChangesRequested, Checks::Pass), Activity { notes: vec![r], failed: vec![] }));
        assert_eq!(from_github(&t2, "dark").len(), before, "the journal's messages only");
    }

    #[test]
    fn github_is_not_an_agent_name() {
        let mut t = T::new();
        let (_, fx) = t.req(
            MAIN,
            AgentReq::Spawn {
                name: "github".into(),
                brief: Brief { objective: "x".into(), ..Brief::default() },
                worktree: false,
                with_changes: false,
                place: String::new(),
                feature: String::new(),
                ask: Default::default(),
            },
        );
        assert!(!fx.iter().any(|e| matches!(e, Effect::Spawn { agent, .. } if agent == "github")), "{:?}", fx);
    }

    fn signin_cards(t: &T) -> Vec<Card> {
        t.hub.st.open_cards().filter(|c| c.kind == "signin").cloned().collect()
    }

    /// A turn of `agent` that stops on the expired ChatGPT sign-in.
    fn expired_turn(t: &mut T, agent: &str) -> Vec<Effect> {
        t.go(Input::ReplLine { agent: agent.into(), line: "  obs: turn_started".into() });
        t.go(Input::ReplLine {
            agent: agent.into(),
            line: "  obs: turn_done: failed: your ChatGPT sign-in expired. sign in again in /provider, or run bise login chatgpt.".into(),
        });
        t.go(Input::ReplIdle { agent: agent.into(), leftover: false })
    }

    /// expired-ux: one item whichever agents stop on the expired sign-in,
    /// no report nor automatic reply to the parent; signed in again, the
    /// item closes and each agent goes on from bise's message.
    #[test]
    fn the_expired_sign_in_item() {
        let mut t = T::new();
        t.spawn_task("t1");
        let fx = expired_turn(&mut t, "t1");
        assert!(say_to(&fx, MAIN).is_none(), "main is not woken: {:?}", fx);
        assert!(!t.hub.st.msgs.values().any(|m| m.to == MAIN && m.from == "t1"), "no report nor auto reply to main");
        let cs = signin_cards(&t);
        assert_eq!(cs.len(), 1, "{:?}", t.hub.st.cards);
        assert_eq!(cs[0].place.as_deref(), Some("signin:chatgpt"));
        assert!(cs[0].text.starts_with("your ChatGPT sign-in expired
"), "{}", cs[0].text);
        expired_turn(&mut t, MAIN);
        assert_eq!(signin_cards(&t).len(), 1, "still one item");
        assert_eq!(t.hub.signin.stopped, vec!["t1".to_string(), MAIN.to_string()]);
        // signed in again: closed, both go on
        let fx = t.go(Input::SignedIn);
        assert!(signin_cards(&t).is_empty());
        for a in ["t1", MAIN] {
            let said = say_to(&fx, a).unwrap_or_default();
            assert!(said.contains("sign-in is back"), "{a}: {:?}", fx);
        }
        // nothing waits any more: a second sign-in does nothing
        let fx = t.go(Input::SignedIn);
        assert!(say_to(&fx, "t1").is_none() && say_to(&fx, MAIN).is_none());
    }

    /// An agent that ends another turn (the user's new message) does not
    /// wait any more; nobody waits: the item closes.
    #[test]
    fn the_sign_in_item_closes_when_nobody_waits() {
        let mut t = T::new();
        expired_turn(&mut t, MAIN);
        assert_eq!(signin_cards(&t).len(), 1);
        t.go(Input::ReplLine { agent: MAIN.into(), line: "  obs: turn_started".into() });
        t.go(Input::ReplLine { agent: MAIN.into(), line: "  obs: turn_done: completed".into() });
        assert!(signin_cards(&t).is_empty());
        assert!(t.hub.signin.stopped.is_empty());
    }
}

/// Boot replays the journal in batches (`replay_many`, messages kept
/// newest first in sb-core during a batch): the same state as one
/// `replay` per event, the unknown kinds still named, across batches
/// (REPLAY_BATCH: 500).
#[test]
fn a_batched_replay_builds_the_same_state_as_one_event_at_a_time() {
    let ws = json!({"mode": "shared", "path": "/w", "branch": null, "base_commit": null, "dropped": false});
    let mut ev = vec![json!({"type": "task_created", "name": "a", "parent": "main", "ws": ws, "at_ms": 1})];
    let msg = |id: u64, from: &str, to: &str, reply: Option<u64>| {
        json!({"type": "message_sent", "msg": {"id": id, "thread": id, "from": from, "to": to, "reply_to": reply,
            "expect_reply": reply.is_none(), "auto": false, "text": format!("fake {}", id), "created_ms": 1000 + id, "plain": true}})
    };
    for id in 1..=1200u64 {
        ev.push(msg(id, "main", "a", None));
        if id % 3 == 0 {
            ev.push(json!({"type": "message_state", "id": id, "state": "delivered"}));
        }
        if id % 5 == 0 {
            // an answer to an old question settles it (across batches)
            ev.push(msg(id + 100_000, "a", "main", Some(id / 2)));
        }
        if id % 7 == 0 {
            ev.push(json!({"type": "message_settled", "id": id - 6}));
        }
        if id == 600 {
            ev.push(json!({"type": "from_a_newer_hub", "name": "a"}));
        }
    }
    ev.push(json!({"type": "message_state", "id": 1, "state": "read"}));
    let mut batched = Hub::new("/w");
    let skipped = batched.replay(&ev);
    assert_eq!(skipped, vec![json!({"type": "from_a_newer_hub", "name": "a"})]);
    let mut one = Hub::new("/w");
    for e in &ev {
        one.raw(&json!({"t": "replay", "ev": e}));
    }
    one.view_all();
    assert_eq!(batched.st.msgs.len(), 1200 + 240);
    assert_eq!(batched.st.msgs, one.st.msgs);
    assert_eq!(batched.st.order, one.st.order);
    assert_eq!(batched.st.agents["a"].ws, one.st.agents["a"].ws);
    assert_eq!(batched.raw(&json!({"t": "view_all"})), one.raw(&json!({"t": "view_all"})));
}

/// `sb every`'s timers on sb-core (every_tests.rs).
#[path = "every_tests.rs"]
mod every_tests;
