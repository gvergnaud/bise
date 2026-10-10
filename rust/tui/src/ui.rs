//! The screen (book §8 "The frame"): the history, the divider, the
//! composer pane, the popup and the key bar, drawn from `App` each frame.

use crate::*;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    let full = app.term.draw(frame, frame.area());
    // book §8 "The frame": the frame, the history and the panel, the
    // divider, the composer pane
    let cols = crate::layout::cols(full.width, full.height);
    // the diff panel (site/m/artifacts D) takes the agents panel's
    // place, wider; under its width the whole screen
    let cols = match app.diff.is_some() && crate::diffview::side(full.width) {
        true => crate::layout::with_side(cols, full.width, crate::diffview::side_w(full.width)),
        false => cols,
    };
    let rows = crate::layout::rows(full.width, full.height);
    draw_bise(app, frame, full, cols, rows);
    approvals_screen::draw(app, frame);
    crate::logview::draw(app, frame);
    crate::diffview::draw_full(app, frame);
    crate::artifacts_screen::draw(app, frame);
    crate::scheduled_screen::draw(app, frame);
    computer_use::draw(app, frame);
    help::draw(app, frame);
}

/// The Switchboard screen (book §8 "The frame", §13 "The composer
/// pane"). Top down: the header (the frame's top edge, or its own row
/// when bare) and 1 blank row; the history on the reading column (its
/// first row pinned to the "inside an agent" / preview line when there
/// is one), the agents panel on its right behind the rule; the card box;
/// 1 blank row; the divider `you → main … state`; the queued messages
/// (1 blank row above them from 20 rows),
/// the images strip; the composer (bar at the margin, text 3 columns
/// after it from 60 columns, else 2, 1 blank bar row above and under the text from 20 rows); the key bar; the frame's
/// bottom edge.
fn draw_bise(app: &mut App, frame: &mut Frame, area: Rect, cols: crate::layout::Cols, rows: crate::layout::Rows) {
    // the composer's text: the column less its lead columns and its
    // right margin (BISE-228: more room from 60 columns)
    let (lead, right) = composer_pad(area.width);
    let inner_w = (cols.col_w.min(cols.pane_w) as usize).saturating_sub((lead + right) as usize).max(1);
    // the voice chip is in the text (BISE-222): no columns of its own
    let text_w = inner_w;
    // the find box floats over the history (BISE-297): the composer
    // keeps its rows and its draft
    let composer_rows = {
        let r = editor::layout_input(&app.ed.text, text_w);
        editor::drawn_rows(&r, app.ed.cursor) as u16
    };
    // the rows that are always there: header and gap, the blank row and
    // the divider, the composer's bar rows, the key bar (its own row
    // from 14 rows) and the frame's bottom edge
    let keys_h = u16::from(!rows.keys_in_divider);
    let edge_h = u16::from(cols.framed);
    let fixed = rows.body + 2 + rows.pad_top + rows.pad_bottom + keys_h + edge_h;
    // what is left keeps a 3-row history
    let left = |used: u16| area.height.saturating_sub(fixed + used + 3);
    let text_rows = composer_rows.clamp(rows.min_text, rows.max_text).min(left(0).max(1));
    // a text taller than the composer with no blank bar row for the
    // scroll hint has its ↑ / ↓ in the right margin: under 60 columns
    // (no margin) it wraps 1 column earlier meanwhile (designer)
    let margin_hint = rows.pad_top == 0 || attach::strip_height(app) > 0;
    let text_w = if right == 0 && margin_hint && composer_rows > text_rows && inner_w > 1 { inner_w - 1 } else { text_w };
    // the agent palette (BISE-265): its list grows the pane upward, past
    // the composer's cap, the history keeps its 3 rows
    let text_rows = if sb::palette::is_open(app) { sb::palette::rows_wanted(app).min(left(0).max(1)) } else { text_rows };
    // the attachments box (book §13) and 1 blank row between it and the
    // divider (from 20 rows; the composer's blank bar row moves there:
    // your message starts right under the box), then the queued
    // messages (BISE-89), with 1 blank tinted row between them and the
    // divider (BISE-224, user request; from 20 rows like the other pads)
    let strip_h = attach::strip_height(app).min(left(text_rows));
    let strip_gap = if strip_h > 0 { rows.pad_top.min(left(text_rows + strip_h)) } else { 0 };
    let pad_top = if strip_h > 0 { 0 } else { rows.pad_top };
    let queue_h = crate::queue::height(app).min(left(text_rows + strip_h + strip_gap));
    let queue_gap = if queue_h > 0 { rows.pad_top.min(left(text_rows + strip_h + strip_gap + queue_h)) } else { 0 };
    let pane_h = text_rows + strip_h + strip_gap + queue_h + queue_gap;
    // the inbox's box (designer's round 2 A): what the rest leaves, with
    // 1 blank row above it from 24 rows, from the gutter (1 column left
    // of the composer's bar, 2 from the frame at least) to the feed
    // area's edge;
    // an open item at most half the feed area; none full screen
    sb::card_frame(app);
    let box_x = cols.x0.saturating_sub(1).max(cols.feed_x.saturating_sub(1));
    let box_w = (cols.feed_x + cols.feed_w).min(area.width).saturating_sub(box_x);
    let room = left(pane_h + 1);
    let fit = sb::BoxFit { screen_h: area.height, room, half: (room + 3) / 2 };
    let card_h = sb::box_height(app, fit, box_w);
    let card_gap = u16::from(card_h > 0 && area.height >= 24 && left(pane_h + 1 + card_h) > 0);
    // the no-vision line names the model of the agent in view
    attach::set_model(&crate::sb::focus_model(app));
    // voice mode (plan §4.6): the pane takes the composer's place, half
    // the screen (the lanes under 30 rows); tab shows the composer again
    let voice = app.voice_mode.as_ref().map(|vm| (vm.view(app.frame_at), vm.typing()));
    let pane_view = voice.as_ref().filter(|(_, typing)| !typing).map(|(v, _)| v.clone());
    let composer_h = match &pane_view {
        Some(_) => crate::voicemode::pane::height(area.height).min(left(0).max(1)),
        None => pad_top + text_rows + rows.pad_bottom,
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(rows.body),   // header, 1 blank row
            Constraint::Min(1),              // history | panel
            Constraint::Length(card_gap),    // a blank row above the strip
            Constraint::Length(card_h),      // the cards' strip
            Constraint::Length(1),           // a blank row above the divider
            Constraint::Length(1),           // the divider
            Constraint::Length(queue_gap),   // a tinted row above the queue
            Constraint::Length(queue_h),     // the queued messages
            Constraint::Length(strip_gap),   // a tinted row above the box
            Constraint::Length(strip_h),     // the attachments box
            Constraint::Length(composer_h),  // the composer: bar rows, its text
            Constraint::Length(keys_h),      // the key bar
            Constraint::Length(edge_h),      // the frame's bottom edge
        ])
        .split(area);
    let (body, card, divider_y) = (chunks[1], chunks[3], chunks[5].y);
    // the composer pane: from the reading column's x to the right margin
    let pane_end = (cols.margin + cols.pane_w).min(area.width);
    let pane = |r: Rect| Rect { x: area.x + cols.x0, width: pane_end.saturating_sub(cols.x0), ..r };
    let short = cols.panel.is_none();
    let words = crate::ctrlhint::words(app);
    // the working count's gust: 5 cells, 3 or 1 as the screen narrows (book §9)
    let gust = crate::gust::mark(app.motion_away, crate::gust::header_size(area.width));
    // the frame (or the bare header row)
    let sb = &app.sb;
    if cols.framed {
        let title = sb.title();
        let counts = |room: usize| voice_header(voice.as_ref().map(|(v, _)| v), room, |room| sb.summary(room, short, words, &gust));
        let edge = |room: usize| sb.edge(room, words, counts);
        chrome::draw_frame(frame.buffer_mut(), area, cols, title, edge, divider_y);
        crate::textlayer::text(Rect { height: 1, ..area }); // BISE-290: the title row
    } else if chunks[0].height > 0 {
        let r = Rect { height: 1, ..chunks[0] };
        let mut head = sb.header(r.width, short, words, &gust);
        if let Some((v, _)) = &voice {
            let mut spans = crate::voicemode::pane::header(v);
            spans.push(Span::raw("  "));
            spans.append(&mut head.spans);
            head.spans = spans;
        }
        frame.render_widget(Paragraph::new(head), r);
        crate::textlayer::text(r); // BISE-290
    }
    // the panel: from the history's first row down to the blank row
    // above the divider
    if let Some(p) = cols.panel {
        let h = divider_y.saturating_sub(body.y + 1);
        let r = Rect { x: area.x + p.x, width: p.w, y: body.y, height: h }.intersection(area);
        if app.diff.is_some() {
            crate::diffview::draw_side(app, frame, r);
        } else {
            sb::draw_panel(app, frame, r);
        }
    }
    // the history: from the column's x to the feed area's right edge
    // (tables and code may run there)
    let feed_right = cols.feed_x + cols.feed_w;
    let mut feed = Rect {
        x: area.x + cols.x0,
        width: feed_right.saturating_sub(cols.x0).min(area.width.saturating_sub(cols.x0)),
        ..body
    };
    if let Some(l) = app.sb.feed_banner() {
        // the pinned line wraps in the column (2 rows at most)
        let rows: Vec<Line> = crate::wrap_line(l, cols.col_w.min(feed.width).max(1) as usize).into_iter().take(2).collect();
        let n = rows.len() as u16;
        if feed.height > n + 2 {
            frame.render_widget(Paragraph::new(rows), Rect { height: n, ..feed });
            crate::textlayer::text(Rect { height: n, ..feed }); // BISE-290
            feed = Rect { y: feed.y + n + 1, height: feed.height - n - 1, ..feed };
        }
    }
    // the scrollbar's column: the panel's rule, else the frame's right
    // edge; bare, the feed area's last column
    let bar = cols.framed.then(|| {
        let x = cols.panel.and_then(|p| p.rule).unwrap_or(area.width.saturating_sub(1));
        Rect { x: area.x + x, width: 1, ..feed }
    });
    // the item full screen (ctrl+o) takes the history's place, in the column
    let view = sb::card_view_open(app);
    if view {
        let r = Rect { x: area.x + cols.x0, width: cols.col_w.min(area.width.saturating_sub(cols.x0)), ..body };
        let r = Rect { height: divider_y.saturating_sub(r.y + 1).min(r.height), ..r };
        sb::draw_card_view(app, frame, r);
        app.vis_events.clear();
        app.vis_rows.clear();
    } else if feed.width > 1 && feed.height > 0 {
        // a screen too small for a feed: nothing to draw (a scrollbar on
        // an empty area panics)
        draw_feed(app, frame, feed, bar);
    } else {
        app.vis_events.clear();
        app.vis_rows.clear();
    }
    // no agents yet, nothing said: the first-run text, dim, in the feed
    let first_run = app.sb.first_run().filter(|_| !view);
    // it goes with your first message; rows before it (the setup's, a
    // note) stay above it
    // the suggestion under the mouse: the place it had in the last frame
    let demo_before = app.demo_rect.take();
    let demo_hovered = demo_before.zip(app.pointer_at).is_some_and(|(r, (x, y))| r.contains((x, y).into()));
    if let Some(text) = first_run.filter(|_| !app.events.iter().any(|e| matches!(e, Ev::You(..)))) {
        let w = (cols.col_w as usize).saturating_sub(3).max(1);
        let mut lines: Vec<Line> = Vec::new();
        // BISE-284: the last line on one row: its suggestion clickable
        // (where in `lines`, from which column), or `⏎ try it` while the
        // composer holds it
        let mut demo_at: Option<(u16, u16)> = None;
        for (i, p) in text.iter().enumerate() {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            if i == text.len() - 1 {
                if let Some((line, at)) = first_run_last(app, p, w, demo_hovered) {
                    demo_at = at.map(|x| (lines.len() as u16, x));
                    lines.push(line);
                    continue;
                }
            }
            lines.extend(wrap_words(p, w).into_iter().map(|l| Line::from(Span::styled(l, Style::default().fg(dim())))));
        }
        // the block at 2/5 of the free rows (the onboarding's optical
        // center, designer); under 12 rows, on top after 1 blank row
        let h = (lines.len() as u16).min(feed.height);
        let y = if feed.height < 12 { 1.min(feed.height - h) } else { (feed.height - h) * 2 / 5 };
        // under the rows the feed drew, one blank row between
        let used = app.vis_events.len() as u16;
        let y = if used > 0 { y.max(used + 1) } else { y };
        let fits = y + h <= feed.height;
        let r = Rect {
            x: feed.x + 3,
            y: feed.y + y,
            width: cols.col_w.saturating_sub(3).min(feed.width.saturating_sub(3)),
            height: feed.height.saturating_sub(y),
        };
        if fits {
            frame.render_widget(Paragraph::new(lines), r);
            crate::textlayer::text(Rect { height: h, ..r }); // BISE-290
            if let Some((row, x)) = demo_at.filter(|&(row, _)| row < r.height) {
                let demo = Rect { x: r.x + x, y: r.y + row, width: sb::DEMO.width() as u16, height: 1 }.intersection(r);
                app.demo_rect = Some(demo);
                crate::pointer::region(demo, crate::pointer::Shape::Pointer);
            }
        }
    }
    // the find bar (find_bar.rs): flush in the history pane's top-right
    // corner, from the row under the frame's top edge, its right edge
    // the column left of the panel's rule (or of the frame's edge; bare,
    // left of the scrollbar)
    if app.find.is_some() && !view {
        let top = area.y + 1;
        let right = bar.map_or(feed.right().saturating_sub(1), |b| b.x);
        let left = area.x + u16::from(cols.framed);
        let pane = Rect { x: left, y: top, width: right.saturating_sub(left), height: feed.bottom().saturating_sub(top) };
        crate::find_bar::draw(app, frame, pane, feed);
    }
    // the raised pane (book §13, BISE-212): the grey fills the inside of
    // the frame, from the row under the divider to the row above the
    // bottom edge, edge to edge between the side edges; the lines (the
    // divider with its labels, the side and bottom edges) stay on the
    // ground, outside the grey. Bare: the full width down to the last row.
    let (tx, tw) = if cols.framed { (area.x + 1, area.width.saturating_sub(2)) } else { (area.x, area.width) };
    let tint = Rect {
        x: tx,
        y: divider_y + 1,
        width: tw,
        height: area.bottom().saturating_sub(divider_y + 1 + edge_h),
    }
    .intersection(area);
    frame.buffer_mut().set_style(tint, Style::default().bg(theme::raised()));
    // the divider: who you talk to, what it does (was the status row); on
    // a short screen the key bar takes the state's place
    let (name, state) = divider_text(app);
    let working = sb::viewed_working(app);
    let who = sb::viewed_who(app);
    let state = if rows.keys_in_divider {
        vec![crate::keybar::line(app, chrome::divider_room(area.width, cols, &name, &who, working.as_ref())).spans]
    } else {
        state
    };
    // the card view: `you → ? perf · your answer`, then a fresh note
    // (`✓ copied 9 chars`: the card's text copies too, BISE-290/302)
    let card_label = sb::card_divider_label(app).map(|mut l| {
        if let Some(t) = fresh_note(&app.flash) {
            l.push(Span::styled(" ✓ ", Style::default().fg(accent())));
            l.push(Span::styled(t, Style::default().fg(text())));
            l.push(Span::raw(" "));
        }
        l
    });
    // voice mode: `you ⇄ main · voice mode · headphones`
    let voice_label = voice.as_ref().map(|(v, _)| crate::voicemode::pane::divider(v));
    let (state_rect, label_rect) = match card_label.or_else(|| sb::palette::divider_label(app)).or(voice_label) {
        Some(label) => chrome::draw_divider_label(frame.buffer_mut(), area, cols, divider_y, label),
        None => chrome::draw_divider(frame.buffer_mut(), area, cols, divider_y, &name, &who, working.as_ref(), state),
    };
    // BISE-290: the divider's words (who, the notes) select and copy
    crate::textlayer::text(Rect { y: divider_y, height: 1, ..area }.intersection(area));
    app.bottom_bar_rect = (!app.tail_visible && !rows.keys_in_divider).then_some(state_rect);
    if let Some(r) = app.bottom_bar_rect {
        crate::pointer::region(r, crate::pointer::Shape::Pointer);
    }
    // the queued messages: ` › text`, the `›` under the composer's bar
    let queue = pane(chunks[7]);
    if queue.height > 0 {
        let r = Rect { x: queue.x.saturating_sub(1), width: queue.width + 1, ..queue }.intersection(area);
        frame.render_widget(Paragraph::new(crate::queue::lines(app, r.width as usize)), r);
        crate::textlayer::text(r); // BISE-290
    }
    // the attachments box: from the bar's column (from 60 columns, the
    // composer's text column), at most the composer's width; no bar:
    // the bar marks your message
    let shift = if lead > TEXT_AT { lead } else { 0 };
    let strip = pane(chunks[9]);
    if strip.height > 0 {
        let strip = Rect { x: strip.x + shift, width: strip.width.saturating_sub(shift), ..strip };
        let r = Rect { width: (inner_w as u16 + lead - shift).min(strip.width), ..strip };
        frame.render_widget(Paragraph::new(attach::strip_lines(app, r.width as usize)), r);
        crate::textlayer::text(r); // BISE-290
    }
    // the composer: its bar at x0 on every row (the blank bar rows
    // around the text too), the text from x0 + `lead`
    let body_rect = pane(chunks[10]);
    let composer = Rect { width: (inner_w as u16 + lead).min(body_rect.width), ..body_rect };
    if let Some(v) = &pane_view {
        crate::voicemode::pane::draw(frame.buffer_mut(), body_rect, v, app.pulse_ms);
        crate::textlayer::text(body_rect); // BISE-290: the captions select and copy
    } else if sb::palette::is_open(app) {
        sb::palette::draw(app, frame, Rect { height: composer_h.saturating_sub(rows.pad_bottom), ..composer }, inner_w, lead, pad_top.min(composer_h));
        let bar = Span::styled("│", Style::default().fg(accent()));
        for y in composer.y + composer_h.saturating_sub(rows.pad_bottom)..composer.y + composer_h {
            let r = Rect { y, height: 1, ..composer }.intersection(frame.area());
            if !r.is_empty() {
                frame.render_widget(Paragraph::new(Line::from(bar.clone())), r);
            }
        }
    } else {
        draw_composer(app, frame, composer, text_w, lead, pad_top.min(composer_h), rows.pad_bottom);
    }
    let text = Rect { y: app.composer.y, height: app.composer.h as u16, ..body_rect };
    // zen (BISE-121) keeps the composer's text, the divider's label and
    // the card box as they are, and the history you read (BISE-132): the
    // feed area, from the history's first row to the divider
    let typed = Rect { x: app.composer.x, y: app.composer.y, width: app.composer.w as u16, height: app.composer.h as u16 };
    let history = Rect { x: area.x + cols.feed_x, width: cols.feed_w, y: body.y, height: divider_y.saturating_sub(body.y) };
    app.zen.keep = vec![typed.intersection(area), label_rect, history.intersection(area)];
    if card_h > 0 {
        let r = Rect { x: area.x + box_x, width: box_w, ..card };
        sb::draw_box(app, frame, r, fit);
        app.zen.keep.push(r.intersection(area));
    }
    if app.find.is_none() && !sb::palette::is_open(app) && pane_view.is_none() {
        draw_popup(app, frame, text);
    }
    // the key bar, from x0 to the right margin (from 60 columns, from
    // the composer's text column)
    let kb = pane(chunks[11]);
    let kb = Rect { x: kb.x + shift, width: kb.width.saturating_sub(shift), ..kb };
    if kb.height > 0 {
        let keys = match &pane_view {
            Some(v) => crate::voicemode::pane::keys(v, kb.width),
            None => crate::keybar::line(app, kb.width),
        };
        frame.render_widget(Paragraph::new(keys), kb);
        crate::textlayer::text(kb); // BISE-290
    }
    // last: the "type to ask about it" popup over a selection in the history
    crate::quote::draw_hint(app, frame);
}

