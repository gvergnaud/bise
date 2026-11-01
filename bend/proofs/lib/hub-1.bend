# bend/proofs/lib/hub-1.bend: helpers the hub proofs share (moved from
# PROOF.bend), layered: a lib calls only the ones before it.
import Base
import ../../vendor/json.bend as J
import ../../hub/model.bend as HM
import ../../hub/gen/state.bend as HS
import ../../hub/gen/state-laws.bend as HSL
import ../../hub/cx.bend as HX
import ../../hub/core.bend as HC
import ../../hub/inv.bend as HI
import ../../hub/wakes.bend as HWK
import ../../hub/timers.bend as HTM
import ../../hub/lazy.bend as HLZ

# ---- Switchboard hub ----

# a step context's state is the replay of its journal: the proof it carries
def hub_core_ok(-o: HS.St, c: HX.Core<o>) -> {HX.Core.d(o, c) == HM.apply_all(HX.journal(HX.Core.fx(o, c)), o) : HS.St}:
  match c:
    case HX.Core{d, fx, p, f, q, v}:
      p

def hub_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, -s: HS.St)
    -> {HM.apply_all(b, HM.apply_all(a, s)) == HM.apply_all(List.append(&2, HM.Ev, a, b), s) : HS.St}:
  match a:
    case Nil{}:
      {==}
    case Con{e, t}:
      hub_append(t, b, HM.apply(s, e))

def hub_commit_go(+h: HC.Hub, need: Maybe<&2, J.Json>, c: HX.Core<HC.Hub.d(h)>, +r: HX.R)
    -> {HC.Hub.d(HC.Turn.hub(HC.commit.go(HC.Hub.d(h), need, c, r, h)))
        == HM.apply_all(HX.journal(HC.Turn.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))), HC.Hub.d(h)) : HS.St}:
  match need:
    case Some{q}:
      {==}
    case None{}:
      hub_core_ok(HC.Hub.d(h), c)

def hub_commit_cx(+h: HC.Hub, cx: HX.Cx<HC.Hub.d(h)>)
    -> {HC.Hub.d(HC.Turn.hub(HC.commit.cx(HC.Hub.d(h), cx, h)))
        == HM.apply_all(HX.journal(HC.Turn.fx(HC.commit.cx(HC.Hub.d(h), cx, h))), HC.Hub.d(h)) : HS.St}:
  match cx:
    case HX.Cx{c, +r}:
      hub_commit_go(h, HX.R.need(r), c, r)

# what sb-core keeps of an input is the replay of its journal
def hub_commit_ok(+h: HC.Hub, +i: J.Json)
    -> {HC.Hub.d(HC.Turn.hub(HC.commit(h, i))) == HM.apply_all(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.Hub.d(h)) : HS.St}:
  hub_commit_cx(h, HC.step(h, i))

def hub_fresh_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, -s: HS.St, fa: HX.Fresh(a, s),
    fb: HX.Fresh(b, HM.apply_all(a, s))) -> HX.Fresh(List.append(&2, HM.Ev, a, b), s):
  match a:
    case Nil{}:
      fb
    case Con{e, t}:
      match fa:
        case HX.Both{x, y}:
          HX.Both{x, hub_fresh_append(t, b, HM.apply(s, e), y, fb)}

def hub_quiet_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, qa: HX.Quiet(a), qb: HX.Quiet(b))
    -> HX.Quiet(List.append(&2, HM.Ev, a, b)):
  match a:
    case Nil{}:
      qb
    case Con{e, t}:
      match qa:
        case HX.Both{x, y}:
          HX.Both{x, hub_quiet_append(t, b, y, qb)}

def hub_fresh_tr(-evs: List<&2, HM.Ev>, -x: HS.St, -y: HS.St, e: {x == y : HS.St}, f: HX.Fresh(evs, x)) -> HX.Fresh(evs, y):
  %e : HX.Fresh(evs, _)
  f

def hub_commit_go_f(+h: HC.Hub, need: Maybe<&2, J.Json>, c: HX.Core<HC.Hub.d(h)>, +r: HX.R)
    -> HX.Both<HX.Fresh(HX.journal(HC.Turn.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))), HC.Hub.d(h)),
               HX.Quiet(HX.journal(HC.Turn.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))))>:
  match need:
    case Some{q}:
      HX.Both{Unit{}, Unit{}}
    case None{}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          HX.Both{f, q}

def hub_commit_f.cx(+h: HC.Hub, cx: HX.Cx<HC.Hub.d(h)>)
    -> HX.Both<HX.Fresh(HX.journal(HC.Turn.fx(HC.commit.cx(HC.Hub.d(h), cx, h))), HC.Hub.d(h)),
               HX.Quiet(HX.journal(HC.Turn.fx(HC.commit.cx(HC.Hub.d(h), cx, h))))>:
  match cx:
    case HX.Cx{c, +r}:
      hub_commit_go_f(h, HX.R.need(r), c, r)

def hub_commit_f(+h: HC.Hub, +i: J.Json)
    -> HX.Both<HX.Fresh(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.Hub.d(h)), HX.Quiet(HX.journal(HC.Turn.fx(HC.commit(h, i))))>:
  hub_commit_f.cx(h, HC.step(h, i))

def hub_fst(-A: Data, -B: Data, x: HX.Both<A, B>) -> A:
  match x:
    case HX.Both{a, b}:
      a

def hub_snd(-A: Data, -B: Data, x: HX.Both<A, B>) -> B:
  match x:
    case HX.Both{a, b}:
      b

def hub_set_waiters(-o: HS.St, cx: HX.Cx<o>, +ws: List<&2, HX.Waiter>)
    -> {HC.waiters_of_cx(o, HC.set_waiters(o, cx, ws)) == ws : List<&2, HX.Waiter>}:
  match cx:
    case HX.Cx{c, r}:
      match r:
        case HX.R{a, b, st, d, e, f, g, h, i, j, k, l, m, n, vc}:
          {==}

def hub_later_tr(-x: List<&2, HX.Waiter>, -y: List<&2, HX.Waiter>, -now: Nat, e: {x == y : List<&2, HX.Waiter>},
    l: HC.Later(y, now)) -> HC.Later(x, now):
  %Equal.sym(List<&2, HX.Waiter>, x, y, e) : HC.Later(_, now)
  l

def hub_keep_later.pick(c: Bool, +w: HX.Waiter, -t: List<&2, HX.Waiter>, -now: Nat,
    +e: {Nat.is_le(HX.Waiter.deadline(w), now) == c : Bool}, ih: HC.Later(HX.waiters_keep(t, now), now))
    -> HC.Later(Bool.pick(List<&2, HX.Waiter>, Nat.is_le(HX.Waiter.deadline(w), now), HX.waiters_keep(t, now), w <> HX.waiters_keep(t, now)), now):
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_le(HX.Waiter.deadline(w), now), True{}, e) : HC.Later(Bool.pick(List<&2, HX.Waiter>, _, HX.waiters_keep(t, now), w <> HX.waiters_keep(t, now)), now)
      ih
    case False{}:
      %Equal.sym(Bool, Nat.is_le(HX.Waiter.deadline(w), now), False{}, e) : HC.Later(Bool.pick(List<&2, HX.Waiter>, _, HX.waiters_keep(t, now), w <> HX.waiters_keep(t, now)), now)
      HX.Both{e, ih}

