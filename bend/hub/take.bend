# hub/take.bend — name-free (architect m_31139): a take of a name (a new
# task, a rename) frees it first, in the same step, so an archived agent's
# name or an old name no longer holds it forever. Only a live agent's name
# (or bise's) is taken (model.bend name_taken).
#
# 1. An archived agent named `n` moves to `n-2` (the first name no agent
#    has, as a name or an old name: newtask.bend unique_free), a journaled
#    rename: its folder (Agent.dir) stays, so its thread, artifacts and
#    pages stay its own; main's feed says so.
# 2. What is still addressed to the old name `n` of its holder (that
#    archived one, or a live agent renamed from `n`) moves to the holder's
#    name or ends, so nothing that grants or owes passes to the new `n`:
#    pages' and artifacts' agent (fx retarget, the shell rewrites them),
#    open cards (closed, opened again for the holder), timers and watches
#    (stopped, set again for the holder). Messages: a live holder's queued
#    ones and replies owed move to it (msgs_moved, same ids); an archived
#    holder's are refused (the user's said undelivered) and settled.
# 3. The take's own event (task_created, renamed) strips `n` from the
#    holder's old names (model.bend apply: law names_apart).
# Cold messages keep their names: they are history.
#
# The rename itself (the user's /rename, main's sb rename) lives here too
# (moved from core.bend): rename_to.
import Base

import ../vendor/json.bend as J

import ./model.bend as M
import ./gen/state.bend as S
import ./gen/state-laws.bend as SL

import ./lazy.bend as LZ

import ./cx.bend as X

import ./keyed.bend as KL

import ./timers.bend as TM

import ./wakes.bend as WK

import ./newtask.bend as NT

import ./gen/fx.bend as F

import ./gen/feed.bend as FD

import ./gen/kit.bend as K

def feed(-o: S.St, +cx: X.Cx<o>, +agent: String, +rec: J.Json) -> X.Cx<o>:
  X.out(o, cx, J.obj(F.feed(agent, rec)))

# ---- the rename ----

def rename.new(-o: S.St, +cx: X.Cx<o>, +old: String, +new: String, +line: String, +e: {M.name_held(X.d(o, cx), new) == False{} : Bool}) -> X.Cx<o>:
  +cx2 = X.emit(o, cx, M.ERenamed{old, new}, e, Unit{}, Unit{})
  +r = X.Cx.r(o, cx2)
  +cx3 = X.with_r(o, cx2, X.R.with_rts(X.R.with_waiters(X.R.with_steered(r, KL.steer_rename(X.R.steered(r), old, new)),
    X.waiters_rename(X.R.waiters(r), old, new)), X.rt_rename(X.R.rts(r), old, new)))
  +cx4 = X.out(o, cx3, J.obj(F.renamed(old, new)))
  feed(o, cx4, M.MAIN(), FD.info(line))

def rename.free(-o: S.St, held: Bool, +cx: X.Cx<o>, +old: String, +new: String, +line: String, +e: {M.name_held(X.d(o, cx), new) == held : Bool}) -> X.Cx<o>:
  match held:
    case True{}:
      X.out(o, cx, J.obj(F.notice(String.append("invalid or taken name: ", new))))
    case False{}:
      rename.new(o, cx, old, new, line, e)

# ---- what the old name still holds ----