/// The divider's text: the name of the agent you talk to, and on the
/// right what it does (the old status row): back to the bottom while
/// scrolled up, a fresh voice or flash note, else its state.
fn divider_text(app: &App) -> (String, Vec<Vec<Span<'static>>>) {
    let name = app.sb.focus.clone();
    let d = Style::default().fg(dim());
    let state = if !app.tail_visible {
        let mut spans = vec![
            Span::styled(format!("{} back to the bottom", theme::glyph("↓")), Style::default().fg(text())),
            Span::styled(" · end", d),
        ];
        if app.unseen > 0 {
            spans.push(Span::styled(format!(" · {} new lines", app.unseen), Style::default().fg(text())));
        }
        vec![spans]
    } else if app.sb.reload_wait.waiting(std::time::Instant::now()) {
        vec![vec![Span::styled(sb::reload_wait::NOTE, d)]]
    } else if let Some(t) = fresh_note(&app.voice_note) {
        vec![vec![Span::styled(format!("{} ", theme::glyph("●")), Style::default().fg(accent())), Span::styled(t, Style::default().fg(text()))]]
    } else if let Some(t) = fresh_note(&app.flash) {
        // ASCII: no mark (its `v` reads as a letter, not a check)
        let mark = if theme::ascii_mode() { "" } else { "✓ " };
        vec![vec![Span::styled(mark, Style::default().fg(accent())), Span::styled(t, Style::default().fg(text()))]]
    } else {
        sb::status_state(app).into_iter().map(|l| l.spans).collect()
    };
    (name, state)
}

