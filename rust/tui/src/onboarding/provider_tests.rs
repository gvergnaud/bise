//! `/provider` (BISE-294): the list, a provider's set-up with every state
//! of the key check, its menu, removing a key, the `/model` hand-off.

use super::provider::*;
use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::HashMap;
use std::path::PathBuf;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-prov-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn env_of(m: HashMap<&'static str, String>) -> impl Fn(&str) -> Option<String> {
    move |k: &str| m.get(k).cloned().filter(|v| !v.is_empty())
}

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn screen(o: &Onb) -> String {
    let (w, h) = (110, 34);
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| draw(f, o, 10)).unwrap();
    let b = t.backend().buffer().clone();
    (0..h)
        .map(|y| (0..w).map(|x| b[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The screen's text, rows trimmed and joined: phrases across wraps.
fn flat(sc: &str) -> String {
    sc.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
}

fn fake_check(c: &crate::keycheck::Call, _: Option<String>) -> Result<(), crate::keycheck::Fail> {
    use crate::keycheck::Fail;
    match c.key.as_str() {
        k if k.contains("bad") => Err(Fail { why: Why::WrongKey, said: "invalid api key".into() }),
        k if k.contains("broke") => Err(Fail { why: Why::NoCredit, said: "insufficient credits".into() }),
        k if k.contains("locked") => Err(Fail { why: Why::NoAccess, said: "not for you".into() }),
        k if k.contains("far") => Err(Fail::of(Why::Unreachable("connection refused".into()))),
        _ => Ok(()),
    }
}

fn no_open(_: &str) -> bool {
    true
}

fn settle(o: &mut Onb, e: Env) {
    for _ in 0..400 {
        o.tick(e);
        if !matches!(o.sub, Sub::Checking(..)) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the check never answered");
}

fn panel(e: Env, ask: Ask) -> Onb {
    let mut o = Onb::provider_panel(e, ask);
    o.checker = fake_check;
    o.panel.as_mut().unwrap().opener = no_open;
    o
}

fn paste(o: &mut Onb, e: Env, k: &str) {
    o.on_paste(k);
    o.on_key(key(KeyCode::Enter), 1, e);
    settle(o, e);
}

/// Down until the row of `name` is selected, then enter.
fn open_row(o: &mut Onb, e: Env, name: &str) {
    let i = o.rows().iter().position(|r| matches!(r, Row::P(p) if p.name == name)).unwrap_or_else(|| panic!("no {name}"));
    while o.sel != i {
        o.on_key(key(KeyCode::Down), 1, e);
    }
    o.on_key(key(KeyCode::Enter), 1, e);
}

fn line_of<'a>(sc: &'a str, what: &str) -> &'a str {
    sc.lines().find(|l| l.contains(what)).unwrap_or_else(|| panic!("no {what}:\n{sc}"))
}

#[test]
fn the_list_says_which_providers_are_set_up_and_from_where() {
    let h = tmp("list");
    let hs = h.to_string_lossy().to_string();
    let e = env_of(HashMap::from([("HOME", hs.clone()), ("OPENAI_API_KEY", "sk-env".into()), ("BISE_MODEL", "openai/gpt-6-astra".into())]));
    let hm = home_of(&e);
    std::fs::create_dir_all(hm.auth_file().parent().unwrap()).unwrap();
    std::fs::write(hm.auth_file(), r#"{"anthropic": {"type": "api", "key": "sk-saved"}}"#).unwrap();
    let o = panel(&e, Ask::default());
    let sc = screen(&o);
    assert!(sc.contains("providers") && sc.contains("the keys i can use. enter sets one up or changes it."), "{sc}");
    assert!(sc.contains("› type to filter"), "{sc}");
    assert!(line_of(&sc, "Anthropic ").contains("✓ saved in bise"), "{sc}");
    let openai = line_of(&sc, "OpenAI ");
    // BISE-298: the roles it runs, by their names
    // (under its row when the column is too narrow for them)
    assert!(openai.contains("✓ from OPENAI_API_KEY") && sc.contains("main · agents · small jobs"), "{sc}");
    // in one column, after the widest state (designer, BISE-301)
    let at = |l: &str, w: &str| l[..l.find(w).unwrap()].chars().count();
    assert_eq!(at(openai, "main"), at(openai, "✓") + "✓ from OPENAI_API_KEY".chars().count() + 3, "{sc}");
    // the private proxy is never offered
    assert!(!sc.contains("foundry"), "{sc}");
    assert!(line_of(&sc, "OpenRouter").contains("not set up"), "{sc}");
    assert!(line_of(&sc, "Google AI Studio").contains("not set up"), "{sc}");
    // the hidden ones: one row, named
    assert!(line_of(&sc, "more providers…").contains(" more"), "{sc}");
    assert!(!sc.contains("Groq "), "{sc}");
    assert!(sc.contains("↑↓ choose · enter open · esc back"), "{sc}");
    assert!(!sc.contains("sk-"), "never a key: {sc}");
}

#[test]
fn more_providers_and_typing_reach_the_hidden_ones() {
    let h = tmp("more");
    let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string()), ("GROQ_API_KEY", "gk".into())]));
    let mut o = panel(&e, Ask::default());
    // a hidden one with a key shows in the list
    assert!(o.rows().iter().any(|r| matches!(r, Row::P(p) if p.id == "groq")));
    assert!(!o.rows().iter().any(|r| matches!(r, Row::P(p) if p.id == "xai")));
    let more = o.rows().iter().position(|r| matches!(r, Row::More(_))).unwrap();
    while o.sel != more {
        o.on_key(key(KeyCode::Down), 1, &e);
    }
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(o.rows().iter().any(|r| matches!(r, Row::P(p) if p.id == "xai")));
    assert!(!o.rows().iter().any(|r| matches!(r, Row::More(_))));
    // a filter: every provider it matches
    let mut o = panel(&e, Ask::default());
    for c in "deeps".chars() {
        o.on_key(key(KeyCode::Char(c)), 1, &e);
    }
    let ids: Vec<String> = o.rows().iter().filter_map(|r| if let Row::P(p) = r { Some(p.id.clone()) } else { None }).collect();
    assert_eq!(ids, ["deepseek"]);
    assert!(screen(&o).contains("› deeps▏"));
    // esc empties the filter, then closes
    assert_eq!(o.on_key(key(KeyCode::Esc), 1, &e), Out::Stay);
    assert_eq!(o.on_key(key(KeyCode::Esc), 1, &e), Out::Done);
}

