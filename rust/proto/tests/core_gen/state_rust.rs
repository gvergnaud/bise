//! The Rust types of sb-core's records a Rust reader parses
//! (`rust/proto/src/core/state/generated.rs`), generated from
//! `bise_proto::core::state::STATE` and its `RUST` list: a part of
//! core_gen.rs (its helpers are the parent's and state.rs's).

use super::state::{is_word, named};
use super::*;
use bise_proto::core::state as st;
use bise_proto::core::state::{Ty, Wire, STATE};

/// The Rust name of a declared record or enum ([`st::RUST`]).
fn rust_name(n: &str) -> &'static str {
    st::RUST.iter().find(|(b, _)| *b == n).map(|(_, r)| *r).unwrap_or_else(|| panic!("{n}: no Rust name in core::state::RUST"))
}

/// A variant's Rust name: its word in CamelCase.
fn camel(word: &str) -> String {
    word.split('_').map(|p| p[..1].to_uppercase() + &p[1..]).collect()
}

/// A key's Rust type, and its serde attribute (an omitted key).
fn rust_key(r: &st::Record, k: &st::Key) -> (String, Option<&'static str>) {
    let f = r.field(k.field);
    assert!(k.read.is_none(), "{}.{}: a hand read has no Rust reader yet", r.name, k.field);
    assert!(f.default.is_none() || matches!(k.wire, Wire::Word), "{}.{}: a declared default has no Rust reader yet", r.name, k.field);
    match k.wire {
        Wire::Str => ("String".into(), None),
        Wire::StrOmit => ("String".into(), Some("String::is_empty")),
        Wire::Nat => ("u64".into(), None),
        Wire::Bool => ("bool".into(), None),
        Wire::Flag => ("bool".into(), Some("super::read::is_false")),
        Wire::OptStr => ("Option<String>".into(), None),
        Wire::OptStrOmit => ("Option<String>".into(), Some("Option::is_none")),
        Wire::OptNat => ("Option<u64>".into(), None),
        Wire::OptNatOmit => ("Option<u64>".into(), Some("Option::is_none")),
        Wire::Json => ("serde_json::Value".into(), None),
        Wire::Strs => ("Vec<String>".into(), None),
        Wire::Nats => ("Vec<u64>".into(), None),
        Wire::Word | Wire::Obj => (rust_name(named(f.ty)).into(), None),
        Wire::OptWord => (format!("Option<{}>", rust_name(named(f.ty))), None),
        Wire::OptObjOmit => (format!("Option<{}>", rust_name(named(f.ty))), Some("Option::is_none")),
        w => panic!("{}.{}: {w:?} has no Rust reader yet", r.name, k.field),
    }
}

