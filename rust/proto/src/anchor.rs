//! Where a note sits on an artifact (office-artifacts plan §5, architect
//! m_23701 §9.5): one type for every target, wire from the start.
//!
//! An [`Anchor`] is an object tagged `"kind"`:
//!
//! - `block`: a block of a bise page or an html artifact, `{block, quote?}`;
//! - `cell`: a range of a sheet, `{sheet, range}` (`B4`, `B4:D9`);
//! - `slide`: a slide of a deck, or a box on it, `{slide, rect?}`;
//! - `shape`: a shape of a slide, `{slide, shape, quote?}` (`shape` is the
//!   shape's `p:cNvPr/@id`, as the SDK's `inspect()` says it);
//! - `para`: a paragraph of a document, `{para, quote?}` (its `w14:paraId`);
//! - `region`: a box on an image or a pdf page, `{page?, rect, quote?}`
//!   (a pdf passage: its words' box and its words);
//! - `text`: a line of any text file, `{line, quote?}`;
//! - `time`: a moment of a video, or a box on its frame there, `{t, rect?}`
//!   (`t` in seconds from its start, finite and never negative; video-view);
//! - `view`: a place a plugin's view names itself, `{key, quote?}` (`key`
//!   its opaque string, 1..=512 bytes; `quote` cleaned on read:
//!   [`clean_quote`]; plugin-apps A5, kit/view's anchors).
//!
//! Slides, pages and lines count from 1. A [`Rect`] is in % of the
//! frame (the slide, the image, the page): `0..=100` each.
//!
//! The wire rule (lib.rs): fields are only added; a kind this version does
//! not know reads as [`Anchor::Unknown`] with its whole object, and writes
//! back unchanged, so a note an older bise keeps loses nothing. A known kind
//! with bad fields reads as `Unknown` too (a store never drops a note).
//! The TS (`anchor.ts` in apps/desktop/src/proto, and
//! `office/sdk/src/anchor.gen.ts` for the SDK) is made by `tests/ts.rs`;
//! `Unknown` is not in it: the SDK's `at()` refuses a kind it does not know.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A box in % of its frame (the slide, the image, the page).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Where a note sits (the module's doc says each kind).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Anchor {
    /// a block of a page or an html artifact
    Block {
        block: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        quote: String,
    },
    /// a range of a sheet: `B4`, `B4:D9`
    Cell { sheet: String, range: String },
    /// a slide, or a box drawn on it
    Slide {
        slide: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        rect: Option<Rect>,
    },
    /// a shape of a slide, by its id (`p:cNvPr/@id`)
    Shape {
        slide: u32,
        shape: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        quote: String,
    },
    /// a paragraph of a document, by its `w14:paraId`
    Para {
        para: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        quote: String,
    },
    /// a box on an image, or on a page of a pdf; a passage of a pdf is
    /// its words' box with the words in `quote` (pdf-notes, architect m_24668)
    Region {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        page: Option<u32>,
        rect: Rect,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        quote: String,
    },
    /// a line of a text file
    Text {
        line: u32,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        quote: String,
    },
    /// a moment of a video (`t` seconds from its start), or a box drawn on
    /// its frame there
    Time {
        #[serde(deserialize_with = "moment")]
        t: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        rect: Option<Rect>,
    },
    /// a place a plugin's view names itself (plugin-apps A5, architect
    /// m_59942): its opaque `key` (1..=512 bytes) and the words he saw
    /// selected (one line, at most [`QUOTE_MAX`] characters, no control
    /// characters: cleaned on read), and its own short name for the place
    /// (`label`: `page 2 · a box`, `0:12`; at most [`LABEL_MAX`]
    /// characters, cleaned the same way; display only: the place is the
    /// key alone, architect m_60509)
    View {
        #[serde(deserialize_with = "view_key")]
        key: String,
        #[serde(default, skip_serializing_if = "String::is_empty", deserialize_with = "view_quote")]
        quote: String,
        #[serde(default, skip_serializing_if = "String::is_empty", deserialize_with = "view_label")]
        label: String,
    },
    /// a kind this version doesn't know (or a known one with bad fields):
    /// the whole object, written back as it came
    #[serde(untagged)]
    #[cfg_attr(feature = "ts", ts(skip))]
    Unknown(Value),
}

/// A video's moment: seconds, finite and never negative (architect m_32064);
/// anything else makes the anchor `Unknown`, kept as it came.
fn moment<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let t = f64::deserialize(d)?;
    match t.is_finite() && t >= 0.0 {
        true => Ok(t),
        false => Err(serde::de::Error::custom("a moment is seconds, 0 or more")),
    }
}

