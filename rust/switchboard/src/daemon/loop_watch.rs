//! The loop's watchdog (hub-stall): what the hub's loop runs (its sb-core
//! step: [`Shell::step`](super::Shell::step), `step_fx`), and a
//! hub.log line when one message holds it. On 2026-10-14 his loop sat
//! 27 min in one call and nothing said where; now hub.log says
//! `hub loop busy 30 s on <msg>` at 5 s, 30 s, then each minute, and
//! `hub loop free after N s` when it moves again.
//!
//! The loop marks each message it takes ([`LoopWatch::on`], `Msg::label`:
//! its kind and its agent or client) and its idle wait; a thread reads
//! the mark once a second. The cadence is pure ([`say_at`]).
//!
//! A message with parts (a Tick) names its part as it goes
//! ([`LoopWatch::phase`]): `hub loop busy 30 s on input Tick: sb-core`
//! (hub-tick: a Tick sat 38 min in sb-core and the line said only Tick);
//! its end line names the part that held it longest, with its time
//! (`on input Tick: idle check 1.8 s`), never merely the last part.
//!
//! A message that held it 1 s or more, never said busy, says
//! `hub loop held 2.1 s on <msg> (<split>)` at its end.
//!
//! Each busy line ends with its split (crate::spent, hub-loop): `(git
//! worktree_create 11.2 s, sb-core 0.4 s, shell 0.4 s)`: sb-core's
//! answers, its git queries and the effects, timed where they run.
//!
//! What may still hold the loop: sb-core's line (`core::CoreLink::call`,
//! strictly one request then one answer: sb-core reads the whole line
//! before it writes, so the two pipes never fill at once), the git
//! queries sb-core asks for, and a few short processes (`ps`, `launchctl`
//! in daemon/drops.rs, off the loop). No peer socket: every write goes through
//! daemon/outbox.rs.

use super::{log_line, Msg};
use crate::paths::Paths;
use crate::spent::Spent;
use crate::util::now_ms;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The first word on a busy loop, then the second; then one per minute.
const FIRST_MS: u64 = 5_000;
const SECOND_MS: u64 = 30_000;
const EVERY_MS: u64 = 60_000;
/// A message that held the loop this long says so at its end (hub-loop:
/// an agent's `sb` waits behind it).
const HELD_MS: u64 = 1_000;

/// When a loop busy for `busy_ms` is said again, `said` lines said
/// already: the next line's due time (ms into the busy spell).
pub(super) fn say_at(said: u32) -> u64 {
    match said {
        0 => FIRST_MS,
        1 => SECOND_MS,
        n => SECOND_MS + EVERY_MS * (n as u64 - 1),
    }
}

#[derive(Default)]
struct Mark {
    /// what the loop runs ("" idle)
    what: String,
    since_ms: u64,
    /// lines said for this busy spell
    said: u32,
    /// a message with parts (a Tick): each part's name and start, in
    /// order; the end line names the longest one, not the last
    phases: Vec<(String, u64)>,
}

impl Mark {
    /// What the message is at now: its kind, and its part if it has parts.
    fn now_on(&self) -> String {
        match self.phases.last() {
            Some((p, _)) => format!("{}: {}", self.what, p),
            None => self.what.clone(),
        }
    }

    /// The part that held it longest, with its time (the end line's
    /// words): `input Tick: idle check 1.8 s`; none for a message
    /// without parts.
    fn held_on(&self, end_ms: u64) -> String {
        let ends = self.phases.iter().skip(1).map(|(_, at)| *at).chain([end_ms]);
        let longest = self.phases.iter().zip(ends).map(|((p, at), end)| (end.saturating_sub(*at), p)).max_by_key(|(ms, _)| *ms);
        match longest {
            Some((ms, p)) => format!("{}: {} {:.1} s", self.what, p, ms as f64 / 1000.0),
            None => self.what.clone(),
        }
    }
}

/// How often the loop's timings by kind go to hub.log (SB_LOOP_STATS_MS
/// in tests and measures).
const STATS_MS: u64 = 600_000;
/// The times kept per kind in a window (a flood counts the rest, unsorted).
const KEEP_PER_KIND: usize = 50_000;

/// The loop's time over a window, by message kind (c4-loop-never-waits):
/// what a hold under 1 s, never in a held line, costs in all, and the
/// state push's median (its goal: under 100 ms).
/// One kind's window: how many, their total ms, the times kept.
type Kind = (u64, u64, Vec<u32>);

#[derive(Default)]
struct Stats {
    since_ms: u64,
    /// each kind: how many, their total ms, the times kept
    kinds: std::collections::BTreeMap<String, Kind>,
}

impl Stats {
    fn add(&mut self, kind: &str, ms: u64) {
        let k = self.kinds.entry(kind.to_string()).or_default();
        k.0 += 1;
        k.1 += ms;
        if k.2.len() < KEEP_PER_KIND {
            k.2.push(ms.min(u32::MAX as u64) as u32);
        }
    }

