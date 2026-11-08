# bend/proofs/cold-store.bend: the proofs of bend/laws/cold-store.bend
# (issue 36 chunk 5a). On the state opened by name (HS.St.open), an event
# that is not a message event computes the same with or without a cold
# message: it never reads St.old. A message event names its message, and
# the needs name it: the other cold ones are left as they were.
import Base
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/gen/state-laws.bend as HSL
import ../hub/inv.bend as HI
import ../hub/cx.bend as HX
import ../hub/lazy.bend as HLZ
import ../hub/needs.bend as N
import ../laws/cold-store.bend as Laws
import ./lib/hub-1.bend as PL1
import ./lib/absurd.bend as AB
import ./lib/hub-cold.bend as PC1
import ./lib/hub-cold-2.bend as PC2
import ./lib/hub-cold-3.bend as PC3
import ./lib/hub-cold-4.bend as PC4

# ---- small facts on ids ----

def cs_eq_sym(+a: Nat, +b: Nat) -> {Nat.is_eq(a, b) == Nat.is_eq(b, a) : Bool}:
  match a b:
    case 0n 0n:
      {==}
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      {==}
    case 1n+ap 1n+bp:
      cs_eq_sym(ap, bp)

# a is b, b is not c: a is not c
def cs_ne(+a: Nat, +b: Nat, +c: Nat, e: {Nat.is_eq(a, b) == True{} : Bool}, n: {Nat.is_eq(b, c) == False{} : Bool})
    -> {Nat.is_eq(a, c) == False{} : Bool}:
  %Equal.sym(Nat, a, b, PC1.hc_eq_nat(a, b, e)) : {Nat.is_eq(_, c) == False{} : Bool}
  n

# a is not above b: b is at or above a
def cs_ngt_ge(+a: Nat, +b: Nat, g: {Nat.is_gt(a, b) == False{} : Bool}) -> {Nat.is_ge(b, a) == True{} : Bool}:
  match a b:
    case 0n 0n:
      {==}
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      AB.absurd_eq(AB.tf_absurd(g), Bool, Nat.is_ge(b, a), True{})
    case 1n+ap 1n+bp:
      cs_ngt_ge(ap, bp, g)

def cs_ge_refl(+n: Nat) -> {Nat.is_ge(n, n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      cs_ge_refl(p)

# ---- the cold message `x` out of a list: the other ops do the same ----

# finding another id
def cs_rec_hd(p: Bool, +h: HS.MRec, +a: List<&2, HS.MRec>, +b: List<&2, HS.MRec>, +id: Nat,
    +ep: {Nat.is_eq(HM.MRec.id(h), id) == p : Bool}, ih: {HM.rec_of(a, id) == HM.rec_of(b, id) : Maybe<&2, HS.MRec>})
    -> {HM.rec_of(h <> a, id) == HM.rec_of(h <> b, id) : Maybe<&2, HS.MRec>}:
  match p:
    case True{}:
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(h <> a, id), Some{h}, HM.rec_of(h <> b, id), PC2.hc_rec_hd_T(h, a, id, ep),
        Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> b, id), Some{h}, PC2.hc_rec_hd_T(h, b, id, ep)))
    case False{}:
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(h <> a, id), HM.rec_of(a, id), HM.rec_of(h <> b, id), PC2.hc_rec_hd_F(h, a, id, ep),
        Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(a, id), HM.rec_of(b, id), HM.rec_of(h <> b, id), ih,
          Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> b, id), HM.rec_of(b, id), PC2.hc_rec_hd_F(h, b, id, ep))))

def cs_rec_drop.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +x: Nat, +id: Nat, +eq: {Nat.is_eq(HM.MRec.id(h), x) == q : Bool},
    +n: {Nat.is_eq(id, x) == False{} : Bool}, ih: {HM.rec_of(HM.drop_msg(t, x), id) == HM.rec_of(t, id) : Maybe<&2, HS.MRec>})
    -> {HM.rec_of(HM.drop_msg(h <> t, x), id) == HM.rec_of(h <> t, id) : Maybe<&2, HS.MRec>}:
  match q:
    case True{}:
      +nx = Equal.trans(Bool, Nat.is_eq(x, id), Nat.is_eq(id, x), False{}, cs_eq_sym(x, id), n)
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.drop_msg(h <> t, x), id), HM.rec_of(t, id), HM.rec_of(h <> t, id),
        Equal.cong(List<&2, HS.MRec>, Maybe<&2, HS.MRec>, w => HM.rec_of(w, id), HM.drop_msg(h <> t, x), t, PC2.hc_drop_T(h, t, x, eq)),
        Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, id), HM.rec_of(t, id), PC2.hc_rec_hd_F(h, t, id, cs_ne(HM.MRec.id(h), x, id, eq, nx))))
    case False{}:
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.drop_msg(h <> t, x), id), HM.rec_of(h <> HM.drop_msg(t, x), id), HM.rec_of(h <> t, id),
        Equal.cong(List<&2, HS.MRec>, Maybe<&2, HS.MRec>, w => HM.rec_of(w, id), HM.drop_msg(h <> t, x), h <> HM.drop_msg(t, x), PC2.hc_drop_F(h, t, x, eq)),
        cs_rec_hd(Nat.is_eq(HM.MRec.id(h), id), h, HM.drop_msg(t, x), t, id, {==}, ih))

# L2: another id is found as before
def cs_rec_drop(+xs: List<&2, HS.MRec>, +x: Nat, +id: Nat, +n: {Nat.is_eq(id, x) == False{} : Bool})
    -> {HM.rec_of(HM.drop_msg(xs, x), id) == HM.rec_of(xs, id) : Maybe<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      cs_rec_drop.go(Nat.is_eq(HM.MRec.id(h), x), h, t, x, id, {==}, n, cs_rec_drop(t, x, id, n))

def cs_drop_map.F(b: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp,
    +ea: {Nat.is_eq(HM.MRec.id(h), id) == False{} : Bool}, +eb: {Nat.is_eq(HM.MRec.id(h), x) == b : Bool},
    ih: {HM.drop_msg(HM.map_msg(t, id, op), x) == HM.map_msg(HM.drop_msg(t, x), id, op) : List<&2, HS.MRec>})
    -> {HM.drop_msg(HM.map_msg(h <> t, id, op), x) == HM.map_msg(HM.drop_msg(h <> t, x), id, op) : List<&2, HS.MRec>}:
  match b:
    case True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> HM.map_msg(t, id, op), PC1.hc_map_hd_F(h, t, id, op, ea))
        : {HM.drop_msg(_, x) == HM.map_msg(HM.drop_msg(h <> t, x), id, op) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> HM.map_msg(t, id, op), x), HM.map_msg(t, id, op), PC2.hc_drop_T(h, HM.map_msg(t, id, op), x, eb))
        : {_ == HM.map_msg(HM.drop_msg(h <> t, x), id, op) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, x), t, PC2.hc_drop_T(h, t, x, eb))
        : {HM.map_msg(t, id, op) == HM.map_msg(_, id, op) : List<&2, HS.MRec>}
      {==}
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> HM.map_msg(t, id, op), PC1.hc_map_hd_F(h, t, id, op, ea))
        : {HM.drop_msg(_, x) == HM.map_msg(HM.drop_msg(h <> t, x), id, op) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> HM.map_msg(t, id, op), x), h <> HM.drop_msg(HM.map_msg(t, id, op), x), PC2.hc_drop_F(h, HM.map_msg(t, id, op), x, eb))
        : {_ == HM.map_msg(HM.drop_msg(h <> t, x), id, op) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, x), h <> HM.drop_msg(t, x), PC2.hc_drop_F(h, t, x, eb))
        : {h <> HM.drop_msg(HM.map_msg(t, id, op), x) == HM.map_msg(_, id, op) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> HM.drop_msg(t, x), id, op), h <> HM.map_msg(HM.drop_msg(t, x), id, op), PC1.hc_map_hd_F(h, HM.drop_msg(t, x), id, op, ea))
        : {h <> HM.drop_msg(HM.map_msg(t, id, op), x) == _ : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => h <> w, HM.drop_msg(HM.map_msg(t, id, op), x), HM.map_msg(HM.drop_msg(t, x), id, op), ih)

