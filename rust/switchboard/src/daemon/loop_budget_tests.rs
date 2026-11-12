//! The hub loop's budget at his scale (hub-loop m_61114, architect
//! m_61266): the shell the hub runs (daemon/assemble.rs) with 50 live
//! agents, 1300 archived ones, 1300 artifacts and 250 pages on disk and
//! a typed client; a state push, a REPL line and one of his messages,
//! each timed on the loop thread's CPU (a loaded machine does not flake
//! it, work a builder thread does off the loop does not count), median
//! of 5. Each budget is 3x the median measured after 9c7cf1468 (the
//! value in its comment): the artifact rows' 5 s build on the loop
//! (2026-11-11, his messages minutes late) blows it by 1000x.

use super::*;

/// The CPU time this thread used, in ms.
fn cpu_ms() -> f64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: a valid timespec for the call to fill
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    t.tv_sec as f64 * 1000.0 + t.tv_nsec as f64 / 1e6
}

/// The median CPU ms of 5 runs of `f`.
fn median5(mut f: impl FnMut()) -> f64 {
    let mut ms: Vec<f64> = (0..5)
        .map(|_| {
            let t = cpu_ms();
            f();
            cpu_ms() - t
        })
        .collect();
    ms.sort_by(f64::total_cmp);
    ms[2]
}

const LIVE: usize = 50;
const ARCHIVED: usize = 1300;
const ARTIFACTS: usize = 1300;
const PAGES: usize = 250;

/// The journal of his hub's size: every task made, the archived ones
/// archived, the live ones working.
fn journal() -> Vec<Value> {
    let mut evs = Vec::new();
    for i in 0..LIVE + ARCHIVED {
        evs.push(json!({"type": "task_created", "name": format!("t{i}"), "parent": "main", "at_ms": 1 + i as u64,
            "brief": {"objective": format!("task {i}: a brief of a realistic length, a sentence or two of what to do"), "constraints": [], "done_when": "landed", "report_format": null},
            "ws": {"base_commit": null, "branch": null, "dropped": false, "mode": "shared", "path": "", "state": "ready", "with_changes": false}}));
    }
    for i in 0..ARCHIVED {
        evs.push(json!({"type": "lifecycle", "name": format!("t{i}"), "lifecycle": "archived", "reason": "drop"}));
    }
    for i in ARCHIVED..ARCHIVED + LIVE {
        evs.push(json!({"type": "declared", "name": format!("t{i}"), "status": "working", "note": "on it"}));
    }
    evs
}

/// The stores of his size, as plain files, with what his real rows read
/// (architect m_61462): each artifact's target a real file in the
/// workspace, made from the workspace (its `cwd`: a real realpath per
/// row, a stat for its `gone`), two versions; each page its notes.json.
fn stores(state: &Path, ws: &Path) {
    let files = ws.join("files");
    std::fs::create_dir_all(&files).unwrap();
    let cwd = ws.to_string_lossy().to_string();
    for i in 0..ARTIFACTS {
        let d = state.join("artifacts").join(format!("a{i}"));
        std::fs::create_dir_all(&d).unwrap();
        let target = files.join(format!("f{i}.md"));
        std::fs::write(&target, format!("# artifact {i}\n")).unwrap();
        let target = target.to_string_lossy().to_string();
        let meta = json!({"id": format!("a{i}"), "title": format!("artifact {i}"), "kind": "doc", "agent": format!("t{}", i % (LIVE + ARCHIVED)), "by": "agent",
            "created_ms": i, "source": target, "cwd": cwd,
            "versions": [{"v": 1, "at_ms": i, "target": target}, {"v": 2, "at_ms": i + 1, "target": target}]});
        std::fs::write(d.join("meta.json"), meta.to_string()).unwrap();
    }
    for i in 0..PAGES {
        let d = state.join("pages").join(format!("p{i}"));
        std::fs::create_dir_all(&d).unwrap();
        let meta = json!({"id": format!("p{i}"), "title": format!("page {i}"), "agent": format!("t{}", ARCHIVED + i % LIVE), "version": 2, "created_ms": i, "updated_ms": i});
        std::fs::write(d.join("meta.json"), meta.to_string()).unwrap();
        let notes = json!([{"id": "n1", "version": 1, "status": "done", "text": "a note"}, {"id": "n2", "version": 2, "status": "sent", "text": "another"}]);
        std::fs::write(d.join("notes.json"), notes.to_string()).unwrap();
    }
}

