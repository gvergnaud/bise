//! The setup card (BISE-245): the ask, not now, yes, the offers, /setup.
//! A temp HOME and BISE_HOME; the checks are a fake runner.

use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::path::{Path, PathBuf};

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-setup-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn vars(home: &Path, more: &[(&str, &str)]) -> Vars {
    let mut v: Vars = more.iter().map(|(k, x)| (k.to_string(), x.to_string())).collect();
    v.insert("HOME".into(), home.to_string_lossy().into());
    v.insert("BISE_HOME".into(), home.join(".bise").to_string_lossy().into());
    v
}

fn draw(app: &mut App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw_sb(app, f)).unwrap();
    let buf = term.backend().buffer().clone();
    buf.content.chunks(w as usize).map(|r| r.iter().map(|c| c.symbol()).collect::<String>()).collect()
}

/// The strip row holding `needle` (BISE-302: numbered, no keys; a
/// click or ctrl+N opens it).
fn strip_row(app: &mut App, w: u16, h: u16, needle: &str) -> String {
    let rows = draw(app, w, h);
    rows.iter().find(|r| r.contains(needle)).cloned().unwrap_or_else(|| panic!("no row {needle:?}:\n{}", rows.join("\n")))
}

fn press(app: &mut App, code: KeyCode, m: KeyModifiers) {
    super::super::key(app, &KeyEvent::new(code, m), false);
}

fn infos(app: &App) -> Vec<String> {
    app.events
        .iter()
        .filter_map(|e| match e {
            Ev::Info(t) | Ev::Warn(t) | Ev::Assistant(t) => Some(t.clone()),
            Ev::Fold { head, text, .. } if text.is_empty() => Some(head.clone()),
            Ev::Fold { head, .. } => Some(format!("▸ {head}")),
            _ => None,
        })
        .collect()
}

/// An app at its first hello in `dir`, the launch armed with `v`.
fn launched(dir: &Path, v: Vars) -> App {
    let mut app = bench::test_app_drained();
    app.sb.workspace = dir.to_string_lossy().into();
    app.sb.ready = true;
    launch_for_tests(&mut app, v);
    pump(&mut app);
    app
}

fn setup_cards(app: &App) -> Vec<String> {
    app.sb.cards.iter().filter(|c| is_local(c.id)).map(|c| c.text.lines().next().unwrap_or("").to_string()).collect()
}

#[test]
fn once_per_user_then_once_per_new_repo() {
    let h = tmp("due");
    let v = vars(&h, &[]);
    let (a, b) = (h.join("a"), h.join("b"));
    assert_eq!(due(&v, Some(&a)), Some(Scope::All));
    remember(&v, Some(&a));
    assert_eq!(due(&v, Some(&a)), None);
    assert_eq!(due(&v, Some(&b)), Some(Scope::Repo));
    assert_eq!(due(&v, None), None);
    assert!(std::fs::read_to_string(h.join(".bise/prefs.json")).unwrap().contains("asked"));
    for off in [("SB_SETUP", "off"), ("SB_ONBOARDING", "off")] {
        assert_eq!(due(&vars(&tmp("off"), &[off]), None), None);
    }
}

