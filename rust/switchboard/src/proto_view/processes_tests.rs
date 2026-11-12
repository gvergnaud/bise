use super::*;

const HUB: &str = "aa";

fn r(pid: u32, ppid: u32, pgid: u32, sid: u32, owners: Option<&str>, args: &str) -> Row {
    Row {
        pid,
        ppid,
        pgid,
        sid,
        zombie: false,
        owners: owners.map(str::to_string),
        start: format!("s{pid}"),
        started_ms: 1_791_100_000_000 + pid as u64 * 1000,
        cpu: 1.25,
        rss_kb: 2048,
        args: args.to_string(),
    }
}

fn agent(name: &str, sids: &[u32], bg: &[(u32, u32, &str)]) -> AgentIn {
    AgentIn {
        name: name.into(),
        dir: name.into(),
        sessions: sids.iter().copied().collect(),
        repl: None,
        bg: bg.iter().map(|(n, pgid, cmd)| BgJob { n: *n, pgid: *pgid, cmd: cmd.to_string() }).collect(),
    }
}

/// perf's REPL (101, its session) runs a sync command now (110 + 111);
/// its bg job 3 (pgid 200: pnpm dev and its vite child) with bise's
/// runner (190) and watch loop (191, 192); a nohup'd server (300) with a
/// child (301), reparented to 1; a hidden /bin/sleep in its session
/// (310, no list visible); docs's own nohup (400); the user's (500); the
/// hub (100).
fn table() -> Vec<Row> {
    let t = Some("aa.perf.1");
    vec![
        r(1, 0, 1, 1, None, "launchd"),
        r(100, 1, 100, 100, None, "bise sbd"),
        r(101, 100, 101, 101, t, "repl-live"),
        r(102, 101, 101, 101, t, "bend-jsrt"),
        r(110, 101, 110, 101, t, "/bin/sh /x/run/bash-1.sh"),
        r(111, 110, 110, 101, t, "cargo test"),
        r(190, 1, 110, 101, None, "/bin/sh"),
        r(191, 190, 110, 101, None, "/bin/sh"),
        r(192, 191, 110, 101, None, "sleep 2"),
        r(200, 190, 200, 101, t, "/bin/sh /x/bg/3.cmd"),
        r(201, 200, 200, 101, t, "node vite --port 5173"),
        r(300, 1, 300, 300, t, "python3 -m http.server 8000"),
        r(301, 300, 300, 300, t, "python3 worker"),
        r(310, 1, 310, 101, None, "sleep 600"),
        r(400, 1, 400, 400, Some("aa.docs.2"), "mkdocs serve"),
        r(500, 1, 500, 500, None, "zsh"),
    ]
}

fn kinds(ts: &[Tree]) -> Vec<(String, ProcKind, u32, Vec<u32>)> {
    ts.iter().map(|t| (t.agent.clone(), t.kind, t.root, t.pids.clone())).collect()
}

#[test]
fn an_agents_trees_are_its_bg_jobs_and_what_it_detached() {
    let agents = [agent("perf", &[101], &[(3, 200, "pnpm dev\n")]), agent("docs", &[], &[])];
    let got = trees(&table(), HUB, &agents, 100);
    assert_eq!(
        kinds(&got),
        vec![
            ("perf".into(), ProcKind::Bg { n: 3 }, 200, vec![200, 201]),
            ("perf".into(), ProcKind::Detached, 300, vec![300, 301]),
            ("perf".into(), ProcKind::Detached, 310, vec![310]),
            ("docs".into(), ProcKind::Detached, 400, vec![400]),
        ],
        "never the REPL, its jsrt, the turn's command (110, 111), bise's runner and watch loop, the user's zsh, the hub"
    );
    assert_eq!(got[0].cmd, "pnpm dev", "a bg job's own command, trimmed");
    assert_eq!(got[1].cmd, "python3 -m http.server 8000", "a detached one's root's command line");
}

#[test]
fn a_stopped_agent_lists_nothing_and_a_gone_bg_job_neither() {
    // not live: not in the list at all
    assert!(trees(&table(), HUB, &[], 100).is_empty());
    // every command runs in a slot: the turn's own (110, still under
    // its REPL's wrapper) is no bg job
    let sync = trees(&table(), HUB, &[agent("perf", &[101], &[(4, 110, "cargo test")])], 100);
    assert!(sync.iter().all(|t| t.root != 110 && !t.pids.contains(&111)), "{sync:?}");
    // a bg job whose group is gone (the pid file of an ended one)
    let got = trees(&table(), HUB, &[agent("docs", &[], &[(1, 999, "x")])], 100);
    assert_eq!(kinds(&got), vec![("docs".into(), ProcKind::Detached, 400, vec![400])]);
}

