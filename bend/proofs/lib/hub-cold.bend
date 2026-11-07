# bend/proofs/lib/hub-cold.bend: the lemmas of the split messages (issue
# 21 steps 3-4: St.hot ids ascending, St.old ids descending, every = the
# hot ones put among the old ones, turned around). Used by the laws
# split_*, ids_ascending, cold_is_inert and the walks' hot_* in
# bend/proofs/hub.bend. Part 1: numbers, inserting by id, the hot and cold ones of a list.
import Base
import ../../hub/model.bend as HM
import ../../hub/gen/state.bend as HS
import ../../hub/gen/state-laws.bend as HSL
import ../../hub/cx.bend as HX
import ../../hub/inv.bend as HI
import ../../hub/lazy.bend as HLZ
import ../../hub/cold.bend as CO
import ../../hub/core.bend as HC
import ./hub-1.bend as PL1
import ./absurd.bend as AB

# ---- numbers ----

def hc_gt_lt(+a: Nat, +b: Nat, h: {Nat.is_gt(a, b) == True{} : Bool}) -> {Nat.is_lt(b, a) == True{} : Bool}:
  match a b:
    case 0n 0n:
      AB.absurd_eq(AB.ft_absurd(h), Bool, Nat.is_lt(b, a), True{})
    case 0n 1n+bp:
      AB.absurd_eq(AB.ft_absurd(h), Bool, Nat.is_lt(b, a), True{})
    case 1n+ap 0n:
      {==}
    case 1n+ap 1n+bp:
      hc_gt_lt(ap, bp, h)

def hc_lt_gt(+a: Nat, +b: Nat, h: {Nat.is_lt(a, b) == True{} : Bool}) -> {Nat.is_gt(b, a) == True{} : Bool}:
  match a b:
    case 0n 0n:
      AB.absurd_eq(AB.ft_absurd(h), Bool, Nat.is_gt(b, a), True{})
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      AB.absurd_eq(AB.ft_absurd(h), Bool, Nat.is_gt(b, a), True{})
    case 1n+ap 1n+bp:
      hc_lt_gt(ap, bp, h)

# a > b: a is not b
def hc_gt_neq(+a: Nat, +b: Nat, h: {Nat.is_gt(a, b) == True{} : Bool}) -> {Nat.is_eq(a, b) == False{} : Bool}:
  match a b:
    case 0n 0n:
      AB.absurd_eq(AB.ft_absurd(h), Bool, Nat.is_eq(a, b), False{})
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      {==}
    case 1n+ap 1n+bp:
      hc_gt_neq(ap, bp, h)

# a <= b (not a > b) and a > c: b > c
def hc_ge_gt(+a: Nat, +b: Nat, +c: Nat, g: {Nat.is_gt(a, b) == False{} : Bool}, h: {Nat.is_gt(a, c) == True{} : Bool})
    -> {Nat.is_gt(b, c) == True{} : Bool}:
  match a b c:
    case 0n _ 0n:
      AB.absurd_eq(AB.ft_absurd(h), Bool, Nat.is_gt(b, c), True{})
    case 0n _ 1n+cp:
      AB.absurd_eq(AB.ft_absurd(h), Bool, Nat.is_gt(b, c), True{})
    case 1n+ap 0n _:
      AB.absurd_eq(AB.tf_absurd(g), Bool, Nat.is_gt(b, c), True{})
    case 1n+ap 1n+bp 0n:
      {==}
    case 1n+ap 1n+bp 1n+cp:
      hc_ge_gt(ap, bp, cp, g, h)

# a is not b: a > b is not b > a
def hc_gt_flip(+a: Nat, +b: Nat, ne: {Nat.is_eq(a, b) == False{} : Bool}) -> {Nat.is_gt(b, a) == Bool.not(Nat.is_gt(a, b)) : Bool}:
  match a b:
    case 0n 0n:
      AB.absurd_eq(AB.tf_absurd(ne), Bool, Nat.is_gt(b, a), Bool.not(Nat.is_gt(a, b)))
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      {==}
    case 1n+ap 1n+bp:
      hc_gt_flip(ap, bp, ne)

# x < n and n <= m (m >= n): x < m, and m is not x
def hc_lt_ge_neq(+x: Nat, +n: Nat, +m: Nat, h: {Nat.is_lt(x, n) == True{} : Bool}, g: {Nat.is_ge(m, n) == True{} : Bool})
    -> {Nat.is_eq(m, x) == False{} : Bool}:
  PL1.hub_lt_neq(x, m, PL1.hub_lt_ge(x, n, m, h, g))

# ---- inserting by id, one step ----

def hc_ins_T(+h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == True{} : Bool})
    -> {HM.ins_desc(h <> t, r) == h <> HM.ins_desc(t, r) : List<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), True{}, e) : {HM.ins_desc.go(h <> t, _, r) == h <> HM.ins_desc(t, r) : List<&2, HS.MRec>}
  {==}

def hc_ins_F(+h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == False{} : Bool})
    -> {HM.ins_desc(h <> t, r) == r <> h <> t : List<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), False{}, e) : {HM.ins_desc.go(h <> t, _, r) == r <> h <> t : List<&2, HS.MRec>}
  {==}

# ---- inserting two records commutes (any list, two different ids) ----

def hc_comm_nil(ga: Bool, +a: HS.MRec, +b: HS.MRec, e: {Nat.is_gt(HM.MRec.id(a), HM.MRec.id(b)) == ga : Bool},
    f: {Nat.is_gt(HM.MRec.id(b), HM.MRec.id(a)) == Bool.not(ga) : Bool})
    -> {HM.ins_desc(HM.ins_desc(Nil{}, a), b) == HM.ins_desc(HM.ins_desc(Nil{}, b), a) : List<&2, HS.MRec>}:
  match ga:
    case True{}:
      %Equal.sym(Bool, Nat.is_gt(HM.MRec.id(a), HM.MRec.id(b)), True{}, e) : {HM.ins_desc.go([a], _, b) == HM.ins_desc(HM.ins_desc(Nil{}, b), a) : List<&2, HS.MRec>}
      %Equal.sym(Bool, Nat.is_gt(HM.MRec.id(b), HM.MRec.id(a)), False{}, f) : {HM.ins_desc.go([a], True{}, b) == HM.ins_desc.go([b], _, a) : List<&2, HS.MRec>}
      {==}
    case False{}:
      %Equal.sym(Bool, Nat.is_gt(HM.MRec.id(a), HM.MRec.id(b)), False{}, e) : {HM.ins_desc.go([a], _, b) == HM.ins_desc(HM.ins_desc(Nil{}, b), a) : List<&2, HS.MRec>}
      %Equal.sym(Bool, Nat.is_gt(HM.MRec.id(b), HM.MRec.id(a)), True{}, f) : {HM.ins_desc.go([a], False{}, b) == HM.ins_desc.go([b], _, a) : List<&2, HS.MRec>}
      {==}