/// The longest key a view may give its anchor, in bytes.
pub const VIEW_KEY_MAX: usize = 512;
/// The most characters a view's quote keeps (kit/office/select.js's
/// `quoteOf` cap).
pub const QUOTE_MAX: usize = 200;

/// A view's key: 1..=[`VIEW_KEY_MAX`] bytes; anything else makes the anchor
/// `Unknown`, kept as it came.
fn view_key<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let key = String::deserialize(d)?;
    match !key.is_empty() && key.len() <= VIEW_KEY_MAX {
        true => Ok(key),
        false => Err(serde::de::Error::custom("a view's key is 1 to 512 bytes")),
    }
}

fn view_quote<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(clean_quote(&String::deserialize(d)?))
}

fn view_label<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(short(&one_line(&String::deserialize(d)?), LABEL_MAX))
}

/// The most characters a view's label keeps.
pub const LABEL_MAX: usize = 60;

/// One line: control characters gone, whitespace runs one space.
fn one_line(s: &str) -> String {
    s.split(|c: char| c.is_whitespace() || c.is_control()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// What a view says he selected, as bise keeps it: control characters
/// gone, whitespace runs one space, at most [`QUOTE_MAX`] characters (`…`
/// when cut). A view's text is third-party input.
pub fn clean_quote(s: &str) -> String {
    short(&one_line(s), QUOTE_MAX)
}

/// A moment as a player says it: `0:12`, `12:05`, `1:02:05` (whole seconds).
pub fn clock(t: f64) -> String {
    let s = if t.is_finite() && t > 0.0 { t.floor() as u64 } else { 0 };
    match s / 3600 {
        0 => format!("{}:{:02}", s / 60, s % 60),
        h => format!("{h}:{:02}:{:02}", (s / 60) % 60, s % 60),
    }
}

impl Rect {
    /// A box in % of its frame, each number clamped to `0..=100` (a NaN
    /// becomes 0): the one way Rust code builds a rect.
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        let pct = |v: f64| if v.is_nan() { 0.0 } else { v.clamp(0.0, 100.0) };
        Rect { x: pct(x), y: pct(y), w: pct(w), h: pct(h) }
    }
}

// Equality is total: a rect comes from JSON, where serde refuses NaN, or
// from [`Rect::new`], which never makes one; Rust code never builds a Rect
// with NaN otherwise. So an anchor can sit in a message part
// (`parts::Part::Anchor`, which is Eq).
impl Eq for Rect {}
impl Eq for Anchor {}

/// What a chip and a note's line call an anchor (designer signs the
/// words, designer m_24098: never a machine id): `Q3!B4:D9`, `slide 2`,
/// `slide 2 · a box`, `slide 2 · "Revenue"`, `slide 2 · a shape`,
/// `"net revenue…"` (a paragraph by its words), `page 1 · a box`,
/// `page 1 · "net revenue…"` (a pdf passage), `line 40`, `0:12`, `0:12 · a box`. Both apps draw it from this rule (fixture anchor_label.json).
pub fn label(a: &Anchor) -> String {
    let quoted = |q: &str| format!("\"{}\"", short(q, 40));
    match a {
        Anchor::Block { block, quote } if quote.trim().is_empty() => format!("block {block}"),
        Anchor::Block { quote, .. } => quoted(quote),
        Anchor::Cell { sheet, range } => format!("{}!{range}", sheet_ref(sheet)),
        Anchor::Slide { slide, rect: None } => format!("slide {slide}"),
        Anchor::Slide { slide, rect: Some(_) } => format!("slide {slide} · a box"),
        Anchor::Shape { slide, quote, .. } if !quote.trim().is_empty() => format!("slide {slide} · {}", quoted(quote)),
        Anchor::Shape { slide, .. } => format!("slide {slide} · a shape"),
        Anchor::Para { quote, .. } if !quote.trim().is_empty() => quoted(quote),
        Anchor::Para { .. } => "a paragraph".into(),
        Anchor::Region { page: Some(p), quote, .. } if !quote.trim().is_empty() => format!("page {p} · {}", quoted(quote)),
        Anchor::Region { page: None, quote, .. } if !quote.trim().is_empty() => quoted(quote),
        Anchor::Region { page: Some(p), .. } => format!("page {p} · a box"),
        Anchor::Region { page: None, .. } => "a box".into(),
        Anchor::Text { line, .. } => format!("line {line}"),
        Anchor::Time { t, rect: None } => clock(*t),
        Anchor::Time { t, rect: Some(_) } => format!("{} · a box", clock(*t)),
        // a view names its place itself (`label`), his words say what is there
        Anchor::View { quote, label, .. } => {
            let (q, l) = (clean_quote(quote), short(&one_line(label), LABEL_MAX));
            match (l.is_empty(), q.is_empty()) {
                (false, false) => format!("{l} · {}", quoted(&q)),
                (false, true) => l,
                (true, false) => quoted(&q),
                (true, true) => "a part".into(),
            }
        }
        Anchor::Unknown(_) => "a part".into(),
    }
}

/// A sheet's name as a reference says it: `Q3`, `'My sheet'` (Excel's
/// rule: quoted unless letters, digits, `_` and `.`, not starting with a
/// digit; a `'` inside doubled).
fn sheet_ref(sheet: &str) -> String {
    let plain = !sheet.is_empty() && !sheet.starts_with(|c: char| c.is_ascii_digit()) && sheet.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
    match plain {
        true => sheet.to_string(),
        false => format!("'{}'", sheet.replace('\'', "''")),
    }
}

/// One line, at most `max` characters, `…` when cut.
fn short(s: &str, max: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    match one.chars().count() <= max {
        true => one,
        false => format!("{}…", one.chars().take(max - 1).collect::<String>().trim_end()),
    }
}

/// A note's place as a plugin's view names it (plugin-apps A5): the
/// view's own key, and the words he selected there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewKey {
    pub key: String,
    pub quote: String,
}