#[test]
fn a_row_sums_its_tree_and_names_its_url_and_output() {
    let rows = table();
    let by: BTreeMap<u32, &Row> = rows.iter().map(|r| (r.pid, r)).collect();
    let t = &trees(&rows, HUB, &[agent("perf", &[101], &[(3, 200, "pnpm dev")])], 100)[..2];
    let ports: BTreeMap<u32, Vec<u16>> = [(201, vec![24678, 5173]), (200, vec![5173])].into();
    let files: BTreeMap<u32, Files> = [(200, Files { cwd: Some("/w/perf".into()), out: None, fd1: Some(Fd1::File) }), (300, Files { cwd: None, out: Some("/w/nohup.out".into()), fd1: Some(Fd1::File) })].into();
    let http: BTreeSet<u16> = [5173].into();
    let found = Found { ports: &ports, files: &files, http: &http };
    let bg = row(&t[0], &by, &found).unwrap();
    assert_eq!((bg.id.as_str(), bg.pid, bg.procs, bg.rss_mb, bg.cpu), ("bg3.1791100200", 200, 2, 4, 2.5));
    assert_eq!((bg.ports.clone(), bg.url.as_deref(), bg.name.as_deref()), (vec![5173, 24678], Some("http://127.0.0.1:5173"), Some("dev server")));
    assert_eq!((bg.cwd.as_deref(), bg.out, bg.state), (Some("/w/perf"), OutKind::Bg, ProcState::Running));
    let d = row(&t[1], &by, &found).unwrap();
    assert_eq!((d.id.as_str(), d.out, d.url.as_deref()), ("p300.1791100300", OutKind::File, None));
    let mut none = t[1].clone();
    none.root = 310;
    assert_eq!(row(&none, &by, &Found { ports: &ports, files: &BTreeMap::new(), http: &http }).unwrap().out, OutKind::None);
    assert_eq!((bg.fd1, d.fd1), (Some(Fd1::File), Some(Fd1::File)), "the row carries where its root's stdout points");
    let tty: BTreeMap<u32, Files> = [(310, Files { cwd: None, out: None, fd1: Some(Fd1::Tty) })].into();
    let t3 = row(&none, &by, &Found { ports: &ports, files: &tty, http: &http }).unwrap();
    assert_eq!((t3.out, t3.fd1), (OutKind::None, Some(Fd1::Tty)));
    // a port that never answered HTTP is no link: no url, its port stays
    let quiet = row(&t[0], &by, &Found { ports: &ports, files: &files, http: &BTreeSet::new() }).unwrap();
    assert_eq!((quiet.ports, quiet.url), (vec![5173, 24678], None));
    // the first port that answered, not the first port
    let late: BTreeSet<u16> = [24678].into();
    assert_eq!(row(&t[0], &by, &Found { ports: &ports, files: &files, http: &late }).unwrap().url.as_deref(), Some("http://127.0.0.1:24678"));
}

#[test]
fn only_an_http_status_line_is_a_web_page() {
    assert!(answers_http(b"HTTP/1.0 200 OK\r\n"));
    assert!(answers_http(b"HTTP/1.1 404 Not Found"));
    assert!(!answers_http(b""));
    assert!(!answers_http(b"SSH-2.0-OpenSSH"));
    assert!(!answers_http(b"{\"jsonrpc\":\"2.0\"}"));
}

