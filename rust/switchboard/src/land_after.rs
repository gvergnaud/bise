//! The job after a land (c5-tip-per-land, architect m_55196): `bise.toml`'s
//! `[land] after` ([`crate::flow::land_after`], read from the trunk's new
//! tip), started by the hub after a land that moved the trunk, as
//! `<after> <base> <tip> <agent>`: argv, never a shell string. This repo:
//! `scripts/tip_check.py`, the quick gate on main after each land.
//!
//! Never waited by the land: started from the land's own thread (after
//! its record, off the hub's loop), its exit reaped by a thread of its
//! own. Not an orphan nobody sees: it carries main's owner tag
//! (`BISE_OWNERS`, [`crate::procs`]), so the processes list shows it
//! under main, his stop stops it, and the hub's quit stops it with
//! main's. Nobody is woken: what it finds goes through its own files
//! (the tip check's red: `tip-red`, which main forwards).
//!
//! [`wanted`] and [`argv`] decide, pure; [`start`] runs the effect. A
//! missing program is one hub.log line; the land is never touched.

use std::path::{Path, PathBuf};

/// Whether a land starts the after job: it moved the trunk (onto it,
/// with commits) and the repo names one.
pub fn wanted(onto_trunk: bool, commits: usize, after: Option<&str>) -> bool {
    onto_trunk && commits > 0 && after.is_some_and(|a| !a.trim().is_empty())
}

/// The job's argv: `after`'s words, then base, tip and the agent; a
/// relative program is the shared folder's file. None: no word.
pub fn argv(shared: &Path, after: &str, base: &str, tip: &str, agent: &str) -> Option<(PathBuf, Vec<String>)> {
    let mut words = after.split_whitespace();
    let prog = words.next()?;
    let prog = if Path::new(prog).is_absolute() { PathBuf::from(prog) } else { shared.join(prog) };
    let args = words.map(str::to_string).chain([base, tip, agent].map(str::to_string)).collect();
    Some((prog, args))
}

/// A land as the after job sees it: the trunk's checkout, whether it
/// went onto the trunk, its commits `base..tip` and who landed them.
pub struct Moved<'a> {
    pub shared: &'a Path,
    pub onto_trunk: bool,
    pub commits: usize,
    pub base: &'a str,
    pub tip: &'a str,
    pub agent: &'a str,
}

/// Start the after job of the land `m`, if `wanted`: its output appended
/// to `log`, `owners` its `BISE_OWNERS` (main's tag). Returns its pid,
/// or None (not wanted, or it could not start: said on stderr, the
/// hub's log).
pub fn start(m: &Moved, owners: &str, log: &Path) -> Option<u32> {
    let shared = m.shared;
    let text = crate::land::git(shared, &["show", &format!("{}:{}", m.tip, crate::flow::TRACKED)]).ok();
    let after = text.as_deref().and_then(crate::flow::land_after);
    if !wanted(m.onto_trunk, m.commits, after.as_deref()) {
        return None;
    }
    let (prog, args) = argv(shared, after.as_deref()?, m.base, m.tip, m.agent)?;
    if !prog.exists() {
        eprintln!("land after: {} not found in {}: nothing started", prog.display(), shared.display());
        return None;
    }
    let out = std::fs::OpenOptions::new().create(true).append(true).open(log);
    let (o, e) = match out.and_then(|f| f.try_clone().map(|g| (f, g))) {
        Ok((f, g)) => (std::process::Stdio::from(f), std::process::Stdio::from(g)),
        Err(_) => (std::process::Stdio::null(), std::process::Stdio::null()),
    };
    let child = std::process::Command::new(&prog)
        .args(&args)
        .current_dir(shared)
        .env(crate::procs::ENV, owners)
        .env_remove("SB_SOCKET")
        .env_remove("SB_AGENT")
        .stdin(std::process::Stdio::null())
        .stdout(o)
        .stderr(e)
        .spawn();
    match child {
        Ok(mut c) => {
            let pid = c.id();
            std::thread::spawn(move || drop(c.wait()));
            Some(pid)
        }
        Err(err) => {
            eprintln!("land after: {} did not start: {}", prog.display(), err);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_land_that_moved_the_trunk_starts_it() {
        assert!(wanted(true, 2, Some("scripts/tip_check.py")));
        assert!(!wanted(false, 2, Some("scripts/tip_check.py")), "a feature or a branch: no");
        assert!(!wanted(true, 0, Some("scripts/tip_check.py")), "nothing moved: no");
        assert!(!wanted(true, 2, None) && !wanted(true, 2, Some("  ")), "no [land] after: no");
    }

    #[test]
    fn its_argv_is_words_then_base_tip_agent_never_a_shell() {
        let (p, a) = argv(Path::new("/r"), "scripts/tip_check.py --x", "b1", "t1", "ag").unwrap();
        assert_eq!(p, PathBuf::from("/r/scripts/tip_check.py"));
        assert_eq!(a, ["--x", "b1", "t1", "ag"]);
        let (p, a) = argv(Path::new("/r"), "/bin/echo", "b; rm -rf /", "t", "a").unwrap();
        assert_eq!((p, a.len()), (PathBuf::from("/bin/echo"), 3), "a base with a ; stays one argument");
        assert!(argv(Path::new("/r"), "   ", "b", "t", "a").is_none());
    }

    #[test]
    fn it_starts_with_main_s_tag_and_a_missing_program_starts_nothing() {
        let root = std::env::temp_dir().join(format!("sb-land-after-{}-{}", std::process::id(), crate::util::now_ms()));
        let ws = root.join("repo");
        std::fs::create_dir_all(ws.join("scripts")).unwrap();
        let sh = |s: &str| assert!(std::process::Command::new("/bin/sh").arg("-c").arg(s).current_dir(&ws).status().unwrap().success(), "{}", s);
        sh("git init -q -b main && git config user.email t@t && git config user.name t && git config commit.gpgsign false");
        sh("printf '[land]\\nafter = \"scripts/after.sh\"\\n' > bise.toml && printf '#!/bin/sh\\necho \"args=$* owners=$BISE_OWNERS\" > out.txt\\n' > scripts/after.sh && chmod +x scripts/after.sh && git add . && git commit -qm a");
        let tip = crate::land::git(&ws, &["rev-parse", "HEAD"]).unwrap();
        let log = root.join("after.log");
        let moved = |onto_trunk: bool, base: &'static str, tip: &str| -> Option<u32> {
            start(&Moved { shared: &ws, onto_trunk, commits: 1, base, tip, agent: "alpha" }, "h.main.1", &log)
        };
        assert!(moved(false, "b0", &tip).is_none(), "not onto the trunk");
        let pid = moved(true, "b0", &tip);
        assert!(pid.is_some());
        let out = ws.join("out.txt");
        let t0 = std::time::Instant::now();
        while !std::fs::read_to_string(&out).is_ok_and(|s| s.ends_with('\n')) && t0.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(std::fs::read_to_string(&out).unwrap(), format!("args=b0 {} alpha owners=h.main.1\n", tip));
        sh("git rm -q scripts/after.sh && git commit -qm gone");
        let tip2 = crate::land::git(&ws, &["rev-parse", "HEAD"]).unwrap();
        let gone = Moved { shared: &ws, onto_trunk: true, commits: 1, base: &tip, tip: &tip2, agent: "alpha" };
        assert!(start(&gone, "h.main.1", &log).is_none(), "its program gone: nothing");
        let _ = std::fs::remove_dir_all(&root);
    }
}
