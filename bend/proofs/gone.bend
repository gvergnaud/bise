# bend/proofs/gone.bend: the proofs of bend/laws/gone.bend. The bounds
# hold because add and drop end with keep (a take of MAX: take_len);
# a card added is first; a card deleted is in no prefix of the rest.
import Base
import ../hub/gone.bend as G
import ../hub/lazy.bend as GLZ
import ../hub/model.bend as HM
import ../laws/gone.bend as Laws
import ../hub/gen/state.bend as HS
import ../hub/gen/state-laws.bend as HSL

# n equals itself
def eq_refl(n: Nat) -> {Nat.is_eq(n, n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      eq_refl(p)

# 0 is at most anything
def zero_le(n: Nat) -> {Nat.is_le(0n, n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      {==}

# a take of n holds n at most
def take_len(xs: List<&2, HS.Item>, n: Nat) -> {Nat.is_le(List.length(&2, HS.Item, List.take(&2, HS.Item, xs, n)), n) == True{} : Bool}:
  match xs n:
    case Nil{} n:
      zero_le(n)
    case Con{h, t} 0n:
      {==}
    case Con{h, t} 1n+p:
      take_len(t, p)

def Laws.gone_add_bound(xs, id, text):
  take_len(G.item(id, text) <> G.del(xs, id), G.MAX())

def Laws.gone_drop_bound(xs, id):
  take_len(G.del(xs, id), G.MAX())

def Laws.gone_add_finds(xs, id, text):
  %Equal.sym(Bool, Nat.is_eq(HS.Item.id(G.item(id, text)), id), True{}, eq_refl(id)) : {GLZ.pick(Maybe<&2, HS.Item>, _, u => Some{G.item(id, text)}, u => G.find(List.take(&2, HS.Item, G.del(xs, id), 63n), id)) == Some{G.item(id, text)} : Maybe<&2, HS.Item>}
  {==}

# del's step: h dropped or kept in front of the rest, id still not found
def del_none.step(b: Bool, -h: HS.Item, -t: List<&2, HS.Item>, -id: Nat, +e: {Nat.is_eq(HS.Item.id(h), id) == b : Bool}, ih: {G.find(G.del(t, id), id) == None{} : Maybe<&2, HS.Item>})
    -> {G.find(Bool.pick(List<&2, HS.Item>, Nat.is_eq(HS.Item.id(h), id), G.del(t, id), h <> G.del(t, id)), id) == None{} : Maybe<&2, HS.Item>}:
  match b:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Item.id(h), id), True{}, e) : {G.find(Bool.pick(List<&2, HS.Item>, _, G.del(t, id), h <> G.del(t, id)), id) == None{} : Maybe<&2, HS.Item>}
      ih
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HS.Item.id(h), id), False{}, e) : {G.find(Bool.pick(List<&2, HS.Item>, _, G.del(t, id), h <> G.del(t, id)), id) == None{} : Maybe<&2, HS.Item>}
      %Equal.sym(Bool, Nat.is_eq(HS.Item.id(h), id), False{}, e) : {GLZ.pick(Maybe<&2, HS.Item>, _, u => Some{h}, u => G.find(G.del(t, id), id)) == None{} : Maybe<&2, HS.Item>}
      ih

