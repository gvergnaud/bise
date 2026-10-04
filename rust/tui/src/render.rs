//! Feed events as styled lines: messages, reasoning sections, notices
//! and the OpenCode inline tool lines.

use crate::code::*;
use crate::markdown::*;
use crate::theme::*;
use crate::wire::*;
use crate::wrap_line;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

// ---- tool-call rendering helpers ----

pub(crate) fn fmt_elapsed(started: std::time::Instant) -> String {
    fmt_duration(started.elapsed())
}

/// A duration as the tools show it: `0.3s`, `12s`, `2m05s`.
pub(crate) fn fmt_duration(d: std::time::Duration) -> String {
    let s = d.as_secs_f64();
    if s < 10.0 {
        format!("{:.1}s", s)
    } else if s < 60.0 {
        format!("{:.0}s", s)
    } else {
        format!("{:.0}m{:02.0}s", (s / 60.0).floor(), s % 60.0)
    }
}

pub(crate) fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{}{}", head, ellipsis())
    }
}

/// `s` in at most `room` chars, the `…` (`...` in ASCII mode) included
/// when it is cut.
pub(crate) fn fit_chars(s: &str, room: usize) -> String {
    if s.chars().count() <= room {
        return s.to_string();
    }
    let e = ellipsis();
    let n = e.chars().count();
    if room < n {
        return s.chars().take(room).collect();
    }
    let head: String = s.chars().take(room - n).collect();
    format!("{}{}", head, e)
}

// naive "field":"value" extractor for JSON-ish args (no parser needed:
// the runtime caps the payload and the shape is known)
pub(crate) fn json_str_field(s: &str, field: &str) -> Option<String> {
    let pat = format!("\"{}\"", field);
    let i = s.find(&pat)?;
    let rest = s[i + pat.len()..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut ch = rest.chars();
    while let Some(c) = ch.next() {
        match c {
            '\\' => match ch.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                // truncated payload: keep the partial value
                None => out.push('\\'),
            },
            '"' => return Some(out),
            _ => out.push(c),
        }
    }
    // the runtime caps the wire payload: a cut string is still useful
    Some(out)
}

// the args preview: what the engineer reads at a glance
//   run_typescript        the first line of main() (the signature)
//   search_tool_functions mode and the query
//   bash / mcp            the raw args
// the first non-empty line of a source, "…" when more lines follow
pub(crate) fn first_line_preview(code: &str) -> String {
    let first = code
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let mut p = truncate_chars(first, 64);
    if code.lines().filter(|l| !l.trim().is_empty()).count() > 1 {
        p.push_str(" …");
    }
    p
}

pub(crate) fn args_preview(name: &str, args: &str) -> String {
    if name == "run_typescript" {
        if let Some(code) = json_str_field(args, "code") {
            return first_line_preview(&code);
        }
    }
    if name == "search_tool_functions" {
        let mode = json_str_field(args, "mode").unwrap_or_else(|| "best_match".into());
        if let Some(q) = json_str_field(args, "query") {
            return format!("{} \"{}\"", mode, truncate_chars(&q, 48));
        }
        return mode;
    }
    truncate_chars(args.trim(), 80)
}

// one event renders as one or many lines (markdown expands messages).
// The shape follows the OpenCode message parts: user messages are blocks
// with a colored left bar and panel background; assistant text is
// markdown in the OpenCode colors; tools are inline tools.
pub(crate) fn ev_lines_t(ev: &Ev, tick: u32, width: usize) -> Vec<Line<'static>> {
    // the feed twin: identical shape, but the tool spinner animates
    match ev {
        Ev::Tool(td) => tool_lines(td, tick, width),
        other => ev_lines(other, width),
    }
}

// ---- the measure (book §11) ----

/// Prose (messages, reports, notices) wraps at this many columns.
pub(crate) const PROSE_MAX: usize = 91;
/// Code (scripts, diffs, outputs) runs up to this many columns.
pub(crate) const CODE_MAX: usize = 103;

/// The prose measure in a feed column of `width`.
pub(crate) fn prose_width(width: usize) -> usize {
    width.clamp(1, PROSE_MAX)
}

/// The code measure in a feed column of `width`.
pub(crate) fn code_width(width: usize) -> usize {
    width.clamp(1, CODE_MAX)
}

/// The rows of one event in a feed column of `width`: prose wrapped at
/// its measure, a tool (its line, its code, its output) at the code
/// measure. The extra width stays empty: rows never stretch.
pub(crate) fn ev_rows(ev: &Ev, tick: u32, width: usize) -> Vec<Line<'static>> {
    let w = match ev {
        Ev::Tool(_) => code_width(width),
        // a reply wraps its prose at the prose measure itself; its tables
        // may run to the code measure (BISE-87)
        Ev::Assistant(_) => code_width(width),
        // a level-3 line is a row of a list, not prose: the code measure
        Ev::AgentMsg { level: 3, text, .. } if !is_brief(text) && report_parts(text).is_none() => code_width(width),
        _ => prose_width(width),
    };
    // a reply's rows are final (md_lines wrapped them): no second pass
    if matches!(ev, Ev::Assistant(_)) && !main_feed() {
        return ev_lines_t(ev, tick, w);
    }
    let mut rows = Vec::new();
    for l in ev_lines_t(ev, tick, w) {
        rows.extend(wrap_line(l, w));
    }
    rows
}

// a thinking section: collapsed it is one dim glyph + duration;
// expanded (ctrl+o, or a click) the reasoning shows under a faint rail
pub(crate) fn thinking_lines(ms: u128, text: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let dim_st = Style::default().fg(dim());
    let label = match fmt_think_ms(ms) {
        d if d.is_empty() => "thought".to_string(),
        d => format!("thought for {}", d),
    };
    let head = Line::from(vec![
        Span::styled(format!(" {} ", G_THINK), dim_st),
        Span::styled(label, dim_st),
        Span::styled(format!(" {}", if open { G_OPEN } else { G_CLOSED }), dim_st),
    ]);
    if !open || text.trim().is_empty() {
        return vec![head];
    }
    let mut rows = vec![head];
    // the BENDSIG line carries the provider signature (the signed
    // thinking transport), never part of the reasoning itself
    let body = unescape_md(text);
    let lines = body
        .split('\n')
        .filter(|l| !l.starts_with("BENDSIG::"))
        .map(|l| Line::from(Span::styled(l.to_string(), dim_st)));
    let bar = Span::styled(RAIL, Style::default().fg(rule()));
    rows.extend(barred_rows(&bar, lines, width));
    rows
}

