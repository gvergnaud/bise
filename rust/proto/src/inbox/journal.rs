//! The typed reader of sb-core's card lines (the start of debt D1's
//! "typed Rust reader of journal lines", architect m_24257): `card_opened`,
//! `card_closed` and `card_answer_pos`, their keys those of their rows in
//! `core::journal::JOURNAL` (the test `the_card_lines_are_the_tables`
//! keeps them in step) and of the declared `Card` and `Ans` journal shapes
//! (core::state, bend/hub/gen/state-codec.bend).
//! A line of another kind, or one that doesn't fit, reads as none: a
//! journal is never refused for one line (a torn last line included).

use serde::Deserialize;
use serde_json::Value;

/// One card line of the journal.
#[derive(Clone, Debug, PartialEq)]
pub enum CardLine {
    Opened(CardOpened),
    Closed(CardClosed),
    AnswerPos(CardAnswerPos),
}

/// `card_opened`: the card, as sb-core's `card_json` writes it.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CardOpened {
    pub card: Card,
}

/// sb-core's card (gen/state-codec.bend `Card.journal_json`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Card {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub for_msg: Option<u64>,
    #[serde(default)]
    pub created_ms: u64,
    #[serde(default)]
    pub place: Option<String>,
    #[serde(default)]
    pub pr: Option<u64>,
    /// what it is about, as it opened (card-links: `sb notify --artifact`,
    /// a page's question); none from a line sb-core wrote before it
    #[serde(default)]
    pub about: Option<crate::rows::ArtLink>,
}

/// `card_closed`: `via` "" when it closed without his answer (an older
/// line has none of his answer's keys: it reads so).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CardClosed {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub resolution: String,
    #[serde(default)]
    pub said: String,
    #[serde(default)]
    pub pick_n: Option<u64>,
    #[serde(default)]
    pub pick: String,
    #[serde(default)]
    pub via: String,
    #[serde(default)]
    pub page: String,
    #[serde(default)]
    pub block: String,
}

/// `card_answer_pos`: his answer's route line in `agent`'s transcript.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CardAnswerPos {
    #[serde(default)]
    pub card: u64,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub pos: u64,
}

/// The tags this reader knows.
pub const TAGS: &[&str] = &["card_opened", "card_closed", "card_answer_pos"];

impl CardLine {
    /// One journal line, when it is a card line that fits.
    pub fn of(line: &Value) -> Option<CardLine> {
        let tag = line.get("type")?.as_str()?;
        let v = line.clone();
        match tag {
            "card_opened" => serde_json::from_value(v).ok().map(CardLine::Opened),
            "card_closed" => serde_json::from_value(v).ok().map(CardLine::Closed),
            "card_answer_pos" => serde_json::from_value(v).ok().map(CardLine::AnswerPos),
            _ => None,
        }
    }

    /// One line of text (a journal file's), when it is a card line.
    pub fn of_text(line: &str) -> Option<(CardLine, Value)> {
        let v: Value = serde_json::from_str(line).ok()?;
        Some((CardLine::of(&v)?, v))
    }
}
