//! The running-processes rows (docs/processes.md), pure: the parse of
//! `ps` and `lsof`, the agents' process trees, the rows, which running
//! ones ended since the last scan and how, the ended log's prune and the
//! output copy's cap, the stop's same-start check. The shell that runs
//! ps and lsof, owns the ended log and stops a process is
//! `daemon/processes.rs`.
//!
//! Ports: `lsof -nP -iTCP -sTCP:LISTEN -Fpn` on macOS and Linux alike;
//! when lsof is missing they are empty (no error row). Linux's
//! `/proc/net/tcp` is not read (later, if lsof is often missing there).

use bise_peer::table::Proc as PeerProc;
use bise_proto::rows::{Fd1, OutKind, Proc, ProcKind, ProcState};
use std::collections::{BTreeMap, BTreeSet};

/// How long an ended row stays.
pub const ENDED_KEEP_MS: u64 = 24 * 3600 * 1000;
/// The kept copy of an ended process's output: its last lines, at most.
pub const COPY_LINES: usize = 2_000;
pub const COPY_BYTES: usize = 256 * 1024;
/// One `proc_out` answer or follow chunk, at most.
pub const OUT_CHUNK: u64 = 64 * 1024;

/// One process of the table: the peer table's fields (owners, session)
/// and `ps`'s (group, start, cpu, memory, whole command line).
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub pid: u32,
    pub ppid: u32,
    pub pgid: u32,
    pub sid: u32,
    pub zombie: bool,
    pub owners: Option<String>,
    /// `ps`'s lstart, as it says it: with the pid, who it is
    pub start: String,
    pub started_ms: u64,
    pub cpu: f64,
    pub rss_kb: u64,
    pub args: String,
}

/// `ps -axww -o pid=,pgid=,lstart=,%cpu=,rss=,args=` by pid.
#[derive(Clone, Debug, PartialEq)]
pub struct Stat {
    pub pgid: u32,
    pub start: String,
    pub cpu: f64,
    pub rss_kb: u64,
    pub args: String,
}

/// The `ps` command line [`parse_stats`] reads.
pub const PS_STATS: [&str; 3] = ["-axww", "-o", "pid=,pgid=,lstart=,%cpu=,rss=,args="];

/// `n` words of `l`, then the rest of the line as it is.
fn words(l: &str, n: usize) -> Option<(Vec<&str>, &str)> {
    let mut out = Vec::with_capacity(n);
    let mut rest = l.trim_start();
    for _ in 0..n {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        out.push(&rest[..end]);
        rest = rest[end..].trim_start();
    }
    Some((out, rest.trim_end()))
}

pub fn parse_stats(text: &str) -> BTreeMap<u32, Stat> {
    text.lines()
        .filter_map(|l| {
            let (w, args) = words(l, 9)?;
            let pid = w[0].parse().ok()?;
            Some((
                pid,
                Stat {
                    pgid: w[1].parse().ok()?,
                    start: w[2..7].join(" "),
                    cpu: w[7].replace(',', ".").parse().ok()?,
                    rss_kb: w[8].parse().ok()?,
                    args: args.to_string(),
                },
            ))
        })
        .collect()
}

/// `Tue Oct  7 00:48:12 2026` as (year, month 1-12, day, h, m, s).
pub fn lstart_parts(s: &str) -> Option<(i32, i32, i32, i32, i32, i32)> {
    let w: Vec<&str> = s.split_whitespace().collect();
    let [_, mon, day, hms, year] = w.as_slice() else { return None };
    let mon = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"].iter().position(|m| m == mon)? as i32 + 1;
    let mut t = hms.split(':').map(|x| x.parse::<i32>().ok());
    let (h, m, sec) = (t.next()??, t.next()??, t.next()??);
    Some((year.parse().ok()?, mon, day.parse().ok()?, h, m, sec))
}

/// The table: the peer's processes with their `ps` stats (a process
/// gone between the two reads is left out); `ms` turns an lstart into
/// epoch ms (local time: the shell's).
pub fn join(table: &[PeerProc], stats: &BTreeMap<u32, Stat>, ms: impl Fn(&str) -> Option<u64>) -> Vec<Row> {
    table
        .iter()
        .filter_map(|p| {
            let s = stats.get(&p.pid)?;
            Some(Row {
                pid: p.pid,
                ppid: p.ppid,
                pgid: s.pgid,
                sid: p.sid,
                zombie: p.zombie,
                owners: p.owners.clone(),
                start: s.start.clone(),
                started_ms: ms(&s.start).unwrap_or(0),
                cpu: s.cpu,
                rss_kb: s.rss_kb,
                args: s.args.clone(),
            })
        })
        .collect()
}