// 800ms -> "0.8s"; 4200ms -> "4.2s"; 12_300ms -> "12s"; 90_000 -> "1m30s";
// 0 (no measured duration: a replayed section, or lines that arrived in
// the same batch) -> ""
pub(crate) fn fmt_think_ms(ms: u128) -> String {
    if ms == 0 {
        String::new()
    } else if ms < 10_000 {
        format!("{}.{}s", ms / 1000, (ms % 1000) / 100)
    } else if ms < 60_000 {
        format!("{}s", ms / 1000)
    } else {
        format!("{}m{}s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

thread_local! {
    /// the feed drawn is main's (ui.rs sets it before each frame)
    static MAIN_FEED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whose feed is drawn: main's, or an agent's (its replies carry no
/// `:*`). The rows built under one owner rebuild under the other.
pub(crate) fn set_main_feed(on: bool) {
    MAIN_FEED.with(|c| c.set(on));
}

pub(crate) fn main_feed() -> bool {
    MAIN_FEED.with(|c| c.get())
}

thread_local! {
    static FEED_OWNER: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// The agent whose feed is drawn: a message it received (no `to` on the
/// wire) names it in the `to` column. The rows built under one owner
/// rebuild under the other (the feed cache is per view).
pub(crate) fn set_feed_owner(name: &str) {
    FEED_OWNER.with(|c| {
        if *c.borrow() != name {
            *c.borrow_mut() = name.to_string();
        }
    });
}

fn feed_owner() -> String {
    FEED_OWNER.with(|c| c.borrow().clone())
}

/// The faint rail in front of disclosed text (reasoning, a report, a
/// brief, a message body).
const RAIL: &str = " │ ";

/// A notice with no §6 glyph (an info line). Not in the book: see the
/// BISE-13 notes.
pub(crate) const G_NOTE: &str = "·";

/// One line in the feed's glyph column: the glyph at column 1, the text
/// from column 3 (under the names of the tool lines).
// a glyph in the glyph column, the text from column 4; its wrapped rows
// hang under the text, never at column 1 (BISE-90)
fn glyph_line(glyph: &str, glyph_st: Style, text: String, text_st: Style, width: usize) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let first = Span::styled(format!(" {} ", glyph), glyph_st);
    let pad = Span::raw(" ".repeat(first.content.width()));
    hung_rows(&first, &pad, [Line::from(Span::styled(text, text_st))], width)
}

/// pr-news (pr-design §4, designer's rule: color means attention): a PR
/// line in main's feed, ` ↑ #412 changes asked · dark-mode is on it`.
/// Plain: `↑` dim, the words in the text color; dim (merged, closed):
/// all dim; red (checks fail): `↑` red (bold under `NO_COLOR`), the
/// words in the text color. The number links to the PR (OSC 8).
fn pr_lines(tone: &str, number: u64, url: &str, text: &str, url_row: bool, width: usize) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let dim_st = Style::default().fg(dim());
    let (mark_st, text_st) = match tone {
        "dim" => (Style::default().fg(faint()), dim_st),
        "red" if no_color => (Style::default().add_modifier(Modifier::BOLD), Style::default().fg(crate::theme::text())),
        "red" => (Style::default().fg(error()), Style::default().fg(crate::theme::text())),
        _ => (dim_st, Style::default().fg(crate::theme::text())),
    };
    let first = Span::styled(format!(" {} ", crate::theme::pr_glyph()), mark_st);
    let pad = Span::raw(" ".repeat(first.content.width()));
    let num = format!("#{}", number);
    let head = if url.is_empty() { Span::styled(num, text_st) } else { crate::textlayer::link(num, url, text_st) };
    let line = Line::from(vec![head, Span::styled(format!(" {}", text), text_st)]);
    let mut rows = hung_rows(&first, &pad, [line], width);
    if url_row && !url.is_empty() {
        rows.extend(hung_rows(&pad, &pad, [Line::from(Span::styled(url.to_string(), dim_st))], width));
    }
    rows
}

/// An artifact as a chip (site/m/artifacts E): ` ↗ pricing page ` on the
/// chips' tint, in the accent, one link to `url` (a click opens it, like
/// ⏎ in /artifacts); a gone file struck through, `▲` after it. Under
/// `NO_COLOR` (or ASCII): bold, `[↗ pricing page]`. Never wraps across
/// two rows: its spaces are non-breaking.
pub(crate) fn artifact_chip(title: &str, gone: bool, url: &str) -> Vec<Span<'static>> {
    let tag = crate::links::add(url);
    let form = chip_form();
    let base = match form {
        ChipForm::Tinted => Style::default().fg(accent()).bg(chip_bg()),
        ChipForm::Bracketed => Style::default().add_modifier(Modifier::BOLD),
    };
    let st = Style { add_modifier: crate::links::with_tag(base.add_modifier, tag), ..base };
    let title_st = if gone { st.add_modifier(Modifier::CROSSED_OUT) } else { st };
    // non-breaking spaces inside: the wrap never splits a chip
    let title = title.replace(' ', "\u{a0}");
    let (open, close) = match form {
        ChipForm::Tinted => ("\u{a0}", "\u{a0}"),
        ChipForm::Bracketed => ("[", "]"),
    };
    let mut out = vec![Span::styled(format!("{}↗\u{a0}", open), st), Span::styled(title, title_st), Span::styled(close.to_string(), st)];
    if gone {
        out.push(Span::styled(format!(" {}", crate::theme::glyph(crate::theme::G_INTERRUPTED)), Style::default().fg(error())));
    }
    out
}

/// A ↗ line's title is cut at this width (its run's kind column).
pub(crate) const MADE_TITLE_MAX: usize = 28;

thread_local! {
    /// the width the titles of the ↗ line being built pad to (its run's
    /// widest, feed.rs sets it)
    static MADE_PAD: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn set_made_pad(w: usize) {
    MADE_PAD.with(|c| c.set(w));
}

/// site/m/artifacts C: `↗ pricing page   page · v3 · pricing-page` (the
/// agent named from 80 columns of feed: at 150, not at 80).
fn made_lines(id: &str, agent: &str, title: &str, kind: &str, v: u32, width: usize) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let gone = crate::artifacts::get(id).is_some_and(|a| a.gone);
    let mut spans = vec![Span::raw(" ")];
    let title = crate::artifacts_screen::cut(title, MADE_TITLE_MAX);
    spans.extend(artifact_chip(&title, gone, &crate::artifacts::url_of(id, None)));
    // the kind column of the run (designer, m_7220)
    let pad = MADE_PAD.with(|c| c.get()).saturating_sub(title.width());
    spans.push(Span::raw("\u{a0}".repeat(pad)));
    let mut words = vec![crate::artifacts::kind_word(kind)];
    if v > 1 {
        words.push(format!("v{}", v));
    }
    if width >= 80 && !agent.is_empty() {
        words.push(agent.to_string());
    }
    spans.push(Span::styled(format!("   {}", words.join(" · ")), Style::default().fg(dim())));
    crate::wrap_line(Line::from(spans), width.max(1))
}

/// site/m/artifacts D: under `✓ x landed 2 commits on main (a1b2c3d)`,
/// `± 3 files +42 −18  a1b2c3d`; `± 3 files` links to the diff (never
/// opened by itself).
fn landed_lines(agent: &str, from: &str, sha: &str, files: u64, add: u64, del: u64, width: usize) -> Vec<Line<'static>> {
    let ask = crate::diffview::Ask::Range(format!("{}..{}", from, sha), agent.to_string());
    let url = crate::diffview::url_of(&ask);
    let tag = crate::links::add(&url);
    let label = format!("{} {}", crate::theme::glyph(crate::theme::G_PATCH), crate::diffview::files_word(files as usize));
    let mut spans = vec![Span::raw("   "), Span::styled(label, crate::links::link_style(Style::default(), text(), tag))];
    if add > 0 {
        spans.push(Span::styled(format!(" +{}", add), Style::default().fg(crate::theme::ok())));
    }
    if del > 0 {
        spans.push(Span::styled(format!(" −{}", del), Style::default().fg(error())));
    }
    spans.push(Span::styled(format!("  {}", sha.chars().take(7).collect::<String>()), Style::default().fg(faint())));
    crate::wrap_line(Line::from(spans), width.max(1))
}

pub(crate) fn ev_lines(ev: &Ev, width: usize) -> Vec<Line<'static>> {
    let dim_st = Style::default().fg(dim());
    let text_st = Style::default().fg(text());
    let err_st = Style::default().fg(error());
    match ev {
        // an image marker is an accent chip `▣ login.png` (book §14)
        Ev::You(t, mark, open) => user_block_lines(t, *mark, *open, width),
        Ev::MarkYou { .. } => vec![],
        // BISE-86 (book §13, §17): `✗ not delivered: {name} stopped.`, and
        // while it waits for an answer `⏎ send again · esc drop`
        Ev::Undelivered { name, open, .. } => {
            let mut l = glyph_line(G_FAILED, err_st, format!("not delivered: {} stopped.", name), text_st, width);
            if *open {
                l[0].spans.push(Span::styled(" ⏎ send again · esc drop", dim_st));
            }
            l
        }
        // in main's feed, main's reply carries `:*` (book §6), its text at
        // column 3; inside an agent, the reply is the view's own voice
        Ev::Assistant(t) if main_feed() => {
            let mark = Span::styled(format!(" {} ", G_MAIN), main_mark_st());
            // wrapped once, at the text's own width: the rows under the
            // first one line up with its text (BISE-97: at 80 columns a
            // row 1 cell too wide wrapped again, leaving one-word rows)
            use unicode_width::UnicodeWidthStr;
            let lead = mark.content.width();
            let rows = md_lines(&unescape_md(t), prose_width(width).saturating_sub(lead), width.saturating_sub(lead));
            hung_rows(&mark, &Span::raw(" ".repeat(lead)), rows, width)
        }
        // inside an agent: the reply starts at the glyph column, like the
        // mockup "inside an agent" (BISE-90; it was one column left of it)
        Ev::Assistant(t) => {
            let lead = Span::raw(" ");
            let rows = md_lines(&unescape_md(t), prose_width(width).saturating_sub(1), width.saturating_sub(1));
            hung_rows(&lead, &lead, rows, width)
        }
        Ev::Thinking { ms, text, open } => thinking_lines(*ms, text, *open, width),
        Ev::Tool(td) => tool_lines(td, 0, width),
        Ev::Idle => vec![Line::from("")],
        // computer use (design §8, m_3774): an action's own row, like `$`
        // and `ƒ`: `↖ clicked "Add to cart" · amazon.fr`, `✗ couldn't …`
        // in the error color, a pause or a stop dim
        Ev::Sub { name, preview, .. } if crate::computer_use::sub_row(name, preview).is_some() => {
            let (did, line) = crate::computer_use::sub_row(name, preview).unwrap_or((crate::computer_use::Did::Done, String::new()));
            let room = width.saturating_sub(3);
            match did {
                crate::computer_use::Did::Done => vec![Line::from(vec![
                    Span::styled(format!(" {} ", crate::computer_use::mark()), dim_st),
                    Span::styled(fit_chars(&line, room), Style::default().fg(text())),
                ])],
                crate::computer_use::Did::Failed => vec![Line::from(vec![
                    Span::styled(format!(" {} ", crate::theme::glyph(G_FAILED)), err_st),
                    Span::styled(fit_chars(&line, room), err_st),
                ])],
                crate::computer_use::Did::Held => vec![Line::from(vec![Span::raw("   "), Span::styled(fit_chars(&line, room), dim_st)])],
            }
        }
        // a sub-call inside a TypeScript run: `↳ github.search_issues ✓`;
        // a failed one says why, in the error color
        Ev::Sub { name, ok, preview } => vec![Line::from(vec![
            Span::styled(format!("   {} ", G_SUBCALL), dim_st),
            Span::styled(name.clone(), dim_st),
            if *ok {
                Span::styled(format!(" {}", G_RECEIVED), dim_st)
            } else {
                let why = fit_chars(preview.trim(), 80);
                Span::styled(format!(" {} {}", G_FAILED, why).trim_end().to_string(), err_st)
            },
        ])],
        Ev::Turn => vec![Line::from(vec![
            Span::styled(" ── turn ", Style::default().fg(faint())),
            Span::styled("─".repeat(24), Style::default().fg(rule())),
        ])],
        Ev::TurnDone => vec![Line::from(vec![
            Span::styled(" └─ ", Style::default().fg(rule())),
            Span::styled(G_RECEIVED, dim_st),
        ])],
        // book §6, §17 (BISE-90): `≡ compacting` (the glyph pulses while
        // it runs: feed.rs Live::Compacting), then `≡ summary ▸`, the
        // summary under the rail once opened
        Ev::Compact => compacting_line(0, true),
        Ev::Compacted { text, open } => summary_lines(text, *open, width),
        Ev::Fold { head, text, open } => fold_lines(head, text, *open, width),
        // an interrupted turn ("turn interrupted by main" too) is dim;
        // any other warning reads as text
        Ev::Warn(t) if t == "turn interrupted" || t.starts_with("turn interrupted by ") => glyph_line(G_INTERRUPTED, dim_st, t.clone(), dim_st, width),
        // the plan's limit line: its usage page clickable (OSC 8)
        Ev::Warn(t) if crate::wire::is_plan_line(t) && t.contains(crate::wire::PLAN_USAGE_TEXT) => {
            use unicode_width::UnicodeWidthStr;
            let (a, b) = t.split_once(crate::wire::PLAN_USAGE_TEXT).unwrap_or((t, ""));
            let first = Span::styled(format!(" {} ", G_INTERRUPTED), dim_st);
            let pad = Span::raw(" ".repeat(first.content.width()));
            let line = Line::from(vec![
                Span::styled(a.to_string(), text_st),
                crate::textlayer::link(crate::wire::PLAN_USAGE_TEXT, crate::wire::PLAN_USAGE_URL, text_st),
                Span::styled(b.to_string(), text_st),
            ]);
            hung_rows(&first, &pad, [line], width)
        }
        Ev::Warn(t) => glyph_line(G_INTERRUPTED, dim_st, t.clone(), text_st, width),
        // a model without vision refused an image: say so, and the way out
        Ev::Err(t) => match crate::attach::no_vision(t) {
            Some(mut spans) => {
                if let Some(first) = spans.first_mut() {
                    first.content = format!(" {} ", G_FAILED).into();
                }
                vec![Line::from(spans)]
            }
            // BISE-293: a refused request, the designer's two lines:
            // bise's (`✗ the turn stopped: OpenAI refused the request
            // (400).`), the provider's own words dim under it
            None => match crate::wire::refusal_parts(t) {
                Some((head, said)) => {
                    let mut l = glyph_line(G_FAILED, err_st, format!("the turn stopped: {}", head), err_st, width);
                    if !said.is_empty() {
                        l.extend(glyph_line(" ", dim_st, said.to_string(), dim_st, width));
                    }
                    l
                }
                None => glyph_line(G_FAILED, err_st, t.clone(), err_st, width),
            },
        },
        // voice mode's transcript lines (design §5): plain faint notes,
        // `· voice mode · 14:02`, `· voice mode ended · 7 min · …`
        Ev::Info(t) if t.starts_with("· voice mode") => {
            let faint_st = Style::default().fg(faint());
            crate::wrap_line(Line::from(vec![Span::raw(" "), Span::styled(t.clone(), faint_st)]), width.max(1))
        }
        Ev::Info(t) => glyph_line(G_NOTE, Style::default().fg(faint()), bend_images::display(t), dim_st, width),
        Ev::Pr { tone, number, url, text, url_row } => pr_lines(tone, *number, url, text, *url_row, width),
        Ev::Made { id, agent, title, kind, v } => made_lines(id, agent, title, kind, *v, width),
        Ev::Landed { agent, from, sha, files, add, del } => landed_lines(agent, from, sha, *files, *add, *del, width),
        Ev::Approval { ok, text, note, asked, open } => answer_lines(*ok, text, note, asked, *open, width),
        Ev::Said { glyph, head, dim } => {
            // ✗ a failure (error); ? it needs you, ✓ it worked (accent)
            let calm = *glyph != "✗";
            let st = if calm { Style::default().fg(accent()) } else { err_st };
            let shown = if *glyph == "✓" { crate::theme::glyph(crate::theme::G_DONE) } else { glyph };
            let mut l = glyph_line(shown, st, head.clone(), if calm { text_st } else { err_st }, width);
            for d in dim {
                l.extend(glyph_line(" ", dim_st, d.clone(), dim_st, width));
            }
            l
        }
        Ev::ToolInfo { .. } | Ev::ToolResult { .. } | Ev::ToolCode { .. } | Ev::ToolIntent { .. } | Ev::Gate(_) => vec![],
        Ev::Usage(u) => vec![Line::from(Span::styled(u.line(), dim_st))],
        Ev::Raw(t) => vec![Line::from(Span::styled(format!("  {}", t), dim_st))],
        Ev::AgentMsg { text, open, .. } if is_brief(text) => brief_lines(text, *open, width),
        Ev::AgentMsg { from, text, open, .. } if report_parts(text).is_some() => {
            let (kind, body) = report_parts(text).unwrap_or_default();
            report_lines(from, kind, body, *open, width)
        }
        Ev::AgentMsg { from, to, text, level: 3, id, open, .. } => l3_lines(from, to, id, text, *open, width),
        Ev::AgentMsg { from, text, .. } => l2_lines(from, text, width),
        Ev::Answered { agent, question, answer, why, open } => answered_lines(agent, question, answer, why, *open, width),
        Ev::TimeMark(t) => vec![Line::from(Span::styled(format!(" {} {} {}", G_NOTE, t, G_NOTE), Style::default().fg(faint())))],
        Ev::Card { text, closed } => card_lines(text, closed, width),
        Ev::CardClosed { .. } | Ev::Ended(_) => vec![],
        Ev::Release(r) => crate::release_row::lines(r, width),
    }
}

// ---- the three levels (book §9) ----

/// The envelope of a level-3 chip: `✉` in text presentation (U+2709
/// U+FE0E: one column; book §9).
pub(crate) const G_ENVELOPE: &str = "\u{2709}\u{FE0E}";
/// Where a level-3 line group starts: x0, flush with the text's left
/// edge (the fold line too; BISE-109, was x0+2).
const L3_X: &str = "";
/// Where its text goes, under the chip (BISE-127): x0+2.
const L3_UNDER: &str = "  ";
/// A level-3 text wider than this (2 rows of 30 columns) may be cut: it
/// opens.
const L3_LONG: usize = 60;
/// A closed level-3 text shows at most this many rows, then `… ▸`.
const L3_ROWS: usize = 2;

/// Who a level-3 message went to: `to`, or with none (what this feed's
/// owner received) the owner (`main` in main's feed), else the message id.
pub(crate) fn l3_receiver(to: &str, id: &str) -> String {
    if !to.is_empty() {
        return to.to_string();
    }
    let owner = if main_feed() { "main".to_string() } else { feed_owner() };
    if owner.is_empty() {
        id.to_string()
    } else {
        owner
    }
}

/// The envelope for the mode: `@` under `BISE_ASCII=1`.
pub(crate) fn envelope() -> &'static str {
    if ascii_mode() {
        "@"
    } else {
        G_ENVELOPE
    }
}

