//! Input of the screen: key, mouse and paste handlers,
//! the composer editor keys, voice keys, and the feed selection copy.

use crate::*;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

/// The feed position under the screen cell, from the last frame (the
/// feed starts at screen row `app.feed_y`: under the Switchboard header).
/// `clamp`: a row below the feed is its last row, at the end; a row
/// above it is none.
pub(crate) fn feed_pos(app: &App, x: u16, y: u16, clamp: bool) -> Option<feedsel::FeedPos> {
    let col = x.saturating_sub(app.feed_x) as usize;
    let y = y.checked_sub(app.feed_y)?;
    let (row, col) = if (y as usize) < app.vis_events.len() {
        (y as usize, col)
    } else if clamp && !app.vis_events.is_empty() {
        (app.vis_events.len() - 1, usize::MAX / 2)
    } else {
        return None;
    };
    Some((app.vis_events[row], *app.vis_rows.get(row)?, col))
}

/// The text of the feed selection (the rows of every event it spans).
pub(crate) fn feed_selection_text(app: &mut App) -> Option<String> {
    let sel = app.feed_sel?;
    let ((e0, r0, c0), (e1, r1, c1)) = sel.range();
    let (debug, w, tick) = (app.debug, app.area_w, app.tick);
    let last = e1.min(app.events.len().saturating_sub(1));
    for i in e0..=last {
        ensure_rows(&app.events, &mut app.cache, i, debug, w, tick);
    }
    let mut rows: Vec<Line<'static>> = Vec::new();
    // per event: the rows taken, the first one, its rows and urls (the copy
    // writes a link's url after its label, links.rs)
    let mut evs: Vec<(usize, usize, &[Line<'static>], &[String])> = Vec::new();
    for i in e0..=last {
        let Some(er) = app.cache.get(i).and_then(|c| c.as_ref()) else { continue };
        let from = if i == e0 { r0 } else { 0 };
        let to = if i == e1 { (r1 + 1).min(er.rows.len()) } else { er.rows.len() };
        let taken = er.rows.get(from..to).unwrap_or(&[]);
        rows.extend(taken.iter().cloned());
        evs.push((taken.len(), from, &er.rows, &er.urls));
    }
    let (rows, to) = crate::links::with_urls(rows, &evs, c0, c1.saturating_add(1));
    Some(feedsel::selection_text(&rows, c0, to))
}

/// The url of the link at column `col` of row `row` of event `i`.
pub(crate) fn feed_link_at(app: &App, i: usize, row: usize, col: usize) -> Option<String> {
    let er = app.cache.get(i)?.as_ref()?;
    crate::links::url_at(&er.rows, &er.urls, row, col)
}

/// Copies to the system clipboard and says so in the status row.
pub(crate) fn copy_text(app: &mut App, text: &str) {
    let note = textlayer::copy_note(text);
    app.flash = Some((note, std::time::Instant::now()));
}

/// Speech-to-text keys (Vibe's text_area._handle_voice_key): Ctrl+R
/// starts, the voice chip at the cursor (BISE-222); while recording any
/// key stops, Ctrl+C / Esc cancel; while transcribing the composer takes
/// the keys again (voice::key_action). `true` when the key was the
/// voice's. `job` resolves the voice model and its key (only when a
/// recording starts).
pub(crate) fn voice_key(
    app: &mut App,
    k: &crossterm::event::KeyEvent,
    job: impl FnOnce() -> Result<voice::VoiceJob, String>,
) -> bool {
    use voice::KeyAction;
    let now = std::time::Instant::now();
    match voice::key_action(app.voice.state(), app.voice.enabled, k.code, k.modifiers) {
        KeyAction::Pass => return false,
        KeyAction::Start => match app.voice.start(job(), now) {
            Ok(()) => {
                app.voice_text.clear();
                crate::attach::insert_live_chip(&mut app.ed);
            }
            // BISE-298: no key is an error with its way out
            Err(m) if m == voice::NEEDS_KEY => {
                push_event(&mut app.events, &mut app.cache, Ev::Err(m));
            }
            Err(m) => {
                push_event(&mut app.events, &mut app.cache, Ev::Warn(m));
            }
        },
        KeyAction::Stop => app.voice.stop(now),
        KeyAction::Cancel => {
            app.voice.cancel();
            end_chip(app, None);
        }
        KeyAction::Swallow => {}
        // BISE-298: voice off and no setup that works: its picker
        KeyAction::OffHint => match job() {
            Ok(_) => app.voice_note = Some((voice::OFF_HINT.into(), now)),
            // after the double ctrl+r window: a second ctrl+r is voice
            // mode, not this picker (voicemode::live::pump opens it)
            Err(_) => app.voice_setup_at = Some(now + crate::voicemode::DOUBLE_CTRL_R),
        },
    }
    true
}

/// The transcription events of this tick: the transcript replaces the
/// voice chip at the end of the clip. A chip gone from the text (a
/// delete, an undo, the history) cancels the transcription; a chip left
/// with no voice at work (voice mode turned off, a restored draft) goes.
pub(crate) fn pump_voice(app: &mut App) {
    let now = std::time::Instant::now();
    for out in app.voice.poll(now) {
        apply_voice(app, out, now);
    }
    let chip = app.ed.mark_at(voice::chip::LABEL).is_some();
    if app.voice.active() && !chip {
        app.voice.cancel();
        end_chip(app, None);
    } else if !app.voice.active() && chip {
        end_chip(app, None);
    }
}

pub(crate) fn apply_voice(app: &mut App, out: voice::VoiceOutput, now: std::time::Instant) {
    let chip = app.ed.mark_at(voice::chip::LABEL).is_some();
    match out {
        // the transcript waits for the end of the clip, in the chip's place
        voice::VoiceOutput::Insert(t) if chip => app.voice_text.push_str(&t),
        voice::VoiceOutput::Insert(t) => {
            // no chip (a test's direct call): at the cursor, a space after a word
            let before = app.ed.cursor.checked_sub(1).and_then(|i| app.ed.text.chars().nth(i));
            let t = if before.is_some_and(|c| !c.is_whitespace())
                && t.starts_with(|c: char| c.is_alphanumeric())
            {
                format!(" {}", t)
            } else {
                t
            };
            app.ed.insert_voice(&t);
            app.popup_sel = 0;
        }
        voice::VoiceOutput::Utterance => {
            let t = std::mem::take(&mut app.voice_text);
            end_chip(app, Some(&t));
            app.ed.break_undo();
        }
        voice::VoiceOutput::Error(m) => {
            end_chip(app, None);
            push_event(&mut app.events, &mut app.cache, Ev::Err(m));
        }
        voice::VoiceOutput::Failed(f) => {
            end_chip(app, None);
            push_event(&mut app.events, &mut app.cache, Ev::Said { glyph: f.glyph, head: f.head, dim: f.dim });
        }
        voice::VoiceOutput::Notice(m) => {
            end_chip(app, None);
            app.voice_note = Some((m, now));
        }
    }
}

/// The voice chip's end: `transcript` replaces it in place (one undo
/// step, the cursor at its end), else it goes and the text around it
/// stays. Nothing when there is no chip.
pub(crate) fn end_chip(app: &mut App, transcript: Option<&str>) {
    app.voice_text.clear();
    let label = voice::chip::LABEL;
    let t = transcript.map(str::trim).unwrap_or("");
    let Some(at) = app.ed.mark_at(label) else {
        app.ed.swap_mark(label, "", 0);
        return;
    };
    let before = at.checked_sub(1).and_then(|i| app.ed.text.chars().nth(i));
    let after = app.ed.text.chars().nth(at + label.chars().count());
    let (with, cursor) = voice::chip::landing(before, t, after);
    app.ed.swap_mark(label, &with, cursor);
    app.popup_sel = 0;
}

/// /voice: voice mode off, or on when its model and key work (BISE-298:
/// `voice is on: <model>`); with no setup that works, the voice picker
/// opens (None: nothing for the feed yet). Saved in bise's prefs.
pub(crate) fn toggle_voice(app: &mut App, job: impl FnOnce() -> Result<voice::VoiceJob, String>) -> Option<Ev> {
    if app.voice.enabled {
        app.voice.enabled = false;
        app.voice.cancel();
        app.voice.drop_kept();
        return Some(saved(voice::save_voice_enabled(false), voice::DISABLED_MESSAGE.into()));
    }
    match job() {
        Ok(j) => {
            app.voice.enabled = true;
            Some(saved(voice::save_voice_enabled(true), format!("{}{}", voice::ENABLED_MESSAGE, j.name)))
        }
        Err(_) => {
            open_voice_setup(app, true);
            None
        }
    }
}

fn saved(r: Result<(), String>, line: String) -> Ev {
    match r {
        Ok(()) => Ev::Info(line),
        Err(e) => Ev::Warn(format!("{} (not saved: {})", line, e)),
    }
}

/// The voice picker over the screen (BISE-298); `turning_on`: esc says
/// voice stays off.
pub(crate) fn open_voice_setup(_app: &mut App, turning_on: bool) {
    crate::onboarding::provider_request(crate::onboarding::Ask {
        open: crate::onboarding::Open::Pick(bise_catalog::roles::VOICE),
        voice_on: turning_on,
        ..Default::default()
    });
}

/// What the voice picker did, in the feed: on (`✓ voice is on: …` and how
/// to talk), or still off.
pub(crate) fn voice_out(app: &mut App, out: crate::onboarding::VoiceOut) {
    match out {
        crate::onboarding::VoiceOut::On(m) => {
            app.voice.enabled = true;
            let head = format!("voice is on: {}.", m);
            let ev = match voice::save_voice_enabled(true) {
                Ok(()) => Ev::Said { glyph: "✓", head, dim: vec![voice::ON_HOW.into()] },
                Err(e) => Ev::Warn(format!("{} (not saved: {})", head, e)),
            };
            push_event(&mut app.events, &mut app.cache, ev);
        }
        crate::onboarding::VoiceOut::Off => {
            push_event(&mut app.events, &mut app.cache, Ev::Info(voice::STAYS_OFF.into()));
        }
    }
}

/// A key for the composer's editor (after the popups and the app keys):
/// moves, selection, deletes, undo/redo, typing, copy/cut; Up/Down move
/// between the visual rows, then through the history from the first and
/// last rows.
pub(crate) fn composer_key(app: &mut App, k: &crossterm::event::KeyEvent) {
    use editor::{Action, Motion};
    let Some(a) = editor::action(k) else { return };
    let w = app.composer.w.max(1);
    match a {
        Action::Up(sel) => {
            if !app.ed.row_up(w, sel) && (sel || !app.ed.history_up(&app.history)) {
                app.ed.move_cursor(Motion::TextStart, sel);
            }
        }
        Action::Down(sel) => {
            if !app.ed.row_down(w, sel) && (sel || !app.ed.history_down(&app.history)) {
                app.ed.move_cursor(Motion::TextEnd, sel);
            }
        }
        // the composer's selection, else the feed's
        Action::Copy => {
            if let Some(t) = app.ed.selected_text().or_else(|| feed_selection_text(app)).or_else(|| crate::diffquote::text(app)) {
                copy_text(app, &t);
            }
        }
        Action::Cut => {
            if let Some(t) = app.ed.cut() {
                copy_text(app, &t);
                app.popup_sel = 0;
            }
        }
        Action::Insert(t) => {
            // BISE-134: typing with a selection in the history quotes it
            // first (quote.rs); the key then types after the chip
            // (not a `/` that starts a command)
            let command = t == "/" && app.ed.text.is_empty();
            match crate::quote::take_selection(app).filter(|_| !command) {
                Some(Ok(chip)) => flash(app, format!("quoted {chip} · backspace on it removes it")),
                Some(Err(e)) => flash(app, e),
                None => {}
            }
            app.ed.insert(&t);
            app.popup_sel = 0;
            // a typed (never a pasted) `:name:` becomes its emoji
            if t == ":" {
                if let Some((text, cur)) = emoji::replace_typed(&app.ed.text, app.ed.cursor) {
                    app.ed.set(&text, cur);
                }
            }
        }
        other => {
            let edits = !matches!(other, Action::Move(..));
            app.ed.apply(&other);
            if edits {
                app.popup_sel = 0;
            }
        }
    }
}

/// A mouse event: help overlay, terminal
/// pane, then the feed (scroll, selection, section toggles) and the
/// composer (cursor, selection).
pub(crate) fn on_mouse(app: &mut App, m: &crossterm::event::MouseEvent, term_h: u16) {
    // BISE-271: a move shows the time of the turn under the mouse; a
    // press, a drag, a scroll put it away
    app.hover = matches!(m.kind, MouseEventKind::Moved).then_some((m.column, m.row));
    // BISE-272: the pointer's shape follows the mouse, pressed or not
    app.pointer_at = Some((m.column, m.row));
    // BISE-290: on any text but the feed's, the composer's and the
    // terminal's, a drag selects and copies, a click on a link opens it;
    // any other click is the screen's own, at the release
    match app.text.on(m, std::time::Instant::now()) {
        textlayer::Out::Pass => on_screen_mouse(app, m, term_h),
        textlayer::Out::Took => {
            app.feed_sel = None;
            app.quote_hint = false;
        }
        textlayer::Out::Copy(t) => copy_text(app, &t),
        textlayer::Out::Open(url) => {
            let note = textlayer::open(app, &url);
            flash(app, note);
        }
        textlayer::Out::Click(press) => {
            on_screen_mouse(app, &press, term_h);
            on_screen_mouse(app, m, term_h);
        }
    }
}

/// A mouse event on the screen's own: help overlay, terminal pane, then
/// the feed (scroll, selection, section toggles) and the composer
/// (cursor, selection).
fn on_screen_mouse(app: &mut App, m: &crossterm::event::MouseEvent, term_h: u16) {
    if help::mouse(app, m) {
        return;
    }
    if approvals_screen::mouse(app, m) {
        return;
    }
    if crate::logview::mouse(app, m) {
        return;
    }
    if crate::artifacts_screen::mouse(app, m) {
        return;
    }
    // the find bar: its chevrons, its ×, its field (find_bar.rs)
    if crate::find_bar::on_mouse(app, m) {
        return;
    }
    if crate::scheduled_screen::mouse(app, m) {
        return;
    }
    if crate::diffview::mouse(app, m) {
        return;
    }
    if crate::computer_use::mouse(app, m) {
        return;
    }
    match app.term.mouse(m, term_h) {
        term::MouseDone::Pass => {}
        term::MouseDone::Took => return,
        term::MouseDone::Copy(t) => return copy_text(app, &t),
    }
    if sb::palette::on_mouse(app, m) {
        return;
    }
    if sb::card_mouse(app, m) {
        return;
    }
    if sb::panel_mouse(app, m) {
        return;
    }
    match m.kind {
        // over a composer taller than its box: its text scrolls, a row
        // per event (a trackpad sends many), the cursor stays put
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
            if app.composer.wheel(m.column, m.row, if m.kind == MouseEventKind::ScrollUp { -1 } else { 1 }) => {}
        MouseEventKind::ScrollUp => {
            app.follow = false;
            app.scroll -= 3;
        }
        MouseEventKind::ScrollDown => {
            if !app.follow {
                app.scroll += 3;
            }
        }
        // click the back-to-bottom bar to return to the tail;
        // click a thinking section to expand/collapse it
        // the composer: a press places the cursor (Shift
        // extends), a drag selects, a double click selects
        // the word, a triple click the whole text; the
        // release copies the selection
        // the copy icon of a code block (codeblock.rs)
        MouseEventKind::Down(MouseButton::Left) if crate::codeblock::click(app, m.column, m.row) => {}
        // the composer's scroll hints: a screenful that way
        MouseEventKind::Down(MouseButton::Left) if app.composer.hint_click(m.column, m.row) => {}
        MouseEventKind::Down(MouseButton::Left)
            if app.composer.hit(&app.ed.text, m.column, m.row, false).is_some() =>
        {
            let ci = app.composer.hit(&app.ed.text, m.column, m.row, false).unwrap_or(0);
            // a click in your message takes the keys back from the find
            // box (BISE-297): it closes, the view stays on the match
            crate::find::close(app);
            let clicks = app.mouse.press(m.column, m.row, std::time::Instant::now());
            match clicks {
                2 => {
                    let (a, b) = editor::word_at(&app.ed.text, ci);
                    app.ed.select_range(a, b);
                }
                3 => app.ed.select_all(),
                _ => app.ed.click(ci, m.modifiers.contains(KeyModifiers::SHIFT)),
            }
            app.mouse.drag = Some(DragIn::Composer);
        }
        MouseEventKind::Drag(MouseButton::Left) if app.mouse.drag == Some(DragIn::Composer) => {
            if let Some(ci) = app.composer.hit(&app.ed.text, m.column, m.row, true) {
                app.ed.click(ci, true);
            }
        }
        MouseEventKind::Up(MouseButton::Left) if app.mouse.drag == Some(DragIn::Composer) => {
            app.mouse.drag = None;
            if let Some(t) = app.ed.selected_text() {
                copy_text(app, &t);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            // BISE-284: the first-run text's `show me what you can do`
            // fills the composer (selected), it does not send
            if let Some(r) = app.demo_rect {
                if r.contains((m.column, m.row).into()) {
                    sb::fill_demo(app);
                    return;
                }
            }
            if let Some(r) = app.bottom_bar_rect {
                let inside = m.column >= r.x
                    && m.column < r.x + r.width
                    && m.row >= r.y
                    && m.row < r.y + r.height;
                if inside {
                    app.follow = true;
                    app.unseen = 0;
                    return;
                }
            }
            // the feed: a press starts a selection (a double
            // click selects the word, a triple the row); the
            // release copies it, or toggles the section when
            // the mouse did not move
            app.quote_hint = false;
            let Some(pos) = feed_pos(app, m.column, m.row, false) else {
                app.feed_sel = None;
                return;
            };
            let clicks = app.mouse.press(m.column, m.row, std::time::Instant::now());
            let row_text = app
                .cache
                .get(pos.0)
                .and_then(|c| c.as_ref())
                .and_then(|c| c.rows.get(pos.1))
                .map(feedsel::line_text)
                .unwrap_or_default();
            let (a, b) = match clicks {
                2 => feedsel::word_cols(&row_text, pos.2),
                3 => (0, row_text.width().saturating_sub(1)),
                _ => (pos.2, pos.2),
            };
            app.feed_sel = Some(feedsel::FeedSel { anchor: (pos.0, pos.1, a), head: (pos.0, pos.1, b) });
            app.mouse.drag = Some(DragIn::Feed { moved: clicks > 1 });
        }
        MouseEventKind::Drag(MouseButton::Left) if matches!(app.mouse.drag, Some(DragIn::Feed { .. })) => {
            // dragging on the top row or below the feed scrolls
            if m.row <= app.feed_y {
                app.follow = false;
                app.scroll -= 1;
            } else if m.row as usize >= app.feed_y as usize + app.area_h && !app.follow {
                app.scroll += 1;
            }
            if let (Some(pos), Some(sel)) = (feed_pos(app, m.column, m.row, true), app.feed_sel.as_mut()) {
                if sel.head != pos {
                    sel.head = pos;
                    app.mouse.drag = Some(DragIn::Feed { moved: true });
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) if matches!(app.mouse.drag, Some(DragIn::Feed { .. })) => {
            let moved = matches!(app.mouse.drag, Some(DragIn::Feed { moved: true }));
            app.mouse.drag = None;
            app.quote_hint = moved;
            if moved {
                if let Some(t) = feed_selection_text(app).filter(|t| !t.is_empty()) {
                    copy_text(app, &t);
                }
                return;
            }
            // a plain click: expand/collapse the section (on the first
            // line of an open fold, its row says which: the fold or the line)
            // a plain click on a link opens it (links.rs); cmd+click is
            // the terminal's own (Ghostty keeps the release)
            let Some((i, row, col)) = app.feed_sel.take().map(|s| s.anchor) else { return };
            if let Some(url) = feed_link_at(app, i, row, col) {
                // a local file opens in your editor (BISE-264)
                let note = textlayer::open(app, &url);
                flash(app, note);
                return;
            }
            crate::feed::toggle_at(&mut app.events, &mut app.cache, i, row);
        }
        _ => {}
    }
}

/// `ctrl+o`: one state for everything folded (thinking, outputs, diffs,
/// reports, briefs, runs of level 3, `▸ why`). Anything closed: open them
/// all; else close them all. New thinking sections follow it.
pub(crate) fn toggle_everything(app: &mut App) {
    let open = crate::feed::anything_closed(&app.events);
    app.show_thinking = open;
    crate::feed::set_everything(&mut app.events, &mut app.cache, open);
}

/// A bracketed paste: the terminal pane, else the composer.
pub(crate) fn on_paste(app: &mut App, text: &str) {
    if app.term.paste(text) {
        return;
    }
    if sb::palette::on_paste(app, text) {
        return;
    }
    if crate::find_bar::on_paste(app, text) {
        return;
    }
    // the paste lands in the composer: it has the keys again
    crate::diffview::give_back(app);
    // normalize CRLF/CR so a terminal paste behaves like the
    // typed newline, then insert at the cursor
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    // a paste that is only image paths (a file dragged into the
    // terminal, iTerm2's "Save to Temp File and Paste Path" on an image)
    // attaches the images; an empty or blank paste (Cmd+V on an image in
    // a terminal that sends one) tries the clipboard image, its chip at
    // the cursor. A text paste never reads the clipboard (osascript is
    // ~0.1 s). Ghostty and kitty send nothing on an image-only clipboard:
    // there Cmd+V reaches us as a key only with a `performable:` binding
    // (the arm below), else Ctrl+V (docs/images.md).
    let attached = if text.trim().is_empty() {
        Some(crate::attach::attach_clipboard(app).map(|l| vec![l]))
    } else {
        crate::attach::on_paste(app, &text)
    };
    match attached {
        Some(Ok(labels)) => flash(app, format!("attached {}", labels.join(" "))),
        Some(Err(e)) if !text.trim().is_empty() => {
            flash(app, format!("image not attached: {e}"));
            app.ed.paste(&text);
        }
        Some(Err(_)) => {}
        // a long text: a chip and an attachment, not a flooded composer
        // (BISE-240, pasted.rs)
        None if crate::pasted::is_long(&text) => {
            let chip = crate::pasted::add(app, &text);
            flash(app, format!("attached {chip}"));
        }
        None => app.ed.paste(&text),
    }
    app.popup_sel = 0;
}

/// A newline in the composer (BISE-276, mdlive.rs): it continues or
/// ends a list, keeps a code line's indentation, else it is typed.
fn newline(app: &mut App) {
    let (a, b) = app.ed.selection().unwrap_or((app.ed.cursor, app.ed.cursor));
    match crate::mdlive::newline(&app.ed.text, a, b) {
        Some(e) => apply_md(app, e),
        None => app.ed.insert("\n"),
    }
}

/// A live-markdown edit: one undo step.
fn apply_md(app: &mut App, e: crate::mdlive::Edit) {
    app.ed.set(&e.text, e.cursor);
    app.ed.anchor = e.anchor.filter(|&a| a != e.cursor);
}

fn flash(app: &mut App, note: String) {
    app.flash = Some((note, std::time::Instant::now()));
}

/// A key event; true when it quits the UI.
pub(crate) fn on_key(app: &mut App, k: &crossterm::event::KeyEvent) -> bool {
    // zen (BISE-128): only the composer's own arms below set it
    app.key_in_composer = false;
    // BISE-290: a key changes the screen: the text's selection goes
    app.text.clear();
    // a cmd+key reached us: the hints say cmd+f from now on (find.rs)
    if k.modifiers.contains(KeyModifiers::SUPER) && !matches!(k.code, KeyCode::Modifier(_)) {
        app.cmd_keys = true;
    }
    // BISE-302: a ctrl+digit only a terminal with them sends: the inbox
    // says ctrl+1 from now on
    if crate::sb::proves_ctrl_digits(k) {
        app.ctrl_digits = true;
    }
    // expired-ux: the ChatGPT sign-in again waits for the browser: c
    // copies its link, esc cancels it (resign.rs)
    if app.help.is_none() && crate::resign::key(app, k) {
        return false;
    }
    if help::on_key(app, k) {
        return false;
    }
    if approvals_screen::on_key(app, k) {
        return false;
    }
    if crate::logview::on_key(app, k) {
        return false;
    }
    if crate::artifacts_screen::on_key(app, k) {
        return false;
    }
    if crate::scheduled_screen::on_key(app, k) {
        return false;
    }
    if crate::computer_use::on_key(app, k) {
        return false;
    }
    if term::on_key(app, k) {
        return false;
    }
    if k.kind != KeyEventKind::Press {
        return false;
    }
    // the diff panel (site/m/artifacts D): ctrl+g opens and closes it;
    // focused, it takes the keys (esc gives them back)
    if crate::diffview::on_key(app, k) {
        return false;
    }
    // voice mode (voicemode/live.rs): its keys first; ctrl+r twice enters
    if crate::voicemode::live::key(app, k) {
        return false;
    }
    if crate::voicemode::live::double_ctrl_r(&mut app.ctrl_r_at, k, std::time::Instant::now()) {
        // the first ctrl+r started dictation (or was to open its
        // picker): it goes, voice mode comes
        app.voice_setup_at = None;
        if app.voice.active() {
            app.voice.cancel();
            end_chip(app, None);
        }
        crate::voicemode::live::request(app);
        return false;
    }
    if voice_key(app, k, voice::resolve_job) {
        return false;
    }
    // cmd+k / ctrl+s: the agent palette takes the keys while it is open
    // (BISE-265)
    if sb::palette::on_key(app, k) {
        return false;
    }
    // ctrl+f: the find field takes the keys while it is open (BISE-237)
    if crate::find_bar::on_key(app, k) {
        return false;
    }
    if app.popup_dismissed.as_deref() != Some(app.ed.text.as_str()) {
        app.popup_dismissed = None;
    }
    let matches = popup_items(app);
    let popup_open = !matches.is_empty();
    let sel = matches.get(app.popup_sel.min(matches.len().saturating_sub(1)));
    if sb::key(app, k, popup_open) {
        return false;
    }
    if at_nav(app, k, sel) {
        return false;
    }
    match (k.code, k.modifiers) {
        // ctrl+c: quit (the agents keep running). A running turn is
        // interrupted by `sb::key` first; a second press quits.
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => return true,
        // ctrl+o: open or close everything folded (book §11, §16)
        (KeyCode::Char('o'), KeyModifiers::CONTROL) => toggle_everything(app),
        // ctrl+y: redo in the composer right after an undo (ctrl+shift+z
        // where the terminal tells it from ctrl+z), else copy the code
        // block under the mouse, else the newest on screen (codeblock.rs)
        (KeyCode::Char('y'), KeyModifiers::CONTROL) if app.ed.can_redo() => {
            app.ed.redo();
            app.popup_sel = 0;
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => crate::codeblock::copy_key(app),
        // space on the item selected in the feed (composer empty; an
        // agent selected in the panel keeps space for its preview)
        (KeyCode::Char(' '), KeyModifiers::NONE) if app.ed.text.is_empty() && app.feed_sel.is_some() => {
            crate::feed::toggle_selected(app);
        }
        // ctrl+l: clear the local feed
        (KeyCode::Char('l'), KeyModifiers::CONTROL) => sb::clear_display(app),
        // esc: close the popup, else drop the selection — it
        // never interrupts (Ctrl+C does, through the flag
        // side-channel)
        (KeyCode::Esc, _) => {
            if sel.is_some_and(|c| c.closable) {
                // close the list, keep the text
                app.popup_dismissed = Some(app.ed.text.clone());
                app.popup_sel = 0;
            } else if popup_open {
                app.ed.clear();
            } else {
                app.ed.anchor = None;
                app.feed_sel = None;
            }
        }
        // scrollback: PgUp/PgDn page, End follows the bottom
        (KeyCode::PageUp, _) => {
            let page = (app.area_h / 2).max(1);
            app.follow = false;
            app.scroll -= page as isize;
        }
        (KeyCode::PageDown, _) => {
            let page = (app.area_h / 2).max(1);
            if !app.follow {
                app.scroll += page as isize;
            }
        }
        // End back to the tail when scrolled up, else the
        // line end (the editor)
        (KeyCode::End, KeyModifiers::NONE) if !app.follow => {
            app.follow = true;
            app.unseen = 0;
        }
        // approvals-design.md §8.1: backspace at the start of a list
        // item's text outdents it (shift+tab switches the mode)
        (KeyCode::Backspace, KeyModifiers::NONE)
            if sel.is_none() && app.ed.anchor.is_none() && crate::mdlive::outdent_at_start(&app.ed.text, app.ed.cursor).is_some() =>
        {
            if let Some(e) = crate::mdlive::outdent_at_start(&app.ed.text, app.ed.cursor) {
                apply_md(app, e);
                app.key_in_composer = true;
            }
        }
        (KeyCode::Tab, _) | (KeyCode::BackTab, _) => {
            let out = k.code == KeyCode::BackTab || k.modifiers.contains(KeyModifiers::SHIFT);
            if let Some(c) = sel {
                // popup completion
                pick(app, c);
            } else if out {
                // approvals-design.md §8.1: in the composer shift+tab always
                // switches yolo ↔ auto (a list item outdents with backspace)
                crate::sb::toggle_approvals(app);
            } else if let Some(e) = crate::mdlive::indent(&app.ed.text, app.ed.cursor, app.ed.anchor, false) {
                // BISE-276: Tab on a list item: one level in
                apply_md(app, e);
                app.key_in_composer = true;
            } else if app.pending {
                // BISE-89 (after Codex): the draft waits in the TUI for
                // the end of the turn, shown above the composer; nothing
                // goes to the hub before
                crate::queue::push(app);
            }
        }
        // a newline in the composer: Shift+Enter (needs
        // the kitty keyboard protocol), Ctrl+J (LF, the one
        // binding EVERY terminal transmits), or alt+enter;
        // plain Enter sends
        // ctrl+v: attach the clipboard image (terminals paste text
        // only); cmd+v arrives here only when the terminal did not paste
        // (Ghostty `keybind = performable:super+v=paste_from_clipboard`
        // passes it through on an image-only clipboard)
        (KeyCode::Char('v'), KeyModifiers::CONTROL) | (KeyCode::Char('v'), KeyModifiers::SUPER) => match crate::attach::attach_clipboard(app) {
            Ok(l) => flash(app, format!("attached {l}")),
            Err(e) => flash(app, e),
        },
        (KeyCode::Enter, KeyModifiers::SHIFT)
        | (KeyCode::Char('j'), KeyModifiers::CONTROL)
        | (KeyCode::Enter, KeyModifiers::ALT) => {
            newline(app);
            app.key_in_composer = true;
        }
        (KeyCode::Enter, _) => {
            if let Some(c) = sel {
                if let Some(v) = c.run.clone() {
                    app.ed.take();
                    handle_input(app, &v);
                } else {
                    pick(app, c);
                }
            } else if app.ed.text.trim().is_empty() && !app.pending && app.sb.waits_for_sign_in() {
                // expired-ux: the agent in view stopped on the expired
                // ChatGPT sign-in: ⏎ signs in again (the key bar says it)
                crate::resign::start(app);
            } else if crate::mdlive::enter_makes_newline(&app.ed.text, app.ed.cursor) {
                // BISE-276: in a code block ⏎ is a newline (you never
                // send half a block): close it with ``` to send
                newline(app);
                app.key_in_composer = true;
            } else if let Some(model) = crate::attach::refused_images(app) {
                // BISE-150: the catalog says this model reads no images:
                // the no-vision line now, the message stays in the composer
                push_event(&mut app.events, &mut app.cache, Ev::Err(crate::attach::no_vision_error(&model)));
                app.follow = true;
            } else if let Some(w) = sb::archived_refusal(app, &app.ed.text) {
                // an archived agent reads nothing: the message stays in
                // the composer, its commands (/restore) still run
                push_event(&mut app.events, &mut app.cache, Ev::Warn(w));
                app.follow = true;
            } else {
                let v = app.ed.take().trim().to_string();
                let v = if v.starts_with('/') { v } else { crate::attach::expand(app, &v) };
                app.follow = true;
                app.unseen = 0;
                if !v.is_empty() {
                    // codex semantics: while the agent works,
                    // Enter STEERS the running turn; at idle it
                    // starts one. Commands pass through. The
                    // explicit "say" keeps text that starts
                    // with a protocol word ("reload ce
                    // fichier", "compact la fonction") a
                    // message, never a command.
                    let line = if v.starts_with('/') {
                        v
                    } else if app.pending {
                        format!("steer {}", v)
                    } else {
                        format!("say {}", v)
                    };
                    handle_input(app, &line);
                }
            }
        }
        // BISE-89: ↑ in an empty composer edits the newest queued
        // message (the history comes after the queue)
        (KeyCode::Up, KeyModifiers::NONE) if !popup_open && app.ed.text.is_empty() && !app.queued.is_empty() => {
            crate::queue::pop_last(app);
            app.key_in_composer = true;
        }
        // the popup takes the plain arrows
        (KeyCode::Up | KeyCode::Down, KeyModifiers::NONE) if popup_open => {
            // BISE-301: past the rows that pick nothing (`/model`'s
            // provider headers, a note)
            let down = k.code == KeyCode::Down;
            let inert = |i: usize| matches.get(i).is_some_and(|c| c.run.is_none() && c.fill == app.ed.text && !c.folder);
            let mut sel = popup_step(app.popup_sel, matches.len(), down);
            for _ in 0..matches.len() {
                if !inert(sel) {
                    break;
                }
                sel = popup_step(sel, matches.len(), down);
            }
            app.popup_sel = sel;
        }
        _ => {
            composer_key(app, k);
            app.key_in_composer = true;
        }
    }
    false
}

/// The selection one row down (or up) in a popup of `len` rows,
/// wrapping; 0 when the list is empty or the selection is stale.
pub(crate) fn popup_step(sel: usize, len: usize, down: bool) -> usize {
    match len {
        0 => 0,
        _ if sel >= len => 0,
        _ if down => (sel + 1) % len,
        _ => sel.checked_sub(1).unwrap_or(len - 1),
    }
}

/// The folder keys of the `@` popup: → on a folder row browses it (the
/// popup stays open on its entries); ← or Backspace on `@dir/` goes one
/// folder up. True when the key was taken.
fn at_nav(app: &mut App, k: &crossterm::event::KeyEvent, sel: Option<&PopItem>) -> bool {
    match (k.code, k.modifiers) {
        (KeyCode::Right, KeyModifiers::NONE) => match sel {
            Some(c) if c.folder => {
                pick(app, c);
                true
            }
            _ => false,
        },
        (KeyCode::Left | KeyCode::Backspace, KeyModifiers::NONE) => match commands::at_up(app) {
            Some((text, cursor)) => {
                app.ed.set(&text, cursor);
                app.popup_sel = 0;
                true
            }
            None => false,
        },
        _ => false,
    }
}

/// Take the popup entry `c` into the composer (a picked path ranks first
/// in the next `@` searches; a folder is browsed, not picked).
fn pick(app: &mut App, c: &PopItem) {
    // an artifact (site/m/artifacts C): the `@token` goes, its chip comes
    if let Some((id, _)) = c.path.as_deref().and_then(crate::artifacts::parse_url) {
        app.ed.set(&c.fill, c.fill_cursor);
        let title = crate::artifacts::get(&id).map_or(id.clone(), |a| a.title);
        crate::attach::insert_artifact(app, &id, &title);
        app.popup_sel = 0;
        return;
    }
    if let Some(p) = c.path.as_ref().filter(|_| !c.folder) {
        files::picked(p);
        // an image is attached, not inserted as a path
        match crate::attach::pick_image(app, p) {
            Some(Ok(l)) => {
                flash(app, format!("attached {l} {}", crate::attach::file_name(p)));
                app.popup_sel = 0;
                return;
            }
            Some(Err(e)) => flash(app, format!("image not attached: {e}")),
            None => {}
        }
    }
    app.ed.set(&c.fill, c.fill_cursor);
    app.popup_sel = 0;
}

#[cfg(test)]
mod keys_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn thinking(open: bool) -> Ev {
        Ev::Thinking { ms: 10, text: "hm".into(), open }
    }

    fn report(open: bool) -> Ev {
        let text = "[report: done] p95 at 180 ms.\n- ran 3 times";
        Ev::AgentMsg { from: "bench".into(), to: String::new(), text: text.into(), level: 3, id: "m_3".into(), open, fold: false }
    }

    fn opens(app: &App) -> Vec<bool> {
        app.events
            .iter()
            .map(|e| match e {
                Ev::Thinking { open, .. } | Ev::AgentMsg { open, .. } => *open,
                _ => true,
            })
            .collect()
    }

    fn press(app: &mut App, code: KeyCode, m: KeyModifiers) {
        on_key(app, &KeyEvent::new(code, m));
    }

    /// ctrl+o (book §16): one state for everything folded. Anything
    /// closed: all open; again: all closed; new thinking follows.
    #[test]
    fn ctrl_o_opens_then_closes_everything_folded() {
        let mut app = crate::sb::bench::test_app();
        app.events = vec![thinking(true), report(false), Ev::Info("x".into()), thinking(false)];
        app.cache = (0..4).map(|_| None).collect();
        press(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert_eq!(opens(&app), vec![true, true, true, true]);
        assert!(app.show_thinking);
        press(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert_eq!(opens(&app), vec![false, false, true, false]);
        assert!(!app.show_thinking);
        // ctrl+t is gone (no alias): nothing changes
        press(&mut app, KeyCode::Char('t'), KeyModifiers::CONTROL);
        assert_eq!(opens(&app), vec![false, false, true, false]);
    }

    /// space on the item selected in the feed toggles it, only with an
    /// empty composer; with text it is a space.
    #[test]
    fn space_toggles_the_selected_feed_item() {
        let mut app = crate::sb::bench::test_app();
        app.events = vec![thinking(false), report(false)];
        app.cache = (0..2).map(|_| None).collect();
        app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (1, 0, 0), head: (1, 0, 2) });
        press(&mut app, KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(opens(&app), vec![false, true]);
        assert_eq!(app.ed.text, "");
        app.ed.text = "hi".into();
        app.ed.cursor = 2;
        press(&mut app, KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(opens(&app), vec![false, true]);
        assert_eq!(app.ed.text, "hi ");
    }
}
