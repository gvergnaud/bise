//! The journal's generated Bend (`bend/hub/gen/journal.bend`,
//! `journal-laws.bend`) from `bise_proto::core::journal`: a part of
//! core_gen.rs (its helpers are the parent's).

use super::*;
use bise_proto::core::journal::NEVER;
use bise_proto::core::{Codec, Field, JOURNAL};

// ---- the journal (bise_proto::core::journal): sb-core's own `M.Ev` ----

/// A journal row's `M.Ev` pattern with each field name replaced by
/// `fill(name)` (a field name is an identifier not followed by `{`).
fn fill(k: &Kind, fill: &dyn Fn(&str) -> String) -> String {
    let p = k.bend;
    let b = p.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.') {
                i += 1;
            }
            let id = &p[start..i];
            if b.get(i) == Some(&b'{') {
                out.push_str(id);
            } else {
                assert!(k.fields.iter().any(|f| f.name == id), "{}: {id} in its pattern is no field", k.tag);
                out.push_str(&fill(id));
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    for f in k.fields {
        assert!(contains_var(p, f.name), "{}: field {} not in its pattern", k.tag, f.name);
    }
    out
}

fn contains_var(p: &str, name: &str) -> bool {
    p.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')).any(|w| w == name)
}

/// A journal field's hand codec, if it has one.
fn codec_of(k: FieldKind) -> Option<&'static Codec> {
    match k {
        FieldKind::Rec(c) | FieldKind::Inline(c) | FieldKind::InlineOpt(c) => Some(c),
        _ => None,
    }
}

const JOURNAL_IMPORTS: &str = "
import ./kit.bend as K

import ../model.bend as M

import ../timers.bend as TM

import ../wakes.bend as WK

import ../xhub-model.bend as XH

import ./state.bend as S

import ./state-codec.bend as SC
";

const EV: &str = "Maybe<&2, M.Ev>";

/// `journal.bend`: the read of a line (its kind by a binary search over
/// the tags, each field with sb-core's default) and the write of an
/// event (one match on `M.Ev`, every constructor a row: no catch-all).
pub(crate) fn journal_codec() -> String {
    let t = &JOURNAL;
    let mut s = String::new();
    header(&mut s, "journal: bise_proto::core::journal::JOURNAL (tag \"type\"), sb-core's journal lines: their read and write.");
    s.push_str(JOURNAL_IMPORTS);
    writeln!(s, "\n# ---- read: each field with sb-core's default when missing; a line of a\n# kind this core doesn't know is skipped (None) ----\n").unwrap();
    for k in t.kinds {
        let opt = k.fields.iter().find(|f| matches!(f.kind, FieldKind::InlineOpt(_)));
        let i = if k.fields.is_empty() { "i" } else { "+i" };
        match opt {
            None => {
                let e = fill(k, &|n| decode(field(k, n).kind, n));
                writeln!(s, "def {}.of({i}: J.Json) -> {EV}:\n  Some{{{e}}}\n", k.rust).unwrap();
            }
            Some(o) => {
                let c = codec_of(o.kind).unwrap();
                let v = var(o.name);
                let e = fill(k, &|n| if n == o.name { v.clone() } else { decode(field(k, n).kind, n) });
                writeln!(s, "# no {} read: the line is skipped", o.name).unwrap();
                writeln!(s, "def {}.with(m: Maybe<&2, {}>, +i: J.Json) -> {EV}:", k.rust, c.ty).unwrap();
                writeln!(s, "  match m:\n    case None{{}}:\n      None{{}}\n    case Some{{+{v}}}:\n      Some{{{e}}}\n").unwrap();
                writeln!(s, "def {}.of(+i: J.Json) -> {EV}:\n  {}.with({}.{}_of(i), i)\n", k.rust, k.rust, c.module, c.name).unwrap();
            }
        }
    }
    lookup_in(&mut s, t.kinds, EV, "None{}", &|k| format!("{}.of(i)", k.rust));
    writeln!(s, "# one journal line's event").unwrap();
    writeln!(s, "def of(+i: J.Json) -> {EV}:\n  of_tag(K.get_s(i, \"{}\"), i)\n", t.tag_key).unwrap();

    writeln!(s, "# ---- write: the key-value list, in wire order ----\n").unwrap();
    writeln!(s, "def kvs(e: M.Ev) -> List<&2, {KV}>:\n  match e:").unwrap();
    for k in t.kinds {
        writeln!(s, "    case {}:\n      {}", fill(k, &|n| format!("+{}", var(n))), kv_list(t.tag_key, k.tag, k)).unwrap();
    }
    for k in NEVER {
        writeln!(s, "    # never journaled (journal.rs NEVER): written as it always was").unwrap();
        writeln!(s, "    case {}:\n      {}", fill(k, &|n| format!("+{}", var(n))), kv_list(t.tag_key, k.tag, k)).unwrap();
    }
    writeln!(s, "\n# one event's journal line\ndef json(e: M.Ev) -> J.Json:\n  J.obj(kvs(e))").unwrap();
    s
}

