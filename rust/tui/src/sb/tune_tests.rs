//! The checks and the offers (BISE-245): temp homes and repos only.

use super::*;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-tune-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A context over a temp HOME and BISE_HOME, running in `dir`.
fn ctx(home: &Path, dir: &Path, vars: &[(&str, &str)]) -> Ctx {
    let mut v: Vars = vars.iter().map(|(k, x)| (k.to_string(), x.to_string())).collect();
    v.insert("HOME".into(), home.to_string_lossy().into());
    v.insert("BISE_HOME".into(), home.join(".bise").to_string_lossy().into());
    let look = v.clone();
    let h = bise_home::Home::from_lookup(&move |k: &str| look.get(k).cloned());
    Ctx { vars: v, home: h, dir: dir.to_path_buf(), cmd_keys: false, scope: Scope::All, mac: true }
}

#[test]
fn the_terminal_comes_from_its_variables() {
    let h = tmp("term");
    let t = |vars: &[(&str, &str)]| terminal(&ctx(&h, &h, vars)).0;
    assert_eq!(t(&[("TERM_PROGRAM", "ghostty"), ("TERM_PROGRAM_VERSION", "1.3.1")]), Term::Ghostty);
    assert_eq!(t(&[("TERM_PROGRAM", "iTerm.app")]), Term::Iterm);
    assert_eq!(t(&[("TERM", "xterm-kitty")]), Term::Kitty);
    assert_eq!(t(&[("TERM_PROGRAM", "ghostty"), ("TMUX", "/tmp/x")]), Term::Tmux);
    assert_eq!(t(&[]), Term::Unknown);
    let c = check_terminal(&ctx(&h, &h, &[("TERM_PROGRAM", "ghostty"), ("TERM_PROGRAM_VERSION", "1.3.1")]));
    assert_eq!(c, Check { mark: Mark::Fine, text: "ghostty 1.3.1".into() });
    assert_eq!(check_truecolor(&ctx(&h, &h, &[("COLORTERM", "truecolor")])).mark, Mark::Fine);
    assert_eq!(check_truecolor(&ctx(&h, &h, &[])).mark, Mark::Note);
    assert_eq!(check_glyphs(&ctx(&h, &h, &[("TERM_PROGRAM", "ghostty")])).mark, Mark::Fine);
    assert_eq!(check_glyphs(&ctx(&h, &h, &[("TERM", "linux")])).mark, Mark::Note);
}

#[test]
fn option_digits_name_the_setting_where_option_types_characters() {
    use crate::optkeys::Layout;
    // iTerm2 on a U.S. layout: bise reads ¡™£… as ⌥1-0 (optkeys.rs)
    assert_eq!(option_digits(&Term::Iterm, Layout::Us), Some(Check { mark: Mark::Fine, text: "⌥0-9 reach me".into() }));
    // another layout keeps its Option characters: the setting, a note
    assert_eq!(
        option_digits(&Term::Iterm, Layout::Other),
        Some(Check { mark: Mark::Note, text: "⌥0-9 type characters here? iterm2 Profiles › Keys › Left Option key: Esc+".into() })
    );
    let apple = option_digits(&Term::Apple, Layout::Other).unwrap();
    assert!(apple.text.contains("Use Option as Meta key"), "{}", apple.text);
    // the terminals whose Option is theirs to set: nothing to say
    for t in [Term::Ghostty, Term::Kitty, Term::Wezterm, Term::Tmux, Term::Unknown] {
        assert_eq!(option_digits(&t, Layout::Other), None);
    }
}

