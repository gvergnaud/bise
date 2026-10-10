//! `sb wake` (event-wake): an agent waits for an event instead of a
//! timer. Its backgrounded bash command ends (the bash tool registers it
//! at the handoff, default on), a pid exits, a launchd job ends, a file
//! appears: bise wakes it ONCE with one line (what ended, its rc, the last
//! lines of its output). A watch past its max wakes it with "still
//! running" (a bash command's: ends quietly after a day).
//!
//! The watches themselves are sb-core's (bend/hub/wakes.bend and
//! core.bend's watches section): their state, their journal lines
//! (`wake_set`, `wake_end`), the decision to wake (a live watch of an
//! active agent, one message from bise, queued while it is busy), the max
//! at the tick, a dropped agent's watches ending with it; laws
//! wake_hit_once, wake_gone_never_hits, wake_drop_ends, wake_max_once,
//! wake_replay_keeps in LAWS.bend.
//!
//! Here, pure: what a watch looks at ([`Spec`], opaque to sb-core), the
//! look over an injected [`Probe`] ([`look`]), the words (the hit line,
//! the "still running" line given at set, `sb wake`'s list), the tail
//! clip, and [`Wakes`], the read-only mirror of sb-core's view (`wakes`,
//! `wakes_ended`). The probing itself (files, pids, launchctl) and its
//! cadence: daemon/wakes.rs.

use crate::util::{clip, one_line};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// `sb wake`'s max when none is given, and the most it takes.
pub const MAX_DEFAULT_MS: u64 = 60 * 60 * 1000;
pub const MAX_MOST_MS: u64 = 24 * 60 * 60 * 1000;
/// A bash command's watch: a day, then it ends without a wake (a dev
/// server runs for days).
pub const BG_MAX_MS: u64 = 24 * 60 * 60 * 1000;
/// The tail a wake carries: at most this many lines and bytes.
pub const TAIL_LINES: usize = 20;
pub const TAIL_BYTES: usize = 2048;

/// What a watch looks at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum What {
    /// A backgrounded bash command: `slot` is `<bg dir>/<id>` (its .rc,
    /// .out, .pid, .slot), `cmd` its first line.
    Bg { slot: String, cmd: String },
    /// A process, and its start time when set (another process with the
    /// same pid later is not it).
    Pid { pid: u32, start: String },
    /// A file that appears (a job's rc file).
    File { path: String },
    /// A launchd job (`launchctl submit -l <label>`).
    Job { label: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub what: What,
    /// A file whose last lines the wake carries (a bash command: its .out).
    pub tail: Option<String>,
    /// The agent's words, said back in the wake.
    pub note: String,
}

impl Spec {
    pub fn json(&self) -> Value {
        let mut v = match &self.what {
            What::Bg { slot, cmd } => json!({"kind": "bg", "slot": slot, "cmd": cmd}),
            What::Pid { pid, start } => json!({"kind": "pid", "pid": pid, "start": start}),
            What::File { path } => json!({"kind": "file", "path": path}),
            What::Job { label } => json!({"kind": "job", "label": label}),
        };
        if let Some(t) = &self.tail {
            v["tail"] = json!(t);
        }
        if !self.note.is_empty() {
            v["note"] = json!(self.note);
        }
        v
    }

    pub fn from_json(v: &Value) -> Option<Spec> {
        let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
        let what = match v["kind"].as_str()? {
            "bg" => What::Bg { slot: s("slot"), cmd: s("cmd") },
            "pid" => What::Pid { pid: u32::try_from(v["pid"].as_u64()?).ok()?, start: s("start") },
            "file" => What::File { path: s("path") },
            "job" => What::Job { label: s("label") },
            _ => return None,
        };
        let tail = v["tail"].as_str().filter(|t| !t.is_empty()).map(String::from);
        Some(Spec { what, tail, note: s("note") })
    }

    /// A bash command's watch: its slot's .out is the tail.
    pub fn bg(slot: &str, cmd: &str) -> Spec {
        Spec {
            what: What::Bg { slot: slot.to_string(), cmd: clip(&one_line(cmd), 80) },
            tail: Some(format!("{}.out", slot)),
            note: String::new(),
        }
    }

    /// What it is, in a few words: `background 3 (`cargo test`)`,
    /// `pid 4242`, `/tmp/x/rc`, `launchd job dev.bise.x`.
    pub fn label(&self) -> String {
        match &self.what {
            What::Bg { slot, cmd } if cmd.is_empty() => format!("background {}", slot_id(slot)),
            What::Bg { slot, cmd } => format!("background {} (`{}`)", slot_id(slot), cmd),
            What::Pid { pid, .. } => format!("pid {}", pid),
            What::File { path } => path.clone(),
            What::Job { label } => format!("launchd job {}", label),
        }
    }
}

