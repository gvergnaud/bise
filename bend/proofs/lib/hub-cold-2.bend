# bend/proofs/lib/hub-cold-2.bend: the lemmas of the split messages (issue
# 21 steps 3-4: St.hot ids ascending, St.old ids descending, every = the
# hot ones put among the old ones, turned around). Used by the laws
# split_*, ids_ascending, cold_is_inert and the walks' hot_* in
# bend/proofs/hub.bend. Part 2: the hot side of every message, finding and dropping a record, the op on an old record.
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

# ---- the hot ones of every message are the hot side ----

def hc_below_hot.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +n: Nat, +e: {HM.MRec.is_hot(x) == c : Bool},
    +l: {Nat.is_lt(HM.MRec.id(x), n) == True{} : Bool}, ih: HI.Below(HI.ids(HM.hot_of(t)), n))
    -> HI.Below(HI.ids(HM.hot_of(x <> t)), n):
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), x <> HM.hot_of(t), PC1.hc_hot_T(x, t, e)) : HI.Below(HI.ids(_), n)
      HX.Both{l, ih}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), HM.hot_of(t), PC1.hc_hot_F(x, t, e)) : HI.Below(HI.ids(_), n)
      ih

def hc_below_hot(+xs: List<&2, HS.MRec>, +n: Nat, b: HI.Below(HI.ids(xs), n)) -> HI.Below(HI.ids(HM.hot_of(xs)), n):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match b:
        case HX.Both{l, bt}:
          hc_below_hot.go(HM.MRec.is_hot(x), x, t, n, {==}, l, hc_below_hot(t, n, bt))

def hc_above_hot.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +n: Nat, +e: {HM.MRec.is_hot(x) == c : Bool},
    +l: {Nat.is_gt(HM.MRec.id(x), n) == True{} : Bool}, ih: HI.Above(HM.hot_of(t), n))
    -> HI.Above(HM.hot_of(x <> t), n):
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), x <> HM.hot_of(t), PC1.hc_hot_T(x, t, e)) : HI.Above(_, n)
      HX.Both{l, ih}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), HM.hot_of(t), PC1.hc_hot_F(x, t, e)) : HI.Above(_, n)
      ih

def hc_above_hot(+xs: List<&2, HS.MRec>, +n: Nat, a: HI.Above(xs, n)) -> HI.Above(HM.hot_of(xs), n):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match a:
        case HX.Both{l, at}:
          hc_above_hot.go(HM.MRec.is_hot(x), x, t, n, {==}, l, hc_above_hot(t, n, at))

def hc_asc_hot.go(c: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(x) == c : Bool},
    ax: HI.Above(t, HM.MRec.id(x)), ih: HI.Asc(HM.hot_of(t))) -> HI.Asc(HM.hot_of(x <> t)):
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), x <> HM.hot_of(t), PC1.hc_hot_T(x, t, e)) : HI.Asc(_)
      HX.Both{hc_above_hot(t, HM.MRec.id(x), ax), ih}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(x <> t), HM.hot_of(t), PC1.hc_hot_F(x, t, e)) : HI.Asc(_)
      ih

def hc_asc_hot(+xs: List<&2, HS.MRec>, a: HI.Asc(xs)) -> HI.Asc(HM.hot_of(xs)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match a:
        case HX.Both{ax, at}:
          hc_asc_hot.go(HM.MRec.is_hot(x), x, t, {==}, ax, hc_asc_hot(t, at))

def hc_hot_cold(+xs: List<&2, HS.MRec>, c: HI.AllCold(xs)) -> {HM.hot_of(xs) == Nil{} : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, t}:
      match c:
        case HX.Both{cx, ct}:
          Equal.trans(List<&2, HS.MRec>, HM.hot_of(x <> t), HM.hot_of(t), Nil{}, PC1.hc_hot_F(x, t, cx), hc_hot_cold(t, ct))

# a cold one put in: the hot ones do not change
def hc_hot_ins_F.go(g: Bool, c: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec,
    +eg: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool}, +ec: {HM.MRec.is_hot(h) == c : Bool}, +hr: {HM.MRec.is_hot(r) == False{} : Bool},
    ih: {HM.hot_of(HM.ins_desc(t, r)) == HM.hot_of(t) : List<&2, HS.MRec>})
    -> {HM.hot_of(HM.ins_desc(h <> t, r)) == HM.hot_of(h <> t) : List<&2, HS.MRec>}:
  match g:
    case True{}:
      match c:
        case True{}:
          %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), PC1.hc_ins_T(h, t, r, eg)) : {HM.hot_of(_) == HM.hot_of(h <> t) : List<&2, HS.MRec>}
          %Equal.sym(List<&2, HS.MRec>, HM.hot_of(h <> HM.ins_desc(t, r)), h <> HM.hot_of(HM.ins_desc(t, r)), PC1.hc_hot_T(h, HM.ins_desc(t, r), ec)) : {_ == HM.hot_of(h <> t) : List<&2, HS.MRec>}
          %Equal.sym(List<&2, HS.MRec>, HM.hot_of(h <> t), h <> HM.hot_of(t), PC1.hc_hot_T(h, t, ec)) : {h <> HM.hot_of(HM.ins_desc(t, r)) == _ : List<&2, HS.MRec>}
          Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => h <> y, HM.hot_of(HM.ins_desc(t, r)), HM.hot_of(t), ih)
        case False{}:
          %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), PC1.hc_ins_T(h, t, r, eg)) : {HM.hot_of(_) == HM.hot_of(h <> t) : List<&2, HS.MRec>}
          %Equal.sym(List<&2, HS.MRec>, HM.hot_of(h <> HM.ins_desc(t, r)), HM.hot_of(HM.ins_desc(t, r)), PC1.hc_hot_F(h, HM.ins_desc(t, r), ec)) : {_ == HM.hot_of(h <> t) : List<&2, HS.MRec>}
          %Equal.sym(List<&2, HS.MRec>, HM.hot_of(h <> t), HM.hot_of(t), PC1.hc_hot_F(h, t, ec)) : {HM.hot_of(HM.ins_desc(t, r)) == _ : List<&2, HS.MRec>}
          ih
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), r <> h <> t, PC1.hc_ins_F(h, t, r, eg)) : {HM.hot_of(_) == HM.hot_of(h <> t) : List<&2, HS.MRec>}
      PC1.hc_hot_F(r, h <> t, hr)

def hc_hot_ins_F(+xs: List<&2, HS.MRec>, +r: HS.MRec, +hr: {HM.MRec.is_hot(r) == False{} : Bool})
    -> {HM.hot_of(HM.ins_desc(xs, r)) == HM.hot_of(xs) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      PC1.hc_hot_F(r, Nil{}, hr)
    case Con{h, t}:
      hc_hot_ins_F.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), HM.MRec.is_hot(h), h, t, r, {==}, {==}, hr, hc_hot_ins_F(t, r, hr))

