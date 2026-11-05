# bend/proofs/lib/hub-2.bend: helpers the hub proofs share (moved from
# PROOF.bend), layered: a lib calls only the ones before it.
import Base
import ../../vendor/json.bend as J
import ../../hub/model.bend as HM
import ../../hub/gen/state.bend as HS
import ../../hub/gen/state-laws.bend as HSL
import ../../hub/cx.bend as HX
import ../../hub/core.bend as HC
import ../../hub/inv.bend as HI
import ../../hub/timers.bend as HTM
import ../../hub/wakes.bend as HWK
import ../../hub/gen/feed.bend as HFD
import ./hub-1.bend as PL1
import ../../hub/lazy.bend as HLZ
import ../../hub/names.bend as HN

def hub_cards_wakes(-o: HS.St, +life: HS.Life, +cx: HX.Cx<o>, +n: String)
    -> {HS.St.cards(HX.d(o, HC.wakes_gone(o, life, cx, n))) == HS.St.cards(HX.d(o, cx)) : List<&2, HS.Card>}:
  match life:
    case HS.LActive{}:
      {==}
    case HS.LStopped{}:
      {==}
    case HS.LFailed{}:
      {==}
    case HS.LArchived{}:
      PL1.hub_cards_wakes_each(o, HWK.ids_of(HS.Wakes.live(HC.wks(o, cx)), HM.names_of(HX.d(o, cx), n)), cx)

def hub_msgs_gone(-o: HS.St, +g: Bool, +cx: HX.Cx<o>, +n: String, +ns: List<&2, String>)
    -> {HM.every(HX.d(o, HC.timers_gone.if(o, g, cx, n, ns))) == HM.every(HX.d(o, cx)) : List<&2, HS.MRec>}:
  match g:
    case False{}:
      {==}
    case True{}:
      PL1.hub_msgs_gone_each(o, HTM.ids_in(HC.live(o, cx), ns), cx, HC.cat(["@", n, " is gone"]))

def hub_agents_gone(-o: HS.St, +g: Bool, +cx: HX.Cx<o>, +n: String, +ns: List<&2, String>)
    -> {PL1.hub_ag(HX.d(o, HC.timers_gone.if(o, g, cx, n, ns))) == PL1.hub_ag(HX.d(o, cx)) : List<&2, HS.Agent>}:
  match g:
    case False{}:
      {==}
    case True{}:
      PL1.hub_agents_gone_each(o, HTM.ids_in(HC.live(o, cx), ns), cx, HC.cat(["@", n, " is gone"]))

def hub_cards_gone(-o: HS.St, +g: Bool, +cx: HX.Cx<o>, +n: String, +ns: List<&2, String>)
    -> {HS.St.cards(HX.d(o, HC.timers_gone.if(o, g, cx, n, ns))) == HS.St.cards(HX.d(o, cx)) : List<&2, HS.Card>}:
  match g:
    case False{}:
      {==}
    case True{}:
      PL1.hub_cards_gone_each(o, HTM.ids_in(HC.live(o, cx), ns), cx, HC.cat(["@", n, " is gone"]))

def hub_d_feed(-o: HS.St, +cx: HX.Cx<o>, +a: String, +rec: J.Json) -> {HX.d(o, HC.feed(o, cx, a, rec)) == HX.d(o, cx) : HS.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

# closing cards keeps the messages
def hub_msgs_close_ids(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +res: String)
    -> {HM.every(HX.d(o, HC.close_ids(o, ids, cx, res))) == HM.every(HX.d(o, cx)) : List<&2, HS.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, rest}:
      +cx2 = HC.emit(o, cx, HM.ECardClosed{id, res, HS.Ans.empty()}, Unit{}, Unit{}, Unit{})
      +cx3 = HC.feed(o, cx2, HM.MAIN(), HFD.card_closed(id, res))
      Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, HC.close_ids(o, rest, cx3, res))), HM.every(HX.d(o, cx3)), HM.every(HX.d(o, cx)),
        hub_msgs_close_ids(o, rest, cx3, res),
        Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, cx3)), HM.every(HX.d(o, cx2)), HM.every(HX.d(o, cx)),
          PL1.hub_msgs_eq(o, cx3, cx2, hub_d_feed(o, cx2, HM.MAIN(), HFD.card_closed(id, res))),
          Equal.trans(List<&2, HS.MRec>, HM.every(HX.d(o, cx2)), HM.every(HM.apply(HX.d(o, cx), HM.ECardClosed{id, res, HS.Ans.empty()})), HM.every(HX.d(o, cx)),
            Equal.cong(HS.St, List<&2, HS.MRec>, s => HM.every(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), HM.ECardClosed{id, res, HS.Ans.empty()}),
              PL1.hub_d_emit(o, cx, HM.ECardClosed{id, res, HS.Ans.empty()}, Unit{}, Unit{}, Unit{})),
            PL1.hub_msgs_closed(HX.d(o, cx), id, res))))

def hub_agents_eq(-o: HS.St, -a: HX.Cx<o>, -b: HX.Cx<o>, e: {HX.d(o, a) == HX.d(o, b) : HS.St}) -> {PL1.hub_ag(HX.d(o, a)) == PL1.hub_ag(HX.d(o, b)) : List<&2, HS.Agent>}:
  Equal.cong(HS.St, List<&2, HS.Agent>, s => PL1.hub_ag(s), HX.d(o, a), HX.d(o, b), e)

def hub_agents_closed(+s: HS.St, +id: Nat, +res: String) -> {PL1.hub_ag(HM.apply(s, HM.ECardClosed{id, res, HS.Ans.empty()})) == PL1.hub_ag(s) : List<&2, HS.Agent>}:
  %HSL.St.open_eq(s) : {PL1.hub_ag(HM.apply(_, HM.ECardClosed{id, res, HS.Ans.empty()})) == PL1.hub_ag(s) : List<&2, HS.Agent>}
  {==}

