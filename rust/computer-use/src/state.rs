//! C6's files: `state.json` (rewritten whole on each change; the TUI reads
//! it by mtime) and `events.jsonl` (appended; the hub turns `stopped` into
//! main's feed line).

use std::collections::BTreeMap;
use std::io::Write;

use serde_json::{json, Map, Value};

use crate::paths::Paths;

/// One agent, as the TUI shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Agent {
    /// "Chrome", "Edge", "TextEdit": what it drives now; None when idle
    pub driving: Option<String>,
    /// "amazon.fr", "TextEdit"
    pub place: Option<String>,
    pub since_ms: Option<u64>,
    /// the targets the user took over (C4/C5 `paused`)
    pub paused: Vec<String>,
    pub stopped: bool,
    /// what it drove when it was stopped (`driving` is None by then): the
    /// stopped line says it ("↖ you stopped it driving Chrome")
    pub was: Option<String>,
    /// who stopped it (`you`, `cancel_bar`, `group_closed`, as in
    /// events.jsonl)
    pub stopped_by: Option<String>,
}

impl Agent {
    /// Nothing to show: dropped from the file.
    pub fn idle(&self) -> bool {
        self.driving.is_none() && self.paused.is_empty() && !self.stopped
    }

    fn json(&self) -> Value {
        let mut v = json!({
            "driving": self.driving,
            "where": self.place,
            "since_ms": self.since_ms,
            "paused": !self.paused.is_empty(),
            "stopped": self.stopped,
        });
        // only a stopped agent has them: absent, not null, otherwise
        for (k, x) in [("was", &self.was), ("stopped_by", &self.stopped_by)] {
            if let Some(x) = x {
                v[k] = json!(x);
            }
        }
        v
    }
}

/// An agent's key's fields (docs/issues/18): `name` (what the user sees)
/// and `hub` (its hub's id, null for an untagged agent). A reader shows
/// only its own hub's agents, or names the project.
fn who(key: &str) -> (Value, Value) {
    let (hub, name) = crate::who::split(key);
    (json!(name), json!(hub))
}

/// The whole state.json (v2: `agents` keyed by the agent's key, each with
/// its `name` and `hub`).
pub fn render(agents: &BTreeMap<String, Agent>, browsers: Value, apps: Value) -> Value {
    let a: Map<String, Value> = agents
        .iter()
        .filter(|(_, a)| !a.idle())
        .map(|(k, a)| {
            let mut v = a.json();
            (v["name"], v["hub"]) = who(k);
            (k.clone(), v)
        })
        .collect();
    json!({"v": 2, "agents": a, "browsers": browsers, "apps": apps})
}

/// One agent of state.json as its readers see it (the TUI's marks, the
/// desktop core's live rows): the file's one reader, next to its writer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Live {
    /// `<hub>.<dir>` (docs/issues/18): what the ctl ops take
    pub key: String,
    /// its hub's tag id (`bise_peer::tags::hub_id` of its socket); None
    /// for an untagged agent
    pub hub: Option<String>,
    /// what the user sees
    pub name: String,
    pub driving: Option<String>,
    pub place: Option<String>,
    pub since_ms: Option<u64>,
    pub paused: bool,
    pub stopped: bool,
    /// a stopped agent: what it drove then, and who stopped it
    pub was: Option<String>,
    pub stopped_by: Option<String>,
}

/// state.json's agents ([`render`]'s `agents`), in key order; an entry
/// without a name is skipped (a v1 file's, keyed by bare name, has none).
pub fn agents(v: &Value) -> Vec<Live> {
    let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).filter(|t| !t.is_empty()).map(String::from);
    let b = |x: &Value, k: &str| x.get(k).and_then(Value::as_bool).unwrap_or(false);
    let Some(m) = v.get("agents").and_then(Value::as_object) else { return Vec::new() };
    m.iter()
        .filter_map(|(key, a)| {
            Some(Live {
                key: key.clone(),
                hub: s(a, "hub"),
                name: s(a, "name")?,
                driving: s(a, "driving"),
                place: s(a, "where"),
                since_ms: a.get("since_ms").and_then(Value::as_u64),
                paused: b(a, "paused"),
                stopped: b(a, "stopped"),
                was: s(a, "was"),
                stopped_by: s(a, "stopped_by"),
            })
        })
        .collect()
}