# a hot one put in a descending list: put in among its hot ones
def hc_hot_ins_T.go(g: Bool, c: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec,
    +eg: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool}, +ec: {HM.MRec.is_hot(h) == c : Bool}, +hr: {HM.MRec.is_hot(r) == True{} : Bool},
    +bt: HI.Below(HI.ids(t), HM.MRec.id(h)), +nh: {Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)) == False{} : Bool},
    ih: {HM.hot_of(HM.ins_desc(t, r)) == HM.ins_desc(HM.hot_of(t), r) : List<&2, HS.MRec>})
    -> {HM.hot_of(HM.ins_desc(h <> t, r)) == HM.ins_desc(HM.hot_of(h <> t), r) : List<&2, HS.MRec>}:
  match g:
    case True{}:
      match c:
        case True{}:
          %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), PC1.hc_ins_T(h, t, r, eg)) : {HM.hot_of(_) == HM.ins_desc(HM.hot_of(h <> t), r) : List<&2, HS.MRec>}
          %Equal.sym(List<&2, HS.MRec>, HM.hot_of(h <> HM.ins_desc(t, r)), h <> HM.hot_of(HM.ins_desc(t, r)), PC1.hc_hot_T(h, HM.ins_desc(t, r), ec)) : {_ == HM.ins_desc(HM.hot_of(h <> t), r) : List<&2, HS.MRec>}
          %Equal.sym(List<&2, HS.MRec>, HM.hot_of(h <> t), h <> HM.hot_of(t), PC1.hc_hot_T(h, t, ec)) : {h <> HM.hot_of(HM.ins_desc(t, r)) == HM.ins_desc(_, r) : List<&2, HS.MRec>}
          %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> HM.hot_of(t), r), h <> HM.ins_desc(HM.hot_of(t), r), PC1.hc_ins_T(h, HM.hot_of(t), r, eg)) : {h <> HM.hot_of(HM.ins_desc(t, r)) == _ : List<&2, HS.MRec>}
          Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => h <> y, HM.hot_of(HM.ins_desc(t, r)), HM.ins_desc(HM.hot_of(t), r), ih)
        case False{}:
          %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), PC1.hc_ins_T(h, t, r, eg)) : {HM.hot_of(_) == HM.ins_desc(HM.hot_of(h <> t), r) : List<&2, HS.MRec>}
          %Equal.sym(List<&2, HS.MRec>, HM.hot_of(h <> HM.ins_desc(t, r)), HM.hot_of(HM.ins_desc(t, r)), PC1.hc_hot_F(h, HM.ins_desc(t, r), ec)) : {_ == HM.ins_desc(HM.hot_of(h <> t), r) : List<&2, HS.MRec>}
          %Equal.sym(List<&2, HS.MRec>, HM.hot_of(h <> t), HM.hot_of(t), PC1.hc_hot_F(h, t, ec)) : {HM.hot_of(HM.ins_desc(t, r)) == HM.ins_desc(_, r) : List<&2, HS.MRec>}
          ih
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), r <> h <> t, PC1.hc_ins_F(h, t, r, eg)) : {HM.hot_of(_) == HM.ins_desc(HM.hot_of(h <> t), r) : List<&2, HS.MRec>}
      +lt = PC1.hc_le_ne_lt(HM.MRec.id(h), HM.MRec.id(r), eg, nh)
      +b = {HX.Both{lt, PL1.hub_below_mono(HI.ids(t), HM.MRec.id(h), HM.MRec.id(r), PC1.hc_lt_ge(HM.MRec.id(h), HM.MRec.id(r), lt), bt)} : HI.Below(HI.ids(h <> t), HM.MRec.id(r))}
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(r <> h <> t), r <> HM.hot_of(h <> t), PC1.hc_hot_T(r, h <> t, hr)) : {_ == HM.ins_desc(HM.hot_of(h <> t), r) : List<&2, HS.MRec>}
      Equal.sym(List<&2, HS.MRec>, HM.ins_desc(HM.hot_of(h <> t), r), r <> HM.hot_of(h <> t), PC1.hc_ins_head(HM.hot_of(h <> t), r, hc_below_hot(h <> t, HM.MRec.id(r), b)))

def hc_hot_ins_T(+xs: List<&2, HS.MRec>, +r: HS.MRec, d: HI.Desc(xs), +nr: {HM.nat_in(HI.ids(xs), HM.MRec.id(r)) == False{} : Bool},
    +hr: {HM.MRec.is_hot(r) == True{} : Bool}) -> {HM.hot_of(HM.ins_desc(xs, r)) == HM.ins_desc(HM.hot_of(xs), r) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      PC1.hc_hot_T(r, Nil{}, hr)
    case Con{h, t}:
      match d:
        case HX.Both{bt, dt}:
          hc_hot_ins_T.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), HM.MRec.is_hot(h), h, t, r, {==}, {==}, hr, bt,
            PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), HM.nat_in(HI.ids(t), HM.MRec.id(r)), nr),
            hc_hot_ins_T(t, r, dt, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), HM.MRec.id(r)), HM.nat_in(HI.ids(t), HM.MRec.id(r)), nr), hr))

def hc_hot_all.go(c: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(h) == c : Bool},
    d: HI.Desc(HM.ins_all(t, os)), +nh: {HM.nat_in(HI.ids(HM.ins_all(t, os)), HM.MRec.id(h)) == False{} : Bool},
    ih: {HM.hot_of(HM.ins_all(t, os)) == HM.ins_all(HM.hot_of(t), HM.hot_of(os)) : List<&2, HS.MRec>})
    -> {HM.hot_of(HM.ins_all(h <> t, os)) == HM.ins_all(HM.hot_of(h <> t), HM.hot_of(os)) : List<&2, HS.MRec>}:
  match c:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(h <> t), h <> HM.hot_of(t), PC1.hc_hot_T(h, t, e)) : {HM.hot_of(HM.ins_all(h <> t, os)) == HM.ins_all(_, HM.hot_of(os)) : List<&2, HS.MRec>}
      Equal.trans(List<&2, HS.MRec>, HM.hot_of(HM.ins_desc(HM.ins_all(t, os), h)), HM.ins_desc(HM.hot_of(HM.ins_all(t, os)), h),
        HM.ins_desc(HM.ins_all(HM.hot_of(t), HM.hot_of(os)), h),
        hc_hot_ins_T(HM.ins_all(t, os), h, d, nh, e),
        Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => HM.ins_desc(y, h), HM.hot_of(HM.ins_all(t, os)), HM.ins_all(HM.hot_of(t), HM.hot_of(os)), ih))
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.hot_of(h <> t), HM.hot_of(t), PC1.hc_hot_F(h, t, e)) : {HM.hot_of(HM.ins_all(h <> t, os)) == HM.ins_all(_, HM.hot_of(os)) : List<&2, HS.MRec>}
      Equal.trans(List<&2, HS.MRec>, HM.hot_of(HM.ins_desc(HM.ins_all(t, os), h)), HM.hot_of(HM.ins_all(t, os)), HM.ins_all(HM.hot_of(t), HM.hot_of(os)),
        hc_hot_ins_F(HM.ins_all(t, os), h, e), ih)

# the hot ones of the two sides put together: the two sides' hot ones put together
def hc_hot_all(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, a: HI.Asc(hs), +d: HI.Desc(os), j: HI.Disjoint(hs, os))
    -> {HM.hot_of(HM.ins_all(hs, os)) == HM.ins_all(HM.hot_of(hs), HM.hot_of(os)) : List<&2, HS.MRec>}:
  match hs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match a j:
        case HX.Both{+ah, +at} HX.Both{+jh, +jt}:
          hc_hot_all.go(HM.MRec.is_hot(h), h, t, os, {==}, PC1.hc_desc_all(t, os, at, d, jt),
            PC1.hc_nin_all(t, os, HM.MRec.id(h), PC1.hc_above_nin(t, HM.MRec.id(h), ah), jh), hc_hot_all(t, os, at, d, jt))

# an ascending list put in a nothing, turned around: itself
def hc_rev_asc(+ys: List<&2, HS.MRec>, a: HI.Asc(ys)) -> {List.reverse(&2, HS.MRec, HM.ins_all(ys, Nil{})) == ys : List<&2, HS.MRec>}:
  match ys:
    case Nil{}:
      {==}
    case Con{y, t}:
      match a:
        case HX.Both{ay, at}:
          +z = HM.ins_all(t, Nil{})
          Equal.trans(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, HM.ins_desc(z, y)), List.reverse(&2, HS.MRec, HM.snoc(HS.MRec, z, y)), y <> t,
            Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => List.reverse(&2, HS.MRec, w), HM.ins_desc(z, y), HM.snoc(HS.MRec, z, y),
              PC1.hc_ins_last(z, y, PC1.hc_above_all(t, Nil{}, HM.MRec.id(y), ay, Unit{}))),
            Equal.trans(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, HM.snoc(HS.MRec, z, y)), y <> List.reverse(&2, HS.MRec, z), y <> t,
              PC1.hc_rev_snoc(z, y),
              Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => y <> w, List.reverse(&2, HS.MRec, z), t, hc_rev_asc(t, at))))

# the two sides' invariant, by part
def hc_p_asc(-hs: List<&2, HS.MRec>, -os: List<&2, HS.MRec>, -nm: Nat, p: HI.Parts(hs, os, nm)) -> HI.Asc(hs):
  match p:
    case HX.Both{a, r}:
      a

def hc_p_desc(-hs: List<&2, HS.MRec>, -os: List<&2, HS.MRec>, -nm: Nat, p: HI.Parts(hs, os, nm)) -> HI.Desc(os):
  match p:
    case HX.Both{a, r}:
      match r:
        case HX.Both{d, r2}:
          d

def hc_p_cold(-hs: List<&2, HS.MRec>, -os: List<&2, HS.MRec>, -nm: Nat, p: HI.Parts(hs, os, nm)) -> HI.AllCold(os):
  match p:
    case HX.Both{a, r}:
      match r:
        case HX.Both{d, r2}:
          match r2:
            case HX.Both{c, r3}:
              c

def hc_p_disj(-hs: List<&2, HS.MRec>, -os: List<&2, HS.MRec>, -nm: Nat, p: HI.Parts(hs, os, nm)) -> HI.Disjoint(hs, os):
  match p:
    case HX.Both{a, r}:
      match r:
        case HX.Both{d, r2}:
          match r2:
            case HX.Both{c, r3}:
              match r3:
                case HX.Both{j, r4}:
                  j

