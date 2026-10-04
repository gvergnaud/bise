//! Scheduled tasks (site/m/timers): what `sb every` sets, as the user
//! reads it. The hub's state carries them (`timers`: the active ones,
//! then a week of ended ones); the feeds carry their ◷ lines (the hub's
//! `scheduled : <json>` when one is set or ends, and each run, a
//! `msg-in` from bise whose text is the wake the agent reads). The words
//! are designer's (site/m/timers 'v1 and the words'): "scheduled task"
//! for one, "scheduled" for the lines and the screen, "run" for a wake.
//! The full screen is `scheduled_screen.rs`.

use crate::wire::Ev;
use serde_json::Value;

/// One scheduled task, from the hub's state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Task {
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
    pub(crate) fn of(v: &Value) -> Option<Task> {
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

    pub(crate) fn active(&self) -> bool {
        self.ended_ms.is_none()
    }

    /// How often, in a line: `every 2m`, `every day 07:30`, `once`.
    pub(crate) fn when(&self) -> String {
        if self.times == Some(1) {
            "once".into()
        } else {
            self.label.clone()
        }
    }

    /// How often, short (the 80-column list): `2m`, `daily 07:30`, `once`.
    pub(crate) fn when_short(&self) -> String {
        if self.times == Some(1) {
            return "once".into();
        }
        match self.label.strip_prefix("every day ") {
            Some(hm) => format!("daily {hm}"),
            None => self.label.trim_start_matches("every ").to_string(),
        }
    }

    /// How many so far, or until when: `2 of 6`, `until 18:00`, or "".
    pub(crate) fn so_far(&self, now: u64) -> String {
        match (self.times, self.until_ms) {
            (Some(n), _) => format!("{} of {}", self.fired, n),
            (None, Some(u)) => format!("until {}", ahead(u, now)),
            _ => String::new(),
        }
    }

    /// Why it ended: `ran its 6 times`, `stopped by you`, `stopped by
    /// answer-line`, `its end time passed`, `answer-line is gone`.
    pub(crate) fn ended_words(&self) -> String {
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

/// The state's `timers` (none from an older hub).
pub(crate) fn from_state(v: &Value) -> Vec<Task> {
    v["timers"].as_array().map(|a| a.iter().filter_map(Task::of).collect()).unwrap_or_default()
}

/// A time to come on the real clock: `14:22`, `tomorrow 07:30`.
pub(crate) fn ahead(ms: u64, now: u64) -> String {
    crate::when::ahead(ms, crate::when::offset_at(ms), now, crate::when::offset_at(now))
}

/// `in 1m`, `in 1h 5m`, `in 2d` (`now` when due).
pub(crate) fn countdown(ms: u64, now: u64) -> String {
    let s = ms.saturating_sub(now) / 1000;
    let (d, h, m) = (s / 86_400, s % 86_400 / 3600, s % 3600 / 60);
    match (d, h, m) {
        (0, 0, 0) => "now".into(),
        (0, 0, m) => format!("in {m}m"),
        (0, h, 0) => format!("in {h}h"),
        (0, h, m) => format!("in {h}h {m}m"),
        (d, _, _) => format!("in {d}d"),
    }
}

/// `s` on one line, cut at `n` chars with `…`.
pub(crate) fn clip(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= n {
        one
    } else {
        format!("{}…", one.chars().take(n.saturating_sub(1)).collect::<String>())
    }
}

/// The hub's `scheduled : <json>` line: one set (`ev: set`) or ended
/// (`ev: end`); main's copy of another agent's (`in: main`) names it.
pub(crate) fn hub_line(raw: &str) -> Option<Ev> {
    hub_line_at(raw, crate::when::now_ms())
}

pub(crate) fn hub_line_at(raw: &str, now: u64) -> Option<Ev> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let t = Task::of(&v)?;
    let head = match v["ev"].as_str()? {
        "set" => {
            let who = if t.by == t.agent || t.by.is_empty() {
                format!("{} scheduled #{}", t.agent, t.id)
            } else {
                format!("{} scheduled #{} for {}", t.by, t.id, t.agent)
            };
            let mut h = format!("{who} · {}", t.when());
            match (t.times, t.until_ms) {
                (Some(n), _) if n > 1 => h.push_str(&format!(" · {n} times")),
                (None, Some(u)) => h.push_str(&format!(" · until {}", ahead(u, now))),
                _ => {}
            }
            h.push_str(&format!(" · next {}", ahead(t.next_ms, now)));
            return Some(Ev::Scheduled { head: h, words: t.text, open: false });
        }
        "end" => {
            let mut h = format!("scheduled #{} ended · {}", t.id, t.ended_words());
            if v["in"].as_str() == Some("main") && t.agent != "main" {
                h.push_str(&format!(" ({} · {})", t.agent, clip(&t.text, 32)));
            }
            h
        }
        _ => return None,
    };
    Some(Ev::Scheduled { head, words: String::new(), open: false })
}

/// The hub's own sender (its id `switchboard`, or `bise` as shown).
pub(crate) fn is_hub(from: &str) -> bool {
    from == "switchboard" || from == "bise"
}

/// A run: the wake an agent reads from bise (every.rs `wake_text`,
/// `timer #48 (every 2m, 2/6, set by answer-line): <words>` then the
/// stop hint) as its ◷ line: `scheduled #48 · 2 of 6 · <first words> ▸`,
/// `scheduled #48 · 1 of 6 · waited 4m for answer-line to finish ▸`,
/// `scheduled #48 · run now by you · <first words> ▸`. None: not a wake.
pub(crate) fn run_line(text: &str) -> Option<Ev> {
    let rest = text.strip_prefix("timer #")?;
    let (id, rest) = rest.split_once(" (")?;
    let id: u64 = id.parse().ok()?;
    let (how, words) = rest.split_once("): ")?;
    let words = match words.rsplit_once("\n(stop it: sb every --stop ") {
        Some((w, _)) => w,
        None => words,
    };
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
        head.push_str(" · run now by you");
    }
    match waited {
        Some(w) => head.push_str(&format!(" · {w}")),
        None => head.push_str(&format!(" · {}", clip(words, 48))),
    }
    Some(Ev::Scheduled { head, words: words.trim().to_string(), open: false })
}