def cs_drop_map.go(a: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp,
    +ea: {Nat.is_eq(HM.MRec.id(h), id) == a : Bool}, +n: {Nat.is_eq(id, x) == False{} : Bool},
    ih: {HM.drop_msg(HM.map_msg(t, id, op), x) == HM.map_msg(HM.drop_msg(t, x), id, op) : List<&2, HS.MRec>})
    -> {HM.drop_msg(HM.map_msg(h <> t, id, op), x) == HM.map_msg(HM.drop_msg(h <> t, x), id, op) : List<&2, HS.MRec>}:
  match a:
    case True{}:
      +eb = cs_ne(HM.MRec.id(h), id, x, ea, n)
      +eo = Equal.trans(Bool, Nat.is_eq(HM.MRec.id(HM.rec_op(h, op)), x), Nat.is_eq(HM.MRec.id(h), x), False{},
        Equal.cong(Nat, Bool, v => Nat.is_eq(v, x), HM.MRec.id(HM.rec_op(h, op)), HM.MRec.id(h), PC1.hc_rec_op_id(h, op)), eb)
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), HM.rec_op(h, op) <> t, PC1.hc_map_hd_T(h, t, id, op, ea))
        : {HM.drop_msg(_, x) == HM.map_msg(HM.drop_msg(h <> t, x), id, op) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(HM.rec_op(h, op) <> t, x), HM.rec_op(h, op) <> HM.drop_msg(t, x), PC2.hc_drop_F(HM.rec_op(h, op), t, x, eo))
        : {_ == HM.map_msg(HM.drop_msg(h <> t, x), id, op) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, x), h <> HM.drop_msg(t, x), PC2.hc_drop_F(h, t, x, eb))
        : {HM.rec_op(h, op) <> HM.drop_msg(t, x) == HM.map_msg(_, id, op) : List<&2, HS.MRec>}
      Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> HM.drop_msg(t, x), id, op), HM.rec_op(h, op) <> HM.drop_msg(t, x), PC1.hc_map_hd_T(h, HM.drop_msg(t, x), id, op, ea))
    case False{}:
      cs_drop_map.F(Nat.is_eq(HM.MRec.id(h), x), h, t, x, id, op, ea, {==}, ih)

# L1: an op on another id, then the drop: the drop, then the op
def cs_drop_map(+xs: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp, +n: {Nat.is_eq(id, x) == False{} : Bool})
    -> {HM.drop_msg(HM.map_msg(xs, id, op), x) == HM.map_msg(HM.drop_msg(xs, x), id, op) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      cs_drop_map.go(Nat.is_eq(HM.MRec.id(h), id), h, t, x, id, op, {==}, n, cs_drop_map(t, x, id, op, n))

def cs_drop_drop.go(p: Bool, q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +a: Nat, +b: Nat,
    +ep: {Nat.is_eq(HM.MRec.id(h), a) == p : Bool}, +eq: {Nat.is_eq(HM.MRec.id(h), b) == q : Bool},
    ih: {HM.drop_msg(HM.drop_msg(t, a), b) == HM.drop_msg(HM.drop_msg(t, b), a) : List<&2, HS.MRec>})
    -> {HM.drop_msg(HM.drop_msg(h <> t, a), b) == HM.drop_msg(HM.drop_msg(h <> t, b), a) : List<&2, HS.MRec>}:
  match p q:
    case True{} True{}:
      +ab = Equal.trans(Nat, b, HM.MRec.id(h), a, Equal.sym(Nat, HM.MRec.id(h), b, PC1.hc_eq_nat(HM.MRec.id(h), b, eq)), PC1.hc_eq_nat(HM.MRec.id(h), a, ep))
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, a), t, PC2.hc_drop_T(h, t, a, ep)) : {HM.drop_msg(_, b) == HM.drop_msg(HM.drop_msg(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, b), t, PC2.hc_drop_T(h, t, b, eq)) : {HM.drop_msg(t, b) == HM.drop_msg(_, a) : List<&2, HS.MRec>}
      Equal.cong(Nat, List<&2, HS.MRec>, v => HM.drop_msg(t, v), b, a, ab)
    case True{} False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, a), t, PC2.hc_drop_T(h, t, a, ep)) : {HM.drop_msg(_, b) == HM.drop_msg(HM.drop_msg(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, b), h <> HM.drop_msg(t, b), PC2.hc_drop_F(h, t, b, eq)) : {HM.drop_msg(t, b) == HM.drop_msg(_, a) : List<&2, HS.MRec>}
      Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> HM.drop_msg(t, b), a), HM.drop_msg(t, b), PC2.hc_drop_T(h, HM.drop_msg(t, b), a, ep))
    case False{} True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, a), h <> HM.drop_msg(t, a), PC2.hc_drop_F(h, t, a, ep)) : {HM.drop_msg(_, b) == HM.drop_msg(HM.drop_msg(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, b), t, PC2.hc_drop_T(h, t, b, eq)) : {HM.drop_msg(h <> HM.drop_msg(t, a), b) == HM.drop_msg(_, a) : List<&2, HS.MRec>}
      PC2.hc_drop_T(h, HM.drop_msg(t, a), b, eq)
    case False{} False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, a), h <> HM.drop_msg(t, a), PC2.hc_drop_F(h, t, a, ep)) : {HM.drop_msg(_, b) == HM.drop_msg(HM.drop_msg(h <> t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, b), h <> HM.drop_msg(t, b), PC2.hc_drop_F(h, t, b, eq)) : {HM.drop_msg(h <> HM.drop_msg(t, a), b) == HM.drop_msg(_, a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> HM.drop_msg(t, a), b), h <> HM.drop_msg(HM.drop_msg(t, a), b), PC2.hc_drop_F(h, HM.drop_msg(t, a), b, eq))
        : {_ == HM.drop_msg(h <> HM.drop_msg(t, b), a) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> HM.drop_msg(t, b), a), h <> HM.drop_msg(HM.drop_msg(t, b), a), PC2.hc_drop_F(h, HM.drop_msg(t, b), a, ep))
        : {h <> HM.drop_msg(HM.drop_msg(t, a), b) == _ : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => h <> w, HM.drop_msg(HM.drop_msg(t, a), b), HM.drop_msg(HM.drop_msg(t, b), a), ih)

# L3: two drops commute
def cs_drop_drop(+xs: List<&2, HS.MRec>, +a: Nat, +b: Nat) -> {HM.drop_msg(HM.drop_msg(xs, a), b) == HM.drop_msg(HM.drop_msg(xs, b), a) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      cs_drop_drop.go(Nat.is_eq(HM.MRec.id(h), a), Nat.is_eq(HM.MRec.id(h), b), h, t, a, b, {==}, {==}, cs_drop_drop(t, a, b))

# a list below h, h not above r: r goes first
def cs_ins_first(+h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, bt: HI.Below(HI.ids(t), HM.MRec.id(h)),
    +g: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == False{} : Bool}) -> {HM.ins_desc(t, r) == r <> t : List<&2, HS.MRec>}:
  PC1.hc_ins_head(t, r, PL1.hub_below_mono(HI.ids(t), HM.MRec.id(h), HM.MRec.id(r), cs_ngt_ge(HM.MRec.id(h), HM.MRec.id(r), g), bt))