#[test]
fn setting_up_a_provider_runs_every_state_of_the_first_run() {
    let h = tmp("setup");
    let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string()), ("BISE_MODEL", "anthropic/claude-sonnet-4-5".into()), ("ANTHROPIC_API_KEY", "a".into())]));
    let hm = home_of(&e);
    let mut o = panel(&e, Ask::default());
    open_row(&mut o, &e, "OpenRouter");
    // not set up: sign in or paste (subscriptions), then its key, checked with its pick
    assert_eq!(o.sub, Sub::OpenRouter(0));
    o.on_key(key(KeyCode::Down), 1, &e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    let Sub::Paste(p, m, _) = &o.sub else { panic!("{:?}", o.sub) };
    assert_eq!((p.id.as_str(), m.clone()), ("openrouter", format!("openrouter/{}", p.model)));
    let sc = screen(&o);
    assert!(sc.contains("paste your OpenRouter key") && sc.contains("get one: https://openrouter.ai"), "{sc}");
    // a wrong key: its words, nothing saved
    paste(&mut o, &e, "bad-key");
    let sc = screen(&o);
    assert!(sc.contains("OpenRouter says this key is wrong.") && sc.contains("OpenRouter said: \"invalid api key\""), "{sc}");
    assert!(sc.contains("tab another provider"), "{sc}");
    assert!(!hm.auth_file().exists());
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(o.sub, Sub::Paste(..)), "{:?}", o.sub);
    // unreachable, then no access: their lines
    paste(&mut o, &e, "far-key");
    assert!(screen(&o).contains("i couldn't reach OpenRouter: connection refused."));
    o.on_key(key(KeyCode::Esc), 1, &e);
    assert!(matches!(o.sub, Sub::List), "no key yet: back to the list, {:?}", o.sub);
    open_row(&mut o, &e, "OpenRouter");
    o.on_key(key(KeyCode::Down), 1, &e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    paste(&mut o, &e, "locked-key");
    assert!(screen(&o).contains("this key can't use"));
    // tab: another provider, from the list
    o.on_key(key(KeyCode::Tab), 1, &e);
    assert!(matches!(o.sub, Sub::List));
    assert!(matches!(&o.rows()[o.sel], Row::P(p) if p.id == "openrouter"));
    // no credit: the key is saved, the billing page linked
    o.on_key(key(KeyCode::Enter), 1, &e);
    o.on_key(key(KeyCode::Down), 1, &e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    paste(&mut o, &e, "broke-key");
    let sc = screen(&o);
    assert!(sc.contains("works, but your OpenRouter account has no credit yet."), "{sc}");
    assert!(sc.contains("add some here: https://openrouter.ai"), "{sc}");
    assert!(std::fs::read_to_string(hm.auth_file()).unwrap().contains("broke-key"));
    // esc: it has a key now, its menu
    o.on_key(key(KeyCode::Esc), 1, &e);
    assert!(matches!(&o.sub, Sub::Menu(p, 0) if p.id == "openrouter"), "{:?}", o.sub);
    // a new key that works
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(o.sub, Sub::Paste(..)), "no confirm: the menu said it, {:?}", o.sub);
    paste(&mut o, &e, "good-key");
    let sc = screen(&o);
    assert!(sc.contains("answered.") && sc.contains("OpenRouter is ready. agents use the key from their next message."), "{sc}");
    let auth = std::fs::read_to_string(hm.auth_file()).unwrap();
    assert!(auth.contains("good-key") && !auth.contains("broke-key"));
    // main's model stays
    assert!(!hm.config_file().exists(), "a key alone keeps the model");
    assert!(!sc.contains("good-key"));
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(&o.sub, Sub::Menu(p, 0) if p.id == "openrouter"), "{:?}", o.sub);
    // the list says so
    o.on_key(key(KeyCode::Esc), 1, &e);
    assert!(line_of(&screen(&o), "OpenRouter").contains("✓ saved in bise"));
}