fn field(k: &Kind, name: &str) -> &'static Field {
    k.fields.iter().find(|f| f.name == name).unwrap()
}

/// A journal field's cases in a decode∘encode proof: each a pattern, and
/// whether it is rewritten by its codec's law `SC.<name>_round`.
fn journal_cases(f: &Field, v: &str) -> Vec<(String, bool)> {
    match f.kind {
        FieldKind::Flag => vec![("True{}".into(), false), ("False{}".into(), false)],
        FieldKind::Rec(c) | FieldKind::Inline(c) if !c.cases.is_empty() => {
            c.cases.iter().map(|p| (p.replace('$', &format!("{v}_")), false)).collect()
        }
        FieldKind::Rec(_) => vec![(v.to_string(), true)],
        FieldKind::Inline(c) if c.module == "SC" => inline_record_cases(c, v).into_iter().map(|p| (p, false)).collect(),
        FieldKind::Inline(c) => panic!("{}: an inline codec needs its cases", c.name),
        k => {
            assert!(cases(k, v).iter().all(|(_, rw)| rw.is_none()), "{}: a list field in the journal", f.name);
            cases(k, v).into_iter().map(|(p, _)| (p, false)).collect()
        }
    }
}

/// An inline declared record without cases (`SC.<T>.<shape>`: card_closed's
/// `Ans.journal`): its line's object holds other keys before its own, so
/// its record law can't close the row's. The proof opens it, `S.<T>{..}`
/// (gen/ only: the state guard allows it), and splits on each field its
/// shape writes only sometimes, like a row's own fields.
fn inline_record_cases(c: &Codec, v: &str) -> Vec<String> {
    use bise_proto::core::state::{Wire, STATE};
    let (name, shape) = c.name.split_once('.').unwrap_or_else(|| panic!("{}: SC codecs are <T>.<shape>", c.name));
    let r = STATE.record(name).unwrap_or_else(|| panic!("{name}: not a declared record"));
    let sh = r.shape(shape).unwrap_or_else(|| panic!("{name}: no shape {shape}"));
    let mut pats: Vec<Vec<String>> = vec![vec![]];
    for f in r.fields {
        let fv = format!("{v}_{}", f.name);
        let alts = match sh.keys.iter().find(|k| k.field == f.name).map(|k| k.wire) {
            Some(Wire::StrOmit) => vec!["SNil{}".to_string(), format!("SCon{{h_{fv}, t_{fv}}}")],
            Some(Wire::OptStr | Wire::OptStrOmit | Wire::OptNat | Wire::OptNatOmit) => vec!["None{}".to_string(), format!("Some{{x_{fv}}}")],
            Some(Wire::Flag) => vec!["True{}".to_string(), "False{}".to_string()],
            _ => vec![fv],
        };
        pats = pats.into_iter().flat_map(|p| alts.iter().map(move |a| [p.clone(), vec![a.clone()]].concat())).collect();
    }
    pats.into_iter().map(|p| format!("S.{name}{{{}}}", p.join(", "))).collect()
}