def hc_p_bh(-hs: List<&2, HS.MRec>, -os: List<&2, HS.MRec>, -nm: Nat, p: HI.Parts(hs, os, nm)) -> HI.Below(HI.ids(hs), nm):
  match p:
    case HX.Both{a, r}:
      match r:
        case HX.Both{d, r2}:
          match r2:
            case HX.Both{c, r3}:
              match r3:
                case HX.Both{j, r4}:
                  match r4:
                    case HX.Both{bh, bo}:
                      bh

def hc_p_bo(-hs: List<&2, HS.MRec>, -os: List<&2, HS.MRec>, -nm: Nat, p: HI.Parts(hs, os, nm)) -> HI.Below(HI.ids(os), nm):
  match p:
    case HX.Both{a, r}:
      match r:
        case HX.Both{d, r2}:
          match r2:
            case HX.Both{c, r3}:
              match r3:
                case HX.Both{j, r4}:
                  match r4:
                    case HX.Both{bh, bo}:
                      bo

# the hot ones of every message (the two sides together, in send order)
# are the hot side's: what the walks read (laws hot_queued_for...)
def hc_hot_every(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, -nm: Nat, +p: HI.Parts(hs, os, nm))
    -> {HM.hot_of(List.reverse(&2, HS.MRec, HM.ins_all(hs, os))) == HM.hot_of(hs) : List<&2, HS.MRec>}:
  +x = HM.ins_all(hs, os)
  Equal.trans(List<&2, HS.MRec>, HM.hot_of(List.reverse(&2, HS.MRec, x)), List.reverse(&2, HS.MRec, HM.hot_of(x)), HM.hot_of(hs),
    PC1.hc_hot_rev(x),
    Equal.trans(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, HM.hot_of(x)), List.reverse(&2, HS.MRec, HM.ins_all(HM.hot_of(hs), Nil{})), HM.hot_of(hs),
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => List.reverse(&2, HS.MRec, w), HM.hot_of(x), HM.ins_all(HM.hot_of(hs), Nil{}),
        Equal.trans(List<&2, HS.MRec>, HM.hot_of(x), HM.ins_all(HM.hot_of(hs), HM.hot_of(os)), HM.ins_all(HM.hot_of(hs), Nil{}),
          hc_hot_all(hs, os, hc_p_asc(hs, os, nm, p), hc_p_desc(hs, os, nm, p), hc_p_disj(hs, os, nm, p)),
          Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => HM.ins_all(HM.hot_of(hs), w), HM.hot_of(os), Nil{}, hc_hot_cold(os, hc_p_cold(hs, os, nm, p))))),
      hc_rev_asc(HM.hot_of(hs), hc_asc_hot(hs, hc_p_asc(hs, os, nm, p)))))

# ---- finding a record by its id ----

def hc_rec_hd_T(+h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +e: {Nat.is_eq(HM.MRec.id(h), id) == True{} : Bool})
    -> {HM.rec_of(h <> t, id) == Some{h} : Maybe<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id), True{}, e) : {HLZ.pick(Maybe<&2, HS.MRec>, _, u => Some{h}, u => HM.rec_of(t, id)) == Some{h} : Maybe<&2, HS.MRec>}
  {==}

def hc_rec_hd_F(+h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +e: {Nat.is_eq(HM.MRec.id(h), id) == False{} : Bool})
    -> {HM.rec_of(h <> t, id) == HM.rec_of(t, id) : Maybe<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id), False{}, e) : {HLZ.pick(Maybe<&2, HS.MRec>, _, u => Some{h}, u => HM.rec_of(t, id)) == HM.rec_of(t, id) : Maybe<&2, HS.MRec>}
  {==}

def hc_rec_miss(+xs: List<&2, HS.MRec>, +id: Nat, +a: {HM.nat_in(HI.ids(xs), id) == False{} : Bool}) -> {HM.rec_of(xs, id) == None{} : Maybe<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), HM.rec_of(t, id), None{},
        hc_rec_hd_F(h, t, id, PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(t), id), a)),
        hc_rec_miss(t, id, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(t), id), a)))

def hc_rec_ins_ne.go(g: Bool, q: Bool, +y: HS.MRec, +t: List<&2, HS.MRec>, +h: HS.MRec, +id: Nat,
    +eg: {Nat.is_gt(HM.MRec.id(y), HM.MRec.id(h)) == g : Bool}, +eq: {Nat.is_eq(HM.MRec.id(y), id) == q : Bool},
    +ne: {Nat.is_eq(HM.MRec.id(h), id) == False{} : Bool}, ih: {HM.rec_of(HM.ins_desc(t, h), id) == HM.rec_of(t, id) : Maybe<&2, HS.MRec>})
    -> {HM.rec_of(HM.ins_desc(y <> t, h), id) == HM.rec_of(y <> t, id) : Maybe<&2, HS.MRec>}:
  match g q:
    case True{} True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), y <> HM.ins_desc(t, h), PC1.hc_ins_T(y, t, h, eg)) : {HM.rec_of(_, id) == HM.rec_of(y <> t, id) : Maybe<&2, HS.MRec>}
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(y <> HM.ins_desc(t, h), id), Some{y}, HM.rec_of(y <> t, id),
        hc_rec_hd_T(y, HM.ins_desc(t, h), id, eq), Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(y <> t, id), Some{y}, hc_rec_hd_T(y, t, id, eq)))
    case True{} False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), y <> HM.ins_desc(t, h), PC1.hc_ins_T(y, t, h, eg)) : {HM.rec_of(_, id) == HM.rec_of(y <> t, id) : Maybe<&2, HS.MRec>}
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(y <> HM.ins_desc(t, h), id), HM.rec_of(HM.ins_desc(t, h), id), hc_rec_hd_F(y, HM.ins_desc(t, h), id, eq)) : {_ == HM.rec_of(y <> t, id) : Maybe<&2, HS.MRec>}
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(y <> t, id), HM.rec_of(t, id), hc_rec_hd_F(y, t, id, eq)) : {HM.rec_of(HM.ins_desc(t, h), id) == _ : Maybe<&2, HS.MRec>}
      ih
    case False{} True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), h <> y <> t, PC1.hc_ins_F(y, t, h, eg)) : {HM.rec_of(_, id) == HM.rec_of(y <> t, id) : Maybe<&2, HS.MRec>}
      hc_rec_hd_F(h, y <> t, id, ne)
    case False{} False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), h <> y <> t, PC1.hc_ins_F(y, t, h, eg)) : {HM.rec_of(_, id) == HM.rec_of(y <> t, id) : Maybe<&2, HS.MRec>}
      hc_rec_hd_F(h, y <> t, id, ne)

def hc_rec_ins_ne(+xs: List<&2, HS.MRec>, +h: HS.MRec, +id: Nat, +ne: {Nat.is_eq(HM.MRec.id(h), id) == False{} : Bool})
    -> {HM.rec_of(HM.ins_desc(xs, h), id) == HM.rec_of(xs, id) : Maybe<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      hc_rec_hd_F(h, Nil{}, id, ne)
    case Con{y, t}:
      hc_rec_ins_ne.go(Nat.is_gt(HM.MRec.id(y), HM.MRec.id(h)), Nat.is_eq(HM.MRec.id(y), id), y, t, h, id, {==}, {==}, ne, hc_rec_ins_ne(t, h, id, ne))

def hc_rec_ins_hit.go(g: Bool, +y: HS.MRec, +t: List<&2, HS.MRec>, +h: HS.MRec,
    +eg: {Nat.is_gt(HM.MRec.id(y), HM.MRec.id(h)) == g : Bool}, +ny: {Nat.is_eq(HM.MRec.id(y), HM.MRec.id(h)) == False{} : Bool},
    ih: {HM.rec_of(HM.ins_desc(t, h), HM.MRec.id(h)) == Some{h} : Maybe<&2, HS.MRec>})
    -> {HM.rec_of(HM.ins_desc(y <> t, h), HM.MRec.id(h)) == Some{h} : Maybe<&2, HS.MRec>}:
  match g:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), y <> HM.ins_desc(t, h), PC1.hc_ins_T(y, t, h, eg)) : {HM.rec_of(_, HM.MRec.id(h)) == Some{h} : Maybe<&2, HS.MRec>}
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(y <> HM.ins_desc(t, h), HM.MRec.id(h)), HM.rec_of(HM.ins_desc(t, h), HM.MRec.id(h)), Some{h},
        hc_rec_hd_F(y, HM.ins_desc(t, h), HM.MRec.id(h), ny), ih)
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(y <> t, h), h <> y <> t, PC1.hc_ins_F(y, t, h, eg)) : {HM.rec_of(_, HM.MRec.id(h)) == Some{h} : Maybe<&2, HS.MRec>}
      hc_rec_hd_T(h, y <> t, HM.MRec.id(h), PL1.hub_eq_refl(HM.MRec.id(h)))