def hub_keep_later(+ws: List<&2, HX.Waiter>, +now: Nat) -> HC.Later(HX.waiters_keep(ws, now), now):
  match ws:
    case Nil{}:
      Unit{}
    case Con{w, t}:
      hub_keep_later.pick(Nat.is_le(HX.Waiter.deadline(w), now), w, t, now, {==}, hub_keep_later(t, now))

def hub_none_tr(-x: List<&2, HX.Waiter>, -y: List<&2, HX.Waiter>, -n: String, e: {x == y : List<&2, HX.Waiter>},
    l: HC.NoneOf(y, n)) -> HC.NoneOf(x, n):
  %Equal.sym(List<&2, HX.Waiter>, x, y, e) : HC.NoneOf(_, n)
  l

def hub_not_none.pick(c: Bool, +w: HX.Waiter, -t: List<&2, HX.Waiter>, -n: String,
    +e: {String.eq(HX.Waiter.agent(w), n) == c : Bool}, ih: HC.NoneOf(HX.waiters_not(t, n), n))
    -> HC.NoneOf(Bool.pick(List<&2, HX.Waiter>, String.eq(HX.Waiter.agent(w), n), HX.waiters_not(t, n), w <> HX.waiters_not(t, n)), n):
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(HX.Waiter.agent(w), n), True{}, e) : HC.NoneOf(Bool.pick(List<&2, HX.Waiter>, _, HX.waiters_not(t, n), w <> HX.waiters_not(t, n)), n)
      ih
    case False{}:
      %Equal.sym(Bool, String.eq(HX.Waiter.agent(w), n), False{}, e) : HC.NoneOf(Bool.pick(List<&2, HX.Waiter>, _, HX.waiters_not(t, n), w <> HX.waiters_not(t, n)), n)
      HX.Both{e, ih}

def hub_not_none(+ws: List<&2, HX.Waiter>, +n: String) -> HC.NoneOf(HX.waiters_not(ws, n), n):
  match ws:
    case Nil{}:
      Unit{}
    case Con{w, t}:
      hub_not_none.pick(String.eq(HX.Waiter.agent(w), n), w, t, n, {==}, hub_not_none(t, n))

# ---- L2: each message is one record (msgs_unique) ----

def hub_bdisc(b: Bool) -> Type:
  match b:
    case True{}:
      Unit
    case False{}:
      Empty

def hub_ft_absurd(e: {False{} == True{} : Bool}) -> Empty:
  %Equal.sym(Bool, False{}, True{}, e) : hub_bdisc(_)
  Unit{}

def hub_absurd_eq(x: Empty, -A: Type, -a: A, -b: A) -> {a == b : A}:
  match x:

# y < x: x is not y
def hub_lt_neq(+y: Nat, +x: Nat, h: {Nat.is_lt(y, x) == True{} : Bool}) -> {Nat.is_eq(x, y) == False{} : Bool}:
  match y x:
    case 0n 0n:
      hub_absurd_eq(hub_ft_absurd(h), Bool, Nat.is_eq(x, y), False{})
    case 0n 1n+xp:
      {==}
    case 1n+yp 0n:
      hub_absurd_eq(hub_ft_absurd(h), Bool, Nat.is_eq(x, y), False{})
    case 1n+yp 1n+xp:
      hub_lt_neq(yp, xp, h)

# x < a and a <= b: x < b
def hub_lt_ge(+x: Nat, +a: Nat, +b: Nat, h: {Nat.is_lt(x, a) == True{} : Bool}, g: {Nat.is_ge(b, a) == True{} : Bool})
    -> {Nat.is_lt(x, b) == True{} : Bool}:
  match x a b:
    case _ 0n _:
      hub_absurd_eq(hub_ft_absurd(h), Bool, Nat.is_lt(x, b), True{})
    case 0n 1n+ap 0n:
      hub_absurd_eq(hub_ft_absurd(g), Bool, Nat.is_lt(x, b), True{})
    case 0n 1n+ap 1n+bp:
      {==}
    case 1n+xp 1n+ap 0n:
      hub_absurd_eq(hub_ft_absurd(g), Bool, Nat.is_lt(x, b), True{})
    case 1n+xp 1n+ap 1n+bp:
      hub_lt_ge(xp, ap, bp, h, g)

def hub_lt_succ(+n: Nat) -> {Nat.is_lt(n, 1n+n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      hub_lt_succ(p)

def hub_rec_op_id(+r: HS.MRec, op: HM.MsgOp, -rest: List<&2, Nat>)
    -> {HS.Msg.id(HS.MRec.msg(HM.rec_op(r, op))) <> rest == HS.Msg.id(HS.MRec.msg(r)) <> rest : List<&2, Nat>}:
  match op:
    case HM.OSt{x}:
      %HSL.MRec.st.eta(r, x) : {HS.Msg.id(HS.MRec.msg(_)) <> rest == HS.Msg.id(HS.MRec.msg(r)) <> rest : List<&2, Nat>}
      {==}
    case HM.OSettle{}:
      %HSL.MRec.settled.eta(r, True{}) : {HS.Msg.id(HS.MRec.msg(_)) <> rest == HS.Msg.id(HS.MRec.msg(r)) <> rest : List<&2, Nat>}
      {==}

def hub_ids_map.pick(c: Bool, +h: HS.MRec, -t: List<&2, HS.MRec>, -id: Nat, +op: HM.MsgOp,
    +e: {Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id) == c : Bool}, ih: {HI.ids(HM.map_msg(t, id, op)) == HI.ids(t) : List<&2, Nat>})
    -> {HI.ids(HLZ.pick(List<&2, HS.MRec>, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id), u => HM.rec_op(h, op) <> t, u => h <> HM.map_msg(t, id, op)))
        == HI.ids(h <> t) : List<&2, Nat>}:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id), True{}, e) : {HI.ids(HLZ.pick(List<&2, HS.MRec>, _, u => HM.rec_op(h, op) <> t, u => h <> HM.map_msg(t, id, op))) == HI.ids(h <> t) : List<&2, Nat>}
      hub_rec_op_id(h, op, HI.ids(t))
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id), False{}, e) : {HI.ids(HLZ.pick(List<&2, HS.MRec>, _, u => HM.rec_op(h, op) <> t, u => h <> HM.map_msg(t, id, op))) == HI.ids(h <> t) : List<&2, Nat>}
      Equal.cong(List<&2, Nat>, List<&2, Nat>, xs => HS.Msg.id(HS.MRec.msg(h)) <> xs, HI.ids(HM.map_msg(t, id, op)), HI.ids(t), ih)

# updating one record keeps every id in place
def hub_ids_map(+xs: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp) -> {HI.ids(HM.map_msg(xs, id, op)) == HI.ids(xs) : List<&2, Nat>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_ids_map.pick(Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), id), h, t, id, op, {==}, hub_ids_map(t, id, op))

# settling a question keeps every id in place
def hub_settle_if_ids(ok: Bool, +xs: List<&2, HS.MRec>, +id: Nat) -> {HI.ids(HI.settle1.if(ok, xs, id)) == HI.ids(xs) : List<&2, Nat>}:
  match ok:
    case True{}:
      hub_ids_map(xs, id, HM.OSettle{})
    case False{}:
      {==}