/// How a level-3 chip is drawn (book §9): on the `chip` tint, or, with
/// no tint (`NO_COLOR`, `BISE_ASCII=1`), between brackets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChipForm {
    Tinted,
    Bracketed,
}

pub(crate) fn chip_form() -> ChipForm {
    if ascii_mode() || chip_bg() == ratatui::style::Color::Reset {
        ChipForm::Bracketed
    } else {
        ChipForm::Tinted
    }
}

/// A level-3 text that may be cut (wider than 2 rows of 30 columns, or
/// several lines): it opens (`▸`).
pub(crate) fn l3_long(text: &str) -> bool {
    use unicode_width::UnicodeWidthStr;
    let t = text.trim();
    t.contains('\n') || t.width() > L3_LONG
}

/// The chip ` ✉︎ sender → receiver `: envelope dim, sender bold text,
/// `→` faint, receiver dim; `compact` (a narrow column), no inner
/// padding nor spaces: `✉︎sender→receiver`. Bracketed:
/// `[✉︎ sender → receiver]` (`[@ sender > receiver]` in ASCII). The
/// names are drawn as given (see [`chip_names`]).
pub(crate) fn chip_spans(from: &str, to: &str, form: ChipForm, compact: bool) -> Vec<Span<'static>> {
    let (open, close) = match (form, compact) {
        (ChipForm::Tinted, false) => (" ", " "),
        (ChipForm::Tinted, true) => ("", ""),
        (ChipForm::Bracketed, _) => ("[", "]"),
    };
    let gap = if compact { "" } else { " " };
    let tint = |st: Style| match form {
        ChipForm::Tinted => st.bg(chip_bg()),
        ChipForm::Bracketed => st,
    };
    let dim_st = tint(Style::default().fg(dim()));
    let mut spans = vec![
        Span::styled(format!("{}{}{}", open, envelope(), gap), dim_st),
        Span::styled(from.to_string(), tint(Style::default().fg(text()).add_modifier(Modifier::BOLD))),
    ];
    if !compact {
        spans.push(Span::styled(" ", dim_st));
    }
    spans.push(Span::styled(glyph("→"), tint(Style::default().fg(faint()))));
    spans.push(Span::styled(format!("{}{}{}", gap, to, close), dim_st));
    spans
}

/// The columns of a chip around its two names.
fn chip_frame(compact: bool, form: ChipForm) -> usize {
    match (compact, form) {
        (false, _) => 7,
        (true, ChipForm::Tinted) => 2,
        (true, ChipForm::Bracketed) => 4,
    }
}

/// The two names of a chip: cut at `cap` with `…`, then, while the chip
/// is wider than `room`, the receiver first, then the sender (book §9
/// 'Short on room'). The envelope and the arrow are never cut.
fn chip_names(from: &str, to: &str, cap: usize, frame: usize, room: usize) -> (String, String) {
    use unicode_width::UnicodeWidthStr;
    let (mut s, mut r) = (fit_chars(from, cap), fit_chars(to, cap));
    let (mut sn, mut rn) = (s.chars().count(), r.chars().count());
    while frame + s.width() + r.width() > room && (rn > 1 || sn > 1) {
        if rn > 1 {
            rn -= 1;
            r = fit_chars(to, rn);
        } else {
            sn -= 1;
            s = fit_chars(from, sn);
        }
    }
    (s, r)
}

/// `line` cut to `room` columns, `tail` after it (a cut row's `… ▸`):
/// at the end of a word when the row has one, no space before `tail`.
fn cut_row(line: Line<'static>, room: usize, tail: &str) -> Line<'static> {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    let keep = room.saturating_sub(tail.width());
    let cells: Vec<(char, Style)> = line.spans.iter().flat_map(|sp| sp.content.chars().map(move |c| (c, sp.style))).collect();
    let tail_st = cells.last().map_or_else(Style::default, |c| c.1);
    let (mut n, mut w) = (0usize, 0usize);
    while n < cells.len() && w + cells[n].0.width().unwrap_or(0) <= keep {
        w += cells[n].0.width().unwrap_or(0);
        n += 1;
    }
    // a word cut in its middle goes whole to the cut
    if n < cells.len() && cells[n].0 != ' ' {
        if let Some(sp) = cells[..n].iter().rposition(|c| c.0 == ' ').filter(|&sp| sp > 0) {
            n = sp;
        }
    }
    while n > 0 && cells[n - 1].0 == ' ' {
        n -= 1;
    }
    let mut kept: Vec<(char, Style)> = cells[..n].to_vec();
    kept.extend(tail.chars().map(|c| (c, tail_st)));
    let mut row = crate::feed::line_from(kept);
    row.alignment = line.alignment;
    row
}

/// The dim text of a level-3 message in rows of `room` columns: closed,
/// at most 2 rows then `… ▸` (`…` alone when it does not open); open,
/// every line, `▾` at the end.
fn l3_text_rows(text: &str, open: bool, room: usize) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let dim_st = Style::default().fg(dim());
    let long = l3_long(text);
    let room = room.max(1);
    if open && long {
        let mut rows: Vec<Line<'static>> = text
            .trim()
            .split('\n')
            .flat_map(|l| wrap_line(Line::from(Span::styled(l.to_string(), dim_st)), room))
            .collect();
        let mark = format!(" {}", glyph(G_OPEN));
        match rows.last_mut() {
            Some(last) if last.width() + mark.width() <= room => last.spans.push(Span::styled(mark, dim_st)),
            _ => rows.push(Line::from(Span::styled(glyph(G_OPEN), dim_st))),
        }
        return rows;
    }
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut rows = wrap_line(Line::from(Span::styled(flat, dim_st)), room);
    let more = if long { format!(" {}", glyph(G_CLOSED)) } else { String::new() };
    if rows.len() > L3_ROWS {
        rows.truncate(L3_ROWS);
        let last = rows.pop().unwrap_or_default();
        rows.push(cut_row(last, room, &format!("{}{}", ellipsis(), more)));
    } else if long {
        // several lines, all shown flat: they open as they were written
        let last = rows.pop().unwrap_or_default();
        if last.width() + more.width() <= room {
            let mut last = last;
            last.spans.push(Span::styled(more, dim_st));
            rows.push(last);
        } else {
            rows.push(cut_row(last, room, &format!("{}{}", ellipsis(), more)));
        }
    }
    rows
}

/// A message between agents (level 3, book §9, BISE-127): at x0 the
/// chip ` ✉︎ sender → receiver ` on the `chip` tint, alone on its row;
/// under it, at x0+2, the dim text, 2 rows at most then `… ▸`; open,
/// the whole text. No `to` (what this feed's owner received): the
/// owner's name (`main` in main's feed), else the message id (BISE-90).
pub(crate) fn l3_lines(from: &str, to: &str, id: &str, text: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    l3_lines_as(chip_form(), from, to, id, text, open, width)
}

pub(crate) fn l3_lines_as(
    form: ChipForm,
    from: &str,
    to: &str,
    id: &str,
    text: &str,
    open: bool,
    width: usize,
) -> Vec<Line<'static>> {
    let mut ls = vec![l3_chip_row(form, from, to, id, width)];
    ls.extend(l3_under(text, open, width));
    ls
}

/// The chip row of a level-3 message. W, the reading width from x0
/// (book §9 'Short on room'): names cut at 24 / 16 / 10, and then only
/// as the row's room asks (BISE-109); under 40, the compact chip.
fn l3_chip_row(form: ChipForm, from: &str, to: &str, id: &str, width: usize) -> Line<'static> {
    let w = width.saturating_sub(L3_X.len());
    let (cap, compact) = match w {
        60.. => (24, false),
        40..=59 => (16, false),
        _ => (10, true),
    };
    let (s, r) = chip_names(from, &l3_receiver(to, id), cap, chip_frame(compact, form), w);
    let mut head = vec![Span::raw(L3_X)];
    head.extend(chip_spans(&s, &r, form, compact));
    Line::from(head)
}

/// The text of a level-3 message under its chip, at x0+2 (BISE-127:
/// at every width, so the texts of a run start on one column).
fn l3_under(text: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    l3_text_rows(text, open, width.saturating_sub(L3_UNDER.len()))
        .into_iter()
        .map(|r| {
            let mut spans = vec![Span::raw(L3_UNDER)];
            spans.extend(r.spans);
            let mut row = Line::from(spans);
            row.alignment = r.alignment;
            row
        })
        .collect()
}

/// A level-3 message right after one with the same sender and receiver
/// (book §9, BISE-127): no chip, its text alone at x0+2, like
/// [`ev_rows`] would draw it under the chip.
pub(crate) fn l3_text_only_rows(ev: &Ev, width: usize) -> Vec<Line<'static>> {
    let Ev::AgentMsg { text, open, .. } = ev else {
        return Vec::new();
    };
    let w = code_width(width);
    l3_under(text, *open, w).into_iter().flat_map(|l| wrap_line(l, w)).collect()
}

/// The speaker of a level-2 block is bold (book §9 'Emphasis'): main's
/// `:*` in accent bold.
fn main_mark_st() -> Style {
    Style::default().fg(accent()).add_modifier(Modifier::BOLD)
}

