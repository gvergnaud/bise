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

#[derive(Clone, Default)]
pub(super) struct LoopWatch {
    mark: Arc<Mutex<Mark>>,
    /// where the message spends its time (crate::spent, the hub's)
    spent: Spent,
}

impl LoopWatch {
    /// The watch and its thread (hub.log in `paths`); `spent`: the
    /// hub's split of each message (hub-loop).
    pub(super) fn start(paths: &Paths, spent: Spent) -> LoopWatch {
        let w = LoopWatch { spent, ..Default::default() };
        let (mark, spent, paths) = (w.mark.clone(), w.spent.clone(), paths.clone());
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
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
        *m = Mark::default();
        out
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
            Msg::Typed { .. } => "typed event".into(),
            Msg::WakeSeen(s) => format!("wake seen {}", s.id),
            Msg::Placed { name, .. } => format!("placed {name}"),
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
}
