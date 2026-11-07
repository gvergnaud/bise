# bend/proofs/lib/hub-cold-3.bend: the lemmas of the split messages (issue
# 21 steps 3-4: St.hot ids ascending, St.old ids descending, every = the
# hot ones put among the old ones, turned around). Used by the laws
# split_*, ids_ascending, cold_is_inert and the walks' hot_* in
# bend/proofs/hub.bend. Part 3: the op through the split: map lemmas, the cold side, back to the hot side, msg_op.
import Base
import ../../hub/model.bend as HM
import ../../hub/gen/state.bend as HS
import ../../hub/cx.bend as HX
import ../../hub/inv.bend as HI
import ../../hub/lazy.bend as HLZ
import ../../hub/cold.bend as CO
import ../../hub/core.bend as HC
import ./hub-1.bend as PL1
import ./absurd.bend as AB
import ./hub-cold.bend as PC1
import ./hub-cold-2.bend as PC2

# ---- the same facts after the op (ids do not change) ----

def hc_above_n(+xs: List<&2, HS.MRec>, +a: Nat, +b: Nat, +e: {a == b : Nat}, x: HI.Above(xs, b)) -> HI.Above(xs, a):
  %Equal.sym(Nat, a, b, e) : HI.Above(xs, _)
  x

def hc_nin_n(+ns: List<&2, Nat>, +a: Nat, +b: Nat, +e: {a == b : Nat}, +x: {HM.nat_in(ns, b) == False{} : Bool})
    -> {HM.nat_in(ns, a) == False{} : Bool}:
  %Equal.sym(Nat, a, b, e) : {HM.nat_in(ns, _) == False{} : Bool}
  x

def hc_above_map.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +n: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    +l: {Nat.is_gt(HM.MRec.id(h), n) == True{} : Bool}, at: HI.Above(t, n), ih: HI.Above(HM.map_msg(t, id, op), n))
    -> HI.Above(HM.map_msg(h <> t, id, op), n):
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), HM.rec_op(h, op) <> t, PC1.hc_map_hd_T(h, t, id, op, eq)) : HI.Above(_, n)
      HX.Both{Equal.trans(Bool, Nat.is_gt(HM.MRec.id(HM.rec_op(h, op)), n), Nat.is_gt(HM.MRec.id(h), n), True{}, PC1.hc_gt_op(h, op, n), l), at}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> HM.map_msg(t, id, op), PC1.hc_map_hd_F(h, t, id, op, eq)) : HI.Above(_, n)
      HX.Both{l, ih}

def hc_above_map(+xs: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +n: Nat, a: HI.Above(xs, n)) -> HI.Above(HM.map_msg(xs, id, op), n):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match a:
        case HX.Both{+l, +at}:
          hc_above_map.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, op, n, {==}, l, at, hc_above_map(t, id, op, n, at))

def hc_asc_map.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    ah: HI.Above(t, HM.MRec.id(h)), at: HI.Asc(t), ih: HI.Asc(HM.map_msg(t, id, op))) -> HI.Asc(HM.map_msg(h <> t, id, op)):
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), HM.rec_op(h, op) <> t, PC1.hc_map_hd_T(h, t, id, op, eq)) : HI.Asc(_)
      HX.Both{hc_above_n(t, HM.MRec.id(HM.rec_op(h, op)), HM.MRec.id(h), PC1.hc_rec_op_id(h, op), ah), at}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> HM.map_msg(t, id, op), PC1.hc_map_hd_F(h, t, id, op, eq)) : HI.Asc(_)
      HX.Both{hc_above_map(t, id, op, HM.MRec.id(h), ah), ih}

def hc_asc_map(+xs: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, a: HI.Asc(xs)) -> HI.Asc(HM.map_msg(xs, id, op)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match a:
        case HX.Both{+ah, +at}:
          hc_asc_map.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, op, {==}, ah, at, hc_asc_map(t, id, op, at))

def hc_desc_map.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    b: HI.Below(HI.ids(t), HM.MRec.id(h)), dt: HI.Desc(t), ih: HI.Desc(HM.map_msg(t, id, op))) -> HI.Desc(HM.map_msg(h <> t, id, op)):
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), HM.rec_op(h, op) <> t, PC1.hc_map_hd_T(h, t, id, op, eq)) : HI.Desc(_)
      HX.Both{PL1.hub_below_n(HI.ids(t), HM.MRec.id(HM.rec_op(h, op)), HM.MRec.id(h), PC1.hc_rec_op_id(h, op), b), dt}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> HM.map_msg(t, id, op), PC1.hc_map_hd_F(h, t, id, op, eq)) : HI.Desc(_)
      HX.Both{PL1.hub_below_tr(HI.ids(HM.map_msg(t, id, op)), HI.ids(t), HM.MRec.id(h), PL1.hub_ids_map(t, id, op), b), ih}