def hub_agents_close_ids(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +res: String)
    -> {PL1.hub_ag(HX.d(o, HC.close_ids(o, ids, cx, res))) == PL1.hub_ag(HX.d(o, cx)) : List<&2, HS.Agent>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, rest}:
      +ev = {HM.ECardClosed{id, res, HS.Ans.empty()} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      +cx3 = HC.feed(o, cx2, HM.MAIN(), HFD.card_closed(id, res))
      Equal.trans(List<&2, HS.Agent>, PL1.hub_ag(HX.d(o, HC.close_ids(o, rest, cx3, res))), PL1.hub_ag(HX.d(o, cx3)), PL1.hub_ag(HX.d(o, cx)),
        hub_agents_close_ids(o, rest, cx3, res),
        Equal.trans(List<&2, HS.Agent>, PL1.hub_ag(HX.d(o, cx3)), PL1.hub_ag(HX.d(o, cx2)), PL1.hub_ag(HX.d(o, cx)),
          hub_agents_eq(o, cx3, cx2, hub_d_feed(o, cx2, HM.MAIN(), HFD.card_closed(id, res))),
          Equal.trans(List<&2, HS.Agent>, PL1.hub_ag(HX.d(o, cx2)), PL1.hub_ag(HM.apply(HX.d(o, cx), ev)), PL1.hub_ag(HX.d(o, cx)),
            Equal.cong(HS.St, List<&2, HS.Agent>, s => PL1.hub_ag(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), PL1.hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
            hub_agents_closed(HX.d(o, cx), id, res))))

def hub_agents_state(+s: HS.St, +id: Nat, +x: HS.MSt) -> {PL1.hub_ag(HM.apply(s, HM.EMsgState{id, x})) == PL1.hub_ag(s) : List<&2, HS.Agent>}:
  %HSL.St.open_eq(s) : {PL1.hub_ag(HM.apply(_, HM.EMsgState{id, x})) == PL1.hub_ag(s) : List<&2, HS.Agent>}
  {==}

def hub_agents_reject(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>)
    -> {PL1.hub_ag(HX.d(o, HC.reject_ids(o, ids, cx))) == PL1.hub_ag(HX.d(o, cx)) : List<&2, HS.Agent>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.EMsgState{id, HS.Rejected{"recipient_unavailable"}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HS.Agent>, PL1.hub_ag(HX.d(o, HC.reject_ids(o, t, cx2))), PL1.hub_ag(HX.d(o, cx2)), PL1.hub_ag(HX.d(o, cx)),
        hub_agents_reject(o, t, cx2),
        Equal.trans(List<&2, HS.Agent>, PL1.hub_ag(HX.d(o, cx2)), PL1.hub_ag(HM.apply(HX.d(o, cx), ev)), PL1.hub_ag(HX.d(o, cx)),
          Equal.cong(HS.St, List<&2, HS.Agent>, s => PL1.hub_ag(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), PL1.hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_agents_state(HX.d(o, cx), id, HS.Rejected{"recipient_unavailable"})))

def hub_names_tr(-a: List<&2, HS.MRec>, -b: List<&2, HS.MRec>, -na: List<&2, String>, -nb: List<&2, String>,
    e: {a == b : List<&2, HS.MRec>}, en: {na == nb : List<&2, String>},
    k: {HM.queued_in(b, nb) == Nil{} : List<&2, HS.Msg>}) -> {HM.queued_in(a, na) == Nil{} : List<&2, HS.Msg>}:
  %Equal.sym(List<&2, HS.MRec>, a, b, e) : {HM.queued_in(_, na) == Nil{} : List<&2, HS.Msg>}
  %Equal.sym(List<&2, String>, na, nb, en) : {HM.queued_in(b, _) == Nil{} : List<&2, HS.Msg>}
  k

# the cards after the closes of stop_task
def hub_dropc(ids: List<&2, Nat>, +cs: List<&2, HS.Card>) -> List<&2, HS.Card>:
  match ids:
    case Nil{}:
      cs
    case Con{+id, +t}:
      hub_dropc(t, HM.drop_card(cs, id))

def hub_dropc_skip.go(+i: Nat, -ns: List<&2, Nat>, +h: HS.Card, +t: List<&2, HS.Card>,
    ef: {Nat.is_eq(HS.Card.id(h), i) == False{} : Bool},
    ih: {hub_dropc(ns, h <> HM.drop_card(t, i)) == h <> hub_dropc(ns, HM.drop_card(t, i)) : List<&2, HS.Card>})
    -> {hub_dropc(i <> ns, h <> t) == h <> hub_dropc(i <> ns, t) : List<&2, HS.Card>}:
  %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), i), False{}, ef) : {hub_dropc(ns, HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, i))) == h <> hub_dropc(i <> ns, t) : List<&2, HS.Card>}
  ih

def hub_dropc_skip(+ids: List<&2, Nat>, +h: HS.Card, +t: List<&2, HS.Card>, +nf: {HM.nat_in(ids, HS.Card.id(h)) == False{} : Bool})
    -> {hub_dropc(ids, h <> t) == h <> hub_dropc(ids, t) : List<&2, HS.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{i, ns}:
      hub_dropc_skip.go(i, ns, h, t,
        Equal.trans(Bool, Nat.is_eq(HS.Card.id(h), i), Nat.is_eq(i, HS.Card.id(h)), False{},
          PL1.hub_eq_sym(HS.Card.id(h), i),
          PL1.hub_lor_false_l(Nat.is_eq(i, HS.Card.id(h)), HM.nat_in(ns, HS.Card.id(h)), nf)),
        hub_dropc_skip(ns, h, HM.drop_card(t, i), PL1.hub_lor_false_r(Nat.is_eq(i, HS.Card.id(h)), HM.nat_in(ns, HS.Card.id(h)), nf)))

def hub_c_sub.pick(c: Bool, +h: HS.Card, -t: List<&2, HS.Card>, -n: String, +x: Nat,
    +e: {Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))) == c : Bool}, e1: {Nat.is_eq(HS.Card.id(h), x) == False{} : Bool},
    ih: {HM.nat_in(HC.card_ids(t, n, Nil{}), x) == False{} : Bool})
    -> {HM.nat_in(Bool.pick(List<&2, Nat>, Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))), HS.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), x) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(Bool, Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))), True{}, e) : {HM.nat_in(Bool.pick(List<&2, Nat>, _, HS.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), x) == False{} : Bool}
      PL1.hub_lor_false(Nat.is_eq(HS.Card.id(h), x), HM.nat_in(HC.card_ids(t, n, Nil{}), x), e1, ih)
    case False{}:
      %Equal.sym(Bool, Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))), False{}, e) : {HM.nat_in(Bool.pick(List<&2, Nat>, _, HS.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), x) == False{} : Bool}
      ih

def hub_c_sub(+t: List<&2, HS.Card>, +x: Nat, +n: String, +nf: {HM.nat_in(HI.card_ids(t), x) == False{} : Bool})
    -> {HM.nat_in(HC.card_ids(t, n, Nil{}), x) == False{} : Bool}:
  match t:
    case Nil{}:
      {==}
    case Con{h, tt}:
      hub_c_sub.pick(Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))), h, tt, n, x, {==},
        PL1.hub_lor_false_l(Nat.is_eq(HS.Card.id(h), x), HM.nat_in(HI.card_ids(tt), x), nf),
        hub_c_sub(tt, x, n, PL1.hub_lor_false_r(Nat.is_eq(HS.Card.id(h), x), HM.nat_in(HI.card_ids(tt), x), nf)))

def hub_KC.pick(c: Bool, +h: HS.Card, +t: List<&2, HS.Card>, +n: String,
    +e: {Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))) == c : Bool}, +dh: {HM.nat_in(HI.card_ids(t), HS.Card.id(h)) == False{} : Bool},
    ih: {HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), t), n, Nil{}) == Nil{} : List<&2, Nat>})
    -> {HC.card_ids(hub_dropc(Bool.pick(List<&2, Nat>, Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))), HS.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), h <> t), n, Nil{}) == Nil{} : List<&2, Nat>}:
  match c:
    case True{}:
      %Equal.sym(Bool, Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))), True{}, e) : {HC.card_ids(hub_dropc(Bool.pick(List<&2, Nat>, _, HS.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), h <> t), n, Nil{}) == Nil{} : List<&2, Nat>}
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), HS.Card.id(h)), True{}, PL1.hub_eq_refl(HS.Card.id(h))) : {HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, HS.Card.id(h)))), n, Nil{}) == Nil{} : List<&2, Nat>}
      ih
    case False{}:
      %Equal.sym(Bool, Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))), False{}, e) : {HC.card_ids(hub_dropc(Bool.pick(List<&2, Nat>, _, HS.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), h <> t), n, Nil{}) == Nil{} : List<&2, Nat>}
      %Equal.sym(List<&2, HS.Card>, hub_dropc(HC.card_ids(t, n, Nil{}), h <> t), h <> hub_dropc(HC.card_ids(t, n, Nil{}), t), hub_dropc_skip(HC.card_ids(t, n, Nil{}), h, t, hub_c_sub(t, HS.Card.id(h), n, dh))) : {HC.card_ids(_, n, Nil{}) == Nil{} : List<&2, Nat>}
      %Equal.sym(Bool, Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))), False{}, e) : {Bool.pick(List<&2, Nat>, _, HS.Card.id(h) <> HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), t), n, Nil{}), HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), t), n, Nil{})) == Nil{} : List<&2, Nat>}
      ih