/// A note's anchor as a view sees it (architect m_60380, signed with
/// apps-media): a `view` anchor gives its key (its label stays out: the
/// place is the key alone, m_60509); a typed kind (a note made before its
/// type moved onto a plugin's view) becomes the canonical JSON of the
/// anchor without its quote (sorted keys, no whitespace), so the view that
/// now opens the type can place it; `Unknown`, or a key over
/// [`VIEW_KEY_MAX`] bytes, is None: the host shows that note detached, its
/// text kept. Pure; nothing on disk is rewritten.
pub fn as_view_key(a: &Anchor) -> Option<ViewKey> {
    let (key, quote) = match a {
        Anchor::View { key, quote, .. } => (key.clone(), quote.clone()),
        Anchor::Unknown(_) => return None,
        typed => {
            let mut v = serde_json::to_value(typed).ok()?;
            let quote = v.as_object_mut()?.remove("quote").and_then(|q| q.as_str().map(str::to_string)).unwrap_or_default();
            (serde_json::to_string(&canonical(v)).ok()?, quote)
        }
    };
    (!key.is_empty() && key.len() <= VIEW_KEY_MAX).then_some(ViewKey { key, quote })
}

/// A JSON value with every object's keys sorted (a Map is ordered by key
/// unless serde_json's preserve_order is on: this keeps the key the same
/// either way).
fn canonical(v: Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut kv: Vec<(String, Value)> = m.into_iter().map(|(k, v)| (k, canonical(v))).collect();
            kv.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(kv.into_iter().collect())
        }
        Value::Array(a) => Value::Array(a.into_iter().map(canonical).collect()),
        other => other,
    }
}

