//! `/approvals` (approvals-design.md §8, designer's mock A, 17): one
//! screen for the mode, the checker and the saved rules of this repo.
//! ↑↓ choose a rule, backspace asks once, inline ("remove cargo test *?
//! enter yes · esc no"), enter removes it (the hub's `remove_rule`),
//! shift+tab switches the mode, esc closes. The rules come live from the
//! hub's `approvals` event (`app.sb.approvals`).

use crate::sb::{Approvals, Rule};
use bise_proto::approvals;
use crate::{theme, App};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

/// The screen's state while open.
#[derive(Debug, Default)]
pub(crate) struct Screen {
    /// The rule under the cursor.
    pub(crate) sel: usize,
    /// backspace was pressed: the selected rule's row asks.
    pub(crate) confirm: bool,
    /// What the hub said went wrong (error color, under the rules).
    pub(crate) said: Option<String>,
}

/// Keys while the screen is open: it takes them all.
pub(crate) fn on_key(app: &mut App, k: &KeyEvent) -> bool {
    let Some(s) = app.approvals.as_mut() else { return false };
    if k.kind != KeyEventKind::Press {
        return true;
    }
    let n = app.sb.approvals.rules.len();
    s.sel = s.sel.min(n.saturating_sub(1));
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    match k.code {
        KeyCode::Esc if s.confirm => s.confirm = false,
        KeyCode::Esc => app.approvals = None,
        KeyCode::Char('c') | KeyCode::Char('g') if ctrl => app.approvals = None,
        KeyCode::Enter if s.confirm => {
            s.confirm = false;
            s.said = None;
            if let Some(r) = app.sb.approvals.rules.get(s.sel) {
                let rule = r.raw.clone();
                app.sb.send(serde_json::json!({"op": "remove_rule", "rule": rule}));
            }
        }
        KeyCode::Backspace | KeyCode::Delete if n > 0 => {
            s.confirm = true;
            s.said = None;
        }
        KeyCode::BackTab => crate::sb::toggle_approvals(app),
        // a move cancels the question
        KeyCode::Up | KeyCode::Char('k') => {
            s.confirm = false;
            s.sel = s.sel.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            s.confirm = false;
            s.sel = (s.sel + 1).min(n.saturating_sub(1));
        }
        KeyCode::Home => (s.confirm, s.sel) = (false, 0),
        KeyCode::End => (s.confirm, s.sel) = (false, n.saturating_sub(1)),
        _ => {}
    }
    true
}

/// The mouse while open: the wheel moves the cursor, the rest is
/// swallowed.
pub(crate) fn mouse(app: &mut App, m: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::MouseEventKind;
    let n = app.sb.approvals.rules.len();
    let Some(s) = app.approvals.as_mut() else { return false };
    match m.kind {
        MouseEventKind::ScrollUp => (s.confirm, s.sel) = (false, s.sel.saturating_sub(1)),
        MouseEventKind::ScrollDown => (s.confirm, s.sel) = (false, (s.sel + 1).min(n.saturating_sub(1))),
        _ => {}
    }
    true
}

// ---- the words (pure) ----

fn dot() -> &'static str {
    if theme::ascii_mode() {
        "-"
    } else {
        "·"
    }
}

/// What a rule allows, as the list shows it: `cargo test *`, `edits to
/// ~/notes`, `gmail.send_email` (bise_proto::approvals, the window's
/// words too).
pub(crate) fn rule_what(r: &Rule) -> String {
    approvals::what(&r.facts(), std::env::var("HOME").ok().as_deref())
}

/// The right column of a rule: its age, its source, where it applies.
pub(crate) fn rule_note(r: &Rule, days: Option<i64>) -> String {
    approvals::note(&r.facts(), days, dot())
}

fn tilde(p: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && p.starts_with(&h) => format!("~{}", &p[h.len()..]),
        _ => p.to_string(),
    }
}