# closing every card of `n` leaves none of its cards
def hub_KC(+cs: List<&2, HS.Card>, +n: String, d: HI.Distinct(HI.card_ids(cs))) -> {HC.card_ids(hub_dropc(HC.card_ids(cs, n, Nil{}), cs), n, Nil{}) == Nil{} : List<&2, Nat>}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match d:
        case HX.Both{dh, dt}:
          hub_KC.pick(Bool.and(String.eq(HS.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HS.Card.kind(h)))), h, t, n, {==}, dh, hub_KC(t, n, dt))

def hub_cards_eq(-o: HS.St, -a: HX.Cx<o>, -b: HX.Cx<o>, e: {HX.d(o, a) == HX.d(o, b) : HS.St}) -> {HS.St.cards(HX.d(o, a)) == HS.St.cards(HX.d(o, b)) : List<&2, HS.Card>}:
  Equal.cong(HS.St, List<&2, HS.Card>, s => HS.St.cards(s), HX.d(o, a), HX.d(o, b), e)

def hub_cards_closed(+s: HS.St, +id: Nat, +res: String) -> {HS.St.cards(HM.apply(s, HM.ECardClosed{id, res, HS.Ans.empty()})) == HM.drop_card(HS.St.cards(s), id) : List<&2, HS.Card>}:
  %HSL.St.open_eq(s) : {HS.St.cards(HM.apply(_, HM.ECardClosed{id, res, HS.Ans.empty()})) == HM.drop_card(HS.St.cards(s), id) : List<&2, HS.Card>}
  {==}

def hub_cards_life(+s: HS.St, +n: String, +l: HS.Life, +r: Maybe<&2, String>) -> {HS.St.cards(HM.apply(s, HM.ELifecycle{n, l, r})) == HS.St.cards(s) : List<&2, HS.Card>}:
  %HSL.St.open_eq(s) : {HS.St.cards(HM.apply(_, HM.ELifecycle{n, l, r})) == HS.St.cards(s) : List<&2, HS.Card>}
  {==}

def hub_cards_state(+s: HS.St, +id: Nat, +x: HS.MSt) -> {HS.St.cards(HM.apply(s, HM.EMsgState{id, x})) == HS.St.cards(s) : List<&2, HS.Card>}:
  %HSL.St.open_eq(s) : {HS.St.cards(HM.apply(_, HM.EMsgState{id, x})) == HS.St.cards(s) : List<&2, HS.Card>}
  {==}

# rejecting messages keeps the cards
def hub_cards_reject(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>)
    -> {HS.St.cards(HX.d(o, HC.reject_ids(o, ids, cx))) == HS.St.cards(HX.d(o, cx)) : List<&2, HS.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.EMsgState{id, HS.Rejected{"recipient_unavailable"}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, HC.reject_ids(o, t, cx2))), HS.St.cards(HX.d(o, cx2)), HS.St.cards(HX.d(o, cx)),
        hub_cards_reject(o, t, cx2),
        Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, cx2)), HS.St.cards(HM.apply(HX.d(o, cx), ev)), HS.St.cards(HX.d(o, cx)),
          Equal.cong(HS.St, List<&2, HS.Card>, s => HS.St.cards(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), PL1.hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_cards_state(HX.d(o, cx), id, HS.Rejected{"recipient_unavailable"})))

# closing cards drops them, one by one
def hub_cards_close_ids(-o: HS.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +res: String)
    -> {HS.St.cards(HX.d(o, HC.close_ids(o, ids, cx, res))) == hub_dropc(ids, HS.St.cards(HX.d(o, cx))) : List<&2, HS.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, rest}:
      +ev = {HM.ECardClosed{id, res, HS.Ans.empty()} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      +cx3 = HC.feed(o, cx2, HM.MAIN(), HFD.card_closed(id, res))
      Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, HC.close_ids(o, rest, cx3, res))), hub_dropc(rest, HS.St.cards(HX.d(o, cx3))),
        hub_dropc(id <> rest, HS.St.cards(HX.d(o, cx))),
        hub_cards_close_ids(o, rest, cx3, res),
        Equal.cong(List<&2, HS.Card>, List<&2, HS.Card>, cs => hub_dropc(rest, cs), HS.St.cards(HX.d(o, cx3)), HM.drop_card(HS.St.cards(HX.d(o, cx)), id),
          Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, cx3)), HS.St.cards(HX.d(o, cx2)), HM.drop_card(HS.St.cards(HX.d(o, cx)), id),
            hub_cards_eq(o, cx3, cx2, hub_d_feed(o, cx2, HM.MAIN(), HFD.card_closed(id, res))),
            Equal.trans(List<&2, HS.Card>, HS.St.cards(HX.d(o, cx2)), HS.St.cards(HM.apply(HX.d(o, cx), ev)), HM.drop_card(HS.St.cards(HX.d(o, cx)), id),
              Equal.cong(HS.St, List<&2, HS.Card>, s => HS.St.cards(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), PL1.hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
              hub_cards_closed(HX.d(o, cx), id, res)))))

def hub_cids_tr(-a: List<&2, HS.Card>, -b: List<&2, HS.Card>, -n: String, e: {a == b : List<&2, HS.Card>},
    k: {HC.card_ids(b, n, Nil{}) == Nil{} : List<&2, Nat>}) -> {HC.card_ids(a, n, Nil{}) == Nil{} : List<&2, Nat>}:
  %Equal.sym(List<&2, HS.Card>, a, b, e) : {HC.card_ids(_, n, Nil{}) == Nil{} : List<&2, Nat>}
  k

# ---- L12 support: no two open cards share an id (cards_unique) ----

def hub_below_notin(+ns: List<&2, Nat>, +x: Nat, bl: HI.Below(ns, x)) -> {HM.nat_in(ns, x) == False{} : Bool}:
  match ns:
    case Nil{}:
      {==}
    case Con{y, t}:
      match bl:
        case HX.Both{ly, bt}:
          PL1.hub_lor_false(Nat.is_eq(y, x), HM.nat_in(t, x),
            Equal.trans(Bool, Nat.is_eq(y, x), Nat.is_eq(x, y), False{}, PL1.hub_eq_sym(y, x), PL1.hub_lt_neq(y, x, ly)),
            hub_below_notin(t, x, bt))

def hub_drop_absent.go(c: Bool, +h: HS.Card, +t: List<&2, HS.Card>, -id: Nat, +e: {Nat.is_eq(HS.Card.id(h), id) == c : Bool},
    ec: {c == False{} : Bool}, ih: {HM.drop_card(t, id) == t : List<&2, HS.Card>}) -> {HM.drop_card(h <> t, id) == h <> t : List<&2, HS.Card>}:
  match c:
    case True{}:
      PL1.hub_absurd_eq(PL1.hub_tf_absurd(ec), List<&2, HS.Card>, HM.drop_card(h <> t, id), h <> t)
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), id), False{}, e) : {HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, id)) == h <> t : List<&2, HS.Card>}
      Equal.cong(List<&2, HS.Card>, List<&2, HS.Card>, xs => h <> xs, HM.drop_card(t, id), t, ih)