/// The shell at his scale, a typed client connected (its other end
/// drained), the artifact rows' first build landed.
fn shell_at_scale(root: &Path) -> (Shell, Receiver<Msg>) {
    let ws = root.join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let paths = Paths { workspace: ws.clone(), state: root.join("state"), worktrees: root.join("wt"), run: root.join("run") };
    std::fs::create_dir_all(&paths.state).unwrap();
    stores(&paths.state, &ws);
    let mut hub = Hub::new(&ws.to_string_lossy());
    let skipped = hub.replay(&journal());
    assert!(skipped.is_empty(), "sb-core skipped {} events", skipped.len());
    let opts = Opts {
        paths: paths.clone(),
        home: bise_home::Home::at(root.join("home")),
        app_root: root.to_path_buf(),
        core_bin: PathBuf::new(),
        jsrt_bin: None,
        exe: root.join("bise"),
        repl_bin: root.join("repl"),
        spawn_env: None,
    };
    let journal = journal_file::Out::new(std::fs::OpenOptions::new().create(true).append(true).open(paths.journal()).unwrap());
    let (tx, rx) = channel();
    let photos = photo_take::off(paths.state.join("photo"));
    let mut sh = Shell::assemble(assemble::Parts { opts, hub, tx, journal, photos, proc_hub: "loop-budget".into() });
    let (ours, theirs) = UnixStream::pair().unwrap();
    std::thread::spawn(move || {
        let mut r = theirs;
        let _ = std::io::copy(&mut r, &mut std::io::sink());
    });
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"proto": bise_proto::PROTO, "client": {"name": "budget", "version": "0", "kind": "desktop"}}});
    sh.rpc_new(1, ours, init, "budget".into());
    // the artifact rows: the first ask starts their build, its result lands
    sh.push_state();
    land_art_rows(&mut sh, &rx);
    (sh, rx)
}

/// Waits for the artifact rows' build (Msg::ArtBuilt) and runs it.
fn land_art_rows(sh: &mut Shell, rx: &Receiver<Msg>) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        if let Ok(Msg::ArtBuilt(b)) = rx.recv_timeout(Duration::from_millis(100)) {
            let (key, view) = *b;
            // its landing may start the next one (a build that wrote
            // seen.json moved the key): until none runs
            sh.art_built(key, view);
            if !sh.art.rows.borrow().building() {
                return;
            }
        }
    }
    panic!("the artifact rows were not built in 30 s");
}

