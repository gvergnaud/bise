//! `/models` and the role steps (BISE-298, roles first BISE-301): the
//! rows and their words (provider then model), a role's steps (which
//! provider, which model, the effort; the fallback row, a typed id, a
//! provider set up on the way), voice (the providers that listen, the
//! transcription check), `/provider`'s menu naming the roles.

use super::provider::*;
use super::roles::*;
use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::HashMap;
use std::path::PathBuf;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-roles-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn screen_w(o: &Onb, w: u16) -> String {
    let h = 34;
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| draw(f, o, 10)).unwrap();
    let b = t.backend().buffer().clone();
    (0..h)
        .map(|y| (0..w).map(|x| b[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn screen(o: &Onb) -> String {
    screen_w(o, 110)
}

/// The row that starts with `what` (a mark `›` or `✓` before it aside).
fn line_of<'a>(sc: &'a str, what: &str) -> &'a str {
    let starts = |l: &str| {
        let t = l.trim_start();
        let t = t.strip_prefix("› ").or_else(|| t.strip_prefix("✓ ")).unwrap_or(t);
        t.trim_start().starts_with(what)
    };
    sc.lines().find(|l| starts(l)).unwrap_or_else(|| panic!("no {what}:\n{sc}"))
}

/// The tests that read CALLS run one at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// The key check: a key holding "bad" is refused; the calls it made.
static CALLS: std::sync::Mutex<Vec<(String, bool)>> = std::sync::Mutex::new(Vec::new());

fn fake_check(c: &crate::keycheck::Call, _: Option<String>) -> Result<(), crate::keycheck::Fail> {
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).push((format!("{}/{} {}", c.provider, c.model, c.api), c.voice));
    if c.key.contains("bad") {
        return Err(crate::keycheck::Fail { why: Why::WrongKey, said: "Invalid API Key".into() });
    }
    Ok(())
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

/// A home with `cfg` as config.toml and these keys in the environment.
fn home(tag: &str, cfg: &str, keys: &[&'static str]) -> (impl Fn(&str) -> Option<String>, bise_home::Home) {
    let h = tmp(tag);
    let mut m: HashMap<&'static str, String> = HashMap::from([("HOME", h.to_string_lossy().to_string())]);
    for k in keys {
        m.insert(k, "sk-good".into());
    }
    let e = move |k: &str| m.get(k).cloned().filter(|v| !v.is_empty());
    let hm = home_of(&e);
    std::fs::create_dir_all(hm.config_file().parent().unwrap()).unwrap();
    std::fs::write(hm.config_file(), cfg).unwrap();
    (e, hm)
}

fn open(e: Env, open: Open) -> Onb {
    let mut o = Onb::provider_panel(e, Ask { open, ..Ask::default() });
    o.checker = fake_check;
    o
}

fn cfg(hm: &bise_home::Home) -> String {
    std::fs::read_to_string(hm.config_file()).unwrap()
}

fn typed(o: &mut Onb, e: Env, t: &str) {
    for c in t.chars() {
        o.on_key(key(KeyCode::Char(c)), 1, e);
    }
}

#[test]
fn models_lists_each_role_with_its_provider_then_its_model() {
    let (e, _hm) = home("list", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY"]);
    let o = open(&e, Open::Roles);
    let sc = screen(&o);
    assert!(sc.contains("which model does what?"), "{sc}");
    assert!(sc.contains("each role picks a provider, then a model. one provider can serve several."), "{sc}");
    // picked: the provider, then the id; a fallback: its rule, the provider, the id
    assert!(line_of(&sc, "main  ").contains("Mistral  mistral-medium-latest · high"), "{sc}");
    assert!(line_of(&sc, "agents ").contains("same as main · Mistral · mistral-medium-latest · high"), "{sc}");
    assert!(line_of(&sc, "small jobs ").contains("auto · Mistral · mistral-small-latest"), "{sc}");
    // voice mode off: off, and how to set it up
    assert!(line_of(&sc, "voice ").trim_end().ends_with("off · enter sets it up"), "{sc}");
    // what the role under the cursor is for, once, under the list
    assert!(sc.contains("main: talks with you and starts the agents."), "{sc}");
    assert!(sc.contains("↑↓ choose   ⏎ change   esc back"), "{sc}");
    // the checker (approvals): no Jev key, the small jobs model
    assert!(line_of(&sc, "checker ").contains("auto · Mistral · mistral-small-latest"), "{sc}");
}

/// The checker's row, `design §4.2` and designer's words: auto (Jev when
/// its key is ready), Jev's two routes, then a chat model, or off.
#[test]
fn the_checker_row_and_its_picker() {
    let _s = serial();
    let (e, hm) = home("checker", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY", "TYPESAFE_API_KEY"]);
    let mut o = open(&e, Open::Roles);
    let sc = screen(&o);
    // unset, TypeSafe's key ready: Jev, dim like small jobs
    assert!(line_of(&sc, "checker ").contains("auto · TypeSafe · jev-1.13"), "{sc}");
    while shown().get(o.sel).map(|r| r.id) != Some("classify") {
        o.on_key(key(KeyCode::Down), 1, &e);
    }
    let sc = screen(&o);
    assert!(sc.split_whitespace().collect::<Vec<_>>().join(" ").contains("checker: in auto, decides which commands run and which ask you."), "{sc}");
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("checker: which provider?") && sc.contains("now: auto · TypeSafe · jev-1.13"), "{sc}");
    assert!(sc.contains("the checker sees the command, the script it runs, and your request."), "{sc}");
    assert!(line_of(&sc, "auto ").contains("TypeSafe · jev-1.13 · now"), "{sc}");
    // designer: no tag on TypeSafe, like every other provider
    assert_eq!(line_of(&sc, "TypeSafe ").trim_end().rsplit("TypeSafe").next().map(str::trim), Some("✓ ready"), "{sc}");
    assert!(line_of(&sc, "OpenRouter ").contains("not set up · jev through OpenRouter"), "{sc}");
    assert!(sc.contains("  ── or a chat model checks ──"), "{sc}");
    assert!(line_of(&sc, "Mistral ").contains("✓ ready · main, agents, small jobs use it"), "{sc}");
    assert_eq!(sc.lines().filter(|l| l.trim_start().starts_with("OpenRouter")).count(), 1, "OpenRouter is Jev's row only: {sc}");
    assert!(sc.lines().any(|l| l.trim_start().trim_start_matches("› ") == "off · every command asks you"), "{sc}");
    // the separator is stepped over
    let rows = o.pick_rows("classify");
    let sep = rows.iter().position(|r| *r == PRow::Sep).unwrap();
    o.sel = sep - 1;
    o.on_key(key(KeyCode::Down), 1, &e);
    assert_eq!(o.sel, sep + 1);
    o.on_key(key(KeyCode::Up), 1, &e);
    assert_eq!(o.sel, sep - 1);
    // TypeSafe: Jev at once, no model and no effort step
    o.sel = rows.iter().position(|r| matches!(r, PRow::Jev(p, _) if p.id == "typesafe")).unwrap();
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(cfg(&hm).contains("classify = \"typesafe/jev-1.13\""), "{}", cfg(&hm));
    let sc = screen(&o);
    assert!(sc.contains("which model does what?"), "back to /models: {sc}");
    assert!(line_of(&sc, "checker ").contains("TypeSafe") && line_of(&sc, "checker ").contains("jev-1.13"), "{sc}");
    // off: the row says so
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(line_of(&screen(&o), "TypeSafe ").trim_end().ends_with("✓ ready · now"), "{}", screen(&o));
    o.sel = o.pick_rows("classify").iter().position(|r| *r == PRow::Off).unwrap();
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(cfg(&hm).contains("classify = \"off\""), "{}", cfg(&hm));
    assert!(line_of(&screen(&o), "checker ").contains("off · every command asks you"), "{}", screen(&o));
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("now: off · every command asks you"), "{sc}");
    // one now: the off row's, not auto's
    assert_eq!(sc.matches("· now").count(), 1, "{sc}");
    assert!(sc.contains("off · every command asks you · now"), "{sc}");
    assert_eq!(o.pick_rows("classify").get(o.sel), Some(&PRow::Off), "the cursor on off");
}

/// designer: the checker counts in the other roles' "use it" only when
/// it checks (the mode is auto and it is not off), like voice when on.
#[test]
fn the_checker_is_named_on_its_provider_only_in_auto() {
    let (e, hm) = home("checker-tags", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY"]);
    let o = open(&e, Open::Pick("agents"));
    assert!(line_of(&screen(&o), "Mistral ").contains("main, small jobs use it"), "{}", screen(&o));
    std::fs::write(hm.config_file(), "approvals = \"auto\"\n[roles]\nmain = \"mistral/mistral-medium-latest\"\n").unwrap();
    let o = open(&e, Open::Pick("agents"));
    assert!(line_of(&screen(&o), "Mistral ").contains("main, small jobs, checker use it"), "{}", screen(&o));
    std::fs::write(hm.config_file(), "approvals = \"auto\"\n[roles]\nmain = \"mistral/mistral-medium-latest\"\nclassify = \"off\"\n").unwrap();
    let o = open(&e, Open::Pick("agents"));
    assert!(line_of(&screen(&o), "Mistral ").contains("main, small jobs use it"), "{}", screen(&o));
}

#[test]
fn a_chat_model_checks_and_openrouter_s_key_goes_first() {
    let _s = serial();
    let (e, hm) = home("checker-chat", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY"]);
    let mut o = open(&e, Open::Pick("classify"));
    // a chat provider: its models, the small one recommended, no effort
    o.sel = o.pick_rows("classify").iter().position(|r| matches!(r, PRow::Provider(p) if p.id == "mistral")).unwrap();
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("checker · Mistral: which model?"), "{sc}");
    assert!(line_of(&sc, "mistral-small-latest").contains("recommended"), "{sc}");
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(cfg(&hm).contains("classify = \"mistral/mistral-small-latest\""), "{}", cfg(&hm));
    // OpenRouter not set up: its key, checked on Jev, then saved
    let mut o = open(&e, Open::Pick("classify"));
    o.sel = o.pick_rows("classify").iter().position(|r| matches!(r, PRow::Jev(p, _) if p.id == "openrouter")).unwrap();
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(&o.sub, Sub::Paste(p, m, _) if p.id == "openrouter" && m == "openrouter/typesafe/jev-1.13"), "{:?}", o.sub);
    CALLS.lock().unwrap_or_else(|x| x.into_inner()).clear();
    typed(&mut o, &e, "sk-or-good");
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    let calls = CALLS.lock().unwrap_or_else(|x| x.into_inner()).clone();
    assert_eq!(calls, vec![("openrouter/typesafe/jev-1.13 systemone".to_string(), false)]);
    assert!(cfg(&hm).contains("classify = \"openrouter/typesafe/jev-1.13\""), "{}", cfg(&hm));
}

#[test]
fn the_providers_line_up_in_one_column() {
    let (e, _hm) = home(
        "column",
        "[roles]\nmain = \"mistral/mistral-medium-latest\"\nagents = \"openai/gpt-6-luna\"\nvoice = \"mistral/voxtral-mini-latest\"\n",
        &["MISTRAL_API_KEY", "OPENAI_API_KEY"],
    );
    let o = open(&e, Open::Roles);
    let sc = screen(&o);
    let col = |row: &str, what: &str| {
        let l = line_of(&sc, row);
        l[..l.find(what).unwrap_or_else(|| panic!("{what}: {sc}"))].chars().count()
    };
    assert_eq!(col("main  ", "Mistral"), col("agents ", "OpenAI"), "{sc}");
    assert_eq!(col("main  ", "mistral-medium"), col("agents ", "gpt-6-luna"), "{sc}");
    assert_eq!(col("main  ", "Mistral"), col("voice ", "Mistral"), "{sc}");
    // an old config naming Voxtral Mini reads as Transcribe 3
    assert!(line_of(&sc, "voice ").contains("voxtral-transcribe-3 · off") && !sc.contains("voxtral-mini"), "{sc}");
}

#[test]
fn a_role_whose_provider_has_no_key_says_so_and_enter_fixes_it() {
    let (e, _hm) = home("broken", "[roles]\nmain = \"anthropic/claude-opus-5-5\"\n", &["MISTRAL_API_KEY"]);
    let mut o = open(&e, Open::Roles);
    let sc = screen(&o);
    // what runs stays on screen
    assert!(line_of(&sc, "main  ").contains("Anthropic  claude-opus-5-5  ✗ no key · enter fixes it"), "{sc}");
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(&o.sub, Sub::Paste(p, m, _) if p.id == "anthropic" && m == "anthropic/claude-opus-5-5"), "{:?}", o.sub);
}

#[test]
fn agents_pick_a_provider_then_a_model_then_its_effort() {
    let (e, hm) = home(
        "agents",
        "model = \"mistral/mistral-medium-latest\"\nagent_model = \"mistral/mistral-small-latest\"\n",
        &["MISTRAL_API_KEY"],
    );
    let mut o = open(&e, Open::Roles);
    o.on_key(key(KeyCode::Down), 1, &e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    // 1. which provider? same as main first, the ready ones, then the others
    let sc = screen(&o);
    assert!(sc.contains("agents: which provider?") && sc.contains("now: Mistral · mistral-small-latest"), "{sc}");
    let rows = o.pick_rows("agents");
    assert_eq!(rows[0], PRow::Fallback("mistral/mistral-medium-latest".into()), "{rows:?}");
    assert!(matches!(&rows[1], PRow::Provider(p) if p.id == "mistral"), "the ready one first: {rows:?}");
    assert!(matches!(rows.last(), Some(PRow::More(_))), "{rows:?}");
    assert!(line_of(&sc, "same as main").contains("Mistral · mistral-medium-latest"), "{sc}");
    assert!(!line_of(&sc, "same as main").contains("now"), "{sc}");
    // one provider, several roles: the others on it say so
    let m = line_of(&sc, "Mistral ");
    assert!(m.contains("›") && m.contains("✓ ready · now · main, small jobs use it"), "{sc}");
    assert!(line_of(&sc, "OpenAI ").contains("not set up"), "{sc}");
    assert!(sc.contains("more providers…"), "{sc}");
    // 2. which model? Mistral's, short ids, the role's marked
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("agents · Mistral: which model?") && sc.contains("type to filter, or a model id that isn't listed."), "{sc}");
    let now = line_of(&sc, "mistral-small-latest");
    assert!(now.contains("›") && now.contains("✓ now") && !now.contains("mistral/"), "{sc}");
    assert!(line_of(&sc, "mistral-medium-latest").contains("recommended"), "{sc}");
    // esc: back to the providers, on Mistral
    o.on_key(key(KeyCode::Esc), 1, &e);
    assert!(matches!(o.pick_rows("agents").get(o.sel), Some(PRow::Provider(p)) if p.id == "mistral"));
    o.on_key(key(KeyCode::Enter), 1, &e);
    // the medium one: how hard should it think? (Mistral: none, high)
    while !matches!(&o.sub, Sub::Model(p, i, f) if o.model_rows(p, f)[*i] == ModelRow::Listed("mistral/mistral-medium-latest".into())) {
        o.on_key(key(KeyCode::Down), 1, &e);
    }
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("how hard should it think?") && sc.contains("mistral-medium-latest for the agents"), "{sc}");
    assert!(line_of(&sc, "high").contains("default"), "{sc}");
    // esc: back to the models, on it
    o.on_key(key(KeyCode::Esc), 1, &e);
    assert!(matches!(&o.sub, Sub::Model(p, i, f) if o.model_rows(p, f)[*i] == ModelRow::Listed("mistral/mistral-medium-latest".into())), "{:?}", o.sub);
    o.on_key(key(KeyCode::Enter), 1, &e);
    // none: written with the model, the old key gone
    o.on_key(key(KeyCode::Up), 1, &e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    let c = cfg(&hm);
    assert!(c.contains("[roles.agents]\nmodel = \"mistral/mistral-medium-latest\"\neffort = \"none\"") && !c.contains("agent_model"), "{c}");
    // back on /models, the row flashing ✓
    let sc = screen(&o);
    assert!(sc.contains("which model does what?"), "{sc}");
    let a = line_of(&sc, "agents ");
    assert!(a.contains("✓") && a.contains("Mistral  mistral-medium-latest · none"), "{sc}");
    // same as main again (its first row): the role leaves config.toml
    o.on_key(key(KeyCode::Enter), 1, &e);
    while o.sel != 0 {
        o.on_key(key(KeyCode::Up), 1, &e);
    }
    o.on_key(key(KeyCode::Enter), 1, &e);
    let c = cfg(&hm);
    assert!(!c.contains("agents") && c.contains("model = \"mistral/mistral-medium-latest\""), "{c}");
    assert!(line_of(&screen(&o), "agents ").contains("same as main"));
}

#[test]
fn a_provider_not_set_up_goes_through_its_key_step_then_its_models() {
    let _one = serial();
    let (e, hm) = home("keyfirst", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY"]);
    let mut o = open(&e, Open::Roles);
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("main: which provider?") && !sc.contains("same as main"), "main has no fallback: {sc}");
    while !matches!(o.pick_rows("main").get(o.sel), Some(PRow::Provider(p)) if p.id == "openai") {
        o.on_key(key(KeyCode::Down), 1, &e);
    }
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(&o.sub, Sub::Paste(p, m, _) if p.id == "openai" && m == "openai/gpt-6-astra"), "{:?}", o.sub);
    let sc = screen(&o);
    assert!(sc.contains("paste your OpenAI key") && sc.contains("for main. then you pick the model."), "{sc}");
    assert!(sc.contains("⏎ check it   esc back to the providers"), "{sc}");
    // esc: back to the providers, on OpenAI
    o.on_key(key(KeyCode::Esc), 1, &e);
    assert!(matches!(o.pick_rows("main").get(o.sel), Some(PRow::Provider(p)) if p.id == "openai"));
    o.on_key(key(KeyCode::Enter), 1, &e);
    // a wrong key: the first run's words; a good one: OpenAI's models
    o.on_paste("sk-bad");
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(matches!(o.sub, Sub::Failed(..)), "{:?}", o.sub);
    o.on_key(key(KeyCode::Enter), 1, &e);
    o.on_paste("sk-good-openai");
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(matches!(&o.sub, Sub::Model(p, 0, f) if p.id == "openai" && f.is_empty()), "{:?}", o.sub);
    assert!(stored(&hm, "openai"), "a normal provider key");
    let sc = screen(&o);
    assert!(sc.contains("main · OpenAI: which model?") && line_of(&sc, "gpt-6-astra").contains("recommended"), "{sc}");
    // an id it doesn't list: checked with one tiny call
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    typed(&mut o, &e, "gpt-6-nova");
    let sc = screen(&o);
    assert!(sc.contains("no listed model matches.") && sc.contains("+ use openai/gpt-6-nova   not in my list: i'll try it with one tiny call"), "{sc}");
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    let calls = CALLS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    assert!(calls.iter().any(|(c, _)| c.starts_with("openai/gpt-6-nova")), "{calls:?}");
    // its effort (or saved when it has none), then main runs it
    if matches!(o.sub, Sub::Effort(..)) {
        o.on_key(key(KeyCode::Enter), 1, &e);
    }
    assert!(cfg(&hm).contains("openai/gpt-6-nova"), "{}", cfg(&hm));
    assert!(line_of(&screen(&o), "main  ").contains("OpenAI"), "{}", screen(&o));
}