# both records go before the head (h <= a, h <= b): their own order decides
def hc_comm_ff(ga: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +a: HS.MRec, +b: HS.MRec,
    +ea: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(a)) == False{} : Bool}, +eb: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(b)) == False{} : Bool},
    +e: {Nat.is_gt(HM.MRec.id(a), HM.MRec.id(b)) == ga : Bool}, +f: {Nat.is_gt(HM.MRec.id(b), HM.MRec.id(a)) == Bool.not(ga) : Bool})
    -> {HM.ins_desc(HM.ins_desc(h <> t, a), b) == HM.ins_desc(HM.ins_desc(h <> t, b), a) : List<&2, HS.MRec>}:
  match ga:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, a), a <> h <> t, hc_ins_F(h, t, a, ea)) : {HM.ins_desc(_, b) == HM.ins_desc(HM.ins_desc(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, b), b <> h <> t, hc_ins_F(h, t, b, eb)) : {HM.ins_desc(a <> h <> t, b) == HM.ins_desc(_, a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(a <> h <> t, b), a <> HM.ins_desc(h <> t, b), hc_ins_T(a, h <> t, b, e)) : {_ == HM.ins_desc(b <> h <> t, a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, b), b <> h <> t, hc_ins_F(h, t, b, eb)) : {a <> _ == HM.ins_desc(b <> h <> t, a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(b <> h <> t, a), a <> b <> h <> t, hc_ins_F(b, h <> t, a, f)) : {a <> b <> h <> t == _ : List<&2, HS.MRec>}
      {==}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, a), a <> h <> t, hc_ins_F(h, t, a, ea)) : {HM.ins_desc(_, b) == HM.ins_desc(HM.ins_desc(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, b), b <> h <> t, hc_ins_F(h, t, b, eb)) : {HM.ins_desc(a <> h <> t, b) == HM.ins_desc(_, a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(a <> h <> t, b), b <> a <> h <> t, hc_ins_F(a, h <> t, b, e)) : {_ == HM.ins_desc(b <> h <> t, a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(b <> h <> t, a), b <> HM.ins_desc(h <> t, a), hc_ins_T(b, h <> t, a, f)) : {b <> a <> h <> t == _ : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, a), a <> h <> t, hc_ins_F(h, t, a, ea)) : {b <> a <> h <> t == b <> _ : List<&2, HS.MRec>}
      {==}

def hc_comm_con(gha: Bool, ghb: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +a: HS.MRec, +b: HS.MRec,
    +ea: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(a)) == gha : Bool}, +eb: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(b)) == ghb : Bool},
    +ne: {Nat.is_eq(HM.MRec.id(a), HM.MRec.id(b)) == False{} : Bool},
    ih: {HM.ins_desc(HM.ins_desc(t, a), b) == HM.ins_desc(HM.ins_desc(t, b), a) : List<&2, HS.MRec>})
    -> {HM.ins_desc(HM.ins_desc(h <> t, a), b) == HM.ins_desc(HM.ins_desc(h <> t, b), a) : List<&2, HS.MRec>}:
  match gha ghb:
    case True{} True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, a), h <> HM.ins_desc(t, a), hc_ins_T(h, t, a, ea)) : {HM.ins_desc(_, b) == HM.ins_desc(HM.ins_desc(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, b), h <> HM.ins_desc(t, b), hc_ins_T(h, t, b, eb)) : {HM.ins_desc(h <> HM.ins_desc(t, a), b) == HM.ins_desc(_, a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> HM.ins_desc(t, a), b), h <> HM.ins_desc(HM.ins_desc(t, a), b), hc_ins_T(h, HM.ins_desc(t, a), b, eb)) : {_ == HM.ins_desc(h <> HM.ins_desc(t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> HM.ins_desc(t, b), a), h <> HM.ins_desc(HM.ins_desc(t, b), a), hc_ins_T(h, HM.ins_desc(t, b), a, ea)) : {h <> HM.ins_desc(HM.ins_desc(t, a), b) == _ : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, x => h <> x, HM.ins_desc(HM.ins_desc(t, a), b), HM.ins_desc(HM.ins_desc(t, b), a), ih)
    case True{} False{}:
      +gba = hc_ge_gt(HM.MRec.id(h), HM.MRec.id(b), HM.MRec.id(a), eb, ea)
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, a), h <> HM.ins_desc(t, a), hc_ins_T(h, t, a, ea)) : {HM.ins_desc(_, b) == HM.ins_desc(HM.ins_desc(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> HM.ins_desc(t, a), b), b <> h <> HM.ins_desc(t, a), hc_ins_F(h, HM.ins_desc(t, a), b, eb)) : {_ == HM.ins_desc(HM.ins_desc(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, b), b <> h <> t, hc_ins_F(h, t, b, eb)) : {b <> h <> HM.ins_desc(t, a) == HM.ins_desc(_, a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(b <> h <> t, a), b <> HM.ins_desc(h <> t, a), hc_ins_T(b, h <> t, a, gba)) : {b <> h <> HM.ins_desc(t, a) == _ : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, a), h <> HM.ins_desc(t, a), hc_ins_T(h, t, a, ea)) : {b <> h <> HM.ins_desc(t, a) == b <> _ : List<&2, HS.MRec>}
      {==}
    case False{} True{}:
      +gab = hc_ge_gt(HM.MRec.id(h), HM.MRec.id(a), HM.MRec.id(b), ea, eb)
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, a), a <> h <> t, hc_ins_F(h, t, a, ea)) : {HM.ins_desc(_, b) == HM.ins_desc(HM.ins_desc(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(a <> h <> t, b), a <> HM.ins_desc(h <> t, b), hc_ins_T(a, h <> t, b, gab)) : {_ == HM.ins_desc(HM.ins_desc(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, b), h <> HM.ins_desc(t, b), hc_ins_T(h, t, b, eb)) : {a <> _ == HM.ins_desc(_, a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> HM.ins_desc(t, b), a), a <> h <> HM.ins_desc(t, b), hc_ins_F(h, HM.ins_desc(t, b), a, ea)) : {a <> h <> HM.ins_desc(t, b) == _ : List<&2, HS.MRec>}
      {==}
    case False{} False{}:
      hc_comm_ff(Nat.is_gt(HM.MRec.id(a), HM.MRec.id(b)), h, t, a, b, ea, eb, {==}, hc_gt_flip(HM.MRec.id(a), HM.MRec.id(b), ne))

# two records of different ids go in, in either order: the same list
def hc_ins_comm(+xs: List<&2, HS.MRec>, +a: HS.MRec, +b: HS.MRec, +ne: {Nat.is_eq(HM.MRec.id(a), HM.MRec.id(b)) == False{} : Bool})
    -> {HM.ins_desc(HM.ins_desc(xs, a), b) == HM.ins_desc(HM.ins_desc(xs, b), a) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      hc_comm_nil(Nat.is_gt(HM.MRec.id(a), HM.MRec.id(b)), a, b, {==}, hc_gt_flip(HM.MRec.id(a), HM.MRec.id(b), ne))
    case Con{h, t}:
      hc_comm_con(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(a)), Nat.is_gt(HM.MRec.id(h), HM.MRec.id(b)), h, t, a, b, {==}, {==}, ne,
        hc_ins_comm(t, a, b, ne))

# ---- ids in and out ----

# x < n: x is not n
def hc_lt_ne(+x: Nat, +n: Nat, h: {Nat.is_lt(x, n) == True{} : Bool}) -> {Nat.is_eq(x, n) == False{} : Bool}:
  Equal.trans(Bool, Nat.is_eq(x, n), Nat.is_eq(n, x), False{}, PL1.hub_eq_sym(x, n), PL1.hub_lt_neq(x, n, h))

# x < n: not x > n
def hc_lt_not_gt(+x: Nat, +n: Nat, h: {Nat.is_lt(x, n) == True{} : Bool}) -> {Nat.is_gt(x, n) == False{} : Bool}:
  match x n:
    case 0n 0n:
      {==}
    case 0n 1n+np:
      {==}
    case 1n+xp 0n:
      AB.absurd_eq(AB.ft_absurd(h), Bool, Nat.is_gt(x, n), False{})
    case 1n+xp 1n+np:
      hc_lt_not_gt(xp, np, h)

# a < b: b >= a
def hc_lt_ge(+a: Nat, +b: Nat, h: {Nat.is_lt(a, b) == True{} : Bool}) -> {Nat.is_ge(b, a) == True{} : Bool}:
  match a b:
    case 0n 0n:
      {==}
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      AB.absurd_eq(AB.ft_absurd(h), Bool, Nat.is_ge(b, a), True{})
    case 1n+ap 1n+bp:
      hc_lt_ge(ap, bp, h)

# not a > b, and a is not b: a < b
def hc_le_ne_lt(+a: Nat, +b: Nat, g: {Nat.is_gt(a, b) == False{} : Bool}, ne: {Nat.is_eq(a, b) == False{} : Bool})
    -> {Nat.is_lt(a, b) == True{} : Bool}:
  match a b:
    case 0n 0n:
      AB.absurd_eq(AB.tf_absurd(ne), Bool, Nat.is_lt(a, b), True{})
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      AB.absurd_eq(AB.tf_absurd(g), Bool, Nat.is_lt(a, b), True{})
    case 1n+ap 1n+bp:
      hc_le_ne_lt(ap, bp, g, ne)

# a = b as booleans say: a and b are the same number
def hc_eq_nat(+a: Nat, +b: Nat, e: {Nat.is_eq(a, b) == True{} : Bool}) -> {a == b : Nat}:
  match a b:
    case 0n 0n:
      {==}
    case 0n 1n+bp:
      AB.absurd_eq(AB.ft_absurd(e), Nat, a, b)
    case 1n+ap 0n:
      AB.absurd_eq(AB.ft_absurd(e), Nat, a, b)
    case 1n+ap 1n+bp:
      Equal.cong(Nat, Nat, x => 1n+x, ap, bp, hc_eq_nat(ap, bp, e))

# every id below n: n is none of them
def hc_below_nin(+ns: List<&2, Nat>, +n: Nat, b: HI.Below(ns, n)) -> {HM.nat_in(ns, n) == False{} : Bool}:
  match ns:
    case Nil{}:
      {==}
    case Con{x, t}:
      match b:
        case HX.Both{l, bt}:
          PL1.hub_lor_false(Nat.is_eq(x, n), HM.nat_in(t, n), hc_lt_ne(x, n, l), hc_below_nin(t, n, bt))

# every id above n: n is none of them
def hc_above_nin(+xs: List<&2, HS.MRec>, +n: Nat, a: HI.Above(xs, n)) -> {HM.nat_in(HI.ids(xs), n) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, t}:
      match a:
        case HX.Both{g, at}:
          PL1.hub_lor_false(Nat.is_eq(HM.MRec.id(x), n), HM.nat_in(HI.ids(t), n),
            hc_gt_neq(HM.MRec.id(x), n, g), hc_above_nin(t, n, at))

# an op keeps a record's id
def hc_rec_op_id(+r: HS.MRec, +op: HM.MsgOp) -> {HM.MRec.id(HM.rec_op(r, op)) == HM.MRec.id(r) : Nat}:
  match op:
    case HM.OSt{x}:
      %HSL.MRec.st.eta(r, x) : {HM.MRec.id(_) == HM.MRec.id(r) : Nat}
      {==}
    case HM.OSettle{}:
      %HSL.MRec.settled.eta(r, True{}) : {HM.MRec.id(_) == HM.MRec.id(r) : Nat}
      {==}

def hc_nin_ins.go(g: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, +n: Nat,
    +e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool},
    +a: {HM.nat_in(HI.ids(h <> t), n) == False{} : Bool}, +b: {Nat.is_eq(HM.MRec.id(r), n) == False{} : Bool},
    ih: {HM.nat_in(HI.ids(HM.ins_desc(t, r)), n) == False{} : Bool})
    -> {HM.nat_in(HI.ids(HM.ins_desc(h <> t, r)), n) == False{} : Bool}:
  match g:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), hc_ins_T(h, t, r, e)) : {HM.nat_in(HI.ids(_), n) == False{} : Bool}
      PL1.hub_lor_false(Nat.is_eq(HM.MRec.id(h), n), HM.nat_in(HI.ids(HM.ins_desc(t, r)), n),
        PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), n), HM.nat_in(HI.ids(t), n), a), ih)
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), r <> h <> t, hc_ins_F(h, t, r, e)) : {HM.nat_in(HI.ids(_), n) == False{} : Bool}
      PL1.hub_lor_false(Nat.is_eq(HM.MRec.id(r), n), HM.nat_in(HI.ids(h <> t), n), b, a)

