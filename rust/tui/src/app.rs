//! The client state: `App` (the feed in focus, the composer, the hub
//! connection) and the composer / mouse geometry of the last frame.

use crate::feed::*;
use crate::*;

// ---- app state ----

pub(crate) struct App {
    pub(crate) connected: bool,
    // the embedded terminal (Ctrl+`)
    pub(crate) term: term::Term,
    // the /help or /shortcuts overlay, when open
    pub(crate) help: Option<help::Overlay>,
    // the /approvals screen, when open
    pub(crate) approvals: Option<approvals_screen::Screen>,
    /// `/log`, the raw session of the agent in view, when open
    pub(crate) logview: Option<crate::logview::View>,
    /// `/artifacts`, the full screen of what your agents made, when open
    pub(crate) artifacts: Option<crate::artifacts_screen::Screen>,
    /// `/scheduled`, the full screen of the scheduled tasks, when open
    pub(crate) scheduled: Option<crate::scheduled_screen::Screen>,
    /// the diff panel (ctrl+g, `/diff`, `± 3 files`), when open
    pub(crate) diff: Option<crate::diffview::Panel>,
    // the /computer-use screen, when open (computer_use.rs)
    pub(crate) computer_use: Option<computer_use::Screen>,
    pub(crate) debug: bool,
    // feed scrollback: follow means stick to the bottom (any scroll up
    // turns it off, End/enter turn it back on). A pinned view is anchored
    // on its first row, (event, row in the event): new content never
    // moves it, and a frame only builds the rows it shows (no sum over
    // the whole history). `scroll` is the move asked since the last
    // frame, in rows; the frame applies it.
    pub(crate) follow: bool,
    pub(crate) anchor: (usize, usize),
    pub(crate) scroll: isize,
    // the event shown on each row of the feed by the last frame (clicks)
    pub(crate) vis_events: Vec<usize>,
    /// the row, among its event's rows, each feed row shows
    pub(crate) vis_rows: Vec<usize>,
    /// the screen column of the feed's first text column
    pub(crate) feed_x: u16,
    /// The screen row of the feed's first row (under the Switchboard
    /// header and the "inside an agent" line; 0 without).
    pub(crate) feed_y: u16,
    /// the in-app selection in the feed
    pub(crate) feed_sel: Option<feedsel::FeedSel>,
    /// the "type to ask about it" popup over the selection is up: set when
    /// a drag ends, dropped by a press or a scroll (quote.rs)
    pub(crate) quote_hint: bool,
    /// ctrl+f: the find field, open (find.rs)
    pub(crate) find: Option<crate::find::Find>,
    /// expired-ux: the ChatGPT sign-in again, from the thread (resign.rs)
    pub(crate) resign: Option<crate::resign::ReSignIn>,
    /// cmd+k / ctrl+s: the agent palette, open (sb/palette.rs, BISE-265)
    pub(crate) palette: Option<crate::sb::palette::Palette>,
    /// a cmd key (SUPER, not cmd alone) reached us this session: the
    /// terminal passes cmd keys, the key bar and the help say cmd+f
    pub(crate) cmd_keys: bool,
    /// BISE-302: what the terminal lets through for the inbox
    /// (reach.rs): ctrl+1-9 arrive, a click arrives. Set at start from
    /// the terminal's answers (run.rs); a ctrl+1-9 that only a terminal
    /// with them sends turns `ctrl_digits` on.
    pub(crate) ctrl_digits: bool,
    pub(crate) clicks: bool,
    // activity that arrived while pinned (shown by the back-to-bottom bar)
    pub(crate) unseen: usize,
    pub(crate) tail_visible: bool,
    pub(crate) bottom_bar_rect: Option<ratatui::layout::Rect>,
    /// the first-run text's `show me what you can do`, where it was
    /// drawn: a click fills the composer with it (BISE-284)
    pub(crate) demo_rect: Option<ratatui::layout::Rect>,
    /// this launch plays the first-run onboarding: at its end, main's
    /// composer opens with `show me what you can do` (BISE-284)
    pub(crate) demo_after_onboarding: bool,
    /// ctrl, option or cmd held alone: the key hints (ctrlhint.rs)
    pub(crate) hold: crate::ctrlhint::Hold,
    // wrapped rows per event, keyed by event index (the codex layout
    // cache: rebuild on mutation, width change, or live-elapsed tools)
    pub(crate) cache: Vec<Option<EventRows>>,
    /// Switchboard: which part of the agent's transcript the feed holds.
    pub(crate) win: sb::FeedWindow,
    pub(crate) area_w: usize,
    pub(crate) area_h: usize,
    pub(crate) events: Vec<Ev>,
    // when the last wire line arrived (thinking duration = the delta to
    // the assistant line) and the ctrl+o state of new thinking sections
    pub(crate) last_line_at: u64,
    /// the hub's time of the last live line of the feed (ms since the
    /// epoch; BISE-271: the pause marks of a replayed feed)
    pub(crate) last_ts: Option<u64>,
    /// where the mouse is, while it moves over the screen (BISE-271:
    /// the time of the turn under it); None once it clicks or leaves
    pub(crate) hover: Option<(u16, u16)>,
    /// the copy icon the last frame drew on a code block (codeblock.rs)
    pub(crate) copy_hit: Option<crate::codeblock::Hit>,
    /// the last code block copied: its icon says `copied` for a moment
    pub(crate) code_copied: Option<crate::codeblock::Copied>,
    /// where the mouse was last seen, moving, pressing or dragging
    /// (BISE-272: the pointer's shape under it)
    pub(crate) pointer_at: Option<(u16, u16)>,
    pub(crate) show_thinking: bool,
    // a Ctrl+C interrupt is in flight (until the dying turn's idle):
    // a second Ctrl+C quits instead of interrupting again
    pub(crate) interrupt_requested: bool,
    pub(crate) pending: bool,
    /// the composer: text, cursor, selection, undo, history recall
    pub(crate) ed: editor::Editor,
    /// the composer's lexed code lines (BISE-276, mdlive.rs)
    pub(crate) md_cache: crate::mdlive::Cache,
    /// where the last frame drew the composer's text (mouse, Up/Down rows)
    pub(crate) composer: ComposerArea,
    /// a short note in the status row ("copied 12 chars") and when
    pub(crate) flash: Option<(String, std::time::Instant)>,
    /// the mouse gesture in progress (selection drags, multi-clicks)
    pub(crate) mouse: MouseState,
    /// the selection on the text layer (BISE-290, textlayer.rs): cards,
    /// popups, help, the panel, the key bar…
    pub(crate) text: crate::textlayer::TextMouse,
    /// speech-to-text (Ctrl+R, /voice)
    pub(crate) voice: voice::Voice,
    /// a voice notice in the status row ("no speech detected") and when
    pub(crate) voice_note: Option<(String, std::time::Instant)>,
    /// the transcript received so far: it replaces the voice chip at
    /// the end of the clip (BISE-222)
    pub(crate) voice_text: String,
    /// voice mode (ctrl+r twice, voicemode/): the pane takes the composer
    pub(crate) voice_mode: Option<crate::voicemode::turn::VoiceMode>,
    /// the first ctrl+r of a possible double (voicemode::live::double_ctrl_r)
    pub(crate) ctrl_r_at: Option<std::time::Instant>,
    /// ctrl+r with dictation off and no setup: its picker opens at this
    /// time, unless a second ctrl+r asked for voice mode first
    pub(crate) voice_setup_at: Option<std::time::Instant>,
    pub(crate) popup_sel: usize,
    /// The composer text the user closed the `@` popup on (Esc): the
    /// popup stays closed until the text changes.
    pub(crate) popup_dismissed: Option<String>,
    pub(crate) history: Vec<String>,
    /// the tick pulses' tick (BISE-204): set by the draw loop from `anim`
    pub(crate) tick: u32,
    /// the pulses' time in ms (BISE-204; the voice chip's blink and wave)
    pub(crate) pulse_ms: u64,
    /// the frame's time: set by the draw loop with the pulses (the voice
    /// chip's timer, run::frame_clock)
    pub(crate) frame_at: std::time::Instant,
    /// the animation clock: the frames of the gust and the pulses
    pub(crate) anim: crate::anim::Clock,
    /// the working gust's motion (BISE-107): set by the draw loop, still
    /// until then (and in tests)
    pub(crate) motion: crate::gust::Motion,
    /// the motion of what is not the agent in view (the panel's gusts,
    /// the header's): still in zen (BISE-132), else `motion`
    pub(crate) motion_away: crate::gust::Motion,
    /// the terminal lost the focus (focus reporting): the gust stands still
    pub(crate) focus_lost: bool,
    /// zen while you type (BISE-121): fed by the loop, read by the draw
    pub(crate) zen: crate::zen::Zen,
    /// the last key went to the composer's own arms (an edit, a move, a
    /// newline), not to an app shortcut: set by `on_key`, read by zen
    /// (BISE-128)
    pub(crate) key_in_composer: bool,
    pub(crate) rx: Receiver<String>,
    pub(crate) should_quit: bool,
    /// Switchboard (docs/): the hub connection, the
    /// agents, the cards and the feeds out of focus.
    pub(crate) sb: sb::Sb,
    /// the images attached in the composer (`[Image #N]`, attach.rs)
    pub(crate) attachments: Vec<crate::attach::Attachment>,
    /// messages queued for after the turn (BISE-89), this feed's
    pub(crate) queued: Vec<crate::queue::Queued>,
    /// a queued message went and its turn has not started yet (queue.rs):
    /// the next one waits, whatever a stale idle state says
    pub(crate) queue_out: Option<std::time::Instant>,
}

