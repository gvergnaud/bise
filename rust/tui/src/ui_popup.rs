//! The slash-command popup above the composer (OpenCode autocomplete),
//! moved out of ui.rs unchanged.

use super::truncate_left;
use crate::*;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

pub(super) fn draw_popup(app: &App, frame: &mut Frame, prompt: Rect) {
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
                // cut at a whole word, never inside one (designer m_53332)
                spans.push(Span::styled(crate::render::truncate_words(&c.desc, room), desc_style));
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