#[test]
fn ghostty_gets_an_offer_for_the_lines_it_misses() {
    let h = tmp("ghostty");
    let c = ctx(&h, &h, &[("TERM_PROGRAM", "ghostty")]);
    // no config yet: the macOS place, both lines
    let (chk, o) = check_cmd_keys(&c);
    assert_eq!(chk.mark, Mark::Offer);
    let file = h.join("Library/Application Support/com.mitchellh.ghostty/config");
    assert_eq!(o, Some(Offer::Keys { terminal: "ghostty".into(), file: file.clone(), add: GHOSTTY_LINES.map(String::from).to_vec() }));
    // an XDG config that has one line (spaces differ): the other one only
    let xdg = h.join(".config/ghostty/config");
    std::fs::create_dir_all(xdg.parent().unwrap()).unwrap();
    std::fs::write(&xdg, "font-size = 14\nkeybind = super+f = unbind\n").unwrap();
    let (chk, o) = check_cmd_keys(&c);
    // the four arrow lines are one key, cmd+↑↓
    assert_eq!(chk.text, "ghostty keeps cmd+v, cmd+k, cmd+a and cmd+↑↓ for itself");
    let rest: Vec<String> = GHOSTTY_LINES.iter().enumerate().filter(|(i, _)| *i != 1).map(|(_, l)| l.to_string()).collect();
    assert_eq!(o, Some(Offer::Keys { terminal: "ghostty".into(), file: xdg.clone(), add: rest }));
    // only the shift ones missing: still cmd+↑↓
    std::fs::write(&xdg, GHOSTTY_LINES[..6].join("\n")).unwrap();
    assert_eq!(check_cmd_keys(&c).0.text, "ghostty keeps cmd+↑↓ for itself");
    std::fs::write(&xdg, GHOSTTY_LINES.join("\n")).unwrap();
    assert_eq!(check_cmd_keys(&c), (Check { mark: Mark::Fine, text: "cmd+v, cmd+f, cmd+k, cmd+a and cmd+↑↓ reach me".into() }, None));
    // the others: a note, no file touched; a cmd key seen: fine
    let (chk, o) = check_cmd_keys(&ctx(&h, &h, &[("TERM_PROGRAM", "Apple_Terminal")]));
    assert_eq!((chk.mark, o), (Mark::Note, None));
    let mut k = ctx(&h, &h, &[("TERM_PROGRAM", "WezTerm")]);
    k.cmd_keys = true;
    assert_eq!(check_cmd_keys(&k).0.mark, Mark::Fine);
}

#[test]
fn a_config_is_backed_up_once_and_only_appended_to() {
    let h = tmp("apply");
    let f = h.join("config");
    std::fs::write(&f, "font-size = 14").unwrap();
    let add = vec![GHOSTTY_LINES[1].to_string()];
    assert_eq!(apply_keys(&f, &add).unwrap(), Some(h.join("config.bise-backup")));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "font-size = 14\nkeybind = super+f=unbind\n");
    assert_eq!(std::fs::read_to_string(h.join("config.bise-backup")).unwrap(), "font-size = 14");
    // a second edit keeps the first backup (the file before bise)
    apply_keys(&f, &[GHOSTTY_LINES[0].to_string()]).unwrap();
    assert_eq!(std::fs::read_to_string(h.join("config.bise-backup")).unwrap(), "font-size = 14");
    // no file: made, with its folders, no backup
    let g = h.join("a/b/config");
    assert_eq!(apply_keys(&g, &add).unwrap(), None);
    assert_eq!(std::fs::read_to_string(&g).unwrap(), "keybind = super+f=unbind\n");
    assert!(!h.join("a/b/config.bise-backup").exists());
    // AGENTS.md is never written over
    let a = h.join("AGENTS.md");
    write_agents(&a, "# one\n").unwrap();
    assert!(write_agents(&a, "# two\n").is_err());
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "# one\n");
    assert_eq!(
        diff_add("~/x/config", 3, &add, false),
        "--- ~/x/config\n+++ ~/x/config\n@@ -3,0 +4,1 @@\n+keybind = super+f=unbind"
    );
}

#[test]
fn the_repo_part_offers_a_starter_agents_md() {
    let h = tmp("repo");
    let repo = h.join("app");
    std::fs::create_dir_all(repo.join(".github/workflows")).unwrap();
    std::fs::write(repo.join("package.json"), r#"{"name":"shop","scripts":{"build":"vite build","test":"vitest"}}"#).unwrap();
    std::fs::write(repo.join("Makefile"), "lint:\n\techo\n.PHONY: lint\n").unwrap();
    std::fs::write(repo.join(".github/workflows/ci.yml"), "on: push").unwrap();
    let git = |args: &[&str]| {
        let mut c = Command::new("git");
        c.arg("-C").arg(&repo).args(args).env("GIT_CONFIG_GLOBAL", "/dev/null").env("GIT_CONFIG_NOSYSTEM", "1");
        output(c, Duration::from_secs(5)).expect("git")
    };
    git(&["init", "-q"]);
    git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "cart: faster checkout"]);
    let mut c = ctx(&h, &repo, &[]);
    c.scope = Scope::Repo;
    let f = run(&c, Duration::from_secs(3));
    assert_eq!(f.checks.len(), 2, "{f:?}");
    let root = repo_root(&repo, Duration::from_secs(2)).unwrap();
    assert_eq!(f.offers, vec![Offer::Agents { file: root.join("AGENTS.md") }]);
    assert_eq!(f.summary(), "checked 2 things · 1 fine · 1 i can fix");
    assert_eq!(f.line(), "1 small fix would help. it waits in your inbox with the exact change. yes or no, whenever you want.");
    // the ask names these very checks
    assert_eq!(subjects(Scope::Repo, true), ["git", "an AGENTS.md"]);
    let d = draft(&facts(&repo));
    for s in ["notes for the agents working on shop.", "- `npm run build`", "- `npm run test`", "- `make lint`", ".github/workflows/ci.yml", "\"cart: faster checkout\""] {
        assert!(d.contains(s), "{s}\n{d}");
    }
    // no Mistral key in this home: the plain draft, no model call
    assert_eq!(agents_text(&c, &repo), (d, false));
    // an AGENTS.md: nothing to offer
    std::fs::write(repo.join("AGENTS.md"), "# x\n").unwrap();
    assert!(run(&c, Duration::from_secs(3)).offers.is_empty());
    // not a repo: no AGENTS.md check at all
    c.dir = h.clone();
    assert_eq!(run(&c, Duration::from_secs(3)).checks.len(), 1);
}

