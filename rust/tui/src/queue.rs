//! Queued messages (BISE-89, book §13, after Codex's pending input
//! preview). During a turn, `tab` keeps the composer text for after the
//! turn: it stays here, nothing goes to the hub until it leaves the
//! queue. The queue shows above the composer; `↑` in an empty composer
//! pops the newest back to edit. When the turn ends, the oldest goes out
//! as a normal message (it starts the next turn; the next one waits for
//! that turn to end). One queue per feed (the `App` fields are swapped
//! with the view); it is saved with the drafts (sb/drafts.rs), so a
//! reload or a restart of the TUI keeps it (BISE-131).

use crate::app::App;
use crate::attach::Attachment;
use crate::theme::{dim, faint, glyph, G_YOU};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

/// The faint line under the queued messages (book §17).
pub(crate) const HINT: &str = "queued · sent when this turn ends · ↑ edit";
/// At most this many queued lines show; the older ones are counted.
const SHOWN: usize = 5;

/// One queued message: the composer text as typed (its image labels)
/// and the images it carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Queued {
    pub(crate) text: String,
    pub(crate) attachments: Vec<Attachment>,
}

/// `tab` during a turn: the composer's text (and its images) goes to the
/// queue, the composer empties. False when there is nothing to queue.
pub(crate) fn push(app: &mut App) -> bool {
    let text = app.ed.text.trim().to_string();
    if text.is_empty() || text.starts_with('/') {
        return false;
    }
    app.ed.take();
    let attachments = std::mem::take(&mut app.attachments);
    app.queued.push(Queued { text, attachments });
    true
}

/// `↑` in an empty composer: the newest queued message back in the
/// composer, to edit (`tab` queues it again, `⏎` steers it now).
pub(crate) fn pop_last(app: &mut App) -> bool {
    if !app.ed.text.is_empty() {
        return false;
    }
    let Some(q) = app.queued.pop() else { return false };
    app.ed.insert(&q.text);
    app.attachments = q.attachments;
    true
}

/// How long a queued message that went may wait for its turn to start
/// before the next one may go anyway: the mark never stalls the queue.
pub(crate) const TURN_WAIT: std::time::Duration = std::time::Duration::from_secs(15);

/// The turn ended: the oldest queued message, ready to send (its image
/// labels expanded). None during a turn, with nothing queued, or while
/// the one sent before has not started its turn yet: an idle state the
/// hub sent before it took that input must not release the next one
/// (it went in the same turn, as steering).
pub(crate) fn next(app: &mut App) -> Option<String> {
    next_at(app, std::time::Instant::now())
}

pub(crate) fn next_at(app: &mut App, now: std::time::Instant) -> Option<String> {
    if app.pending || app.queued.is_empty() || waits(app, now) {
        return None;
    }
    let q = app.queued.remove(0);
    // expand with its own images; the composer keeps its own
    let mine = std::mem::replace(&mut app.attachments, q.attachments);
    let text = crate::attach::expand(app, &q.text);
    app.attachments = mine;
    app.queue_out = Some(now);
    Some(text)
}

/// The queued message that went has its turn (it started or ended), or
/// the hub refused it: the next one goes at that turn's end.
pub(crate) fn seen(app: &mut App) {
    app.queue_out = None;
}

/// A queued message went and its turn has not started; past
/// [`TURN_WAIT`] the mark goes (one log line) and the queue moves on.
fn waits(app: &mut App, now: std::time::Instant) -> bool {
    let Some(t) = app.queue_out else { return false };
    if now.saturating_duration_since(t) < TURN_WAIT {
        return true;
    }
    app.queue_out = None;
    log_line(&format!("queue: no turn started {} s after a queued message: the next one may go", TURN_WAIT.as_secs()));
    false
}

/// One line in `<bise home>/logs/tui.log` (none from the unit tests: they
/// never write the real home).
#[cfg(not(test))]
fn log_line(line: &str) {
    use std::io::Write;
    let dir = bise_home::Home::from_env().logs_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("tui.log")) {
        let _ = writeln!(f, "{} {}", crate::when::now_ms(), line);
    }
}

#[cfg(test)]
fn log_line(_line: &str) {}

/// Rows the queue takes above the composer (0 when empty).
pub(crate) fn height(app: &App) -> u16 {
    match app.queued.len() {
        0 => 0,
        n => (n.min(SHOWN) + usize::from(n > SHOWN) + 1) as u16,
    }
}