# n is in neither xs nor r: not in xs with r put in
def hc_nin_ins(+xs: List<&2, HS.MRec>, +r: HS.MRec, +n: Nat, +a: {HM.nat_in(HI.ids(xs), n) == False{} : Bool},
    +b: {Nat.is_eq(HM.MRec.id(r), n) == False{} : Bool}) -> {HM.nat_in(HI.ids(HM.ins_desc(xs, r)), n) == False{} : Bool}:
  match xs:
    case Nil{}:
      PL1.hub_lor_false(Nat.is_eq(HM.MRec.id(r), n), False{}, b, {==})
    case Con{h, t}:
      hc_nin_ins.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), h, t, r, n, {==}, a, b,
        hc_nin_ins(t, r, n, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), n), HM.nat_in(HI.ids(t), n), a), b))

def hc_nin_all(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +n: Nat, +a: {HM.nat_in(HI.ids(hs), n) == False{} : Bool},
    +b: {HM.nat_in(HI.ids(os), n) == False{} : Bool}) -> {HM.nat_in(HI.ids(HM.ins_all(hs, os)), n) == False{} : Bool}:
  match hs:
    case Nil{}:
      b
    case Con{h, t}:
      hc_nin_ins(HM.ins_all(t, os), h, n,
        hc_nin_all(t, os, n, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), n), HM.nat_in(HI.ids(t), n), a), b),
        PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), n), HM.nat_in(HI.ids(t), n), a))

# ---- put all in ----

# r goes in first or last: the same (r's id is none of hs')
def hc_all_ins(+hs: List<&2, HS.MRec>, +ys: List<&2, HS.MRec>, +r: HS.MRec, +nr: {HM.nat_in(HI.ids(hs), HM.MRec.id(r)) == False{} : Bool})
    -> {HM.ins_all(hs, HM.ins_desc(ys, r)) == HM.ins_desc(HM.ins_all(hs, ys), r) : List<&2, HS.MRec>}:
  match hs:
    case Nil{}:
      {==}
    case Con{h, t}:
      +ne = Equal.trans(Bool, Nat.is_eq(HM.MRec.id(r), HM.MRec.id(h)), Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), False{},
        PL1.hub_eq_sym(HM.MRec.id(r), HM.MRec.id(h)),
        PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), HM.nat_in(HI.ids(t), HM.MRec.id(r)), nr))
      Equal.trans(List<&2, HS.MRec>, HM.ins_desc(HM.ins_all(t, HM.ins_desc(ys, r)), h),
        HM.ins_desc(HM.ins_desc(HM.ins_all(t, ys), r), h), HM.ins_desc(HM.ins_desc(HM.ins_all(t, ys), h), r),
        Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, x => HM.ins_desc(x, h), HM.ins_all(t, HM.ins_desc(ys, r)), HM.ins_desc(HM.ins_all(t, ys), r),
          hc_all_ins(t, ys, r, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), HM.nat_in(HI.ids(t), HM.MRec.id(r)), nr))),
        hc_ins_comm(HM.ins_all(t, ys), r, h, ne))

# a new last hot one: the same as putting it among the old ones first
def hc_all_snoc(+hs: List<&2, HS.MRec>, +r: HS.MRec, +os: List<&2, HS.MRec>)
    -> {HM.ins_all(HM.snoc(HS.MRec, hs, r), os) == HM.ins_all(hs, HM.ins_desc(os, r)) : List<&2, HS.MRec>}:
  match hs:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, x => HM.ins_desc(x, h), HM.ins_all(HM.snoc(HS.MRec, t, r), os), HM.ins_all(t, HM.ins_desc(os, r)),
        hc_all_snoc(t, r, os))

# r above every id: it goes at the head
def hc_ins_head(+xs: List<&2, HS.MRec>, +r: HS.MRec, b: HI.Below(HI.ids(xs), HM.MRec.id(r))) -> {HM.ins_desc(xs, r) == r <> xs : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match b:
        case HX.Both{l, bt}:
          hc_ins_F(h, t, r, hc_lt_not_gt(HM.MRec.id(h), HM.MRec.id(r), l))

# r below every id: it goes last
def hc_ins_last(+xs: List<&2, HS.MRec>, +r: HS.MRec, a: HI.Above(xs, HM.MRec.id(r))) -> {HM.ins_desc(xs, r) == HM.snoc(HS.MRec, xs, r) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match a:
        case HX.Both{g, at}:
          Equal.trans(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), h <> HM.snoc(HS.MRec, t, r),
            hc_ins_T(h, t, r, g),
            Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, x => h <> x, HM.ins_desc(t, r), HM.snoc(HS.MRec, t, r), hc_ins_last(t, r, at)))

# ---- an op on one record, through the inserts ----

# no record has the id: the op changes nothing
def hc_map_miss.go(c: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp,
    +e: {Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id) == c : Bool}, +f: {c == False{} : Bool},
    ih: {HM.map_msg(t, id, op) == t : List<&2, HS.MRec>}) -> {HM.map_msg(h <> t, id, op) == h <> t : List<&2, HS.MRec>}:
  match c:
    case True{}:
      AB.absurd_eq(AB.tf_absurd(f), List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> t)
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id), False{}, e) : {HLZ.pick(List<&2, HS.MRec>, _, u => HM.rec_op(h, op) <> t, u => h <> HM.map_msg(t, id, op)) == h <> t : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, x => h <> x, HM.map_msg(t, id, op), t, ih)

