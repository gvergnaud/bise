//! The week's transcript reader, and the hub's side of `/usage`
//! (cost-pause): it owns the project's [`Ledger`] (crate::usage_ledger,
//! pure) and answers `usage/read`. The same one read feeds the pulse's
//! week (daemon/pulse.rs, crate::pulse): the agents' turns and main's
//! entries for him. No other code reads the week's transcripts.
//!
//! Never on the boot path (issue 15): the fill starts at the hub's first
//! tick (daemon/pulse.rs `pulse_check`; a `usage/read` before it starts
//! it too), on a thread at low priority, from the transcripts of the
//! agent folders touched in the last [`DAYS`] days (their mtime), each
//! read backwards from its end until its lines are older than the week
//! (and the pulse's window). Archived agents count when they fall in that
//! window. Until the fill is back, `usage/read` says `counting` with what
//! the live lines gave so far (the pulse too). From the fill's start on,
//! the live REPLs' usage lines (the same hook as recycle's churn) are
//! added as they come, and the feed's lines go to the pulse; the fill
//! counts only the lines written before it started, so no call or turn
//! is counted twice. Nothing is written.

use super::*;
use crate::usage_ledger::{local_day, week_start, Cost, Ledger, Report, Sum, Tokens, DAYS};
use bise_proto::hub::HubEv;
use bise_proto::rows::{UsageRow, UsageSum};
use std::io::{Read, Seek, SeekFrom};

/// The ledger and how far its fill is.
#[derive(Debug, Default)]
pub(super) struct Usage {
    ledger: Ledger,
    /// None: never asked; Some(false): counting; Some(true): filled
    filled: Option<bool>,
}

/// The local day of `ms` on this machine.
fn day(ms: u64) -> i64 {
    local_day(ms, bise_home::clock::offset_at(ms))
}

impl Shell {
    /// A line of agent `name`'s live REPL (before the feed). The ledger
    /// keys its rows by the agent's folder, which never changes (name-free:
    /// an archived agent's name may go to a new one; its spend stays its).
    pub(super) fn usage_line(&mut self, name: &str, line: &str) {
        if self.usage.filled.is_some() {
            let dir = self.dir_of(name).unwrap_or_else(|| name.to_string());
            self.usage.ledger.line(&dir, day(now_ms()), line);
        }
    }

    /// The week's fill started (the live lines count from then on).
    pub(super) fn usage_started(&self) -> bool {
        self.usage.filled.is_some()
    }

    /// The fill, once: from the first tick (or a `usage/read` before it).
    pub(super) fn week_fill_start(&mut self) {
        if self.usage.filled.is_none() {
            self.usage.filled = Some(false);
            self.usage_fill(now_ms());
        }
    }

    /// The fill's ledger is back: it joins the live one.
    pub(super) fn usage_counted(&mut self, filled: Ledger) {
        self.usage.ledger.merge(filled);
        self.usage.filled = Some(true);
    }

    /// `usage/read`'s answer; the first read starts the fill.
    pub(super) fn usage_ev(&mut self) -> HubEv {
        let now = now_ms();
        self.week_fill_start();
        let today = day(now);
        self.usage.ledger.forget_before(today);
        // the catalog's prices, re-read when config changed
        let setup = self.config.setup();
        let live: BTreeSet<&str> = self.hub.st.agents.values().filter(|a| a.lifecycle != crate::model::Lifecycle::Archived).map(|a| a.dir.as_str()).collect();
        let names: BTreeMap<&str, &str> = self.hub.st.agents.values().map(|a| (a.dir.as_str(), a.name.as_str())).collect();
        let cost = |model: &str, t: &Tokens| {
            let r = setup.catalog.resolve(model);
            if setup.catalog.provider(&r.provider).is_some_and(|p| p.signs_in()) {
                return Cost::Plan;
            }
            r.price.cost(t.input, t.output, t.cache_read, t.cache_write).map_or(Cost::Unknown, Cost::Usd)
        };
        let report = named(self.usage.ledger.report(today, cost, |dir| !live.contains(dir)), &names);
        usage_ev(self.project(), self.usage.filled != Some(true), report)
    }

    /// Reads the week of every agent folder touched in the week, on a
    /// thread at low priority: its usage lines (the ledger) and, from the
    /// pulse's window on, its turns and main's entries (the pulse's
    /// week); written before `until` only.
    fn usage_fill(&mut self, until: u64) {
        let since = week_start(day(until));
        let pulse_since = crate::pulse::window_from(until, &bise_home::clock::offset_at);
        let main_dir = self.dir_of(crate::model::MAIN);
        let agents = self.opts.paths.agent_dir("x").parent().map(Path::to_path_buf);
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            low_priority();
            let mut l = Ledger::default();
            let mut week = super::pulse::Week::default();
            let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(DAYS as u64 * 86_400);
            for e in agents.and_then(|d| std::fs::read_dir(d).ok()).into_iter().flatten().flatten() {
                let t = e.path().join("transcript.log");
                let fresh = std::fs::metadata(&t).and_then(|m| m.modified()).is_ok_and(|m| m >= cutoff);
                if !fresh {
                    continue;
                }
                let dir = e.file_name().to_string_lossy().to_string();
                let is_main = main_dir.as_deref() == Some(dir.as_str());
                read_week(&t, &dir, is_main, (since, pulse_since, until), &mut l, &mut week);
            }
            let _ = tx.send(Msg::UsageCounted(Box::new((l, week))));
        });
    }
}

