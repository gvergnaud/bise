# bend/proofs/lib/hub-views.bend: the lemmas of the MCP App views' laws
# (bend/laws/hub.bend, "MCP App views"): a card closed leaves the others,
# two cards closed in either order leave the same cards.
import Base
import ../../hub/model.bend as HM
import ../../hub/gen/state.bend as HS
import ../../hub/lazy.bend as HLZ
import ./hub-cold.bend as PLC

# card_close_keeps_other's step: h's id is a or not
def hv_keep.go(ca: Bool, +h: HS.Card, +t: List<&2, HS.Card>, +a: Nat, +b: Nat,
    +e: {Nat.is_eq(a, b) == False{} : Bool},
    +ea: {Nat.is_eq(HS.Card.id(h), a) == ca : Bool},
    +ih: {HM.card_of(HM.drop_card(t, a), b) == HM.card_of(t, b) : Maybe<&2, HS.Card>})
    -> {HM.card_of(HM.drop_card(h <> t, a), b) == HM.card_of(h <> t, b) : Maybe<&2, HS.Card>}:
  match ca:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), a), True{}, ea) : {HM.card_of(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, a)), b) == HM.card_of(h <> t, b) : Maybe<&2, HS.Card>}
      %Equal.sym(Nat, HS.Card.id(h), a, PLC.hc_eq_nat(HS.Card.id(h), a, ea)) : {HM.card_of(t, b) == HLZ.pick(Maybe<&2, HS.Card>, Nat.is_eq(_, b), u => Some{h}, u => HM.card_of(t, b)) : Maybe<&2, HS.Card>}
      %Equal.sym(Bool, Nat.is_eq(a, b), False{}, e) : {HM.card_of(t, b) == HLZ.pick(Maybe<&2, HS.Card>, _, u => Some{h}, u => HM.card_of(t, b)) : Maybe<&2, HS.Card>}
      {==}
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), a), False{}, ea) : {HM.card_of(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, a)), b) == HM.card_of(h <> t, b) : Maybe<&2, HS.Card>}
      Equal.cong(Maybe<&2, HS.Card>, Maybe<&2, HS.Card>, y => HLZ.pick(Maybe<&2, HS.Card>, Nat.is_eq(HS.Card.id(h), b), u => Some{h}, u => y),
        HM.card_of(HM.drop_card(t, a), b), HM.card_of(t, b), ih)

def hv_keep(+xs: List<&2, HS.Card>, +a: Nat, +b: Nat, +e: {Nat.is_eq(a, b) == False{} : Bool})
    -> {HM.card_of(HM.drop_card(xs, a), b) == HM.card_of(xs, b) : Maybe<&2, HS.Card>}:
  match xs:
    case Nil{}:
      {==}
    case Con{+h, +t}:
      hv_keep.go(Nat.is_eq(HS.Card.id(h), a), h, t, a, b, e, {==}, hv_keep(t, a, b, e))

# card_close_commutes' step: h's id is a, b, both or neither
def hv_comm.go(ca: Bool, cb: Bool, +h: HS.Card, +t: List<&2, HS.Card>, +a: Nat, +b: Nat,
    +ea: {Nat.is_eq(HS.Card.id(h), a) == ca : Bool},
    +eb: {Nat.is_eq(HS.Card.id(h), b) == cb : Bool},
    +ih: {HM.drop_card(HM.drop_card(t, a), b) == HM.drop_card(HM.drop_card(t, b), a) : List<&2, HS.Card>})
    -> {HM.drop_card(HM.drop_card(h <> t, a), b) == HM.drop_card(HM.drop_card(h <> t, b), a) : List<&2, HS.Card>}:
  match ca cb:
    case True{} True{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), a), True{}, ea) : {HM.drop_card(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, a)), b) == HM.drop_card(HLZ.pick(List<&2, HS.Card>, Nat.is_eq(HS.Card.id(h), b), u => t, u => h <> HM.drop_card(t, b)), a) : List<&2, HS.Card>}
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), b), True{}, eb) : {HM.drop_card(t, b) == HM.drop_card(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, b)), a) : List<&2, HS.Card>}
      %Equal.trans(Nat, b, HS.Card.id(h), a, Equal.sym(Nat, HS.Card.id(h), b, PLC.hc_eq_nat(HS.Card.id(h), b, eb)), PLC.hc_eq_nat(HS.Card.id(h), a, ea)) : {HM.drop_card(t, b) == HM.drop_card(t, _) : List<&2, HS.Card>}
      {==}
    case True{} False{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), a), True{}, ea) : {HM.drop_card(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, a)), b) == HM.drop_card(HLZ.pick(List<&2, HS.Card>, Nat.is_eq(HS.Card.id(h), b), u => t, u => h <> HM.drop_card(t, b)), a) : List<&2, HS.Card>}
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), b), False{}, eb) : {HM.drop_card(t, b) == HM.drop_card(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, b)), a) : List<&2, HS.Card>}
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), a), True{}, ea) : {HM.drop_card(t, b) == HLZ.pick(List<&2, HS.Card>, _, u => HM.drop_card(t, b), u => h <> HM.drop_card(HM.drop_card(t, b), a)) : List<&2, HS.Card>}
      {==}
    case False{} True{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), a), False{}, ea) : {HM.drop_card(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, a)), b) == HM.drop_card(HLZ.pick(List<&2, HS.Card>, Nat.is_eq(HS.Card.id(h), b), u => t, u => h <> HM.drop_card(t, b)), a) : List<&2, HS.Card>}
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), b), True{}, eb) : {HLZ.pick(List<&2, HS.Card>, _, u => HM.drop_card(t, a), u => h <> HM.drop_card(HM.drop_card(t, a), b)) == HM.drop_card(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, b)), a) : List<&2, HS.Card>}
      {==}
    case False{} False{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), a), False{}, ea) : {HM.drop_card(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, a)), b) == HM.drop_card(HLZ.pick(List<&2, HS.Card>, Nat.is_eq(HS.Card.id(h), b), u => t, u => h <> HM.drop_card(t, b)), a) : List<&2, HS.Card>}
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), b), False{}, eb) : {HLZ.pick(List<&2, HS.Card>, _, u => HM.drop_card(t, a), u => h <> HM.drop_card(HM.drop_card(t, a), b)) == HM.drop_card(HLZ.pick(List<&2, HS.Card>, _, u => t, u => h <> HM.drop_card(t, b)), a) : List<&2, HS.Card>}
      %Equal.sym(Bool, Nat.is_eq(HS.Card.id(h), a), False{}, ea) : {h <> HM.drop_card(HM.drop_card(t, a), b) == HLZ.pick(List<&2, HS.Card>, _, u => HM.drop_card(t, b), u => h <> HM.drop_card(HM.drop_card(t, b), a)) : List<&2, HS.Card>}
      Equal.cong(List<&2, HS.Card>, List<&2, HS.Card>, y => h <> y, HM.drop_card(HM.drop_card(t, a), b), HM.drop_card(HM.drop_card(t, b), a), ih)

def hv_comm(+xs: List<&2, HS.Card>, +a: Nat, +b: Nat)
    -> {HM.drop_card(HM.drop_card(xs, a), b) == HM.drop_card(HM.drop_card(xs, b), a) : List<&2, HS.Card>}:
  match xs:
    case Nil{}:
      {==}
    case Con{+h, +t}:
      hv_comm.go(Nat.is_eq(HS.Card.id(h), a), Nat.is_eq(HS.Card.id(h), b), h, t, a, b, {==}, {==}, hv_comm(t, a, b))
