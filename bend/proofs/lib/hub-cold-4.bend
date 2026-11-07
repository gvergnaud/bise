# bend/proofs/lib/hub-cold-4.bend: the lemmas of the split messages (issue
# 21 steps 3-4: St.hot ids ascending, St.old ids descending, every = the
# hot ones put among the old ones, turned around). Used by the laws
# split_*, ids_ascending, cold_is_inert and the walks' hot_* in
# bend/proofs/hub.bend. Part 4: every message after an event, the state, the replay, msgs_kept, the split along a step.
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
import ./hub-cold.bend as PC1
import ./hub-cold-2.bend as PC2
import ./hub-cold-3.bend as PC3
import ./hub-cold-5.bend as PC5

# ---- every message after an event: the one-list event, and the split kept ----

# the two sides `x`: every message is `ys`, and the split holds
def hc_EvOk(+x: HM.Ms, +nm: Nat, +ys: List<&2, HS.MRec>) -> Data:
  HX.Both<{List.reverse(&2, HS.MRec, HM.ins_all(HM.Ms.hot(x), HM.Ms.old(x))) == ys : List<&2, HS.MRec>},
    HI.Parts(HM.Ms.hot(x), HM.Ms.old(x), nm)>

def hc_ev_op.go(+x: HM.Ms, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +id: Nat, +op: HM.MsgOp,
    +p: HI.Parts(hot, old, nm), o: PC3.hc_OpOk(x, hot, old, nm, id, op))
    -> hc_EvOk(x, nm, HM.map_msg(List.reverse(&2, HS.MRec, HM.ins_all(hot, old)), id, op)):
  match o:
    case HX.Both{e, q}:
      +a = HM.ins_all(hot, old)
      HX.Both{Equal.trans(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, HM.ins_all(HM.Ms.hot(x), HM.Ms.old(x))),
          List.reverse(&2, HS.MRec, HM.map_msg(a, id, op)), HM.map_msg(List.reverse(&2, HS.MRec, a), id, op),
          Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => List.reverse(&2, HS.MRec, w), HM.ins_all(HM.Ms.hot(x), HM.Ms.old(x)), HM.map_msg(a, id, op), e),
          Equal.sym(List<&2, HS.MRec>, HM.map_msg(List.reverse(&2, HS.MRec, a), id, op), List.reverse(&2, HS.MRec, HM.map_msg(a, id, op)),
            PC2.hc_map_rev(a, id, op, PC1.hc_desc_all(hot, old, PC2.hc_p_asc(hot, old, nm, p), PC2.hc_p_desc(hot, old, nm, p), PC2.hc_p_disj(hot, old, nm, p))))),
        q}

# a state or settle event on the split: map_msg on every message
def hc_ev_op(+hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +id: Nat, +op: HM.MsgOp, +p: HI.Parts(hot, old, nm))
    -> hc_EvOk(HM.msg_op(hot, old, id, op), nm, HM.map_msg(List.reverse(&2, HS.MRec, HM.ins_all(hot, old)), id, op)):
  hc_ev_op.go(HM.msg_op(hot, old, id, op), hot, old, nm, id, op, p, PC3.hc_op(hot, old, nm, id, op, p))

def hc_settle.if(ok: Bool, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +id: Nat, +p: HI.Parts(hot, old, nm))
    -> hc_EvOk(HM.settle_in.if(ok, hot, old, id), nm, HI.settle1.if(ok, List.reverse(&2, HS.MRec, HM.ins_all(hot, old)), id)):
  match ok:
    case True{}:
      hc_ev_op(hot, old, nm, id, HM.OSettle{}, p)
    case False{}:
      HX.Both{{==}, p}

def hc_settle.go(o: Maybe<&2, HS.MRec>, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +id: Nat, +from: String,
    +p: HI.Parts(hot, old, nm))
    -> hc_EvOk(HM.settle_in.go(o, hot, old, id, from), nm, HI.settle1.go(o, List.reverse(&2, HS.MRec, HM.ins_all(hot, old)), id, from)):
  match o:
    case None{}:
      HX.Both{{==}, p}
    case Some{+r}:
      hc_settle.if(String.eq(HS.Msg.to(HS.MRec.msg(r)), from) || String.eq(from, HM.USER()), hot, old, nm, id, p)

