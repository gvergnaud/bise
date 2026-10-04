//! The line of an answer main gave an agent for you (level 2, C2
//! `answered`): closed, one or two rows, the gist of the question and
//! of the answer, `:* docs asked: v1 or v2? · i answered: v2 ▸`; open,
//! the whole question, answer and why under the rail, their commands
//! and paths in code style.

use crate::markdown::md_lines;
use crate::render::{cut_row, hung_rows};
use crate::theme::*;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// The most a gist of the question shows, in chars (`…` included).
const Q_GIST: usize = 64;
/// The most a gist of the answer shows.
const A_GIST: usize = 40;
/// Closed, the line takes at most this many rows.
const ROWS: usize = 2;
const RAIL: &str = " │ ";

/// `text` without the `name: ` an agent puts before its own words
/// (`sock-path: gate quick fails…`): the line already names it.
pub(crate) fn without_name<'a>(text: &'a str, name: &str) -> &'a str {
    let t = text.trim();
    if name.is_empty() {
        return t;
    }
    match t.get(..name.len()) {
        Some(head) if head.eq_ignore_ascii_case(name) => {
            t[name.len()..].strip_prefix(':').map_or(t, str::trim_start)
        }
        _ => t,
    }
}

/// `s` on one line: whitespace runs to one space, no backticks.
fn flat(s: &str) -> String {
    s.replace('`', "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `s` without its `( … )` asides.
fn drop_asides(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    // `50 MB (…).` leaves `50 MB .`
    flat(&out).replace(" .", ".").replace(" ,", ",").replace(" ?", "?").replace(" :", ":")
}

/// The sentences of a flat `s`, each with its end mark (`?`, `!`,
/// `.`); `colon`: a `:` or `;` ends one too (an answer's `v2: …`).
fn sentences(s: &str, colon: bool) -> Vec<String> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, &c) in cs.iter().enumerate() {
        cur.push(c);
        let ends = matches!(c, '.' | '?' | '!') || (colon && matches!(c, ':' | ';'));
        if ends && cs.get(i + 1).is_none_or(|n| *n == ' ') {
            out.push(std::mem::take(&mut cur).trim().to_string());
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// `s` in at most `cap` chars, cut at the end of a word with `…`.
fn cut_words(s: &str, cap: usize) -> String {
    if s.chars().count() <= cap {
        return s.to_string();
    }
    let e = ellipsis();
    let keep = cap.saturating_sub(e.chars().count()).max(1);
    let head: String = s.chars().take(keep + 1).collect();
    // the last whole word: up to the last space that fits
    let head = match head.rfind(' ') {
        Some(sp) if sp > 0 => head[..sp].to_string(),
        _ => s.chars().take(keep).collect(),
    };
    let head = head.trim_end_matches([' ', ',', ';', ':', '-', '—']);
    format!("{}{}", head, e)
}

/// The gist of a sentence: its end mark kept only when it is `?` or `!`.
fn gist_of(sentence: &str, cap: usize) -> String {
    let s = sentence.trim_end_matches(['.', ':', ';']).trim_end();
    cut_words(s, cap)
}

/// The gist of the question: its first real question (a sentence of 5
/// words or more that ends with `?`; `which do you want?` is not one),
/// else its first sentence; no asides; cut at a word.
pub(crate) fn question_gist(agent: &str, question: &str) -> String {
    let all = sentences(&drop_asides(&flat(without_name(question, agent))), false);
    let pick = all
        .iter()
        .find(|s| s.ends_with('?') && s.split_whitespace().count() >= 5)
        .or_else(|| all.first());
    pick.map_or_else(String::new, |s| gist_of(s, Q_GIST))
}

/// The gist of the answer: its first sentence or clause before a `:`
/// (`run it through launchd: D=…` → `run it through launchd`).
pub(crate) fn answer_gist(answer: &str) -> String {
    let all = sentences(&drop_asides(&flat(without_name(answer, "main"))), true);
    all.first().map_or_else(String::new, |s| gist_of(s, A_GIST))
}

/// Whether the gists show all of the question and the answer.
fn gists_say_all(agent: &str, question: &str, answer: &str) -> bool {
    let same = |gist: String, full: &str| {
        let full = flat(full);
        gist == full || gist == full.trim_end_matches('.')
    };
    same(question_gist(agent, question), without_name(question, agent)) && same(answer_gist(answer), without_name(answer, "main"))
}

/// Whether the line opens: a why, or words the gists leave out.
pub(crate) fn answered_opens(agent: &str, question: &str, answer: &str, why: &str) -> bool {
    !why.trim().is_empty() || !gists_say_all(agent, question, answer)
}

/// Main answered an agent for you (level 2): `:* docs asked: v1 or v2?
/// · i answered: v2 ▸` on one row, or the question's gist on a row and
/// the answer's on the next. Open, when the gists cut the words: one
/// row `:* i answered docs ▾` and, under the rail, the whole question,
/// answer and why; when they do not, the line `▾ why` and the why.
pub(crate) fn answered_lines(agent: &str, question: &str, answer: &str, why: &str, open: bool, width: usize) -> Vec<Line<'static>> {
    let text_st = Style::default().fg(text());
    let dim_st = Style::default().fg(dim());
    let all = gists_say_all(agent, question, answer);
    let has_why = !why.trim().is_empty();
    let opens = has_why || !all;
    use unicode_width::UnicodeWidthStr;
    let asked = format!("{} asked: {}", agent, question_gist(agent, question));
    let answered = format!("i answered: {}", answer_gist(answer));
    // only the why behind the fold: it says so
    let mark = match (opens, open, all) {
        (false, ..) => String::new(),
        (true, o, true) => format!(" {} why", glyph(if o { G_OPEN } else { G_CLOSED })),
        (true, o, false) => format!(" {}", glyph(if o { G_OPEN } else { G_CLOSED })),
    };
    let main = Span::styled(format!(" {} ", G_MAIN), Style::default().fg(accent()).add_modifier(Modifier::BOLD));
    // the answer's row starts under the agent's name
    let indent = Span::raw(" ".repeat(main.content.width()));
    let inner = width.saturating_sub(main.content.width()).max(1);
    let one = format!("{} · {}", asked, answered);
    let mut ls = if open && !all {
        // open on the whole words: the gists would repeat them, the head
        // says who alone
        let head = fit_row(&format!("i answered {}", agent), inner, &mark, text_st, dim_st);
        let mut row = vec![main];
        row.extend(head.spans);
        vec![Line::from(row)]
    } else if one.width() + mark.width() <= inner {
        vec![Line::from(vec![main, Span::styled(one, text_st), Span::styled(mark, dim_st)])]
    } else {
        // the question on the first row, the answer on the second (ROWS),
        // each cut at a word to its row; the row break separates them
        let asked = fit_row(&asked, inner, "", text_st, text_st);
        let answered = fit_row(&answered, inner, &mark, text_st, dim_st);
        let mut first = vec![main];
        first.extend(asked.spans);
        let mut second = vec![indent];
        second.extend(answered.spans);
        vec![Line::from(first), Line::from(second)]
    };
    debug_assert!(ls.len() <= ROWS);
    if !open || !opens {
        return ls;
    }
    let room = width.saturating_sub(RAIL.len());
    let mut body: Vec<Line<'static>> = Vec::new();
    let part = |label: &str, words: &str, body: &mut Vec<Line<'static>>| {
        // the label goes in the words before the wrap (after it, the
        // first row ran over and left a word alone), then turns dim
        let src = if label.is_empty() { code_style(words.trim()) } else { format!("{} {}", label, code_style(words.trim())) };
        let mut rows = md_lines(&src, room, room);
        if rows.is_empty() {
            rows.push(Line::from(""));
        }
        dim_head(&mut rows[0], label.chars().count(), dim_st);
        if !body.is_empty() {
            body.push(Line::from(""));
        }
        body.extend(rows);
    };
    if !all {
        part("asked:", without_name(question, agent), &mut body);
        part("i answered:", without_name(answer, "main"), &mut body);
    }
    if has_why {
        part(if all { "" } else { "why:" }, why, &mut body);
    }
    let bar = Span::styled(RAIL, Style::default().fg(rule()));
    ls.extend(hung_rows(&bar, &bar, body, width));
    ls
}

/// The first `n` chars of `row` in `st` (a label's), its spans split
/// where they end.
fn dim_head(row: &mut Line<'static>, n: usize, st: Style) {
    let mut left = n;
    let mut out: Vec<Span<'static>> = Vec::new();
    for sp in row.spans.drain(..) {
        let len = sp.content.chars().count();
        if left == 0 {
            out.push(sp);
        } else if len <= left {
            left -= len;
            out.push(Span::styled(sp.content, st));
        } else {
            let head: String = sp.content.chars().take(left).collect();
            let tail: String = sp.content.chars().skip(left).collect();
            left = 0;
            out.push(Span::styled(head, st));
            out.push(Span::styled(tail, sp.style));
        }
    }
    row.spans = out;
}

/// `s` then `tail` in `room` columns: `s` cut at the end of a word with
/// `…` when they do not fit (never `……`).
fn fit_row(s: &str, room: usize, tail: &str, st: Style, tail_st: Style) -> Line<'static> {
    use unicode_width::UnicodeWidthStr;
    let mut row = if s.width() + tail.width() <= room {
        Line::from(Span::styled(s.to_string(), st))
    } else {
        let bare = s.strip_suffix(ellipsis()).unwrap_or(s);
        cut_row(Line::from(Span::styled(bare.to_string(), st)), room.saturating_sub(tail.width()), ellipsis())
    };
    row.spans.push(Span::styled(tail.to_string(), tail_st));
    row
}