# dropping an id no card has changes nothing
def hub_drop_absent(+cs: List<&2, HS.Card>, +id: Nat, +nf: {HM.nat_in(HI.card_ids(cs), id) == False{} : Bool}) -> {HM.drop_card(cs, id) == cs : List<&2, HS.Card>}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_drop_absent.go(Nat.is_eq(HS.Card.id(h), id), h, t, id, {==},
        PL1.hub_lor_false_l(Nat.is_eq(HS.Card.id(h), id), HM.nat_in(HI.card_ids(t), id), nf),
        hub_drop_absent(t, id, PL1.hub_lor_false_r(Nat.is_eq(HS.Card.id(h), id), HM.nat_in(HI.card_ids(t), id), nf)))

def hub_cids_snoc(+cs: List<&2, HS.Card>, +c: HS.Card) -> {HI.card_ids(HM.snoc(HS.Card, cs, c)) == HM.snoc(Nat, HI.card_ids(cs), HS.Card.id(c)) : List<&2, Nat>}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.cong(List<&2, Nat>, List<&2, Nat>, ys => HS.Card.id(h) <> ys, HI.card_ids(HM.snoc(HS.Card, t, c)), HM.snoc(Nat, HI.card_ids(t), HS.Card.id(c)), hub_cids_snoc(t, c))

def hub_drop_notin.go(c: Bool, +h: HS.Card, +t: List<&2, HS.Card>, -id: Nat, +x: Nat, +e: {Nat.is_eq(HS.Card.id(h), id) == c : Bool},
    +nf: {HM.nat_in(HI.card_ids(h <> t), x) == False{} : Bool}, ih: {HM.nat_in(HI.card_ids(HM.drop_card(t, id)), x) == False{} : Bool})
    -> {HM.nat_in(HI.card_ids(HM.drop_card(h <> t, id)), x) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), id), True{}, e) : {HM.nat_in(HI.card_ids(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, id))), x) == False{} : Bool}
      PL1.hub_lor_false_r(Nat.is_eq(HS.Card.id(h), x), HM.nat_in(HI.card_ids(t), x), nf)
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), id), False{}, e) : {HM.nat_in(HI.card_ids(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, id))), x) == False{} : Bool}
      PL1.hub_lor_false(Nat.is_eq(HS.Card.id(h), x), HM.nat_in(HI.card_ids(HM.drop_card(t, id)), x),
        PL1.hub_lor_false_l(Nat.is_eq(HS.Card.id(h), x), HM.nat_in(HI.card_ids(t), x), nf), ih)

# dropping a card never brings back an id
def hub_drop_notin(+cs: List<&2, HS.Card>, +id: Nat, +x: Nat, +nf: {HM.nat_in(HI.card_ids(cs), x) == False{} : Bool})
    -> {HM.nat_in(HI.card_ids(HM.drop_card(cs, id)), x) == False{} : Bool}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_drop_notin.go(Nat.is_eq(HS.Card.id(h), id), h, t, id, x, {==}, nf,
        hub_drop_notin(t, id, x, PL1.hub_lor_false_r(Nat.is_eq(HS.Card.id(h), x), HM.nat_in(HI.card_ids(t), x), nf)))

def hub_ucards_cons(+h: HS.Card, -ys: List<&2, HS.Card>, -n: Nat, nf: {HM.nat_in(HI.card_ids(ys), HS.Card.id(h)) == False{} : Bool},
    lh: {Nat.is_lt(HS.Card.id(h), n) == True{} : Bool}, u: HX.Both<HI.Distinct(HI.card_ids(ys)), HI.Below(HI.card_ids(ys), n)>)
    -> HX.Both<HI.Distinct(HI.card_ids(h <> ys)), HI.Below(HI.card_ids(h <> ys), n)>:
  match u:
    case HX.Both{d2, b2}:
      HX.Both{HX.Both{nf, d2}, HX.Both{lh, b2}}

def hub_ucards_drop.go(c: Bool, +h: HS.Card, +t: List<&2, HS.Card>, +id: Nat, -n: Nat, +e: {Nat.is_eq(HS.Card.id(h), id) == c : Bool},
    +dh: {HM.nat_in(HI.card_ids(t), HS.Card.id(h)) == False{} : Bool}, +lh: {Nat.is_lt(HS.Card.id(h), n) == True{} : Bool},
    +ut: HX.Both<HI.Distinct(HI.card_ids(t)), HI.Below(HI.card_ids(t), n)>, ih: HX.Both<HI.Distinct(HI.card_ids(HM.drop_card(t, id))), HI.Below(HI.card_ids(HM.drop_card(t, id)), n)>) -> HX.Both<HI.Distinct(HI.card_ids(HM.drop_card(h <> t, id))), HI.Below(HI.card_ids(HM.drop_card(h <> t, id)), n)>:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), id), True{}, e) : HX.Both<HI.Distinct(HI.card_ids(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, id)))), HI.Below(HI.card_ids(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, id))), n)>
      ut
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), id), False{}, e) : HX.Both<HI.Distinct(HI.card_ids(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, id)))), HI.Below(HI.card_ids(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, id))), n)>
      hub_ucards_cons(h, HM.drop_card(t, id), n, hub_drop_notin(t, id, HS.Card.id(h), dh), lh, ih)

# closing a card keeps the open cards unique
def hub_ucards_drop(+cs: List<&2, HS.Card>, +id: Nat, +n: Nat, u: HX.Both<HI.Distinct(HI.card_ids(cs)), HI.Below(HI.card_ids(cs), n)>) -> HX.Both<HI.Distinct(HI.card_ids(HM.drop_card(cs, id))), HI.Below(HI.card_ids(HM.drop_card(cs, id)), n)>:
  match cs:
    case Nil{}:
      u
    case Con{h, t}:
      match u:
        case HX.Both{d, b}:
          match d b:
            case HX.Both{+dh, +dt} HX.Both{+lh, +bt}:
              hub_ucards_drop.go(Nat.is_eq(HS.Card.id(h), id), h, t, id, n, {==}, dh, lh, HX.Both{dt, bt}, hub_ucards_drop(t, id, n, HX.Both{dt, bt}))

# opening a card with the next free id keeps the open cards unique
def hub_ucards_open(+cs: List<&2, HS.Card>, +c: HS.Card, +nc: Nat, +k: {HS.Card.id(c) == nc : Nat}, u: HX.Both<HI.Distinct(HI.card_ids(cs)), HI.Below(HI.card_ids(cs), nc)>)
    -> HX.Both<HI.Distinct(HI.card_ids(HM.put_card(cs, c))), HI.Below(HI.card_ids(HM.put_card(cs, c)), Nat.max(nc, 1n+HS.Card.id(c)))>:
  match u:
    case HX.Both{+d, +b}:
      +top = Nat.max(nc, 1n+HS.Card.id(c))
      +absent = hub_drop_absent(cs, HS.Card.id(c), hub_below_notin(HI.card_ids(cs), HS.Card.id(c), PL1.hub_below_n(HI.card_ids(cs), HS.Card.id(c), nc, k, b)))
      +e = Equal.trans(List<&2, Nat>, HI.card_ids(HM.put_card(cs, c)), HI.card_ids(HM.snoc(HS.Card, cs, c)), HM.snoc(Nat, HI.card_ids(cs), HS.Card.id(c)),
        Equal.cong(List<&2, HS.Card>, List<&2, Nat>, xs => HI.card_ids(HM.snoc(HS.Card, xs, c)), HM.drop_card(cs, HS.Card.id(c)), cs, absent),
        hub_cids_snoc(cs, c))
      HX.Both{PL1.hub_distinct_tr(HI.card_ids(HM.put_card(cs, c)), HM.snoc(Nat, HI.card_ids(cs), HS.Card.id(c)), e,
          PL1.hub_distinct_snoc(HI.card_ids(cs), HS.Card.id(c), d, PL1.hub_below_n(HI.card_ids(cs), HS.Card.id(c), nc, k, b))),
        PL1.hub_below_tr(HI.card_ids(HM.put_card(cs, c)), HM.snoc(Nat, HI.card_ids(cs), HS.Card.id(c)), top, e,
          PL1.hub_below_snoc(HI.card_ids(cs), HS.Card.id(c), top, PL1.hub_below_mono(HI.card_ids(cs), nc, top, Nat.max_ge_l(nc, 1n+HS.Card.id(c)), b),
            PL1.hub_lt_ge(HS.Card.id(c), 1n+HS.Card.id(c), top, PL1.hub_lt_succ(HS.Card.id(c)), Nat.max_ge_r(nc, 1n+HS.Card.id(c)))))}