def hc_desc_map(+xs: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, d: HI.Desc(xs)) -> HI.Desc(HM.map_msg(xs, id, op)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match d:
        case HX.Both{+b, +dt}:
          hc_desc_map.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, op, {==}, b, dt, hc_desc_map(t, id, op, dt))

def hc_disj_map.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp,
    +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool}, +jh: {HM.nat_in(HI.ids(os), HM.MRec.id(h)) == False{} : Bool},
    jt: HI.Disjoint(t, os), ih: HI.Disjoint(HM.map_msg(t, id, op), os)) -> HI.Disjoint(HM.map_msg(h <> t, id, op), os):
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), HM.rec_op(h, op) <> t, PC1.hc_map_hd_T(h, t, id, op, eq)) : HI.Disjoint(_, os)
      HX.Both{hc_nin_n(HI.ids(os), HM.MRec.id(HM.rec_op(h, op)), HM.MRec.id(h), PC1.hc_rec_op_id(h, op), jh), jt}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> HM.map_msg(t, id, op), PC1.hc_map_hd_F(h, t, id, op, eq)) : HI.Disjoint(_, os)
      HX.Both{jh, ih}

def hc_disj_map(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, j: HI.Disjoint(hs, os))
    -> HI.Disjoint(HM.map_msg(hs, id, op), os):
  match hs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match j:
        case HX.Both{+jh, +jt}:
          hc_disj_map.go(Nat.is_eq(HM.MRec.id(h), id), h, t, os, id, op, {==}, jh, jt, hc_disj_map(t, os, id, op, jt))

# the same ids on the other side: still apart
def hc_disj_tr(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +os2: List<&2, HS.MRec>, +e: {HI.ids(os2) == HI.ids(os) : List<&2, Nat>},
    j: HI.Disjoint(hs, os)) -> HI.Disjoint(hs, os2):
  match hs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match j:
        case HX.Both{jh, jt}:
          HX.Both{Equal.trans(Bool, HM.nat_in(HI.ids(os2), HM.MRec.id(h)), HM.nat_in(HI.ids(os), HM.MRec.id(h)), False{},
            Equal.cong(List<&2, Nat>, Bool, w => HM.nat_in(w, HM.MRec.id(h)), HI.ids(os2), HI.ids(os), e), jh), hc_disj_tr(t, os, os2, e, jt)}

# ---- cold ones ----

def hc_allcold_ins.go(g: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, +e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool},
    +ch: {HM.MRec.is_hot(h) == False{} : Bool}, ct: HI.AllCold(t), +cr: {HM.MRec.is_hot(r) == False{} : Bool}, ih: HI.AllCold(HM.ins_desc(t, r)))
    -> HI.AllCold(HM.ins_desc(h <> t, r)):
  match g:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), PC1.hc_ins_T(h, t, r, e)) : HI.AllCold(_)
      HX.Both{ch, ih}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), r <> h <> t, PC1.hc_ins_F(h, t, r, e)) : HI.AllCold(_)
      HX.Both{cr, HX.Both{ch, ct}}

def hc_allcold_ins(+xs: List<&2, HS.MRec>, +r: HS.MRec, c: HI.AllCold(xs), +cr: {HM.MRec.is_hot(r) == False{} : Bool})
    -> HI.AllCold(HM.ins_desc(xs, r)):
  match xs:
    case Nil{}:
      HX.Both{cr, Unit{}}
    case Con{h, t}:
      match c:
        case HX.Both{+ch, +ct}:
          hc_allcold_ins.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), h, t, r, {==}, ch, ct, cr, hc_allcold_ins(t, r, ct, cr))

def hc_allcold_all(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, ch: HI.AllCold(hs), co: HI.AllCold(os)) -> HI.AllCold(HM.ins_all(hs, os)):
  match hs:
    case Nil{}:
      co
    case Con{h, t}:
      match ch:
        case HX.Both{x, ct}:
          hc_allcold_ins(HM.ins_all(t, os), h, hc_allcold_all(t, os, ct, co), x)

def hc_allcold_cold.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == c : Bool}, ih: HI.AllCold(HM.cold_of(t)))
    -> HI.AllCold(HM.cold_of(x <> t)):
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), HM.cold_of(t), PC1.hc_cold_T(x, t, e)) : HI.AllCold(_)
      ih
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), x <> HM.cold_of(t), PC1.hc_cold_F(x, t, e)) : HI.AllCold(_)
      HX.Both{e, ih}

