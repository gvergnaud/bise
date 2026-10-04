//! `sb every` (docs/ambient-roadmap.md B, standing orders): the hub's
//! timers, so an agent never burns turns sleeping. A timer wakes its
//! agent with a message from `bise` every N (at least a minute) or every
//! day at HH:MM (local time), until a time or for N times. Durable: three
//! journal lines of the hub's own (`every_set`, `every_fired`,
//! `every_stop`), read back at the hub's start like the PR lines. A wake
//! while the agent is busy waits for its next idle: one pending wake at
//! most, never stacked. A dropped agent's timers stop with it.
//!
//! Pure: the clock and the agents' states come from the hub (`core.rs`).

use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const MIN_MS: u64 = 60_000;

/// The shortest period the hub takes: a minute; `$SB_EVERY_MIN_MS` lowers
/// it for the tests (a real hub never sets it).
pub fn min_ms() -> u64 {
    std::env::var("SB_EVERY_MIN_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(MIN_MS)
}
const HOUR_MS: u64 = 60 * MIN_MS;
const DAY_MS: u64 = 24 * HOUR_MS;
/// A wake that did not reach its agent is tried again this much later.
const RETRY_MS: u64 = MIN_MS;

/// When a timer fires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sched {
    /// every that many ms (at least `MIN_MS`)
    Every(u64),
    /// every day at that minute of the day, local time
    Daily(u32),
}

impl Sched {
    pub fn label(&self) -> String {
        match self {
            Sched::Every(ms) => format!("every {}", dur_label(*ms)),
            Sched::Daily(m) => format!("every day {:02}:{:02}", m / 60, m % 60),
        }
    }

    /// The first time after `now` (Every: from `from`, a missed period is
    /// skipped, never caught up).
    fn next(&self, from: u64, now: u64) -> u64 {
        match self {
            Sched::Every(p) => {
                let n = from + p;
                if n <= now {
                    now + p
                } else {
                    n
                }
            }
            Sched::Daily(m) => next_daily(now, *m),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timer {
    pub id: u64,
    /// the agent it wakes
    pub agent: String,
    /// who set it
    pub by: String,
    pub text: String,
    pub sched: Sched,
    pub next_ms: u64,
    pub until_ms: Option<u64>,
    pub times: Option<u64>,
    pub fired: u64,
    /// `--page <id>`: the page this order refreshes (its frame says
    /// `watching`, its `stop` ends the timer)
    pub page: Option<String>,
    /// when it last woke its agent (0: never)
    pub last_ms: u64,
    /// its last runs (at most [`RUNS_KEPT`], oldest first): each wake,
    /// a run now too
    pub runs: Vec<u64>,
}

impl Timer {
    fn ran(&mut self, at: u64) {
        if at == 0 || self.runs.last() == Some(&at) {
            return;
        }
        self.runs.push(at);
        if self.runs.len() > RUNS_KEPT {
            self.runs.remove(0);
        }
    }

    /// The timer in the hub's state (amb-mac's menu, m_5435).
    pub fn json(&self) -> Value {
        let mut v = json!({"id": self.id, "agent": self.agent, "label": self.sched.label(), "text": self.text,
                           "by": self.by, "next_ms": self.next_ms, "fired": self.fired, "last_ms": self.last_ms});
        if let Some(u) = self.until_ms {
            v["until_ms"] = json!(u);
        }
        if let Some(t) = self.times {
            v["times"] = json!(t);
        }
        if let Some(p) = &self.page {
            v["page"] = json!(p);
        }
        if let Sched::Every(ms) = self.sched {
            v["every_ms"] = json!(ms);
        }
        v["runs"] = json!(self.runs);
        v
    }
}

/// What `add` takes: a request already parsed and checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct New {
    pub agent: String,
    pub by: String,
    pub text: String,
    pub sched: Sched,
    pub until_ms: Option<u64>,
    pub times: Option<u64>,
    pub page: Option<String>,
}

/// What a tick asks the hub to do for one timer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// wake `agent` with `text`; `journal` records it. The hub then says
    /// whether it reached the agent: [`Timers::delivered`] or
    /// [`Timers::undelivered`] (a fire counts only once delivered)
    Wake { id: u64, agent: String, text: String, journal: Value },
    /// the timer ends (`journal` records why)
    Stop { journal: Value },
}

/// The agent of a timer as the hub sees it at a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentNow {
    /// archived, dropped, unknown: its timers stop
    Gone,
    /// ready for a wake now
    Idle,
    /// busy, starting, stopped: the wake waits
    Busy,
}