/// A fixture line's JSON, its keys in their order (serde_json's map is
/// sorted; the laws compare objects key by key).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Jv {
    Null,
    Bool(bool),
    Num(u64),
    Str(String),
    Arr(Vec<Jv>),
    Obj(Vec<(String, Jv)>),
}

impl Jv {
    pub(crate) fn get(&self, k: &str) -> Option<&Jv> {
        match self {
            Jv::Obj(kvs) => kvs.iter().find(|(x, _)| x == k).map(|(_, v)| v),
            _ => None,
        }
    }

    fn tag(&self) -> &str {
        match self.get("type") {
            Some(Jv::Str(s)) => s,
            _ => panic!("a fixture line has its type"),
        }
    }
}

/// Reads one fixture line: plain strings (no escapes), Nat numbers.
pub(crate) fn jv(text: &str) -> Jv {
    fn ws(b: &[u8], i: &mut usize) {
        while *i < b.len() && (b[*i] as char).is_ascii_whitespace() {
            *i += 1;
        }
    }
    fn val(b: &[u8], i: &mut usize) -> Jv {
        ws(b, i);
        let lit = |i: &mut usize, w: &str, v: Jv| {
            assert!(b[*i..].starts_with(w.as_bytes()), "fixture: bad literal");
            *i += w.len();
            v
        };
        match b[*i] {
            b'n' => lit(i, "null", Jv::Null),
            b't' => lit(i, "true", Jv::Bool(true)),
            b'f' => lit(i, "false", Jv::Bool(false)),
            b'"' => Jv::Str(string(b, i)),
            b'[' => {
                *i += 1;
                let mut xs = Vec::new();
                loop {
                    ws(b, i);
                    if b[*i] == b']' {
                        *i += 1;
                        return Jv::Arr(xs);
                    }
                    xs.push(val(b, i));
                    ws(b, i);
                    if b[*i] == b',' {
                        *i += 1;
                    }
                }
            }
            b'{' => {
                *i += 1;
                let mut kvs = Vec::new();
                loop {
                    ws(b, i);
                    if b[*i] == b'}' {
                        *i += 1;
                        return Jv::Obj(kvs);
                    }
                    let k = string(b, i);
                    ws(b, i);
                    assert_eq!(b[*i], b':', "fixture: a key without its value");
                    *i += 1;
                    kvs.push((k, val(b, i)));
                    ws(b, i);
                    if b[*i] == b',' {
                        *i += 1;
                    }
                }
            }
            _ => {
                let s = *i;
                while *i < b.len() && b[*i].is_ascii_digit() {
                    *i += 1;
                }
                let n: u64 = std::str::from_utf8(&b[s..*i]).unwrap().parse().expect("a fixture number is a Nat");
                assert!(n <= u32::MAX as u64, "a fixture number is a Bend Nat literal (at most 2^32 - 1): {n}");
                Jv::Num(n)
            }
        }
    }
    fn string(b: &[u8], i: &mut usize) -> String {
        assert_eq!(b[*i], b'"');
        *i += 1;
        let s = *i;
        while b[*i] != b'"' {
            assert!(b[*i] != b'\\', "a fixture string is plain");
            *i += 1;
        }
        *i += 1;
        std::str::from_utf8(&b[s..*i - 1]).unwrap().to_string()
    }
    let b = text.as_bytes();
    let mut i = 0;
    let v = val(b, &mut i);
    ws(b, &mut i);
    assert_eq!(i, b.len(), "fixture: one value per line");
    v
}

/// A fixture value as a Bend `J.Json` term.
pub(crate) fn bend_json(v: &Jv) -> String {
    match v {
        Jv::Null => "J.JNull{}".into(),
        Jv::Bool(b) => format!("J.JBool{{{}}}", if *b { "True{}" } else { "False{}" }),
        Jv::Num(n) => format!("J.JNum{{{n}n}}"),
        Jv::Str(s) => format!("J.JStr{{\"{s}\"}}"),
        Jv::Arr(xs) => format!("J.JArr{{[{}]}}", xs.iter().map(bend_json).collect::<Vec<_>>().join(", ")),
        Jv::Obj(kvs) => {
            let kvs: Vec<String> = kvs.iter().map(|(k, v)| format!("J.kv(\"{k}\", {})", bend_json(v))).collect();
            format!("J.obj([{}])", kvs.join(", "))
        }
    }
}

