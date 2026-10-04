//! `sb every` (docs/ambient-roadmap.md B, standing orders): the words and
//! the clock of the hub's timers. A timer wakes its agent with a message
//! from `bise` every N (at least a minute) or every day at HH:MM (local
//! time), until a time or for N times.
//!
//! The timers themselves are sb-core's (bend/hub/timers.bend and
//! core.bend's timers section): their state, their journal lines
//! (`every_set`, `every_fired`, `every_run`, `every_stop`, replayed with
//! the rest of the journal), the decision to fire (due, its agent idle,
//! never a second wake while one from bise is queued), and a dropped
//! agent's timers ending with it; laws fire_is_a_message,
//! wake_never_stacked, times_bound, stop_leaves_no_timer in LAWS.bend.
//!
//! Here, pure: the parsing of `sb every`'s arguments (parse_dur,
//! parse_hhmm, parse_until), the labels, `sb every`'s list and the wake
//! texts (sb-core asks for them with an `every_wake` need), the local
//! clock (next_daily), and [`Timers`], the read-only mirror of the timers
//! sb-core's view carries (`timers`, `timers_ended`) for /scheduled, the
//! ◷ lines and `sb every`. The clock and the view come from the hub
//! (`core.rs`).

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

    /// A new timer's first wake: a period from now, or the next time the
    /// local clock reads its minute.
    pub fn first(&self, now: u64) -> u64 {
        match self {
            Sched::Every(p) => now + p,
            Sched::Daily(m) => next_daily(now, *m),
        }
    }
}

/// A new timer: a request already parsed and checked.
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

impl New {
    /// sb-core's `every_set` input: today's line without its id (sb-core
    /// gives it), the first wake on the hub's clock.
    pub fn input(&self, now: u64) -> Value {
        let mut v = json!({"t": "every_set", "agent": self.agent, "by": self.by, "text": self.text,
                           "next_ms": self.sched.first(now)});
        match self.sched {
            Sched::Every(p) => v["every_ms"] = json!(p),
            Sched::Daily(m) => v["daily_min"] = json!(m),
        }
        for (k, o) in [("until_ms", self.until_ms.map(|u| json!(u))), ("times", self.times.map(|t| json!(t))),
                       ("page", self.page.as_ref().map(|p| json!(p)))] {
            if let Some(o) = o {
                v[k] = o;
            }
        }
        v
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
    /// A timer of sb-core's view (timers.bend `timer_json`), None when it
    /// is not one.
    pub fn from_view(v: &Value) -> Option<Timer> {
        let sched = match (v["every_ms"].as_u64(), v["daily_min"].as_u64()) {
            (Some(p), _) => Sched::Every(p),
            (None, Some(m)) => Sched::Daily(m as u32),
            _ => return None,
        };
        Some(Timer {
            id: v["id"].as_u64()?,
            agent: v["agent"].as_str().unwrap_or_default().to_string(),
            by: v["by"].as_str().unwrap_or_default().to_string(),
            text: v["text"].as_str().unwrap_or_default().to_string(),
            sched,
            next_ms: v["next_ms"].as_u64().unwrap_or(0),
            until_ms: v["until_ms"].as_u64(),
            times: v["times"].as_u64(),
            fired: v["fired"].as_u64().unwrap_or(0),
            page: v["page"].as_str().map(String::from),
            last_ms: v["last_ms"].as_u64().unwrap_or(0),
            runs: v["runs"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect(),
        })
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

/// The timers as sb-core's view last sent them (a mirror: never changed
/// here).
#[derive(Clone, Debug, Default)]
pub struct Timers {
    pub map: BTreeMap<u64, Timer>,
    /// the timers that ended, newest last (the TUI's `/scheduled` shows a
    /// week of them; sb-core keeps the last 200)
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

/// How long `/scheduled` shows an ended timer.
pub const ENDED_SHOWN_MS: u64 = 7 * DAY_MS;

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

impl Timers {
    /// The live timers of sb-core's view (`timers`), in id order.
    pub fn load_live(&mut self, v: &Value) {
        self.map = v.as_array().into_iter().flatten().filter_map(Timer::from_view).map(|t| (t.id, t)).collect();
    }

    /// The ended timers of sb-core's view (`timers_ended`), oldest first.
    pub fn load_ended(&mut self, v: &Value) {
        self.ended = v
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| {
                Some(Ended {
                    timer: Timer::from_view(e)?,
                    ended_ms: e["ended_ms"].as_u64().unwrap_or(0),
                    why: e["why"].as_str().unwrap_or_default().to_string(),
                })
            })
            .collect();
    }

    /// The timers in the hub's state: the running ones, then the ones
    /// that ended in the last [`ENDED_SHOWN_MS`] (with `ended_ms`).
    pub fn state(&self, now: u64) -> Vec<Value> {
        let ended = self.ended.iter().filter(|e| e.ended_ms + ENDED_SHOWN_MS > now).map(Ended::json);
        self.map.values().map(Timer::json).chain(ended).collect()
    }

    /// The answer to sb-core's `every_wake` need (core.bend
    /// `timers_tick`): for each wake it may send (`id`, the count it
    /// would reach, how long it waited), the message (today's words) and,
    /// for a daily timer, its next time on the local clock. Matched by id.
    pub fn wakes(&self, q: &Value, now: u64) -> Value {
        let wakes: Vec<Value> = q["wakes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| {
                let t = self.map.get(&w["id"].as_u64()?)?;
                let waited = w["waited_ms"].as_u64().unwrap_or(0);
                let fired = w["fired"].as_u64().unwrap_or(t.fired + 1);
                let wake = Wake::Due { fired, waited_ms: if waited >= MIN_MS { waited } else { 0 } };
                let next = match t.sched {
                    Sched::Daily(m) => next_daily(now, m),
                    Sched::Every(_) => 0,
                };
                Some(json!({"id": t.id, "text": wake_text(t, wake, now), "next_ms": next}))
            })
            .collect();
        json!({"wakes": wakes})
    }

    /// `/scheduled`'s run now: the message of timer `id` (outside its
    /// count), or None.
    pub fn run_text(&self, id: u64, now: u64) -> Option<String> {
        Some(wake_text(self.map.get(&id)?, Wake::Now, now))
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

    /// How a timer ended, from its `why` (the views' `end`).
    #[test]
    fn ends_read_from_why() {
        assert_eq!(end_of("it ran its times").0, "times");
        assert_eq!(end_of("its end time passed").0, "until");
        assert_eq!(end_of("@w is gone").0, "gone");
        assert_eq!(end_of("stopped by answer-line"), ("stopped", "answer-line".to_string()));
    }

    /// The local clock (the timers' tests on sb-core are in core_tests).
    #[test]
    fn the_local_clock() {
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