/// `s` cut at spaces into rows of at most `w` columns.
fn wrap_words(s: &str, w: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ') {
        if !cur.is_empty() && cur.width() + 1 + word.width() > w {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    out.push(cur);
    out
}

/// The history in `area`; the scrollbar thumb in `bar` (framed: on the
/// panel's rule or the frame's edge), else in the area's last column.
fn draw_feed(app: &mut App, frame: &mut Frame, area: Rect, bar: Option<Rect>) {
    // ---- feed: cached wrapped rows, only the VISIBLE slice rendered ----
    // (a Paragraph over the whole history re-wraps everything each frame
    // and lags long sessions). The column keeps one column of margin on
    // each edge: the history never touches the screen border, and the
    // scrollbar gets its own gutter.
    // the rect starts at the reading column's x (book §8); its last
    // column is the scrollbar's
    let feed_w = (area.width as usize).saturating_sub(usize::from(bar.is_none())).max(1);
    let area_w = feed_w;
    // event-wake (designer's page): the agent in view waits for an event:
    // one live dim line under its last row, `… waiting for cargo test ·
    // 2m` (its own row, kept off the feed's rows), a blank row above it
    // (designer m_17730)
    let waiting = app.sb.waiting_words(crate::when::now_ms()).filter(|_| area.height > 3);
    let area_h = (area.height as usize).saturating_sub(if waiting.is_some() { 2 } else { 0 });
    let text_area = Rect {
        x: area.x,
        y: area.y,
        width: feed_w as u16,
        height: area.height,
    }
    // a 1..3-column feed: the margins leave no room (never outside it)
    .intersection(area);

    let n = app.events.len();
    if app.cache.len() < n {
        app.cache.resize_with(n, || None);
    }
    // main's replies carry `:*` in main's feed only (BISE-15)
    crate::render::set_main_feed(app.sb.is_main_focus());
    // a message this feed's owner received names it in the `to` column (BISE-90)
    crate::render::set_feed_owner(app.sb.focus_name());
    let (debug, tick) = (app.debug, app.tick);
    macro_rules! rows_of {
        () => {
            &mut |i: usize| ensure_rows(&app.events, &mut app.cache, i, debug, area_w, tick)
        };
    }
    // find (BISE-237): its share of the scan, the view to its match
    crate::find::step(app, area_w, crate::find::BUDGET);
    let down = app.scroll > 0;
    // a scroll puts the "type to ask about it" popup away (quote.rs)
    if app.scroll != 0 {
        app.quote_hint = false;
    }
    let mut anchor = if app.follow {
        bottom_anchor(n, area_h, rows_of!())
    } else {
        move_anchor(app.anchor, app.scroll, n, rows_of!())
    };
    app.scroll = 0;
    // the rows from the anchor down; fewer than the screen: the bottom
    let mut vis: Vec<Line> = Vec::with_capacity(area_h + 2);
    let mut vis_events: Vec<usize> = Vec::with_capacity(area_h + 2);
    let mut vis_rows: Vec<usize> = Vec::with_capacity(area_h + 2);
    let mut tail_visible = true;
    // voice mode: the reply being said, lit (its rows restyled each frame)
    let lit = lit_event(app);
    let mut lit_rows: Option<Vec<Line<'static>>> = None;
    for pass in 0..2 {
        vis.clear();
        vis_events.clear();
        vis_rows.clear();
        tail_visible = true;
        let (mut i, mut skip) = anchor;
        while i < n {
            ensure_rows(&app.events, &mut app.cache, i, debug, area_w, tick);
            let mut rows = app.cache[i].as_ref().map(|c| &c.rows[..]).unwrap_or(&[]);
            if let Some((_, l)) = lit.as_ref().filter(|(li, _)| *li == i) {
                rows = lit_rows.get_or_insert_with(|| crate::voicemode::pane::lit::light(rows, &l.text, &l.spans));
            }
            for (ri, r) in rows.iter().enumerate().skip(skip) {
                if vis.len() >= area_h {
                    tail_visible = false;
                    break;
                }
                // the feed selection on the selection background
                let line = match app.feed_sel.and_then(|s| s.cols(i, ri)) {
                    Some((a, b)) => feedsel::highlight(r, a, b, theme::selection_bg()),
                    None => r.clone(),
                };
                // the matches of the find field (BISE-237)
                match app.find.as_ref().and_then(|f| f.marker()) {
                    Some(m) => vis.push(m.paint(line, i, ri)),
                    None => vis.push(line),
                }
                vis_events.push(i);
                vis_rows.push(ri);
            }
            skip = 0;
            if !tail_visible {
                break;
            }
            i += 1;
        }
        // a full screen that shows the last row is the tail too
        if pass == 0 && vis.len() < area_h && anchor != (0, 0) {
            anchor = bottom_anchor(n, area_h, rows_of!());
            continue;
        }
        break;
    }
    if tail_visible && down && !app.follow {
        app.follow = true;
        app.unseen = 0;
    }
    let rows_shown = vis.len();
    frame.render_widget(Paragraph::new(Text::from(vis)), text_area);
    if let Some(w) = waiting.filter(|_| tail_visible) {
        let y = text_area.y + rows_shown as u16 + 1;
        let line = Line::from(Span::styled(format!("{} {}", theme::G_WAITING, w), Style::default().fg(theme::dim())));
        let r = Rect { y, height: 1, ..text_area }.intersection(area);
        frame.render_widget(Paragraph::new(line), r);
    }
    // the visible links, for the OSC 8 of the backend (links.rs)
    for (y, (&i, &ri)) in vis_events.iter().zip(&vis_rows).enumerate() {
        let Some(er) = app.cache.get(i).and_then(|c| c.as_ref()) else { continue };
        if er.urls.is_empty() {
            continue;
        }
        for (a, b, k) in crate::links::row_links(&er.rows, &er.urls, ri) {
            let x0 = text_area.x as usize + a;
            let x1 = (text_area.x as usize + b).min(text_area.right() as usize);
            if x0 >= x1 {
                continue;
            }
            crate::links::push_hit(crate::links::Hit {
                y: text_area.y + y as u16,
                x0: x0 as u16,
                x1: x1 as u16,
                tag: (k % 127 + 1) as u8,
                url: er.urls[k].clone(),
                id: format!("bise{}-{}", i, k),
            });
        }
    }

    // BISE-272: the hand over the rows a click opens or closes
    for (y, (&i, &ri)) in vis_events.iter().zip(vis_rows.iter()).enumerate() {
        if crate::feed::toggles_at(&app.events, &app.cache, i, ri) {
            let r = Rect { y: text_area.y + y as u16, height: 1, ..text_area };
            crate::pointer::region(r.intersection(frame.area()), crate::pointer::Shape::Pointer);
        }
    }

    if let Some((x, y)) = app.hover {
        hover_time(app, frame.buffer_mut(), text_area, &vis_events, (x, y));
    }
    // the copy icon of the code block under the mouse (codeblock.rs)
    crate::codeblock::draw(app, frame.buffer_mut(), text_area, &vis_events, &vis_rows);

    // the scrollbar (BISE-90, main's call): only while scrolled up from
    // the bottom, faint, one column, no arrows; never at the tail
    if !tail_visible && n > 0 {
        // the scrollbar counts events, not rows: the rows of the whole
        // history are never summed
        let shown = vis_events.last().map_or(1, |l| l + 1 - anchor.0.min(*l));
        let pos = if tail_visible { n - 1 } else { anchor.0 };
        let mut state = ScrollbarState::new(n)
            .position(pos)
            .viewport_content_length(shown);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(None)
                .thumb_symbol(theme::glyph("┃"))
                .thumb_style(Style::default().fg(if bar.is_some() { dim() } else { crate::theme::faint() })),
            bar.unwrap_or(area).intersection(frame.area()),
            &mut state,
        );
    }
    app.anchor = anchor;
    app.vis_events = vis_events;
    app.vis_rows = vis_rows;
    app.feed_x = text_area.x;
    app.feed_y = text_area.y;
    app.area_w = area_w;
    app.area_h = area_h;
    app.tail_visible = tail_visible;
}

