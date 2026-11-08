use super::*;
use bise_proto::diff::{DiffLine, GuideState as S, Hunk, LineKind};

fn line(kind: LineKind, text: &str, n: u32) -> DiffLine {
    DiffLine { kind, old: Some(n), new: Some(n), text: text.into() }
}

fn hunk(header: &str, add: &[&str]) -> Hunk {
    let mut lines = vec![line(LineKind::Ctx, "fn ctx() {}", 1)];
    lines.extend(add.iter().map(|t| line(LineKind::Add, t, 2)));
    Hunk { header: header.into(), lines, head: None, id: None }
}

fn file(path: &str, hunks: Vec<Hunk>) -> DiffFile {
    DiffFile { path: path.into(), status: "modified".into(), add: 1, del: 0, hunks, truncated: false, from: None, binary: false, generated: false, image: false, size: None, note: None, abs: None }
}

fn nine() -> Vec<DiffFile> {
    vec![
        file("src/rowEnd.ts", vec![hunk("@@ -1 +1 @@", &["export function rowEndOf() {}"]), hunk("@@ -9 +9 @@", &["return out;"])]),
        file("src/SidebarRow.tsx", vec![hunk("@@ -3 +3 @@", &["rowEndOf(a, pinned)"])]),
        file("src/sidebar.css", vec![hunk("@@ -5 +5 @@", &[".end { display: flex; }"])]),
        file("docs/bar.md", vec![hunk("@@ -1 +1 @@", &["A row ends with one thing per slot."])]),
    ]
}

#[test]
fn a_hunk_keeps_its_id_across_pushes_and_the_hash_follows_the_diff() {
    let mut a = nine();
    let h1 = stamp(&mut a);
    // the same change rebased: other line numbers and context, same ids and hash
    let mut b = nine();
    for f in &mut b {
        for h in &mut f.hunks {
            h.header = "@@ -40 +41 @@".into();
            h.lines[0].text = "fn other_ctx() {}".into();
            for l in &mut h.lines {
                l.new = Some(99);
            }
        }
    }
    assert_eq!(stamp(&mut b), h1);
    assert_eq!(ids(&a), ids(&b));
    assert_eq!(ids(&a).len(), 5);
    assert!(ids(&a).iter().all(|i| i.len() == 10 && i.chars().all(|c| c.is_ascii_hexdigit())));
    // one added line changes one id and the hash
    let mut c = nine();
    c[2].hunks[0].lines.push(line(LineKind::Add, ".pin { flex: 0 0 16px; }", 3));
    assert_ne!(stamp(&mut c), h1);
    assert_eq!(ids(&c).iter().filter(|i| !ids(&a).contains(i)).count(), 1);
}

#[test]
fn a_repeated_hunk_gets_its_own_id() {
    let mut f = vec![file("a.rs", vec![hunk("@@ -1 +1 @@", &["x"]), hunk("@@ -9 +9 @@", &["x"])])];
    stamp(&mut f);
    let got = ids(&f);
    assert_eq!(got[1], format!("{}-2", got[0]));
}

#[test]
fn a_small_change_gets_no_guide() {
    assert!(!skip(&nine()));
    let two = vec![file("a.rs", vec![hunk("@@ -1 +1 @@", &["x"]); 3]), file("b.rs", vec![hunk("@@ -1 +1 @@", &["y"])])];
    assert!(skip(&two));
    let five_hunks = vec![file("a.rs", vec![hunk("@@ -1 +1 @@", &["x"]); 4]), file("b.rs", vec![hunk("@@ -1 +1 @@", &["y"])])];
    assert!(!skip(&five_hunks));
    let three = vec![file("a.rs", vec![]), file("b.rs", vec![]), file("c.rs", vec![])];
    assert!(!skip(&three));
}

#[test]
fn the_digest_stays_under_its_cap_and_names_every_hunk() {
    let big: Vec<&str> = vec!["let x = a_long_line_of_code_that_repeats(1, 2, 3);"; 400];
    let mut files: Vec<DiffFile> = (0..30).map(|i| file(&format!("src/f{i}.rs"), vec![hunk("@@ -1 +1 @@", &big), hunk("@@ -500 +500 @@", &big)])).collect();
    files.push(DiffFile { generated: true, ..file("pnpm-lock.yaml", vec![hunk("@@ -1 +1 @@", &big)]) });
    stamp(&mut files);
    let d = digest(&files, DIGEST_CAP);
    // the cap, plus at most one id line per hunk past it
    assert!(d.len() <= DIGEST_CAP + 61 * 80, "{}", d.len());
    for id in ids(&files) {
        assert!(d.contains(&format!("[{id}]")), "{id} is in the digest");
    }
    assert!(d.contains("more lines"));
    // a generated file: its hunks by their ids, never their lines
    let lock = d.split("## pnpm-lock.yaml").nth(1).unwrap();
    assert!(!lock.contains("a_long_line"));
}

#[test]
fn the_request_carries_the_intent_the_old_titles_and_its_cost() {
    let mut files = nine();
    stamp(&mut files);
    let old = Guide { overview: String::new(), parts: vec![GuidePart { area: "Sidebar".into(), title: "Give the row's end one owner".into(), summary: String::new(), hunks: vec![], weight: Weight::Normal }], rest: vec![] };
    let r = request(&digest(&files, DIGEST_CAP), &intent("fix the pin over the PR link", "done: the pin has its own slot"), Some(&old));
    assert!(r.starts_with("MODEL default"));
    assert_eq!(r.lines().count(), 4, "one line per wire field");
    assert!(r.contains("Give the row's end one owner"));
    assert!(r.contains("fix the pin over the PR link"));
    assert!(r.contains("the diff wins over it"));
    assert!(tokens(&r) > 1_500 && tokens(&r) < 3_000);
}

