//! The photo's codegen helpers (issue 36): which shapes read back whole,
//! the lists of records a photo holds (`Wire::Recs`), and each list's
//! codec and round law. A part of core_gen.rs, used by core_gen/state.rs.

use super::state::{dflt, hand_of, inner_shape, named, slots, ty};
use super::*;
use bise_proto::core::state as st;
use bise_proto::core::state::{Ty, Wire, STATE};

/// A shape read back whole: every field carried, no hand key (its
/// `_round` law is over the record).
pub(crate) fn whole(r: &st::Record, sh: &st::Shape) -> bool {
    sh.read && hand_of(sh).is_none() && r.fields.iter().all(|f| sh.keys.iter().any(|k| k.field == f.name))
}

/// The (record, shape) pairs a `Wire::Recs` key writes a list of.
pub(crate) fn listed() -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for r in STATE.records {
        for sh in r.shapes {
            for k in sh.keys.iter().filter(|k| k.wire == Wire::Recs) {
                let inner = STATE.record(named(r.field(k.field).ty)).unwrap();
                let pair = (inner.name, inner_shape(inner, sh.name).name);
                if !out.contains(&pair) {
                    out.push(pair);
                }
            }
        }
    }
    out
}

/// A list of records in one shape: `T.<shape>_list` / `_list_of` and its
/// round law, by induction on the list with the record's `_round`.
pub(crate) fn list_codec(s: &mut String, n: &str, shn: &str) {
    let l = format!("{n}.{shn}_list");
    let lt = format!("List<&2, S.{n}>");
    writeln!(s, "# ---- a list of {n}, each in its {shn} shape ----
").unwrap();
    writeln!(s, "def {l}(xs: {lt}) -> List<&2, J.Json>:
  match xs:
    case Nil{{}}:
      Nil{{}}
    case Con{{+h, +t}}:
      {n}.{shn}_json(h) <> {l}(t)
").unwrap();
    writeln!(s, "def {l}_of.go(xs: List<&2, J.Json>) -> {lt}:
  match xs:
    case Nil{{}}:
      Nil{{}}
    case Con{{+h, +t}}:
      {n}.{shn}_of(h) <> {l}_of.go(t)
").unwrap();
    writeln!(s, "# an array's items; anything else: none
def {l}_of(+j: J.Json) -> {lt}:
  match j:
    case J.JArr{{+xs}}:
      {l}_of.go(xs)
    case _:
      Nil{{}}
").unwrap();
    writeln!(s, "law {l}_round.go:
  for xs: {lt}
  {{{l}_of.go({l}(xs)) == xs : {lt}}}
").unwrap();
    writeln!(s, "def {l}_round.go(xs):
  match xs:
    case Nil{{}}:
      {{==}}
    case Con{{h, t}}:
      %{n}.{shn}_round(h) : {{{n}.{shn}_of({n}.{shn}_json(h)) <> {l}_of.go({l}(t)) == _ <> t : {lt}}}
      %{l}_round.go(t) : {{{n}.{shn}_of({n}.{shn}_json(h)) <> {l}_of.go({l}(t)) == {n}.{shn}_of({n}.{shn}_json(h)) <> _ : {lt}}}
      {{==}}
").unwrap();
    writeln!(s, "law {l}_round:
  for xs: {lt}
  {{{l}_of(J.JArr{{{l}(xs)}}) == xs : {lt}}}
").unwrap();
    writeln!(s, "def {l}_round(xs):
  {l}_round.go(xs)
").unwrap();
}

/// A list field's item type, or None for a scalar.
fn item_ty(t: Ty) -> Option<String> {
    match t {
        Ty::Strs => Some("String".into()),
        Ty::Nats => Some("Nat".into()),
        Ty::Ts(n) => Some(format!("S.{n}")),
        _ => None,
    }
}

/// St in slices, the photo in lines (c6 c-2, architect m_56956): a
/// slice is an St holding the next k records, its lists' items in field
/// order. `St.slice_take(k, st)` holds the first k (and st's scalars),
/// `St.slice_drop(k, st)` the rest (and St.empty()'s scalars),
/// `St.slice_join(a, b)` puts a's items before b's (a's scalars), and
/// `St.slice_round` proves that a take joined to its drop is st. The
/// budget of a list is what the lists before it left of k
/// (`St.slice.left`), the same term in a take and its drop, so the law
/// rewrites once per list with `St.slice.take_drop`.
pub(crate) fn st_slices(s: &mut String) {
    let r = STATE.record("St").unwrap();
    let n = r.fields.len();
    let f = slots(n);
    let items: Vec<Option<String>> = r.fields.iter().map(|fl| item_ty(fl.ty)).collect();
    // the budget term of each list field, in field order
    let mut budget: Vec<Option<String>> = Vec::new();
    let mut k = "k".to_string();
    for (i, it) in items.iter().enumerate() {
        match it {
            Some(a) => {
                budget.push(Some(k.clone()));
                k = format!("St.slice.left({a}, {}, {k})", f[i]);
            }
            None => budget.push(None),
        }
    }
    let lists = |op: &str, scalar: &dyn Fn(usize) -> String| -> String {
        let args: Vec<String> = (0..n)
            .map(|i| match (&items[i], &budget[i]) {
                (Some(a), Some(b)) => format!("List.{op}(&2, {a}, {}, {b})", f[i]),
                _ => scalar(i),
            })
            .collect();
        format!("S.St{{{}}}", args.join(", "))
    };
    let pat = format!("S.St{{{}}}", f.iter().map(|x| format!("+{x}")).collect::<Vec<_>>().join(", "));
    let names: Vec<&str> = r.fields.iter().map(|fl| fl.name).collect();
    writeln!(s, "# ---- St in slices, the photo in lines: {} ----
", names.join(", ")).unwrap();
    writeln!(s, "# what k leaves after xs's first items (0n once they spend it)
def St.slice.left(-A: Data, xs: List<&2, A>, k: Nat) -> Nat:
  match xs:
    case Nil{{}}:
      k
    case Con{{h, t}}:
      match k:
        case 0n:
          0n
        case 1n+p:
          St.slice.left(A, t, p)

def St.slice.take_drop(-A: Data, +xs: List<&2, A>, +k: Nat)
    -> {{List.append(&2, A, List.take(&2, A, xs, k), List.drop(&2, A, xs, k)) == xs : List<&2, A>}}:
  match xs k:
    case Nil{{}} _:
      {{==}}
    case Con{{h, t}} 0n:
      {{==}}
    case Con{{h, t}} 1n+p:
      Equal.cong(List<&2, A>, List<&2, A>, y => h <> y, List.append(&2, A, List.take(&2, A, t, p), List.drop(&2, A, t, p)), t, St.slice.take_drop(A, t, p))

def St.slice.none(-A: Data, xs: List<&2, A>) -> Bool:
  match xs:
    case Nil{{}}:
      True{{}}
    case Con{{h, t}}:
      False{{}}
").unwrap();
    writeln!(s, "# the first k records of st's lists, in field order; st's scalars
def St.slice_take(+k: Nat, st: S.St) -> S.St:
  match st:
    case {pat}:
      {}
", lists("take", &|i| f[i].clone())).unwrap();
    let empty: Vec<String> = r.fields.iter().map(|fl| dflt(fl, "S.")).collect();
    writeln!(s, "# st's lists past their first k records; St.empty()'s scalars
def St.slice_drop(+k: Nat, st: S.St) -> S.St:
  match st:
    case {pat}:
      {}
", lists("drop", &|i| empty[i].clone())).unwrap();
    let a: Vec<String> = (0..n).map(|i| format!("a{i}")).collect();
    let b: Vec<String> = (0..n).map(|i| format!("b{i}")).collect();
    let joined: Vec<String> = (0..n)
        .map(|i| match &items[i] {
            Some(t) => format!("List.append(&2, {t}, {}, {})", a[i], b[i]),
            None => a[i].clone(),
        })
        .collect();
    writeln!(s, "# a's records before b's; a's scalars
def St.slice_join(a: S.St, b: S.St) -> S.St:
  match a b:
    case S.St{{{}}} S.St{{{}}}:
      S.St{{{}}}
", a.join(", "), b.join(", "), joined.join(", ")).unwrap();
    let none: Vec<String> = (0..n).filter_map(|i| items[i].as_ref().map(|t| format!("St.slice.none({t}, {})", f[i]))).collect();
    writeln!(s, "# no record left in st's lists
def St.slice_none(st: S.St) -> Bool:
  match st:
    case {pat}:
      {}
", none.join(" && ")).unwrap();
    // the law: one rewrite per list, its join back to the field
    let st_ty = |i: usize| ty(r.fields[i].ty, "S.");
    writeln!(s, "law St.slice_round:
  for +k: Nat
  for +st: S.St
  {{St.slice_join(St.slice_take(k, st), St.slice_drop(k, st)) == st : S.St}}

def St.slice_round(k, st):
  match st:
    case {pat}:").unwrap();
    let mut cur: Vec<String> = (0..n)
        .map(|i| match (&items[i], &budget[i]) {
            (Some(t), Some(b)) => format!("List.append(&2, {t}, List.take(&2, {t}, {x}, {b}), List.drop(&2, {t}, {x}, {b}))", x = f[i]),
            _ => f[i].clone(),
        })
        .collect();
    let whole = format!("S.St{{{}}}", f.join(", "));
    for i in 0..n {
        let (Some(t), Some(b)) = (&items[i], &budget[i]) else { continue };
        let mut p = cur.clone();
        p[i] = "_".into();
        writeln!(s, "      %Equal.sym({}, {}, {}, St.slice.take_drop({t}, {}, {b})) : {{S.St{{{}}} == {whole} : S.St}}",
            st_ty(i), cur[i], f[i], f[i], p.join(", ")).unwrap();
        cur[i] = f[i].clone();
    }
    writeln!(s, "      {{==}}
").unwrap();
}