/// BISE-271: the mouse over a turn (its reply, the row that ends it):
/// when it ended, dim, right-aligned in the history's column, on the
/// hovered row, else on the nearest row of the same event whose end is
/// blank. Drawn over the frame: nothing moves; no blank room, nothing.
fn hover_time(app: &App, buf: &mut ratatui::buffer::Buffer, area: Rect, vis_events: &[usize], (x, y): (u16, u16)) {
    if x < area.x || x >= area.right() || y < area.y {
        return;
    }
    let row = (y - area.y) as usize;
    let Some(&i) = vis_events.get(row) else { return };
    let Some(ts) = crate::feed::turn_end_of(&app.events, i) else { return };
    let label = crate::when::ended_now(ts);
    let w = label.width() as u16;
    // one blank column before the label
    if w + 1 >= area.width {
        return;
    }
    let x0 = area.right() - w;
    let blank = |buf: &ratatui::buffer::Buffer, r: usize| {
        (x0 - 1..area.right()).all(|cx| buf.cell((cx, area.y + r as u16)).is_none_or(|c| c.symbol() == " "))
    };
    // the rows of this event on screen, the hovered one first, then
    // the nearest
    let mut rows: Vec<usize> = (0..vis_events.len()).filter(|&r| vis_events[r] == i).collect();
    rows.sort_by_key(|&r| r.abs_diff(row));
    let Some(r) = rows.into_iter().find(|&r| blank(buf, r)) else { return };
    buf.set_string(x0, area.y + r as u16, &label, Style::default().fg(dim()));
}