def hc_map_miss(+xs: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +a: {HM.nat_in(HI.ids(xs), id) == False{} : Bool})
    -> {HM.map_msg(xs, id, op) == xs : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      +l = PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(t), id), a)
      hc_map_miss.go(Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id), h, t, id, op, {==}, l,
        hc_map_miss(t, id, op, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(t), id), a)))

# the map's step on a head whose id is (not) the one
def hc_map_hd_F(+h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +e: {Nat.is_eq(HM.MRec.id(h), id) == False{} : Bool})
    -> {HM.map_msg(h <> t, id, op) == h <> HM.map_msg(t, id, op) : List<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id), False{}, e) : {HLZ.pick(List<&2, HS.MRec>, _, u => HM.rec_op(h, op) <> t, u => h <> HM.map_msg(t, id, op)) == h <> HM.map_msg(t, id, op) : List<&2, HS.MRec>}
  {==}

def hc_map_hd_T(+h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +e: {Nat.is_eq(HM.MRec.id(h), id) == True{} : Bool})
    -> {HM.map_msg(h <> t, id, op) == HM.rec_op(h, op) <> t : List<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id), True{}, e) : {HLZ.pick(List<&2, HS.MRec>, _, u => HM.rec_op(h, op) <> t, u => h <> HM.map_msg(t, id, op)) == HM.rec_op(h, op) <> t : List<&2, HS.MRec>}
  {==}

# the op's record keeps its id, so it compares the same
def hc_gt_op(+y: HS.MRec, +op: HM.MsgOp, +n: Nat) -> {Nat.is_gt(HM.MRec.id(HM.rec_op(y, op)), n) == Nat.is_gt(HM.MRec.id(y), n) : Bool}:
  Equal.cong(Nat, Bool, x => Nat.is_gt(x, n), HM.MRec.id(HM.rec_op(y, op)), HM.MRec.id(y), hc_rec_op_id(y, op))

def hc_gt_op_eq(+y: HS.MRec, +op: HM.MsgOp, +n: Nat, -g: Bool, +eg: {Nat.is_gt(HM.MRec.id(y), n) == g : Bool})
    -> {Nat.is_gt(HM.MRec.id(HM.rec_op(y, op)), n) == g : Bool}:
  Equal.trans(Bool, Nat.is_gt(HM.MRec.id(HM.rec_op(y, op)), n), Nat.is_gt(HM.MRec.id(y), n), g, hc_gt_op(y, op, n), eg)

def hc_map_ins_ne.go(g: Bool, q: Bool, +y: HS.MRec, +t: List<&2, HS.MRec>, +h: HS.MRec, +id: Nat, +op: HM.MsgOp,
    +eg: {Nat.is_gt(HM.MRec.id(y), HM.MRec.id(h)) == g : Bool}, +eq: {Nat.is_eq(HM.MRec.id(y), id) == q : Bool},
    +ne: {Nat.is_eq(HM.MRec.id(h), id) == False{} : Bool},
    ih: {HM.map_msg(HM.ins_desc(t, h), id, op) == HM.ins_desc(HM.map_msg(t, id, op), h) : List<&2, HS.MRec>})
    -> {HM.map_msg(HM.ins_desc(y <> t, h), id, op) == HM.ins_desc(HM.map_msg(y <> t, id, op), h) : List<&2, HS.MRec>}:
  match g q:
    case True{} True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), y <> HM.ins_desc(t, h), hc_ins_T(y, t, h, eg)) : {HM.map_msg(_, id, op) == HM.ins_desc(HM.map_msg(y <> t, id, op), h) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(y <> HM.ins_desc(t, h), id, op), HM.rec_op(y, op) <> HM.ins_desc(t, h), hc_map_hd_T(y, HM.ins_desc(t, h), id, op, eq)) : {_ == HM.ins_desc(HM.map_msg(y <> t, id, op), h) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(y <> t, id, op), HM.rec_op(y, op) <> t, hc_map_hd_T(y, t, id, op, eq)) : {HM.rec_op(y, op) <> HM.ins_desc(t, h) == HM.ins_desc(_, h) : List<&2, HS.MRec>}
      Equal.sym(List<&2, HS.MRec>, HM.ins_desc(HM.rec_op(y, op) <> t, h), HM.rec_op(y, op) <> HM.ins_desc(t, h), hc_ins_T(HM.rec_op(y, op), t, h, hc_gt_op_eq(y, op, HM.MRec.id(h), True{}, eg)))
    case True{} False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), y <> HM.ins_desc(t, h), hc_ins_T(y, t, h, eg)) : {HM.map_msg(_, id, op) == HM.ins_desc(HM.map_msg(y <> t, id, op), h) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(y <> HM.ins_desc(t, h), id, op), y <> HM.map_msg(HM.ins_desc(t, h), id, op), hc_map_hd_F(y, HM.ins_desc(t, h), id, op, eq)) : {_ == HM.ins_desc(HM.map_msg(y <> t, id, op), h) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(y <> t, id, op), y <> HM.map_msg(t, id, op), hc_map_hd_F(y, t, id, op, eq)) : {y <> HM.map_msg(HM.ins_desc(t, h), id, op) == HM.ins_desc(_, h) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> HM.map_msg(t, id, op), h), y <> HM.ins_desc(HM.map_msg(t, id, op), h), hc_ins_T(y, HM.map_msg(t, id, op), h, eg)) : {y <> HM.map_msg(HM.ins_desc(t, h), id, op) == _ : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, x => y <> x, HM.map_msg(HM.ins_desc(t, h), id, op), HM.ins_desc(HM.map_msg(t, id, op), h), ih)
    case False{} True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), h <> y <> t, hc_ins_F(y, t, h, eg)) : {HM.map_msg(_, id, op) == HM.ins_desc(HM.map_msg(y <> t, id, op), h) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> y <> t, id, op), h <> HM.map_msg(y <> t, id, op), hc_map_hd_F(h, y <> t, id, op, ne)) : {_ == HM.ins_desc(HM.map_msg(y <> t, id, op), h) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(y <> t, id, op), HM.rec_op(y, op) <> t, hc_map_hd_T(y, t, id, op, eq)) : {h <> _ == HM.ins_desc(_, h) : List<&2, HS.MRec>}
      Equal.sym(List<&2, HS.MRec>, HM.ins_desc(HM.rec_op(y, op) <> t, h), h <> HM.rec_op(y, op) <> t, hc_ins_F(HM.rec_op(y, op), t, h, hc_gt_op_eq(y, op, HM.MRec.id(h), False{}, eg)))
    case False{} False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), h <> y <> t, hc_ins_F(y, t, h, eg)) : {HM.map_msg(_, id, op) == HM.ins_desc(HM.map_msg(y <> t, id, op), h) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> y <> t, id, op), h <> HM.map_msg(y <> t, id, op), hc_map_hd_F(h, y <> t, id, op, ne)) : {_ == HM.ins_desc(HM.map_msg(y <> t, id, op), h) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(y <> t, id, op), y <> HM.map_msg(t, id, op), hc_map_hd_F(y, t, id, op, eq)) : {h <> _ == HM.ins_desc(_, h) : List<&2, HS.MRec>}
      Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> HM.map_msg(t, id, op), h), h <> y <> HM.map_msg(t, id, op), hc_ins_F(y, HM.map_msg(t, id, op), h, eg))