/// `lsof -Fpn` (or `-Fpftn`) output by pid: each `p<pid>` line opens a
/// process, the `f`/`t`/`n` lines after it are its files.
fn lsof_by_pid(out: &str) -> BTreeMap<u32, Vec<(String, String, String)>> {
    let mut m: BTreeMap<u32, Vec<(String, String, String)>> = BTreeMap::new();
    let (mut pid, mut fd, mut ty) = (None, String::new(), String::new());
    for l in out.lines() {
        let (k, v) = l.split_at(l.len().min(1));
        match k {
            "p" => pid = v.parse().ok(),
            "f" => (fd, ty) = (v.to_string(), String::new()),
            "t" => ty = v.to_string(),
            "n" => {
                if let Some(p) = pid {
                    m.entry(p).or_default().push((fd.clone(), ty.clone(), v.to_string()));
                }
            }
            _ => {}
        }
    }
    m
}

/// The TCP ports each pid listens on (`lsof -nP -iTCP -sTCP:LISTEN -Fpn`:
/// `n*:5173`, `n127.0.0.1:8000`, `n[::1]:3000`), sorted, once each.
pub fn lsof_ports(out: &str) -> BTreeMap<u32, Vec<u16>> {
    lsof_by_pid(out)
        .into_iter()
        .map(|(pid, files)| {
            let mut ports: Vec<u16> = files.iter().filter_map(|(_, _, n)| n.rsplit_once(':').and_then(|(_, p)| p.parse().ok())).collect();
            ports.sort_unstable();
            ports.dedup();
            (pid, ports)
        })
        .collect()
}

/// Where a process runs and writes (`lsof -nP -a -p <pids> -d cwd,1
/// -Fpftn`): its working folder, its stdout when that is a regular
/// file (a tty, a pipe, /dev/null: none), and where its stdout points
/// (`fd1`: closed when lsof lists the pid without an fd 1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Files {
    pub cwd: Option<String>,
    pub out: Option<String>,
    pub fd1: Option<Fd1>,
}

pub fn lsof_files(out: &str) -> BTreeMap<u32, Files> {
    lsof_by_pid(out)
        .into_iter()
        .map(|(pid, files)| {
            let mut f = Files { fd1: Some(Fd1::Closed), ..Files::default() };
            for (fd, ty, n) in files {
                match fd.as_str() {
                    "cwd" => f.cwd = Some(n),
                    // `1w`, `1u`: stdout, its mode after the number
                    d if d.trim_end_matches(|c: char| c.is_ascii_alphabetic()) == "1" => {
                        let to = fd1_of(&ty, &n);
                        if to == Fd1::File {
                            f.out = Some(n);
                        }
                        f.fd1 = Some(to);
                    }
                    _ => {}
                }
            }
            (pid, f)
        })
        .collect()
}

/// What an lsof file line (its `t` type, its `n` name) says fd 1 points at.
pub fn fd1_of(ty: &str, name: &str) -> Fd1 {
    match ty {
        "CHR" if name == "/dev/null" => Fd1::Null,
        "CHR" => Fd1::Tty,
        "REG" if name == "/dev/null" => Fd1::Null,
        "REG" => Fd1::File,
        "PIPE" | "FIFO" => Fd1::Pipe,
        "unix" | "IPv4" | "IPv6" | "sock" => Fd1::Socket,
        _ => Fd1::Unknown,
    }
}

/// What a server command is, when it is one bise knows ("dev server",
/// "http server"); None for any other command (a test run, a build).
pub fn server_name(cmd: &str) -> Option<String> {
    let c = format!(" {} ", cmd.split_whitespace().collect::<Vec<_>>().join(" "));
    let has = |w: &str| c.contains(&format!(" {w} "));
    let dev = ["pnpm dev", "npm run dev", "yarn dev", "bun dev", "bun run dev", "pnpm run dev", "pnpm start", "npm start", "yarn start", "vite", "next dev", "astro dev", "cargo watch", "trunk serve"];
    if dev.iter().any(|d| has(d)) {
        return Some("dev server".into());
    }
    if has("http.server") || has("serve") || has("http-server") || has("caddy") {
        return Some("http server".into());
    }
    if has("grafana-server") || has("grafana") {
        return Some("grafana (local)".into());
    }
    if has("storybook") {
        return Some("storybook".into());
    }
    None
}

