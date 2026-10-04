use super::*;
use serde_json::json;

fn panel(d: Diff) -> Panel {
    Panel {
        ask: Ask::Agent("pricing-page".into()),
        req: 1,
        diff: Some(d),
        focused: true,
        cursor: 0,
        top: 0,
        unfolded: HashSet::new(),
        folded: HashSet::new(),
        list: None,
        rows: Vec::new(),
        body: Rect::default(),
        area: Rect::default(),
        page: 10,
        changes_seen: None,
        what: "pricing-page vs main".into(),
        last_land: None,
        side: true,
        sel: None,
    }
}

fn hunk_json() -> Value {
    json!({"old": 38, "new": 38, "head": "export function Pricing()", "lines": [
        " export function Pricing() {", "   return (", "     <section className=\"plans\">",
        "-      <Banner text=\"save 20% this week\" />", "-      <Plan name=\"free\" />",
        "+      <Plan name=\"free\" note=\"for side projects\" />", "       <Plan name=\"team\" highlight />"]})
}

pub(crate) fn one_file() -> Value {
    json!({"ev": "diff", "req": 1, "title": "pricing-page vs main", "branch": "pricing-page", "commits": 2, "uncommitted": false,
        "files": [{"path": "src/pages/pricing.tsx", "status": "M", "add": 1, "del": 2, "abs": "/w/src/pages/pricing.tsx", "hunks": [hunk_json()]}]})
}

pub(crate) fn many_files() -> Value {
    let f = |p: &str, st: &str, a: u64, d: u64| json!({"path": p, "status": st, "add": a, "del": d, "abs": format!("/w/{p}"), "hunks": [hunk_json()]});
    let big_lines: Vec<String> = (0..260).map(|i| if i % 2 == 0 { format!("+line {i}") } else { format!("-old {i}") }).collect();
    let big_hunks: Vec<Value> = big_lines.chunks(20).enumerate().map(|(k, c)| json!({"old": 100 + k * 30, "new": 100 + k * 30, "head": ".plan", "lines": c})).collect();
    json!({"ev": "diff", "req": 1, "title": "pricing-page vs main", "branch": "pricing-page", "commits": 6, "uncommitted": true, "working": true,
        "files": [
            f("src/pages/pricing.tsx", "M", 30, 12), f("src/data/plans.ts", "A", 41, 0), f("src/components/Plan.tsx", "M", 9, 4),
            f("src/components/Banner.tsx", "D", 0, 26),
            {"path": "src/styles/pricing.css", "status": "M", "add": 130, "del": 130, "abs": "/w/src/styles/pricing.css", "hunks": big_hunks},
            f("tests/pricing.test.tsx", "M", 22, 3), f("package.json", "M", 1, 1),
            {"path": "package-lock.json", "status": "M", "add": 312, "del": 290, "hunks": [hunk_json()]},
            {"path": "public/plans/company.svg", "status": "A", "image": true, "binary": true, "abs": "/w/public/plans/company.svg"},
        ]})
}

fn text(lines: &[Line]) -> String {
    lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().trim_end().to_string()).collect::<Vec<_>>().join("\n")
}

#[test]
fn one_file_shows_its_hunk_with_both_line_numbers() {
    let mut p = panel(Diff::of(&one_file()));
    let out = text(&lines(&mut p, 78, 30, 0));
    let (title, rest) = out.split_once('\n').unwrap();
    assert!(title.starts_with("pricing-page vs main · 1 file +1 −2   ") && title.ends_with("ctrl+g close"), "{out}");
    assert!(rest.starts_with("branch pricing-page · 2 commits"), "{out}");
    assert!(!out.contains("files ▸"), "one file: no list on top\n{out}");
    assert!(out.contains("▾ src/pages/pricing.tsx"), "{out}");
    assert!(out.contains("@@ export function Pricing() @@"), "{out}");
    assert!(out.contains("  38   38   export function Pricing() {"), "{out}");
    assert!(out.contains("  41      −       <Banner text=\"save 20% this week\" />"), "{out}");
    assert!(out.contains("       41 +       <Plan name=\"free\" note=\"for side projects\" />"), "{out}");
    assert!(out.contains("end of the diff · 1 file"), "{out}");
}