def hub_settle_go_ids(o: Maybe<&2, HS.MRec>, +xs: List<&2, HS.MRec>, +id: Nat, +from: String)
    -> {HI.ids(HI.settle1.go(o, xs, id, from)) == HI.ids(xs) : List<&2, Nat>}:
  match o:
    case None{}:
      {==}
    case Some{+r}:
      hub_settle_if_ids(String.eq(HS.Msg.to(HS.MRec.msg(r)), from) || String.eq(from, HM.USER()), xs, id)

def hub_settle_ids(r: Maybe<&2, Nat>, +xs: List<&2, HS.MRec>, +from: String)
    -> {HI.ids(HI.settle1(r, xs, from)) == HI.ids(xs) : List<&2, Nat>}:
  match r:
    case None{}:
      {==}
    case Some{+id}:
      hub_settle_go_ids(HM.rec_of(xs, id), xs, id, from)

def hub_ids_snoc(+xs: List<&2, HS.MRec>, +r: HS.MRec)
    -> {HI.ids(HM.snoc(HS.MRec, xs, r)) == HM.snoc(Nat, HI.ids(xs), HS.Msg.id(HS.MRec.msg(r))) : List<&2, Nat>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.cong(List<&2, Nat>, List<&2, Nat>, ys => HS.Msg.id(HS.MRec.msg(h)) <> ys,
        HI.ids(HM.snoc(HS.MRec, t, r)), HM.snoc(Nat, HI.ids(t), HS.Msg.id(HS.MRec.msg(r))), hub_ids_snoc(t, r))

def hub_tf_absurd(e: {True{} == False{} : Bool}) -> Empty:
  %e : hub_bdisc(_)
  Unit{}

def hub_or_false_l(a: Bool, -b: Bool, e: {Bool.or(a, b) == False{} : Bool}) -> {a == False{} : Bool}:
  match a:
    case False{}:
      {==}
    case True{}:
      hub_absurd_eq(hub_tf_absurd(e), Bool, True{}, False{})

def hub_or_false_r(a: Bool, -b: Bool, e: {Bool.or(a, b) == False{} : Bool}) -> {b == False{} : Bool}:
  match a:
    case False{}:
      e
    case True{}:
      hub_absurd_eq(hub_tf_absurd(e), Bool, b, False{})

def hub_or_false(a: Bool, -b: Bool, ea: {a == False{} : Bool}, eb: {b == False{} : Bool}) -> {Bool.or(a, b) == False{} : Bool}:
  %Equal.sym(Bool, a, False{}, ea) : {Bool.or(_, b) == False{} : Bool}
  eb

# the same three for the lazy or of a search (hub/lazy.bend: str_in, nat_in)
def hub_lor_false_l(a: Bool, -b: Bool, e: {HLZ.or(a, u => b) == False{} : Bool}) -> {a == False{} : Bool}:
  match a:
    case False{}:
      {==}
    case True{}:
      hub_absurd_eq(hub_tf_absurd(e), Bool, True{}, False{})

def hub_lor_false_r(a: Bool, -b: Bool, e: {HLZ.or(a, u => b) == False{} : Bool}) -> {b == False{} : Bool}:
  match a:
    case False{}:
      e
    case True{}:
      hub_absurd_eq(hub_tf_absurd(e), Bool, b, False{})

def hub_lor_false(a: Bool, -b: Bool, ea: {a == False{} : Bool}, eb: {b == False{} : Bool}) -> {HLZ.or(a, u => b) == False{} : Bool}:
  %Equal.sym(Bool, a, False{}, ea) : {HLZ.or(_, u => b) == False{} : Bool}
  eb

# y is neither in t nor x: it is not in t ++ [x]
def hub_nat_in_snoc(+t: List<&2, Nat>, +x: Nat, +y: Nat, +a: {HM.nat_in(t, y) == False{} : Bool}, b: {Nat.is_eq(x, y) == False{} : Bool})
    -> {HM.nat_in(HM.snoc(Nat, t, x), y) == False{} : Bool}:
  match t:
    case Nil{}:
      hub_lor_false(Nat.is_eq(x, y), False{}, b, {==})
    case Con{h, tt}:
      hub_lor_false(Nat.is_eq(h, y), HM.nat_in(HM.snoc(Nat, tt, x), y),
        hub_lor_false_l(Nat.is_eq(h, y), HM.nat_in(tt, y), a),
        hub_nat_in_snoc(tt, x, y, hub_lor_false_r(Nat.is_eq(h, y), HM.nat_in(tt, y), a), b))

def hub_distinct_snoc(+ns: List<&2, Nat>, +x: Nat, d: HI.Distinct(ns), bl: HI.Below(ns, x)) -> HI.Distinct(HM.snoc(Nat, ns, x)):
  match ns:
    case Nil{}:
      HX.Both{{==}, Unit{}}
    case Con{y, t}:
      match d bl:
        case HX.Both{dy, dt} HX.Both{ly, bt}:
          HX.Both{hub_nat_in_snoc(t, x, y, dy, hub_lt_neq(y, x, ly)), hub_distinct_snoc(t, x, dt, bt)}

def hub_below_mono(+ns: List<&2, Nat>, +a: Nat, +b: Nat, +g: {Nat.is_ge(b, a) == True{} : Bool}, bl: HI.Below(ns, a)) -> HI.Below(ns, b):
  match ns:
    case Nil{}:
      Unit{}
    case Con{y, t}:
      match bl:
        case HX.Both{ly, bt}:
          HX.Both{hub_lt_ge(y, a, b, ly, g), hub_below_mono(t, a, b, g, bt)}

def hub_below_snoc(+ns: List<&2, Nat>, +x: Nat, +n: Nat, bl: HI.Below(ns, n), lx: {Nat.is_lt(x, n) == True{} : Bool})
    -> HI.Below(HM.snoc(Nat, ns, x), n):
  match ns:
    case Nil{}:
      HX.Both{lx, Unit{}}
    case Con{y, t}:
      match bl:
        case HX.Both{ly, bt}:
          HX.Both{ly, hub_below_snoc(t, x, n, bt, lx)}

def hub_distinct_tr(-x: List<&2, Nat>, -y: List<&2, Nat>, e: {x == y : List<&2, Nat>}, d: HI.Distinct(y)) -> HI.Distinct(x):
  %Equal.sym(List<&2, Nat>, x, y, e) : HI.Distinct(_)
  d

def hub_below_tr(-x: List<&2, Nat>, -y: List<&2, Nat>, -n: Nat, e: {x == y : List<&2, Nat>}, b: HI.Below(y, n)) -> HI.Below(x, n):
  %Equal.sym(List<&2, Nat>, x, y, e) : HI.Below(_, n)
  b

def hub_below_n(-ns: List<&2, Nat>, -a: Nat, -b: Nat, e: {a == b : Nat}, bl: HI.Below(ns, b)) -> HI.Below(ns, a):
  %Equal.sym(Nat, a, b, e) : HI.Below(ns, _)
  bl

# an update of one record keeps the state unique
def hub_unique_map(+ms: List<&2, HS.MRec>, +id: Nat, +op: HM.MsgOp, -nm: Nat,
    u: HX.Both<HI.Distinct(HI.ids(ms)), HI.Below(HI.ids(ms), nm)>)
    -> HX.Both<HI.Distinct(HI.ids(HM.map_msg(ms, id, op))), HI.Below(HI.ids(HM.map_msg(ms, id, op)), nm)>:
  match u:
    case HX.Both{d, b}:
      HX.Both{hub_distinct_tr(HI.ids(HM.map_msg(ms, id, op)), HI.ids(ms), hub_ids_map(ms, id, op), d),
        hub_below_tr(HI.ids(HM.map_msg(ms, id, op)), HI.ids(ms), nm, hub_ids_map(ms, id, op), b)}

# a new message with the next free id keeps the state unique
def hub_unique_sent(+ms: List<&2, HS.MRec>, +m: HS.Msg, +nm: Nat, k: {HS.Msg.id(m) == nm : Nat},
    u: HX.Both<HI.Distinct(HI.ids(ms)), HI.Below(HI.ids(ms), nm)>)
    -> HX.Both<HI.Distinct(HI.ids(HI.settle1(HS.Msg.reply_to(m), HM.snoc(HS.MRec, ms, HM.new_rec(m)), HS.Msg.from(m)))),
               HI.Below(HI.ids(HI.settle1(HS.Msg.reply_to(m), HM.snoc(HS.MRec, ms, HM.new_rec(m)), HS.Msg.from(m))),
                 Nat.max(Nat.max(nm, 1n+HS.Msg.id(m)), 1n+HS.Msg.thread(m)))>:
  match u:
    case HX.Both{+d, +b}:
      +xs = HM.snoc(HS.MRec, ms, HM.new_rec(m))
      +a = Nat.max(nm, 1n+HS.Msg.id(m))
      +top = Nat.max(a, 1n+HS.Msg.thread(m))
      +e = Equal.trans(List<&2, Nat>, HI.ids(HI.settle1(HS.Msg.reply_to(m), xs, HS.Msg.from(m))), HI.ids(xs),
        HM.snoc(Nat, HI.ids(ms), HS.Msg.id(m)),
        hub_settle_ids(HS.Msg.reply_to(m), xs, HS.Msg.from(m)), hub_ids_snoc(ms, HM.new_rec(m)))
      +lt = hub_lt_ge(HS.Msg.id(m), a, top,
        hub_lt_ge(HS.Msg.id(m), 1n+HS.Msg.id(m), a, hub_lt_succ(HS.Msg.id(m)), Nat.max_ge_r(nm, 1n+HS.Msg.id(m))),
        Nat.max_ge_l(a, 1n+HS.Msg.thread(m)))
      HX.Both{hub_distinct_tr(HI.ids(HI.settle1(HS.Msg.reply_to(m), xs, HS.Msg.from(m))), HM.snoc(Nat, HI.ids(ms), HS.Msg.id(m)), e,
          hub_distinct_snoc(HI.ids(ms), HS.Msg.id(m), d, hub_below_n(HI.ids(ms), HS.Msg.id(m), nm, k, b))),
        hub_below_tr(HI.ids(HI.settle1(HS.Msg.reply_to(m), xs, HS.Msg.from(m))), HM.snoc(Nat, HI.ids(ms), HS.Msg.id(m)), top, e,
          hub_below_snoc(HI.ids(ms), HS.Msg.id(m), top,
            hub_below_mono(HI.ids(ms), a, top, Nat.max_ge_l(a, 1n+HS.Msg.thread(m)),
              hub_below_mono(HI.ids(ms), nm, a, Nat.max_ge_l(nm, 1n+HS.Msg.id(m)), b)), lt))}

# an event keeps the state unique: hub-cold.bend (hc_apply_unique), over
# every message of the split
def hub_unique_tr(-x: HS.St, -y: HS.St, e: {x == y : HS.St}, u: HI.Unique(y)) -> HI.Unique(x):
  %Equal.sym(HS.St, x, y, e) : HI.Unique(_)
  u

# ---- L2: a message once sent is never removed (msgs_kept) ----

def hub_prefix_refl(+a: List<&2, Nat>) -> HI.IsPrefix(a, a):
  match a:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      HX.Both{{==}, hub_prefix_refl(t)}

def hub_prefix_snoc(+a: List<&2, Nat>, +b: List<&2, Nat>, -x: Nat, p: HI.IsPrefix(a, b)) -> HI.IsPrefix(a, HM.snoc(Nat, b, x)):
  match a b:
    case Nil{} _:
      Unit{}
    case Con{y, t} Nil{}:
      match p:
    case Con{y, t} Con{z, u}:
      match p:
        case HX.Both{e, pt}:
          HX.Both{e, hub_prefix_snoc(t, u, x, pt)}

def hub_prefix_trans(+a: List<&2, Nat>, +b: List<&2, Nat>, +c: List<&2, Nat>, p: HI.IsPrefix(a, b), q: HI.IsPrefix(b, c)) -> HI.IsPrefix(a, c):
  match a b c:
    case Nil{} _ _:
      Unit{}
    case Con{x, t} Nil{} _:
      match p:
    case Con{x, t} Con{y, u} Nil{}:
      match q:
    case Con{x, t} Con{y, u} Con{z, v}:
      match p q:
        case HX.Both{e1, pt} HX.Both{e2, qt}:
          HX.Both{Equal.trans(Nat, x, y, z, e1, e2), hub_prefix_trans(t, u, v, pt, qt)}

def hub_prefix_tr(-a: List<&2, Nat>, -x: List<&2, Nat>, -y: List<&2, Nat>, e: {x == y : List<&2, Nat>}, p: HI.IsPrefix(a, y)) -> HI.IsPrefix(a, x):
  %Equal.sym(List<&2, Nat>, x, y, e) : HI.IsPrefix(a, _)
  p

# every event keeps the messages in order: hub-cold.bend (hc_apply_kept),
# over every message of the split

def hub_kept_tr(-a: HS.St, -x: HS.St, -y: HS.St, e: {x == y : HS.St}, k: HI.Kept(a, y)) -> HI.Kept(a, x):
  %Equal.sym(HS.St, x, y, e) : HI.Kept(a, _)
  k

# ---- L3: delivered only from the queue (delivered_once) ----

def hub_deliv_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, -s: HS.St, fa: HX.Deliv(a, s),
    fb: HX.Deliv(b, HM.apply_all(a, s))) -> HX.Deliv(List.append(&2, HM.Ev, a, b), s):
  match a:
    case Nil{}:
      fb
    case Con{e, t}:
      match fa:
        case HX.Both{x, y}:
          HX.Both{x, hub_deliv_append(t, b, HM.apply(s, e), y, fb)}