/// A live agent as the scan reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentIn {
    pub name: String,
    pub dir: String,
    /// its REPLs' session ids (`repl.sids`): the REPL is their leader
    pub sessions: BTreeSet<u32>,
    /// its live REPL as `repl.json` names it (the one a hub adopts): its
    /// pid and its say port. An adopted REPL is not the hub's child and
    /// may be in no session of `repl.sids`: this pid still keeps it and
    /// what it runs out of the detached rows
    pub repl: Option<Repl>,
    pub bg: Vec<BgJob>,
}

/// An agent's REPL process (`repl.json`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repl {
    pub pid: u32,
    /// the say port: the hub's line to its REPL, never asked HTTP
    pub port: u16,
}

/// The ports the HTTP probe never connects to: the agents' say ports (a
/// connection there is the hub's alone; a probe queued behind the hub's
/// was read as his words, say-port) and `own`, the ports bise itself
/// serves (its pages server, its plugins bridges).
pub fn never_probed(agents: &[AgentIn], own: &[u16]) -> BTreeSet<u16> {
    agents.iter().filter_map(|a| a.repl.map(|r| r.port)).chain(own.iter().copied()).collect()
}

/// The port of a loopback base (`http://127.0.0.1:<port>/<token>`, a
/// bridge's `base`).
pub fn base_port(base: &str) -> Option<u16> {
    let rest = base.strip_prefix("http://127.0.0.1:")?;
    rest.split('/').next()?.parse().ok()
}

/// A running background job of its bash tool (`bg/<n>.slot` with a live
/// `<n>.pid`).
#[derive(Clone, Debug, PartialEq)]
pub struct BgJob {
    pub n: u32,
    /// its process group (`<n>.pid`)
    pub pgid: u32,
    /// `<n>.cmd`
    pub cmd: String,
}

/// One process tree an agent left running.
#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    pub agent: String,
    pub dir: String,
    pub kind: ProcKind,
    pub root: u32,
    /// its processes, the root first
    pub pids: Vec<u32>,
    /// a bg job's command (a detached one: its root's command line)
    pub cmd: String,
}

/// A bg job's own command from its `<n>.cmd`: without the guard the
/// runtime puts first when the agent works in its own folder
/// (`cd '<dir>' || exit 1`, bash-pure.bend `with_workdir`), trimmed.
pub fn job_cmd(raw: &str) -> String {
    let raw = raw.trim();
    match raw.split_once('\n') {
        Some((first, rest)) if first.starts_with("cd ") && first.trim_end().ends_with(" || exit 1") => rest.trim().to_string(),
        _ => raw.to_string(),
    }
}

/// A detached process's command as a shell would read it back: its argv
/// joined, an argument with a space or a shell character in single
/// quotes (`sh -c 'while true; do date; done'`), never `ps`'s
/// space-joined line that loses them.
pub fn shell_join(argv: &[String]) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c);
    argv.iter()
        .map(|a| if !a.is_empty() && a.chars().all(safe) { a.clone() } else { format!("'{}'", a.replace('\'', r"'\''")) })
        .collect::<Vec<_>>()
        .join(" ")
}

/// One of bise's own bash wrappers for agent `dir`: the turn's command,
/// a bg job's runner or its watch loop (`/bin/sh <agent>/run/bend-sh-*.sh`
/// and its forks, which keep its command line).
fn bise_wrapper(args: &str, dir: &str) -> bool {
    args.contains(&format!("/agents/{dir}/run/bend-sh-"))
}

/// bise's plugins bridge for a REPL (`<bise> plugins serve --dir <run>/plugins
/// --parent <repl>`, plugins/src/bridge.rs): it forks away from its REPL
/// (its parent is 1) and ends with it, and its port is bise's loopback
/// API, never the agent's server (Gabriel's 13 rows, procs-bar). `args` is
/// ps's space-joined line, so the binary's path is all before
/// ` plugins serve `: it can hold spaces (`.../bise cand.app/.../bin/bise`,
/// a release candidate or a renamed app), never split on whitespace.
fn bise_bridge(args: &str) -> bool {
    let Some((bin, rest)) = args.split_once(" plugins serve ") else { return false };
    (bin == "bise" || bin.ends_with("/bise")) && format!(" {rest}").contains(" --parent ")
}