# the record of an id in every message: the one of the split
def hc_rec_every(+hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, -nm: Nat, +id: Nat, +p: HI.Parts(hot, old, nm))
    -> {HM.rec_of(List.reverse(&2, HS.MRec, HM.ins_all(hot, old)), id) == HM.rec_in(hot, old, id) : Maybe<&2, HS.MRec>}:
  Equal.trans(Maybe<&2, HS.MRec>, HM.rec_of(List.reverse(&2, HS.MRec, HM.ins_all(hot, old)), id), HM.rec_of(HM.ins_all(hot, old), id),
    HM.rec_in(hot, old, id),
    PC2.hc_rec_rev(HM.ins_all(hot, old), id, PC1.hc_desc_all(hot, old, PC2.hc_p_asc(hot, old, nm, p), PC2.hc_p_desc(hot, old, nm, p), PC2.hc_p_disj(hot, old, nm, p))),
    PC2.hc_rec_all(hot, old, id, PC2.hc_p_asc(hot, old, nm, p), PC2.hc_p_disj(hot, old, nm, p)))

# a reply's settle on the split: settle1 on every message
def hc_settle(r: Maybe<&2, Nat>, +hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +from: String, +p: HI.Parts(hot, old, nm))
    -> hc_EvOk(HM.settle_in(r, hot, old, from), nm, HI.settle1(r, List.reverse(&2, HS.MRec, HM.ins_all(hot, old)), from)):
  match r:
    case None{}:
      HX.Both{{==}, p}
    case Some{+id}:
      %Equal.sym(Maybe<&2, HS.MRec>, HM.rec_of(List.reverse(&2, HS.MRec, HM.ins_all(hot, old)), id), HM.rec_in(hot, old, id),
        hc_rec_every(hot, old, nm, id, p)) : hc_EvOk(HM.settle_in.go(HM.rec_in(hot, old, id), hot, old, id, from), nm,
        HI.settle1.go(_, List.reverse(&2, HS.MRec, HM.ins_all(hot, old)), id, from))
      hc_settle.go(HM.rec_in(hot, old, id), hot, old, nm, id, from, p)

def hc_disj_snoc(+hs: List<&2, HS.MRec>, +os: List<&2, HS.MRec>, +x: HS.MRec, j: HI.Disjoint(hs, os),
    +nx: {HM.nat_in(HI.ids(os), HM.MRec.id(x)) == False{} : Bool}) -> HI.Disjoint(HM.snoc(HS.MRec, hs, x), os):
  match hs:
    case Nil{}:
      HX.Both{nx, Unit{}}
    case Con{h, t}:
      match j:
        case HX.Both{jh, jt}:
          HX.Both{jh, hc_disj_snoc(t, os, x, jt, nx)}

# a new message's record
def hc_new(+m: HS.Msg) -> HS.MRec:
  HM.new_rec(m)