/// The composer's text area in the last frame: its screen origin, its
/// width in columns, its visible rows and the first layout row shown.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ComposerArea {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) w: usize,
    pub(crate) h: usize,
    pub(crate) top: usize,
    /// what the view last followed: (cursor, text bytes, width); the
    /// next frame scrolls to the cursor only when it changed, so a wheel
    /// scroll stays until you type or move
    pub(crate) seen: (usize, usize, usize),
    /// the layout rows the frame drew (all of them, shown or not)
    pub(crate) total: usize,
    /// the whole composer box (its bar and pad rows): the wheel over it
    /// scrolls the text (x, y, width, height)
    pub(crate) pane: (u16, u16, u16, u16),
}

/// What a left press started: a selection in the composer (or none),
/// and the last press for double/triple clicks.
#[derive(Debug, Default, Clone)]
pub(crate) struct MouseState {
    pub(crate) drag: Option<DragIn>,
    pub(crate) last_press: Option<(std::time::Instant, u16, u16)>,
    pub(crate) clicks: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragIn {
    Composer,
    /// a press in the feed; `moved` once it selects (a drag, a double
    /// or triple click) rather than clicks
    Feed { moved: bool },
}

impl MouseState {
    /// Counts this press: 1, 2 (double) or 3 (triple click), for a press
    /// at the same cell within 400 ms of the last one.
    pub(crate) fn press(&mut self, x: u16, y: u16, now: std::time::Instant) -> u8 {
        let again = self
            .last_press
            .is_some_and(|(t, px, py)| px == x && py == y && now.duration_since(t) < Duration::from_millis(400));
        self.clicks = if again { self.clicks % 3 + 1 } else { 1 };
        self.last_press = Some((now, x, y));
        self.clicks
    }
}

impl ComposerArea {
    /// The char index under the screen cell (x, y), if inside the text.
    pub(crate) fn hit(&self, text: &str, x: u16, y: u16, clamp: bool) -> Option<usize> {
        let inside = x >= self.x.saturating_sub(1)
            && (x as usize) < self.x as usize + self.w
            && y >= self.y
            && (y as usize) < self.y as usize + self.h;
        if !inside && !clamp {
            return None;
        }
        let rows = editor::layout_input(text, self.w);
        let dy = y as isize - self.y as isize;
        let last = rows.len().saturating_sub(1) as isize;
        let row = (self.top as isize + dy).clamp(0, last) as usize;
        let col = x.saturating_sub(self.x) as usize;
        Some(editor::ci_at(&rows, row, col))
    }

