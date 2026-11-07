# bend/proofs/hub.bend: the proofs of bend/laws/hub.bend (moved from
# PROOF.bend). Each holds by unfolding the definitions involved; the
# checker verifies the equalities.
import Base
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/gen/state-laws.bend as HSL
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/keyed.bend as HKL
import ../hub/inv.bend as HI
import ../hub/gen/state-codec.bend as HSC
import ../hub/timers.bend as HTM
import ../hub/wakes.bend as HWK
import ../hub/xhub-model.bend as HXH
import ../hub/xhub.bend as HXB
import ../hub/pause.bend as HP
import ../hub/reach.bend as HR
import ../hub/answers.bend as HA
import ../hub/lazy.bend as HLZ
import ../hub/names.bend as HN
import ../hub/gen/fx.bend as HF
import ../hub/gen/feed.bend as HFD
import ../hub/viewctx.bend as HVC
import ./lib/hub-1.bend as PL1
import ./lib/absurd.bend as AB
import ./lib/hub-2.bend as PL2
import ./lib/hub-3.bend as PL3
import ./lib/hub-names.bend as PLN
import ./lib/hub-clean.bend as PLK
import ./lib/hub-cold.bend as PLC
import ./lib/hub-cold-2.bend as PLC2
import ./lib/hub-cold-3.bend as PLC3
import ./lib/hub-cold-4.bend as PLC4
import ./lib/hub-cold-5.bend as PLC5
import ./lib/hub-views.bend as PLV
import ../hub/cold.bend as CO
import ../laws/hub.bend as Laws

def Laws.hub_step_replays(h, i):
  PL1.hub_core_ok(HC.Hub.d(h), HX.Cx.c(HC.Hub.d(h), HC.step(h, i)))

def Laws.hub_run_replays(ins, h):
  PL1.hub_run_replays(ins, h)

def Laws.msg_ids_fresh(ins, h):
  match ins:
    case Nil{}:
      Unit{}
    case Con{i, rest}:
      PL1.hub_fresh_append(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i)))), HC.Hub.d(h),
        PL1.hub_fst(HX.Fresh(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.Hub.d(h)), HX.Quiet(HX.journal(HC.Turn.fx(HC.commit(h, i)))), PL1.hub_commit_f(h, i)),
        PL1.hub_fresh_tr(HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i)))),
          HC.Hub.d(HC.Turn.hub(HC.commit(h, i))), HM.apply_all(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.Hub.d(h)),
          PL1.hub_commit_ok(h, i), Laws.msg_ids_fresh(rest, HC.Turn.hub(HC.commit(h, i)))))

def Laws.auto_never_asks(ins, h):
  match ins:
    case Nil{}:
      Unit{}
    case Con{i, rest}:
      PL1.hub_quiet_append(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i)))),
        PL1.hub_snd(HX.Fresh(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.Hub.d(h)), HX.Quiet(HX.journal(HC.Turn.fx(HC.commit(h, i)))), PL1.hub_commit_f(h, i)),
        Laws.auto_never_asks(rest, HC.Turn.hub(HC.commit(h, i))))

def Laws.tick_clears_late_waiters(o, cx):
  PL1.hub_later_tr(HC.waiters_of_cx(o, HC.tick(o, cx)), HC.tick.keep(o, cx), HX.R.now(HX.Cx.r(o, cx)),
    PL1.hub_set_waiters(o, HC.tick.go(o, cx), HC.tick.keep(o, cx)),
    PL1.hub_keep_later(HX.R.waiters(HX.Cx.r(o, cx)), HX.R.now(HX.Cx.r(o, cx))))

# a stop's state is the one before its waiters and the views' contexts
# went (both runtime only)
def hv_stop_d(-o: HS.St, +cx: HX.Cx<o>, +n: String, +life: HS.Life, +why: String)
    -> {HX.d(o, HC.stop_task(o, cx, n, life, why)) == HX.d(o, HC.wakes_gone(o, life, HC.timers_gone(o, life, HC.stop_task.go(o, cx, n, life, why), n), n)) : HS.St}:
  +wg = HC.wakes_gone(o, life, HC.timers_gone(o, life, HC.stop_task.go(o, cx, n, life, why), n), n)
  +vd = HC.view_ctx_drop(o, wg, n)
  Equal.trans(HS.St, HX.d(o, HC.stop_task(o, cx, n, life, why)), HX.d(o, vd), HX.d(o, wg),
    PL1.hub_d_with_r(o, vd, HX.R.with_waiters(HC.rr(o, vd), HX.waiters_not(HX.R.waiters(HC.rr(o, cx)), n))),
    PL1.hub_d_with_r(o, wg, HX.R.with_ctx(HC.rr(o, wg), HVC.drop(HX.R.ctx(HC.rr(o, wg)), n))))

def Laws.stop_leaves_no_waiter(o, cx, n, life, why):
  PL1.hub_none_tr(HC.waiters_of_cx(o, HC.stop_task(o, cx, n, life, why)), HX.waiters_not(HX.R.waiters(HX.Cx.r(o, cx)), n), n,
    PL1.hub_set_waiters(o, HC.view_ctx_drop(o, HC.wakes_gone(o, life, HC.timers_gone(o, life, HC.stop_task.go(o, cx, n, life, why), n), n), n), HX.waiters_not(HX.R.waiters(HX.Cx.r(o, cx)), n)),
    PL1.hub_not_none(HX.R.waiters(HX.Cx.r(o, cx)), n))

def Laws.init_unique(w):
  HX.Both{Unit{}, Unit{}}

def Laws.msgs_unique(ins, h, u):
  PLC4.hc_unique(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))),
    PLC4.hc_split_tr(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)), Laws.hub_run_replays(ins, h),
      PLC4.hc_split_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h))))

def Laws.msgs_kept(ins, h, u):
  PL1.hub_kept_tr(HC.Hub.d(h), HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h), PLC4.hc_kept_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h)))

def Laws.delivered_once(ins, h):
  match ins:
    case Nil{}:
      Unit{}
    case Con{i, rest}:
      PL1.hub_deliv_append(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i)))), HC.Hub.d(h),
        PL1.hub_commit_v.cx(h, HC.step(h, i)),
        PL1.hub_deliv_tr(HC.RunR.journal(HC.run(rest, HC.Turn.hub(HC.commit(h, i)))),
          HC.Hub.d(HC.Turn.hub(HC.commit(h, i))), HM.apply_all(HX.journal(HC.Turn.fx(HC.commit(h, i))), HC.Hub.d(h)),
          PL1.hub_commit_ok(h, i), Laws.delivered_once(rest, HC.Turn.hub(HC.commit(h, i)))))

def Laws.stop_leaves_no_queue(o, cx, n, life, why, u):
  +cx1 = HC.out(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})])
  +cx2 = HC.fail_waiters(o, cx1, n, "stopped")
  +r = HC.rr(o, cx2)
  +cx3 = HC.set_r(o, cx2, HX.R.with_steered(r, HKL.steer_del(HX.R.steered(r), n)))
  +x4 = HX.Rt.with_turn(HX.Rt.with_run(HC.rt(o, cx3, n), HS.RDown{}), None{})
  +cx4 = HC.upd_rt(o, cx3, x4)
  +ev = {HM.ELifecycle{n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})} : HM.Ev}
  +cx5 = HC.emit(o, cx4, ev, Unit{}, Unit{}, Unit{})
  +m5 = HM.every(HX.d(o, cx5))
  +n5 = HM.names_of(HX.d(o, cx5), n)
  # stop_task rejects the queued ones it finds in the hot index
  +ids = HC.ids_of(HM.queued_in(HS.St.hot(HX.d(o, cx5)), n5))
  +cx6 = HC.reject_ids(o, ids, cx5)
  +cc = HC.close_cards(o, cx6, n, Nil{}, "agent stopped")
  +g = HC.stop_task.go(o, cx, n, life, why)
  +tg = HC.timers_gone(o, life, g, n)
  +wg = HC.wakes_gone(o, life, tg, n)
  # d is unchanged up to cx4
  +e4 = Equal.trans(HS.St, HX.d(o, cx4), HX.d(o, cx3), HX.d(o, cx), PL1.hub_d_upd_rt(o, cx3, x4),
    Equal.trans(HS.St, HX.d(o, cx3), HX.d(o, cx2), HX.d(o, cx), PL1.hub_d_with_r(o, cx2, HX.R.with_steered(r, HKL.steer_del(HX.R.steered(r), n))),
      Equal.trans(HS.St, HX.d(o, cx2), HX.d(o, cx1), HX.d(o, cx), PL1.hub_d_fail_waiters(o, cx1, n, "stopped"),
        PL1.hub_d_hout(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})]))))
  # the split of cx5 is right, so its queued ones are all of them
  +sp5 = PLC4.hc_split_tr(HX.d(o, cx5), HM.apply(HX.d(o, cx4), ev), PL1.hub_d_emit(o, cx4, ev, Unit{}, Unit{}, Unit{}),
    PLC4.hc_split_apply(HX.d(o, cx4), ev, PLC4.hc_split_tr(HX.d(o, cx4), HX.d(o, cx), e4, u), Unit{}))
  +hq = PL3.hub_hot_qf(HX.d(o, cx5), n, PLC4.hc_hot_index(HX.d(o, cx5), sp5))
  # the messages at the end: those of cx5 with the queued ones rejected
  +ef = Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.every(HX.d(o, g)), PL1.hub_rej(ids, m5),
    Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.every(HX.d(o, tg)), HM.every(HX.d(o, g)),
      Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.every(HX.d(o, wg)), HM.every(HX.d(o, tg)),
        PL1.hub_msgs_eq(o, HC.stop_task(o, cx, n, life, why), wg, hv_stop_d(o, cx, n, life, why)),
        PL1.hub_msgs_wakes(o, life, tg, n)),
      PL2.hub_msgs_gone(o, HR.life_gone(life), g, n, HM.names_of(HX.d(o, g), n))),
    Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, g)), HM.every(HX.d(o, cx6)), PL1.hub_rej(ids, m5),
      Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, g)), HM.every(HX.d(o, cc)), HM.every(HX.d(o, cx6)),
        Equal.cong(HS.St, List<&2, HS.MRec>, s => HM.every(s), HX.d(o, g), HX.d(o, cc), PLV.hv_nos_d(o, HS.St.cards(HX.d(o, cx6)), cc, n)),
        PL2.hub_msgs_close_ids(o, HC.card_ids(HS.St.cards(HX.d(o, cx6)), n, Nil{}), cx6, "agent stopped")),
      PLC4.hc_every_reject(o, ids, cx5, sp5)))
  # the names at the end: those of cx5 (no agent changes after it)
  +en = PLN.nm_names_of_eq(HX.d(o, HC.stop_task(o, cx, n, life, why)), HX.d(o, cx5), n,
    Equal.trans(List<&2, HS.Agent>, PL1.hub_ag(HX.d(o, HC.stop_task(o, cx, n, life, why))), PL1.hub_ag(HX.d(o, g)), PL1.hub_ag(HX.d(o, cx5)),
      Equal.trans(List<&2, HS.Agent>, PL1.hub_ag(HX.d(o, HC.stop_task(o, cx, n, life, why))), PL1.hub_ag(HX.d(o, tg)), PL1.hub_ag(HX.d(o, g)),
        Equal.trans(List<&2, HS.Agent>, PL1.hub_ag(HX.d(o, HC.stop_task(o, cx, n, life, why))), PL1.hub_ag(HX.d(o, wg)), PL1.hub_ag(HX.d(o, tg)),
          PL2.hub_agents_eq(o, HC.stop_task(o, cx, n, life, why), wg, hv_stop_d(o, cx, n, life, why)),
          PL1.hub_agents_wakes(o, life, tg, n)),
        PL2.hub_agents_gone(o, HR.life_gone(life), g, n, HM.names_of(HX.d(o, g), n))),
      Equal.trans(List<&2, HS.Agent>, PL1.hub_ag(HX.d(o, g)), PL1.hub_ag(HX.d(o, cx6)), PL1.hub_ag(HX.d(o, cx5)),
        Equal.trans(List<&2, HS.Agent>, PL1.hub_ag(HX.d(o, g)), PL1.hub_ag(HX.d(o, cc)), PL1.hub_ag(HX.d(o, cx6)),
          Equal.cong(HS.St, List<&2, HS.Agent>, s => PL1.hub_ag(s), HX.d(o, g), HX.d(o, cc), PLV.hv_nos_d(o, HS.St.cards(HX.d(o, cx6)), cc, n)),
          PL2.hub_agents_close_ids(o, HC.card_ids(HS.St.cards(HX.d(o, cx6)), n, Nil{}), cx6, "agent stopped")),
        PL2.hub_agents_reject(o, ids, cx5))))
  PL2.hub_names_tr(HM.every(HX.d(o, HC.stop_task(o, cx, n, life, why))), PL1.hub_rej(ids, m5), HM.names_of(HX.d(o, HC.stop_task(o, cx, n, life, why)), n), n5, ef, en,
    Equal.trans(List<&2, HS.Msg>, HM.queued_in(PL1.hub_rej(ids, m5), n5), HM.queued_in(PL1.hub_rej(HC.ids_of(HM.queued_in(m5, n5)), m5), n5), Nil{},
      Equal.cong(List<&2, HS.Msg>, List<&2, HS.Msg>, x => HM.queued_in(PL1.hub_rej(HC.ids_of(x), m5), n5), HM.queued_in(HS.St.hot(HX.d(o, cx5)), n5), HM.queued_in(m5, n5), hq),
      PL1.hub_K(m5, n5, PL1.hub_fst(HI.Distinct(HI.ids(m5)), HI.Below(HI.ids(m5), HS.St.next_msg(HX.d(o, cx5))), PLC4.hc_unique(HX.d(o, cx5), sp5)))))

