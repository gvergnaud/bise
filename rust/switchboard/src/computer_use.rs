//! The hub's side of computer use (docs/computer-use-design.md §7.3,
//! C6): every stop the broker writes in `events.jsonl` becomes one line
//! in main's feed ("↖ api-v2 stopped driving Chrome · you stopped it"),
//! and an agent dropped (archived) closes its tab group (`bise
//! computer-use drop <agent>`, unless the user touched one of its tabs:
//! the extension decides).
//!
//! The hub never decodes more than it needs: the events file is read
//! from where it ended at the hub's start (old stops are not said
//! again), a few lines per tick.

use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

/// `<run dir>/computer-use` (C6).
fn dir() -> PathBuf {
    bise_home::Home::from_env().run_dir().join("computer-use")
}

/// Follows `events.jsonl` from its end at the start: this hub's agents'
/// events only (the file is the machine's, docs/issues/18).
pub struct Watch {
    file: PathBuf,
    offset: u64,
    /// a line cut by a write in progress
    rest: String,
    /// this hub's id in the tags (`procs::hub_id` of its socket)
    hub: String,
}

impl Watch {
    pub fn new(hub: &str) -> Watch {
        Watch::at(dir().join("events.jsonl"), hub)
    }

    pub fn at(file: PathBuf, hub: &str) -> Watch {
        let offset = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        Watch { file, offset, rest: String::new(), hub: hub.to_string() }
    }

    /// The feed lines of the events written since the last call.
    pub fn poll(&mut self) -> Vec<String> {
        let Ok(mut f) = std::fs::File::open(&self.file) else { return Vec::new() };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            // the file was replaced: from its start
            self.offset = 0;
            self.rest.clear();
        }
        if len == self.offset || f.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut buf = Vec::new();
        // at most 64 KB per tick
        let n = f.take(64 * 1024).read_to_end(&mut buf).unwrap_or(0);
        self.offset += n as u64;
        self.rest.push_str(&String::from_utf8_lossy(&buf));
        let mut out = Vec::new();
        while let Some(i) = self.rest.find('\n') {
            let line: String = self.rest.drain(..=i).collect();
            let ev = serde_json::from_str::<Value>(line.trim()).ok().filter(|ev| ev["hub"] == self.hub.as_str());
            if let Some(t) = ev.as_ref().and_then(feed_line) {
                out.push(t);
            }
        }
        out
    }
}

/// Main's line for one event: a stop only (designer m_3551, m_3896). The
/// agent by its name (the event's `agent` is its key, `<hub>.<dir>`).
pub fn feed_line(ev: &Value) -> Option<String> {
    if ev.get("event")?.as_str()? != "stopped" {
        return None;
    }
    let agent = ev.get("name")?.as_str().filter(|a| !a.is_empty())?;
    let driving = ev.get("driving").and_then(Value::as_str).filter(|d| !d.is_empty());
    let what = match driving {
        Some(d) => format!("↖ {agent} stopped driving {d}"),
        None => format!("↖ {agent} stopped driving"),
    };
    let by = match ev.get("by").and_then(Value::as_str).unwrap_or("you") {
        "cancel_bar" => format!("you pressed Cancel in {}", driving.unwrap_or("Chrome")),
        "group_closed" => "you closed its tab group".to_string(),
        _ => "you stopped it".to_string(),
    };
    Some(format!("{what} · {by}"))
}

/// A journal event that archives an agent: its name.
pub fn archived(ev: &Value) -> Option<&str> {
    (ev.get("type")?.as_str()? == "lifecycle" && ev.get("lifecycle")?.as_str()? == "archived")
        .then(|| ev.get("name")?.as_str())
        .flatten()
}

/// `/drop`: the agent's tab group closes, when the broker knows it
/// (state.json has its key, `<hub>.<dir>`; nothing runs for an agent
/// that never drove).
pub fn drop_agent(hub: &str, dir_name: &str) {
    let agent = bise_peer::tags::agent_key(hub, dir_name);
    let state = std::fs::read_to_string(dir().join("state.json")).unwrap_or_default();
    let known = serde_json::from_str::<Value>(&state).ok().is_some_and(|v| v["agents"].get(&agent).is_some());
    if !known {
        return;
    }
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from(crate::switch::EXE));
    std::thread::spawn(move || {
        let _ = std::process::Command::new(exe)
            .args(["computer-use", "drop", &agent])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    });
}

/// The agent of `key` waits on the user in state.json `v`: stopped (his
/// next message is the go-ahead the stop asked for, C6 m_3893) or paused
/// (he took the wheel; writing to it gives the wheel back).
pub fn held(v: &Value, key: &str) -> bool {
    bise_computer_use::state::agents(v).iter().any(|a| a.key == key && (a.stopped || a.paused))
}

/// Whether computer use is on (the `computer` plugin, what /computer-use
/// sets): off, his words never read state.json.
pub fn is_on() -> bool {
    bend_plugins::state::is_on(&bend_plugins::state::state_path(), "computer")
}

