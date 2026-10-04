//! The terminal's tab title while bise runs (term-title, designer's
//! pick): what waits for you, then the project, e.g. `#2 ↗1 ●3 · harness`.
//! `#N` the inbox's cards, `↗N` the new artifacts, `●N` the agents at
//! work (working, or waiting on another agent; main left out), then the
//! repo's folder name. A count at 0 is left out; all at 0: `harness`.
//! The counts come first: a narrow tab cuts the end, and the folder is
//! the part you can guess. `●` and not the TUI's `∿`: the tab's system
//! font draws `∿` as a tick. `BISE_ASCII=1`: the TUI's own ASCII forms
//! (`*` for `●`, `.` for `·`), `+` for `↗` (it has none): `#2 +1 *3 . harness`.
//!
//! Written with OSC 0 (the tab and the window title: iTerm's tab reads
//! the icon name) when the text holds still for [`DEBOUNCE`]; the title
//! you had is pushed first (`CSI 22;0 t`) and popped back on exit
//! (`CSI 23;0 t`, crash::restore_terminal), after a reset to an empty
//! title for the terminals without the title stack (Terminal.app: its
//! default title again). Off with `BISE_TERM_TITLE=0`, and never when
//! stdout is not a terminal.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How long a new title must hold still before it is written: a burst
/// (the hub's replay, an agent starting) writes once.
pub(crate) const DEBOUNCE: Duration = Duration::from_millis(300);

/// The folder name is cut at this many characters (with `…`).
const REPO_MAX: usize = 32;

/// What the title says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Status {
    /// The workspace's folder name ("" before the hub said it).
    pub(crate) repo: String,
    /// The inbox's cards.
    pub(crate) inbox: usize,
    /// The artifacts added since you last looked.
    pub(crate) new: u64,
    /// The agents at work (working or waiting), main left out.
    pub(crate) running: usize,
}

/// The title's text; "" when there is nothing to say yet.
pub(crate) fn text(s: &Status, ascii: bool) -> String {
    let (artifact, agent, dot) = if ascii { ("+", "*", ".") } else { ("↗", "●", "·") };
    let mut parts: Vec<String> = Vec::new();
    if s.inbox > 0 {
        parts.push(format!("#{}", s.inbox));
    }
    if s.new > 0 {
        parts.push(format!("{artifact}{}", s.new));
    }
    if s.running > 0 {
        parts.push(format!("{agent}{}", s.running));
    }
    let repo = clean(&s.repo);
    match (parts.is_empty(), repo.is_empty()) {
        (true, _) => repo,
        (false, true) => parts.join(" "),
        (false, false) => format!("{} {dot} {repo}", parts.join(" ")),
    }
}

/// The folder name as a title can carry it: no control characters (an
/// escape would end the OSC), cut at [`REPO_MAX`].
fn clean(repo: &str) -> String {
    let s: String = repo.chars().filter(|c| !c.is_control()).collect();
    let s = s.trim();
    if s.chars().count() > REPO_MAX {
        let cut: String = s.chars().take(REPO_MAX - 1).collect();
        format!("{cut}…")
    } else {
        s.to_string()
    }
}

/// The folder name of a workspace path.
pub(crate) fn repo_name(workspace: &str) -> String {
    std::path::Path::new(workspace.trim_end_matches('/'))
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The debounce: what is on the tab, and the text waiting to be.
#[derive(Default)]
pub(crate) struct Title {
    shown: Option<String>,
    pending: Option<(String, Instant)>,
}

impl Title {
    /// The text to write now, if any: `want` once it held still for
    /// [`DEBOUNCE`] and differs from what is shown. An empty `want` is
    /// never written (the hub has not said the workspace yet).
    pub(crate) fn next(&mut self, want: String, now: Instant) -> Option<String> {
        if want.is_empty() || self.shown.as_deref() == Some(want.as_str()) {
            self.pending = None;
            return None;
        }
        match &self.pending {
            Some((p, since)) if *p == want => {
                if now.duration_since(*since) < DEBOUNCE {
                    return None;
                }
            }
            _ => {
                self.pending = Some((want, now));
                return None;
            }
        }
        self.pending = None;
        self.shown = Some(want.clone());
        Some(want)
    }
}

/// The bytes that set the title: OSC 0, BEL-ended; the first write
/// pushes the title you had.
pub(crate) fn set_bytes(text: &str, push: bool) -> String {
    let push = if push { "\x1b[22;0t" } else { "" };
    format!("{push}\x1b]0;{text}\x07")
}

/// The bytes that give the title back: an empty title (the terminals
/// without the title stack show their own again), then the pop.
pub(crate) const RESTORE: &str = "\x1b]0;\x07\x1b[23;0t";

/// The title was pushed: the exit pops it (any thread, the crash hook too).
static PUSHED: AtomicBool = AtomicBool::new(false);

/// `BISE_TERM_TITLE=0` (or `off`, `false`) turns the title off.
fn switched_off(v: Option<&str>) -> bool {
    v.is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "off" | "false" | "no"))
}

fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        !switched_off(std::env::var("BISE_TERM_TITLE").ok().as_deref()) && std::io::stdout().is_terminal()
    })
}

thread_local! {
    static STATE: std::cell::RefCell<Title> = std::cell::RefCell::new(Title::default());
}

/// Once a loop turn (the UI thread): the title follows `s`.
pub(crate) fn tick(s: &Status, now: Instant) {
    if !enabled() {
        return;
    }
    let want = text(s, crate::theme::ascii_mode());
    let Some(t) = STATE.with(|st| st.borrow_mut().next(want, now)) else {
        return;
    };
    let push = !PUSHED.swap(true, Ordering::SeqCst);
    let mut out = std::io::stdout();
    let _ = out.write_all(set_bytes(&t, push).as_bytes());
    let _ = out.flush();
}

