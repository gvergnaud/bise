//! sb-core's state, generated (`bend/hub/gen/state.bend`,
//! `state-laws.bend`, `state-codec.bend`) from
//! `bise_proto::core::state::STATE`: a part of core_gen.rs (its helpers
//! are the parent's).
//!
//! - state.bend: the types; per record `T.empty()`, per field `T.f` and
//!   `T.with_f`. The only file where a record is built or opened by
//!   position (the gate's state guard checks it).
//! - state-laws.bend: per field `T.f.get_with`, `T.f.eta` (stated
//!   rebuilt == setter, so a proof rewrites with it as is), `T.f.set_get`
//!   (setting a field to its own value) and `T.f.set_set` (the last set
//!   wins; state-timers m_32709).
//! - state-codec.bend: per word enum its words both ways (`E.word`,
//!   `E.of_word`, `E.opt_json`, `E.opt_of`) and `E.round`, and as a journal
//!   row's value `E.journal_of/_json/_round` and `E.opt_journal_*`; per
//!   tagged enum `E.journal_kvs`, `_json`, `_of` and `_round`; per record shape
//!   `T.<shape>_kvs`, `_json`, and when sb-core reads it `_of`, one law
//!   per field it carries (`T.<shape>.<field>`: its read of the written
//!   object is its value) and the record's (`T.<shape>_round`), which
//!   rewrites once per field: no case split over the record; per view
//!   shape a law pinning it on fixtures/core_view.jsonl.

use super::journal::{bend_json, jv, Jv};
use super::*;
use bise_proto::core::state as st;
use bise_proto::core::state::{Ty, Wire, STATE};

const S: &str = "S.";

/// A field's Bend type, its declared names prefixed with `q`.
fn ty(t: Ty, q: &str) -> String {
    match t {
        Ty::Nat => "Nat".into(),
        Ty::Str => "String".into(),
        Ty::Bool => "Bool".into(),
        Ty::Json => "J.Json".into(),
        Ty::OptNat => "Maybe<&2, Nat>".into(),
        Ty::OptStr => "Maybe<&2, String>".into(),
        Ty::Strs => "List<&2, String>".into(),
        Ty::Nats => "List<&2, Nat>".into(),
        Ty::T(n) => format!("{q}{n}"),
        Ty::OptT(n) => format!("Maybe<&2, {q}{n}>"),
        Ty::Ts(n) => format!("List<&2, {q}{n}>"),
    }
}

/// A declared Bend term (a default) with its constructor prefixed.
fn qual(term: &str, q: &str) -> String {
    match term.chars().next() {
        Some(c) if c.is_ascii_uppercase() => format!("{q}{term}"),
        _ => term.to_string(),
    }
}

/// A field's default, its declared names prefixed with `q`.
fn dflt(f: &st::Field, q: &str) -> String {
    if let Some(d) = f.default {
        return qual(d, q);
    }
    match f.ty {
        Ty::Nat => "0n".into(),
        Ty::Str => "\"\"".into(),
        Ty::Bool => "False{}".into(),
        Ty::Json => "J.JNull{}".into(),
        Ty::OptNat | Ty::OptStr | Ty::OptT(_) => "None{}".into(),
        Ty::Strs | Ty::Nats | Ty::Ts(_) => "Nil{}".into(),
        Ty::T(n) => {
            assert!(STATE.record(n).is_some(), "{}: an enum field says its default", f.name);
            format!("{q}{n}.empty()")
        }
    }
}

/// `T{a, b, ..}`.
fn rec(q: &str, name: &str, args: &[String]) -> String {
    format!("{q}{name}{{{}}}", args.join(", "))
}

/// The pattern variables of a record's fields: f0, f1, ...
fn slots(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("f{i}")).collect()
}

// ---- state.bend ----

pub(crate) fn state_types() -> String {
    let mut s = String::new();
    header(&mut s, "state: bise_proto::core::state::STATE, sb-core's records: their types, empty(), a getter and a setter per field.");
    writeln!(s).unwrap();
    writeln!(s, "# ---- the enums ----\n").unwrap();
    for e in STATE.enums {
        writeln!(s, "# {}", e.doc).unwrap();
        writeln!(s, "type {} is Data:", e.name).unwrap();
        for v in e.variants {
            let fs: Vec<String> = v.fields.iter().map(|f| format!("{}: {}", var(f.name), ty(f.ty, ""))).collect();
            writeln!(s, "  {}{{{}}}", v.name, fs.join(", ")).unwrap();
        }
        writeln!(s).unwrap();
    }
    writeln!(s, "# ---- the records ----\n").unwrap();
    for r in STATE.records {
        writeln!(s, "# {}", r.doc).unwrap();
        let fs: Vec<String> = r.fields.iter().map(|f| format!("{}: {}", var(f.name), ty(f.ty, ""))).collect();
        writeln!(s, "type {} is Data:\n  {}{{{}}}\n", r.name, r.name, fs.join(", ")).unwrap();
    }
    for r in STATE.records {
        let n = r.name;
        let ds: Vec<String> = r.fields.iter().map(|f| dflt(f, "")).collect();
        writeln!(s, "# ---- {n} ----\n").unwrap();
        writeln!(s, "def {n}.empty() -> {n}:\n  {}\n", rec("", n, &ds)).unwrap();
        let vars = slots(r.fields.len());
        for (i, f) in r.fields.iter().enumerate() {
            let t = ty(f.ty, "");
            let get: Vec<String> = (0..vars.len()).map(|j| if j == i { vars[j].clone() } else { "_".into() }).collect();
            writeln!(s, "def {n}.{}(r: {n}) -> {t}:\n  match r:\n    case {}:\n      {}\n", f.name, rec("", n, &get), vars[i]).unwrap();
            let keep: Vec<String> = (0..vars.len()).map(|j| if j == i { "_".into() } else { vars[j].clone() }).collect();
            let mut put = vars.clone();
            put[i] = "v".into();
            writeln!(s, "def {n}.with_{}(r: {n}, v: {t}) -> {n}:\n  match r:\n    case {}:\n      {}\n", f.name, rec("", n, &keep), rec("", n, &put)).unwrap();
            // the field changed by g in one match, r consumed: the field
            // keeps one owner, so g can reuse it in place (a getter then a
            // setter shares it while r lives, and a list walk copies it)
            let mut over = vars.clone();
            over[i] = format!("g({})", vars[i]);
            writeln!(s, "def {n}.over_{}(r: {n}, g: {t} -> {t}) -> {n}:\n  match r:\n    case {}:\n      {}\n", f.name, rec("", n, &vars), rec("", n, &over)).unwrap();
        }
        // the record rebuilt by name from its getters (a proof's goal on
        // T.open(r) computes; T.open_eq rewrites r into it)
        let mut open = format!("{n}.empty()");
        for f in r.fields {
            open = format!("{n}.with_{}({open}, {n}.{}(r))", f.name, f.name);
        }
        writeln!(s, "def {n}.open(+r: {n}) -> {n}:\n  {open}\n").unwrap();
    }
    s
}