    /// The window's line, from `since_ms` to `now`, and a new window:
    /// `hub loop stats 600 s: busy 4.12 % (24.7 s); state push 312 p50 38
    /// p95 120 max 1200 ms 11.2 s; ...` (the kinds by total, the first 6).
    /// None for a window without a message.
    fn line(&mut self, now: u64) -> Option<String> {
        let span = now.saturating_sub(self.since_ms).max(1);
        let kinds = std::mem::take(&mut self.kinds);
        self.since_ms = now;
        if kinds.is_empty() {
            return None;
        }
        let busy: u64 = kinds.values().map(|k| k.1).sum();
        let mut by: Vec<(String, Kind)> = kinds.into_iter().collect();
        by.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(a.0.cmp(&b.0)));
        let parts: Vec<String> = by
            .into_iter()
            .take(6)
            .map(|(kind, (n, total, mut ms))| {
                ms.sort_unstable();
                let at = |q: usize| ms.get((ms.len().saturating_sub(1)) * q / 100).copied().unwrap_or(0);
                format!("{kind} {n} p50 {} p95 {} max {} ms {:.1} s", at(50), at(95), ms.last().copied().unwrap_or(0), total as f64 / 1000.0)
            })
            .collect();
        Some(format!(
            "hub loop stats {} s: busy {:.2} % ({:.1} s); {}",
            span / 1000,
            100.0 * busy as f64 / span as f64,
            busy as f64 / 1000.0,
            parts.join("; ")
        ))
    }
}

#[derive(Clone, Default)]
pub(super) struct LoopWatch {
    mark: Arc<Mutex<Mark>>,
    /// where the message spends its time (crate::spent, the hub's)
    spent: Spent,
    /// the loop's time by kind, said every STATS_MS
    stats: Arc<Mutex<Stats>>,
}

impl LoopWatch {
    /// The watch and its thread (hub.log in `paths`); `spent`: the
    /// hub's split of each message (hub-loop).
    pub(super) fn start(paths: &Paths, spent: Spent) -> LoopWatch {
        let w = LoopWatch { spent, ..Default::default() };
        let every =
            bise_home::env::test_setting("SB_LOOP_STATS_MS").and_then(|v| v.parse::<u64>().ok()).unwrap_or(STATS_MS).max(1_000);
        w.stats.lock().unwrap_or_else(|e| e.into_inner()).since_ms = now_ms();
        let (mark, spent, paths, stats) = (w.mark.clone(), w.spent.clone(), paths.clone(), w.stats.clone());
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            let said = {
                let mut s = stats.lock().unwrap_or_else(|e| e.into_inner());
                let now = now_ms();
                (now.saturating_sub(s.since_ms) >= every).then(|| s.line(now)).flatten()
            };
            if let Some(l) = said {
                log_line(&paths, &l);
            }
            let line = {
                let mut m = mark.lock().unwrap_or_else(|e| e.into_inner());
                let busy = now_ms().saturating_sub(m.since_ms);
                if m.what.is_empty() || busy < say_at(m.said) {
                    None
                } else {
                    m.said += 1;
                    Some(format!("hub loop busy {} s on {} ({})", busy / 1000, m.now_on(), split(busy, &spent)))
                }
            };
            if let Some(l) = line {
                log_line(&paths, &l);
            }
        });
        w
    }

    /// The loop takes `what`.
    pub(super) fn on(&self, what: String) {
        let mut m = self.mark.lock().unwrap_or_else(|e| e.into_inner());
        self.spent.reset();
        *m = Mark { what, since_ms: now_ms(), said: 0, phases: Vec::new() };
    }

    /// The message the loop runs is at `phase` now (hub-tick: a Tick's
    /// parts): the busy lines name it, the spell keeps its start, and its
    /// end line names the part that held it longest (send-fast: the held
    /// line named the Tick's last part, 'computer use', for 1,096 s of
    /// his 24 h that were idle_check's).
    pub(super) fn phase(&self, phase: &str) {
        let mut m = self.mark.lock().unwrap_or_else(|e| e.into_inner());
        m.phases.push((phase.to_string(), now_ms()));
    }

    /// The loop is done with it; a spell long enough to be said says its
    /// end too. The line to log, if any.
    pub(super) fn idle(&self) -> Option<String> {
        let mut m = self.mark.lock().unwrap_or_else(|e| e.into_inner());
        let end = now_ms();
        let busy = end.saturating_sub(m.since_ms);
        let out = if m.said > 0 {
            Some(format!("hub loop free after {} s on {} ({})", busy / 1000, m.held_on(end), split(busy, &self.spent)))
        } else {
            // a hold too short for a busy line, said once at its end
            (busy >= HELD_MS).then(|| format!("hub loop held {:.1} s on {} ({})", busy as f64 / 1000.0, m.held_on(end), split(busy, &self.spent)))
        };
        if !m.what.is_empty() {
            self.stats.lock().unwrap_or_else(|e| e.into_inner()).add(kind_of(&m.what), busy);
        }
        *m = Mark::default();
        out
    }
}