/// A path in `w` columns, cut in the middle so its own name stays:
/// `~/lab/…/acme`, `/private/…/ws`, then `…/acme`.
pub(crate) fn short_path(p: &str, w: usize) -> String {
    if p.width() <= w {
        return p.to_string();
    }
    let (root, rest) = if let Some(r) = p.strip_prefix("~/") {
        ("~/", r)
    } else if let Some(r) = p.strip_prefix('/') {
        ("/", r)
    } else {
        ("", p)
    };
    let segs: Vec<&str> = rest.split('/').filter(|x| !x.is_empty()).collect();
    let last = segs.last().copied().unwrap_or(rest);
    if segs.len() > 2 {
        let two = format!("{root}{}/…/{last}", segs[0]);
        if two.width() <= w {
            return two;
        }
    }
    format!("…/{last}")
}

fn s(t: impl Into<String>, c: ratatui::style::Color) -> Span<'static> {
    Span::styled(t.into(), Style::default().fg(c))
}

fn pad(t: &str, w: usize) -> String {
    format!("{t}{}", " ".repeat(w.saturating_sub(t.width())))
}

/// `t` cut to `w` columns, with `…`.
fn cut(t: &str, w: usize) -> String {
    if t.width() <= w {
        return t.to_string();
    }
    let mut out = String::new();
    for c in t.chars() {
        if out.width() + unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) + 1 > w {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

/// The key line: dim, the keys in text color.
fn keyline(parts: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut v = Vec::new();
    for (i, (k, what)) in parts.iter().enumerate() {
        if i > 0 {
            v.push(s(format!(" {} ", dot()), theme::dim()));
        }
        v.push(s(k.to_string(), theme::text()));
        v.push(s(format!(" {what}"), theme::dim()));
    }
    v
}

/// The screen's lines for a column `w` wide with `rows` rows. `days`:
/// each rule's age in days (None: unknown).
pub(crate) fn lines(a: &Approvals, sc: &Screen, days: &[Option<i64>], w: usize, rows: usize) -> Vec<Line<'static>> {
    let d = dot();
    let ascii = theme::ascii_mode();
    let bold = |t: String| Span::styled(t, Style::default().fg(theme::text()).add_modifier(Modifier::BOLD));
    const SAY: &str = "what runs without asking you in ";
    let repo = if a.repo.is_empty() { "this repo".to_string() } else { short_path(&tilde(&a.repo), w.saturating_sub(SAY.width() + 1)) };
    let mut v = vec![
        Line::from(bold("approvals".into())),
        Line::from(s(format!("{SAY}{repo}."), theme::dim())),
        Line::raw(""),
    ];
    // the mode and the checker: label, value, what changes it
    let mode = match a.word() {
        "auto" if a.env => format!("auto {d} this session (BISE_APPROVALS)"),
        "yolo" if a.env => format!("yolo {d} this session (BISE_APPROVALS)"),
        m => m.to_string(),
    };
    let checker = match a.checker.as_str() {
        "jev" if !a.checker_model.is_empty() => format!("{} {d} {}", a.checker_who, a.checker_model),
        "jev" => format!("Jev {d} {}", a.checker_who),
        "model" => a.checker_who.clone(),
        "off" => format!("off {d} every command asks you"),
        _ => "none yet".into(),
    };
    let whats: Vec<String> = a.rules.iter().map(rule_what).collect();
    // one column for the values' hints and the rules' notes (the mock)
    let col = whats
        .iter()
        .map(|t| t.width())
        .chain([11 + mode.width(), 11 + checker.width()])
        .max()
        .unwrap_or(0)
        .saturating_add(3)
        .max(35);
    let col = col.min(w.saturating_sub(18).max(12));
    let row = |label: &str, value: &str, hint: &str| {
        Line::from(vec![
            s(format!("  {}", pad(label, 11)), theme::dim()),
            s(pad(&cut(value, col.saturating_sub(11 + 1)), col - 11), theme::text()),
            s(hint.to_string(), theme::dim()),
        ])
    };
    let st = if ascii { "shift+tab" } else { "⇧⇥" };
    v.push(row("mode", &mode, &format!("{st} switches")));
    v.push(row("checker", &checker, "/models changes it"));
    v.push(Line::raw(""));
    v.push(Line::from(s("  always allowed here", theme::dim())));
    let n = a.rules.len();
    if n == 0 {
        v.push(Line::from(s(
            cut("  nothing yet: a card's \"always allow\" adds a rule here.", w),
            theme::dim(),
        )));
    }
    // the rules that fit, the cursor's in view
    let room = rows.saturating_sub(v.len() + 4).max(1);
    let sel = sc.sel.min(n.saturating_sub(1));
    let top = if n <= room { 0 } else { sel.saturating_sub(room - 1).min(n - room) };
    let shown = room.min(n);
    if top > 0 {
        v.push(Line::from(s(format!("  ↑ {top} more"), theme::dim())));
    }
    for (i, (r, what)) in a.rules.iter().zip(&whats).enumerate().skip(top).take(shown) {
        let on = i == sel;
        let mark = if on { s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent()) } else { Span::raw("  ") };
        let text = if on { bold(pad(&cut(what, col - 1), col)) } else { s(pad(&cut(what, col - 1), col), theme::text()) };
        // backspace: the right column asks (designer: the rule's text stays)
        if on && sc.confirm {
            v.push(Line::from(vec![
                mark,
                text,
                s("remove it?", theme::accent()),
                s(format!(" enter yes {d} esc no"), theme::dim()),
            ]));
            continue;
        }
        let note = cut(&rule_note(r, days.get(i).copied().flatten()), w.saturating_sub(col + 2));
        v.push(Line::from(vec![mark, text, s(note, theme::dim())]));
    }
    if top + shown < n {
        v.push(Line::from(s(format!("  ↓ {} more", n - top - shown), theme::dim())));
    }
    if let Some(e) = &sc.said {
        v.push(Line::raw(""));
        v.push(Line::from(s(cut(e, w), theme::error())));
    }
    v.push(Line::raw(""));
    let keys: Vec<(&str, &str)> = if n == 0 {
        vec![(st, "switches"), ("esc", "back")]
    } else if sc.confirm {
        vec![("enter", "remove"), ("esc", "keep it")]
    } else {
        vec![("↑↓", "choose"), ("backspace", "remove"), ("esc", "back")]
    };
    v.push(Line::from(keyline(&keys)));
    v
}