def Laws.stop_leaves_no_card(o, cx, n, life, why, u):
  +cx1 = HC.out(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})])
  +cx2 = HC.fail_waiters(o, cx1, n, "stopped")
  +r = HC.rr(o, cx2)
  +cx3 = HC.set_r(o, cx2, HX.R.with_steered(r, HKL.steer_del(HX.R.steered(r), n)))
  +x4 = HX.Rt.with_turn(HX.Rt.with_run(HC.rt(o, cx3, n), HS.RDown{}), None{})
  +cx4 = HC.upd_rt(o, cx3, x4)
  +ev = {HM.ELifecycle{n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})} : HM.Ev}
  +cx5 = HC.emit(o, cx4, ev, Unit{}, Unit{}, Unit{})
  +ids = HC.ids_of(HM.queued_in(HS.St.hot(HX.d(o, cx5)), HM.names_of(HX.d(o, cx5), n)))
  +cx6 = HC.reject_ids(o, ids, cx5)
  +c6 = HS.St.cards(HX.d(o, cx6))
  +cc = HC.close_cards(o, cx6, n, Nil{}, "agent stopped")
  +g = HC.stop_task.go(o, cx, n, life, why)
  +tg = HC.timers_gone(o, life, g, n)
  +wg = HC.wakes_gone(o, life, tg, n)
  +e4 = Equal.trans(HS.St, HX.d(o, cx4), HX.d(o, cx3), HX.d(o, cx), PL1.hub_d_upd_rt(o, cx3, x4),
    Equal.trans(HS.St, HX.d(o, cx3), HX.d(o, cx2), HX.d(o, cx), PL1.hub_d_with_r(o, cx2, HX.R.with_steered(r, HKL.steer_del(HX.R.steered(r), n))),
      Equal.trans(HS.St, HX.d(o, cx2), HX.d(o, cx1), HX.d(o, cx), PL1.hub_d_fail_waiters(o, cx1, n, "stopped"),
        PL1.hub_d_hout(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})]))))
  # the cards of cx6 are those of cx
  +e6 = Equal.trans(List<&2, HS.Card>, c6, HS.St.cards(HX.d(o, cx5)), HS.St.cards(HX.d(o, cx)),
    PL2.hub_cards_reject(o, ids, cx5),
    Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, cx5)), HS.St.cards(HM.apply(HX.d(o, cx4), ev)), HS.St.cards(HX.d(o, cx)),
      Equal.cong(HS.St, List<&2, HS.Card>, s => HS.St.cards(s), HX.d(o, cx5), HM.apply(HX.d(o, cx4), ev), PL1.hub_d_emit(o, cx4, ev, Unit{}, Unit{}, Unit{})),
      Equal.trans(List<&2, HS.Card>, HS.St.cards(HM.apply(HX.d(o, cx4), ev)), HS.St.cards(HX.d(o, cx4)), HS.St.cards(HX.d(o, cx)),
        PL2.hub_cards_life(HX.d(o, cx4), n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})),
        PL2.hub_cards_eq(o, cx4, cx, e4))))
  # the cards at the end: those of cx6 without the task's
  +ef = Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), HS.St.cards(HX.d(o, g)),
    PL2.hub_dropc(HC.card_ids(c6, n, Nil{}), c6),
    Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), HS.St.cards(HX.d(o, tg)), HS.St.cards(HX.d(o, g)),
      Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), HS.St.cards(HX.d(o, wg)), HS.St.cards(HX.d(o, tg)),
        PL2.hub_cards_eq(o, HC.stop_task(o, cx, n, life, why), wg, hv_stop_d(o, cx, n, life, why)),
        PL2.hub_cards_wakes(o, life, tg, n)),
      PL2.hub_cards_gone(o, HR.life_gone(life), g, n, HM.names_of(HX.d(o, g), n))),
    Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, g)), HS.St.cards(HX.d(o, cc)), PL2.hub_dropc(HC.card_ids(c6, n, Nil{}), c6),
      Equal.cong(HS.St, List<&2, HS.Card>, s => HS.St.cards(s), HX.d(o, g), HX.d(o, cc), PLV.hv_nos_d(o, c6, cc, n)),
      PL2.hub_cards_close_ids(o, HC.card_ids(c6, n, Nil{}), cx6, "agent stopped")))
  PL2.hub_cids_tr(HS.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), PL2.hub_dropc(HC.card_ids(c6, n, Nil{}), c6), n, ef,
    PL2.hub_KC(c6, n, PL1.hub_distinct_tr(HI.card_ids(c6), HI.card_ids(HS.St.cards(HX.d(o, cx))),
      Equal.cong(List<&2, HS.Card>, List<&2, Nat>, cs => HI.card_ids(cs), c6, HS.St.cards(HX.d(o, cx)), e6),
      PL1.hub_fst(HI.Distinct(HI.card_ids(HS.St.cards(HX.d(o, cx)))), HI.Below(HI.card_ids(HS.St.cards(HX.d(o, cx))), HS.St.next_card(HX.d(o, cx))), u))))

def Laws.init_cards_unique(w):
  HX.Both{Unit{}, Unit{}}

def Laws.cards_unique(ins, h, u):
  PL2.hub_ucards_tr(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h),
    PL2.hub_apply_all_ucards(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h)))

def Laws.init_names_apart(w):
  HX.Both{Unit{}, Unit{}}

def Laws.names_apart(ins, h, u):
  PL3.hub_napart_tr(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h),
    PL3.hub_apply_all_names(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h)))

def Laws.fire_is_a_message(o, p, t, due, next):
  match p:
    case (cx, HC.SErr{e}):
      PL3.hub_retry_journal(o, due, cx, t)
    case (cx, HC.SOk{m}):
      PL3.hub_count_journal(o, due, cx, t, next)

def Laws.wake_never_stacked(o, cx, t, text, due, next, q):
  %Equal.sym(Bool, HC.bise_queued(HX.d(o, cx), HS.Timer.agent(t)), True{}, q) : {HC.timer_fire.go(o, Bool.and(Bool.not(HP.is_paused(HX.d(o, cx))), HTM.may_fire(_, due, t, HC.timer_ready(o, cx, HS.Timer.agent(t)), HC.now(o, cx))), cx, t, text, due, next) == cx : HX.Cx<o>}
  %Equal.sym(Bool, Bool.and(Bool.not(HP.is_paused(HX.d(o, cx))), False{}), False{}, PL2.hub_and_false_r(Bool.not(HP.is_paused(HX.d(o, cx))))) : {HC.timer_fire.go(o, _, cx, t, text, due, next) == cx : HX.Cx<o>}
  {==}

# times_bound on the timer's times and count (a split on `times`)
def times_bound.go(m: Maybe<&2, Nat>, +f: Nat) -> {HTM.under.go(m, 1n+f) == Bool.not(HTM.spent.go(m, f)) : Bool}:
  match m:
    case None{}:
      {==}
    case Some{n}:
      PL3.hub_lt_not_le(1n+f, n)

# a counted fire's four setters (next, fired, last, runs), each rewritten
# with its eta: the timer's times are its own, its count the new one
def Laws.times_bound(t, next, at):
  +f = {1n+HS.Timer.fired(t) : Nat}
  +c = HS.Timer.with_next(t, next)
  +b = HS.Timer.with_fired(c, f)
  +a = HS.Timer.with_last(b, at)
  %HSL.Timer.runs.eta(a, HTM.runs_add(HS.Timer.runs(t), at)) : {HTM.under.go(HS.Timer.times(_), HS.Timer.fired(_)) == Bool.not(HTM.spent(t)) : Bool}
  %HSL.Timer.last.eta(b, at) : {HTM.under.go(HS.Timer.times(_), HS.Timer.fired(_)) == Bool.not(HTM.spent(t)) : Bool}
  %HSL.Timer.fired.eta(c, f) : {HTM.under.go(HS.Timer.times(_), HS.Timer.fired(_)) == Bool.not(HTM.spent(t)) : Bool}
  %HSL.Timer.next.eta(t, next) : {HTM.under.go(HS.Timer.times(_), f) == Bool.not(HTM.spent(t)) : Bool}
  times_bound.go(HS.Timer.times(t), HS.Timer.fired(t))

def Laws.named_keeps_the_rest(t, name):
  %Equal.sym(HS.Timer, HS.Timer.with_name(HS.Timer.with_name(t, name), HS.Timer.name(t)), HS.Timer.with_name(t, HS.Timer.name(t)),
    HSL.Timer.name.set_set(t, name, HS.Timer.name(t))) : {_ == t : HS.Timer}
  HSL.Timer.name.set_get(t)

def Laws.named_is_the_name(t, name):
  HSL.Timer.name.get_with(t, name)

def Laws.named_announces_once(t, name):
  HSL.Timer.held.get_with(HS.Timer.with_name(t, name), False{})

# ---- the project's pause (cost-pause) ----

def Laws.paused_delivers_nothing(o, cx, n, q):
  %Equal.sym(Bool, HP.is_paused(HX.d(o, cx)), True{}, q) : {HC.pump.paused(o, _, cx, n) == cx : HX.Cx<o>}
  {==}

def Laws.timer_held_while_paused(o, cx, t, due, q):
  %Equal.sym(Bool, HP.is_paused(HX.d(o, cx)), True{}, q) : {Bool.and(Bool.not(_), HTM.may_fire(HC.bise_queued(HX.d(o, cx), HS.Timer.agent(t)), due, t, HC.timer_ready(o, cx, HS.Timer.agent(t)), HC.now(o, cx))) == False{} : Bool}
  {==}