/// The released journal fixture: one line per kind, in a -34 hub's shape
/// (fake data; a wake_set's watch has today's `resumes`).
fn journal_fixture() -> Vec<Jv> {
    let text = std::fs::read_to_string(root().join("rust/proto/fixtures/core_journal.jsonl")).unwrap();
    text.lines().filter(|l| !l.trim().is_empty()).map(jv).collect()
}

/// What the generated write gives back for a fixture line: the line
/// itself, but an every_set's `at` after the timer's keys (its order
/// before: next_ms, at, then the schedule).
fn written_back(v: &Jv) -> Jv {
    match v {
        Jv::Obj(kvs) if v.tag() == "every_set" => {
            let (at, rest): (Vec<_>, Vec<_>) = kvs.iter().cloned().partition(|(k, _)| k == "at");
            Jv::Obj([rest, at].concat())
        }
        _ => v.clone(),
    }
}

/// `journal-laws.bend`: per row, its tag alone reads its defaults and
/// (but an inline read that may fail) its write reads back as itself;
/// an unknown or missing tag is skipped; each fixture line reads back
/// as itself.
pub(crate) fn journal_laws() -> String {
    let t = &JOURNAL;
    let mut s = String::new();
    header(&mut s, "journal laws: each journal line reads back what it writes; the tag table; the fixture.");
    s.push_str(JOURNAL_IMPORTS);
    writeln!(s, "\nimport ./journal.bend as GJ\n").unwrap();
    let bare = |tag: &str| format!("J.obj([J.kv(\"{}\", J.JStr{{\"{tag}\"}})])", t.tag_key);
    for k in t.kinds {
        let b = bare(k.tag);
        let want = if k.fields.iter().any(|f| matches!(f.kind, FieldKind::InlineOpt(_))) {
            "None{}".to_string()
        } else {
            let e = fill(k, &|n| match field(k, n).kind {
                FieldKind::Rec(c) => format!("{}.{}_of(J.JNull{{}})", c.module, c.name),
                FieldKind::Inline(c) => format!("{}.{}_of({b})", c.module, c.name),
                kd => default(kd).to_string(),
            });
            format!("Some{{{e}}}")
        };
        let law = format!("journal_{}_bare", k.tag);
        writeln!(s, "law {law}:\n  {{GJ.of({b}) == {want} : {EV}}}\n\ndef {law}():\n  {{==}}\n").unwrap();
    }
    for (law, input) in [("unknown", bare("weather")), ("untagged", "J.obj(Nil{})".to_string())] {
        let law = format!("journal_{law}_tag");
        writeln!(s, "law {law}:\n  {{GJ.of({input}) == None{{}} : {EV}}}\n\ndef {law}():\n  {{==}}\n").unwrap();
    }
    for k in t.kinds.iter().filter(|k| !k.fields.iter().any(|f| matches!(f.kind, FieldKind::InlineOpt(_)))) {
        journal_round(&mut s, k);
    }
    writeln!(s, "# ---- the released fixture (rust/proto/fixtures/core_journal.jsonl) ----\n").unwrap();
    writeln!(s, "def back(m: {EV}) -> Maybe<&2, J.Json>:\n  match m:\n    case None{{}}:\n      None{{}}\n    case Some{{+e}}:\n      Some{{GJ.json(e)}}\n").unwrap();
    for (n, line) in journal_fixture().iter().enumerate() {
        let tag = line.tag();
        let (l, w) = (bend_json(line), bend_json(&written_back(line)));
        let law = format!("journal_fixture_{n}_{tag}");
        writeln!(s, "law {law}:\n  {{back(GJ.of({l})) == Some{{{w}}} : Maybe<&2, J.Json>}}\n\ndef {law}():\n  {{==}}\n").unwrap();
    }
    // what we write after one read is stable: written again after a
    // second read, the same line (every_set's schedule is normalized once)
    writeln!(s, "def again(m: Maybe<&2, J.Json>) -> Maybe<&2, J.Json>:\n  match m:\n    case None{{}}:\n      None{{}}\n    case Some{{+j}}:\n      back(GJ.of(j))\n").unwrap();
    let extra: Vec<Jv> = STABLE_EXTRA.iter().map(|l| jv(l)).collect();
    for (n, line) in journal_fixture().iter().chain(&extra).enumerate() {
        let (tag, l) = (line.tag(), bend_json(line));
        let law = format!("journal_stable_{n}_{tag}");
        writeln!(s, "law {law}:\n  {{again(back(GJ.of({l}))) == back(GJ.of({l})) : Maybe<&2, J.Json>}}\n\ndef {law}():\n  {{==}}\n").unwrap();
    }
    s
}