/// The header's summary in voice mode (design §0): `● voice mode 2:14`
/// first, in every view, then the summary in the room left (it goes
/// whole when there is none).
fn voice_header(v: Option<&crate::voicemode::PaneView>, room: usize, summary: impl Fn(usize) -> Vec<Span<'static>>) -> Vec<Span<'static>> {
    use unicode_width::UnicodeWidthStr;
    let Some(v) = v else {
        return summary(room);
    };
    let mut head = crate::voicemode::pane::header(v);
    let w: usize = head.iter().map(|s| s.content.width()).sum();
    if w > room {
        return summary(room);
    }
    let rest = summary(room.saturating_sub(w + 3));
    let rest_w: usize = rest.iter().map(|s| s.content.width()).sum();
    if !rest.is_empty() && w + 3 + rest_w <= room {
        head.push(Span::styled(" · ", Style::default().fg(crate::theme::faint())));
        head.extend(rest);
    }
    head
}

/// The rows of the event the voice is saying, lit in step (plan §4.6):
/// the last reply of the agent in view whose text is `lit.text`.
fn lit_event(app: &App) -> Option<(usize, crate::voicemode::Lit)> {
    let lit = app.voice_mode.as_ref()?.lit()?;
    if lit.agent != app.sb.focus_name() {
        return None;
    }
    let i = app.events.iter().rposition(|e| matches!(e, Ev::Assistant(t) if *t == lit.text))?;
    Some((i, lit))
}

/// A status note (flash, voice) still worth showing: younger than 2 s.
fn fresh_note(note: &Option<(String, std::time::Instant)>) -> Option<String> {
    note.as_ref()
        .filter(|(_, at)| at.elapsed() < Duration::from_secs(2))
        .map(|(t, _)| t.clone())
}

/// The composer's typed text as drawn rows, `inner` columns wide, at
/// most `text_rows` of them (scrolled to keep the cursor row in view;
/// sets `app.composer.top`).
/// The voice chip's frame at `now` (BISE-222): the phase, the live
/// levels, the timer, the clock's pulse time (held in zen); still when
/// the gust is (`BISE_REDUCE_MOTION`, a slow draw, no focus). The draw
/// passes the frame's time (`app.frame_at`, run::frame_clock).
pub(crate) fn voice_look(app: &App, now: std::time::Instant) -> voice::chip::Look {
    voice::chip::Look {
        phase: if app.voice.state() == voice::VoiceState::Flushing {
            voice::chip::Phase::Transcribing
        } else {
            voice::chip::Phase::Recording
        },
        levels: app.voice.levels(),
        secs: app.voice.clip_len(now).as_secs(),
        ms: app.pulse_ms,
        still: matches!(app.motion, crate::gust::Motion::Still),
    }
}

fn typed_lines(app: &mut App, inner: usize, text_rows: usize) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    // rows by display width (emojis are 2 columns); the cursor is
    // the REVERSED grapheme, or a REVERSED space on a newline or at
    // the end of the text; the selection has the selection colors
    let rows = editor::layout_input(&app.ed.text, inner);
    let drawn = editor::drawn_rows(&rows, app.ed.cursor);
    let cursor = app.ed.cursor;
    let selection = app.ed.selection();
    let (cur_row, _) = editor::row_col(&rows, cursor);
    // taller than the box: the view scrolls the least that keeps the
    // cursor row visible when the cursor or the text changed, else stays
    // where the wheel left it
    let now = (cursor, app.ed.text.len(), inner);
    let follow = app.composer.seen != now;
    let top = editor::view_top(app.composer.top, drawn, text_rows, cur_row, follow);
    app.composer.top = top;
    app.composer.seen = now;
    app.composer.total = drawn;
    let text_style = Style::default().fg(theme::text());
    let voice_look = voice_look(app, app.frame_at);
    let voice_form = voice::chip::Form::now();
    // BISE-276: the live markdown of the rows shown (mdlive.rs): one
    // style per char, the selection's tint over it
    let shown = || rows.iter().take(drawn).skip(top).flatten();
    let lo = shown().map(|c| c.ci).min().unwrap_or(0);
    let hi = shown().map(|c| c.ci + c.text.chars().count().max(1)).max().unwrap_or(lo);
    let md = crate::mdlive::styles(&app.ed.text, lo, hi, &mut app.md_cache);
    let style_at = |ci: usize, sel: bool| {
        let st = ci.checked_sub(lo).and_then(|j| md.get(j)).copied().unwrap_or(text_style);
        if sel { st.bg(theme::selection_bg()) } else { st }
    };
    for row in rows.iter().take(drawn).skip(top) {
        let mut spans: Vec<Span> = Vec::new();
        let mut buf = String::new();
        let mut buf_style = text_style;
        for cell in row {
            let n = cell.text.chars().count().max(1);
            let is_cursor = if cell.newline {
                cell.ci == cursor
            } else {
                cell.ci <= cursor && cursor < cell.ci + n
            };
            let in_sel = selection.is_some_and(|(a, b)| a <= cell.ci && cell.ci < b);
            if cell.chip {
                // a chip `▣ N` / `❝ N` (atomic): its own spans
                if !buf.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut buf), buf_style));
                }
                // the pill ` ❝ 1 ` (BISE-205): selected or under the
                // cursor, the whole pill shows it
                let mut over = Style::default();
                if in_sel {
                    over = over.bg(theme::selection_bg());
                }
                if is_cursor {
                    over = over.add_modifier(Modifier::REVERSED);
                }
                if attach::is_voice(cell.text) {
                    spans.extend(voice::chip::spans(&voice_look, voice::chip::fit(inner), voice_form, over));
                } else {
                    spans.extend(attach::chip_pill(cell.text, over).into_iter().filter(|s| !s.content.is_empty()));
                }
                continue;
            }
            let cell_style = style_at(cell.ci, in_sel);
            if is_cursor || buf_style != cell_style {
                if !buf.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut buf), buf_style));
                }
                buf_style = cell_style;
            }
            if is_cursor {
                // a pending dead key (Option+e…): its accent, marked,
                // before the cursor, like macOS; on a full row (no
                // column left) it takes the cursor cell instead, so
                // the row never overflows and the cursor stays seen
                let marked = Style::default().fg(theme::accent()).add_modifier(Modifier::UNDERLINED);
                let row_w: usize = row.iter().map(|c| c.w).sum();
                match app.ed.pending_dead() {
                    Some(acc) if row_w + 1 > inner => spans.push(Span::styled(
                        acc.to_string(),
                        marked.add_modifier(Modifier::REVERSED),
                    )),
                    acc => {
                        if let Some(acc) = acc {
                            spans.push(Span::styled(acc.to_string(), marked));
                        }
                        spans.push(Span::styled(
                            cell.text.to_string(),
                            cell_style.add_modifier(Modifier::REVERSED),
                        ));
                    }
                }
            } else if !cell.newline {
                buf.push_str(cell.text);
            } else if in_sel {
                // a selected newline shows as one selected blank
                buf.push(' ');
            }
        }
        if !buf.is_empty() {
            spans.push(Span::styled(buf, buf_style));
        }
        out.push(Line::from(spans));
    }
    out
}

