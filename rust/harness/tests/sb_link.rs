//! `sb` is the bise binary called by that name (busybox style): a link
//! named `sb` and `bise sb` give the same exit code, stdout, stderr and
//! hub request for every command, with and without a hub, with and
//! without the agent's variables.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

const EXE: &str = env!("CARGO_BIN_EXE_bise");

/// One argument list per documented form (and the errors of each kind).
fn cases() -> Vec<Vec<&'static str>> {
    vec![
        vec![],
        vec!["help"],
        vec!["send", "--help"],
        vec!["nope"],
        vec!["send"],
        vec!["list"],
        vec!["tasks"],
        vec!["send", "main", "hi \"there\"", "--expect-reply", "--mode", "queued"],
        vec!["send", "main", "--reply-to", "m_1", "--why", "because", "ok"],
        vec!["ask", "main", "why?", "--timeout", "5"],
        vec!["wait", "m_1", "--timeout", "1"],
        vec!["status", "working", "--note", "a note"],
        vec!["report", "done", "all good", "--decision", "d1"],
        vec!["follow", "perf"],
        vec!["follow", "perf", "--off"],
        vec!["inspect", "main", "--query", "x", "--around", "#3", "--limit", "2"],
        vec!["inspect", "main", "--origin"],
        vec!["spawn", "t1", "--objective", "do it"],
        vec!["interrupt", "t1"],
        vec!["stop", "t1", "why"],
        vec!["drop", "t1"],
        vec!["card", "a question?"],
        vec!["close", "1", "handled"],
        vec!["rename", "t1", "t2"],
        vec!["restore", "t1"],
        vec!["isolate", "t1"],
        vec!["move", "t1", "shared"],
        vec!["land", "--here", "the message"],
        vec!["history", "q"],
        vec!["history", "q w", "--agent", "main,t1", "--role", "user", "--since", "2026-09-01", "--archived", "--page", "2"],
        vec!["history", "q", "--role", "nope"],
        vec!["show", "main#3", "--context", "2"],
        vec!["show", "main"],
        vec!["worktree", "/tmp/x"],
        vec!["artifact", "add", "out/plan.md", "--title", "the plan", "--kind", "doc"],
        vec!["artifact", "list", "plan", "--agent", "t1"],
        vec!["artifact", "nope"],
        vec!["flow"],
        vec!["flow", "trunk"],
        vec!["flow", "nope"],
        vec!["feature"],
        vec!["feature", "new", "computer-use"],
        vec!["feature", "merge"],
        vec!["spawn", "cu-a", "--feature", "computer-use", "--objective", "do it"],
        vec!["spawn", "cu-b", "--feature", "computer-use", "--place", "new", "--objective", "do it"],
        // agent-made pages (docs/ambient-pages.md §2.2)
        vec!["page"],
        vec!["page", "list"],
        vec!["page", "notes", "weekly-update"],
        vec!["page", "publish", "/nonexistent/page.html", "--id", "w", "--notes-done", "n1,n2", "--note-answer", "n3=kept"],
        // standing orders (docs/ambient-roadmap.md B)
        vec!["every"],
        // an absolute --until: a relative one differs by the ms between two runs
        vec!["every", "10m", "check HN", "--until", "2030-01-01 18:00", "--times", "3", "--to", "t1"],
        vec!["every", "day", "07:30", "make the morning page"],
        vec!["every", "--stop", "2"],
        vec!["every", "soon", "x"],
        // event-wake: wake on an event (a pid's start time differs between
        // two runs: only the refused pid)
        vec!["wake"],
        vec!["wake", "--on-file", "/nonexistent/rc", "--tail", "/nonexistent/log", "--note", "the gate", "--max", "2h"],
        vec!["wake", "--stop", "3"],
        vec!["wake", "--on-exit", "nope"],
        // what bise keeps about the user (keeps.rs)
        vec!["taste"],
        vec!["taste", "add", "no emoji", "--from", "the launch post"],
        vec!["taste", "remove", "2"],
        vec!["taste", "fix"],
        vec!["people"],
        vec!["people", "set", "Nina", "support lead"],
        vec!["people", "remove", "Nina"],
        vec!["people", "set", "Nina"],
        // followed jobs (S10): the user hears once when one ends
        vec!["follow", "t1"],
        vec!["follow", "acme/t1", "--off"],
        vec!["follow"],
        vec!["version"],
        vec!["version", "switch", "HEAD"],
        vec!["restart", "current"],
    ]
}

/// Every command of `sb help` has a case (a new command needs one).
#[test]
fn every_command_has_a_case() {
    let names: Vec<&str> = cases().iter().filter_map(|c| c.first().copied()).collect();
    for c in switchboard::cli::COMMANDS {
        for alt in c.syntax.split(" | sb ") {
            let name = alt.trim_start_matches("sb ").split(' ').next().unwrap();
            assert!(names.contains(&name), "no case for sb {}", name);
        }
    }
}

/// A hub that records each request and answers `{"ok":true,...}`.
fn fake_hub(socket: &Path) -> Arc<Mutex<Vec<String>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let l = UnixListener::bind(socket).unwrap();
    let s = seen.clone();
    std::thread::spawn(move || {
        for c in l.incoming() {
            let Ok(mut c) = c else { continue };
            let mut line = String::new();
            // the CLI's "is the hub there" probe sends nothing
            if BufReader::new(&c).read_line(&mut line).unwrap_or(0) == 0 {
                continue;
            }
            s.lock().unwrap().push(line.trim().to_string());
            let _ = c.write_all(b"{\"ok\":true,\"text\":\"fake text\",\"message_id\":\"m_9\",\"to\":\"main\",\"reply\":\"r\"}\n");
        }
    });
    seen
}

type Run = (Option<i32>, String, String, Vec<String>);

fn run(prog: &Path, pre: &[&str], args: &[&str], env: &[(&str, &Path)], home: &Path, seen: &Mutex<Vec<String>>) -> Run {
    seen.lock().unwrap().clear();
    let mut c = Command::new(prog);
    c.args(pre).args(args).env_clear().env("HOME", home).env("PATH", "/usr/bin:/bin");
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    let req = seen.lock().unwrap().clone();
    (o.status.code(), String::from_utf8_lossy(&o.stdout).into(), String::from_utf8_lossy(&o.stderr).into(), req)
}

#[test]
fn the_sb_link_is_bise_sb() {
    let d: PathBuf = std::env::temp_dir().join(format!("sb-link-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("bin")).unwrap();
    let link = d.join("bin/sb");
    std::os::unix::fs::symlink(EXE, &link).unwrap();
    let socket = d.join("hub.sock");
    let seen = fake_hub(&socket);
    let agent = Path::new("t1");
    let envs: [&[(&str, &Path)]; 3] = [&[], &[("SB_SOCKET", &socket)], &[("SB_SOCKET", &socket), ("SB_AGENT", agent)]];
    let mut hub_requests = 0;
    for env in envs {
        for args in cases() {
            let a = run(&link, &[], &args, env, &d, &seen);
            let b = run(Path::new(EXE), &["sb"], &args, env, &d, &seen);
            assert_eq!(a, b, "sb {:?} with {:?}", args, env);
            hub_requests += a.3.len();
        }
    }
    // the agent's requests did reach the fake hub
    assert!(hub_requests > 20, "{}", hub_requests);
    // and `sb help` through the link is sb's help, not bise's
    let h = run(&link, &[], &["help"], &[], &d, &seen);
    assert!(h.1.contains("sb send") && !h.1.contains("--headless"), "{}", h.1);
    let _ = std::fs::remove_dir_all(&d);
}