#[derive(Clone, Debug, Default)]
pub struct Timers {
    pub map: BTreeMap<u64, Timer>,
    last_id: u64,
    /// the timers that ended, newest last (the TUI's `/scheduled`
    /// shows a week of them; at most [`ENDED_KEPT`])
    pub ended: Vec<Ended>,
}

/// A timer that ended: when and why (its `every_stop` line).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ended {
    pub timer: Timer,
    /// 0: an old journal line without its time
    pub ended_ms: u64,
    pub why: String,
}

const ENDED_KEPT: usize = 200;
/// How long `/scheduled` shows an ended timer.
pub const ENDED_SHOWN_MS: u64 = 7 * DAY_MS;
/// The last runs a timer keeps (its opened view: 'its runs').
const RUNS_KEPT: usize = 5;

/// How a timer ended, from its `why`: `times` (it ran its times),
/// `until` (its end time passed), `gone` (its agent is gone), `stopped`
/// with who stopped it (`user`, or an agent's name).
pub fn end_of(why: &str) -> (&'static str, String) {
    if why == "it ran its times" {
        ("times", String::new())
    } else if why == "its end time passed" {
        ("until", String::new())
    } else if why.ends_with(" is gone") {
        ("gone", String::new())
    } else if let Some(by) = why.strip_prefix("stopped by ") {
        ("stopped", if by == "the user" { "user".into() } else { by.to_string() })
    } else {
        ("stopped", String::new())
    }
}

impl Ended {
    /// An ended timer in the hub's state: the timer's fields, then
    /// `ended_ms`, `why`, `end` and `stopped_by` ([`end_of`]).
    pub fn json(&self) -> Value {
        let mut v = self.timer.json();
        let (end, by) = end_of(&self.why);
        v["ended_ms"] = json!(self.ended_ms);
        v["why"] = json!(self.why);
        v["end"] = json!(end);
        if !by.is_empty() {
            v["stopped_by"] = json!(by);
        }
        v
    }
}

/// Whether a journal event is one of the timers' lines.
pub fn is_line(ev: &Value) -> bool {
    matches!(ev["type"].as_str(), Some("every_set" | "every_fired" | "every_stop" | "every_run"))
}

impl Timers {
    /// Read one journal line (idempotent: a replay gives the same state).
    pub fn read(&mut self, ev: &Value) {
        let id = ev["id"].as_u64().unwrap_or(0);
        match ev["type"].as_str() {
            Some("every_set") => {
                let sched = match (ev["every_ms"].as_u64(), ev["daily_min"].as_u64()) {
                    (Some(p), _) => Sched::Every(p.max(1000)),
                    (None, Some(m)) => Sched::Daily(m as u32 % (24 * 60)),
                    _ => return,
                };
                self.last_id = self.last_id.max(id);
                self.map.insert(
                    id,
                    Timer {
                        id,
                        agent: ev["agent"].as_str().unwrap_or_default().to_string(),
                        by: ev["by"].as_str().unwrap_or_default().to_string(),
                        text: ev["text"].as_str().unwrap_or_default().to_string(),
                        sched,
                        next_ms: ev["next_ms"].as_u64().unwrap_or(0),
                        until_ms: ev["until_ms"].as_u64(),
                        times: ev["times"].as_u64(),
                        fired: 0,
                        page: ev["page"].as_str().map(String::from),
                        last_ms: 0,
                        runs: Vec::new(),
                    },
                );
            }
            Some("every_fired") => {
                if let Some(t) = self.map.get_mut(&id) {
                    t.fired = ev["fired"].as_u64().unwrap_or(t.fired);
                    t.next_ms = ev["next_ms"].as_u64().unwrap_or(t.next_ms);
                    t.last_ms = ev["at"].as_u64().unwrap_or(t.last_ms);
                    if ev["undelivered"].as_bool() == Some(true) {
                        // the lost wake was its last run: not a run
                        let at = t.runs.pop();
                        if at.is_some_and(|a| a != t.last_ms) {
                            t.runs.extend(at);
                        }
                    } else {
                        t.ran(t.last_ms);
                    }
                }
            }
            // a run now (`/scheduled`'s r): outside the count, the next
            // wake unchanged
            Some("every_run") => {
                if let Some(t) = self.map.get_mut(&id) {
                    t.last_ms = ev["at"].as_u64().unwrap_or(t.last_ms);
                    t.ran(t.last_ms);
                }
            }
            Some("every_stop") => {
                if let Some(timer) = self.map.remove(&id) {
                    let why = ev["why"].as_str().unwrap_or_default().to_string();
                    self.ended.push(Ended { timer, ended_ms: ev["at"].as_u64().unwrap_or(0), why });
                    if self.ended.len() > ENDED_KEPT {
                        self.ended.remove(0);
                    }
                }
            }
            _ => {}
        }
    }