def hc_allcold_cold(+xs: List<&2, HS.MRec>) -> HI.AllCold(HM.cold_of(xs)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      hc_allcold_cold.go(HM.MRec.is_hot(x), x, t, {==}, hc_allcold_cold(t))

def hc_nin_cold.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +n: Nat, +e: {HM.MRec.is_hot(x) == c : Bool},
    +a: {HM.nat_in(HI.ids(x <> t), n) == False{} : Bool}, ih: {HM.nat_in(HI.ids(HM.cold_of(t)), n) == False{} : Bool})
    -> {HM.nat_in(HI.ids(HM.cold_of(x <> t)), n) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), HM.cold_of(t), PC1.hc_cold_T(x, t, e)) : {HM.nat_in(HI.ids(_), n) == False{} : Bool}
      ih
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), x <> HM.cold_of(t), PC1.hc_cold_F(x, t, e)) : {HM.nat_in(HI.ids(_), n) == False{} : Bool}
      PL1.hub_lor_false(Nat.is_eq(HM.MRec.id(x), n), HM.nat_in(HI.ids(HM.cold_of(t)), n), PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(x), n), HM.nat_in(HI.ids(t), n), a), ih)

def hc_nin_cold(+xs: List<&2, HS.MRec>, +n: Nat, +a: {HM.nat_in(HI.ids(xs), n) == False{} : Bool}) -> {HM.nat_in(HI.ids(HM.cold_of(xs)), n) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, t}:
      hc_nin_cold.go(HM.MRec.is_hot(x), x, t, n, {==}, a, hc_nin_cold(t, n, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(x), n), HM.nat_in(HI.ids(t), n), a)))

def hc_below_cold.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +n: Nat, +e: {HM.MRec.is_hot(x) == c : Bool},
    +l: {Nat.is_lt(HM.MRec.id(x), n) == True{} : Bool}, ih: HI.Below(HI.ids(HM.cold_of(t)), n))
    -> HI.Below(HI.ids(HM.cold_of(x <> t)), n):
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), HM.cold_of(t), PC1.hc_cold_T(x, t, e)) : HI.Below(HI.ids(_), n)
      ih
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), x <> HM.cold_of(t), PC1.hc_cold_F(x, t, e)) : HI.Below(HI.ids(_), n)
      HX.Both{l, ih}

def hc_below_cold(+xs: List<&2, HS.MRec>, +n: Nat, b: HI.Below(HI.ids(xs), n)) -> HI.Below(HI.ids(HM.cold_of(xs)), n):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match b:
        case HX.Both{l, bt}:
          hc_below_cold.go(HM.MRec.is_hot(x), x, t, n, {==}, l, hc_below_cold(t, n, bt))

def hc_above_cold.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +n: Nat, +e: {HM.MRec.is_hot(x) == c : Bool},
    +l: {Nat.is_gt(HM.MRec.id(x), n) == True{} : Bool}, ih: HI.Above(HM.cold_of(t), n))
    -> HI.Above(HM.cold_of(x <> t), n):
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), HM.cold_of(t), PC1.hc_cold_T(x, t, e)) : HI.Above(_, n)
      ih
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), x <> HM.cold_of(t), PC1.hc_cold_F(x, t, e)) : HI.Above(_, n)
      HX.Both{l, ih}

def hc_above_cold(+xs: List<&2, HS.MRec>, +n: Nat, a: HI.Above(xs, n)) -> HI.Above(HM.cold_of(xs), n):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match a:
        case HX.Both{l, at}:
          hc_above_cold.go(HM.MRec.is_hot(x), x, t, n, {==}, l, hc_above_cold(t, n, at))

def hc_asc_cold.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == c : Bool},
    ax: HI.Above(t, HM.MRec.id(x)), ih: HI.Asc(HM.cold_of(t))) -> HI.Asc(HM.cold_of(x <> t)):
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), HM.cold_of(t), PC1.hc_cold_T(x, t, e)) : HI.Asc(_)
      ih
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), x <> HM.cold_of(t), PC1.hc_cold_F(x, t, e)) : HI.Asc(_)
      HX.Both{hc_above_cold(t, HM.MRec.id(x), ax), ih}

def hc_asc_cold(+xs: List<&2, HS.MRec>, a: HI.Asc(xs)) -> HI.Asc(HM.cold_of(xs)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match a:
        case HX.Both{ax, at}:
          hc_asc_cold.go(HM.MRec.is_hot(x), x, t, {==}, ax, hc_asc_cold(t, at))

def hc_disj_cold.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == c : Bool},
    +jx: {HM.nat_in(HI.ids(os), HM.MRec.id(x)) == False{} : Bool}, ih: HI.Disjoint(HM.cold_of(t), os)) -> HI.Disjoint(HM.cold_of(x <> t), os):
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), HM.cold_of(t), PC1.hc_cold_T(x, t, e)) : HI.Disjoint(_, os)
      ih
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), x <> HM.cold_of(t), PC1.hc_cold_F(x, t, e)) : HI.Disjoint(_, os)
      HX.Both{jx, ih}

def hc_disj_cold(+xs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, j: HI.Disjoint(xs, os)) -> HI.Disjoint(HM.cold_of(xs), os):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match j:
        case HX.Both{jx, jt}:
          hc_disj_cold.go(HM.MRec.is_hot(x), x, t, os, {==}, jx, hc_disj_cold(t, os, jt))