# on the state opened by name (S.St.open), each event's apply computes
def hub_apply_ucards.go(+e: HM.Ev, +s: HS.St, u: HI.UniqueCards(s), k: HX.FreshEv(s, e)) -> HI.UniqueCards(HM.apply(HS.St.open(s), e)):
  match e:
    case HM.ECardOpened{c}:
      hub_ucards_open(HS.St.cards(s), c, HS.St.next_card(s), k, u)
    case HM.ECardClosed{id, res, a}:
      hub_ucards_drop(HS.St.cards(s), id, HS.St.next_card(s), u)
    case HM.ECardAnswerPos{a, b, c}:
      u
    case HM.ECardWithdrawn{a, b}:
      u
    case HM.ECardLate{a}:
      u
    case HM.ETaskCreated{a, b, c, d, f}:
      u
    case HM.ELifecycle{a, b, c}:
      u
    case HM.EWorkspace{a, b}:
      u
    case HM.ESnapshot{a, b}:
      u
    case HM.EPlaced{a, b, c}:
      u
    case HM.EDeclared{a, b, c}:
      u
    case HM.EReported{a, b}:
      u
    case HM.EFileTouched{a, b}:
      u
    case HM.EClaimDropped{a, b}:
      u
    case HM.ERenamed{a, b}:
      u
    case HM.EMsgsMoved{a, b}:
      u
    case HM.EMsgSent{m}:
      u
    case HM.EMsgState{a, b}:
      u
    case HM.ESettled{a}:
      u
    case HM.EMainNote{a, b}:
      u
    case HM.ENotesFlushed{}:
      u
    case HM.ETimerSet{a, b}:
      u
    case HM.ETimerFired{a, b, c, d, f}:
      u
    case HM.ETimerRun{a, b}:
      u
    case HM.ETimerStop{a, b, c}:
      u
    case HM.ETimerName{a, b, c, d}:
      u
    case HM.EX{a}:
      u
    case HM.EAct{a}:
      u
    case HM.EFollow{a, b, c}:
      u
    case HM.EJobEnd{a, b}:
      u
    case HM.EWake{a}:
      u
    case HM.EPaused{a}:
      u
    case HM.EResumed{a}:
      u

def hub_apply_ucards(+s: HS.St, e: HM.Ev, u: HI.UniqueCards(s), k: HX.FreshEv(s, e)) -> HI.UniqueCards(HM.apply(s, e)):
  %HSL.St.open_eq(s) : HI.UniqueCards(HM.apply(_, e))
  hub_apply_ucards.go(e, s, u, k)