/// An agent writing to you (level 2): normal text, `@ name to you: …`
/// with its speaker in bold (from main: `:* …`, `:*` accent bold).
fn l2_lines(from: &str, body: &str, width: usize) -> Vec<Line<'static>> {
    let text_st = Style::default().fg(text());
    let bold_st = text_st.add_modifier(Modifier::BOLD);
    let (glyph, glyph_st, lead) = if from == "main" {
        (G_MAIN, main_mark_st(), String::new())
    } else {
        (G_MSG, bold_st, format!("{} to you:", from))
    };
    let mut lines = md_lines(&unescape_md(body.trim()), width.saturating_sub(3), width.saturating_sub(3));
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    if !lead.is_empty() {
        // the speaker bold, the space after it plain
        lines[0].spans.insert(0, Span::styled(" ", text_st));
        lines[0].spans.insert(0, Span::styled(lead, bold_st));
    }
    let mark = Span::styled(format!(" {} ", glyph), glyph_st);
    hung_rows(&mark, &Span::raw("   "), lines, width)
}

/// Main answered an agent for you (level 2): `:* docs asked: v1 or v2?
/// i answered: v2. ▸ why`; open, the why under the rail.
fn answered_lines(agent: &str, question: &str, answer: &str, why: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let text_st = Style::default().fg(text());
    let q = question.trim().replace('\n', " ");
    let sep = if q.ends_with(['?', '.', '!', ':']) { " " } else { "; " };
    let mut line = vec![Span::styled(
        format!("{} asked: {}{}i answered: {}", agent, q, sep, answer.trim().replace('\n', " ")),
        text_st,
    )];
    if !why.trim().is_empty() {
        line.push(Span::styled(format!(" {} why", if open { G_OPEN } else { G_CLOSED }), Style::default().fg(dim())));
    }
    let mark = Span::styled(format!(" {} ", G_MAIN), main_mark_st());
    let mut ls = hung_rows(&mark, &Span::raw("   "), [Line::from(line)], width);
    if open && !why.trim().is_empty() {
        let bar = Span::styled(RAIL, Style::default().fg(rule()));
        ls.extend(barred_rows(&bar, md_lines(why.trim(), width.saturating_sub(3), width.saturating_sub(3)), width));
    }
    ls
}

/// The fold of a run of level-3 lines (book §10): `▸ 47 messages
/// between 30 agents`, dim at x0 like the chips, no chip (§9), cut
/// from the right with `…` to fit `width`; the last run, still growing,
/// carries the working pulse.
pub(crate) fn fold_line(n: usize, agents: usize, open: bool, live: bool, tick: u32, width: usize) -> Line<'static> {
    let label = format!(
        "{} {} messages between {} agent{}",
        if open { G_OPEN } else { G_CLOSED },
        n,
        agents,
        if agents == 1 { "" } else { "s" }
    );
    let pulse = if live { 2 } else { 0 };
    let room = width.saturating_sub(L3_X.len() + pulse).max(1);
    let mut row = vec![Span::raw(L3_X), Span::styled(fit_chars(&label, room), Style::default().fg(dim()))];
    if live {
        let (g, c) = working_frame(tick);
        row.push(Span::styled(format!(" {}", g), Style::default().fg(c)));
    }
    Line::from(row)
}

/// A card line of the hub (`#3 question @docs : v1 or v2?`): its kind,
/// the agent, the text.
pub(crate) fn card_parts(t: &str) -> Option<(&str, &str, &str)> {
    let rest = t.strip_prefix('#')?;
    let (_, rest) = rest.split_once(' ')?;
    let (kind, rest) = rest.split_once(' ')?;
    let rest = rest.strip_prefix('@')?;
    let (name, text) = rest.split_once(" : ").unwrap_or((rest, ""));
    Some((kind, name, text))
}

// a card in the history (book §9): a question or a blocker is level 1,
// an accent bar `┃`, the bold accent title `? {name} needs you`, the
// body in text; a done or failed card is one line for you (`✓` accent, `✗`).
// Answered (`closed`: the hub's word, book §10, §12), a level-1 card
// fades in place: dim bar, dim title with ` · answered`, dim body; the
// answer follows as its own line.
fn card_lines(t: &str, closed: &str, width: usize) -> Vec<Line<'static>> {
    let text_st = Style::default().fg(text());
    let (kind, name, body) = card_parts(t).unwrap_or(("question", "", t));
    match kind {
        "done" => {
            let check = Style::default().fg(accent());
            return glyph_line(done_glyph(), check, format!("{} is done: {}", name, body), text_st, width);
        }
        k if k.contains("fail") => {
            return glyph_line(G_FAILED, Style::default().fg(error()), format!("{} failed: {}", name, body), text_st, width)
        }
        _ => {}
    }
    let mut title = if name.is_empty() { "needs you".to_string() } else { format!("{} needs you", name) };
    let (bar_st, title_st, body_st) = if closed.is_empty() {
        let accent_st = Style::default().fg(accent()).add_modifier(Modifier::BOLD);
        (Style::default().fg(accent()), accent_st, text_st)
    } else {
        title.push_str(&format!(" · {}", closed_word(closed)));
        let dim_st = Style::default().fg(dim());
        (dim_st, dim_st, dim_st)
    };
    let bar = Span::styled(" ┃ ", bar_st);
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{} ", G_CARD), title_st),
        Span::styled(title, title_st),
    ])];
    lines.extend(body.split('\n').map(|l| Line::from(Span::styled(l.to_string(), body_st))));
    barred_rows(&bar, lines, width)
}

/// How a card was closed, in the user's words (the hub's `card-closed`
/// result: `answered`, `answered via @x`, `closed`, `accepted`, …).
pub(crate) fn closed_word(res: &str) -> String {
    let res = res.trim();
    if let Some(who) = res.strip_prefix("answered via @") {
        return format!("answered by {}", who);
    }
    match res {
        "closed" => "closed".into(),
        "accepted" => "dropped".into(),
        "refused" => "kept".into(),
        "vue" => "seen".into(),
        "reprise" => "resumed".into(),
        "task stopped" => "agent stopped".into(),
        "" => "answered".into(),
        r => r.to_string(),
    }
}

// your message (book §6, mockups): `›` dim in the glyph column, the text
// from column 3, each of its lines (Shift+Enter, paste) on its own rows.
// No bar, no background: the terminal's own shows through.
/// A longer message of yours folds to this many rows in the history
/// (BISE-239; 12 since BISE-261, 20 since BISE-262: the user found
/// 8 then 12 too few), then `▸ n more lines`; ctrl+o, a click on that
/// row or space opens it whole.
pub(crate) const YOU_ROWS: usize = 20;

/// Whether your message folds: more than [`YOU_ROWS`] lines (a quote, a
/// chip count one), or as many rows at 80 columns (one long paragraph).
/// Width-free, so ctrl+o and find know it without the feed's width; at
/// the feed's width a message that still fits shows whole.
pub(crate) fn you_folds(msg: &str) -> bool {
    use unicode_width::UnicodeWidthStr;
    let (quotes, body) = crate::quote::split(msg);
    // a long paste (BISE-240) is its chip in the line; it always
    // folds: its full text shows only open
    let (body, pastes) = crate::pasted::fold(body);
    let rows: usize = quotes.len() + body.split('\n').map(|l| l.width().div_ceil(77).max(1)).sum::<usize>();
    !pastes.is_empty() || rows > YOU_ROWS
}

/// The rows of your message's long pastes, under its text (behind its
/// bar): one dim row each, the full text when the message is open
/// (pasted.rs).
fn paste_rows(pastes: &[crate::pasted::Pasted], open: bool, width: usize) -> Vec<Line<'static>> {
    let bar = Span::styled(format!("{}  ", user_bar()), Style::default().fg(accent()));
    hung_rows(&bar, &bar, crate::pasted::rows(pastes, open, width.saturating_sub(3)), width)
}

/// How many rows [`paste_rows`] takes in `msg` (feed::toggle_at: a
/// click there opens or closes the message).
pub(crate) fn you_paste_rows(msg: &str, open: bool, width: usize) -> usize {
    let (_, body) = crate::quote::split(msg);
    let (_, pastes) = crate::pasted::fold(body);
    if pastes.is_empty() {
        return 0;
    }
    paste_rows(&pastes, open, width).len()
}

pub(crate) fn user_block_lines(msg: &str, mark: Mark, open: bool, width: usize) -> Vec<Line<'static>> {
    let style = Style::default().fg(text());
    // BISE-134: the quotes in front (quote.rs), one dim line each:
    // `❝ the selected words… · main · 3 lines`
    let (quotes, body) = crate::quote::split(msg);
    let d = Style::default().fg(dim());
    let quote_lines: Vec<Line<'static>> = quotes
        .iter()
        .map(|q| {
            let about = format!(" · {}", crate::quote::about(q));
            let room = width.saturating_sub(3 + 2 + unicode_width::UnicodeWidthStr::width(about.as_str())).clamp(8, 60);
            let g = crate::theme::glyph(crate::theme::G_QUOTE);
            Line::from(Span::styled(format!("{g} {}{about}", crate::quote::preview(&q.text, room)), d))
        })
        .collect();
    let folds = you_folds(msg);
    // voice mode (design §5): what you said, not typed, carries `said`
    let said = crate::voicemode::live::was_said(msg);
    // BISE-240: each long paste is its chip in the line (pasted.rs)
    let (folded, pastes) = crate::pasted::fold(body);
    let msg = folded.as_str();
    let mut lines: Vec<Line<'static>> = msg
        .split('\n')
        .map(|l| Line::from(crate::attach::chip_spans(l.trim_end_matches('\r'), style)))
        .collect();
    // opened whole: `▾` after its last line (click it, or ctrl+o, to fold)
    if folds && open {
        if let Some(last) = lines.last_mut() {
            last.spans.push(Span::styled(format!(" {}", glyph(G_OPEN)), d));
        }
    }
    // BISE-90 (user decision, marketing's look): a thin accent bar at
    // column 0 on every row of your message, the text from column 3 (`›`
    // stays the composer's prompt); the heavy `┃` is a card's
    let bar = Span::styled(format!("{}  ", user_bar()), Style::default().fg(accent()));
    // each line (a quote, a line of text) and its rows
    let only_quotes = msg.is_empty() && !quotes.is_empty();
    let mut all: Vec<Line<'static>> = quote_lines.into_iter().chain(if only_quotes { vec![] } else { lines }).collect();
    let units: Vec<Vec<Line<'static>>> = all.iter().map(|l| hung_rows(&bar, &bar, [l.clone()], width)).collect();
    let total: usize = units.iter().map(Vec::len).sum();
    let mut rows: Vec<Line<'static>> = Vec::new();
    if folds && !open && total > YOU_ROWS {
        // BISE-239: closed, YOU_ROWS rows then `▸ n more lines` (a line
        // cut to fit counts as hidden), its mark after the hint
        let mut shown = 0;
        for u in &units {
            let room = YOU_ROWS - rows.len();
            rows.extend(u.iter().take(room).cloned());
            if u.len() > room {
                break;
            }
            shown += 1;
        }
        let mut hint = vec![bar.clone(), Span::styled(crate::toolbox::more_label(units.len() - shown), d)];
        hint.extend(said_span(said));
        hint.extend(mark_span(mark));
        rows.push(Line::from(hint));
    } else {
        // its mark at the end (C3): `·` sent, `✓` got, `✓✓` read (accent);
        // said in voice mode: `said ✓✓`
        if let Some(last) = all.last_mut() {
            last.spans.extend(said_span(said));
            last.spans.extend(mark_span(mark));
        }
        rows = hung_rows(&bar, &bar, all, width);
    }
    // its long pastes, one dim row each (the full text when open)
    rows.extend(paste_rows(&pastes, open, width));
    // the sizes of its images, dim, under it (still behind the bar)
    rows.extend(sizes_rows(msg, width));
    rows
}

/// The rows of the sizes of your message's images (`▣ a.png 1284×322 ·
/// ▣ b.png …`), dim behind its bar; several when they wrap. `msg`: its
/// text with the long pastes folded to their chips.
fn sizes_rows(msg: &str, width: usize) -> Vec<Line<'static>> {
    let Some(sizes) = crate::attach::sizes_line(msg) else { return vec![] };
    let bar = Span::styled(format!("{}  ", user_bar()), Style::default().fg(accent()));
    hung_rows(&bar, &bar, [Line::from(Span::styled(sizes, Style::default().fg(dim())))], width)
}

