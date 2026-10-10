use super::*;
use serde_json::json;

fn text(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn row(id: &str, state: &str, detail: &str, fix: Option<&str>) -> Value {
    json!({"id": id, "state": state, "detail": detail, "fix": fix})
}

fn check(rows: Vec<Value>) -> Value {
    json!({"browser": "Chrome", "min_major": 116, "extension": {"id": "x", "dir": "/h/.bise/dev/try/computer-use/extension"}, "rows": rows})
}

fn no_helper() -> Vec<Value> {
    vec![row("accessibility", "not_yet", "", Some("install_helper")), row("screen_recording", "not_yet", "", Some("install_helper"))]
}

fn around() -> Around {
    Around { provider: "Anthropic".into(), mode: "yolo".into(), ..Around::default() }
}

/// design §8: chrome → chrome extension → live test, the fix lines of
/// the selected row, its ⏎ on the key bar; no helper, no "for apps"
#[test]
fn the_extension_step_shows_the_load_unpacked_steps() {
    let mut r = vec![row("browser", "done", "Chrome 154", None), row("extension", "waits", "add bise to Chrome", Some("add_extension")), row("live_test", "not_yet", "", None)];
    r.extend(no_helper());
    let rows = rows(&check(r), &Busy::default());
    assert_eq!(rows.len(), 3, "{rows:#?}");
    let sel = follow(&[], &rows, 0);
    assert_eq!(sel, 1, "the first step that isn't done");
    let t = text(&lines(&rows, sel, &around(), 80));
    assert_eq!(
        t,
        " computer use · agents can drive Chrome

 ✓ Chrome              Chrome 154
 ? Chrome extension    add bise to Chrome
                       1  ⏎ opens chrome://extensions · turn on Developer mode,…
                       2  Load unpacked · cmd+shift+g, cmd+v (i copied the fold…
                       3  i'll see it here, no key needed
 · live test           last

 what the agents see goes only to their model: Anthropic.
 in yolo, agents act without asking, purchases included. ⇧⇥ for auto.

 ⏎ open chrome://extensions   ↑↓ step   esc later"
    );
}

/// Chrome not open, too old, not installed: the designer's lines
#[test]
fn the_browser_rows_say_what_to_do() {
    let one = |r: Value| {
        let rows = rows(&check(vec![r, row("extension", "not_yet", "", None), row("live_test", "not_yet", "", None)]), &Busy::default());
        (rows[0].st, rows[0].detail.clone(), rows[0].action.clone(), rows[1].detail.clone())
    };
    assert_eq!(
        one(row("browser", "waits", "Chrome isn't open", Some("open_browser"))),
        (St::Waits, "Chrome isn't open. open it, i'll wait".into(), Some("open Chrome".into()), "after Chrome".into())
    );
    assert_eq!(
        one(row("browser", "failed", "Chrome 112", Some("update_browser"))),
        (St::Failed, "Chrome 112 is too old. bise needs 116".into(), Some("open chrome://settings/help".into()), "after Chrome".into())
    );
    assert_eq!(one(row("browser", "failed", "Chrome isn't installed", Some("install_browser"))).2, Some("open its download page".into()));
    // Edge: the rows name it
    let mut c = check(vec![row("browser", "done", "Edge 140", None), row("extension", "failed", "", Some("repair"))]);
    c["browser"] = json!("Edge");
    let r = rows(&c, &Busy::default());
    assert_eq!((r[0].label.as_str(), r[1].label.as_str()), ("Edge", "Edge extension"));
    assert_eq!((r[1].detail.as_str(), r[1].action.as_deref()), ("the extension can't reach bise", Some("repair it")));
}

/// the live test: running, done (the ready line), failed (try again)
#[test]
fn the_live_test_runs_then_says_ready() {
    let base = |lt: Value| check(vec![row("browser", "done", "Chrome 154", None), row("extension", "done", "v0.1.0", None), lt]);
    let running = rows(&base(row("live_test", "waits", "last", Some("run_live_test"))), &Busy { live_test: true, ..Busy::default() });
    assert_eq!((running[2].st, running[2].detail.as_str()), (St::Checking, "opening a tab in the background…"));
    let done = rows(&base(row("live_test", "done", "ready", None)), &Busy::default());
    assert!(ready(&done));
    let t = text(&lines(&done, 2, &Around { mode: "auto".into(), ..around() }, 100));
    assert!(t.contains(" ✓ live test           opened a tab, clicked a button. all set"), "{t}");
    // Chrome only: only what's true (m_3904)
    assert!(t.contains(" ✓ Chrome is ready. ask any agent to use it."), "{t}");
    assert!(t.contains(" in auto, agents ask you in their thread before buying"), "{t}");
    assert!(t.ends_with(" ↑↓ step   esc later"), "{t}");
    let failed = rows(&base(row("live_test", "failed", "the click did not land", Some("run_live_test"))), &Busy::default());
    assert_eq!((failed[2].st, failed[2].action.as_deref()), (St::Failed, Some("try again")));
    assert_eq!(failed[2].detail, "the click didn't land", "contractions (m_3904)");
    assert!(!ready(&failed));
}

/// the helper installed: the "for apps" rows after the live test, the
/// Screen Recording relaunch shows its wait
#[test]
fn the_apps_rows_come_after_the_live_test() {
    let c = check(vec![
        row("browser", "done", "Chrome 154", None),
        row("extension", "done", "v0.1.0", None),
        row("live_test", "done", "ready", None),
        row("accessibility", "done", "", None),
        row("screen_recording", "waits", "", Some("request_screen_recording")),
    ]);
    let r = rows(&c, &Busy::default());
    assert_eq!(follow(&[], &r, 0), 4);
    let t = text(&lines(&r, 4, &Around { apps: true, ..around() }, 90));
    assert!(t.starts_with(" computer use · agents can drive your apps and Chrome"), "{t}");
    assert!(t.contains("   for apps\n ✓ accessibility       bise can click and type in apps\n ? screen recording    optional · for screenshots of apps\n                       turn on bise Computer Use in the list. macOS reopens it, i'll wait"), "{t}");
    assert!(t.ends_with(" ⏎ open System Settings   ↑↓ step   esc later"), "{t}");
    // an apps row not done yet: the ready line names Chrome only
    assert!(t.contains(" ✓ Chrome is ready. ask any agent to use it."), "{t}");
    let mut all = c.clone();
    all["rows"][4] = row("screen_recording", "done", "", None);
    let t = text(&lines(&rows(&all, &Busy::default()), 4, &Around { apps: true, ..around() }, 90));
    assert!(t.contains(" ✓ ready. ask any agent to use Chrome or an app.\n"), "{t}");
    let reopening = rows(&c, &Busy { reopening: Some(Instant::now()), ..Busy::default() });
    assert_eq!((reopening[4].st, reopening[4].detail.as_str()), (St::Checking, "bise Computer Use reopens…"));
}

/// a selected row that turns ✓ moves the cursor to the next open one
#[test]
fn the_cursor_moves_on_when_a_step_is_done() {
    let r = |ext: &str| {
        rows(
            &check(vec![row("browser", "done", "Chrome 154", None), row("extension", ext, "", Some("add_extension")), row("live_test", if ext == "done" { "waits" } else { "not_yet" }, "", None)]),
            &Busy::default(),
        )
    };
    let before = r("waits");
    let after = r("done");
    assert_eq!(follow(&before, &after, 1), 2);
    // a move by hand stays
    assert_eq!(follow(&before, &before, 0), 0);
}

/// the drivers' line and the stop key (m_3896)
#[test]
fn who_drives_in_words() {
    let d = |n: &str| (n.to_string(), Driver { driving: Some("Chrome".into()), ..Driver::default() });
    let one: Drivers = [d("api-v2")].into_iter().collect();
    assert_eq!(drivers_line(&one).as_deref(), Some("↖ api-v2 drives Chrome"));
    let two: Drivers = [d("api-v2"), d("perf")].into_iter().collect();
    assert_eq!(drivers_line(&two).as_deref(), Some("↖ api-v2 and perf drive Chrome"));
    let three: Drivers = [d("a"), d("b"), d("c")].into_iter().collect();
    assert_eq!(drivers_line(&three).as_deref(), Some("↖ 3 agents drive Chrome"));
    assert_eq!(drivers_line(&Drivers::new()), None);
    let rows = rows(&check(vec![row("browser", "done", "Chrome 154", None)]), &Busy::default());
    let t = text(&lines(&rows, 0, &Around { drivers: drivers_line(&two), ..around() }, 80));
    assert!(t.ends_with(" ↖ api-v2 and perf drive Chrome\n\n x stop all   ↑↓ step   esc later"), "{t}");
}

/// state.json (C6) as the marks read it
#[test]
fn state_json_reads_who_drives() {
    let v = json!({"v": 2, "agents": {
        "aa.api-v2": {"name": "api-v2", "hub": "aa", "driving": "Chrome", "where": "amazon.fr", "since_ms": 5, "paused": false, "stopped": false},
        "aa.held": {"name": "held", "hub": "aa", "driving": null, "where": null, "paused": false, "stopped": true},
        "aa.mail": {"name": "mail", "hub": "aa", "driving": "Mail", "where": "Mail", "paused": true, "stopped": false},
        // another project's perf, and an agent of no hub: not this TUI's
        "bb.perf": {"name": "perf", "hub": "bb", "driving": "Chrome", "where": "x.org", "paused": false, "stopped": false},
        "bench": {"name": "bench", "hub": null, "driving": "Chrome", "where": "x.org", "paused": false, "stopped": false}
    }});
    let d = parse_state(&v, "aa");
    assert_eq!(d.keys().collect::<Vec<_>>(), ["api-v2", "held", "mail"]);
    // a hub whose state dir is too long for a unix socket (an agent's deep
    // TMPDIR): it and the TUI both hash the short path it is reached by,
    // so its agent stays shown (harness's tui_socket test: same input)
    let natural = std::path::PathBuf::from(format!("/var/folders/xy/{}/T/.bise/hubs/tmp-x-ee85d2ab/hub.sock", "a".repeat(80)));
    let short = bise_home::socket::socket_path(&natural);
    assert_ne!(short, natural, "relocated");
    let hub = hub_of(&short);
    let v = json!({"v": 2, "agents": {format!("{hub}.perf"): {"name": "perf", "hub": hub, "driving": "Chrome", "paused": false, "stopped": false}}});
    assert_eq!(parse_state(&v, &hub_of(&short)).keys().collect::<Vec<_>>(), ["perf"]);
    assert!(parse_state(&v, &hub_of(&natural)).is_empty(), "the natural path would blank the driving line");
    assert_eq!((d["api-v2"].driving.as_deref(), d["api-v2"].place.as_deref(), d["api-v2"].paused, d["api-v2"].stopped), (Some("Chrome"), Some("amazon.fr"), false, false));
    assert!(d["held"].stopped && d["held"].driving.is_none());
    assert!(d["mail"].paused);
    assert_eq!(short_app("Calculator"), "Calcula…");
    assert_eq!(short_app("Chrome"), "Chrome");
}

/// the tool rows read `summary` from the runtime's 200-character preview
#[test]
fn an_action_row_reads_its_summary() {
    let ok = r##"{"summary":"clicked \"Add to cart\" · amazon.fr","changed":"- button \"x\" [e1]","ok":true,"title":"Shop","url":"https://amazon.fr/"##;
    assert_eq!(sub_row("computer.act", ok), Some((Did::Done, "clicked \"Add to cart\" · amazon.fr".into())));
    let err = r##"{"error":{"summary":"couldn't find \"Email\"","candidates":[],"code":"not_found","message":"…"}}"##;
    assert_eq!(sub_row("computer.act", err), Some((Did::Failed, "couldn't find \"Email\"".into())));
    let held = r##"{"error":{"summary":"paused · you took the wheel in Mail","code":"paused","message":"m"}}"##;
    assert_eq!(sub_row("computer.act", held).map(|x| x.0), Some(Did::Held));
    // a cut summary is no row; quiet calls and other tools neither
    assert_eq!(sub_row("computer.act", r##"{"summary":"clicked \"Add to c"##), None);
    assert_eq!(sub_row("computer.snapshot", r##"{"refs":3,"target":"tab:1","text":"# x"##), None);
    assert_eq!(sub_row("github.search", r##"{"summary":"x"}"##), None);
    assert_eq!(sub_row("computer.act", r##"{"summary":"typed \u00e9t\u00e9 · Figma","ok":true}"##).map(|x| x.1), Some("typed été · Figma".into()));
}

/// S29/L19 (architect m_16121): the status row's words are
/// `bise_computer_use::live::line`'s, the ones the window's row carries;
/// the TUI adds its own age form (`2m`) and colours only
#[test]
fn the_status_row_says_the_shared_line() {
    let at = |d: &Driver| status_line_of(d, 125_000).map(|l| text(&[l]));
    let drives = Driver { name: "api-v2".into(), driving: Some("Chrome".into()), place: Some("amazon.fr".into()), since_ms: Some(5_000), ..Driver::default() };
    assert_eq!(at(&drives).as_deref(), Some("↖ driving Chrome · amazon.fr · 2m"));
    assert_eq!(at(&drives), bise_computer_use::live::line(&drives, Some("2m"), false).map(|l| l.text()));
    let paused = Driver { paused: true, ..drives.clone() };
    assert_eq!(at(&paused).as_deref(), Some("? you took the wheel · ⏎ give it back"));
    let stopped = Driver { stopped: true, driving: None, was: Some("Chrome".into()), stopped_by: Some("you".into()), ..drives.clone() };
    assert_eq!(at(&stopped).as_deref(), Some("↖ you stopped it driving Chrome · write to it to go on"));
    assert_eq!(at(&Driver::default()), None);
}