/// A message's kind for the stats: its label (`Msg::label`) without
/// whose it is: `repl line t1` -> `repl line`, `sb list from main` ->
/// `sb list`, `placed t1` -> `placed`; `input Tick`, `state push` whole.
fn kind_of(label: &str) -> &str {
    if label.starts_with("a ") || label.starts_with("another ") {
        return label;
    }
    let words = if label.starts_with("placed ") { 1 } else { 2 };
    match label.match_indices(' ').nth(words - 1) {
        Some((i, _)) => &label[..i],
        None => label,
    }
}

// moved from daemon.rs (hub-loop): the loop's lanes for each message
impl Msg {
    /// An input for the loop, boxed.
    pub(super) fn input(i: crate::core::Input) -> Msg {
        Msg::In(Box::new(i))
    }

    /// The inbox's urgent lane (daemon/inbox.rs): a client's own message
    /// (its hello or `initialize`, a line, its end, a thread's typed answer
    /// to its request), one lane so its order holds (architect m_14659);
    /// and a stop (`bise switchboard --stop`, a switch's, idle-exit's),
    /// which runs at once: in the rest lane, behind the REPLs' lines, a
    /// stop asked at 22:18:16 ran at 22:19:41 (2026-10-11), after the
    /// client had said "stopped" and a new `bise` had attached.
    pub(super) fn is_urgent(m: &Msg) -> bool {
        // an agent's read too (daemon/accept.rs READS, hub-loop)
        matches!(m, Msg::OlderHello { .. } | Msg::RpcNew { .. } | Msg::ClientLine { .. } | Msg::ClientGone { .. } | Msg::Typed { .. } | Msg::Shutdown { .. })
            || matches!(m, Msg::AgentNew { v, .. } if super::accept::is_read(v))
    }
}

impl Msg {
    /// The loop watch's words for this message: its kind, and whose.
    pub(super) fn label(&self) -> String {
        match self {
            Msg::In(i) => format!("input {}", input_kind(i)),
            Msg::ReplConnected { dir, .. } => format!("repl connected {dir}"),
            Msg::ReplSpawned { dir, .. } => format!("repl spawned {dir}"),
            Msg::ReplLine { dir, .. } => format!("repl line {dir}"),
            Msg::ReplStartStep { dir, .. } => format!("repl start step {dir}"),
            Msg::ReplGone { dir, .. } => format!("repl gone {dir}"),
            Msg::ClientLine { id, .. } => format!("client line {id:?}"),
            Msg::ClientGone { id } => format!("client gone {id:?}"),
            Msg::RpcNew { id, .. } => format!("client initialize {id:?}"),
            Msg::AgentNew { v, .. } => format!(
                "sb {} from {}",
                v.get("cmd").or_else(|| v.get("op")).and_then(|x| x.as_str()).unwrap_or("?"),
                v.get("from").and_then(|x| x.as_str()).unwrap_or("?")
            ),
            Msg::OlderHello { .. } => "older hello".into(),
            Msg::XHub { .. } => "another hub's op".into(),
            Msg::Version { .. } => "sb version".into(),
            Msg::Page(_) => "page".into(),
            Msg::Land { .. } => "land".into(),
            Msg::GateChecked { dir, .. } => format!("gate checked {dir}"),
            Msg::View(_) => "a view's request".into(),
            Msg::RoleLine { dir, .. } => format!("role line {dir}"),
            Msg::GuideSeen(_) => "guide seen".into(),
            Msg::GuideDone { hash, .. } => format!("guide {hash}"),
            Msg::Typed { .. } => "typed event".into(),
            Msg::WakeSeen(s) => format!("wake seen {}", s.id),
            Msg::Placed { name, .. } => format!("placed {name}"),
            Msg::Unplaced { name, .. } => format!("unplaced {name}"),
            Msg::CommitStyle(_) => "commit style".to_string(),
            Msg::ArtBuilt(_) => "artifact rows built".to_string(),
            _ => "a hub message".into(),
        }
    }
}

/// A busy spell's split (crate::spent): its longest parts, the shell's.
fn split(busy_ms: u64, spent: &Spent) -> String {
    crate::spent::words(Duration::from_millis(busy_ms), &spent.now())
}

impl super::Shell {
    /// One sb-core step: its effects run, then what follows every step.
    pub(super) fn step(&mut self, input: crate::core::Input) {
        let fx = self.hub.handle(input, &mut self.env);
        self.step_fx(fx);
    }