#[test]
fn many_files_list_the_first_six_fold_the_big_and_the_generated() {
    let mut p = panel(Diff::of(&many_files()));
    let rows = body_rows(&p, p.diff.as_ref().unwrap(), 78);
    let out = text(&rows.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>());
    let head = text(&head_rows(&p, p.diff.as_ref().unwrap(), 78, None, 0));
    assert!(head.contains("pricing-page vs main · 9 files +545 −466   ∿ still working"), "{head}");
    assert!(head.contains("branch pricing-page · 6 commits + changes not committed yet"), "{head}");
    assert!(out.starts_with("files"), "{out}");
    assert!(out.contains("all 9 files ▸"), "{out}");
    assert!(out.contains("M src/pages/pricing.tsx") && out.contains("D src/components/Banner.tsx"), "{out}");
    assert!(out.contains("  ↓ 3 more"), "{out}");
    assert!(out.contains("more lines in this file · ⏎ shows them"), "{out}");
    assert!(out.contains("▸ package-lock.json · generated, folded"), "{out}");
    assert!(out.contains("▸ public/plans/company.svg · an image, ⏎ opens it"), "{out}");
    // ⏎ on the fold: the whole file
    let fold = rows.iter().position(|(_, k)| matches!(k, Kind::Fold(_))).unwrap();
    p.rows = rows.iter().map(|(_, k)| k.clone()).collect();
    p.cursor = fold;
    p.unfolded.insert("src/styles/pricing.css".into());
    let rows = body_rows(&p, p.diff.as_ref().unwrap(), 78);
    assert!(!rows.iter().any(|(_, k)| matches!(k, Kind::Fold(_))));
    for (l, _) in &rows {
        assert!(l.width() <= 78, "too wide: {:?}", l);
    }
}

#[test]
fn scrolled_the_head_says_which_file() {
    let mut p = panel(Diff::of(&many_files()));
    let _ = lines(&mut p, 78, 20, 0);
    p.cursor = 60;
    let out = text(&lines(&mut p, 78, 20, 0));
    assert!(out.lines().nth(1).unwrap().starts_with("file "), "{out}");
    assert!(out.lines().nth(1).unwrap().contains(" of 9 · "), "{out}");
}

#[test]
fn the_file_list_filters() {
    let mut p = panel(Diff::of(&many_files()));
    p.list = Some(List { filter: String::new(), sel: 0 });
    let out = text(&lines(&mut p, 78, 24, 0));
    assert!(out.contains("› type to filter the files"), "{out}");
    assert!(out.contains("A public/plans/company.svg") && out.contains("binary"), "{out}");
    assert!(out.contains("M changed   A added   D deleted"), "{out}");
    p.list = Some(List { filter: "pricing".into(), sel: 0 });
    let d = p.diff.clone().unwrap();
    assert_eq!(list_matches(&d, "pricing").len(), 3);
}

#[test]
fn the_doors_name_what_they_open() {
    let ask = Ask::Range("a1b2c3d..e4f5a6b".into(), "pricing-page".into());
    assert_eq!(ask_of_url(&url_of(&ask)), Some(ask));
    assert_eq!(ask_of_url("bise-diff:branch/sculpt"), Some(Ask::Branch("sculpt".into())));
    assert_eq!(ask_of_url("bise-diff:pr/7"), Some(Ask::Pr(7)));
    assert_eq!(ask_of_url("https://x"), None);
    assert!(is_lock("web/package-lock.json") && is_lock("Cargo.lock") && !is_lock("src/lock.rs"));
    use crate::diffbranches::{branch_words, Branch};
    let b = Branch { branch: "fix-csv".into(), pr: Some(7), add: 8, del: 2, ..Default::default() };
    assert_eq!(branch_words(&b, 0), "no agent · PR #7 open");
    let b = Branch { branch: "sculpt".into(), agents: vec!["s1".into(), "s2".into()], commits: 4, ..Default::default() };
    assert_eq!(branch_words(&b, 0), "s1, s2 · 4 commits");
    assert_eq!(counts(42, 18), "+42 −18");
}