def hub_deliv_tr(-evs: List<&2, HM.Ev>, -x: HS.St, -y: HS.St, e: {x == y : HS.St}, f: HX.Deliv(evs, x)) -> HX.Deliv(evs, y):
  %e : HX.Deliv(evs, _)
  f

def hub_commit_go_v(+h: HC.Hub, need: Maybe<&2, J.Json>, c: HX.Core<HC.Hub.d(h)>, +r: HX.R)
    -> HX.Deliv(HX.journal(HC.Turn.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))), HC.Hub.d(h)):
  match need:
    case Some{q}:
      Unit{}
    case None{}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          v

def hub_commit_v.cx(+h: HC.Hub, cx: HX.Cx<HC.Hub.d(h)>)
    -> HX.Deliv(HX.journal(HC.Turn.fx(HC.commit.cx(HC.Hub.d(h), cx, h))), HC.Hub.d(h)):
  match cx:
    case HX.Cx{c, +r}:
      hub_commit_go_v(h, HX.R.need(r), c, r)

# ---- L12: a stop leaves no queued message and no card of the task ----

def hub_d_with_r(-o: HS.St, cx: HX.Cx<o>, -r: HX.R) -> {HX.d(o, HX.with_r(o, cx, r)) == HX.d(o, cx) : HS.St}:
  match cx:
    case HX.Cx{c, r0}:
      {==}

