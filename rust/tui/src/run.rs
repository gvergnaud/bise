//! The interactive loop (ratatui) of the Switchboard client, and the
//! ingestion of one agent's wire lines into the feed in focus.

use crate::*;
use crossterm::event::{
    poll, read, EnableBracketedPaste, EnableMouseCapture, Event, KeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use std::io;
use std::time::Duration;

// one wire line into the feed of the app (the focused view): the TUI's
// own fold of a thread's lines, the reference the parity law
// (entry_ev_tests.rs) and the line-fed tests read since P4d-feed's
// switch (the feeds come from the hub's entries, sb/feed_entries.rs)
#[cfg(test)]
pub(crate) fn ingest_line(app: &mut App, line: String, ts: Option<u64>) {
    if line == "--- idle" {
        app.pending = false;
        app.interrupt_requested = false;
    }
    // a turn of this feed started or ended: the queued message that went
    // has its turn, so the next one may go at a turn's end (queue.rs)
    if line.starts_with("  obs: turn_started") || line.starts_with("  obs: turn_done:") {
        crate::queue::seen(app);
    }
    // the gap since the previous wire line arrived (ms since the epoch):
    // a hub that doesn't give the line's time marks a pause by it
    let now = crate::when::now_ms();
    let arrived = u128::from(bise_proto::thread::words::thought_ms(app.last_line_at, now));
    app.last_line_at = now;
    let (line, replayed) = strip_history(&line);
    // thinking duration: the model's reply comes one batch after the
    // previous line (G2: bise-proto's words::thought_ms over the lines'
    // own times, the ms the hub wrote them, as the hub's fold; a hub
    // that doesn't say: when they arrived)
    let ms = match (ts, app.last_ts) {
        (Some(t), Some(prev)) if !replayed => u128::from(bise_proto::thread::words::thought_ms(prev, t)),
        (Some(_), None) => 0,
        _ => arrived,
    };
    // a line after a pause: a time mark first (BISE-14, book §10). The
    // hub's time says the pause, replayed feeds included (BISE-271); a
    // hub without it, the time the line arrived. A line the REPL
    // replays (`history `) has the time of the replay: no mark, no end
    let ts = if replayed { None } else { ts };
    match ts {
        Some(t) => {
            if let Some(prev) = app.last_ts {
                let gap = u128::from(t.saturating_sub(prev));
                crate::feed::pause_mark(&mut app.events, &mut app.cache, gap, || crate::when::mark_now(t));
            }
            app.last_ts = Some(t);
        }
        None if !replayed => {
            crate::feed::pause_mark(&mut app.events, &mut app.cache, arrived, crate::feed::local_hhmm);
        }
        None => {}
    }
    // the end of a turn keeps its time (BISE-271: its hover)
    let ended = (line.starts_with("  obs: turn_done: ") && !replayed).then(|| ts.unwrap_or_else(crate::when::now_ms));
    // a replayed reasoning section has no duration
    let ms = if replayed { 0 } else { ms };
    let parsed = if replayed {
        parse_history_line(line)
    } else {
        parse_line(line)
    };
    if let Some(ev) = parsed {
        // the reasoning rides inside the assistant text
        // (think markers): it becomes its own collapsed
        // section, never raw history text
        let evs: Vec<Ev> = match ev {
            Ev::Assistant(t) => match split_thinking(&t) {
                Some((think, vis)) => {
                    let mut v = vec![Ev::Thinking {
                        ms,
                        text: think.to_string(),
                        open: app.show_thinking,
                    }];
                    if !vis.trim().is_empty() {
                        v.push(Ev::Assistant(vis.to_string()));
                    }
                    v
                }
                None => vec![Ev::Assistant(t)],
            },
            other => vec![other],
        };
        for ev in evs {
            let finished = match &ev {
                Ev::Tool(td) if !matches!(td.state, ToolState::Run) => {
                    Some(td.id)
                }
                _ => None,
            };
            // the view is top-anchored: a pinned view never
            // moves, a following view re-sticks in draw
            let appended =
                push_event(&mut app.events, &mut app.cache, ev);
            if appended && !app.follow {
                app.unseen += 1;
            }
            if let (true, Some(id)) = (replayed, finished) {
                hide_replayed_elapsed(&mut app.events, &mut app.cache, id);
            }
        }
    }
    if let Some(t) = ended {
        push_event(&mut app.events, &mut app.cache, Ev::Ended(t));
    }
}

/// The terminal in UI mode: raw + alternate screen, mouse reports,
/// bracketed paste (a multi-line paste arrives as ONE Event::Paste
/// instead of a keystroke storm where every Enter would send), and the
/// kitty keyboard protocol (Shift+Enter reported distinctly; terminals
/// without support ignore the push, Ctrl+J remains the fallback). Where
/// the terminal confirms them, the flags of the ctrl hints too (ctrl
/// alone, releases, the typed text: ctrlhint.rs).
/// Fallible, unlike `ratatui::init` (which panics), and without its
/// panic hook: `crash::install` restores every one of these modes.
fn init_terminal() -> io::Result<(crate::links::Tui, bool)> {
    use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
    let setup = || -> io::Result<(crate::links::Tui, bool)> {
        enable_raw_mode()?;
        // BISE-02: light or dark from the terminal background, before the alternate screen
        crate::theme_detect::init();
        crossterm::execute!(io::stdout(), EnterAlternateScreen)?;
        let _ = crossterm::execute!(io::stdout(), EnableMouseCapture);
        let _ = crossterm::execute!(io::stdout(), EnableBracketedPaste);
        // BISE-107: the gust stops while the terminal is not focused
        let _ = crossterm::execute!(io::stdout(), crossterm::event::EnableFocusChange);
        let protocol = push_keyboard_flags();
        Ok((ratatui::Terminal::new(crate::links::LinkBackend::new(io::stdout()))?, protocol))
    };
    setup().inspect_err(|_| crash::restore_terminal())
}

/// One push of the kitty keyboard flags (`crash::restore_terminal` pops
/// one): the ctrl hints' [`crate::ctrlhint::FLAGS`] when the terminal's
/// `CSI ? u` reply keeps them all, else flag 1 alone as before. True
/// when the terminal speaks the protocol (its reply has flag 1): ctrl+1-9
/// reach bise (BISE-302, reach.rs).
fn push_keyboard_flags() -> bool {
    use crossterm::event::PopKeyboardEnhancementFlags;
    let basic = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES;
    let speaks = |reply: &[u8]| crate::ctrlhint::reply_flags(reply).is_some_and(|f| f & basic.bits() != 0);
    let reply = if crate::ctrlhint::wanted() {
        let _ = crossterm::execute!(io::stdout(), PushKeyboardEnhancementFlags(crate::ctrlhint::FLAGS));
        let reply = crate::theme_detect::query(b"\x1b[?u").unwrap_or_default();
        if crate::ctrlhint::confirmed(&reply) {
            // the flags carry the event types: voice mode's hold space
            crate::voicemode::live::RELEASES.store(true, std::sync::atomic::Ordering::Relaxed);
            return true;
        }
        let _ = crossterm::execute!(io::stdout(), PopKeyboardEnhancementFlags);
        let _ = crossterm::execute!(io::stdout(), PushKeyboardEnhancementFlags(basic));
        reply
    } else {
        let _ = crossterm::execute!(io::stdout(), PushKeyboardEnhancementFlags(basic));
        crate::theme_detect::query(b"\x1b[?u").unwrap_or_default()
    };
    speaks(&reply)
}

/// Takes the waiting wire lines for at most 12 ms. The hub replays whole
/// feeds on connect: the lines are taken in slices, a frame in between,
/// so the UI never waits for a replay to end. True when lines are left
/// (the next frame must not wait for input).
fn drain_lines(app: &mut App) -> bool {
    let slice = std::time::Instant::now();
    loop {
        if slice.elapsed() >= Duration::from_millis(12) {
            return true;
        }
        match app.rx.try_recv() {
            Ok(line) => sb::dispatch(app, &line),
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // the hub reader is gone for good: stop, the caller
                // decides (a version switch re-executes the TUI)
                app.connected = false;
                app.should_quit = true;
                return false;
            }
        }
    }
}