// ---- state-laws.bend ----

pub(crate) fn state_laws() -> String {
    let mut s = String::new();
    header(&mut s, "state laws: per field, a setter then its getter gives the value (get_with); the record rebuilt from its getters is the setter (eta, stated for a rewrite: `%T.f.eta(r, v) : {<goal with _ at the setter>}`, then any other getter computes, with no record written by position); setting a field to its own value is the record (set_get); of two sets the last wins (set_set).");
    writeln!(s, "\nimport ./state.bend as S\n").unwrap();
    for r in STATE.records {
        let n = r.name;
        let vars = slots(r.fields.len());
        let open = format!("  match r:\n    case {}:\n      {{==}}\n", rec(S, n, &vars));
        writeln!(s, "# ---- {n} ----\n").unwrap();
        for (i, f) in r.fields.iter().enumerate() {
            let t = ty(f.ty, S);
            let fname = f.name;
            writeln!(s, "law {n}.{fname}.get_with:\n  for r: S.{n}\n  for v: {t}").unwrap();
            writeln!(s, "  {{S.{n}.{fname}(S.{n}.with_{fname}(r, v)) == v : {t}}}\n").unwrap();
            writeln!(s, "def {n}.{fname}.get_with(r, v):\n{open}").unwrap();
            let gets: Vec<String> = r.fields.iter().enumerate().map(|(j, g)| if j == i { "v".into() } else { format!("S.{n}.{}(r)", g.name) }).collect();
            writeln!(s, "law {n}.{fname}.eta:\n  for r: S.{n}\n  for v: {t}").unwrap();
            writeln!(s, "  {{{} == S.{n}.with_{fname}(r, v) : S.{n}}}\n", rec(S, n, &gets)).unwrap();
            writeln!(s, "def {n}.{fname}.eta(r, v):\n{open}").unwrap();
            writeln!(s, "law {n}.{fname}.set_get:\n  for r: S.{n}\n  {{S.{n}.with_{fname}(r, S.{n}.{fname}(r)) == r : S.{n}}}\n").unwrap();
            writeln!(s, "def {n}.{fname}.set_get(r):\n{open}").unwrap();
            writeln!(s, "law {n}.{fname}.set_set:\n  for r: S.{n}\n  for a: {t}\n  for b: {t}").unwrap();
            writeln!(s, "  {{S.{n}.with_{fname}(S.{n}.with_{fname}(r, a), b) == S.{n}.with_{fname}(r, b) : S.{n}}}\n").unwrap();
            writeln!(s, "def {n}.{fname}.set_set(r, a, b):\n{open}").unwrap();
            writeln!(s, "law {n}.{fname}.over:\n  for r: S.{n}\n  for g: {t} -> {t}\n  {{S.{n}.over_{fname}(r, g) == S.{n}.with_{fname}(r, g(S.{n}.{fname}(r))) : S.{n}}}\n").unwrap();
            writeln!(s, "def {n}.{fname}.over(r, g):\n{open}").unwrap();
        }
        writeln!(s, "law {n}.open_eq:\n  for r: S.{n}\n  {{S.{n}.open(r) == r : S.{n}}}\n").unwrap();
        writeln!(s, "def {n}.open_eq(r):\n{open}").unwrap();
    }
    s
}

// ---- state-codec.bend ----

/// An enum written as its bare word: no tag key, no fields.
fn is_word(e: &st::Enum) -> bool {
    e.tag_key.is_empty() && e.variants.iter().all(|v| v.fields.is_empty())
}

fn the_enum(n: &str) -> &'static st::Enum {
    STATE.enum_(n).unwrap_or_else(|| panic!("{n}: not a declared enum"))
}

/// A field's declared type name (an enum or a record).
fn named(t: Ty) -> &'static str {
    match t {
        Ty::T(n) | Ty::OptT(n) | Ty::Ts(n) => n,
        t => panic!("{t:?}: not a declared type"),
    }
}

/// A variant pattern with fresh variables (`S.Every{p_0}`).
fn variant_pat(v: &st::Variant, p: &str) -> String {
    let xs: Vec<String> = (0..v.fields.len()).map(|i| format!("{p}_{i}")).collect();
    rec(S, v.name, &xs)
}

/// The value of one variant field on the wire, and its read from a lookup.
fn scalar(t: Ty, v: &str) -> (String, &'static str) {
    match t {
        Ty::Str => (format!("J.JStr{{{v}}}"), "rd.s"),
        Ty::Nat => (format!("J.JNum{{{v}}}"), "rd.n"),
        t => panic!("{t:?}: an enum variant's field is a string or a number"),
    }
}