/// The id of a bash slot (`<dir>/3` → `3`).
fn slot_id(slot: &str) -> &str {
    slot.rsplit('/').next().unwrap_or(slot)
}

/// One live watch, as sb-core's view carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watch {
    pub id: u64,
    pub agent: String,
    pub spec: Spec,
    pub set_at: u64,
    pub max_at: u64,
    pub quiet: bool,
}

impl Watch {
    fn from_view(v: &Value) -> Option<Watch> {
        Some(Watch {
            id: v["id"].as_u64()?,
            agent: v["agent"].as_str()?.to_string(),
            spec: Spec::from_json(&v["spec"])?,
            set_at: v["set_at"].as_u64().unwrap_or(0),
            max_at: v["max_at"].as_u64().unwrap_or(0),
            quiet: v["quiet"].as_bool().unwrap_or(false),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ended {
    pub watch: Watch,
    pub at: u64,
    pub why: String,
}

/// The read-only mirror of sb-core's watches (its view's `wakes`,
/// `wakes_ended`).
#[derive(Clone, Debug, Default)]
pub struct Wakes {
    pub live: BTreeMap<u64, Watch>,
    pub ended: Vec<Ended>,
}

impl Wakes {
    pub fn load_live(&mut self, v: &Value) {
        self.live = v.as_array().into_iter().flatten().filter_map(Watch::from_view).map(|w| (w.id, w)).collect();
    }

    pub fn load_ended(&mut self, v: &Value) {
        self.ended = v
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| {
                Some(Ended {
                    watch: Watch::from_view(e)?,
                    at: e["ended_at"].as_u64().unwrap_or(0),
                    why: e["why"].as_str().unwrap_or_default().to_string(),
                })
            })
            .collect();
    }

    /// The live watch of `agent`'s background command `slot` (its id in
    /// the bash contract: `sb wake --stop bg/3`).
    pub fn of_bg(&self, agent: &str, slot: &str) -> Option<u64> {
        self.live.values().find(|w| w.agent == agent && matches!(&w.spec.what, What::Bg { slot: s, .. } if slot_id(s) == slot)).map(|w| w.id)
    }

    /// `sb wake`: the agent's live watches, then its last ended ones.
    pub fn list(&self, agent: &str, now: u64) -> String {
        let mut out: Vec<String> = self
            .live
            .values()
            .filter(|w| w.agent == agent)
            .map(|w| {
                let left = if w.quiet { String::new() } else { format!(" · still running at {}", dur(w.max_at.saturating_sub(now))) };
                format!("#{} {} · set {} ago{}{}", w.id, w.spec.label(), dur(now.saturating_sub(w.set_at)), left, note_part(&w.spec.note))
            })
            .collect();
        if out.is_empty() {
            out.push("no watch: sb wake --on-exit <pid> | --on-file <path> | --on-job <launchd label>".into());
        }
        let ended: Vec<String> = self
            .ended
            .iter()
            .rev()
            .filter(|e| e.watch.agent == agent)
            .take(5)
            .map(|e| format!("#{} {} · {} {} ago", e.watch.id, e.watch.spec.label(), why_words(&e.why), dur(now.saturating_sub(e.at))))
            .collect();
        if !ended.is_empty() {
            out.push("ended:".into());
            out.extend(ended.into_iter().map(|l| format!("  {}", l)));
        }
        out.join("\n")
    }
}

fn why_words(why: &str) -> String {
    match why {
        "hit" => "woke you".into(),
        "max" => "ran out of time".into(),
        "gone" => "ended with its agent".into(),
        w => w.to_string(),
    }
}

fn note_part(note: &str) -> String {
    if note.trim().is_empty() {
        String::new()
    } else {
        format!(" · {}", clip(&one_line(note), 120))
    }
}

/// `45s`, `2m04s`, `1h05m`, `2d3h`.
pub fn dur(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..=59 => format!("{}s", s),
        60..=3599 => format!("{}m{:02}s", s / 60, s % 60),
        3600..=86_399 => format!("{}h{:02}m", s / 3600, s % 3600 / 60),
        _ => format!("{}d{}h", s / 86_400, s % 86_400 / 3600),
    }
}

/// A launchd job, as `launchctl list <label>` says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobState {
    pub pid: Option<u32>,
    pub last_exit: Option<i64>,
}

/// `launchctl list <label>`'s output: its `"PID" = n;` while it runs, its
/// `"LastExitStatus" = n;`.
pub fn parse_job(out: &str) -> JobState {
    let num = |key: &str| {
        out.lines().find_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix(&format!("\"{}\" = ", key))?;
            rest.trim_end_matches(';').trim().parse::<i64>().ok()
        })
    };
    JobState { pid: num("PID").and_then(|p| u32::try_from(p).ok()), last_exit: num("LastExitStatus") }
}