def cs_drop_ins.go(g: Bool, b: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +r: HS.MRec, +x: Nat,
    +eg: {Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)) == g : Bool}, +eb: {Nat.is_eq(HM.MRec.id(h), x) == b : Bool},
    +nr: {Nat.is_eq(HM.MRec.id(r), x) == False{} : Bool}, bt: HI.Below(HI.ids(t), HM.MRec.id(h)),
    ih: {HM.drop_msg(HM.ins_desc(t, r), x) == HM.ins_desc(HM.drop_msg(t, x), r) : List<&2, HS.MRec>})
    -> {HM.drop_msg(HM.ins_desc(h <> t, r), x) == HM.ins_desc(HM.drop_msg(h <> t, x), r) : List<&2, HS.MRec>}:
  match g b:
    case True{} True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), PC1.hc_ins_T(h, t, r, eg))
        : {HM.drop_msg(_, x) == HM.ins_desc(HM.drop_msg(h <> t, x), r) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, x), t, PC2.hc_drop_T(h, t, x, eb))
        : {HM.drop_msg(h <> HM.ins_desc(t, r), x) == HM.ins_desc(_, r) : List<&2, HS.MRec>}
      PC2.hc_drop_T(h, HM.ins_desc(t, r), x, eb)
    case True{} False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), h <> HM.ins_desc(t, r), PC1.hc_ins_T(h, t, r, eg))
        : {HM.drop_msg(_, x) == HM.ins_desc(HM.drop_msg(h <> t, x), r) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, x), h <> HM.drop_msg(t, x), PC2.hc_drop_F(h, t, x, eb))
        : {HM.drop_msg(h <> HM.ins_desc(t, r), x) == HM.ins_desc(_, r) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> HM.drop_msg(t, x), r), h <> HM.ins_desc(HM.drop_msg(t, x), r), PC1.hc_ins_T(h, HM.drop_msg(t, x), r, eg))
        : {HM.drop_msg(h <> HM.ins_desc(t, r), x) == _ : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> HM.ins_desc(t, r), x), h <> HM.drop_msg(HM.ins_desc(t, r), x), PC2.hc_drop_F(h, HM.ins_desc(t, r), x, eb))
        : {_ == h <> HM.ins_desc(HM.drop_msg(t, x), r) : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => h <> w, HM.drop_msg(HM.ins_desc(t, r), x), HM.ins_desc(HM.drop_msg(t, x), r), ih)
    case False{} True{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), r <> h <> t, PC1.hc_ins_F(h, t, r, eg))
        : {HM.drop_msg(_, x) == HM.ins_desc(HM.drop_msg(h <> t, x), r) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, x), t, PC2.hc_drop_T(h, t, x, eb))
        : {HM.drop_msg(r <> h <> t, x) == HM.ins_desc(_, r) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(t, r), r <> t, cs_ins_first(h, t, r, bt, eg))
        : {HM.drop_msg(r <> h <> t, x) == _ : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(r <> h <> t, x), r <> HM.drop_msg(h <> t, x), PC2.hc_drop_F(r, h <> t, x, nr))
        : {_ == r <> t : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => r <> w, HM.drop_msg(h <> t, x), t, PC2.hc_drop_T(h, t, x, eb))
    case False{} False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> t, r), r <> h <> t, PC1.hc_ins_F(h, t, r, eg))
        : {HM.drop_msg(_, x) == HM.ins_desc(HM.drop_msg(h <> t, x), r) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(h <> t, x), h <> HM.drop_msg(t, x), PC2.hc_drop_F(h, t, x, eb))
        : {HM.drop_msg(r <> h <> t, x) == HM.ins_desc(_, r) : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.ins_desc(h <> HM.drop_msg(t, x), r), r <> h <> HM.drop_msg(t, x), PC1.hc_ins_F(h, HM.drop_msg(t, x), r, eg))
        : {HM.drop_msg(r <> h <> t, x) == _ : List<&2, HS.MRec>}
      %Equal.sym(List<&2, HS.MRec>, HM.drop_msg(r <> h <> t, x), r <> HM.drop_msg(h <> t, x), PC2.hc_drop_F(r, h <> t, x, nr))
        : {_ == r <> h <> HM.drop_msg(t, x) : List<&2, HS.MRec>}
      Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => r <> w, HM.drop_msg(h <> t, x), h <> HM.drop_msg(t, x), PC2.hc_drop_F(h, t, x, eb))

# L5: a record of another id into a descending list, then the drop: the drop, then the record
def cs_drop_ins(+xs: List<&2, HS.MRec>, +r: HS.MRec, +x: Nat, +nr: {Nat.is_eq(HM.MRec.id(r), x) == False{} : Bool}, d: HI.Desc(xs))
    -> {HM.drop_msg(HM.ins_desc(xs, r), x) == HM.ins_desc(HM.drop_msg(xs, x), r) : List<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      PC2.hc_drop_F(r, Nil{}, x, nr)
    case Con{h, t}:
      match d:
        case HX.Both{bt, dt}:
          cs_drop_ins.go(Nat.is_gt(HM.MRec.id(h), HM.MRec.id(r)), Nat.is_eq(HM.MRec.id(h), x), h, t, r, x, {==}, {==}, nr, bt, cs_drop_ins(t, r, x, nr, dt))

# L4: records the cold one is not among, put into the descending side
def cs_drop_all(+cs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +x: Nat, +nx: {HM.nat_in(HI.ids(cs), x) == False{} : Bool},
    +a: HI.Asc(cs), +d: HI.Desc(os), +j: HI.Disjoint(cs, os))
    -> {HM.drop_msg(HM.ins_all(cs, os), x) == HM.ins_all(cs, HM.drop_msg(os, x)) : List<&2, HS.MRec>}:
  match cs:
    case Nil{}:
      {==}
    case Con{r, t}:
      match a j:
        case HX.Both{ab, at} HX.Both{jr, jt}:
          +nr = PL1.hub_lor_false_l(Nat.is_eq(HM.MRec.id(r), x), HM.nat_in(HI.ids(t), x), nx)
          +nt = PL1.hub_lor_false_r(Nat.is_eq(HM.MRec.id(r), x), HM.nat_in(HI.ids(t), x), nx)
          Equal.trans(List<&2, HS.MRec>, HM.drop_msg(HM.ins_desc(HM.ins_all(t, os), r), x), HM.ins_desc(HM.drop_msg(HM.ins_all(t, os), x), r), HM.ins_desc(HM.ins_all(t, HM.drop_msg(os, x)), r),
            cs_drop_ins(HM.ins_all(t, os), r, x, nr, PC1.hc_desc_all(t, os, at, d, jt)),
            Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => HM.ins_desc(w, r), HM.drop_msg(HM.ins_all(t, os), x), HM.ins_all(t, HM.drop_msg(os, x)), cs_drop_all(t, os, x, nt, at, d, jt)))

# ---- a message op with the cold message `x` out: the same op, x out of its old side ----

# the two sides with `x` out of the old one
def dx(+q: HM.Ms, +x: Nat) -> HM.Ms:
  HM.Ms{HM.Ms.hot(q), HM.drop_msg(HM.Ms.old(q), x)}