def hc_rec_ins_hit(+xs: List<&2, HS.MRec>, +h: HS.MRec, +a: {HM.nat_in(HI.ids(xs), HM.MRec.id(h)) == False{} : Bool})
    -> {HM.rec_of(HM.ins_desc(xs, h), HM.MRec.id(h)) == Some{h} : Maybe<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      hc_rec_hd_T(h, Nil{}, HM.MRec.id(h), PL1.hub_eq_refl(HM.MRec.id(h)))
    case Con{y, t}:
      hc_rec_ins_hit.go(Nat.is_gt(HM.MRec.id(y), HM.MRec.id(h)), y, t, h, {==},
        PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(y), HM.MRec.id(h)), HM.nat_in(HI.ids(t), HM.MRec.id(h)), a),
        hc_rec_ins_hit(t, h, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(y), HM.MRec.id(h)), HM.nat_in(HI.ids(t), HM.MRec.id(h)), a)))

def hc_rec_all.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat,
    +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool}, +nh: {HM.nat_in(HI.ids(HM.ins_all(t, os)), HM.MRec.id(h)) == False{} : Bool},
    ih: {HM.rec_of(HM.ins_all(t, os), id) == HM.rec_in(t, os, id) : Maybe<&2, HS.MRec>})
    -> {HM.rec_of(HM.ins_all(h <> t, os), id) == HM.rec_in(h <> t, os, id) : Maybe<&2, HS.MRec>}:
  match q:
    case True{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), Some{h}, hc_rec_hd_T(h, t, id, eq)) : {HM.rec_of(HM.ins_all(h <> t, os), id) == HM.rec_in.go(_, os, id) : Maybe<&2, HS.MRec>}
      %PC1.hc_eq_nat(HM.MRec.id(h), id, eq) : {HM.rec_of(HM.ins_desc(HM.ins_all(t, os), h), _) == Some{h} : Maybe<&2, HS.MRec>}
      hc_rec_ins_hit(HM.ins_all(t, os), h, nh)
    case False{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), HM.rec_of(t, id), hc_rec_hd_F(h, t, id, eq)) : {HM.rec_of(HM.ins_all(h <> t, os), id) == HM.rec_in.go(_, os, id) : Maybe<&2, HS.MRec>}
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.ins_desc(HM.ins_all(t, os), h), id), HM.rec_of(HM.ins_all(t, os), id), HM.rec_in(t, os, id),
        hc_rec_ins_ne(HM.ins_all(t, os), h, id, eq), ih)

# a record found among the two sides put together: the one rec_in finds
def hc_rec_all(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat, a: HI.Asc(hs), j: HI.Disjoint(hs, os))
    -> {HM.rec_of(HM.ins_all(hs, os), id) == HM.rec_in(hs, os, id) : Maybe<&2, HS.MRec>}:
  match hs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match a j:
        case HX.Both{ah, at} HX.Both{jh, jt}:
          hc_rec_all.go(Nat.is_eq(HM.MRec.id(h), id), h, t, os, id, {==},
            PC1.hc_nin_all(t, os, HM.MRec.id(h), PC1.hc_above_nin(t, HM.MRec.id(h), ah), jh), hc_rec_all(t, os, id, at, jt))

# ---- turned around: the same records found, the same op ----

def hc_nin_rev(+xs: List<&2, HS.MRec>, +n: Nat, +a: {HM.nat_in(HI.ids(xs), n) == False{} : Bool})
    -> {HM.nat_in(HI.ids(List.reverse(&2, HS.MRec, xs)), n) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, t}:
      +r = List.reverse(&2, HS.MRec, t)
      %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> t), HM.snoc(HS.MRec, r, x), PC1.hc_rev_cons(t, x)) : {HM.nat_in(HI.ids(_), n) == False{} : Bool}
      %Equal.sym(List<&2, Nat>, HI.ids(HM.snoc(HS.MRec, r, x)), HM.snoc(Nat, HI.ids(r), HM.MRec.id(x)), PL1.hub_ids_snoc(r, x)) : {HM.nat_in(_, n) == False{} : Bool}
      PL1.hub_nat_in_snoc(HI.ids(r), HM.MRec.id(x), n,
        hc_nin_rev(t, n, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(x), n), HM.nat_in(HI.ids(t), n), a)),
        PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(x), n), HM.nat_in(HI.ids(t), n), a))

def hc_rec_snoc_ne.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +x: HS.MRec, +id: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    ih: {HM.rec_of(HM.snoc(HS.MRec, t, x), id) == HM.rec_of(t, id) : Maybe<&2, HS.MRec>})
    -> {HM.rec_of(h <> HM.snoc(HS.MRec, t, x), id) == HM.rec_of(h <> t, id) : Maybe<&2, HS.MRec>}:
  match q:
    case True{}:
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(h <> HM.snoc(HS.MRec, t, x), id), Some{h}, HM.rec_of(h <> t, id),
        hc_rec_hd_T(h, HM.snoc(HS.MRec, t, x), id, eq), Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), Some{h}, hc_rec_hd_T(h, t, id, eq)))
    case False{}:
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(h <> HM.snoc(HS.MRec, t, x), id), HM.rec_of(HM.snoc(HS.MRec, t, x), id), HM.rec_of(h <> t, id),
        hc_rec_hd_F(h, HM.snoc(HS.MRec, t, x), id, eq),
        Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.snoc(HS.MRec, t, x), id), HM.rec_of(t, id), HM.rec_of(h <> t, id), ih,
          Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), HM.rec_of(t, id), hc_rec_hd_F(h, t, id, eq))))

def hc_rec_snoc_ne(+xs: List<&2, HS.MRec>, +x: HS.MRec, +id: Nat, +ne: {Nat.is_eq(HM.MRec.id(x), id) == False{} : Bool})
    -> {HM.rec_of(HM.snoc(HS.MRec, xs, x), id) == HM.rec_of(xs, id) : Maybe<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      hc_rec_hd_F(x, Nil{}, id, ne)
    case Con{h, t}:
      hc_rec_snoc_ne.go(Nat.is_eq(HM.MRec.id(h), id), h, t, x, id, {==}, hc_rec_snoc_ne(t, x, id, ne))

def hc_rec_snoc_hit(+xs: List<&2, HS.MRec>, +x: HS.MRec, +id: Nat, +eq: {Nat.is_eq(HM.MRec.id(x), id) == True{} : Bool},
    +a: {HM.nat_in(HI.ids(xs), id) == False{} : Bool}) -> {HM.rec_of(HM.snoc(HS.MRec, xs, x), id) == Some{x} : Maybe<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      hc_rec_hd_T(x, Nil{}, id, eq)
    case Con{h, t}:
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(h <> HM.snoc(HS.MRec, t, x), id), HM.rec_of(HM.snoc(HS.MRec, t, x), id), Some{x},
        hc_rec_hd_F(h, HM.snoc(HS.MRec, t, x), id, PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(t), id), a)),
        hc_rec_snoc_hit(t, x, id, eq, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(t), id), a)))

# n is none of t's ids when every one of them is below n's... record
def hc_desc_nin(+x: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +eq: {Nat.is_eq(HM.MRec.id(x), id) == True{} : Bool},
    b: HI.Below(HI.ids(t), HM.MRec.id(x))) -> {HM.nat_in(HI.ids(t), id) == False{} : Bool}:
  %PC1.hc_eq_nat(HM.MRec.id(x), id, eq) : {HM.nat_in(HI.ids(t), _) == False{} : Bool}
  PC1.hc_below_nin(HI.ids(t), HM.MRec.id(x), b)

def hc_rec_rev.go(q: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +eq: {Nat.is_eq(HM.MRec.id(x), id) == q : Bool},
    b: HI.Below(HI.ids(t), HM.MRec.id(x)), ih: {HM.rec_of(List.reverse(&2, HS.MRec, t), id) == HM.rec_of(t, id) : Maybe<&2, HS.MRec>})
    -> {HM.rec_of(List.reverse(&2, HS.MRec, x <> t), id) == HM.rec_of(x <> t, id) : Maybe<&2, HS.MRec>}:
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> t), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), PC1.hc_rev_cons(t, x)) : {HM.rec_of(_, id) == HM.rec_of(x <> t, id) : Maybe<&2, HS.MRec>}
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), id), Some{x}, HM.rec_of(x <> t, id),
        hc_rec_snoc_hit(List.reverse(&2, HS.MRec, t), x, id, eq, hc_nin_rev(t, id, hc_desc_nin(x, t, id, eq, b))),
        Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(x <> t, id), Some{x}, hc_rec_hd_T(x, t, id, eq)))
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> t), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), PC1.hc_rev_cons(t, x)) : {HM.rec_of(_, id) == HM.rec_of(x <> t, id) : Maybe<&2, HS.MRec>}
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), id), HM.rec_of(List.reverse(&2, HS.MRec, t), id), HM.rec_of(x <> t, id),
        hc_rec_snoc_ne(List.reverse(&2, HS.MRec, t), x, id, eq),
        Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(List.reverse(&2, HS.MRec, t), id), HM.rec_of(t, id), HM.rec_of(x <> t, id), ih,
          Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(x <> t, id), HM.rec_of(t, id), hc_rec_hd_F(x, t, id, eq))))

