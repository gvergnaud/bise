//! The process table of this user, each process with its `BISE_OWNERS`
//! (macOS `ps -E`, Linux `/proc/<pid>/environ`). macOS hides the
//! environment of its own binaries (`/bin/sh`, `/usr/bin/python3`...):
//! their `owners` is None, and the session id (`sid`) tells whose they
//! are. [`snapshot`] reads all of it (`ps -axE`); [`around`] reads only
//! the processes a judge walks, one by one, cheap enough per request.

use crate::tags::ENV;

/// One process of the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    /// its start time, as the system says it: with the pid, who it is
    pub start: String,
    pub zombie: bool,
    /// its session id (`getsid`)
    pub sid: u32,
    /// `BISE_OWNERS`: None when it has no such variable
    pub owners: Option<String>,
    pub command: String,
}

/// `ps -axww -E -o pid=,ppid=,stat=,lstart=,command=` (macOS): the
/// environment comes after the command, one `K=V` word each; the last
/// ` BISE_OWNERS=` word is the variable (a command naming it comes first).
pub fn parse_ps(text: &str) -> Vec<Proc> {
    let key = format!(" {}=", ENV);
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let pid = w.next()?.parse().ok()?;
            let ppid = w.next()?.parse().ok()?;
            let stat = w.next()?;
            let start: Vec<&str> = w.by_ref().take(5).collect();
            if start.len() < 5 {
                return None;
            }
            let command = w.next().unwrap_or("").to_string();
            let owners = l.rfind(&key).map(|i| {
                l[i + key.len()..].split_whitespace().next().filter(|v| !v.contains('=')).unwrap_or("").to_string()
            });
            Some(Proc {
                pid,
                ppid,
                start: start.join(" "),
                zombie: stat.starts_with('Z'),
                sid: 0,
                owners,
                command,
            })
        })
        .collect()
}

/// Only the processes a judge of `pids` reads ([`crate::judge`]'s walk):
/// each pid and its parents up to pid 1, then the leaders of their
/// sessions, read one by one (macOS `proc_pidinfo` and
/// `KERN_PROCARGS2`, what `ps -E` reads; Linux `/proc/<pid>`): about a
/// thousand times cheaper than [`snapshot`] (`ps -axE`, ~200 ms on a busy
/// Mac), so a hub can judge every request. A pid it cannot read is
/// missing (a peer gone mid-read: the judge says Gone). `start` is left
/// empty: the judge never reads it (the cleanup sweeps use [`snapshot`]).
pub fn around(pids: &[u32]) -> Vec<Proc> {
    let mut got: std::collections::BTreeMap<u32, Proc> = Default::default();
    for &from in pids {
        let mut cur = from;
        while cur > 1 && !got.contains_key(&cur) {
            let Some(p) = one(cur) else { break };
            let next = p.ppid;
            got.insert(cur, p);
            cur = next;
        }
    }
    let leaders: Vec<u32> = got.values().map(|p| p.sid).filter(|s| *s > 1 && !got.contains_key(s)).collect();
    for s in leaders {
        if let Some(p) = one(s) {
            got.insert(s, p);
        }
    }
    got.into_values().collect()
}

/// One process, read alone; None when it is gone or unreadable. A
/// zombie (killed, not reaped yet) is Some with `zombie` set: it answers
/// `kill -0`, but it runs nothing.
#[cfg(target_os = "macos")]
pub fn one(pid: u32) -> Option<Proc> {
    // SAFETY: proc_pidinfo writes at most `size` bytes into `info`
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let n = unsafe { libc::proc_pidinfo(pid as libc::c_int, libc::PROC_PIDTBSDINFO, 0, &mut info as *mut _ as *mut libc::c_void, size) };
    if n != size {
        return None;
    }
    let zombie = info.pbi_status == libc::SZOMB;
    let (command, env) = match (zombie, procargs(pid)) {
        (true, _) => (String::new(), vec![]),
        (false, Some(a)) => a,
        // this user's process whose arguments can't be read is exiting:
        // gone (fail closed: never a process with no tag)
        // SAFETY: getuid(2) cannot fail
        (false, None) if info.pbi_uid == unsafe { libc::getuid() } => return None,
        // another user's (root's login...): no list, like `ps -E`
        (false, None) => (String::new(), vec![]),
    };
    // SAFETY: getsid(2) on a pid; a gone one returns -1
    let sid = unsafe { libc::getsid(pid as libc::pid_t) };
    if sid < 0 {
        return None;
    }
    let key = format!("{}=", ENV);
    Some(Proc {
        pid,
        ppid: info.pbi_ppid,
        start: String::new(),
        zombie,
        sid: sid as u32,
        owners: env.iter().find_map(|kv| kv.strip_prefix(&key).map(str::to_string)),
        command,
    })
}