/// A doc comment, wrapped at 76 columns.
fn rust_doc(s: &mut String, indent: &str, doc: &str) {
    let mut line = String::new();
    for w in doc.split_whitespace() {
        if !line.is_empty() && indent.len() + 4 + line.len() + 1 + w.len() > 76 {
            writeln!(s, "{indent}/// {line}").unwrap();
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(w);
    }
    writeln!(s, "{indent}/// {line}").unwrap();
}

fn rust_enum(s: &mut String, e: &st::Enum) {
    let n = rust_name(e.name);
    let var = |v: &st::Variant| format!("{n}::{}", camel(v.word));
    let fallback = e.variants.iter().find(|v| v.name == e.fallback).unwrap();
    let fty = |f: &st::Field| match f.ty {
        Ty::Str => "String",
        Ty::Nat => "u64",
        t => panic!("{t:?}: an enum variant's field is a string or a number"),
    };
    rust_doc(s, "", e.doc);
    let word = is_word(e);
    assert!(word || !e.tag_key.is_empty(), "{}: a hand codec has no Rust reader yet", e.name);
    if word {
        writeln!(s, "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]").unwrap();
    } else {
        writeln!(s, "#[derive(Clone, Debug, PartialEq, Eq)]").unwrap();
    }
    writeln!(s, "pub enum {n} {{").unwrap();
    for v in e.variants {
        if word && v.name == e.fallback {
            writeln!(s, "    #[default]").unwrap();
        }
        match v.fields {
            [] => writeln!(s, "    {},", camel(v.word)).unwrap(),
            fs => {
                let fs: Vec<String> = fs.iter().map(|f| format!("{}: {}", f.name, fty(f))).collect();
                writeln!(s, "    {} {{ {} }},", camel(v.word), fs.join(", ")).unwrap();
            }
        }
    }
    writeln!(s, "}}\n").unwrap();
    let pat = |v: &st::Variant| match v.fields {
        [] => var(v),
        _ => format!("{} {{ .. }}", var(v)),
    };
    // a struct literal on its lines, as rustfmt writes it (`ind`: the
    // match arm's indent)
    let lit = |v: &st::Variant, ind: &str, val: &dyn Fn(&st::Field) -> String| match v.fields {
        [] => var(v),
        fs => {
            let fs: Vec<String> = fs.iter().map(|f| format!("{ind}    {}: {},\n", f.name, val(f))).collect();
            format!("{} {{\n{}{ind}}}", var(v), fs.concat())
        }
    };
    let read = |v: &st::Variant| lit(v, "            ", &|f| format!("field(&o, \"{}\")", f.name));
    let me = if word { "self" } else { "&self" };
    writeln!(s, "impl {n} {{\n    /// Its word.\n    pub fn word({me}) -> &'static str {{\n        match self {{").unwrap();
    for v in e.variants {
        writeln!(s, "            {} => \"{}\",", pat(v), v.word).unwrap();
    }
    writeln!(s, "        }}\n    }}").unwrap();
    if word {
        writeln!(s, "\n    /// A word as sb-core reads it: any other reads `{}`.", fallback.word).unwrap();
        writeln!(s, "    pub fn of_word(w: &str) -> {n} {{\n        match w {{").unwrap();
        for v in e.variants.iter().filter(|v| v.name != e.fallback) {
            writeln!(s, "            \"{}\" => {},", v.word, var(v)).unwrap();
        }
        writeln!(s, "            _ => {},\n        }}\n    }}", var(fallback)).unwrap();
    }
    writeln!(s, "}}\n").unwrap();
    if word {
        writeln!(s, "impl Serialize for {n} {{
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {{
        s.serialize_str(self.word())
    }}
}}

/// Not a string: an error (its field then reads its default).
impl<'de> Deserialize<'de> for {n} {{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {{
        Ok({n}::of_word(&String::deserialize(d)?))
    }}
}}
").unwrap();
        return;
    }
    let tk = e.tag_key;
    let dflt = lit(fallback, "        ", &|_| "Default::default()".into());
    writeln!(s, "impl Default for {n} {{
    fn default() -> Self {{
        {dflt}
    }}
}}
").unwrap();
    writeln!(s, "/// Its word under \"{tk}\", its fields beside it.
impl Serialize for {n} {{
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {{
        let mut m = s.serialize_map(None)?;
        m.serialize_entry(\"{tk}\", self.word())?;
        match self {{").unwrap();
    for v in e.variants {
        match v.fields {
            [] => writeln!(s, "            {} => {{}}", var(v)).unwrap(),
            fs => {
                let names: Vec<&str> = fs.iter().map(|f| f.name).collect();
                writeln!(s, "            {} {{ {} }} => {{", var(v), names.join(", ")).unwrap();
                for f in fs {
                    writeln!(s, "                m.serialize_entry(\"{0}\", {0})?;", f.name).unwrap();
                }
                writeln!(s, "            }}").unwrap();
            }
        }
    }
    writeln!(s, "        }}
        m.end()
    }}
}}

/// Any other word, or none: `{}`, its fields read as usual.
impl<'de> Deserialize<'de> for {n} {{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {{
        let o = Obj::deserialize(d)?;
        Ok(match word(&o, \"{tk}\") {{", fallback.word).unwrap();
    for v in e.variants.iter().filter(|v| v.name != e.fallback) {
        writeln!(s, "            \"{}\" => {},", v.word, read(v)).unwrap();
    }
    writeln!(s, "            _ => {},
        }})
    }}
}}
", read(fallback)).unwrap();
}

fn rust_record(s: &mut String, r: &st::Record) {
    let n = rust_name(r.name);
    let sh = r.shape("journal").unwrap_or_else(|| panic!("{}: a Rust record reads its journal shape", r.name));
    for f in r.fields {
        assert!(sh.keys.iter().any(|k| k.field == f.name), "{}.{}: not in its journal shape", r.name, f.name);
    }
    rust_doc(s, "", r.doc);
    writeln!(s, "#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]\npub struct {n} {{").unwrap();
    for k in sh.keys {
        let (t, skip) = rust_key(r, k);
        if let Some(skip) = skip {
            writeln!(s, "    #[serde(skip_serializing_if = \"{skip}\")]").unwrap();
        }
        writeln!(s, "    pub {}: {t},", k.key).unwrap();
    }
    writeln!(s, "}}\n").unwrap();
    writeln!(s, "/// Not an object: an error (its reader skips it).
impl<'de> Deserialize<'de> for {n} {{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {{
        let o = Obj::deserialize(d)?;
        Ok({n} {{").unwrap();
    for k in sh.keys {
        writeln!(s, "            {0}: field(&o, \"{0}\"),", k.key).unwrap();
    }
    writeln!(s, "        }})\n    }}\n}}\n").unwrap();
}

pub(crate) fn rust_types() -> String {
    let mut s = String::new();
    writeln!(s, "//! Generated by bise-proto (rust/proto/tests/core_gen.rs), never
//! hand-edited: `{BLESS}=1 cargo test -p bise-proto --test core_gen`.
//!
//! sb-core's records a Rust reader parses ([`super::RUST`]), in their
//! journal shape: a field is named as its key, read as sb-core reads it
//! (a missing key or a value of another type: its default), written in
//! the shape's key order.

use super::read::{{field, word, Obj}};
use serde::ser::SerializeMap;
use serde::{{Deserialize, Deserializer, Serialize, Serializer}};
").unwrap();
    for (b, _) in st::RUST {
        if let Some(e) = STATE.enum_(b) {
            rust_enum(&mut s, e);
        } else {
            rust_record(&mut s, STATE.record(b).unwrap_or_else(|| panic!("{b}: not declared")));
        }
    }
    s.pop();
    s
}
