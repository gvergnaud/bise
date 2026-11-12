//! The UI words, once (debt-audit pick 2, docs/issues/29-ui-words-live-once.md;
//! architect m_27078): every word the TUI and the desktop show lives here,
//! as data, one file per area. The clients and the tests read it:
//!
//! - Rust: `words::inbox::where_inbox` (a word with no arg, a `&str`),
//!   `words::inbox::where_thread(agent)` (a template, its args by name:
//!   a wrong count is a compile error);
//! - TS: `apps/desktop/src/proto/words_<area>.gen.ts`, `inbox.where_thread({ agent })`;
//! - the Python tests: `tests/words/<area>.gen.json`, read by
//!   `tests/words.py`'s `w("inbox.where_thread", agent="main")`.
//!
//! Both generated outputs come from `rust/proto/tests/ts.rs` (one bless,
//! fresh-checked by proto_ts). `tests/word_lint.py` refuses a test literal
//! that equals one of these words (`tests/word_literals.txt`, shrink-only).
//!
//! Rules:
//! - designer owns the words: a key or a template moves here, its words
//!   never change in the move;
//! - UI words only: what an agent reads (prompts, sb outputs, `[bise]`
//!   lines) stays out;
//! - a template is data: `{arg}` holes, and a plural's two forms (`one`
//!   when the arg `n` is 1). A state -> word choice is one key per form and
//!   the `match` stays in the Rust fn that picks; wording logic that is not
//!   data (clip to a width, fit a row, a number's format) stays a Rust fn;
//! - a word goes in [`common`] only when two areas read it (the commit that
//!   puts it there names the two);
//! - every key is read by a client (Rust as `<area>::<name>`, the desktop
//!   as `<area>.<name>`): deleting a word deletes its key (the repo's
//!   tests/words_read_check.py, a text scan the quick gate runs on any reader's change);
//! - a key must not share a name with any arg of its area: the generated
//!   fn param shadows the const, E0308 (a `title` word breaks every
//!   `(title, …)` template of its area; tests.rs says which).

use std::fmt::Display;

pub mod bims;
pub mod common;
pub mod composer;
pub mod debug;
pub mod ds;
pub mod frame;
pub mod hero;
pub mod inbox;
pub mod library;
pub mod menus;
pub mod pages;
pub mod panel;
pub mod projects;
pub mod running;
pub mod scheduled;
pub mod settings;
pub mod side;
pub mod thread;
pub mod views;
pub mod viewkit;

/// One word of the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Word {
    /// `<area>.<name>`
    pub key: &'static str,
    /// the holes of the template, in order
    pub args: &'static [&'static str],
    /// a plural's form for `n == 1`
    pub one: Option<&'static str>,
    /// the template (the other form of a plural)
    pub text: &'static str,
    /// what it is, where it shows (its `///` lines)
    pub doc: &'static str,
}

/// Every area's words, by area.
pub const ALL: &[(&str, &[Word])] = &[
    ("bims", bims::WORDS),
    ("common", common::WORDS),
    ("composer", composer::WORDS),
    ("debug", debug::WORDS),
    ("ds", ds::WORDS),
    ("frame", frame::WORDS),
    ("hero", hero::WORDS),
    ("inbox", inbox::WORDS),
    ("library", library::WORDS),
    ("menus", menus::WORDS),
    ("pages", pages::WORDS),
    ("panel", panel::WORDS),
    ("projects", projects::WORDS),
    ("running", running::WORDS),
    ("scheduled", scheduled::WORDS),
    ("settings", settings::WORDS),
    ("side", side::WORDS),
    ("thread", thread::WORDS),
    ("views", views::WORDS),
    ("viewkit", viewkit::WORDS),
];