def cs_op_cold.go(back: Bool, +c: HS.MRec, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp,
    +n: {Nat.is_eq(id, x) == False{} : Bool})
    -> {HM.msg_op.cold.go(back, c, hot, HM.drop_msg(old, x), id, op) == dx(HM.msg_op.cold.go(back, c, hot, old, id, op), x) : HM.Ms}:
  match back:
    case True{}:
      Equal.cong(List<&2, HS.MRec>, HM.Ms, w => HM.Ms{HM.ins_asc(hot, HM.rec_op(c, op)), w}, HM.drop_msg(HM.drop_msg(old, x), id), HM.drop_msg(HM.drop_msg(old, id), x),
        cs_drop_drop(old, x, id))
    case False{}:
      Equal.cong(List<&2, HS.MRec>, HM.Ms, w => HM.Ms{hot, w}, HM.map_msg(HM.drop_msg(old, x), id, op), HM.drop_msg(HM.map_msg(old, id, op), x),
        Equal.sym(List<&2, HS.MRec>, HM.drop_msg(HM.map_msg(old, id, op), x), HM.map_msg(HM.drop_msg(old, x), id, op), cs_drop_map(old, x, id, op, n)))

def cs_op_cold(m: Maybe<&2, HS.MRec>, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp,
    +n: {Nat.is_eq(id, x) == False{} : Bool})
    -> {HM.msg_op.cold(m, hot, HM.drop_msg(old, x), id, op) == dx(HM.msg_op.cold(m, hot, old, id, op), x) : HM.Ms}:
  match m:
    case None{}:
      {==}
    case Some{+c}:
      cs_op_cold.go(HM.MRec.is_hot(HM.rec_op(c, op)), c, hot, old, x, id, op, n)

def cs_op.in(in_hot: Bool, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp,
    +n: {Nat.is_eq(id, x) == False{} : Bool}, +nh: {HM.nat_in(HI.ids(hot), x) == False{} : Bool},
    +a: HI.Asc(hot), +d: HI.Desc(old), +j: HI.Disjoint(hot, old))
    -> {HM.msg_op.in(in_hot, hot, HM.drop_msg(old, x), id, op) == dx(HM.msg_op.in(in_hot, hot, old, id, op), x) : HM.Ms}:
  match in_hot:
    case True{}:
      +hs = HM.map_msg(hot, id, op)
      +nhs = Equal.trans(Bool, HM.nat_in(HI.ids(hs), x), HM.nat_in(HI.ids(hot), x), False{},
        Equal.cong(List<&2, Nat>, Bool, v => HM.nat_in(v, x), HI.ids(hs), HI.ids(hot), PL1.hub_ids_map(hot, id, op)), nh)
      Equal.cong(List<&2, HS.MRec>, HM.Ms, w => HM.Ms{HM.hot_of(hs), w}, HM.ins_all(HM.cold_of(hs), HM.drop_msg(old, x)), HM.drop_msg(HM.ins_all(HM.cold_of(hs), old), x),
        Equal.sym(List<&2, HS.MRec>, HM.drop_msg(HM.ins_all(HM.cold_of(hs), old), x), HM.ins_all(HM.cold_of(hs), HM.drop_msg(old, x)),
          cs_drop_all(HM.cold_of(hs), old, x, PC3.hc_nin_cold(hs, x, nhs), PC3.hc_asc_cold(hs, PC3.hc_asc_map(hot, id, op, a)), d,
            PC3.hc_disj_cold(hs, old, PC3.hc_disj_map(hot, old, id, op, j)))))
    case False{}:
      Equal.trans(HM.Ms, HM.msg_op.cold(HM.rec_of(HM.drop_msg(old, x), id), hot, HM.drop_msg(old, x), id, op),
        HM.msg_op.cold(HM.rec_of(old, id), hot, HM.drop_msg(old, x), id, op), dx(HM.msg_op.cold(HM.rec_of(old, id), hot, old, id, op), x),
        Equal.cong(Maybe<&2, HS.MRec>, HM.Ms, m => HM.msg_op.cold(m, hot, HM.drop_msg(old, x), id, op), HM.rec_of(HM.drop_msg(old, x), id), HM.rec_of(old, id),
          cs_rec_drop(old, x, id, n)),
        cs_op_cold(HM.rec_of(old, id), hot, old, x, id, op, n))

# OP: a message op on another id, with `x` out of the old side
def cs_op(+hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp,
    +n: {Nat.is_eq(id, x) == False{} : Bool}, +nh: {HM.nat_in(HI.ids(hot), x) == False{} : Bool},
    +a: HI.Asc(hot), +d: HI.Desc(old), +j: HI.Disjoint(hot, old))
    -> {HM.msg_op(hot, HM.drop_msg(old, x), id, op) == dx(HM.msg_op(hot, old, id, op), x) : HM.Ms}:
  cs_op.in(Maybe.is_some(&2, HS.MRec, HM.rec_of(hot, id)), hot, old, x, id, op, n, nh, a, d, j)

# ---- the settle of a reply ----

def cs_rec_in.go(m: Maybe<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +n: {Nat.is_eq(id, x) == False{} : Bool})
    -> {HM.rec_in.go(m, HM.drop_msg(old, x), id) == HM.rec_in.go(m, old, id) : Maybe<&2, HS.MRec>}:
  match m:
    case None{}:
      cs_rec_drop(old, x, id, n)
    case Some{r}:
      {==}

def cs_settle.if(ok: Bool, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat,
    +n: {Nat.is_eq(id, x) == False{} : Bool}, +nh: {HM.nat_in(HI.ids(hot), x) == False{} : Bool},
    +a: HI.Asc(hot), +d: HI.Desc(old), +j: HI.Disjoint(hot, old))
    -> {HM.settle_in.if(ok, hot, HM.drop_msg(old, x), id) == dx(HM.settle_in.if(ok, hot, old, id), x) : HM.Ms}:
  match ok:
    case True{}:
      cs_op(hot, old, x, id, HM.OSettle{}, n, nh, a, d, j)
    case False{}:
      {==}

def cs_settle.go(orig: Maybe<&2, HS.MRec>, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +from: String,
    +n: {Nat.is_eq(id, x) == False{} : Bool}, +nh: {HM.nat_in(HI.ids(hot), x) == False{} : Bool},
    +a: HI.Asc(hot), +d: HI.Desc(old), +j: HI.Disjoint(hot, old))
    -> {HM.settle_in.go(orig, hot, HM.drop_msg(old, x), id, from) == dx(HM.settle_in.go(orig, hot, old, id, from), x) : HM.Ms}:
  match orig:
    case None{}:
      {==}
    case Some{+o}:
      cs_settle.if(String.eq(HS.Msg.to(HS.MRec.msg(o)), from) || String.eq(from, HM.USER()), hot, old, x, id, n, nh, a, d, j)

# SET: the settle of a reply to another message, with `x` out of the old side
def cs_settle(r: Maybe<&2, Nat>, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +from: String,
    +n: {HM.nat_in(N.Needs.msgs(N.of_reply(r)), x) == False{} : Bool}, +nh: {HM.nat_in(HI.ids(hot), x) == False{} : Bool},
    +a: HI.Asc(hot), +d: HI.Desc(old), +j: HI.Disjoint(hot, old))
    -> {HM.settle_in(r, hot, HM.drop_msg(old, x), from) == dx(HM.settle_in(r, hot, old, from), x) : HM.Ms}:
  match r:
    case None{}:
      {==}
    case Some{+id}:
      +ni = PL1.hub_lor_false_l(Nat.is_eq(id, x), HM.nat_in(Nil{}, x), n)
      Equal.trans(HM.Ms, HM.settle_in.go(HM.rec_in(hot, HM.drop_msg(old, x), id), hot, HM.drop_msg(old, x), id, from),
        HM.settle_in.go(HM.rec_in(hot, old, id), hot, HM.drop_msg(old, x), id, from), dx(HM.settle_in.go(HM.rec_in(hot, old, id), hot, old, id, from), x),
        Equal.cong(Maybe<&2, HS.MRec>, HM.Ms, m => HM.settle_in.go(m, hot, HM.drop_msg(old, x), id, from), HM.rec_in(hot, HM.drop_msg(old, x), id), HM.rec_in(hot, old, id),
          cs_rec_in.go(HM.rec_of(hot, id), old, x, id, ni)),
        cs_settle.go(HM.rec_in(hot, old, id), hot, old, x, id, from, ni, nh, a, d, j))