/// A caught panic of one handler: shown in the feed (it was logged by
/// the hook), the editor state clamped back to valid.
fn report_crash(app: &mut App, c: &crash::Crash, what: &str) {
    app.ed.cursor = app.ed.cursor.min(app.ed.len());
    app.ed.anchor = app.ed.anchor.map(|a| a.min(app.ed.len()));
    push_event(&mut app.events, &mut app.cache, Ev::Err(c.line(what)));
}

/// Consecutive frames that panicked before the UI gives up (a draw that
/// always panics would loop forever): it exits with the terminal back.
const MAX_DRAW_CRASHES: u32 = 3;

pub(crate) fn run_tui(app: &mut App) -> io::Result<()> {
    crash::install();
    let (mut terminal, protocol) = init_terminal()?;
    // BISE-302: ctrl+1-9 and clicks, what the inbox's words offer
    let reach = crate::reach::detect(protocol);
    app.ctrl_digits = reach.ctrl_digits;
    app.clicks = reach.clicks;
    crash::set_ui_thread(true);
    // BISE-60: the first launch of the switchboard UI plays the onboarding
    // (BISE-284: at its end, main's composer holds `show me what you can do`)
    app.demo_after_onboarding = crate::onboarding::due(&|k| std::env::var(k).ok().filter(|v| !v.is_empty()));
    crate::onboarding::request_if_due(app);
    // BISE-245: the setup card, when due, at the first hello
    crate::sb::setup::arm(app);
    let r = ui_loop(app, &mut terminal);
    crash::set_ui_thread(false);
    // BISE-272: the terminal's own mouse pointer back
    let _ = terminal.backend_mut().set_pointer(crate::pointer::Shape::Default);
    // no orphan shell
    app.term.shutdown();
    crash::restore_terminal();
    r
}

/// Startup timing (SB_TIMING): what the loop spent until the first
/// usable frame (the hub's replay taken in, then drawn).
#[derive(Default)]
struct Startup {
    on: bool,
    done: bool,
    frames: u32,
    drain_ms: f64,
    draw_ms: f64,
    ready_seen: bool,
    caught_up: bool,
}