def hub_d_emit(-o: HS.St, cx: HX.Cx<o>, -e: HM.Ev, -k: HX.FreshEv(HX.d(o, cx), e), -kq: HX.QuietEv(e), -kv: HX.DelivEv(HX.d(o, cx), e))
    -> {HX.d(o, HX.emit(o, cx, e, k, kq, kv)) == HM.apply(HX.d(o, cx), e) : HS.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_set_rt(-o: HS.St, +cx: HX.Cx<o>, +x: HX.Rt) -> {HX.d(o, HC.set_rt(o, cx, x)) == HX.d(o, cx) : HS.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_if_agent(-o: HS.St, known: Bool, +cx: HX.Cx<o>, +x: HX.Rt) -> {HX.d(o, HC.if_agent(o, known, cx, x)) == HX.d(o, cx) : HS.St}:
  match known:
    case True{}:
      hub_d_set_rt(o, cx, x)
    case False{}:
      {==}

def hub_d_upd_rt(-o: HS.St, +cx: HX.Cx<o>, +x: HX.Rt) -> {HX.d(o, HC.upd_rt(o, cx, x)) == HX.d(o, cx) : HS.St}:
  hub_d_if_agent(o, HC.has_agent(o, cx, HX.Rt.name(x)), cx, x)

def hub_d_reply_err(-o: HS.St, +cx: HX.Cx<o>, +t: Nat, +why: String) -> {HX.d(o, HC.reply_err(o, cx, t, why)) == HX.d(o, cx) : HS.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_reply_all(-o: HS.St, +ws: List<&2, HX.Waiter>, +cx: HX.Cx<o>, +why: String) -> {HX.d(o, HC.reply_all(o, ws, cx, why)) == HX.d(o, cx) : HS.St}:
  match ws:
    case Nil{}:
      {==}
    case Con{w, rest}:
      Equal.trans(HS.St, HX.d(o, HC.reply_all(o, rest, HC.reply_err(o, cx, HX.Waiter.token(w), why), why)),
        HX.d(o, HC.reply_err(o, cx, HX.Waiter.token(w), why)), HX.d(o, cx),
        hub_d_reply_all(o, rest, HC.reply_err(o, cx, HX.Waiter.token(w), why), why),
        hub_d_reply_err(o, cx, HX.Waiter.token(w), why))

def hub_d_fail_waiters(-o: HS.St, +cx: HX.Cx<o>, +a: String, +why: String) -> {HX.d(o, HC.fail_waiters(o, cx, a, why)) == HX.d(o, cx) : HS.St}:
  +ws = HX.R.waiters(HC.rr(o, cx))
  +cx2 = HC.set_r(o, cx, HX.R.with_waiters(HC.rr(o, cx), HX.waiters_not(ws, a)))
  +cx3 = HC.reply_all(o, HX.waiters_of(ws, a), cx2, why)
  Equal.trans(HS.St, HX.d(o, HC.upd_rt(o, cx3, HX.Rt.with_waiting(HC.rt(o, cx3, a), False{}))), HX.d(o, cx3), HX.d(o, cx),
    hub_d_upd_rt(o, cx3, HX.Rt.with_waiting(HC.rt(o, cx3, a), False{})),
    Equal.trans(HS.St, HX.d(o, cx3), HX.d(o, cx2), HX.d(o, cx),
      hub_d_reply_all(o, HX.waiters_of(ws, a), cx2, why),
      hub_d_with_r(o, cx, HX.R.with_waiters(HC.rr(o, cx), HX.waiters_not(ws, a)))))

def hub_eq_refl(+n: Nat) -> {Nat.is_eq(n, n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      hub_eq_refl(p)

def hub_eq_sym(+a: Nat, +b: Nat) -> {Nat.is_eq(a, b) == Nat.is_eq(b, a) : Bool}:
  match a b:
    case 0n 0n:
      {==}
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      {==}
    case 1n+ap 1n+bp:
      hub_eq_sym(ap, bp)

# the records after the rejections of stop_task
def hub_rej(ids: List<&2, Nat>, +xs: List<&2, HS.MRec>) -> List<&2, HS.MRec>:
  match ids:
    case Nil{}:
      xs
    case Con{+id, +t}:
      hub_rej(t, HM.map_msg(xs, id, HM.OSt{HS.Rejected{"recipient_unavailable"}}))

# every message after stop_task's rejections: hub-cold.bend
# (hc_msgs_reject), with the split's invariant

def hub_rej_skip.go(+i: Nat, -ns: List<&2, Nat>, +h: HS.MRec, +t: List<&2, HS.MRec>,
    ef: {Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), i) == False{} : Bool},
    ih: {hub_rej(ns, h <> HM.map_msg(t, i, HM.OSt{HS.Rejected{"recipient_unavailable"}})) == h <> hub_rej(ns, HM.map_msg(t, i, HM.OSt{HS.Rejected{"recipient_unavailable"}})) : List<&2, HS.MRec>})
    -> {hub_rej(i <> ns, h <> t) == h <> hub_rej(i <> ns, t) : List<&2, HS.MRec>}:
  %Equal.sym(Bool, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), i), False{}, ef) : {hub_rej(ns, HLZ.pick(List<&2, HS.MRec>, _, u => HM.rec_op(h, HM.OSt{HS.Rejected{"recipient_unavailable"}}) <> t, u => h <> HM.map_msg(t, i, HM.OSt{HS.Rejected{"recipient_unavailable"}}))) == h <> hub_rej(i <> ns, t) : List<&2, HS.MRec>}
  ih

# a record whose id is not rejected stays in place
def hub_rej_skip(+ids: List<&2, Nat>, +h: HS.MRec, +t: List<&2, HS.MRec>, +nf: {HM.nat_in(ids, HS.Msg.id(HS.MRec.msg(h))) == False{} : Bool})
    -> {hub_rej(ids, h <> t) == h <> hub_rej(ids, t) : List<&2, HS.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{i, ns}:
      hub_rej_skip.go(i, ns, h, t,
        Equal.trans(Bool, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), i), Nat.is_eq(i, HS.Msg.id(HS.MRec.msg(h))), False{},
          hub_eq_sym(HS.Msg.id(HS.MRec.msg(h)), i),
          hub_lor_false_l(Nat.is_eq(i, HS.Msg.id(HS.MRec.msg(h))), HM.nat_in(ns, HS.Msg.id(HS.MRec.msg(h))), nf)),
        hub_rej_skip(ns, h, HM.map_msg(t, i, HM.OSt{HS.Rejected{"recipient_unavailable"}}),
          hub_lor_false_r(Nat.is_eq(i, HS.Msg.id(HS.MRec.msg(h))), HM.nat_in(ns, HS.Msg.id(HS.MRec.msg(h))), nf)))