/// The world a look reads: the shell gives the real one
/// (daemon/wakes.rs), the tests a fake.
pub trait Probe {
    fn exists(&self, path: &str) -> bool;
    /// The first bytes of a file (at most a few KB).
    fn head(&self, path: &str) -> Option<String>;
    /// The last bytes of a file (at most a few KB).
    fn tail(&self, path: &str) -> Option<String>;
    fn alive(&self, pid: u32) -> bool;
    /// The process's start time (`ps -o lstart=`); None: unknown.
    fn start(&self, pid: u32) -> Option<String>;
    /// The job's state; None: no such job (removed, or never loaded).
    fn job(&self, label: &str) -> Option<JobState>;
}

/// What a look saw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seen {
    Running,
    /// It ended: its rc when known (a bash command, a launchd job, an rc
    /// file's first line), and what the wake says of it.
    Ended { rc: Option<String> },
}

/// How much a look may cost now (daemon/wakes.rs's cadence): `start`:
/// read a pid's start time (a `ps`), `job`: ask launchctl (a process).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Costly {
    pub start: bool,
    pub job: bool,
}

/// Has the watch's event happened? `job_pid`: the job's pid seen by an
/// earlier look (kill(0) on it is enough while it lives); a job look
/// returns the pid it saw, for the next one.
pub fn look(spec: &Spec, p: &dyn Probe, costly: Costly, job_pid: Option<u32>) -> (Seen, Option<u32>) {
    match &spec.what {
        What::Bg { slot, .. } => {
            let rc = format!("{}.rc", slot);
            if p.exists(&rc) {
                return (Seen::Ended { rc: p.head(&rc).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) }, None);
            }
            // its files gone (removed by hand): ended, rc unknown
            let gone = ["pid", "out"].iter().all(|e| !p.exists(&format!("{}.{}", slot, e))) && !p.exists(&format!("{}.slot", slot));
            (if gone { Seen::Ended { rc: None } } else { Seen::Running }, None)
        }
        What::Pid { pid, start } => {
            if !p.alive(*pid) {
                return (Seen::Ended { rc: None }, None);
            }
            // the same pid, another process: ours ended
            let reused = costly.start && !start.is_empty() && p.start(*pid).is_some_and(|s| s != *start);
            (if reused { Seen::Ended { rc: None } } else { Seen::Running }, None)
        }
        What::File { path } => {
            if p.exists(path) {
                (Seen::Ended { rc: p.head(path).and_then(|s| first_rc(&s)) }, None)
            } else {
                (Seen::Running, None)
            }
        }
        What::Job { label } => {
            if let Some(pid) = job_pid.filter(|pid| p.alive(*pid)) {
                return (Seen::Running, Some(pid));
            }
            if !costly.job {
                return (Seen::Running, job_pid);
            }
            match p.job(label) {
                None => (Seen::Ended { rc: None }, None),
                Some(JobState { pid: Some(pid), .. }) => (Seen::Running, Some(pid)),
                Some(JobState { pid: None, last_exit }) => (Seen::Ended { rc: last_exit.map(|r| r.to_string()) }, None),
            }
        }
    }
}

/// An rc file's first line, when it is a number.
fn first_rc(s: &str) -> Option<String> {
    let l = s.lines().next()?.trim();
    l.parse::<i64>().ok().map(|_| l.to_string())
}

/// The end of a file, for a wake: its last [`TAIL_LINES`] lines, at most
/// [`TAIL_BYTES`] bytes, cut on a char boundary.
pub fn tail_clip(text: &str) -> String {
    let text = text.trim_end();
    let lines: Vec<&str> = text.lines().collect();
    let mut out = lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n");
    if out.len() > TAIL_BYTES {
        let mut cut = out.len() - TAIL_BYTES;
        while !out.is_char_boundary(cut) {
            cut += 1;
        }
        out = format!("…{}", &out[cut..]);
    }
    out
}

