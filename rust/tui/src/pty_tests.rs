use super::*;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn args_need_an_absolute_folder_and_clamp_the_size() {
    let a = parse_args(&s(&["--cwd", "/w/shop", "--cols", "120", "--rows", "30"])).unwrap();
    assert_eq!(a, Args { cwd: "/w/shop".into(), cols: 120, rows: 30, shell: None });
    let a = parse_args(&s(&["--cwd", "/w", "--cols", "1", "--rows", "0", "--shell", "/bin/sh"])).unwrap();
    assert_eq!((a.cols, a.rows, a.shell.as_deref()), (2, 1, Some("/bin/sh")));
    let a = parse_args(&s(&["--cwd", "/w"])).unwrap();
    assert_eq!((a.cols, a.rows), (80, 24), "the default size");
    assert!(parse_args(&s(&["--cols", "80"])).is_err(), "no folder");
    assert!(parse_args(&s(&["--cwd", "w/shop"])).is_err(), "a relative folder");
    assert!(parse_args(&s(&["--cwd", "/w", "--cols", "many"])).is_err(), "not a number");
}

#[test]
fn clamps_keep_a_size_a_terminal_can_have() {
    assert_eq!(clamp(0, 0), (2, 1));
    assert_eq!(clamp(80, 24), (80, 24));
    assert_eq!(clamp(9000, 9000), (500, 200));
}

#[test]
fn the_shell_gets_his_settings_and_nothing_of_bise_or_a_test_run() {
    let parent: Vec<(OsString, OsString)> = [
        ("PATH", "/usr/bin:/bin"),
        ("HOME", "/h"),
        ("LANG", "fr_FR.UTF-8"),
        ("EDITOR", "nvim"),
        // User: his
        ("BISE_HOME", "/h/.bise"),
        ("MISTRAL_API_KEY", "k"),
        // Internal: bise's own
        ("SB_SOCKET", "/s"),
        ("BISE_APP_ROOT", "/app"),
        ("BEND_SESSION_FILE", "/f"),
        ("BISE_OWNERS", "x"),
        // Test: a test run's
        ("BISE_TEST_HOME", "/t"),
        ("SB_STATE_DIR", "/t/state"),
        ("BISE_HOME_WORKSPACE", "/t/ws"),
        // a terminal type of the parent's
        ("TERM", "dumb"),
    ]
    .iter()
    .map(|(k, v)| (OsString::from(k), OsString::from(v)))
    .collect();
    let env = shell_env(parent);
    let get = |k: &str| env.get(k).map(|v| v.to_string_lossy().into_owned());
    for (k, v) in [("PATH", "/usr/bin:/bin"), ("HOME", "/h"), ("LANG", "fr_FR.UTF-8"), ("EDITOR", "nvim"), ("BISE_HOME", "/h/.bise"), ("MISTRAL_API_KEY", "k")] {
        assert_eq!(get(k).as_deref(), Some(v), "lost {k}");
    }
    for k in ["SB_SOCKET", "BISE_APP_ROOT", "BEND_SESSION_FILE", "BISE_OWNERS", "BISE_TEST_HOME", "SB_STATE_DIR", "BISE_HOME_WORKSPACE"] {
        assert_eq!(get(k), None, "{k} reached his shell");
    }
    assert_eq!(get("TERM").as_deref(), Some("xterm-256color"));
    assert_eq!(get("COLORTERM").as_deref(), Some("truecolor"));
}

/// What a fresh terminal shows after the snapshot: the same cells, the
/// same cursor, the same screen (main or alternate), the same modes.
fn replayed(p: &vt100::Parser) -> vt100::Parser {
    let (rows, cols) = p.screen().size();
    let mut fresh = vt100::Parser::new(rows, cols, 0);
    fresh.process(&snapshot(p.screen()));
    fresh
}

#[test]
fn a_snapshot_redraws_a_plain_shell_screen() {
    let mut p = vt100::Parser::new(5, 20, 100);
    p.process(b"$ ls\r\n\x1b[31mred.txt\x1b[0m  b.txt\r\n$ ");
    let r = replayed(&p);
    assert_eq!(r.screen().contents(), p.screen().contents());
    assert_eq!(r.screen().cursor_position(), p.screen().cursor_position());
    assert_eq!(r.screen().cell(1, 0).unwrap().fgcolor(), vt100::Color::Idx(1), "colors kept");
    assert!(!r.screen().alternate_screen());
}

#[test]
fn a_snapshot_redraws_a_full_screen_program_on_the_alternate_screen() {
    // a vim-like program: the shell's lines, then the alternate screen with
    // its own rows, a cursor in the middle, the application cursor keys
    let mut p = vt100::Parser::new(6, 30, 100);
    p.process(b"$ vim notes.md\r\n");
    p.process(b"\x1b[?1049h\x1b[?1h\x1b[H\x1b[2J# notes\r\n~\r\n~\r\n\x1b[6;1H\"notes.md\" 1L\x1b[1;3H");
    let r = replayed(&p);
    assert!(r.screen().alternate_screen(), "the program's screen, not the shell's");
    assert_eq!(r.screen().contents(), p.screen().contents());
    assert_eq!(r.screen().cursor_position(), (0, 2));
    assert!(r.screen().application_cursor(), "its keys mode");
    // the program quits: the shell's screen comes back, as on his terminal
    let mut r = r;
    r.process(b"\x1b[?1049l");
    assert!(!r.screen().alternate_screen());
}

#[test]
fn exit_codes_and_signals() {
    assert_eq!(exit_code(&ExitStatus::with_exit_code(0)), Some(0));
    assert_eq!(exit_code(&ExitStatus::with_exit_code(3)), Some(3));
    assert_eq!(exit_code(&ExitStatus::with_signal("Hangup")), None);
}
