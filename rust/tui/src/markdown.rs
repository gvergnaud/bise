//! Markdown rendering of user and assistant messages.

use crate::theme;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use crate::wrap_line;

// ---- markdown rendering (user + assistant messages) ----
// The wire carries newlines escaped as a literal backslash-n; the TUI
// unescapes and renders a pragmatic markdown subset: fenced code
// blocks, headers, bullet lists, blockquotes, GFM tables, and inline
// bold, italic and code.

pub(crate) fn unescape_md(s: &str) -> String {
    bise_proto::thread::lines::unescape(s)
}

/// A markdown link `[label](url "title")` at `cs[i]` (`[`): the label,
/// the url and where the link ends.
fn md_link_at(cs: &[char], i: usize) -> Option<(String, String, usize)> {
    let close = (i + 1..cs.len()).find(|k| cs[*k] == ']' || cs[*k] == '[')?;
    if cs[close] != ']' || close == i + 1 || cs.get(close + 1) != Some(&'(') {
        return None;
    }
    // the url's parentheses may nest: `(https://en.wikipedia.org/wiki/A_(b))`
    let mut depth = 0usize;
    let mut end = None;
    for (k, c) in cs.iter().enumerate().skip(close + 2) {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => {
                end = Some(k);
                break;
            }
            ')' => depth -= 1,
            _ => {}
        }
    }
    let end = end?;
    let inner: String = cs[close + 2..end].iter().collect();
    let url = inner.split_whitespace().next().unwrap_or("");
    let url = url.strip_prefix('<').and_then(|u| u.strip_suffix('>')).unwrap_or(url);
    let label: String = cs[i + 1..close].iter().collect();
    // an artifact (site/m/artifacts E): `[the mock](artifact:artifacts@v3)`
    if crate::artifacts::parse_url(url).is_some() || crate::links::linkable(url) {
        return Some((label, url.to_string(), end + 1));
    }
    // a link to a local file (BISE-264): `[the guide](docs/guide.md#L12)`
    let file = crate::file_links::target(url)?;
    Some((label, crate::file_links::url_of(&file), end + 1))
}

/// An autolink `<https://…>` at `cs[i]` (`<`): the url and where it ends.
fn autolink_at(cs: &[char], i: usize) -> Option<(String, usize)> {
    let close = (i + 1..cs.len()).find(|k| cs[*k] == '>' || cs[*k] == '<' || cs[*k].is_whitespace())?;
    if cs[close] != '>' {
        return None;
    }
    let url: String = cs[i + 1..close].iter().collect();
    crate::links::linkable(&url).then_some((url, close + 1))
}

/// An artifact named in a reply (site/m/artifacts E) is a chip: an
/// `artifact:` link (its label, else the artifact's title), or a plain
/// link or path the list has registered (its title). False: `url` names
/// no artifact, the caller draws it as before.
fn artifact_chip(spans: &mut Vec<Span<'static>>, label: Option<&str>, url: &str) -> bool {
    let (id, v) = match crate::artifacts::parse_url(url) {
        Some(x) => x,
        None => match crate::artifacts::resolve(url) {
            Some(id) => (id, None),
            None => return false,
        },
    };
    let known = crate::artifacts::get(&id);
    let title = match (label.filter(|l| !l.trim().is_empty() && crate::artifacts::parse_url(url).is_some()), &known) {
        (Some(l), _) => l.to_string(),
        (None, Some(a)) => a.title.clone(),
        (None, None) => id.clone(),
    };
    let gone = known.as_ref().is_some_and(|a| a.gone);
    spans.extend(crate::render::artifact_chip(&title, gone, &crate::artifacts::url_of(&id, v)));
    true
}

/// The spans of a link: its label (inline styles kept), each span tagged
/// and underlined; with OSC 8 off, ` (url)` dim after a label that is not
/// the url.
fn link_spans(spans: &mut Vec<Span<'static>>, label: &str, url: &str, fg: Color, base: Style) {
    let tag = crate::links::add(url);
    let st = crate::links::link_style(base, fg, tag);
    for sp in spans_of(label, st, false) {
        let s = crate::links::link_style(sp.style, sp.style.fg.unwrap_or(fg), tag);
        spans.push(Span::styled(sp.content, s));
    }
    if !crate::links::osc8() && label != url {
        spans.push(Span::styled(format!(" ({})", url), base.fg(theme::dim())));
    }
}

/// The spans of an emphasis (`**…**`, `*…*`) in `st`: its links, code
/// and nested emphasis parsed as anywhere else, so `**http://…**` is a
/// link; a link inside keeps the emphasis's color.
fn emphasis(spans: &mut Vec<Span<'static>>, inner: &str, st: Style, links: bool) {
    for mut sp in spans_of(inner, st, links) {
        if crate::links::tag_of(sp.style.add_modifier) > 0 && sp.style.fg == Some(theme::dim()) {
            sp.style = sp.style.fg(st.fg.unwrap_or(theme::text()));
        }
        spans.push(sp);
    }
}