fn enum_codec(s: &mut String, e: &st::Enum) {
    let n = e.name;
    let others: Vec<&st::Variant> = e.variants.iter().filter(|v| v.name != e.fallback).collect();
    let fallback = e.variants.iter().find(|v| v.name == e.fallback).expect("an enum's fallback is one of its variants");
    if is_word(e) {
        writeln!(s, "# ---- {n} (its word) ----\n").unwrap();
        writeln!(s, "def {n}.word(e: S.{n}) -> String:\n  match e:").unwrap();
        for v in e.variants {
            writeln!(s, "    case S.{}{{}}:\n      \"{}\"", v.name, v.word).unwrap();
        }
        let mut chain = format!("S.{}{{}}", fallback.name);
        for v in others.iter().rev() {
            chain = format!("Bool.pick(S.{n}, String.eq(w, \"{}\"), S.{}{{}}, {chain})", v.word, v.name);
        }
        writeln!(s, "\n# any other word: {}\ndef {n}.of_word(+w: String) -> S.{n}:\n  {chain}\n", fallback.word).unwrap();
        writeln!(s, "def {n}.opt_json(m: Maybe<&2, S.{n}>) -> J.Json:\n  match m:\n    case None{{}}:\n      J.JNull{{}}\n    case Some{{+e}}:\n      J.JStr{{{n}.word(e)}}\n").unwrap();
        writeln!(s, "def {n}.opt_of(m: Maybe<&2, J.Json>) -> Maybe<&2, S.{n}>:\n  match m:\n    case Some{{J.JStr{{+w}}}}:\n      Some{{{n}.of_word(w)}}\n    case _:\n      None{{}}\n").unwrap();
        writeln!(s, "law {n}.round:\n  for e: S.{n}\n  {{{n}.of_word({n}.word(e)) == e : S.{n}}}\n").unwrap();
        writeln!(s, "def {n}.round(e):\n  match e:").unwrap();
        for v in e.variants {
            writeln!(s, "    case S.{}{{}}:\n      {{==}}", v.name).unwrap();
        }
        writeln!(s, "\nlaw {n}.opt_round:\n  for m: Maybe<&2, S.{n}>\n  {{{n}.opt_of(Some{{{n}.opt_json(m)}}) == m : Maybe<&2, S.{n}>}}\n").unwrap();
        writeln!(s, "def {n}.opt_round(m):\n  match m:\n    case None{{}}:\n      {{==}}").unwrap();
        for v in e.variants {
            writeln!(s, "    case Some{{S.{}{{}}}}:\n      {{==}}", v.name).unwrap();
        }
        writeln!(s).unwrap();
        // as a journal row's value (a Rec codec row: `SC.<E>.journal`, and
        // `SC.<E>.opt_journal` for a Maybe of it): a string, else the fallback / none
        writeln!(s, "def {n}.journal_of(j: J.Json) -> S.{n}:\n  match j:\n    case J.JStr{{+w}}:\n      {n}.of_word(w)\n    case _:\n      S.{}{{}}\n", fallback.name).unwrap();
        writeln!(s, "def {n}.journal_json(e: S.{n}) -> J.Json:\n  J.JStr{{{n}.word(e)}}\n").unwrap();
        writeln!(s, "law {n}.journal_round:\n  for e: S.{n}\n  {{{n}.journal_of({n}.journal_json(e)) == e : S.{n}}}\n").unwrap();
        writeln!(s, "def {n}.journal_round(e):\n  {n}.round(e)\n").unwrap();
        writeln!(s, "def {n}.opt_journal_of(+j: J.Json) -> Maybe<&2, S.{n}>:\n  {n}.opt_of(Some{{j}})\n").unwrap();
        writeln!(s, "def {n}.opt_journal_json(m: Maybe<&2, S.{n}>) -> J.Json:\n  {n}.opt_json(m)\n").unwrap();
        writeln!(s, "law {n}.opt_journal_round:\n  for m: Maybe<&2, S.{n}>\n  {{{n}.opt_journal_of({n}.opt_journal_json(m)) == m : Maybe<&2, S.{n}>}}\n").unwrap();
        writeln!(s, "def {n}.opt_journal_round(m):\n  {n}.opt_round(m)\n").unwrap();
        return;
    }
    if e.tag_key.is_empty() {
        // a hand codec's (a timer's schedule): state-hand.bend
        return;
    }
    let tk = e.tag_key;
    writeln!(s, "# ---- {n} (its word under \"{tk}\", its fields beside it) ----\n").unwrap();
    writeln!(s, "def {n}.journal_kvs(e: S.{n}) -> List<&2, {KV}>:\n  match e:").unwrap();
    for v in e.variants {
        let pats: Vec<String> = v.fields.iter().map(|f| format!("+{}", var(f.name))).collect();
        let mut kvs = vec![format!("J.kv(\"{tk}\", J.JStr{{\"{}\"}})", v.word)];
        for f in v.fields {
            kvs.push(format!("J.kv(\"{}\", {})", f.name, scalar(f.ty, &var(f.name)).0));
        }
        writeln!(s, "    case {}:\n      [{}]", rec(S, v.name, &pats), kvs.join(", ")).unwrap();
    }
    writeln!(s, "\ndef {n}.journal_json(e: S.{n}) -> J.Json:\n  J.obj({n}.journal_kvs(e))\n").unwrap();
    let read = |v: &st::Variant| {
        let rs: Vec<String> = v.fields.iter().map(|f| format!("{}(J.get(j, \"{}\"))", scalar(f.ty, "").1, f.name)).collect();
        rec(S, v.name, &rs)
    };
    let mut chain = read(fallback);
    for v in others.iter().rev() {
        chain = format!("Bool.pick(S.{n}, String.eq(w, \"{}\"), {}, {chain})", v.word, read(v));
    }
    let j = if e.variants.iter().any(|v| !v.fields.is_empty()) { "+j" } else { "j" };
    writeln!(s, "def {n}.of_word(+w: String, {j}: J.Json) -> S.{n}:\n  {chain}\n").unwrap();
    writeln!(s, "# any other word, or none: {} (an inline row reads the line's own object)\ndef {n}.journal_of(+j: J.Json) -> S.{n}:\n  {n}.of_word(rd.s(J.get(j, \"{tk}\")), j)\n", fallback.word).unwrap();
    writeln!(s, "law {n}.journal_round:\n  for e: S.{n}\n  {{{n}.journal_of({n}.journal_json(e)) == e : S.{n}}}\n").unwrap();
    writeln!(s, "def {n}.journal_round(e):\n  match e:").unwrap();
    for v in e.variants {
        writeln!(s, "    case {}:\n      {{==}}", variant_pat(v, "x")).unwrap();
    }
    writeln!(s).unwrap();
}

/// How a key's segment of the key-value list is: always one key-value
/// (`Lit`), or its presence or its key known only from its value.
fn stuck(w: Wire) -> bool {
    matches!(w, Wire::StrOmit | Wire::OptStrOmit | Wire::OptNatOmit | Wire::Flag | Wire::Hand(_) | Wire::Inline)
}

/// A record's shape of the same name as `shape`, else its journal's.
fn inner_shape(r: &st::Record, shape: &str) -> &'static st::Shape {
    let r = STATE.record(r.name).unwrap();
    r.shape(shape).or_else(|| r.shape("journal")).unwrap_or_else(|| panic!("{}: no {shape} or journal shape", r.name))
}