    /// A step's effects (`fx`, sb-core's answer to one input) run, then
    /// what follows every step.
    pub(super) fn step_fx(&mut self, fx: Vec<crate::core::Effect>) {
        self.run_timed(fx);
        self.answers_flush();
        // a gate card closed without an answer is a no
        self.gate_sweep();
        // a stop or a /drop: its folders and processes (daemon/drops.rs)
        self.downs_after_step();
    }

    /// Run a step's effects, each timed in the split (crate::spent).
    pub(super) fn run_timed(&mut self, fx: Vec<crate::core::Effect>) {
        for e in fx {
            let _t = self.hub.spent.part(fx_part(&e));
            self.run(e);
        }
    }
}

/// An effect's name for the split: the ones that run a process or touch
/// files by name, the rest as one.
pub(super) fn fx_part(e: &crate::core::Effect) -> &'static str {
    use crate::core::Effect as E;
    match e {
        E::Journal(_) => "fx journal",
        E::Spawn { .. } => "fx spawn",
        E::Kill { .. } => "fx kill",
        E::Place(_) => "fx place",
        E::Say { .. } => "fx say",
        E::Line { .. } => "fx line",
        E::Land { .. } => "fx land",
        E::State => "fx state",
        _ => "fx other",
    }
}

/// An input's variant name (its Debug's first word).
fn input_kind(i: &crate::core::Input) -> String {
    let d = format!("{i:?}");
    d.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("?").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_busy_loop_is_said_at_5_s_30_s_then_each_minute() {
        let at: Vec<u64> = (0..5).map(say_at).collect();
        assert_eq!(at, [5_000, 30_000, 90_000, 150_000, 210_000]);
    }

    #[test]
    fn a_short_message_says_nothing_and_a_long_one_says_its_end() {
        let w = LoopWatch::default();
        w.on("repl line route-name".into());
        assert_eq!(w.idle(), None);
        w.on("repl line t1".into());
        w.mark.lock().unwrap().since_ms -= 1_500;
        assert!(w.idle().is_some_and(|l| l.starts_with("hub loop held 1.5 s on repl line t1 (")), "a hold of 1 s or more");
        w.on("client line 3".into());
        w.mark.lock().unwrap().said = 1;
        assert!(w.idle().is_some_and(|l| l.starts_with("hub loop free after ") && l.ends_with(" on client line 3 (shell 0.0 s)")));
    }

    /// hub-tick: a Tick's phases name the line, the spell keeps its start.
    #[test]
    fn a_phase_names_the_busy_line_and_keeps_the_spell() {
        let w = LoopWatch::default();
        w.on("input Tick".into());
        let since = w.mark.lock().unwrap().since_ms;
        w.phase("sb-core");
        w.phase("wakes look");
        let m = w.mark.lock().unwrap();
        assert_eq!((m.now_on().as_str(), m.since_ms), ("input Tick: wakes look", since));
    }

    /// send-fast: the end line names the part that held the loop
    /// longest, with its time, not the last part.
    #[test]
    fn the_end_line_names_the_longest_part_not_the_last() {
        let w = LoopWatch::default();
        w.on("input Tick".into());
        {
            let mut m = w.mark.lock().unwrap();
            let t = now_ms();
            m.since_ms = t - 2_000;
            m.phases = vec![("sb-core".into(), t - 2_000), ("idle check".into(), t - 1_900), ("computer use".into(), t - 100)];
        }
        let l = w.idle().unwrap();
        assert!(l.starts_with("hub loop held 2.0 s on input Tick: idle check 1.8 s ("), "{l}");
    }

    #[test]
    fn a_kind_is_the_label_without_whose() {
        let k: Vec<&str> = ["repl line t1", "sb list from main", "placed t1", "input Tick", "state push", "a view's request", "page"]
            .into_iter()
            .map(kind_of)
            .collect();
        assert_eq!(k, ["repl line", "sb list", "placed", "input Tick", "state push", "a view's request", "page"]);
    }

    /// c4-loop-never-waits: every message counts, under 1 s too; the
    /// kinds by total, each with its median; a new window after.
    #[test]
    fn the_stats_line_says_the_share_and_each_kinds_median() {
        let mut s = Stats { since_ms: 0, ..Default::default() };
        for ms in [10, 20, 30, 40, 900] {
            s.add("state push", ms);
        }
        s.add("repl line", 2_000);
        let l = s.line(100_000).unwrap();
        assert_eq!(
            l,
            "hub loop stats 100 s: busy 3.00 % (3.0 s); repl line 1 p50 2000 p95 2000 max 2000 ms 2.0 s; \
             state push 5 p50 30 p95 40 max 900 ms 1.0 s"
        );
        assert_eq!((s.line(200_000), s.since_ms), (None, 200_000), "a new window, empty");
    }
}
