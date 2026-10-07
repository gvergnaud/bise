use super::*;
use std::io::Write;

const DAY: u64 = 86_400_000;
const NOW: u64 = 1_790_700_000_000;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sb-search-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn append(agents: &Path, dir: &str, lines: &[(u64, &str)]) {
    std::fs::create_dir_all(agents.join(dir)).unwrap();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(agents.join(dir).join("transcript.log"))
        .unwrap();
    for (ms, l) in lines {
        writeln!(f, "{}\t{}", ms, l).unwrap();
    }
}

fn who() -> Vec<Who> {
    vec![
        Who { name: "main".into(), dir: "main".into(), aliases: vec![], archived: false },
        Who { name: "feed".into(), dir: "feed-2".into(), aliases: vec!["old-feed".into()], archived: true },
    ]
}

fn q(text: &str) -> Query {
    Query { text: text.into(), limit: DEFAULT_HITS, page: 1, ..Query::default() }
}

/// main: a user message two weeks ago, a compaction, then more; an
/// archived task (dir `feed-2`) that did the work.
fn fixture(name: &str) -> (PathBuf, Index) {
    let agents = tmp(name);
    let old = NOW - 14 * DAY - 3_600_000;
    append(
        &agents,
        "main",
        &[
            (old, "sb you : Peux-tu refaire le diviseur façon rafale ?"),
            (old + 1, "  obs: turn_started"),
            (old + 2, "  obs: assistant: <think>secret plan</think>I asked feed to redo the divider (m_12, commit 685220f)."),
            (old + 3, "sb msg : main → feed : redo the gust divider"),
            (old + 4, "tool #3 bash : git log --oneline -3"),
            (old + 5, "tool_result #3 ok : 685220f divider gust"),
            (NOW - DAY, "  obs: compaction_started #40 (auto)"),
            (NOW - DAY, "  obs: compaction_done: ## Summary the user wants a calmer UI"),
            (NOW - 1000, "sb you : and now the cards"),
        ],
    );
    append(
        &agents,
        "feed-2",
        &[
            (old + 10, "sb msg-in : main m_12 : redo the gust divider"),
            (old + 20, "  obs: assistant: Done: the divider is a gust now (685220f)."),
        ],
    );
    let mut ix = Index::default();
    ix.refresh(&agents);
    (agents, ix)
}

#[test]
fn roles_of_lines() {
    let r = |l: &str| classify(l).map(|x| x.0);
    assert_eq!(r("sb you : hi"), Some(Role::User));
    assert_eq!(r("  obs: assistant: hello"), Some(Role::Assistant));
    assert_eq!(r("  obs: assistant: <think>only thinking</think>"), None);
    assert_eq!(r("sb msg-in : main m_1 : x"), Some(Role::Message));
    assert_eq!(r("sb msg : main → a : x"), Some(Role::Message));
    assert_eq!(r("tool #2 bash : ls"), Some(Role::Tool));
    assert_eq!(r("tool_result #2 ok : a b"), Some(Role::Tool));
    assert_eq!(r("sb spawn : main → nouvelle tâche @a : x"), Some(Role::Hub));
    assert_eq!(r("  obs: compaction_done: sum"), Some(Role::Hub));
    assert_eq!(r("  obs: turn_started"), None);
    assert_eq!(r("tool_code #2 : ls"), None);
}

#[test]
fn terms_fold_case_accents_and_keep_phrases() {
    assert_eq!(terms_of("Façon  \"Gust Divider\" É"), vec!["facon", "gust divider", "e"]);
    assert_eq!(fold("Déjà VU"), "deja vu");
    assert!(terms_of("  ").is_empty());
}

