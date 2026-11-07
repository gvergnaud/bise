# hub/inv.bend — the invariants the laws state about the hub's state
# (bend/laws/hub.bend). Definitions only: the proofs are in bend/proofs/hub.bend
# and bend/proofs/lib/.
import Base

import ./model.bend as M
import ./gen/state.bend as S

import ./cx.bend as X

import ./core.bend as H

import ./pause.bend as P

import ./timers.bend as TM

import ./wakes.bend as WK

# the message ids, in the order of the state
def ids(xs: List<&2, S.MRec>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      S.Msg.id(S.MRec.msg(r)) <> ids(t)

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
def Unique(+s: S.St) -> Data:
  X.Both<Distinct(ids(M.every(s))), Below(ids(M.every(s)), S.St.next_msg(s))>

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
def Kept(+s1: S.St, +s2: S.St) -> Data:
  IsPrefix(ids(M.every(s1)), ids(M.every(s2)))

# ---- the split of the messages (issue 21 steps 3-4; laws split_*) ----

# every id of `xs` is above `n`
def Above(xs: List<&2, S.MRec>, +n: Nat) -> Data:
  match xs:
    case Nil{}:
      Unit
    case Con{+x, +t}:
      X.Both<{Nat.is_gt(M.MRec.id(x), n) == True{} : Bool}, Above(t, n)>

# ids ascending (each below every later one), descending
def Asc(xs: List<&2, S.MRec>) -> Data:
  match xs:
    case Nil{}:
      Unit
    case Con{+x, +t}:
      X.Both<Above(t, M.MRec.id(x)), Asc(t)>

def Desc(xs: List<&2, S.MRec>) -> Data:
  match xs:
    case Nil{}:
      Unit
    case Con{+x, +t}:
      X.Both<Below(ids(t), M.MRec.id(x)), Desc(t)>

def AllCold(xs: List<&2, S.MRec>) -> Data:
  match xs:
    case Nil{}:
      Unit
    case Con{+x, +t}:
      X.Both<{M.MRec.is_hot(x) == False{} : Bool}, AllCold(t)>

# no id of `hs` is one of `os`
def Disjoint(hs: List<&2, S.MRec>, +os: List<&2, S.MRec>) -> Data:
  match hs:
    case Nil{}:
      Unit
    case Con{+h, +t}:
      X.Both<{M.nat_in(ids(os), M.MRec.id(h)) == False{} : Bool}, Disjoint(t, os)>

# the two sides: hot ids ascending, old ids descending, every old one
# cold, no id on both sides, every id below the next free one
def Parts(+hs: List<&2, S.MRec>, +os: List<&2, S.MRec>, +nm: Nat) -> Data:
  X.Both<Asc(hs), X.Both<Desc(os), X.Both<AllCold(os), X.Both<Disjoint(hs, os), X.Both<Below(ids(hs), nm), Below(ids(os), nm)>>>>>

def Split(+s: S.St) -> Data:
  Parts(S.St.hot(s), S.St.old(s), S.St.next_msg(s))

# ---- every message as one list: what the split must keep (cold_is_inert) ----
# The three message events on one list of every message, as model.bend's
# apply did them before the split (issue 21 steps 3-4): a new message at
# the end, settled by a reply from its recipient (or the user); a state
# or a settle on the record of its id.
def settle1.if(ok: Bool, +msgs: List<&2, S.MRec>, +id: Nat) -> List<&2, S.MRec>:
  match ok:
    case True{}:
      M.map_msg(msgs, id, M.OSettle{})
    case False{}:
      msgs

def settle1.go(orig: Maybe<&2, S.MRec>, +msgs: List<&2, S.MRec>, +id: Nat, +from: String) -> List<&2, S.MRec>:
  match orig:
    case None{}:
      msgs
    case Some{+o}:
      settle1.if(String.eq(S.Msg.to(S.MRec.msg(o)), from) || String.eq(from, M.USER()), msgs, id)

def settle1(r: Maybe<&2, Nat>, +msgs: List<&2, S.MRec>, +from: String) -> List<&2, S.MRec>:
  match r:
    case None{}:
      msgs
    case Some{+id}:
      settle1.go(M.rec_of(msgs, id), msgs, id, from)

def sent1(+msgs: List<&2, S.MRec>, +m: S.Msg) -> List<&2, S.MRec>:
  settle1(S.Msg.reply_to(m), M.snoc(S.MRec, msgs, M.new_rec(m)), S.Msg.from(m))

# every message after an event, on one list
def apply1(+msgs: List<&2, S.MRec>, e: M.Ev) -> List<&2, S.MRec>:
  match e:
    case M.EMsgSent{+m}:
      sent1(msgs, m)
    case M.EMsgState{+id, +s}:
      M.map_msg(msgs, id, M.OSt{s})
    case M.ESettled{+id}:
      M.map_msg(msgs, id, M.OSettle{})
    case M.EMsgsMoved{+from, +to}:
      M.moved(msgs, from, to)
    case _:
      msgs

# the card ids, in the order of the state
def card_ids(cs: List<&2, S.Card>) -> List<&2, Nat>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      S.Card.id(c) <> card_ids(t)

# law cards_unique: no two open cards share an id, every id is below the
# next free one
def UniqueCards(+s: S.St) -> Data:
  X.Both<Distinct(card_ids(S.St.cards(s))), Below(card_ids(S.St.cards(s)), S.St.next_card(s))>

def stuck.agent(m: Maybe<&2, S.Agent>, +s: S.St, +n: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      Bool.and(H.is_active(S.Agent.life(a)), Bool.and(Bool.not(List.is_empty(&2, S.Msg, M.queued_for(s, n))), Bool.not(P.is_paused(s))))

# law L4: agent `n` is idle, active, and has messages queued (a message
# stuck: nothing would ever deliver it). A paused project holds them on
# purpose (cost-pause): /resume delivers them, so they are not stuck
def stuck(+s: S.St, +rts: List<&2, X.Rt>, +n: String) -> Bool:
  Bool.and(H.is_idle(X.Rt.run(H.rt_find(rts, n))), stuck.agent(M.get_agent(s, n), s, n))

# ---- law L4 support: the names of two agents never meet ----

# an agent's names: its name, then its old ones
def names(+a: S.Agent) -> List<&2, String>:
  S.Agent.name(a) <> S.Agent.aliases(a)

def profile(xs: List<&2, S.Agent>) -> List<&2, List<&2, String>>:
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
# another one, live or archived (the agents split: St.agents, then arch)
def NamesApart(+s: S.St) -> Data:
  NamesOK(profile(List.append(&2, S.Agent, S.St.agents(s), S.St.arch(s))))

# law no_reserved_names (c4b-core-step-flat): no name or old name of an
# agent, live or archived, is a never_agent name (the user's, bise's,
# GitHub's, a view's), so model.bend's lookups answer none for one
# without a walk
def Clean(xs: List<&2, S.Agent>) -> Data:
  match xs:
    case Nil{}:
      Unit
    case Con{+a, +t}:
      X.Both<{M.never_in(names(a)) == False{} : Bool}, Clean(t)>

def NoReserved(+s: S.St) -> Data:
  X.Both<Clean(S.St.agents(s)), Clean(S.St.arch(s))>

# ---- the agents split (issue 21 sha 2, hub/agents.bend) ----

# every agent of `xs` is live (`v` True) or archived (`v` False)
def Lives(xs: List<&2, S.Agent>, +v: Bool) -> Data:
  match xs:
    case Nil{}:
      Unit
    case Con{+a, +t}:
      X.Both<{M.live_life(S.Agent.life(a)) == v : Bool}, Lives(t, v)>

# law split_agents_*: St.agents holds no archived agent, arch only
# archived ones (any journal: every event keeps it, Fresh or not)
def AgentsSplit(+s: S.St) -> Data:
  X.Both<Lives(S.St.agents(s), True{}), Lives(S.St.arch(s), False{})>

# ---- the timers (`sb every`, hub/timers.bend) ----

# law wake_drop_ends: no watch of the list belongs to `n`
def WatchNoneOf(xs: List<&2, S.Watch>, +n: String) -> Data:
  match xs:
    case Nil{}:
      Unit
    case Con{+w, +r}:
      X.Both<{String.eq(S.Watch.agent(w), n) == False{} : Bool}, WatchNoneOf(r, n)>

# law stop_leaves_no_timer: no timer of the list belongs to `n`
def TimerNoneOf(xs: List<&2, S.Timer>, +n: String) -> Data:
  match xs:
    case Nil{}:
      Unit
    case Con{+t, +r}:
      X.Both<{String.eq(S.Timer.agent(t), n) == False{} : Bool}, TimerNoneOf(r, n)>

def fire_spent(sp: Bool, +xs: List<&2, M.Ev>, +id: Nat, +at: Nat) -> List<&2, M.Ev>:
  match sp:
    case True{}:
      M.snoc(M.Ev, xs, M.ETimerStop{id, "it ran its times", at})
    case False{}:
      xs

# what a fire whose message was queued adds to the journal: a run now is
# a run; a due wake is counted (fired + 1, its next time), and its last
# one ends the timer
def fire_count(due: Bool, +xs: List<&2, M.Ev>, +t: S.Timer, +next: Nat, +at: Nat) -> List<&2, M.Ev>:
  match due:
    case False{}:
      M.snoc(M.Ev, xs, M.ETimerRun{S.Timer.id(t), at})
    case True{}:
      fire_spent(TM.spent(t), M.snoc(M.Ev, xs, M.ETimerFired{S.Timer.id(t), 1n+S.Timer.fired(t), next, at, False{}}), S.Timer.id(t), at)

# what a refused wake adds: a due one, only its retry line (fired
# unchanged, tried again in a minute); a run now, nothing
def fire_retry(due: Bool, +xs: List<&2, M.Ev>, +t: S.Timer, +now: Nat) -> List<&2, M.Ev>:
  match due:
    case False{}:
      xs
    case True{}:
      M.snoc(M.Ev, xs, M.ETimerFired{S.Timer.id(t), S.Timer.fired(t), Nat.add(now, TM.MIN_MS()), S.Timer.last(t), True{}})

# law fire_is_a_message: the journal after the wake's send `p`: only the
# retry line when the send was refused, the fire's events when it was queued
def fire_journal(-o: S.St, p: X.Cx<o> & H.SendR, +t: S.Timer, +due: Bool, +next: Nat) -> List<&2, M.Ev>:
  match p:
    case (+cx, H.SErr{_}):
      fire_retry(due, X.journal(X.Core.fx(o, X.Cx.c(o, cx))), t, H.now(o, cx))
    case (+cx, H.SOk{_}):
      fire_count(due, X.journal(X.Core.fx(o, X.Cx.c(o, cx))), t, next, H.now(o, cx))