# the split with the new message last on the hot side
def hc_parts_sent(+hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +m: HS.Msg, +p: HI.Parts(hot, old, nm),
    +k: {Nat.is_ge(HS.Msg.id(m), nm) == True{} : Bool})
    -> HI.Parts(HM.snoc(HS.MRec, hot, hc_new(m)), old, Nat.max(Nat.max(nm, 1n+HS.Msg.id(m)), 1n+HS.Msg.thread(m))):
  +r0 = hc_new(m)
  +hs = HM.snoc(HS.MRec, hot, r0)
  +a = Nat.max(nm, 1n+HS.Msg.id(m))
  +top = Nat.max(a, 1n+HS.Msg.thread(m))
  +bh0 = PL1.hub_below_mono(HI.ids(hot), nm, HS.Msg.id(m), k, PC2.hc_p_bh(hot, old, nm, p))
  +bo0 = PL1.hub_below_mono(HI.ids(old), nm, HS.Msg.id(m), k, PC2.hc_p_bo(hot, old, nm, p))
  +lt = PL1.hub_lt_ge(HS.Msg.id(m), a, top,
    PL1.hub_lt_ge(HS.Msg.id(m), 1n+HS.Msg.id(m), a, PL1.hub_lt_succ(HS.Msg.id(m)), Nat.max_ge_r(nm, 1n+HS.Msg.id(m))),
    Nat.max_ge_l(a, 1n+HS.Msg.thread(m)))
  HX.Both{PC2.hc_asc_snoc(hot, r0, PC2.hc_p_asc(hot, old, nm, p), bh0),
    HX.Both{PC2.hc_p_desc(hot, old, nm, p),
      HX.Both{PC2.hc_p_cold(hot, old, nm, p),
        HX.Both{hc_disj_snoc(hot, old, r0, PC2.hc_p_disj(hot, old, nm, p), PC1.hc_below_nin(HI.ids(old), HS.Msg.id(m), bo0)),
          HX.Both{PL1.hub_below_tr(HI.ids(hs), HM.snoc(Nat, HI.ids(hot), HS.Msg.id(m)), top, PL1.hub_ids_snoc(hot, r0),
              PL1.hub_below_snoc(HI.ids(hot), HS.Msg.id(m), top, PL1.hub_below_mono(HI.ids(hot), a, top, Nat.max_ge_l(a, 1n+HS.Msg.thread(m)), PL1.hub_below_mono(HI.ids(hot), nm, a, Nat.max_ge_l(nm, 1n+HS.Msg.id(m)), PC2.hc_p_bh(hot, old, nm, p))), lt)),
            PL1.hub_below_mono(HI.ids(old), a, top, Nat.max_ge_l(a, 1n+HS.Msg.thread(m)), PL1.hub_below_mono(HI.ids(old), nm, a, Nat.max_ge_l(nm, 1n+HS.Msg.id(m)), PC2.hc_p_bo(hot, old, nm, p)))}}}}}

# a new message (an id at or above the next free one) on the split: last on every message, the split kept
def hc_sent(+hot: List<&2, HS.MRec>, +old: List<&2, HS.MRec>, +nm: Nat, +m: HS.Msg, +p: HI.Parts(hot, old, nm),
    +k: {Nat.is_ge(HS.Msg.id(m), nm) == True{} : Bool})
    -> hc_EvOk(HM.settle_in(HS.Msg.reply_to(m), HM.snoc(HS.MRec, hot, HM.new_rec(m)), old, HS.Msg.from(m)),
      Nat.max(Nat.max(nm, 1n+HS.Msg.id(m)), 1n+HS.Msg.thread(m)), HI.sent1(List.reverse(&2, HS.MRec, HM.ins_all(hot, old)), m)):
  +r0 = hc_new(m)
  +hs = HM.snoc(HS.MRec, hot, r0)
  +e = HM.ins_all(hot, old)
  +a = Nat.max(nm, 1n+HS.Msg.id(m))
  +top = Nat.max(a, 1n+HS.Msg.thread(m))
  +bh0 = PL1.hub_below_mono(HI.ids(hot), nm, HS.Msg.id(m), k, PC2.hc_p_bh(hot, old, nm, p))
  +bo0 = PL1.hub_below_mono(HI.ids(old), nm, HS.Msg.id(m), k, PC2.hc_p_bo(hot, old, nm, p))
  +lt = PL1.hub_lt_ge(HS.Msg.id(m), a, top,
    PL1.hub_lt_ge(HS.Msg.id(m), 1n+HS.Msg.id(m), a, PL1.hub_lt_succ(HS.Msg.id(m)), Nat.max_ge_r(nm, 1n+HS.Msg.id(m))),
    Nat.max_ge_l(a, 1n+HS.Msg.thread(m)))
  +ps = hc_parts_sent(hot, old, nm, m, p, k)
  +last = Equal.trans(List<&2, HS.MRec>, HM.ins_all(hs, old), HM.ins_all(hot, HM.ins_desc(old, r0)), r0 <> e,
    PC1.hc_all_snoc(hot, r0, old),
    Equal.trans(List<&2, HS.MRec>, HM.ins_all(hot, HM.ins_desc(old, r0)), HM.ins_desc(e, r0), r0 <> e,
      PC1.hc_all_ins(hot, old, r0, PC1.hc_below_nin(HI.ids(hot), HS.Msg.id(m), bh0)),
      PC1.hc_ins_head(e, r0, PC1.hc_below_all(hot, old, HS.Msg.id(m), bh0, bo0))))
  +ev = Equal.trans(List<&2, HS.MRec>, HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, e), r0), List.reverse(&2, HS.MRec, r0 <> e),
    List.reverse(&2, HS.MRec, HM.ins_all(hs, old)),
    Equal.sym(List<&2, HS.MRec>, List.reverse(&2, HS.MRec, r0 <> e), HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, e), r0), PC1.hc_rev_cons(e, r0)),
    Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, w => List.reverse(&2, HS.MRec, w), r0 <> e, HM.ins_all(hs, old),
      Equal.sym(List<&2, HS.MRec>, HM.ins_all(hs, old), r0 <> e, last)))
  %Equal.sym(List<&2, HS.MRec>, HM.snoc(HS.MRec, List.reverse(&2, HS.MRec, e), r0), List.reverse(&2, HS.MRec, HM.ins_all(hs, old)), ev)
    : hc_EvOk(HM.settle_in(HS.Msg.reply_to(m), hs, old, HS.Msg.from(m)), top, HI.settle1(HS.Msg.reply_to(m), _, HS.Msg.from(m)))
  hc_settle(HS.Msg.reply_to(m), hs, old, top, HS.Msg.from(m), ps)

