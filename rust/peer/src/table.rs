//! The process table of this user, each process with its `BISE_OWNERS`
//! (macOS `ps -E`, Linux `/proc/<pid>/environ`). macOS hides the
//! environment of its own binaries (`/bin/sh`, `/usr/bin/python3`...):
//! their `owners` is None, and the session id (`sid`) tells whose they
//! are.

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
    rd.flatten()
        .filter_map(|e| {
            let pid: u32 = e.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(e.path().join("stat")).ok()?;
            let (head, rest) = stat.rsplit_once(')')?;
            let f: Vec<&str> = rest.split_whitespace().collect();
            let env = std::fs::read(e.path().join("environ")).ok()?;
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
        })
        .collect()
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
}