/// Where the composer's text starts from its bar under 60 columns:
/// x0 + 3, the history's text column (BISE-108, user request: more room
/// after the bar).
const TEXT_AT: u16 = 3;

/// From this width the composer pane gets its padding (BISE-228).
const ROOMY_FROM: u16 = 60;

/// The composer's lead (from its bar to its text) and right margin (from
/// the wrap to the pane's edge) on a `width`-column screen (book §13,
/// BISE-228, user request: ~8 px more room each side of what you type;
/// designer: the text, the attachments box and the key bar at x0 + 4,
/// 2 columns before the wrap). Under 60 columns, 3 and 0 as before:
/// every column counts there.
pub(crate) fn composer_pad(width: u16) -> (u16, u16) {
    if width >= ROOMY_FROM {
        (TEXT_AT + 1, 2)
    } else {
        (TEXT_AT, 0)
    }
}

/// The Switchboard composer (book §8 "The frame", §13): a bar `│` at
/// the area's column 0 on every row (faint while empty, accent with text
/// or while recording), `pad_top` / `pad_bottom` blank bar rows around
/// the text, the text from the area's column `lead` wrapped at `inner` columns and scrolled with the cursor row
/// in view. Empty: the cursor at column `lead` and the dim placeholder.
/// Recording or transcribing: the voice chip in the text (BISE-222).
fn draw_composer(app: &mut App, frame: &mut Frame, area: Rect, inner: usize, lead: u16, pad_top: u16, pad_bottom: u16) {
    let text_y = area.y + pad_top.min(area.height.saturating_sub(1));
    let text_rows = (area.height.saturating_sub(pad_top + pad_bottom) as usize).max(1);
    let text_w = inner.max(1);
    // the scroll and what it followed carry over from the last frame
    app.composer = ComposerArea {
        x: area.x + lead,
        y: text_y,
        w: text_w,
        h: text_rows,
        pane: (area.x, area.y, area.width, area.height),
        ..app.composer
    };
    // BISE-272: the text cursor where a click places yours (`ComposerArea::hit`)
    let typed = Rect { x: app.composer.x.saturating_sub(1), y: text_y, width: text_w as u16 + 1, height: text_rows as u16 };
    crate::pointer::region(typed.intersection(frame.area()), crate::pointer::Shape::Text);
    let empty = app.ed.is_empty();
    // the find box has the keys (BISE-297): the draft as it is, no caret
    let finding = app.find.is_some();
    // the diff panel has the keys (designer m_7291): no caret, the draft
    // dim, the placeholder says where the keys are
    let diff_keys = crate::diffview::has_keys(app);
    let rows = if empty {
        let note = if diff_keys { crate::diffview::COMPOSER_NOTE.to_string() } else { sb::placeholder(app).unwrap_or_default() };
        let caret = if finding || diff_keys { Style::default() } else { Style::default().fg(text()).add_modifier(Modifier::REVERSED) };
        let mut spans = vec![Span::styled(" ", caret)];
        if !note.is_empty() {
            spans.push(Span::styled(format!(" {}", note), Style::default().fg(dim())));
        }
        vec![Line::from(spans)]
    } else if sb::setup::masked(app) {
        // the key card (BISE-245): a key is never shown
        typed_lines(app, text_w, text_rows)
            .into_iter()
            .map(|l| {
                Line::from(
                    l.spans
                        .into_iter()
                        .map(|s| Span::styled(s.content.chars().map(|c| if c == ' ' { ' ' } else { '•' }).collect::<String>(), s.style))
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    } else if finding {
        typed_lines(app, text_w, text_rows)
            .into_iter()
            .map(|l| Line::from(l.spans.into_iter().map(|s| Span::styled(s.content, s.style.remove_modifier(Modifier::REVERSED))).collect::<Vec<_>>()))
            .collect()
    } else if diff_keys {
        typed_lines(app, text_w, text_rows)
            .into_iter()
            .map(|l| {
                Line::from(l.spans.into_iter().map(|s| Span::styled(s.content, s.style.remove_modifier(Modifier::REVERSED).fg(dim()))).collect::<Vec<_>>())
            })
            .collect()
    } else {
        typed_lines(app, text_w, text_rows)
    };
    let bar_st = Style::default().fg(composer_bar_color(app));
    let bar = Span::styled(format!("│{}", " ".repeat(lead.saturating_sub(1) as usize)), bar_st);
    // a text taller than the box (designer): `↑ 4 lines above` /
    // `↓ 2 lines below`, dim, in the blank bar rows; without them (under
    // 20 rows) a dim ↑ / ↓ right of the first / last text row
    let (above, below) = if empty { (0, 0) } else { app.composer.more() };
    let hint_st = Style::default().fg(dim());
    let pad_line = |n: usize, up: bool| {
        let mut spans = vec![bar.clone()];
        if n > 0 {
            spans.push(Span::styled(composer_more(n, up), hint_st));
        }
        Line::from(spans)
    };
    let mut lines: Vec<Line> = Vec::with_capacity(area.height as usize);
    for i in 0..pad_top.min(area.height) {
        lines.push(pad_line(if i + 1 == pad_top { above } else { 0 }, true));
    }
    let mut body = rows.into_iter();
    for _ in 0..text_rows {
        let mut spans = vec![bar.clone()];
        if let Some(l) = body.next() {
            spans.extend(l.spans);
        }
        lines.push(Line::from(spans));
    }
    let mut first_pad_bottom = true;
    while lines.len() < area.height as usize {
        lines.push(pad_line(if first_pad_bottom && pad_bottom > 0 { below } else { 0 }, false));
        first_pad_bottom = false;
    }
    frame.render_widget(Paragraph::new(lines), area);
    // no blank row for the words: the arrow in the right margin
    // (`draw_bise` keeps a column for it under 60 columns)
    let x = area.x + lead + text_w as u16;
    let fa = frame.area();
    let last = text_y + text_rows as u16 - 1;
    for (n, pad, y, g) in [(above, pad_top, text_y, "↑"), (below, pad_bottom, last, "↓")] {
        if n > 0 && pad == 0 && x < fa.right() && y < fa.bottom() {
            frame.buffer_mut().set_string(x, y, g, hint_st);
        }
    }
}

/// The composer's scroll hint: `↑ 4 lines above`, `↓ 1 line below`.
pub(crate) fn composer_more(n: usize, up: bool) -> String {
    let (g, way) = if up { ("↑", "above") } else { ("↓", "below") };
    format!("{} {} {} {}", g, n, if n == 1 { "line" } else { "lines" }, way)
}

/// The composer's bar: accent as soon as there is text, an image or a
/// recording; a quiet line (`rule`) only when the composer is empty.
fn composer_bar_color(app: &App) -> ratatui::style::Color {
    if app.ed.is_empty() && !app.voice.active() && attach::strip_height(app) == 0 {
        crate::theme::rule()
    } else {
        accent()
    }
}

fn draw_popup(app: &App, frame: &mut Frame, prompt: Rect) {
    // ---- slash-command popup (OpenCode autocomplete: split border,
    // backgroundMenu, primary selection)
    let matches = popup_items(app);
    if !matches.is_empty() {
        // `/archive`'s and `/restore`'s question, dim, above the rows
        let title = crate::commands::popup_title(app);
        let n = matches.len().min(8) as u16 + u16::from(title.is_some());
        // wide enough for the widest line, whole (QA 6), within the prompt
        let need = matches
            .iter()
            .map(|c| c.mark.map(|(g, _)| g.width() + 1).unwrap_or(0) + c.label.width() + c.desc.width() + 6)
            .max()
            .unwrap_or(0);
        let min = if matches[0].closable { 72 } else { 56 };
        let w = (need.max(min) as u16).min(prompt.width);
        let sel_i = app.popup_sel.min(matches.len() - 1);
        let top = popup_top(sel_i, matches.len(), 8);
        let area = Rect {
            x: prompt.x,
            y: prompt.y.saturating_sub(n + 2),
            width: w,
            height: n + 2,
        }
        // a short terminal: the prompt sits near the top, the popup
        // would hang below the screen (ratatui panics outside its buffer)
        .intersection(frame.area());
        frame.render_widget(Clear, area);
        // BISE-272: over what it covers, no click does anything
        crate::pointer::region(area, crate::pointer::Shape::Default);
        let head = title.map(|t| Line::from(Span::styled(format!(" {t}"), Style::default().fg(theme::dim()))));
        let lines: Vec<Line> = head
            .into_iter()
            .chain(matches
            .iter()
            .enumerate()
            .skip(top)
            .take(8)
            .map(|(i, c)| {
                let sel = i == sel_i;
                let (name_style, desc_style) = if sel {
                    (
                        Style::default()
                            .bg(theme::accent())
                            .fg(theme::on_accent())
                            .add_modifier(Modifier::BOLD),
                        Style::default().bg(theme::accent()).fg(theme::on_accent()),
                    )
                } else if c.run.is_none() && c.fill == app.ed.text && !c.folder && c.desc.is_empty() {
                    // a row that picks nothing (`/model`'s provider headers,
                    // BISE-301): dim, so the rows you pick stay what you read
                    (Style::default().fg(theme::dim()), Style::default().fg(theme::dim()))
                } else {
                    (Style::default().fg(theme::text()), Style::default().fg(theme::dim()))
                };
                let mut spans = Vec::new();
                if let Some((g, color)) = c.mark {
                    let st = if sel {
                        Style::default().bg(theme::accent()).fg(theme::on_accent())
                    } else {
                        Style::default().fg(color)
                    };
                    spans.push(Span::styled(format!(" {}", g), st));
                }
                // columns, not chars: an emoji mark is 2 columns wide
                let mark_w = c.mark.map(|(g, _)| g.width() + 1).unwrap_or(0);
                // a long path keeps its end (the file name) in view
                let label = truncate_left(&c.label, (w as usize).saturating_sub(mark_w + 4));
                spans.push(Span::styled(format!(" {} ", label), name_style));
                let room = (w as usize).saturating_sub(label.width() + mark_w + 6);
                spans.push(Span::styled(truncate_chars(&c.desc, room), desc_style));
                Line::from(spans)
            }))
            .collect();
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT)
                    .border_set(SPLIT)
                    .border_style(Style::default().fg(theme::rule()))
                    .style(Style::default().bg(Color::Reset)),
            ),
            area,
        );
        // BISE-290: its text selects, copies and has links (between the rules)
        crate::textlayer::text(Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area });
    }
}