#[test]
fn the_card_waits_in_the_strip_and_not_now_leaves_one_row() {
    let h = tmp("ask");
    let v = vars(&h, &[]);
    let mut app = launched(&h, v.clone());
    assert_eq!(setup_cards(&app), vec!["can i set bise up for your terminal and this repo?"]);
    assert!(!app.sb.card.open, "never opened for you");
    // its row numbered, no keys on it (BISE-302)
    let rows = draw(&mut app, 120, 30);
    assert!(!rows.iter().any(|r| r.contains("2 not now")), "{}", rows.join("\n"));
    strip_row(&mut app, 120, 30, " 1 ? main · ");
    assert!(!rows.iter().any(|r| r.contains(&format!("#{LOCAL}"))), "no hub number");
    // the empty thread stays with the card waiting in the strip
    assert!(rows.iter().any(|r| r.contains(super::super::panel::FIRST_RUN[1])), "{}", rows.join("\n"));
    // a snapshot from the hub keeps it
    app.sb.cards.retain(|c| !is_local(c.id));
    put_back(&mut app.sb);
    assert_eq!(setup_cards(&app).len(), 1);
    // opened: what set up does, the checks counted, nothing picked
    press(&mut app, KeyCode::Char('1'), KeyModifiers::CONTROL);
    let rows = draw(&mut app, 140, 40).join("\n");
    let n = tune::subjects(Scope::All, cfg!(target_os = "macos")).len();
    for s in [
        "? can i set bise up for your terminal and this repo?",
        "about a minute",
        &format!("i'll check {n} things: your terminal, "),
        "connectors key.",
        "checking changes nothing.",
        "1 yes, check   2 not now",
    ] {
        assert!(rows.contains(s), "{s}: {rows}");
    }
    assert!(!rows.contains('▸'), "nothing picked: {rows}");
    assert!(rows.contains("1-2 answer   ←→ choose   ctrl+o full screen   esc back to your message"), "{rows}");
    // 2: not now
    press(&mut app, KeyCode::Char('2'), KeyModifiers::NONE);
    assert!(setup_cards(&app).is_empty());
    assert_eq!(infos(&app), vec!["– not now · type /setup whenever you want"]);
    assert_eq!(due(&v, None), None, "never asked again");
    // one dim row, no note glyph; the empty thread stays under it
    let rows = draw(&mut app, 120, 30);
    let at = |n: &str| rows.iter().position(|r| r.contains(n));
    let not_now = at("– not now · type /setup whenever you want").unwrap_or_else(|| panic!("{}", rows.join("\n")));
    assert!(!rows[not_now].contains("· – not now"), "{}", rows[not_now]);
    assert!(at("what's on your mind?").is_some_and(|y| y > not_now + 1), "{}", rows.join("\n"));
    // the repo part only, for a user who answered, in a new repo
    let repo = h.join("r");
    std::fs::create_dir_all(&repo).unwrap();
    let mut c = std::process::Command::new("git");
    c.arg("-C").arg(&repo).args(["init", "-q"]);
    tune::output(c, Duration::from_secs(5)).unwrap();
    let app = launched(&repo, v.clone());
    assert_eq!(setup_cards(&app), vec!["new repo: can i set bise up for it?"]);
}