# id is not found in the list del leaves
def del_none(xs: List<&2, HS.Item>, +id: Nat) -> {G.find(G.del(xs, id), id) == None{} : Maybe<&2, HS.Item>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      del_none.step(Nat.is_eq(HS.Item.id(h), id), h, t, id, {==}, del_none(t, id))

# Some is not None
def sdisc(m: Maybe<&2, HS.Item>) -> Type:
  match m:
    case Some{x}:
      Unit
    case None{}:
      Empty

def some_not_none(-x: HS.Item, +e: {Some{x} == None{} : Maybe<&2, HS.Item>}) -> Empty:
  %e : sdisc(_)
  Unit{}

def from_empty(-T: Type, x: Empty) -> T:
  match x:

# take's step: h's id is not id (b False) when the whole list has no id
def take_none.step(b: Bool, +h: HS.Item, +t: List<&2, HS.Item>, -p: Nat, +id: Nat, +eb: {Nat.is_eq(HS.Item.id(h), id) == b : Bool},
    +e: {G.find(h <> t, id) == None{} : Maybe<&2, HS.Item>},
    ih: {G.find(t, id) == None{} : Maybe<&2, HS.Item>} -> {G.find(List.take(&2, HS.Item, t, p), id) == None{} : Maybe<&2, HS.Item>})
    -> {G.find(List.take(&2, HS.Item, h <> t, 1n+p), id) == None{} : Maybe<&2, HS.Item>}:
  match b:
    case True{}:
      +e2 = {%eb : {GLZ.pick(Maybe<&2, HS.Item>, _, u => Some{h}, u => G.find(t, id)) == None{} : Maybe<&2, HS.Item>}; e : {GLZ.pick(Maybe<&2, HS.Item>, True{}, u => Some{h}, u => G.find(t, id)) == None{} : Maybe<&2, HS.Item>}}
      from_empty({G.find(List.take(&2, HS.Item, h <> t, 1n+p), id) == None{} : Maybe<&2, HS.Item>}, some_not_none(h, e2))
    case False{}:
      +e2 = {%eb : {GLZ.pick(Maybe<&2, HS.Item>, _, u => Some{h}, u => G.find(t, id)) == None{} : Maybe<&2, HS.Item>}; e : {GLZ.pick(Maybe<&2, HS.Item>, False{}, u => Some{h}, u => G.find(t, id)) == None{} : Maybe<&2, HS.Item>}}
      %Equal.sym(Bool, Nat.is_eq(HS.Item.id(h), id), False{}, eb) : {GLZ.pick(Maybe<&2, HS.Item>, _, u => Some{h}, u => G.find(List.take(&2, HS.Item, t, p), id)) == None{} : Maybe<&2, HS.Item>}
      ih(e2)

# no prefix of a list holds an id the list does not
def take_none(xs: List<&2, HS.Item>, n: Nat, +id: Nat) -> {G.find(xs, id) == None{} : Maybe<&2, HS.Item>} -> {G.find(List.take(&2, HS.Item, xs, n), id) == None{} : Maybe<&2, HS.Item>}:
  match xs n:
    case Nil{} n:
      e => {==}
    case Con{+h, +t} 0n:
      e => {==}
    case Con{+h, +t} 1n+p:
      e => take_none.step(Nat.is_eq(HS.Item.id(h), id), h, t, p, id, {==}, e, take_none(t, p, id))

def Laws.gone_drop_once(xs, id):
  take_none(G.del(xs, id), G.MAX(), id)(del_none(xs, id))

def Laws.gone_withdrawn_adds(s, id, text):
  %HSL.St.open_eq(s) : {HS.St.gone(HM.apply(_, HM.ECardWithdrawn{id, text})) == G.add(HS.St.gone(s), id, text) : List<&2, HS.Item>}
  {==}

def Laws.gone_late_drops(s, id):
  %HSL.St.open_eq(s) : {HS.St.gone(HM.apply(_, HM.ECardLate{id})) == G.drop(HS.St.gone(s), id) : List<&2, HS.Item>}
  {==}

def Laws.gone_replay_withdrawn(s, id, text):
  %HSL.St.open_eq(s) : {G.find(HS.St.gone(HM.apply_all([HM.ECardWithdrawn{id, text}], _)), id) == Some{G.item(id, text)} : Maybe<&2, HS.Item>}
  Laws.gone_add_finds(HS.St.gone(s), id, text)

def Laws.gone_replay_late(s, id, text):
  %HSL.St.open_eq(s) : {G.find(HS.St.gone(HM.apply_all([HM.ECardWithdrawn{id, text}, HM.ECardLate{id}], _)), id) == None{} : Maybe<&2, HS.Item>}
  Laws.gone_drop_once(G.add(HS.St.gone(s), id, text), id)

# on the state opened by name (S.St.open), each event's apply computes
def gone_bound.go(+e: HM.Ev, +s: HS.St, h: {Nat.is_le(List.length(&2, HS.Item, HS.St.gone(s)), G.MAX()) == True{} : Bool})
    -> {Nat.is_le(List.length(&2, HS.Item, HS.St.gone(HM.apply(HS.St.open(s), e))), G.MAX()) == True{} : Bool}:
  match e:
        case HM.ETaskCreated{a, b, c, d, f}:
          h
        case HM.ELifecycle{a, b, c}:
          h
        case HM.EWorkspace{a, b}:
          h
        case HM.ESnapshot{a, b}:
          h
        case HM.EPlaced{a, b, c}:
          h
        case HM.EDropped{a, b, c}:
          h
        case HM.EDeclared{a, b, c}:
          h
        case HM.EReported{a, b}:
          h
        case HM.EFileTouched{a, b}:
          h
        case HM.EClaimDropped{a, b}:
          h
        case HM.ERenamed{a, b}:
          h
        case HM.EMsgsMoved{a, b}:
          h
        case HM.EMsgSent{a}:
          h
        case HM.EMsgState{a, b}:
          h
        case HM.ESettled{a}:
          h
        case HM.ECardOpened{a}:
          h
        case HM.ECardClosed{a, b, c}:
          h
        case HM.ECardAnswerPos{a, b, c}:
          h
        case HM.EMainNote{a, b}:
          h
        case HM.ENotesFlushed{}:
          h
        case HM.ETimerSet{a, b}:
          h
        case HM.ETimerFired{a, b, c, d, f}:
          h
        case HM.ETimerRun{a, b}:
          h
        case HM.ETimerStop{a, b, c}:
          h
        case HM.ETimerName{a, b, c, d}:
          h
        case HM.EX{a}:
          h
        case HM.EAct{a}:
          h
        case HM.EFollow{a, b, c}:
          h
        case HM.EJobEnd{a, b}:
          h
        case HM.EWake{a}:
          h
        case HM.EPaused{a}:
          h
        case HM.EResumed{a}:
          h
        case HM.ECardWithdrawn{id, text}:
          Laws.gone_add_bound(HS.St.gone(s), id, text)
        case HM.ECardLate{id}:
          Laws.gone_drop_bound(HS.St.gone(s), id)

def Laws.gone_bound_after_apply(s, e, h):
  %HSL.St.open_eq(s) : {Nat.is_le(List.length(&2, HS.Item, HS.St.gone(HM.apply(_, e))), G.MAX()) == True{} : Bool}
  gone_bound.go(e, s, h)

def Laws.gone_init_empty(workspace):
  {==}

# withdrawn-answer: the late answer's laws hold by unfolding
def Laws.answer_closed_card_is_late(o, cx, id, text, parts, via):
  {==}

def Laws.late_answer_refused(o, cx, id, text, parts):
  {==}

def Laws.late_answer_sent(o, cx, it, id, text, parts):
  {==}

def Laws.late_answer_once(o, cx, m, id):
  {==}

def Laws.late_answer_keeps_its_parts(it, text, parts):
  {==}

def Laws.late_answer_to_main(it, text, parts):
  {==}

def Laws.late_answer_keeps_his_words(it, text, parts):
  {==}