# x is none of hs': hs stays apart from ys with x put in
def hc_disj_ins(+hs: List<&2, HS.MRec>, +ys: List<&2, HS.MRec>, +x: HS.MRec, j: HI.Disjoint(hs, ys),
    +nx: {HM.nat_in(HI.ids(hs), HM.MRec.id(x)) == False{} : Bool}) -> HI.Disjoint(hs, HM.ins_desc(ys, x)):
  match hs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match j:
        case HX.Both{jh, jt}:
          +ne = Equal.trans(Bool, Nat.is_eq(HM.MRec.id(x), HM.MRec.id(h)), Nat.is_eq(HM.MRec.id(h), HM.MRec.id(x)), False{},
            PL1.hub_eq_sym(HM.MRec.id(x), HM.MRec.id(h)),
            PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(x)), HM.nat_in(HI.ids(t), HM.MRec.id(x)), nx))
          HX.Both{PC1.hc_nin_ins(ys, x, HM.MRec.id(h), jh, ne),
            hc_disj_ins(t, ys, x, jt, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(x)), HM.nat_in(HI.ids(t), HM.MRec.id(x)), nx))}

# a list's hot ones stay apart from its cold ones put among the old ones
def hc_disj_hot_cold.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == c : Bool},
    ax: HI.Above(t, HM.MRec.id(x)), +jx: {HM.nat_in(HI.ids(os), HM.MRec.id(x)) == False{} : Bool},
    ih: HI.Disjoint(HM.hot_of(t), HM.ins_all(HM.cold_of(t), os))) -> HI.Disjoint(HM.hot_of(x <> t), HM.ins_all(HM.cold_of(x <> t), os)):
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), x <> HM.hot_of(t), PC1.hc_hot_T(x, t, e)) : HI.Disjoint(_, HM.ins_all(HM.cold_of(x <> t), os))
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), HM.cold_of(t), PC1.hc_cold_T(x, t, e)) : HI.Disjoint(x <> HM.hot_of(t), HM.ins_all(_, os))
      HX.Both{PC1.hc_nin_all(HM.cold_of(t), os, HM.MRec.id(x), hc_nin_cold(t, HM.MRec.id(x), PC1.hc_above_nin(t, HM.MRec.id(x), ax)), jx), ih}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), HM.hot_of(t), PC1.hc_hot_F(x, t, e)) : HI.Disjoint(_, HM.ins_all(HM.cold_of(x <> t), os))
      %Equal.sym(List<&2, HS.MRec>, HM.cold_of(x <> t), x <> HM.cold_of(t), PC1.hc_cold_F(x, t, e)) : HI.Disjoint(HM.hot_of(t), HM.ins_all(_, os))
      hc_disj_ins(HM.hot_of(t), HM.ins_all(HM.cold_of(t), os), x, ih, PC1.hc_nin_hot(t, HM.MRec.id(x), PC1.hc_above_nin(t, HM.MRec.id(x), ax)))

def hc_disj_hot_cold(+xs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, a: HI.Asc(xs), j: HI.Disjoint(xs, os))
    -> HI.Disjoint(HM.hot_of(xs), HM.ins_all(HM.cold_of(xs), os)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match a j:
        case HX.Both{+ax, +at} HX.Both{+jx, +jt}:
          hc_disj_hot_cold.go(HM.MRec.is_hot(x), x, t, os, {==}, ax, jx, hc_disj_hot_cold(t, os, at, jt))

# ---- a record back to the hot side (ids ascending) ----

def hc_gt_trans(+a: Nat, +b: Nat, +c: Nat, ab: {Nat.is_gt(a, b) == True{} : Bool}, bc: {Nat.is_gt(b, c) == True{} : Bool})
    -> {Nat.is_gt(a, c) == True{} : Bool}:
  match a b c:
    case 0n 0n _:
      AB.absurd_eq(AB.ft_absurd(ab), Bool, Nat.is_gt(a, c), True{})
    case 0n 1n+bp _:
      AB.absurd_eq(AB.ft_absurd(ab), Bool, Nat.is_gt(a, c), True{})
    case 1n+ap 0n 0n:
      AB.absurd_eq(AB.ft_absurd(bc), Bool, Nat.is_gt(a, c), True{})
    case 1n+ap 0n 1n+cp:
      AB.absurd_eq(AB.ft_absurd(bc), Bool, Nat.is_gt(a, c), True{})
    case 1n+ap 1n+bp 0n:
      {==}
    case 1n+ap 1n+bp 1n+cp:
      hc_gt_trans(ap, bp, cp, ab, bc)

def hc_above_mono(+xs: List<&2, HS.MRec>, +m: Nat, +n: Nat, a: HI.Above(xs, m), +g: {Nat.is_gt(m, n) == True{} : Bool}) -> HI.Above(xs, n):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match a:
        case HX.Both{l, at}:
          HX.Both{hc_gt_trans(HM.MRec.id(x), m, n, l, g), hc_above_mono(t, m, n, at, g)}

def hc_asc_T(+h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == True{} : Bool})
    -> {HM.ins_asc(h <> t, r) == r <> h <> t : List<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), True{}, e) : {HM.ins_asc.go(h <> t, _, r) == r <> h <> t : List<&2, HS.MRec>}
  {==}