// inline styles in the OpenCode markdown colors: **strong** is
// markdownStrong (orange), *emph* is markdownEmph (yellow), `code` is
// markdownCode (green); links (`[label](url)`, `<url>`, a bare
// http(s) url) underlined, tagged for the click and OSC 8 (links.rs)
pub(crate) fn inline_spans(s: &str, base: Style) -> Vec<Span<'static>> {
    spans_of(s, base, true)
}

/// The inline spans of `s`; `links`: false inside a link's label.
fn spans_of(s: &str, base: Style, links: bool) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '[' && links {
            if let Some((label, url, end)) = md_link_at(&cs, i) {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                if !artifact_chip(&mut spans, Some(&label), &url) {
                    link_spans(&mut spans, &label, &url, base.fg.unwrap_or(theme::text()), base);
                }
                i = end;
                continue;
            }
        }
        if c == '<' && links {
            if let Some((url, end)) = autolink_at(&cs, i) {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                link_spans(&mut spans, &url, &url, base.fg.unwrap_or(theme::text()), base);
                i = end;
                continue;
            }
        }
        if links && (c == 'h' || c == 'H') {
            if let Some(n) = crate::links::bare_at(&cs, i) {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                let url: String = cs[i..i + n].iter().collect();
                if !artifact_chip(&mut spans, None, &url) {
                    let tag = crate::links::add(&url);
                    spans.push(Span::styled(url, crate::links::link_style(base, theme::dim(), tag)));
                }
                i += n;
                continue;
            }
        }
        // a bare path to a local file (BISE-264: file_links.rs)
        if links {
            if let Some((n, url)) = crate::file_links::bare_at(&cs, i) {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                let shown: String = cs[i..i + n].iter().collect();
                if !artifact_chip(&mut spans, None, &shown) {
                    let tag = crate::links::add(&url);
                    spans.push(Span::styled(shown, crate::links::link_style(base, theme::dim(), tag)));
                }
                i += n;
                continue;
            }
        }
        if c == '`' {
            if let Some(j) = (i + 1..cs.len()).find(|k| cs[*k] == '`') {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                let code: String = cs[i + 1..j].iter().collect();
                let mut st = Style::default().fg(theme::ok()).add_modifier(Modifier::BOLD);
                // a code span that is a url, or a local file (BISE-264), is its link
                if links && crate::links::is_bare_url(code.trim()) {
                    let tag = crate::links::add(code.trim());
                    st = crate::links::link_style(st, theme::ok(), tag);
                } else if let Some(file) = crate::file_links::target(code.trim()).filter(|_| links) {
                    let tag = crate::links::add(&crate::file_links::url_of(&file));
                    st = crate::links::link_style(st, theme::ok(), tag);
                }
                spans.push(Span::styled(code, st));
                i = j + 1;
                continue;
            }
        }
        if c == '*' {
            if i + 1 < cs.len() && cs[i + 1] == '*' {
                if let Some(j) =
                    (i + 3..cs.len()).find(|k| cs[*k] == '*' && cs.get(k + 1) == Some(&'*'))
                {
                    if !plain.is_empty() {
                        spans.push(Span::styled(std::mem::take(&mut plain), base));
                    }
                    let inner: String = cs[i + 2..j].iter().collect();
                    emphasis(&mut spans, &inner, base.add_modifier(Modifier::BOLD).fg(theme::accent()), links);
                    i = j + 2;
                    continue;
                }
            } else if let Some(j) = (i + 2..cs.len()).find(|k| {
                cs[*k] == '*' && cs.get(k - 1) != Some(&'*') && cs.get(k + 1) != Some(&'*')
            }) {
                if !plain.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut plain), base));
                }
                let inner: String = cs[i + 1..j].iter().collect();
                emphasis(&mut spans, &inner, base.add_modifier(Modifier::ITALIC).fg(theme::text()), links);
                i = j + 1;
                continue;
            }
        }
        plain.push(c);
        i += 1;
    }
    if !plain.is_empty() {
        spans.push(Span::styled(plain, base));
    }
    spans
}