/// `KERN_PROCARGS2` of `pid`: its argv[0] and its environment. Its
/// layout: argc (an int), the exec path, NULs, argc args, then the
/// environment up to an empty string.
#[cfg(target_os = "macos")]
fn procargs(pid: u32) -> Option<(String, Vec<String>)> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
    let mut len: libc::size_t = 0;
    // SAFETY: a size query (null buffer), then a read of at most `len` bytes
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) } != 0 || len < 4 {
        return None;
    }
    let mut buf = vec![0u8; len];
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr() as *mut libc::c_void, &mut len, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    buf.truncate(len);
    Some(parse_procargs(&buf))
}

/// The pure half of [`procargs`] (tested without a process).
fn parse_procargs(buf: &[u8]) -> (String, Vec<String>) {
    let (path, argv, env) = split_procargs(buf);
    (argv.into_iter().next().filter(|a| !a.is_empty()).unwrap_or(path), env)
}

/// `KERN_PROCARGS2`'s parts: the exec path, argv, the environment.
fn split_procargs(buf: &[u8]) -> (String, Vec<String>, Vec<String>) {
    let Some(head) = buf.get(..4) else { return Default::default() };
    let argc = i32::from_ne_bytes([head[0], head[1], head[2], head[3]]).max(0) as usize;
    let rest = &buf[4..];
    // the exec path, then its NUL padding
    let path_end = rest.iter().position(|b| *b == 0).unwrap_or(rest.len());
    let path = String::from_utf8_lossy(&rest[..path_end]).to_string();
    let mut i = path_end;
    while rest.get(i) == Some(&0) {
        i += 1;
    }
    let mut words = rest[i.min(rest.len())..].split(|b| *b == 0).map(|w| String::from_utf8_lossy(w).to_string());
    let argv: Vec<String> = words.by_ref().take(argc).collect();
    let env = words.take_while(|w| !w.is_empty()).collect();
    (path, argv, env)
}

/// `pid`'s argv, each argument whole (`ps -o args` joins them with
/// spaces and loses their quoting); None when it is gone or unreadable.
#[cfg(target_os = "macos")]
pub fn argv(pid: u32) -> Option<Vec<String>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
    let mut len: libc::size_t = 0;
    // SAFETY: a size query (null buffer), then a read of at most `len` bytes
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) } != 0 || len < 4 {
        return None;
    }
    let mut buf = vec![0u8; len];
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr() as *mut libc::c_void, &mut len, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    buf.truncate(len);
    Some(split_procargs(&buf).1).filter(|a| !a.is_empty())
}