/// One key's key-value(s) for the value `v`: a literal key-value, or the
/// list expression of a stuck one.
fn key_kv(k: &st::Key, f: &st::Field, shape: &str, v: &str) -> String {
    let key = k.key;
    match k.wire {
        Wire::Str => format!("J.kv(\"{key}\", J.JStr{{{v}}})"),
        Wire::Nat => format!("J.kv(\"{key}\", J.JNum{{{v}}})"),
        Wire::Bool => format!("J.kv(\"{key}\", J.JBool{{{v}}})"),
        Wire::OptStr => format!("J.kv(\"{key}\", K.jmaybe_s({v}))"),
        Wire::OptNat => format!("J.kv(\"{key}\", K.jmaybe_n({v}))"),
        Wire::Json => format!("J.kv(\"{key}\", {v})"),
        Wire::Strs => format!("J.kv(\"{key}\", J.JArr{{K.jstrs({v})}})"),
        Wire::Nats => format!("J.kv(\"{key}\", J.JArr{{K.jnats({v})}})"),
        Wire::Word => format!("J.kv(\"{key}\", J.JStr{{{}.word({v})}})", named(f.ty)),
        Wire::OptWord => format!("J.kv(\"{key}\", {}.opt_json({v}))", named(f.ty)),
        Wire::Obj => match STATE.record(named(f.ty)) {
            Some(r) => format!("J.kv(\"{key}\", {}.{}_json({v}))", r.name, inner_shape(r, shape).name),
            None => format!("J.kv(\"{key}\", {}.journal_json({v}))", named(f.ty)),
        },
        Wire::StrOmit => format!("K.omit_s(\"{key}\", {v})"),
        Wire::OptStrOmit => format!("K.omit_ms(\"{key}\", {v})"),
        Wire::OptNatOmit => format!("K.omit_mn(\"{key}\", {v})"),
        Wire::Flag => format!("K.flag(\"{key}\", {v})"),
        Wire::Hand(h) => format!("[SH.{}_kv({v})]", h.name),
        Wire::Inline => match STATE.record(named(f.ty)) {
            Some(r) => format!("{}.{}_kvs({v})", r.name, inner_shape(r, shape).name),
            None => format!("{}.journal_kvs({v})", named(f.ty)),
        },
    }
}

/// The segments of a shape's key-value list: runs of literal keys, each
/// stuck key alone. Each: its keys (indexes into `shape.keys`) and
/// whether it is stuck.
fn segments(sh: &st::Shape) -> Vec<(Vec<usize>, bool)> {
    let mut out: Vec<(Vec<usize>, bool)> = Vec::new();
    for (i, k) in sh.keys.iter().enumerate() {
        match (stuck(k.wire), out.last_mut()) {
            (false, Some((run, false))) => run.push(i),
            (st, _) => out.push((vec![i], st)),
        }
    }
    out
}

/// A segment's list expression, each field's value from `val`.
fn seg_expr(r: &st::Record, sh: &st::Shape, seg: &(Vec<usize>, bool), val: &dyn Fn(&str) -> String) -> String {
    let kvs: Vec<String> = seg.0.iter().map(|&i| {
        let k = &sh.keys[i];
        key_kv(k, r.field(k.field), sh.name, &val(k.field))
    }).collect();
    if seg.1 {
        kvs.into_iter().next().unwrap()
    } else {
        format!("[{}]", kvs.join(", "))
    }
}

/// A shape's whole key-value list.
fn kvs_expr(r: &st::Record, sh: &st::Shape, val: &dyn Fn(&str) -> String) -> String {
    let segs = segments(sh);
    let exprs: Vec<String> = segs.iter().map(|g| seg_expr(r, sh, g, val)).collect();
    if segs.iter().all(|g| !g.1) {
        // one literal list
        return exprs.into_iter().next().unwrap_or_else(|| "Nil{}".into());
    }
    format!("K.cat([{}])", exprs.join(", "))
}

/// The read of a key's field from the lookups `look(key)`.
fn key_read(k: &st::Key, f: &st::Field, shape: &str, look: &dyn Fn(&str) -> String) -> String {
    if let Some(h) = k.read {
        let ms: Vec<String> = h.keys.iter().map(|x| look(x)).collect();
        return format!("SH.{}({})", h.name, ms.join(", "));
    }
    let m = look(k.key);
    match k.wire {
        Wire::Str | Wire::StrOmit => format!("rd.s({m})"),
        Wire::Nat => format!("rd.n({m})"),
        Wire::Bool | Wire::Flag => format!("rd.b({m})"),
        Wire::OptStr | Wire::OptStrOmit => format!("K.opt_s({m})"),
        Wire::OptNat | Wire::OptNatOmit => format!("K.opt_n({m})"),
        Wire::Json => format!("rd.j({m})"),
        Wire::Strs => format!("rd.ss({m})"),
        Wire::Nats => format!("rd.ns({m})"),
        Wire::Word => format!("{}.of_word(rd.s({m}))", named(f.ty)),
        Wire::OptWord => format!("{}.opt_of({m})", named(f.ty)),
        Wire::Obj => match STATE.record(named(f.ty)) {
            Some(r) => {
                let sh = inner_shape(r, shape);
                assert!(sh.read && hand_of(sh).is_none(), "{}.{}: a nested record read whole", r.name, sh.name);
                format!("{}.{}_of(rd.j({m}))", r.name, sh.name)
            }
            None => format!("{}.journal_of(rd.j({m}))", named(f.ty)),
        },
        Wire::Hand(h) => {
            let ms: Vec<String> = h.keys.iter().map(|(_, x)| look(x)).collect();
            format!("SH.{}_of({})", h.name, ms.join(", "))
        }
        Wire::Inline => panic!("{}: an inline field is written only", f.name),
    }
}

/// A readable shape's hand-coded key (at most one): its read is a Maybe.
fn hand_of(sh: &st::Shape) -> Option<(usize, &'static st::Hand)> {
    let hs: Vec<(usize, &'static st::Hand)> = sh.keys.iter().enumerate().filter_map(|(i, k)| match k.wire {
        Wire::Hand(h) => Some((i, h)),
        _ => None,
    }).collect();
    assert!(hs.len() <= 1, "{}: one hand key at most in a read shape", sh.name);
    hs.into_iter().next()
}

/// The keys a key's read looks up.
fn looked_up(k: &st::Key) -> Vec<&'static str> {
    if let Some(h) = k.read {
        return h.keys.to_vec();
    }
    match k.wire {
        Wire::Hand(h) => h.keys.iter().map(|(_, x)| *x).collect(),
        _ => vec![k.key],
    }
}