# ---- the cold one is on no hot side ----

def cs_isnone(m: Maybe<&2, HS.MRec>) -> Type:
  match m:
    case None{}:
      Unit
    case Some{_}:
      Empty

def cs_some_none(-r: HS.MRec, e: {Some{r} == None{} : Maybe<&2, HS.MRec>}) -> Empty:
  %Equal.sym(Maybe<&2, HS.MRec>, Some{r}, None{}, e) : cs_isnone(_)
  Unit{}

def cs_disj_nin.go(q: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +x: Nat, -r: HS.MRec,
    +eq: {Nat.is_eq(HM.MRec.id(h), x) == q : Bool}, +jh: {HM.nat_in(HI.ids(os), HM.MRec.id(h)) == False{} : Bool},
    +f: {HM.rec_of(os, x) == Some{r} : Maybe<&2, HS.MRec>}, ih: {HM.nat_in(HI.ids(t), x) == False{} : Bool})
    -> {HM.nat_in(HI.ids(h <> t), x) == False{} : Bool}:
  match q:
    case True{}:
      +nx = Equal.trans(Bool, HM.nat_in(HI.ids(os), x), HM.nat_in(HI.ids(os), HM.MRec.id(h)), False{},
        Equal.cong(Nat, Bool, v => HM.nat_in(HI.ids(os), v), x, HM.MRec.id(h), Equal.sym(Nat, HM.MRec.id(h), x, PC1.hc_eq_nat(HM.MRec.id(h), x, eq))), jh)
      +sn = Equal.trans(Maybe<&2, HS.MRec>, Some{r}, HM.rec_of(os, x), None{}, Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(os, x), Some{r}, f), PC2.hc_rec_miss(os, x, nx))
      AB.absurd_eq(cs_some_none(r, sn), Bool, HM.nat_in(HI.ids(h <> t), x), False{})
    case False{}:
      PL1.hub_lor_false(Nat.is_eq(HM.MRec.id(h), x), HM.nat_in(HI.ids(t), x), eq, ih)

def cs_disj_nin(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +x: Nat, -r: HS.MRec, +j: HI.Disjoint(hs, os),
    +f: {HM.rec_of(os, x) == Some{r} : Maybe<&2, HS.MRec>}) -> {HM.nat_in(HI.ids(hs), x) == False{} : Bool}:
  match hs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match j:
        case HX.Both{jh, jt}:
          cs_disj_nin.go(Nat.is_eq(HM.MRec.id(h), x), h, t, os, x, r, {==}, jh, f, cs_disj_nin(t, os, x, r, jt, f))

# the state opened, its two sides emptied (what a message event builds on)
def rest(+s: HS.St) -> HS.St:
  HS.St.with_old(HS.St.with_hot(HS.St.open(s), Nil{}), Nil{})

# a state or a settle on the message `id` (not `x`)
def cs_op_ev(+s: HS.St, +x: Nat, -r: HS.MRec, +id: Nat, +op: HM.MsgOp, +ni: {Nat.is_eq(id, x) == False{} : Bool}, +u: HI.Split(s),
    +o: {HM.rec_of(HS.St.old(s), x) == Some{r} : Maybe<&2, HS.MRec>})
    -> {HS.St.with_old(HS.St.with_hot(rest(s), HM.Ms.hot(HM.msg_op(HS.St.hot(s), HM.drop_msg(HS.St.old(s), x), id, op))), HM.Ms.old(HM.msg_op(HS.St.hot(s), HM.drop_msg(HS.St.old(s), x), id, op)))
        == HS.St.with_old(HS.St.with_hot(rest(s), HM.Ms.hot(dx(HM.msg_op(HS.St.hot(s), HS.St.old(s), id, op), x))), HM.Ms.old(dx(HM.msg_op(HS.St.hot(s), HS.St.old(s), id, op), x))) : HS.St}:
  +hot = HS.St.hot(s)
  +old = HS.St.old(s)
  +nm = HS.St.next_msg(s)
  +nh = cs_disj_nin(hot, old, x, r, PC2.hc_p_disj(hot, old, nm, u), o)
  Equal.cong(HM.Ms, HS.St, q => HS.St.with_old(HS.St.with_hot(rest(s), HM.Ms.hot(q)), HM.Ms.old(q)),
    HM.msg_op(hot, HM.drop_msg(old, x), id, op), dx(HM.msg_op(hot, old, id, op), x),
    cs_op(hot, old, x, id, op, ni, nh, PC2.hc_p_asc(hot, old, nm, u), PC2.hc_p_desc(hot, old, nm, u), PC2.hc_p_disj(hot, old, nm, u)))

# a new message: its reply's settle (not `x`), `x` on no hot side
def cs_sent_ev(+s: HS.St, +x: Nat, -r: HS.MRec, +m: HS.Msg, +n: {HM.nat_in(N.Needs.msgs(N.of_reply(HS.Msg.reply_to(m))), x) == False{} : Bool},
    +u: HI.Split(s), +k: {HS.Msg.id(m) == HS.St.next_msg(s) : Nat}, +o: {HM.rec_of(HS.St.old(s), x) == Some{r} : Maybe<&2, HS.MRec>})
    -> {HS.St.with_old(HS.St.with_next_msg(HS.St.with_hot(rest(s), HM.Ms.hot(HM.settle_in(HS.Msg.reply_to(m), HM.snoc(HS.MRec, HS.St.hot(s), HM.new_rec(m)), HM.drop_msg(HS.St.old(s), x), HS.Msg.from(m)))),
          Nat.max(Nat.max(HS.St.next_msg(s), 1n+HS.Msg.id(m)), 1n+HS.Msg.thread(m))),
          HM.Ms.old(HM.settle_in(HS.Msg.reply_to(m), HM.snoc(HS.MRec, HS.St.hot(s), HM.new_rec(m)), HM.drop_msg(HS.St.old(s), x), HS.Msg.from(m))))
        == HS.St.with_old(HS.St.with_next_msg(HS.St.with_hot(rest(s), HM.Ms.hot(dx(HM.settle_in(HS.Msg.reply_to(m), HM.snoc(HS.MRec, HS.St.hot(s), HM.new_rec(m)), HS.St.old(s), HS.Msg.from(m)), x))),
          Nat.max(Nat.max(HS.St.next_msg(s), 1n+HS.Msg.id(m)), 1n+HS.Msg.thread(m))),
          HM.Ms.old(dx(HM.settle_in(HS.Msg.reply_to(m), HM.snoc(HS.MRec, HS.St.hot(s), HM.new_rec(m)), HS.St.old(s), HS.Msg.from(m)), x))) : HS.St}:
  +hot = HS.St.hot(s)
  +old = HS.St.old(s)
  +nm = HS.St.next_msg(s)
  +top = Nat.max(Nat.max(nm, 1n+HS.Msg.id(m)), 1n+HS.Msg.thread(m))
  +hs = HM.snoc(HS.MRec, hot, HM.new_rec(m))
  +kg = Equal.trans(Bool, Nat.is_ge(HS.Msg.id(m), nm), Nat.is_ge(nm, nm), True{}, Equal.cong(Nat, Bool, v => Nat.is_ge(v, nm), HS.Msg.id(m), nm, k), cs_ge_refl(nm))
  +ps = PC4.hc_parts_sent(hot, old, nm, m, u, kg)
  +nh = cs_disj_nin(hs, old, x, r, PC2.hc_p_disj(hs, old, top, ps), o)
  Equal.cong(HM.Ms, HS.St, q => HS.St.with_old(HS.St.with_next_msg(HS.St.with_hot(rest(s), HM.Ms.hot(q)), top), HM.Ms.old(q)),
    HM.settle_in(HS.Msg.reply_to(m), hs, HM.drop_msg(old, x), HS.Msg.from(m)), dx(HM.settle_in(HS.Msg.reply_to(m), hs, old, HS.Msg.from(m)), x),
    cs_settle(HS.Msg.reply_to(m), hs, old, x, HS.Msg.from(m), n, nh, PC2.hc_p_asc(hs, old, top, ps), PC2.hc_p_desc(hs, old, top, ps), PC2.hc_p_disj(hs, old, top, ps)))