# ---- an event on the state: every message as on one list, the split kept ----

def hc_StOk(+s: HS.St, e: HM.Ev) -> Data:
  HX.Both<{HM.every(HM.apply(s, e)) == HI.apply1(HM.every(s), e) : List<&2, HS.MRec>}, HI.Split(HM.apply(s, e))>

# a new message's id is at or above the next free one (HX.FreshEv: it is
# the next free one; a boot replay skips the others, CO.replay_fresh)
def hc_GeEv(+s: HS.St, e: HM.Ev) -> Data:
  match e:
    case HM.EMsgSent{+m}:
      {Nat.is_ge(HS.Msg.id(m), HS.St.next_msg(s)) == True{} : Bool}
    case _:
      Unit

# on the state opened by name (S.St.open), each event's apply computes
def hc_apply.go(+e: HM.Ev, +s: HS.St, sp: HI.Split(s), k: hc_GeEv(s, e)) -> hc_StOk(HS.St.open(s), e):
  match e:
    case HM.ETaskCreated{a, b, c, d, f}:
      HX.Both{{==}, sp}
    case HM.ELifecycle{a, b, c}:
      HX.Both{{==}, sp}
    case HM.EWorkspace{a, b}:
      HX.Both{{==}, sp}
    case HM.ESnapshot{a, b}:
      HX.Both{{==}, sp}
    case HM.EPlaced{a, b, c}:
      HX.Both{{==}, sp}
    case HM.EDropped{a, b, c}:
      HX.Both{{==}, sp}
    case HM.EDeclared{a, b, c}:
      HX.Both{{==}, sp}
    case HM.EReported{a, b}:
      HX.Both{{==}, sp}
    case HM.EFileTouched{a, b}:
      HX.Both{{==}, sp}
    case HM.EClaimDropped{a, b}:
      HX.Both{{==}, sp}
    case HM.ERenamed{a, b}:
      HX.Both{{==}, sp}
    case HM.EMsgsMoved{f, t}:
      PC5.hm_split(HS.St.hot(s), HS.St.old(s), HS.St.next_msg(s), f, t, sp)
    case HM.EMsgSent{m}:
      hc_sent(HS.St.hot(s), HS.St.old(s), HS.St.next_msg(s), m, sp, k)
    case HM.EMsgState{id, x}:
      hc_ev_op(HS.St.hot(s), HS.St.old(s), HS.St.next_msg(s), id, HM.OSt{x}, sp)
    case HM.ESettled{id}:
      hc_ev_op(HS.St.hot(s), HS.St.old(s), HS.St.next_msg(s), id, HM.OSettle{}, sp)
    case HM.ECardOpened{c}:
      HX.Both{{==}, sp}
    case HM.ECardClosed{a, b, c}:
      HX.Both{{==}, sp}
    case HM.ECardAnswerPos{a, b, c}:
      HX.Both{{==}, sp}
    case HM.ECardWithdrawn{a, b}:
      HX.Both{{==}, sp}
    case HM.ECardLate{a}:
      HX.Both{{==}, sp}
    case HM.EMainNote{a, b}:
      HX.Both{{==}, sp}
    case HM.ENotesFlushed{}:
      HX.Both{{==}, sp}
    case HM.ETimerSet{a, b}:
      HX.Both{{==}, sp}
    case HM.ETimerFired{a, b, c, d, f}:
      HX.Both{{==}, sp}
    case HM.ETimerRun{a, b}:
      HX.Both{{==}, sp}
    case HM.ETimerStop{a, b, c}:
      HX.Both{{==}, sp}
    case HM.ETimerName{a, b, c, d}:
      HX.Both{{==}, sp}
    case HM.EX{a}:
      HX.Both{{==}, sp}
    case HM.EAct{a}:
      HX.Both{{==}, sp}
    case HM.EFollow{a, b, c}:
      HX.Both{{==}, sp}
    case HM.EJobEnd{a, b}:
      HX.Both{{==}, sp}
    case HM.EWake{a}:
      HX.Both{{==}, sp}
    case HM.EPaused{a}:
      HX.Both{{==}, sp}
    case HM.EResumed{a}:
      HX.Both{{==}, sp}