impl Startup {
    fn ready(app: &App) -> bool {
        sb::is_ready(app)
    }
    fn after_drain(&mut self, app: &App, t: std::time::Instant, backlog: bool) {
        if !self.on || self.done {
            return;
        }
        self.drain_ms += t.elapsed().as_secs_f64() * 1000.0;
        if !self.ready_seen && Self::ready(app) {
            self.ready_seen = true;
            crate::timing::mark(&format!(
                "ready dispatched ({} frames, drain {:.1} ms, draw {:.1} ms so far)",
                self.frames, self.drain_ms, self.draw_ms
            ));
        }
        if self.ready_seen && !backlog && !self.caught_up {
            self.caught_up = true;
            let evs: usize = app.events.len()
                + sb::background_events(app);
            crate::timing::mark(&format!(
                "caught up ({} events in all feeds, {} in focus, drain {:.1} ms)",
                evs,
                app.events.len(),
                self.drain_ms
            ));
        }
    }
    fn after_draw(&mut self, t: std::time::Instant) {
        if !self.on || self.done {
            return;
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        self.draw_ms += ms;
        self.frames += 1;
        if self.frames == 1 {
            crate::timing::mark(&format!("first frame ({:.1} ms)", ms));
        }
        if self.caught_up {
            self.done = true;
            crate::timing::mark(&format!(
                "usable frame ({:.1} ms; {} frames, drain {:.1} ms, draw {:.1} ms in all)",
                ms, self.frames, self.drain_ms, self.draw_ms
            ));
        }
    }
}

/// One frame of the UI: the view, the one-time hints, then the frame
/// passes: the theme's ground on every cell (BISE-92), zen's fade while
/// you type (BISE-121), `BISE_ASCII`.
pub(crate) fn draw_frame(app: &mut App, f: &mut ratatui::Frame) {
    crate::links::begin_frame(); // the feed says where its links are
    crate::pointer::begin_frame(); // BISE-272: what has which pointer shape
    crate::textlayer::begin_frame(); // BISE-290: where the text is
    sb::draw_sb(app, f);
    // the demo's guided tips (tour.rs), else the one-time hints (BISE-61)
    if !crate::tour::draw(app, f) {
        crate::hints::draw(f, app.ctrl_digits);
    }
    crate::ctrlhint::draw(app, f.buffer_mut()); // ctrl held: the key hints
    crate::sanitize::cells(f.buffer_mut()); // no TAB/CR/ESC in a cell: no ghosts
    // BISE-290: the text's links and selection, the frame kept for the mouse
    crate::textlayer::finish(f.buffer_mut(), &app.text);
    crate::theme::paint(f.buffer_mut()); // BISE-92: bise paints its ground
    let depth = app.zen.depth(std::time::Instant::now());
    if depth > 0.0 {
        let attention = [crate::theme::accent(), crate::theme::error()];
        crate::zen::fade(f.buffer_mut(), depth, &app.zen.keep, &attention, app.zen.no_color);
    }
    crate::theme::asciify(f.buffer_mut()); // BISE-84: BISE_ASCII=1
}

/// What an input event can change, taken before it (zen, BISE-121,
/// BISE-124): the composer, and what is outside it.
struct Before {
    text: String,
    dead: Option<char>,
    popup: bool,
    /// the help overlay or the terminal pane in front: they take the keys
    front: bool,
    scene: sb::Scene,
    feed_sel: Option<crate::feedsel::FeedSel>,
    view: (bool, isize),
    show_thinking: bool,
    voice: crate::voice::VoiceState,
}

impl Before {
    fn of(app: &App) -> Before {
        Before {
            text: app.ed.text.clone(),
            dead: app.ed.pending_dead(),
            popup: !crate::commands::popup_items(app).is_empty(),
            front: app.help.is_some() || app.term.shown(),
            scene: sb::scene(app),
            feed_sel: app.feed_sel,
            view: (app.follow, app.scroll),
            show_thinking: app.show_thinking,
            voice: app.voice.state(),
        }
    }
}

/// What an input event is for zen (BISE-121, BISE-124, BISE-128), from
/// the state before it and after it. A key reached the composer when
/// `on_key` gave it to the composer's own arms (no app shortcut took it)
/// and nothing outside the composer changed.
fn zen_input(app: &App, ev: &Event, before: &Before) -> crate::zen::Input {
    use crate::zen::Input;
    let changed = app.ed.text != before.text || app.ed.pending_dead() != before.dead;
    let popup = before.popup || !crate::commands::popup_items(app).is_empty();
    // BISE-134: a typed key that took the history's selection as a quote
    // is typing (the selection is the composer's now)
    let quoted = before.feed_sel.is_some() && app.feed_sel.is_none() && changed && app.key_in_composer;
    let outside = before.front
        || app.help.is_some()
        || app.term.shown()
        || sb::scene(app) != before.scene
        || (app.feed_sel != before.feed_sel && !quoted)
        || (app.follow, app.scroll) != before.view
        || app.show_thinking != before.show_thinking
        || app.voice.state() != before.voice;
    match ev {
        Event::Key(k) => crate::zen::key_input(k, app.key_in_composer && !outside, changed, popup),
        Event::Paste(_) if !outside && !popup => {
            if changed {
                Input::Typing
            } else {
                Input::Hold
            }
        }
        Event::Paste(_) | Event::Mouse(_) | Event::FocusLost => Input::Other,
        _ => Input::Neutral,
    }
}

/// The frame's clock at `now`, before each draw (BISE-204: every
/// animation reads the clock, never the turns): the pulses' time (the
/// voice chip's blink and wave), the gust's motion, the frame's time
/// (the voice chip's timer). No input needed: the loop wakes every
/// 50 ms while the voice is at work.
pub(crate) fn frame_clock(app: &mut App, now: std::time::Instant, last_draw: Duration, reduce_motion: bool) {
    let zen = app.zen.active(now);
    let frame = app.anim.gust(now);
    // the tick pulses (`∿` of a running tool, `·` starting) hold
    // still while you type (zen, BISE-121)
    app.pulse_ms = app.anim.pulse_ms(now, zen, app.zen.end());
    app.tick = (app.pulse_ms / crate::anim::PULSE_MS) as u32;
    app.motion = crate::gust::motion(app.focus_lost, last_draw, reduce_motion, zen, frame);
    // zen (BISE-132): the other agents' gusts stand still, their `∿`
    // pulsing slowly in color (a hush: it still works)
    app.motion_away = crate::gust::away(app.motion, zen, app.zen.no_color, frame);
    app.frame_at = now;
}

fn ui_loop(app: &mut App, terminal: &mut crate::links::Tui) -> io::Result<()> {
    let mut draw_crashes = 0u32;
    // the gust's motion (BISE-107): the last draw's time, the env once
    let mut last_draw = Duration::ZERO;
    let reduce_motion = crate::gust::reduce_motion();
    // zen (BISE-121): no ramp with less motion, the dim attribute under NO_COLOR
    if reduce_motion {
        app.zen.fade = Duration::ZERO;
    }
    app.zen.no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let mut startup = Startup { on: crate::timing::enabled(), ..Startup::default() };
    loop {
        let t_drain = std::time::Instant::now();
        let backlog = match crash::guarded(|| drain_lines(app)) {
            Ok(b) => b,
            Err(c) => {
                report_crash(app, &c, "a hub line");
                true
            }
        };
        startup.after_drain(app, t_drain, backlog);
        for note in crash::take_notes() {
            push_event(&mut app.events, &mut app.cache, Ev::Err(note));
            app.zen.leave(std::time::Instant::now());
        }
        // a card, a message to you, a confirm: zen steps aside (BISE-121)
        app.zen.calls(app.sb.calls(), std::time::Instant::now());
        if app.should_quit {
            break;
        }
        // BISE-60: the onboarding (first launch, /welcome) over the whole screen
        if crate::onboarding::take_request() {
            let shown = crash::guarded(|| {
                crate::onboarding::show(app, terminal, &mut |app| {
                    if let Err(c) = crash::guarded(|| drain_lines(app)) {
                        report_crash(app, &c, "a hub line");
                    }
                })
            });
            match shown {
                Ok(r) => r?,
                Err(c) => report_crash(app, &c, "the onboarding"),
            }
            // BISE-284: the first open of the thread after the first run
            if std::mem::take(&mut app.demo_after_onboarding) {
                crate::sb::prefill_demo(app);
            }
            // BISE-294: /provider may have changed the keys; the /model
            // that sent it there runs now its provider is set up
            crate::models::forget_keys();
            // BISE-298: the voice picker turned voice on, or left it off
            if let Some(out) = crate::onboarding::take_voice_out() {
                crate::input::voice_out(app, out);
            }
            if let Some(line) = crate::onboarding::provider_line() {
                let evs = crate::sb::handle_input(app, &line);
                let _ = evs;
            }
            continue;
        }
        // voice mode's settings (/voice) or its first-time screen
        if let Some(open) = crate::voicemode::settings::take_request() {
            let shown = crash::guarded(|| {
                crate::voicemode::settings::show(
                    app,
                    terminal,
                    open,
                    &mut |app| {
                        if let Err(c) = crash::guarded(|| drain_lines(app)) {
                            report_crash(app, &c, "a hub line");
                        }
                    },
                    &mut crate::voicemode::sample::Player::live(),
                )
            });
            match shown {
                Ok(r) => crate::voicemode::live::after_settings(app, open, r?),
                Err(c) => report_crash(app, &c, "the voice settings"),
            }
            continue;
        }
        pump_voice(app);
        crate::voicemode::live::pump(app);
        frame_clock(app, std::time::Instant::now(), last_draw, reduce_motion);
        let t_draw = std::time::Instant::now();
        let drawn = crash::guarded(|| {
            // BISE-92: the terminal's own background follows the theme
            crate::theme_detect::sync_terminal_bg();
            terminal.draw(|f| draw_frame(app, f))
        });
        startup.after_draw(t_draw);
        last_draw = t_draw.elapsed();
        match drawn {
            Ok(r) => {
                r?;
                draw_crashes = 0;
                // BISE-272: the pointer's shape under the mouse, on a change
                terminal.backend_mut().set_pointer(crate::pointer::wanted(app))?;
            }
            Err(c) => {
                draw_crashes += 1;
                if draw_crashes >= MAX_DRAW_CRASHES {
                    return Err(io::Error::other(format!(
                        "the screen could not be drawn ({} panics in a row): {} at {}{}",
                        draw_crashes,
                        c.message,
                        c.location,
                        c.log.map(|p| format!(" — details: {}", p.display())).unwrap_or_default()
                    )));
                }
                report_crash(app, &c, "a frame");
                // the half-drawn buffer is dropped: the next frame repaints all
                let _ = terminal.clear();
            }
        }
        // expired-ux: the ChatGPT sign-in again from the thread, its answer
        crate::resign::tick(app);
        // the terminal's tab title: inbox, new artifacts, agents at work, repo
        crate::termtitle::tick(&sb::title_status(app), std::time::Instant::now());
        // the voice chip moves every 50 ms while recording or
        // transcribing (its meter, blink and wave; Vibe's poll)
        let wait = if backlog || app.find.as_ref().is_some_and(|f| f.busy()) {
            Duration::ZERO
        } else if app.voice.active() || app.voice_mode.is_some() {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(80)
        };
        // ctrl, option or cmd held: wake when its hints are due
        let wait = app.hold.due(std::time::Instant::now()).map_or(wait, |d| wait.min(d));
        if poll(wait)? {
            let ev = read()?;
            // ctrl, option, cmd alone, releases and repeats (ctrlhint.rs): the hold
            // sees them all, the handlers only presses
            app.hold.event(&ev, std::time::Instant::now());
            // voice mode's space: presses, repeats and releases (hold space)
            if let Event::Key(k) = &ev {
                if crate::voicemode::live::space(app, k) {
                    continue;
                }
            }
            let Some(ev) = crate::ctrlhint::for_handlers(ev).map(crate::optkeys::read_back) else { continue };
            // keep-state: a reload waits while keys arrive
            if matches!(ev, Event::Key(_) | Event::Paste(_)) {
                app.sb.reload_wait.key(std::time::Instant::now());
            }
            let term_h = terminal.size().map(|s| s.height).unwrap_or(24);
            let before = Before::of(app);
            let zen_ev = ev.clone();
            let handled = crash::guarded(|| match ev {
                Event::Mouse(m) => {
                    on_mouse(app, &m, term_h);
                    false
                }
                Event::Paste(text) => {
                    on_paste(app, &text);
                    false
                }
                Event::Key(k) => {
                    app.hover = None;
                    on_key(app, &k)
                }
                Event::FocusLost => {
                    app.focus_lost = true;
                    app.hover = None;
                    false
                }
                Event::FocusGained => {
                    app.focus_lost = false;
                    false
                }
                _ => false,
            });
            match handled {
                Ok(true) => break,
                Ok(false) => {
                    let i = zen_input(app, &zen_ev, &before);
                    app.zen.input(i, std::time::Instant::now());
                }
                Err(c) => {
                    report_crash(app, &c, "an input event");
                    app.zen.leave(std::time::Instant::now());
                }
            }
        }
        // BISE-120a: the drafts on disk, once they stop moving
        sb::drafts::tick(app);
        // keep-state: the reload the hub asked for, once the keys stop
        if app.sb.reload_wait.due(std::time::Instant::now()) {
            app.should_quit = true;
        }
        sb::setup::pump(app);
        // background lines: /plugins login's, "needs a login", /keychain's
        crate::plugins::pump(app);
        crate::keychain::pump(app);
    }
    Ok(())
}

#[cfg(test)]
mod zen_tests {
    //! BISE-121: zen enters and leaves on the loop's real events, and
    //! fades the chrome but the history (BISE-132), the composer's text,
    //! the label, what needs you.
    use super::*;
    use crate::zen::Input;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;
    use std::time::Instant;

