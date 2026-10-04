//! Ask about this (BISE-134, book §13 "Ask about a selection"): text
//! selected in the history becomes context for the agent.
//!
//! Select text in the history (the release copies it, as before), then
//! type: the first typed key puts the selection in the composer as a
//! quote chip `❝ 1` at the composer's cursor, like an image (BISE-207),
//! and ends the selection; the key then types right after the chip. The strip above the composer lists
//! it: `❝ 1 “first words of the selection…”  main · 3 lines`; a
//! backspace on the chip removes it, like an image. Several selections,
//! several quotes: at most [`MAX`].
//!
//! On send, each quote still in the text leaves it and goes in front of
//! the message as one tag, the text after them:
//!
//! ```text
//! <selection from="main">
//! the selected text
//! </selection>
//! what does this mean?
//! ```
//!
//! `from`: who wrote the selected lines (the agent in view, `you`, the
//! agent a message came from), several joined with `, `. The history
//! draws each tag as one dim line over your text: `❝ the selected
//! text… · main · 3 lines`.
//!
//! A quote is an [`Attachment`](crate::attach::Attachment): its label
//! is `[Quote #N]`, its marker the tag. So the drafts file, the queue
//! and the per-view drafts keep it like an image, with no new state.

use crate::app::App;
use crate::attach::Attachment;

/// Quotes in one message, at most.
pub(crate) const MAX: usize = 4;
/// A quote keeps at most this many characters of the selection.
pub(crate) const MAX_CHARS: usize = 8_000;
/// How the label of a quote starts (`[Quote #2]`).
pub(crate) const OPEN: &str = "[Quote #";

const TAG_OPEN: &str = "<selection from=\"";
const TAG_CLOSE: &str = "</selection>";

/// The label of quote `n`.
pub(crate) fn label(n: usize) -> String {
    format!("{OPEN}{n}]")
}

pub(crate) fn is_quote(label: &str) -> bool {
    label.starts_with(OPEN)
}

/// The tag a quote sends: `<selection from="main">\n{text}\n</selection>`.
/// A `</selection>` inside the text is broken (`</selection >`) so the
/// tag always ends where it should; a `"` in `from` becomes `'`.
#[cfg(test)]
pub(crate) fn tag(from: &str, text: &str) -> String {
    tag_at(from, &Where::default(), text)
}

/// Where in a diff a quote comes from (diffquote.rs): the file, the
/// lines in the new file and in the old one (`192-194`; empty: none).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Where {
    pub(crate) file: String,
    pub(crate) new: String,
    pub(crate) old: String,
}

/// The tag of a quote from a diff: `<selection from="t1 vs main"
/// file="src/a.rs" new="12-14" old="11-12">`, an attribute only when
/// it has a value.
pub(crate) fn tag_at(from: &str, at: &Where, text: &str) -> String {
    let attr = |s: &str| s.replace('"', "'").replace('\n', " ");
    let mut head = format!("{TAG_OPEN}{}\"", attr(from));
    for (k, v) in [("file", &at.file), ("new", &at.new), ("old", &at.old)] {
        if !v.is_empty() {
            head.push_str(&format!(" {k}=\"{}\"", attr(v)));
        }
    }
    let text = text.replace(TAG_CLOSE, "</selection >");
    format!("{head}>\n{text}\n{TAG_CLOSE}")
}

/// A quote read back from its tag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Quote {
    pub(crate) from: String,
    pub(crate) text: String,
    /// from a diff: where (else all empty)
    pub(crate) at: Where,
}