# h is not the record: the op and putting h in commute
def hc_map_ins_ne(+xs: List<&2, HS.MRec>, +h: HS.MRec, +id: Nat, +op: HM.MsgOp, +ne: {Nat.is_eq(HM.MRec.id(h), id) == False{} : Bool})
    -> {HM.map_msg(HM.ins_desc(xs, h), id, op) == HM.ins_desc(HM.map_msg(xs, id, op), h) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      hc_map_hd_F(h, Nil{}, id, op, ne)
    case Con{y, t}:
      hc_map_ins_ne.go(Nat.is_gt(HM.MRec.id(y), HM.MRec.id(h)), Nat.is_eq(HM.MRec.id(y), id), y, t, h, id, op, {==}, {==}, ne,
        hc_map_ins_ne(t, h, id, op, ne))

def hc_gt_op_r(+y: HS.MRec, +h: HS.MRec, +op: HM.MsgOp, -g: Bool, +eg: {Nat.is_gt(HM.MRec.id(y), HM.MRec.id(h)) == g : Bool})
    -> {Nat.is_gt(HM.MRec.id(y), HM.MRec.id(HM.rec_op(h, op))) == g : Bool}:
  Equal.trans(Bool, Nat.is_gt(HM.MRec.id(y), HM.MRec.id(HM.rec_op(h, op))), Nat.is_gt(HM.MRec.id(y), HM.MRec.id(h)), g,
    Equal.cong(Nat, Bool, x => Nat.is_gt(HM.MRec.id(y), x), HM.MRec.id(HM.rec_op(h, op)), HM.MRec.id(h), hc_rec_op_id(h, op)), eg)

def hc_map_ins_hit.go(g: Bool, +y: HS.MRec, +t: List<&2, HS.MRec>, +h: HS.MRec, +op: HM.MsgOp,
    +eg: {Nat.is_gt(HM.MRec.id(y), HM.MRec.id(h)) == g : Bool}, +ny: {Nat.is_eq(HM.MRec.id(y), HM.MRec.id(h)) == False{} : Bool},
    +eh: {Nat.is_eq(HM.MRec.id(h), HM.MRec.id(h)) == True{} : Bool},
    ih: {HM.map_msg(HM.ins_desc(t, h), HM.MRec.id(h), op) == HM.ins_desc(t, HM.rec_op(h, op)) : List<&2, HS.MRec>})
    -> {HM.map_msg(HM.ins_desc(y <> t, h), HM.MRec.id(h), op) == HM.ins_desc(y <> t, HM.rec_op(h, op)) : List<&2, HS.MRec>}:
  match g:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), y <> HM.ins_desc(t, h), hc_ins_T(y, t, h, eg)) : {HM.map_msg(_, HM.MRec.id(h), op) == HM.ins_desc(y <> t, HM.rec_op(h, op)) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(y <> HM.ins_desc(t, h), HM.MRec.id(h), op), y <> HM.map_msg(HM.ins_desc(t, h), HM.MRec.id(h), op), hc_map_hd_F(y, HM.ins_desc(t, h), HM.MRec.id(h), op, ny)) : {_ == HM.ins_desc(y <> t, HM.rec_op(h, op)) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, HM.rec_op(h, op)), y <> HM.ins_desc(t, HM.rec_op(h, op)), hc_ins_T(y, t, HM.rec_op(h, op), hc_gt_op_r(y, h, op, True{}, eg))) : {y <> HM.map_msg(HM.ins_desc(t, h), HM.MRec.id(h), op) == _ : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, x => y <> x, HM.map_msg(HM.ins_desc(t, h), HM.MRec.id(h), op), HM.ins_desc(t, HM.rec_op(h, op)), ih)
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), h <> y <> t, hc_ins_F(y, t, h, eg)) : {HM.map_msg(_, HM.MRec.id(h), op) == HM.ins_desc(y <> t, HM.rec_op(h, op)) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> y <> t, HM.MRec.id(h), op), HM.rec_op(h, op) <> y <> t, hc_map_hd_T(h, y <> t, HM.MRec.id(h), op, eh)) : {_ == HM.ins_desc(y <> t, HM.rec_op(h, op)) : List<&2, HS.MRec>}
      Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, HM.rec_op(h, op)), HM.rec_op(h, op) <> y <> t, hc_ins_F(y, t, HM.rec_op(h, op), hc_gt_op_r(y, h, op, False{}, eg)))

# h's id is none of xs': the op on h after it goes in is h's op going in
def hc_map_ins_hit(+xs: List<&2, HS.MRec>, +h: HS.MRec, +op: HM.MsgOp, +a: {HM.nat_in(HI.ids(xs), HM.MRec.id(h)) == False{} : Bool})
    -> {HM.map_msg(HM.ins_desc(xs, h), HM.MRec.id(h), op) == HM.ins_desc(xs, HM.rec_op(h, op)) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      hc_map_hd_T(h, Nil{}, HM.MRec.id(h), op, PL1.hub_eq_refl(HM.MRec.id(h)))
    case Con{y, t}:
      hc_map_ins_hit.go(Nat.is_gt(HM.MRec.id(y), HM.MRec.id(h)), y, t, h, op, {==},
        PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(y), HM.MRec.id(h)), HM.nat_in(HI.ids(t), HM.MRec.id(h)), a),
        PL1.hub_eq_refl(HM.MRec.id(h)),
        hc_map_ins_hit(t, h, op, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(y), HM.MRec.id(h)), HM.nat_in(HI.ids(t), HM.MRec.id(h)), a)))

# the op on a hot record, then all put in = all put in, then the op
def hc_map_all_hot.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp,
    +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool}, +nt: {HM.nat_in(HI.ids(t), HM.MRec.id(h)) == False{} : Bool},
    +no: {HM.nat_in(HI.ids(os), id) == False{} : Bool},
    ih: {HM.ins_all(HM.map_msg(t, id, op), os) == HM.map_msg(HM.ins_all(t, os), id, op) : List<&2, HS.MRec>})
    -> {HM.ins_all(HM.map_msg(h <> t, id, op), os) == HM.map_msg(HM.ins_all(h <> t, os), id, op) : List<&2, HS.MRec>}:
  match q:
    case True{}:
      +e = hc_eq_nat(HM.MRec.id(h), id, eq)
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), HM.rec_op(h, op) <> t, hc_map_hd_T(h, t, id, op, eq)) : {HM.ins_all(_, os) == HM.map_msg(HM.ins_all(h <> t, os), id, op) : List<&2, HS.MRec>}
      %e : {HM.ins_desc(HM.ins_all(t, os), HM.rec_op(h, op)) == HM.map_msg(HM.ins_desc(HM.ins_all(t, os), h), _, op) : List<&2, HS.MRec>}
      Equal.sym(List<&2, HS.MRec>, HM.map_msg(HM.ins_desc(HM.ins_all(t, os), h), HM.MRec.id(h), op), HM.ins_desc(HM.ins_all(t, os), HM.rec_op(h, op)),
        hc_map_ins_hit(HM.ins_all(t, os), h, op, hc_nin_all(t, os, HM.MRec.id(h), nt,
          Equal.trans(Bool, HM.nat_in(HI.ids(os), HM.MRec.id(h)), HM.nat_in(HI.ids(os), id), False{},
            Equal.cong(Nat, Bool, x => HM.nat_in(HI.ids(os), x), HM.MRec.id(h), id, e), no))))
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> HM.map_msg(t, id, op), hc_map_hd_F(h, t, id, op, eq)) : {HM.ins_all(_, os) == HM.map_msg(HM.ins_all(h <> t, os), id, op) : List<&2, HS.MRec>}
      Equal.trans(List<&2, HS.MRec>, HM.ins_desc(HM.ins_all(HM.map_msg(t, id, op), os), h), HM.ins_desc(HM.map_msg(HM.ins_all(t, os), id, op), h),
        HM.map_msg(HM.ins_desc(HM.ins_all(t, os), h), id, op),
        Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, x => HM.ins_desc(x, h), HM.ins_all(HM.map_msg(t, id, op), os), HM.map_msg(HM.ins_all(t, os), id, op), ih),
        Equal.sym(List<&2, HS.MRec>, HM.map_msg(HM.ins_desc(HM.ins_all(t, os), h), id, op), HM.ins_desc(HM.map_msg(HM.ins_all(t, os), id, op), h),
          hc_map_ins_ne(HM.ins_all(t, os), h, id, op, eq)))

def hc_map_all_hot(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, a: HI.Asc(hs),
    +no: {HM.nat_in(HI.ids(os), id) == False{} : Bool})
    -> {HM.ins_all(HM.map_msg(hs, id, op), os) == HM.map_msg(HM.ins_all(hs, os), id, op) : List<&2, HS.MRec>}:
  match hs:
    case Nil{}:
      Equal.sym(List<&2, HS.MRec>, HM.map_msg(os, id, op), os, hc_map_miss(os, id, op, no))
    case Con{h, t}:
      match a:
        case HX.Both{ab, at}:
          hc_map_all_hot.go(Nat.is_eq(HM.MRec.id(h), id), h, t, os, id, op, {==}, hc_above_nin(t, HM.MRec.id(h), ab), no,
            hc_map_all_hot(t, os, id, op, at, no))