    /// One event through the loop's handlers and zen, at `now`.
    fn event(app: &mut App, ev: Event, now: Instant) -> Input {
        let before = Before::of(app);
        match &ev {
            Event::Key(k) => {
                on_key(app, k);
            }
            Event::Paste(t) => on_paste(app, t),
            Event::Mouse(m) => on_mouse(app, m, 36),
            _ => {}
        }
        let i = zen_input(app, &ev, &before);
        app.zen.input(i, now);
        i
    }

    fn key(c: KeyCode) -> Event {
        Event::Key(KeyEvent::new(c, KeyModifiers::NONE))
    }

    fn screen(app: &mut App) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(120, 36)).unwrap();
        t.draw(|f| draw_frame(app, f)).unwrap();
        t.backend().buffer().clone()
    }

    fn app_with_agents() -> App {
        let mut app = crate::sb::bench::test_app();
        crate::sb::hub_reads::rows_for_tests::apply(&mut app, vec![crate::sb::hub_reads::rows_for_tests::agent("main", "idle", ""), crate::sb::hub_reads::rows_for_tests::agent("docs", "working", "write the docs")], vec![]);
        crate::sb::hub_reads::ready(&mut app);
        crate::sb::entries_for_tests::lines(&mut app, "main", &["sb you : ship it"]);
        app
    }

    fn with(c: KeyCode, m: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(c, m))
    }

    #[test]
    fn typing_enters_and_every_other_input_leaves() {
        let t = Instant::now();
        let mut app = app_with_agents();
        assert_eq!(event(&mut app, key(KeyCode::Char('h')), t), Input::Typing);
        assert_eq!(event(&mut app, key(KeyCode::Char('i')), t), Input::Typing);
        assert!(app.zen.active(t));
        assert_eq!(event(&mut app, key(KeyCode::Backspace), t), Input::Typing);
        // a mouse move, click or scroll: out
        let m = MouseEvent { kind: MouseEventKind::Moved, column: 3, row: 3, modifiers: KeyModifiers::NONE };
        assert_eq!(event(&mut app, Event::Mouse(m), t), Input::Other);
        assert!(!app.zen.active(t));
        // ctrl+o, esc, tab: out
        event(&mut app, key(KeyCode::Char('y')), t);
        assert_eq!(event(&mut app, with(KeyCode::Char('o'), KeyModifiers::CONTROL), t), Input::Other);
        event(&mut app, key(KeyCode::Char('y')), t);
        assert_eq!(event(&mut app, key(KeyCode::Esc), t), Input::Other);
        event(&mut app, key(KeyCode::Char('y')), t);
        assert_eq!(event(&mut app, key(KeyCode::Tab), t), Input::Other);
        // a paste types; the focus lost leaves; a resize is neutral
        assert_eq!(event(&mut app, Event::Paste("more".into()), t), Input::Typing);
        assert_eq!(event(&mut app, Event::Resize(80, 20), t), Input::Neutral);
        assert!(app.zen.active(t));
        assert_eq!(event(&mut app, Event::FocusLost, t), Input::Other);
        // a popup (`/` in an empty composer) is not zen
        app.ed.clear();
        assert_eq!(event(&mut app, key(KeyCode::Char('/')), t), Input::Other);
        assert!(!app.zen.active(t));
    }

    /// BISE-134: typing with a selection in the history quotes it and
    /// enters zen like any typing; esc on the selection is still out.
    #[test]
    fn typing_that_quotes_the_selection_is_typing() {
        let t = Instant::now();
        let mut app = app_with_agents();
        app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 200) });
        assert_eq!(event(&mut app, key(KeyCode::Char('w')), t), Input::Typing);
        assert!(app.ed.text.starts_with("[Quote #1] w"), "{}", app.ed.text);
        assert!(app.zen.active(t));
        app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 200) });
        assert_eq!(event(&mut app, key(KeyCode::Esc), t), Input::Other);
        assert!(!app.zen.active(t));
    }

    #[test]
    fn keys_that_edit_or_move_in_the_composer_keep_zen() {
        // BISE-124 (user: « quand je tape un accent genre ` ou les arrow
        // keys, etc le zen mode s'enlève »)
        let t = Instant::now();
        let mut app = app_with_agents();
        let (n, s, a, c) = (KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::ALT, KeyModifiers::CONTROL);
        event(&mut app, key(KeyCode::Char('d')), t);
        // Ghostty on a U.S. layout: ⌥` is Char('`') + ALT, a dead key
        // (the text does not change yet), then the letter
        assert_eq!(event(&mut app, with(KeyCode::Char('`'), a), t), Input::Typing);
        assert!(app.zen.active(t));
        assert_eq!(event(&mut app, key(KeyCode::Char('e')), t), Input::Typing);
        assert_eq!(app.ed.text, "dè");
        // an Option character (⌥c = ç), an accent the terminal composed
        assert_eq!(event(&mut app, with(KeyCode::Char('c'), a), t), Input::Typing);
        assert_eq!(event(&mut app, key(KeyCode::Char('é')), t), Input::Typing);
        assert_eq!(app.ed.text, "dèçé");
        // the moves hold it: arrows, word moves, home/end, emacs keys
        for (k, m) in [
            (KeyCode::Left, n),
            (KeyCode::Right, n),
            (KeyCode::Left, a),
            (KeyCode::Right, c),
            (KeyCode::Left, s),
            (KeyCode::Up, n),
            (KeyCode::Down, n),
            (KeyCode::Home, n),
            (KeyCode::End, n),
            (KeyCode::Char('a'), c),
            (KeyCode::Char('b'), a),
        ] {
            let i = event(&mut app, with(k, m), t);
            assert!(i == Input::Hold || i == Input::Typing, "{k:?} {m:?}: {i:?}");
            assert!(app.zen.active(t), "{k:?} {m:?}");
        }
        // the edits: word delete, the new lines (shift/alt+⏎, ctrl+j)
        event(&mut app, key(KeyCode::End), t);
        assert_eq!(event(&mut app, with(KeyCode::Backspace, a), t), Input::Typing);
        event(&mut app, key(KeyCode::Char('x')), t);
        assert_eq!(event(&mut app, with(KeyCode::Enter, s), t), Input::Typing);
        assert_eq!(event(&mut app, with(KeyCode::Enter, a), t), Input::Typing);
        assert_eq!(event(&mut app, with(KeyCode::Char('j'), c), t), Input::Typing);
        assert!(app.ed.text.ends_with("x\n\n\n"));
        assert!(app.zen.active(t));
        // a move 4 s later starts the 5 s again (BISE-128: was 8 s)
        let later = t + std::time::Duration::from_secs(4);
        assert_eq!(event(&mut app, key(KeyCode::Left), later), Input::Hold);
        assert!(app.zen.active(later + std::time::Duration::from_millis(4999)));
        assert!(!app.zen.active(later + std::time::Duration::from_secs(5)));
        // an arrow alone never starts zen
        let mut app = app_with_agents();
        app.ed.insert("abc");
        assert_eq!(event(&mut app, key(KeyCode::Left), t), Input::Hold);
        assert!(!app.zen.active(t));
    }

    #[test]
    fn switching_agents_leaves_zen() {
        let t = Instant::now();
        let mut app = app_with_agents();
        event(&mut app, key(KeyCode::Char('h')), t);
        assert_eq!(app.sb.focus_name(), "main");
        // ⌥1 goes to the agent numbered 1 (the composer keeps its draft)
        assert_eq!(event(&mut app, with(KeyCode::Char('1'), KeyModifiers::ALT), t), Input::Other);
        assert_eq!(app.sb.focus_name(), "docs");
        assert!(!app.zen.active(t));
        // on an empty composer, ⌥↓ moves the panel's selection: out
        let mut app = app_with_agents();
        event(&mut app, key(KeyCode::Char('h')), t);
        event(&mut app, key(KeyCode::Backspace), t);
        assert!(app.zen.active(t));
        assert_eq!(event(&mut app, with(KeyCode::Down, KeyModifiers::ALT), t), Input::Other);
        assert!(!app.zen.active(t));
    }

    /// BISE-128 (user: « Quand je fais enter ça devrait enlever le zen
    /// mode direct. pareil si je lance un shortcut pour switcher ou
    /// naviguer dans la UI »): ⏎ send and every shortcut of help::ROWS
    /// that switches or moves the UI leave zen at once, each in a state
    /// where it does its job.
    #[test]
    fn enter_and_every_ui_shortcut_leave_zen() {
        let t = Instant::now();
        let (n, s, a, c) = (KeyModifiers::NONE, KeyModifiers::SHIFT, KeyModifiers::ALT, KeyModifiers::CONTROL);
        let card = crate::sb::hub_reads::rows_for_tests::lines(
            vec![crate::sb::hub_reads::rows_for_tests::agent("main", "idle", ""), crate::sb::hub_reads::rows_for_tests::agent("docs", "working", "write the docs")],
            vec![crate::sb::hub_reads::rows_for_tests::card(7, "question", "docs", "v1 or v2?"), crate::sb::hub_reads::rows_for_tests::card(8, "question", "docs", "ship?")],
        );
        // (what, the key, set up the app once in zen, composer empty or not)
        type Setup = fn(&mut App);
        let none: Setup = |_| {};
        // ⌥↓ on an empty composer selects an agent in the panel
        let select: Setup = |app| {
            on_key(app, &KeyEvent::new(KeyCode::Down, KeyModifiers::ALT));
        };
        let scrolled: Setup = |app| {
            app.follow = false;
            app.scroll = -10;
        };
        let in_docs: Setup = |app| crate::sb::focus(app, "docs");
        let pending: Setup = |app| app.pending = true;
        // the `$` popup lists the skills index: one skill, whatever the
        // machine's index holds
        crate::skills::TEST_INDEX.with(|t| *t.borrow_mut() = Some(crate::skills::parse_index("bend\tthe Bend guide\t/x/SKILL.md\n")));
        let cases: Vec<(&str, KeyCode, KeyModifiers, bool, bool, Setup)> = vec![
            // (name, code, mods, with cards, text in the composer, setup)
            ("⏎ send", KeyCode::Enter, n, false, true, none),
            ("⏎ on an empty composer", KeyCode::Enter, n, false, false, none),
            ("⌥1 to an agent", KeyCode::Char('1'), a, false, true, none),
            ("⌥0 back to main", KeyCode::Char('0'), a, false, true, in_docs),
            ("alt+↓ next agent", KeyCode::Down, a, false, false, none),
            ("alt+↑ previous agent", KeyCode::Up, a, false, false, none),
            ("⏎ enter the selected agent", KeyCode::Enter, n, false, false, select),
            ("space preview", KeyCode::Char(' '), n, false, false, select),
            ("D drop", KeyCode::Char('D'), s, false, false, select),
            ("A archived", KeyCode::Char('A'), s, false, false, select),
            ("esc close the selection", KeyCode::Esc, n, false, false, select),
            ("esc back to main", KeyCode::Esc, n, false, false, in_docs),
            ("esc draft away", KeyCode::Esc, n, false, true, none),
            ("ctrl+2 another inbox item", KeyCode::Char('2'), c, true, true, none),
            ("ctrl+n next card", KeyCode::Char('n'), c, true, true, none),
            ("ctrl+p previous card", KeyCode::Char('p'), c, true, true, none),
            ("ctrl+x close the card", KeyCode::Char('x'), c, true, true, none),
            ("pgup the card", KeyCode::PageUp, n, true, true, none),
            ("ctrl+o open everything", KeyCode::Char('o'), c, false, true, none),
            ("pgup the feed", KeyCode::PageUp, n, false, true, none),
            ("pgdn the feed", KeyCode::PageDown, n, false, true, scrolled),
            ("end back to the bottom", KeyCode::End, n, false, true, scrolled),
            ("ctrl+l clear", KeyCode::Char('l'), c, false, true, none),
            ("ctrl+c interrupt", KeyCode::Char('c'), c, false, true, pending),
            ("ctrl+r voice", KeyCode::Char('r'), c, false, true, none),
            ("ctrl+v attach", KeyCode::Char('v'), c, false, true, none),
            ("cmd+c copy", KeyCode::Char('c'), KeyModifiers::SUPER, false, true, none),
            ("tab", KeyCode::Tab, n, false, true, none),
            ("/ popup", KeyCode::Char('/'), n, false, false, none),
            ("$ popup", KeyCode::Char('$'), n, false, false, none),
        ];
        for (what, code, m, cards, text, setup) in cases {
            let mut app = app_with_agents();
            if cards {
                for l in &card {
                    sb::dispatch(&mut app, l);
                }
                let calls = app.sb.calls();
                app.zen.calls(calls, t);
                on_key(&mut app, &KeyEvent::new(KeyCode::Char('1'), c));
                assert!(crate::sb::card_divider_label(&app).is_some(), "{what}: an inbox item is open");
            }
            setup(&mut app);
            event(&mut app, key(KeyCode::Char('h')), t);
            if !text {
                event(&mut app, key(KeyCode::Backspace), t);
            }
            assert!(app.zen.active(t), "{what}: in zen first");
            assert_eq!(app.ed.text.is_empty(), !text, "{what}");
            assert_eq!(event(&mut app, with(code, m), t), Input::Other, "{what}");
            assert!(!app.zen.active(t), "{what}");
        }
        // the help overlay comes up by ⏎ (/help): out; a key typed in it
        // (its filter) does not start zen
        let mut app = app_with_agents();
        app.ed.insert("/help");
        assert_eq!(event(&mut app, key(KeyCode::Enter), t), Input::Other);
        assert!(app.help.is_some());
        assert_eq!(event(&mut app, key(KeyCode::Char('z')), t), Input::Other);
        assert!(!app.zen.active(t));
    }

    #[test]
    fn a_card_or_a_message_to_you_leaves_zen() {
        let t = Instant::now();
        let mut app = app_with_agents();
        let calls = |app: &mut App| {
            let n = app.sb.calls();
            app.zen.calls(n, t);
        };
        calls(&mut app);
        event(&mut app, key(KeyCode::Char('h')), t);
        calls(&mut app);
        assert!(app.zen.active(t), "nothing new");
        // a new card
        let state = crate::sb::hub_reads::rows_for_tests::lines(vec![crate::sb::hub_reads::rows_for_tests::agent("main", "idle", "")], vec![crate::sb::hub_reads::rows_for_tests::card(7, "question", "docs", "v1 or v2?")]);
        for l in &state {
            sb::dispatch(&mut app, l);
        }
        calls(&mut app);
        assert!(!app.zen.active(t));
        // the same card again: zen holds
        event(&mut app, key(KeyCode::Char('e')), t);
        for l in &state {
            sb::dispatch(&mut app, l);
        }
        calls(&mut app);
        assert!(app.zen.active(t));
        // a message to you, in a feed out of view
        crate::sb::entries_for_tests::lines(&mut app, "docs", &["sb msg-you : docs : la v2 est prête"]);
        calls(&mut app);
        assert!(!app.zen.active(t));
    }

    #[test]
    fn zen_fades_the_chrome_but_the_history_the_typed_text_the_label_and_the_accent() {
        let mut app = app_with_agents();
        let calm = screen(&mut app);
        let t = Instant::now() - std::time::Duration::from_secs(1);
        for c in "hello".chars() {
            event(&mut app, key(KeyCode::Char(c)), t);
        }
        // the same screen out of zen, then in zen for 1 s (the fade done)
        app.zen = crate::zen::Zen::default();
        let plain = screen(&mut app);
        app.zen.input(Input::Typing, t);
        let zen = screen(&mut app);
        let (ground, accent, depth) = (crate::theme::bg(), crate::theme::accent(), crate::zen::DEPTH);
        // the typed text: same cells, same colors
        let (cx, cy) = (app.composer.x, app.composer.y);
        for x in cx..cx + 5 {
            assert_eq!(zen[(x, cy)], plain[(x, cy)], "composer cell {x}");
        }
        // the divider's label (`you → main`) as it was
        let label_y = (0..36).find(|&y| (0..120).map(|x| zen[(x, y)].symbol()).collect::<String>().contains("you → main")).unwrap();
        let lx = (0..120).find(|&x| zen[(x, label_y)].symbol() == "y").unwrap();
        let row = |b: &Buffer| (lx - 1..lx + 11).map(|x| b[(x, label_y)].clone()).collect::<Vec<_>>();
        assert_eq!(row(&zen), row(&plain));
        // BISE-132: the history you read, as it was: every cell of the
        // feed area, from the history's first row to the divider
        let cols = crate::layout::cols(120, 36);
        let rows = crate::layout::rows(120, 36);
        let feed = |b: &Buffer| {
            (rows.body..label_y).flat_map(|y| (cols.feed_x..cols.feed_x + cols.feed_w).map(move |x| (x, y))).map(|p| b[p].clone()).collect::<Vec<_>>()
        };
        assert_eq!(feed(&zen), feed(&plain));
        assert!(feed(&plain).iter().map(|c| c.symbol()).collect::<String>().contains("ship it"));
        // the chrome: 45 % toward its ground (the panel's rows, the header,
        // the key bar)
        let fades = |x: u16, y: u16| zen[(x, y)].fg == crate::zen::toward(plain[(x, y)].fg, plain[(x, y)].bg, depth).unwrap() && zen[(x, y)].fg != plain[(x, y)].fg;
        let find = |s: &str, from_x: u16| {
            (0..36)
                .flat_map(|y| (from_x..120).map(move |x| (x, y)))
                .find(|&(x, y)| (x..(x + s.chars().count() as u16).min(120)).map(|x| plain[(x, y)].symbol()).collect::<String>() == s)
                .unwrap()
        };
        let panel_x = cols.panel.unwrap().x;
        let (dx, dy) = find("docs", panel_x);
        assert!(fades(dx, dy), "the panel's agent row");
        assert!((0..120).any(|x| fades(x, 0)), "the header");
        let keybar = (label_y + 1..36).rev().find(|&y| (0..120).any(|x| plain[(x, y)].symbol() != " ")).unwrap();
        assert!((0..120).any(|x| fades(x, keybar)), "the key bar");
        // every accent cell (what needs you, the agent you talk to) kept;
        // no ground moves; nothing else changed but colors
        let mut faded = 0;
        for (a, b) in plain.content.iter().zip(&zen.content) {
            assert_eq!(a.symbol(), b.symbol());
            assert_eq!(a.bg, b.bg);
            if a.fg == accent {
                assert_eq!(b.fg, accent);
            }
            faded += usize::from(a.fg != b.fg);
        }
        assert!(faded > 50, "{faded} cells faded");
        assert!(calm.content.iter().all(|c| c.bg != ground || c.fg != crate::zen::toward(crate::theme::text(), ground, depth).unwrap()));
        // out: the same screen as before zen, once the fade is over
        app.zen.leave(t + std::time::Duration::from_millis(10));
        assert_eq!(screen(&mut app), plain);
    }
}

