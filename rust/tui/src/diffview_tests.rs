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
        side: true,
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
    assert!(out.starts_with("pricing-page vs main · 1 file +1 −2\nbranch pricing-page · 2 commits"), "{out}");
    assert!(!out.contains("f the whole list"), "one file: no list on top\n{out}");
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
    assert!(out.contains("f the whole list"), "{out}");
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