#[test]
fn voice_lists_only_the_providers_that_listen() {
    let (e, _hm) = home("voice", "", &["OPENAI_API_KEY"]);
    let mut o = open(&e, Open::Pick("voice"));
    let sc = screen(&o);
    assert!(sc.contains("voice: which provider?") && sc.contains("only the providers that can listen. ctrl+r starts, any key stops."), "{sc}");
    // the ready one first and preselected, then the others
    let open_ai = line_of(&sc, "OpenAI ");
    assert!(open_ai.contains("›") && open_ai.contains("✓ ready"), "{sc}");
    assert!(sc.find("OpenAI ").unwrap() < sc.find("Mistral ").unwrap(), "{sc}");
    assert!(line_of(&sc, "Mistral ").contains("not set up"), "{sc}");
    assert!(line_of(&sc, "ElevenLabs ").contains("not set up · voice only"), "{sc}");
    // Groq and Deepgram: not offered; no chat-only provider
    assert!(!sc.contains("Groq") && !sc.contains("Deepgram") && !sc.contains("Anthropic") && !sc.contains("more providers"), "{sc}");
    assert!(sc.contains("esc not now"), "{sc}");
    // its voice models, short, its pick first
    o.on_key(key(KeyCode::Enter), 1, &e);
    let sc = screen(&o);
    assert!(sc.contains("voice · OpenAI: which model?") && sc.contains("you talk, it types in the composer."), "{sc}");
    let first = line_of(&sc, "gpt-transcribe ");
    assert!(first.contains("›") && first.contains("recommended") && !first.contains("openai/"), "{sc}");
    assert!(line_of(&sc, "whisper-1").trim() == "whisper-1", "{sc}");
    assert!(!sc.contains("gpt-6"), "voice models only: {sc}");
}