#[test]
fn the_connectors_key_is_found_where_the_harness_finds_it() {
    let h = tmp("key");
    let (chk, o) = check_key(&ctx(&h, &h, &[]));
    assert_eq!(chk.mark, Mark::Offer);
    assert_eq!(o, Some(Offer::Key { provider: "mistral".into(), env: "MISTRAL_API_KEY".into() }));
    assert_eq!(check_key(&ctx(&h, &h, &[("MISTRAL_API_KEY", "k")])).0.mark, Mark::Fine);
}

#[test]
fn the_summary_and_main_s_line() {
    let c = |m| Check { mark: m, text: String::new() };
    let mut f = Found { checks: vec![c(Mark::Fine), c(Mark::Fine), c(Mark::Offer), c(Mark::Offer), c(Mark::Note)], offers: Vec::new() };
    f.offers = vec![Offer::Key { provider: "m".into(), env: "E".into() }; 2];
    assert_eq!(f.summary(), "checked 5 things · 2 fine · 2 i can fix · 1 note");
    assert_eq!(f.line(), "2 small fixes would help. each one waits in your inbox with the exact change. yes or no to each, whenever you want.");
    let fine = Found { checks: vec![c(Mark::Fine); 7], offers: Vec::new() };
    assert_eq!(fine.summary(), "checked 7 things · all fine");
    // the ask counts the checks that run: 9 on macOS, 7 elsewhere
    assert_eq!(
        subjects(Scope::All, true),
        ["your terminal", "its keys", "⌥0-9", "colors", "glyphs", "git", "gh", "an AGENTS.md", "the connectors key"]
    );
    assert_eq!(subjects(Scope::All, false).len(), 7);
    f.offers.clear();
    assert_eq!(f.line(), "all good here. nothing to change.");
    assert_eq!(unfence("```markdown\n# A\n- b\n```"), "# A\n- b\n");
}

#[test]
fn a_slow_check_is_cut_at_its_timeout() {
    let t = std::time::Instant::now();
    let mut c = Command::new("sleep");
    c.arg("5");
    assert_eq!(output(c, Duration::from_millis(200)), None);
    assert!(t.elapsed() < Duration::from_secs(2));
}

#[test]
fn setup_ghostty_adds_the_lines_once_with_a_backup() {
    // BISE-273: `bise setup ghostty`, the card's change without the card
    let h = tmp("setup-ghostty");
    let c = ctx(&h, &h, &[("XDG_CONFIG_HOME", &h.join("xdg").to_string_lossy())]);
    let (code, out) = setup_ghostty(&c, true);
    assert_eq!(code, 0);
    assert!(out.join("\n").contains("+keybind = super+f=unbind") && out.last().unwrap().contains("dry run"), "{out:?}");
    let mac = h.join("Library/Application Support/com.mitchellh.ghostty/config");
    assert!(!mac.exists(), "a dry run writes nothing");
    std::fs::create_dir_all(mac.parent().unwrap()).unwrap();
    std::fs::write(&mac, "font-size = 14\nkeybind = super+k=unbind").unwrap();
    let (code, out) = setup_ghostty(&c, false);
    assert_eq!(code, 0, "{out:?}");
    let text = std::fs::read_to_string(&mac).unwrap();
    assert!(text.starts_with("font-size = 14\nkeybind = super+k=unbind\n"), "{text}");
    assert!(ghostty_missing(&text).is_empty(), "{text}");
    assert_eq!(text.matches("super+k").count(), 1);
    assert_eq!(std::fs::read_to_string(backup_of(&mac)).unwrap(), "font-size = 14\nkeybind = super+k=unbind");
    let (code, out) = setup_ghostty(&c, false);
    assert_eq!((code, out[0].contains("nothing to do")), (0, true), "{out:?}");
}