# the op on an old record, then all put in = all put in, then the op
def hc_map_all_old(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +nh: {HM.nat_in(HI.ids(hs), id) == False{} : Bool})
    -> {HM.ins_all(hs, HM.map_msg(os, id, op)) == HM.map_msg(HM.ins_all(hs, os), id, op) : List<&2, HS.MRec>}:
  match hs:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.trans(List<&2, HS.MRec>, HM.ins_desc(HM.ins_all(t, HM.map_msg(os, id, op)), h), HM.ins_desc(HM.map_msg(HM.ins_all(t, os), id, op), h),
        HM.map_msg(HM.ins_desc(HM.ins_all(t, os), h), id, op),
        Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, x => HM.ins_desc(x, h), HM.ins_all(t, HM.map_msg(os, id, op)), HM.map_msg(HM.ins_all(t, os), id, op),
          hc_map_all_old(t, os, id, op, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(t), id), nh))),
        Equal.sym(List<&2, HS.MRec>, HM.map_msg(HM.ins_desc(HM.ins_all(t, os), h), id, op), HM.ins_desc(HM.map_msg(HM.ins_all(t, os), id, op), h),
          hc_map_ins_ne(HM.ins_all(t, os), h, id, op, PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(t), id), nh))))

# ---- turning around (List.reverse: every message in send order) ----

def hc_app_assoc(+a: List<&2, HS.MRec>, +b: List<&2, HS.MRec>, +c: List<&2, HS.MRec>)
    -> {List.append(&2, HS.MRec, List.append(&2, HS.MRec, a, b), c) == List.append(&2, HS.MRec, a, List.append(&2, HS.MRec, b, c)) : List<&2, HS.MRec>}:
  match a:
    case Nil{}:
      {==}
    case Con{x, t}:
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => x <> y, List.append(&2, HS.MRec, List.append(&2, HS.MRec, t, b), c),
        List.append(&2, HS.MRec, t, List.append(&2, HS.MRec, b, c)), hc_app_assoc(t, b, c))

def hc_app_one(+a: List<&2, HS.MRec>, +r: HS.MRec) -> {List.append(&2, HS.MRec, a, [r]) == HM.snoc(HS.MRec, a, r) : List<&2, HS.MRec>}:
  match a:
    case Nil{}:
      {==}
    case Con{x, t}:
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => x <> y, List.append(&2, HS.MRec, t, [r]), HM.snoc(HS.MRec, t, r), hc_app_one(t, r))

# reverse's accumulator is appended
def hc_rg_app(+xs: List<&2, HS.MRec>, +acc: List<&2, HS.MRec>)
    -> {List.reverse.go(&2, HS.MRec, xs, acc) == List.append(&2, HS.MRec, List.reverse.go(&2, HS.MRec, xs, Nil{}), acc) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, t}:
      +r = List.reverse.go(&2, HS.MRec, t, Nil{})
      Equal.trans(List<&2, HS.MRec>, List.reverse.go(&2, HS.MRec, t, x <> acc), List.append(&2, HS.MRec, r, x <> acc),
        List.append(&2, HS.MRec, List.reverse.go(&2, HS.MRec, t, [x]), acc),
        hc_rg_app(t, x <> acc),
        Equal.trans(List<&2, HS.MRec>, List.append(&2, HS.MRec, r, x <> acc), List.append(&2, HS.MRec, List.append(&2, HS.MRec, r, [x]), acc),
          List.append(&2, HS.MRec, List.reverse.go(&2, HS.MRec, t, [x]), acc),
          Equal.sym(List<&2, HS.MRec>, List.append(&2, HS.MRec, List.append(&2, HS.MRec, r, [x]), acc), List.append(&2, HS.MRec, r, x <> acc), hc_app_assoc(r, [x], acc)),
          Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => List.append(&2, HS.MRec, y, acc), List.append(&2, HS.MRec, r, [x]), List.reverse.go(&2, HS.MRec, t, [x]),
            Equal.sym(List<&2, HS.MRec>, List.reverse.go(&2, HS.MRec, t, [x]), List.append(&2, HS.MRec, r, [x]), hc_rg_app(t, [x])))))

# a head turned around goes last
def hc_rev_cons(+xs: List<&2, HS.MRec>, +r: HS.MRec)
    -> {List.reverse(&2, HS.MRec, r <> xs) == HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, xs), r) : List<&2, HS.MRec>}:
  Equal.trans(List<&2, HS.MRec>, List.reverse.go(&2, HS.MRec, xs, [r]), List.append(&2, HS.MRec, List.reverse.go(&2, HS.MRec, xs, Nil{}), [r]),
    HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, xs), r),
    hc_rg_app(xs, [r]), hc_app_one(List.reverse.go(&2, HS.MRec, xs, Nil{}), r))

# a last one turned around comes first
def hc_rev_snoc(+xs: List<&2, HS.MRec>, +y: HS.MRec)
    -> {List.reverse(&2, HS.MRec, HM.snoc(HS.MRec, xs, y)) == y <> List.reverse(&2, HS.MRec, xs) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{z, u}:
      Equal.trans(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, z <> HM.snoc(HS.MRec, u, y)), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, HM.snoc(HS.MRec, u, y)), z),
        y <> List.reverse(&2, HS.MRec, z <> u),
        hc_rev_cons(HM.snoc(HS.MRec, u, y), z),
        Equal.trans(List<&2, HS.MRec>, HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, HM.snoc(HS.MRec, u, y)), z), HM.snoc(HS.MRec, y <> List.reverse(&2, HS.MRec, u), z),
          y <> List.reverse(&2, HS.MRec, z <> u),
          Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => HM.snoc(HS.MRec, w, z), List.reverse(&2, HS.MRec, HM.snoc(HS.MRec, u, y)), y <> List.reverse(&2, HS.MRec, u), hc_rev_snoc(u, y)),
          Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => y <> w, HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, u), z), List.reverse(&2, HS.MRec, z <> u),
            Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, z <> u), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, u), z), hc_rev_cons(u, z)))))

# ---- the hot ones and the cold ones of a list ----

def hc_hot_T(+x: HS.MRec, +t: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == True{} : Bool}) -> {HM.hot_of(x <> t) == x <> HM.hot_of(t) : List<&2, HS.MRec>}:
  %Equal.sym(Bool, HM.MRec.is_hot(x), True{}, e) : {Bool.pick(List<&2, HS.MRec>, _, x <> HM.hot_of(t), HM.hot_of(t)) == x <> HM.hot_of(t) : List<&2, HS.MRec>}
  {==}

def hc_hot_F(+x: HS.MRec, +t: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == False{} : Bool}) -> {HM.hot_of(x <> t) == HM.hot_of(t) : List<&2, HS.MRec>}:
  %Equal.sym(Bool, HM.MRec.is_hot(x), False{}, e) : {Bool.pick(List<&2, HS.MRec>, _, x <> HM.hot_of(t), HM.hot_of(t)) == HM.hot_of(t) : List<&2, HS.MRec>}
  {==}

def hc_cold_T(+x: HS.MRec, +t: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == True{} : Bool}) -> {HM.cold_of(x <> t) == HM.cold_of(t) : List<&2, HS.MRec>}:
  %Equal.sym(Bool, HM.MRec.is_hot(x), True{}, e) : {Bool.pick(List<&2, HS.MRec>, _, HM.cold_of(t), x <> HM.cold_of(t)) == HM.cold_of(t) : List<&2, HS.MRec>}
  {==}

def hc_cold_F(+x: HS.MRec, +t: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == False{} : Bool}) -> {HM.cold_of(x <> t) == x <> HM.cold_of(t) : List<&2, HS.MRec>}:
  %Equal.sym(Bool, HM.MRec.is_hot(x), False{}, e) : {Bool.pick(List<&2, HS.MRec>, _, HM.cold_of(t), x <> HM.cold_of(t)) == x <> HM.cold_of(t) : List<&2, HS.MRec>}
  {==}

