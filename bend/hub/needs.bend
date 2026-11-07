# hub/needs.bend: the records a journal event or a replay batch may touch
# (issue 36 chunk 5, plan v2 signed m_35902). A cold record (an old
# message, an archived agent) may leave St for the cold store; a step or a
# batch asks for the ones it needs BEFORE anything applies, so M.apply
# never asks (laws apply_is_local, fetch_then_apply_list). Messages by id
# (ids never change); agents by name, through the renames of the batch.
import Base
import ./model.bend as M
import ./gen/state.bend as S
import ./gen/in.bend as GI
import ./gen/kit.bend as K
import ./lazy.bend as LZ

# the messages (by id) and the agents (by name) a step or a batch needs: a
# pair, not a record (it never enters St: state_guard's rule is for St's)
def Needs() -> Data:
  Sigma<&2, &2, List<&2, Nat>, _ => List<&2, String>>

def Needs.msgs(x: Needs()) -> List<&2, Nat>:
  match x:
    case Tuple{+m, _}:
      m

def Needs.agents(x: Needs()) -> List<&2, String>:
  match x:
    case Tuple{_, +a}:
      a

def none() -> Needs():
  Tuple{Nil{}, Nil{}}

def of_agent(+n: String) -> Needs():
  Tuple{Nil{}, [n]}

def of_msg(+id: Nat) -> Needs():
  Tuple{[id], Nil{}}

# a message event's reply settles the question it answers (settle_in)
def of_reply(r: Maybe<&2, Nat>) -> Needs():
  match r:
    case None{}:
      none()
    case Some{+id}:
      of_msg(id)

def union(a: Needs(), b: Needs()) -> Needs():
  match a b:
    case Tuple{+am, +aa} Tuple{+bm, +ba}:
      Tuple{List.append(&2, Nat, am, bm), List.append(&2, String, aa, ba)}

# the records an event may read or change, besides the hot side and the
# arch name index (names, aliases, dirs: name_taken, resolve, dirs_of and
# the strips of a new task or a rename read only those)
def needs(e: M.Ev) -> Needs():
  match e:
    case M.ETaskCreated{+name, _, _, _, _}:
      of_agent(name)
    case M.ELifecycle{+name, _, _}:
      of_agent(name)
    case M.EWorkspace{+name, _}:
      of_agent(name)
    case M.ESnapshot{+name, _}:
      of_agent(name)
    case M.EPlaced{+name, _, _}:
      of_agent(name)
    case M.EDropped{+name, _, _}:
      of_agent(name)
    case M.EDeclared{+name, _, _}:
      of_agent(name)
    case M.EReported{+name, _}:
      of_agent(name)
    case M.EFileTouched{+name, _}:
      of_agent(name)
    case M.EClaimDropped{+name, _}:
      of_agent(name)
    case M.ERenamed{+name, _}:
      of_agent(name)
    case M.EFollow{+name, _, _}:
      of_agent(name)
    case M.EJobEnd{+name, _}:
      of_agent(name)
    case M.EMsgSent{+m}:
      of_reply(S.Msg.reply_to(m))
    case M.EMsgState{+id, _}:
      of_msg(id)
    case M.ESettled{+id}:
      of_msg(id)
    case _:
      none()

# ---- a batch: its needs at its start, through its renames ----
# `rn`: for each name a rename of the batch gave, the names it had before
# in the batch (newest first), so a later event under the new name needs
# the record stored under an older one

def Rn() -> Data:
  Sigma<&2, &2, String, _ => List<&2, String>>

def was_of(rn: List<&2, Rn()>, +n: String) -> List<&2, String>:
  match rn:
    case Nil{}:
      Nil{}
    case Con{Tuple{+k, +w}, +t}:
      # the walk stops at the first rename to `n` (LZ.pick: Bool.pick computes both)
      LZ.pick(List<&2, String>, String.eq(k, n), u => w, u => was_of(t, n))

# an event's agent needs, with the earlier names of each in the batch
def with_was(xs: List<&2, String>, +rn: List<&2, Rn()>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      List.append(&2, String, n <> was_of(rn, n), with_was(t, rn))

def renames(e: M.Ev, +rn: List<&2, Rn()>) -> List<&2, Rn()>:
  match e:
    case M.ERenamed{+name, +nn}:
      Tuple{nn, name <> was_of(rn, name)} <> rn
    case _:
      rn

def needs_with.go(x: Needs(), +rn: List<&2, Rn()>) -> Needs():
  match x:
    case Tuple{+ms, +ags}:
      Tuple{ms, with_was(ags, rn)}

def needs_with(e: M.Ev, +rn: List<&2, Rn()>) -> Needs():
  needs_with.go(needs(e), rn)

def needs_batch.go(es: List<&2, M.Ev>, +rn: List<&2, Rn()>) -> Needs():
  match es:
    case Nil{}:
      none()
    case Con{+e, +t}:
      union(needs_with(e, rn), needs_batch.go(t, renames(e, rn)))

def needs_batch(es: List<&2, M.Ev>) -> Needs():
  needs_batch.go(es, Nil{})

# ---- a step: its needs at its start, before anything is decided ----
# (law step_is_local) the records an input may read: the messages it
# names (a wait's or a forward's `msg`, a send's `reply_to`, a queued one
# to unqueue or steer), the question a card asks about (its for_msg, read
# from the hot cards), and the agent it names (a restore, a drop, a
# rename...: resolve and name_taken read only the name index, the record
# is for the agent ops). A tick needs none: cold_out keeps in memory every
# record the runtime names (waiters, steered), and a boot's runtime names
# none.

def of_id(+id: Nat) -> Needs():
  Bool.pick(Needs(), Nat.is_eq(id, 0n), none(), of_msg(id))

def of_card(m: Maybe<&2, S.Card>) -> Needs():
  match m:
    case None{}:
      none()
    case Some{+c}:
      of_reply(S.Card.for_msg(c))

def card_needs(+st: S.St, +card: Nat) -> Needs():
  of_card(M.card_of(S.St.cards(st), card))

def needs_in(+st: S.St, i: GI.In) -> Needs():
  match i:
    case GI.IReq{_, _, +q, _, _, _}:
      union(of_id(K.get_n(q, "msg")), of_id(K.get_n(q, "reply_to")))
    case GI.IUnqueue{+id}:
      of_msg(id)
    case GI.ISteerQueued{+id}:
      of_msg(id)
    case GI.IAnswer{+card, _, _}:
      card_needs(st, card)
    case GI.IAnswerPos{+card, _, _}:
      card_needs(st, card)
    case GI.IClose{+card}:
      card_needs(st, card)
    case GI.IConfirmClose{+card, _}:
      card_needs(st, card)
    case GI.IPageAnswered{+card, _, _, _, _}:
      card_needs(st, card)
    case GI.IRestore{+name}:
      of_agent(name)
    case GI.IDrop{+name, _}:
      of_agent(name)
    case GI.IConfirmOpen{+agent, _}:
      of_agent(agent)
    case GI.IIsolate{+name}:
      of_agent(name)
    case GI.IPlaced{+name, _, _}:
      of_agent(name)
    case GI.IRename{+name, _, _}:
      of_agent(name)
    case GI.IFeatureRetire{+name, _, _}:
      of_agent(name)
    case _:
      none()

# ---- a record out of St (what the cold store holds instead) ----

# the state without the cold message `id` (the laws' "stored" record)
def drop_old(+st: S.St, +id: Nat) -> S.St:
  S.St.with_old(st, M.drop_msg(S.St.old(st), id))