def ail.go(+e: HM.Ev, +s: HS.St, +x: Nat, -r: HS.MRec, +n: {HM.nat_in(N.Needs.msgs(N.needs(e)), x) == False{} : Bool},
    +u: HI.Split(s), +k: HX.FreshEv(s, e), +o: {HM.rec_of(HS.St.old(s), x) == Some{r} : Maybe<&2, HS.MRec>})
    -> {HM.apply(N.drop_old(HS.St.open(s), x), e) == N.drop_old(HM.apply(HS.St.open(s), e), x) : HS.St}:
  match e:
    case HM.ETaskCreated{a, b, c, d, f}:
      {==}
    case HM.ELifecycle{a, b, c}:
      {==}
    case HM.EWorkspace{a, b}:
      {==}
    case HM.ESnapshot{a, b}:
      {==}
    case HM.EPlaced{a, b, c}:
      {==}
    case HM.EDropped{a, b, c}:
      {==}
    case HM.EDeclared{a, b, c}:
      {==}
    case HM.EReported{a, b}:
      {==}
    case HM.EFileTouched{a, b}:
      {==}
    case HM.EClaimDropped{a, b}:
      {==}
    case HM.ERenamed{a, b}:
      {==}
    case HM.EMsgsMoved{f, t}:
      {==}
    case HM.EMsgSent{m}:
      cs_sent_ev(s, x, r, HM.msg_of_sent(m), n, u, k, o)
    case HM.EMsgState{id, y}:
      cs_op_ev(s, x, r, id, HM.OSt{y}, PL1.hub_lor_false_l(Nat.is_eq(id, x), HM.nat_in(Nil{}, x), n), u, o)
    case HM.ESettled{id}:
      cs_op_ev(s, x, r, id, HM.OSettle{}, PL1.hub_lor_false_l(Nat.is_eq(id, x), HM.nat_in(Nil{}, x), n), u, o)
    case HM.ECardOpened{c}:
      {==}
    case HM.ECardClosed{a, b, c}:
      {==}
    case HM.ECardAnswerPos{a, b, c}:
      {==}
    case HM.ECardWithdrawn{a, b}:
      {==}
    case HM.ECardLate{a}:
      {==}
    case HM.EMainNote{a, b}:
      {==}
    case HM.ENotesFlushed{}:
      {==}
    case HM.ETimerSet{a, b}:
      {==}
    case HM.ETimerFired{a, b, c, d, f}:
      {==}
    case HM.ETimerRun{a, b}:
      {==}
    case HM.ETimerStop{a, b, c}:
      {==}
    case HM.ETimerName{a, b, c, d}:
      {==}
    case HM.EX{a}:
      {==}
    case HM.EAct{a}:
      {==}
    case HM.EFollow{a, b, c}:
      {==}
    case HM.EJobEnd{a, b}:
      {==}
    case HM.EWake{a}:
      {==}
    case HM.EPaused{a}:
      {==}
    case HM.EResumed{a}:
      {==}

# ---- the cold message `x` stays where it was: an event that does not name it ----

def cs_rec_map.go(a: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +x: Nat,
    +ea: {Nat.is_eq(HM.MRec.id(h), id) == a : Bool}, +n: {Nat.is_eq(id, x) == False{} : Bool},
    ih: {HM.rec_of(HM.map_msg(t, id, op), x) == HM.rec_of(t, x) : Maybe<&2, HS.MRec>})
    -> {HM.rec_of(HM.map_msg(h <> t, id, op), x) == HM.rec_of(h <> t, x) : Maybe<&2, HS.MRec>}:
  match a:
    case True{}:
      +eb = cs_ne(HM.MRec.id(h), id, x, ea, n)
      +eo = Equal.trans(Bool, Nat.is_eq(HM.MRec.id(HM.rec_op(h, op)), x), Nat.is_eq(HM.MRec.id(h), x), False{},
        Equal.cong(Nat, Bool, v => Nat.is_eq(v, x), HM.MRec.id(HM.rec_op(h, op)), HM.MRec.id(h), PC1.hc_rec_op_id(h, op)), eb)
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), HM.rec_op(h, op) <> t, PC1.hc_map_hd_T(h, t, id, op, ea))
        : {HM.rec_of(_, x) == HM.rec_of(h <> t, x) : Maybe<&2, HS.MRec>}
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.rec_op(h, op) <> t, x), HM.rec_of(t, x), HM.rec_of(h <> t, x),
        PC2.hc_rec_hd_F(HM.rec_op(h, op), t, x, eo), Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(h <> t, x), HM.rec_of(t, x), PC2.hc_rec_hd_F(h, t, x, eb)))
    case False{}:
      %Equal.sym(List<&2, HS.MRec>, HM.map_msg(h <> t, id, op), h <> HM.map_msg(t, id, op), PC1.hc_map_hd_F(h, t, id, op, ea))
        : {HM.rec_of(_, x) == HM.rec_of(h <> t, x) : Maybe<&2, HS.MRec>}
      cs_rec_hd(Nat.is_eq(HM.MRec.id(h), x), h, HM.map_msg(t, id, op), t, x, {==}, ih)

# an op on another id finds `x` as before
def cs_rec_map(+xs: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, +x: Nat, +n: {Nat.is_eq(id, x) == False{} : Bool})
    -> {HM.rec_of(HM.map_msg(xs, id, op), x) == HM.rec_of(xs, x) : Maybe<&2, HS.MRec>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      cs_rec_map.go(Nat.is_eq(HM.MRec.id(h), id), h, t, id, op, x, {==}, n, cs_rec_map(t, id, op, x, n))

def cs_keep_cold.go(back: Bool, +c: HS.MRec, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp,
    +n: {Nat.is_eq(id, x) == False{} : Bool})
    -> {HM.rec_of(HM.Ms.old(HM.msg_op.cold.go(back, c, hot, old, id, op)), x) == HM.rec_of(old, x) : Maybe<&2, HS.MRec>}:
  match back:
    case True{}:
      cs_rec_drop(old, id, x, Equal.trans(Bool, Nat.is_eq(x, id), Nat.is_eq(id, x), False{}, cs_eq_sym(x, id), n))
    case False{}:
      cs_rec_map(old, id, op, x, n)

def cs_keep_cold(m: Maybe<&2, HS.MRec>, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp,
    +n: {Nat.is_eq(id, x) == False{} : Bool})
    -> {HM.rec_of(HM.Ms.old(HM.msg_op.cold(m, hot, old, id, op)), x) == HM.rec_of(old, x) : Maybe<&2, HS.MRec>}:
  match m:
    case None{}:
      {==}
    case Some{+c}:
      cs_keep_cold.go(HM.MRec.is_hot(HM.rec_op(c, op)), c, hot, old, x, id, op, n)

def cs_keep_op.in(in_hot: Bool, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp,
    +n: {Nat.is_eq(id, x) == False{} : Bool}, +nh: {HM.nat_in(HI.ids(hot), x) == False{} : Bool},
    +a: HI.Asc(hot), +j: HI.Disjoint(hot, old))
    -> {HM.rec_of(HM.Ms.old(HM.msg_op.in(in_hot, hot, old, id, op)), x) == HM.rec_of(old, x) : Maybe<&2, HS.MRec>}:
  match in_hot:
    case True{}:
      +hs = HM.map_msg(hot, id, op)
      +cs = HM.cold_of(hs)
      +nhs = Equal.trans(Bool, HM.nat_in(HI.ids(hs), x), HM.nat_in(HI.ids(hot), x), False{},
        Equal.cong(List<&2, Nat>, Bool, v => HM.nat_in(v, x), HI.ids(hs), HI.ids(hot), PL1.hub_ids_map(hot, id, op)), nh)
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.ins_all(cs, old), x), HM.rec_in(cs, old, x), HM.rec_of(old, x),
        PC2.hc_rec_all(cs, old, x, PC3.hc_asc_cold(hs, PC3.hc_asc_map(hot, id, op, a)), PC3.hc_disj_cold(hs, old, PC3.hc_disj_map(hot, old, id, op, j))),
        Equal.cong(Maybe<&2, HS.MRec>, Maybe<&2, HS.MRec>, m => HM.rec_in.go(m, old, x), HM.rec_of(cs, x), None{},
          PC2.hc_rec_miss(cs, x, PC3.hc_nin_cold(hs, x, nhs))))
    case False{}:
      cs_keep_cold(HM.rec_of(old, id), hot, old, x, id, op, n)