/// The tag at the start of `s` (spaces and newlines before it allowed):
/// the quote and the rest after it.
fn parse_one(s: &str) -> Option<(Quote, &str)> {
    let s = s.trim_start();
    let rest = s.strip_prefix(TAG_OPEN)?;
    let (head, rest) = rest.split_once("\">")?;
    if head.contains('\n') {
        return None;
    }
    // `from` up to its quote, then ` key="value"` pairs
    let (from, mut attrs) = head.split_once('"').unwrap_or((head, ""));
    let mut at = Where::default();
    while !attrs.trim_start().is_empty() {
        let (k, v) = attrs.trim_start().split_once("=\"")?;
        let (v, more) = v.split_once('"').unwrap_or((v, ""));
        match k {
            "file" => at.file = v.to_string(),
            "new" => at.new = v.to_string(),
            "old" => at.old = v.to_string(),
            _ => {}
        }
        attrs = more;
    }
    let (text, rest) = rest.split_once(TAG_CLOSE)?;
    let text = text.strip_prefix('\n').unwrap_or(text);
    let text = text.strip_suffix('\n').unwrap_or(text);
    Some((Quote { from: from.to_string(), text: text.to_string(), at }, rest))
}

/// The tags in front of a message, and its text after them (the newline
/// after the last tag dropped). No tag: none, the whole message.
pub(crate) fn split(msg: &str) -> (Vec<Quote>, &str) {
    let mut out = Vec::new();
    let mut rest = msg;
    while let Some((q, r)) = parse_one(rest) {
        out.push(q);
        rest = r;
    }
    if out.is_empty() {
        return (out, msg);
    }
    (out, rest.strip_prefix('\n').unwrap_or(rest))
}

/// Lines in `text` (a last empty line not counted; at least 1).
pub(crate) fn line_count(text: &str) -> usize {
    text.trim_end_matches('\n').split('\n').count().max(1)
}

/// The start of the quote on one line: its words, newlines as spaces,
/// at most `max` columns (`…` when cut).
pub(crate) fn preview(text: &str, max: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.width() <= max {
        return flat;
    }
    let mut out = String::new();
    let mut used = 1; // the `…`
    for ch in flat.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > max {
            break;
        }
        used += w;
        out.push(ch);
    }
    format!("{}…", out.trim_end())
}

/// What the strip and the history say after the words: `main · 3 lines`.
/// The words the strip and the history preview: a diff's lines without
/// their marks (` `, `-`, `+`), else the text.
pub(crate) fn words(q: &Quote) -> String {
    if q.at.file.is_empty() {
        return q.text.clone();
    }
    q.text.lines().map(|l| l.get(1..).unwrap_or("")).collect::<Vec<_>>().join("\n")
}

/// From a diff, the place instead of the speaker (designer m_7568):
/// `src/quote.rs:192-194 · 3 lines`, `src/a.rs:40-41 · 2 removed lines`.
pub(crate) fn about(q: &Quote) -> String {
    let n = line_count(&q.text);
    let s = if n == 1 { "" } else { "s" };
    match (&q.at.file, &q.at.new, &q.at.old) {
        (f, _, _) if f.is_empty() => format!("{} · {} line{}", q.from, n, s),
        (f, new, _) if !new.is_empty() => format!("{}:{} · {} line{}", f, new, n, s),
        (f, _, old) if !old.is_empty() => format!("{}:{} · {} removed line{}", f, old, n, s),
        (f, _, _) => format!("{} · {} line{}", f, n, s),
    }
}

/// The quote an attachment holds (a quote's marker is its tag).
pub(crate) fn of(a: &Attachment) -> Option<Quote> {
    if !is_quote(&a.label) {
        return None;
    }
    parse_one(&a.marker).map(|(q, _)| q)
}

/// The quote chips in `text` (first char, past it, N).
pub(crate) fn chips(text: &str) -> Vec<(usize, usize, usize)> {
    crate::attach::find_labels(text, OPEN)
}

/// The selected text as a quote in the composer: its chip at the cursor
/// ([`crate::attach::insert_chip`]), one undo step. `Err`: why not (a
/// flash). The text is trimmed and cut to [`MAX_CHARS`].
pub(crate) fn add(app: &mut App, from: &str, text: &str) -> Result<String, String> {
    add_at(app, from, &Where::default(), text)
}