/// The queue at `width` columns: ` › text…` dim per message (one line,
/// cut), the newest last, then the faint hint.
pub(crate) fn lines(app: &App, width: usize) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let n = app.queued.len();
    if n == 0 {
        return Vec::new();
    }
    let d = Style::default().fg(dim());
    let mut out = Vec::new();
    if n > SHOWN {
        out.push(Line::from(Span::styled(format!("   + {} more", n - SHOWN), d)));
    }
    let lead = format!(" {} ", glyph(G_YOU));
    let room = width.saturating_sub(lead.width()).max(1);
    for q in &app.queued[n.saturating_sub(SHOWN)..] {
        let flat = bend_images::display(&q.text).split_whitespace().collect::<Vec<_>>().join(" ");
        out.push(Line::from(vec![Span::styled(lead.clone(), d), Span::styled(cut(&flat, room), d)]));
    }
    out.push(Line::from(Span::styled(format!("   {}", HINT), Style::default().fg(faint()))));
    out
}

/// `s` in `room` columns, `…` when cut.
fn cut(s: &str, room: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut w = 0;
    let mut out = String::new();
    let total: usize = s.chars().map(|c| c.width().unwrap_or(0)).sum();
    if total <= room {
        return s.to_string();
    }
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > room {
            break;
        }
        w += cw;
        out.push(c);
    }
    out.push('…');
    out
}