#[test]
fn finds_a_message_from_before_a_compaction_and_one_of_an_archived_task() {
    let (_d, ix) = fixture("compaction");
    let out = ix.search(&who(), &q("facon rafale"), NOW).unwrap();
    assert!(out.contains("main#1 · 14j ago · user: Peux-tu refaire le diviseur façon rafale ?"), "{}", out);
    let out = ix.search(&who(), &q("gust now"), NOW).unwrap();
    assert!(out.contains("feed#2 · 14j ago · archived · assistant: Done: the divider is a gust now"), "{}", out);
    // the compaction summary is searchable too, as a hub line
    let out = ix.search(&who(), &q("calmer"), NOW).unwrap();
    assert!(out.contains("main#8") && out.contains("compaction summary:"), "{}", out);
    // thinking is not indexed
    assert!(ix.search(&who(), &q("secret"), NOW).unwrap().starts_with("no match"));
}

#[test]
fn ranks_user_and_assistant_first_and_dedups_messages() {
    let (_d, ix) = fixture("rank");
    let out = ix.search(&who(), &q("685220f"), NOW).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines[0].starts_with("3 hits for \"685220f\""), "{}", out);
    // assistant hits (newest first), then the tool result
    assert!(lines[1].starts_with("feed#2"), "{}", out);
    assert!(lines[2].starts_with("main#3"), "{}", out);
    assert!(lines[3].starts_with("main#6") && lines[3].contains("result: ok : 685220f"), "{}", out);
    // the message is in both threads: one hit
    let out = ix.search(&who(), &q("redo the gust"), NOW).unwrap();
    assert!(out.starts_with("1 hit for"), "{}", out);
}

#[test]
fn filters_agent_role_time_archived() {
    let (_d, ix) = fixture("filters");
    let w = who();
    let run = |f: &dyn Fn(&mut Query)| {
        let mut x = q("divider");
        f(&mut x);
        ix.search(&w, &x, NOW).unwrap()
    };
    let heads = |s: String| s.lines().skip(1).filter(|l| !l.starts_with("--")).map(|l| l.split(' ').next().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(heads(run(&|x| x.agents = vec!["old-feed".into()])), vec!["feed#2", "feed#1"]);
    assert_eq!(heads(run(&|x| x.roles = vec![Role::Tool])), vec!["main#6"]);
    assert_eq!(heads(run(&|x| x.archived = Archived::No)), vec!["main#3", "main#4", "main#6"]);
    assert_eq!(heads(run(&|x| x.archived = Archived::Only)), vec!["feed#2", "feed#1"]);
    assert!(run(&|x| x.since = Some(NOW - 2 * DAY)).starts_with("no match"));
    assert!(run(&|x| x.until = Some(NOW - 2 * DAY)).starts_with("4 hits"));
    let mut x = q("divider");
    x.agents = vec!["nobody".into()];
    assert_eq!(ix.search(&w, &x, NOW), Err("no agent named nobody".into()));
    assert!(ix.search(&w, &q(" "), NOW).is_err());
}

#[test]
fn pages_stay_under_the_budget() {
    let agents = tmp("budget");
    let long = format!("sb you : needle {}", "blah ".repeat(2000));
    let lines: Vec<(u64, &str)> = (0..500).map(|i| (NOW - i, long.as_str())).collect();
    append(&agents, "main", &lines);
    let mut ix = Index::default();
    ix.refresh(&agents);
    let mut x = q("needle");
    x.limit = 1000;
    let out = ix.search(&who(), &x, NOW).unwrap();
    assert!(out.len() <= BUDGET + 400, "{}", out.len());
    assert!(out.starts_with("500 hits for \"needle\" --limit 1000 (page 1/25"), "{}", &out[..200]);
    assert!(out.contains("next: sb history \"needle\" --limit 1000 --page 2"), "{}", out);
    x.page = 99;
    let last = ix.search(&who(), &x, NOW).unwrap();
    assert!(last.contains("(page 25/25") && !last.contains("next:"));
}

#[test]
fn refresh_reads_only_whole_new_lines() {
    let (agents, mut ix) = fixture("incr");
    let before = ix.stats();
    let path = agents.join("main").join("transcript.log");
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    write!(f, "{}\tsb you : a fresh zebra", NOW).unwrap();
    ix.refresh(&agents);
    assert!(ix.search(&who(), &q("zebra"), NOW).unwrap().starts_with("no match"));
    writeln!(f).unwrap();
    ix.refresh(&agents);
    let out = ix.search(&who(), &q("zebra"), NOW).unwrap();
    assert!(out.contains("main#10 · 0s ago · user: a fresh zebra"), "{}", out);
    assert_eq!(ix.stats().docs, before.docs + 1);
    // a new thread appears
    append(&agents, "late", &[(NOW, "sb you : zebra again")]);
    ix.refresh(&agents);
    assert!(ix.search(&who(), &q("zebra"), NOW).unwrap().contains("late#1 · 0s ago · archived"));
}

#[test]
fn show_opens_a_hit_with_neighbors_and_links() {
    let (_d, ix) = fixture("show");
    let out = ix.show(&who(), "main", 3, 1, NOW).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines[0].starts_with("main's thread, #3 ("), "{}", out);
    assert!(lines[0].ends_with("UTC, assistant):"), "{}", out);
    assert!(lines[1].starts_with("#1 · 14j ago · user: Peux-tu"), "{}", out);
    assert!(lines[2].starts_with(">> #3 · 14j ago · assistant: I asked feed"), "{}", out);
    assert!(lines[3].starts_with("#4 "), "{}", out);
    let foot = lines[4];
    assert!(!foot.contains("earlier:"), "{}", foot);
    assert!(foot.contains("later: sb show main#5"), "{}", foot);
    assert!(foot.contains("agents: feed"), "{}", foot);
    assert!(foot.contains("messages: m_12"), "{}", foot);
    assert!(foot.contains("commits: 685220f"), "{}", foot);
    // a position between entries opens the next one; by old name too
    assert!(ix.show(&who(), "old-feed", 2, 0, NOW).unwrap().contains(">> #2 "));
    assert!(ix.show(&who(), "main", 2, 0, NOW).unwrap().contains(">> #3 "));
    assert!(ix.show(&who(), "main", 99, 0, NOW).is_err());
    assert!(ix.show(&who(), "nobody", 1, 0, NOW).is_err());
}