/// The cases a field's proof splits on (pattern, its value as read back),
/// one case (the variable itself) when it doesn't split.
fn split(k: &st::Key, f: &st::Field, v: &str) -> Vec<(String, String)> {
    let one = |p: &str| (p.to_string(), p.to_string());
    match k.wire {
        Wire::StrOmit => vec![one("SNil{}"), one(&format!("SCon{{{v}_h, {v}_t}}"))],
        Wire::OptStr | Wire::OptStrOmit | Wire::OptNat | Wire::OptNatOmit => vec![one("None{}"), one(&format!("Some{{{v}_x}}"))],
        Wire::Flag => vec![one("True{}"), one("False{}")],
        Wire::Word => the_enum(named(f.ty)).variants.iter().map(|x| one(&format!("S.{}{{}}", x.name))).collect(),
        Wire::OptWord => {
            let mut cs = vec![one("None{}")];
            cs.extend(the_enum(named(f.ty)).variants.iter().map(|x| one(&format!("Some{{S.{}{{}}}}", x.name))));
            cs
        }
        Wire::Hand(h) => the_enum(named(f.ty)).variants.iter().map(|x| {
            let p = variant_pat(x, v);
            (p.clone(), format!("Some{{SH.{}_norm({p})}}", h.name))
        }).collect(),
        _ => vec![(v.to_string(), v.to_string())],
    }
}

/// Whether a stuck key, its value the case pattern `p` (or a variable),
/// writes `look` in its segment: Some(true) it does, Some(false) it
/// writes nothing under it, None unknown (a variable: the segment stays
/// stuck and needs its skip lemma).
fn writes(k: &st::Key, p: Option<&str>, look: &str) -> Option<bool> {
    let p = p?;
    Some(match k.wire {
        Wire::StrOmit => p.starts_with("SCon") && k.key == look,
        Wire::OptStrOmit | Wire::OptNatOmit => p.starts_with("Some") && k.key == look,
        Wire::Flag => p == "True{}" && k.key == look,
        Wire::Hand(h) => h.keys.iter().any(|(vn, key)| p.starts_with(&format!("S.{vn}{{")) && *key == look),
        _ => k.key == look,
    })
}

/// The skip lemma of a stuck key whose value is `v`: past its segment,
/// a lookup of `look` reads what the rest of the list reads.
fn skip_call(k: &st::Key, v: &str, rest: &str, look: &str) -> String {
    match k.wire {
        Wire::StrOmit => format!("skip_s(\"{}\", {v}, {rest}, \"{look}\", {{==}})", k.key),
        Wire::OptStrOmit => format!("skip_ms(\"{}\", {v}, {rest}, \"{look}\", {{==}})", k.key),
        Wire::OptNatOmit => format!("skip_mn(\"{}\", {v}, {rest}, \"{look}\", {{==}})", k.key),
        Wire::Flag => format!("skip_flag(\"{}\", {v}, {rest}, \"{look}\", {{==}})", k.key),
        Wire::Hand(h) => {
            let ds: Vec<&str> = h.keys.iter().map(|_| "{==}").collect();
            format!("skip.{}({v}, {rest}, \"{look}\", {})", h.name, ds.join(", "))
        }
        w => panic!("{}: {w:?} is never stuck in a read shape", k.field),
    }
}

/// The value of a list field read back, when it needs its kit lemma.
fn list_lemma(w: Wire, v: &str) -> Option<String> {
    match w {
        Wire::Strs => Some(format!("K.strs_round({v})")),
        Wire::Nats => Some(format!("K.nats_round({v})")),
        _ => None,
    }
}