/// `pid`'s argv (`/proc/<pid>/cmdline`, NUL-separated).
#[cfg(target_os = "linux")]
pub fn argv(pid: u32) -> Option<Vec<String>> {
    let b = std::fs::read(std::path::Path::new("/proc").join(pid.to_string()).join("cmdline")).ok()?;
    let b = b.strip_suffix(&[0]).unwrap_or(&b);
    Some(b.split(|c| *c == 0).map(|w| String::from_utf8_lossy(w).to_string()).collect::<Vec<_>>()).filter(|a| !a.is_empty() && !b.is_empty())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn argv(_: u32) -> Option<Vec<String>> {
    None
}

#[cfg(target_os = "linux")]
pub fn one(pid: u32) -> Option<Proc> {
    linux_proc(pid, &std::path::Path::new("/proc").join(pid.to_string()))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn one(_: u32) -> Option<Proc> {
    None
}

/// The process table of this user, with each process's `BISE_OWNERS`.
pub fn snapshot() -> Vec<Proc> {
    #[cfg(target_os = "linux")]
    {
        linux_snapshot()
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::process::Command::new("ps")
            .args(["-axww", "-E", "-o", "pid=,ppid=,stat=,lstart=,command="])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .map(|o| parse_ps(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
            .into_iter()
            .map(|mut p| {
                // SAFETY: getsid(2) on a pid; a stale one returns -1
                p.sid = unsafe { libc::getsid(p.pid as libc::pid_t) }.max(0) as u32;
                p
            })
            .collect()
    }
}

#[cfg(target_os = "linux")]
fn linux_snapshot() -> Vec<Proc> {
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return vec![];
    };
    rd.flatten().filter_map(|e| linux_proc(e.file_name().to_str()?.parse().ok()?, &e.path())).collect()
}

/// One process of `/proc` (`dir` is `/proc/<pid>`).
#[cfg(target_os = "linux")]
fn linux_proc(pid: u32, dir: &std::path::Path) -> Option<Proc> {
    let stat = std::fs::read_to_string(dir.join("stat")).ok()?;
    let (head, rest) = stat.rsplit_once(')')?;
    let f: Vec<&str> = rest.split_whitespace().collect();
    let env = std::fs::read(dir.join("environ")).ok()?;
    let key = format!("{}=", ENV);
    let owners = env
        .split(|b| *b == 0)
        .filter_map(|kv| std::str::from_utf8(kv).ok())
        .find_map(|kv| kv.strip_prefix(&key).map(str::to_string));
    Some(Proc {
        pid,
        ppid: f.get(1)?.parse().ok()?,
        start: f.get(19)?.to_string(),
        zombie: f.first() == Some(&"Z"),
        sid: f.get(3)?.parse().ok()?,
        owners,
        command: head.split_once('(').map(|(_, c)| c.to_string()).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps_lines_give_the_list() {
        let text = "  101   100 S    Tue Oct  7 00:48:12 2026 /x/repl-live A=1 BISE_OWNERS=aa.t1.3 B=2
  102 101 Z+ Tue Oct  7 00:48:13 2026 rg BISE_OWNERS=aa.t9.1 x HOME=/h BISE_OWNERS=aa.t1.3
  103 1 S Tue Oct  7 00:48:13 2026 sleep 5 HOME=/h BISE_OWNERS= TERM=x
  104 1 S Tue Oct  7 00:48:13 2026 sleep 5 HOME=/h
";
        let v = parse_ps(text);
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].owners.as_deref(), Some("aa.t1.3"));
        assert_eq!(v[0].start, "Tue Oct 7 00:48:12 2026");
        assert_eq!(v[0].command, "/x/repl-live");
        assert!(v[1].zombie && v[1].owners.as_deref() == Some("aa.t1.3"));
        assert_eq!(v[2].owners.as_deref(), Some(""));
        assert_eq!(v[3].owners, None);
    }

    #[test]
    fn procargs_give_the_whole_argv() {
        let mut b = 3i32.to_ne_bytes().to_vec();
        b.extend_from_slice(b"/bin/sh\0\0\0sh\0-c\0while true; do date; done\0A=1\0\0");
        let (path, argv, env) = split_procargs(&b);
        assert_eq!((path.as_str(), env), ("/bin/sh", vec!["A=1".to_string()]));
        assert_eq!(argv, ["sh", "-c", "while true; do date; done"]);
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn argv_reads_a_live_process() {
        let mut c = std::process::Command::new("/bin/sh").args(["-c", "sleep 5; echo 'a b'"]).spawn().unwrap();
        let got = argv(c.id());
        let _ = c.kill();
        let _ = c.wait();
        assert_eq!(got.unwrap(), ["/bin/sh", "-c", "sleep 5; echo 'a b'"]);
    }

    #[test]
    fn procargs_give_argv0_and_the_environment() {
        let mut b = 2i32.to_ne_bytes().to_vec();
        b.extend_from_slice(b"/x/sb\0\0\0\0sb\0list\0A=1\0BISE_OWNERS=aa.t1.3\0\0junk\0");
        let (cmd, env) = parse_procargs(&b);
        assert_eq!(cmd, "sb");
        assert_eq!(env, ["A=1", "BISE_OWNERS=aa.t1.3"]);
        // no argv: the exec path; truncated or empty: nothing, no panic
        let mut b = 0i32.to_ne_bytes().to_vec();
        b.extend_from_slice(b"/x/sb\0\0E=1\0");
        assert_eq!(parse_procargs(&b), ("/x/sb".to_string(), vec!["E=1".to_string()]));
        assert_eq!(parse_procargs(&[1, 0]), Default::default());
        assert_eq!(parse_procargs(&9i32.to_ne_bytes()).1, Vec::<String>::new());
    }

    /// A child of this test with a tag: this test binary, running
    /// [`sleeper`] (macOS hides the environment of its own binaries, so
    /// never `/bin/sleep`). `spawn` returns once it has exec'd.
    fn tagged_child(tag: &str) -> std::process::Child {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "table::tests::sleeper", "--ignored", "--test-threads=1"])
            .env(ENV, tag)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap()
    }

    /// Not a test: the body of [`tagged_child`].
    #[test]
    #[ignore]
    fn sleeper() {
        std::thread::sleep(std::time::Duration::from_secs(30));
    }

    /// On the real OS: what the judge reads in [`around`] is what it reads
    /// in the whole table, so its verdict is the same (the guard of the
    /// cheap reader, docs/issues/17).
    #[test]
    fn around_reads_what_the_judge_reads_in_the_whole_table() {
        use crate::judge::{judge, judge_any, HubSide};
        let mut c = tagged_child("aa.t9.1");
        let me = std::process::id();
        let whole = snapshot();
        let near = around(&[c.id(), me]);
        let _ = c.kill();
        let _ = c.wait();
        let p = near.iter().find(|p| p.pid == c.id()).expect("the child, read alone");
        assert_eq!(p.ppid, me);
        assert_eq!(p.owners.as_deref(), Some("aa.t9.1"), "its tag, from KERN_PROCARGS2 / environ");
        for q in &near {
            let w = whole.iter().find(|w| w.pid == q.pid).unwrap_or_else(|| panic!("{} in the whole table", q.pid));
            assert_eq!((q.ppid, q.sid, q.zombie, &q.owners), (w.ppid, w.sid, w.zombie, &w.owners), "pid {}", q.pid);
        }
        let hub = HubSide { pid: 1, id: "aa".into(), owners: Default::default(), siblings: ["aa".to_string()].into() };
        for pid in [c.id(), me] {
            assert_eq!(judge(&near, pid, &hub), judge(&whole, pid, &hub), "judge of {pid}");
            assert_eq!(judge_any(&near, pid, 1), judge_any(&whole, pid, 1), "judge_any of {pid}");
        }
        assert_eq!(judge(&near, c.id(), &hub), crate::judge::Who::Agent("t9".into()));
    }

    /// On the real OS: a peer gone before or while it is read is Gone,
    /// never Outside, and nothing panics.
    #[test]
    fn a_peer_gone_mid_read_is_gone() {
        use crate::judge::{judge, HubSide, Who};
        let hub = HubSide { pid: std::process::id(), id: "aa".into(), owners: Default::default(), siblings: Default::default() };
        let mut c = tagged_child("aa.t9.1");
        let pid = c.id();
        let _ = c.kill();
        let _ = c.wait();
        assert!(around(&[pid]).iter().all(|p| p.pid != pid), "a reaped pid is not read");
        assert_eq!(judge(&around(&[pid, hub.pid]), pid, &hub), Who::Gone);
        // killed while the walk runs: Gone or (read first) the agent, never Outside
        for _ in 0..20 {
            let mut c = tagged_child("aa.t9.1");
            let pid = c.id();
            let reader = std::thread::spawn(move || around(&[pid]));
            let _ = c.kill();
            let table = reader.join().expect("no panic");
            let _ = c.wait();
            assert_ne!(judge(&table, pid, &hub), Who::Outside, "pid {pid}: {table:?}");
        }
    }
}