def Laws.pause_idempotent(o, cx, q):
  %Equal.sym(Bool, HP.is_paused(HX.d(o, cx)), True{}, q) : {HC.pause.go(o, _, cx) == HC.notice(o, cx, HP.ALREADY()) : HX.Cx<o>}
  {==}

def Laws.resume_idempotent(o, cx, q):
  %Equal.sym(Bool, HP.is_paused(HX.d(o, cx)), False{}, q) : {HC.resume.go(o, _, cx) == HC.notice(o, cx, HP.NOT_PAUSED()) : HX.Cx<o>}
  {==}

def Laws.paused_line_pauses(s, at):
  %HSL.St.open_eq(s) : {HS.St.paused(HM.apply(_, HM.EPaused{at})) == Some{at} : Maybe<&2, Nat>}
  {==}

def Laws.resumed_line_resumes(s, at):
  %HSL.St.open_eq(s) : {HS.St.paused(HM.apply(_, HM.EResumed{at})) == None{} : Maybe<&2, Nat>}
  {==}

def Laws.pause_keeps_order(s, at):
  %HSL.St.open_eq(s) : {HM.every(HM.apply(_, HM.EPaused{at})) == HM.every(s) : List<&2, HS.MRec>}
  {==}

def Laws.resume_keeps_order(s, at):
  %HSL.St.open_eq(s) : {HM.every(HM.apply(_, HM.EResumed{at})) == HM.every(s) : List<&2, HS.MRec>}
  {==}

def Laws.stop_leaves_no_timer(o, cx, n, why):
  +g = HC.stop_task.go(o, cx, n, HS.LArchived{}, why)
  +tg = HC.timers_gone(o, HS.LArchived{}, g, n)
  +wids = HWK.ids_of(HS.Wakes.live(HC.wks(o, tg)), HM.names_of(HX.d(o, tg), n))
  +wg = HC.wakes_gone(o, HS.LArchived{}, tg, n)
  +st = HC.stop_task(o, cx, n, HS.LArchived{}, why)
  +ns = HM.names_of(HX.d(o, g), n)
  +xs = HS.Tms.live(HS.St.tm(HX.d(o, g)))
  +ids = HTM.ids_in(xs, ns)
  +e0 = Equal.cong(HS.St, List<&2, HS.Timer>, s => HS.Tms.live(HS.St.tm(s)), HX.d(o, st), HX.d(o, wg),
    hv_stop_d(o, cx, n, HS.LArchived{}, why))
  +e1 = Equal.trans(List<&2, HS.Timer>, HS.Tms.live(HS.St.tm(HX.d(o, st))), HS.Tms.live(HS.St.tm(HX.d(o, wg))), HS.Tms.live(HS.St.tm(HX.d(o, tg))),
    e0, PL3.hub_live_wakes_each(o, wids, tg))
  +e2 = PL3.hub_live_gone_each(o, ids, g, HC.cat(["@", n, " is gone"]))
  PL3.hub_tnone_tr(HS.Tms.live(HS.St.tm(HX.d(o, st))), HTM.keep_out(xs, ids), n,
    Equal.trans(List<&2, HS.Timer>, HS.Tms.live(HS.St.tm(HX.d(o, st))), HS.Tms.live(HS.St.tm(HX.d(o, tg))), HTM.keep_out(xs, ids), e1, e2),
    PL3.hub_none_keep(xs, ids, ns, n, PL3.hub_all_self(ids), PL3.hub_str_in_head(n, HM.names_of.go(HM.get_agent(HX.d(o, g), n)))))

# an end's two setters: its live list is the dropped one (ended's eta,
# then live's get_with)
def Laws.wake_hit_once(ws, id, why, at):
  +d = HWK.drop(HS.Wakes.live(ws), id)
  +l = HS.Wakes.with_live(ws, d)
  %HSL.Wakes.ended.eta(l, HWK.ended_add(HWK.find(HS.Wakes.live(ws), id), HS.Wakes.ended(ws), at, why)) : {HWK.find(HS.Wakes.live(_), id) == None{} : Maybe<&2, HS.Watch>}
  %Equal.sym(List<&2, HS.Watch>, HS.Wakes.live(l), d, HSL.Wakes.live.get_with(ws, d)) : {HWK.find(_, id) == None{} : Maybe<&2, HS.Watch>}
  PL3.hub_wk_gone(HS.Wakes.live(ws), id)

def Laws.wake_gone_never_hits(o, cx, text):
  {==}

def Laws.wake_replay_keeps(x):
  HSC.Watch.journal_round(x)

# ---- who a wake reaches (wake-deliver) ----

def Laws.wake_steers_busy(a, text):
  {==}

def Laws.busy_steers_now(m, rest, q):
  %Equal.sym(Bool, HS.Msg.queued(m), False{}, q) : {Bool.pick(List<&2, HS.Msg>, _, HC.not_queued_mode(rest), m <> HC.not_queued_mode(rest)) == m <> HC.not_queued_mode(rest) : List<&2, HS.Msg>}
  {==}

def Laws.timer_reaches_busy(o, cx, a, p, q):
  %Equal.sym(Bool, HC.is_active(HS.Agent.life(a)), True{}, p) : {Bool.and(_, HR.reachable(HX.Rt.run(HC.rt(o, cx, HS.Agent.name(a))))) == True{} : Bool}
  %Equal.sym(HS.Run, HX.Rt.run(HC.rt(o, cx, HS.Agent.name(a))), HS.RBusy{}, q) : {Bool.and(True{}, HR.reachable(_)) == True{} : Bool}
  {==}

def Laws.timer_fires_busy(o, cx, t, p, q, r):
  %Equal.sym(Bool, HP.is_paused(HX.d(o, cx)), False{}, p) : {Bool.and(Bool.not(_), HTM.may_fire(HC.bise_queued(HX.d(o, cx), HS.Timer.agent(t)), True{}, t, HC.timer_ready(o, cx, HS.Timer.agent(t)), HC.now(o, cx))) == HTM.may_fire.go(True{}, t, True{}, HC.now(o, cx)) : Bool}
  %Equal.sym(Bool, HC.bise_queued(HX.d(o, cx), HS.Timer.agent(t)), False{}, q) : {Bool.and(True{}, HTM.may_fire(_, True{}, t, HC.timer_ready(o, cx, HS.Timer.agent(t)), HC.now(o, cx))) == HTM.may_fire.go(True{}, t, True{}, HC.now(o, cx)) : Bool}
  %Equal.sym(Bool, HC.timer_ready(o, cx, HS.Timer.agent(t)), True{}, r) : {Bool.and(True{}, HTM.may_fire(False{}, True{}, t, _, HC.now(o, cx))) == HTM.may_fire.go(True{}, t, True{}, HC.now(o, cx)) : Bool}
  {==}

def Laws.paused_steers_nothing(o, cx, n, q, r):
  Laws.paused_delivers_nothing(o, cx, n, q)

def Laws.page_answer_closes(o, c, cx, to, page, text, block):
  {==}

def Laws.page_answer_tells_asker(o, cx, c, to, page, text):
  {==}

def Laws.page_answer_quiet(o, cx, c, to, page, text):
  {==}

def Laws.page_card_asker_main(s):
  {==}

def Laws.page_card_asker_first(s, h, t, q):
  %Equal.sym(Bool, HR.agent_active(s, h), True{}, q) : {HR.page_asker.pick(_, h, HR.page_asker(s, t)) == h : String}
  {==}

def Laws.page_card_asker_next(s, h, t, q):
  %Equal.sym(Bool, HR.agent_active(s, h), False{}, q) : {HR.page_asker.pick(_, h, HR.page_asker(s, t)) == HR.page_asker(s, t) : String}
  {==}

def Laws.page_card_opens(o, cx, agents, text, about):
  {==}

def Laws.page_card_opens_question(o, cx, agents, text, about):
  {==}

def Laws.page_card_other_kind(o, cx, agents, text, about):
  {==}

def Laws.card_opened_about(id, kind, agent, text, for_msg, created, place, pr, about):
  {==}

def Laws.page_asker_other(x, y, q):
  %Equal.sym(Bool, String.eq(x, y), False{}, q) : {Bool.and(True{}, Bool.not(_)) == True{} : Bool}
  {==}

def Laws.page_asker_same(x, y, q):
  %Equal.sym(Bool, String.eq(x, y), True{}, q) : {Bool.and(True{}, Bool.not(_)) == False{} : Bool}
  {==}

def Laws.page_asker_unknown(to):
  {==}

def Laws.wake_max_once(xs, id, now):
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      PL3.hub_wk_due.go(Nat.is_eq(HS.Watch.id(h), id), h, t, id, now, {==}, Laws.wake_max_once(t, id, now))

def Laws.wake_drop_ends(o, cx, n):
  +ns = HM.names_of(HX.d(o, cx), n)
  +xs = HS.Wakes.live(HC.wks(o, cx))
  +ids = HWK.ids_of(xs, ns)
  PL3.hub_wnone_tr(HS.Wakes.live(HC.wks(o, HC.wakes_gone(o, HS.LArchived{}, cx, n))), HWK.keep_out(xs, ids), n,
    PL3.hub_wk_live_each(o, ids, cx),
    PL3.hub_wk_none_keep(xs, ids, ns, n, PL3.hub_wk_all_self(ids), PL3.hub_wk_str_in_head(n, HM.names_of.go(HM.get_agent(HX.d(o, cx), n)))))

# ---- queued user input (the window's "send queued") ----

def Laws.user_queued_never_answers_a_card(o, cx, to, text, parts, from):
  {==}

def Laws.user_queued_waits_for_turn_end(o, cx, name, q, m):
  {==}

def Laws.user_queued_said_at_idle(o, cx, name, q, m):
  {==}

# split on a message's queued flag: its value and its equation
def hub_bb.pick(q: Bool, +h: HS.Msg, +t: List<&2, HS.Msg>, +e: {HS.Msg.queued(h) == q : Bool},
    ih: {Laws.any_queued(HC.not_queued_mode(t)) == False{} : Bool})
    -> {Laws.any_queued(Bool.pick(List<&2, HS.Msg>, HS.Msg.queued(h), HC.not_queued_mode(t), h <> HC.not_queued_mode(t))) == False{} : Bool}:
  match q:
    case True{}:
      %Equal.sym(Bool, HS.Msg.queued(h), True{}, e) : {Laws.any_queued(Bool.pick(List<&2, HS.Msg>, _, HC.not_queued_mode(t), h <> HC.not_queued_mode(t))) == False{} : Bool}
      ih
    case False{}:
      %Equal.sym(Bool, HS.Msg.queued(h), False{}, e) : {Laws.any_queued(Bool.pick(List<&2, HS.Msg>, _, HC.not_queued_mode(t), h <> HC.not_queued_mode(t))) == False{} : Bool}
      %Equal.sym(Bool, HS.Msg.queued(h), False{}, e) : {_ || Laws.any_queued(HC.not_queued_mode(t)) == False{} : Bool}
      ih

def Laws.busy_batch_has_no_queued(xs):
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_bb.pick(HS.Msg.queued(h), h, t, {==}, Laws.busy_batch_has_no_queued(t))

# ---- bise's cross-hub messages (desktop S2, hub/xhub.bend) ----

def Laws.xhub_only_main_forwards(s, from, id, e):
  %Equal.sym(Bool, String.eq(from, HM.MAIN()), False{}, e) : {Bool.pick(HXB.Fwd, _, HXB.forward.done(HXH.out_by_msg(HS.Xs.outbox(HS.St.x(s)), id), s, id), HXB.FwdErr{"only main forwards to the projects"}) == HXB.FwdErr{"only main forwards to the projects"} : HXB.Fwd}
  {==}