/// The one writer of state.json: `f` changes the file's value (read
/// under the lock) and, when it returns Ok, the result is written. The
/// whole read-change-write holds an exclusive flock on
/// `state.json.lock`, so writers that race (the broker, `control` with
/// no broker from the hub's resume threads, the CLI) lose no change. An
/// Err from `f` writes nothing.
pub fn update<T, E: From<std::io::Error>>(paths: &Paths, f: impl FnOnce(&mut Value) -> Result<T, E>) -> Result<T, E> {
    paths.ensure()?;
    let _lock = lock(&paths.state_file().with_extension("json.lock"))?;
    let mut v = read(paths);
    let out = f(&mut v)?;
    write(paths, &v)?;
    Ok(out)
}

/// Each write's own tmp name in this process (with the pid: across them).
static TMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// An exclusive flock on `path`, held until the file is dropped.
fn lock(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::io::AsRawFd;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
    // SAFETY: flock on a descriptor this function owns; blocks until free
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(f)
}

/// Write state.json atomically (0600), through a tmp name only this call
/// uses, removed when the rename fails. Only [`update`] calls it, under
/// the lock.
fn write(paths: &Paths, v: &Value) -> std::io::Result<()> {
    let f = paths.state_file();
    let n = TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = f.with_extension(format!("json.tmp-{}-{n}", std::process::id()));
    write_tmp(&tmp, v).and_then(|()| std::fs::rename(&tmp, &f)).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn write_tmp(tmp: &std::path::Path, v: &Value) -> std::io::Result<()> {
    std::fs::write(tmp, serde_json::to_string_pretty(v).unwrap_or_default() + "\n")?;
    crate::paths::private(tmp, 0o600)
}

pub fn read(paths: &Paths) -> Value {
    std::fs::read_to_string(paths.state_file())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({"agents": {}, "browsers": [], "apps": {}}))
}

/// The agents a new broker starts with: the stopped ones stay stopped
/// (a restart must not hand the wheel back), and the paused stay paused.
/// A v1 file (agents keyed by bare name, before docs/issues/18) gives
/// none: its names are no agent's key.
pub fn restore(paths: &Paths) -> BTreeMap<String, Agent> {
    let v = read(paths);
    let mut out = BTreeMap::new();
    if v["v"] != 2 {
        return out;
    }
    if let Some(m) = v.get("agents").and_then(Value::as_object) {
        for (name, a) in m {
            let stopped = a.get("stopped").and_then(Value::as_bool).unwrap_or(false);
            if stopped {
                let s = |k: &str| a.get(k).and_then(Value::as_str).map(String::from);
                out.insert(name.clone(), Agent { stopped, was: s("was"), stopped_by: s("stopped_by"), ..Agent::default() });
            }
        }
    }
    out
}

/// Append one line to events.jsonl: `stopped|paused|resumed`, by
/// `you|cancel_bar|group_closed`.
pub fn event(paths: &Paths, agent: &str, event: &str, by: &str) -> std::io::Result<()> {
    event_driving(paths, agent, event, by, None)
}

/// [`event`] with what the agent drove just before (`"Chrome"`,
/// `"TextEdit"`): `stopped` and `paused` carry it for main's feed line
/// ("↖ api-v2 stopped driving Chrome · you stopped it"); state.json has
/// `driving: null` by then (C6, m_3897).
pub fn event_driving(paths: &Paths, agent: &str, event: &str, by: &str, driving: Option<&str>) -> std::io::Result<()> {
    paths.ensure()?;
    let (name, hub) = who(agent);
    let mut v = json!({"t": crate::now_ms(), "agent": agent, "name": name, "hub": hub, "event": event, "by": by});
    if matches!(event, "stopped" | "paused") {
        v["driving"] = json!(driving);
    }
    let line = v.to_string() + "\n";
    let f = paths.events_file();
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&f)?;
    crate::paths::private(&f, 0o600)?;
    file.write_all(line.as_bytes())
}