/// [`add`] for a quote from a diff: its tag says where.
pub(crate) fn add_at(app: &mut App, from: &str, at: &Where, text: &str) -> Result<String, String> {
    let text = text.trim_matches('\n').trim_end();
    if text.trim().is_empty() {
        return Err("nothing selected".into());
    }
    if chips(&app.ed.text).len() >= MAX {
        return Err(format!("{MAX} quotes at most: backspace on one to make room"));
    }
    let text: String = match text.char_indices().nth(MAX_CHARS) {
        Some((b, _)) => format!("{}…", &text[..b]),
        None => text.to_string(),
    };
    // the lowest free number, shared with images and pastes (BISE-240)
    let n = crate::attach::next_number(app);
    let l = label(n);
    app.attachments.push(Attachment { label: l.clone(), marker: tag_at(from, at, &text), info: Default::default() });
    crate::attach::insert_chip(&mut app.ed, &l);
    Ok(crate::attach::chip_name(&l))
}

/// Who wrote the events `from..=to` of the feed in view: `you`, the
/// sender of an agent message, else the agent in view; in order, once
/// each, joined with `, `.
pub(crate) fn speakers(app: &App, from: usize, to: usize) -> String {
    use crate::wire::Ev;
    let mut who: Vec<String> = Vec::new();
    for ev in app.events.iter().take(to.saturating_add(1)).skip(from) {
        let w = match ev {
            Ev::You(..) | Ev::Undelivered { .. } => "you".to_string(),
            Ev::AgentMsg { from, .. } if !from.is_empty() => from.trim_start_matches('@').to_string(),
            Ev::TimeMark(_) | Ev::Idle | Ev::Turn | Ev::TurnDone | Ev::Usage(_) => continue,
            _ => app.sb.focus_name().to_string(),
        };
        if !who.contains(&w) {
            who.push(w);
        }
    }
    if who.is_empty() {
        who.push(app.sb.focus_name().to_string());
    }
    who.join(", ")
}

/// A typed key with a selection in the history: the selection becomes a
/// quote (and ends). None without a selection.
pub(crate) fn take_selection(app: &mut App) -> Option<Result<String, String>> {
    // lines selected in the diff panel quote the same way (diffquote.rs)
    let Some(sel) = app.feed_sel else { return crate::diffquote::take(app) };
    if app.mouse.drag.is_some() {
        return None;
    }
    let text = crate::input::feed_selection_text(app).unwrap_or_default();
    app.feed_sel = None;
    let ((e0, _, _), (e1, _, _)) = sel.range();
    let from = speakers(app, e0, e1);
    Some(add(app, &from, &text))
}

// ---- the popup over the selection (BISE-229) ----
//
// The key bar says `type ask about it` at the bottom of the screen, far
// from the eyes that follow the selection (user: « mes yeux suivent la
// sélection mais le hint est tout en bas »). So once a drag ends, a
// one-row pill says it right above the selection's first row too:
// ` type ask about it · cmd+c copy ` on the pink pill, the words and
// colors of the key bar. No room above (the first row is the feed's
// top row, or scrolled out): under the last row. No room either (the
// selection fills the feed): none, the key bar says it. It starts at
// the selection's first column, pushed left to stay in the feed column
// (never over the panel, the divider or the composer). Narrow: without
// `· cmd+c copy`; narrower than the short pill: none. A press, a scroll,
// typing (the quote chip takes over) or esc (the selection ends) puts it
// away. `NO_COLOR`: `[ type ask about it · cmd+c copy ]`, the ask pair
// bold, no tint. It is drawn last over the history: no layout moves.

/// The popup's row, in at most `width` columns: the long form, else the
/// short one, else none. `who`: the agent it names (the diff's popup,
/// designer m_7568: ` type ask t1 about it `), dropped when it does
/// not fit; empty: none (the thread's).
pub(crate) fn hint_line(width: usize, no_color: bool, who: &str) -> Option<ratatui::text::Line<'static>> {
    if !who.is_empty() {
        if let Some(l) = hint_words(width, no_color, &format!(" ask {who} about it")) {
            return Some(l);
        }
    }
    hint_words(width, no_color, " ask about it")
}