# queued_in's step (model.bend queued_in.keep, the queued test matched
# first) is the pick on "queued and to one of `ns`"
def hub_qkeep(q: Bool, -ns: List<&2, String>, -m: HS.Msg, -rest: List<&2, HS.Msg>)
    -> {Bool.pick(List<&2, HS.Msg>, HLZ.and(q, u => HM.str_in(ns, HS.Msg.to(m))), m <> rest, rest) == HM.queued_in.keep(q, ns, m, rest) : List<&2, HS.Msg>}:
  match q:
    case True{}:
      {==}
    case False{}:
      {==}

def hub_q_sub.pick(c: Bool, +h: HS.MRec, -tt: List<&2, HS.MRec>, -n: List<&2, String>, +x: Nat,
    +e: {HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))) == c : Bool},
    e1: {Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), x) == False{} : Bool},
    ih: {HM.nat_in(HC.ids_of(HM.queued_in(tt, n)), x) == False{} : Bool})
    -> {HM.nat_in(HC.ids_of(Bool.pick(List<&2, HS.Msg>, HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))),
          HS.MRec.msg(h) <> HM.queued_in(tt, n), HM.queued_in(tt, n))), x) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(Bool, HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))), True{}, e) : {HM.nat_in(HC.ids_of(Bool.pick(List<&2, HS.Msg>, _, HS.MRec.msg(h) <> HM.queued_in(tt, n), HM.queued_in(tt, n))), x) == False{} : Bool}
      hub_lor_false(Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), x), HM.nat_in(HC.ids_of(HM.queued_in(tt, n)), x), e1, ih)
    case False{}:
      %Equal.sym(Bool, HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))), False{}, e) : {HM.nat_in(HC.ids_of(Bool.pick(List<&2, HS.Msg>, _, HS.MRec.msg(h) <> HM.queued_in(tt, n), HM.queued_in(tt, n))), x) == False{} : Bool}
      ih

# the queued ids of a list are among its ids
def hub_q_sub(+t: List<&2, HS.MRec>, +x: Nat, +n: List<&2, String>, +nf: {HM.nat_in(HI.ids(t), x) == False{} : Bool})
    -> {HM.nat_in(HC.ids_of(HM.queued_in(t, n)), x) == False{} : Bool}:
  match t:
    case Nil{}:
      {==}
    case Con{h, tt}:
      %hub_qkeep(HM.is_queued(HS.MRec.st(h)), n, HS.MRec.msg(h), HM.queued_in(tt, n)) : {HM.nat_in(HC.ids_of(_), x) == False{} : Bool}
      hub_q_sub.pick(HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))), h, tt, n, x, {==},
        hub_lor_false_l(Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), x), HM.nat_in(HI.ids(tt), x), nf),
        hub_q_sub(tt, x, n, hub_lor_false_r(Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), x), HM.nat_in(HI.ids(tt), x), nf)))

def hub_and_left(a: Bool, -b: Bool, e: {Bool.and(a, b) == True{} : Bool}) -> {a == True{} : Bool}:
  match a:
    case True{}:
      {==}
    case False{}:
      hub_absurd_eq(hub_ft_absurd(e), Bool, False{}, True{})

# a record's message is its message after a new state (its eta, no record written by position)
def hub_msg_with_st(+h: HS.MRec, +v: HS.MSt) -> {HS.MRec.msg(HS.MRec.with_st(h, v)) == HS.MRec.msg(h) : HS.Msg}:
  %HSL.MRec.st.eta(h, v) : {HS.MRec.msg(_) == HS.MRec.msg(h) : HS.Msg}
  {==}

def hub_K.nf(+h: HS.MRec, +t: List<&2, HS.MRec>, +n: List<&2, String>, +dh: {HM.nat_in(HI.ids(t), HS.Msg.id(HS.MRec.msg(h))) == False{} : Bool})
    -> {HM.nat_in(HC.ids_of(HM.queued_in(t, n)), HS.Msg.id(HS.MRec.msg(HS.MRec.with_st(h, HS.Rejected{"recipient_unavailable"})))) == False{} : Bool}:
  %Equal.sym(HS.Msg, HS.MRec.msg(HS.MRec.with_st(h, HS.Rejected{"recipient_unavailable"})), HS.MRec.msg(h), hub_msg_with_st(h, HS.Rejected{"recipient_unavailable"})) : {HM.nat_in(HC.ids_of(HM.queued_in(t, n)), HS.Msg.id(_)) == False{} : Bool}
  hub_q_sub(t, HS.Msg.id(HS.MRec.msg(h)), n, dh)

def hub_K.pick(c: Bool, +h: HS.MRec, +t: List<&2, HS.MRec>, +n: List<&2, String>,
    +e: {HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))) == c : Bool}, +dh: {HM.nat_in(HI.ids(t), HS.Msg.id(HS.MRec.msg(h))) == False{} : Bool},
    ih: {HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n) == Nil{} : List<&2, HS.Msg>})
    -> {HM.queued_in(hub_rej(HC.ids_of(Bool.pick(List<&2, HS.Msg>, HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))), HS.MRec.msg(h) <> HM.queued_in(t, n), HM.queued_in(t, n))), h <> t), n) == Nil{} : List<&2, HS.Msg>}:
  match c:
    case True{}:
      %Equal.sym(Bool, HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))), True{}, e) : {HM.queued_in(hub_rej(HC.ids_of(Bool.pick(List<&2, HS.Msg>, _, HS.MRec.msg(h) <> HM.queued_in(t, n), HM.queued_in(t, n))), h <> t), n) == Nil{} : List<&2, HS.Msg>}
      %Equal.sym(Bool, Nat.is_eq(HS.Msg.id(HS.MRec.msg(h)), HS.Msg.id(HS.MRec.msg(h))), True{}, hub_eq_refl(HS.Msg.id(HS.MRec.msg(h)))) : {HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), HLZ.pick(List<&2, HS.MRec>, _, u => HM.rec_op(h, HM.OSt{HS.Rejected{"recipient_unavailable"}}) <> t, u => h <> HM.map_msg(t, HS.Msg.id(HS.MRec.msg(h)), HM.OSt{HS.Rejected{"recipient_unavailable"}}))), n) == Nil{} : List<&2, HS.Msg>}
      %Equal.sym(List<&2, HS.MRec>, hub_rej(HC.ids_of(HM.queued_in(t, n)), HS.MRec.with_st(h, HS.Rejected{"recipient_unavailable"}) <> t), HS.MRec.with_st(h, HS.Rejected{"recipient_unavailable"}) <> hub_rej(HC.ids_of(HM.queued_in(t, n)), t), hub_rej_skip(HC.ids_of(HM.queued_in(t, n)), HS.MRec.with_st(h, HS.Rejected{"recipient_unavailable"}), t, hub_K.nf(h, t, n, dh))) : {HM.queued_in(_, n) == Nil{} : List<&2, HS.Msg>}
      %Equal.sym(HS.MSt, HS.MRec.st(HS.MRec.with_st(h, HS.Rejected{"recipient_unavailable"})), HS.Rejected{"recipient_unavailable"}, HSL.MRec.st.get_with(h, HS.Rejected{"recipient_unavailable"})) : {HM.queued_in.keep(HM.is_queued(_), n, HS.MRec.msg(HS.MRec.with_st(h, HS.Rejected{"recipient_unavailable"})), HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n)) == Nil{} : List<&2, HS.Msg>}
      ih
    case False{}:
      %Equal.sym(Bool, HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))), False{}, e) : {HM.queued_in(hub_rej(HC.ids_of(Bool.pick(List<&2, HS.Msg>, _, HS.MRec.msg(h) <> HM.queued_in(t, n), HM.queued_in(t, n))), h <> t), n) == Nil{} : List<&2, HS.Msg>}
      %Equal.sym(List<&2, HS.MRec>, hub_rej(HC.ids_of(HM.queued_in(t, n)), h <> t), h <> hub_rej(HC.ids_of(HM.queued_in(t, n)), t), hub_rej_skip(HC.ids_of(HM.queued_in(t, n)), h, t, hub_q_sub(t, HS.Msg.id(HS.MRec.msg(h)), n, dh))) : {HM.queued_in(_, n) == Nil{} : List<&2, HS.Msg>}
      %hub_qkeep(HM.is_queued(HS.MRec.st(h)), n, HS.MRec.msg(h), HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n)) : {_ == Nil{} : List<&2, HS.Msg>}
      %Equal.sym(Bool, HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))), False{}, e) : {Bool.pick(List<&2, HS.Msg>, _, HS.MRec.msg(h) <> HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n), HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n)) == Nil{} : List<&2, HS.Msg>}
      ih