/// The turn edges between an agent's row before (`was`: its turn runs,
/// its ended turns) and its row now. Both halves come from one source,
/// sb-core's runtime state in one view (issue 22: `rows::Agent::turn_running`
/// and `turns`), so a turn's end is the count moving with the run no
/// longer busy, never two rows apart. One end per ended turn, a start
/// before it when the row didn't say running (a turn that started and
/// ended between two rows, no entry of it seen, is a start then an end),
/// then a start when it runs now; a flip alone is that edge. A lower
/// count is sb-core restarted (its count is back to 0): a new baseline,
/// nothing drawn. True: a start, false: an end, in order (proto-lead
/// m_14731).
pub(crate) fn turn_edges(was: (bool, u64), now: (bool, u64)) -> Vec<bool> {
    let mut out = Vec::new();
    if now.1 < was.1 {
        return out;
    }
    let mut running = was.0;
    for _ in 0..now.1 - was.1 {
        if !running {
            out.push(true);
        }
        out.push(false);
        running = false;
    }
    match (running, now.0) {
        (false, true) => out.push(true),
        (true, false) => out.push(false),
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(ls: &[Line]) -> Vec<String> {
        ls.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
    }

    #[test]
    fn tab_queues_and_the_arrow_brings_it_back() {
        let mut app = crate::sb::bench::test_app();
        app.pending = true;
        app.ed.insert("also check logout");
        assert!(push(&mut app));
        assert_eq!(app.ed.text, "");
        app.ed.insert("and the cookie headers");
        assert!(push(&mut app));
        // nothing to queue: empty, or a command
        assert!(!push(&mut app));
        app.ed.insert("/help");
        assert!(!push(&mut app));
        app.ed.take();
        assert_eq!(app.queued.len(), 2);
        // ↑ in an empty composer: the newest comes back to edit
        assert!(pop_last(&mut app));
        assert_eq!(app.ed.text, "and the cookie headers");
        assert_eq!(app.queued.len(), 1);
        // not while the composer has text
        assert!(!pop_last(&mut app));
        // tab queues it again, at the end
        assert!(push(&mut app));
        assert_eq!(app.queued.iter().map(|q| q.text.as_str()).collect::<Vec<_>>(), ["also check logout", "and the cookie headers"]);
    }

    #[test]
    fn the_oldest_goes_when_the_turn_ends() {
        let mut app = crate::sb::bench::test_app();
        app.pending = true;
        for t in ["one", "two"] {
            app.ed.insert(t);
            push(&mut app);
        }
        // during the turn: nothing leaves
        assert_eq!(next(&mut app), None);
        app.pending = false;
        assert_eq!(next(&mut app).as_deref(), Some("one"));
        // the next one waits for the turn that one starts
        app.pending = true;
        seen(&mut app);
        assert_eq!(next(&mut app), None);
        app.pending = false;
        assert_eq!(next(&mut app).as_deref(), Some("two"));
        assert_eq!(next(&mut app), None);
    }

    fn two_queued() -> App {
        let mut app = crate::sb::bench::test_app();
        app.pending = true;
        for t in ["one", "two"] {
            app.ed.insert(t);
            push(&mut app);
        }
        app.pending = false;
        app
    }

    /// Law (architect m_12881): an idle state the hub sent before it took
    /// the queued input (the state's main=idle after the send, before
    /// turn_started) does not release the next one: it would go in the
    /// same turn, as steering.
    #[test]
    fn a_stale_idle_after_the_send_does_not_release_the_next_one() {
        let mut app = two_queued();
        let t0 = std::time::Instant::now();
        assert_eq!(next_at(&mut app, t0).as_deref(), Some("one"));
        app.pending = true;
        // the stale idle state
        app.pending = false;
        assert_eq!(next_at(&mut app, t0 + std::time::Duration::from_millis(60)), None);
        assert_eq!(app.queued.len(), 1);
    }

    /// Law: its turn starting clears the mark (the feed's turn_started line).
    #[test]
    fn the_turn_starting_clears_the_mark() {
        let mut app = two_queued();
        assert_eq!(next(&mut app).as_deref(), Some("one"));
        crate::run::ingest_line(&mut app, "  obs: turn_started".into(), None);
        assert_eq!(app.queue_out, None);
        crate::run::ingest_line(&mut app, "--- idle".into(), None);
        assert_eq!(next(&mut app).as_deref(), Some("two"));
    }

    /// Law: its turn ending clears it too (a turn_started line missed).
    #[test]
    fn the_turn_ending_clears_the_mark() {
        let mut app = two_queued();
        assert_eq!(next(&mut app).as_deref(), Some("one"));
        crate::run::ingest_line(&mut app, "  obs: turn_done: completed".into(), None);
        crate::run::ingest_line(&mut app, "--- idle".into(), None);
        assert_eq!(next(&mut app).as_deref(), Some("two"));
    }

    /// Law: a refusal of that input (the hub's notice) clears it: no turn
    /// will start for it.
    #[test]
    fn a_refusal_clears_the_mark() {
        let mut app = two_queued();
        assert_eq!(next(&mut app).as_deref(), Some("one"));
        crate::sb::dispatch(&mut app, r#"{"jsonrpc": "2.0", "method": "hub/notice", "params": {"project": "p", "text": "no agent named x"}}"#);
        assert_eq!(app.queue_out, None);
        assert_eq!(next(&mut app).as_deref(), Some("two"));
    }

    /// Law: the mark never stalls the queue: past TURN_WAIT with no turn,
    /// the next one goes.
    #[test]
    fn the_mark_goes_after_its_bound() {
        let mut app = two_queued();
        let t0 = std::time::Instant::now();
        assert_eq!(next_at(&mut app, t0).as_deref(), Some("one"));
        assert_eq!(next_at(&mut app, t0 + TURN_WAIT - std::time::Duration::from_millis(1)), None);
        assert_eq!(next_at(&mut app, t0 + TURN_WAIT).as_deref(), Some("two"));
    }

    #[test]
    fn the_queue_shows_above_the_composer() {
        let mut app = crate::sb::bench::test_app();
        assert_eq!(height(&app), 0);
        assert!(lines(&app, 40).is_empty());
        app.pending = true;
        for t in ["also check logout", "a much longer message that will not fit in forty columns at all"] {
            app.ed.insert(t);
            push(&mut app);
        }
        let ls = text_of(&lines(&app, 40));
        assert_eq!(height(&app), 3);
        assert_eq!(ls[0], format!(" {} also check logout", G_YOU));
        assert!(ls[1].ends_with('…') && unicode_width::UnicodeWidthStr::width(ls[1].as_str()) <= 40, "{ls:#?}");
        assert_eq!(ls[2], format!("   {}", HINT));
        // many: the newest five, the older ones counted
        for i in 0..6 {
            app.ed.insert(&format!("m{i}"));
            push(&mut app);
        }
        let ls = text_of(&lines(&app, 40));
        assert_eq!(ls[0], "   + 3 more");
        assert_eq!(ls[5], format!(" {} m5", G_YOU));
        assert_eq!(height(&app) as usize, ls.len());
    }

    #[test]
    fn a_queued_image_keeps_its_image() {
        let mut app = crate::sb::bench::test_app();
        app.pending = true;
        let a = Attachment { label: "[Image #1]".into(), marker: "<image name=a.png b64=/x>".into(), info: Default::default() };
        app.attachments = vec![a.clone()];
        app.ed.insert("look [Image #1]");
        push(&mut app);
        assert!(app.attachments.is_empty(), "the composer starts over");
        // the composer's own images stay when the queued one goes
        let mine = Attachment { label: "[Image #1]".into(), marker: "<image other>".into(), info: Default::default() };
        app.attachments = vec![mine.clone()];
        app.pending = false;
        assert_eq!(next(&mut app).as_deref(), Some("look <image name=a.png b64=/x>"));
        assert_eq!(app.attachments, vec![mine]);
    }

    /// The agent's rows in order from its first, through
    /// [`turn_edges`]: every edge drawn.
    fn edges(rows: &[(bool, u64)]) -> Vec<bool> {
        rows.windows(2).flat_map(|w| turn_edges(w[0], w[1])).collect()
    }

    /// Law (proto-lead m_14731): a turn is never missed. A flip alone is
    /// its edge; two quick turns between two rows (no entry of them seen)
    /// are two start/end pairs; an end and a start together come end first.
    #[test]
    fn every_turn_gets_its_edges() {
        assert_eq!(turn_edges((false, 0), (true, 0)), [true]);
        assert_eq!(turn_edges((true, 0), (false, 1)), [false]);
        assert_eq!(turn_edges((false, 3), (false, 5)), [true, false, true, false], "two quick turns");
        assert_eq!(turn_edges((true, 3), (true, 3)), Vec::<bool>::new());
        assert_eq!(edges(&[(false, 3), (true, 3), (true, 4), (true, 5), (false, 5)]), [true, false, true, false, true, false], "two turns ended, each the next one started in its step, then the last flipped");
        assert_eq!(edges(&[(false, 2), (false, 3), (false, 4)]), [true, false, true, false], "a turn with no entry and no working row: a pair each");
    }

    /// Law (issue 22): the row's two halves come from one view, so one
    /// turn's end is one row: running with n, then not running with n+1,
    /// one end; a turn that ends and the next that starts in the same
    /// sb-core step (the idle's pump sends the queued message) is the
    /// count moving while the row stays running: an end, then a start.
    #[test]
    fn a_turns_end_is_one_row() {
        assert_eq!(edges(&[(true, 1), (false, 2)]), [false]);
        assert_eq!(edges(&[(true, 1), (true, 2)]), [false, true], "ended and the next one started in one step");
        assert_eq!(edges(&[(true, 1), (false, 2), (true, 2), (false, 3)]), [false, true, false]);
    }

    /// Law (architect m_16521): sb-core restarted, its counts are back to
    /// 0: a lower count is a new baseline, nothing drawn, and the next
    /// ended turn draws its one end (no start/end burst).
    #[test]
    fn a_lower_count_is_a_new_baseline() {
        assert_eq!(edges(&[(true, 5), (true, 0), (false, 1)]), [false]);
        assert_eq!(turn_edges((false, 5), (false, 0)), Vec::<bool>::new());
        assert_eq!(turn_edges((true, 5), (false, 0)), Vec::<bool>::new());
    }

    /// Law: the race tui_queue_tmux caught, on one-source rows. The sleep
    /// turn ends (queued-one goes and starts its turn in the same step):
    /// queued-two waits for queued-one's own turn to end.
    #[test]
    fn the_next_queued_one_waits_for_the_turn_the_first_one_started() {
        let mut app = two_queued();
        app.pending = true;
        let mut sent = Vec::new();
        let mut row = |app: &mut App, was, now| {
            for started in turn_edges(was, now) {
                seen(app);
                if !started {
                    app.pending = false;
                    if let Some(m) = next(app) {
                        app.pending = true;
                        sent.push(m);
                    }
                }
            }
        };
        row(&mut app, (true, 1), (true, 2));
        assert_eq!(app.queued.len(), 1, "two waits");
        row(&mut app, (true, 2), (true, 2));
        assert_eq!(app.queued.len(), 1, "a row with nothing new: two still waits");
        row(&mut app, (true, 2), (false, 3));
        assert!(app.queued.is_empty());
        assert_eq!(sent, ["one", "two"]);
    }
}
