# hub/inv.bend — the invariants the laws state about the hub's state
# (LAWS.bend). Definitions only: the proofs are in PROOF.bend.
import Base

import ./model.bend as M

import ./cx.bend as X

import ./core.bend as H

import ./timers.bend as TM

# the message ids, in the order of the state
def ids(xs: List<&2, M.MRec>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      M.Msg.id(M.MRec.msg(r)) <> ids(t)

# no id appears twice
def Distinct(ns: List<&2, Nat>) -> Data:
  match ns:
    case Nil{}:
      Unit
    case Con{+x, +t}:
      X.Both<{M.nat_in(t, x) == False{} : Bool}, Distinct(t)>

# every id is below `n`
def Below(ns: List<&2, Nat>, +n: Nat) -> Data:
  match ns:
    case Nil{}:
      Unit
    case Con{+x, +t}:
      X.Both<{Nat.is_lt(x, n) == True{} : Bool}, Below(t, n)>

# law msgs_unique: each message is ONE record (its text and its single
# delivery state, RFC 0003 §6.1): no two records share an id, and every
# id is below the next free one (so a new message can never take one)
def Unique(+s: M.St) -> Data:
  X.Both<Distinct(ids(M.St.msgs(s))), Below(ids(M.St.msgs(s)), M.St.next_msg(s))>

# `a` is the start of `b`
def IsPrefix(a: List<&2, Nat>, b: List<&2, Nat>) -> Data:
  match a b:
    case Nil{} _:
      Unit
    case Con{+x, +t} Nil{}:
      Empty
    case Con{+x, +t} Con{+y, +u}:
      X.Both<{x == y : Nat}, IsPrefix(t, u)>

# law msgs_kept: the messages of a state, in order, start the messages
# of any later state: a message once sent is never removed
def Kept(+s1: M.St, +s2: M.St) -> Data:
  IsPrefix(ids(M.St.msgs(s1)), ids(M.St.msgs(s2)))

# the card ids, in the order of the state
def card_ids(cs: List<&2, M.Card>) -> List<&2, Nat>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      M.Card.id(c) <> card_ids(t)

# law cards_unique: no two open cards share an id, every id is below the
# next free one
def UniqueCards(+s: M.St) -> Data:
  X.Both<Distinct(card_ids(M.St.cards(s))), Below(card_ids(M.St.cards(s)), M.St.next_card(s))>

def stuck.agent(m: Maybe<&2, M.Agent>, +s: M.St, +n: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      Bool.and(H.is_active(M.Agent.life(a)), Bool.not(List.is_empty(&2, M.Msg, M.queued_for(s, n))))

# law L4: agent `n` is idle, active, and has messages queued (a message
# stuck: nothing would ever deliver it)
def stuck(+s: M.St, +rts: List<&2, X.Rt>, +n: String) -> Bool:
  Bool.and(H.is_idle(X.Rt.run(H.rt_find(rts, n))), stuck.agent(M.get_agent(s, n), s, n))

# ---- law L4 support: the names of two agents never meet ----

# an agent's names: its name, then its old ones
def names(+a: M.Agent) -> List<&2, String>:
  M.Agent.name(a) <> M.Agent.aliases(a)

def profile(xs: List<&2, M.Agent>) -> List<&2, List<&2, String>>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +t}:
      names(a) <> profile(t)

def Apart.q(+n: String, +al: List<&2, String>, q: List<&2, String>) -> Data:
  match q:
    case Nil{}:
      Unit
    case Con{+m, +bl}:
      X.Both<{M.str_in(m <> bl, n) == False{} : Bool}, {M.str_in(al, m) == False{} : Bool}>

# p comes before q: q has none of p's names as a name, p none of q's
def Apart(p: List<&2, String>, +q: List<&2, String>) -> Data:
  match p:
    case Nil{}:
      Unit
    case Con{+n, +al}:
      Apart.q(n, al, q)

def ApartAll(+p: List<&2, String>, qs: List<&2, List<&2, String>>) -> Data:
  match qs:
    case Nil{}:
      Unit
    case Con{+q, +r}:
      X.Both<Apart(p, q), ApartAll(p, r)>

def NamesOK(ps: List<&2, List<&2, String>>) -> Data:
  match ps:
    case Nil{}:
      Unit
    case Con{+p, +r}:
      X.Both<ApartAll(p, r), NamesOK(r)>

# law names_apart: no name or old name of an agent is a name or old name of
# another one
def NamesApart(+s: M.St) -> Data:
  NamesOK(profile(M.St.agents(s)))

# ---- the timers (`sb every`, hub/timers.bend) ----

# law stop_leaves_no_timer: no timer of the list belongs to `n`
def TimerNoneOf(xs: List<&2, TM.Timer>, +n: String) -> Data:
  match xs:
    case Nil{}:
      Unit
    case Con{+t, +r}:
      X.Both<{String.eq(TM.Timer.agent(t), n) == False{} : Bool}, TimerNoneOf(r, n)>

def fire_spent(sp: Bool, +xs: List<&2, M.Ev>, +id: Nat, +at: Nat) -> List<&2, M.Ev>:
  match sp:
    case True{}:
      M.snoc(M.Ev, xs, M.ETimerStop{id, "it ran its times", at})
    case False{}:
      xs

# what a fire whose message was queued adds to the journal: a run now is
# a run; a due wake is counted (fired + 1, its next time), and its last
# one ends the timer
def fire_count(due: Bool, +xs: List<&2, M.Ev>, +t: TM.Timer, +next: Nat, +at: Nat) -> List<&2, M.Ev>:
  match due:
    case False{}:
      M.snoc(M.Ev, xs, M.ETimerRun{TM.Timer.id(t), at})
    case True{}:
      fire_spent(TM.spent(t), M.snoc(M.Ev, xs, M.ETimerFired{TM.Timer.id(t), 1n+TM.Timer.fired(t), next, at, False{}}), TM.Timer.id(t), at)

# what a refused wake adds: a due one, only its retry line (fired
# unchanged, tried again in a minute); a run now, nothing
def fire_retry(due: Bool, +xs: List<&2, M.Ev>, +t: TM.Timer, +now: Nat) -> List<&2, M.Ev>:
  match due:
    case False{}:
      xs
    case True{}:
      M.snoc(M.Ev, xs, M.ETimerFired{TM.Timer.id(t), TM.Timer.fired(t), Nat.add(now, TM.MIN_MS()), TM.Timer.last(t), True{}})

# law fire_is_a_message: the journal after the wake's send `p`: only the
# retry line when the send was refused, the fire's events when it was queued
def fire_journal(-o: M.St, p: X.Cx<o> & H.SendR, +t: TM.Timer, +due: Bool, +next: Nat) -> List<&2, M.Ev>:
  match p:
    case (+cx, H.SErr{_}):
      fire_retry(due, X.journal(X.Core.fx(o, X.Cx.c(o, cx))), t, H.now(o, cx))
    case (+cx, H.SOk{_}):
      fire_count(due, X.journal(X.Core.fx(o, X.Cx.c(o, cx))), t, next, H.now(o, cx))
