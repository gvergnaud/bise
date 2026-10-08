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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

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