#[cfg(test)]
mod paint_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;

    fn resets(app: &mut App) -> Vec<(u16, u16)> {
        let mut t = Terminal::new(TestBackend::new(120, 36)).unwrap();
        t.draw(|f| draw_frame(app, f)).unwrap();
        let b = t.backend().buffer();
        let mut out = Vec::new();
        for y in 0..36 {
            for x in 0..120 {
                let c = &b[(x, y)];
                if c.bg == Color::Reset || c.fg == Color::Reset {
                    out.push((x, y));
                }
            }
        }
        out
    }

    fn with_agents_and_a_card(app: &mut App) {
        crate::sb::hub_reads::rows_for_tests::apply(
            app,
            vec![crate::sb::hub_reads::rows_for_tests::agent("main", "idle", ""), crate::sb::hub_reads::rows_for_tests::agent("docs", "working", "write the docs")],
            vec![crate::sb::hub_reads::rows_for_tests::card(1, "question", "docs", "v1 or v2 for the api docs?")],
        );
        crate::sb::entries_for_tests::lines(app, "main", &["sb you : ship it", "sb msg : docs → main : found it", "sb card : #1 question @docs : v1 or v2?"]);
    }

    #[test]
    fn no_cell_keeps_the_terminals_background() {
        for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
            crate::theme::set_mode(mode);
            let mut app = crate::sb::bench::test_app();
            with_agents_and_a_card(&mut app);
            assert_eq!(resets(&mut app), vec![], "main, {mode:?}");
            // the card view (ctrl+1)
            sb::key(&mut app, &KeyEvent::new(KeyCode::Char('1'), KeyModifiers::CONTROL), false);
            assert_eq!(resets(&mut app), vec![], "card, {mode:?}");
            // the / popup
            app.ed.text = "/".into();
            app.ed.cursor = 1;
            assert_eq!(resets(&mut app), vec![], "popup, {mode:?}");
            app.ed.text.clear();
            app.ed.cursor = 0;
            // /help
            sb::handle_input(&mut app, "/help");
            assert!(app.help.is_some());
            assert_eq!(resets(&mut app), vec![], "help, {mode:?}");
            app.help = None;
            // inside an agent
            sb::focus(&mut app, "docs");
            assert_eq!(resets(&mut app), vec![], "agent, {mode:?}");
            // every cell with no color of its own is the theme's ground
            let mut t = Terminal::new(TestBackend::new(40, 5)).unwrap();
            t.draw(|f| draw_frame(&mut app, f)).unwrap();
            assert!(t.backend().buffer().content.iter().any(|c| c.bg == crate::theme::bg()));
        }
        crate::theme::set_mode(crate::theme::Mode::Dark);
    }
}