#[test]
fn close_is_not_now_and_nothing_runs() {
    let h = tmp("close");
    let mut app = launched(&h, vars(&h, &[]));
    set_runner(&mut app, |_, _| panic!("no checks without a yes"), vars(&h, &[]));
    let id = app.sb.cards.iter().find(|c| is_local(c.id)).unwrap().id;
    super::super::cards::open_view(&mut app, Some(id));
    press(&mut app, KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert_eq!(infos(&app), vec!["– not now · type /setup whenever you want"]);
}

/// The checks as a fake: a Ghostty config to change, the key, then the
/// starter AGENTS.md.
fn fake(ctx: tune::Ctx, tx: mpsc::Sender<Msg>) {
    let home = ctx.home.user_home().to_path_buf();
    let c = |m, t: &str| tune::Check { mark: m, text: t.into() };
    let f = Found {
        checks: vec![
            c(tune::Mark::Fine, "ghostty 1.3.1"),
            c(tune::Mark::Offer, "ghostty keeps cmd+v, cmd+f, cmd+k, cmd+a and cmd+↑↓ for itself"),
            c(tune::Mark::Offer, "no AGENTS.md in this repo"),
            c(tune::Mark::Offer, "no MISTRAL_API_KEY: the connectors are off"),
            c(tune::Mark::Note, "gh isn't logged in · gh auth login"),
        ],
        offers: vec![
            Offer::Keys { terminal: "ghostty".into(), file: home.join("ghostty/config"), add: tune::GHOSTTY_LINES.map(String::from).to_vec() },
            Offer::Agents { file: ctx.dir.join("AGENTS.md") },
            Offer::Key { provider: "mistral".into(), env: "MISTRAL_API_KEY".into() },
        ],
    };
    tx.send(Msg::Found(f)).unwrap();
    tx.send(Msg::Agents(ctx.dir.join("AGENTS.md"), "# AGENTS.md\n\n- `cargo test`\n".into())).unwrap();
}

#[test]
fn yes_folds_the_checks_and_brings_one_card_per_change() {
    let h = tmp("yes");
    let v = vars(&h, &[]);
    std::fs::create_dir_all(h.join("ghostty")).unwrap();
    std::fs::write(h.join("ghostty/config"), "font-size = 14\n").unwrap();
    let mut app = launched(&h, v.clone());
    set_runner(&mut app, fake, v.clone());
    press(&mut app, KeyCode::Char('1'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Char('1'), KeyModifiers::NONE);
    pump(&mut app);
    assert_eq!(
        infos(&app),
        vec![
            "▸ checked 5 things · 1 fine · 3 i can fix · 1 note",
            "3 small fixes would help. each one waits in your inbox with the exact change. yes or no to each, whenever you want.",
        ]
    );
    assert!(!app.sb.card.open, "the offers are not opened for you");
    assert_eq!(
        setup_cards(&app),
        vec![
            "let cmd+v, cmd+f, cmd+k, cmd+a and cmd+↑↓ reach bise",
            "turn on web search and the other connectors",
            "write a starter AGENTS.md",
        ]
    );
    // the strip: each row, its faint end when it fits in the reading
    // column (not the Ghostty one's), the key's action
    let keys = strip_row(&mut app, 160, 40, "? main · let cmd+v");
    assert!(keys.contains(" 1 ? main · let cmd+v, cmd+f, cmd+k, cmd+a and cmd+↑↓ reach bise"), "{keys}");
    let rows = draw(&mut app, 160, 40);
    let agents = rows.iter().find(|r| r.contains("? main · write a starter")).unwrap();
    assert!(agents.contains("? main · write a starter AGENTS.md new file · 3 lines"), "{agents}");
    // the fold opens on every check
    let fold = app.events.iter().position(|e| matches!(e, Ev::Fold { .. })).unwrap();
    assert!(crate::feed::toggle_event(&mut app.events, &mut app.cache, fold));
    let rows = draw(&mut app, 120, 40).join("\n");
    assert!(rows.contains("▾ checked 5 things") && rows.contains("✓ ghostty 1.3.1") && rows.contains("– gh isn't logged in"), "{rows}");
    // the ghostty card: the exact diff inside; nothing written before a yes
    let ids: Vec<u64> = app.sb.cards.iter().filter(|c| is_local(c.id)).map(|c| c.id).collect();
    super::super::cards::open_view(&mut app, Some(ids[0]));
    // in place its first lines; full screen (ctrl+o) all of it, the tabs
    let rows = draw(&mut app, 120, 40).join("\n");
    assert!(rows.contains("more lines · ctrl+o full screen"), "{rows}");
    press(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL);
    let rows = draw(&mut app, 120, 40).join("\n");
    assert!(rows.contains("+keybind = performable:super+v=paste_from_clipboard") && rows.contains("config.bise-backup"), "{rows}");
    for s in [
        // the long title is cut before the meta (`reach bi…`)
        "? let cmd+v, cmd+f, cmd+k, cmd+a and cmd+↑↓ reach",
        "Ghostty config · +8 lines",
        "Ghostty keys",
        "AGENTS.md",
        "connectors",
        "right now Ghostty keeps these keys for itself.",
        "i'd add 8 lines to ~/ghostty/config:",
        "cmd+↑↓ jump to your message's start or end (with shift, they select to there).",
        "i copy the file to config.bise-backup first.",
        "1 yes, add them   2 no",
    ] {
        assert!(rows.contains(s), "{s}: {rows}");
    }
    press(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Right, KeyModifiers::NONE);
    let bar: String = crate::keybar::line(&app, 200).spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(bar, "⏎ yes, add them   ←→ choose   ↑↓ other items   ctrl+o full screen   esc back to your message");
    assert_eq!(std::fs::read_to_string(h.join("ghostty/config")).unwrap(), "font-size = 14\n");
    press(&mut app, KeyCode::Char('1'), KeyModifiers::NONE);
    assert!(std::fs::read_to_string(h.join("ghostty/config")).unwrap().ends_with("keybind = super+shift+arrow_down=unbind\n"));
    assert_eq!(std::fs::read_to_string(h.join("ghostty/config.bise-backup")).unwrap(), "font-size = 14\n");
    let last = infos(&app).pop().unwrap();
    assert_eq!(last, "✓ Ghostty config · 8 lines added, the old one in ~/ghostty/config.bise-backup · reload Ghostty (cmd+shift+,) to use them");
    // the view moved on to the key card: typed text is masked, never
    // in the history, saved in auth.json
    let key_id = ids[1];
    assert_eq!(app.sb.current_card().map(|c| c.id), Some(key_id));
    assert!(masked(&app));
    let bar: String = crate::keybar::line(&app, 200).spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(bar, "paste your key   ⏎ save   ctrl+x not now   esc back");
    press(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL);
    let rows = draw(&mut app, 120, 40).join("\n");
    assert!(rows.contains("it goes in ~/.bise/auth.json") && rows.contains("no key yet? console.mistral.ai"), "{rows}");
    press(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL);
    app.ed.insert("sk-test-123");
    let rows = draw(&mut app, 120, 40).join("\n");
    assert!(!rows.contains("sk-test-123") && rows.contains("•••••••••••"), "{rows}");
    press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(!app.history.iter().any(|h| h.contains("sk-test")));
    let auth = std::fs::read_to_string(h.join(".bise/auth.json")).unwrap();
    assert!(auth.contains("sk-test-123"), "{auth}");
    assert_eq!(infos(&app).pop().unwrap(), "✓ MISTRAL_API_KEY saved · the agents you start from now on can search the web");
    // AGENTS.md: no
    press(&mut app, KeyCode::Char('2'), KeyModifiers::NONE);
    assert!(!h.join("AGENTS.md").exists());
    assert_eq!(infos(&app).pop().unwrap(), "– no AGENTS.md · type /setup whenever you want");
    assert!(setup_cards(&app).is_empty() && !app.sb.card.open);
}

#[test]
fn a_paste_that_is_not_a_key_keeps_the_card() {
    let h = tmp("badkey");
    let mut app = bench::test_app_drained();
    set_runner(&mut app, fake, vars(&h, &[]));
    let id = add_for_tests(&mut app, What::Key { provider: "mistral".into(), env: "MISTRAL_API_KEY".into() });
    super::super::cards::open_view(&mut app, Some(id));
    app.ed.insert("not a key");
    press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(setup_cards(&app).len(), 1);
    assert!(!h.join(".bise/auth.json").exists());
    // ctrl+x: no, nothing saved
    press(&mut app, KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert_eq!(infos(&app).pop().unwrap(), "– no connectors key · type /setup whenever you want");
}

#[test]
fn slash_setup_runs_the_checks_again() {
    let h = tmp("again");
    let mut app = bench::test_app_drained();
    app.sb.workspace = h.to_string_lossy().into();
    set_runner(&mut app, fake, vars(&h, &[]));
    command(&mut app);
    pump(&mut app);
    assert_eq!(setup_cards(&app).len(), 3);
    // again: the old offers go, the new ones come
    command(&mut app);
    pump(&mut app);
    assert_eq!(setup_cards(&app).len(), 3);
    assert_eq!(due(&vars(&h, &[]), None), None, "asked: no card at the next launch");
    assert!(crate::commands::COMMANDS.iter().any(|c| c.name == "/setup"));
}

#[test]
fn ascii_marks() {
    crate::theme::set_ascii_for_tests(true);
    let h = tmp("ascii");
    let mut app = launched(&h, vars(&h, &[]));
    let id = app.sb.cards.iter().find(|c| is_local(c.id)).unwrap().id;
    super::super::cards::open_view(&mut app, Some(id));
    press(&mut app, KeyCode::Char('2'), KeyModifiers::NONE);
    crate::theme::set_ascii_for_tests(false);
    assert_eq!(infos(&app), vec!["- not now · type /setup whenever you want"]);
}

// subscriptions (designer): the ask says the ChatGPT plan works here too
// only when Codex uses it and bise doesn't
#[test]
fn the_ask_names_the_chatgpt_plan_only_when_codex_uses_it() {
    let text = |l: &Look| l.body.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>().join("\n");
    assert!(text(&ask_look(Scope::All, true)).contains("use your ChatGPT plan here too · /provider"));
    assert!(!text(&ask_look(Scope::All, false)).contains("ChatGPT"));
}