/// The note bise sends an agent when you stop its scheduled task: for
/// the agent only (its ◷ ended line says it in the thread).
pub(crate) fn is_stop_note(text: &str) -> bool {
    text.starts_with("the user stopped timer #")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: u64 = 1_790_000_000_000;

    fn head(ev: Option<Ev>) -> (String, String) {
        match ev {
            Some(Ev::Scheduled { head, words, .. }) => (head, words),
            _ => panic!("not a ◷ line"),
        }
    }

    /// site/m/timers 'v1 and the words': a run, a run that waited, a
    /// run now; the agent's stop hint never shows; not a wake: None.
    #[test]
    fn a_run_reads_as_its_scheduled_line() {
        let (h, w) = head(run_line("timer #48 (every 2m, 2/6, set by answer-line): check the build and tell me what failed\n(stop it: sb every --stop 48)"));
        assert_eq!(h, "scheduled #48 · 2 of 6 · check the build and tell me what failed");
        assert_eq!(w, "check the build and tell me what failed");
        let (h, _) = head(run_line("timer #48 (every 2m, 1/6, waited 4m for answer-line to finish, set by answer-line): x\n(stop it: sb every --stop 48)"));
        assert_eq!(h, "scheduled #48 · 1 of 6 · waited 4m for answer-line to finish");
        let (h, _) = head(run_line("timer #51 (every day 07:30, run now by the user, set by main): ship\n(stop it: sb every --stop 51)"));
        assert_eq!(h, "scheduled #51 · run now by you · ship");
        assert!(run_line("timer set: #1 @main every 10m").is_none());
        assert!(run_line("hello").is_none());
        assert!(is_stop_note("the user stopped timer #3 (x): don't set it again unless they ask"));
    }

    /// The hub's set and ended lines; main's copy of another agent's
    /// ended one names it.
    #[test]
    fn set_and_ended_lines() {
        let t = json!({"id": 48, "agent": "answer-line", "by": "answer-line", "label": "every 2m", "text": "check the build",
                       "next_ms": NOW + 120_000, "fired": 0, "times": 6});
        let mut set = t.clone();
        set["ev"] = json!("set");
        let (h, w) = head(hub_line_at(&set.to_string(), NOW));
        assert!(h.starts_with("answer-line scheduled #48 · every 2m · 6 times · next "), "{h}");
        assert_eq!(w, "check the build");
        let mut by_main = set.clone();
        by_main["by"] = json!("main");
        by_main["times"] = json!(null);
        let (h, _) = head(hub_line_at(&by_main.to_string(), NOW));
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
            assert_eq!(head(hub_line_at(&e.to_string(), NOW)), (format!("scheduled #48 ended · {words}"), String::new()));
        }
        let mut e = t.clone();
        e["ev"] = json!("end");
        e["end"] = json!("times");
        e["in"] = json!("main");
        assert_eq!(head(hub_line_at(&e.to_string(), NOW)).0, "scheduled #48 ended · ran its 6 times (answer-line · check the build)");
        assert!(hub_line_at("{}", NOW).is_none());
    }

    #[test]
    fn counts_and_short_forms() {
        let mut t = Task { label: "every day 07:30".into(), ..Default::default() };
        assert_eq!((t.when(), t.when_short()), ("every day 07:30".into(), "daily 07:30".into()));
        t.label = "every 15m".into();
        assert_eq!(t.when_short(), "15m");
        t.times = Some(1);
        assert_eq!((t.when(), t.when_short(), t.so_far(NOW)), ("once".into(), "once".into(), "0 of 1".into()));
        assert_eq!(countdown(NOW + 61_000, NOW), "in 1m");
        assert_eq!(countdown(NOW + 3_900_000, NOW), "in 1h 5m");
        assert_eq!(countdown(NOW, NOW), "now");
    }
}