/// One readable shape's per-field laws and its record law.
fn shape_laws(s: &mut String, r: &st::Record, sh: &st::Shape) {
    let n = r.name;
    let shn = sh.name;
    let vars: Vec<String> = r.fields.iter().map(|f| var(f.name)).collect();
    let fors: String = r.fields.iter().map(|f| format!("  for +{}: {}\n", var(f.name), ty(f.ty, S))).collect();
    let segs = segments(sh);
    let hand = hand_of(sh);
    let field_var = |name: &str| var(name);
    for (ki, k) in sh.keys.iter().enumerate() {
        let f = r.field(k.field);
        let v = field_var(k.field);
        let fty = if matches!(k.wire, Wire::Hand(_)) { format!("Maybe<&2, {}>", ty(f.ty, S)) } else { ty(f.ty, S) };
        // the law, over the record's variables
        let obj = format!("J.obj({n}.{shn}_kvs({}))", rec(S, n, &vars));
        let read0 = key_read(k, f, shn, &|key| format!("J.get({obj}, \"{key}\")"));
        let want0 = match k.wire {
            Wire::Hand(h) => format!("Some{{SH.{}_norm({v})}}", h.name),
            _ => v.clone(),
        };
        let law = format!("{n}.{shn}.{}", k.field);
        writeln!(s, "law {law}:\n{fors}  {{{read0} == {want0} : {fty}}}\n").unwrap();
        writeln!(s, "def {law}({}):", vars.join(", ")).unwrap();
        let cases = split(k, f, &v);
        let ind = if cases.len() > 1 { "      " } else { "  " };
        if cases.len() > 1 {
            writeln!(s, "  match {v}:").unwrap();
        }
        for (pat, want) in &cases {
            if cases.len() > 1 {
                writeln!(s, "    case {pat}:").unwrap();
            }
            // each field's value in this case
            let val = |name: &str| if name == k.field { pat.clone() } else { field_var(name) };
            let pat_of = |key: &st::Key| if key.field == k.field && cases.len() > 1 { Some(pat.as_str()) } else { None };
            let obj = format!("J.obj({n}.{shn}_kvs({}))", rec(S, n, &r.fields.iter().map(|g| val(g.name)).collect::<Vec<_>>()));
            // each lookup's current term (rewritten as its skips go)
            let looks = looked_up(k);
            let mut cur: Vec<String> = looks.iter().map(|key| format!("J.get({obj}, \"{key}\")")).collect();
            for (li, look) in looks.iter().enumerate() {
                for (gi, g) in segs.iter().enumerate() {
                    let gk = &sh.keys[g.0[0]];
                    if g.1 {
                        match writes(gk, pat_of(gk), look) {
                            Some(true) => break,
                            Some(false) => continue,
                            None => {
                                let rest: Vec<String> = segs[gi + 1..].iter().map(|x| seg_expr(r, sh, x, &val)).collect();
                                let rest = format!("[{}]", rest.join(", "));
                                let mut holed = cur.clone();
                                holed[li] = "_".into();
                                let motive = key_read(k, f, shn, &|key| holed[looks.iter().position(|x| *x == key).unwrap()].clone());
                                let call = skip_call(gk, &field_var(gk.field), &rest, look);
                                writeln!(s, "{ind}%{call} : {{{motive} == {want} : {fty}}}").unwrap();
                                cur[li] = format!("J.get_kv(K.cat({rest}), \"{look}\")");
                            }
                        }
                    } else if g.0.iter().any(|&i| sh.keys[i].key == *look) {
                        break;
                    }
                }
            }
            match (list_lemma(k.wire, &v), &k.wire) {
                (Some(l), _) => writeln!(s, "{ind}{l}").unwrap(),
                (None, Wire::Obj) if STATE.record(named(f.ty)).is_none() => writeln!(s, "{ind}{}.journal_round({v})", named(f.ty)).unwrap(),
                _ => writeln!(s, "{ind}{{==}}").unwrap(),
            }
        }
        writeln!(s).unwrap();
        let _ = ki;
    }
    // the record: each carried field rewritten once by its law
    let carried = |name: &str| sh.keys.iter().find(|k| k.field == name);
    let want: Vec<String> = r.fields.iter().map(|f| match carried(f.name) {
        Some(k) => match k.wire {
            Wire::Hand(h) => format!("SH.{}_norm({})", h.name, var(f.name)),
            _ => var(f.name),
        },
        None => dflt(f, S),
    }).collect();
    let rty = if hand.is_some() { format!("Maybe<&2, S.{n}>") } else { format!("S.{n}") };
    let wrap = |x: String| if hand.is_some() { format!("Some{{{x}}}") } else { x };
    let input = rec(S, n, &vars);
    let law = format!("{n}.{shn}_round_by_fields");
    let what = if hand.is_some() { "its hand key read back normalised, " } else { "" };
    writeln!(s, "# the fields it carries come back ({what}the others at their default)").unwrap();
    writeln!(s, "law {law}:\n{fors}  {{{n}.{shn}_of({n}.{shn}_json({input})) == {} : {rty}}}\n", wrap(rec(S, n, &want))).unwrap();
    writeln!(s, "def {law}({}):", vars.join(", ")).unwrap();
    let obj = format!("J.obj({n}.{shn}_kvs({input}))");
    let look = |key: &str| format!("J.get({obj}, \"{key}\")");
    let reads: Vec<String> = r.fields.iter().map(|f| match carried(f.name) {
        Some(k) if !matches!(k.wire, Wire::Hand(_)) => key_read(k, f, shn, &look),
        Some(k) => format!("SH.{}_norm({})", match k.wire { Wire::Hand(h) => h.name, _ => unreachable!() }, var(f.name)),
        None => dflt(f, S),
    }).collect();
    let mut now = reads.clone();
    if let Some((hi, h)) = hand {
        let k = &sh.keys[hi];
        let f = r.field(k.field);
        let read = key_read(k, f, shn, &look);
        let hty = format!("Maybe<&2, {}>", ty(f.ty, S));
        let v = var(f.name);
        writeln!(s, "  %Equal.sym({hty}, {read}, Some{{SH.{}_norm({v})}}, {n}.{shn}.{}({})) : {{{n}.{shn}_with(_, {obj}) == {} : {rty}}}",
            h.name, k.field, vars.join(", "), wrap(rec(S, n, &want))).unwrap();
    }
    for k in sh.keys {
        if matches!(k.wire, Wire::Hand(_)) {
            continue;
        }
        let i = r.fields.iter().position(|f| f.name == k.field).unwrap();
        let f = &r.fields[i];
        let mut holed = now.clone();
        holed[i] = "_".into();
        writeln!(s, "  %Equal.sym({}, {}, {}, {n}.{shn}.{}({})) : {{{} == {} : {rty}}}",
            ty(f.ty, S), reads[i], var(f.name), k.field, vars.join(", "), wrap(rec(S, n, &holed)), wrap(rec(S, n, &want))).unwrap();
        now[i] = var(f.name);
    }
    writeln!(s, "  {{==}}\n").unwrap();
    // a shape that carries every field, read whole: the law over the
    // record (what gen/journal-laws.bend's rows call)
    if hand.is_none() && r.fields.iter().all(|f| carried(f.name).is_some()) {
        writeln!(s, "law {n}.{shn}_round:\n  for x: S.{n}\n  {{{n}.{shn}_of({n}.{shn}_json(x)) == x : S.{n}}}\n").unwrap();
        writeln!(s, "def {n}.{shn}_round(x):\n  match x:\n    case {}:\n      {law}({})\n", rec(S, n, &vars), vars.join(", ")).unwrap();
    }
}

fn shape_codec(s: &mut String, r: &st::Record, sh: &st::Shape) {
    let n = r.name;
    let shn = sh.name;
    let pats: Vec<String> = r.fields.iter().map(|f| {
        if sh.keys.iter().any(|k| k.field == f.name) { format!("+{}", var(f.name)) } else { "_".into() }
    }).collect();
    let words: Vec<&str> = sh.keys.iter().map(|k| k.key).collect();
    writeln!(s, "# ---- {n}, its {shn} shape: {} ----\n", words.join(", ")).unwrap();
    writeln!(s, "def {n}.{shn}_kvs(r: S.{n}) -> List<&2, {KV}>:\n  match r:\n    case {}:\n      {}\n", rec(S, n, &pats), kvs_expr(r, sh, &|name| var(name))).unwrap();
    writeln!(s, "def {n}.{shn}_json(r: S.{n}) -> J.Json:\n  J.obj({n}.{shn}_kvs(r))\n").unwrap();
    if !sh.read {
        return;
    }
    let carried = |name: &str| sh.keys.iter().find(|k| k.field == name);
    let look = |key: &str| format!("J.get(j, \"{key}\")");
    match hand_of(sh) {
        None => {
            let reads: Vec<String> = r.fields.iter().map(|f| match carried(f.name) {
                Some(k) => key_read(k, f, shn, &look),
                None => dflt(f, S),
            }).collect();
            writeln!(s, "def {n}.{shn}_of(+j: J.Json) -> S.{n}:\n  {}\n", rec(S, n, &reads)).unwrap();
        }
        Some((hi, h)) => {
            let hk = &sh.keys[hi];
            let reads: Vec<String> = r.fields.iter().map(|f| match carried(f.name) {
                Some(k) if k.field == hk.field => "h".into(),
                Some(k) => key_read(k, f, shn, &look),
                None => dflt(f, S),
            }).collect();
            let hty = ty(r.field(hk.field).ty, S);
            writeln!(s, "def {n}.{shn}_with(m: Maybe<&2, {hty}>, +j: J.Json) -> Maybe<&2, S.{n}>:\n  match m:\n    case None{{}}:\n      None{{}}\n    case Some{{+h}}:\n      Some{{{}}}\n", rec(S, n, &reads)).unwrap();
            let ms: Vec<String> = h.keys.iter().map(|(_, key)| look(key)).collect();
            writeln!(s, "# none without its {} (the line is skipped)", h.name).unwrap();
            writeln!(s, "def {n}.{shn}_of(+j: J.Json) -> Maybe<&2, S.{n}>:\n  {n}.{shn}_with(SH.{}_of({}), j)\n", h.name, ms.join(", ")).unwrap();
        }
    }
    shape_laws(s, r, sh);
}