fn hint_words(width: usize, no_color: bool, words: &str) -> Option<ratatui::text::Line<'static>> {
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    let bg = if no_color { Style::default() } else { Style::default().bg(crate::theme::pill_bg()) };
    let (key, ask, sep, copy_key, copy) = if no_color {
        let b = Style::default().fg(crate::theme::text()).add_modifier(Modifier::BOLD);
        let t = Style::default().fg(crate::theme::text());
        (b, b, t, t, t)
    } else {
        let a = bg.fg(crate::theme::accent());
        let d = bg.fg(crate::theme::dim());
        (a.add_modifier(Modifier::BOLD), a, d, bg.fg(crate::theme::text()), d)
    };
    let (open, close) = if no_color { ("[ ", " ]") } else { (" ", " ") };
    let long = vec![
        Span::styled(open, bg),
        Span::styled("type", key),
        Span::styled(words.to_string(), ask),
        Span::styled(" · ", sep),
        Span::styled("cmd+c", copy_key),
        Span::styled(" copy", copy),
        Span::styled(close, bg),
    ];
    let short: Vec<Span<'static>> = long[..3].iter().cloned().chain([Span::styled(close, bg)]).collect();
    [long, short].into_iter().map(Line::from).find(|l| l.width() <= width)
}

/// Where the popup goes: `w` columns on the row above the selection's
/// first row, else under its last row, in `area` (the feed column);
/// `vis` is the (event, row) of each row of `area`, top down.
pub(crate) fn hint_rect(sel: crate::feedsel::FeedSel, vis: &[(usize, usize)], area: ratatui::layout::Rect, w: u16) -> Option<ratatui::layout::Rect> {
    let (a, b) = sel.range();
    if w == 0 || w > area.width {
        return None;
    }
    let first = vis.iter().position(|&r| r == (a.0, a.1));
    let last = vis.iter().rposition(|&r| r == (b.0, b.1));
    let h = area.height as usize;
    let y = match (first, last) {
        (Some(y0), _) if y0 >= 1 => y0 - 1,
        (_, Some(y1)) if y1 + 1 < h => y1 + 1,
        _ => return None,
    };
    let x = (a.2.min(u16::MAX as usize) as u16).min(area.width - w);
    Some(ratatui::layout::Rect { x: area.x + x, y: area.y + y as u16, width: w, height: 1 })
}