/// `text` as rows: prose wrapped at `prose` columns; a table (BISE-87)
/// as wide as it needs up to `wide` (the code measure), laid out here so
/// no later wrap cuts its rows.
pub(crate) fn md_lines(text: &str, prose: usize, wide: usize) -> Vec<Line<'static>> {
    // a copy of a code block gives its lines as written (tabs kept)
    let written: Vec<&str> = text.split('\n').collect();
    // the open block (codeblock.rs): its tag, its colored lines, its code
    let mut tag = String::new();
    let mut hl: Vec<Vec<Span<'static>>> = Vec::new();
    let mut code: Vec<String> = Vec::new();
    let boxed = wide.min(crate::render::CODE_MAX);
    let text = crate::sanitize::clean(text, crate::sanitize::TAB_CODE);
    let text: &str = &text;
    let mut done: Vec<Line<'static>> = Vec::new();
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut in_code = false;
    // BISE-276: a fence's language colors its lines (syntax.rs)
    let mut lang: Option<&'static crate::syntax::Lang> = None;
    let mut state = crate::syntax::State::Normal;
    let raws: Vec<&str> = text.split('\n').collect();
    let mut k = 0;
    while k < raws.len() {
        let line = raws[k].trim_end();
        k += 1;
        // a GFM table: a row, then its delimiter row (never in a fence)
        if !in_code {
            if let Some(n) = table_at(&raws[k - 1..]) {
                done.extend(out.drain(..).flat_map(|l| wrap_line(l, prose)));
                done.extend(table::lines(&raws[k - 1..k - 1 + n], prose, wide.max(prose)));
                k += n - 1;
                continue;
            }
        }
        if line.starts_with("```") {
            in_code = !in_code;
            if in_code {
                // the prose before the block, wrapped; the block is a box
                // (codeblock.rs) at the code measure
                done.extend(out.drain(..).flat_map(|l| wrap_line(l, prose)));
                let info = line.trim_start_matches('`').trim();
                tag = info.split(|c: char| c.is_whitespace() || c == ',' || c == '{').next().unwrap_or("").to_string();
                lang = crate::syntax::lang_of(info);
            } else {
                done.extend(crate::codeblock::lines(&tag, &hl, code.join("\n"), boxed));
                hl.clear();
                code.clear();
                lang = None;
            }
            state = crate::syntax::State::Normal;
            continue;
        }
        if in_code {
            // the line as written; the cleaned one when the clean moved
            // the lines (an escape sequence across a newline)
            let w = if written.len() == raws.len() { written[k - 1] } else { raws[k - 1] };
            code.push(w.strip_suffix('\r').unwrap_or(w).to_string());
            let Some(l) = lang else {
                hl.push(vec![Span::styled(line.to_string(), Style::default().fg(theme::text()).bg(Color::Reset))]);
                continue;
            };
            let (runs, next) = crate::syntax::line(l, state, line);
            state = next;
            let mut spans = Vec::new();
            let mut rest = line;
            for (n, t) in runs {
                let b = rest.char_indices().nth(n).map(|(b, _)| b).unwrap_or(rest.len());
                spans.push(Span::styled(rest[..b].to_string(), crate::syntax::style(t).bg(Color::Reset)));
                rest = &rest[b..];
            }
            hl.push(spans);
            continue;
        }
        if line.is_empty() {
            out.push(Line::from(""));
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let t = line.trim_start();
        let base = Style::default().fg(theme::text());
        if t.starts_with('#') {
            let level = t.chars().take_while(|c| *c == '#').count();
            let head = t[level..].trim_start();
            out.push(Line::from(Span::styled(
                head.to_string(),
                Style::default().fg(theme::text()).add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        // OpenCode markdownListEnumeration: "N. item" in the info color
        let dot = t
            .char_indices()
            .find(|(i, c)| *i > 0 && *c == '.' && t[i + 1..].starts_with(' '));
        if let Some((i, _)) = dot {
            if t[..i].chars().all(|c| c.is_ascii_digit()) {
                out.push(Line::from_iter(
                    std::iter::once(Span::styled(
                        format!("  {} ", &t[..i + 1]),
                        Style::default().fg(theme::dim()),
                    ))
                    .chain(inline_spans(t[i + 1..].trim_start(), base)),
                ));
                continue;
            }
        }
        if indent == 0 && (t.starts_with("- ") || t.starts_with("* ")) {
            out.push(Line::from_iter(
                std::iter::once(Span::styled("  - ", Style::default().fg(theme::accent())))
                    .chain(inline_spans(&t[2..], base)),
            ));
            continue;
        }
        if indent == 0 && t.starts_with("> ") {
            out.push(Line::from_iter(
                std::iter::once(Span::styled("  | ", Style::default().fg(theme::text())))
                    .chain(inline_spans(&t[2..], Style::default().fg(theme::text()))),
            ));
            continue;
        }
        out.push(Line::from(inline_spans(line, base)));
    }
    done.extend(out.into_iter().flat_map(|l| wrap_line(l, prose)));
    // a block still open (a reply streaming in, a fence never closed)
    if in_code {
        done.extend(crate::codeblock::lines(&tag, &hl, code.join("\n"), boxed));
    }
    done
}

// ---- GFM tables (BISE-87) ----

/// A table starts at `rows[0]`: a row with `|`, then a delimiter row
/// with as many cells. How many lines it takes (header, delimiter, the
/// rows up to a blank line or a line without `|`), else None.
fn table_at(rows: &[&str]) -> Option<usize> {
    let head = rows.first()?.trim();
    if !head.contains('|') || head.starts_with("```") {
        return None;
    }
    let aligns = table::delimiter(rows.get(1)?)?;
    if aligns.len() != table::cells(head).len() {
        return None;
    }
    let body = rows[2..]
        .iter()
        .take_while(|r| {
            let t = r.trim();
            !t.is_empty() && t.contains('|') && !t.starts_with("```")
        })
        .count();
    Some(2 + body)
}

mod table {
    use super::inline_spans;
    use crate::theme::{ascii_mode, dim, text};
    use super::wrap_line;
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use unicode_width::UnicodeWidthStr;

    /// The blank between two columns.
    const GAP: usize = 2;
    /// A column never gets narrower than its longest word, at least this
    /// and at most `MAX_FLOOR` (or its natural width): narrower, the
    /// table turns into blocks.
    const MIN_COL: usize = 8;
    const MAX_FLOOR: usize = 12;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Align {
        Left,
        Center,
        Right,
    }

    /// The cells of a row: split on `|` outside backticks (`\|` is a
    /// pipe), the outer pipes dropped, each cell trimmed.
    pub(super) fn cells(row: &str) -> Vec<String> {
        let t = row.trim();
        let t = t.strip_prefix('|').unwrap_or(t);
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut code = false;
        let mut chars = t.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' if chars.peek() == Some(&'|') => {
                    cur.push('|');
                    chars.next();
                }
                '`' => {
                    code = !code;
                    cur.push(c);
                }
                '|' if !code => out.push(std::mem::take(&mut cur).trim().to_string()),
                _ => cur.push(c),
            }
        }
        // a trailing pipe closes the last cell; without it, what is left
        // is one more cell (a row still streaming counts too)
        if !cur.trim().is_empty() || !t.trim_end().ends_with('|') {
            out.push(cur.trim().to_string());
        }
        out
    }

    /// The alignments of a delimiter row (`| :--- | :---: | ---: |`),
    /// None when it is not one.
    pub(super) fn delimiter(row: &str) -> Option<Vec<Align>> {
        let t = row.trim();
        if !t.contains('-') {
            return None;
        }
        let cs = cells(t);
        if cs.is_empty() {
            return None;
        }
        cs.iter()
            .map(|c| {
                let (l, r) = (c.starts_with(':'), c.len() > 1 && c.ends_with(':'));
                let dashes = c.trim_start_matches(':').trim_end_matches(':');
                if dashes.is_empty() || !dashes.chars().all(|ch| ch == '-') {
                    return None;
                }
                Some(match (l, r) {
                    (true, true) => Align::Center,
                    (false, true) => Align::Right,
                    _ => Align::Left,
                })
            })
            .collect()
    }

    fn width_of(spans: &[Span<'static>]) -> usize {
        spans.iter().map(|s| s.content.width()).sum()
    }

    /// The column widths that fit `width`: each column its natural width
    /// when all fit; else the widest ones shrink to one cap, never under
    /// their `floor`. None when even the floors do not fit.
    pub(super) fn fit(natural: &[usize], floor: &[usize], width: usize) -> Option<Vec<usize>> {
        let gaps = GAP * natural.len().saturating_sub(1);
        let total = |ws: &[usize]| ws.iter().sum::<usize>() + gaps;
        if total(natural) <= width {
            return Some(natural.to_vec());
        }
        let floor: Vec<usize> = natural.iter().zip(floor).map(|(&n, &f)| n.min(f)).collect();
        if total(&floor) > width {
            return None;
        }
        let capped = |cap: usize| -> Vec<usize> {
            natural.iter().zip(&floor).map(|(&n, &f)| n.min(cap).max(f)).collect()
        };
        // the largest cap that fits
        let (mut lo, mut hi) = (0, natural.iter().copied().max().unwrap_or(0));
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if total(&capped(mid)) <= width {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let mut ws = capped(lo);
        // the columns left above the cap share what remains, one each
        let mut left = width - total(&ws);
        for (w, &n) in ws.iter_mut().zip(natural) {
            if left == 0 {
                break;
            }
            if *w < n {
                *w += 1;
                left -= 1;
            }
        }
        Some(ws)
    }

    /// One cell wrapped into its column by words: rows of spans,
    /// trailing blanks off, no wrap mark.
    fn wrap_cell(spans: Vec<Span<'static>>, w: usize) -> Vec<Vec<Span<'static>>> {
        wrap_line(Line::from(spans), w)
            .into_iter()
            .map(|l| {
                let mut sp = l.spans;
                while let Some(last) = sp.last_mut() {
                    let t = last.content.trim_end().to_string();
                    if t.is_empty() {
                        sp.pop();
                        continue;
                    }
                    last.content = t.into();
                    break;
                }
                sp
            })
            .collect()
    }

    /// The longest word of a cell, as its narrowest column (8 to 12).
    fn floor_of(spans: &[Span<'static>]) -> usize {
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let word = text.split_whitespace().map(|w| w.width()).max().unwrap_or(0);
        MIN_COL.max(word.min(MAX_FLOOR))
    }

    /// The rows of a table (header, delimiter, body lines): as wide as it
    /// needs up to `wide` columns, the header bold over a faint rule (one
    /// segment per column), the columns aligned by display width, a long
    /// cell wrapped inside its column (then a blank line between rows).
    /// Too many columns for `wide`: one block per row, wrapped at `prose`.
    pub(super) fn lines(src: &[&str], prose: usize, wide: usize) -> Vec<Line<'static>> {
        let base = Style::default().fg(text());
        let head_st = base.add_modifier(Modifier::BOLD);
        let head = cells(src[0]);
        let aligns = delimiter(src[1]).unwrap_or_default();
        let n = head.len();
        let body: Vec<Vec<String>> = src[2..]
            .iter()
            .map(|r| {
                let mut cs = cells(r);
                cs.resize(n, String::new());
                cs
            })
            .collect();
        let head_spans: Vec<Vec<Span<'static>>> = head.iter().map(|c| inline_spans(c, head_st)).collect();
        let body_spans: Vec<Vec<Vec<Span<'static>>>> =
            body.iter().map(|r| r.iter().map(|c| inline_spans(c, base)).collect()).collect();
        let mut natural: Vec<usize> = head_spans.iter().map(|s| width_of(s).max(1)).collect();
        let mut floor: Vec<usize> = head_spans.iter().map(|s| floor_of(s)).collect();
        for r in &body_spans {
            for (c, s) in r.iter().enumerate() {
                natural[c] = natural[c].max(width_of(s));
                floor[c] = floor[c].max(floor_of(s));
            }
        }
        let Some(ws) = fit(&natural, &floor, wide.max(1)) else {
            return blocks(&head_spans, &body_spans, prose.max(1));
        };
        let mut out = row_lines(&head_spans, &ws, &aligns);
        let rule = if ascii_mode() { "-" } else { "─" };
        let segs: Vec<String> = ws.iter().map(|&w| rule.repeat(w)).collect();
        out.push(Line::from(Span::styled(segs.join(&" ".repeat(GAP)), Style::default().fg(crate::theme::rule()))));
        let rows: Vec<Vec<Line<'static>>> = body_spans.iter().map(|r| row_lines(r, &ws, &aligns)).collect();
        // a wrapped row: a blank line between rows keeps them apart
        let spaced = rows.iter().any(|r| r.len() > 1);
        for (i, r) in rows.into_iter().enumerate() {
            if spaced && i > 0 {
                out.push(Line::from(""));
            }
            out.extend(r);
        }
        out
    }

    /// One table row: its cells wrapped, side by side, aligned.
    fn row_lines(row: &[Vec<Span<'static>>], ws: &[usize], aligns: &[Align]) -> Vec<Line<'static>> {
        let wrapped: Vec<Vec<Vec<Span<'static>>>> =
            row.iter().zip(ws).map(|(s, &w)| wrap_cell(s.clone(), w)).collect();
        let height = wrapped.iter().map(|c| c.len()).max().unwrap_or(1);
        let last = ws.len().saturating_sub(1);
        (0..height)
            .map(|y| {
                let mut spans: Vec<Span<'static>> = Vec::new();
                for (c, cell) in wrapped.iter().enumerate() {
                    let part = cell.get(y).cloned().unwrap_or_default();
                    let room = ws[c].saturating_sub(width_of(&part));
                    let (l, r) = match aligns.get(c).copied().unwrap_or(Align::Left) {
                        Align::Left => (0, room),
                        Align::Right => (room, 0),
                        Align::Center => (room / 2, room - room / 2),
                    };
                    if c > 0 {
                        spans.push(Span::raw(" ".repeat(GAP)));
                    }
                    if l > 0 {
                        spans.push(Span::raw(" ".repeat(l)));
                    }
                    spans.extend(part);
                    if r > 0 && c < last {
                        spans.push(Span::raw(" ".repeat(r)));
                    }
                }
                Line::from(spans)
            })
            .collect()
    }

    /// Too many columns: one block per row. Its first cell is the title
    /// (bold); then `  key  value` for each other column, the keys dim and
    /// padded to the longest, the value wrapped at `prose` under its
    /// column; a blank line between blocks.
    fn blocks(head: &[Vec<Span<'static>>], body: &[Vec<Vec<Span<'static>>>], prose: usize) -> Vec<Line<'static>> {
        let key_st = Style::default().fg(dim());
        let keys: Vec<String> = head.iter().map(|k| k.iter().map(|s| s.content.as_ref()).collect()).collect();
        let key_w = keys.iter().skip(1).map(|k| k.width()).max().unwrap_or(0);
        let indent = 2 + key_w + GAP;
        let mut out = Vec::new();
        for (i, r) in body.iter().enumerate() {
            if i > 0 {
                out.push(Line::from(""));
            }
            let title: Vec<Span<'static>> = r[0]
                .iter()
                .map(|s| Span::styled(s.content.clone(), s.style.add_modifier(Modifier::BOLD)))
                .collect();
            out.extend(wrap_line(Line::from(title), prose));
            for (k, v) in keys.iter().zip(r).skip(1) {
                let pad = key_w - k.width();
                let mut first = true;
                for part in wrap_cell(v.clone(), prose.saturating_sub(indent).max(1)) {
                    let lead = if first {
                        Span::styled(format!("  {}{}{}", k, " ".repeat(pad), " ".repeat(GAP)), key_st)
                    } else {
                        Span::raw(" ".repeat(indent))
                    };
                    first = false;
                    let mut spans = vec![lead];
                    spans.extend(part);
                    out.push(Line::from(spans));
                }
            }
        }
        if body.is_empty() {
            out.push(Line::from(Span::styled(keys.join("  "), key_st)));
        }
        out
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;
    use ratatui::Terminal;
    use unicode_width::UnicodeWidthStr;

    /// The rows as drawn in a TestBackend `width` columns wide.
    fn drawn(text: &str, width: usize) -> Vec<String> {
        let rows = md_lines(text, width.min(76), width);
        let h = rows.len().max(1) as u16;
        let mut term = Terminal::new(TestBackend::new(width as u16, h)).unwrap();
        term.draw(|f| f.render_widget(Paragraph::new(rows), f.area())).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| {
                let mut s = String::new();
                let mut x = 0;
                while x < width as u16 {
                    let sym = buf[(x, y)].symbol().to_string();
                    x += sym.width().max(1) as u16;
                    s.push_str(&sym);
                }
                s.trim_end().to_string()
            })
            .collect()
    }

    fn texts(text: &str, width: usize) -> Vec<String> {
        md_lines(text, width.min(76), width)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().trim_end().to_string())
            .collect()
    }

    const T: &str = "| name | status | p95 |\n|:---|:---:|---:|\n| users | done | 180 ms |\n| billing | working | 2 s |";

    #[test]
    fn columns_align_as_the_delimiter_says() {
        let rows = drawn(T, 76);
        assert_eq!(
            rows,
            [
                "name     status      p95",
                "───────  ───────  ──────",
                "users     done    180 ms",
                "billing  working     2 s",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
            "{rows:#?}"
        );
        // the header is bold, the rule a quiet line
        let ls = md_lines(T, 76, 100);
        assert!(ls[0].spans.iter().filter(|s| !s.content.trim().is_empty()).all(|s| s.style.add_modifier.contains(Modifier::BOLD)));
        assert_eq!(ls[1].spans[0].style.fg, Some(crate::theme::rule()));
    }

    #[test]
    fn wide_chars_align_by_display_width() {
        let t = "| k | v |\n|---|---|\n| 日本語 | a |\n| 👍 ok | b |\n| x | c |";
        let rows = drawn(t, 76);
        // the second column starts at the same display column on every row
        let col = |r: &str, needle: char| -> usize {
            let i = r.find(needle).unwrap();
            r[..i].width()
        };
        assert_eq!(col(&rows[2], 'a'), col(&rows[4], 'c'), "{rows:#?}");
        assert_eq!(col(&rows[3], 'b'), col(&rows[4], 'c'), "{rows:#?}");
        assert_eq!(col(&rows[0], 'v'), col(&rows[4], 'c'), "{rows:#?}");
    }

    #[test]
    fn a_long_cell_wraps_inside_its_column() {
        let long = "the session cookie is set with SameSite=None and without Secure so the browser drops it";
        let t = format!("| id | why | fix |\n|---|---|---|\n| 1 | {long} | set Secure |\n| 2 | short | none |");
        let rows = drawn(&t, 40);
        assert!(rows.iter().all(|r| r.width() <= 40), "{rows:#?}");
        // the long cell takes several rows, its text stays in its column
        let why_col = rows[0].find("why").unwrap();
        let cont: Vec<&String> = rows[2..].iter().take_while(|r| !r.is_empty()).collect();
        assert!(cont.len() >= 3, "{rows:#?}");
        for r in &cont[1..] {
            assert!(r[..why_col].trim().is_empty(), "{rows:#?}");
        }
        let joined: String = cont.iter().map(|r| r[why_col..].split("  ").next().unwrap().trim()).collect::<Vec<_>>().join(" ");
        assert!(joined.starts_with("the session cookie"), "{joined}");
        assert!(rows.iter().any(|r| r.contains("set Secure")), "{rows:#?}");
        // a wrapped row: one blank line between the rows of that table
        let blank = rows.iter().position(|r| r.is_empty()).unwrap();
        assert!(rows[blank + 1].starts_with("2 "), "{rows:#?}");
        // no wrap: no blank lines
        assert!(!drawn(T, 76).iter().any(|r| r.is_empty()));
        // a table wider than the prose measure runs to the code measure
        let wide = "| a | b |\n|---|---|\n| ".to_string() + &"x".repeat(50) + " | " + &"y".repeat(40) + " |";
        let rows = drawn(&wide, 100);
        assert_eq!(rows[2].width(), 92, "{rows:#?}");
    }

    #[test]
    fn too_many_columns_turn_into_blocks() {
        let head = (0..10).map(|i| format!("column{i}")).collect::<Vec<_>>().join(" | ");
        let delim = ["---"; 10].join(" | ");
        let row = (0..10).map(|i| format!("value{i}")).collect::<Vec<_>>().join(" | ");
        let t = format!("| {head} |\n| {delim} |\n| {row} |\n| {row} |");
        let rows = texts(&t, 30);
        // the first cell is the title (bold), then `  key  value`, the
        // keys padded to the longest, a blank line between blocks
        assert_eq!(rows[0], "value0");
        assert_eq!(rows[1], "  column1  value1");
        assert_eq!(rows[9], "  column9  value9");
        assert_eq!(rows[10], "");
        assert_eq!(rows.len(), 21, "{rows:#?}");
        let ls = md_lines(&t, 30, 30);
        assert!(ls[0].spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(ls[1].spans[0].style.fg, Some(crate::theme::dim()));
        // a long value wraps under its column
        let t = "| name | status | age | context | note | owner | branch | model | tokens |\n|---|---|---|---|---|---|---|---|---|\n| auth-fix | working | 12m | 21% | the login breaks on safari because of the cookie | main | fix/auth | opus | 12k |";
        let rows = texts(t, 30);
        assert_eq!(rows[0], "auth-fix");
        let note: Vec<&String> = rows.iter().skip_while(|r| !r.starts_with("  note")).take_while(|r| !r.starts_with("  owner")).collect();
        assert!(note.len() >= 2, "{rows:#?}");
        let col = note[0].find("the").unwrap();
        assert!(note[1..].iter().all(|r| r[..col].trim().is_empty() && r.width() <= 30), "{rows:#?}");
    }

    #[test]
    fn cells_keep_their_inline_markdown() {
        let t = "| a | b |\n|---|---|\n| **bold** | `code` |";
        let ls = md_lines(t, 76, 100);
        let spans = &ls[2].spans;
        let bold = spans.iter().find(|s| s.content == "bold").unwrap();
        assert!(bold.style.add_modifier.contains(Modifier::BOLD));
        assert!(spans.iter().any(|s| s.content == "code"));
        // an escaped pipe and a pipe in code stay in their cell
        let t = "| a | b |\n|---|---|\n| x \\| y | `p|q` |";
        let rows = texts(t, 76);
        assert!(rows[2].starts_with("x | y") && rows[2].ends_with("p|q"), "{rows:#?}");
    }

    #[test]
    fn a_streamed_half_table_never_panics() {
        // every prefix of a message with a table, at several widths
        let msg = format!("before\n\n{}\n\nafter **done**", T.replace("users", "日本 users 👍"));
        let cs: Vec<char> = msg.chars().collect();
        for n in 0..=cs.len() {
            let part: String = cs[..n].iter().collect();
            for w in [1, 5, 12, 40, 76] {
                let _ = md_lines(&part, w, w);
                let _ = md_lines(&part, w.min(3), w);
            }
        }
        // a header whose delimiter hasn't arrived yet is a plain line
        let rows = texts("| name | status |\n|---", 76);
        assert_eq!(rows[0], "| name | status |");
        // a row cut halfway gets empty cells
        let rows = texts("| a | b | c |\n|---|---|---|\n| 1 | 2", 76);
        assert_eq!(rows[2], "1  2");
    }

    #[test]
    fn code_fences_with_a_language_are_colored() {
        let ls = md_lines("```ts\nconst x = 1 // c\n```\n```\nconst y\n```", 80, 80);
        let fg = |row: usize, w: &str| ls[row].spans.iter().find(|s| s.content.contains(w)).and_then(|s| s.style.fg);
        assert_eq!(fg(1, "const"), Some(theme::syntax_keyword()));
        assert_eq!(fg(1, "1"), Some(theme::syntax_number()));
        assert_eq!(fg(1, "// c"), Some(theme::syntax_comment()));
        // no tag: plain text, as before
        assert_eq!(fg(4, "const y"), Some(theme::text()));
        // a box at the code measure: the tag in its top border
        let text = |r: usize| -> String { ls[r].spans.iter().map(|s| s.content.as_ref()).collect() };
        assert_eq!(text(0), format!("╭─ ts {}╮", "─".repeat(73)));
        assert_eq!(text(1), format!("│ const x = 1 // c{} │", " ".repeat(60)));
        assert_eq!(text(2), format!("╰{}╯", "─".repeat(78)));
        assert_eq!(text(3), format!("╭{}╮", "─".repeat(78)));
    }

    #[test]
    fn a_long_code_line_wraps_inside_the_box_with_the_wrap_mark() {
        let long = "let total = first_value + second_value + third_value + fourth_value;";
        let ls = md_lines(&format!("```rust\n{long}\n```"), 40, 40);
        let text: Vec<String> = ls.iter().map(crate::feedsel::line_text).collect();
        assert_eq!(text.len(), 5, "{text:#?}");
        assert!(text.iter().all(|t| t.width() == 40), "{text:#?}");
        assert!(text[2].starts_with("│ » ") && text[3].starts_with("│ » "), "{text:#?}");
        assert!(crate::feedsel::is_soft(&ls[2]) && crate::feedsel::is_soft(&ls[3]));
        // the selection copies the code, not the borders nor the padding
        let sel = crate::feedsel::selection_text(&ls[1..4], 0, usize::MAX);
        assert_eq!(sel, long);
    }

    #[test]
    fn in_ascii_a_wrapped_code_line_hangs_on_two_blanks_with_no_mark() {
        crate::theme::set_ascii_for_tests(true);
        let long = "let total = first_value + second_value + third_value + fourth_value;";
        let ls = md_lines(&format!("```rust\n{long}\n```"), 40, 40);
        crate::theme::set_ascii_for_tests(false);
        let text: Vec<String> = ls.iter().map(crate::feedsel::line_text).collect();
        assert!(text[0].starts_with("+- rust -"), "{text:#?}");
        assert!(text[2].starts_with("|   ") && !text[2].contains('}') && !text[2].contains('»'), "{text:#?}");
    }

    #[test]
    fn a_block_keeps_its_code_as_written_for_the_copy() {
        let (ls, found) = crate::codeblock::collect(|| md_lines("say\n```make\nall:\n\tcc main.c\n```\nend", 76, 100));
        let blocks = crate::codeblock::locate(&ls, found);
        assert_eq!(blocks.len(), 1);
        let b = &blocks[0];
        assert_eq!((b.row, b.len, b.x, b.w), (1, 4, 0, 100));
        assert_eq!(b.code, "all:\n\tcc main.c");
        // no language known: plain text, its tag still in the border
        assert!(crate::feedsel::line_text(&ls[1]).starts_with("╭─ make ─"));
        // a fence never closed (a reply streaming in) is a box too
        let ls = md_lines("```ts\nconst a", 76, 76);
        assert_eq!(ls.len(), 3);
        assert!(crate::feedsel::line_text(&ls[2]).starts_with('╰'));
    }

    #[test]
    fn no_table_inside_a_code_fence() {
        let t = format!("```\n{T}\n```");
        let rows = texts(&t, 76);
        assert!(rows.iter().any(|r| r.contains("| name | status | p95 |")), "{rows:#?}");
        // the rows are the code box's (its borders), never a table's
        assert!(!rows.iter().any(|r| r.contains('┼') || r.contains("─┬")), "{rows:#?}");
    }

    #[test]
    fn column_widths_fit() {
        use super::table::fit;
        assert_eq!(fit(&[4, 5], &[8, 8], 76), Some(vec![4, 5]));
        // the widest shrinks first; the narrow ones keep their width
        assert_eq!(fit(&[3, 60, 10], &[8, 12, 8], 40), Some(vec![3, 23, 10]));
        let ws = fit(&[50, 60, 10], &[8, 8, 8], 40).unwrap();
        assert_eq!(ws.iter().sum::<usize>() + 4, 40);
        assert_eq!(ws[2], 10);
        // never under the floors: too many columns, no fit
        assert_eq!(fit(&[20; 10], &[8; 10], 30), None);
    }
}