/// A fixture object as the record its view shape writes, and the JSON
/// term it must write: the shape pinned key for key.
fn pinned(r: &st::Record, sh: &st::Shape, o: &Jv) -> String {
    let lit = |f: &st::Field, k: Option<&st::Key>| -> String {
        let Some(k) = k else { return dflt(f, S) };
        if let Wire::Hand(h) = k.wire {
            let e = the_enum(named(f.ty));
            for (vn, key) in h.keys {
                if let Some(Jv::Num(x)) = o.get(key) {
                    let v = e.variants.iter().find(|v| v.name == *vn).unwrap();
                    assert_eq!(v.fields.len(), 1, "{vn}: a hand variant holds one number");
                    return format!("S.{vn}{{{x}n}}");
                }
            }
            panic!("{}: a fixture object without its {}", r.name, h.name);
        }
        if let Wire::Inline = k.wire {
            let inner = STATE.record(named(f.ty)).expect("an inline record");
            return pinned_rec(inner, inner_shape(inner, sh.name), o);
        }
        let got = o.get(k.key);
        match (f.ty, got) {
            (Ty::Nat, Some(Jv::Num(x))) => format!("{x}n"),
            (Ty::Str, Some(Jv::Str(x))) => format!("\"{x}\""),
            (Ty::Bool, Some(Jv::Bool(b))) => if *b { "True{}".into() } else { "False{}".into() },
            (Ty::OptNat, Some(Jv::Num(x))) => format!("Some{{{x}n}}"),
            (Ty::OptStr, Some(Jv::Str(x))) => format!("Some{{\"{x}\"}}"),
            (Ty::OptNat | Ty::OptStr, Some(Jv::Null)) => "None{}".into(),
            (Ty::Json, Some(v)) => bend_json(v),
            (Ty::Nats | Ty::Strs, Some(Jv::Arr(xs))) if xs.is_empty() => "Nil{}".into(),
            (Ty::Nats, Some(Jv::Arr(xs))) => format!("[{}]", xs.iter().map(|x| match x { Jv::Num(n) => format!("{n}n"), _ => panic!("a number list") }).collect::<Vec<_>>().join(", ")),
            (Ty::Strs, Some(Jv::Arr(xs))) => format!("[{}]", xs.iter().map(|x| match x { Jv::Str(t) => format!("\"{t}\""), _ => panic!("a string list") }).collect::<Vec<_>>().join(", ")),
            (_, None) => dflt(f, S),
            (t, v) => panic!("{}.{}: fixture value {v:?} for {t:?}", r.name, f.name),
        }
    };
    let args: Vec<String> = r.fields.iter().map(|f| lit(f, sh.keys.iter().find(|k| k.field == f.name))).collect();
    rec(S, r.name, &args)
}

fn pinned_rec(r: &st::Record, sh: &st::Shape, o: &Jv) -> String {
    pinned(r, sh, o)
}

/// The view shapes pinned on fixtures/core_view.jsonl: the view's key and
/// the record its items are (its view shape, else its journal shape).
const PINS: &[(&str, &str)] = &[("timers", "Timer"), ("timers_ended", "TimerEnd"), ("cards", "Card"), ("wakes", "Watch"), ("wakes_ended", "WatchEnd")];

fn view_pins(s: &mut String) {
    let text = std::fs::read_to_string(root().join("rust/proto/fixtures/core_view.jsonl")).unwrap();
    writeln!(s, "# ---- the view shapes, pinned on fixtures/core_view.jsonl (a key order change is red) ----\n").unwrap();
    for (key, rname) in PINS {
        let r = STATE.record(rname).unwrap();
        // a record whose view is its journal shape (a watch) is pinned on that
        let sh = inner_shape(r, "view");
        // the first item whose numbers are Bend Nat literals
        let item = text
            .lines()
            .filter(|l| l.starts_with('{'))
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter_map(|v| v.get(*key).and_then(|x| x.as_array()).and_then(|a| a.first().cloned()))
            .find(small)
            .unwrap_or_else(|| panic!("core_view.jsonl: no {key} item to pin {rname}'s view on"));
        let o = jv(&ordered(&text, key, &item));
        let law = format!("{rname}.view_pinned");
        writeln!(s, "law {law}:\n  {{{rname}.{}_json({}) == {} : J.Json}}\n", sh.name, pinned(r, sh, &o), bend_json(&o)).unwrap();
        writeln!(s, "def {law}():\n  {{==}}\n").unwrap();
    }
}

/// Every number of a JSON value is a Bend Nat literal.
fn small(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Number(n) => n.as_u64().is_some_and(|x| x <= u32::MAX as u64),
        serde_json::Value::Array(xs) => xs.iter().all(small),
        serde_json::Value::Object(m) => m.values().all(small),
        serde_json::Value::String(t) => !t.contains(['"', '\\', '\n']),
        _ => true,
    }
}

/// The item's text as the fixture line has it (serde_json's map sorts
/// its keys; the pin is on the line's own order): the object of the
/// line's `key` array that equals `item`.
fn ordered(text: &str, key: &str, item: &serde_json::Value) -> String {
    for l in text.lines().filter(|l| l.starts_with('{')) {
        let Some(at) = l.find(&format!("\"{key}\":[")) else { continue };
        let rest = &l[at + key.len() + 4..];
        let mut depth = 0;
        let mut start = None;
        for (i, c) in rest.char_indices() {
            match c {
                '{' => {
                    if depth == 0 {
                        start = Some(i);
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let obj = &rest[start.unwrap()..=i];
                        if serde_json::from_str::<serde_json::Value>(obj).ok().as_ref() == Some(item) {
                            return obj.to_string();
                        }
                    }
                }
                ']' if depth == 0 => break,
                _ => {}
            }
        }
    }
    panic!("core_view.jsonl: {key}'s item not found as text")
}

const CODEC_KIT: &str = r##"
# ---- readers: a looked-up value, the default when missing or of another
# type (today's C.get_*) ----