def hc_hot_snoc.go(c: Bool, +a: HS.MRec, +t: List<&2, HS.MRec>, +x: HS.MRec, +e: {HM.MRec.is_hot(a) == c : Bool},
    ih: {HM.hot_of(HM.snoc(HS.MRec, t, x)) == HM.snoc(HS.MRec, HM.hot_of(t), x) : List<&2, HS.MRec>})
    -> {HM.hot_of(a <> HM.snoc(HS.MRec, t, x)) == HM.snoc(HS.MRec, HM.hot_of(a <> t), x) : List<&2, HS.MRec>}:
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(a <> HM.snoc(HS.MRec, t, x)), a <> HM.hot_of(HM.snoc(HS.MRec, t, x)), hc_hot_T(a, HM.snoc(HS.MRec, t, x), e)) : {_ == HM.snoc(HS.MRec, HM.hot_of(a <> t), x) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(a <> t), a <> HM.hot_of(t), hc_hot_T(a, t, e)) : {a <> HM.hot_of(HM.snoc(HS.MRec, t, x)) == HM.snoc(HS.MRec, _, x) : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => a <> y, HM.hot_of(HM.snoc(HS.MRec, t, x)), HM.snoc(HS.MRec, HM.hot_of(t), x), ih)
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(a <> HM.snoc(HS.MRec, t, x)), HM.hot_of(HM.snoc(HS.MRec, t, x)), hc_hot_F(a, HM.snoc(HS.MRec, t, x), e)) : {_ == HM.snoc(HS.MRec, HM.hot_of(a <> t), x) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(a <> t), HM.hot_of(t), hc_hot_F(a, t, e)) : {HM.hot_of(HM.snoc(HS.MRec, t, x)) == HM.snoc(HS.MRec, _, x) : List<&2, HS.MRec>}
      ih

# a hot last one stays last among the hot ones
def hc_hot_snoc_T(+xs: List<&2, HS.MRec>, +x: HS.MRec, +e: {HM.MRec.is_hot(x) == True{} : Bool})
    -> {HM.hot_of(HM.snoc(HS.MRec, xs, x)) == HM.snoc(HS.MRec, HM.hot_of(xs), x) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      hc_hot_T(x, Nil{}, e)
    case Con{a, t}:
      hc_hot_snoc.go(HM.MRec.is_hot(a), a, t, x, {==}, hc_hot_snoc_T(t, x, e))

def hc_hot_snoc_F.go(c: Bool, +a: HS.MRec, +t: List<&2, HS.MRec>, +x: HS.MRec, +e: {HM.MRec.is_hot(a) == c : Bool},
    ih: {HM.hot_of(HM.snoc(HS.MRec, t, x)) == HM.hot_of(t) : List<&2, HS.MRec>})
    -> {HM.hot_of(a <> HM.snoc(HS.MRec, t, x)) == HM.hot_of(a <> t) : List<&2, HS.MRec>}:
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(a <> HM.snoc(HS.MRec, t, x)), a <> HM.hot_of(HM.snoc(HS.MRec, t, x)), hc_hot_T(a, HM.snoc(HS.MRec, t, x), e)) : {_ == HM.hot_of(a <> t) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(a <> t), a <> HM.hot_of(t), hc_hot_T(a, t, e)) : {a <> HM.hot_of(HM.snoc(HS.MRec, t, x)) == _ : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => a <> y, HM.hot_of(HM.snoc(HS.MRec, t, x)), HM.hot_of(t), ih)
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(a <> HM.snoc(HS.MRec, t, x)), HM.hot_of(HM.snoc(HS.MRec, t, x)), hc_hot_F(a, HM.snoc(HS.MRec, t, x), e)) : {_ == HM.hot_of(a <> t) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(a <> t), HM.hot_of(t), hc_hot_F(a, t, e)) : {HM.hot_of(HM.snoc(HS.MRec, t, x)) == _ : List<&2, HS.MRec>}
      ih

def hc_hot_snoc_F(+xs: List<&2, HS.MRec>, +x: HS.MRec, +e: {HM.MRec.is_hot(x) == False{} : Bool})
    -> {HM.hot_of(HM.snoc(HS.MRec, xs, x)) == HM.hot_of(xs) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      hc_hot_F(x, Nil{}, e)
    case Con{a, t}:
      hc_hot_snoc_F.go(HM.MRec.is_hot(a), a, t, x, {==}, hc_hot_snoc_F(t, x, e))

def hc_hot_rev.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == c : Bool},
    ih: {HM.hot_of(List.reverse(&2, HS.MRec, t)) == List.reverse(&2, HS.MRec, HM.hot_of(t)) : List<&2, HS.MRec>})
    -> {HM.hot_of(List.reverse(&2, HS.MRec, x <> t)) == List.reverse(&2, HS.MRec, HM.hot_of(x <> t)) : List<&2, HS.MRec>}:
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> t), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), hc_rev_cons(t, x)) : {HM.hot_of(_) == List.reverse(&2, HS.MRec, HM.hot_of(x <> t)) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x)), HM.snoc(HS.MRec, HM.hot_of(List.reverse(&2, HS.MRec, t)), x), hc_hot_snoc_T(List.reverse(&2, HS.MRec, t), x, e)) : {_ == List.reverse(&2, HS.MRec, HM.hot_of(x <> t)) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), x <> HM.hot_of(t), hc_hot_T(x, t, e)) : {HM.snoc(HS.MRec, HM.hot_of(List.reverse(&2, HS.MRec, t)), x) == List.reverse(&2, HS.MRec, _) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> HM.hot_of(t)), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, HM.hot_of(t)), x), hc_rev_cons(HM.hot_of(t), x)) : {HM.snoc(HS.MRec, HM.hot_of(List.reverse(&2, HS.MRec, t)), x) == _ : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => HM.snoc(HS.MRec, y, x), HM.hot_of(List.reverse(&2, HS.MRec, t)), List.reverse(&2, HS.MRec, HM.hot_of(t)), ih)
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> t), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), hc_rev_cons(t, x)) : {HM.hot_of(_) == List.reverse(&2, HS.MRec, HM.hot_of(x <> t)) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x)), HM.hot_of(List.reverse(&2, HS.MRec, t)), hc_hot_snoc_F(List.reverse(&2, HS.MRec, t), x, e)) : {_ == List.reverse(&2, HS.MRec, HM.hot_of(x <> t)) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), HM.hot_of(t), hc_hot_F(x, t, e)) : {HM.hot_of(List.reverse(&2, HS.MRec, t)) == List.reverse(&2, HS.MRec, _) : List<&2, HS.MRec>}
      ih

# the hot ones of a list turned around: its hot ones, turned around
def hc_hot_rev(+xs: List<&2, HS.MRec>) -> {HM.hot_of(List.reverse(&2, HS.MRec, xs)) == List.reverse(&2, HS.MRec, HM.hot_of(xs)) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, t}:
      hc_hot_rev.go(HM.MRec.is_hot(x), x, t, {==}, hc_hot_rev(t))

# ---- sorted stays sorted ----

def hc_below_ins.go(g: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, +n: Nat, +e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool},
    b: HI.Below(HI.ids(h <> t), n), +l: {Nat.is_lt(HM.MRec.id(r), n) == True{} : Bool}, ih: HI.Below(HI.ids(HM.ins_desc(t, r)), n))
    -> HI.Below(HI.ids(HM.ins_desc(h <> t, r)), n):
  match g:
    case True{}:
      match b:
        case HX.Both{lh, bt}:
          %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), hc_ins_T(h, t, r, e)) : HI.Below(HI.ids(_), n)
          HX.Both{lh, ih}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), r <> h <> t, hc_ins_F(h, t, r, e)) : HI.Below(HI.ids(_), n)
      HX.Both{l, b}

def hc_below_ins(+xs: List<&2, HS.MRec>, +r: HS.MRec, +n: Nat, b: HI.Below(HI.ids(xs), n), +l: {Nat.is_lt(HM.MRec.id(r), n) == True{} : Bool})
    -> HI.Below(HI.ids(HM.ins_desc(xs, r)), n):
  match xs:
    case Nil{}:
      HX.Both{l, Unit{}}
    case Con{h, t}:
      match b:
        case HX.Both{+lh, +bt}:
          hc_below_ins.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), h, t, r, n, {==}, HX.Both{lh, bt}, l, hc_below_ins(t, r, n, bt, l))

def hc_below_all(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +n: Nat, bh: HI.Below(HI.ids(hs), n), bo: HI.Below(HI.ids(os), n))
    -> HI.Below(HI.ids(HM.ins_all(hs, os)), n):
  match hs:
    case Nil{}:
      bo
    case Con{h, t}:
      match bh:
        case HX.Both{l, bt}:
          hc_below_ins(HM.ins_all(t, os), h, n, hc_below_all(t, os, n, bt, bo), l)

