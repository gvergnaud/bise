//! Scheduled tasks (site/m/timers) as the user reads them, pure (batch 3b,
//! architect m_11122): moved from the TUI's scheduled.rs so the hub's
//! thread fold and the TUI read the hub's `scheduled : <json>` lines and
//! the runs (bise's wakes) with one parser and one set of words. The
//! words are designer's: "scheduled task" for one, "scheduled" for the
//! lines, "run" for a wake. The caller passes `now` (the fold: the line's
//! own time) and the UTC offset at a moment (`off`).

use super::Scheduled;
use serde_json::Value;

/// The UTC offset (seconds east) at a moment: the caller's clock.
pub type Offset<'a> = &'a dyn Fn(u64) -> i32;

/// One scheduled task: the hub's state (`timers`) or its line's JSON.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Task {
    pub id: u64,
    /// the agent it wakes
    pub agent: String,
    /// who set it
    pub by: String,
    /// `every 2m`, `every day 07:30`
    pub label: String,
    pub text: String,
    pub next_ms: u64,
    pub last_ms: u64,
    pub fired: u64,
    pub times: Option<u64>,
    pub until_ms: Option<u64>,
    /// the page it keeps fresh (ambient's `--page`)
    pub page: Option<String>,
    /// its last runs, oldest first
    pub runs: Vec<u64>,
    /// ended: when (an active one has none)
    pub ended_ms: Option<u64>,
    /// how it ended: `times`, `until`, `gone`, `stopped`
    pub end: String,
    /// who stopped it: `user`, an agent, or empty
    pub stopped_by: String,
}

impl Task {
    pub fn of(v: &Value) -> Option<Task> {
        let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
        Some(Task {
            id: v["id"].as_u64()?,
            agent: s("agent"),
            by: s("by"),
            label: s("label"),
            text: s("text"),
            next_ms: v["next_ms"].as_u64().unwrap_or(0),
            last_ms: v["last_ms"].as_u64().unwrap_or(0),
            fired: v["fired"].as_u64().unwrap_or(0),
            times: v["times"].as_u64(),
            until_ms: v["until_ms"].as_u64(),
            page: v["page"].as_str().filter(|p| !p.is_empty()).map(String::from),
            runs: v["runs"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default(),
            ended_ms: v["ended_ms"].as_u64(),
            end: s("end"),
            stopped_by: s("stopped_by"),
        })
    }

    pub fn active(&self) -> bool {
        self.ended_ms.is_none()
    }

    /// How often, in a line: `every 2m`, `every day 07:30`, `once`.
    pub fn when(&self) -> String {
        every_words(&self.label, self.times)
    }

    /// How often, short (the 80-column list): `2m`, `daily 07:30`, `once`.
    pub fn when_short(&self) -> String {
        if self.times == Some(1) {
            return "once".into();
        }
        match self.label.strip_prefix("every day ") {
            Some(hm) => format!("daily {hm}"),
            None => self.label.trim_start_matches("every ").to_string(),
        }
    }

    /// How many so far, or until when: `2 of 6`, `until 18:00`, or "".
    pub fn so_far(&self, now: u64, off: Offset) -> String {
        match (self.times, self.until_ms) {
            (Some(n), _) => format!("{} of {}", self.fired, n),
            (None, Some(u)) => format!("until {}", ahead(u, now, off)),
            _ => String::new(),
        }
    }

    /// Why it ended: `ran its 6 times`, `stopped by you`, `stopped by
    /// answer-line`, `its end time passed`, `answer-line is gone`.
    pub fn ended_words(&self) -> String {
        match self.end.as_str() {
            "times" => match self.times {
                Some(1) => "ran once".into(),
                Some(n) => format!("ran its {n} times"),
                None => "ran its times".into(),
            },
            "until" => "its end time passed".into(),
            "gone" => format!("{} is gone", self.agent),
            _ => match self.stopped_by.as_str() {
                "user" => "stopped by you".into(),
                "" => "stopped".into(),
                by => format!("stopped by {by}"),
            },
        }
    }
}

/// How often a task runs, in words, from its fields (the hub's typed rows
/// and the TUI's lines say it with this one fn): `every 2m`, `every day
/// 07:30` (its schedule's label), `once` (one run).
pub fn every_words(label: &str, times: Option<u64>) -> String {
    if times == Some(1) {
        "once".into()
    } else {
        label.to_string()
    }
}

/// A time to come: `14:22`, `tomorrow 07:30` (when.rs, at `off`).
pub fn ahead(ms: u64, now: u64, off: Offset) -> String {
    super::when::ahead(ms, off(ms), now, off(now))
}

/// `s` on one line, cut at `n` chars with `…`.
pub fn clip(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= n {
        one
    } else {
        format!("{}…", one.chars().take(n.saturating_sub(1)).collect::<String>())
    }
}

/// The hub's `scheduled : <json>` line: one set (`ev: set`) or ended
/// (`ev: end`); main's copy of another agent's (`in: main`) names it.
pub fn hub_line(raw: &str, now: u64, off: Offset) -> Option<Scheduled> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let t = Task::of(&v)?;
    match v["ev"].as_str()? {
        "set" => {
            let who = if t.by == t.agent || t.by.is_empty() {
                format!("{} scheduled #{}", t.agent, t.id)
            } else {
                format!("{} scheduled #{} for {}", t.by, t.id, t.agent)
            };
            let mut h = format!("{who} · {}", t.when());
            match (t.times, t.until_ms) {
                (Some(n), _) if n > 1 => h.push_str(&format!(" · {n} times")),
                (None, Some(u)) => h.push_str(&format!(" · until {}", ahead(u, now, off))),
                _ => {}
            }
            h.push_str(&format!(" · next {}", ahead(t.next_ms, now, off)));
            Some(Scheduled { id: t.id, head: h, words: t.text })
        }
        "end" => {
            let mut h = format!("scheduled #{} ended · {}", t.id, t.ended_words());
            if v["in"].as_str() == Some("main") && t.agent != "main" {
                h.push_str(&format!(" ({} · {})", t.agent, clip(&t.text, 32)));
            }
            Some(Scheduled { id: t.id, head: h, words: String::new() })
        }
        _ => None,
    }
}