def Laws.xhub_forward_once(s, id, ob, e):
  %Equal.sym(Maybe<&2, HS.Out>, HXH.out_by_msg(HS.Xs.outbox(HS.St.x(s)), id), Some{ob}, e) : {HXB.forward.done(_, s, id) == HXB.FwdErr{String.append(String.append(HXB.mid(id), " was already forwarded to "), HS.Out.project(ob))} : HXB.Fwd}
  {==}

def Laws.xhub_forward_needs_his_message(m, id, e):
  %Equal.sym(Bool, String.eq(HS.Msg.from(m), HM.USER()), False{}, e) : {Bool.pick(HXB.Fwd, Bool.and(_, String.eq(HS.Msg.to(m), HM.MAIN())), HXB.FwdOk{HS.Msg.text(m)}, HXB.FwdErr{String.append(HXB.mid(id), " is not a message the user sent you: forward his own words only (your own question: sb project ask)")}) == HXB.FwdErr{String.append(HXB.mid(id), " is not a message the user sent you: forward his own words only (your own question: sb project ask)")} : HXB.Fwd}
  {==}

def Laws.xhub_outbox_settles_once(x, xid):
  %HSL.Xs.outbox.eta(x, HXH.out_set(HS.Xs.outbox(x), xid, HXH.OpSettled{})) : {HXB.open.of(HXH.out_of(HS.Xs.outbox(_), xid)) == None{} : Maybe<&2, HS.Out>}
  PL3.hub_x_settled(HS.Xs.outbox(x), xid)

def Laws.xhub_xin_delivered_once(x, i):
  %HSL.Xs.xin.eta(x, List.append(&2, HS.XIn, HS.Xs.xin(x), [i])) : {HXB.seen.of(HXH.xin_of(HS.Xs.xin(_), HS.XIn.hub(i), HS.XIn.xid(i))) == True{} : Bool}
  PL3.hub_x_seen(HS.Xs.xin(x), i)

def Laws.xhub_xin_replied_once(i):
  %HSL.XIn.replied.eta(i, True{}) : {HXB.reply.of(Some{_}) == None{} : Maybe<&2, HS.XIn>}
  {==}

def Laws.route_one_outcome(r, s, e):
  match s:
    case HS.RHeld{}:
      AB.absurd_eq(AB.tf_absurd(e), Maybe<&2, HS.Route>, HXB.held.of(Some{HXH.route_op(HXH.REnd{HS.RHeld{}}, r)}), None{})
    case HS.RSent{}:
      %HSL.Route.st.eta(r, HS.RSent{}) : {HXB.held.of(Some{_}) == None{} : Maybe<&2, HS.Route>}
      {==}
    case HS.RCancelled{}:
      %HSL.Route.st.eta(r, HS.RCancelled{}) : {HXB.held.of(Some{_}) == None{} : Maybe<&2, HS.Route>}
      {==}

def Laws.route_correct_only_held(o, cx, rid, to, name):
  {==}

# a correction sets the name over the target: the outer setter first
def Laws.route_correct_keeps_due(r, to, name):
  %HSL.Route.name.eta(HS.Route.with_to(r, to), name) : {HS.Route.due(_) == HS.Route.due(r) : Nat}
  %HSL.Route.to.eta(r, to) : {HS.Route.due(_) == HS.Route.due(r) : Nat}
  {==}

# the hold sets the act over the due: the outer setter first
def Laws.route_fires_at_due_only(x, tm, d, now):
  %HSL.Route.act.eta(HS.Route.with_due(HS.Route.with_rid(d, HS.Xs.next_rid(x)), Nat.add(now, HXB.HOLD_MS())), HS.Tms.next_act(tm)) : {HS.Act.due(HXB.route_act(_)) == Nat.add(now, 2000n) : Nat}
  %HSL.Route.due.eta(HS.Route.with_rid(d, HS.Xs.next_rid(x)), Nat.add(now, HXB.HOLD_MS())) : {HS.Route.due(_) == Nat.add(now, 2000n) : Nat}
  {==}

def Laws.route_fire_makes_no_turn(o, cx, r):
  {==}

def Laws.route_unclear_goes_plain(o, cx, r, e):
  %Equal.sym(Bool, HXB.is_unclear(r), True{}, e) : {HLZ.pick(HX.Cx<o>, _, u => HC.route_plain(o, cx, r), u => HC.route_fire(o, Some{r}, cx)) == HC.route_plain(o, cx, r) : HX.Cx<o>}
  {==}

def Laws.route_fire_says_where(o, cx, r, id, xid):
  {==}

def Laws.route_to_keeps_parts(r, to, name):
  %HSL.Route.name.eta(HS.Route.with_to(r, to), name) : {HS.Route.parts(_) == HS.Route.parts(r) : String}
  %HSL.Route.to.eta(r, to) : {HS.Route.parts(_) == HS.Route.parts(r) : String}
  {==}

def Laws.route_out_carries_parts(x, r, id):
  {==}

def Laws.route_no_parts_no_key():
  {==}

def Laws.xin_input_keeps_parts(text, parts):
  {==}

def Laws.xin_ask_has_no_parts(text, parts):
  {==}

def Laws.follow_quiet_before_end(o, cx, from, summary):
  {==}

def Laws.follow_done_at_its_report(o, cx, from, summary):
  {==}

def Laws.follow_failed_at_once(o, cx, from, summary):
  {==}

def Laws.follow_done_once(o, cx, name, state, summary, label, hub):
  {==}

# land-claims: drop_str leaves no `p`
def hub_drop_none.keep(b: Bool, +h: String, +t: List<&2, String>, +p: String, +e: {String.eq(h, p) == b : Bool}, ih: {HM.str_in(HN.drop_str(t, p), p) == False{} : Bool})
    -> {HM.str_in(HN.drop_str.keep(String.eq(h, p), h, HN.drop_str(t, p)), p) == False{} : Bool}:
  match b:
    case True{}:
      %Equal.sym(Bool, String.eq(h, p), True{}, e) : {HM.str_in(HN.drop_str.keep(_, h, HN.drop_str(t, p)), p) == False{} : Bool}
      ih
    case False{}:
      %Equal.sym(Bool, String.eq(h, p), False{}, e) : {HM.str_in(HN.drop_str.keep(_, h, HN.drop_str(t, p)), p) == False{} : Bool}
      PL1.hub_lor_false(String.eq(h, p), HM.str_in(HN.drop_str(t, p), p), e, ih)

def hub_drop_none(+xs: List<&2, String>, +p: String) -> {HM.str_in(HN.drop_str(xs, p), p) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{+h, +t}:
      hub_drop_none.keep(String.eq(h, p), h, t, p, {==}, hub_drop_none(t, p))

# a lazy or whose right side is true is true
def hub_lor_true_r(a: Bool, -b: Bool, e: {b == True{} : Bool}) -> {HLZ.or(a, u => b) == True{} : Bool}:
  match a:
    case True{}:
      {==}
    case False{}:
      e

# a lazy or whose left side is true is true
def hub_lor_true_l(-a: Bool, -b: Bool, e: {a == True{} : Bool}) -> {HLZ.or(a, u => b) == True{} : Bool}:
  %Equal.sym(Bool, a, True{}, e) : {HLZ.or(_, u => b) == True{} : Bool}
  {==}

# the last of `xs ++ [p]` is p
def hub_str_in_snoc(+xs: List<&2, String>, +p: String) -> {HM.str_in(HM.snoc(String, xs, p), p) == True{} : Bool}:
  match xs:
    case Nil{}:
      hub_lor_true_l(String.eq(p, p), False{}, PL2.hub_str_eq_refl(p))
    case Con{+h, +t}:
      hub_lor_true_r(String.eq(h, p), HM.str_in(HM.snoc(String, t, p), p), hub_str_in_snoc(t, p))

def hub_file_claims(c: Bool, +fs: List<&2, String>, +p: String, +e: {HM.str_in(fs, p) == c : Bool})
    -> {HM.str_in(HM.agent_op.file(c, fs, p), p) == True{} : Bool}:
  match c:
    case True{}:
      e
    case False{}:
      hub_str_in_snoc(fs, p)

def Laws.claim_dropped_unclaims(a, p):
  %Equal.sym(List<&2, String>, HS.Agent.files(HS.Agent.with_files(a, HN.drop_str(HS.Agent.files(a), p))), HN.drop_str(HS.Agent.files(a), p), HSL.Agent.files.get_with(a, HN.drop_str(HS.Agent.files(a), p))) : {HM.str_in(_, p) == False{} : Bool}
  hub_drop_none(HS.Agent.files(a), p)

def Laws.file_touched_claims(a, p):
  %Equal.sym(List<&2, String>, HS.Agent.files(HS.Agent.with_files(a, HM.agent_op.file(HM.str_in(HS.Agent.files(a), p), HS.Agent.files(a), p))), HM.agent_op.file(HM.str_in(HS.Agent.files(a), p), HS.Agent.files(a), p), HSL.Agent.files.get_with(a, HM.agent_op.file(HM.str_in(HS.Agent.files(a), p), HS.Agent.files(a), p))) : {HM.str_in(_, p) == True{} : Bool}
  hub_file_claims(HM.str_in(HS.Agent.files(a), p), HS.Agent.files(a), p, {==})

def Laws.follow_one_shot(a):
  %HSL.Agent.follow.eta(a, None{}) : {HM.follows(_) == False{} : Bool}
  {==}

def Laws.follow_hub_journaled(a, hub):
  %HSL.Agent.follow.eta(a, Some{hub}) : {HM.follow_hub(_) == hub : String}
  {==}

def Laws.follow_hub_cleared(a):
  %HSL.Agent.follow.eta(a, None{}) : {HM.follow_hub(_) == "" : String}
  {==}

def Laws.follow_end_back_only_to_who_asked(o, cx, hub, j):
  {==}

def Laws.act_fires_once(tm, id):
  +d = HTM.act_drop(HS.Tms.acts(tm), id)
  %Equal.sym(List<&2, HS.Act>, HS.Tms.acts(HS.Tms.with_acts(tm, d)), d, HSL.Tms.acts.get_with(tm, d)) : {HTM.act_find(_, id) == None{} : Maybe<&2, HS.Act>}
  PL3.hub_act_gone(HS.Tms.acts(tm), id)

def hub_due_all.go(c: Bool, +h: HS.Act, +t: List<&2, HS.Act>, +now: Nat, +e: {Nat.is_le(HS.Act.due(h), now) == c : Bool},
    ih: {Laws.all_due(HTM.acts_due(t, now), now) == True{} : Bool})
    -> {Laws.all_due(HTM.acts_due(h <> t, now), now) == True{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_le(HS.Act.due(h), now), True{}, e) : {Laws.all_due(Bool.pick(List<&2, HS.Act>, _, h <> HTM.acts_due(t, now), HTM.acts_due(t, now)), now) == True{} : Bool}
      %Equal.sym(Bool, Nat.is_le(HS.Act.due(h), now), True{}, e) : {Bool.and(_, Laws.all_due(HTM.acts_due(t, now), now)) == True{} : Bool}
      ih
    case False{}:
      %Equal.sym(Bool, Nat.is_le(HS.Act.due(h), now), False{}, e) : {Laws.all_due(Bool.pick(List<&2, HS.Act>, _, h <> HTM.acts_due(t, now), HTM.acts_due(t, now)), now) == True{} : Bool}
      ih

def Laws.act_due_only(xs, now):
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_due_all.go(Nat.is_le(HS.Act.due(h), now), h, t, now, {==}, Laws.act_due_only(t, now))

# ---- the composer rule ----

def Laws.card_confirm_words_refused(o, cx, c, text, parts, via, e):
  %Equal.sym(Bool, HA.confirm_ok(text), False{}, e) : {HC.answer.confirm(o, _, cx, c, text, via) == HC.words_refused(o, cx, c, HA.confirm_opts()) : HX.Cx<o>}
  {==}

def Laws.card_confirm_no_alias(o, cx, c, text, parts, via, e):
  %Equal.sym(Bool, HA.is_no(String.to_lower(String.trim(text))), True{}, e) : {HC.answer.confirm(o, HA.confirm_pick(_, String.trim(text)), cx, c, text, via) == HC.answer.confirm(o, True{}, cx, c, text, via) : HX.Cx<o>}
  {==}

def Laws.card_confirm_nope_is_words():
  {==}

def Laws.card_confirm_now_is_words():
  {==}

def Laws.card_confirm_no_thanks_is_words():
  {==}

def Laws.card_choice_words_refused(o, cx, c, text, parts, via, e):
  %Equal.sym(Bool, HC.is_option(HS.Card.text(c), text), False{}, e) : {HC.answer.choice(o, _, cx, c, text) == HC.words_refused(o, cx, c, "one of its numbers") : HX.Cx<o>}
  {==}

def Laws.xhub_ask_needs_words(text, b, e):
  %Equal.sym(Bool, HM.trim_is_empty(text), b, e) : {Bool.pick(Maybe<&2, String>, _, Some{"empty question"}, None{}) == Bool.pick(Maybe<&2, String>, b, Some{"empty question"}, None{}) : Maybe<&2, String>}
  {==}

# ---- the hot messages (issue 21) ----

def Laws.split_at_init(w):
  HX.Both{Unit{}, HX.Both{Unit{}, HX.Both{Unit{}, HX.Both{Unit{}, HX.Both{Unit{}, Unit{}}}}}}

def Laws.split_kept_by_run(ins, h, u):
  PLC4.hc_split_tr(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)), Laws.hub_run_replays(ins, h),
    PLC4.hc_split_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h)))

