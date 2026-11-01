# bend/proofs/inbox.bend: the proofs of bend/laws/inbox.bend. The close's
# and the answer's laws hold by unfolding; answer_pos_once splits on
# whether the card is due, and a card deleted from the due list is no
# longer in it (hub_nat_in_del).
import Base
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/answers.bend as HA
import ../hub/picks.bend as HPK
import ../hub/lazy.bend as HLZ
import ../hub/cardkind.bend as CK
import ../laws/inbox.bend as Laws

def Laws.closed_without_him_has_no_answer(o, cx, id, res):
  {==}

def Laws.answered_close_carries_his_words(c, said, via):
  {==}

def Laws.a_picked_digit_journals_its_words():
  {==}

def Laws.an_approval_digit_journals_its_words():
  {==}

def Laws.answer_pos_not_due(o, cx, card, agent, pos):
  {==}

def Laws.drop_card_words():
  {==}

def Laws.only_his_kinds_are_his_cards():
  {==}

def Laws.archive_ask_is_a_drop_card(o, cx, q, name):
  {==}

def Laws.a_drop_that_asks_archives_nothing(o, cx, a, has_wt, loss, busy):
  {==}

def Laws.drop_option_1_is_yes(o, cx, parts, via):
  {==}

def Laws.drop_option_2_is_no(o, cx, parts, via):
  {==}

def Laws.old_drop_card_digit_keeps(o, cx, parts, via):
  {==}

def Laws.old_drop_card_yes_archives(o, cx, parts, via):
  {==}

# a card deleted from a list is no longer in it (ih: the same for the tail)
def hub_nat_in_del.step(b: Bool, -h: Nat, -t: List<&2, Nat>, -n: Nat, +e: {Nat.is_eq(h, n) == b : Bool}, ih: {HA.nat_in(HA.nat_del(t, n), n) == False{} : Bool})
    -> {HA.nat_in(Bool.pick(List<&2, Nat>, Nat.is_eq(h, n), HA.nat_del(t, n), h <> HA.nat_del(t, n)), n) == False{} : Bool}:
  match b:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(h, n), True{}, e) : {HA.nat_in(Bool.pick(List<&2, Nat>, _, HA.nat_del(t, n), h <> HA.nat_del(t, n)), n) == False{} : Bool}
      ih
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(h, n), False{}, e) : {HA.nat_in(Bool.pick(List<&2, Nat>, _, HA.nat_del(t, n), h <> HA.nat_del(t, n)), n) == False{} : Bool}
      %Equal.sym(Bool, Nat.is_eq(h, n), False{}, e) : {HLZ.or(_, u => HA.nat_in(HA.nat_del(t, n), n)) == False{} : Bool}
      ih

def hub_nat_in_del(xs: List<&2, Nat>, +n: Nat) -> {HA.nat_in(HA.nat_del(xs, n), n) == False{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_nat_in_del.step(Nat.is_eq(h, n), h, t, n, {==}, hub_nat_in_del(t, n))

def hub_pos_once.go(-o: HS.St, +c: HX.Core<o>, +a: List<&2, HX.Rt>, +b: List<&2, HX.Waiter>, +s: List<&2, HX.Steer>,
    +d: List<&2, HX.Cnt>, +e: List<&2, HX.Kv>, +f: List<&2, HX.Kn>, +g: String, +h: Bool, +i: Nat, +j: Bool,
    +k: List<&2, J.Json>, +l: Maybe<&2, J.Json>, +m: Maybe<&2, HPK.Answer>, +du: List<&2, Nat>, +vc: List<&2, HS.Vctx>,
    +card: Nat, +agent: String, +pos: Nat, +agent2: String, +pos2: Nat, q: Bool, +eq: {HA.nat_in(du, card) == q : Bool})
    -> {HA.answer_pos(o, HA.answer_pos(o, HX.Cx{c, HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}}, card, agent, pos), card, agent2, pos2)
        == HA.answer_pos(o, HX.Cx{c, HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}}, card, agent, pos) : HX.Cx<o>}:
  match q:
    case False{}:
      %Equal.sym(Bool, HA.nat_in(du, card), False{}, eq) : {HA.answer_pos(o, HA.answer_pos.go(o, _, HX.Cx{c, HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}}, card, agent, pos), card, agent2, pos2) == HA.answer_pos.go(o, _, HX.Cx{c, HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}}, card, agent, pos) : HX.Cx<o>}
      %Equal.sym(Bool, HA.nat_in(du, card), False{}, eq) : {HA.answer_pos.go(o, _, HX.Cx{c, HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}}, card, agent2, pos2) == HX.Cx{c, HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}} : HX.Cx<o>}
      {==}
    case True{}:
      %Equal.sym(Bool, HA.nat_in(du, card), True{}, eq) : {HA.answer_pos(o, HA.answer_pos.go(o, _, HX.Cx{c, HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}}, card, agent, pos), card, agent2, pos2) == HA.answer_pos.go(o, _, HX.Cx{c, HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}}, card, agent, pos) : HX.Cx<o>}
      %Equal.sym(Bool, HA.nat_in(HA.nat_del(du, card), card), False{}, hub_nat_in_del(du, card)) : {HA.answer_pos.go(o, _, HA.answer_pos.go(o, True{}, HX.Cx{c, HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}}, card, agent, pos), card, agent2, pos2) == HA.answer_pos.go(o, True{}, HX.Cx{c, HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}}, card, agent, pos) : HX.Cx<o>}
      {==}

def Laws.answer_pos_once(o, cx, card, agent, pos, agent2, pos2):
  match cx:
    case HX.Cx{c, r}:
      match r:
        case HX.R{a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc}:
          hub_pos_once.go(o, c, a, b, s, d, e, f, g, h, i, j, k, l, m, du, vc, card, agent, pos, agent2, pos2, HA.nat_in(du, card), {==})