/// How many rows the sizes of `msg`'s images take under it at `width`
/// (feed::toggle_at: a click there does nothing; a few screenshots wrap
/// to several rows, which hid the `▸ n more lines` row from the click).
pub(crate) fn you_sizes_rows(msg: &str, width: usize) -> usize {
    let (_, body) = crate::quote::split(msg);
    let (folded, _) = crate::pasted::fold(body);
    sizes_rows(&folded, width).len()
}

// ---- an answer to an inbox item (BISE-307, designer) ----

/// The columns an answer's head may take before it is cut, width-free
/// (80 columns less the mark) like [`you_folds`].
const ANSWER_HEAD: usize = 77;

/// Whether the line of an answer opens: its head is cut at 80 columns
/// (the sentence and the question on one line), the question has more
/// lines, or its words fold like a long message of yours.
pub(crate) fn answer_opens(text: &str, note: &str, asked: &str) -> bool {
    use unicode_width::UnicodeWidthStr;
    let asked = asked.trim();
    let head = text.width() + if asked.is_empty() { 0 } else { 3 + one_line(asked.lines().next().unwrap_or("")).width() };
    head > ANSWER_HEAD || asked.contains('\n') || you_folds(note)
}

/// `s` on one line: runs of whitespace become one space.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `s` in at most `w` columns, cut with `…`.
fn cut_cols(s: &str, w: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    if s.width() <= w {
        return s.to_string();
    }
    let e = ellipsis();
    let room = w.saturating_sub(e.width());
    let (mut out, mut used) = (String::new(), 0);
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0);
        if used + cw > room {
            break;
        }
        used += cw;
        out.push(ch);
    }
    out.push_str(e);
    out
}

/// An inbox item answered (approvals-design.md §9; BISE-307, designer):
/// the head `✓ you answered main · <the question>` on one row (✓
/// accent, ✗ dim; the sentence in text, the question dim, cut with `…`),
/// then your own words like a message of yours (the accent bar, text
/// from column 3, 20 rows then `▸ n more lines`). Open: `▾` at the
/// head's end, the head whole, the full question hung at column 3, dim,
/// the words whole.
fn answer_lines(ok: bool, text: &str, note: &str, asked: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let d = Style::default().fg(dim());
    let text_st = Style::default().fg(crate::theme::text());
    let mut mark_st = if ok { Style::default().fg(accent()) } else { d };
    if ok && crate::find::no_color() {
        mark_st = mark_st.add_modifier(Modifier::BOLD);
    }
    let mark = Span::styled(format!(" {} ", glyph(if ok { G_RECEIVED } else { G_FAILED })), mark_st);
    let pad = Span::raw("   ");
    let asked = asked.trim();
    let opens = answer_opens(text, note, asked);
    let mut rows = if open && opens {
        let head = Line::from(vec![Span::styled(text.to_string(), text_st), Span::styled(format!(" {}", glyph(G_OPEN)), d)]);
        let mut rows = hung_rows(&mark, &pad, [head], width);
        let q = asked.lines().map(|l| Line::from(Span::styled(l.trim_end().to_string(), d)));
        rows.extend(hung_rows(&pad, &pad, q, width));
        rows
    } else {
        let room = width.saturating_sub(3).max(1);
        let mut head = vec![mark];
        if text.width() >= room || asked.is_empty() {
            head.push(Span::styled(cut_cols(text, room), text_st));
        } else {
            head.push(Span::styled(text.to_string(), text_st));
            let left = room - text.width();
            if left > 3 + 1 {
                // its first line (a question's options open under it)
                let first = asked.lines().next().unwrap_or("");
                head.push(Span::styled(format!(" · {}", cut_cols(&one_line(first), left - 3)), d));
            }
        }
        vec![Line::from(head)]
    };
    if note.trim().is_empty() {
        return rows;
    }
    // your words, like a message of yours
    let bar = Span::styled(format!("{}  ", user_bar()), Style::default().fg(accent()));
    let units: Vec<Vec<Line<'static>>> = note
        .trim()
        .split('\n')
        .map(|l| hung_rows(&bar, &bar, [Line::from(Span::styled(l.trim_end().to_string(), text_st))], width))
        .collect();
    let total: usize = units.iter().map(Vec::len).sum();
    if !open && total > YOU_ROWS {
        let (mut words, mut shown) = (Vec::new(), 0);
        for u in &units {
            let room = YOU_ROWS - words.len();
            words.extend(u.iter().take(room).cloned());
            if u.len() > room {
                break;
            }
            shown += 1;
        }
        rows.extend(words);
        rows.push(Line::from(vec![bar.clone(), Span::styled(crate::toolbox::more_label(units.len() - shown), d)]));
    } else {
        rows.extend(units.into_iter().flatten());
    }
    rows
}

/// The bar in front of your messages: `│`, `|` under `BISE_ASCII=1`.
fn user_bar() -> &'static str {
    if crate::theme::ascii_mode() {
        "|"
    } else {
        "│"
    }
}

/// The mark after your message (C3).
/// ` said`, faint, before your message's mark when you said it in voice
/// mode (design §5: `said ✓✓`).
fn said_span(said: bool) -> Option<Span<'static>> {
    said.then(|| Span::styled(" said", Style::default().fg(faint())))
}

fn mark_span(mark: Mark) -> Option<Span<'static>> {
    let (g, c) = match mark {
        Mark::Sent => (G_SENDING, dim()),
        Mark::Received => (G_RECEIVED, faint()),
        Mark::Read => (G_READ, accent()),
        Mark::Failed => (G_FAILED, error()),
    };
    Some(Span::styled(format!(" {}", glyph(g)), Style::default().fg(c)))
}

// each line wrapped to the width left after the bar, every row (the
// wrapped continuations too) behind the same bar, so the text stays
// aligned; a continuation keeps its soft mark (the copy joins it)
fn barred_rows(
    bar: &Span<'static>,
    lines: impl IntoIterator<Item = Line<'static>>,
    width: usize,
) -> Vec<Line<'static>> {
    hung_rows(bar, bar, lines, width)
}

// the same with a different prefix on the very first row (a glyph) and
// on all the others (its blank indent); both the same width
pub(crate) fn hung_rows(
    first: &Span<'static>,
    rest: &Span<'static>,
    lines: impl IntoIterator<Item = Line<'static>>,
    width: usize,
) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthStr;
    let inner = width.saturating_sub(first.content.width()).max(1);
    let mut rows = Vec::new();
    for l in lines {
        for r in wrap_line(l, inner) {
            let mut spans = vec![if rows.is_empty() { first.clone() } else { rest.clone() }];
            spans.extend(r.spans);
            let mut row = Line::from(spans);
            row.alignment = r.alignment;
            rows.push(row);
        }
    }
    rows
}


// the tool line (book §6, mockup "inside an agent"): the tool's glyph,
// its name, its state (the working pulse and the elapsed time while it
// runs, `✓` once ok, `✗` in the error color on failure), the args.
// " 1.2s" after the tool name; nothing for a replayed tool (no duration)
pub(crate) fn elapsed_label(elapsed: &Option<String>) -> String {
    match elapsed.as_deref() {
        Some(e) if !e.is_empty() => format!(" {}", e),
        _ => String::new(),
    }
}

// the name and the one-line args of a tool, and its decoded source
pub(crate) fn tool_meta(td: &ToolData) -> (String, String, Option<(CodeLang, String)>) {
    let name = td.name.clone().unwrap_or_else(|| format!("#{}", td.id));
    // the source of a code tool (run_typescript, bash, apply_patch), when
    // the runtime sent it: rendered in full, highlighted, under the line
    let code = match (&td.code, code_lang(&name)) {
        (Some(raw), Some(lang)) => {
            Some((lang, tool_source(lang, wire_decode(raw)))).filter(|(_, c)| !c.trim().is_empty())
        }
        _ => None,
    };
    let args = match &code {
        // the block shows the whole source: a gray copy on the tool line
        // would only repeat it
        Some((CodeLang::Bash | CodeLang::TypeScript, _)) => String::new(),
        // a patch keeps its one-line summary (the files it touches)
        Some((CodeLang::Patch, src)) => patch_summary(src),
        None => td
            .args
            .as_deref()
            .map(|a| args_preview(&name, a))
            .unwrap_or_default(),
    };
    (name, args, code)
}

// the tool line itself: the only part of a running tool that changes
// from one frame to the next (spinner, elapsed)
pub(crate) fn tool_head(td: &ToolData, tick: u32, name: &str, args: &str) -> Line<'static> {
    if matches!(name, "apply_patch" | "edit" | "write_file") {
        if let Some(src) = td.code.as_deref().map(|raw| tool_source(CodeLang::Patch, wire_decode(raw))) {
            return edit_head(td, tick, &src);
        }
    }
    // book §6: `$` bash, `ƒ` TypeScript; other tools keep an empty
    // glyph column
    let (glyph, label) = match name {
        "bash" => (G_BASH, "bash"),
        "run_typescript" => (G_TS, "typescript"),
        other => (" ", other),
    };
    let dim_st = Style::default().fg(dim());
    // §9 Emphasis: a one-line tool call is the agent's own work, dim
    let mut row = vec![
        Span::styled(format!(" {} ", glyph), dim_st),
        Span::styled(label.to_string(), dim_st),
    ];
    match td.state {
        ToolState::Run => {
            let (g, c) = working_frame(tick);
            row.push(Span::styled(format!(" {}", g), Style::default().fg(c)));
            row.push(Span::styled(format!(" {}", fmt_elapsed(td.started)), dim_st));
        }
        ToolState::Ok => {
            row.push(Span::styled(format!(" {}{}", G_RECEIVED, elapsed_label(&td.elapsed)), dim_st));
        }
        ToolState::Fail => {
            row.push(Span::styled(
                format!(" {}{}", G_FAILED, elapsed_label(&td.elapsed)),
                Style::default().fg(error()),
            ));
        }
    }
    if !args.is_empty() {
        row.push(Span::styled(" · ".to_string(), dim_st));
        // a done call's paths are links (BISE-264); a running one is
        // redrawn each frame, outside its event's links
        if matches!(td.state, ToolState::Run) {
            row.push(Span::styled(args.to_string(), dim_st));
        } else {
            row.extend(crate::file_links::plain_spans(args, dim_st));
        }
    }
    Line::from(row)
}

// everything under the tool line (book §11, progressive disclosure): a
// bash or TypeScript script always in full (never folded, whatever its
// length); an edit's diff only when opened (its line says the rest);
// the output one line, `▸ output`, until opened
pub(crate) fn tool_body(td: &ToolData, code: &Option<(CodeLang, String)>, width: usize) -> Vec<Line<'static>> {
    let mut ls = Vec::new();
    match code {
        Some((CodeLang::Patch, src)) => {
            if td.expanded {
                ls.extend(code_block_lines(src, CodeLang::Patch, &td.state, width));
            }
            // an edit's result is on its line (✓ +3 −1, or why it failed)
            return ls;
        }
        Some((lang, src)) => ls.extend(code_block_lines(src, *lang, &td.state, width)),
        None => {}
    }
    ls.extend(output_lines(td, width));
    ls
}