def hc_above_ins.go(g: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, +n: Nat, +e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool},
    a: HI.Above(h <> t, n), +l: {Nat.is_gt(HM.MRec.id(r), n) == True{} : Bool}, ih: HI.Above(HM.ins_desc(t, r), n))
    -> HI.Above(HM.ins_desc(h <> t, r), n):
  match g:
    case True{}:
      match a:
        case HX.Both{lh, at}:
          %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), hc_ins_T(h, t, r, e)) : HI.Above(_, n)
          HX.Both{lh, ih}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), r <> h <> t, hc_ins_F(h, t, r, e)) : HI.Above(_, n)
      HX.Both{l, a}

def hc_above_ins(+xs: List<&2, HS.MRec>, +r: HS.MRec, +n: Nat, a: HI.Above(xs, n), +l: {Nat.is_gt(HM.MRec.id(r), n) == True{} : Bool})
    -> HI.Above(HM.ins_desc(xs, r), n):
  match xs:
    case Nil{}:
      HX.Both{l, Unit{}}
    case Con{h, t}:
      match a:
        case HX.Both{+lh, +at}:
          hc_above_ins.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), h, t, r, n, {==}, HX.Both{lh, at}, l, hc_above_ins(t, r, n, at, l))

def hc_above_all(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +n: Nat, ah: HI.Above(hs, n), ao: HI.Above(os, n))
    -> HI.Above(HM.ins_all(hs, os), n):
  match hs:
    case Nil{}:
      ao
    case Con{h, t}:
      match ah:
        case HX.Both{l, at}:
          hc_above_ins(HM.ins_all(t, os), h, n, hc_above_all(t, os, n, at, ao), l)

def hc_desc_ins.go(g: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, +e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool},
    +bt: HI.Below(HI.ids(t), HM.MRec.id(h)), dt: HI.Desc(t), +nr: {HM.nat_in(HI.ids(h <> t), HM.MRec.id(r)) == False{} : Bool},
    ih: HI.Desc(HM.ins_desc(t, r)))
    -> HI.Desc(HM.ins_desc(h <> t, r)):
  match g:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), hc_ins_T(h, t, r, e)) : HI.Desc(_)
      HX.Both{hc_below_ins(t, r, HM.MRec.id(h), bt, hc_gt_lt(HM.MRec.id(h), HM.MRec.id(r), e)), ih}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), r <> h <> t, hc_ins_F(h, t, r, e)) : HI.Desc(_)
      +lt = hc_le_ne_lt(HM.MRec.id(h), HM.MRec.id(r), e, PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), HM.nat_in(HI.ids(t), HM.MRec.id(r)), nr))
      HX.Both{HX.Both{lt, PL1.hub_below_mono(HI.ids(t), HM.MRec.id(h), HM.MRec.id(r), hc_lt_ge(HM.MRec.id(h), HM.MRec.id(r), lt), bt)}, HX.Both{bt, dt}}

# a new id put in a descending list: still descending
def hc_desc_ins(+xs: List<&2, HS.MRec>, +r: HS.MRec, d: HI.Desc(xs), +nr: {HM.nat_in(HI.ids(xs), HM.MRec.id(r)) == False{} : Bool})
    -> HI.Desc(HM.ins_desc(xs, r)):
  match xs:
    case Nil{}:
      HX.Both{Unit{}, Unit{}}
    case Con{h, t}:
      match d:
        case HX.Both{+bt, +dt}:
          hc_desc_ins.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), h, t, r, {==}, bt, dt, nr,
            hc_desc_ins(t, r, dt, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), HM.nat_in(HI.ids(t), HM.MRec.id(r)), nr)))

# the hot ones put among the old ones: descending
def hc_desc_all(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, a: HI.Asc(hs), +d: HI.Desc(os), j: HI.Disjoint(hs, os))
    -> HI.Desc(HM.ins_all(hs, os)):
  match hs:
    case Nil{}:
      d
    case Con{h, t}:
      match a j:
        case HX.Both{ah, at} HX.Both{jh, jt}:
          hc_desc_ins(HM.ins_all(t, os), h, hc_desc_all(t, os, at, d, jt), hc_nin_all(t, os, HM.MRec.id(h), hc_above_nin(t, HM.MRec.id(h), ah), jh))

# ---- the hot ones and the cold ones go back in together ----

def hc_nin_hot.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +n: Nat, +e: {HM.MRec.is_hot(x) == c : Bool},
    +a: {HM.nat_in(HI.ids(x <> t), n) == False{} : Bool}, ih: {HM.nat_in(HI.ids(HM.hot_of(t)), n) == False{} : Bool})
    -> {HM.nat_in(HI.ids(HM.hot_of(x <> t)), n) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), x <> HM.hot_of(t), hc_hot_T(x, t, e)) : {HM.nat_in(HI.ids(_), n) == False{} : Bool}
      PL1.hub_lor_false(Nat.is_eq(HM.MRec.id(x), n), HM.nat_in(HI.ids(HM.hot_of(t)), n), PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(x), n), HM.nat_in(HI.ids(t), n), a), ih)
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), HM.hot_of(t), hc_hot_F(x, t, e)) : {HM.nat_in(HI.ids(_), n) == False{} : Bool}
      ih

def hc_nin_hot(+xs: List<&2, HS.MRec>, +n: Nat, +a: {HM.nat_in(HI.ids(xs), n) == False{} : Bool}) -> {HM.nat_in(HI.ids(HM.hot_of(xs)), n) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, t}:
      hc_nin_hot.go(HM.MRec.is_hot(x), x, t, n, {==}, a, hc_nin_hot(t, n, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(x), n), HM.nat_in(HI.ids(t), n), a)))

def hc_part.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == c : Bool},
    +nx: {HM.nat_in(HI.ids(t), HM.MRec.id(x)) == False{} : Bool},
    ih: {HM.ins_all(HM.hot_of(t), HM.ins_all(HM.cold_of(t), os)) == HM.ins_all(t, os) : List<&2, HS.MRec>})
    -> {HM.ins_all(HM.hot_of(x <> t), HM.ins_all(HM.cold_of(x <> t), os)) == HM.ins_all(x <> t, os) : List<&2, HS.MRec>}:
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), x <> HM.hot_of(t), hc_hot_T(x, t, e)) : {HM.ins_all(_, HM.ins_all(HM.cold_of(x <> t), os)) == HM.ins_all(x <> t, os) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), HM.cold_of(t), hc_cold_T(x, t, e)) : {HM.ins_all(x <> HM.hot_of(t), HM.ins_all(_, os)) == HM.ins_all(x <> t, os) : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => HM.ins_desc(y, x), HM.ins_all(HM.hot_of(t), HM.ins_all(HM.cold_of(t), os)), HM.ins_all(t, os), ih)
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), HM.hot_of(t), hc_hot_F(x, t, e)) : {HM.ins_all(_, HM.ins_all(HM.cold_of(x <> t), os)) == HM.ins_all(x <> t, os) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), x <> HM.cold_of(t), hc_cold_F(x, t, e)) : {HM.ins_all(HM.hot_of(t), HM.ins_all(_, os)) == HM.ins_all(x <> t, os) : List<&2, HS.MRec>}
      Equal.trans(List<&2, HS.MRec>, HM.ins_all(HM.hot_of(t), HM.ins_desc(HM.ins_all(HM.cold_of(t), os), x)),
        HM.ins_desc(HM.ins_all(HM.hot_of(t), HM.ins_all(HM.cold_of(t), os)), x), HM.ins_all(x <> t, os),
        hc_all_ins(HM.hot_of(t), HM.ins_all(HM.cold_of(t), os), x, hc_nin_hot(t, HM.MRec.id(x), nx)),
        Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => HM.ins_desc(y, x), HM.ins_all(HM.hot_of(t), HM.ins_all(HM.cold_of(t), os)), HM.ins_all(t, os), ih))

# a list's hot ones put in after its cold ones: the list put in
def hc_part(+xs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, a: HI.Asc(xs))
    -> {HM.ins_all(HM.hot_of(xs), HM.ins_all(HM.cold_of(xs), os)) == HM.ins_all(xs, os) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, t}:
      match a:
        case HX.Both{ax, at}:
          hc_part.go(HM.MRec.is_hot(x), x, t, os, {==}, hc_above_nin(t, HM.MRec.id(x), ax), hc_part(t, os, at))