/// A run: the wake an agent reads from bise (switchboard every.rs
/// `wake_text`, `timer #48 (every 2m, 2/6, set by answer-line): <words>`
/// then the stop hint) as its ◷ line: `scheduled #48 · 2 of 6 · <first
/// words>`, `scheduled #48 · 1 of 6 · waited 4m for answer-line to
/// finish`, `scheduled #48 · ran now, by you · <first words>`. None: not
/// a wake.
pub fn run_line(text: &str) -> Option<Scheduled> {
    // the one parser of a wake (lines.rs)
    let (id, how, words) = super::lines::timer_wake(text)?;
    let mut head = format!("scheduled #{id}");
    let mut waited = None;
    let mut now = false;
    for part in how.split(", ") {
        if let Some((a, b)) = part.split_once('/').filter(|(a, b)| a.parse::<u64>().is_ok() && b.parse::<u64>().is_ok()) {
            head.push_str(&format!(" · {a} of {b}"));
        } else if part.starts_with("waited ") {
            waited = Some(part.to_string());
        } else if part == "run now by the user" {
            now = true;
        }
    }
    if now {
        head.push_str(" · ran now, by you");
    }
    match waited {
        Some(w) => head.push_str(&format!(" · {w}")),
        None => head.push_str(&format!(" · {}", clip(words, 48))),
    }
    Some(Scheduled { id, head, words: words.trim().to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: u64 = 1_790_000_000_000;
    const UTC: Offset = &|_| 0;

    fn hl(raw: &str) -> Option<(String, String)> {
        hub_line(raw, NOW, UTC).map(|l| (l.head, l.words))
    }

    /// site/m/timers 'v1 and the words': a run, a run that waited, a
    /// run now; the agent's stop hint never shows; not a wake: None.
    #[test]
    fn a_run_reads_as_its_scheduled_line() {
        let l = run_line("timer #48 (every 2m, 2/6, set by answer-line): check the build and tell me what failed\n(stop it: sb every --stop 48)").unwrap();
        assert_eq!((l.id, l.head.as_str()), (48, "scheduled #48 · 2 of 6 · check the build and tell me what failed"));
        assert_eq!(l.words, "check the build and tell me what failed");
        let l = run_line("timer #48 (every 2m, 1/6, waited 4m for answer-line to finish, set by answer-line): x\n(stop it: sb every --stop 48)").unwrap();
        assert_eq!(l.head, "scheduled #48 · 1 of 6 · waited 4m for answer-line to finish");
        let l = run_line("timer #51 (every day 07:30, run now by the user, set by main): ship\n(stop it: sb every --stop 51)").unwrap();
        assert_eq!(l.head, "scheduled #51 · ran now, by you · ship");
        assert!(run_line("timer set: #1 @main every 10m").is_none());
        assert!(run_line("hello").is_none());
    }

    /// The hub's set and ended lines; main's copy of another agent's
    /// ended one names it.
    #[test]
    fn set_and_ended_lines() {
        let t = json!({"id": 48, "agent": "answer-line", "by": "answer-line", "label": "every 2m", "text": "check the build",
                       "next_ms": NOW + 120_000, "fired": 0, "times": 6});
        let mut set = t.clone();
        set["ev"] = json!("set");
        let (h, w) = hl(&set.to_string()).unwrap();
        assert!(h.starts_with("answer-line scheduled #48 · every 2m · 6 times · next "), "{h}");
        assert_eq!(w, "check the build");
        assert_eq!(hub_line(&set.to_string(), NOW, UTC).unwrap().id, 48);
        let mut by_main = set.clone();
        by_main["by"] = json!("main");
        by_main["times"] = json!(null);
        let (h, _) = hl(&by_main.to_string()).unwrap();
        assert!(h.starts_with("main scheduled #48 for answer-line · every 2m · next "), "{h}");
        for (end, by, words) in [
            ("times", "", "ran its 6 times"),
            ("stopped", "user", "stopped by you"),
            ("stopped", "answer-line", "stopped by answer-line"),
            ("until", "", "its end time passed"),
            ("gone", "", "answer-line is gone"),
        ] {
            let mut e = t.clone();
            e["ev"] = json!("end");
            e["ended_ms"] = json!(NOW);
            e["end"] = json!(end);
            e["stopped_by"] = json!(by);
            assert_eq!(hl(&e.to_string()), Some((format!("scheduled #48 ended · {words}"), String::new())));
        }
        let mut e = t.clone();
        e["ev"] = json!("end");
        e["end"] = json!("times");
        e["in"] = json!("main");
        assert_eq!(hl(&e.to_string()).unwrap().0, "scheduled #48 ended · ran its 6 times (answer-line · check the build)");
        assert!(hl("{}").is_none());
    }

    /// `now` and the offset are the caller's: the same line read at the
    /// same moment says the same words (a replay never moves them).
    #[test]
    fn the_times_are_the_callers() {
        let set = json!({"ev": "set", "id": 1, "agent": "a", "by": "a", "label": "every day 07:30", "text": "x", "next_ms": NOW + 3_600_000});
        let paris: Offset = &|_| 7200;
        let at = |off| hub_line(&set.to_string(), NOW, off).unwrap().head;
        assert_eq!(at(UTC), at(UTC));
        assert_ne!(at(UTC), at(paris), "the offset moves the clock time");
    }

    #[test]
    fn counts_and_short_forms() {
        let mut t = Task { label: "every day 07:30".into(), ..Default::default() };
        assert_eq!((t.when(), t.when_short()), ("every day 07:30".into(), "daily 07:30".into()));
        t.label = "every 15m".into();
        assert_eq!(t.when_short(), "15m");
        t.times = Some(1);
        assert_eq!((t.when(), t.when_short(), t.so_far(NOW, UTC)), ("once".into(), "once".into(), "0 of 1".into()));
    }
}