#[test]
fn no_key_at_all_preselects_mistral_and_esc_leaves_voice_off() {
    let (e, _hm) = home("novoice", "", &[]);
    let mut o = Onb::provider_panel(&e, Ask { open: Open::Pick("voice"), voice_on: true, ..Ask::default() });
    o.checker = fake_check;
    let sc = screen(&o);
    assert!(!sc.contains("✓ ready"), "{sc}");
    assert!(line_of(&sc, "Mistral ").contains("›"), "{sc}");
    let _ = take_voice_out();
    assert_eq!(o.on_key(key(KeyCode::Esc), 1, &e), Out::Done);
    assert_eq!(take_voice_out(), Some(VoiceOut::Off));
}

#[test]
fn a_voice_provider_is_set_up_then_its_model_picked_without_a_second_check() {
    let _one = serial();
    let (e, hm) = home("voicekey", "", &[]);
    let mut o = Onb::provider_panel(&e, Ask { open: Open::Pick("voice"), voice_on: true, ..Ask::default() });
    o.checker = fake_check;
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    let _ = take_voice_out();
    // Mistral: its key first, checked with its voice pick
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(matches!(&o.sub, Sub::Paste(p, m, _) if p.id == "mistral" && m == "mistral/voxtral-transcribe-3"), "{:?}", o.sub);
    let sc = screen(&o);
    assert!(sc.contains("paste your Mistral key") && sc.contains("for voice. then you pick the model."), "{sc}");
    // wrong key, then a good one: checked by a transcription
    o.on_paste("sk-bad");
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(matches!(o.sub, Sub::Failed(..)), "{:?}", o.sub);
    assert!(screen(&o).contains("Invalid API Key"), "{}", screen(&o));
    o.on_key(key(KeyCode::Enter), 1, &e);
    o.on_paste("sk-good-mistral");
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    // then its voice models, on the one just checked
    assert!(matches!(&o.sub, Sub::Model(p, 0, _) if p.id == "mistral"), "{:?}", o.sub);
    let sc = screen(&o);
    // Transcribe 3 first; Voxtral Mini is not offered
    assert!(sc.contains("voxtral-transcribe-3") && !sc.contains("voxtral-mini") && !sc.contains("mistral-medium-latest"), "{sc}");
    assert!(sc.find("voxtral-transcribe-3") < sc.find("voxtral-small-transcribe-3"), "{sc}");
    assert_eq!(o.on_key(key(KeyCode::Enter), 1, &e), Out::Done);
    // (the chat checks of the other tests run alongside: the voice ones)
    let calls: Vec<_> = CALLS.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|(_, v)| *v).cloned().collect();
    assert!(calls.len() == 2 && calls.iter().all(|(c, _)| c == "mistral/voxtral-transcribe-3 mistral"), "{calls:?}");
    assert_eq!(take_voice_out(), Some(VoiceOut::On("mistral/voxtral-transcribe-3".into())));
    assert!(cfg(&hm).contains("[roles]\nvoice = \"mistral/voxtral-transcribe-3\""), "{}", cfg(&hm));
    assert!(stored(&hm, "mistral"), "a normal provider key: /provider shows it, chat can use it");
}