def Laws.split_after_replay(ms, st, i, sk, u):
  PLC4.hc_replay_batch(ms, st, i, sk, u)

def Laws.ids_ascending(s, u):
  PLC2.hc_every_asc(HS.St.hot(s), HS.St.old(s), HS.St.next_msg(s), u)

def Laws.cold_is_inert(s, e, u, k):
  PLC4.hc_every_apply(s, e, u, PLC4.hc_fresh_ge(s, e, k))

def Laws.take_moves_live_queued(r, n, h, q, e):
  PLC5.hm_live_queued(r, n, h, q, e)

def Laws.take_moves_live_owed(r, n, h, o, e):
  PLC5.hm_live_owed(r, n, h, o, e)

def msgs_moved_hot(+s: HS.St, +n: String, +h: String) -> {HS.St.hot(HM.apply(s, HM.EMsgsMoved{n, h})) == HM.moved(HS.St.hot(s), n, h) : List<&2, HS.MRec>}:
  %HSL.St.open_eq(s) : {HS.St.hot(HM.apply(_, HM.EMsgsMoved{n, h})) == HM.moved(HS.St.hot(s), n, h) : List<&2, HS.MRec>}
  {==}

def Laws.msgs_moved_replays(s, n, h, u):
  HX.Both{msgs_moved_hot(s, n, h), PLC4.hc_every_apply(s, HM.EMsgsMoved{n, h}, u, Unit{})}

# a cold message owes no reply (PL3.hub_or_false_l: and is not queued)
def hub_or_false_r(+x: Bool, -y: Bool, e: {Bool.or(x, y) == False{} : Bool}) -> {y == False{} : Bool}:
  match x:
    case False{}:
      e
    case True{}:
      AB.absurd_eq(AB.ft_absurd(Equal.sym(Bool, True{}, False{}, e)), Bool, y, False{})

# pause.bend's is_queued is model.bend's
def hub_pq_false(+s: HS.MSt, e: {HM.is_queued(s) == False{} : Bool}) -> {HP.is_queued(s) == False{} : Bool}:
  match s:
    case HS.Queued{w}:
      AB.absurd_eq(AB.ft_absurd(Equal.sym(Bool, True{}, False{}, e)), Bool, HP.is_queued(HS.Queued{w}), False{})
    case HS.Delivered{}:
      {==}
    case HS.Rejected{w}:
      {==}
    case HS.Cancelled{}:
      {==}

# unanswered_in over every message == over the hot ones
def hub_ui_hot.pick(c: Bool, +r: HS.MRec, +t: List<&2, HS.MRec>, +ns: List<&2, String>, +e: {HM.MRec.is_hot(r) == c : Bool},
    ih: {HM.unanswered_in(t, ns) == HM.unanswered_in(HM.hot_of(t), ns) : List<&2, HS.Msg>})
    -> {HM.unanswered_in(r <> t, ns) == HM.unanswered_in(Bool.pick(List<&2, HS.MRec>, HM.MRec.is_hot(r), r <> HM.hot_of(t), HM.hot_of(t)), ns) : List<&2, HS.Msg>}:
  match c:
    case True{}:
      %Equal.sym(Bool, HM.MRec.is_hot(r), True{}, e) : {HM.unanswered_in(r <> t, ns) == HM.unanswered_in(Bool.pick(List<&2, HS.MRec>, _, r <> HM.hot_of(t), HM.hot_of(t)), ns) : List<&2, HS.Msg>}
      Equal.cong(List<&2, HS.Msg>, List<&2, HS.Msg>,
        x => Bool.pick(List<&2, HS.Msg>, Bool.and(HM.str_in(ns, HS.Msg.to(HS.MRec.msg(r))), HM.MRec.owes(r)), HS.MRec.msg(r) <> x, x),
        HM.unanswered_in(t, ns), HM.unanswered_in(HM.hot_of(t), ns), ih)
    case False{}:
      %Equal.sym(Bool, HM.MRec.is_hot(r), False{}, e) : {HM.unanswered_in(r <> t, ns) == HM.unanswered_in(Bool.pick(List<&2, HS.MRec>, _, r <> HM.hot_of(t), HM.hot_of(t)), ns) : List<&2, HS.Msg>}
      %Equal.sym(Bool, HM.MRec.owes(r), False{}, hub_or_false_r(HM.is_queued(HS.MRec.st(r)), HM.MRec.owes(r), e)) : {Bool.pick(List<&2, HS.Msg>, Bool.and(HM.str_in(ns, HS.Msg.to(HS.MRec.msg(r))), _), HS.MRec.msg(r) <> HM.unanswered_in(t, ns), HM.unanswered_in(t, ns)) == HM.unanswered_in(HM.hot_of(t), ns) : List<&2, HS.Msg>}
      %Equal.sym(Bool, Bool.and(HM.str_in(ns, HS.Msg.to(HS.MRec.msg(r))), False{}), False{}, PL3.hub_and_false(HM.str_in(ns, HS.Msg.to(HS.MRec.msg(r))))) : {Bool.pick(List<&2, HS.Msg>, _, HS.MRec.msg(r) <> HM.unanswered_in(t, ns), HM.unanswered_in(t, ns)) == HM.unanswered_in(HM.hot_of(t), ns) : List<&2, HS.Msg>}
      ih

def hub_ui_hot(+xs: List<&2, HS.MRec>, +ns: List<&2, String>) -> {HM.unanswered_in(xs, ns) == HM.unanswered_in(HM.hot_of(xs), ns) : List<&2, HS.Msg>}:
  match xs:
    case Nil{}:
      {==}
    case Con{r, t}:
      hub_ui_hot.pick(HM.MRec.is_hot(r), r, t, ns, {==}, hub_ui_hot(t, ns))

# the pause's walks over every message == over the hot ones
def hub_held_hot.pick(c: Bool, +r: HS.MRec, +t: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(r) == c : Bool},
    ih: {HP.held.go(t) == HP.held.go(HM.hot_of(t)) : Nat})
    -> {HP.held.go(r <> t) == HP.held.go(Bool.pick(List<&2, HS.MRec>, HM.MRec.is_hot(r), r <> HM.hot_of(t), HM.hot_of(t))) : Nat}:
  match c:
    case True{}:
      %Equal.sym(Bool, HM.MRec.is_hot(r), True{}, e) : {HP.held.go(r <> t) == HP.held.go(Bool.pick(List<&2, HS.MRec>, _, r <> HM.hot_of(t), HM.hot_of(t))) : Nat}
      Equal.cong(Nat, Nat, x => Bool.pick(Nat, HP.is_queued(HS.MRec.st(r)), (1n + x : Nat), x), HP.held.go(t), HP.held.go(HM.hot_of(t)), ih)
    case False{}:
      %Equal.sym(Bool, HM.MRec.is_hot(r), False{}, e) : {HP.held.go(r <> t) == HP.held.go(Bool.pick(List<&2, HS.MRec>, _, r <> HM.hot_of(t), HM.hot_of(t))) : Nat}
      %Equal.sym(Bool, HP.is_queued(HS.MRec.st(r)), False{}, hub_pq_false(HS.MRec.st(r), PL3.hub_or_false_l(HM.is_queued(HS.MRec.st(r)), HM.MRec.owes(r), e))) : {Bool.pick(Nat, _, (1n + HP.held.go(t) : Nat), HP.held.go(t)) == HP.held.go(HM.hot_of(t)) : Nat}
      ih

def hub_held_hot(+xs: List<&2, HS.MRec>) -> {HP.held.go(xs) == HP.held.go(HM.hot_of(xs)) : Nat}:
  match xs:
    case Nil{}:
      {==}
    case Con{r, t}:
      hub_held_hot.pick(HM.MRec.is_hot(r), r, t, {==}, hub_held_hot(t))

def hub_his_hot.pick(c: Bool, +r: HS.MRec, +t: List<&2, HS.MRec>, +e: {HM.MRec.is_hot(r) == c : Bool},
    ih: {HP.his.go(t) == HP.his.go(HM.hot_of(t)) : Nat})
    -> {HP.his.go(r <> t) == HP.his.go(Bool.pick(List<&2, HS.MRec>, HM.MRec.is_hot(r), r <> HM.hot_of(t), HM.hot_of(t))) : Nat}:
  match c:
    case True{}:
      %Equal.sym(Bool, HM.MRec.is_hot(r), True{}, e) : {HP.his.go(r <> t) == HP.his.go(Bool.pick(List<&2, HS.MRec>, _, r <> HM.hot_of(t), HM.hot_of(t))) : Nat}
      Equal.cong(Nat, Nat, x => Bool.pick(Nat, Bool.and(HP.is_queued(HS.MRec.st(r)), Bool.not(String.eq(HS.Msg.from(HS.MRec.msg(r)), HM.HUB()))), (1n + x : Nat), x), HP.his.go(t), HP.his.go(HM.hot_of(t)), ih)
    case False{}:
      %Equal.sym(Bool, HM.MRec.is_hot(r), False{}, e) : {HP.his.go(r <> t) == HP.his.go(Bool.pick(List<&2, HS.MRec>, _, r <> HM.hot_of(t), HM.hot_of(t))) : Nat}
      %Equal.sym(Bool, HP.is_queued(HS.MRec.st(r)), False{}, hub_pq_false(HS.MRec.st(r), PL3.hub_or_false_l(HM.is_queued(HS.MRec.st(r)), HM.MRec.owes(r), e))) : {Bool.pick(Nat, Bool.and(_, Bool.not(String.eq(HS.Msg.from(HS.MRec.msg(r)), HM.HUB()))), (1n + HP.his.go(t) : Nat), HP.his.go(t)) == HP.his.go(HM.hot_of(t)) : Nat}
      ih

def hub_his_hot(+xs: List<&2, HS.MRec>) -> {HP.his.go(xs) == HP.his.go(HM.hot_of(xs)) : Nat}:
  match xs:
    case Nil{}:
      {==}
    case Con{r, t}:
      hub_his_hot.pick(HM.MRec.is_hot(r), r, t, {==}, hub_his_hot(t))

