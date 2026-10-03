//! The sign-ins on the key step, `/provider` and `/models`
//! (docs/subscriptions-design.md, the designer's final words): every
//! state drawn, with a fake sign-in (never a real account), on a temp
//! HOME. No token is ever on screen.

use super::signin::*;
use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

// ---- the fake sign-in (this thread's script) ----

thread_local! {
    static STATE: RefCell<PlanState> = const { RefCell::new(PlanState::NotSetUp) };
    static SEEN: Cell<Detected> = const { Cell::new(Detected { codex_chatgpt: false, claude_plan: false }) };
    static POLLS: RefCell<VecDeque<Poll>> = const { RefCell::new(VecDeque::new()) };
    static CANCELLED: Cell<bool> = const { Cell::new(false) };
    static OPENED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static STARTED: RefCell<Vec<Kind>> = const { RefCell::new(Vec::new()) };
}

/// The token the fake hands the check: it must never be drawn.
const TOKEN: &str = "tok-SECRET-0123456789";
const URL: &str = "http://127.0.0.1:1455/authorize?state=fake";

struct FakeFlow;

impl Flow for FakeFlow {
    fn url(&self) -> &str {
        URL
    }
    fn poll(&mut self) -> Poll {
        let p = POLLS.with(|q| q.borrow_mut().pop_front()).unwrap_or(Poll::Waiting);
        // signed in: auth.json would say so now
        if let Poll::Done(Some(a)) = &p {
            STATE.with(|s| *s.borrow_mut() = PlanState::SignedIn(a.clone()));
        }
        p
    }
    fn cancel(&mut self) {
        CANCELLED.with(|c| c.set(true));
    }
}

fn fake() -> Logins {
    Logins {
        start: |k, _| {
            STARTED.with(|s| s.borrow_mut().push(k));
            Ok(Box::new(FakeFlow))
        },
        state: |_| STATE.with(|s| s.borrow().clone()),
        detect: |_, _| SEEN.with(|s| s.get()),
        sign_out: |_| Ok(false),
        token: |_| match STATE.with(|s| s.borrow().clone()) {
            PlanState::SignedIn(_) => Ok(TOKEN.into()),
            _ => Err("signed out".into()),
        },
        models: |_| {
            vec![
                PlanModel { slug: "gpt-6.1-sol".into(), name: "GPT-6.1 Sol".into() },
                PlanModel { slug: "gpt-6.1-luna".into(), name: "GPT-6.1 Luna".into() },
            ]
        },
        fetch_models: |_| {},
        open: |u| {
            OPENED.with(|o| o.borrow_mut().push(u.to_string()));
            true
        },
    }
}

fn script(state: PlanState, seen: Detected, polls: Vec<Poll>) {
    STATE.with(|s| *s.borrow_mut() = state);
    SEEN.with(|s| s.set(seen));
    POLLS.with(|q| *q.borrow_mut() = polls.into());
    CANCELLED.with(|c| c.set(false));
    OPENED.with(|o| o.borrow_mut().clear());
    STARTED.with(|s| s.borrow_mut().clear());
}

fn me() -> Account {
    Account { email: "you@example.com".into(), plan: Some("Plus".into()) }
}

// ---- the setup: a temp HOME whose catalog has the plan's provider ----

/// The `chatgpt` provider as subs-auth's catalog will have it (until it
/// lands, config.toml adds it).
const CONFIG: &str = r#"
[providers.chatgpt]
name = "ChatGPT"
api = "openai-responses"
base_url = "https://api.openai.com/v1"
key_env = ""

[models."chatgpt/gpt-6.1-sol"]
context = 400000
"#;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-signin-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join(".bise")).unwrap();
    std::fs::write(d.join(".bise/config.toml"), CONFIG).unwrap();
    d
}

fn env_of(m: HashMap<&'static str, String>) -> impl Fn(&str) -> Option<String> {
    move |k: &str| m.get(k).cloned().filter(|v| !v.is_empty())
}