// ---- commands and paths in code style ----

/// A word is code: a path, a flag, a variable, an assignment, a dotted
/// name (`gate.sh`, `bise.sockpath.gate`), shell punctuation.
fn is_code(word: &str) -> bool {
    let core = word.trim_start_matches('(').trim_end_matches(['.', ',', ':', ';', ')', '!', '?']);
    if core.is_empty() || core.contains("://") || core.contains("](") {
        return false;
    }
    let slashes = core.matches('/').count();
    let dotted = core.len() > 4
        && !core.chars().all(|c| c.is_ascii_digit() || c == '.')
        && core.char_indices().any(|(i, c)| {
            c == '.' && i > 0 && core[..i].ends_with(|p: char| p.is_alphanumeric()) && core[i + 1..].starts_with(|n: char| n.is_alphanumeric())
        });
    (core.starts_with('-') && core.len() > 1 && !core.chars().all(|c| c == '-' || c == '—') || core == "--")
        || core.starts_with(['~', '/', '$'])
        || core.starts_with("./")
        || slashes >= 2
        || (slashes == 1 && core.contains('.'))
        || core.contains(['$', '=', '\\', '|', '&', '<', '>', '{', '}', '[', ']', '_'])
        || dotted
}

/// A plain word a command may start with (`launchctl submit -l …`).
fn command_word(word: &str) -> bool {
    const PROSE: [&str; 24] = [
        "use", "run", "pass", "with", "add", "the", "a", "an", "and", "or", "to", "try", "then", "via", "by", "set", "call",
        "please", "it", "its", "is", "of", "in", "on",
    ];
    let w = word.trim_start_matches('(');
    !w.is_empty()
        && w.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '.' | '_'))
        && !PROSE.contains(&w)
}