def hc_asc_F(+h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == False{} : Bool})
    -> {HM.ins_asc(h <> t, r) == h <> HM.ins_asc(t, r) : List<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), False{}, e) : {HM.ins_asc.go(h <> t, _, r) == h <> HM.ins_asc(t, r) : List<&2, HS.MRec>}
  {==}

def hc_all_asc.go(g: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +r: HS.MRec,
    +e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool}, +ne: {Nat.is_eq(HM.MRec.id(r), HM.MRec.id(h)) == False{} : Bool},
    ih: {HM.ins_all(HM.ins_asc(t, r), os) == HM.ins_desc(HM.ins_all(t, os), r) : List<&2, HS.MRec>})
    -> {HM.ins_all(HM.ins_asc(h <> t, r), os) == HM.ins_desc(HM.ins_all(h <> t, os), r) : List<&2, HS.MRec>}:
  match g:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_asc(h <> t, r), r <> h <> t, hc_asc_T(h, t, r, e)) : {HM.ins_all(_, os) == HM.ins_desc(HM.ins_all(h <> t, os), r) : List<&2, HS.MRec>}
      {==}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_asc(h <> t, r), h <> HM.ins_asc(t, r), hc_asc_F(h, t, r, e)) : {HM.ins_all(_, os) == HM.ins_desc(HM.ins_all(h <> t, os), r) : List<&2, HS.MRec>}
      Equal.trans(List<&2, HS.MRec>, HM.ins_desc(HM.ins_all(HM.ins_asc(t, r), os), h), HM.ins_desc(HM.ins_desc(HM.ins_all(t, os), r), h),
        HM.ins_desc(HM.ins_desc(HM.ins_all(t, os), h), r),
        Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => HM.ins_desc(y, h), HM.ins_all(HM.ins_asc(t, r), os), HM.ins_desc(HM.ins_all(t, os), r), ih),
        PC1.hc_ins_comm(HM.ins_all(t, os), r, h, ne))

# r back on the hot side, all put in = r put among all of them
def hc_all_asc(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +r: HS.MRec, +nr: {HM.nat_in(HI.ids(hs), HM.MRec.id(r)) == False{} : Bool})
    -> {HM.ins_all(HM.ins_asc(hs, r), os) == HM.ins_desc(HM.ins_all(hs, os), r) : List<&2, HS.MRec>}:
  match hs:
    case Nil{}:
      {==}
    case Con{h, t}:
      +ne = Equal.trans(Bool, Nat.is_eq(HM.MRec.id(r), HM.MRec.id(h)), Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), False{},
        PL1.hub_eq_sym(HM.MRec.id(r), HM.MRec.id(h)),
        PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), HM.nat_in(HI.ids(t), HM.MRec.id(r)), nr))
      hc_all_asc.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), h, t, os, r, {==}, ne,
        hc_all_asc(t, os, r, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), HM.nat_in(HI.ids(t), HM.MRec.id(r)), nr)))

def hc_above_asc.go(g: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, +n: Nat, +e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool},
    +l: {Nat.is_gt(HM.MRec.id(h), n) == True{} : Bool}, at: HI.Above(t, n), +gr: {Nat.is_gt(HM.MRec.id(r), n) == True{} : Bool},
    ih: HI.Above(HM.ins_asc(t, r), n)) -> HI.Above(HM.ins_asc(h <> t, r), n):
  match g:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_asc(h <> t, r), r <> h <> t, hc_asc_T(h, t, r, e)) : HI.Above(_, n)
      HX.Both{gr, HX.Both{l, at}}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_asc(h <> t, r), h <> HM.ins_asc(t, r), hc_asc_F(h, t, r, e)) : HI.Above(_, n)
      HX.Both{l, ih}

def hc_above_asc(+xs: List<&2, HS.MRec>, +r: HS.MRec, +n: Nat, a: HI.Above(xs, n), +gr: {Nat.is_gt(HM.MRec.id(r), n) == True{} : Bool})
    -> HI.Above(HM.ins_asc(xs, r), n):
  match xs:
    case Nil{}:
      HX.Both{gr, Unit{}}
    case Con{h, t}:
      match a:
        case HX.Both{+l, +at}:
          hc_above_asc.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), h, t, r, n, {==}, l, at, gr, hc_above_asc(t, r, n, at, gr))