# rejecting every message queued for `n` leaves none queued for it
def hub_K(+xs: List<&2, HS.MRec>, +n: List<&2, String>, d: HI.Distinct(HI.ids(xs))) -> {HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(xs, n)), xs), n) == Nil{} : List<&2, HS.Msg>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match d:
        case HX.Both{dh, dt}:
          %hub_qkeep(HM.is_queued(HS.MRec.st(h)), n, HS.MRec.msg(h), HM.queued_in(t, n)) : {HM.queued_in(hub_rej(HC.ids_of(_), h <> t), n) == Nil{} : List<&2, HS.Msg>}
          hub_K.pick(HLZ.and(HM.is_queued(HS.MRec.st(h)), u => HM.str_in(n, HS.Msg.to(HS.MRec.msg(h)))), h, t, n, {==}, dh, hub_K(t, n, dt))

def hub_d_hout(-o: HS.St, +cx: HX.Cx<o>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> {HX.d(o, HC.out(o, cx, kvs)) == HX.d(o, cx) : HS.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_msgs_closed(+s: HS.St, +id: Nat, +res: String) -> {HM.every(HM.apply(s, HM.ECardClosed{id, res, HS.Ans.empty()})) == HM.every(s) : List<&2, HS.MRec>}:
  %HSL.St.open_eq(s) : {HM.every(HM.apply(_, HM.ECardClosed{id, res, HS.Ans.empty()})) == HM.every(s) : List<&2, HS.MRec>}
  {==}

def hub_msgs_life(+s: HS.St, +n: String, +l: HS.Life, +r: Maybe<&2, String>) -> {HM.every(HM.apply(s, HM.ELifecycle{n, l, r})) == HM.every(s) : List<&2, HS.MRec>}:
  %HSL.St.open_eq(s) : {HM.every(HM.apply(_, HM.ELifecycle{n, l, r})) == HM.every(s) : List<&2, HS.MRec>}
  {==}

def hub_msgs_eq(-o: HS.St, -a: HX.Cx<o>, -b: HX.Cx<o>, e: {HX.d(o, a) == HX.d(o, b) : HS.St}) -> {HM.every(HX.d(o, a)) == HM.every(HX.d(o, b)) : List<&2, HS.MRec>}:
  Equal.cong(HS.St, List<&2, HS.MRec>, s => HM.every(s), HX.d(o, a), HX.d(o, b), e)

# the agents split (hub/agents.bend): every agent, St.agents then arch
# (HI.NamesApart's list, what HM.get_agent reads)
def hub_ag(+s: HS.St) -> List<&2, HS.Agent>:
  List.append(&2, HS.Agent, HS.St.agents(s), HS.St.arch(s))

# ---- a drop's timer stops (HC.timers_gone) touch no message, agent or card ----

def hub_msgs_tstop(+s: HS.St, +id: Nat, +why: String, +at: Nat) -> {HM.every(HM.apply(s, HM.ETimerStop{id, why, at})) == HM.every(s) : List<&2, HS.MRec>}:
  %HSL.St.open_eq(s) : {HM.every(HM.apply(_, HM.ETimerStop{id, why, at})) == HM.every(s) : List<&2, HS.MRec>}
  {==}

def hub_agents_tstop(+s: HS.St, +id: Nat, +why: String, +at: Nat) -> {hub_ag(HM.apply(s, HM.ETimerStop{id, why, at})) == hub_ag(s) : List<&2, HS.Agent>}:
  %HSL.St.open_eq(s) : {hub_ag(HM.apply(_, HM.ETimerStop{id, why, at})) == hub_ag(s) : List<&2, HS.Agent>}
  {==}

def hub_cards_tstop(+s: HS.St, +id: Nat, +why: String, +at: Nat) -> {HS.St.cards(HM.apply(s, HM.ETimerStop{id, why, at})) == HS.St.cards(s) : List<&2, HS.Card>}:
  %HSL.St.open_eq(s) : {HS.St.cards(HM.apply(_, HM.ETimerStop{id, why, at})) == HS.St.cards(s) : List<&2, HS.Card>}
  {==}

def hub_msgs_gone_each(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +why: String)
    -> {HM.every(HX.d(o, HC.timers_gone.each(o, ids, cx, why))) == HM.every(HX.d(o, cx)) : List<&2, HS.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.ETimerStop{id, why, HC.now(o, cx)} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, HC.timers_gone.each(o, t, cx2, why))), HM.every(HX.d(o, cx2)), HM.every(HX.d(o, cx)),
        hub_msgs_gone_each(o, t, cx2, why),
        Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, cx2)), HM.every(HM.apply(HX.d(o, cx), ev)), HM.every(HX.d(o, cx)),
          Equal.cong(HS.St, List<&2, HS.MRec>, s => HM.every(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_msgs_tstop(HX.d(o, cx), id, why, HC.now(o, cx))))

def hub_agents_gone_each(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +why: String)
    -> {hub_ag(HX.d(o, HC.timers_gone.each(o, ids, cx, why))) == hub_ag(HX.d(o, cx)) : List<&2, HS.Agent>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.ETimerStop{id, why, HC.now(o, cx)} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HS.Agent>, hub_ag(HX.d(o, HC.timers_gone.each(o, t, cx2, why))), hub_ag(HX.d(o, cx2)), hub_ag(HX.d(o, cx)),
        hub_agents_gone_each(o, t, cx2, why),
        Equal.trans(List<&2, HS.Agent>, hub_ag(HX.d(o, cx2)), hub_ag(HM.apply(HX.d(o, cx), ev)), hub_ag(HX.d(o, cx)),
          Equal.cong(HS.St, List<&2, HS.Agent>, s => hub_ag(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_agents_tstop(HX.d(o, cx), id, why, HC.now(o, cx))))

def hub_cards_gone_each(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +why: String)
    -> {HS.St.cards(HX.d(o, HC.timers_gone.each(o, ids, cx, why))) == HS.St.cards(HX.d(o, cx)) : List<&2, HS.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.ETimerStop{id, why, HC.now(o, cx)} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, HC.timers_gone.each(o, t, cx2, why))), HS.St.cards(HX.d(o, cx2)), HS.St.cards(HX.d(o, cx)),
        hub_cards_gone_each(o, t, cx2, why),
        Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, cx2)), HS.St.cards(HM.apply(HX.d(o, cx), ev)), HS.St.cards(HX.d(o, cx)),
          Equal.cong(HS.St, List<&2, HS.Card>, s => HS.St.cards(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_cards_tstop(HX.d(o, cx), id, why, HC.now(o, cx))))

