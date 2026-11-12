//! His last messages in a thread (`thread/marks`, msg-dots): the desktop's
//! strip of ticks beside a thread, one per message of his, needs where his
//! last ones are without holding the thread. Which entry is his and the words
//! a tick shows; the hub's transcript walker (the one ⌘F's `thread/find`
//! runs, backward here) applies it from the transcript's end. Pure.
//!
//! His message is an [`EntryKind::You`] entry, by its line's type: a task's
//! brief comes as a message from main (`sb msg-in`, a `from_agent` entry),
//! never his.

use super::words::page_note_words;
use super::{Entry, EntryKind, Pos};
use crate::parts::{file_name, parse, Part};

/// Marks one `thread_marks` carries at most.
pub const MAX_MARKS: usize = 50;
/// A mark's words at most (chars): the strip's bubble shows three lines of them.
pub const TEXT_MAX: usize = 200;

/// One message of his: its entry's pos, when he sent it, his words cut
/// short, and his message as typed parts, lean ([`lean`]).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Mark {
    pub pos: Pos,
    pub at_ms: u64,
    /// his words only, on one line ([`short`]): never a marker or a tag
    /// (msg-rail-fix: the bubble showed `<image name=… path=…>`)
    pub text: String,
    /// his message as the thread draws it (the desktop's bubble uses the
    /// thread's own renderer): his words cut to [`TEXT_MAX`], his chips
    /// (images, files, pastes with their text emptied); none from an
    /// older hub
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(optional, as = "Option<Vec<Part>>"))]
    pub parts: Vec<Part>,
}

/// His words on one line (runs of whitespace folded), at most [`TEXT_MAX`]
/// chars, cut at a word with '…' when longer.
pub fn short(text: &str) -> String {
    let one = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= TEXT_MAX {
        return one;
    }
    let cut: String = one.chars().take(TEXT_MAX).collect();
    let at_word = cut.rfind(' ').filter(|&i| i > TEXT_MAX / 2).map_or(cut.as_str(), |i| &cut[..i]);
    format!("{}…", at_word.trim_end())
}

/// The entry's mark when it is a message of his: its parts, or its text
/// read back into parts ([`parse`]: an older transcript has no
/// `you-parts` line), lean; its words from them.
pub fn mark_of(e: &Entry) -> Option<Mark> {
    if e.kind != EntryKind::You {
        return None;
    }
    let parts = lean(if e.parts.is_empty() { parse(&e.text) } else { e.parts.clone() });
    Some(Mark { pos: e.pos, at_ms: e.at_ms, text: short(&words(&parts)), parts })
}

/// His parts as a tick's bubble draws them, small on the wire: the
/// model's own lines out (a hint, a view's context, a forward's header,
/// an unknown part), his words cut to [`TEXT_MAX`] chars in all ('…' at
/// the cut, the words after it out), a paste's and a page's text emptied
/// (their chip and notes stay), a quote's cut short. Pure.
pub fn lean(parts: Vec<Part>) -> Vec<Part> {
    let mut left = TEXT_MAX;
    let mut out = Vec::with_capacity(parts.len());
    for p in parts {
        out.push(match p {
            Part::Hint { .. } | Part::ViewContext { .. } | Part::Forwarded { .. } | Part::Unknown(_) => continue,
            Part::Text { text } => {
                if left == 0 {
                    continue;
                }
                let n = text.chars().count();
                if n <= left {
                    left -= n;
                    Part::Text { text }
                } else {
                    let cut: String = text.chars().take(left).collect();
                    left = 0;
                    Part::Text { text: format!("{}…", cut.trim_end()) }
                }
            }
            Part::Paste { n, .. } => Part::Paste { n, text: String::new() },
            Part::Page { to, page, title, version, notes, .. } => Part::Page { text: String::new(), to, page, title, version, notes },
            Part::Quote { from, text, file, new, old, n } => Part::Quote { from, text: short(&text), file, new, old, n },
            Part::Anchor { artifact, version, rev, anchor, quote, n } => Part::Anchor { artifact, version, rev, anchor, quote: short(&quote), n },
            p @ (Part::Image { .. } | Part::File { .. } | Part::Plugin { .. }) => p,
        });
    }
    out
}