def hc_asc_ins.go(g: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, +e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool},
    +ah: HI.Above(t, HM.MRec.id(h)), at: HI.Asc(t), +ne: {Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)) == False{} : Bool},
    ih: HI.Asc(HM.ins_asc(t, r))) -> HI.Asc(HM.ins_asc(h <> t, r)):
  match g:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_asc(h <> t, r), r <> h <> t, hc_asc_T(h, t, r, e)) : HI.Asc(_)
      HX.Both{HX.Both{e, hc_above_mono(t, HM.MRec.id(h), HM.MRec.id(r), ah, e)}, HX.Both{ah, at}}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_asc(h <> t, r), h <> HM.ins_asc(t, r), hc_asc_F(h, t, r, e)) : HI.Asc(_)
      HX.Both{hc_above_asc(t, r, HM.MRec.id(h), ah, PC1.hc_lt_gt(HM.MRec.id(h), HM.MRec.id(r), PC1.hc_le_ne_lt(HM.MRec.id(h), HM.MRec.id(r), e, ne))), ih}

# r (none of hs' ids) back on the hot side: still ascending
def hc_asc_ins(+hs: List<&2, HS.MRec>, +r: HS.MRec, a: HI.Asc(hs), +nr: {HM.nat_in(HI.ids(hs), HM.MRec.id(r)) == False{} : Bool})
    -> HI.Asc(HM.ins_asc(hs, r)):
  match hs:
    case Nil{}:
      HX.Both{Unit{}, Unit{}}
    case Con{h, t}:
      match a:
        case HX.Both{+ah, +at}:
          hc_asc_ins.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), h, t, r, {==}, ah, at,
            PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), HM.nat_in(HI.ids(t), HM.MRec.id(r)), nr),
            hc_asc_ins(t, r, at, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), HM.nat_in(HI.ids(t), HM.MRec.id(r)), nr)))

def hc_below_asc.go(g: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, +n: Nat, +e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool},
    +l: {Nat.is_lt(HM.MRec.id(h), n) == True{} : Bool}, bt: HI.Below(HI.ids(t), n), +lr: {Nat.is_lt(HM.MRec.id(r), n) == True{} : Bool},
    ih: HI.Below(HI.ids(HM.ins_asc(t, r)), n)) -> HI.Below(HI.ids(HM.ins_asc(h <> t, r)), n):
  match g:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_asc(h <> t, r), r <> h <> t, hc_asc_T(h, t, r, e)) : HI.Below(HI.ids(_), n)
      HX.Both{lr, HX.Both{l, bt}}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_asc(h <> t, r), h <> HM.ins_asc(t, r), hc_asc_F(h, t, r, e)) : HI.Below(HI.ids(_), n)
      HX.Both{l, ih}

def hc_below_asc(+xs: List<&2, HS.MRec>, +r: HS.MRec, +n: Nat, b: HI.Below(HI.ids(xs), n), +lr: {Nat.is_lt(HM.MRec.id(r), n) == True{} : Bool})
    -> HI.Below(HI.ids(HM.ins_asc(xs, r)), n):
  match xs:
    case Nil{}:
      HX.Both{lr, Unit{}}
    case Con{h, t}:
      match b:
        case HX.Both{+l, +bt}:
          hc_below_asc.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), h, t, r, n, {==}, l, bt, lr, hc_below_asc(t, r, n, bt, lr))

def hc_disj_asc.go(g: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +r: HS.MRec,
    +e: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool}, +jh: {HM.nat_in(HI.ids(os), HM.MRec.id(h)) == False{} : Bool},
    jt: HI.Disjoint(t, os), +nr: {HM.nat_in(HI.ids(os), HM.MRec.id(r)) == False{} : Bool}, ih: HI.Disjoint(HM.ins_asc(t, r), os))
    -> HI.Disjoint(HM.ins_asc(h <> t, r), os):
  match g:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_asc(h <> t, r), r <> h <> t, hc_asc_T(h, t, r, e)) : HI.Disjoint(_, os)
      HX.Both{nr, HX.Both{jh, jt}}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_asc(h <> t, r), h <> HM.ins_asc(t, r), hc_asc_F(h, t, r, e)) : HI.Disjoint(_, os)
      HX.Both{jh, ih}

def hc_disj_asc(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +r: HS.MRec, j: HI.Disjoint(hs, os),
    +nr: {HM.nat_in(HI.ids(os), HM.MRec.id(r)) == False{} : Bool}) -> HI.Disjoint(HM.ins_asc(hs, r), os):
  match hs:
    case Nil{}:
      HX.Both{nr, Unit{}}
    case Con{h, t}:
      match j:
        case HX.Both{+jh, +jt}:
          hc_disj_asc.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), h, t, os, r, {==}, jh, jt, nr, hc_disj_asc(t, os, r, jt, nr))

# ---- a message event's op on the split: the op on every message, the split kept ----

# the two sides after the op: all put in = the op on all put in, and the split holds
def hc_OpOk(+x: HM.Ms, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +id: Nat, +op: HM.MsgOp) -> Data:
  HX.Both<{HM.ins_all(HM.Ms.hot(x), HM.Ms.old(x)) == HM.map_msg(HM.ins_all(hot, old), id, op) : List<&2, HS.MRec>},
    HI.Parts(HM.Ms.hot(x), HM.Ms.old(x), nm)>