/// One agent's transcript `t` read from its end for the week: its usage
/// lines into `l` (local days from `since`), its turns and, for main, its
/// entries for him into `week` (from `pulse_since`); the lines written
/// before `until` only (the live side counts the others).
fn read_week(t: &Path, dir: &str, is_main: bool, (since, pulse_since, until): (i64, u64, u64), l: &mut Ledger, week: &mut super::pulse::Week) {
    let mut turns = crate::pulse::TurnsBack::new(pulse_since, until);
    // main's lines of the window, newest first (folded forward)
    let mut said: Vec<(u64, String)> = Vec::new();
    lines_backwards(t, |raw| {
        let usage = l.transcript_line(dir, raw, since, until, day);
        let Some((ms, rest)) = raw.split_once('\t').and_then(|(m, r)| Some((m.parse::<u64>().ok()?, r))) else { return true };
        let pulse = turns.line(ms, rest);
        if is_main && pulse && ms < until {
            said.push((ms, rest.to_string()));
        }
        usage || pulse
    });
    week.turns.insert(dir.to_string(), turns.turns());
    if is_main {
        super::pulse::with_ctx(|ctx| week.said.push_all(said.into_iter().rev(), ctx));
    }
}

/// This thread runs at low priority (the week's fill: many hubs read
/// theirs at once when the desktop opens).
fn low_priority() {
    #[cfg(target_os = "macos")]
    // SAFETY: sets this thread's own QoS class; no pointer is passed
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
    #[cfg(target_os = "linux")]
    // SAFETY: nice value of this thread (its tid), no pointer passed
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, libc::gettid() as libc::id_t, 10);
    }
}

/// Calls `f` on each line of `path` from the last to the first, until it
/// answers false; reads 64 KiB at a time from the end.
fn lines_backwards(path: &Path, mut f: impl FnMut(&str) -> bool) {
    const CHUNK: u64 = 64 * 1024;
    let Ok(mut file) = std::fs::File::open(path) else { return };
    let Ok(mut pos) = file.seek(SeekFrom::End(0)) else { return };
    let mut tail: Vec<u8> = Vec::new();
    while pos > 0 {
        let n = CHUNK.min(pos);
        pos -= n;
        let mut buf = vec![0u8; n as usize];
        if file.seek(SeekFrom::Start(pos)).is_err() || file.read_exact(&mut buf).is_err() {
            return;
        }
        buf.extend_from_slice(&tail);
        // every whole line after the first newline of the chunk (the part
        // before it may be cut: kept for the next chunk)
        let first = if pos == 0 { None } else { buf.iter().position(|b| *b == b'\n') };
        let start = first.map_or(0, |i| i + 1);
        for l in buf[start..].split(|b| *b == b'\n').rev() {
            if !l.is_empty() && !f(&String::from_utf8_lossy(l)) {
                return;
            }
        }
        tail = first.map(|i| buf[..i].to_vec()).unwrap_or_default();
    }
}

fn usage_sum(s: &Sum) -> UsageSum {
    let t: &Tokens = &s.tokens;
    let priced = t.calls > s.unpriced + s.plan;
    let plan_only = s.plan > 0 && s.plan == t.calls;
    UsageSum {
        input: t.input,
        output: t.output,
        cache_read: t.cache_read,
        cache_write: t.cache_write,
        calls: t.calls,
        cost_usd: s.cost_usd,
        unpriced: s.unpriced,
        plan: s.plan,
        words: bise_proto::thread::words::usage_words(s.cost_usd, priced, plan_only, t.input, t.output, t.calls),
    }
}

/// A report by folder, each row under its agent's name now (a folder no
/// agent has: its own name).
fn named(mut r: Report, names: &BTreeMap<&str, &str>) -> Report {
    for (n, _, _) in r.agents.iter_mut() {
        if let Some(name) = names.get(n.as_str()) {
            *n = name.to_string();
        }
    }
    r
}