def cs_keep_op(+hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +op: HM.MsgOp,
    +n: {Nat.is_eq(id, x) == False{} : Bool}, +nh: {HM.nat_in(HI.ids(hot), x) == False{} : Bool},
    +a: HI.Asc(hot), +j: HI.Disjoint(hot, old))
    -> {HM.rec_of(HM.Ms.old(HM.msg_op(hot, old, id, op)), x) == HM.rec_of(old, x) : Maybe<&2, HS.MRec>}:
  cs_keep_op.in(Maybe.is_some(&2, HS.MRec, HM.rec_of(hot, id)), hot, old, x, id, op, n, nh, a, j)

def cs_keep_settle.if(ok: Bool, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat,
    +n: {Nat.is_eq(id, x) == False{} : Bool}, +nh: {HM.nat_in(HI.ids(hot), x) == False{} : Bool}, +a: HI.Asc(hot), +j: HI.Disjoint(hot, old))
    -> {HM.rec_of(HM.Ms.old(HM.settle_in.if(ok, hot, old, id)), x) == HM.rec_of(old, x) : Maybe<&2, HS.MRec>}:
  match ok:
    case True{}:
      cs_keep_op(hot, old, x, id, HM.OSettle{}, n, nh, a, j)
    case False{}:
      {==}

def cs_keep_settle.go(orig: Maybe<&2, HS.MRec>, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +id: Nat, +from: String,
    +n: {Nat.is_eq(id, x) == False{} : Bool}, +nh: {HM.nat_in(HI.ids(hot), x) == False{} : Bool}, +a: HI.Asc(hot), +j: HI.Disjoint(hot, old))
    -> {HM.rec_of(HM.Ms.old(HM.settle_in.go(orig, hot, old, id, from)), x) == HM.rec_of(old, x) : Maybe<&2, HS.MRec>}:
  match orig:
    case None{}:
      {==}
    case Some{+o}:
      cs_keep_settle.if(String.eq(HS.Msg.to(HS.MRec.msg(o)), from) || String.eq(from, HM.USER()), hot, old, x, id, n, nh, a, j)

def cs_keep_settle(r: Maybe<&2, Nat>, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +x: Nat, +from: String,
    +n: {HM.nat_in(N.Needs.msgs(N.of_reply(r)), x) == False{} : Bool}, +nh: {HM.nat_in(HI.ids(hot), x) == False{} : Bool},
    +a: HI.Asc(hot), +j: HI.Disjoint(hot, old))
    -> {HM.rec_of(HM.Ms.old(HM.settle_in(r, hot, old, from)), x) == HM.rec_of(old, x) : Maybe<&2, HS.MRec>}:
  match r:
    case None{}:
      {==}
    case Some{+id}:
      +ni = PL1.hub_lor_false_l(Nat.is_eq(id, x), HM.nat_in(Nil{}, x), n)
      cs_keep_settle.go(HM.rec_in(hot, old, id), hot, old, x, id, from, ni, nh, a, j)

def cs_keep.go(+e: HM.Ev, +s: HS.St, +x: Nat, -r: HS.MRec, +n: {HM.nat_in(N.Needs.msgs(N.needs(e)), x) == False{} : Bool},
    +u: HI.Split(s), +k: HX.FreshEv(s, e), +o: {HM.rec_of(HS.St.old(s), x) == Some{r} : Maybe<&2, HS.MRec>})
    -> {HM.rec_of(HS.St.old(HM.apply(HS.St.open(s), e)), x) == Some{r} : Maybe<&2, HS.MRec>}:
  match e:
    case HM.EMsgSent{sent}:
      +m = HM.msg_of_sent(sent)
      +hot = HS.St.hot(s)
      +old = HS.St.old(s)
      +nm = HS.St.next_msg(s)
      +top = Nat.max(Nat.max(nm, 1n+HS.Msg.id(m)), 1n+HS.Msg.thread(m))
      +hs = HM.snoc(HS.MRec, hot, HM.new_rec(m))
      +kg = Equal.trans(Bool, Nat.is_ge(HS.Msg.id(m), nm), Nat.is_ge(nm, nm), True{}, Equal.cong(Nat, Bool, v => Nat.is_ge(v, nm), HS.Msg.id(m), nm, k), cs_ge_refl(nm))
      +ps = PC4.hc_parts_sent(hot, old, nm, m, u, kg)
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.Ms.old(HM.settle_in(HS.Msg.reply_to(m), hs, old, HS.Msg.from(m))), x), HM.rec_of(old, x), Some{r},
        cs_keep_settle(HS.Msg.reply_to(m), hs, old, x, HS.Msg.from(m), n, cs_disj_nin(hs, old, x, r, PC2.hc_p_disj(hs, old, top, ps), o),
          PC2.hc_p_asc(hs, old, top, ps), PC2.hc_p_disj(hs, old, top, ps)), o)
    case HM.EMsgState{id, y}:
      +hot = HS.St.hot(s)
      +old = HS.St.old(s)
      +nm = HS.St.next_msg(s)
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.Ms.old(HM.msg_op(hot, old, id, HM.OSt{y})), x), HM.rec_of(old, x), Some{r},
        cs_keep_op(hot, old, x, id, HM.OSt{y}, PL1.hub_lor_false_l(Nat.is_eq(id, x), HM.nat_in(Nil{}, x), n),
          cs_disj_nin(hot, old, x, r, PC2.hc_p_disj(hot, old, nm, u), o), PC2.hc_p_asc(hot, old, nm, u), PC2.hc_p_disj(hot, old, nm, u)), o)
    case HM.ESettled{id}:
      +hot = HS.St.hot(s)
      +old = HS.St.old(s)
      +nm = HS.St.next_msg(s)
      Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(HM.Ms.old(HM.msg_op(hot, old, id, HM.OSettle{})), x), HM.rec_of(old, x), Some{r},
        cs_keep_op(hot, old, x, id, HM.OSettle{}, PL1.hub_lor_false_l(Nat.is_eq(id, x), HM.nat_in(Nil{}, x), n),
          cs_disj_nin(hot, old, x, r, PC2.hc_p_disj(hot, old, nm, u), o), PC2.hc_p_asc(hot, old, nm, u), PC2.hc_p_disj(hot, old, nm, u)), o)
    case HM.ETaskCreated{a, b, c, d, f}:
      o
    case HM.ELifecycle{a, b, c}:
      o
    case HM.EWorkspace{a, b}:
      o
    case HM.ESnapshot{a, b}:
      o
    case HM.EPlaced{a, b, c}:
      o
    case HM.EDropped{a, b, c}:
      o
    case HM.EDeclared{a, b, c}:
      o
    case HM.EReported{a, b}:
      o
    case HM.EFileTouched{a, b}:
      o
    case HM.EClaimDropped{a, b}:
      o
    case HM.ERenamed{a, b}:
      o
    case HM.EMsgsMoved{f, t}:
      o
    case HM.ECardOpened{c}:
      o
    case HM.ECardClosed{a, b, c}:
      o
    case HM.ECardAnswerPos{a, b, c}:
      o
    case HM.ECardWithdrawn{a, b}:
      o
    case HM.ECardLate{a}:
      o
    case HM.EMainNote{a, b}:
      o
    case HM.ENotesFlushed{}:
      o
    case HM.ETimerSet{a, b}:
      o
    case HM.ETimerFired{a, b, c, d, f}:
      o
    case HM.ETimerRun{a, b}:
      o
    case HM.ETimerStop{a, b, c}:
      o
    case HM.ETimerName{a, b, c, d}:
      o
    case HM.EX{a}:
      o
    case HM.EAct{a}:
      o
    case HM.EFollow{a, b, c}:
      o
    case HM.EJobEnd{a, b}:
      o
    case HM.EWake{a}:
      o
    case HM.EPaused{a}:
      o
    case HM.EResumed{a}:
      o