/// A word that ends a sentence or a clause: no code run goes past it.
fn ends_clause(word: &str) -> bool {
    word.ends_with(['.', ',', ':', '?', '!'])
}

/// `text` with its commands and paths in backticks, for [`md_lines`];
/// a text with backticks of its own is markdown already, kept as is.
pub(crate) fn code_style(text: &str) -> String {
    if text.contains('`') {
        return text.to_string();
    }
    text.lines().map(code_style_line).collect::<Vec<_>>().join("\n")
}

fn code_style_line(line: &str) -> String {
    let indent = &line[..line.len() - line.trim_start().len()];
    let words: Vec<&str> = line.split_whitespace().collect();
    // a word is code on its own, or inside a "…" (a shell argument)
    let mut code: Vec<bool> = Vec::with_capacity(words.len());
    // a word ends its clause (`ends_clause`), unless a "…" is still open
    // after it (`echo \$? > $D/rc` inside `-c "…"`)
    let mut stop: Vec<bool> = Vec::with_capacity(words.len());
    let mut quoted = false;
    for w in &words {
        let toggles = w.matches('"').count() % 2 == 1;
        code.push(quoted || toggles || is_code(w));
        if toggles {
            quoted = !quoted;
        }
        stop.push(ends_clause(w) && !quoted);
    }
    // a gap of 1 or 2 plain words between two code words, in one clause,
    // is the same command (`$D; mkdir -p`, `-e $D/err -- /bin/zsh`)
    let mut i = 0;
    while i < words.len() {
        if code[i] && !stop[i] {
            if let Some(j) = (i + 1..words.len().min(i + 4)).find(|&j| code[j]) {
                if j > i + 1 && (i + 1..j).all(|k| command_word(words[k]) && !stop[k]) {
                    code[i + 1..j].iter_mut().for_each(|c| *c = true);
                }
            }
        }
        i += 1;
    }
    // a run that starts with a flag takes its command, 2 words at most
    // (`(ulimit -f`, `cargo test --release`)
    for i in 0..words.len() {
        if code[i] && (i == 0 || !code[i - 1]) && words[i].starts_with('-') {
            let mut k = i;
            while k > 0 && i - k < 2 && !code[k - 1] && command_word(words[k - 1]) && !stop[k - 1] {
                k -= 1;
                if words[k].starts_with('(') {
                    break;
                }
            }
            code[k..i].iter_mut().for_each(|c| *c = true);
        }
    }
    let mut out = String::from(indent);
    let mut i = 0;
    while i < words.len() {
        if !out.trim().is_empty() {
            out.push(' ');
        }
        if !code[i] {
            out.push_str(words[i]);
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < words.len() && code[j + 1] && !stop[j] {
            j += 1;
        }
        out.push_str(&ticked(&words[i..=j].join(" ")));
        i = j + 1;
    }
    out
}

/// A run of code words in backticks: an opening `(` and the closing
/// punctuation stay outside.
fn ticked(run: &str) -> String {
    let lead = if run.starts_with('(') && !run.contains(')') { "(" } else { "" };
    let body = &run[lead.len()..];
    let mut end = body.len();
    for (i, c) in body.char_indices().rev() {
        let unmatched = c == ')' && body[..i].matches('(').count() <= body[..i].matches(')').count();
        if matches!(c, '.' | ',' | ':' | ';' | '!' | '?') || unmatched {
            end = i;
        } else {
            break;
        }
    }
    if end == 0 {
        return run.to_string();
    }
    format!("{}`{}`{}", lead, &body[..end], &body[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows_text(rows: &[Line<'static>]) -> Vec<String> {
        rows.iter().map(|r| r.spans.iter().map(|s| s.content.clone()).collect::<String>()).collect()
    }

    const Q: &str = "sock-path: gate quick fails here because my bash tool caps written files at 50 MB \
        (ulimit -f 102400, can't raise it). rustc gets SIGXFSZ (signal 25) linking bend-tui's lib test binary. \
        my options: run it in tmux, or split the test binary. which do you want?";
    const A: &str = "run it through launchd, it doesn't inherit your ulimit: D=$TMPDIR/gate-sock; mkdir -p $D; \
        launchctl submit -l bise.sockpath.gate -o $D/out -e $D/err -- /bin/zsh -c \"[ -f $D/rc ] && exec sleep 86400; \
        cd ~/.bise/worktrees/h/sock-path/harness && CARGO_INCREMENTAL=0 nice -n 10 tests/gate.sh quick; echo \\$? > $D/rc; \
        exec sleep 86400\". check $D/rc with an sb every one-shot timer (no polling), then launchctl remove bise.sockpath.gate. \
        you keep the slot until you land.";
    const WHY: &str = "the brief puts heavy jobs one at a time; launchd is how the others ran.";

    fn rows(open: bool, width: usize) -> Vec<String> {
        rows_text(&answered_lines("sock-path", Q, A, WHY, open, width)).into_iter().map(|r| r.trim_end().to_string()).collect()
    }

    #[test]
    fn the_gists_drop_the_name_the_asides_and_the_rest() {
        assert_eq!(without_name(Q, "sock-path").split_whitespace().next(), Some("gate"));
        assert_eq!(without_name("Docs:v2", "docs"), "v2");
        assert_eq!(without_name("docsite is up", "docs"), "docsite is up");
        assert_eq!(question_gist("sock-path", Q), "gate quick fails here because my bash tool caps written files…");
        // `which do you want?` is too short to be the gist
        assert!(!question_gist("sock-path", Q).contains("which"));
        assert_eq!(answer_gist(A), "run it through launchd, it doesn't…");
        // a real question wins over the context before it
        assert_eq!(question_gist("docs", "i read the brief. should the examples use v1 or v2?"), "should the examples use v1 or v2?");
        assert_eq!(answer_gist("v2: the brief says so."), "v2");
        assert!(gists_say_all("docs", "v1 or v2?", "v2"));
        assert!(!gists_say_all("docs", "v1 or v2? the brief is old.", "v2"));
    }

    #[test]
    fn folded_it_is_two_rows_at_most_cut_at_a_word() {
        for width in [91, 77, 40] {
            let r = rows(false, width);
            assert!(r.len() <= ROWS, "{width}: {r:#?}");
            assert!(r[0].starts_with(" :* sock-path asked: gate quick fails"), "{r:#?}");
            assert!(r.last().unwrap().ends_with('▸'), "{r:#?}");
            assert!(!r.iter().any(|l| l.contains("sock-path: ")), "the name once: {r:#?}");
            assert!(r.iter().all(|l| unicode_width::UnicodeWidthStr::width(l.as_str()) <= width), "{r:#?}");
        }
        assert_eq!(
            rows(false, 91),
            [
                " :* sock-path asked: gate quick fails here because my bash tool caps written files…",
                "    i answered: run it through launchd, it doesn't… ▸",
            ]
        );
        // narrow: the second row is cut at a word, `… ▸`
        let r = rows(false, 40);
        assert!(r[1].ends_with("… ▸") && !r[1].ends_with(" … ▸"), "{r:#?}");
    }

    #[test]
    fn open_it_shows_everything_with_its_commands_in_code() {
        let lines = answered_lines("sock-path", Q, A, WHY, true, 91);
        let r: Vec<String> = rows_text(&lines).into_iter().map(|r| r.trim_end().to_string()).collect();
        // one head row: the gists would repeat the words under it
        assert_eq!(r[0], " :* i answered sock-path ▾", "{r:#?}");
        let body = r[1..].join("\n");
        // the words read on as one text across the rail's rows
        let flat = r[1..].iter().map(|l| l.trim_start_matches(" │").trim()).collect::<Vec<_>>().join(" ");
        assert!(flat.contains("(ulimit -f 102400, can't raise it)"), "{flat}");
        for want in [" │ asked: gate quick fails", " │ i answered: run it through launchd", " │ why: the brief"] {
            assert!(body.contains(want), "{want:?} missing in:\n{body}");
        }
        assert!(!body.contains('`'), "the backticks are styles: {body}");
        // the command is one code span, the prose around it is not
        let code: Vec<String> = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|s| s.style.fg == Some(ok()))
            .map(|s| s.content.to_string())
            .collect();
        let code = code.join("|");
        assert!(code.contains("launchctl submit -l bise.sockpath.gate"), "{code}");
        assert!(code.contains("$D/rc"), "{code}");
        assert!(!code.contains("check") && !code.contains("launchd,"), "{code}");
    }

    #[test]
    fn commands_and_paths_get_backticks() {
        assert_eq!(code_style("edit web/src/auth.ts now."), "edit `web/src/auth.ts` now.");
        assert_eq!(code_style("caps at 50 MB (ulimit -f 102400, no)"), "caps at 50 MB (`ulimit -f` 102400, no)");
        assert_eq!(code_style("then launchctl remove bise.sockpath.gate."), "then launchctl remove `bise.sockpath.gate`.");
        assert_eq!(code_style("run cargo test --release, then land."), "run `cargo test --release`, then land.");
        assert_eq!(code_style("v1 or v2, e.g. the new one"), "v1 or v2, e.g. the new one");
        assert_eq!(code_style("already `marked` here"), "already `marked` here");
        assert_eq!(code_style("see https://bise.dev/x for it"), "see https://bise.dev/x for it");
    }

    #[test]
    fn a_short_answer_keeps_its_why_fold() {
        let r = rows_text(&answered_lines("docs", "docs: v1 or v2?", "v2", "the brief says v2.", false, 100));
        assert_eq!(r[0].trim_end(), " :* docs asked: v1 or v2? · i answered: v2 ▸ why");
        let r = rows_text(&answered_lines("docs", "v1 or v2?", "v2", "", false, 100));
        assert_eq!(r[0].trim_end(), " :* docs asked: v1 or v2? · i answered: v2");
    }
}