def hub_apply_all_ucards(+evs: List<&2, HM.Ev>, +s: HS.St, u: HI.UniqueCards(s), f: HX.Fresh(evs, s)) -> HI.UniqueCards(HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      u
    case Con{e, t}:
      match f:
        case HX.Both{k, ft}:
          hub_apply_all_ucards(t, HM.apply(s, e), hub_apply_ucards(s, e, u, k), ft)

def hub_ucards_tr(-x: HS.St, -y: HS.St, e: {x == y : HS.St}, u: HI.UniqueCards(y)) -> HI.UniqueCards(x):
  %Equal.sym(HS.St, x, y, e) : HI.UniqueCards(_)
  u

# ---- L4 support: string equality is reflexive ----

def hub_bool_cmp_refl(b: Bool) -> {Bool.cmp(b, b) == EQ{} : Cmp}:
  match b:
    case True{}:
      {==}
    case False{}:
      {==}

def hub_word_cmp_refl(+n: Nat, +w: Word(n)) -> {Word.cmp(n, w, w) == EQ{} : Cmp}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      match w:
        case WCon{b, t}:
          %Equal.sym(Cmp, Word.cmp(p, t, t), EQ{}, hub_word_cmp_refl(p, t)) : {Word.cmp.fin(b, b, _) == EQ{} : Cmp}
          hub_bool_cmp_refl(b)

# String.order (bend 2.0.38+) finds every string equal to itself
def hub_sord_refl(+s: String) -> {String.order.go(s, s, EQ{}) == EQ{} : Cmp}:
  match s:
    case SNil{}:
      {==}
    case SCon{Chr{x}, t}:
      match x:
        case U32{w}:
          %Equal.sym(Cmp, Word.cmp(32n, w, w), EQ{}, hub_word_cmp_refl(32n, w)) : {String.order.go(t, t, _) == EQ{} : Cmp}
          hub_sord_refl(t)

# every string equals itself
def hub_str_eq_refl(+s: String) -> {String.eq(s, s) == True{} : Bool}:
  %Equal.sym(Cmp, String.order.go(s, s, EQ{}), EQ{}, hub_sord_refl(s)) : {Cmp.is_eq(_) == True{} : Bool}
  {==}

def hub_and_false_r(a: Bool) -> {Bool.and(a, False{}) == False{} : Bool}:
  match a:
    case True{}:
      {==}
    case False{}:
      {==}

# ---- L4 support: string equality is sound (equal strings are the same) ----

def hub_cdisc(c: Cmp) -> Type:
  match c:
    case EQ{}:
      Unit
    case _:
      Empty

def hub_lt_eq(e: {LT{} == EQ{} : Cmp}) -> Empty:
  %Equal.sym(Cmp, LT{}, EQ{}, e) : hub_cdisc(_)
  Unit{}

def hub_gt_eq(e: {GT{} == EQ{} : Cmp}) -> Empty:
  %Equal.sym(Cmp, GT{}, EQ{}, e) : hub_cdisc(_)
  Unit{}

def hub_bool_sound(a: Bool, b: Bool, e: {Bool.cmp(a, b) == EQ{} : Cmp}) -> {a == b : Bool}:
  match a b:
    case False{} False{}:
      {==}
    case True{} True{}:
      {==}
    case False{} True{}:
      PL1.hub_absurd_eq(hub_lt_eq(e), Bool, False{}, True{})
    case True{} False{}:
      PL1.hub_absurd_eq(hub_gt_eq(e), Bool, True{}, False{})

def hub_word_sound.fin(a: Bool, b: Bool, t: Cmp, +et: {t == EQ{} : Cmp}, e: {Word.cmp.fin(a, b, t) == EQ{} : Cmp}) -> {a == b : Bool}:
  match t:
    case LT{}:
      PL1.hub_absurd_eq(hub_lt_eq(et), Bool, a, b)
    case GT{}:
      PL1.hub_absurd_eq(hub_gt_eq(et), Bool, a, b)
    case EQ{}:
      hub_bool_sound(a, b, e)

def hub_word_tail_eq(a: Bool, b: Bool, t: Cmp, e: {Word.cmp.fin(a, b, t) == EQ{} : Cmp}) -> {t == EQ{} : Cmp}:
  match t:
    case LT{}:
      e
    case GT{}:
      e
    case EQ{}:
      {==}

# equal words are the same word
def hub_word_sound(+n: Nat, w1: Word(n), w2: Word(n), +e: {Word.cmp(n, w1, w2) == EQ{} : Cmp}) -> {w1 == w2 : Word(n)}:
  match n:
    case 0n:
      match w1 w2:
        case WNil{} WNil{}:
          {==}
    case 1n+p:
      match w1 w2:
        case WCon{+a, +at} WCon{+b, +bt}:
          +et = hub_word_tail_eq(a, b, Word.cmp(p, at, bt), e)
          +ht = hub_word_sound(p, at, bt, et)
          +hb = hub_word_sound.fin(a, b, Word.cmp(p, at, bt), et, e)
          %hb : {WCon{a, at} == WCon{_, bt} : Word(1n+p)}
          %ht : {WCon{a, at} == WCon{a, _} : Word(1n+p)}
          {==}

def hub_ccomb(c: Cmp, r: Cmp) -> Cmp:
  match c:
    case EQ{}:
      r
    case LT{}:
      LT{}
    case GT{}:
      GT{}

def hub_ccmp(a: Char, b: Char) -> Cmp:
  match a b:
    case Chr{x} Chr{y}:
      U32.cmp(x, y)

# the comparison of String.order, by structure
def hub_cmpc(a: String, b: String) -> Cmp:
  match a b:
    case SNil{} SNil{}:
      EQ{}
    case SNil{} SCon{h, t}:
      LT{}
    case SCon{h, t} SNil{}:
      GT{}
    case SCon{h1, t1} SCon{h2, t2}:
      hub_ccomb(hub_ccmp(h1, h2), hub_cmpc(t1, t2))

def hub_cmpc_tail(c: Cmp, -r: Cmp, e: {Cmp.is_eq(hub_ccomb(c, r)) == True{} : Bool}) -> {Cmp.is_eq(r) == True{} : Bool}:
  match c:
    case EQ{}:
      e
    case LT{}:
      PL1.hub_absurd_eq(PL1.hub_ft_absurd(e), Bool, Cmp.is_eq(r), True{})
    case GT{}:
      PL1.hub_absurd_eq(PL1.hub_ft_absurd(e), Bool, Cmp.is_eq(r), True{})

def hub_cmpc_head(c: Cmp, -r: Cmp, e: {Cmp.is_eq(hub_ccomb(c, r)) == True{} : Bool}) -> {c == EQ{} : Cmp}:
  match c:
    case EQ{}:
      {==}
    case LT{}:
      PL1.hub_absurd_eq(PL1.hub_ft_absurd(e), Cmp, LT{}, EQ{})
    case GT{}:
      PL1.hub_absurd_eq(PL1.hub_ft_absurd(e), Cmp, GT{}, EQ{})

def hub_u32_sound(x: U32, y: U32, e: {U32.cmp(x, y) == EQ{} : Cmp}) -> {Chr{x} == Chr{y} : Char}:
  match x y:
    case U32{w1} U32{w2}:
      %hub_word_sound(32n, w1, w2, e) : {Chr{U32{w1}} == Chr{U32{_}} : Char}
      {==}

# a comparison that finds the strings equal means they are the same
def hub_cmpc_sound(+a: String, +b: String, +e: {Cmp.is_eq(hub_cmpc(a, b)) == True{} : Bool}) -> {a == b : String}:
  match a b:
    case SNil{} SNil{}:
      {==}
    case SNil{} SCon{h, t}:
      PL1.hub_absurd_eq(PL1.hub_ft_absurd(e), String, SNil{}, SCon{h, t})
    case SCon{h, t} SNil{}:
      PL1.hub_absurd_eq(PL1.hub_ft_absurd(e), String, SCon{h, t}, SNil{})
    case SCon{Chr{x}, t1} SCon{Chr{y}, t2}:
      +ht = hub_cmpc_sound(t1, t2, hub_cmpc_tail(U32.cmp(x, y), hub_cmpc(t1, t2), e))
      +hh = hub_u32_sound(x, y, hub_cmpc_head(U32.cmp(x, y), hub_cmpc(t1, t2), e))
      %hh : {SCon{Chr{x}, t1} == SCon{_, t2} : String}
      %ht : {SCon{Chr{x}, t1} == SCon{Chr{x}, _} : String}
      {==}

# String.order.go after a head comparison c is that c, then hub_cmpc
def hub_sord_cmpc(+a: String, +b: String, +c: Cmp) -> {String.order.go(a, b, c) == hub_ccomb(c, hub_cmpc(a, b)) : Cmp}:
  match a b:
    case SNil{} SNil{}:
      match c:
        case EQ{}:
          {==}
        case LT{}:
          {==}
        case GT{}:
          {==}
    case SNil{} SCon{h, t}:
      match c:
        case EQ{}:
          {==}
        case LT{}:
          {==}
        case GT{}:
          {==}
    case SCon{h, t} SNil{}:
      match c:
        case EQ{}:
          {==}
        case LT{}:
          {==}
        case GT{}:
          {==}
    case SCon{Chr{x}, t1} SCon{Chr{y}, t2}:
      match c:
        case EQ{}:
          hub_sord_cmpc(t1, t2, U32.cmp(x, y))
        case LT{}:
          {==}
        case GT{}:
          {==}

def hub_str_eq_sound.k(+a: String, +b: String, e: {String.eq(a, b) == True{} : Bool}) -> {Cmp.is_eq(hub_cmpc(a, b)) == True{} : Bool}:
  %hub_sord_cmpc(a, b, EQ{}) : {Cmp.is_eq(_) == True{} : Bool}
  e

# String.eq is sound: strings it finds equal are the same string
def hub_str_eq_sound(+a: String, +b: String, e: {String.eq(a, b) == True{} : Bool}) -> {a == b : String}:
  hub_cmpc_sound(a, b, hub_str_eq_sound.k(a, b, e))

# ---- L4 support: a delivery only shrinks queues ----

# ---- L4: the frame of an agent (what makes it stuck) ----

# ---- L4 support: a delivery only empties queues ----

# ---- L4 support: queue implications ----

# ---- L4 support: the frame of a delivery to another agent ----

# ---- L4: no agent stuck, except the listed ones ----

# ---- L4 support: names apart — the operations that keep names ----

def hub_is_rename(op: HM.AgentOp) -> Bool:
  match op:
    case HM.ORename{x}:
      True{}
    case _:
      False{}

def hub_names_op(+h: HS.Agent, op: HM.AgentOp, nr: {hub_is_rename(op) == False{} : Bool}) -> {HI.names(HM.agent_op(h, op)) == HI.names(h) : List<&2, String>}:
  match op:
    case HM.OLife{l, why}:
      %HSL.Agent.life.eta(h, l) : {HI.names(HS.Agent.with_failure(HS.Agent.with_declared(_, HM.agent_op.decl(l, HS.Agent.declared(h))), why)) == HI.names(h) : List<&2, String>}
      {==}
    case HM.OWs{w}:
      %HSL.Agent.ws.eta(h, w) : {HI.names(_) == HI.names(h) : List<&2, String>}
      {==}
    case HM.ODeclared{d, n}:
      %HSL.Agent.declared.eta(h, d) : {HI.names(HS.Agent.with_note(_, n)) == HI.names(h) : List<&2, String>}
      {==}
    case HM.OFile{+p}:
      %HSL.Agent.files.eta(h, HM.agent_op.file(HM.str_in(HS.Agent.files(h), p), HS.Agent.files(h), p)) : {HI.names(_) == HI.names(h) : List<&2, String>}
      {==}
    case HM.OUnclaim{+p}:
      %HSL.Agent.files.eta(h, HN.drop_str(HS.Agent.files(h), p)) : {HI.names(_) == HI.names(h) : List<&2, String>}
      {==}
    case HM.ORename{nn}:
      PL1.hub_absurd_eq(PL1.hub_tf_absurd(nr), List<&2, String>, HI.names(HM.agent_op(h, HM.ORename{nn})), HI.names(h))
    case HM.OSnap{x}:
      %HSL.Agent.snap.eta(h, x) : {HI.names(_) == HI.names(h) : List<&2, String>}
      {==}
    case HM.OReport{x}:
      %HSL.Agent.report.eta(h, x) : {HI.names(_) == HI.names(h) : List<&2, String>}
      {==}
    case HM.OFollow{x}:
      %HSL.Agent.follow.eta(h, x) : {HI.names(_) == HI.names(h) : List<&2, String>}
      {==}

def hub_prof_map.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, +e: {String.eq(HS.Agent.name(h), n) == c : Bool},
    +nr: {hub_is_rename(op) == False{} : Bool}, ih: {HI.profile(HM.map_agent(t, n, op)) == HI.profile(t) : List<&2, List<&2, String>>})
    -> {HI.profile(HM.map_agent.go(h <> t, String.eq(HS.Agent.name(h), n), n, op)) == HI.profile(h <> t) : List<&2, List<&2, String>>}:
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(HS.Agent.name(h), n), True{}, e) : {HI.profile(HM.map_agent.go(h <> t, _, n, op)) == HI.profile(h <> t) : List<&2, List<&2, String>>}
      Equal.cong(List<&2, String>, List<&2, List<&2, String>>, x => x <> HI.profile(t), HI.names(HM.agent_op(h, op)), HI.names(h), hub_names_op(h, op, nr))
    case False{}:
      %Equal.sym(Bool, String.eq(HS.Agent.name(h), n), False{}, e) : {HI.profile(HM.map_agent.go(h <> t, _, n, op)) == HI.profile(h <> t) : List<&2, List<&2, String>>}
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, x => HI.names(h) <> x, HI.profile(HM.map_agent(t, n, op)), HI.profile(t), ih)