fn descendants(from: &[u32], kids: &BTreeMap<u32, Vec<u32>>) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    let mut todo: Vec<u32> = from.to_vec();
    while let Some(p) = todo.pop() {
        if out.insert(p) {
            todo.extend(kids.get(&p).into_iter().flatten().copied());
        }
    }
    out
}

/// The trees each live agent left running: a bg job is its process group
/// and what that started; any other process of the agent's
/// (`procs::select`: its tag, its REPL's session) outside its REPL's
/// tree (the REPL: its sessions' leaders, the hub's child, repl.json's
/// pid; the turn's running command, its MCP servers) and
/// outside bise's own job runner is a detached tree, by root. Never `me`
/// nor its ancestors.
pub fn trees(rows: &[Row], hub: &str, agents: &[AgentIn], me: u32) -> Vec<Tree> {
    let peer: Vec<PeerProc> = rows
        .iter()
        .map(|r| PeerProc { pid: r.pid, ppid: r.ppid, start: r.start.clone(), zombie: r.zombie, sid: r.sid, owners: r.owners.clone(), command: r.args.clone() })
        .collect();
    let by_pid: BTreeMap<u32, &Row> = rows.iter().map(|r| (r.pid, r)).collect();
    let mut kids: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for r in rows.iter().filter(|r| !r.zombie) {
        kids.entry(r.ppid).or_default().push(r.pid);
    }
    let mut out = Vec::new();
    for a in agents {
        let dirs: BTreeSet<String> = [a.dir.clone()].into();
        let want = crate::procs::Want::Dirs { dirs: &dirs, before: u64::MAX };
        let mine: BTreeSet<u32> = crate::procs::select(&peer, hub, &want, &a.sessions, me).into_iter().map(|p| p.pid).collect();
        // the REPLs (their sessions' leaders, and the hub's own children:
        // a REPL just spawned whose session isn't in repl.sids yet, the
        // adopted one repl.json names) and what they run now
        let repls: Vec<u32> = a
            .sessions
            .iter()
            .copied()
            .chain(a.repl.map(|r| r.pid))
            .chain(mine.iter().copied().filter(|p| by_pid.get(p).is_some_and(|r| r.ppid == me)))
            .filter(|s| by_pid.get(s).is_some_and(|r| !r.zombie))
            .collect::<BTreeSet<u32>>()
            .into_iter()
            .collect();
        let mut out_of_list = descendants(&repls, &kids);
        let mut taken: BTreeSet<u32> = BTreeSet::new();
        for j in &a.bg {
            let group: Vec<u32> = rows.iter().filter(|r| r.pgid == j.pgid && !r.zombie).map(|r| r.pid).collect();
            // every command runs in a slot: one still under its REPL's
            // wrapper is the turn's running command (not handed off yet)
            if group.is_empty() || out_of_list.contains(&j.pgid) {
                continue;
            }
            // bise's runner (the job leader's parent) and its watch loop
            if let Some(runner) = by_pid.get(&j.pgid).map(|l| l.ppid).filter(|p| *p > 1 && !repls.contains(p)) {
                let rg = by_pid.get(&runner).map(|r| r.pgid);
                let mates: Vec<u32> = rows.iter().filter(|r| Some(r.pgid) == rg && r.pgid != j.pgid).map(|r| r.pid).chain([runner]).collect();
                out_of_list.extend(descendants(&mates, &kids).into_iter().filter(|p| by_pid.get(p).is_some_and(|r| r.pgid != j.pgid)));
            }
            let all = descendants(&group, &kids);
            let mut pids: Vec<u32> = all.iter().copied().filter(|p| !taken.contains(p)).collect();
            pids.sort_by_key(|p| (*p != j.pgid, *p));
            taken.extend(pids.iter().copied());
            out.push(Tree { agent: a.name.clone(), dir: a.dir.clone(), kind: ProcKind::Bg { n: j.n }, root: j.pgid, pids, cmd: job_cmd(&j.cmd) });
        }
        // bise's wrappers left after a handoff (a runner, its watch loop
        // and its sleeps) are bise's, never the agent's: whatever its
        // bg job's group or the session they sit in
        // and its plugins bridge (with the servers it runs)
        let wrappers: Vec<u32> = rows
            .iter()
            .filter(|r| !r.zombie && !taken.contains(&r.pid) && (bise_wrapper(&r.args, &a.dir) || (mine.contains(&r.pid) && bise_bridge(&r.args))))
            .map(|r| r.pid)
            .collect();
        out_of_list.extend(descendants(&wrappers, &kids).into_iter().filter(|p| !taken.contains(p)));
        let left: BTreeSet<u32> = mine.iter().copied().filter(|p| !out_of_list.contains(p) && !taken.contains(p) && by_pid.get(p).is_some_and(|r| !r.zombie)).collect();
        for root in left.iter().copied().filter(|p| by_pid.get(p).is_some_and(|r| !left.contains(&r.ppid))) {
            let mut pids: Vec<u32> = descendants(&[root], &kids).into_iter().filter(|p| left.contains(p)).collect();
            pids.sort_by_key(|p| (*p != root, *p));
            let cmd = by_pid.get(&root).map(|r| r.args.clone()).unwrap_or_default();
            out.push(Tree { agent: a.name.clone(), dir: a.dir.clone(), kind: ProcKind::Detached, root, pids, cmd });
        }
    }
    out
}