    /// A new timer: its journal line (already read in) and its id.
    pub fn add(&mut self, n: New, now: u64) -> (u64, Value) {
        let id = self.last_id + 1;
        let next = match n.sched {
            Sched::Every(p) => now + p,
            Sched::Daily(m) => next_daily(now, m),
        };
        let mut j = json!({"type": "every_set", "id": id, "agent": n.agent, "by": n.by, "text": n.text,
                           "next_ms": next, "at": now});
        match n.sched {
            Sched::Every(p) => j["every_ms"] = json!(p),
            Sched::Daily(m) => j["daily_min"] = json!(m),
        }
        if let Some(u) = n.until_ms {
            j["until_ms"] = json!(u);
        }
        if let Some(t) = n.times {
            j["times"] = json!(t);
        }
        if let Some(p) = &n.page {
            j["page"] = json!(p);
        }
        self.read(&j);
        (id, j)
    }

    /// Stop a timer: its journal line (already read in), or None.
    pub fn stop(&mut self, id: u64, why: &str, now: u64) -> Option<Value> {
        self.map.contains_key(&id).then(|| {
            let j = json!({"type": "every_stop", "id": id, "why": why, "at": now});
            self.read(&j);
            j
        })
    }

    /// `/scheduled`'s run now: one wake of timer `id` at once, outside
    /// its count, its next wake unchanged. Its agent, the wake's text and
    /// its journal line (already read in), or None.
    pub fn run_now(&mut self, id: u64, now: u64) -> Option<(String, String, Value)> {
        let t = self.map.get(&id)?.clone();
        let j = json!({"type": "every_run", "id": id, "at": now});
        self.read(&j);
        Some((t.agent.clone(), wake_text(&t, Wake::Now, now), j))
    }

    /// The timers in the hub's state: the running ones, then the ones
    /// that ended in the last [`ENDED_SHOWN_MS`] (with `ended_ms`).
    pub fn state(&self, now: u64) -> Vec<Value> {
        let ended = self.ended.iter().filter(|e| e.ended_ms + ENDED_SHOWN_MS > now).map(Ended::json);
        self.map.values().map(Timer::json).chain(ended).collect()
    }

    /// One tick: what to do now. `agent` says how each agent is; the
    /// acts' journal lines are already read in.
    pub fn tick(&mut self, now: u64, agent: impl Fn(&str) -> AgentNow) -> Vec<Act> {
        let mut acts = Vec::new();
        let ids: Vec<u64> = self.map.keys().copied().collect();
        for id in ids {
            let t = self.map[&id].clone();
            let a = agent(&t.agent);
            let why = if a == AgentNow::Gone {
                Some(format!("@{} is gone", t.agent))
            } else if t.until_ms.is_some_and(|u| now >= u) {
                Some("its end time passed".to_string())
            } else {
                None
            };
            if let Some(why) = why {
                if let Some(journal) = self.stop(id, &why, now) {
                    acts.push(Act::Stop { journal });
                }
                continue;
            }
            if now < t.next_ms || a != AgentNow::Idle {
                continue;
            }
            let fired = t.fired + 1;
            let journal = json!({"type": "every_fired", "id": id, "fired": fired, "next_ms": t.sched.next(t.next_ms, now), "at": now});
            self.read(&journal);
            // due while its agent was busy: how long it waited
            let waited = now.saturating_sub(t.next_ms);
            let wake = Wake::Due { fired, waited_ms: if waited >= MIN_MS { waited } else { 0 } };
            acts.push(Act::Wake { id, agent: t.agent.clone(), text: wake_text(&t, wake, now), journal });
        }
        acts
    }