#[test]
fn show_stays_under_the_budget() {
    let agents = tmp("show-budget");
    let long = format!("sb you : {}", "word ".repeat(3000));
    let lines: Vec<(u64, &str)> = (0..30).map(|i| (NOW - i, long.as_str())).collect();
    append(&agents, "main", &lines);
    let mut ix = Index::default();
    ix.refresh(&agents);
    let out = ix.show(&who(), "main", 15, 10, NOW).unwrap();
    assert!(out.len() <= BUDGET + 2500, "{}", out.len());
    assert!(out.contains("whole entry: sb inspect main --at #15"));
}

#[test]
fn times_and_refs() {
    assert_eq!(parse_time("2w", NOW), Ok(NOW - 14 * DAY));
    assert_eq!(parse_time("3j", NOW), Ok(NOW - 3 * DAY));
    assert_eq!(parse_time("90m", NOW), Ok(NOW - 90 * 60_000));
    let t = parse_time("2026-09-30T14:03", NOW).unwrap();
    assert_eq!(fmt_iso(t), "2026-09-30T14:03");
    assert_eq!(fmt_iso(parse_time("1970-01-01", NOW).unwrap()), "1970-01-01T00:00");
    assert_eq!(fmt_iso(parse_time("2024-02-29", NOW).unwrap()), "2024-02-29T00:00");
    assert!(parse_time("yesterday", NOW).is_err());
    assert!(parse_time("2026-13-01", NOW).is_err());
    let a = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(parse_ref(&a(&["main#12"])), Some(("main".into(), 12)));
    assert_eq!(parse_ref(&a(&["@feed", "#7"])), Some(("feed".into(), 7)));
    assert_eq!(parse_ref(&a(&["feed", "7"])), Some(("feed".into(), 7)));
    assert_eq!(parse_ref(&a(&["feed"])), None);
}