/// His words in `parts`: what he typed, his `@plugin`s, his notes on a
/// page; with none, his chips' names (an image alone says `[Image #1]`).
fn words(parts: &[Part]) -> String {
    let mut said = String::new();
    for p in parts {
        match p {
            Part::Text { text } => said.push_str(text),
            Part::Plugin { name } => said.push_str(&format!("@{name}")),
            Part::Page { notes, .. } => {
                let w: Vec<String> = notes.iter().map(|n| page_note_words(n, true)).filter(|w| !w.is_empty()).collect();
                said.push_str(&format!(" {} ", w.join(" · ")));
            }
            _ => {}
        }
    }
    if !said.trim().is_empty() {
        return said;
    }
    let names: Vec<String> = parts
        .iter()
        .filter_map(|p| match p {
            Part::Image { name, .. } => Some(name.clone()),
            Part::File { path, .. } => Some(file_name(path).to_string()),
            _ => None,
        })
        .collect();
    names.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thread::{fold, Attached, Ctx, Line};

    fn folded(lines: &[Line]) -> Vec<Entry> {
        let page = |_: &str| None;
        let ctx = Ctx { open_cards: &[], page: &page, provider: &|id: &str, _: &str| id.to_string(), width: &|s: &str| s.chars().count(), offset: &|_| 0, attached: &Attached::plain };
        fold(lines, &ctx)
    }

    #[test]
    fn his_messages_only_and_a_brief_is_mains() {
        let lines: Vec<Line> = vec![
            (1, 10, "sb msg-in :: {\"body\":\"# Task `x`\\n\\nObjective: go\",\"from\":\"main\",\"id\":\"m_1\"}".into()),
            (2, 20, "sb you : make it\n".into()),
            (3, 30, "  obs: assistant: on it".into()),
            (4, 40, "sb msg-in : peer m_3 : nice".into()),
            (5, 50, "sb you : and faster".into()),
        ];
        let marks: Vec<Mark> = folded(&lines).iter().filter_map(mark_of).collect();
        let t = |s: &str| vec![Part::Text { text: s.into() }];
        assert_eq!(marks.iter().map(|m| (m.pos, m.at_ms, m.text.as_str())).collect::<Vec<_>>(), vec![(2, 20, "make it"), (5, 50, "and faster")]);
        assert_eq!(marks.iter().map(|m| m.parts.clone()).collect::<Vec<_>>(), vec![t("make it\n"), t("and faster")]);
    }

    fn you(text: &str, parts: Vec<Part>) -> Entry {
        let mut e = Entry::new(7, 70, EntryKind::You, text.to_string());
        e.parts = parts;
        e
    }

    // msg-rail-fix (his m_63243): the bubble showed '<image name="[Image #1]" path=…>'
    // Mutation: mark_of with `text: short(&e.text)` turns it red.
    #[test]
    fn a_mark_says_his_words_never_a_marker_and_carries_his_chips() {
        let img = Part::Image { name: "[Image #1]".into(), source: "/u/shot.png".into(), mime: "image/png".into(), b64_file: "/h/images/ab.b64".into(), caption: String::new() };
        let file = Part::File { path: "/u/deck q3.pptx".into(), name: "[File #2]".into(), missing: false };
        let parts = vec![Part::Text { text: "look at ".into() }, img.clone(), Part::Text { text: " and ".into() }, file.clone(), Part::Paste { n: 3, text: "a long paste".repeat(50) }];
        let text = crate::parts::render(&parts);
        let m = mark_of(&you(&text, parts)).unwrap();
        assert_eq!(m.text, "look at and");
        assert_eq!(m.parts, vec![Part::Text { text: "look at ".into() }, img.clone(), Part::Text { text: " and ".into() }, file, Part::Paste { n: 3, text: String::new() }]);
        // the wire's own line (fixtures/hub_ev.jsonl, the desktop's typed fixture): this mark
        let wire = include_str!("../../fixtures/hub_ev.jsonl").lines().find(|l| l.contains("\"thread_marks\"") && l.contains("\"parts\"")).unwrap();
        let v: serde_json::Value = serde_json::from_str(wire).unwrap();
        assert_eq!(v["marks"][0], serde_json::to_value(Mark { pos: 52, at_ms: 1_700_000_900_000, ..m.clone() }).unwrap());
        // an older transcript (no you-parts line): its text read back, the same words
        let old = mark_of(&you(&format!("look at {}", crate::parts::image_marker("[Image #1]", "/u/shot.png", "image/png", "/h/images/ab.b64", "")), Vec::new())).unwrap();
        assert_eq!(old.text, "look at");
        assert!(old.parts.contains(&img), "{:?}", old.parts);
        // an image alone: its name, never empty
        assert_eq!(mark_of(&you("", vec![img])).unwrap().text, "[Image #1]");
    }

    #[test]
    fn lean_cuts_his_words_in_all_and_drops_the_models_lines() {
        let long = "word ".repeat(30);
        let parts = vec![
            Part::Hint { of: crate::parts::HintKind::Capsule, text: "answer short".into() },
            Part::Text { text: long.clone() },
            Part::Text { text: long.clone() },
            Part::Text { text: long },
        ];
        let l = lean(parts);
        let n: usize = l.iter().map(|p| if let Part::Text { text } = p { text.trim_end_matches('…').chars().count() } else { 0 }).sum();
        assert!(n <= TEXT_MAX, "{n}");
        assert_eq!(l.len(), 2, "{l:?}");
        assert!(matches!(&l[1], Part::Text { text } if text.ends_with('…')));
    }

    #[test]
    fn short_folds_whitespace_and_cuts_at_a_word() {
        assert_eq!(short("a\n\n  b\tc"), "a b c");
        let s = short(&"word ".repeat(100));
        assert!(s.ends_with("word…"), "{s}");
        assert!(s.chars().count() <= TEXT_MAX + 1);
        assert_eq!(short(&"x".repeat(300)).chars().count(), TEXT_MAX + 1);
    }
}