    /// The wake of timer `id` reached its agent: the timer ends when it
    /// ran its times (its journal line, already read in).
    pub fn delivered(&mut self, id: u64, now: u64) -> Option<Value> {
        let t = self.map.get(&id)?;
        if t.times.is_some_and(|n| t.fired >= n) {
            return self.stop(id, "it ran its times", now);
        }
        None
    }

    /// The wake of timer `id` did not reach its agent (the core dropped
    /// it): the fire does not count and it is tried again in a minute
    /// (amb-tools m_5822: a one-shot timer was spent on a lost wake). Its
    /// journal line, already read in.
    pub fn undelivered(&mut self, id: u64, now: u64) -> Option<Value> {
        let t = self.map.get(&id)?;
        let j = json!({"type": "every_fired", "id": id, "fired": t.fired.saturating_sub(1), "next_ms": now + RETRY_MS, "at": t.last_ms, "undelivered": true});
        self.read(&j);
        Some(j)
    }

    /// `sb every`: one line per timer, or a line saying there is none.
    pub fn list(&self, now: u64) -> String {
        if self.map.is_empty() {
            return "no timers. `sb every <10m|1h|day 07:30> \"<message>\"` sets one.".into();
        }
        self.map.values().map(|t| line(t, now)).collect::<Vec<_>>().join("\n")
    }

    /// The timers that watch page `id` (`--page`).
    pub fn of_page(&self, id: &str) -> Vec<&Timer> {
        self.map.values().filter(|t| t.page.as_deref() == Some(id)).collect()
    }

    /// The timers of `agent`, for `sb tasks`.
    pub fn of(&self, agent: &str, now: u64) -> Vec<String> {
        self.map.values().filter(|t| t.agent == agent).map(|t| line(t, now)).collect()
    }
}

/// One timer as `sb every` and `sb tasks` show it.
fn line(t: &Timer, now: u64) -> String {
    let mut s = format!("#{} @{} {} · next {}", t.id, t.agent, t.sched.label(), when_label(t.next_ms, now));
    if let Some(u) = t.until_ms {
        s.push_str(&format!(" · until {}", when_label(u, now)));
    }
    if let Some(n) = t.times {
        s.push_str(&format!(" · {}/{} times", t.fired, n));
    }
    if let Some(p) = &t.page {
        s.push_str(&format!(" · page {p}"));
    }
    s.push_str(&format!(" · \"{}\" (by {})", clip(&t.text, 80), t.by));
    s
}

/// Which wake: a due one (its count, how long it waited for its busy
/// agent) or a run now (outside the count).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wake {
    Due { fired: u64, waited_ms: u64 },
    Now,
}

/// The message a wake carries: `timer #48 (every 2m, until 18:00, 2/6,
/// waited 4m, set by answer-line): <text>` then the stop hint; a run now
/// says `run now by the user` in place of the count. The TUI reads it
/// back (its ◷ line): keep the shape.
fn wake_text(t: &Timer, wake: Wake, now: u64) -> String {
    let mut how = t.sched.label();
    if let Some(u) = t.until_ms {
        how.push_str(&format!(", until {}", when_label(u, now)));
    }
    match wake {
        Wake::Due { fired, waited_ms } => {
            if let Some(n) = t.times {
                how.push_str(&format!(", {}/{}", fired, n));
            }
            if waited_ms > 0 {
                how.push_str(&format!(", waited {} for {} to finish", dur_label(waited_ms / MIN_MS * MIN_MS), t.agent));
            }
        }
        Wake::Now => how.push_str(", run now by the user"),
    }
    format!("timer #{} ({}, set by {}): {}\n(stop it: sb every --stop {})", t.id, how, t.by, t.text, t.id)
}

fn clip(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= n {
        one
    } else {
        format!("{}…", one.chars().take(n).collect::<String>())
    }
}

// ---- parsing (the CLI's: the agent's own clock and time zone) ----

