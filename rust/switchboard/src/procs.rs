//! The processes an agent starts (BISE-243): its bash commands, their
//! background jobs, dev servers, test hubs, tmux servers. The hub kills
//! them when the agent is stopped or archived, when the hub quits for
//! good, and at its start for the agents that are gone
//! (docs/proc-cleanup.md).
//!
//! How they are tracked: each REPL gets `BISE_OWNERS`, a list of tags
//! `<hub>.<dir>.<spawn ms>` (the hub: a hash of its socket path). Every
//! process the agent starts inherits it, even one that leaves its
//! process group or session (`setsid`, `nohup`, a tmux server, a hub
//! that daemonizes) or is reparented to pid 1. A hub an agent starts
//! gives its own REPLs the list it inherited plus its own tag, so what
//! they start is the outer agent's too. A process chooses to outlive its
//! agent with `BISE_OWNERS=` (empty): `scripts/relaunch-live.sh` does.
//!
//! The list is read from the process table at kill time (macOS `ps -E`,
//! Linux `/proc/<pid>/environ`): a reused pid does not carry the tag, so
//! it is never hit. macOS hides the environment of its own binaries
//! (`/bin/sleep`, `/bin/sh`, `/usr/bin/python3`...): for those, the REPL
//! runs in its own session (`setsid`), each REPL's pid is kept in its
//! agent's folder (`repl.sids`), and a process with no visible list in
//! such a session is the agent's. A session whose leader is alive and
//! not that agent's tagged REPL (a reused pid) is ignored. A tagged process's children are the agent's too when
//! they carry no list at all (`env -i`); a child with another agent's
//! tags, or with an empty list, is not. The hub itself and its ancestors
//! are never killed (a hub an agent relaunched carries that agent's tag).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant};

// The tags, the process table and its reading live in `bise-peer` (one
// home with the computer-use broker, docs/issues/18).
pub use bise_peer::table::{parse_ps, snapshot, Proc};
pub use bise_peer::tags::{hub_id, parse_tag, tag, Tag, ENV};

fn split(list: &str) -> impl Iterator<Item = &str> {
    list.split(',').map(str::trim).filter(|t| !t.is_empty())
}

/// The list without this hub's own tags: a hub relaunched by one of its
/// agents is not that agent's (nor are the processes it starts).
pub fn without_hub(list: &str, hub: &str) -> String {
    split(list)
        .filter(|t| parse_tag(t).is_none_or(|t| t.hub != hub))
        .collect::<Vec<_>>()
        .join(",")
}

/// A REPL's list: what the hub inherited (not its own tags) + its tag.
pub fn for_repl(inherited: Option<&str>, hub: &str, dir: &str, ms: u64) -> String {
    let mut l = without_hub(inherited.unwrap_or(""), hub);
    if !l.is_empty() {
        l.push(',');
    }
    l.push_str(&tag(hub, dir, ms));
    l
}

/// Which agents' processes to kill: this hub's tags whose dir is in
/// `dirs` (spawned before `before` ms), or, at the start, whose dir is
/// not one of the live agents'.
pub enum Want<'a> {
    Dirs { dirs: &'a BTreeSet<String>, before: u64 },
    NotLive(&'a BTreeSet<String>),
}

impl Want<'_> {
    fn hits(&self, hub: &str, owners: &str) -> bool {
        split(owners).filter_map(parse_tag).any(|t| {
            t.hub == hub
                && match self {
                    Want::Dirs { dirs, before } => t.ms <= *before && dirs.iter().any(|d| tag(hub, d, 0) == tag(hub, &t.dir, 0)),
                    Want::NotLive(live) => !live.iter().any(|d| tag(hub, d, 0) == tag(hub, &t.dir, 0)),
                }
        })
    }
}