/// `s` in at most `max` columns: its end, after a `…` when cut.
pub(crate) fn truncate_left(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let mut out: Vec<char> = Vec::new();
    let mut used = 1; // the `…`
    for ch in s.chars().rev() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + cw > max {
            break;
        }
        used += cw;
        out.push(ch);
    }
    if max == 0 {
        return String::new();
    }
    std::iter::once('…').chain(out.into_iter().rev()).collect()
}

/// The first-run text's last line, when it fits on one row of `w`
/// columns (BISE-284): `try: "show me what you can do"`, the sentence
/// underlined in the accent under the mouse, and the column where it
/// starts; `⏎ try it · or just type your own` while the composer holds it.
fn first_run_last(app: &App, text: &str, w: usize, hovered: bool) -> Option<(Line<'static>, Option<u16>)> {
    let dim = Style::default().fg(dim());
    if sb::demo_ready(app) {
        let (key, words) = sb::DEMO_READY;
        if key.width() + words.width() > w {
            return None;
        }
        let key = Span::styled(key, Style::default().fg(accent()).add_modifier(Modifier::BOLD));
        return Some((Line::from(vec![key, Span::styled(words, dim)]), None));
    }
    let at = text.find(sb::DEMO)?;
    if text.width() > w {
        return None;
    }
    let (head, tail) = (&text[..at], &text[at + sb::DEMO.len()..]);
    let x = head.width() as u16;
    let demo = if hovered {
        Style::default().fg(accent()).add_modifier(Modifier::UNDERLINED)
    } else {
        dim
    };
    let line = Line::from(vec![
        Span::styled(head.to_string(), dim),
        Span::styled(sb::DEMO, demo),
        Span::styled(tail.to_string(), dim),
    ]);
    Some((line, Some(x)))
}