/// A tree's id: `bg3.<start s>`, `p<pid>.<start s>` (a bg slot's number
/// comes back once its agent cleaned it: the start keeps them apart).
pub fn id_of(kind: ProcKind, root: u32, started_ms: u64) -> String {
    match kind {
        ProcKind::Bg { n } => format!("bg{n}.{}", started_ms / 1000),
        _ => format!("p{root}.{}", started_ms / 1000),
    }
}

/// What the shell found about a tree's root and its pids.
#[derive(Clone, Debug)]
pub struct Found<'a> {
    pub ports: &'a BTreeMap<u32, Vec<u16>>,
    /// the ports that answered HTTP ([`answers_http`]): a row's `url`
    /// is the first of its ports there, else none (a port that is not a
    /// web page never opens an empty browser tab)
    pub http: &'a BTreeSet<u16>,
    pub files: &'a BTreeMap<u32, Files>,
}

/// What the shell's probe sends a port (`daemon/processes.rs`): a HEAD
/// of `/`, the connection closed after it.
pub const HTTP_PROBE: &[u8] = b"HEAD / HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";

/// A port answered HTTP: the first bytes it sent back are a status line
/// (`HTTP/1.1 200 OK`, a 404 or a 500 too: something serves pages
/// there); nothing, a TLS alert or another protocol's greeting is not.
pub fn answers_http(first: &[u8]) -> bool {
    first.starts_with(b"HTTP/")
}

/// A running tree's row: its sums from the same table, its ports, the
/// root's folder, its output's kind.
pub fn row(t: &Tree, rows: &BTreeMap<u32, &Row>, found: &Found) -> Option<Proc> {
    let root = rows.get(&t.root)?;
    let mut ports: Vec<u16> = t.pids.iter().flat_map(|p| found.ports.get(p).into_iter().flatten().copied()).collect();
    ports.sort_unstable();
    ports.dedup();
    let files = found.files.get(&t.root);
    let cpu: f64 = t.pids.iter().filter_map(|p| rows.get(p)).map(|r| r.cpu).sum();
    let rss: u64 = t.pids.iter().filter_map(|p| rows.get(p)).map(|r| r.rss_kb).sum();
    let out = match t.kind {
        ProcKind::Bg { .. } => OutKind::Bg,
        _ if files.and_then(|f| f.out.as_ref()).is_some() => OutKind::File,
        _ => OutKind::None,
    };
    Some(Proc {
        id: id_of(t.kind, t.root, root.started_ms),
        agent: t.agent.clone(),
        kind: t.kind,
        cmd: t.cmd.clone(),
        cwd: files.and_then(|f| f.cwd.clone()),
        pid: t.root,
        started_ms: root.started_ms,
        url: ports.iter().find(|p| found.http.contains(p)).map(|p| format!("http://127.0.0.1:{p}")),
        ports,
        name: server_name(&t.cmd),
        state: ProcState::Running,
        ended_ms: None,
        cpu: (cpu * 10.0).round() / 10.0,
        rss_mb: (rss / 1024) as u32,
        procs: t.pids.len() as u32,
        out,
        fd1: files.and_then(|f| f.fd1),
    })
}