/// The processes to kill, pure: the tagged ones, the ones with no
/// visible list in one of the agents' REPL `sessions`, and their
/// children that carry no list; never `me` nor its ancestors (nor
/// anything below `me` that is not tagged).
pub fn select(procs: &[Proc], hub: &str, want: &Want, sessions: &BTreeSet<u32>, me: u32) -> Vec<Proc> {
    let by_pid: BTreeMap<u32, &Proc> = procs.iter().map(|p| (p.pid, p)).collect();
    // a session is the agent's while its leader is gone (a zombie: the
    // REPL just killed) or is its REPL
    let sessions: BTreeSet<u32> = sessions
        .iter()
        .copied()
        .filter(|s| {
            *s > 1 && by_pid.get(s).filter(|l| !l.zombie).is_none_or(|l| l.owners.as_deref().is_some_and(|o| want.hits(hub, o)))
        })
        .collect();
    let seed = |p: &Proc| match p.owners.as_deref() {
        Some(o) => want.hits(hub, o),
        None => sessions.contains(&p.sid),
    };
    let mut protect: BTreeSet<u32> = [0, 1, me].into();
    let mut p = me;
    while let Some(pp) = by_pid.get(&p).map(|x| x.ppid) {
        if !protect.insert(pp) {
            break;
        }
        p = pp;
    }
    let mut children: BTreeMap<u32, Vec<&Proc>> = BTreeMap::new();
    for p in procs {
        children.entry(p.ppid).or_default().push(p);
    }
    let mut out: BTreeSet<u32> = BTreeSet::new();
    let mut todo: Vec<u32> = procs
        .iter()
        .filter(|p| !p.zombie && seed(p))
        .map(|p| p.pid)
        .collect();
    while let Some(pid) = todo.pop() {
        if protect.contains(&pid) || !out.insert(pid) {
            continue;
        }
        for c in children.get(&pid).into_iter().flatten() {
            let mine = match c.owners.as_deref() {
                None => true,
                Some(o) => want.hits(hub, o),
            };
            if mine && !c.zombie {
                todo.push(c.pid);
            }
        }
    }
    out.iter().filter_map(|p| by_pid.get(p).map(|x| (*x).clone())).collect()
}

extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
    fn setsid() -> i32;
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn close(fd: i32) -> i32;
    fn getdtablesize() -> i32;
}

const F_GETFD: i32 = 1;
const FD_CLOEXEC: i32 = 1;

/// In a forked child, before exec: close every descriptor above stderr
/// that is not close-on-exec. On macOS Rust makes its pipes with
/// `pipe()` then sets close-on-exec: a process another thread spawns in
/// between inherits both ends. A long-lived child (sb-core, a REPL) then
/// holds the write end of that spawn's exec-status pipe open, and the
/// spawn waits for its end forever: the hub's first REPL never started
/// (BISE-291). Our own descriptors are all close-on-exec (Rust opens
/// them so) and the child's stdio is already on 0-2.
/// Async-signal-safe: fcntl and close only.
fn close_leaked_fds() {
    // SAFETY: plain syscalls on descriptor numbers
    unsafe {
        let max = getdtablesize().clamp(256, 65536);
        for fd in 3..max {
            let flags = fcntl(fd, F_GETFD);
            if flags >= 0 && flags & FD_CLOEXEC == 0 {
                close(fd);
            }
        }
    }
}

/// Start `cmd` with no descriptor leaked from a concurrent spawn (see
/// `close_leaked_fds`): every long-lived process the hub starts.
pub fn no_leaked_fds(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: close_leaked_fds is async-signal-safe
    unsafe {
        cmd.pre_exec(|| {
            close_leaked_fds();
            Ok(())
        });
    }
}

/// Run `cmd` in a session of its own (a REPL: its session id is its
/// pid), with no leaked descriptor.
pub fn own_session(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: setsid and close_leaked_fds are async-signal-safe; nothing
    // else runs between fork and exec
    unsafe {
        cmd.pre_exec(|| {
            setsid();
            close_leaked_fds();
            Ok(())
        });
    }
}

/// The REPL session ids kept for an agent (`repl.sids`, one per line).
pub fn read_sids(file: &Path) -> BTreeSet<u32> {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}

pub fn add_sid(file: &Path, pid: u32) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{}", pid);
    }
}

const SIGTERM: i32 = 15;
const SIGKILL: i32 = 9;

/// kill(2) on a bare pid; pid 0 or a pid out of range does nothing (kill
/// with 0 would signal our own process group).
fn send(pid: u32, sig: i32) -> bool {
    let Ok(pid) = i32::try_from(pid) else { return false };
    // SAFETY: kill(2) on a positive pid; a stale one fails with ESRCH
    pid > 0 && unsafe { kill(pid, sig) } == 0
}

/// Is `pid` a live process of ours (`kill -0`, without a `kill` process:
/// every spawn from the hub is a window for the BISE-291 race, BISE-292)?
pub fn alive(pid: u32) -> bool {
    send(pid, 0)
}

/// When `pid` started (`ps -o lstart=`), to tell it from a later process
/// with the same pid (`sb wake --on-exit`); None: no such process.
pub fn start_time(pid: u32) -> Option<String> {
    let out = std::process::Command::new("ps")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).split_whitespace().collect::<Vec<_>>().join(" ");
    (out.status.success() && !s.is_empty()).then_some(s)
}