# woken.go, for every accumulator
def hub_wk_hot.pick(c: Bool, +r: HS.MRec, +t: List<&2, HS.MRec>, +acc: List<&2, String>, +e: {HM.MRec.is_hot(r) == c : Bool},
    ih1: {HP.woken.go(t, Bool.pick(List<&2, String>, Bool.and(HP.is_queued(HS.MRec.st(r)), Bool.not(HP.str_in(acc, HS.Msg.to(HS.MRec.msg(r))))), HS.Msg.to(HS.MRec.msg(r)) <> acc, acc))
      == HP.woken.go(HM.hot_of(t), Bool.pick(List<&2, String>, Bool.and(HP.is_queued(HS.MRec.st(r)), Bool.not(HP.str_in(acc, HS.Msg.to(HS.MRec.msg(r))))), HS.Msg.to(HS.MRec.msg(r)) <> acc, acc)) : List<&2, String>},
    ih0: {HP.woken.go(t, acc) == HP.woken.go(HM.hot_of(t), acc) : List<&2, String>})
    -> {HP.woken.go(r <> t, acc) == HP.woken.go(Bool.pick(List<&2, HS.MRec>, HM.MRec.is_hot(r), r <> HM.hot_of(t), HM.hot_of(t)), acc) : List<&2, String>}:
  match c:
    case True{}:
      %Equal.sym(Bool, HM.MRec.is_hot(r), True{}, e) : {HP.woken.go(r <> t, acc) == HP.woken.go(Bool.pick(List<&2, HS.MRec>, _, r <> HM.hot_of(t), HM.hot_of(t)), acc) : List<&2, String>}
      ih1
    case False{}:
      %Equal.sym(Bool, HM.MRec.is_hot(r), False{}, e) : {HP.woken.go(r <> t, acc) == HP.woken.go(Bool.pick(List<&2, HS.MRec>, _, r <> HM.hot_of(t), HM.hot_of(t)), acc) : List<&2, String>}
      %Equal.sym(Bool, HP.is_queued(HS.MRec.st(r)), False{}, hub_pq_false(HS.MRec.st(r), PL3.hub_or_false_l(HM.is_queued(HS.MRec.st(r)), HM.MRec.owes(r), e))) : {HP.woken.go(t, Bool.pick(List<&2, String>, Bool.and(_, Bool.not(HP.str_in(acc, HS.Msg.to(HS.MRec.msg(r))))), HS.Msg.to(HS.MRec.msg(r)) <> acc, acc)) == HP.woken.go(HM.hot_of(t), acc) : List<&2, String>}
      ih0

def hub_wk_hot(+xs: List<&2, HS.MRec>, +acc: List<&2, String>) -> {HP.woken.go(xs, acc) == HP.woken.go(HM.hot_of(xs), acc) : List<&2, String>}:
  match xs:
    case Nil{}:
      {==}
    case Con{r, t}:
      hub_wk_hot.pick(HM.MRec.is_hot(r), r, t, acc, {==},
        hub_wk_hot(t, Bool.pick(List<&2, String>, Bool.and(HP.is_queued(HS.MRec.st(r)), Bool.not(HP.str_in(acc, HS.Msg.to(HS.MRec.msg(r))))), HS.Msg.to(HS.MRec.msg(r)) <> acc, acc)),
        hub_wk_hot(t, acc))

def Laws.hot_queued_for(s, n, h):
  PL3.hub_hot_qf(s, n, PLC4.hc_hot_index(s, h))

def Laws.hot_unanswered_for(s, n, h):
  PL3.hub_via_hot(List<&2, HS.Msg>, x => HM.unanswered_in(x, HM.names_of(s, n)), HS.St.hot(s), HM.every(s),
    hub_ui_hot(HS.St.hot(s), HM.names_of(s, n)), hub_ui_hot(HM.every(s), HM.names_of(s, n)), PLC4.hc_hot_index(s, h))

def Laws.hot_held(s, h):
  PL3.hub_via_hot(Nat, x => HP.held.go(x), HS.St.hot(s), HM.every(s), hub_held_hot(HS.St.hot(s)), hub_held_hot(HM.every(s)), PLC4.hc_hot_index(s, h))

def Laws.hot_woken(s, h):
  PL3.hub_via_hot(Nat, x => List.length(&2, String, HP.woken.go(x, Nil{})), HS.St.hot(s), HM.every(s),
    Equal.cong(List<&2, String>, Nat, y => List.length(&2, String, y), HP.woken.go(HS.St.hot(s), Nil{}), HP.woken.go(HM.hot_of(HS.St.hot(s)), Nil{}), hub_wk_hot(HS.St.hot(s), Nil{})),
    Equal.cong(List<&2, String>, Nat, y => List.length(&2, String, y), HP.woken.go(HM.every(s), Nil{}), HP.woken.go(HM.hot_of(HM.every(s)), Nil{}), hub_wk_hot(HM.every(s), Nil{})),
    PLC4.hc_hot_index(s, h))

def Laws.hot_his(s, h):
  PL3.hub_via_hot(Nat, x => HP.his.go(x), HS.St.hot(s), HM.every(s), hub_his_hot(HS.St.hot(s)), hub_his_hot(HM.every(s)), PLC4.hc_hot_index(s, h))

# ---- MCP App views ----

# a new card or a joined one: neither touches a runtime
def hv_rt_at(-o: HS.St, m: Maybe<&2, HS.Card>, +cx: HX.Cx<o>, +v: HS.ViewAsk, +text: String)
    -> {HX.R.rts(HX.Cx.r(o, HC.open_view_confirm.at(o, m, cx, v, text))) == HX.R.rts(HX.Cx.r(o, cx)) : List<&2, HX.Rt>}:
  match m:
    case None{}:
      match cx:
        case HX.Cx{c, r}:
          match r:
            case HX.R{a, b, st, d, e, f, g, h, i, j, k, l, m, n, vc}:
              {==}
    case Some{x}:
      match cx:
        case HX.Cx{c, r}:
          {==}

# open_view_confirm's two branches: the card (outside the runtime) or the
# notice; neither touches a runtime
def hv_rt(-o: HS.St, ok: Bool, +cx: HX.Cx<o>, +v: HS.ViewAsk, +text: String)
    -> {HX.R.rts(HX.Cx.r(o, HC.open_view_confirm.go(o, ok, cx, v, text))) == HX.R.rts(HX.Cx.r(o, cx)) : List<&2, HX.Rt>}:
  match ok:
    case False{}:
      match cx:
        case HX.Cx{c, r}:
          {==}
    case True{}:
      hv_rt_at(o, HC.view_card_of(HS.St.cards(HX.d(o, cx)), v), cx, v, text)

def Laws.view_card_keeps_runtime(o, cx, agent, call, text, ask):
  hv_rt(o, HC.live_agent(HM.agent_named(HS.St.agents(HX.d(o, cx)), agent)), cx, HC.view_ask(agent, call, ask), text)

# an effect out journals nothing
def hv_out_journal(-o: HS.St, +cx: HX.Cx<o>, +x: J.Json)
    -> {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HX.out(o, cx, x)))) == HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))) : List<&2, HM.Ev>}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          Equal.sym(List<&2, HM.Ev>, HX.journal(fx), HX.journal(HM.snoc(HX.Fx, fx, HX.FO{x})), HX.journal_snoc_out(fx, x))

def Laws.view_msg_needs_active(o, cx, agent, plugin, text, e):
  %Equal.sym(Bool, HC.live_agent(HM.agent_named(HS.St.agents(HX.d(o, cx)), agent)), False{}, e) : {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.view_message.go(o, _, cx, agent, plugin, text)))) == HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))) : List<&2, HM.Ev>}
  hv_out_journal(o, cx, J.obj(HF.notice(HC.cat(["the view's message didn't reach @", agent, ": it is not active"]))))

def Laws.view_card_needs_active(o, cx, agent, call, text, ask, e):
  %Equal.sym(Bool, HC.live_agent(HM.agent_named(HS.St.agents(HX.d(o, cx)), agent)), False{}, e) : {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.open_view_confirm.go(o, _, cx, HC.view_ask(agent, call, ask), text)))) == HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))) : List<&2, HM.Ev>}
  +n = J.obj(HF.notice(HC.cat(["the view's call didn't reach @", agent, ": it is not active"])))
  +said = J.obj(HF.view_card(agent, call, None{}))
  Equal.trans(List<&2, HM.Ev>, HX.journal(HX.Core.fx(o, HX.Cx.c(o, HX.out(o, HX.out(o, cx, n), said)))), HX.journal(HX.Core.fx(o, HX.Cx.c(o, HX.out(o, cx, n)))), HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))),
    hv_out_journal(o, HX.out(o, cx, n), said), hv_out_journal(o, cx, n))

# the last effect of effects with one more at their end is that one
def hv_last_snoc(+xs: List<&2, HX.Fx>, +y: HX.Fx) -> {Laws.last_fx(HM.snoc(HX.Fx, xs, y)) == Some{y} : Maybe<&2, HX.Fx>}:
  match xs:
    case Nil{}:
      {==}
    case Con{+h, +t}:
      %Equal.sym(Maybe<&2, HX.Fx>, Laws.last_fx(HM.snoc(HX.Fx, t, y)), Some{y}, hv_last_snoc(t, y)) : {Laws.last_fx.or(_, h) == Some{y} : Maybe<&2, HX.Fx>}
      {==}

# an effect out is the step's last
def hv_last_out(-o: HS.St, +cx: HX.Cx<o>, +x: J.Json)
    -> {Laws.last_fx(HX.Core.fx(o, HX.Cx.c(o, HX.out(o, cx, x)))) == Some{HX.FO{x}} : Maybe<&2, HX.Fx>}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          hv_last_snoc(fx, HX.FO{x})

def Laws.view_card_says_its_id(o, cx, v, text):
  hv_last_out(o, HC.open_view_confirm.card(o, cx, v, text), J.obj(HF.view_card(HS.ViewAsk.agent(v), HS.ViewAsk.call(v), Some{HS.St.next_card(HX.d(o, cx))})))

def Laws.view_card_joins_same_ask(o, cx, c, v, text):
  hv_out_journal(o, cx, J.obj(HF.view_card(HS.ViewAsk.agent(v), HS.ViewAsk.call(v), Some{HS.Card.id(c)})))

def Laws.view_card_joined_says_its_id(o, cx, c, v, text):
  hv_last_out(o, cx, J.obj(HF.view_card(HS.ViewAsk.agent(v), HS.ViewAsk.call(v), Some{HS.Card.id(c)})))

# view_card_of's step: the head is that ask's card or not
def hv_of.go(h: Bool, +c: HS.Card, +rest: List<&2, HS.Card>, +v: HS.ViewAsk,
    +eh: {HC.view_is(HS.Card.view(c), v) == h : Bool},
    ih: {Laws.found_is(HC.view_card_of(rest, v), v) == True{} : Bool})
    -> {Laws.found_is(HC.view_card_of(c <> rest, v), v) == True{} : Bool}:
  match h:
    case True{}:
      %Equal.sym(Bool, HC.view_is(HS.Card.view(c), v), True{}, eh) : {Laws.found_is(HLZ.pick(Maybe<&2, HS.Card>, _, u => Some{c}, u => HC.view_card_of(rest, v)), v) == True{} : Bool}
      eh
    case False{}:
      %Equal.sym(Bool, HC.view_is(HS.Card.view(c), v), False{}, eh) : {Laws.found_is(HLZ.pick(Maybe<&2, HS.Card>, _, u => Some{c}, u => HC.view_card_of(rest, v)), v) == True{} : Bool}
      ih