# a descending list turned around: the same record for an id
def hc_rec_rev(+xs: List<&2, HS.MRec>, +id: Nat, d: HI.Desc(xs)) -> {HM.rec_of(List.reverse(&2, HS.MRec, xs), id) == HM.rec_of(xs, id) : Maybe<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, t}:
      match d:
        case HX.Both{b, dt}:
          hc_rec_rev.go(Nat.is_eq(HM.MRec.id(x), id), x, t, id, {==}, b, hc_rec_rev(t, id, dt))

def hc_map_snoc_ne.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +x: HS.MRec, +id: Nat, +op: HM.MsgOp, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    ih: {HM.map_msg(HM.snoc(HS.MRec, t, x), id, op) == HM.snoc(HS.MRec, HM.map_msg(t, id, op), x) : List<&2, HS.MRec>})
    -> {HM.map_msg(h <> HM.snoc(HS.MRec, t, x), id, op) == HM.snoc(HS.MRec, HM.map_msg(h <> t, id, op), x) : List<&2, HS.MRec>}:
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> HM.snoc(HS.MRec, t, x), id, op), HM.rec_op(h, op) <> HM.snoc(HS.MRec, t, x), PC1.hc_map_hd_T(h, HM.snoc(HS.MRec, t, x), id, op, eq)) : {_ == HM.snoc(HS.MRec, HM.map_msg(h <> t, id, op), x) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), HM.rec_op(h, op) <> t, PC1.hc_map_hd_T(h, t, id, op, eq)) : {HM.rec_op(h, op) <> HM.snoc(HS.MRec, t, x) == HM.snoc(HS.MRec, _, x) : List<&2, HS.MRec>}
      {==}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> HM.snoc(HS.MRec, t, x), id, op), h <> HM.map_msg(HM.snoc(HS.MRec, t, x), id, op), PC1.hc_map_hd_F(h, HM.snoc(HS.MRec, t, x), id, op, eq)) : {_ == HM.snoc(HS.MRec, HM.map_msg(h <> t, id, op), x) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> HM.map_msg(t, id, op), PC1.hc_map_hd_F(h, t, id, op, eq)) : {h <> HM.map_msg(HM.snoc(HS.MRec, t, x), id, op) == HM.snoc(HS.MRec, _, x) : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => h <> y, HM.map_msg(HM.snoc(HS.MRec, t, x), id, op), HM.snoc(HS.MRec, HM.map_msg(t, id, op), x), ih)

def hc_map_snoc_ne(+xs: List<&2, HS.MRec>, +x: HS.MRec, +id: Nat, +op: HM.MsgOp, +ne: {Nat.is_eq(HM.MRec.id(x), id) == False{} : Bool})
    -> {HM.map_msg(HM.snoc(HS.MRec, xs, x), id, op) == HM.snoc(HS.MRec, HM.map_msg(xs, id, op), x) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      PC1.hc_map_hd_F(x, Nil{}, id, op, ne)
    case Con{h, t}:
      hc_map_snoc_ne.go(Nat.is_eq(HM.MRec.id(h), id), h, t, x, id, op, {==}, hc_map_snoc_ne(t, x, id, op, ne))

def hc_map_snoc_hit(+xs: List<&2, HS.MRec>, +x: HS.MRec, +id: Nat, +op: HM.MsgOp, +eq: {Nat.is_eq(HM.MRec.id(x), id) == True{} : Bool},
    +a: {HM.nat_in(HI.ids(xs), id) == False{} : Bool}) -> {HM.map_msg(HM.snoc(HS.MRec, xs, x), id, op) == HM.snoc(HS.MRec, xs, HM.rec_op(x, op)) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      PC1.hc_map_hd_T(x, Nil{}, id, op, eq)
    case Con{h, t}:
      Equal.trans(List<&2, HS.MRec>, HM.map_msg(h <> HM.snoc(HS.MRec, t, x), id, op), h <> HM.map_msg(HM.snoc(HS.MRec, t, x), id, op),
        h <> HM.snoc(HS.MRec, t, HM.rec_op(x, op)),
        PC1.hc_map_hd_F(h, HM.snoc(HS.MRec, t, x), id, op, PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(t), id), a)),
        Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => h <> y, HM.map_msg(HM.snoc(HS.MRec, t, x), id, op), HM.snoc(HS.MRec, t, HM.rec_op(x, op)),
          hc_map_snoc_hit(t, x, id, op, eq, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(t), id), a))))

def hc_map_rev.go(q: Bool, +x: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +eq: {Nat.is_eq(HM.MRec.id(x), id) == q : Bool},
    b: HI.Below(HI.ids(t), HM.MRec.id(x)),
    ih: {HM.map_msg(List.reverse(&2, HS.MRec, t), id, op) == List.reverse(&2, HS.MRec, HM.map_msg(t, id, op)) : List<&2, HS.MRec>})
    -> {HM.map_msg(List.reverse(&2, HS.MRec, x <> t), id, op) == List.reverse(&2, HS.MRec, HM.map_msg(x <> t, id, op)) : List<&2, HS.MRec>}:
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> t), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), PC1.hc_rev_cons(t, x)) : {HM.map_msg(_, id, op) == List.reverse(&2, HS.MRec, HM.map_msg(x <> t, id, op)) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(x <> t, id, op), HM.rec_op(x, op) <> t, PC1.hc_map_hd_T(x, t, id, op, eq)) : {HM.map_msg(HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), id, op) == List.reverse(&2, HS.MRec, _) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, HM.rec_op(x, op) <> t), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), HM.rec_op(x, op)), PC1.hc_rev_cons(t, HM.rec_op(x, op))) : {HM.map_msg(HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), id, op) == _ : List<&2, HS.MRec>}
      hc_map_snoc_hit(List.reverse(&2, HS.MRec, t), x, id, op, eq, hc_nin_rev(t, id, hc_desc_nin(x, t, id, eq, b)))
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> t), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), PC1.hc_rev_cons(t, x)) : {HM.map_msg(_, id, op) == List.reverse(&2, HS.MRec, HM.map_msg(x <> t, id, op)) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(x <> t, id, op), x <> HM.map_msg(t, id, op), PC1.hc_map_hd_F(x, t, id, op, eq)) : {HM.map_msg(HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), id, op) == List.reverse(&2, HS.MRec, _) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> HM.map_msg(t, id, op)), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, HM.map_msg(t, id, op)), x), PC1.hc_rev_cons(HM.map_msg(t, id, op), x)) : {HM.map_msg(HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), id, op) == _ : List<&2, HS.MRec>}
      Equal.trans(List<&2, HS.MRec>, HM.map_msg(HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), id, op), HM.snoc(HS.MRec, HM.map_msg(List.reverse(&2, HS.MRec, t), id, op), x),
        HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, HM.map_msg(t, id, op)), x),
        hc_map_snoc_ne(List.reverse(&2, HS.MRec, t), x, id, op, eq),
        Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, y => HM.snoc(HS.MRec, y, x), HM.map_msg(List.reverse(&2, HS.MRec, t), id, op), List.reverse(&2, HS.MRec, HM.map_msg(t, id, op)), ih))

# a descending list turned around: the op is the same
def hc_map_rev(+xs: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, d: HI.Desc(xs))
    -> {HM.map_msg(List.reverse(&2, HS.MRec, xs), id, op) == List.reverse(&2, HS.MRec, HM.map_msg(xs, id, op)) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, t}:
      match d:
        case HX.Both{b, dt}:
          hc_map_rev.go(Nat.is_eq(HM.MRec.id(x), id), x, t, id, op, {==}, b, hc_map_rev(t, id, op, dt))

# ---- every message: ids ascending, each one record ----

def hc_above_snoc(+xs: List<&2, HS.MRec>, +x: HS.MRec, +n: Nat, a: HI.Above(xs, n), +g: {Nat.is_gt(HM.MRec.id(x), n) == True{} : Bool})
    -> HI.Above(HM.snoc(HS.MRec, xs, x), n):
  match xs:
    case Nil{}:
      HX.Both{g, Unit{}}
    case Con{h, t}:
      match a:
        case HX.Both{gh, at}:
          HX.Both{gh, hc_above_snoc(t, x, n, at, g)}

