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