#[test]
fn ps_and_lsof_parse() {
    let ps = "  200   200 Tue Oct  7 00:48:12 2026   12.5  348160 node /x/vite.js --port 5173\n  201 200 Tue Oct  7 00:48:13 2026 0,0 1024 sh -c  'a  b'\nbad line\n";
    let s = parse_stats(ps);
    assert_eq!(s.len(), 2);
    assert_eq!((s[&200].pgid, s[&200].start.as_str(), s[&200].cpu, s[&200].rss_kb), (200, "Tue Oct 7 00:48:12 2026", 12.5, 348160));
    assert_eq!(s[&200].args, "node /x/vite.js --port 5173");
    assert_eq!((s[&201].cpu, s[&201].args.as_str()), (0.0, "sh -c  'a  b'"), "a comma decimal; the args' own spacing kept");
    assert_eq!(lstart_parts("Tue Oct  7 00:48:12 2026"), Some((2026, 10, 7, 0, 48, 12)));
    assert_eq!(lstart_parts("Tue Foo 7 00:48:12 2026"), None);
    let ports = lsof_ports("p4242\nf5\nn*:5173\nf7\nn127.0.0.1:24678\nf8\nn[::1]:5173\np4243\nf3\nn*:8000\n");
    assert_eq!(ports, [(4242, vec![5173, 24678]), (4243, vec![8000])].into(), "sorted, once each, ipv6 too, by pid");
    assert!(lsof_ports("").is_empty());
    let files = lsof_files("p300\nfcwd\ntDIR\nn/w/perf\nf1\ntREG\nn/w/nohup.out\np301\nfcwd\ntDIR\nn/w\nf1u\ntCHR\nn/dev/ttys001\np302\nf1w\ntREG\nn/dev/null\n");
    assert_eq!(files[&300], Files { cwd: Some("/w/perf".into()), out: Some("/w/nohup.out".into()), fd1: Some(Fd1::File) });
    assert_eq!(files[&301], Files { cwd: Some("/w".into()), out: None, fd1: Some(Fd1::Tty) }, "a tty is no file to read");
    assert_eq!((files[&302].out.clone(), files[&302].fd1), (None, Some(Fd1::Null)));
}

#[test]
fn fd1_says_where_stdout_points() {
    // macOS lsof -F lines: a pipe, /dev/null as a CHR, a unix socket, a pid with no fd 1
    let files = lsof_files("p400\nfcwd\ntDIR\nn/w\nf1w\ntPIPE\nn->0x1234\np401\nf1u\ntCHR\nn/dev/null\np402\nf1u\ntunix\nn->0x99\np403\nfcwd\ntDIR\nn/w\np404\nf1w\ntFIFO\nnpipe\np405\nf1u\ntKQUEUE\nncount=0\n");
    let got: Vec<(u32, Option<Fd1>)> = files.iter().map(|(p, f)| (*p, f.fd1)).collect();
    assert_eq!(
        got,
        [(400, Some(Fd1::Pipe)), (401, Some(Fd1::Null)), (402, Some(Fd1::Socket)), (403, Some(Fd1::Closed)), (404, Some(Fd1::Pipe)), (405, Some(Fd1::Unknown))]
    );
    assert!(files.values().all(|f| f.out.is_none()), "only a regular file is read");
    assert_eq!(fd1_of("IPv4", "127.0.0.1:5000"), Fd1::Socket);
    assert_eq!(fd1_of("CHR", "/dev/ttys004"), Fd1::Tty);
}

#[test]
fn servers_are_named() {
    assert_eq!(server_name("pnpm dev --port 5173").as_deref(), Some("dev server"));
    assert_eq!(server_name("sh -c 'python3 -m http.server 0 & wait'").as_deref(), Some("http server"));
    assert_eq!(server_name("cargo test -p switchboard"), None);
}

fn proc_row(agent: &str, id: &str, ports: &[u16]) -> Proc {
    Proc {
        id: id.into(),
        agent: agent.into(),
        kind: ProcKind::Detached,
        cmd: "x".into(),
        cwd: None,
        pid: 1,
        started_ms: 0,
        ports: ports.to_vec(),
        url: None,
        name: None,
        state: ProcState::Running,
        ended_ms: None,
        cpu: 3.0,
        rss_mb: 9,
        procs: 2,
        out: OutKind::Bg,
        fd1: None,
    }
}

#[test]
fn what_ended_and_how() {
    let was: BTreeMap<(String, String), Proc> = [proc_row("perf", "bg1.5", &[]), proc_row("perf", "bg2.6", &[])].into_iter().map(|p| ((p.agent.clone(), p.id.clone()), p)).collect();
    let now = [proc_row("perf", "bg2.6", &[])];
    let g = gone(&was, &now);
    assert_eq!(g.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["bg1.5"]);
    let e = ended(g[0], Why::Exit(Some(2)), 77);
    assert_eq!((e.state, e.ended_ms, e.cpu, e.rss_mb, e.procs), (ProcState::Exited { rc: Some(2) }, Some(77), 0.0, 0, 0));
    assert_eq!(ended(g[0], Why::You, 1).state, ProcState::StoppedByYou);
    assert_eq!(ended(g[0], Why::Agent, 1).state, ProcState::StoppedWith);
}

