//! c6 (architect m_56402): a message's words are not in sb-core's state.
//! The journal keeps them (its `message_sent` line, whole); sb-core's
//! state keeps the message's header only. What sb-core writes that
//! carries a message's words (a feed record's text or parts) carries a
//! [`Body`] instead: the words themselves when sb-core made them
//! ([`Body::Lit`]), else a reference to the message by its id. The shell
//! fills it from the journal before the record is written ([`fill`]: the
//! one interpreter of a `Body`, called by switchboard's core_link).
//!
//! A message whose words can't be read (no line, a parse error, a
//! truncated journal) is a [`Missing`], rendered by [`Missing::text`]
//! everywhere, never an empty string.

use super::{FieldKind, Table};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A message's words as sb-core writes them in a record (Bend `K.Body`,
/// gen/kit.bend), on the wire `{"body": <word>, ...}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "body", rename_all = "snake_case")]
pub enum Body {
    /// words sb-core made itself (a literal)
    Lit { text: String },
    /// message `id`'s text, whole
    Text { id: u64 },
    /// message `id`'s parts, as their JSON value
    Parts { id: u64 },
}

/// A message's words as its `message_sent` line has them: its text and
/// its parts as JSON text ("" when it has none).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Words {
    pub text: String,
    pub parts: String,
}

impl Words {
    /// The words of a `message_sent` journal line; none for another line.
    pub fn of_journal(ev: &Value) -> Option<(u64, Words)> {
        if ev["type"] != "message_sent" {
            return None;
        }
        let m = &ev["msg"];
        let id = m["id"].as_u64()?;
        let s = |k: &str| m[k].as_str().unwrap_or_default().to_string();
        Some((id, Words { text: s("text"), parts: s("parts") }))
    }

    /// Its parts (msg-parts), read from their JSON (sb-core keeps them as
    /// an opaque string); none for a message without (or an unreadable
    /// one, as the fold reads it).
    pub fn parts_list(&self) -> Vec<crate::parts::Part> {
        match self.parts.is_empty() {
            true => Vec::new(),
            false => serde_json::from_str(&self.parts).unwrap_or_default(),
        }
    }
}

/// Message `id`'s words could not be read from the journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Missing(pub u64);

impl Missing {
    /// What a missing message's words read as, wherever they go.
    pub fn text(&self) -> String {
        format!("[m_{}: its words are missing from the journal]", self.0)
    }
}

/// The words `b` names: its literal, message `id`'s text or its parts'
/// JSON text.
pub fn words(b: &Body, get: &mut dyn FnMut(u64) -> Result<Words, Missing>) -> Result<String, Missing> {
    Ok(match b {
        Body::Lit { text } => text.clone(),
        Body::Text { id } => get(*id)?.text,
        Body::Parts { id } => get(*id)?.parts,
    })
}

/// What `b` reads as in a field of `kind`: a [`FieldKind::Body`] its
/// words as a string, a [`FieldKind::BodyJson`] its words parsed (sb-core's
/// `parts_json`: null when they don't parse). A missing message reads
/// [`Missing::text`], in a JSON field as one text part.
pub fn value(b: &Body, kind: FieldKind, get: &mut dyn FnMut(u64) -> Result<Words, Missing>) -> Value {
    let json = kind == FieldKind::BodyJson;
    debug_assert!(matches!(kind, FieldKind::Body | FieldKind::BodyJson | FieldKind::OptBodyOmit), "{kind:?}: not a body field");
    match (words(b, get), json) {
        (Ok(w), false) => Value::String(w),
        (Ok(w), true) => serde_json::from_str(&w).unwrap_or(Value::Null),
        (Err(e), false) => Value::String(e.text()),
        (Err(e), true) => serde_json::json!([{"kind": "text", "text": e.text()}]),
    }
}

/// Fill the [`FieldKind::Body`], [`FieldKind::BodyJson`] and
/// [`FieldKind::OptBodyOmit`] fields of `v`,
/// a record of `table` as sb-core wrote it (its tag under the table's
/// key), in place. A field that isn't a [`Body`] is a bug of sb-core's
/// encoder: an error, the record left as it is.
pub fn fill(table: &Table, v: &mut Value, get: &mut dyn FnMut(u64) -> Result<Words, Missing>) -> Result<(), String> {
    let Some(kind) = v[table.tag_key].as_str().and_then(|t| table.kind(t)) else { return Ok(()) };
    let mut bodies = Vec::new();
    for f in kind.fields.iter().filter(|f| matches!(f.kind, FieldKind::Body | FieldKind::BodyJson | FieldKind::OptBodyOmit)) {
        let Some(slot) = v.get(f.name) else { continue };
        let b: Body = serde_json::from_value(slot.clone()).map_err(|e| format!("{} {}: not a body: {e}", kind.tag, f.name))?;
        bodies.push((f, b));
    }
    for (f, b) in bodies {
        v[f.name] = value(&b, f.kind, get);
    }
    Ok(())
}

#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