# an operation that is not a rename keeps every agent's names
def hub_prof_map(+xs: List<&2, HS.Agent>, +n: String, +op: HM.AgentOp, +nr: {hub_is_rename(op) == False{} : Bool})
    -> {HI.profile(HM.map_agent(xs, n, op)) == HI.profile(xs) : List<&2, List<&2, String>>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_prof_map.pick(String.eq(HS.Agent.name(h), n), h, t, n, op, {==}, nr, hub_prof_map(t, n, op, nr))

# ---- L4 support: names apart — a free name ----

def hub_str_eq_sym_false.aa(+a: String, -b: String, s: {b == a : String}, e: {String.eq(a, b) == False{} : Bool}) -> {String.eq(a, a) == False{} : Bool}:
  %s : {String.eq(a, _) == False{} : Bool}
  e

def hub_str_eq_sym_false.bad(+a: String, -b: String, s: {b == a : String}, e: {String.eq(a, b) == False{} : Bool}) -> {True{} == False{} : Bool}:
  Equal.trans(Bool, True{}, String.eq(a, a), False{}, Equal.sym(Bool, String.eq(a, a), True{}, hub_str_eq_refl(a)), hub_str_eq_sym_false.aa(a, b, s, e))

def hub_str_eq_sym_false.pick(c: Bool, +a: String, +b: String, +ec: {String.eq(b, a) == c : Bool}, e: {String.eq(a, b) == False{} : Bool})
    -> {String.eq(b, a) == False{} : Bool}:
  match c:
    case False{}:
      ec
    case True{}:
      PL1.hub_absurd_eq(PL1.hub_tf_absurd(hub_str_eq_sym_false.bad(a, b, hub_str_eq_sound(b, a, ec), e)), Bool, String.eq(b, a), False{})

def hub_str_eq_sym_false(+a: String, +b: String, e: {String.eq(a, b) == False{} : Bool}) -> {String.eq(b, a) == False{} : Bool}:
  hub_str_eq_sym_false.pick(String.eq(b, a), a, b, {==}, e)

# n is none of the names of the listed agents
def hub_free(ps: List<&2, List<&2, String>>, +n: String) -> Data:
  match ps:
    case Nil{}:
      Unit
    case Con{+p, +r}:
      HX.Both<{HM.str_in(p, n) == False{} : Bool}, hub_free(r, n)>

def hub_mdisc(m: Maybe<&2, HS.Agent>) -> Type:
  match m:
    case None{}:
      Unit
    case Some{x}:
      Empty

def hub_some_none(-a: HS.Agent, e: {Some{a} == None{} : Maybe<&2, HS.Agent>}) -> Empty:
  %Equal.sym(Maybe<&2, HS.Agent>, Some{a}, None{}, e) : hub_mdisc(_)
  Unit{}

# name-free: a name no agent has is free among the agents' names once
# the take strips it from the old names (lib/hub-names.bend nm_free)

# ---- L4 support: names apart — a new agent ----

def hub_allapart(ps: List<&2, List<&2, String>>, +q: List<&2, String>) -> Data:
  match ps:
    case Nil{}:
      Unit
    case Con{+p, +r}:
      HX.Both<HI.Apart(p, q), hub_allapart(r, q)>

def hub_aa_snoc(+p: List<&2, String>, +r: List<&2, List<&2, String>>, +q: List<&2, String>, a: HI.ApartAll(p, r), b: HI.Apart(p, q)) -> HI.ApartAll(p, HM.snoc(List<&2, String>, r, q)):
  match r:
    case Nil{}:
      HX.Both{b, Unit{}}
    case Con{x, t}:
      match a:
        case HX.Both{a1, a2}:
          HX.Both{a1, hub_aa_snoc(p, t, q, a2, b)}

def hub_nok_snoc(+ps: List<&2, List<&2, String>>, +q: List<&2, String>, nok: HI.NamesOK(ps), aa: hub_allapart(ps, q)) -> HI.NamesOK(HM.snoc(List<&2, String>, ps, q)):
  match ps:
    case Nil{}:
      HX.Both{Unit{}, Unit{}}
    case Con{p, r}:
      match nok aa:
        case HX.Both{n1, n2} HX.Both{a1, a2}:
          HX.Both{hub_aa_snoc(p, r, q, n1, a1), hub_nok_snoc(r, q, n2, a2)}

def hub_apart_new(+p: List<&2, String>, +n: String, +f: {HM.str_in(p, n) == False{} : Bool}) -> HI.Apart(p, [n]):
  match p:
    case Nil{}:
      Unit{}
    case Con{m, al}:
      HX.Both{PL1.hub_lor_false(String.eq(n, m), False{}, hub_str_eq_sym_false(m, n, PL1.hub_lor_false_l(String.eq(m, n), HM.str_in(al, n), f)), {==}),
        PL1.hub_lor_false_r(String.eq(m, n), HM.str_in(al, n), f)}

def hub_allapart_new(+ps: List<&2, List<&2, String>>, +n: String, f: hub_free(ps, n)) -> hub_allapart(ps, [n]):
  match ps:
    case Nil{}:
      Unit{}
    case Con{p, r}:
      match f:
        case HX.Both{f1, f2}:
          HX.Both{hub_apart_new(p, n, f1), hub_allapart_new(r, n, f2)}

def hub_put_absent.pick(c: Bool, +h: HS.Agent, +t: List<&2, HS.Agent>, +n: String, +a: HS.Agent, +e: {String.eq(HS.Agent.name(h), n) == c : Bool},
    ec: {c == False{} : Bool}, ih: {HI.profile(HM.put_agent(t, n, a)) == HM.snoc(List<&2, String>, HI.profile(t), HI.names(a)) : List<&2, List<&2, String>>})
    -> {HI.profile(HLZ.pick(List<&2, HS.Agent>, String.eq(HS.Agent.name(h), n), u => a <> t, u => h <> HM.put_agent(t, n, a))) == HM.snoc(List<&2, String>, HI.profile(h <> t), HI.names(a)) : List<&2, List<&2, String>>}:
  match c:
    case True{}:
      PL1.hub_absurd_eq(PL1.hub_tf_absurd(ec), List<&2, List<&2, String>>, HI.profile(HLZ.pick(List<&2, HS.Agent>, String.eq(HS.Agent.name(h), n), u => a <> t, u => h <> HM.put_agent(t, n, a))), HM.snoc(List<&2, String>, HI.profile(h <> t), HI.names(a)))
    case False{}:
      %Equal.sym(Bool, String.eq(HS.Agent.name(h), n), False{}, e) : {HI.profile(HLZ.pick(List<&2, HS.Agent>, _, u => a <> t, u => h <> HM.put_agent(t, n, a))) == HM.snoc(List<&2, String>, HI.profile(h <> t), HI.names(a)) : List<&2, List<&2, String>>}
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, x => HI.names(h) <> x, HI.profile(HM.put_agent(t, n, a)), HM.snoc(List<&2, String>, HI.profile(t), HI.names(a)), ih)