#[test]
fn ended_rows_stay_a_day() {
    let at = |id: &str, ms: u64| ended(&proc_row("perf", id, &[]), Why::Exit(None), ms);
    let now = 10 * ENDED_KEEP_MS;
    let kept = prune(vec![at("b", now - 1000), at("old", now - ENDED_KEEP_MS), at("a", now - 5000)], now);
    assert_eq!(kept.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["a", "b"], "oldest first, a day old gone");
}

#[test]
fn the_output_copy_keeps_the_last_lines_and_bytes() {
    let many: String = (0..2500).map(|i| format!("line {i}\n")).collect();
    let tail = std::str::from_utf8(copy_tail(many.as_bytes())).unwrap();
    assert!(tail.starts_with("line 500\n") && tail.ends_with("line 2499\n"), "the last 2,000 lines");
    assert_eq!(tail.lines().count(), COPY_LINES);
    let long: Vec<u8> = (0..400).flat_map(|_| [b'x'; 1023].into_iter().chain(*b"\n")).collect();
    let t = copy_tail(&long);
    assert!(t.len() <= COPY_BYTES && t.len() > COPY_BYTES - 1024 && t[0] == b'x', "256 KB at most, from a whole line");
    assert_eq!(copy_tail(b"a\nb"), b"a\nb");
}

#[test]
fn an_output_range_is_its_last_chunk_or_from_an_offset() {
    assert_eq!(out_range(10, None), (0, 10));
    assert_eq!(out_range(OUT_CHUNK * 3, None), (OUT_CHUNK * 2, OUT_CHUNK));
    assert_eq!(out_range(100, Some(40)), (40, 60));
    assert_eq!(out_range(100, Some(400)), (100, 0), "past the end: nothing new");
    assert_eq!(out_range(OUT_CHUNK * 3, Some(0)), (0, OUT_CHUNK), "capped");
}

#[test]
fn a_stop_signals_only_the_process_the_row_saw() {
    let rows = table();
    let started = rows[9].started_ms;
    assert!(same_process(&rows, 200, started));
    assert!(!same_process(&rows, 200, started + 1000), "the pid went to another process");
    assert!(!same_process(&rows, 999, started));
    assert_eq!(stopped_msg("pnpm dev\n--x", "bg3.1"), "[bise] he stopped pnpm dev (bg3.1) from the processes list");
}

#[test]
fn counts_are_the_running_ones_and_their_lowest_port() {
    let mut stopped = proc_row("docs", "p2.1", &[80]);
    stopped.state = ProcState::StoppedByYou;
    let c = counts(&[proc_row("perf", "bg1.1", &[8000]), proc_row("perf", "p9.1", &[5173, 9000]), proc_row("perf", "bg2.1", &[]), stopped]);
    assert_eq!(c, [("perf".to_string(), (3, Some(5173)))].into());
}

#[test]
fn a_bg_jobs_command_has_no_workdir_guard() {
    assert_eq!(job_cmd("cd '/private/tmp/ws' || exit 1\npython3 -u -m http.server 8000\n"), "python3 -u -m http.server 8000");
    assert_eq!(job_cmd("  pnpm dev\n"), "pnpm dev");
    assert_eq!(job_cmd("cd sub && make\nmake test"), "cd sub && make\nmake test", "an agent's own cd stays");
    assert_eq!(job_cmd("cd '/w' || exit 1\ncat <<EOF\nx\nEOF"), "cat <<EOF\nx\nEOF");
}

/// The shapes a real throwaway hub showed (procs-tui m_38502): a REPL
/// just spawned whose session isn't in repl.sids yet (the hub's child),
/// and a bg job's runner (the turn's wrapper, reparented to 1, in the
/// REPL's group) with its watch loop and its sleep.
#[test]
fn a_new_repl_and_bises_wrappers_are_never_detached() {
    let t = Some("aa.perf.1");
    let rows = vec![
        r(1, 0, 1, 1, None, "launchd"),
        r(100, 1, 100, 100, None, "bise sbd"),
        r(101, 100, 101, 101, t, "/w/harness/repl-live"),
        r(190, 1, 101, 101, t, "/bin/sh /h/st/agents/perf/run/bend-sh-60287-1.sh"),
        r(200, 190, 200, 101, t, "/bin/sh /h/st/agents/perf/tmp/bg/0.cmd"),
        r(201, 200, 200, 101, t, "python3 -u -m http.server 8000"),
        r(202, 190, 202, 202, t, "/bin/sh /h/st/agents/perf/run/bend-sh-60287-1.sh"),
        r(203, 202, 202, 202, t, "sleep 2"),
        r(300, 1, 300, 300, t, "sh -c while true; do date; sleep 2; done"),
    ];
    let got = trees(&rows, HUB, &[agent("perf", &[], &[(0, 200, "cd '/h/ws' || exit 1\npython3 -u -m http.server 8000")])], 100);
    assert_eq!(
        kinds(&got),
        vec![("perf".into(), ProcKind::Bg { n: 0 }, 200, vec![200, 201]), ("perf".into(), ProcKind::Detached, 300, vec![300])],
        "not the REPL (no sids yet), not the runner nor its watch loop in a group of its own"
    );
    assert_eq!(got[0].cmd, "python3 -u -m http.server 8000");
}