def hc_asc_snoc(+xs: List<&2, HS.MRec>, +x: HS.MRec, a: HI.Asc(xs), b: HI.Below(HI.ids(xs), HM.MRec.id(x))) -> HI.Asc(HM.snoc(HS.MRec, xs, x)):
  match xs:
    case Nil{}:
      HX.Both{Unit{}, Unit{}}
    case Con{y, t}:
      match a b:
        case HX.Both{ay, at} HX.Both{ly, bt}:
          HX.Both{hc_above_snoc(t, x, HM.MRec.id(y), ay, PC1.hc_lt_gt(HM.MRec.id(y), HM.MRec.id(x), ly)), hc_asc_snoc(t, x, at, bt)}

def hc_below_rev(+xs: List<&2, HS.MRec>, +n: Nat, b: HI.Below(HI.ids(xs), n)) -> HI.Below(HI.ids(List.reverse(&2, HS.MRec, xs)), n):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match b:
        case HX.Both{l, bt}:
          +r = List.reverse(&2, HS.MRec, t)
          %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> t), HM.snoc(HS.MRec, r, x), PC1.hc_rev_cons(t, x)) : HI.Below(HI.ids(_), n)
          %Equal.sym(List<&2, Nat>, HI.ids(HM.snoc(HS.MRec, r, x)), HM.snoc(Nat, HI.ids(r), HM.MRec.id(x)), PL1.hub_ids_snoc(r, x)) : HI.Below(_, n)
          PL1.hub_below_snoc(HI.ids(r), HM.MRec.id(x), n, hc_below_rev(t, n, bt), l)

# a descending list turned around: ascending
def hc_asc_rev(+xs: List<&2, HS.MRec>, d: HI.Desc(xs)) -> HI.Asc(List.reverse(&2, HS.MRec, xs)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match d:
        case HX.Both{+b, dt}:
          %Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, x <> t), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, t), x), PC1.hc_rev_cons(t, x)) : HI.Asc(_)
          hc_asc_snoc(List.reverse(&2, HS.MRec, t), x, hc_asc_rev(t, dt), hc_below_rev(t, HM.MRec.id(x), b))

def hc_asc_distinct(+xs: List<&2, HS.MRec>, a: HI.Asc(xs)) -> HI.Distinct(HI.ids(xs)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      match a:
        case HX.Both{ax, at}:
          HX.Both{PC1.hc_above_nin(t, HM.MRec.id(x), ax), hc_asc_distinct(t, at)}

# every message of the two sides, in send order: ids ascending (law ids_ascending)
def hc_every_asc(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, -nm: Nat, +p: HI.Parts(hs, os, nm))
    -> HI.Asc(List.reverse(&2, HS.MRec, HM.ins_all(hs, os))):
  hc_asc_rev(HM.ins_all(hs, os), PC1.hc_desc_all(hs, os, hc_p_asc(hs, os, nm, p), hc_p_desc(hs, os, nm, p), hc_p_disj(hs, os, nm, p)))

# ...each one record, every id below the next free one (law msgs_unique)
def hc_every_unique(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +nm: Nat, +p: HI.Parts(hs, os, nm))
    -> HX.Both<HI.Distinct(HI.ids(List.reverse(&2, HS.MRec, HM.ins_all(hs, os)))), HI.Below(HI.ids(List.reverse(&2, HS.MRec, HM.ins_all(hs, os))), nm)>:
  HX.Both{hc_asc_distinct(List.reverse(&2, HS.MRec, HM.ins_all(hs, os)), hc_every_asc(hs, os, nm, p)),
    hc_below_rev(HM.ins_all(hs, os), nm, PC1.hc_below_all(hs, os, nm, hc_p_bh(hs, os, nm, p), hc_p_bo(hs, os, nm, p)))}

# ---- the record found by its id (as Maybe facts: no case on a found one) ----

def hc_val(m: Maybe<&2, HS.MRec>, +d: HS.MRec) -> HS.MRec:
  match m:
    case None{}:
      d
    case Some{+x}:
      x

def hc_some_inj(+a: HS.MRec, +b: HS.MRec, e: {Some{a} == Some{b} : Maybe<&2, HS.MRec>}) -> {a == b : HS.MRec}:
  Equal.cong(Maybe<&2, HS.MRec>, HS.MRec, m => hc_val(m, a), Some{a}, Some{b}, e)

def hc_none_some(+c: HS.MRec, e: {None{} == Some{c} : Maybe<&2, HS.MRec>}) -> Empty:
  AB.ft_absurd(Equal.cong(Maybe<&2, HS.MRec>, Bool, m => Maybe.is_some(&2, HS.MRec, m), None{}, Some{c}, e))

def hc_some_none(+c: HS.MRec, e: {Some{c} == None{} : Maybe<&2, HS.MRec>}) -> Empty:
  AB.tf_absurd(Equal.cong(Maybe<&2, HS.MRec>, Bool, m => Maybe.is_some(&2, HS.MRec, m), Some{c}, None{}, e))

# the found record's id is the one asked
def hc_okid(m: Maybe<&2, HS.MRec>, +id: Nat) -> Bool:
  match m:
    case None{}:
      True{}
    case Some{+c}:
      Nat.is_eq(HM.MRec.id(c), id)

def hc_okid_all.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    ih: {hc_okid(HM.rec_of(t, id), id) == True{} : Bool}) -> {hc_okid(HM.rec_of(h <> t, id), id) == True{} : Bool}:
  match q:
    case True{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), Some{h}, hc_rec_hd_T(h, t, id, eq)) : {hc_okid(_, id) == True{} : Bool}
      eq
    case False{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), HM.rec_of(t, id), hc_rec_hd_F(h, t, id, eq)) : {hc_okid(_, id) == True{} : Bool}
      ih

def hc_okid_all(+xs: List<&2, HS.MRec>, +id: Nat) -> {hc_okid(HM.rec_of(xs, id), id) == True{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hc_okid_all.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, {==}, hc_okid_all(t, id))

def hc_found_id(+xs: List<&2, HS.MRec>, +id: Nat, +c: HS.MRec, +f: {HM.rec_of(xs, id) == Some{c} : Maybe<&2, HS.MRec>})
    -> {Nat.is_eq(HM.MRec.id(c), id) == True{} : Bool}:
  Equal.trans(Bool, hc_okid(Some{c}, id), hc_okid(HM.rec_of(xs, id), id), True{},
    Equal.cong(Maybe<&2, HS.MRec>, Bool, m => hc_okid(m, id), Some{c}, HM.rec_of(xs, id), Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(xs, id), Some{c}, f)),
    hc_okid_all(xs, id))

def hc_found_eq(+xs: List<&2, HS.MRec>, +id: Nat, +c: HS.MRec, +f: {HM.rec_of(xs, id) == Some{c} : Maybe<&2, HS.MRec>})
    -> {HM.MRec.id(c) == id : Nat}:
  PC1.hc_eq_nat(HM.MRec.id(c), id, hc_found_id(xs, id, c, f))

# the found record's id is below n when every id is
def hc_oklt(m: Maybe<&2, HS.MRec>, +n: Nat) -> Bool:
  match m:
    case None{}:
      True{}
    case Some{+c}:
      Nat.is_lt(HM.MRec.id(c), n)

def hc_oklt_all.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +n: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    +l: {Nat.is_lt(HM.MRec.id(h), n) == True{} : Bool}, ih: {hc_oklt(HM.rec_of(t, id), n) == True{} : Bool})
    -> {hc_oklt(HM.rec_of(h <> t, id), n) == True{} : Bool}:
  match q:
    case True{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), Some{h}, hc_rec_hd_T(h, t, id, eq)) : {hc_oklt(_, n) == True{} : Bool}
      l
    case False{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), HM.rec_of(t, id), hc_rec_hd_F(h, t, id, eq)) : {hc_oklt(_, n) == True{} : Bool}
      ih

def hc_oklt_all(+xs: List<&2, HS.MRec>, +id: Nat, +n: Nat, b: HI.Below(HI.ids(xs), n)) -> {hc_oklt(HM.rec_of(xs, id), n) == True{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match b:
        case HX.Both{l, bt}:
          hc_oklt_all.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, n, {==}, l, hc_oklt_all(t, id, n, bt))

def hc_found_below(+xs: List<&2, HS.MRec>, +id: Nat, +n: Nat, +c: HS.MRec, b: HI.Below(HI.ids(xs), n),
    +f: {HM.rec_of(xs, id) == Some{c} : Maybe<&2, HS.MRec>}) -> {Nat.is_lt(HM.MRec.id(c), n) == True{} : Bool}:
  Equal.trans(Bool, hc_oklt(Some{c}, n), hc_oklt(HM.rec_of(xs, id), n), True{},
    Equal.cong(Maybe<&2, HS.MRec>, Bool, m => hc_oklt(m, n), Some{c}, HM.rec_of(xs, id), Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(xs, id), Some{c}, f)),
    hc_oklt_all(xs, id, n, b))

