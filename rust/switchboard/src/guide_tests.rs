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
    let r = request(&files, &intent("fix the pin over the PR link", "done: the pin has its own slot"), Some(&old));
    assert!(r.starts_with("MODEL default"));
    assert_eq!(r.lines().count(), 4, "one line per wire field");
    assert!(r.contains("Give the row's end one owner"));
    assert!(r.contains("fix the pin over the PR link"));
    assert!(r.contains("the diff wins over it"));
    // the ids to place, last in his message
    let user = r.lines().nth(2).unwrap();
    assert!(user.ends_with(&format!("All hunk ids to place (5): {}", ids(&files).join(" "))), "{user}");
    // the cost counts the second pass it may need: a quarter of the first call
    let first = r.chars().count() / 4 + 1_500;
    assert_eq!(tokens(&r) as usize, first + first / 4);
}

#[test]
fn a_generated_files_hunks_are_never_asked_for() {
    let mut files = nine();
    files.push(DiffFile { generated: true, ..file("pnpm-lock.yaml", vec![hunk("@@ -1 +1 @@", &["lock: 1"])]) });
    stamp(&mut files);
    let lock = ids(&files).pop().unwrap();
    let r = request(&files, "", None);
    assert!(r.contains("All hunk ids to place (5): "));
    assert!(!r.lines().nth(2).unwrap().rsplit("to place").next().unwrap().contains(&lock));
}

#[test]
fn a_diff_past_250_hunks_gets_the_bigger_digest() {
    let mut files: Vec<DiffFile> = (0..26).map(|i| file(&format!("src/f{i}.rs"), (0..10).map(|j| hunk(&format!("@@ -{j} +{j} @@"), &["x"])).collect())).collect();
    assert_eq!(cap(&files[..25]), DIGEST_CAP, "250 hunks");
    assert_eq!(cap(&files), DIGEST_CAP_BIG, "260 hunks");
    assert_eq!(DIGEST_CAP_BIG, 120_000);
    // the request reads it with that cap
    let long: Vec<&str> = vec!["let x = a_long_line_of_code_that_repeats(1, 2, 3);"; 60];
    for f in &mut files {
        for h in &mut f.hunks {
            *h = hunk(&h.header.clone(), &long);
        }
    }
    stamp(&mut files);
    let r = request(&files, "", None);
    assert!(r.len() > DIGEST_CAP + 20_000, "{}", r.len());
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
    assert_eq!(g.parts.len(), 10);
    assert_eq!(g.rest, ids[10..].to_vec());
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

/// A diff of `n` files, one hunk each, hunk i with `sizes[i]` added lines;
/// the last file generated when `lock`.
fn sized(sizes: &[usize], lock: bool) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = sizes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let add: Vec<String> = (0..*n).map(|k| format!("line {i} {k}")).collect();
            file(&format!("src/f{i}.rs"), vec![hunk("@@ -1 +1 @@", &add.iter().map(String::as_str).collect::<Vec<_>>())])
        })
        .collect();
    if lock {
        files.push(DiffFile { generated: true, ..file("pnpm-lock.yaml", vec![hunk("@@ -1 +1 @@", &["a"; 500])]) });
    }
    stamp(&mut files);
    files
}

fn guide(parts: &[&[String]], rest: &[String]) -> Guide {
    Guide {
        overview: "o".into(),
        parts: parts.iter().enumerate().map(|(i, h)| GuidePart { area: format!("A{i}"), title: format!("Idea {i}"), summary: "s".into(), hunks: h.to_vec(), weight: Weight::Normal }).collect(),
        rest: rest.to_vec(),
    }
}