/// The typed answer of a report.
pub(crate) fn usage_ev(project: bise_proto::Project, counting: bool, r: Report) -> HubEv {
    let row = |name: String, models: String, r: &crate::usage_ledger::Row| UsageRow { name, models, today: usage_sum(&r.today), week: usage_sum(&r.week) };
    HubEv::Usage {
        project,
        counting,
        all: Box::new(row(String::new(), String::new(), &r.all)),
        agents: r.agents.iter().map(|(n, m, x)| row(n.clone(), m.clone(), x)).collect(),
        models: r.models.iter().map(|(n, x)| row(n.clone(), String::new(), x)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_file_backwards_across_chunks() {
        let dir = std::env::temp_dir().join(format!("usage-back-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.log");
        let lines: Vec<String> = (0..20_000).map(|i| format!("{i}\tline {i}")).collect();
        std::fs::write(&p, lines.join("\n") + "\n").unwrap();
        let mut seen = Vec::new();
        lines_backwards(&p, |l| {
            seen.push(l.to_string());
            true
        });
        let want: Vec<String> = lines.iter().rev().cloned().collect();
        assert_eq!(seen, want);
        // stops when asked
        let mut n = 0;
        lines_backwards(&p, |_| {
            n += 1;
            n < 3
        });
        assert_eq!(n, 3);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Law (brief m_44811): the fill plus the live lines count a turn
    /// once, the fill started mid-turn; its usage lines too.
    #[test]
    fn the_fill_and_the_live_lines_count_a_turn_once() {
        use crate::pulse::{turn_line, Turns};
        let dir = std::env::temp_dir().join(format!("usage-turn-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("transcript.log");
        let usage = "  obs: usage: model=nowhere/m in=10 out=0 cache_read=0 cache_write=0";
        let lines: Vec<(u64, &str)> = vec![(1_000, "sb you : go"), (1_001, "  obs: turn_started"), (1_002, usage), (5_000, usage), (9_000, "  obs: turn_done: completed")];
        let text: String = lines.iter().map(|(ms, l)| format!("{ms}\t{l}\n")).collect();
        std::fs::write(&p, text).unwrap();
        // the fill starts at 3_000: it reads the lines before, the live side the others
        let until = 3_000;
        let (mut l, mut week) = (Ledger::default(), super::super::pulse::Week::default());
        read_week(&p, "main", false, (0, 0, until), &mut l, &mut week);
        let mut live = Turns::default();
        for (ms, line) in lines.iter().filter(|(ms, _)| *ms >= until) {
            l.line("main", 0, line);
            live.line(*ms, turn_line(line));
        }
        let t = Turns::merge(week.turns.remove("main").unwrap(), live, true);
        assert_eq!((t.closed, t.open), (vec![(1_001, 9_000)], None));
        let r = l.report(0, |_, _| crate::usage_ledger::Cost::Unknown, |_| false);
        assert_eq!(r.all.week.tokens.calls, 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The fill's cost before (usage lines only) and after (the pulse's
    /// turns and main's fold too), on a copy of a hub's agent folders in
    /// `$TMPDIR/pulse-bench/agents` (brief m_44811): wall and CPU time.
    /// `cargo test -p switchboard --release --lib fill_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn fill_cost() {
        let agents = std::env::temp_dir().join("pulse-bench/agents");
        let until = now_ms();
        let since = week_start(day(until));
        let pulse_since = crate::pulse::window_from(until, &bise_home::clock::offset_at);
        let cpu = || {
            let mut u: libc::rusage = unsafe { std::mem::zeroed() };
            // SAFETY: getrusage fills the struct it is given
            unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut u) };
            let t = |v: libc::timeval| v.tv_sec as f64 + v.tv_usec as f64 / 1e6;
            t(u.ru_utime) + t(u.ru_stime)
        };
        let dirs: Vec<(String, std::path::PathBuf)> = std::fs::read_dir(&agents).unwrap().flatten().map(|e| (e.file_name().to_string_lossy().to_string(), e.path().join("transcript.log"))).filter(|(_, p)| p.exists()).collect();
        for round in 0..2 {
            let (w, c) = (std::time::Instant::now(), cpu());
            let mut l = Ledger::default();
            for (dir, t) in &dirs {
                lines_backwards(t, |raw| l.transcript_line(dir, raw, since, until, day));
            }
            let before = (w.elapsed().as_secs_f64(), cpu() - c);
            let (w, c) = (std::time::Instant::now(), cpu());
            let (mut l, mut week) = (Ledger::default(), super::super::pulse::Week::default());
            for (dir, t) in &dirs {
                read_week(t, dir, dir == "main", (since, pulse_since, until), &mut l, &mut week);
            }
            let after = (w.elapsed().as_secs_f64(), cpu() - c);
            let busy: u64 = crate::pulse::buckets(week.turns.values().flat_map(|t| t.intervals(until)), pulse_since).iter().sum();
            eprintln!("round {round}: {} transcripts; before {:.2} s wall {:.2} s cpu; after {:.2} s wall {:.2} s cpu; busy {} h, said {}", dirs.len(), before.0, before.1, after.0, after.1, busy / 3_600_000, week.said.times(0).len());
        }
    }

    #[test]
    fn the_typed_answer_says_unpriced_and_counting() {
        let mut l = Ledger::default();
        l.line("main", 5, "  obs: usage: model=nowhere/m in=40000 out=0 cache_read=0 cache_write=0");
        let ev = usage_ev(bise_proto::Project::default(), true, l.report(5, |_, _| crate::usage_ledger::Cost::Unknown, |_| false));
        let HubEv::Usage { counting, all, agents, .. } = ev else { panic!() };
        assert!(counting);
        assert_eq!(all.today.words, "— · 40k");
        assert_eq!(agents[0].name, "main");
    }
}
