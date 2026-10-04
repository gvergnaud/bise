//! expired-ux: the ChatGPT sign-in again, from the thread (designer
//! m_7456). The plan's sign-in expires about every hour (OpenAI refuses
//! the refresh, devkit issue #5); the agents it stopped wait on bise's
//! `signin` item (switchboard signin_card.rs).
//!
//! ⏎ on an empty composer in the thread of an agent that waits (or `1`
//! on the item) opens the browser on ChatGPT's sign-in, with the saved
//! client, and stays in the thread: the key bar says
//! `waiting for you to sign in to ChatGPT in your browser…   c copy the
//! link   esc cancel` (`c copied` for 2 s, like the first run). Signed in,
//! nothing more here: the hub reads auth.json, closes the item, and the
//! agents go on. Refused, cancelled or failed: one ▲ line in the thread.

use crate::onboarding::{auth_paths, Flow, Kind, Logins, Poll, DENIED, UNFINISHED};
use crate::wire::Ev;
use crate::App;
use std::time::{Duration, Instant};

/// How long the key bar says `c copied`.
const COPIED_FOR: Duration = Duration::from_secs(2);

/// The waiting words (the first run's, onboarding.rs).
pub(crate) const WAITING: &str = "waiting for you to sign in to ChatGPT in your browser…";

/// A sign-in under way from the thread.
pub(crate) struct ReSignIn {
    flow: Box<dyn Flow>,
    copied: Option<Instant>,
}

impl ReSignIn {
    /// `c` was pressed less than 2 s ago and the link is on the clipboard.
    pub(crate) fn just_copied(&self) -> bool {
        self.copied.is_some_and(|at| at.elapsed() < COPIED_FOR)
    }
}

/// Start it (one at a time): the browser opens on its link. It can't
/// start: why, as a ▲ line.
pub(crate) fn start(app: &mut App) {
    if app.resign.is_some() {
        return;
    }
    let logins = Logins::real();
    match (logins.start)(Kind::ChatGpt, &auth_paths(&bise_home::Home::from_env())) {
        Ok(f) => {
            let _ = (logins.open)(f.url());
            app.resign = Some(ReSignIn { flow: f, copied: None });
        }
        Err(e) => say(app, &format!("▲ {}", e.trim_end_matches('.'))),
    }
}

/// Each loop turn: its answer, when it came.
pub(crate) fn tick(app: &mut App) {
    let Some(r) = app.resign.as_mut() else { return };
    let line = match r.flow.poll() {
        Poll::Waiting => return,
        // the hub sees auth.json: the item closes, the agents go on
        Poll::Done(_) => None,
        Poll::Denied => Some(DENIED.to_string()),
        Poll::Unfinished => Some(UNFINISHED.to_string()),
        Poll::Failed(e) => Some(format!("▲ {}", e.trim_end_matches('.'))),
    };
    app.resign = None;
    if let Some(l) = line {
        say(app, &l);
    }
}

/// Its keys while it waits: `c` (on an empty composer) copies the link,
/// esc cancels. True: the key was taken.
pub(crate) fn key(app: &mut App, k: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::{KeyCode, KeyModifiers};
    let Some(r) = app.resign.as_mut() else { return false };
    match (k.code, k.modifiers) {
        (KeyCode::Esc, _) => {
            r.flow.cancel();
            app.resign = None;
            true
        }
        (KeyCode::Char('c'), KeyModifiers::NONE) if app.ed.text.is_empty() => {
            if crate::clipboard::copy(r.flow.url()) {
                r.copied = Some(Instant::now());
            }
            true
        }
        _ => false,
    }
}

/// A ▲ line in the thread (the designer's lines carry their own ▲).
fn say(app: &mut App, line: &str) {
    let w = line.strip_prefix("▲ ").unwrap_or(line).to_string();
    crate::feed::push_event(&mut app.events, &mut app.cache, Ev::Warn(w));
    app.follow = true;
}