/// SIGTERM to `pid` (`kill <pid>`).
pub fn terminate(pid: u32) {
    send(pid, SIGTERM);
}

/// SIGKILL to `pid` (`kill -9 <pid>`).
pub fn kill_now(pid: u32) {
    send(pid, SIGKILL);
}

fn signal(p: &Proc, sig: i32) {
    // SAFETY: kill(2) with a pid from the table; a stale pid fails
    unsafe {
        kill(p.pid as i32, sig);
    }
}

/// Kill what `want` names: SIGTERM, up to `grace` for them to go, then
/// SIGKILL to the ones still there (same pid, same start time). Returns
/// the processes it signalled.
pub fn reap(hub: &str, want: &Want, sessions: &BTreeSet<u32>, grace: Duration) -> Vec<Proc> {
    let me = std::process::id();
    let hit = select(&snapshot(), hub, want, sessions, me);
    if hit.is_empty() {
        return hit;
    }
    for p in &hit {
        signal(p, SIGTERM);
    }
    let t0 = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let now: BTreeSet<(u32, String)> = snapshot()
            .into_iter()
            .filter(|p| !p.zombie)
            .map(|p| (p.pid, p.start))
            .collect();
        let left: Vec<&Proc> = hit.iter().filter(|p| now.contains(&(p.pid, p.start.clone()))).collect();
        if left.is_empty() {
            break;
        }
        if t0.elapsed() >= grace {
            for p in left {
                signal(p, SIGKILL);
            }
            break;
        }
    }
    hit
}