def hc_apply(+s: HS.St, e: HM.Ev, sp: HI.Split(s), k: hc_GeEv(s, e)) -> hc_StOk(s, e):
  %HSL.St.open_eq(s) : hc_StOk(_, e)
  hc_apply.go(e, s, sp, k)

# the split holds after an event (law split_kept_by_run, one event)
def hc_ok_split(-s: HS.St, -e: HM.Ev, o: hc_StOk(s, e)) -> HI.Split(HM.apply(s, e)):
  match o:
    case HX.Both{_, q}:
      q

def hc_ok_every(-s: HS.St, -e: HM.Ev, o: hc_StOk(s, e)) -> {HM.every(HM.apply(s, e)) == HI.apply1(HM.every(s), e) : List<&2, HS.MRec>}:
  match o:
    case HX.Both{q, _}:
      q

def hc_split_apply(+s: HS.St, e: HM.Ev, sp: HI.Split(s), k: hc_GeEv(s, e)) -> HI.Split(HM.apply(s, e)):
  hc_ok_split(s, e, hc_apply(s, e, sp, k))

# every message after an event: the one-list event on every message (law cold_is_inert)
def hc_every_apply(+s: HS.St, e: HM.Ev, sp: HI.Split(s), k: hc_GeEv(s, e))
    -> {HM.every(HM.apply(s, e)) == HI.apply1(HM.every(s), e) : List<&2, HS.MRec>}:
  hc_ok_every(s, e, hc_apply(s, e, sp, k))