/// every_set lines whose read changes them (the stable laws, beside the
/// fixture's): a period under a second, a daily minute past a day, no
/// optional key, no schedule (skipped).
const STABLE_EXTRA: &[&str] = &[
    r#"{"type":"every_set","id":3,"agent":"docs","by":"main","text":"ping","next_ms":5000,"at":4000,"every_ms":500}"#,
    r#"{"type":"every_set","id":4,"agent":"docs","by":"user","text":"standup","next_ms":5000,"at":4000,"daily_min":3000,"held":false}"#,
    r#"{"type":"every_set","id":5,"agent":"docs","by":"user","text":"none","next_ms":5000,"at":4000}"#,
];

/// A row's decode∘encode law: split on each field with cases, each
/// record without cases rewritten by its codec's own law.
fn journal_round(s: &mut String, k: &Kind) {
    let vars = field_vars(k);
    let law = format!("journal_{}_round", k.tag);
    writeln!(s, "law {law}:").unwrap();
    for f in k.fields {
        writeln!(s, "  for {}: {}", var(f.name), bend_ty(f.kind)).unwrap();
    }
    let ev = |args: &[String]| fill(k, &|n| args[k.fields.iter().position(|f| f.name == n).unwrap()].clone());
    writeln!(s, "  {{GJ.of(GJ.json({})) == Some{{{}}} : {EV}}}\n", ev(&vars), ev(&vars)).unwrap();
    writeln!(s, "def {law}({}):", vars.join(", ")).unwrap();
    let per: Vec<Vec<(String, bool)>> = k.fields.iter().zip(&vars).map(|(f, v)| journal_cases(f, v)).collect();
    let split: Vec<usize> = (0..per.len()).filter(|&i| per[i].len() > 1).collect();
    let mut combos: Vec<Vec<usize>> = vec![vec![]];
    for &i in &split {
        combos = combos.into_iter().flat_map(|c| (0..per[i].len()).map(move |j| [c.clone(), vec![j]].concat())).collect();
    }
    let ind = if split.is_empty() { "  " } else { "      " };
    if !split.is_empty() {
        writeln!(s, "  match {}:", split.iter().map(|&i| vars[i].clone()).collect::<Vec<_>>().join(" ")).unwrap();
    }
    for c in combos {
        let args: Vec<String> = (0..per.len()).map(|i| per[i][split.iter().position(|&x| x == i).map_or(0, |n| c[n])].0.clone()).collect();
        if !split.is_empty() {
            writeln!(s, "    case {}:", split.iter().map(|&i| args[i].clone()).collect::<Vec<_>>().join(" ")).unwrap();
        }
        let lhs = format!("GJ.of(GJ.json({}))", ev(&args));
        let mut rhs = args.clone();
        for (i, f) in k.fields.iter().enumerate() {
            if per[i].len() == 1 && per[i][0].1 {
                let c = codec_of(f.kind).unwrap();
                let mut hole = rhs.clone();
                hole[i] = "_".into();
                writeln!(s, "{ind}%{}.{}_round({}) : {{{lhs} == Some{{{}}} : {EV}}}", c.module, c.name, vars[i], ev(&hole)).unwrap();
                rhs[i] = format!("{m}.{}_of({m}.{}_json({}))", c.name, c.name, vars[i], m = c.module);
            }
        }
        writeln!(s, "{ind}{{==}}").unwrap();
    }
    writeln!(s).unwrap();
}