def Laws.view_card_of_its_ask(cs, v):
  match cs:
    case Nil{}:
      {==}
    case Con{+c, +rest}:
      hv_of.go(HC.view_is(HS.Card.view(c), v), c, rest, v, {==}, Laws.view_card_of_its_ask(rest, v))

def Laws.view_card_typed(id, text, v, now):
  {==}

def Laws.view_card_id(id, text, v, now):
  {==}

def Laws.view_card_says_no_card(o, cx, agent, call, text, ask, e):
  %Equal.sym(Bool, HC.live_agent(HM.agent_named(HS.St.agents(HX.d(o, cx)), agent)), False{}, e) : {Laws.last_fx(HX.Core.fx(o, HX.Cx.c(o, HC.open_view_confirm.go(o, _, cx, HC.view_ask(agent, call, ask), text)))) == Some{HX.FO{J.obj(HF.view_card(agent, call, None{}))}} : Maybe<&2, HX.Fx>}
  hv_last_out(o, HX.out(o, cx, J.obj(HF.notice(HC.cat(["the view's call didn't reach @", agent, ": it is not active"])))), J.obj(HF.view_card(agent, call, None{})))

def hv_add0(n: Nat) -> {Nat.add(n, 0n) == n : Nat}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      %hv_add0(p) : {1n+Nat.add(p, 0n) == 1n+_ : Nat}
      {==}

def hv_assoc(a: Nat, +b: Nat, +c: Nat) -> {Nat.add(Nat.add(a, b), c) == Nat.add(a, Nat.add(b, c)) : Nat}:
  match a:
    case 0n:
      {==}
    case 1n+p:
      %hv_assoc(p, b, c) : {1n+Nat.add(Nat.add(p, b), c) == 1n+_ : Nat}
      {==}

# the confirms of the effects with one more at their end
def hv_csnoc(+xs: List<&2, HX.Fx>, +y: HX.Fx)
    -> {Laws.confirms(HM.snoc(HX.Fx, xs, y)) == Nat.add(Laws.confirms(xs), Laws.fx_confirms(y)) : Nat}:
  match xs:
    case Nil{}:
      hv_add0(Laws.fx_confirms(y))
    case Con{+h, +t}:
      %Equal.sym(Nat, Laws.confirms(HM.snoc(HX.Fx, t, y)), Nat.add(Laws.confirms(t), Laws.fx_confirms(y)), hv_csnoc(t, y)) : {Nat.add(Laws.fx_confirms(h), _) == Nat.add(Nat.add(Laws.fx_confirms(h), Laws.confirms(t)), Laws.fx_confirms(y)) : Nat}
      Equal.sym(Nat, Nat.add(Nat.add(Laws.fx_confirms(h), Laws.confirms(t)), Laws.fx_confirms(y)), Nat.add(Laws.fx_confirms(h), Nat.add(Laws.confirms(t), Laws.fx_confirms(y))),
        hv_assoc(Laws.fx_confirms(h), Laws.confirms(t), Laws.fx_confirms(y)))

# one effect out whose confirms are k
def hv_csnoc_k(+xs: List<&2, HX.Fx>, +y: HX.Fx, +k: Nat, ek: {Laws.fx_confirms(y) == k : Nat})
    -> {Laws.confirms(HM.snoc(HX.Fx, xs, y)) == Nat.add(Laws.confirms(xs), k) : Nat}:
  %ek : {Laws.confirms(HM.snoc(HX.Fx, xs, y)) == Nat.add(Laws.confirms(xs), _) : Nat}
  hv_csnoc(xs, y)

# a confirm, a journal event, a feed line: one confirm more
def hv_c3(+xs: List<&2, HX.Fx>, +a: HX.Fx, +b: HX.Fx, +c: HX.Fx,
    ea: {Laws.fx_confirms(a) == 1n : Nat}, eb: {Laws.fx_confirms(b) == 0n : Nat}, ec: {Laws.fx_confirms(c) == 0n : Nat})
    -> {Laws.confirms(HM.snoc(HX.Fx, HM.snoc(HX.Fx, HM.snoc(HX.Fx, xs, a), b), c)) == Nat.add(Laws.confirms(xs), 1n) : Nat}:
  +s1 = HM.snoc(HX.Fx, xs, a)
  +s2 = HM.snoc(HX.Fx, s1, b)
  +e3 = Equal.trans(Nat, Laws.confirms(HM.snoc(HX.Fx, s2, c)), Nat.add(Laws.confirms(s2), 0n), Laws.confirms(s2),
    hv_csnoc_k(s2, c, 0n, ec), hv_add0(Laws.confirms(s2)))
  +e2 = Equal.trans(Nat, Laws.confirms(s2), Nat.add(Laws.confirms(s1), 0n), Laws.confirms(s1),
    hv_csnoc_k(s1, b, 0n, eb), hv_add0(Laws.confirms(s1)))
  Equal.trans(Nat, Laws.confirms(HM.snoc(HX.Fx, s2, c)), Laws.confirms(s1), Nat.add(Laws.confirms(xs), 1n),
    Equal.trans(Nat, Laws.confirms(HM.snoc(HX.Fx, s2, c)), Laws.confirms(s2), Laws.confirms(s1), e3, e2),
    hv_csnoc_k(xs, a, 1n, ea))

# an out, an event, an out: that event only
def hv_j3(+xs: List<&2, HX.Fx>, +x: J.Json, +e: HM.Ev, +y: J.Json)
    -> {HX.journal(HM.snoc(HX.Fx, HM.snoc(HX.Fx, HM.snoc(HX.Fx, xs, HX.FO{x}), HX.FJ{e}), HX.FO{y})) == HM.snoc(HM.Ev, HX.journal(xs), e) : List<&2, HM.Ev>}:
  +s1 = HM.snoc(HX.Fx, xs, HX.FO{x})
  +s2 = HM.snoc(HX.Fx, s1, HX.FJ{e})
  %HX.journal_snoc_out(s2, y) : {_ == HM.snoc(HM.Ev, HX.journal(xs), e) : List<&2, HM.Ev>}
  %HX.journal_snoc_ev(s1, e) : {_ == HM.snoc(HM.Ev, HX.journal(xs), e) : List<&2, HM.Ev>}
  %HX.journal_snoc_out(xs, x) : {HM.snoc(HM.Ev, _, e) == HM.snoc(HM.Ev, HX.journal(xs), e) : List<&2, HM.Ev>}
  {==}

def Laws.view_card_due_answers(o, cx, c):
  match cx:
    case HX.Cx{core, r}:
      match core:
        case HX.Core{d, fx, p, f, q, v}:
          hv_c3(fx, HX.FO{J.obj(HF.confirm_card(HS.Card.id(c), HS.Card.agent(c), HC.VIEW_CARD_LATE()))},
            HX.FJ{HM.ECardClosed{HS.Card.id(c), "timed out", HS.Ans.empty()}},
            HX.FO{J.obj(HF.feed(HM.MAIN(), HFD.card_closed(HS.Card.id(c), "timed out")))}, {==}, {==}, {==})

def Laws.view_card_due_closes(o, cx, c):
  match cx:
    case HX.Cx{core, r}:
      match core:
        case HX.Core{d, fx, p, f, q, v}:
          hv_j3(fx, J.obj(HF.confirm_card(HS.Card.id(c), HS.Card.agent(c), HC.VIEW_CARD_LATE())),
            HM.ECardClosed{HS.Card.id(c), "timed out", HS.Ans.empty()},
            J.obj(HF.feed(HM.MAIN(), HFD.card_closed(HS.Card.id(c), "timed out"))))

def Laws.view_card_due_after_close(o, cx):
  {==}

# a confirm out: one confirm more
def hv_out_c(-o: HS.St, +cx: HX.Cx<o>, +x: J.Json, ex: {Laws.confirm_n(x) == 1n : Nat})
    -> {Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HX.out(o, cx, x)))) == Nat.add(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), 1n) : Nat}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          hv_csnoc_k(fx, HX.FO{x}, 1n, ex)

# view_nos' step: the card is the agent's view card (one confirm) or not
def hv_nos.go(-o: HS.St, hit: Bool, +c: HS.Card, +rest: List<&2, HS.Card>, +cx: HX.Cx<o>, +agent: String,
    ih: {Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_nos(o, rest, HC.view_no.one(o, hit, cx, c), agent))))
      == Nat.add(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_no.one(o, hit, cx, c)))), Laws.view_cards(rest, agent)) : Nat})
    -> {Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_nos(o, rest, HC.view_no.one(o, hit, cx, c), agent))))
      == Nat.add(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), Bool.pick(Nat, hit, 1n+Laws.view_cards(rest, agent), Laws.view_cards(rest, agent))) : Nat}:
  match hit:
    case False{}:
      ih
    case True{}:
      +x = J.obj(HF.confirm_card(HS.Card.id(c), HS.Card.agent(c), HC.VIEW_CARD_STOPPED()))
      +k = Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx)))
      +rc = Laws.view_cards(rest, agent)
      Equal.trans(Nat, Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_nos(o, rest, HX.out(o, cx, x), agent)))),
        Nat.add(Nat.add(k, 1n), rc), Nat.add(k, 1n+rc),
        Equal.trans(Nat, Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_nos(o, rest, HX.out(o, cx, x), agent)))),
          Nat.add(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HX.out(o, cx, x)))), rc), Nat.add(Nat.add(k, 1n), rc),
          ih,
          Equal.cong(Nat, Nat, y => Nat.add(y, rc), Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HX.out(o, cx, x)))), Nat.add(k, 1n), hv_out_c(o, cx, x, {==}))),
        hv_assoc(k, 1n, rc))

def Laws.view_card_stop_answers(o, cs, cx, agent):
  match cs:
    case Nil{}:
      Equal.sym(Nat, Nat.add(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), 0n), Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), hv_add0(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx)))))
    case Con{+c, +rest}:
      +h = String.eq(HS.Card.agent(c), agent) && HC.is_view(HS.Card.view(c))
      hv_nos.go(o, h, c, rest, cx, agent, Laws.view_card_stop_answers(o, rest, HC.view_no.one(o, h, cx, c), agent))

# boot's answer to one view card: a confirm, its close, its feed line
def hv_end_one(-o: HS.St, +cx: HX.Cx<o>, +c: HS.Card)
    -> {Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_ends.one(o, True{}, cx, c)))) == Nat.add(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), 1n) : Nat}:
  match cx:
    case HX.Cx{core, r}:
      match core:
        case HX.Core{d, fx, p, f, q, v}:
          hv_c3(fx, HX.FO{J.obj(HF.confirm_card(HS.Card.id(c), HS.Card.agent(c), HC.VIEW_CARD_ENDED()))},
            HX.FJ{HM.ECardClosed{HS.Card.id(c), "ended with the hub", HS.Ans.empty()}},
            HX.FO{J.obj(HF.feed(HM.MAIN(), HFD.card_closed(HS.Card.id(c), "ended with the hub")))}, {==}, {==}, {==})

# view_ends' step: the card is a view's (one confirm) or not
def hv_ends.go(-o: HS.St, hit: Bool, +c: HS.Card, +rest: List<&2, HS.Card>, +cx: HX.Cx<o>,
    ih: {Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_ends(o, rest, HC.view_ends.one(o, hit, cx, c)))))
      == Nat.add(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_ends.one(o, hit, cx, c)))), Laws.view_cards_all(rest)) : Nat})
    -> {Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_ends(o, rest, HC.view_ends.one(o, hit, cx, c)))))
      == Nat.add(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), Bool.pick(Nat, hit, 1n+Laws.view_cards_all(rest), Laws.view_cards_all(rest))) : Nat}:
  match hit:
    case False{}:
      ih
    case True{}:
      +one = HC.view_ends.one(o, True{}, cx, c)
      +k = Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx)))
      +rc = Laws.view_cards_all(rest)
      Equal.trans(Nat, Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_ends(o, rest, one)))),
        Nat.add(Nat.add(k, 1n), rc), Nat.add(k, 1n+rc),
        Equal.trans(Nat, Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_ends(o, rest, one)))),
          Nat.add(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, one))), rc), Nat.add(Nat.add(k, 1n), rc),
          ih,
          Equal.cong(Nat, Nat, y => Nat.add(y, rc), Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, one))), Nat.add(k, 1n), hv_end_one(o, cx, c))),
        hv_assoc(k, 1n, rc))