/// `10m`, `1h30m`, `90s`, `2d`: ms.
pub fn parse_dur(s: &str) -> Result<u64, String> {
    let bad = || format!("not a duration: {} (10m, 1h, 1h30m, 2d)", s);
    let (mut total, mut num) = (0u64, String::new());
    for c in s.trim().chars() {
        if c.is_ascii_digit() {
            num.push(c);
            continue;
        }
        let unit = match c {
            's' => 1000,
            'm' => MIN_MS,
            'h' => HOUR_MS,
            'd' => DAY_MS,
            _ => return Err(bad()),
        };
        total += num.parse::<u64>().map_err(|_| bad())? * unit;
        num.clear();
    }
    if !num.is_empty() || total == 0 {
        return Err(bad());
    }
    Ok(total)
}

fn dur_label(ms: u64) -> String {
    let (d, h, m) = (ms / DAY_MS, ms % DAY_MS / HOUR_MS, ms % HOUR_MS / MIN_MS);
    let mut s = String::new();
    for (n, u) in [(d, "d"), (h, "h"), (m, "m")] {
        if n > 0 {
            s.push_str(&format!("{}{}", n, u));
        }
    }
    if s.is_empty() {
        format!("{}s", ms / 1000)
    } else {
        s
    }
}

/// `07:30`: the minute of the day.
pub fn parse_hhmm(s: &str) -> Result<u32, String> {
    let bad = || format!("not a time of day: {} (07:30)", s);
    let (h, m) = s.trim().split_once(':').ok_or_else(bad)?;
    let (h, m): (u32, u32) = (h.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?);
    if h > 23 || m > 59 {
        return Err(bad());
    }
    Ok(h * 60 + m)
}

/// `--until`: `18:00` (its next time), `tomorrow 18:00`, `2025-10-03 18:00`
/// (or with a T), or a duration from now (`2h`). Absolute ms.
pub fn parse_until(s: &str, now: u64) -> Result<u64, String> {
    let s = s.trim();
    if let Ok(d) = parse_dur(s) {
        return Ok(now + d);
    }
    if let Some(rest) = s.strip_prefix("tomorrow") {
        let m = match rest.trim() {
            "" => 0,
            t => parse_hhmm(t)?,
        };
        let today = local_at(now, 0, 0).ok_or("no local time")?;
        return local_at(today + DAY_MS + HOUR_MS * 2, m / 60, m % 60).ok_or_else(|| "no local time".to_string());
    }
    if let Ok(m) = parse_hhmm(s) {
        return Ok(next_daily(now, m));
    }
    let (date, time) = s.split_once([' ', 'T']).unwrap_or((s, "00:00"));
    let p: Vec<&str> = date.split('-').collect();
    let m = parse_hhmm(time)?;
    match p.as_slice() {
        [y, mo, d] => {
            let (y, mo, d) = (y.parse::<i32>(), mo.parse::<i32>(), d.parse::<i32>());
            match (y, mo, d) {
                (Ok(y), Ok(mo), Ok(d)) => mktime(y, mo, d, (m / 60) as i32, (m % 60) as i32)
                    .filter(|t| *t > now)
                    .ok_or_else(|| format!("{} is not in the future", s)),
                _ => Err(format!("not a time: {}", s)),
            }
        }
        _ => Err(format!("not a time: {} (18:00, tomorrow 18:00, 2025-10-03 18:00, 2h)", s)),
    }
}

// ---- local time (libc) ----

fn tm_of(ms: u64) -> Option<libc::tm> {
    let t = (ms / 1000) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::localtime_r(&t, &mut tm) };
    (!r.is_null()).then_some(tm)
}