#[test]
fn an_answer_to_an_older_ask_is_dropped() {
    let v = json!({"req": 99});
    let d = Diff::of(&v);
    assert!(d.files.is_empty());
}

// ---- the focus rules (designer m_7291: a letter is never lost) ----

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ch(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::NONE)
}

/// An app with the panel on the right (`side`) or full screen, with the
/// keys, its rows built.
fn app_with_panel(side: bool) -> App {
    let mut app = crate::sb::bench::test_app();
    let mut p = panel(Diff::of(&many_files()));
    p.side = side;
    let _ = lines(&mut p, 78, 30, 0);
    app.diff = Some(p);
    app
}

#[test]
fn on_the_right_the_panel_has_no_letter_keys() {
    for c in ['a', 'f', 'j', 'k', ']', '[', ' ', 'F', '?', '/', '@'] {
        assert_eq!(panel_key(&ch(c), false), None, "{c:?} types in the composer");
    }
    assert_eq!(panel_key(&key(KeyCode::Tab, KeyModifiers::NONE), false), Some(PanelKey::NextFile));
    assert_eq!(panel_key(&key(KeyCode::BackTab, KeyModifiers::SHIFT), false), Some(PanelKey::PrevFile));
    assert_eq!(panel_key(&key(KeyCode::Esc, KeyModifiers::NONE), false), Some(PanelKey::Close));
    assert_eq!(panel_key(&key(KeyCode::Up, KeyModifiers::NONE), false), Some(PanelKey::Up));
    assert_eq!(panel_key(&key(KeyCode::Enter, KeyModifiers::NONE), false), Some(PanelKey::Enter));
    // shift+⏎ is the composer's newline
    assert_eq!(panel_key(&key(KeyCode::Enter, KeyModifiers::SHIFT), false), None);
    // full screen (no composer) keeps its letters, and tab
    assert_eq!(panel_key(&ch('f'), true), Some(PanelKey::Files));
    assert_eq!(panel_key(&ch('j'), true), Some(PanelKey::Down));
    assert_eq!(panel_key(&ch(']'), true), Some(PanelKey::NextFile));
    assert_eq!(panel_key(&key(KeyCode::Tab, KeyModifiers::NONE), true), Some(PanelKey::NextFile));
}

#[test]
fn a_letter_typed_with_the_panel_focused_goes_to_the_composer() {
    let mut app = app_with_panel(true);
    assert!(has_keys(&app));
    // "fix this": its f must not open the file list
    for c in "fix this".chars() {
        let k = ch(c);
        if !on_key(&mut app, &k) {
            crate::input::composer_key(&mut app, &k);
        }
    }
    assert_eq!(app.ed.text, "fix this");
    let p = app.diff.as_ref().expect("the panel stays open");
    assert!(p.list.is_none() && !p.focused);
    assert!(!has_keys(&app));
    // the composer has the keys: ↑ is the composer's, not the panel's
    let cursor = app.diff.as_ref().unwrap().cursor;
    assert!(!on_key(&mut app, &key(KeyCode::Down, KeyModifiers::NONE)));
    assert_eq!(app.diff.as_ref().unwrap().cursor, cursor);
}