#[test]
fn the_journal_fixture_has_every_kind_once() {
    let tags: Vec<String> = journal_fixture().iter().map(|l| l.tag().to_string()).collect();
    let want: Vec<&str> = JOURNAL.kinds.iter().map(|k| k.tag).collect();
    assert_eq!(tags, want, "core_journal.jsonl: one line per JOURNAL row, in its order");
}

/// `NEVER`'s values stay unwritten: no sb-core source builds one (a
/// step that would journal it fails here, not in a replay).
#[test]
fn no_step_journals_a_held_end() {
    let dir = root().join("bend/hub");
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "bend") {
            let text = std::fs::read_to_string(&p).unwrap();
            for bad in ["REnd{S.RHeld{}}", "REnd{RHeld{}}"] {
                assert!(!text.contains(bad), "{}: builds {bad}, which journal.rs NEVER writes back as a route_held", p.display());
            }
        }
    }
}


// ---- the journal's kinds, as a Rust enum (debt D1, its first part) ----

/// `rust/proto/src/core/journal_kind.rs`: one unit variant per JOURNAL
/// row, so a Rust reader of the journal matches a line's kind
/// exhaustively (a new row is a new variant: every such match fails to
/// compile until it decides).
pub fn journal_kind_rust() -> String {
    let t = &JOURNAL;
    let mut s = String::new();
    writeln!(
        s,
        "//! Generated by bise-proto (rust/proto/tests/core_gen.rs), never
//! hand-edited: `BISE_PROTO_BLESS=1 cargo test -p bise-proto --test core_gen`.
//!
//! The kind of one of sb-core's journal lines (its `\"{}\"`), one variant
//! per row of [`super::JOURNAL`], in its order, and `Unknown` for a tag
//! this build does not know (a newer sb-core). The lines' fields stay
//! `Value`s (debt D1's rest).

/// The kind of a journal line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JournalKind {{",
        t.tag_key
    )
    .unwrap();
    for k in t.kinds {
        writeln!(s, "    /// `{}`\n    {},", k.tag, k.rust).unwrap();
    }
    writeln!(s, "    /// a tag this build does not know\n    Unknown,\n}}\n\nimpl JournalKind {{").unwrap();
    writeln!(s, "    /// Every known kind, in the table's order.\n    pub const ALL: &'static [JournalKind] = &[").unwrap();
    for k in t.kinds {
        writeln!(s, "        JournalKind::{},", k.rust).unwrap();
    }
    writeln!(s, "    ];\n\n    /// Its tag; none for `Unknown`.\n    pub fn tag(self) -> Option<&'static str> {{\n        match self {{").unwrap();
    for k in t.kinds {
        writeln!(s, "            JournalKind::{} => Some(\"{}\"),", k.rust, k.tag).unwrap();
    }
    writeln!(s, "            JournalKind::Unknown => None,\n        }}\n    }}\n").unwrap();
    writeln!(s, "    /// The kind of a tag; any other is `Unknown`.\n    pub fn from_tag(tag: &str) -> JournalKind {{\n        match tag {{").unwrap();
    for k in t.kinds {
        writeln!(s, "            \"{}\" => JournalKind::{},", k.tag, k.rust).unwrap();
    }
    writeln!(
        s,
        "            _ => JournalKind::Unknown,
        }}
    }}

    /// The kind of a journal line (its `\"{}\"`; none or not a string: `Unknown`).
    pub fn of_line(ev: &serde_json::Value) -> JournalKind {{
        ev.get(\"{}\").and_then(serde_json::Value::as_str).map_or(JournalKind::Unknown, JournalKind::from_tag)
    }}
}}",
        t.tag_key, t.tag_key
    )
    .unwrap();
    s
}