#[test]
fn a_set_up_provider_has_its_menu() {
    let h = tmp("menu");
    let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string()), ("OPENAI_API_KEY", "sk-env".into())]));
    let hm = home_of(&e);
    let mut o = panel(&e, Ask { provider: Some("openai".into()), ..Ask::default() });
    // a key from the environment only: nothing to remove
    assert!(matches!(&o.sub, Sub::Menu(p, 0) if p.id == "openai"), "{:?}", o.sub);
    let sc = screen(&o);
    assert!(sc.contains("✓ ready · from OPENAI_API_KEY"), "{sc}");
    assert!(sc.contains("change it where you set it, or paste one here."), "{sc}");
    // BISE-301: keys and accounts only; the roles on it, picked on /models
    assert!(sc.contains("1 · paste a new key") && sc.contains("2 · open the keys page") && sc.contains("3 · open billing"), "{sc}");
    assert!(!sc.contains("use it for") && !sc.contains("remove the key"), "{sc}");
    assert!(sc.contains("no role uses it yet. /models picks one."), "{sc}");
    // a page: opened, said
    o.on_key(key(KeyCode::Char('2')), 1, &e);
    assert!(screen(&o).contains("opening https://platform.openai.com/api-keys"));
    // main on OpenAI (/models): the menu says so
    std::fs::create_dir_all(hm.config_file().parent().unwrap()).unwrap();
    std::fs::write(hm.config_file(), "[roles]\nmain = \"openai/gpt-6-astra\"\n").unwrap();
    o.setup = setup_of(&e, &hm);
    assert!(screen(&o).contains("main, agents and small jobs use it. /models changes that."), "{}", screen(&o));
    // a pasted key over the environment's: saved, said, removable
    o.on_key(key(KeyCode::Char('1')), 1, &e);
    paste(&mut o, &e, "good-key");
    let sc = flat(&screen(&o));
    assert!(sc.contains("OPENAI_API_KEY in your environment holds another key: i use this one."), "{sc}");
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("✓ ready · saved in bise") && sc.contains("4 · remove the key"), "{sc}");
    o.on_key(key(KeyCode::Char('4')), 1, &e);
    let sc = screen(&o);
    // BISE-298: the roles that stop, by name
    let fl = flat(&sc);
    assert!(sc.contains("remove the OpenAI key saved in bise?") && fl.contains("main, agents and small jobs use OpenAI. without the key they stop."), "{sc}");
    assert!(sc.contains("enter remove · esc keep it"), "{sc}");
    o.on_key(key(KeyCode::Esc), 1, &e);
    assert!(matches!(&o.sub, Sub::Menu(_, 3)));
    o.on_key(key(KeyCode::Enter), 1, &e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    // the environment's key stays: still ready, said
    assert!(matches!(&o.sub, Sub::Menu(p, 0) if p.id == "openai"), "{:?}", o.sub);
    assert!(screen(&o).contains("✓ removed. OPENAI_API_KEY still gives me a key."));
    assert!(!std::fs::read_to_string(hm.auth_file()).unwrap().contains("good-key"));
}

#[test]
fn removing_the_only_key_leaves_it_not_set_up() {
    let h = tmp("rm");
    let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string())]));
    let hm = home_of(&e);
    std::fs::create_dir_all(hm.auth_file().parent().unwrap()).unwrap();
    std::fs::write(hm.auth_file(), r#"{"mistral": {"type": "api", "key": "m-saved"}}"#).unwrap();
    let mut o = panel(&e, Ask { provider: Some("mistral".into()), ..Ask::default() });
    let mistral = o.rows().iter().find_map(|r| if let Row::P(p) = r { (p.id == "mistral").then(|| p.clone()) } else { None }).unwrap();
    let n = o.items(&mistral).len();
    assert!(matches!(&o.sub, Sub::Menu(p, 0) if p.id == "mistral"));
    o.on_key(key(KeyCode::Char(char::from_digit(n as u32, 10).unwrap())), 1, &e);
    assert!(matches!(o.sub, Sub::Remove(_)), "{:?}", o.sub);
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(o.sub, Sub::List));
    let sc = screen(&o);
    assert!(sc.contains("✓ removed the Mistral key.") && line_of(&sc, "Mistral ").contains("not set up"), "{sc}");
}