def cs_keep(+s: HS.St, +e: HM.Ev, +x: Nat, -r: HS.MRec, +n: {HM.nat_in(N.Needs.msgs(N.needs(e)), x) == False{} : Bool},
    +u: HI.Split(s), +k: HX.FreshEv(s, e), +o: {HM.rec_of(HS.St.old(s), x) == Some{r} : Maybe<&2, HS.MRec>})
    -> {HM.rec_of(HS.St.old(HM.apply(s, e)), x) == Some{r} : Maybe<&2, HS.MRec>}:
  %HSL.St.open_eq(s) : {HM.rec_of(HS.St.old(HM.apply(_, e)), x) == Some{r} : Maybe<&2, HS.MRec>}
  cs_keep.go(e, s, x, r, n, u, k, o)

def Laws.apply_is_local_msg(s, e, x, r, u, k, o, n):
  %HSL.St.open_eq(s) : {HM.apply(N.drop_old(_, x), e) == N.drop_old(HM.apply(_, e), x) : HS.St}
  ail.go(e, s, x, r, n, u, k, o)

# ---- a batch: one ask before it, the same state ----

def cs_msgs_with(x: N.Needs(), +rn: List<&2, N.Rn()>) -> {N.Needs.msgs(N.needs_with.go(x, rn)) == N.Needs.msgs(x) : List<&2, Nat>}:
  match x:
    case Tuple{ms, ags}:
      {==}

def cs_msgs_union(a: N.Needs(), b: N.Needs()) -> {N.Needs.msgs(N.union(a, b)) == List.append(&2, Nat, N.Needs.msgs(a), N.Needs.msgs(b)) : List<&2, Nat>}:
  match a b:
    case Tuple{am, aa} Tuple{bm, ba}:
      {==}

def cs_nin_left(+a: List<&2, Nat>, -b: List<&2, Nat>, +x: Nat, +n: {HM.nat_in(List.append(&2, Nat, a, b), x) == False{} : Bool})
    -> {HM.nat_in(a, x) == False{} : Bool}:
  match a:
    case Nil{}:
      {==}
    case Con{h, t}:
      PL1.hub_lor_false(Nat.is_eq(h, x), HM.nat_in(t, x),
        PL1.hub_lor_false_l(Nat.is_eq(h, x), HM.nat_in(List.append(&2, Nat, t, b), x), n),
        cs_nin_left(t, b, x, PL1.hub_lor_false_r(Nat.is_eq(h, x), HM.nat_in(List.append(&2, Nat, t, b), x), n)))

def cs_nin_right(+a: List<&2, Nat>, +b: List<&2, Nat>, +x: Nat, +n: {HM.nat_in(List.append(&2, Nat, a, b), x) == False{} : Bool})
    -> {HM.nat_in(b, x) == False{} : Bool}:
  match a:
    case Nil{}:
      n
    case Con{h, t}:
      cs_nin_right(t, b, x, PL1.hub_lor_false_r(Nat.is_eq(h, x), HM.nat_in(List.append(&2, Nat, t, b), x), n))

def cs_fetch(+es: List<&2, HM.Ev>, +s: HS.St, +x: Nat, +r: HS.MRec, +rn: List<&2, N.Rn()>, +u: HI.Split(s), +f: HX.Fresh(es, s),
    +o: {HM.rec_of(HS.St.old(s), x) == Some{r} : Maybe<&2, HS.MRec>}, +n: {HM.nat_in(N.Needs.msgs(N.needs_batch.go(es, rn)), x) == False{} : Bool})
    -> {HM.apply_all(es, N.drop_old(s, x)) == N.drop_old(HM.apply_all(es, s), x) : HS.St}:
  match es:
    case Nil{}:
      {==}
    case Con{e, t}:
      match f:
        case HX.Both{k, ft}:
          +a = N.needs_with(e, rn)
          +b = N.needs_batch.go(t, N.renames(e, rn))
          +napp = Equal.trans(Bool, HM.nat_in(List.append(&2, Nat, N.Needs.msgs(a), N.Needs.msgs(b)), x), HM.nat_in(N.Needs.msgs(N.union(a, b)), x), False{},
            Equal.cong(List<&2, Nat>, Bool, v => HM.nat_in(v, x), List.append(&2, Nat, N.Needs.msgs(a), N.Needs.msgs(b)), N.Needs.msgs(N.union(a, b)),
              Equal.sym(List<&2, Nat>, N.Needs.msgs(N.union(a, b)), List.append(&2, Nat, N.Needs.msgs(a), N.Needs.msgs(b)), cs_msgs_union(a, b))), n)
          +ne = Equal.trans(Bool, HM.nat_in(N.Needs.msgs(N.needs(e)), x), HM.nat_in(N.Needs.msgs(a), x), False{},
            Equal.cong(List<&2, Nat>, Bool, v => HM.nat_in(v, x), N.Needs.msgs(N.needs(e)), N.Needs.msgs(a),
              Equal.sym(List<&2, Nat>, N.Needs.msgs(a), N.Needs.msgs(N.needs(e)), cs_msgs_with(N.needs(e), rn))),
            cs_nin_left(N.Needs.msgs(a), N.Needs.msgs(b), x, napp))
          +nt = cs_nin_right(N.Needs.msgs(a), N.Needs.msgs(b), x, napp)
          Equal.trans(HS.St, HM.apply_all(t, HM.apply(N.drop_old(s, x), e)), HM.apply_all(t, N.drop_old(HM.apply(s, e), x)), N.drop_old(HM.apply_all(t, HM.apply(s, e)), x),
            Equal.cong(HS.St, HS.St, w => HM.apply_all(t, w), HM.apply(N.drop_old(s, x), e), N.drop_old(HM.apply(s, e), x), Laws.apply_is_local_msg(s, e, x, r, u, k, o, ne)),
            cs_fetch(t, HM.apply(s, e), x, r, N.renames(e, rn), PC4.hc_split_apply(s, e, u, PC4.hc_fresh_ge(s, e, k)), ft, cs_keep(s, e, x, r, ne, u, k, o), nt))

def Laws.fetch_then_apply_list(s, es, x, r, u, f, o, n):
  cs_fetch(es, s, x, r, Nil{}, u, f, o, n)