/// The screen, over the whole frame, when open: one column (80 at most),
/// its block at 2/5 of the free rows, like `/models`.
pub(crate) fn draw(app: &mut App, frame: &mut Frame) {
    let Some(sc) = app.approvals.as_mut() else { return };
    let a = &app.sb.approvals;
    sc.sel = sc.sel.min(a.rules.len().saturating_sub(1));
    let full = frame.area();
    crate::pointer::region(full, crate::pointer::Shape::Default);
    frame.render_widget(Clear, full);
    if full.width < 24 || full.height < 6 {
        return;
    }
    let w = full.width.saturating_sub(4).min(80);
    let now = crate::when::now_ms();
    let now_off = crate::when::offset_at(now);
    let days: Vec<Option<i64>> = a
        .rules
        .iter()
        .map(|r| r.added.map(|ms| crate::when::days_ago(ms, crate::when::offset_at(ms), now, now_off)))
        .collect();
    let lines = lines(a, sc, &days, w as usize, full.height as usize);
    let h = (lines.len() as u16).min(full.height);
    let y = full.y + (full.height - h) * 2 / 5;
    let area = Rect { x: full.x + (full.width - w) / 2, y, width: w, height: h };
    frame.render_widget(Paragraph::new(lines), area);
    crate::textlayer::text(area);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn rule(tool: &str, pattern: &str, from: &str) -> Rule {
        Rule { tool: tool.into(), pattern: pattern.into(), from: from.into(), ..Default::default() }
    }

    fn state() -> Approvals {
        Approvals {
            mode: "auto".into(),
            checker: "jev".into(),
            checker_who: "TypeSafe".into(),
            checker_model: "jev-1.13".into(),
            repo: "/w/acme".into(),
            rules: vec![
                rule("bash", "cargo test *", "card #12, api-v2"),
                rule("bash", "npm install *", "card #14, api-v2, dark-mode, i18n"),
                rule("bash", "npm run build *", ""),
                rule("gmail.send_email", "", ""),
            ],
            ..Default::default()
        }
    }

    /// designer's mock A, 17
    #[test]
    fn the_screen_says_mode_checker_and_rules() {
        let a = state();
        let days = [Some(0), Some(0), Some(2), None];
        let t = text(&lines(&a, &Screen::default(), &days, 80, 40));
        assert_eq!(
            t,
            "approvals
what runs without asking you in /w/acme.

  mode       auto                    ⇧⇥ switches
  checker    TypeSafe · jev-1.13     /models changes it

  always allowed here
› cargo test *                       today · from api-v2
  npm install *                      today · 3 agents
  npm run build *                    2 days ago
  gmail.send_email                   a connector

↑↓ choose · backspace remove · esc back"
        );
    }

    #[test]
    fn backspace_asks_inline() {
        let a = state();
        let sc = Screen { sel: 1, confirm: true, said: None };
        let t = text(&lines(&a, &sc, &[], 80, 40));
        assert!(t.contains(&format!("\n› {}remove it? enter yes · esc no\n", pad("npm install *", 35))), "{t}");
        assert!(t.ends_with("\nenter remove · esc keep it"), "{t}");
        let sc = Screen { sel: 1, confirm: false, said: Some("this rule is no longer in ~/.bise/approvals.toml.".into()) };
        assert!(text(&lines(&a, &sc, &[], 80, 40)).contains("\nthis rule is no longer in"));
    }

    #[test]
    fn no_rule_yet_and_a_short_screen() {
        let mut a = state();
        a.rules.clear();
        let t = text(&lines(&a, &Screen::default(), &[], 80, 40));
        assert!(t.contains("nothing yet: a card's \"always allow\" adds a rule here."), "{t}");
        assert!(t.ends_with("⇧⇥ switches · esc back"), "{t}");
        // many rules on few rows: the cursor's row shows, the rest is counted
        let mut a = state();
        a.rules = (0..30).map(|i| rule("bash", &format!("tool{i} *"), "")).collect();
        let sc = Screen { sel: 20, ..Default::default() };
        let l = lines(&a, &sc, &[], 80, 16);
        let t = text(&l);
        assert!(l.len() <= 16, "{t}");
        assert!(t.contains("› tool20 *") && t.contains("more"), "{t}");
    }

    #[test]
    fn a_long_repo_keeps_its_name() {
        assert_eq!(short_path("~/lab/acme", 40), "~/lab/acme");
        assert_eq!(short_path("~/lab/clients/2026/acme", 16), "~/lab/…/acme");
        assert_eq!(short_path("/private/var/folders/c5/T/ws", 20), "/private/…/ws");
        assert_eq!(short_path("/private/var/folders/c5/T/a-long-name", 14), "…/a-long-name");
    }

    #[test]
    fn the_words_of_a_rule() {
        // age and source: bise_proto::approvals' own tests
        let mut r = Rule { tool: "write_file".into(), path: "/n/notes/".into(), every: true, ..Default::default() };
        assert_eq!(rule_what(&r), "edits to /n/notes/");
        assert_eq!(rule_note(&r, None), "every project");
        r = Rule { tool: "bash".into(), pattern: "cp *".into(), outside: true, ..Default::default() };
        assert_eq!(rule_note(&r, Some(1)), "yesterday · outside the sandbox");
    }
}