/// How a running row that is gone ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// he stopped it
    You,
    /// its agent stopped, dropped, or the hub quit (proc-cleanup)
    Agent,
    /// by itself: its bg slot's `.rc` when still there
    Exit(Option<i32>),
}

/// `r` ended at `at`, as `why` says.
pub fn ended(r: &Proc, why: Why, at: u64) -> Proc {
    Proc {
        state: match why {
            Why::You => ProcState::StoppedByYou,
            Why::Agent => ProcState::StoppedWith,
            Why::Exit(rc) => ProcState::Exited { rc },
        },
        ended_ms: Some(at),
        cpu: 0.0,
        rss_mb: 0,
        procs: 0,
        ..r.clone()
    }
}

/// The rows of `was` that `now` no longer lists, by (agent, id).
pub fn gone<'a>(was: &'a BTreeMap<(String, String), Proc>, now: &[Proc]) -> Vec<&'a Proc> {
    let keys: BTreeSet<(&str, &str)> = now.iter().map(|p| (p.agent.as_str(), p.id.as_str())).collect();
    was.values().filter(|p| !keys.contains(&(p.agent.as_str(), p.id.as_str()))).collect()
}

/// The ended rows still kept at `now` (24 h), oldest first.
pub fn prune(ended: Vec<Proc>, now: u64) -> Vec<Proc> {
    let mut v: Vec<Proc> = ended.into_iter().filter(|p| p.ended_ms.is_some_and(|e| now.saturating_sub(e) < ENDED_KEEP_MS)).collect();
    v.sort_by_key(|p| p.ended_ms);
    v
}

/// What an ended process's output copy keeps: the last `COPY_LINES`
/// lines and at most `COPY_BYTES` of them (a cut line starts the copy at
/// its next line).
pub fn copy_tail(bytes: &[u8]) -> &[u8] {
    let mut start = bytes.len().saturating_sub(COPY_BYTES);
    if start > 0 && bytes[start - 1] != b'\n' {
        start = bytes[start..].iter().position(|b| *b == b'\n').map(|i| start + i + 1).unwrap_or(bytes.len());
    }
    let tail = &bytes[start..];
    // the start of the COPY_LINES-th line from the end (a last line
    // without its newline counts)
    let body = tail.strip_suffix(b"\n").unwrap_or(tail);
    let mut lines = 0;
    for (i, b) in body.iter().enumerate().rev() {
        if *b == b'\n' {
            lines += 1;
            if lines == COPY_LINES {
                return &tail[i + 1..];
            }
        }
    }
    tail
}

/// The range a `proc_out` reads from a file of `size` bytes: from
/// `from` (past the end: the end), none for its last chunk; at most
/// `OUT_CHUNK` bytes.
pub fn out_range(size: u64, from: Option<u64>) -> (u64, u64) {
    let at = match from {
        Some(f) => f.min(size),
        None => size.saturating_sub(OUT_CHUNK),
    };
    (at, (size - at).min(OUT_CHUNK))
}

/// A stop may signal `pid` only while it is still the process the row
/// saw (same start): a pid the system gave to another process is never
/// hit.
pub fn same_process(rows: &[Row], pid: u32, started_ms: u64) -> bool {
    rows.iter().any(|r| r.pid == pid && !r.zombie && r.started_ms == started_ms)
}

/// The bise line a stop appends to a bg job's output before its kill.
pub const STOPPED_NOTE: &str = "[bise: stopped by you from the processes list]";

/// bise's queued message to the owning agent after his stop.
pub fn stopped_msg(cmd: &str, id: &str) -> String {
    let first = cmd.lines().next().unwrap_or("").trim();
    format!("[bise] he stopped {first} ({id}) from the processes list")
}

/// Each agent's running count and the lowest port its running ones
/// listen on (the `Agent.procs` / `procs_port` fields).
pub fn counts(rows: &[Proc]) -> BTreeMap<String, (u16, Option<u16>)> {
    let mut m: BTreeMap<String, (u16, Option<u16>)> = BTreeMap::new();
    for p in rows.iter().filter(|p| p.running()) {
        let e = m.entry(p.agent.clone()).or_default();
        e.0 = e.0.saturating_add(1);
        e.1 = match (e.1, p.ports.first().copied()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }
    m
}

#[cfg(test)]
#[path = "processes_tests.rs"]
mod tests;