#[test]
fn over_5_percent_of_the_lines_out_of_the_parts_calls_a_second_pass() {
    // 100 lines of code, a 500-line lock file left out of the count
    let files = sized(&[50, 44, 6], true);
    let all = ids(&files);
    let (a, b, c, lock) = (&all[0], &all[1], &all[2], &all[3]);
    // 6 of 100 lines in rest: a second pass on that hunk alone
    let g = guide(&[&[a.clone(), b.clone()]], &[c.clone(), lock.clone()]);
    assert_eq!(second(&g, &files), Some(Second { left: vec![c.clone()], big: None }));
    // 5 of 100: the first answer stands
    let files5 = sized(&[50, 45, 5], true);
    let all5 = ids(&files5);
    let g5 = guide(&[&[all5[0].clone(), all5[1].clone()]], &[all5[2].clone(), all5[3].clone()]);
    assert_eq!(second(&g5, &files5), None);
    // the lock file alone in rest never calls one, however big
    let g = guide(&[&[a.clone(), b.clone(), c.clone()]], std::slice::from_ref(lock));
    assert_eq!(second(&g, &files), None);
    let cv = cover(&g, &files);
    assert_eq!((cv.lines, cv.rest, cv.biggest, cv.parts), (100, 0, 100, 1));
}

#[test]
fn one_part_with_over_half_the_lines_of_a_big_diff_calls_a_second_pass() {
    // 51 hunks: one of 60 lines, fifty of 1
    let mut sizes = vec![60];
    sizes.extend([1; 50]);
    let files = sized(&sizes, false);
    let all = ids(&files);
    let g = guide(&[&all[..30], &all[30..]], &[]);
    let s = second(&g, &files).unwrap();
    assert_eq!(s.big, Some(0));
    assert_eq!(s.left, all[..30].to_vec(), "the big part's hunks, in the diff's order");
    // the same on a diff of 50 hunks: kept
    let files50 = sized(&sizes[..50], false);
    let all50 = ids(&files50);
    assert_eq!(second(&guide(&[&all50[..30], &all50[30..]], &[]), &files50), None);
    // just half: kept
    let even = sized(&[{ 50 }].into_iter().chain([1; 50]).collect::<Vec<_>>(), false);
    let ev = ids(&even);
    assert_eq!(second(&guide(&[&ev[..1], &ev[1..]], &[]), &even), None);
}

#[test]
fn the_second_request_holds_only_the_left_out_hunks_and_the_numbered_titles() {
    let mut sizes = vec![60];
    sizes.extend([1; 50]);
    let files = sized(&sizes, false);
    let all = ids(&files);
    let g = guide(&[&all[..30], &all[30..50]], &all[50..]);
    let s = second(&g, &files).unwrap();
    let r = second_request(&g, &files, &s);
    assert_eq!(r.lines().count(), 4, "one line per wire field");
    assert!(r.contains("2. A1: Idea 1"));
    assert!(!r.contains("1. A0"), "the big part is named apart, not numbered");
    assert!(r.contains("held too much of the change") && r.contains("A0: Idea 0"));
    assert!(r.contains(&format!("[{}]", all[0])) && r.contains(&format!("[{}]", all[50])));
    assert!(!r.contains(&format!("[{}]", all[30])), "a placed hunk stays out");
    let user = r.lines().nth(2).unwrap();
    assert!(user.ends_with(&format!("ids to place (31): {}", s.left.join(" "))));
}

#[test]
fn a_second_answer_places_the_left_out_hunks_each_once() {
    let files = sized(&[10, 10, 10, 10, 10, 10], false);
    let all = ids(&files);
    let g = guide(&[&all[..1], &all[1..2]], &all[2..]);
    let s = second(&g, &files).unwrap();
    assert_eq!(s.left, all[2..].to_vec());
    // one to part 2, two to a new part, one repeated, one unknown, a bad number, one left out
    let reply = format!(
        r#"{{"assign": {{"{}": 2, "{}": "1", "{}": 9, "zzzz": 1, "{}": 2}}, "new": [{{"area": "Tests", "title": "Cover the slot", "summary": "s", "hunks": ["{}", "{}", "{}"], "weight": "critical"}}, {{"title": "Empty", "hunks": ["{}"]}}]}}"#,
        all[3], all[2], all[5], all[0], all[4], all[3], all[4], all[0]
    );
    let m = merge(g, &reply, &s).unwrap();
    assert_eq!(m.parts.len(), 3);
    assert_eq!(m.parts[0].hunks, vec![all[0].clone(), all[2].clone()], "a placed one never moves");
    assert_eq!(m.parts[1].hunks, vec![all[1].clone(), all[3].clone()]);
    assert_eq!((m.parts[2].title.as_str(), m.parts[2].hunks.clone(), m.parts[2].weight), ("Cover the slot", vec![all[4].clone()], Weight::Critical));
    assert_eq!(m.rest, vec![all[5].clone()], "an id it did not place stays in rest");
    let mut seen: Vec<&String> = m.parts.iter().flat_map(|p| &p.hunks).chain(&m.rest).collect();
    seen.sort();
    let mut want: Vec<&String> = all.iter().collect();
    want.sort();
    assert_eq!(seen, want);
}