/// Perf on a real hub state, read-only:
/// `SB_SEARCH_BENCH=<state>/agents cargo test -p switchboard --release bench_real -- --ignored --nocapture`
#[test]
#[ignore]
fn bench_real() {
    let Some(dir) = bise_home::env::test_setting("SB_SEARCH_BENCH") else { return };
    let dir = PathBuf::from(dir);
    let t = std::time::Instant::now();
    let mut ix = Index::default();
    ix.refresh(&dir);
    let cold = t.elapsed();
    let s = ix.stats();
    let t = std::time::Instant::now();
    ix.refresh(&dir);
    let warm_refresh = t.elapsed();
    println!(
        "threads {} docs {} transcript {:.1} MB folded {:.1} MB; cold build {:?}; refresh with nothing new {:?}",
        s.threads,
        s.docs,
        s.bytes as f64 / 1e6,
        s.folded_bytes as f64 / 1e6,
        cold,
        warm_refresh
    );
    let mut per: BTreeMap<Role, (usize, usize)> = BTreeMap::new();
    for t in ix.threads.values() {
        for d in &t.docs {
            let e = per.entry(d.role).or_default();
            e.0 += 1;
            e.1 += d.folded.len();
        }
    }
    println!("per role (docs, folded bytes): {:?}", per);
    for query in ["compaction", "divider gust", "\"sb history\"", "zzzqqqxx", "the"] {
        let t = std::time::Instant::now();
        let out = ix.search(&[], &q(query), NOW).unwrap();
        println!("{:>16}: {:?}, {} bytes, {}", query, t.elapsed(), out.len(), out.lines().next().unwrap_or(""));
    }
}

// stream C (S2 step 3): other projects' threads in one index, keyed
// `<p>/<dir>`; --project scopes it, --all ranks everything together, hits
// read `<p>/<agent>#<pos>` and the page line keeps the flag
#[test]
fn other_projects_share_one_index_and_one_ranking() {
    let shop = tmp("xshop");
    let docs = tmp("xdocs");
    append(&shop, "perf", &[(NOW - 2000, "sb you : why does p99 rise")]);
    append(&shop, "perf", &[(NOW - 1500, "  obs: assistant: p99 rises from the cold cache")]);
    append(&docs, "main", &[(NOW - 1000, "sb you : p99 docs page")]);
    let mut ix = Index::default();
    ix.refresh_as(&shop, "shop");
    ix.refresh_as(&docs, "docs");
    let who = vec![
        Who { name: "shop/perf".into(), dir: "shop/perf".into(), aliases: vec!["shop/speed".into()], archived: false },
        Who { name: "docs/main".into(), dir: "docs/main".into(), aliases: vec![], archived: false },
    ];
    let all = ix.search(&who, &Query { all: true, ..q("p99") }, NOW).unwrap();
    assert!(all.starts_with("3 hits") && all.contains("--all"), "{all}");
    assert!(all.contains("docs/main#1") && all.contains("shop/perf#1") && all.contains("shop/perf#2"), "{all}");
    let one = ix.search(&who, &Query { scope: Some("shop".into()), limit: 1, ..q("p99") }, NOW).unwrap();
    assert!(one.starts_with("2 hits") && !one.contains("docs/"), "{one}");
    assert!(one.contains("--project shop --limit 1 --page 2"), "{one}");
    // an old name of a project's agent finds its thread
    let by = ix.search(&who, &Query { agents: vec!["shop/speed".into()], ..q("p99") }, NOW).unwrap();
    assert!(by.starts_with("2 hits"), "{by}");
    assert!(ix.show(&who, "shop/perf", 2, 1, NOW).unwrap().contains("cold cache"));
    // a project that left the registry: its threads go
    ix.retain_prefixes(&["shop".into()]);
    assert!(ix.search(&who, &Query { all: true, ..q("p99") }, NOW).unwrap().starts_with("2 hits"));
    let _ = std::fs::remove_dir_all(&shop);
    let _ = std::fs::remove_dir_all(&docs);
}