/// The words of an area split into submodules, as one list (architect
/// m_27109: an area file near 1,000 lines splits, its keys stay
/// `<area>.<name>`). In the area's file:
///
/// ```ignore
/// const PARTS: &[&[Word]] = &[general::WORDS, update::WORDS];
/// pub const WORDS: &[Word] = &super::concat::<{ super::count(PARTS) }>(PARTS);
/// ```
pub const fn concat<const N: usize>(parts: &[&[Word]]) -> [Word; N] {
    const BLANK: Word = Word { key: "", args: &[], one: None, text: "", doc: "" };
    let mut out = [BLANK; N];
    let (mut i, mut at) = (0, 0);
    while i < parts.len() {
        let mut j = 0;
        while j < parts[i].len() {
            out[at] = parts[i][j];
            at += 1;
            j += 1;
        }
        i += 1;
    }
    assert!(at == N, "concat: N is not the parts' count");
    out
}

/// How many words the parts hold (`concat`'s N).
pub const fn count(parts: &[&[Word]]) -> usize {
    let (mut i, mut n) = (0, 0);
    while i < parts.len() {
        n += parts[i].len();
        i += 1;
    }
    n
}

/// A template with its holes filled: `{name}` takes that arg's words.
/// The catalog test holds every hole to a declared arg; an unknown one
/// would stay as written.
pub fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let hole = after.find('}').and_then(|j| args.iter().find(|(n, _)| *n == &after[..j]).map(|(_, v)| (v, &after[j + 1..])));
        match hole {
            Some((v, tail)) => {
                out.push_str(&v.to_string());
                rest = tail;
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A plural's form: `one` when the arg `n` is 1, else `other`.
pub fn pick(one: &'static str, other: &'static str, args: &[(&str, &dyn Display)]) -> &'static str {
    match args.iter().find(|(n, _)| *n == "n") {
        Some((_, v)) if v.to_string() == "1" => one,
        _ => other,
    }
}

/// The holes of a template, in order (`{agent}` -> `agent`).
pub fn holes(template: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        let after = &rest[i + 1..];
        let Some(j) = after.find('}') else { break };
        out.push(&after[..j]);
        rest = &after[j + 1..];
    }
    out
}

/// An area's words: `crate::words! { inbox: /// doc  name = "text"; ... }`.
///
/// - `name = "words";` -> `pub const name: &str`;
/// - `name(a, b) = "… {a} … {b}";` -> `pub fn name(a: impl Display, b: impl Display) -> String`;
/// - `name(n, …) = one "{n} row" | "{n} rows";` -> the same fn, `one` when n is 1.
#[macro_export]
macro_rules! words {
    ($area:ident: $( $(#[doc = $doc:literal])* $name:ident $( ( $($arg:ident),+ ) )? = $(one $one:literal |)? $text:literal ; )* ) => {
        $( $crate::words!(@item [$($doc)*] $name ( $($($arg),+)? ) [$($one)?] $text); )*

        /// This area's words, in order (bise_proto::words::ALL).
        pub const WORDS: &[$crate::words::Word] = &[
            $( $crate::words::Word {
                key: concat!(stringify!($area), ".", stringify!($name)),
                args: &[$($(stringify!($arg)),+)?],
                one: $crate::words!(@one $($one)?),
                text: $text,
                doc: concat!($($doc, "\n",)* ""),
            }, )*
        ];
    };
    (@one) => { None };
    (@one $one:literal) => { Some($one) };
    (@item [$($doc:literal)*] $name:ident () [] $text:literal) => {
        $(#[doc = $doc])*
        #[allow(non_upper_case_globals)]
        pub const $name: &str = $text;
    };
    (@item [$($doc:literal)*] $name:ident ($($arg:ident),+) [] $text:literal) => {
        $(#[doc = $doc])*
        pub fn $name($($arg: impl ::std::fmt::Display),+) -> String {
            $crate::words::fill($text, &[$((stringify!($arg), &$arg as &dyn ::std::fmt::Display)),+])
        }
    };
    (@item [$($doc:literal)*] $name:ident ($($arg:ident),+) [$one:literal] $text:literal) => {
        $(#[doc = $doc])*
        pub fn $name($($arg: impl ::std::fmt::Display),+) -> String {
            let args: &[(&str, &dyn ::std::fmt::Display)] = &[$((stringify!($arg), &$arg as &dyn ::std::fmt::Display)),+];
            $crate::words::fill($crate::words::pick($one, $text, args), args)
        }
    };
}

#[cfg(test)]
mod tests;