#[test]
fn a_ready_voice_provider_is_two_enters_and_one_check() {
    let _one = serial();
    let (e, hm) = home("voiceready", "", &["OPENAI_API_KEY"]);
    let mut o = open(&e, Open::Pick("voice"));
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    let _ = take_voice_out();
    o.on_key(key(KeyCode::Enter), 1, &e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(o.panel.as_ref().unwrap().closed);
    let calls: Vec<_> = CALLS.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|(_, v)| *v).cloned().collect();
    assert_eq!(calls, vec![("openai/gpt-transcribe openai".to_string(), true)]);
    assert_eq!(take_voice_out(), Some(VoiceOut::On("openai/gpt-transcribe".into())));
    assert!(cfg(&hm).contains("voice = \"openai/gpt-transcribe\""));
}

#[test]
fn mistral_serves_main_and_voice() {
    let _one = serial();
    let (e, hm) = home("both", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY"]);
    let mut o = open(&e, Open::Roles);
    let _ = take_voice_out();
    while shown()[o.sel].id != "voice" {
        o.on_key(key(KeyCode::Down), 1, &e);
    }
    assert!(screen(&o).contains("voice: writes down what you say. enter opens /voice."), "{}", screen(&o));
    // voice-menu (designer): ⏎ on voice opens /voice's screen (on speech
    // to text), the one editor of the voice model; /models closes
    {
        let _s = crate::voicemode::settings::test_serial();
        let _ = crate::voicemode::settings::take_request();
        assert_eq!(o.on_key(key(KeyCode::Enter), 1, &e), Out::Done);
        assert_eq!(crate::voicemode::settings::take_request(), Some(crate::voicemode::settings::Open::Settings));
    }
    // its speech-to-text row's ⏎: the voice picker (the same as ctrl+r's)
    let mut o = Onb::provider_panel(&e, Ask { open: Open::Pick("voice"), from_settings: true, ..Ask::default() });
    o.checker = fake_check;
    let sc = screen(&o);
    // the key main uses: ready, nothing to set up
    assert!(line_of(&sc, "Mistral ").contains("› ") && line_of(&sc, "Mistral ").contains("✓ ready · main, agents, small jobs use it"), "{sc}");
    o.on_key(key(KeyCode::Enter), 1, &e);
    o.on_key(key(KeyCode::Enter), 1, &e);
    settle(&mut o, &e);
    assert!(cfg(&hm).contains("voice = \"mistral/voxtral-transcribe-3\""), "{}", cfg(&hm));
    // from /voice: the model only, dictation as it was
    assert_eq!(take_voice_out(), None);
    // esc on /voice comes back to /models, on the voice row
    let o = open(&e, Open::RolesAt("voice"));
    assert_eq!(shown()[o.sel].id, "voice");
    let sc = screen(&o);
    assert!(line_of(&sc, "main  ").contains("Mistral") && line_of(&sc, "voice ").contains("Mistral  voxtral-transcribe-3"), "{sc}");
    assert!(line_of(&sc, "voice ").contains("› "), "{sc}");
}

#[test]
fn provider_menu_names_the_roles_and_sends_to_models() {
    let (e, _hm) = home("menu", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY", "ELEVENLABS_API_KEY"]);
    let mut o = Onb::provider_panel(&e, Ask { provider: Some("mistral".into()), ..Ask::default() });
    let sc = screen(&o);
    assert!(sc.contains("main, agents and small jobs use it. /models changes that."), "{sc}");
    assert!(!sc.contains("use it for") && !sc.contains("use Mistral for"), "{sc}");
    assert!(line_of(&sc, "1 · paste a new key").contains("›"), "{sc}");
    // the list: the roles each provider runs, in one column
    o.on_key(key(KeyCode::Esc), 1, &e);
    let sc = screen(&o);
    assert!(line_of(&sc, "Mistral ").contains("main · agents · small jobs"), "{sc}");
    // a voice-only provider with a key shows; nothing uses it yet
    assert!(sc.contains("ElevenLabs"), "{sc}");
    let el = o.every_of("elevenlabs");
    o.sel = o.rows().iter().position(|r| *r == Row::P(el.clone())).unwrap();
    o.on_key(key(KeyCode::Enter), 1, &e);
    assert!(screen(&o).contains("no role uses it yet. /models picks one."), "{}", screen(&o));
}

#[test]
fn models_fits_narrow_screens_by_cutting_the_fallbacks_id_first() {
    let (e, _hm) = home("narrow", "[roles.main]\nmodel = \"anthropic/claude-opus-5-5\"\neffort = \"high\"\n", &["ANTHROPIC_API_KEY"]);
    let o = open(&e, Open::Roles);
    let wide = screen_w(&o, 110);
    let narrow = screen_w(&o, 50);
    assert!(line_of(&wide, "main  ").contains("Anthropic  claude-opus-5-5 · high"), "{wide}");
    assert!(line_of(&wide, "agents  ").contains("same as main · Anthropic · claude-opus-5-5 · high"), "{wide}");
    // a row never wraps: the fallback's id goes, never its provider; the
    // picked id stays
    assert!(line_of(&narrow, "main  ").contains("Anthropic  claude-opus-5-5"), "{narrow}");
    let a = line_of(&narrow, "agents  ");
    assert!(a.contains("same as main · Anthropic") && !a.contains("claude"), "{narrow}");
    assert!(!narrow.lines().any(|l| l.trim_start().starts_with("claude")), "{narrow}");
}

#[test]
fn an_agent_with_its_own_model_shows_under_agents() {
    let (e, _hm) = home("own", "[roles]\nmain = \"mistral/mistral-medium-latest\"\n", &["MISTRAL_API_KEY"]);
    let o = Onb::provider_panel(&e, Ask { open: Open::Roles, overrides: vec![("perf".into(), "openai/gpt-6-sol".into())], ..Ask::default() });
    let sc = screen(&o);
    let i = sc.lines().position(|l| l.contains("agents   ")).expect("agents row");
    assert!(sc.lines().nth(i + 1).unwrap().contains("1 agent uses its own model: perf · openai/gpt-6-sol"), "{sc}");
}