/// The output of a tool (the runtime's one-line preview): closed,
/// `▸ output` (` · 3 failed` when the text says so); a failure shows its
/// reason in the error color instead, `▸` when cut. Open, the whole
/// text under the rail.
pub(crate) fn output_lines(td: &ToolData, width: usize) -> Vec<Line<'static>> {
    let Some((ok, preview)) = &td.result else { return Vec::new() };
    // images in the result: `result · ▣ shot.png 390×844` (book §14)
    let images = crate::attach::result_spans(preview);
    let shown = if images.is_some() { crate::attach::without_markers(preview) } else { preview.clone() };
    let text = shown.trim();
    let faint_st = Style::default().fg(dim());
    if let Some(spans) = images.filter(|_| *ok) {
        let mut row = vec![Span::raw("   ")];
        if !text.is_empty() {
            row.push(Span::styled(format!("{} ", if td.expanded { G_OPEN } else { G_CLOSED }), faint_st));
        }
        row.extend(spans);
        let mut ls = vec![Line::from(row)];
        if td.expanded && !text.is_empty() {
            let hl: Vec<Vec<Span<'static>>> = text.split('\n').map(|l| vec![Span::styled(l.to_string(), faint_st)]).collect();
            ls.extend(rail_rows(&hl, width));
        }
        return ls;
    }
    if text.is_empty() {
        return Vec::new();
    }
    let text_st = Style::default().fg(if *ok { dim() } else { error() });
    let mut ls = Vec::new();
    if td.expanded {
        ls.push(Line::from(Span::styled(format!("   {} output", G_OPEN), faint_st)));
        let hl: Vec<Vec<Span<'static>>> = text
            .split('\n')
            .map(|l| vec![Span::styled(l.to_string(), text_st)])
            .collect();
        ls.extend(rail_rows(&hl, width));
        return ls;
    }
    if *ok {
        let mut label = format!("   {} output", G_CLOSED);
        if let Some(k) = failed_count(text) {
            label.push_str(&format!(" · {} failed", k));
        }
        ls.push(Line::from(Span::styled(label, faint_st)));
        return ls;
    }
    // a failure: one line, its reason first; `▸` when there is more
    let room = width.saturating_sub(3 + 2).max(8);
    let first = text.lines().next().unwrap_or("");
    let cut = first.chars().count() > room || text.lines().nth(1).is_some();
    let mut row = vec![Span::styled(format!("   {}", fit_chars(first, room)), text_st)];
    if cut {
        row.push(Span::styled(format!(" {}", G_CLOSED), faint_st));
    }
    ls.push(Line::from(row));
    ls
}

/// "3 failed" in a test runner's output: the first count above zero.
pub(crate) fn failed_count(text: &str) -> Option<u64> {
    let mut rest = text;
    while let Some(k) = rest.find(" failed") {
        let before = rest[..k].trim_end_matches(',');
        let n: String = before.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
        let n: String = n.chars().rev().collect();
        if let Ok(v) = n.parse::<u64>() {
            if v > 0 && (before.len() == n.len() || !before[..before.len() - n.len()].ends_with(|c: char| c.is_alphanumeric())) {
                return Some(v);
            }
        }
        rest = &rest[k + " failed".len()..];
    }
    None
}

/// An edit's line (book §11): `± edit {path} ✓ +{a} −{d} ▸`; several
/// files read `{n} files`; a failure gives its reason in the error color.
pub(crate) fn edit_head(td: &ToolData, tick: u32, src: &str) -> Line<'static> {
    let files = patch_files(src);
    let target = match files.as_slice() {
        [(p, _, _)] => p.clone(),
        fs => format!("{} files", fs.len()),
    };
    let (adds, dels) = files.iter().fold((0, 0), |(a, d), f| (a + f.1, d + f.2));
    let dim_st = Style::default().fg(dim());
    let mut row = vec![
        Span::styled(format!(" {} ", G_PATCH), Style::default().fg(text())),
        Span::styled(
            if td.name.as_deref() == Some("write_file") { "write " } else { "edit " }.to_string(),
            Style::default().fg(text()),
        ),
    ];
    // one file edited: its path is a link once done (BISE-264)
    if files.len() == 1 && !matches!(td.state, ToolState::Run) {
        row.extend(crate::file_links::plain_spans(&target, Style::default().fg(text())));
    } else {
        row.push(Span::styled(target, Style::default().fg(text())));
    }
    let mark = format!(" {}", if td.expanded { G_OPEN } else { G_CLOSED });
    match td.state {
        ToolState::Run => {
            let (g, c) = working_frame(tick);
            row.push(Span::styled(format!(" {}", g), Style::default().fg(c)));
            row.push(Span::styled(format!(" {}", fmt_elapsed(td.started)), dim_st));
        }
        ToolState::Ok => {
            let mut counts = format!(" {}", G_RECEIVED);
            if adds > 0 {
                counts.push_str(&format!(" +{}", adds));
            }
            if dels > 0 {
                counts.push_str(&format!(" −{}", dels));
            }
            counts.push_str(&mark);
            row.push(Span::styled(counts, dim_st));
        }
        ToolState::Fail => {
            let reason = td
                .result
                .as_ref()
                .map(|(_, r)| r.trim().to_string())
                .filter(|r| !r.is_empty())
                .unwrap_or_else(|| "failed".into());
            row.push(Span::styled(format!(" {} {}", G_FAILED, reason), Style::default().fg(error())));
            row.push(Span::styled(mark, dim_st));
        }
    }
    Line::from(row)
}

pub(crate) fn tool_lines(td: &ToolData, tick: u32, width: usize) -> Vec<Line<'static>> {
    let (name, args, code) = tool_meta(td);
    // a closed skill call is its sentence, in every view (BISE-283)
    if crate::toolrow::is_skill(td) && crate::toolrow::row_mode(td) {
        return crate::toolrow::rows(td, tick, code_width(width));
    }
    if crate::toolbox::opens_as_box(td) {
        return crate::toolbox::box_lines(td, &code, &[], tick, code_width(width));
    }
    let mut ls = vec![tool_head(td, tick, &name, &args)];
    ls.extend(tool_body(td, &code, width));
    ls
}

// ---- compaction (book §6: `≡` dim, pulsing while it runs) ----

/// `≡ compacting`, the glyph pulsing (dim / faint) while `running`.
pub(crate) fn compacting_line(tick: u32, running: bool) -> Vec<Line<'static>> {
    let glyph_c = if running && !(tick / 4).is_multiple_of(2) { faint() } else { dim() };
    vec![Line::from(vec![
        Span::styled(format!(" {} ", G_COMPACTING), Style::default().fg(glyph_c)),
        Span::styled("compacting", Style::default().fg(dim())),
    ])]
}

/// `≡ summary ▸`; open, the summary under the rail.
fn summary_lines(summary: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let dim_st = Style::default().fg(dim());
    let body = unescape_md(summary);
    let has = !body.trim().is_empty();
    let mut head = vec![
        Span::styled(format!(" {} ", G_SUMMARY), dim_st),
        Span::styled("summary", dim_st),
    ];
    if has {
        head.push(Span::styled(format!(" {}", if open { G_OPEN } else { G_CLOSED }), dim_st));
    }
    let mut ls = vec![Line::from(head)];
    if open && has {
        let bar = Span::styled(RAIL, Style::default().fg(rule()));
        ls.extend(barred_rows(&bar, md_lines(&body, width.saturating_sub(3), width.saturating_sub(3)), width));
    }
    ls
}

/// A folded dim row (BISE-245): `▸ head`; open, `▾ head` and the text's
/// lines under the rail, dim.
fn fold_lines(head: &str, text: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let dim_st = Style::default().fg(dim());
    // nothing folded: the dim row alone (a setup answer's result)
    if text.trim().is_empty() {
        return vec![Line::from(Span::styled(head.to_string(), dim_st))];
    }
    let g = crate::theme::glyph(if open { G_OPEN } else { G_CLOSED });
    let mut ls = vec![Line::from(vec![Span::styled(format!("{g} "), dim_st), Span::styled(head.to_string(), dim_st)])];
    if open {
        let bar = Span::styled(RAIL, Style::default().fg(rule()));
        let rows: Vec<Line<'static>> =
            text.lines().map(|l| Line::from(Span::styled(l.to_string(), dim_st))).collect();
        ls.extend(barred_rows(&bar, rows, width));
    }
    ls
}

// ---- folded messages: reports in main, the brief inside an agent ----

/// A report (`[report: done] summary…`): its kind and its text.
pub(crate) fn report_parts(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("[report: ")?;
    let (kind, body) = rest.split_once(']')?;
    Some((kind.trim(), body.trim_start()))
}

/// The brief an agent got from main (`# Task \`name\`` …).
pub(crate) fn is_brief(text: &str) -> bool {
    text.starts_with("# Task `")
}

/// A report is one line, `✓ bench: the summary ▸ report`; open, the rest
/// of it under the rail. The glyph says the kind: `✓` done (accent), `✗` failed,
/// `?` blocked, `·` progress.
fn report_lines(from: &str, kind: &str, body: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let (glyph, color, st) = match kind {
        "done" => (done_glyph(), accent(), text()),
        k if k.contains("fail") => (G_FAILED, error(), text()),
        // BISE-299: main's to handle, not yours: dim
        "blocked" => (G_NEEDS_YOU, dim(), text()),
        _ => (G_STARTING, dim(), dim()),
    };
    let first = body.lines().next().unwrap_or("").trim();
    let rest = body.split_once('\n').map(|(_, r)| r.trim_matches('\n')).unwrap_or("");
    let head = format!("{}: ", from);
    let label = format!(" {} report", if open { G_OPEN } else { G_CLOSED });
    let room = width.saturating_sub(3 + head.chars().count() + label.chars().count()).max(8);
    let more = !rest.trim().is_empty() || first.chars().count() > room;
    let shown = if open { first.to_string() } else { fit_chars(first, room) };
    // the speaker of a report on your request is bold (level 2, book §9
    // 'Emphasis'); a progress line stays dim, unbold
    let head_st = match kind {
        "done" | "blocked" => Style::default().fg(st).add_modifier(Modifier::BOLD),
        k if k.contains("fail") => Style::default().fg(st).add_modifier(Modifier::BOLD),
        _ => Style::default().fg(st),
    };
    let mut row = vec![Span::styled(head, head_st), Span::styled(shown, Style::default().fg(st))];
    if more {
        row.push(Span::styled(label, Style::default().fg(dim())));
    }
    // open, its first line may be longer than the row: its wrapped rows
    // hang under the text, never at column 1 (BISE-90)
    let mark = Span::styled(format!(" {} ", glyph), Style::default().fg(color));
    let mut ls = hung_rows(&mark, &Span::raw("   "), [Line::from(row)], width);
    if open && !rest.trim().is_empty() {
        let bar = Span::styled(" │ ", Style::default().fg(rule()));
        ls.extend(barred_rows(&bar, md_lines(rest, width.saturating_sub(3), width.saturating_sub(3)), width));
    }
    ls
}

/// The brief inside an agent: `◇ brief ▸`; open, the brief under the
/// rail (without its `# Task` title: the agent is the view).
fn brief_lines(brief: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let mut ls = vec![Line::from(vec![
        Span::styled(format!(" {} ", G_BRIEF), Style::default().fg(text())),
        Span::styled("brief", Style::default().fg(text())),
        Span::styled(format!(" {}", if open { G_OPEN } else { G_CLOSED }), Style::default().fg(dim())),
    ])];
    if open {
        let body = brief.split_once('\n').map(|(_, r)| r.trim_matches('\n')).unwrap_or("");
        let bar = Span::styled(" │ ", Style::default().fg(rule()));
        ls.extend(barred_rows(&bar, md_lines(body, width.saturating_sub(3), width.saturating_sub(3)), width));
    }
    ls
}

// a multi-line user message (Shift+Enter, paste) and an agent message:
// one row per line, a long line wrapped, every row behind the bar
#[cfg(test)]
mod multiline_tests {
    use crate::feed::build_rows;
    use crate::wire::Ev;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;
    use ratatui::Terminal;