def hc_ge_refl(+n: Nat) -> {Nat.is_ge(n, n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      hc_ge_refl(p)

# the journal's next free id is at or above it
def hc_fresh_ge(+s: HS.St, e: HM.Ev, k: HX.FreshEv(s, e)) -> hc_GeEv(s, e):
  match e:
    case HM.EMsgSent{+m}:
      %Equal.sym(Nat, HS.Msg.id(m), HS.St.next_msg(s), k) : {Nat.is_ge(_, HS.St.next_msg(s)) == True{} : Bool}
      hc_ge_refl(HS.St.next_msg(s))
    case HM.ETaskCreated{a, b, c, d, f}:
      Unit{}
    case HM.ELifecycle{a, b, c}:
      Unit{}
    case HM.EWorkspace{a, b}:
      Unit{}
    case HM.ESnapshot{a, b}:
      Unit{}
    case HM.EPlaced{a, b, c}:
      Unit{}
    case HM.EDropped{a, b, c}:
      Unit{}
    case HM.EDeclared{a, b, c}:
      Unit{}
    case HM.EReported{a, b}:
      Unit{}
    case HM.EFileTouched{a, b}:
      Unit{}
    case HM.EClaimDropped{a, b}:
      Unit{}
    case HM.ERenamed{a, b}:
      Unit{}
    case HM.EMsgsMoved{a, b}:
      Unit{}
    case HM.EMsgState{id, x}:
      Unit{}
    case HM.ESettled{id}:
      Unit{}
    case HM.ECardOpened{c}:
      Unit{}
    case HM.ECardClosed{a, b, c}:
      Unit{}
    case HM.ECardAnswerPos{a, b, c}:
      Unit{}
    case HM.ECardWithdrawn{a, b}:
      Unit{}
    case HM.ECardLate{a}:
      Unit{}
    case HM.EMainNote{a, b}:
      Unit{}
    case HM.ENotesFlushed{}:
      Unit{}
    case HM.ETimerSet{a, b}:
      Unit{}
    case HM.ETimerFired{a, b, c, d, f}:
      Unit{}
    case HM.ETimerRun{a, b}:
      Unit{}
    case HM.ETimerStop{a, b, c}:
      Unit{}
    case HM.ETimerName{a, b, c, d}:
      Unit{}
    case HM.EX{a}:
      Unit{}
    case HM.EAct{a}:
      Unit{}
    case HM.EFollow{a, b, c}:
      Unit{}
    case HM.EJobEnd{a, b}:
      Unit{}
    case HM.EWake{a}:
      Unit{}
    case HM.EPaused{a}:
      Unit{}
    case HM.EResumed{a}:
      Unit{}

# the split holds after a journal of fresh events
def hc_split_all(+evs: List<&2, HM.Ev>, +s: HS.St, sp: HI.Split(s), f: HX.Fresh(evs, s)) -> HI.Split(HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      sp
    case Con{e, t}:
      match f:
        case HX.Both{k, ft}:
          hc_split_all(t, HM.apply(s, e), hc_split_apply(s, e, sp, hc_fresh_ge(s, e, k)), ft)

# a split state's every message: each one record, ids below the next free one (law msgs_unique)
def hc_unique(+s: HS.St, +sp: HI.Split(s)) -> HI.Unique(s):
  PC2.hc_every_unique(HS.St.hot(s), HS.St.old(s), HS.St.next_msg(s), sp)

# ---- the boot replay keeps the split (law split_after_replay) ----

# an event the replay applies: a new message's id at or above the next free one
def hc_replay_ge(+st: HS.St, e: HM.Ev, ok: {CO.replay_fresh(st, e) == True{} : Bool}) -> hc_GeEv(st, e):
  match e:
    case HM.EMsgSent{+m}:
      ok
    case HM.EMsgState{a, b}:
      Unit{}
    case HM.ESettled{a}:
      Unit{}
    case HM.ETaskCreated{a, b, c, d, f}:
      Unit{}
    case HM.ELifecycle{a, b, c}:
      Unit{}
    case HM.EWorkspace{a, b}:
      Unit{}
    case HM.ESnapshot{a, b}:
      Unit{}
    case HM.EPlaced{a, b, c}:
      Unit{}
    case HM.EDropped{a, b, c}:
      Unit{}
    case HM.EDeclared{a, b, c}:
      Unit{}
    case HM.EReported{a, b}:
      Unit{}
    case HM.EFileTouched{a, b}:
      Unit{}
    case HM.EClaimDropped{a, b}:
      Unit{}
    case HM.ERenamed{a, b}:
      Unit{}
    case HM.EMsgsMoved{a, b}:
      Unit{}
    case HM.ECardOpened{c}:
      Unit{}
    case HM.ECardClosed{a, b, c}:
      Unit{}
    case HM.ECardAnswerPos{a, b, c}:
      Unit{}
    case HM.ECardWithdrawn{a, b}:
      Unit{}
    case HM.ECardLate{a}:
      Unit{}
    case HM.EMainNote{a, b}:
      Unit{}
    case HM.ENotesFlushed{}:
      Unit{}
    case HM.ETimerSet{a, b}:
      Unit{}
    case HM.ETimerFired{a, b, c, d, f}:
      Unit{}
    case HM.ETimerRun{a, b}:
      Unit{}
    case HM.ETimerStop{a, b, c}:
      Unit{}
    case HM.ETimerName{a, b, c, d}:
      Unit{}
    case HM.EX{a}:
      Unit{}
    case HM.EAct{a}:
      Unit{}
    case HM.EFollow{a, b, c}:
      Unit{}
    case HM.EJobEnd{a, b}:
      Unit{}
    case HM.EWake{a}:
      Unit{}
    case HM.EPaused{a}:
      Unit{}
    case HM.EResumed{a}:
      Unit{}

def hc_replay_one(ok: Bool, +st: HS.St, +e: HM.Ev, +eok: {CO.replay_fresh(st, e) == ok : Bool}, sp: HI.Split(st))
    -> HI.Split(CO.replay_one(ok, st, e)):
  match ok:
    case True{}:
      hc_split_apply(st, e, sp, hc_replay_ge(st, e, eok))
    case False{}:
      sp

def hc_replay_batch(+ms: List<&2, Maybe<&2, HM.Ev>>, +st: HS.St, +i: Nat, +sk: List<&2, Nat>, sp: HI.Split(st))
    -> HI.Split(Pair.fst(HS.St, List<&2, Nat>, CO.replay_batch(ms, st, i, sk))):
  match ms:
    case Nil{}:
      sp
    case Con{m, rest}:
      match m:
        case None{}:
          hc_replay_batch(rest, st, 1n+i, i <> sk, sp)
        case Some{e}:
          hc_replay_batch(rest, CO.replay_one(CO.replay_fresh(st, e), st, e), 1n+i,
            Bool.pick(List<&2, Nat>, CO.replay_fresh(st, e), sk, i <> sk),
            hc_replay_one(CO.replay_fresh(st, e), st, e, {==}, sp))

# ---- a message once sent is never removed (law msgs_kept) ----

# the one-list event keeps the ids, in order (a new one comes last)
def hc_kept1(+xs: List<&2, HS.MRec>, e: HM.Ev) -> HI.IsPrefix(HI.ids(xs), HI.ids(HI.apply1(xs, e))):
  match e:
    case HM.EMsgSent{+m}:
      +r0 = hc_new(m)
      +ys = HM.snoc(HS.MRec, xs, r0)
      PL1.hub_prefix_tr(HI.ids(xs), HI.ids(HI.settle1(HS.Msg.reply_to(m), ys, HS.Msg.from(m))), HM.snoc(Nat, HI.ids(xs), HS.Msg.id(m)),
        Equal.trans(List<&2, Nat>, HI.ids(HI.settle1(HS.Msg.reply_to(m), ys, HS.Msg.from(m))), HI.ids(ys), HM.snoc(Nat, HI.ids(xs), HS.Msg.id(m)),
          PL1.hub_settle_ids(HS.Msg.reply_to(m), ys, HS.Msg.from(m)), PL1.hub_ids_snoc(xs, r0)),
        PL1.hub_prefix_snoc(HI.ids(xs), HI.ids(xs), HS.Msg.id(m), PL1.hub_prefix_refl(HI.ids(xs))))
    case HM.EMsgState{+id, +x}:
      PL1.hub_prefix_tr(HI.ids(xs), HI.ids(HM.map_msg(xs, id, HM.OSt{x})), HI.ids(xs), PL1.hub_ids_map(xs, id, HM.OSt{x}), PL1.hub_prefix_refl(HI.ids(xs)))
    case HM.ESettled{+id}:
      PL1.hub_prefix_tr(HI.ids(xs), HI.ids(HM.map_msg(xs, id, HM.OSettle{})), HI.ids(xs), PL1.hub_ids_map(xs, id, HM.OSettle{}), PL1.hub_prefix_refl(HI.ids(xs)))
    case HM.ETaskCreated{a, b, c, d, f}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ELifecycle{a, b, c}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EWorkspace{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ESnapshot{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EPlaced{a, b, c}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EDropped{a, b, c}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EDeclared{a, b, c}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EReported{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EFileTouched{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EClaimDropped{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ERenamed{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EMsgsMoved{+f, +t}:
      %Equal.sym(List<&2, Nat>, HI.ids(HM.moved(xs, f, t)), HI.ids(xs), PC5.hm_ids(xs, f, t)) : HI.IsPrefix(HI.ids(xs), _)
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ECardOpened{c}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ECardClosed{a, b, c}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ECardAnswerPos{a, b, c}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ECardWithdrawn{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ECardLate{a}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EMainNote{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ENotesFlushed{}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ETimerSet{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ETimerFired{a, b, c, d, f}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ETimerRun{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ETimerStop{a, b, c}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.ETimerName{a, b, c, d}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EX{a}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EAct{a}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EFollow{a, b, c}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EJobEnd{a, b}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EWake{a}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EPaused{a}:
      PL1.hub_prefix_refl(HI.ids(xs))
    case HM.EResumed{a}:
      PL1.hub_prefix_refl(HI.ids(xs))

def hc_apply_kept(+s: HS.St, +e: HM.Ev, +sp: HI.Split(s), +k: hc_GeEv(s, e)) -> HI.Kept(s, HM.apply(s, e)):
  %Equal.sym(List<&2, HS.MRec>, HM.every(HM.apply(s, e)), HI.apply1(HM.every(s), e), hc_every_apply(s, e, sp, k))
    : HI.IsPrefix(HI.ids(HM.every(s)), HI.ids(_))
  hc_kept1(HM.every(s), e)

def hc_kept_all(+evs: List<&2, HM.Ev>, +s: HS.St, +sp: HI.Split(s), f: HX.Fresh(evs, s)) -> HI.Kept(s, HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      PL1.hub_prefix_refl(HI.ids(HM.every(s)))
    case Con{e, t}:
      match f:
        case HX.Both{k, ft}:
          +g = hc_fresh_ge(s, e, k)
          PL1.hub_prefix_trans(HI.ids(HM.every(s)), HI.ids(HM.every(HM.apply(s, e))), HI.ids(HM.every(HM.apply_all(t, HM.apply(s, e)))),
            hc_apply_kept(s, e, sp, g), hc_kept_all(t, HM.apply(s, e), hc_split_apply(s, e, sp, g), ft))

# ---- the split along a step (stop_leaves_no_queue, the walks) ----

def hc_split_tr(-a: HS.St, -b: HS.St, e: {a == b : HS.St}, sp: HI.Split(b)) -> HI.Split(a):
  %Equal.sym(HS.St, a, b, e) : HI.Split(_)
  sp

# the hot index: the hot side's hot ones are every message's
def hc_hot_index(+s: HS.St, +sp: HI.Split(s)) -> {HM.hot_of(HS.St.hot(s)) == HM.hot_of(HM.every(s)) : List<&2, HS.MRec>}:
  Equal.sym(List<&2, HS.MRec>, HM.hot_of(HM.every(s)), HM.hot_of(HS.St.hot(s)), PC2.hc_hot_every(HS.St.hot(s), HS.St.old(s), HS.St.next_msg(s), sp))

def hc_split_reject(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, sp: HI.Split(HX.d(o, cx))) -> HI.Split(HX.d(o, HC.reject_ids(o, ids, cx))):
  match ids:
    case Nil{}:
      sp
    case Con{id, t}:
      +ev = {HM.EMsgState{id, HS.Rejected{"recipient_unavailable"}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      hc_split_reject(o, t, cx2,
        hc_split_tr(HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), PL1.hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{}),
          hc_split_apply(HX.d(o, cx), ev, sp, Unit{})))

# rejecting the ids: every message rejected by id, as on one list
def hc_every_reject(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +sp: HI.Split(HX.d(o, cx)))
    -> {HM.every(HX.d(o, HC.reject_ids(o, ids, cx))) == PL1.hub_rej(ids, HM.every(HX.d(o, cx))) : List<&2, HS.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.EMsgState{id, HS.Rejected{"recipient_unavailable"}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      +sp2 = hc_split_tr(HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), PL1.hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{}),
        hc_split_apply(HX.d(o, cx), ev, sp, Unit{}))
      +mm = HM.map_msg(HM.every(HX.d(o, cx)), id, HM.OSt{HS.Rejected{"recipient_unavailable"}})
      Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, HC.reject_ids(o, t, cx2))), PL1.hub_rej(t, HM.every(HX.d(o, cx2))),
        PL1.hub_rej(id <> t, HM.every(HX.d(o, cx))),
        hc_every_reject(o, t, cx2, sp2),
        Equal.cong(List<&2, HS.MRec>, List<&2, HS.MRec>, xs => PL1.hub_rej(t, xs), HM.every(HX.d(o, cx2)), mm,
          Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, cx2)), HM.every(HM.apply(HX.d(o, cx), ev)), mm,
            Equal.cong(HS.St, List<&2, HS.MRec>, s => HM.every(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), PL1.hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
            hc_every_apply(HX.d(o, cx), ev, sp, Unit{}))))