/// Gives the terminal its title back (crash::restore_terminal). Does
/// nothing when bise never set one; safe to call twice.
pub(crate) fn restore() {
    if !PUSHED.swap(false, Ordering::SeqCst) {
        return;
    }
    STATE.with(|st| *st.borrow_mut() = Title::default());
    let mut out = std::io::stdout();
    let _ = out.write_all(RESTORE.as_bytes());
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(repo: &str, inbox: usize, new: u64, running: usize) -> Status {
        Status { repo: repo.into(), inbox, new, running }
    }

    #[test]
    fn the_counts_come_first_then_the_folder() {
        assert_eq!(text(&st("harness", 2, 1, 3), false), "#2 ↗1 ●3 · harness");
        assert_eq!(text(&st("harness", 12, 140, 9), false), "#12 ↗140 ●9 · harness");
    }

    #[test]
    fn a_count_at_zero_is_left_out() {
        assert_eq!(text(&st("harness", 0, 0, 0), false), "harness");
        assert_eq!(text(&st("harness", 2, 0, 0), false), "#2 · harness");
        assert_eq!(text(&st("harness", 0, 1, 0), false), "↗1 · harness");
        assert_eq!(text(&st("harness", 0, 0, 3), false), "●3 · harness");
        assert_eq!(text(&st("harness", 2, 0, 3), false), "#2 ●3 · harness");
        assert_eq!(text(&st("harness", 0, 1, 3), false), "↗1 ●3 · harness");
    }

    #[test]
    fn ascii_takes_the_tuis_forms() {
        assert_eq!(text(&st("harness", 2, 1, 3), true), "#2 +1 *3 . harness");
        assert_eq!(text(&st("harness", 0, 0, 0), true), "harness");
        assert!(text(&st("harness", 2, 1, 3), true).is_ascii());
    }

    #[test]
    fn no_folder_yet() {
        assert_eq!(text(&st("", 0, 0, 0), false), "");
        assert_eq!(text(&st("", 1, 0, 2), false), "#1 ●2");
    }

    #[test]
    fn the_folder_is_cleaned_and_cut() {
        assert_eq!(text(&st("evil\x1b]0;x\x07name", 0, 0, 0), false), "evil]0;xname");
        let long = "a".repeat(50);
        let t = text(&st(&long, 1, 0, 0), false);
        assert_eq!(t, format!("#1 · {}…", "a".repeat(REPO_MAX - 1)));
        assert_eq!(text(&st("café-ü", 0, 0, 0), false), "café-ü");
    }

    #[test]
    fn repo_name_is_the_last_folder() {
        assert_eq!(repo_name("/Users/me/lab/bend-lab/harness"), "harness");
        assert_eq!(repo_name("/Users/me/lab/bend-lab/harness/"), "harness");
        assert_eq!(repo_name(""), "");
    }

    #[test]
    fn a_title_is_written_once_it_holds_still() {
        let t0 = Instant::now();
        let mut t = Title::default();
        assert_eq!(t.next("harness".into(), t0), None);
        assert_eq!(t.next("harness".into(), t0 + Duration::from_millis(100)), None);
        assert_eq!(t.next("harness".into(), t0 + DEBOUNCE), Some("harness".into()));
        // shown: nothing more to write
        assert_eq!(t.next("harness".into(), t0 + DEBOUNCE * 3), None);
    }

    #[test]
    fn a_burst_writes_only_its_last_text() {
        let t0 = Instant::now();
        let mut t = Title::default();
        let ms = |n| t0 + Duration::from_millis(n);
        assert_eq!(t.next("●1 · harness".into(), ms(0)), None);
        assert_eq!(t.next("●2 · harness".into(), ms(100)), None);
        assert_eq!(t.next("●3 · harness".into(), ms(200)), None);
        assert_eq!(t.next("●3 · harness".into(), ms(400)), None);
        assert_eq!(t.next("●3 · harness".into(), ms(500)), Some("●3 · harness".into()));
    }

    #[test]
    fn going_back_to_the_shown_text_cancels_the_change() {
        let t0 = Instant::now();
        let mut t = Title::default();
        t.next("harness".into(), t0);
        assert_eq!(t.next("harness".into(), t0 + DEBOUNCE), Some("harness".into()));
        assert_eq!(t.next("●1 · harness".into(), t0 + DEBOUNCE * 2), None);
        assert_eq!(t.next("harness".into(), t0 + DEBOUNCE * 3), None);
        assert_eq!(t.next("harness".into(), t0 + DEBOUNCE * 9), None);
    }

    #[test]
    fn an_empty_text_is_never_written() {
        let t0 = Instant::now();
        let mut t = Title::default();
        assert_eq!(t.next(String::new(), t0), None);
        assert_eq!(t.next(String::new(), t0 + DEBOUNCE * 2), None);
    }

    #[test]
    fn the_bytes_push_once_and_restore_pops() {
        assert_eq!(set_bytes("harness", true), "\x1b[22;0t\x1b]0;harness\x07");
        assert_eq!(set_bytes("#1 · harness", false), "\x1b]0;#1 · harness\x07");
        assert!(RESTORE.ends_with("\x1b[23;0t"));
        assert!(RESTORE.starts_with("\x1b]0;\x07"));
    }

    #[test]
    fn the_switch() {
        for v in ["0", "off", "false", "OFF", " 0 ", "no"] {
            assert!(switched_off(Some(v)), "{v}");
        }
        for v in [None, Some("1"), Some(""), Some("on")] {
            assert!(!switched_off(v), "{v:?}");
        }
    }
}