def hub_cards_wake(+s: HS.St, +e: HWK.WkEv) -> {HS.St.cards(HM.apply(s, HM.EWake{e})) == HS.St.cards(s) : List<&2, HS.Card>}:
  %HSL.St.open_eq(s) : {HS.St.cards(HM.apply(_, HM.EWake{e})) == HS.St.cards(s) : List<&2, HS.Card>}
  {==}

# a drop's watch ends leave the cards as they were
def hub_cards_wakes_each(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>)
    -> {HS.St.cards(HX.d(o, HC.wakes_gone.each(o, ids, cx))) == HS.St.cards(HX.d(o, cx)) : List<&2, HS.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.EWake{HWK.WEnd{id, "gone", HC.now(o, cx)}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, HC.wakes_gone.each(o, t, cx2))), HS.St.cards(HX.d(o, cx2)), HS.St.cards(HX.d(o, cx)),
        hub_cards_wakes_each(o, t, cx2),
        Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, cx2)), HS.St.cards(HM.apply(HX.d(o, cx), ev)), HS.St.cards(HX.d(o, cx)),
          Equal.cong(HS.St, List<&2, HS.Card>, s => HS.St.cards(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_cards_wake(HX.d(o, cx), HWK.WEnd{id, "gone", HC.now(o, cx)})))

def hub_msgs_wake(+s: HS.St, +e: HWK.WkEv) -> {HM.every(HM.apply(s, HM.EWake{e})) == HM.every(s) : List<&2, HS.MRec>}:
  %HSL.St.open_eq(s) : {HM.every(HM.apply(_, HM.EWake{e})) == HM.every(s) : List<&2, HS.MRec>}
  {==}

# a drop's watch ends leave the messages and the agents as they were
def hub_msgs_wakes_each(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>)
    -> {HM.every(HX.d(o, HC.wakes_gone.each(o, ids, cx))) == HM.every(HX.d(o, cx)) : List<&2, HS.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.EWake{HWK.WEnd{id, "gone", HC.now(o, cx)}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, HC.wakes_gone.each(o, t, cx2))), HM.every(HX.d(o, cx2)), HM.every(HX.d(o, cx)),
        hub_msgs_wakes_each(o, t, cx2),
        Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, cx2)), HM.every(HM.apply(HX.d(o, cx), ev)), HM.every(HX.d(o, cx)),
          Equal.cong(HS.St, List<&2, HS.MRec>, s => HM.every(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_msgs_wake(HX.d(o, cx), HWK.WEnd{id, "gone", HC.now(o, cx)})))

def hub_agents_wake(+s: HS.St, +e: HWK.WkEv) -> {HS.St.agents(HM.apply(s, HM.EWake{e})) == HS.St.agents(s) : List<&2, HS.Agent>}:
  %HSL.St.open_eq(s) : {HS.St.agents(HM.apply(_, HM.EWake{e})) == HS.St.agents(s) : List<&2, HS.Agent>}
  {==}

def hub_ag_wake(+s: HS.St, +e: HWK.WkEv) -> {hub_ag(HM.apply(s, HM.EWake{e})) == hub_ag(s) : List<&2, HS.Agent>}:
  %HSL.St.open_eq(s) : {hub_ag(HM.apply(_, HM.EWake{e})) == hub_ag(s) : List<&2, HS.Agent>}
  {==}

def hub_agents_wakes_each(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>)
    -> {hub_ag(HX.d(o, HC.wakes_gone.each(o, ids, cx))) == hub_ag(HX.d(o, cx)) : List<&2, HS.Agent>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.EWake{HWK.WEnd{id, "gone", HC.now(o, cx)}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HS.Agent>, hub_ag(HX.d(o, HC.wakes_gone.each(o, t, cx2))), hub_ag(HX.d(o, cx2)), hub_ag(HX.d(o, cx)),
        hub_agents_wakes_each(o, t, cx2),
        Equal.trans(List<&2, HS.Agent>, hub_ag(HX.d(o, cx2)), hub_ag(HM.apply(HX.d(o, cx), ev)), hub_ag(HX.d(o, cx)),
          Equal.cong(HS.St, List<&2, HS.Agent>, s => hub_ag(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_ag_wake(HX.d(o, cx), HWK.WEnd{id, "gone", HC.now(o, cx)})))

def hub_msgs_wakes(-o: HS.St, +life: HS.Life, +cx: HX.Cx<o>, +n: String)
    -> {HM.every(HX.d(o, HC.wakes_gone(o, life, cx, n))) == HM.every(HX.d(o, cx)) : List<&2, HS.MRec>}:
  match life:
    case HS.LActive{}:
      {==}
    case HS.LStopped{}:
      {==}
    case HS.LFailed{}:
      {==}
    case HS.LArchived{}:
      hub_msgs_wakes_each(o, HWK.ids_of(HS.Wakes.live(HC.wks(o, cx)), HM.names_of(HX.d(o, cx), n)), cx)

def hub_agents_wakes(-o: HS.St, +life: HS.Life, +cx: HX.Cx<o>, +n: String)
    -> {hub_ag(HX.d(o, HC.wakes_gone(o, life, cx, n))) == hub_ag(HX.d(o, cx)) : List<&2, HS.Agent>}:
  match life:
    case HS.LActive{}:
      {==}
    case HS.LStopped{}:
      {==}
    case HS.LFailed{}:
      {==}
    case HS.LArchived{}:
      hub_agents_wakes_each(o, HWK.ids_of(HS.Wakes.live(HC.wks(o, cx)), HM.names_of(HX.d(o, cx), n)), cx)

# a run's state is its journal's replay (law hub_run_replays; here so
# that each laws file over a run can use it)
def hub_run_replays(+ins: List<&2, J.Json>, +h: HC.Hub) -> {HC.Hub.d(HC.RunR.hub(HC.run(ins, h))) == HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)) : HS.St}:
  match ins:
    case Nil{}:
      {==}
    case Con{i, rest}:
      Equal.trans(HS.St,
        HC.Hub.d(HC.RunR.hub(HC.run(rest, HC.Turn.hub(HC.commit(h, i))))),
        HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i)))), HC.Hub.d(HC.Turn.hub(HC.commit(h, i)))),
        HM.apply_all(List.append(&2, HM.Ev, HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i))))), HC.Hub.d(h)),
        hub_run_replays(rest, HC.Turn.hub(HC.commit(h, i))),
        Equal.trans(HS.St,
          HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i)))), HC.Hub.d(HC.Turn.hub(HC.commit(h, i)))),
          HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i)))), HM.apply_all(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.Hub.d(h))),
          HM.apply_all(List.append(&2, HM.Ev, HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i))))), HC.Hub.d(h)),
          Equal.cong(HS.St, HS.St, x => HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i)))), x),
            HC.Hub.d(HC.Turn.hub(HC.commit(h, i))), HM.apply_all(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.Hub.d(h)),
            hub_commit_ok(h, i)),
          hub_append(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i)))), HC.Hub.d(h))))