/// The shape of apps-gallery's REPL tonight (say-port): adopted after a
/// hub restart, so not the hub's child, and in no session of repl.sids.
/// repl.json names it: never a detached row, so its say port is never
/// listed nor HTTP-probed (the probes queued behind the hub's connection
/// and became his words at the next one); and the probe skips every say
/// port and bise's own ports whatever the rows say.
#[test]
fn an_adopted_repl_is_never_detached_and_its_say_port_never_probed() {
    let t = Some("aa.apps.1");
    let rows = vec![
        r(1, 0, 1, 1, None, "launchd"),
        r(100, 1, 100, 100, None, "bise sbd"),
        r(66559, 1, 66559, 66559, t, "/v/repl-live"),
        r(66600, 66559, 66559, 66559, t, "/bin/sh -c pnpm test"),
        r(300, 1, 300, 300, t, "python3 -m http.server 8000"),
    ];
    let mut a = agent("apps", &[], &[]);
    a.repl = Some(Repl { pid: 66559, port: 63180 });
    let got = trees(&rows, HUB, std::slice::from_ref(&a), 100);
    assert_eq!(kinds(&got), vec![("apps".into(), ProcKind::Detached, 300, vec![300])], "only the agent's own server");
    assert_eq!(never_probed(&[a, agent("docs", &[], &[])], &[47438, 51800]), [47438, 51800, 63180].into());
    assert_eq!(base_port("http://127.0.0.1:51800/tok"), Some(51800));
    assert_eq!(base_port("http://example.com:80/x"), None);
}

/// bise's plugins bridge forks away from its REPL (parent 1, the REPL's
/// group) and keeps the agent's tag: never a detached row, nor what it
/// runs; a `plugins serve` the agent typed without bise's `--parent` is.
#[test]
fn bises_plugins_bridge_is_never_listed() {
    let t = Some("aa.perf.1");
    let rows = vec![
        r(1, 0, 1, 1, None, "launchd"),
        r(100, 1, 100, 100, None, "bise sbd"),
        r(101, 100, 101, 101, t, "/v/repl-live"),
        r(320, 1, 101, 101, t, "/v/bise plugins serve --dir /h/run/50158/plugins --parent 101"),
        r(321, 320, 101, 101, t, "node /h/plugins/x/server.js"),
        // an app bundle whose path holds a space (a release candidate's
        // 'bise cand.app'): ps joins argv with spaces (agents-new, 090684cfb)
        r(322, 1, 101, 101, t, "/d/bise cand.app/Contents/Resources/bin/bise plugins serve --dir /h/run/50159/plugins --parent 101"),
        r(330, 1, 330, 330, t, "bise plugins serve --dir /tmp/p"),
        r(340, 1, 340, 340, t, "python3 -m http.server 8000"),
    ];
    let got = trees(&rows, HUB, &[agent("perf", &[101], &[])], 100);
    assert_eq!(kinds(&got), vec![("perf".into(), ProcKind::Detached, 330, vec![330]), ("perf".into(), ProcKind::Detached, 340, vec![340])]);
}

#[test]
fn a_detached_command_keeps_its_quoting() {
    let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(shell_join(&a(&["sh", "-c", "while true; do date; sleep 2; done"])), "sh -c 'while true; do date; sleep 2; done'");
    assert_eq!(shell_join(&a(&["python3", "-u", "-m", "http.server", "8000", "--bind", "127.0.0.1"])), "python3 -u -m http.server 8000 --bind 127.0.0.1");
    assert_eq!(shell_join(&a(&["echo", "it's", ""])), r"echo 'it'\''s' ''");
    assert_eq!(shell_join(&a(&["node", "/w/my app/x.js", "$HOME"])), "node '/w/my app/x.js' '$HOME'");
}