// The words of the wakes (designer m_16533): one shape for all,
// `<what> <ended|appeared> · rc N · after D · <note>`, each part only
// when known, lowercase, no period; then the tail.

/// What a wake names: `background 3`, `pid 4242`, a path, `launchd job x`.
fn what(spec: &Spec) -> String {
    match &spec.what {
        What::Bg { slot, .. } => format!("background {}", slot_id(slot)),
        What::Pid { pid, .. } => format!("pid {}", pid),
        What::File { path } => path.clone(),
        What::Job { label } => format!("launchd job {}", label),
    }
}

/// The head's last parts: the agent's note, then a bash command's line.
fn words_after(spec: &Spec) -> Vec<String> {
    let mut v = Vec::new();
    if !spec.note.trim().is_empty() {
        v.push(clip(&one_line(&spec.note), 120));
    }
    if let What::Bg { cmd, .. } = &spec.what {
        if !cmd.is_empty() {
            v.push(cmd.clone());
        }
    }
    v
}

/// The tail part: `its last N lines (<file>):` and the lines, or `it
/// printed nothing`; no tail file, no part.
fn tail_part(file: &str, tail: Option<&str>) -> String {
    let t = tail.map(tail_clip).unwrap_or_default();
    match t.lines().count() {
        0 => "it printed nothing".to_string(),
        1 => format!("its last line ({}):\n{}", file, t),
        n => format!("its last {} lines ({}):\n{}", n, file, t),
    }
}

/// The wake of a watch whose event happened (`waited`: since its set).
pub fn hit_text(spec: &Spec, rc: Option<&str>, tail: Option<&str>, waited: u64) -> String {
    let rc = rc.map(|r| format!("rc {}", r));
    let after = format!("after {}", dur(waited));
    let mut parts = match &spec.what {
        What::File { .. } => vec![format!("{} appeared", what(spec)), after].into_iter().chain(rc).collect::<Vec<_>>(),
        _ => std::iter::once(format!("{} ended", what(spec))).chain(rc).chain([after]).collect(),
    };
    parts.extend(words_after(spec));
    let mut out = parts.join(" · ");
    if let Some(file) = spec.tail.as_deref() {
        out.push('\n');
        out.push_str(&tail_part(file, tail));
    }
    out
}

/// A duration as `--max` takes it: `1h`, `1h30m`, `45m` (at least 1m).
fn dur_flag(ms: u64) -> String {
    let m = (ms / 60_000).max(1);
    match (m / 60, m % 60) {
        (0, m) => format!("{}m", m),
        (h, 0) => format!("{}h", h),
        (h, m) => format!("{}h{}m", h, m),
    }
}

/// A word for a shell line: as is when plain, else in single quotes.
fn sh_word(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-:@+=,".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// The `sb wake` line that sets the same watch again.
pub fn again(spec: &Spec, max_ms: u64) -> String {
    let mut v = vec!["sb wake".to_string()];
    match &spec.what {
        What::Pid { pid, .. } => v.push(format!("--on-exit {}", pid)),
        What::File { path } => v.push(format!("--on-file {}", sh_word(path))),
        What::Job { label } => v.push(format!("--on-job {}", sh_word(label))),
        What::Bg { slot, .. } => v.push(format!("--on-file {}", sh_word(&format!("{}.rc", slot)))),
    }
    if let Some(t) = &spec.tail {
        v.push(format!("--tail {}", sh_word(t)));
    }
    if !spec.note.is_empty() {
        v.push(format!("--note {}", sh_word(&spec.note)));
    }
    if max_ms != MAX_DEFAULT_MS {
        v.push(format!("--max {}", dur_flag(max_ms)));
    }
    v.join(" ")
}

/// The wake of a watch past its max (given to sb-core at the set).
pub fn max_text(spec: &Spec, max_ms: u64) -> String {
    let mut parts = vec![format!("{} still running", what(spec)), format!("after {}", dur(max_ms))];
    parts.extend(words_after(spec));
    format!(
        "{}\nthis watch stopped at its --max ({}). to keep waiting: {}",
        parts.join(" · "),
        dur_flag(max_ms),
        again(spec, max_ms)
    )
}

/// The set's answer: what the agent will hear and how to stop it.
pub fn set_text(id: u64, spec: &Spec, max_ms: u64) -> String {
    format!("watch #{} set: you'll be woken when {} ends (at most {}; sb wake --stop {})", id, spec.label(), dur(max_ms), id)
}

#[cfg(test)]
#[path = "wake_tests.rs"]
mod tests;