impl Anchor {
    /// The kinds this version knows, as on the wire.
    pub const KINDS: &'static [&'static str] = &["block", "cell", "slide", "shape", "para", "region", "text", "time", "view"];

    /// The kind as on the wire (an unknown one's own, `""` when it has none).
    pub fn kind(&self) -> &str {
        match self {
            Anchor::Block { .. } => "block",
            Anchor::Cell { .. } => "cell",
            Anchor::Slide { .. } => "slide",
            Anchor::Shape { .. } => "shape",
            Anchor::Para { .. } => "para",
            Anchor::Region { .. } => "region",
            Anchor::Text { .. } => "text",
            Anchor::Time { .. } => "time",
            Anchor::View { .. } => "view",
            Anchor::Unknown(v) => v.get("kind").and_then(Value::as_str).unwrap_or(""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read(v: Value) -> Anchor {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn each_kind_reads_and_writes_back() {
        let lines = [
            json!({"kind": "block", "block": "b3", "quote": "the plan"}),
            json!({"kind": "cell", "sheet": "Q3", "range": "B4:D9"}),
            json!({"kind": "slide", "slide": 2, "rect": {"x": 10.0, "y": 20.0, "w": 30.0, "h": 5.5}}),
            json!({"kind": "shape", "slide": 3, "shape": "12"}),
            json!({"kind": "para", "para": "3F2A1B00", "quote": "net revenue"}),
            json!({"kind": "region", "page": 1, "rect": {"x": 0.0, "y": 0.0, "w": 50.0, "h": 50.0}}),
            json!({"kind": "region", "page": 2, "rect": {"x": 10.0, "y": 30.0, "w": 60.0, "h": 4.0}, "quote": "net revenue grew"}),
            json!({"kind": "text", "line": 40}),
            json!({"kind": "time", "t": 12.5}),
            json!({"kind": "time", "t": 3725.0, "rect": {"x": 10.0, "y": 20.0, "w": 30.0, "h": 5.5}}),
            json!({"kind": "view", "key": "card:42"}),
        ];
        let mut seen = std::collections::BTreeSet::new();
        for l in lines {
            let a = read(l.clone());
            assert!(!matches!(a, Anchor::Unknown(_)), "{l}");
            assert!(Anchor::KINDS.contains(&a.kind()));
            assert_eq!(serde_json::to_value(&a).unwrap(), l);
            seen.insert(a.kind().to_string());
        }
        // KINDS is every variant, no more (the TS's ANCHOR_KINDS is made from it)
        assert_eq!(seen.len(), Anchor::KINDS.len());
    }

    #[test]
    fn an_unknown_kind_keeps_its_whole_object() {
        let l = json!({"kind": "frame", "frame": 7, "t": 1.5});
        let a = read(l.clone());
        assert_eq!(a, Anchor::Unknown(l.clone()));
        assert_eq!(a.kind(), "frame");
        assert_eq!(serde_json::to_value(&a).unwrap(), l);
        // a known kind with a field missing is kept the same way, not refused
        let bad = json!({"kind": "cell", "sheet": "Q3"});
        assert_eq!(read(bad.clone()), Anchor::Unknown(bad));
    }

    /// architect m_23734: a known kind with bad fields is Unknown and its
    /// line comes back byte for byte (art-notes says "bad cell anchor" for it).
    #[test]
    fn a_known_kind_with_bad_fields_writes_back_byte_equal() {
        for line in [r#"{"kind":"cell","range":"B4","sheet":7}"#, r#"{"kind":"slide","slide":"two"}"#, r#"{"kind":"region","page":1}"#, r#"{"kind":"time","t":-1.0}"#, r#"{"kind":"time","t":"0:12"}"#, r#"{"kind":"time"}"#] {
            let a: Anchor = serde_json::from_str(line).unwrap();
            assert!(matches!(a, Anchor::Unknown(_)), "{line}");
            assert!(Anchor::KINDS.contains(&a.kind()), "{line}");
            assert_eq!(serde_json::to_string(&a).unwrap(), line);
        }
    }

    /// architect m_59942: a view's key is opaque, 1..=512 bytes; outside
    /// that the anchor is Unknown and comes back as it came.
    #[test]
    fn a_view_key_is_one_to_512_bytes() {
        let at = |key: &str| json!({"kind": "view", "key": key});
        assert!(matches!(read(at(&"k".repeat(VIEW_KEY_MAX))), Anchor::View { .. }));
        for bad in [at(""), at(&"k".repeat(VIEW_KEY_MAX + 1)), json!({"kind": "view", "key": 7}), json!({"kind": "view"})] {
            let a = read(bad.clone());
            assert!(matches!(a, Anchor::Unknown(_)), "{bad}");
            assert_eq!(a.kind(), "view");
            assert_eq!(serde_json::to_value(&a).unwrap(), bad);
        }
    }

    /// A view's quote is its text: one line, no control characters, at most
    /// QUOTE_MAX characters, cleaned on read so a label never shows raw input.
    #[test]
    fn a_view_quote_is_cleaned_on_read() {
        let a = read(json!({"kind": "view", "key": "p3", "quote": "  net\u{0007} revenue\n\tgrew\u{001b}[2J "}));
        assert_eq!(a, Anchor::View { key: "p3".into(), quote: "net revenue grew [2J".into(), label: String::new() });
        let Anchor::View { quote, .. } = read(json!({"kind": "view", "key": "p3", "quote": "é".repeat(QUOTE_MAX + 5)})) else { panic!() };
        assert_eq!(quote.chars().count(), QUOTE_MAX);
        assert!(quote.ends_with('…'));
        assert_eq!(label(&Anchor::View { key: "p3".into(), quote: String::new(), label: String::new() }), "a part");
    }

    /// architect m_60509: a view's label is its own short name for the
    /// place, cleaned and cut on read like quote; display only.
    #[test]
    fn a_view_label_is_cleaned_and_said_before_the_quote() {
        let Anchor::View { label: l, .. } = read(json!({"kind": "view", "key": "b1", "label": "page 2\n·\u{0007} a box"})) else { panic!() };
        assert_eq!(l, "page 2 · a box");
        let Anchor::View { label: l, .. } = read(json!({"kind": "view", "key": "b1", "label": "x".repeat(LABEL_MAX + 9)})) else { panic!() };
        assert_eq!(l.chars().count(), LABEL_MAX);
        let v = |quote: &str, label: &str| Anchor::View { key: "b1".into(), quote: quote.into(), label: label.into() };
        assert_eq!(super::label(&v("net revenue", "page 2")), "page 2 · \"net revenue\"");
        assert_eq!(super::label(&v("", "0:12 · a box")), "0:12 · a box");
        assert_eq!(super::label(&v("net revenue", "")), "\"net revenue\"");
    }

    /// architect m_60380: every kind as a view sees it, one row each.
    #[test]
    fn as_view_key_per_kind() {
        let k = |v: Value| as_view_key(&read(v)).map(|x| (x.key, x.quote));
        let s = |key: &str, quote: &str| Some((key.to_string(), quote.to_string()));
        let rows = [
            (json!({"kind": "block", "block": "b3", "quote": "the plan"}), s(r#"{"block":"b3","kind":"block"}"#, "the plan")),
            (json!({"kind": "cell", "sheet": "Q3", "range": "B4:D9"}), s(r#"{"kind":"cell","range":"B4:D9","sheet":"Q3"}"#, "")),
            (json!({"kind": "slide", "slide": 2, "rect": {"y": 20.0, "x": 10.0, "w": 30.0, "h": 5.5}}), s(r#"{"kind":"slide","rect":{"h":5.5,"w":30.0,"x":10.0,"y":20.0},"slide":2}"#, "")),
            (json!({"kind": "shape", "slide": 3, "shape": "12", "quote": "Revenue"}), s(r#"{"kind":"shape","shape":"12","slide":3}"#, "Revenue")),
            (json!({"kind": "para", "para": "3F2A1B00", "quote": "net"}), s(r#"{"kind":"para","para":"3F2A1B00"}"#, "net")),
            (json!({"kind": "region", "page": 2, "rect": {"x": 1.0, "y": 2.0, "w": 3.0, "h": 4.0}, "quote": "grew"}), s(r#"{"kind":"region","page":2,"rect":{"h":4.0,"w":3.0,"x":1.0,"y":2.0}}"#, "grew")),
            (json!({"kind": "text", "line": 40}), s(r#"{"kind":"text","line":40}"#, "")),
            (json!({"kind": "time", "t": 12.5}), s(r#"{"kind":"time","t":12.5}"#, "")),
            (json!({"kind": "view", "key": "row:7", "quote": "net", "label": "row 7"}), s("row:7", "net")),
            (json!({"kind": "voxel", "x": 1}), None),
            (json!({"kind": "cell", "sheet": "Q3"}), None),
            (json!({"kind": "block", "block": "b".repeat(VIEW_KEY_MAX)}), None),
        ];
        for (a, want) in rows {
            assert_eq!(k(a.clone()), want, "{a}");
        }
    }

    /// architect m_60509 (1): two anchors that differ only by label are
    /// the same place (the label is display only).
    #[test]
    fn a_view_label_never_changes_the_place() {
        let a = read(json!({"kind": "view", "key": "box:1", "label": "page 2 · a box"}));
        let b = read(json!({"kind": "view", "key": "box:1", "label": "page 3"}));
        assert_eq!(as_view_key(&a), as_view_key(&b));
    }

    #[test]
    fn a_moment_says_its_clock() {
        assert_eq!([clock(0.0), clock(12.9), clock(725.0), clock(3725.4), clock(-3.0), clock(f64::NAN)], ["0:00", "0:12", "12:05", "1:02:05", "0:00", "0:00"]);
    }

    #[test]
    fn a_rect_is_clamped_to_its_frame() {
        assert_eq!(Rect::new(-5.0, 20.0, 130.0, f64::NAN), Rect { x: 0.0, y: 20.0, w: 100.0, h: 0.0 });
        assert!(serde_json::from_str::<Rect>(r#"{"x":NaN,"y":0,"w":1,"h":1}"#).is_err(), "JSON has no NaN");
    }

    #[test]
    fn inside_a_note_too() {
        #[derive(Serialize, Deserialize)]
        struct Note {
            text: String,
            at: Anchor,
        }
        let l = json!({"text": "why?", "at": {"kind": "voxel", "x": 1}});
        let n: Note = serde_json::from_value(l.clone()).unwrap();
        assert_eq!(n.at.kind(), "voxel");
        assert_eq!(serde_json::to_value(&n).unwrap(), l);
    }
}