def take.queued(-o: S.St, ms: List<&2, S.Msg>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ms:
    case Nil{}:
      cx
    case Con{+m, +rest}:
      +cx2 = X.emit(o, cx, M.EMsgState{S.Msg.id(m), S.Rejected{"recipient_unavailable"}}, Unit{}, Unit{}, Unit{})
      +cx3 = Bool.pick(X.Cx<o>, String.eq(S.Msg.from(m), M.USER()), feed(o, cx2, M.MAIN(), FD.undelivered(S.Msg.to(m), K.BText{S.Msg.id(m)})), cx2)
      take.queued(o, rest, cx3)

def take.owed(-o: S.St, ms: List<&2, S.Msg>, +cx: X.Cx<o>) -> X.Cx<o>:
  match ms:
    case Nil{}:
      cx
    case Con{+m, +rest}:
      take.owed(o, rest, X.emit(o, cx, M.ESettled{S.Msg.id(m)}, Unit{}, Unit{}, Unit{}))

def take.card(-o: S.St, hit: Bool, +cx: X.Cx<o>, +c: S.Card, +h: String) -> X.Cx<o>:
  match hit:
    case False{}:
      cx
    case True{}:
      +res = String.append("moved to @", h)
      +cx2 = X.emit(o, cx, M.ECardClosed{S.Card.id(c), res, S.Ans.empty()}, Unit{}, Unit{}, Unit{})
      +cx3 = feed(o, cx2, M.MAIN(), FD.card_closed(S.Card.id(c), res))
      +id = S.St.next_card(X.d(o, cx3))
      # the same card, every other field kept: for @h, now, a new id
      +moved = S.Card.with_created(S.Card.with_agent(c, h), X.now(o, cx3))
      +cx4 = X.emit(o, cx3, M.ECardOpened{S.Card.with_id(moved, id)}, SL.Card.id.get_with(moved, id), Unit{}, Unit{})
      feed(o, cx4, M.MAIN(), FD.card(id, S.Card.kind(c), h, S.Card.text(c)))

def take.cards(-o: S.St, cs: List<&2, S.Card>, +cx: X.Cx<o>, +n: String, +h: String) -> X.Cx<o>:
  match cs:
    case Nil{}:
      cx
    case Con{+c, +rest}:
      take.cards(o, rest, take.card(o, String.eq(S.Card.agent(c), n), cx, c, h), n, h)

def take.timers(-o: S.St, ts: List<&2, S.Timer>, +cx: X.Cx<o>, +h: String, +why: String) -> X.Cx<o>:
  match ts:
    case Nil{}:
      cx
    case Con{+t, +rest}:
      +cx2 = X.emit(o, cx, M.ETimerStop{S.Timer.id(t), why, X.now(o, cx)}, Unit{}, Unit{}, Unit{})
      +id = {1n+S.Tms.last_id(S.St.tm(X.d(o, cx2))) : Nat}
      take.timers(o, rest, X.emit(o, cx2, M.ETimerSet{TM.moved(t, h, id), X.now(o, cx2)}, Unit{}, Unit{}, Unit{}), h, why)

def take.watches(-o: S.St, ws: List<&2, S.Watch>, +cx: X.Cx<o>, +h: String, +why: String) -> X.Cx<o>:
  match ws:
    case Nil{}:
      cx
    case Con{+w, +rest}:
      +cx2 = X.emit(o, cx, M.EWake{WK.WEnd{S.Watch.id(w), why, X.now(o, cx)}}, Unit{}, Unit{}, Unit{})
      +id = {1n+S.Wakes.last_id(S.Tms.wk(S.St.tm(X.d(o, cx2)))) : Nat}
      take.watches(o, rest, X.emit(o, cx2, M.EWake{WK.WSet{WK.moved(w, h, id)}}, Unit{}, Unit{}, Unit{}), h, why)

# a hot message to `n` (the records model.bend `moved` rewrites)
def hot_to(xs: List<&2, S.MRec>, +n: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+r, +t}:
      LZ.or(M.MRec.is_hot(r) && String.eq(S.Msg.to(S.MRec.msg(r)), n), u => hot_to(t, n))

# a live holder's queued messages and owed replies to `n` were meant for it
# (architect m_31494): msgs_moved, same ids, never an empty one
def take.moved(-o: S.St, hit: Bool, +cx: X.Cx<o>, +n: String, +h: String) -> X.Cx<o>:
  match hit:
    case False{}:
      cx
    case True{}:
      X.emit(o, cx, M.EMsgsMoved{n, h}, Unit{}, Unit{}, Unit{})

# an archived holder's own were closed by the archive: refused, settled
def take.msgs(-o: S.St, live: Bool, +cx: X.Cx<o>, +n: String, +h: String) -> X.Cx<o>:
  match live:
    case True{}:
      take.moved(o, hot_to(S.St.hot(X.d(o, cx)), n), cx, n, h)
    case False{}:
      +hot = S.St.hot(X.d(o, cx))
      take.owed(o, M.unanswered_in(hot, [n]), take.queued(o, M.queued_in(hot, [n]), cx))

# `h` held `n` as an old name: what still says `n` is h's, or ends
def take.held(-o: S.St, m: Maybe<&2, String>, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  match m:
    case None{}:
      cx
    case Some{+h}:
      +s = X.d(o, cx)
      +why = String.concat(["its old name @", n, " went to another agent: now under @", h])
      +cx2 = X.out(o, cx, J.obj(F.retarget(n, h)))
      +cx3 = take.msgs(o, M.live_agent(M.get_agent(s, h)), cx2, n, h)
      +cx4 = take.cards(o, S.St.cards(s), cx3, n, h)
      +cx5 = take.timers(o, TM.of_name(S.Tms.live(S.St.tm(s)), n), cx4, h, why)
      take.watches(o, WK.of_name(S.Wakes.live(S.Tms.wk(S.St.tm(s))), n), cx5, h, why)

# an archived agent named `n` moves to the first free `n-<i>`
def take.archived.life(-o: S.St, l: S.Life, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  match l:
    case S.LArchived{}:
      +nn = NT.unique_free(X.d(o, cx), n)
      +line = String.concat(["@", n, " (archived) is now @", nn, ": its name went to the new @", n])
      rename.free(o, M.name_held(X.d(o, cx), nn), cx, n, nn, line, {==})
    case _:
      cx

def take.archived(-o: S.St, m: Maybe<&2, S.Agent>, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  match m:
    case Some{+a}:
      take.archived.life(o, S.Agent.life(a), cx, n)
    case None{}:
      cx

# a take of `n` frees it: after it, no agent has `n` as its name unless a
# live one does (the caller refuses), and its old holder's things are its
def take(-o: S.St, +cx: X.Cx<o>, +n: String) -> X.Cx<o>:
  +cx2 = take.archived(o, M.get_agent(X.d(o, cx), n), cx, n)
  take.held(o, M.alias_of(X.d(o, cx2), n), cx2, n)

# the rename (the user's /rename, main's sb rename): the new name taken,
# then the rename on the state the take left (refused when a live agent
# has it: the callers checked it on this step's state)
def rename_to(-o: S.St, +cx: X.Cx<o>, +old: String, +new: String) -> X.Cx<o>:
  +cx2 = take(o, cx, new)
  rename.free(o, M.name_held(X.d(o, cx2), new), cx2, old, new, String.concat(["@", old, " s'appelle maintenant @", new]), {==})