# found: a Bool that holds; not found: False
def hc_whenF(m: Maybe<&2, HS.MRec>, +b: Bool) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{_}:
      b

# an id found on the hot side is not on the old side
def hc_disj_found.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat,
    +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool}, +jh: {HM.nat_in(HI.ids(os), HM.MRec.id(h)) == False{} : Bool},
    ih: {hc_whenF(HM.rec_of(t, id), HM.nat_in(HI.ids(os), id)) == False{} : Bool})
    -> {hc_whenF(HM.rec_of(h <> t, id), HM.nat_in(HI.ids(os), id)) == False{} : Bool}:
  match q:
    case True{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), Some{h}, hc_rec_hd_T(h, t, id, eq)) : {hc_whenF(_, HM.nat_in(HI.ids(os), id)) == False{} : Bool}
      %Equal.sym(Nat, id, HM.MRec.id(h), Equal.sym(Nat, HM.MRec.id(h), id, PC1.hc_eq_nat(HM.MRec.id(h), id, eq))) : {HM.nat_in(HI.ids(os), _) == False{} : Bool}
      jh
    case False{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), HM.rec_of(t, id), hc_rec_hd_F(h, t, id, eq)) : {hc_whenF(_, HM.nat_in(HI.ids(os), id)) == False{} : Bool}
      ih

def hc_disj_found.all(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat, j: HI.Disjoint(hs, os))
    -> {hc_whenF(HM.rec_of(hs, id), HM.nat_in(HI.ids(os), id)) == False{} : Bool}:
  match hs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match j:
        case HX.Both{jh, jt}:
          hc_disj_found.go(Nat.is_eq(HM.MRec.id(h), id), h, t, os, id, {==}, jh, hc_disj_found.all(t, os, id, jt))

def hc_disj_found(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat, +c: HS.MRec, j: HI.Disjoint(hs, os),
    +f: {HM.rec_of(hs, id) == Some{c} : Maybe<&2, HS.MRec>}) -> {HM.nat_in(HI.ids(os), id) == False{} : Bool}:
  Equal.trans(Bool, hc_whenF(Some{c}, HM.nat_in(HI.ids(os), id)), hc_whenF(HM.rec_of(hs, id), HM.nat_in(HI.ids(os), id)), False{},
    Equal.cong(Maybe<&2, HS.MRec>, Bool, m => hc_whenF(m, HM.nat_in(HI.ids(os), id)), Some{c}, HM.rec_of(hs, id),
      Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(hs, id), Some{c}, f)),
    hc_disj_found.all(hs, os, id, j))

# not found: the id is none of the list's
def hc_noneF(m: Maybe<&2, HS.MRec>, +b: Bool) -> Bool:
  match m:
    case None{}:
      b
    case Some{_}:
      False{}

def hc_none_nin.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    ih: {hc_noneF(HM.rec_of(t, id), HM.nat_in(HI.ids(t), id)) == False{} : Bool})
    -> {hc_noneF(HM.rec_of(h <> t, id), HM.nat_in(HI.ids(h <> t), id)) == False{} : Bool}:
  match q:
    case True{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), Some{h}, hc_rec_hd_T(h, t, id, eq)) : {hc_noneF(_, HM.nat_in(HI.ids(h <> t), id)) == False{} : Bool}
      {==}
    case False{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), HM.rec_of(t, id), hc_rec_hd_F(h, t, id, eq)) : {hc_noneF(_, HM.nat_in(HI.ids(h <> t), id)) == False{} : Bool}
      %Equal.sym(Bool, Nat.is_eq(HM.MRec.id(h), id), False{}, eq) : {hc_noneF(HM.rec_of(t, id), HLZ.or(_, u => HM.nat_in(HI.ids(t), id))) == False{} : Bool}
      ih

def hc_none_nin.all(+xs: List<&2, HS.MRec>, +id: Nat) -> {hc_noneF(HM.rec_of(xs, id), HM.nat_in(HI.ids(xs), id)) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hc_none_nin.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, {==}, hc_none_nin.all(t, id))

def hc_none_nin(+xs: List<&2, HS.MRec>, +id: Nat, +f: {HM.rec_of(xs, id) == None{} : Maybe<&2, HS.MRec>})
    -> {HM.nat_in(HI.ids(xs), id) == False{} : Bool}:
  Equal.trans(Bool, hc_noneF(None{}, HM.nat_in(HI.ids(xs), id)), hc_noneF(HM.rec_of(xs, id), HM.nat_in(HI.ids(xs), id)), False{},
    Equal.cong(Maybe<&2, HS.MRec>, Bool, m => hc_noneF(m, HM.nat_in(HI.ids(xs), id)), None{}, HM.rec_of(xs, id),
      Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(xs, id), None{}, f)),
    hc_none_nin.all(xs, id))

# ---- dropping a record by its id ----

def hc_drop_T(+h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +e: {Nat.is_eq(HM.MRec.id(h), id) == True{} : Bool})
    -> {HM.drop_msg(h <> t, id) == t : List<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_eq(HM.MRec.id(h), id), True{}, e) : {HLZ.pick(List<&2, HS.MRec>, _, u => t, u => h <> HM.drop_msg(t, id)) == t : List<&2, HS.MRec>}
  {==}

def hc_drop_F(+h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +e: {Nat.is_eq(HM.MRec.id(h), id) == False{} : Bool})
    -> {HM.drop_msg(h <> t, id) == h <> HM.drop_msg(t, id) : List<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_eq(HM.MRec.id(h), id), False{}, e) : {HLZ.pick(List<&2, HS.MRec>, _, u => t, u => h <> HM.drop_msg(t, id)) == h <> HM.drop_msg(t, id) : List<&2, HS.MRec>}
  {==}

def hc_nin_drop.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +n: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    +a: {HM.nat_in(HI.ids(h <> t), n) == False{} : Bool}, ih: {HM.nat_in(HI.ids(HM.drop_msg(t, id)), n) == False{} : Bool})
    -> {HM.nat_in(HI.ids(HM.drop_msg(h <> t, id)), n) == False{} : Bool}:
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), t, hc_drop_T(h, t, id, eq)) : {HM.nat_in(HI.ids(_), n) == False{} : Bool}
      PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), n), HM.nat_in(HI.ids(t), n), a)
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), h <> HM.drop_msg(t, id), hc_drop_F(h, t, id, eq)) : {HM.nat_in(HI.ids(_), n) == False{} : Bool}
      PL1.hub_lor_false(Nat.is_eq(HM.MRec.id(h), n), HM.nat_in(HI.ids(HM.drop_msg(t, id)), n),
        PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(h), n), HM.nat_in(HI.ids(t), n), a), ih)

def hc_nin_drop(+xs: List<&2, HS.MRec>, +id: Nat, +n: Nat, +a: {HM.nat_in(HI.ids(xs), n) == False{} : Bool})
    -> {HM.nat_in(HI.ids(HM.drop_msg(xs, id)), n) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hc_nin_drop.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, n, {==}, a,
        hc_nin_drop(t, id, n, PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(h), n), HM.nat_in(HI.ids(t), n), a)))

def hc_below_drop.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +n: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    +l: {Nat.is_lt(HM.MRec.id(h), n) == True{} : Bool}, bt: HI.Below(HI.ids(t), n), ih: HI.Below(HI.ids(HM.drop_msg(t, id)), n))
    -> HI.Below(HI.ids(HM.drop_msg(h <> t, id)), n):
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), t, hc_drop_T(h, t, id, eq)) : HI.Below(HI.ids(_), n)
      bt
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), h <> HM.drop_msg(t, id), hc_drop_F(h, t, id, eq)) : HI.Below(HI.ids(_), n)
      HX.Both{l, ih}

def hc_below_drop(+xs: List<&2, HS.MRec>, +id: Nat, +n: Nat, b: HI.Below(HI.ids(xs), n)) -> HI.Below(HI.ids(HM.drop_msg(xs, id)), n):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match b:
        case HX.Both{+l, +bt}:
          hc_below_drop.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, n, {==}, l, bt, hc_below_drop(t, id, n, bt))