/// His words to the agent of `key`: when state.json holds it (stopped or
/// paused), the broker's `resume` (the call the TUI's ⏎ makes:
/// `bise_computer_use::cli::control`, on the files when no broker runs)
/// on a thread of its own, never waited on: the hub's loop goes on at
/// once (architect m_16548). The model takes seconds before its first
/// computer call, the resume milliseconds. A driving or unknown agent:
/// nothing, no thread. True when it asked.
pub fn resume_if_held(paths: &bise_computer_use::paths::Paths, key: &str) -> bool {
    if !held(&bise_computer_use::state::read(paths), key) {
        return false;
    }
    let (paths, agent) = (paths.clone(), key.to_string());
    std::thread::spawn(move || {
        let _ = bise_computer_use::cli::control(&paths, "resume", &serde_json::json!({"agent": agent}));
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    /// A computer-use world of its own on a temp folder (never the real
    /// run dir), no broker: `control` works on its files.
    fn world(name: &str, agents: Value) -> (std::path::PathBuf, bise_computer_use::paths::Paths) {
        let d = std::env::temp_dir().join(format!("sb-cu-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = bise_computer_use::paths::Paths::new(d.join("run"), d.join("root"), d.join("home"));
        bise_computer_use::state::update(&p, |v| -> std::io::Result<()> {
            *v = json!({"v": 2, "agents": agents, "browsers": [], "apps": {}});
            Ok(())
        })
        .unwrap();
        (d, p)
    }

    const HUB: &str = "0123456789abcdef";

    #[test]
    fn writing_to_a_stopped_or_paused_agent_resumes_it() {
        let a = |k: &str, driving: Option<&str>, paused: bool, stopped: bool| {
            (format!("{HUB}.{k}"), json!({"name": k, "hub": HUB, "driving": driving, "paused": paused, "stopped": stopped}))
        };
        let agents: serde_json::Map<String, Value> = [
            a("halt", None, false, true),
            a("held", Some("Chrome"), true, false),
            a("busy", Some("Chrome"), false, false),
        ]
        .into_iter()
        .collect();
        let (d, p) = world("resume", Value::Object(agents));
        let key = |k: &str| bise_computer_use::who::key(HUB, k);
        assert!(held(&bise_computer_use::state::read(&p), &key("halt")));
        assert!(held(&bise_computer_use::state::read(&p), &key("held")));
        // driving, or never drove: nothing to resume, the file untouched
        let before = std::fs::read(p.state_file()).unwrap();
        assert!(!resume_if_held(&p, &key("busy")));
        assert!(!resume_if_held(&p, &key("never")));
        assert_eq!(std::fs::read(p.state_file()).unwrap(), before);
        // stopped, then paused: resumed (no broker: on the files)
        // both at once, not waited on: each lands on its thread a moment
        // later, and with no broker `control` edits the file under its
        // lock (state::update), so neither loses the other's change
        assert!(resume_if_held(&p, &key("halt")));
        assert!(resume_if_held(&p, &key("held")));
        let resumed = |v: &Value| !held(v, &key("halt")) && !held(v, &key("held"));
        let t = std::time::Instant::now();
        while !resumed(&bise_computer_use::state::read(&p)) && t.elapsed() < std::time::Duration::from_secs(5) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let after = bise_computer_use::state::read(&p);
        assert!(!held(&after, &key("halt")) && !held(&after, &key("held")), "{after}");
        assert!(after["agents"].get(key("busy")).is_some(), "the driving agent is kept: {after}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_stop_is_one_line_in_mains_feed() {
        let l = |v: Value| feed_line(&v);
        assert_eq!(
            l(json!({"t": 1, "agent": "aa.api-v2", "name": "api-v2", "hub": "aa", "event": "stopped", "by": "you", "driving": "Chrome"})).as_deref(),
            Some("↖ api-v2 stopped driving Chrome · you stopped it")
        );
        assert_eq!(
            l(json!({"name": "api-v2", "event": "stopped", "by": "cancel_bar", "driving": "Edge"})).as_deref(),
            Some("↖ api-v2 stopped driving Edge · you pressed Cancel in Edge")
        );
        assert_eq!(
            l(json!({"name": "perf", "event": "stopped", "by": "group_closed", "driving": null})).as_deref(),
            Some("↖ perf stopped driving · you closed its tab group")
        );
        assert_eq!(l(json!({"agent": "perf", "event": "paused", "by": "you"})), None);
        assert_eq!(l(json!({"agent": "perf", "event": "resumed", "by": "you"})), None);
    }

    #[test]
    fn the_watch_reads_what_comes_after_its_start() {
        let d = std::env::temp_dir().join(format!("sb-cu-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("events.jsonl");
        let mut w = std::fs::File::create(&f).unwrap();
        writeln!(w, "{}", json!({"agent": "aa.old", "name": "old", "hub": "aa", "event": "stopped", "by": "you"})).unwrap();
        let mut watch = Watch::at(f.clone(), "aa");
        assert!(watch.poll().is_empty(), "old stops are not said again");
        writeln!(w, "{}", json!({"agent": "aa.a", "name": "a", "hub": "aa", "event": "paused", "by": "you"})).unwrap();
        // another project's perf, and an agent of no hub: not this hub's feed
        writeln!(w, "{}", json!({"agent": "bb.perf", "name": "perf", "hub": "bb", "event": "stopped", "by": "you"})).unwrap();
        writeln!(w, "{}", json!({"agent": "bench", "name": "bench", "hub": null, "event": "stopped", "by": "you"})).unwrap();
        write!(w, "{}", json!({"agent": "aa.b", "name": "b", "hub": "aa", "event": "stopped", "by": "you", "driving": "Chrome"})).unwrap();
        assert!(watch.poll().is_empty(), "a line without its end waits");
        writeln!(w).unwrap();
        assert_eq!(watch.poll(), ["↖ b stopped driving Chrome · you stopped it"]);
        assert!(watch.poll().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_archived_agent_is_named() {
        assert_eq!(archived(&json!({"type": "lifecycle", "name": "api-v2", "lifecycle": "archived", "reason": null})), Some("api-v2"));
        assert_eq!(archived(&json!({"type": "lifecycle", "name": "api-v2", "lifecycle": "active"})), None);
        assert_eq!(archived(&json!({"type": "snapshot", "name": "api-v2"})), None);
    }
}