# a new agent under a free name goes last
def hub_put_absent(+xs: List<&2, HS.Agent>, +n: String, +a: HS.Agent, f: hub_free(HI.profile(xs), n))
    -> {HI.profile(HM.put_agent(xs, n, a)) == HM.snoc(List<&2, String>, HI.profile(xs), HI.names(a)) : List<&2, List<&2, String>>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match f:
        case HX.Both{f1, f2}:
          hub_put_absent.pick(String.eq(HS.Agent.name(h), n), h, t, n, a, {==},
            PL1.hub_lor_false_l(String.eq(HS.Agent.name(h), n), HM.str_in(HS.Agent.aliases(h), n), f1), hub_put_absent(t, n, a, f2))

# ---- L4 support: names apart — a rename ----

def hub_str_in_snoc_false(+l: List<&2, String>, +y: String, +z: String, +a: {HM.str_in(l, z) == False{} : Bool}, b: {String.eq(y, z) == False{} : Bool})
    -> {HM.str_in(HM.snoc(String, l, y), z) == False{} : Bool}:
  match l:
    case Nil{}:
      PL1.hub_lor_false(String.eq(y, z), False{}, b, {==})
    case Con{h, t}:
      PL1.hub_lor_false(String.eq(h, z), HM.str_in(HM.snoc(String, t, y), z), PL1.hub_lor_false_l(String.eq(h, z), HM.str_in(t, z), a),
        hub_str_in_snoc_false(t, y, z, PL1.hub_lor_false_r(String.eq(h, z), HM.str_in(t, z), a), b))

# the renamed entry against one listed after it
def hub_apart_ren_after(+m: String, +al: List<&2, String>, +new: String, +y: List<&2, String>, ay: HI.Apart(m <> al, y), fy: {HM.str_in(y, new) == False{} : Bool})
    -> HI.Apart(new <> HM.snoc(String, al, m), y):
  match y:
    case Nil{}:
      Unit{}
    case Con{yn, ya}:
      match ay:
        case HX.Both{+a1, +a2}:
          HX.Both{fy, hub_str_in_snoc_false(al, m, yn, a2, hub_str_eq_sym_false(yn, m, PL1.hub_lor_false_l(String.eq(yn, m), HM.str_in(ya, m), a1)))}

def hub_aa_ren(+m: String, +al: List<&2, String>, +new: String, +ps: List<&2, List<&2, String>>, a: HI.ApartAll(m <> al, ps), f: hub_free(ps, new))
    -> HI.ApartAll(new <> HM.snoc(String, al, m), ps):
  match ps:
    case Nil{}:
      Unit{}
    case Con{y, r}:
      match a f:
        case HX.Both{a1, a2} HX.Both{f1, f2}:
          HX.Both{hub_apart_ren_after(m, al, new, y, a1, f1), hub_aa_ren(m, al, new, r, a2, f2)}

# an entry listed before the renamed one
def hub_apart_ren_before(+x: List<&2, String>, +m: String, +al: List<&2, String>, +new: String, ax: HI.Apart(x, m <> al), +fx: {HM.str_in(x, new) == False{} : Bool})
    -> HI.Apart(x, new <> HM.snoc(String, al, m)):
  match x:
    case Nil{}:
      Unit{}
    case Con{xn, xa}:
      match ax:
        case HX.Both{+b1, +b2}:
          HX.Both{PL1.hub_lor_false(String.eq(new, xn), HM.str_in(HM.snoc(String, al, m), xn),
              hub_str_eq_sym_false(xn, new, PL1.hub_lor_false_l(String.eq(xn, new), HM.str_in(xa, new), fx)),
              hub_str_in_snoc_false(al, m, xn, PL1.hub_lor_false_r(String.eq(m, xn), HM.str_in(al, xn), b1), PL1.hub_lor_false_l(String.eq(m, xn), HM.str_in(al, xn), b1))),
            PL1.hub_lor_false_r(String.eq(xn, new), HM.str_in(xa, new), fx)}

def hub_aa_map_ren.top(+x: List<&2, String>, +h: HS.Agent, +t: List<&2, HS.Agent>, +new: String, ah: HI.Apart(x, HI.names(h)), +fx: {HM.str_in(x, new) == False{} : Bool},
    at: HI.ApartAll(x, HI.profile(t))) -> HI.ApartAll(x, HI.profile(HM.agent_op(h, HM.ORename{new}) <> t)):
  %HSL.Agent.name.eta(h, new) : HI.ApartAll(x, HI.profile(HS.Agent.with_aliases(_, HM.snoc(String, HS.Agent.aliases(h), HS.Agent.name(h))) <> t))
  HX.Both{hub_apart_ren_before(x, HS.Agent.name(h), HS.Agent.aliases(h), new, ah, fx), at}

def hub_aa_map_ren.pick(c: Bool, +x: List<&2, String>, +h: HS.Agent, +t: List<&2, HS.Agent>, +old: String, +new: String, +e: {String.eq(HS.Agent.name(h), old) == c : Bool},
    ah: HI.Apart(x, HI.names(h)), +fx: {HM.str_in(x, new) == False{} : Bool},
    at: HI.ApartAll(x, HI.profile(t)), ih: HI.ApartAll(x, HI.profile(HM.map_agent(t, old, HM.ORename{new}))))
    -> HI.ApartAll(x, HI.profile(HM.map_agent.go(h <> t, String.eq(HS.Agent.name(h), old), old, HM.ORename{new}))):
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(HS.Agent.name(h), old), True{}, e) : HI.ApartAll(x, HI.profile(HM.map_agent.go(h <> t, _, old, HM.ORename{new})))
      hub_aa_map_ren.top(x, h, t, new, ah, fx, at)
    case False{}:
      %Equal.sym(Bool, String.eq(HS.Agent.name(h), old), False{}, e) : HI.ApartAll(x, HI.profile(HM.map_agent.go(h <> t, _, old, HM.ORename{new})))
      HX.Both{ah, ih}
