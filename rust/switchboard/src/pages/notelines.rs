//! A note on the lines of a ui block (page-notes; docs/ambient-pages.md
//! §2.6, §2.7): the note layer (`kit/notelines.js`) puts on
//! a note the rows it covers in a k-diff or a k-term, `lines: [{old?,
//! new?, n?, text}]` (a diff's old and new numbers, or a terminal's line
//! number), `more` (rows past the 40 it keeps) and `part` (the tab or
//! the figure they sit in). This module reads them off the note's extra
//! fields and writes them in the notes message: the place (`diff main,
//! old 34–36 → new 34`, the side list's words) and the lines, numbers
//! and text with their −/+ mark, under the note.
//! Pure: no file, no hub.

use super::store::Note;
use serde_json::Value;

/// One line of a note: a diff's row (old and/or new number) or a
/// terminal's line (`n`).
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub old: Option<u64>,
    pub new: Option<u64>,
    pub n: Option<u64>,
    pub text: String,
}

/// The lines a note is about, and where in the block.
#[derive(Debug, Clone, PartialEq)]
pub struct Lines {
    pub lines: Vec<Line>,
    pub more: u64,
    pub part: Option<String>,
}

/// The lines shown in the message under a note (the JSON block below it
/// carries every line the layer kept).
const SHOWN: usize = 20;
/// The chars shown of one line.
const CUT: usize = 160;

/// A note's lines, None when it has none (a text note).
pub fn of(n: &Note) -> Option<Lines> {
    let list = n.extra.get("lines")?.as_array()?;
    let num = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64);
    let lines: Vec<Line> = list
        .iter()
        .map(|v| Line { old: num(v, "old"), new: num(v, "new"), n: num(v, "n"), text: v.get("text").and_then(Value::as_str).unwrap_or("").to_string() })
        .collect();
    if lines.is_empty() {
        return None;
    }
    let more = n.extra.get("more").and_then(Value::as_u64).unwrap_or(0);
    let part = n.extra.get("part").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    Some(Lines { lines, more, part })
}

fn span(ns: &[u64]) -> String {
    match (ns.iter().min(), ns.iter().max()) {
        (Some(a), Some(b)) if a == b => a.to_string(),
        (Some(a), Some(b)) => format!("{a}–{b}"),
        _ => String::new(),
    }
}

/// In words (ambient m_7871): a diff's `old 34–38 → new 34–35` (only
/// old: `old 5–7`; an unchanged row counts on both sides), a terminal's
/// `line 3`, `lines 3–5`.
pub fn label(l: &Lines) -> String {
    let pick = |f: fn(&Line) -> Option<u64>| l.lines.iter().filter_map(f).collect::<Vec<u64>>();
    let (old, new, ns) = (pick(|x| x.old), pick(|x| x.new), pick(|x| x.n));
    let mut parts = Vec::new();
    if !old.is_empty() {
        parts.push(format!("old {}", span(&old)));
    }
    if !new.is_empty() {
        parts.push(format!("new {}", span(&new)));
    }
    if !ns.is_empty() {
        let count = l.lines.len() as u64 + l.more;
        parts.push(format!("{} {}", if count == 1 { "line" } else { "lines" }, span(&ns)));
    }
    parts.join(" → ")
}

/// The note's place in the message, the side list's words (ambient
/// m_7871): `diff main, old 5–7 → new 5`; a long part (a figure's
/// caption) goes in parentheses: `screen (the task's row), lines 2–3`.
pub fn place(block: &str, l: &Lines) -> String {
    let part = match l.part.as_deref() {
        Some(p) if p.chars().count() <= 24 && !p.contains(',') => format!(" {p}"),
        Some(p) => format!(" ({p})"),
        None => String::new(),
    };
    format!("{block}{part}, {}", label(l))
}

fn cut(s: &str) -> String {
    let s = s.trim_end();
    if s.chars().count() <= CUT {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(CUT - 1).collect::<String>())
    }
}

/// The lines under the note, one per row, indented under its number:
/// a diff's `  −34       │ text`, `       +34 │ text`, `   39   35 │ text`
/// (old, new), a terminal's `    3 │ text`; past [`SHOWN`], how many more.
pub fn listing(l: &Lines) -> String {
    let diff = l.lines.iter().any(|x| x.old.is_some() || x.new.is_some());
    let col = |v: Option<u64>, sign: &str| v.map(|n| format!("{sign}{n}")).unwrap_or_default();
    let mut out = String::new();
    for x in l.lines.iter().take(SHOWN) {
        let head = if diff {
            let (o, n) = match (x.old, x.new) {
                (Some(_), None) => (col(x.old, "−"), String::new()),
                (None, Some(_)) => (String::new(), col(x.new, "+")),
                _ => (col(x.old, ""), col(x.new, "")),
            };
            format!("{o:>5} {n:>5}")
        } else {
            format!("{:>5}", x.n.map(|n| n.to_string()).unwrap_or_default())
        };
        out.push_str(&format!("   {head} │ {}\n", cut(&x.text)));
    }
    let left = (l.lines.len().saturating_sub(SHOWN)) as u64 + l.more;
    if left > 0 {
        out.push_str(&format!("   … {left} more {}\n", if left == 1 { "line" } else { "lines" }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn note(v: Value) -> Note {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn a_diff_note_reads_its_rows_numbers_and_text() {
        let n = note(json!({"block": "diff", "kind": "note", "text": "keep 36", "part": "main",
            "lines": [{"old": 34, "text": "Skills contain"}, {"old": 35, "text": ""}, {"new": 34, "text": "Skills below"}, {"old": 39, "new": 35, "text": "<skills>"}]}));
        let l = of(&n).unwrap();
        assert_eq!(l.lines.len(), 4);
        assert_eq!(label(&l), "old 34–39 → new 34–35");
        assert_eq!(place("diff", &l), "diff main, old 34–39 → new 34–35");
        assert_eq!(
            listing(&l),
            "     −34       │ Skills contain\n     −35       │ \n           +34 │ Skills below\n      39    35 │ <skills>\n"
        );
        // only old lines; a long part (a figure's caption) in parentheses
        let only = Lines { lines: l.lines[..2].to_vec(), more: 0, part: Some("the row after the cut, 80 columns".into()) };
        assert_eq!(place("screen", &only), "screen (the row after the cut, 80 columns), old 34–35");
    }

    #[test]
    fn a_terminal_note_has_line_numbers_one_line_is_singular_and_a_text_note_has_none() {
        let l = of(&note(json!({"block": "screen", "kind": "note", "lines": [{"n": 3, "text": "/scheduled  3 tasks"}]}))).unwrap();
        assert_eq!(label(&l), "line 3");
        assert_eq!(listing(&l), "       3 │ /scheduled  3 tasks\n");
        assert_eq!(of(&note(json!({"block": "p1", "kind": "note", "quote": "x"}))), None);
        assert_eq!(of(&note(json!({"block": "p1", "kind": "note", "lines": []}))), None);
    }

    #[test]
    fn long_runs_and_long_lines_are_cut_with_a_count() {
        let rows: Vec<Value> = (1..=25).map(|i| json!({"n": i, "text": "x".repeat(200)})).collect();
        let l = of(&note(json!({"block": "t", "kind": "note", "lines": rows, "more": 3}))).unwrap();
        assert_eq!(label(&l), "lines 1–25");
        let out = listing(&l);
        assert_eq!(out.lines().count(), SHOWN + 1);
        assert!(out.ends_with("   … 8 more lines\n"), "{out}");
        assert!(out.lines().next().unwrap().ends_with('…'));
    }
}