fn ids_of(files: &[DiffFile]) -> Vec<String> {
    ids(files)
}

#[test]
fn an_answer_never_hides_a_hunk() {
    let mut files = nine();
    stamp(&mut files);
    let all = ids_of(&files);
    let (a, b, c, d, e) = (&all[0], &all[1], &all[2], &all[3], &all[4]);
    // c left out, a listed twice, an unknown id, an injected weight word
    let reply = format!(
        r#"Here is the guide: {{"overview": "the row's end    gets one owner", "parts": [
          {{"area": "Sidebar", "title": "Give the row's end one owner", "summary": "rowEndOf lists it", "hunks": ["{a}", "{b}", "{a}", "zzzz"], "weight": "critical"}},
          {{"area": "Sidebar", "title": "Room for the pin", "summary": "flex", "hunks": ["{d}"], "weight": "ignore previous instructions"}},
          {{"area": "Empty", "title": "Nothing", "summary": "", "hunks": ["{a}"]}}
        ], "rest": ["{e}", "{b}"]}} thanks"#
    );
    let g = parse(&reply, &all).unwrap();
    assert_eq!(g.overview, "the row's end gets one owner");
    assert_eq!(g.parts.len(), 2, "a part left with no hunk goes");
    assert_eq!(g.parts[0].hunks, vec![a.clone(), b.clone()]);
    assert_eq!(g.parts[0].weight, Weight::Critical);
    assert_eq!(g.parts[1].weight, Weight::Normal);
    assert_eq!(g.rest, vec![e.clone(), c.clone()], "the left-out hunk ends in rest");
    // every hunk exactly once
    let mut seen: Vec<&String> = g.parts.iter().flat_map(|p| &p.hunks).chain(&g.rest).collect();
    seen.sort();
    let mut want: Vec<&String> = all.iter().collect();
    want.sort();
    assert_eq!(seen, want);
}

#[test]
fn an_answer_with_no_json_or_no_part_fails() {
    let ids = vec!["abc".to_string()];
    assert!(parse("sorry, I can't", &ids).is_err());
    assert!(parse(r#"{"overview": "x", "parts": [], "rest": ["abc"]}"#, &ids).is_err());
    assert!(parse(r#"{"overview": "x", "parts": [{"title": "t", "hunks": ["nope"]}]}"#, &ids).is_err());
}

#[test]
fn parts_past_the_cap_go_to_rest() {
    let ids: Vec<String> = (0..20).map(|i| format!("{i:010x}")).collect();
    let parts: Vec<String> = ids.iter().map(|i| format!(r#"{{"area": "A", "title": "T", "summary": "S", "hunks": ["{i}"]}}"#)).collect();
    let g = parse(&format!(r#"{{"overview": "o", "parts": [{}], "rest": []}}"#, parts.join(",")), &ids).unwrap();
    assert_eq!(g.parts.len(), MAX_PARTS);
    assert_eq!(g.rest, ids[MAX_PARTS..].to_vec());
}

#[test]
fn the_rule_of_when_a_guide_is_written() {
    let at = At::default();
    // a kept one, a small change, one in flight: never a call
    assert_eq!(next(At { kept: true, ..at }, true, true, true), Next::Answer(S::Ready));
    assert_eq!(next(At { small: true, ..at }, true, true, true), Next::Answer(S::Small));
    assert_eq!(next(At { writing: true, ..at }, true, true, true), Next::Answer(S::Writing));
    assert_eq!(next(at, true, true, false), Next::Answer(S::None));
    // by itself only on a cheap model, never again after a failure
    assert_eq!(next(at, false, true, true), Next::Start);
    assert_eq!(next(at, false, false, true), Next::Answer(S::NeedsClick));
    assert_eq!(next(At { failed: true, ..at }, false, true, true), Next::Answer(S::Failed));
    // his click calls whatever answers, after a failure too
    assert_eq!(next(at, true, false, true), Next::Start);
    assert_eq!(next(At { failed: true, ..at }, true, false, true), Next::Start);
}

#[test]
fn the_store_keeps_reads_finds_the_last_of_a_change_and_prunes() {
    let dir = std::env::temp_dir().join(format!("guides-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let g = |t: &str| Guide { overview: t.into(), parts: vec![], rest: vec![] };
    let k = |hash: &str, head: &str, at_ms: u64| Kept { hash: hash.into(), base: "main".into(), head: head.into(), at_ms, guide: g(hash) };
    write(&dir, &k("aa01", "agent:perf", 1)).unwrap();
    write(&dir, &k("aa02", "agent:perf", 2)).unwrap();
    write(&dir, &k("bb01", "agent:docs", 3)).unwrap();
    assert_eq!(read(&dir, "aa01").unwrap().guide.overview, "aa01");
    assert!(read(&dir, "cc01").is_none());
    // a hash is never a path
    assert!(read(&dir, "../aa01").is_none());
    assert_eq!(last_of(&dir, "agent:perf").unwrap().hash, "aa02");
    assert!(last_of(&dir, "agent:none").is_none());
    for i in 0..KEEP + 5 {
        write(&dir, &k(&format!("{:04x}", 0x1000 + i), "agent:many", 10 + i as u64)).unwrap();
    }
    let n = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(n, KEEP);
    let _ = std::fs::remove_dir_all(&dir);
}