    /// The rows hidden above and under the view.
    pub(crate) fn more(&self) -> (usize, usize) {
        (self.top, self.total.saturating_sub(self.top + self.h))
    }

    /// The wheel at (x, y): over the composer box with a text taller
    /// than it, scrolls the text `dy` rows (the cursor stays where it
    /// is) and says true; else false (the history scrolls).
    pub(crate) fn wheel(&mut self, x: u16, y: u16, dy: isize) -> bool {
        let (px, py, pw, ph) = self.pane;
        let over = x >= px && x < px.saturating_add(pw) && y >= py && y < py.saturating_add(ph);
        if !over || self.total <= self.h {
            return false;
        }
        let max = self.total - self.h;
        self.top = (self.top as isize + dy).clamp(0, max as isize) as usize;
        true
    }

    /// A click on a scroll hint (`↑ 4 lines above` in the blank bar row,
    /// or the ↑ / ↓ in the right margin): a screenful that way.
    pub(crate) fn hint_click(&mut self, x: u16, y: u16) -> bool {
        let (above, below) = self.more();
        let (x, y) = (x as usize, y as usize);
        let (tx, ty, py) = (self.x as usize, self.y as usize, self.pane.1 as usize);
        let margin = x == tx + self.w;
        let up = above > 0 && ((y + 1 == ty && ty > py && x >= tx) || (margin && y == ty));
        let down = below > 0 && ((y == ty + self.h && x >= tx) || (margin && y + 1 == ty + self.h));
        let page = self.h as isize;
        match (up, down) {
            (true, _) => self.wheel(x as u16, y as u16, -page),
            (_, true) => self.wheel(x as u16, y as u16, page),
            _ => false,
        }
    }
}

impl App {
    /// A fresh screen: empty feed following the tail, empty composer,
    /// main in focus, fed by the hub lines of `rx`.
    pub(crate) fn new(
        sb: sb::Sb,
        rx: std::sync::mpsc::Receiver<String>,
        debug: bool,
        area_w: usize,
        voice: crate::voice::Voice,
    ) -> App {
        App {
            connected: true,
            term: crate::term::Term::default(),
            help: None,
            approvals: None,
            logview: None,
            artifacts: None,
            scheduled: None,
            diff: None,
            computer_use: None,
            debug,
            follow: true,
            anchor: (0, 0),
            scroll: 0,
            vis_events: Vec::new(),
            vis_rows: Vec::new(),
            feed_x: 0,
            feed_y: 0,
            feed_sel: None,
            quote_hint: false,
            find: None,
            resign: None,
            palette: None,
            cmd_keys: false,
            ctrl_digits: true,
            clicks: true,
            unseen: 0,
            tail_visible: true,
            bottom_bar_rect: None,
            demo_rect: None,
            demo_after_onboarding: false,
            hold: Default::default(),
            cache: Vec::new(),
            win: Default::default(),
            area_w,
            area_h: 24,
            events: Vec::new(),
            last_line_at: 0,
            last_ts: None,
            hover: None,
            copy_hit: None,
            code_copied: None,
            show_thinking: false,
            interrupt_requested: false,
            pending: false,
            ed: crate::editor::Editor::default(),
            md_cache: Default::default(),
            composer: ComposerArea::default(),
            flash: None,
            voice,
            voice_note: None,
            voice_text: String::new(),
            voice_mode: None,
            ctrl_r_at: None,
            voice_setup_at: None,
            mouse: MouseState::default(),
            text: Default::default(),
            popup_sel: 0,
            popup_dismissed: None,
            history: Vec::new(),
            tick: 0,
            pulse_ms: 0,
            frame_at: std::time::Instant::now(),
            anim: crate::anim::Clock::default(),
            motion: crate::gust::Motion::Still,
            motion_away: crate::gust::Motion::Still,
            focus_lost: false,
            pointer_at: None,
            zen: crate::zen::Zen::default(),
            key_in_composer: false,
            rx,
            should_quit: false,
            sb,
            attachments: Vec::new(),
            queued: Vec::new(),
            queue_out: None,
        }
    }
}