def hc_cold_drop.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    +ch: {HM.MRec.is_hot(h) == False{} : Bool}, ct: HI.AllCold(t), ih: HI.AllCold(HM.drop_msg(t, id)))
    -> HI.AllCold(HM.drop_msg(h <> t, id)):
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), t, hc_drop_T(h, t, id, eq)) : HI.AllCold(_)
      ct
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), h <> HM.drop_msg(t, id), hc_drop_F(h, t, id, eq)) : HI.AllCold(_)
      HX.Both{ch, ih}

def hc_cold_drop(+xs: List<&2, HS.MRec>, +id: Nat, c: HI.AllCold(xs)) -> HI.AllCold(HM.drop_msg(xs, id)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match c:
        case HX.Both{+ch, +ct}:
          hc_cold_drop.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, {==}, ch, ct, hc_cold_drop(t, id, ct))

def hc_desc_drop.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    b: HI.Below(HI.ids(t), HM.MRec.id(h)), dt: HI.Desc(t), ih: HI.Desc(HM.drop_msg(t, id)))
    -> HI.Desc(HM.drop_msg(h <> t, id)):
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), t, hc_drop_T(h, t, id, eq)) : HI.Desc(_)
      dt
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), h <> HM.drop_msg(t, id), hc_drop_F(h, t, id, eq)) : HI.Desc(_)
      HX.Both{hc_below_drop(t, id, HM.MRec.id(h), b), ih}

def hc_desc_drop(+xs: List<&2, HS.MRec>, +id: Nat, d: HI.Desc(xs)) -> HI.Desc(HM.drop_msg(xs, id)):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match d:
        case HX.Both{+b, +dt}:
          hc_desc_drop.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, {==}, b, dt, hc_desc_drop(t, id, dt))

# a descending list without the id: the id is gone
def hc_drop_gone.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    b: HI.Below(HI.ids(t), HM.MRec.id(h)), ih: {HM.nat_in(HI.ids(HM.drop_msg(t, id)), id) == False{} : Bool})
    -> {HM.nat_in(HI.ids(HM.drop_msg(h <> t, id)), id) == False{} : Bool}:
  match q:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), t, hc_drop_T(h, t, id, eq)) : {HM.nat_in(HI.ids(_), id) == False{} : Bool}
      hc_desc_nin(h, t, id, eq, b)
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), h <> HM.drop_msg(t, id), hc_drop_F(h, t, id, eq)) : {HM.nat_in(HI.ids(_), id) == False{} : Bool}
      PL1.hub_lor_false(Nat.is_eq(HM.MRec.id(h), id), HM.nat_in(HI.ids(HM.drop_msg(t, id)), id), eq, ih)

def hc_drop_gone(+xs: List<&2, HS.MRec>, +id: Nat, d: HI.Desc(xs)) -> {HM.nat_in(HI.ids(HM.drop_msg(xs, id)), id) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match d:
        case HX.Both{+b, +dt}:
          hc_drop_gone.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, {==}, b, hc_drop_gone(t, id, dt))

def hc_drop_disj(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +id: Nat, j: HI.Disjoint(hs, os)) -> HI.Disjoint(hs, HM.drop_msg(os, id)):
  match hs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match j:
        case HX.Both{jh, jt}:
          HX.Both{hc_nin_drop(os, id, HM.MRec.id(h), jh), hc_drop_disj(t, os, id, jt)}

# ---- the op on an old record = drop it, put its new record back in ----

def hc_reins(m: Maybe<&2, HS.MRec>, +xs: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp) -> List<&2, HS.MRec>:
  match m:
    case None{}:
      xs
    case Some{+c}:
      HM.ins_desc(HM.drop_msg(xs, id), HM.rec_op(c, op))

def hc_reins_t.go(m: Maybe<&2, HS.MRec>, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp,
    +ne: {Nat.is_eq(HM.MRec.id(h), id) == False{} : Bool}, b: HI.Below(HI.ids(t), HM.MRec.id(h)),
    +em: {HM.rec_of(t, id) == m : Maybe<&2, HS.MRec>}, ih: {hc_reins(m, t, id, op) == HM.map_msg(t, id, op) : List<&2, HS.MRec>})
    -> {hc_reins(m, h <> t, id, op) == h <> HM.map_msg(t, id, op) : List<&2, HS.MRec>}:
  match m:
    case None{}:
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => h <> w, t, HM.map_msg(t, id, op), ih)
    case Some{+c}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), h <> HM.drop_msg(t, id), hc_drop_F(h, t, id, ne)) : {HM.ins_desc(_, HM.rec_op(c, op)) == h <> HM.map_msg(t, id, op) : List<&2, HS.MRec>}
      +g = Equal.trans(Bool, Nat.is_gt(HM.MRec.id(h), HM.MRec.id(HM.rec_op(c, op))), Nat.is_gt(HM.MRec.id(h), HM.MRec.id(c)), True{},
        Equal.cong(Nat, Bool, x => Nat.is_gt(HM.MRec.id(h), x), HM.MRec.id(HM.rec_op(c, op)), HM.MRec.id(c), PC1.hc_rec_op_id(c, op)),
        PC1.hc_lt_gt(HM.MRec.id(c), HM.MRec.id(h), hc_found_below(t, id, HM.MRec.id(h), c, b, em)))
      Equal.trans(List<&2, HS.MRec>, HM.ins_desc(h <> HM.drop_msg(t, id), HM.rec_op(c, op)), h <> HM.ins_desc(HM.drop_msg(t, id), HM.rec_op(c, op)),
        h <> HM.map_msg(t, id, op),
        PC1.hc_ins_T(h, HM.drop_msg(t, id), HM.rec_op(c, op), g),
        Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => h <> w, HM.ins_desc(HM.drop_msg(t, id), HM.rec_op(c, op)), HM.map_msg(t, id, op), ih))

def hc_reins.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +eq: {Nat.is_eq(HM.MRec.id(h), id) == q : Bool},
    b: HI.Below(HI.ids(t), HM.MRec.id(h)), ih: {hc_reins(HM.rec_of(t, id), t, id, op) == HM.map_msg(t, id, op) : List<&2, HS.MRec>})
    -> {hc_reins(HM.rec_of(h <> t, id), h <> t, id, op) == HM.map_msg(h <> t, id, op) : List<&2, HS.MRec>}:
  match q:
    case True{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), Some{h}, hc_rec_hd_T(h, t, id, eq)) : {hc_reins(_, h <> t, id, op) == HM.map_msg(h <> t, id, op) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, id), t, hc_drop_T(h, t, id, eq)) : {HM.ins_desc(_, HM.rec_op(h, op)) == HM.map_msg(h <> t, id, op) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), HM.rec_op(h, op) <> t, PC1.hc_map_hd_T(h, t, id, op, eq)) : {HM.ins_desc(t, HM.rec_op(h, op)) == _ : List<&2, HS.MRec>}
      PC1.hc_ins_head(t, HM.rec_op(h, op), PL1.hub_below_n(HI.ids(t), HM.MRec.id(HM.rec_op(h, op)), HM.MRec.id(h), PC1.hc_rec_op_id(h, op), b))
    case False{}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), HM.rec_of(t, id), hc_rec_hd_F(h, t, id, eq)) : {hc_reins(_, h <> t, id, op) == HM.map_msg(h <> t, id, op) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> HM.map_msg(t, id, op), PC1.hc_map_hd_F(h, t, id, op, eq)) : {hc_reins(HM.rec_of(t, id), h <> t, id, op) == _ : List<&2, HS.MRec>}
      hc_reins_t.go(HM.rec_of(t, id), h, t, id, op, eq, b, {==}, ih)

def hc_reins_map(+xs: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, d: HI.Desc(xs))
    -> {hc_reins(HM.rec_of(xs, id), xs, id, op) == HM.map_msg(xs, id, op) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match d:
        case HX.Both{+b, +dt}:
          hc_reins.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, op, {==}, b, hc_reins_map(t, id, op, dt))

# the op on the old record c (found by its id): drop it, put the new one back in
def hc_drop_reins(+xs: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +c: HS.MRec, d: HI.Desc(xs),
    +f: {HM.rec_of(xs, id) == Some{c} : Maybe<&2, HS.MRec>})
    -> {HM.ins_desc(HM.drop_msg(xs, id), HM.rec_op(c, op)) == HM.map_msg(xs, id, op) : List<&2, HS.MRec>}:
  Equal.trans(List<&2, HS.MRec>, hc_reins(Some{c}, xs, id, op), hc_reins(HM.rec_of(xs, id), xs, id, op), HM.map_msg(xs, id, op),
    Equal.cong(Maybe<&2, HS.MRec>, List<&2, HS.MRec>, m => hc_reins(m, xs, id, op), Some{c}, HM.rec_of(xs, id),
      Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(xs, id), Some{c}, f)),
    hc_reins_map(xs, id, op, d))