# the op left a cold record cold: it stays cold
def hc_allcold_map(+old: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +c: HS.MRec, d: HI.Desc(old), co: HI.AllCold(old),
    +f: {HM.rec_of(old, id) == Some{c} : Maybe<&2, HS.MRec>}, +eb: {HM.MRec.is_hot(HM.rec_op(c, op)) == False{} : Bool})
    -> HI.AllCold(HM.map_msg(old, id, op)):
  %PC2.hc_drop_reins(old, id, op, c, d, f) : HI.AllCold(_)
  hc_allcold_ins(HM.drop_msg(old, id), HM.rec_op(c, op), PC2.hc_cold_drop(old, id, co), eb)

def hc_op.back(b: Bool, +c: HS.MRec, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +id: Nat, +op: HM.MsgOp,
    +eb: {HM.MRec.is_hot(HM.rec_op(c, op)) == b : Bool}, +f: {HM.rec_of(old, id) == Some{c} : Maybe<&2, HS.MRec>},
    +nh: {HM.nat_in(HI.ids(hot), id) == False{} : Bool}, +p: HI.Parts(hot, old, nm))
    -> hc_OpOk(HM.msg_op.cold.go(b, c, hot, old, id, op), hot, old, nm, id, op):
  match b:
    case True{}:
    +ah = PC2.hc_p_asc(hot, old, nm, p)
    +dold = PC2.hc_p_desc(hot, old, nm, p)
    +co = PC2.hc_p_cold(hot, old, nm, p)
    +j = PC2.hc_p_disj(hot, old, nm, p)
    +bh = PC2.hc_p_bh(hot, old, nm, p)
    +bo = PC2.hc_p_bo(hot, old, nm, p)
    +r = HM.rec_op(c, op)
    +rid = Equal.trans(Nat, HM.MRec.id(r), HM.MRec.id(c), id, PC1.hc_rec_op_id(c, op), PC2.hc_found_eq(old, id, c, f))
    +nr = hc_nin_n(HI.ids(hot), HM.MRec.id(r), id, rid, nh)
      +dr = HM.drop_msg(old, id)
      HX.Both{
        Equal.trans(List<&2, HS.MRec>, HM.ins_all(HM.ins_asc(hot, r), dr), HM.ins_desc(HM.ins_all(hot, dr), r), HM.map_msg(HM.ins_all(hot, old), id, op),
          hc_all_asc(hot, dr, r, nr),
          Equal.trans(List<&2, HS.MRec>, HM.ins_desc(HM.ins_all(hot, dr), r), HM.ins_all(hot, HM.ins_desc(dr, r)), HM.map_msg(HM.ins_all(hot, old), id, op),
            Equal.sym(List<&2, HS.MRec>, HM.ins_all(hot, HM.ins_desc(dr, r)), HM.ins_desc(HM.ins_all(hot, dr), r), PC1.hc_all_ins(hot, dr, r, nr)),
            Equal.trans(List<&2, HS.MRec>, HM.ins_all(hot, HM.ins_desc(dr, r)), HM.ins_all(hot, HM.map_msg(old, id, op)), HM.map_msg(HM.ins_all(hot, old), id, op),
              Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => HM.ins_all(hot, w), HM.ins_desc(dr, r), HM.map_msg(old, id, op), PC2.hc_drop_reins(old, id, op, c, dold, f)),
              PC1.hc_map_all_old(hot, old, id, op, nh)))),
        HX.Both{hc_asc_ins(hot, r, ah, nr),
          HX.Both{PC2.hc_desc_drop(old, id, dold),
            HX.Both{PC2.hc_cold_drop(old, id, co),
              HX.Both{hc_disj_asc(hot, dr, r, PC2.hc_drop_disj(hot, old, id, j), hc_nin_n(HI.ids(dr), HM.MRec.id(r), id, rid, PC2.hc_drop_gone(old, id, dold))),
                HX.Both{hc_below_asc(hot, r, nm, bh,
                    Equal.trans(Bool, Nat.is_lt(HM.MRec.id(r), nm), Nat.is_lt(HM.MRec.id(c), nm), True{},
                      Equal.cong(Nat, Bool, x => Nat.is_lt(x, nm), HM.MRec.id(r), HM.MRec.id(c), PC1.hc_rec_op_id(c, op)),
                      PC2.hc_found_below(old, id, nm, c, bo, f))),
                  PC2.hc_below_drop(old, id, nm, bo)}}}}}}
    case False{}:
    +ah = PC2.hc_p_asc(hot, old, nm, p)
    +dold = PC2.hc_p_desc(hot, old, nm, p)
    +co = PC2.hc_p_cold(hot, old, nm, p)
    +j = PC2.hc_p_disj(hot, old, nm, p)
    +bh = PC2.hc_p_bh(hot, old, nm, p)
    +bo = PC2.hc_p_bo(hot, old, nm, p)
    +r = HM.rec_op(c, op)
    +rid = Equal.trans(Nat, HM.MRec.id(r), HM.MRec.id(c), id, PC1.hc_rec_op_id(c, op), PC2.hc_found_eq(old, id, c, f))
    +nr = hc_nin_n(HI.ids(hot), HM.MRec.id(r), id, rid, nh)
      HX.Both{PC1.hc_map_all_old(hot, old, id, op, nh),
        HX.Both{ah,
          HX.Both{hc_desc_map(old, id, op, dold),
            HX.Both{hc_allcold_map(old, id, op, c, dold, co, f, eb),
              HX.Both{hc_disj_tr(hot, old, HM.map_msg(old, id, op), PL1.hub_ids_map(old, id, op), j),
                HX.Both{bh, PL1.hub_below_tr(HI.ids(HM.map_msg(old, id, op)), HI.ids(old), nm, PL1.hub_ids_map(old, id, op), bo)}}}}}}