#[test]
fn the_panels_keys_move_it_and_esc_closes_it() {
    let mut app = app_with_panel(true);
    assert!(on_key(&mut app, &key(KeyCode::Tab, KeyModifiers::NONE)));
    let p = app.diff.as_ref().unwrap();
    assert!(matches!(p.rows[p.cursor], Kind::FileHead(0)), "tab: the first file");
    assert!(on_key(&mut app, &key(KeyCode::Tab, KeyModifiers::NONE)));
    let p = app.diff.as_ref().unwrap();
    assert!(matches!(p.rows[p.cursor], Kind::FileHead(1)), "tab: the next file");
    assert!(on_key(&mut app, &key(KeyCode::BackTab, KeyModifiers::SHIFT)));
    let p = app.diff.as_ref().unwrap();
    assert!(matches!(p.rows[p.cursor], Kind::FileHead(0)), "shift+tab: back");
    assert!(p.focused);
    assert!(on_key(&mut app, &key(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(app.diff.is_none(), "esc closes the panel that has the keys");
}

#[test]
fn esc_with_the_composer_focused_is_the_composers() {
    let mut app = app_with_panel(true);
    app.diff.as_mut().unwrap().focused = false;
    assert!(!on_key(&mut app, &key(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(app.diff.is_some());
    // ctrl+g closes it from anywhere
    assert!(on_key(&mut app, &key(KeyCode::Char('g'), KeyModifiers::CONTROL)));
    assert!(app.diff.is_none());
}

#[test]
fn the_file_list_keeps_its_letters() {
    let mut app = app_with_panel(true);
    app.diff.as_mut().unwrap().list = Some(List::default());
    assert!(on_key(&mut app, &ch('c')));
    assert_eq!(app.diff.as_ref().unwrap().list.as_ref().unwrap().filter, "c");
    assert!(app.ed.text.is_empty());
    assert!(on_key(&mut app, &key(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(app.diff.as_ref().is_some_and(|p| p.list.is_none() && p.focused), "esc: back to the diff");
}

#[test]
fn full_screen_keeps_every_key() {
    let mut app = app_with_panel(false);
    assert!(on_key(&mut app, &ch('x')));
    assert!(app.ed.text.is_empty());
    assert!(on_key(&mut app, &ch('f')));
    assert!(app.diff.as_ref().unwrap().list.is_some());
}

#[test]
fn a_paste_or_a_click_out_of_it_gives_the_keys_back() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let mut app = app_with_panel(true);
    crate::input::on_paste(&mut app, "hello");
    assert_eq!(app.ed.text, "hello");
    assert!(!has_keys(&app));
    let p = app.diff.as_mut().unwrap();
    p.area = Rect { x: 70, y: 0, width: 80, height: 30 };
    p.body = Rect { x: 71, y: 3, width: 78, height: 27 };
    let click = |x, y| MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: x, row: y, modifiers: KeyModifiers::NONE };
    assert!(mouse(&mut app, &click(80, 1)), "a click in it");
    assert!(has_keys(&app), "takes the keys");
    assert!(!mouse(&mut app, &click(10, 35)), "a click in the composer goes on");
    assert!(!has_keys(&app), "and gives them back");
}

#[test]
fn a_click_door_leaves_the_keys_to_the_composer() {
    let mut app = crate::sb::bench::test_app();
    request(&mut app, Ask::Branch("sb/t1".into()), By::Click);
    assert!(!app.diff.as_ref().unwrap().focused);
    request(&mut app, Ask::Branch("sb/t1".into()), By::Key);
    assert!(app.diff.as_ref().unwrap().focused);
}

#[test]
fn the_title_row_says_ctrl_g_close_and_the_list_its_count() {
    let mut p = panel(Diff::of(&many_files()));
    p.focused = false;
    let out = text(&lines(&mut p, 78, 30, 0));
    let first = out.lines().next().unwrap();
    assert!(first.ends_with("ctrl+g close"), "{first}");
    assert!(first.chars().count() <= 76, "{first}");
    assert!(out.contains("all 9 files ▸"), "{out}");
    // full screen: esc closes, the key bar says it
    p.side = false;
    let out = text(&lines(&mut p, 78, 30, 0));
    assert!(!out.contains("ctrl+g close"), "{out}");
}

#[test]
fn the_key_bar_says_where_the_keys_go() {
    let mut app = app_with_panel(true);
    app.diff.as_mut().unwrap().cursor = 0;
    let bar = |app: &App| crate::keybar::line(app, 150).spans.iter().map(|s| s.content.to_string()).collect::<String>();
    assert_eq!(bar(&app), "↑↓ scroll   tab next file   ⏎ open in your editor   esc close   type to write");
    app.diff.as_mut().unwrap().focused = false;
    let b = bar(&app);
    assert!(b.ends_with("   ctrl+g close") && !b.contains("tip"), "{b}");
}

// ---- a landed door: that land, never the branch vs today's main ----

#[test]
fn a_land_is_titled_by_its_agent_and_its_commit() {
    assert_eq!(range_title("a1b2c3d4..e0f3df59aa", "diff-focus"), "diff-focus landed on main · e0f3df5");
    assert_eq!(range_title("a1b2..e0f3", ""), "a1b2..e0f3");
}

#[test]
fn the_last_land_of_an_agent_is_its_newest_landed_line() {
    use crate::wire::Ev;
    let land = |a: &str, from: &str, sha: &str| Ev::Landed { agent: a.into(), from: from.into(), sha: sha.into(), files: 3, add: 1, del: 1 };
    let events = vec![land("t1", "aaa", "bbb"), land("t2", "ccc", "ddd"), land("t1", "bbb", "eee")];
    assert_eq!(last_land(&events, "t1"), Some(Ask::Range("bbb..eee".into(), "t1".into())));
    assert_eq!(last_land(&events, "t3"), None);
}

#[test]
fn an_empty_diff_of_an_agent_that_landed_offers_its_last_land() {
    let empty = json!({"ev": "diff", "req": 1, "title": "pricing-page vs main", "branch": "pricing-page", "files": []});
    let mut p = panel(Diff::of(&empty));
    let out = text(&lines(&mut p, 78, 20, 0));
    assert!(out.contains("no changes against main"), "never landed: {out}");
    p.last_land = Some(Ask::Range("aaa..bbb".into(), "pricing-page".into()));
    let out = text(&lines(&mut p, 78, 20, 0));
    assert!(out.contains("pricing-page's work is all on main already · show what it landed last"), "{out}");
    assert!(!out.contains("no changes"), "{out}");
    // ⏎ on it: that land's range, with the keys
    let mut app = crate::sb::bench::test_app();
    app.diff = Some(p);
    assert_eq!(key_pairs(&app)[0], ("⏎", "show what it landed last".to_string()));
    assert!(on_key(&mut app, &key(KeyCode::Enter, KeyModifiers::NONE)));
    let p = app.diff.as_ref().unwrap();
    assert_eq!(p.ask, Ask::Range("aaa..bbb".into(), "pricing-page".into()));
    assert_eq!(p.what, "pricing-page landed on main · bbb");
    assert!(p.focused);
}

// ---- an agent whose folder is gone: one plain line, never git's fatal ----

#[test]
fn a_gone_folder_is_one_dim_line_that_offers_the_last_land() {
    let gone = json!({"ev": "diff", "req": 1, "title": "diff-focus vs main", "files": [], "gone": true,
                      "note": "diff-focus is archived and its folder is gone"});
    let mut p = panel(Diff::of(&gone));
    let out = text(&lines(&mut p, 78, 20, 0));
    assert!(out.contains("diff-focus is archived and its folder is gone"), "{out}");
    assert!(!out.contains("show what") && !out.contains("▲") && !out.contains("no changes"), "no land: {out}");
    p.last_land = Some(Ask::Range("aaa..bbb".into(), "diff-focus".into()));
    let out = text(&lines(&mut p, 78, 20, 0));
    assert!(out.contains("diff-focus is archived and its folder is gone · show what it landed last"), "{out}");
    // its title row ends with `ctrl+g close`, like every title row (designer m_7487)
    let rows = lines(&mut p, 78, 20, 0);
    assert!(text(&rows[..1]).trim_end().ends_with("ctrl+g close"), "{out}");
    let mut app = crate::sb::bench::test_app();
    app.diff = Some(p);
    assert_eq!(key_pairs(&app)[0], ("⏎", "show what it landed last".to_string()));
    assert!(on_key(&mut app, &key(KeyCode::Enter, KeyModifiers::NONE)));
    assert_eq!(app.diff.as_ref().unwrap().ask, Ask::Range("aaa..bbb".into(), "diff-focus".into()));
}

#[test]
fn a_git_failure_is_marked_and_dim() {
    let bad = json!({"ev": "diff", "req": 1, "title": "abc..def", "files": [], "error": "git couldn't read this diff: bad revision 'abc..def'"});
    let mut p = panel(Diff::of(&bad));
    let out = text(&lines(&mut p, 78, 20, 0));
    assert!(out.contains("▲ git couldn't read this diff: bad revision 'abc..def'"), "{out}");
    assert!(text(&lines(&mut p, 78, 20, 0)[..1]).trim_end().ends_with("ctrl+g close"), "{out}");
}