    fn screen(ev: Ev, width: u16) -> Vec<String> {
        let rows = build_rows(&[ev], 0, false, width as usize, 0);
        let h = rows.len() as u16;
        let mut term = Terminal::new(TestBackend::new(width, h)).unwrap();
        term.draw(|f| f.render_widget(Paragraph::new(rows), f.area())).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
            .collect()
    }

    #[test]
    fn user_message_keeps_its_line_breaks() {
        let long = "word ".repeat(12);
        let text = format!("first line\nsecond line\n{}end", long);
        let s = screen(Ev::You(text, crate::wire::Mark::Read, false), 30);
        // the bar on every row (BISE-90), every row's text at column 3
        assert_eq!(s[0].as_str(), "│  first line", "{s:#?}");
        assert_eq!(s[1].as_str(), "│  second line", "{s:#?}");
        // the long line wraps into several rows, all at the same column
        assert!(s.len() >= 5, "{s:#?}");
        for r in &s[2..] {
            assert!(r.starts_with("│  word") || r.starts_with("│  end") || r.trim_start_matches('│').trim() == "✓✓", "{s:#?}");
            assert!(r.chars().count() <= 30);
        }
        assert!(s.last().unwrap().ends_with("end ✓✓"), "{s:#?}");
        assert!(!s.iter().any(|r| r.contains('\n')));
    }

    fn answer(ok: bool, text: &str, note: &str, asked: &str, open: bool) -> Ev {
        Ev::Approval { ok, text: text.into(), note: note.into(), asked: asked.into(), open }
    }

    /// BISE-307 (the user: « crop trop vite, et ne peuvent pas être
    /// ouvertes »): the head on one row with the question, your words
    /// whole under the bar like a message of yours.
    #[test]
    fn an_answer_reads_like_your_message() {
        let words = "Non mais ça dépend des tâches quoi, la plupart du temps on veut que chaque agent garde son worktree";
        let s = screen(answer(true, "you answered main", words, "Should every task get its own worktree?", false), 60);
        assert_eq!(s[0], " ✓ you answered main · Should every task get its own worktr…", "{s:#?}");
        assert!(s[1..].iter().all(|r| r.starts_with("│  ")), "{s:#?}");
        let said: String = s[1..].iter().map(|r| r.trim_start_matches("│  ")).collect::<Vec<_>>().join(" ");
        assert_eq!(said, words, "the whole answer, never cut");
        // a picked option on the line, a short question: nothing to open
        let s = screen(answer(true, "you answered perf: both", "", "v1 or v2?", false), 60);
        assert_eq!(s, vec![" ✓ you answered perf: both · v1 or v2?"]);
        assert!(!crate::feed::discloses(&answer(true, "you answered perf: both", "", "v1 or v2?", false)));
        // a no: ✗, its note under the bar
        let s = screen(answer(false, "you said no to t3: rm -rf build", "pas maintenant", "", false), 60);
        assert_eq!(s, vec![" ✗ you said no to t3: rm -rf build", "│  pas maintenant"]);
    }