def rd.s(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{J.JStr{+s}}:
      s
    case _:
      ""

def rd.n(m: Maybe<&2, J.Json>) -> Nat:
  match m:
    case Some{J.JNum{+n}}:
      n
    case _:
      0n

def rd.b(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{J.JBool{+b}}:
      b
    case _:
      False{}

def rd.j(m: Maybe<&2, J.Json>) -> J.Json:
  match m:
    case Some{+j}:
      j
    case None{}:
      J.JNull{}

def rd.ss(m: Maybe<&2, J.Json>) -> List<&2, String>:
  K.strs(K.arr(m))

def rd.ns(m: Maybe<&2, J.Json>) -> List<&2, Nat>:
  K.nats(K.arr(m))

# ---- skips: a key looked up past a segment that doesn't write it reads
# what the rest reads (the round laws rewrite with them, one per stuck
# segment: no case split over the record) ----

def kv_key(p: Sigma<&2, &2, String, _ => J.Json>) -> String:
  match p:
    case Tuple{+k, _}:
      k

law skip_one:
  for p: Sigma<&2, &2, String, _ => J.Json>
  for rest: List<&2, List<&2, Sigma<&2, &2, String, _ => J.Json>>>
  for key: String
  for d: {String.eq(kv_key(p), key) == False{} : Bool}
  {J.get_kv(K.cat(rest), key) == J.get_kv(K.cat([p] <> rest), key) : Maybe<&2, J.Json>}

def skip_one(p, rest, key, d):
  match p:
    case Tuple{k, v}:
      %Equal.sym(Bool, String.eq(k, key), False{}, d) : {J.get_kv(K.cat(rest), key) == Bool.pick(Maybe<&2, J.Json>, _, Some{v}, J.get_kv(K.cat(rest), key)) : Maybe<&2, J.Json>}
      {==}

law skip_s:
  for k: String
  for s: String
  for rest: List<&2, List<&2, Sigma<&2, &2, String, _ => J.Json>>>
  for key: String
  for d: {String.eq(k, key) == False{} : Bool}
  {J.get_kv(K.cat(rest), key) == J.get_kv(K.cat(K.omit_s(k, s) <> rest), key) : Maybe<&2, J.Json>}

def skip_s(k, s, rest, key, d):
  match s:
    case SNil{}:
      {==}
    case SCon{h, t}:
      skip_one(J.kv(k, J.JStr{SCon{h, t}}), rest, key, d)

law skip_ms:
  for k: String
  for m: Maybe<&2, String>
  for rest: List<&2, List<&2, Sigma<&2, &2, String, _ => J.Json>>>
  for key: String
  for d: {String.eq(k, key) == False{} : Bool}
  {J.get_kv(K.cat(rest), key) == J.get_kv(K.cat(K.omit_ms(k, m) <> rest), key) : Maybe<&2, J.Json>}

def skip_ms(k, m, rest, key, d):
  match m:
    case None{}:
      {==}
    case Some{x}:
      skip_one(J.kv(k, J.JStr{x}), rest, key, d)

law skip_mn:
  for k: String
  for m: Maybe<&2, Nat>
  for rest: List<&2, List<&2, Sigma<&2, &2, String, _ => J.Json>>>
  for key: String
  for d: {String.eq(k, key) == False{} : Bool}
  {J.get_kv(K.cat(rest), key) == J.get_kv(K.cat(K.omit_mn(k, m) <> rest), key) : Maybe<&2, J.Json>}

def skip_mn(k, m, rest, key, d):
  match m:
    case None{}:
      {==}
    case Some{x}:
      skip_one(J.kv(k, J.JNum{x}), rest, key, d)

law skip_flag:
  for k: String
  for b: Bool
  for rest: List<&2, List<&2, Sigma<&2, &2, String, _ => J.Json>>>
  for key: String
  for d: {String.eq(k, key) == False{} : Bool}
  {J.get_kv(K.cat(rest), key) == J.get_kv(K.cat(K.flag(k, b) <> rest), key) : Maybe<&2, J.Json>}

def skip_flag(k, b, rest, key, d):
  match b:
    case True{}:
      skip_one(J.kv(k, J.JBool{True{}}), rest, key, d)
    case False{}:
      {==}
"##;

/// A hand codec's skip: past its one key-value, whose key its variant
/// says, a lookup of another key reads what the rest reads.
fn hand_skip(s: &mut String, h: &st::Hand, e: &st::Enum) {
    let n = h.name;
    let ds: Vec<String> = (0..h.keys.len()).map(|i| format!("d{i}")).collect();
    writeln!(s, "law skip.{n}:\n  for x: S.{}\n  for rest: List<&2, List<&2, {KV}>>\n  for key: String", e.name).unwrap();
    for (i, (_, key)) in h.keys.iter().enumerate() {
        writeln!(s, "  for d{i}: {{String.eq(\"{key}\", key) == False{{}} : Bool}}").unwrap();
    }
    writeln!(s, "  {{J.get_kv(K.cat(rest), key) == J.get_kv(K.cat([SH.{n}_kv(x)] <> rest), key) : Maybe<&2, J.Json>}}\n").unwrap();
    writeln!(s, "def skip.{n}(x, rest, key, {}):\n  match x:", ds.join(", ")).unwrap();
    for v in e.variants {
        let i = h.keys.iter().position(|(vn, _)| *vn == v.name).unwrap_or_else(|| panic!("{n}: no key for {}", v.name));
        let p = variant_pat(v, "x");
        writeln!(s, "    case {p}:\n      skip_one(SH.{n}_kv({p}), rest, key, d{i})").unwrap();
    }
    writeln!(s).unwrap();
}

pub(crate) fn state_codec() -> String {
    let mut s = String::new();
    header(&mut s, "state codec: bise_proto::core::state::STATE's JSON shapes, their reads and writes, each with its round law.");
    writeln!(s, "\nimport ./kit.bend as K\n\nimport ./state.bend as S\n\nimport ../state-hand.bend as SH").unwrap();
    s.push_str(CODEC_KIT);
    writeln!(s).unwrap();
    // the hand codecs' skips, once each
    let mut hands: Vec<(&'static st::Hand, &'static st::Enum)> = Vec::new();
    for r in STATE.records {
        for sh in r.shapes {
            for k in sh.keys {
                if let Wire::Hand(h) = k.wire {
                    if !hands.iter().any(|(x, _)| x.name == h.name) {
                        hands.push((h, the_enum(named(r.field(k.field).ty))));
                    }
                }
            }
        }
    }
    for (h, e) in hands {
        hand_skip(&mut s, h, e);
    }
    for e in STATE.enums {
        enum_codec(&mut s, e);
    }
    for r in STATE.records {
        for sh in r.shapes {
            shape_codec(&mut s, r, sh);
        }
    }
    view_pins(&mut s);
    s
}