#[test]
fn a_model_of_a_provider_without_a_key_sets_it_up_then_runs_its_line() {
    let h = tmp("ask");
    let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string())]));
    let hm = home_of(&e);
    let ask = Ask {
        provider: Some("openrouter".into()),
        model: Some("openrouter/x-ai/grok-9".into()),
        line: Some("/model openrouter/x-ai/grok-9".into()),
        ..Ask::default()
    };
    let mut o = panel(&e, ask);
    assert!(matches!(&o.sub, Sub::Paste(p, m, _) if p.id == "openrouter" && m == "openrouter/x-ai/grok-9"), "{:?}", o.sub);
    paste(&mut o, &e, "good-key");
    let sc = screen(&o);
    assert!(sc.contains("it works: x-ai/grok-9 answered.") && sc.contains("OpenRouter is ready. enter switches to x-ai/grok-9."), "{sc}");
    assert!(!hm.config_file().exists());
    assert_eq!(o.on_key(key(KeyCode::Enter), 1, &e), Out::Done);
    assert_eq!(take_line().as_deref(), Some("/model openrouter/x-ai/grok-9"));
    assert_eq!(take_line(), None);
}

#[test]
fn a_local_provider_needs_no_key() {
    let h = tmp("local");
    let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string())]));
    let hm = home_of(&e);
    let mut o = panel(&e, Ask { provider: Some("ollama".into()), ..Ask::default() });
    assert!(matches!(&o.sub, Sub::Menu(p, 0) if p.id == "ollama"), "{:?}", o.sub);
    // BISE-301: no key, so nothing to do here: its models are on /models
    let sc = screen(&o);
    assert!(sc.contains("✓ no key needed") && sc.contains("no role uses it yet. /models picks one."), "{sc}");
    assert!(sc.contains("esc back") && !sc.contains("1 ·"), "{sc}");
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(&o.sub, Sub::Menu(p, 0) if p.id == "ollama"), "{:?}", o.sub);
    assert!(!hm.config_file().exists());
}

#[test]
fn the_model_list_has_only_the_models_of_ready_providers() {
    let picks = crate::models::picks_with(&|id| id == "anthropic");
    assert!(!picks.is_empty());
    for p in &picks {
        let full = crate::models::full_name(&p.value).unwrap_or_default();
        assert!(full.starts_with("anthropic/"), "{} = {}", p.value, full);
    }
    let all = crate::models::picks_with(&|_| true);
    assert!(all.iter().any(|p| p.value.starts_with("openrouter/")));
    assert!(all.len() > picks.len());
    // what a machine with no key reads: the keyless ones
    let h = tmp("ready");
    let hs = h.to_string_lossy().to_string();
    let ready = crate::models::ready_in(&|k| (k == "HOME").then(|| hs.clone()), &home_of(&|k: &str| (k == "HOME").then(|| hs.clone())));
    assert!(ready.iter().any(|r| r == "ollama") && !ready.iter().any(|r| r == "openrouter"), "{ready:?}");
    crate::models::TEST_READY.with(|r| *r.borrow_mut() = Some(ready));
    assert_eq!(crate::models::keyless("openrouter/x-ai/grok-9"), Some(("openrouter".into(), "OpenRouter".into())));
    assert_eq!(crate::models::keyless("ollama/llama9"), None);
    assert_eq!(crate::models::keyless("nowhere/x"), None);
}

#[test]
fn a_turn_without_a_key_says_so_in_one_line() {
    use crate::wire::{no_key, parse_line};
    let why = "no openrouter key yet (OPENROUTER_API_KEY is not set): /provider sets it up";
    assert_eq!(no_key(why).as_deref(), Some("turn stopped: no OpenRouter key yet. /provider sets it up."));
    // an older runtime's words
    assert_eq!(no_key("OPENROUTER_API_KEY is not set").as_deref(), Some("turn stopped: no OpenRouter key yet. /provider sets it up."));
    assert_eq!(no_key("HTTP 500: boom"), None);
    assert_eq!(no_key("my var is not set"), None);
    // the feed: the discarded candidate goes, the turn's end says it once
    assert!(parse_line(&format!("  obs: candidate_discarded: {}", why)).is_none());
    match parse_line(&format!("  obs: turn_done: failed: {}", why)) {
        Some(crate::Ev::Err(t)) => assert_eq!(t, "turn stopped: no OpenRouter key yet. /provider sets it up."),
        _ => panic!("no turn line"),
    }
}