/// One line for the hub log: `3 (1234 sleep, 1240 bise, ...)`.
pub fn describe(hit: &[Proc]) -> String {
    let names: Vec<String> = hit
        .iter()
        .take(12)
        .map(|p| format!("{} {}", p.pid, Path::new(&p.command).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()))
        .collect();
    format!("{} ({}{})", hit.len(), names.join(", "), if hit.len() > 12 { ", ..." } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    const F_SETFD: i32 = 2;
    const EEXIST: i32 = 17;

    /// Does a descriptor that is not close-on-exec in the child (as one
    /// leaked by a concurrent spawn) reach exec, with `fix` applied?
    ///
    /// The test's descriptor is opened close-on-exec (Rust's `open` sets
    /// it atomically), so no child another test thread spawns can
    /// inherit it; only this child clears the flag, in a pre_exec run
    /// before `fix`'s. A pre_exec after `fix` fails the spawn with
    /// EEXIST if the descriptor is still open. No pipe, no timing: the
    /// old test made a plain `pipe()` and waited 2 s for EOF, and any
    /// child another test spawned in that window (the control below:
    /// `sleep 5`) kept the write end open: 1 run in 3 failed.
    fn leaks(fix: Option<fn(&mut std::process::Command)>) -> bool {
        use std::os::fd::AsRawFd;
        use std::os::unix::process::CommandExt;
        let f = std::fs::File::open("/dev/null").unwrap();
        let fd = f.as_raw_fd();
        let mut cmd = std::process::Command::new("/usr/bin/true");
        // SAFETY: fcntl only, async-signal-safe; the error carries no allocation
        unsafe {
            cmd.pre_exec(move || {
                fcntl(fd, F_SETFD, 0);
                Ok(())
            });
        }
        if let Some(fix) = fix {
            fix(&mut cmd);
        }
        unsafe {
            cmd.pre_exec(move || match fcntl(fd, F_GETFD) {
                -1 => Ok(()),
                _ => Err(std::io::Error::from_raw_os_error(EEXIST)),
            });
        }
        let leaked = match cmd.spawn() {
            Ok(mut c) => {
                let _ = c.wait();
                false
            }
            Err(e) if e.raw_os_error() == Some(EEXIST) => true,
            Err(e) => panic!("spawn: {}", e),
        };
        drop(f);
        leaked
    }

    /// BISE-291: a descriptor another thread made (not yet close-on-exec,
    /// as Rust's pipes on macOS for a moment) does not stay open in a
    /// long-lived child.
    #[test]
    fn a_long_lived_child_does_not_keep_a_leaked_pipe_open() {
        assert!(!leaks(Some(no_leaked_fds)), "no_leaked_fds: the child kept the leaked descriptor");
        assert!(!leaks(Some(own_session)), "own_session: the child kept the leaked descriptor");
    }

    /// The control: without it, the child does keep the descriptor (the
    /// test above tests something).
    #[test]
    fn a_plain_child_inherits_a_pipe_that_is_not_close_on_exec() {
        assert!(leaks(None));
    }

    fn p(pid: u32, ppid: u32, owners: Option<&str>) -> Proc {
        Proc {
            pid,
            ppid,
            start: format!("s{}", pid),
            zombie: false,
            sid: pid,
            owners: owners.map(str::to_string),
            command: format!("/bin/p{}", pid),
        }
    }

    fn pids(v: Vec<Proc>) -> Vec<u32> {
        v.into_iter().map(|p| p.pid).collect()
    }

    /// A REPL's list keeps the outer hubs' tags, drops its own hub's
    /// (a hub an agent relaunched), and adds its tag.
    #[test]
    fn a_repl_list_nests_the_outer_agents() {
        let h = "00000000000000aa";
        assert_eq!(for_repl(None, h, "t1", 5), "00000000000000aa.t1.5");
        assert_eq!(
            for_repl(Some("bb.x.1,00000000000000aa.main.2"), h, "t 1.b", 5),
            "bb.x.1,00000000000000aa.t_1_b.5"
        );
    }

    /// The tagged processes of the dropped agent go, with their children
    /// that carry no list; not another agent's, not a process that opted
    /// out, not a later REPL of the same agent, never the hub or its
    /// ancestors, and never a pid whose process has no tag.
    #[test]
    fn select_takes_the_agent_s_tree_only() {
        let h = "aa";
        let procs = vec![
            p(1, 0, None),
            p(50, 1, Some("zz.agent.1")),          // an agent of an outer hub
            p(100, 50, Some("zz.agent.1,aa.t1.3")), // the hub, relaunched by t1
            p(101, 100, Some("aa.t1.10")),         // t1's REPL
            p(102, 101, Some("aa.t1.10")),         // its bash
            p(103, 102, None),                     // env -i under it
            p(104, 1, Some("aa.t1.10,cc.x.1")),    // t1's test hub REPL, reparented
            p(105, 102, Some("")),                 // opted out
            p(106, 105, None),                     // under the opted-out one
            p(107, 101, Some("aa.t2.10")),         // t2's (odd parent)
            p(108, 100, Some("aa.t1.99")),         // t1 respawned after the drop
            p(109, 1, None),                       // the user's
        ];
        let dirs: BTreeSet<String> = ["t1".to_string()].into();
        let none = BTreeSet::new();
        let got = pids(select(&procs, h, &Want::Dirs { dirs: &dirs, before: 50 }, &none, 100));
        assert_eq!(got, vec![101, 102, 103, 104]);
        // at the start: every agent that is not live (t2 is)
        let live: BTreeSet<String> = ["t2".to_string()].into();
        assert_eq!(pids(select(&procs, h, &Want::NotLive(&live), &none, 100)), vec![101, 102, 103, 104, 108]);
        // another hub's agent named t1 is not this one
        assert!(select(&procs, "dd", &Want::Dirs { dirs: &dirs, before: 50 }, &none, 100).is_empty());
    }

    /// macOS hides the environment of its own binaries: in a REPL's
    /// session, a process with no visible list is the agent's; not when
    /// the session's leader is alive and someone else (a reused pid).
    #[test]
    fn select_takes_the_hidden_ones_of_the_repl_session() {
        let h = "aa";
        let mut s1 = p(201, 1, None); // /bin/sleep, orphaned, in 101's session
        s1.sid = 101;
        let mut s2 = p(202, 1, None); // in 300's session (leader: the user's)
        s2.sid = 300;
        let mut s3 = p(203, 1, Some("")); // opted out, in 101's session
        s3.sid = 101;
        let mut s4 = p(204, 1, None); // in 400's session (leader gone)
        s4.sid = 400;
        let procs = vec![p(100, 1, None), p(101, 100, Some("aa.t1.10")), p(300, 1, Some("zz.u.1")), s1, s2, s3, s4];
        let dirs: BTreeSet<String> = ["t1".to_string()].into();
        let want = Want::Dirs { dirs: &dirs, before: 50 };
        let sids: BTreeSet<u32> = [101, 300, 400].into();
        assert_eq!(pids(select(&procs, h, &want, &sids, 100)), vec![101, 201, 204]);
        assert_eq!(pids(select(&procs, h, &want, &[1u32].into(), 100)), vec![101]);
        // the REPL just killed, a zombie (no list shown): its session still counts
        let mut procs = procs;
        procs[1].zombie = true;
        procs[1].owners = None;
        assert_eq!(pids(select(&procs, h, &want, &sids, 100)), vec![201, 204]);
    }

}