#[test]
fn the_loop_stays_in_budget_at_his_scale() {
    let root = std::env::temp_dir().join(format!("sb-loop-budget-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let t = std::time::Instant::now();
    let (mut sh, rx) = shell_at_scale(&root);
    let setup = t.elapsed();
    let live = format!("t{}", ARCHIVED);
    // a state push with no store write (most of them: an agent's status)
    // the loop thread's store file reads at each push: none, ever
    // (architect m_61480; crate::store_io counts them)
    let mut read: Vec<(&str, u64)> = Vec::new();
    let r = crate::store_io::reads();
    let push = median5(|| {
        sh.state_gate.changed();
        sh.push_state();
    });
    read.push(("a push", crate::store_io::reads() - r));
    // a store write, the push that sees it (it starts a build of the
    // rows: never on the loop), then the push after its build landed
    // (the typed rows made again once, on the loop)
    let (mut wrote, mut built) = (Vec::new(), Vec::new());
    for _ in 0..5 {
        crate::store_gen::bump();
        sh.state_gate.changed();
        // each push with the artifact rows the view writer asks for at
        // its next write (debounced: not at every push, always after a write)
        let (t, r) = (cpu_ms(), crate::store_io::reads());
        sh.push_state();
        let _ = sh.artifact_rows_now();
        wrote.push(cpu_ms() - t);
        read.push(("the push after a store write", crate::store_io::reads() - r));
        land_art_rows(&mut sh, &rx);
        sh.state_gate.changed();
        let (t, r) = (cpu_ms(), crate::store_io::reads());
        sh.push_state();
        let _ = sh.artifact_rows_now();
        built.push(cpu_ms() - t);
        read.push(("the push after the rows' build", crate::store_io::reads() - r));
    }
    let mid = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[2]
    };
    let (wrote, built) = (mid(wrote), mid(built));
    // a REPL line of a live agent
    let line = median5(|| sh.feed(&live, "a line of the agent's output, as long as a usual one"));
    // one of his messages to a live agent
    let mut n = 0;
    let deliver = median5(|| {
        n += 1;
        sh.step(Input::ClientInput { client: 1, focus: live.clone(), text: format!("his message {n}"), queued: false, parts: String::new(), from: String::new() });
    });
    let _ = std::fs::remove_dir_all(&root);
    eprintln!("loop budget at his scale: setup {setup:?}, push {push:.1} ms, after a write {wrote:.1} ms, after its build {built:.1} ms, repl line {line:.2} ms, his message {deliver:.1} ms (CPU, median of 5); store reads on the loop: {read:?}");
    for (what, n) in &read {
        assert_eq!(*n, 0, "{what} read {n} store files on the loop thread (a disk wait at load: hub-loop 2026-11-11)");
    }
    assert!(push < PUSH_MS, "a state push took {push:.1} ms of the loop's CPU (budget {PUSH_MS})");
    assert!(wrote < WROTE_MS, "the push after a store write took {wrote:.1} ms of the loop's CPU (budget {WROTE_MS})");
    assert!(built < BUILT_MS, "the push after the rows' build took {built:.1} ms of the loop's CPU (budget {BUILT_MS})");
    assert!(line < LINE_MS, "a REPL line took {line:.2} ms of the loop's CPU (budget {LINE_MS})");
    assert!(deliver < DELIVER_MS, "his message took {deliver:.1} ms of the loop's CPU (budget {DELIVER_MS})");
}

// The CPU budgets (architect m_61480): 3x the medians measured on
// sb/hub-loop after 0b8089fe9 (2026-11-12, M-series, load 20-30), with
// the fake above (real target files, 2 versions, notes.json per page).
// Thread CPU never counts disk wait: tonight's regression (the rows
// built on the loop again, the mutation this test was checked against)
// moved these by < 10 ms with a warm cache, and the store reads' count
// above by ~1555 per push: that count is the guard for it.
/// a push with no store write: 52-59 ms.
const PUSH_MS: f64 = 165.0;
/// the push after a store write (it starts the rows' build): 43-58 ms.
const WROTE_MS: f64 = 165.0;
/// the push after the rows' build landed: it converts the 1550 typed
/// rows once (60-210 ms by the view writer's debounce).
const BUILT_MS: f64 = 630.0;
/// a REPL line: 0.02-0.09 ms; 3x rounded up to the ms.
const LINE_MS: f64 = 1.0;
/// his message to a live agent: 4.8-8.3 ms (3x the high side 7.4 of
/// the medians kept, 23).
const DELIVER_MS: f64 = 23.0;

/// Lines in each thread of the history test: his 1340 threads, fewer
/// lines each than his ~700 (927k entries, 2026-11-12) so the test stays
/// a unit test; the loop's work before this change grew with both.
const ENTRIES: usize = 40;

/// Each agent's transcript.log, ENTRIES lines of his kinds.
fn transcripts(sh: &Shell) {
    let agents = sh.opts.paths.state.join("agents");
    for a in sh.hub.st.agents.values() {
        let d = agents.join(&a.dir);
        std::fs::create_dir_all(&d).unwrap();
        let lines: String = (0..ENTRIES)
            .map(|i| match i % 4 {
                0 => format!("{}\tsb you : look at the walrus in thread {} line {i}\n", 1_000 + i, a.name),
                1 => format!("{}\t  obs: assistant: the walrus of {} is in the attic, line {i}\n", 1_000 + i, a.name),
                2 => format!("{}\ttool #{i} bash : rg -n walrus src/ | head -20\n", 1_000 + i),
                _ => format!("{}\ttool_result #{i} ok : src/attic.rs:12: walrus\n", 1_000 + i),
            })
            .collect();
        std::fs::write(d.join("transcript.log"), lines).unwrap();
    }
}

/// sb history and sb show at his scale (history-loop, main m_61883,
/// architect m_61891): the loop only hands the request to the history
/// worker. On 2026-11-12 the search (its refresh of 1340 transcripts,
/// then the ranking) ran on the loop: 32 s at load 90, his messages
/// waited. The guard: 0 transcript reads on the loop thread and its CPU
/// under HISTORY_MS, for a cold search, a warm one, a show and --all.
#[test]
fn sb_history_and_show_never_hold_the_loop() {
    let root = std::env::temp_dir().join(format!("sb-loop-history-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (mut sh, _rx) = shell_at_scale(&root);
    transcripts(&sh);
    let live = format!("t{ARCHIVED}");
    let dir = sh.hub.st.agents[&live].dir.clone();
    let asker = accept::Asker { who: crate::peer::Who::Agent(dir), peer: "budget".into(), sock: crate::peer::Sock::Agent };
    let mut token = 1000;
    let mut ask = |sh: &mut Shell, mut v: Value| {
        v["from"] = json!(live);
        token += 1;
        let (ours, theirs) = UnixStream::pair().unwrap();
        let (t, w, r) = (cpu_ms(), std::time::Instant::now(), crate::store_io::reads());
        sh.agent_new(token, theirs, v, &asker);
        let (ms, wall, n) = (cpu_ms() - t, w.elapsed(), crate::store_io::reads() - r);
        // EINVAL once the reply is written and its end closed (macOS)
        let _ = ours.set_read_timeout(Some(Duration::from_secs(120)));
        let mut line = String::new();
        std::io::BufRead::read_line(&mut std::io::BufReader::new(ours), &mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line:?}"));
        (ms, wall, n, reply)
    };
    let mut seen = Vec::new();
    for (what, v) in [
        ("a cold sb history", json!({"cmd": "history", "query": "walrus attic"})),
        ("a warm sb history", json!({"cmd": "history", "query": "walrus attic", "page": 2})),
        ("sb show", json!({"cmd": "show", "agent": "t3", "pos": 2})),
        ("sb history --all", json!({"cmd": "history", "query": "walrus", "all": true})),
    ] {
        let (ms, wall, n, reply) = ask(&mut sh, v);
        seen.push((what, ms, wall, n, reply));
    }
    let _ = std::fs::remove_dir_all(&root);
    for (what, ms, wall, n, _) in &seen {
        eprintln!("{what}: the loop's CPU {ms:.2} ms, its wall {wall:?}, {n} transcript reads on the loop");
    }
    // what they answer: the search's own (history_work.rs's test compares it)
    assert_eq!(seen[0].4["ok"], json!(true), "{}", seen[0].4);
    assert!(seen[0].4["text"].as_str().is_some_and(|t| t.contains("walrus")), "{}", seen[0].4);
    assert_eq!(seen[2].4["ok"], json!(true), "{}", seen[2].4);
    for (what, ms, _, n, _) in &seen {
        assert_eq!(*n, 0, "{what} read {n} transcript files on the loop thread (32 s on 2026-11-12)");
        assert!(*ms < HISTORY_MS, "{what} took {ms:.2} ms of the loop's CPU (budget {HISTORY_MS})");
    }
}

/// sb history or sb show's part on the loop (architect m_61891): its
/// sender's check (sb-core's speaks step, every agent request takes it),
/// the agents' list for the search, the job handed to the worker:
/// 4.5-5.4 ms (2026-11-12, M-series, load 50-150), 3x the high side.
/// Before, the search on the loop: a cold one here 2.3 s of reads, his
/// 32 s at his scale (hub.log, 2026-11-12).
const HISTORY_MS: f64 = 16.0;