    /// Open: `▾` at the head's end, the question whole at column 3, the
    /// words whole; a long answer folds at 20 rows like a message.
    #[test]
    fn an_answer_opens_on_its_question_and_folds_when_long() {
        let q = "Which provider do we ship first?
Anthropic has the most users, OpenAI the most demand.";
        let ev = answer(true, "you answered main", "Anthropic first, then OpenAI next week", q, true);
        assert!(crate::feed::discloses(&ev));
        let s = screen(ev, 60);
        assert_eq!(s[0], format!(" ✓ you answered main {}", crate::theme::G_OPEN), "{s:#?}");
        assert_eq!(s[1], "   Which provider do we ship first?", "{s:#?}");
        assert_eq!(s[2], "   Anthropic has the most users, OpenAI the most demand.", "{s:#?}");
        assert_eq!(s[3], "│  Anthropic first, then OpenAI next week", "{s:#?}");
        let long: String = (1..=30).map(|i| format!("line {i}
")).collect();
        let s = screen(answer(true, "you answered main", &long, "go?", false), 60);
        assert_eq!(s.len(), 1 + 20 + 1, "{s:#?}");
        assert_eq!(s[21], "│  ▸ 10 more lines", "{s:#?}");
        let s = screen(answer(true, "you answered main", &long, "go?", true), 60);
        assert_eq!(s.len(), 2 + 30, "{s:#?}");
        // an approval's long command: cut on the line, whole open
        let cmd = format!("you allowed t3: npm publish --access public {}", "--tag next ".repeat(8));
        let s = screen(answer(true, &cmd, "", "", false), 60);
        assert!(s.len() == 1 && s[0].ends_with('…'), "{s:#?}");
        let s = screen(answer(true, &cmd, "", "", true), 60);
        assert!(s.len() > 1 && s.last().unwrap().ends_with(crate::theme::G_OPEN), "{s:#?}");
    }

    #[test]
    fn agent_message_opens_under_its_chip() {
        // a level-3 message opens whole: every line under the chip at
        // x0+2 (BISE-127)
        let text = format!("one{}two {}", '\n', "x ".repeat(30));
        let s = screen(Ev::AgentMsg { from: "main".into(), to: "docs".into(), text, level: 3, id: String::new(), open: true, fold: false }, 70);
        let head = format!(" {} main → docs", super::G_ENVELOPE);
        assert_eq!(s[0], head, "{s:#?}");
        let hang = "  ";
        assert_eq!(s[1], "  one", "{s:#?}");
        assert!(s[2].starts_with(&format!("{hang}two x")), "{s:#?}");
        assert!(s.len() >= 3 && s[1..].iter().all(|r| r.starts_with(hang)), "{s:#?}");
        assert!(s.last().unwrap().ends_with(crate::theme::G_OPEN), "{s:#?}");
    }
}

// BISE-95 (book §9 'Emphasis'): what's for you reads bigger through
// contrast and room
#[cfg(test)]
mod emphasis_tests {
    use super::set_main_feed;
    use crate::feed::build_rows;
    use crate::theme::*;
    use crate::wire::Ev;
    use ratatui::style::Modifier;
    use ratatui::text::Line;

    fn rows(events: &[Ev], i: usize) -> Vec<Line<'static>> {
        build_rows(events, i, false, 80, 0)
    }

    fn text_of(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn msg(from: &str, text: &str, level: u8) -> Ev {
        Ev::AgentMsg {
            from: from.into(),
            to: if level == 2 { "you".into() } else { "main".into() },
            text: text.into(),
            level,
            id: String::new(),
            open: false,
            fold: false,
        }
    }

    fn bold(l: &Line, needle: &str) -> bool {
        l.spans.iter().any(|s| s.content.contains(needle) && s.style.add_modifier.contains(Modifier::BOLD))
    }

    #[test]
    fn the_speaker_of_level_2_is_bold() {
        set_main_feed(true);
        let reply = rows(&[Ev::Assistant("found it.".into())], 0);
        let mark = reply[0].spans.iter().find(|s| s.content.contains(G_MAIN)).expect("the :* mark");
        assert!(mark.style.add_modifier.contains(Modifier::BOLD), "{reply:?}");
        assert_eq!(mark.style.fg, Some(accent()));
        let to_you = rows(&[msg("auth-fix", "want me to fix the other two?", 2)], 0);
        assert!(bold(&to_you[0], "auth-fix to you:"), "{to_you:?}");
        assert!(text_of(&to_you[0]).contains("auth-fix to you: want me"), "{to_you:?}");
        // the body stays plain text
        assert!(!bold(&to_you[0], "want me"), "{to_you:?}");
        let done = rows(&[msg("bench", "[report: done] 3 ops faster", 3)], 0);
        assert!(bold(&done[0], "bench:"), "{done:?}");
        let progress = rows(&[msg("bench", "[report: progress] halfway", 3)], 0);
        assert!(!bold(&progress[0], "bench:"), "{progress:?}");
        let answered = rows(
            &[Ev::Answered {
                agent: "docs".into(),
                question: "v1 or v2?".into(),
                answer: "v2".into(),
                why: String::new(),
                open: false,
            }],
            0,
        );
        assert!(bold(&answered[0], G_MAIN), "{answered:?}");
        set_main_feed(false);
    }

    #[test]
    fn the_agents_own_work_is_dim() {
        let t = rows(&[Ev::Thinking { ms: 6000, text: "hm".into(), open: false }], 0);
        assert!(t[0].spans.iter().all(|s| s.style.fg == Some(dim())), "{t:?}");
        // level 3: dim and faint, the sender alone in bold text on its
        // chip (BISE-106); no accent, main included
        for from in ["auth-fix", "main"] {
            let l3 = rows(&[msg(from, "found it: the test races the login event", 3)], 0);
            for s in l3[0].spans.iter().filter(|s| s.style.fg.is_some()) {
                let c = s.style.fg.unwrap();
                assert!(c == dim() || c == faint() || (c == text() && s.content == from && s.style.add_modifier.contains(Modifier::BOLD)), "{l3:?}");
            }
            assert!(bold(&l3[0], from), "{l3:?}");
        }
    }

    #[test]
    fn level_2_has_a_blank_row_above_and_below() {
        set_main_feed(true);
        let evs = vec![
            Ev::Thinking { ms: 6000, text: String::new(), open: false },
            Ev::Assistant("auth-fix found it.".into()),
            msg("auth-fix", "want me to fix the other two?", 2),
            msg("auth-fix", "found it", 3),
            msg("docs", "v2 is out", 3),
        ];
        // after the thinking, between two level-2 blocks, before level 3
        for i in 1..=3 {
            assert_eq!(text_of(&rows(&evs, i)[0]), "", "event {i}: {:?}", rows(&evs, i));
        }
        // a new pair of agents starts after a blank row (BISE-106)
        assert_eq!(text_of(&rows(&evs, 4)[0]), "", "{:?}", rows(&evs, 4));
        set_main_feed(false);
    }
}

// BISE-106 (book §5 `chip`, §9 'Level 3 is an envelope chip', 'Short on
// room'): a message between agents is a tinted envelope chip; BISE-127:
// its dim text under it at x0+2, at every width
#[cfg(test)]
mod chip_tests {
    use super::*;
    use crate::feed::build_rows;
    use ratatui::style::Color;
    use unicode_width::UnicodeWidthStr;

    fn msg(from: &str, to: &str, text: &str) -> Ev {
        Ev::AgentMsg { from: from.into(), to: to.into(), text: text.into(), level: 3, id: String::new(), open: false, fold: false }
    }

    fn text_of(l: &Line) -> String {
        l.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn texts(ls: &[Line]) -> Vec<String> {
        ls.iter().map(|l| text_of(l).trim_end().to_string()).collect()
    }

    /// The columns a row takes, the way the terminal draws it (a wrapped
    /// row may keep the blank after its last word, as everywhere).
    fn cols(l: &Line) -> usize {
        text_of(l).trim_end().width()
    }

    #[test]
    fn the_envelope_is_one_column() {
        assert_eq!(G_ENVELOPE.width(), 1);
        assert_eq!(G_ENVELOPE.chars().collect::<Vec<_>>(), ['\u{2709}', '\u{fe0e}']);
        // one cell in the buffer, the next cell free
        let mut b = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 4, 1));
        b.set_string(0, 0, format!("{G_ENVELOPE}ab"), Style::default());
        assert_eq!((b[(0, 0)].symbol(), b[(1, 0)].symbol(), b[(2, 0)].symbol()), (G_ENVELOPE, "a", "b"));
    }

    #[test]
    fn the_chip_is_tinted_with_a_bold_sender() {
        let ls = l3_lines_as(ChipForm::Tinted, "auth-fix", "release", "", "heads-up", false, 100);
        assert_eq!(texts(&ls), [format!(" {G_ENVELOPE} auth-fix → release"), "  heads-up".to_string()]);
        let spans = &ls[0].spans;
        // the chip at x0, flush with the text (BISE-109): every cell
        // tinted, 1 each side; nothing on the ground before it
        assert_eq!(spans[0].content, "");
        assert_eq!(spans[0].style.bg, None);
        let chip: Vec<&Span> = spans[1..6].iter().collect();
        assert!(chip.iter().all(|s| s.style.bg == Some(chip_bg())), "{spans:?}");
        assert_eq!(chip.iter().map(|s| s.content.width()).sum::<usize>(), " ✉ auth-fix → release ".width());
        assert_eq!(spans.len(), 6, "the chip is alone on its row: {spans:?}");
        let st = |needle: &str| spans.iter().find(|s| s.content.trim() == needle).unwrap().style;
        assert_eq!(st("auth-fix").fg, Some(text()));
        assert!(st("auth-fix").add_modifier.contains(Modifier::BOLD));
        assert_eq!(st("→").fg, Some(faint()));
        assert_eq!(st("release").fg, Some(dim()));
        assert_eq!(st(G_ENVELOPE).fg, Some(dim()));
        // under it at x0+2, on the ground, the dim text (BISE-127)
        let under = &ls[1].spans;
        assert_eq!((under[0].content.as_ref(), under[0].style.bg), ("  ", None));
        assert_eq!((under[1].style.fg, under[1].style.bg), (Some(dim()), None));
        assert_ne!(chip_bg(), Color::Reset);
    }

    #[test]
    fn names_are_cut_at_24_only_when_longer() {
        // BISE-109: room for real names (was 12)
        let ls = l3_lines_as(ChipForm::Tinted, "a-very-long-sender", "a-very-long-receiver", "", "hi", false, 100);
        assert_eq!(texts(&ls)[0], format!(" {G_ENVELOPE} a-very-long-sender → a-very-long-receiver"));
        let ls = l3_lines_as(ChipForm::Tinted, "a-very-long-sender-for-web", "a-very-long-receiver-of-news", "", "hi", false, 100);
        assert_eq!(texts(&ls)[0], format!(" {G_ENVELOPE} a-very-long-sender-for-… → a-very-long-receiver-of…"));
    }

    #[test]
    fn the_text_goes_under_and_stops_after_2_rows() {
        let long = "word ".repeat(60);
        let ls = l3_lines_as(ChipForm::Tinted, "docs", "main", "", &long, false, 80);
        let t = texts(&ls);
        assert_eq!(t.len(), 3, "{t:#?}");
        assert_eq!(t[0], format!(" {G_ENVELOPE} docs → main"));
        assert!(t[1..].iter().all(|r| r.starts_with("  word")), "{t:#?}");
        assert!(t[2].ends_with("word… ▸"), "{t:#?}");
        assert!(ls.iter().all(|l| cols(l) <= 80), "{t:#?}");
        // the text uses the width under the chip, not the room beside it
        assert!(t[1].width() > 70, "{t:#?}");
        // open: the whole text, still at x0+2, `▾` at the end
        let ls = l3_lines_as(ChipForm::Tinted, "docs", "main", "", &long, true, 80);
        let t = texts(&ls);
        assert!(t.len() > 3 && t[1..].iter().all(|r| r.starts_with("  word")), "{t:#?}");
        assert!(t.last().unwrap().ends_with("word ▾") && !t.iter().any(|r| r.contains('…')), "{t:#?}");
        assert_eq!(t.iter().map(|r| r.matches("word").count()).sum::<usize>(), 60);
        // a short text that is cut (not long enough to open): `…` alone
        assert!(!l3_long("x"));
    }

    #[test]
    fn two_chips_of_different_widths_start_their_texts_on_one_column() {
        // the user's screenshot (BISE-127): texts beside chips started at
        // the chip's width
        let a = texts(&l3_lines_as(ChipForm::Tinted, "main", "docs", "", "use v2", false, 100));
        let b = texts(&l3_lines_as(ChipForm::Tinted, "the-auth-fix-agent", "release-notes", "", "heads-up", false, 100));
        assert_eq!((a[1].as_str(), b[1].as_str()), ("  use v2", "  heads-up"));
    }

    #[test]
    fn short_on_room_the_chip_shrinks_and_the_text_stays_at_x0_plus_2() {
        let ls = l3_lines_as(ChipForm::Tinted, "a-long-sender", "a-long-receiv", "", "hello there", false, 63);
        let t = texts(&ls);
        assert_eq!(t, [format!(" {G_ENVELOPE} a-long-sender → a-long-receiv"), "  hello there".to_string()]);
        // 40 ≤ W < 60: names up to 16
        let ls = l3_lines_as(ChipForm::Tinted, "auth-fix-web", "release", "", &"word ".repeat(30), false, 50);
        let t = texts(&ls);
        assert_eq!(t[0], format!(" {G_ENVELOPE} auth-fix-web → release"));
        assert_eq!(t.len(), 3, "{t:#?}");
        assert!(t[1..].iter().all(|r| r.starts_with("  word")), "{t:#?}");
        assert!(t[2].ends_with("… ▸"), "{t:#?}");
        let ls = l3_lines_as(ChipForm::Tinted, "the-auth-fix-for-web", "release", "", "ok", false, 50);
        assert_eq!(texts(&ls)[0], format!(" {G_ENVELOPE} the-auth-fix-fo… → release"));
        // W < 40: compact, still tinted, names up to 10; the text still
        // at x0+2 (BISE-127: at every width)
        let ls = l3_lines_as(ChipForm::Tinted, "auth-fix", "release", "", "ok", false, 30);
        assert_eq!(texts(&ls), [format!("{G_ENVELOPE}auth-fix→release"), "  ok".to_string()]);
        assert!(ls[0].spans[1..].iter().all(|s| s.style.bg == Some(chip_bg())), "{:?}", ls[0]);
        let ls = l3_lines_as(ChipForm::Tinted, "auth-fix-web", "release", "", "ok", false, 30);
        assert_eq!(texts(&ls)[0], format!("{G_ENVELOPE}auth-fix-…→release"));
        // tighter still: the receiver is cut before the sender, never
        // the envelope nor the arrow
        let ls = l3_lines_as(ChipForm::Tinted, "auth-fix", "release", "", "ok", false, 12);
        let head = texts(&ls)[0].clone();
        assert_eq!(head, format!("{G_ENVELOPE}auth-fix→r…"));
        let ls = l3_lines_as(ChipForm::Tinted, "auth-fix", "release", "", "ok", false, 8);
        assert_eq!(texts(&ls)[0], format!("{G_ENVELOPE}auth…→…"));
        for w in 1..120 {
            let ls = l3_lines_as(ChipForm::Tinted, "auth-fix-web", "release-notes", "", &"word ".repeat(40), false, w);
            let t = texts(&ls);
            assert!(t[0].contains(G_ENVELOPE) && t[0].contains('→'), "{w}: {t:#?}");
            assert!(t.len() <= 3 && t[1..].iter().all(|r| r.starts_with("  ")), "{w}: {t:#?}");
            assert!(w < 8 || ls.iter().all(|l| cols(l) <= w), "{w}: {t:#?}");
        }
    }

    #[test]
    fn no_tint_and_ascii_use_brackets() {
        // NO_COLOR / 16 colors: no tint, `[✉︎ sender → receiver]`
        let ls = l3_lines_as(ChipForm::Bracketed, "auth-fix", "release", "", "heads-up", false, 100);
        assert_eq!(texts(&ls), [format!("[{G_ENVELOPE} auth-fix → release]"), "  heads-up".to_string()]);
        assert!(ls.iter().flat_map(|l| l.spans.iter()).all(|s| s.style.bg.is_none()), "{ls:?}");
        assert!(ls[0].spans.iter().any(|s| s.content == "auth-fix" && s.style.add_modifier.contains(Modifier::BOLD)));
        // the same cut rules
        let ls = l3_lines_as(ChipForm::Bracketed, "auth-fix", "release", "", "ok", false, 30);
        assert_eq!(texts(&ls), [format!("[{G_ENVELOPE}auth-fix→release]"), "  ok".to_string()]);
        // ASCII: `[@ sender > receiver]`, whatever the colors
        crate::theme::set_ascii_for_tests(true);
        let ls = l3_lines("auth-fix", "release", "", "heads-up", false, 100);
        let long = l3_lines("a", "b", "", &"word ".repeat(60), false, 80);
        crate::theme::set_ascii_for_tests(false);
        assert_eq!(texts(&ls), ["[@ auth-fix > release]", "  heads-up"]);
        assert!(ls[0].spans.iter().all(|s| s.style.bg.is_none()));
        assert!(texts(&long)[2].ends_with("word... +"), "{:#?}", texts(&long));
        assert!(texts(&long).iter().all(|r| r.is_ascii()));
    }

    #[test]
    fn one_pair_stacks_a_new_pair_after_a_blank_row() {
        // (3 messages: a 4th would fold the run)
        let evs = vec![msg("auth-fix", "release", "heads-up"), msg("release", "auth-fix", "ok"), msg("docs", "main", "v1 or v2?")];
        let first = |evs: &[Ev], i: usize| text_of(&build_rows(evs, i, false, 100, 0)[0]);
        assert!(first(&evs, 0).contains("auth-fix → release"));
        assert!(first(&evs, 1).contains("release → auth-fix"), "the same pair, either way round: no blank row");
        assert_eq!(first(&evs, 2), "", "a new pair: a blank row");
        let evs = vec![msg("docs", "main", "v1 or v2?"), msg("main", "docs", "v2")];
        assert!(first(&evs, 1).contains("main → docs"));
        // main is a plain name here: no `:*`, no accent
        let rows = build_rows(&evs, 1, false, 100, 0);
        assert!(!text_of(&rows[0]).contains(G_MAIN));
        assert!(rows.iter().flat_map(|l| l.spans.iter()).all(|s| s.style.fg != Some(accent()) && s.style.bg != Some(accent())));
    }

    #[test]
    fn one_sender_and_receiver_in_a_row_share_one_chip() {
        // BISE-127: the chip once, then each text on its own rows at x0+2
        let evs = vec![msg("main", "docs", "use v2"), msg("main", "docs", "and the api"), msg("main", "docs", &"word ".repeat(40))];
        let all: Vec<String> = (0..evs.len()).flat_map(|i| texts(&build_rows(&evs, i, false, 60, 0))).collect();
        assert_eq!(all.len(), 5, "{all:#?}");
        assert_eq!(all[..3], [format!(" {G_ENVELOPE} main → docs"), "  use v2".into(), "  and the api".into()]);
        assert!(all[3].starts_with("  word") && all[4].starts_with("  word") && all[4].ends_with("… ▸"), "{all:#?}");
        // the way back is its own chip (no blank row: the same pair)
        let evs = vec![msg("main", "docs", "use v2"), msg("docs", "main", "ok")];
        let all: Vec<String> = (0..2).flat_map(|i| texts(&build_rows(&evs, i, false, 60, 0))).collect();
        assert_eq!(all, [format!(" {G_ENVELOPE} main → docs"), "  use v2".into(), format!(" {G_ENVELOPE} docs → main"), "  ok".into()]);
    }

    #[test]
    fn the_fold_line_sits_at_x0_and_is_cut_from_the_right() {
        let l = fold_line(47, 30, false, false, 0, 100);
        assert_eq!(text_of(&l), format!("{G_CLOSED} 47 messages between 30 agents"));
        assert!(l.spans.iter().all(|s| s.style.bg.is_none()));
        let l = fold_line(47, 30, false, true, 0, 20);
        let t = text_of(&l);
        assert_eq!(t.width(), 20, "{t:?}");
        assert!(t.starts_with(&format!("{G_CLOSED} 47 messages")) && t.contains('…'), "{t:?}");
        // the pulse stays
        assert!(t.ends_with(crate::theme::working_frame(0).0), "{t:?}");
    }
}