/// The events, oldest first (tests, `status`).
pub fn events(paths: &Paths) -> Vec<Value> {
    std::fs::read_to_string(paths.events_file())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one writer's law: 16 threads at once, no broker, each stops
    /// its own agent through `control` (on the file) and every odd one
    /// resumes it after: every change is in the file (evens stopped, odds
    /// gone), and no tmp file is left.
    #[test]
    fn racing_writers_lose_no_change() {
        let d = std::env::temp_dir().join(format!("cu-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = Paths::new(d.join("run"), d.join("bise"), d.join("home"));
        let keys: Vec<String> = (0..16).map(|i| format!("00000000000000aa.a{i}")).collect();
        std::thread::scope(|s| {
            for (i, k) in keys.iter().enumerate() {
                let p = &p;
                s.spawn(move || {
                    crate::cli::control(p, "stop", &json!({"agent": k})).unwrap();
                    if i % 2 == 1 {
                        crate::cli::control(p, "resume", &json!({"agent": k})).unwrap();
                    }
                });
            }
        });
        let v = read(&p);
        for (i, k) in keys.iter().enumerate() {
            match i % 2 {
                0 => assert_eq!(v["agents"][k]["stopped"], true, "{k} lost its stop: {v}"),
                _ => assert!(v["agents"].get(k).is_none(), "{k} lost its resume: {v}"),
            }
        }
        let left: Vec<String> = std::fs::read_dir(p.state_file().parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.contains("tmp"))
            .collect();
        assert!(left.is_empty(), "tmp files left: {left:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The whole file replaced, through the one writer.
    fn put(p: &Paths, v: Value) {
        update(p, |f| -> std::io::Result<()> {
            *f = v;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn renders_restores_and_appends() {
        let d = std::env::temp_dir().join(format!("cu-state-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = Paths::new(d.join("run"), d.join("bise"), d.join("home"));
        let mut agents = BTreeMap::new();
        agents.insert("idle".to_string(), Agent::default());
        agents.insert(
            "api-v2".to_string(),
            Agent { driving: Some("Chrome".into()), place: Some("amazon.fr".into()), since_ms: Some(5), ..Agent::default() },
        );
        agents.insert("held".to_string(), Agent { stopped: true, ..Agent::default() });
        let v = render(&agents, json!([]), json!({"helper": "absent"}));
        put(&p, v);
        let back = read(&p);
        assert_eq!(
            back["agents"]["api-v2"],
            json!({"driving": "Chrome", "where": "amazon.fr", "since_ms": 5, "paused": false, "stopped": false, "name": "api-v2", "hub": null})
        );
        assert!(back["agents"].get("idle").is_none());
        let r = restore(&p);
        assert_eq!(r.keys().collect::<Vec<_>>(), ["held"]);
        event(&p, "api-v2", "stopped", "you").unwrap();
        event(&p, "00000000000000aa.perf", "resumed", "you").unwrap();
        let ev = events(&p);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0]["event"], "stopped");
        assert_eq!((&ev[1]["agent"], &ev[1]["name"], &ev[1]["hub"]), (&json!("00000000000000aa.perf"), &json!("perf"), &json!("00000000000000aa")));
        // a v1 file (keyed by bare names) restores nothing
        put(&p, json!({"agents": {"perf": {"stopped": true}}}));
        assert!(restore(&p).is_empty());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(p.state_file()).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// What `render` writes, `agents` reads back: the key, its hub and
    /// name, every field; idle agents absent, untagged ones with no hub,
    /// a v1 file's nameless entries skipped.
    #[test]
    fn render_then_agents_round_trips() {
        let mut m = BTreeMap::new();
        m.insert("idle".to_string(), Agent::default());
        m.insert(
            "00000000000000aa.perf".to_string(),
            Agent { driving: Some("Chrome".into()), place: Some("amazon.fr".into()), since_ms: Some(5), ..Agent::default() },
        );
        m.insert("00000000000000bb.perf".to_string(), Agent { paused: vec!["*".into()], ..Agent::default() });
        m.insert("loose".to_string(), Agent { stopped: true, ..Agent::default() });
        let got = agents(&render(&m, json!([]), json!({})));
        let live = |key: &str, hub: Option<&str>, name: &str| Live { key: key.into(), hub: hub.map(String::from), name: name.into(), ..Live::default() };
        assert_eq!(
            got,
            [
                Live { driving: Some("Chrome".into()), place: Some("amazon.fr".into()), since_ms: Some(5), ..live("00000000000000aa.perf", Some("00000000000000aa"), "perf") },
                Live { paused: true, ..live("00000000000000bb.perf", Some("00000000000000bb"), "perf") },
                Live { stopped: true, ..live("loose", None, "loose") },
            ]
        );
        assert!(agents(&json!({"agents": {"perf": {"stopped": true}}})).is_empty());
        assert!(agents(&json!({})).is_empty());
    }
}