/// Days since 1970-01-01 of a civil date (the proleptic Gregorian
/// calendar; H. Hinnant's days_from_civil).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `YYYY-MM-DD` as days since 1970-01-01 (None: not a date).
pub fn parse_day(s: &str) -> Option<i64> {
    let mut it = s.trim().splitn(3, '-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| days_from_civil(y, m, d))
}

/// The user's local date at `ms`, as days since 1970-01-01.
pub fn local_day(ms: u64) -> Option<i64> {
    let tm = tm_of(ms)?;
    Some(days_from_civil(tm.tm_year as i64 + 1900, tm.tm_mon as u32 + 1, tm.tm_mday as u32))
}

fn mk(mut tm: libc::tm) -> Option<u64> {
    tm.tm_isdst = -1;
    let t = unsafe { libc::mktime(&mut tm) };
    (t >= 0).then(|| t as u64 * 1000)
}

fn mktime(y: i32, mo: i32, d: i32, h: i32, mi: i32) -> Option<u64> {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = y - 1900;
    tm.tm_mon = mo - 1;
    tm.tm_mday = d;
    tm.tm_hour = h;
    tm.tm_min = mi;
    mk(tm)
}

/// The same local day as `ms`, at h:m.
fn local_at(ms: u64, h: u32, m: u32) -> Option<u64> {
    let mut tm = tm_of(ms)?;
    tm.tm_hour = h as i32;
    tm.tm_min = m as i32;
    tm.tm_sec = 0;
    mk(tm)
}

/// The next time after `now` the local clock reads that minute of the day.
pub fn next_daily(now: u64, minute: u32) -> u64 {
    let (h, m) = (minute / 60, minute % 60);
    let Some(today) = local_at(now, h, m) else { return now + DAY_MS };
    if today > now {
        return today;
    }
    let mut tm = tm_of(now).expect("local time");
    tm.tm_mday += 1;
    tm.tm_hour = h as i32;
    tm.tm_min = m as i32;
    tm.tm_sec = 0;
    mk(tm).unwrap_or(today + DAY_MS)
}

/// `14:20`, `tomorrow 07:30`, or `2025-10-04 18:00` past tomorrow.
fn when_label(ms: u64, now: u64) -> String {
    let (Some(t), Some(n)) = (tm_of(ms), tm_of(now)) else { return format!("{} ms", ms) };
    let hm = format!("{:02}:{:02}", t.tm_hour, t.tm_min);
    let day = |tm: &libc::tm| (tm.tm_year, tm.tm_yday);
    if day(&t) == day(&n) {
        return hm;
    }
    let tomorrow = tm_of(now + DAY_MS).map(|x| day(&x));
    if Some(day(&t)) == tomorrow {
        return format!("tomorrow {}", hm);
    }
    format!("{}-{:02}-{:02} {}", t.tm_year + 1900, t.tm_mon + 1, t.tm_mday, hm)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_790_000_000_000;

    fn new(sched: Sched) -> New {
        New { agent: "w".into(), by: "main".into(), text: "check HN".into(), sched, until_ms: None, times: None, page: None }
    }

    fn wakes(acts: &[Act]) -> usize {
        acts.iter().filter(|a| matches!(a, Act::Wake { .. })).count()
    }

    #[test]
    fn durations_parse_and_label() {
        assert_eq!(parse_dur("10m").unwrap(), 10 * MIN_MS);
        assert_eq!(parse_dur("1h30m").unwrap(), 90 * MIN_MS);
        assert_eq!(parse_dur("2d").unwrap(), 2 * DAY_MS);
        for bad in ["", "10", "m", "10x", "1h3"] {
            assert!(parse_dur(bad).is_err(), "{}", bad);
        }
        assert_eq!(dur_label(90 * MIN_MS), "1h30m");
        assert_eq!(parse_hhmm("07:30").unwrap(), 450);
        assert!(parse_hhmm("24:00").is_err());
    }

    #[test]
    fn a_timer_fires_when_due_and_its_agent_is_idle_never_stacked() {
        let mut ts = Timers::default();
        let (id, _) = ts.add(new(Sched::Every(10 * MIN_MS)), NOW);
        assert_eq!(id, 1);
        assert!(ts.tick(NOW + MIN_MS, |_| AgentNow::Idle).is_empty(), "not due yet");
        // due while busy: nothing, however long it stays busy
        for k in 0..30 {
            assert!(ts.tick(NOW + (10 + k) * MIN_MS, |_| AgentNow::Busy).is_empty());
        }
        // idle again: one wake, the next one a period later (no catch-up)
        let acts = ts.tick(NOW + 40 * MIN_MS, |_| AgentNow::Idle);
        assert_eq!(wakes(&acts), 1);
        let Act::Wake { text, journal, .. } = &acts[0] else { panic!() };
        // due at +10m, busy until +40m: it says how long it waited
        assert!(text.starts_with("timer #1 (every 10m, waited 30m for w to finish, set by main): check HN"), "{}", text);
        assert_eq!(journal["next_ms"], NOW + 50 * MIN_MS);
        assert!(ts.tick(NOW + 41 * MIN_MS, |_| AgentNow::Idle).is_empty());
        assert_eq!(wakes(&ts.tick(NOW + 50 * MIN_MS, |_| AgentNow::Idle)), 1);
    }

    #[test]
    fn the_journal_rebuilds_the_timers() {
        let mut ts = Timers::default();
        let mut lines = vec![ts.add(new(Sched::Every(MIN_MS)), NOW).1];
        let mut n = new(Sched::Every(5 * MIN_MS));
        n.agent = "x".into();
        n.times = Some(2);
        lines.push(ts.add(n, NOW).1);
        for a in ts.tick(NOW + 5 * MIN_MS, |_| AgentNow::Idle) {
            match a {
                Act::Wake { journal, .. } | Act::Stop { journal } => lines.push(journal),
            }
        }
        lines.push(ts.stop(1, "asked", NOW + 6 * MIN_MS).unwrap());
        let mut back = Timers::default();
        for l in lines.iter().chain(lines.iter()) {
            assert!(is_line(l));
            back.read(l);
        }
        assert_eq!(back.map, ts.map);
        assert_eq!(back.map[&2].fired, 1);
        assert_eq!(back.add(new(Sched::Every(MIN_MS)), NOW).0, 3, "ids never reused");
    }

    #[test]
    fn timers_end_with_their_times_their_end_or_their_agent() {
        let mut ts = Timers::default();
        let mut n = new(Sched::Every(MIN_MS));
        n.times = Some(2);
        ts.add(n, NOW);
        let mut n = new(Sched::Every(MIN_MS));
        n.until_ms = Some(NOW + 3 * MIN_MS);
        n.agent = "u".into();
        ts.add(n, NOW);
        ts.add(New { agent: "gone".into(), ..new(Sched::Every(MIN_MS)) }, NOW);
        let st = |a: &str| if a == "gone" { AgentNow::Gone } else { AgentNow::Idle };
        let deliver = |ts: &mut Timers, acts: Vec<Act>| {
            for a in acts {
                if let Act::Wake { id, .. } = a {
                    ts.delivered(id, NOW);
                }
            }
        };
        let acts = ts.tick(NOW + MIN_MS, st);
        assert_eq!(wakes(&acts), 2);
        assert!(!ts.map.contains_key(&3), "a dropped agent's timer stops");
        deliver(&mut ts, acts);
        let acts = ts.tick(NOW + 2 * MIN_MS, st);
        assert!(ts.map.contains_key(&1), "its second fire counts once delivered");
        deliver(&mut ts, acts);
        assert!(!ts.map.contains_key(&1), "two times: done");
        ts.tick(NOW + 3 * MIN_MS, st);
        assert!(ts.map.is_empty(), "past its end");
    }

    /// amb-tools m_5822: a one-shot day timer fired, its wake never reached
    /// main, and `it ran its times` spent it. A fire counts only once its
    /// wake is delivered; a lost one is tried again a minute later.
    #[test]
    fn a_one_shot_timer_is_spent_only_by_a_delivered_wake() {
        let mut ts = Timers::default();
        let mut n = new(Sched::Daily(7 * 60 + 30));
        n.times = Some(1);
        let (id, set) = ts.add(n, NOW);
        let due = ts.map[&id].next_ms;
        let mut lines = vec![set];
        let acts = ts.tick(due, |_| AgentNow::Idle);
        assert_eq!(wakes(&acts), 1);
        let Act::Wake { journal, .. } = &acts[0] else { panic!() };
        lines.push(journal.clone());
        // lost: not spent, not counted, due again in a minute
        let j = ts.undelivered(id, due).unwrap();
        lines.push(j);
        assert_eq!((ts.map[&id].fired, ts.map[&id].next_ms), (0, due + RETRY_MS));
        assert!(ts.tick(due + 1000, |_| AgentNow::Idle).is_empty(), "no retry before the minute");
        // the retry is delivered: now it ran its times
        let acts = ts.tick(due + RETRY_MS, |_| AgentNow::Idle);
        assert_eq!(wakes(&acts), 1);
        let Act::Wake { text, journal, .. } = &acts[0] else { panic!() };
        assert!(text.contains("1/1"), "{text}");
        lines.push(journal.clone());
        lines.push(ts.delivered(id, due + RETRY_MS).expect("ran its times"));
        assert!(ts.map.is_empty());
        // the journal replays to the same end
        let mut back = Timers::default();
        for l in &lines {
            back.read(l);
        }
        assert!(back.map.is_empty());
        // a delivered wake of a timer with times left: no stop
        let (id2, _) = ts.add(new(Sched::Every(MIN_MS)), NOW);
        ts.tick(NOW + MIN_MS, |_| AgentNow::Idle);
        assert_eq!(ts.delivered(id2, NOW + MIN_MS), None);
    }

    /// `/scheduled`: a week of ended timers with why they ended, each
    /// timer's last runs, a run now outside the count, and the wake's
    /// text says how long a busy agent made it wait.
    #[test]
    fn ended_timers_runs_and_run_now() {
        let mut ts = Timers::default();
        let mut n = new(Sched::Every(2 * MIN_MS));
        n.times = Some(6);
        let (id, set) = ts.add(n, NOW);
        let mut lines = vec![set];
        // due at +2m, its agent busy until +6m: one wake that waited 4m
        assert!(ts.tick(NOW + 5 * MIN_MS, |_| AgentNow::Busy).is_empty());
        let acts = ts.tick(NOW + 6 * MIN_MS, |_| AgentNow::Idle);
        let Act::Wake { text, journal, .. } = &acts[0] else { panic!() };
        assert!(text.starts_with("timer #1 (every 2m, 1/6, waited 4m for w to finish, set by main): check HN"), "{text}");
        lines.push(journal.clone());
        // a run now: outside the count, the next wake unchanged
        let next = ts.map[&id].next_ms;
        let (agent, text, j) = ts.run_now(id, NOW + 7 * MIN_MS).unwrap();
        assert_eq!(agent, "w");
        assert!(text.starts_with("timer #1 (every 2m, run now by the user, set by main): check HN"), "{text}");
        lines.push(j);
        assert_eq!((ts.map[&id].fired, ts.map[&id].next_ms), (1, next));
        assert_eq!(ts.map[&id].runs, vec![NOW + 6 * MIN_MS, NOW + 7 * MIN_MS]);
        assert!(ts.run_now(99, NOW).is_none());
        // stopped by the user: ended, with when and why, in the state a week
        lines.push(ts.stop(id, "stopped by the user", NOW + 8 * MIN_MS).unwrap());
        assert!(ts.map.is_empty());
        let st = ts.state(NOW + 9 * MIN_MS);
        assert_eq!(st.len(), 1);
        assert_eq!((st[0]["end"].as_str(), st[0]["stopped_by"].as_str()), (Some("stopped"), Some("user")));
        assert_eq!(st[0]["ended_ms"], NOW + 8 * MIN_MS);
        assert_eq!(st[0]["runs"].as_array().unwrap().len(), 2);
        assert!(ts.state(NOW + 8 * MIN_MS + ENDED_SHOWN_MS).is_empty(), "a week later: gone from the list");
        // the journal gives back the same ended timer
        let mut back = Timers::default();
        for l in &lines {
            back.read(l);
        }
        assert_eq!(back.ended, ts.ended);
        assert_eq!(end_of("it ran its times").0, "times");
        assert_eq!(end_of("its end time passed").0, "until");
        assert_eq!(end_of("@w is gone").0, "gone");
        assert_eq!(end_of("stopped by answer-line"), ("stopped", "answer-line".to_string()));
    }

    #[test]
    fn daily_timers_land_on_the_local_clock() {
        let n = next_daily(NOW, 7 * 60 + 30);
        assert!(n > NOW && n <= NOW + DAY_MS + HOUR_MS);
        let tm = tm_of(n).unwrap();
        assert_eq!((tm.tm_hour, tm.tm_min, tm.tm_sec), (7, 30, 0));
        assert!(next_daily(n, 450) > n, "at 07:30 sharp: tomorrow's");
        let u = parse_until("tomorrow 18:00", NOW).unwrap();
        let tm = tm_of(u).unwrap();
        assert_eq!((tm.tm_hour, tm.tm_min), (18, 0));
        assert!(u > NOW + 6 * HOUR_MS && u < NOW + 2 * DAY_MS);
        assert_eq!(parse_until("2h", NOW).unwrap(), NOW + 2 * HOUR_MS);
        assert!(parse_until("2001-01-01 10:00", NOW).is_err());
        assert!(when_label(u, NOW).starts_with("tomorrow 18:00"));
    }
}