fn home_env(h: &std::path::Path, more: &[(&'static str, &str)]) -> impl Fn(&str) -> Option<String> {
    let mut m = HashMap::from([("HOME", h.to_string_lossy().to_string()), ("BISE_HOME", h.join(".bise").to_string_lossy().to_string())]);
    for (k, v) in more {
        m.insert(*k, v.to_string());
    }
    env_of(m)
}

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn screen_at(o: &Onb, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| draw(f, o, 10)).unwrap();
    let b = t.backend().buffer().clone();
    let sc = (0..h)
        .map(|y| (0..w).map(|x| b[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!sc.contains("SECRET"), "a token on screen:\n{sc}");
    sc
}

fn screen(o: &Onb) -> String {
    screen_at(o, 150, 36)
}

/// The screen's text, rows trimmed and joined: phrases across wraps.
fn flat(sc: &str) -> String {
    sc.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
}

fn has(sc: &str, words: &[&str]) {
    for w in words {
        assert!(sc.contains(w) || flat(sc).contains(w), "missing {w:?}\n{sc}");
    }
}

/// The check: the plan's token passes; "bad" fails as a wrong key,
/// "limit" as the plan's limit.
fn fake_check(c: &crate::keycheck::Call, _: Option<String>) -> Result<(), crate::keycheck::Fail> {
    use crate::keycheck::Fail;
    match c.key.as_str() {
        k if k.contains("bad") => Err(Fail { why: Why::WrongKey, said: String::new() }),
        k if k.contains("limit") => Err(Fail::of(Why::NoCredit)),
        _ => Ok(()),
    }
}

/// The first run's key step on a temp HOME, the fake sign-in in.
fn first_run(e: Env) -> Onb {
    let mut o = Onb::new(e);
    o.with_logins(fake(), e);
    o.checker = fake_check;
    o.go(Step::Model, 0);
    o
}

/// Each frame: the sign-in, the check and the sign out answer.
fn settle(o: &mut Onb, e: Env) {
    for _ in 0..400 {
        o.tick(e);
        if !matches!(o.sub, Sub::Checking(..) | Sub::SignIn(..)) && o.signing_out.is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("no answer: {:?}", o.sub);
}

fn row_of(o: &Onb, opt: Opt) -> usize {
    o.opts().iter().position(|x| *x == opt).expect("row")
}

// ---- the first run ----

#[test]
fn the_pay_step_offers_a_plan_or_a_key() {
    let h = tmp("pay");
    let e = home_env(&h, &[]);
    script(PlanState::NotSetUp, Detected::default(), vec![]);
    let o = first_run(&e);
    let sc = screen(&o);
    has(
        &sc,
        &[
            "how do you want to pay for the models?",
            "a plan you already have, or a key. you can add more later in /provider.",
            "› Continue with ChatGPT      use your Plus or Pro plan",
            "  OpenRouter                 sign in, or paste its key",
            "  an API key                 Anthropic, OpenAI, Google, Mistral…",
            "  a coding plan key          GLM, Kimi or MiniMax",
            "↑↓ choose   ⏎ go   esc back",
        ],
    );
    assert!(!sc.contains("Codex") && !sc.contains("Claude Code"), "{sc}");
    // nothing started before enter
    assert!(STARTED.with(|s| s.borrow().is_empty()));
}

#[test]
fn codex_signed_in_marks_the_chatgpt_row() {
    let h = tmp("codex");
    let e = home_env(&h, &[]);
    script(PlanState::NotSetUp, Detected { codex_chatgpt: true, claude_plan: false }, vec![]);
    let o = first_run(&e);
    has(&screen(&o), &["› Continue with ChatGPT      use your Plus or Pro plan · you use it in Codex already"]);
    // 80 columns: the description goes under its label, whole
    let sc = screen_at(&o, 80, 36);
    has(&sc, &["Continue with ChatGPT", "use your Plus or Pro plan · you use it in Codex already"]);
}

#[test]
fn claude_code_plan_says_why_once_and_only_without_an_anthropic_key() {
    let h = tmp("claude");
    let e = home_env(&h, &[]);
    let line = "your Claude plan works only in Claude Code (Anthropic's terms). for Claude here, use an API key.";
    script(PlanState::NotSetUp, Detected { codex_chatgpt: false, claude_plan: true }, vec![]);
    let o = first_run(&e);
    let sc = screen(&o);
    has(&sc, &[line]);
    assert_eq!(flat(&sc).matches("Claude Code").count(), 1, "{sc}");
    has(&screen_at(&o, 80, 36), &[line]);
    // an Anthropic key: nothing to say
    let e = home_env(&h, &[("ANTHROPIC_API_KEY", "sk-ant-x")]);
    let o = first_run(&e);
    assert!(!flat(&screen(&o)).contains("Claude Code"), "{}", screen(&o));
}

#[test]
fn continue_with_chatgpt_waits_then_signs_in_checks_and_runs_main_on_the_plan() {
    let h = tmp("happy");
    let e = home_env(&h, &[]);
    script(PlanState::NotSetUp, Detected::default(), vec![Poll::Waiting, Poll::Done(Some(me()))]);
    let mut o = first_run(&e);
    o.sel = row_of(&o, Opt::ChatGpt);
    o.on_key(key(KeyCode::Enter), 1, &e);
    // waiting: the browser opened on the link
    assert_eq!(o.sub, Sub::SignIn(Kind::ChatGpt, false));
    assert_eq!(OPENED.with(|x| x.borrow().clone()), vec![URL.to_string()]);
    let sc = screen(&o);
    has(&sc, &["waiting for you to sign in to ChatGPT in your browser…", "c copy the link   esc cancel"]);
    // c copies the link
    o.on_key(key(KeyCode::Char('c')), 2, &e);
    assert_eq!(crate::clipboard::test_clipboard().as_deref(), Some(URL));
    has(&screen(&o), &["the link is in your clipboard."]);
    // still waiting, then back from the browser: signed in, the plan checked
    o.tick(&e);
    assert!(matches!(o.sub, Sub::SignIn(..)));
    o.tick(&e);
    assert!(matches!(o.sub, Sub::Checking(_, _, Tried::Plan(_))), "{:?}", o.sub);
    assert!(o.flow.is_none());
    has(&screen(&o), &["✓ signed in as you@example.com · ChatGPT Plus", "checking your plan…"]);
    settle(&mut o, &e);
    assert_eq!(o.sub, Sub::Works(o.plan_provider().unwrap(), "chatgpt/gpt-6.1-sol".into()));
    let sc = screen(&o);
    has(&sc, &["✓ signed in as you@example.com · ChatGPT Plus", "main and your agents use gpt-6.1-sol now, on your plan.", "⏎ go on"]);
    // main runs on the plan; no token written by the screens
    let cfg = std::fs::read_to_string(h.join(".bise/config.toml")).unwrap();
    assert!(cfg.contains("chatgpt/gpt-6.1-sol"), "{cfg}");
    assert!(!cfg.contains("SECRET") && !h.join(".bise/auth.json").exists());
    o.on_key(key(KeyCode::Enter), 3, &e);
    assert_eq!(o.step, Step::Lines);
}

#[test]
fn a_refused_plan_comes_back_to_the_list_with_its_line() {
    let h = tmp("denied");
    let e = home_env(&h, &[]);
    script(PlanState::NotSetUp, Detected::default(), vec![Poll::Denied]);
    let mut o = first_run(&e);
    o.sel = row_of(&o, Opt::ChatGpt);
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert_eq!(o.sub, Sub::List);
    has(&screen(&o), &["▲ ChatGPT signed you in but didn't let bise use your plan. try again and allow it, or pick another way."]);
    // the line goes with the next pick
    o.on_key(key(KeyCode::Enter), 2, &e);
    assert!(!flat(&screen(&o)).contains("didn't let bise"));
}

#[test]
fn esc_cancels_the_sign_in_and_closes_its_listener() {
    let h = tmp("esc");
    let e = home_env(&h, &[]);
    script(PlanState::NotSetUp, Detected::default(), vec![]);
    let mut o = first_run(&e);
    o.sel = row_of(&o, Opt::ChatGpt);
    o.on_key(key(KeyCode::Enter), 1, &e);
    o.on_key(key(KeyCode::Esc), 2, &e);
    assert!(CANCELLED.with(|c| c.get()), "the listener must close");
    assert!(o.flow.is_none());
    assert_eq!((o.step, o.sub.clone()), (Step::Model, Sub::List));
    has(&screen(&o), &["▲ the sign-in wasn't finished. try again, or pick another way."]);
    // five minutes without an answer: the same line
    script(PlanState::NotSetUp, Detected::default(), vec![Poll::Unfinished]);
    o.on_key(key(KeyCode::Enter), 3, &e);
    settle(&mut o, &e);
    has(&screen(&o), &["▲ the sign-in wasn't finished. try again, or pick another way."]);
    // esc on the list: back to the theme
    o.on_key(key(KeyCode::Esc), 4, &e);
    assert_eq!(o.step, Step::Theme);
}

#[test]
fn an_expired_plan_check_says_so_and_enter_signs_in_again() {
    let h = tmp("expired");
    let e = home_env(&h, &[]);
    script(PlanState::NotSetUp, Detected::default(), vec![Poll::Done(Some(me()))]);
    let mut o = first_run(&e);
    o.logins.token = |_| Ok("tok-bad-SECRET".into());
    o.sel = row_of(&o, Opt::ChatGpt);
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(matches!(o.sub, Sub::Failed(_, _, Tried::Plan(_), _)), "{:?}", o.sub);
    has(&screen(&o), &[EXPIRED, "⏎ sign in again"]);
    o.on_key(key(KeyCode::Enter), 2, &e);
    assert_eq!(o.sub, Sub::SignIn(Kind::ChatGpt, false));
    // the plan's limit: its own line
    o.logins.token = |_| Ok("tok-limit-SECRET".into());
    script(PlanState::SignedIn(me()), Detected::default(), vec![Poll::Done(Some(me()))]);
    settle(&mut o, &e);
    has(&screen(&o), &[LIMIT]);
}

#[test]
fn openrouter_offers_sign_in_or_a_key() {
    let h = tmp("or");
    let e = home_env(&h, &[]);
    script(PlanState::NotSetUp, Detected::default(), vec![Poll::Done(None)]);
    let mut o = first_run(&e);
    o.sel = row_of(&o, Opt::OpenRouter);
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert_eq!(o.sub, Sub::OpenRouter(0));
    has(&screen(&o), &["OpenRouter", "› sign in with OpenRouter", "  paste a key", "↑↓ choose   ⏎ go   esc back"]);
    // paste a key: its models, then the key field
    o.on_key(key(KeyCode::Down), 2, &e);
    o.on_key(key(KeyCode::Enter), 2, &e);
    assert!(matches!(&o.sub, Sub::Model(p, ..) if p.id == "openrouter"), "{:?}", o.sub);
    // sign in: the browser, then (its key saved) its models
    o.sub = Sub::OpenRouter(0);
    o.on_key(key(KeyCode::Enter), 3, &e);
    assert_eq!(o.sub, Sub::SignIn(Kind::OpenRouter, false));
    has(&screen(&o), &["waiting for you to sign in to OpenRouter in your browser…"]);
    settle(&mut o, &e);
    assert!(matches!(&o.sub, Sub::Model(p, ..) if p.id == "openrouter"), "{:?}", o.sub);
}

#[test]
fn a_coding_plan_key_lists_the_coding_plans_only() {
    let h = tmp("coding");
    let e = home_env(&h, &[]);
    script(PlanState::NotSetUp, Detected::default(), vec![]);
    let mut o = first_run(&e);
    o.sel = row_of(&o, Opt::Coding);
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert_eq!(o.sub, Sub::Which(0));
    assert!(o.which_list().iter().all(|p| CODING_PLANS.contains(&p.id.as_str())));
    has(&screen(&o), &["which coding plan?"]);
    // an API key: never a coding plan
    o.on_key(key(KeyCode::Esc), 2, &e);
    o.sel = row_of(&o, Opt::Paste);
    o.on_key(key(KeyCode::Enter), 3, &e);
    assert!(!o.which_list().is_empty() && o.which_list().iter().all(|p| !CODING_PLANS.contains(&p.id.as_str())));
}

// ---- /provider ----

fn panel(e: Env) -> Onb {
    let mut o = Onb::provider_panel(e, provider::Ask::default());
    o.with_logins(fake(), e);
    o.checker = fake_check;
    o.panel.as_mut().unwrap().opener = |_| true;
    o
}

fn chatgpt_row(sc: &str) -> String {
    sc.lines().find(|l| l.contains("ChatGPT")).unwrap_or_default().to_string()
}

#[test]
fn the_chatgpt_row_says_each_state() {
    let h = tmp("rows");
    let e = home_env(&h, &[]);
    for (state, words) in [
        (PlanState::NotSetUp, "ChatGPT           not set up"),
        (PlanState::SignedOut, "ChatGPT           signed out"),
        (PlanState::SignedIn(me()), "ChatGPT           ✓ signed in · you@example.com · Plus"),
        (PlanState::Expired, "ChatGPT           ▲ sign-in expired · ⏎ sign in again"),
    ] {
        script(state.clone(), Detected::default(), vec![]);
        let o = panel(&e);
        let sc = screen(&o);
        assert!(chatgpt_row(&sc).contains(words), "{state:?}: {words:?}\n{sc}");
        // the plan first
        assert_eq!(o.rows().first(), Some(&provider::Row::P(o.plan_provider().unwrap())), "{state:?}");
    }
}

#[test]
fn enter_on_chatgpt_signs_in_or_opens_its_menu() {
    let h = tmp("menu");
    let e = home_env(&h, &[]);
    // expired: enter signs in again
    script(PlanState::Expired, Detected::default(), vec![]);
    let mut o = panel(&e);
    o.sel = 0;
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert_eq!(o.sub, Sub::SignIn(Kind::ChatGpt, false));
    has(&screen(&o), &["waiting for you to sign in to ChatGPT in your browser…"]);
    // esc: back to the list, the line said, the listener closed
    o.on_key(key(KeyCode::Esc), 2, &e);
    assert!(CANCELLED.with(|c| c.get()));
    assert_eq!(o.sub, Sub::List);
    has(&screen(&o), &["▲ the sign-in wasn't finished. try again, or pick another way."]);
    // signed in: its menu
    script(PlanState::SignedIn(me()), Detected::default(), vec![]);
    let mut o = panel(&e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(&o.sub, Sub::Menu(p, 0) if p.plan), "{:?}", o.sub);
    let sc = screen(&o);
    has(&sc, &["✓ signed in · you@example.com · Plus", "› 1 · sign in again or switch account", "2 · sign out", "3 · your plan's usage on chatgpt.com ↗"]);
    // the usage page
    o.on_key(key(KeyCode::Char('3')), 2, &e);
    has(&screen(&o), &[&format!("opening {}", PLAN_USAGE_URL)]);
    // sign out (ChatGPT didn't confirm in the fake): the row says so
    o.on_key(key(KeyCode::Char('2')), 3, &e);
    has(&screen(&o), &["signing out…"]);
    STATE.with(|s| *s.borrow_mut() = PlanState::SignedOut);
    settle(&mut o, &e);
    assert_eq!(o.sub, Sub::List);
    let sc = screen(&o);
    has(&sc, &["signed out here. ChatGPT didn't confirm: to be sure, remove bise from the connected apps in your ChatGPT settings."]);
    assert!(chatgpt_row(&sc).contains("signed out"), "{sc}");
}

#[test]
fn a_sign_in_from_provider_checks_the_plan_and_keeps_main() {
    let h = tmp("psign");
    let e = home_env(&h, &[("ANTHROPIC_API_KEY", "sk-ant-x")]);
    script(PlanState::NotSetUp, Detected::default(), vec![Poll::Done(Some(me()))]);
    let mut o = panel(&e);
    let main_before = o.model.clone();
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(matches!(&o.sub, Sub::Works(p, _) if p.plan), "{:?}", o.sub);
    has(&screen(&o), &["✓ signed in as you@example.com · ChatGPT Plus"]);
    assert_eq!(o.model, main_before);
}

#[test]
fn openrouter_on_provider_signs_in_or_pastes() {
    let h = tmp("por");
    let e = home_env(&h, &[]);
    script(PlanState::NotSetUp, Detected::default(), vec![]);
    let mut o = panel(&e);
    let i = o.rows().iter().position(|r| matches!(r, provider::Row::P(p) if p.id == "openrouter")).unwrap();
    o.sel = i;
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert_eq!(o.sub, Sub::OpenRouter(0));
    has(&screen(&o), &["› sign in with OpenRouter", "  paste a key"]);
    o.on_key(key(KeyCode::Down), 2, &e);
    o.on_key(key(KeyCode::Enter), 2, &e);
    assert!(matches!(&o.sub, Sub::Paste(p, ..) if p.id == "openrouter"), "{:?}", o.sub);
    // esc: back to the list
    o.on_key(key(KeyCode::Esc), 3, &e);
    assert_eq!(o.sub, Sub::List);
}

// ---- /models ----

#[test]
fn the_plans_models_say_the_plan_pays() {
    let h = tmp("models");
    let e = home_env(&h, &[]);
    script(PlanState::SignedIn(me()), Detected::default(), vec![]);
    let mut o = Onb::provider_panel(&e, provider::Ask { open: roles::Open::Pick(bise_catalog::roles::MAIN), ..Default::default() });
    o.with_logins(fake(), &e);
    // the providers: the plan's row says who is signed in
    has(&screen(&o), &["ChatGPT", "✓ signed in · you@example.com · Plus"]);
    let p = o.plan_provider().unwrap();
    // the account's list (its cache) then the catalog's
    assert_eq!(o.models_of(&p)[..2], ["chatgpt/gpt-6.1-sol".to_string(), "chatgpt/gpt-6.1-luna".to_string()]);
    o.sub = Sub::Model(p, 0, String::new());
    let sc = screen(&o);
    let row = sc.lines().find(|l| l.contains("gpt-6.1-luna")).unwrap_or_default();
    assert!(row.contains("your ChatGPT plan"), "{sc}");
}