/// Draws the popup over the history when a selection is up and its drag
/// ended (the last thing drawn over the history).
pub(crate) fn draw_hint(app: &App, frame: &mut ratatui::Frame) {
    let Some(sel) = app.feed_sel else { return };
    if !app.quote_hint || app.mouse.drag.is_some() || app.term.shown() {
        return;
    }
    let area = ratatui::layout::Rect { x: app.feed_x, y: app.feed_y, width: app.area_w.min(u16::MAX as usize) as u16, height: app.area_h.min(u16::MAX as usize) as u16 }
        .intersection(frame.area());
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let Some(line) = hint_line(area.width as usize, no_color, "") else { return };
    let vis: Vec<(usize, usize)> = app.vis_events.iter().copied().zip(app.vis_rows.iter().copied()).collect();
    let Some(r) = hint_rect(sel, &vis, area, line.width() as u16) else { return };
    crate::pointer::region(r, crate::pointer::Shape::Default); // BISE-272: over what it covers
    frame.render_widget(ratatui::widgets::Paragraph::new(line), r);
    crate::textlayer::text(r); // BISE-290: its text selects and copies
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tag_reads_back() {
        let t = tag("main", "fn a() {}\n  b");
        assert_eq!(t, "<selection from=\"main\">\nfn a() {}\n  b\n</selection>");
        let msg = format!("{t}\n{}\nwhy?", tag("you, docs", "x </selection> y"));
        let (qs, rest) = split(&msg);
        assert_eq!(rest, "why?");
        assert_eq!(qs[0], Quote { from: "main".into(), text: "fn a() {}\n  b".into(), ..Default::default() });
        assert_eq!(qs[1], Quote { from: "you, docs".into(), text: "x </selection > y".into(), ..Default::default() });
        assert_eq!(split("hello <selection from=\"x\">"), (vec![], "hello <selection from=\"x\">"));
        assert_eq!(split("<selection from=\"x\">\nno end"), (vec![], "<selection from=\"x\">\nno end"));
    }

    use crate::wire::Ev;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn press(app: &mut App, c: KeyCode) {
        crate::input::on_key(app, &KeyEvent::new(c, KeyModifiers::NONE));
    }

    /// main's feed: an answer, the whole of its first row selected.
    fn selected() -> App {
        let mut app = crate::sb::bench::test_app_drained();
        app.sb.focus = "main".into();
        app.events = vec![Ev::Assistant("the login breaks on safari".into())];
        app.cache = vec![None];
        app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 200) });
        app
    }

    /// Typing with a selection: the chip first, then the key; the
    /// selection ends; the strip lists the quote; the key bar says so
    /// before.
    #[test]
    fn typing_with_a_selection_quotes_it() {
        let mut app = selected();
        assert_eq!(crate::keybar::mode(&app), crate::keybar::Mode::Quote);
        press(&mut app, KeyCode::Char('w'));
        assert_eq!(app.ed.text, "[Quote #1] w");
        assert_eq!(app.ed.cursor, 12);
        assert!(app.feed_sel.is_none(), "the selection is taken");
        let q = of(&app.attachments[0]).unwrap();
        assert_eq!(q.from, "main");
        assert!(q.text.contains("the login breaks on safari"), "{q:?}");
        assert_eq!(crate::attach::strip_height(&app), 3);
        let rows: Vec<String> = crate::attach::strip_lines(&app, 70)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert!(rows[1].starts_with("│   ❝ 1  “") && rows[1].ends_with("main · 1 line  │"), "{rows:?}");
        // the next key just types; a second selection is quote 2, at the
        // cursor
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.ed.text, "[Quote #1] wh");
        app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 200) });
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.ed.text, "[Quote #1] wh [Quote #2] y");
        assert_eq!(app.ed.cursor, app.ed.text.chars().count());
        // no selection: no quote mode
        assert_ne!(crate::keybar::mode(&app), crate::keybar::Mode::Quote);
    }

    /// BISE-207: the chip goes at the cursor (a space before it when it
    /// would touch a word or a chip, one after), the key right after it;
    /// one undo step takes the key, the next the chip.
    #[test]
    fn the_quote_goes_at_the_cursor() {
        let cases = [
            ("hello", 0, "[Quote #1] whello", 11),
            ("hello", 2, "he [Quote #1] wllo", 14),
            ("hello", 5, "hello [Quote #1] w", 17),
            ("see [Quote #1]", 14, "see [Quote #1] [Quote #2] w", 26),
            ("a [Image #1] b", 12, "a [Image #1] [Quote #1] w b", 24),
        ];
        for (text, cursor, want, at) in cases {
            let mut app = selected();
            if text.contains("[Quote #1]") {
                app.attachments.push(Attachment { label: label(1), marker: tag("you", "q"), info: Default::default() });
            }
            app.ed.set(text, cursor);
            press(&mut app, KeyCode::Char('w'));
            assert_eq!(app.ed.text, want, "{text:?} at {cursor}");
            assert_eq!(app.ed.cursor, at + 1, "{text:?} at {cursor}: the key lands after the chip");
            app.ed.undo();
            app.ed.undo();
            assert_eq!(app.ed.text, text, "{text:?}: two undo steps");
        }
    }

    /// A backspace on the chip removes the quote; at most MAX.
    #[test]
    fn a_quote_is_removed_and_capped() {
        let mut app = selected();
        press(&mut app, KeyCode::Char('x'));
        app.ed.cursor = 10; // right after `[Quote #1]`
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.ed.text, " x");
        assert_eq!(crate::attach::strip_height(&app), 0);
        assert_eq!(crate::attach::expand(&mut app, " x"), " x");
        for _ in 0..MAX {
            app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 200) });
            press(&mut app, KeyCode::Char('a'));
        }
        assert_eq!(chips(&app.ed.text).len(), MAX);
        app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 200) });
        press(&mut app, KeyCode::Char('b'));
        assert_eq!(chips(&app.ed.text).len(), MAX);
        assert!(app.flash.as_ref().is_some_and(|(f, _)| f.contains("at most")));
        assert_eq!(app.ed.text.matches('b').count(), 1, "the key still types");
    }

    /// ⏎: the tags first, one per quote, then the text; the history
    /// draws a dim quote line over your words.
    #[test]
    fn the_sent_message_carries_the_tag() {
        let mut app = selected();
        for c in "why?".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        let sent = app.history[0].clone();
        let (qs, rest) = split(&sent);
        assert!(sent.starts_with("<selection from=\"main\">\n"), "{sent}");
        assert!(sent.contains("\n</selection>\nwhy?"), "{sent}");
        assert_eq!(rest, "why?");
        assert_eq!(qs.len(), 1);
        assert!(app.attachments.is_empty());
        let rows: Vec<String> = crate::render::user_block_lines(&sent, crate::wire::Mark::Sent, false, 60)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert!(rows[0].contains("❝ ") && rows[0].contains("safari · main · 1 line"), "{rows:?}");
        assert!(rows[1].contains("why?"), "{rows:?}");
        // a quote in the middle of the text still goes first, its label gone
        app.attachments.push(Attachment { label: label(1), marker: tag("you", "q"), info: Default::default() });
        assert_eq!(
            crate::attach::expand(&mut app, "look [Quote #1] here"),
            "<selection from=\"you\">\nq\n</selection>\nlook here"
        );
    }

    #[test]
    fn speakers_name_who_wrote_the_lines() {
        let mut app = crate::sb::bench::test_app();
        app.sb.focus = "docs".into();
        app.events = vec![
            Ev::You("hi".into(), crate::wire::Mark::Read, false),
            Ev::Assistant("a".into()),
            Ev::TimeMark("14:02".into()),
            Ev::Assistant("b".into()),
        ];
        assert_eq!(speakers(&app, 0, 3), "you, docs");
        assert_eq!(speakers(&app, 1, 3), "docs");
    }

    #[test]
    fn preview_and_about() {
        assert_eq!(preview("the login\n  breaks on safari", 40), "the login breaks on safari");
        assert_eq!(preview("the login breaks on safari", 12), "the login b…");
        let q = Quote { from: "main".into(), text: "a\nb\nc\n".into(), ..Default::default() };
        assert_eq!(about(&q), "main · 3 lines");
        assert_eq!(about(&Quote { from: "you".into(), text: "a".into(), ..Default::default() }), "you · 1 line");
        let at = |new: &str, old: &str| Where { file: "src/quote.rs".into(), new: new.into(), old: old.into() };
        let d = |text: &str, at| Quote { from: "t1 vs main".into(), text: text.into(), at };
        assert_eq!(about(&d(" a\n-b\n+c", at("192-193", "190-191"))), "src/quote.rs:192-193 · 3 lines");
        assert_eq!(about(&d("-a\n-b", at("", "190-191"))), "src/quote.rs:190-191 · 2 removed lines");
    }

    /// A quote from a diff says where in its tag, and reads back.
    #[test]
    fn a_diff_tag_says_where() {
        let at = Where { file: "src/a \"b\".rs".into(), new: "12-14".into(), old: "11".into() };
        let t = tag_at("t1 vs main", &at, " fn a() {\n-    x\n+    y");
        assert_eq!(t, "<selection from=\"t1 vs main\" file=\"src/a 'b'.rs\" new=\"12-14\" old=\"11\">\n fn a() {\n-    x\n+    y\n</selection>");
        let msg = format!("{t}\nwhy?");
        let (qs, rest) = split(&msg);
        assert_eq!(rest, "why?");
        assert_eq!(qs[0].from, "t1 vs main");
        assert_eq!(qs[0].at, Where { file: "src/a 'b'.rs".into(), new: "12-14".into(), old: "11".into() });
        let only_new = tag_at("PR #7", &Where { file: "x.rs".into(), new: "3".into(), old: String::new() }, "+a");
        assert!(only_new.starts_with("<selection from=\"PR #7\" file=\"x.rs\" new=\"3\">\n"), "{only_new}");
        assert_eq!(split(&only_new).0[0].at.old, "");
    }

    // ---- the popup over the selection ----

    fn row_text(l: &ratatui::text::Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// The key bar's words and colors; the short form when narrow; none
    /// narrower; NO_COLOR in brackets, no tint.
    #[test]
    fn the_hint_reads_like_the_key_bar() {
        use ratatui::style::Modifier;
        let l = hint_line(80, false, "").unwrap();
        assert_eq!(row_text(&l), " type ask about it · cmd+c copy ");
        // the diff's names who gets it, else the thread's words
        assert_eq!(row_text(&hint_line(80, false, "t1").unwrap()), " type ask t1 about it · cmd+c copy ");
        assert_eq!(row_text(&hint_line(30, false, "t1").unwrap()), " type ask t1 about it ");
        assert_eq!(row_text(&hint_line(20, false, "t1").unwrap()), " type ask about it ");
        assert_eq!(l.spans[1].style.fg, Some(crate::theme::accent()));
        assert!(l.spans[1].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(l.spans[2].style.fg, Some(crate::theme::accent()));
        assert_eq!(l.spans[4].style.fg, Some(crate::theme::text()));
        assert_eq!(l.spans[5].style.fg, Some(crate::theme::dim()));
        assert!(l.spans.iter().all(|s| s.style.bg == Some(crate::theme::pill_bg())), "the pill under every cell");
        assert_eq!(row_text(&hint_line(31, false, "").unwrap()), " type ask about it ");
        assert_eq!(row_text(&hint_line(19, false, "").unwrap()), " type ask about it ");
        assert!(hint_line(18, false, "").is_none());
        let n = hint_line(80, true, "").unwrap();
        assert_eq!(row_text(&n), "[ type ask about it · cmd+c copy ]");
        assert!(n.spans.iter().all(|s| s.style.bg.is_none()), "NO_COLOR: no tint");
        assert!(n.spans[1].style.add_modifier.contains(Modifier::BOLD) && n.spans[2].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(row_text(&hint_line(33, true, "").unwrap()), "[ type ask about it ]");
        assert!(hint_line(20, true, "").is_none());
    }

    /// Above the first row at the selection's first column; pushed left
    /// to stay in the column; under the last row when the first is the
    /// top row or scrolled out; none when the selection fills the feed.
    #[test]
    fn the_hint_sits_above_else_below_else_nowhere() {
        use crate::feedsel::FeedSel;
        use ratatui::layout::Rect;
        let area = Rect { x: 2, y: 3, width: 60, height: 6 };
        // rows: event 0 (2 rows), event 1 (3 rows), event 2 (1 row)
        let vis = [(0, 0), (0, 1), (1, 0), (1, 1), (1, 2), (2, 0)];
        let sel = |a: (usize, usize, usize), b: (usize, usize, usize)| FeedSel { anchor: a, head: b };
        // above, at the selection's first column (dragged backwards too)
        let r = hint_rect(sel((1, 2, 30), (1, 1, 10)), &vis, area, 32).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (2 + 10, 3 + 2, 32, 1));
        // near the right edge: pushed left, inside the column
        let r = hint_rect(sel((1, 0, 50), (1, 0, 55)), &vis, area, 32).unwrap();
        assert_eq!((r.x, r.y), (2 + 60 - 32, 3 + 1));
        // the first row is the feed's top row: under the last row
        let r = hint_rect(sel((0, 0, 4), (0, 1, 9)), &vis, area, 19).unwrap();
        assert_eq!((r.x, r.y), (2 + 4, 3 + 2));
        // the first row scrolled out above: under the last
        let r = hint_rect(sel((0, 0, 4), (1, 1, 9)), &[(1, 0), (1, 1), (1, 2)], area, 19).unwrap();
        assert_eq!(r.y, 3 + 2);
        // the selection fills the feed: none (the key bar says it)
        assert!(hint_rect(sel((0, 0, 0), (2, 0, 9)), &vis, area, 19).is_none());
        // the first row at the top, the last scrolled out below: none
        assert!(hint_rect(sel((0, 0, 0), (7, 0, 9)), &vis, area, 19).is_none());
        // off screen altogether: none; wider than the column: none
        assert!(hint_rect(sel((8, 0, 0), (8, 0, 9)), &vis, area, 19).is_none());
        assert!(hint_rect(sel((1, 0, 0), (1, 0, 9)), &vis, area, 61).is_none());
    }

    fn frame_text(app: &mut App, w: u16, h: u16) -> Vec<String> {
        let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
        t.draw(|f| crate::run::draw_frame(app, f)).unwrap();
        let b = t.backend().buffer().clone();
        (0..h).map(|y| (0..w).map(|x| b[(x, y)].symbol()).collect::<String>()).collect()
    }

    /// The whole screen: a drag over the history puts the popup right
    /// above the selection, over the history only (the rows under it
    /// don't move); a scroll, a press or typing puts it away.
    #[test]
    fn a_drag_shows_the_hint_over_the_selection() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        let mut app = crate::sb::bench::test_app_drained();
        app.sb.focus = "main".into();
        app.events = (0..6).map(|i| Ev::Assistant(format!("answer number {i} about the login flow"))).collect();
        app.cache = (0..6).map(|_| None).collect();
        let before = frame_text(&mut app, 100, 30);
        let y = before.iter().position(|r| r.contains("answer number 4")).unwrap() as u16;
        let col = |r: &str, pat: &str| r.find(pat).map(|i| unicode_width::UnicodeWidthStr::width(&r[..i]));
        let x = col(&before[y as usize], "number 4").unwrap() as u16;
        let mouse = |app: &mut App, kind, column| {
            crate::input::on_mouse(app, &MouseEvent { kind, column, row: y, modifiers: KeyModifiers::NONE }, 30)
        };
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x);
        mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), x + 7);
        assert!(!frame_text(&mut app, 100, 30).iter().any(|r| r.contains("ask about it ·")), "not while dragging");
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x + 7);
        let after = frame_text(&mut app, 100, 30);
        let hint = " type ask about it · cmd+c copy ";
        assert!(after[y as usize - 1].contains(hint), "{after:#?}");
        assert_eq!(col(&after[y as usize - 1], hint), Some(x as usize), "at the selection's first column");
        // only that row changes, and only under the pill
        for (i, (a, b)) in before.iter().zip(&after).enumerate() {
            if i != y as usize - 1 && !b.contains("copied") && !a.contains("copied") {
                assert!(i + 5 > after.len() || a == b, "row {i} moved:\n{a}\n{b}");
            }
        }
        // a scroll puts it away (the selection stays)
        app.scroll = -1;
        assert!(!frame_text(&mut app, 100, 30).iter().any(|r| r.contains("ask about it ·")));
        assert!(app.feed_sel.is_some());
        // a new drag brings it back; typing takes the selection: gone
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x);
        mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), x + 3);
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x + 3);
        let y = frame_text(&mut app, 100, 30).iter().position(|r| r.contains(hint));
        assert!(y.is_some());
        press(&mut app, KeyCode::Char('w'));
        assert!(!frame_text(&mut app, 100, 30).iter().any(|r| r.contains("ask about it ·")));
    }
}