def Laws.boot_answers_view_cards(o, cs, cx):
  match cs:
    case Nil{}:
      Equal.sym(Nat, Nat.add(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), 0n), Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), hv_add0(Laws.confirms(HX.Core.fx(o, HX.Cx.c(o, cx)))))
    case Con{+c, +rest}:
      +h = HC.is_view(HS.Card.view(c))
      hv_ends.go(o, h, c, rest, cx, Laws.boot_answers_view_cards(o, rest, HC.view_ends.one(o, h, cx, c)))

def Laws.boot_closes_view_cards(o, cx, c):
  match cx:
    case HX.Cx{core, r}:
      match core:
        case HX.Core{d, fx, p, f, q, v}:
          hv_j3(fx, J.obj(HF.confirm_card(HS.Card.id(c), HS.Card.agent(c), HC.VIEW_CARD_ENDED())),
            HM.ECardClosed{HS.Card.id(c), "ended with the hub", HS.Ans.empty()},
            J.obj(HF.feed(HM.MAIN(), HFD.card_closed(HS.Card.id(c), "ended with the hub"))))

def Laws.view_context_says_kept(o, ok, cx, c):
  match ok:
    case False{}:
      hv_last_out(o, cx, J.obj(HF.view_context(HS.Vctx.agent(c), HS.Vctx.call(c), False{}, HC.view_context.why(False{}, HS.Vctx.agent(c)))))
    case True{}:
      hv_last_out(o, HC.set_r(o, cx, HX.R.with_ctx(HC.rr(o, cx), HVC.put(HX.R.ctx(HC.rr(o, cx)), c))), J.obj(HF.view_context(HS.Vctx.agent(c), HS.Vctx.call(c), True{}, "")))

def Laws.view_context_none_text(text):
  {==}

def Laws.view_context_none_parts(parts, text):
  {==}

# view_context_taken_once's step: the head is that agent's or not
def hv_take.go(h: Bool, +c: HS.Vctx, +rest: List<&2, HS.Vctx>, +a: String,
    +eh: {String.eq(HVC.agent_of(c), a) == h : Bool},
    ih: {HVC.of(HVC.drop(rest, a), a) == Nil{} : List<&2, HS.Vctx>})
    -> {HVC.of(HVC.drop(c <> rest, a), a) == Nil{} : List<&2, HS.Vctx>}:
  match h:
    case True{}:
      %Equal.sym(Bool, String.eq(HVC.agent_of(c), a), True{}, eh) : {HVC.of(HLZ.pick(List<&2, HS.Vctx>, _, u => HVC.drop(rest, a), u => c <> HVC.drop(rest, a)), a) == Nil{} : List<&2, HS.Vctx>}
      ih
    case False{}:
      %Equal.sym(Bool, String.eq(HVC.agent_of(c), a), False{}, eh) : {HVC.of(HLZ.pick(List<&2, HS.Vctx>, _, u => HVC.drop(rest, a), u => c <> HVC.drop(rest, a)), a) == Nil{} : List<&2, HS.Vctx>}
      %Equal.sym(Bool, String.eq(HVC.agent_of(c), a), False{}, eh) : {HLZ.pick(List<&2, HS.Vctx>, _, u => c <> HVC.of(HVC.drop(rest, a), a), u => HVC.of(HVC.drop(rest, a), a)) == Nil{} : List<&2, HS.Vctx>}
      ih

def Laws.view_context_taken_once(cs, agent):
  match cs:
    case Nil{}:
      {==}
    case Con{+c, +rest}:
      hv_take.go(String.eq(HVC.agent_of(c), agent), c, rest, agent, {==}, Laws.view_context_taken_once(rest, agent))

# view_context_keeps_others' step: the head is a's (so not b's) or not
def hv_keep_ctx.go(h: Bool, +c: HS.Vctx, +rest: List<&2, HS.Vctx>, +a: String, +b: String,
    +e: {String.eq(a, b) == False{} : Bool},
    +eh: {String.eq(HVC.agent_of(c), a) == h : Bool},
    ih: {HVC.of(HVC.drop(rest, a), b) == HVC.of(rest, b) : List<&2, HS.Vctx>})
    -> {HVC.of(HVC.drop(c <> rest, a), b) == HVC.of(c <> rest, b) : List<&2, HS.Vctx>}:
  match h:
    case True{}:
      %Equal.sym(Bool, String.eq(HVC.agent_of(c), a), True{}, eh) : {HVC.of(HLZ.pick(List<&2, HS.Vctx>, _, u => HVC.drop(rest, a), u => c <> HVC.drop(rest, a)), b) == HVC.of(c <> rest, b) : List<&2, HS.Vctx>}
      %Equal.sym(String, HVC.agent_of(c), a, PL2.hub_str_eq_sound(HVC.agent_of(c), a, eh)) : {HVC.of(HVC.drop(rest, a), b) == HLZ.pick(List<&2, HS.Vctx>, String.eq(_, b), u => c <> HVC.of(rest, b), u => HVC.of(rest, b)) : List<&2, HS.Vctx>}
      %Equal.sym(Bool, String.eq(a, b), False{}, e) : {HVC.of(HVC.drop(rest, a), b) == HLZ.pick(List<&2, HS.Vctx>, _, u => c <> HVC.of(rest, b), u => HVC.of(rest, b)) : List<&2, HS.Vctx>}
      ih
    case False{}:
      %Equal.sym(Bool, String.eq(HVC.agent_of(c), a), False{}, eh) : {HVC.of(HLZ.pick(List<&2, HS.Vctx>, _, u => HVC.drop(rest, a), u => c <> HVC.drop(rest, a)), b) == HVC.of(c <> rest, b) : List<&2, HS.Vctx>}
      Equal.cong(List<&2, HS.Vctx>, List<&2, HS.Vctx>, y => HLZ.pick(List<&2, HS.Vctx>, String.eq(HVC.agent_of(c), b), u => c <> y, u => y),
        HVC.of(HVC.drop(rest, a), b), HVC.of(rest, b), ih)

def hv_keep_ctx(+cs: List<&2, HS.Vctx>, +a: String, +b: String, +e: {String.eq(a, b) == False{} : Bool})
    -> {HVC.of(HVC.drop(cs, a), b) == HVC.of(cs, b) : List<&2, HS.Vctx>}:
  match cs:
    case Nil{}:
      {==}
    case Con{+c, +rest}:
      hv_keep_ctx.go(String.eq(HVC.agent_of(c), a), c, rest, a, b, e, {==}, hv_keep_ctx(rest, a, b, e))

def Laws.view_context_keeps_others(cs, a, b, e):
  hv_keep_ctx(cs, a, b, e)

def Laws.card_close_keeps_other(xs, a, b, e):
  PLV.hv_keep(xs, a, b, e)

def Laws.card_close_commutes(xs, a, b):
  PLV.hv_comm(xs, a, b)

def Laws.view_msg_from_view(agent, plugin, text, id, now):
  {==}

def Laws.view_name_taken(st):
  {==}

def Laws.view_msg_to_its_agent(agent, plugin, text, id, now):
  {==}

def Laws.view_msg_queued(agent, plugin, text, id, now):
  {==}

def Laws.view_msg_plugin(agent, plugin, text, id, now):
  {==}

# ---- c4b-core-step-flat: St.order changes only by an order event ----

def apply_order.go(+e: HM.Ev, +s: HS.St, k: {HM.order_ev(e) == False{} : Bool}) -> {HS.St.order(HM.apply(HS.St.open(s), e)) == HS.St.order(s) : List<&2, String>}:
  match e:
    case HM.ETaskCreated{x0, x1, x2, x3, x4}:
      AB.absurd_eq(AB.tf_absurd(k), List<&2, String>, HS.St.order(HM.apply(HS.St.open(s), HM.ETaskCreated{x0, x1, x2, x3, x4})), HS.St.order(s))
    case HM.ELifecycle{x0, x1, x2}:
      {==}
    case HM.EWorkspace{x0, x1}:
      {==}
    case HM.ESnapshot{x0, x1}:
      {==}
    case HM.EPlaced{x0, x1, x2}:
      {==}
    case HM.EDropped{x0, x1, x2}:
      {==}
    case HM.EDeclared{x0, x1, x2}:
      {==}
    case HM.EReported{x0, x1}:
      {==}
    case HM.EFileTouched{x0, x1}:
      {==}
    case HM.EClaimDropped{x0, x1}:
      {==}
    case HM.ERenamed{x0, x1}:
      AB.absurd_eq(AB.tf_absurd(k), List<&2, String>, HS.St.order(HM.apply(HS.St.open(s), HM.ERenamed{x0, x1})), HS.St.order(s))
    case HM.EMsgsMoved{x0, x1}:
      {==}
    case HM.EMsgSent{x0}:
      {==}
    case HM.EMsgState{x0, x1}:
      {==}
    case HM.ESettled{x0}:
      {==}
    case HM.ECardOpened{x0}:
      {==}
    case HM.ECardClosed{x0, x1, x2}:
      {==}
    case HM.ECardAnswerPos{x0, x1, x2}:
      {==}
    case HM.ECardWithdrawn{x0, x1}:
      {==}
    case HM.ECardLate{x0}:
      {==}
    case HM.EMainNote{x0, x1}:
      {==}
    case HM.ENotesFlushed{}:
      {==}
    case HM.ETimerSet{x0, x1}:
      {==}
    case HM.ETimerFired{x0, x1, x2, x3, x4}:
      {==}
    case HM.ETimerRun{x0, x1}:
      {==}
    case HM.ETimerStop{x0, x1, x2}:
      {==}
    case HM.ETimerName{x0, x1, x2, x3}:
      {==}
    case HM.EX{x0}:
      {==}
    case HM.EAct{x0}:
      {==}
    case HM.EFollow{x0, x1, x2}:
      {==}
    case HM.EJobEnd{x0, x1}:
      {==}
    case HM.EWake{x0}:
      {==}
    case HM.EPaused{x0}:
      {==}
    case HM.EResumed{x0}:
      {==}

def Laws.apply_keeps_order(s, e, k):
  %HSL.St.open_eq(s) : {HS.St.order(HM.apply(_, e)) == HS.St.order(s) : List<&2, String>}
  apply_order.go(e, s, k)

# ---- c4b-core-step-flat: no agent holds a never_agent name ----

def Laws.init_no_reserved_names(w):
  HX.Both{HX.Both{{==}, Unit{}}, Unit{}}

def Laws.no_reserved_names(ins, h, u):
  PLK.cl_tr(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h),
    PLK.cl_apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h)))

def Laws.never_agent_named(s, u, n, k):
  match u:
    case HX.Both{la, lr}:
      PLK.cl_named(List.append(&2, HS.Agent, HS.St.agents(s), HS.St.arch(s)), n, k, PLK.cl_join(HS.St.agents(s), HS.St.arch(s), la, lr))

def Laws.empty_name_held(s):
  {==}

def Laws.empty_name_refused(o, cx, t, e):
  %Equal.sym(Bool, String.is_empty(HC.NewTask.base(t)), True{}, e) : {HC.create_task.go(o, _, cx, t) == (cx, HC.TErr{"a new task needs a name"}) : HX.Cx<o> & HC.TaskR}
  {==}
