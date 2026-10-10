//! The one line an agent's computer use shows under its own head (S29/L19,
//! architect m_16121: one owner of the words, designer m_16125): the TUI's
//! status row and the desktop window's live mark both draw [`line`], so
//! the two never say it differently. The agent's name is not in it (it
//! sits under that agent's head); `drivers_line` in the TUI names agents.

use crate::state::Live;

/// How the surface colours the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// it drives an app now (the working colour, like `∿`)
    Driving,
    /// the user took the wheel: it waits on him (accent)
    Paused,
    /// it was stopped and stays stopped until he writes to it (dim)
    Stopped,
}

/// `mark` then `words`, one space between (`text`): a surface may colour
/// the mark apart (the TUI's `?` in accent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: Kind,
    pub mark: &'static str,
    pub words: String,
}

impl Line {
    pub fn text(&self) -> String {
        format!("{} {}", self.mark, self.words)
    }
}

/// The agent's line, None when it holds nothing. `age`: how long it has
/// driven, in the surface's own form (`2m` in the TUI, `2 min` in the
/// window; None leaves it out). `ascii`: `C` for `↖`, `-` for `·`, `enter`
/// for `⏎` (designer m_3551).
///
/// - driving: `↖ driving Chrome · amazon.fr · 2m` (the place is the tab's
///   host; a Mac app's place is its own name, said once: `↖ driving Notes
///   · 2m`)
/// - paused: `? you took the wheel · ⏎ give it back`
/// - stopped: `↖ you stopped it driving Chrome · write to it to go on`
///   (someone else: `↖ stopped driving Chrome · …`)
pub fn line(a: &Live, age: Option<&str>, ascii: bool) -> Option<Line> {
    let mark = if ascii { "C" } else { "↖" };
    let dot = if ascii { "-" } else { "·" };
    if a.stopped {
        let who = if a.stopped_by.as_deref() == Some("you") { "you stopped it" } else { "stopped" };
        let what = a.was.as_deref().map(|w| format!(" driving {w}")).unwrap_or_default();
        return Some(Line { kind: Kind::Stopped, mark, words: format!("{who}{what} {dot} write to it to go on") });
    }
    if a.paused {
        let enter = if ascii { "enter" } else { "⏎" };
        return Some(Line { kind: Kind::Paused, mark: "?", words: format!("you took the wheel {dot} {enter} give it back") });
    }
    let app = a.driving.as_deref()?;
    let mut words = format!("driving {app}");
    if let Some(p) = a.place.as_deref().filter(|p| *p != app) {
        words.push_str(&format!(" {dot} {p}"));
    }
    if let Some(t) = age {
        words.push_str(&format!(" {dot} {t}"));
    }
    Some(Line { kind: Kind::Driving, mark, words })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live() -> Live {
        Live { key: "00000000000000aa.perf".into(), hub: Some("00000000000000aa".into()), name: "perf".into(), ..Live::default() }
    }

    #[test]
    fn the_three_states_in_designers_words() {
        let drives = Live { driving: Some("Chrome".into()), place: Some("amazon.fr".into()), since_ms: Some(5), ..live() };
        assert_eq!(line(&drives, Some("2m"), false).unwrap().text(), "↖ driving Chrome · amazon.fr · 2m");
        assert_eq!(line(&drives, None, false).unwrap().text(), "↖ driving Chrome · amazon.fr");
        assert_eq!(line(&drives, Some("2m"), true).unwrap().text(), "C driving Chrome - amazon.fr - 2m");
        assert_eq!(line(&drives, None, false).unwrap().kind, Kind::Driving);
        let app = Live { driving: Some("Notes".into()), place: Some("Notes".into()), ..live() };
        assert_eq!(line(&app, Some("2m"), false).unwrap().text(), "↖ driving Notes · 2m");
        let nowhere = Live { driving: Some("Chrome".into()), ..live() };
        assert_eq!(line(&nowhere, Some("2m"), false).unwrap().text(), "↖ driving Chrome · 2m");

        let paused = Live { paused: true, driving: Some("Chrome".into()), ..live() };
        let p = line(&paused, Some("2m"), false).unwrap();
        assert_eq!((p.kind, p.mark, p.text()), (Kind::Paused, "?", "? you took the wheel · ⏎ give it back".to_string()));
        assert_eq!(line(&paused, None, true).unwrap().text(), "? you took the wheel - enter give it back");

        let mine = Live { stopped: true, paused: true, was: Some("Chrome".into()), stopped_by: Some("you".into()), ..live() };
        let s = line(&mine, Some("2m"), false).unwrap();
        assert_eq!((s.kind, s.text()), (Kind::Stopped, "↖ you stopped it driving Chrome · write to it to go on".to_string()));
        let other = Live { stopped: true, was: Some("Chrome".into()), stopped_by: Some("group_closed".into()), ..live() };
        assert_eq!(line(&other, None, false).unwrap().text(), "↖ stopped driving Chrome · write to it to go on");
        let bare = Live { stopped: true, ..live() };
        assert_eq!(line(&bare, None, false).unwrap().text(), "↖ stopped · write to it to go on");

        assert_eq!(line(&live(), Some("2m"), false), None);
    }
}