#[test]
fn a_split_part_keeps_what_the_second_answer_leaves_and_new_parts_stop_at_ten() {
    let mut sizes = vec![30, 30];
    sizes.extend([1; 50]);
    let files = sized(&sizes, false);
    let all = ids(&files);
    // part 1 holds the two big hunks and ten small ones: over half
    let mut nine: Vec<Vec<String>> = vec![all[..12].to_vec()];
    nine.extend((0..8).map(|i| all[12 + i * 5..17 + i * 5].to_vec()));
    let refs: Vec<&[String]> = nine.iter().map(Vec::as_slice).collect();
    let g = guide(&refs, &all[52..]);
    let s = second(&g, &files).unwrap();
    assert_eq!(s.big, Some(0));
    // the first big hunk to a new part, the second to another, the rest unsaid
    let reply = format!(r#"{{"assign": {{}}, "new": [{{"title": "One", "hunks": ["{}"]}}, {{"title": "Two", "hunks": ["{}"]}}, {{"title": "Three", "hunks": ["{}"]}}]}}"#, all[0], all[1], all[2]);
    let m = merge(g, &reply, &s).unwrap();
    assert_eq!(m.parts.len(), 10, "9 parts and one new: the cap");
    assert_eq!(m.parts[9].hunks, vec![all[0].clone()]);
    assert_eq!(m.parts[0].hunks, all[1..12].to_vec(), "what it left stays in the big part, in order");
}

#[test]
fn the_guide_runs_one_call_or_two_never_more() {
    let files = sized(&[50, 44, 6], false);
    let all = ids(&files);
    let first = format!(r#"{{"overview": "o", "parts": [{{"title": "Core", "hunks": ["{}", "{}"]}}], "rest": []}}"#, all[0], all[1]);
    let second_reply = format!(r#"{{"assign": {{"{}": 1}}, "new": []}}"#, all[2]);
    // a fake model: its replies in order, the requests kept
    let mut asked: Vec<String> = vec![];
    let replies = [first.clone(), second_reply];
    let w = run("REQ", &files, |req, _| {
        asked.push(req.to_string());
        Ok(replies[asked.len() - 1].clone())
    })
    .unwrap();
    assert_eq!(asked.len(), 2);
    assert_eq!(asked[0], "REQ");
    assert!(asked[1].contains(&format!("({}): {}", 1, all[2])));
    assert_eq!(w.second, Pass::Merged);
    assert_eq!(w.guide.parts[0].hunks, all);
    assert!(w.guide.rest.is_empty());
    // a failed second pass: the first answer stands, the hunk in rest
    let mut n = 0;
    let w = run("REQ", &files, |_, _| {
        n += 1;
        if n == 1 { Ok(first.clone()) } else { Err("timeout".into()) }
    })
    .unwrap();
    assert_eq!(w.second, Pass::Failed("timeout".into()));
    assert_eq!(w.guide.rest, vec![all[2].clone()]);
    // every hunk placed: one call
    let all_in = format!(r#"{{"overview": "o", "parts": [{{"title": "Core", "hunks": ["{}", "{}", "{}"]}}]}}"#, all[0], all[1], all[2]);
    let mut calls = 0;
    let w = run("REQ", &files, |_, _| {
        calls += 1;
        Ok(all_in.clone())
    })
    .unwrap();
    assert_eq!((calls, w.second), (1, Pass::None));
}