def hc_op.cold(m: Maybe<&2, HS.MRec>, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +id: Nat, +op: HM.MsgOp,
    +em: {HM.rec_of(old, id) == m : Maybe<&2, HS.MRec>}, +nh: {HM.nat_in(HI.ids(hot), id) == False{} : Bool}, +p: HI.Parts(hot, old, nm))
    -> hc_OpOk(HM.msg_op.cold(m, hot, old, id, op), hot, old, nm, id, op):
  match m:
    case None{}:
      HX.Both{Equal.sym(List<&2, HS.MRec>, HM.map_msg(HM.ins_all(hot, old), id, op), HM.ins_all(hot, old),
        PC1.hc_map_miss(HM.ins_all(hot, old), id, op, PC1.hc_nin_all(hot, old, id, nh, PC2.hc_none_nin(old, id, em)))), p}
    case Some{+c}:
      hc_op.back(HM.MRec.is_hot(HM.rec_op(c, op)), c, hot, old, nm, id, op, {==}, em, nh, p)

def hc_op.in(m: Maybe<&2, HS.MRec>, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +id: Nat, +op: HM.MsgOp,
    +em: {HM.rec_of(hot, id) == m : Maybe<&2, HS.MRec>}, +p: HI.Parts(hot, old, nm))
    -> hc_OpOk(HM.msg_op.in(Maybe.is_some(&2, HS.MRec, m), hot, old, id, op), hot, old, nm, id, op):
  match m:
    case None{}:
      hc_op.cold(HM.rec_of(old, id), hot, old, nm, id, op, {==}, PC2.hc_none_nin(hot, id, em), p)
    case Some{+c}:
      +ah = PC2.hc_p_asc(hot, old, nm, p)
      +j = PC2.hc_p_disj(hot, old, nm, p)
      +hm = HM.map_msg(hot, id, op)
      +am = hc_asc_map(hot, id, op, ah)
      +jm = hc_disj_map(hot, old, id, op, j)
      +bm = PL1.hub_below_tr(HI.ids(hm), HI.ids(hot), nm, PL1.hub_ids_map(hot, id, op), PC2.hc_p_bh(hot, old, nm, p))
      HX.Both{
        Equal.trans(List<&2, HS.MRec>, HM.ins_all(HM.hot_of(hm), HM.ins_all(HM.cold_of(hm), old)), HM.ins_all(hm, old), HM.map_msg(HM.ins_all(hot, old), id, op),
          PC1.hc_part(hm, old, am), PC1.hc_map_all_hot(hot, old, id, op, ah, PC2.hc_disj_found(hot, old, id, c, j, em))),
        HX.Both{PC2.hc_asc_hot(hm, am),
          HX.Both{PC1.hc_desc_all(HM.cold_of(hm), old, hc_asc_cold(hm, am), PC2.hc_p_desc(hot, old, nm, p), hc_disj_cold(hm, old, jm)),
            HX.Both{hc_allcold_all(HM.cold_of(hm), old, hc_allcold_cold(hm), PC2.hc_p_cold(hot, old, nm, p)),
              HX.Both{hc_disj_hot_cold(hm, old, am, jm),
                HX.Both{PC2.hc_below_hot(hm, nm, bm), PC1.hc_below_all(HM.cold_of(hm), old, nm, hc_below_cold(hm, nm, bm), PC2.hc_p_bo(hot, old, nm, p))}}}}}}

# the op on the split: the op on every message, and the split holds after it
def hc_op(+hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +id: Nat, +op: HM.MsgOp, +p: HI.Parts(hot, old, nm))
    -> hc_OpOk(HM.msg_op(hot, old, id, op), hot, old, nm, id, op):
  hc_op.in(HM.rec_of(hot, id), hot, old, nm, id, op, {==}, p)
