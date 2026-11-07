# hub/view.bend — the state the Rust views read (board, snapshot,
# contexts, prompts), as JSON: sb-core is the only source of truth, the
# daemon stores this as is. Its shape is declared in bise_proto::core::view
# (View, ViewAgent, ViewMsg); the encoders are generated (gen/view.bend).
import Base

import ../vendor/json.bend as J

import ./gen/view.bend as V

import ./model.bend as M
import ./gen/state.bend as S
import ./gen/state-codec.bend as SC

import ./cx.bend as X

import ./core.bend as H

import ./timers.bend as TM
import ./wakes.bend as WK
import ./lazy.bend as LZ
import ./agents.bend as AG

def jdecl(d: Maybe<&2, S.Decl>, +note: String) -> J.Json:
  match d:
    case None{}:
      J.JNull{}
    case Some{+x}:
      J.obj([J.kv("status", J.JStr{SC.Decl.word(x)}), J.kv("note", J.JStr{note})])

# the recipient of a message (none: unknown message)
def waits_to(m: Maybe<&2, S.Msg>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{+x}:
      Some{S.Msg.to(x)}

# who `n` waits on: the recipient of the message its first `sb wait` (or
# `sb ask`) waits for the reply to; none when it waits on no one
def waits_on(+ws: List<&2, X.Waiter>, +s: S.St, +n: String) -> Maybe<&2, String>:
  match ws:
    case Nil{}:
      None{}
    case Con{+w, +t}:
      LZ.pick(Maybe<&2, String>, String.eq(X.Waiter.agent(w), n),
        u => waits_to(M.msg_of(s, X.Waiter.msg(w))), u => waits_on(t, s, n))

def agent_json(+a: S.Agent, +x: X.Rt, +wo: Maybe<&2, String>) -> J.Json:
  V.agent(S.Agent.name(a), S.Agent.dir(a), S.Agent.is_main(a), S.Agent.parent(a),
    S.Agent.brief(a), S.Agent.created(a), S.Agent.ws(a), SC.Life.word(S.Agent.life(a)),
    S.Agent.failure(a), jdecl(S.Agent.declared(a), S.Agent.note(a)), S.Agent.report(a),
    S.Agent.aliases(a), S.Agent.files(a), S.Agent.snap(a), M.follows(a),
    H.run_str(X.Rt.run(x)), X.Rt.waiting(x), wo, X.Rt.turn(x), X.Rt.turns(x))

def is_archived(l: S.Life) -> Bool:
  match l:
    case S.LArchived{}:
      True{}
    case _:
      False{}

# a match, not Bool.pick: pick would encode every agent, kept or not
def agents_json.keep(keep: Bool, +a: S.Agent, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +s: S.St, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}:
      agent_json(a, H.rt_find(rts, S.Agent.name(a)), waits_on(ws, s, S.Agent.name(a))) <> rest
    case False{}:
      rest

# every agent (`all`), else the ones in `names` (the only ones a step
# can change, see `changed`). idle-cpu: a step's view sent every agent
# not archived, changed or not, and the hub ticks every 500 ms: ~127 KB
# per idle tick for 20 agents, re-encoded and re-parsed twice a second.
def agents_json(xs: List<&2, S.Agent>, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +s: S.St, +all: Bool, +names: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +t}:
      agents_json.keep(all || M.str_in(names, S.Agent.name(a)),
        a, rts, ws, s, agents_json(t, rts, ws, s, all, names))

# view-all-perf: how many of the newest messages view_all sends whole
def RECENT() -> Nat:
  256n

# view-all-perf: a message view_all sends whole (its text and parts): a
# hot one (queued, owing a reply), a question not settled, or one of the
# RECENT newest (`next`: the next message id). Every other one is cold:
# its compact row in the view's `cold` (cold_json), without its body;
# the hub asks msg_bodies when it needs one (on a big hub the bodies were
# ~85 % of view_all, the rows' keys most of the rest). A step's view and
# msg_bodies send every message whole.
def ships_whole(+r: S.MRec, +next: Nat) -> Bool:
  +m = S.MRec.msg(r)
  M.MRec.is_hot(r) || (S.Msg.expect(m) && Bool.not(S.MRec.settled(r))) || Nat.is_le(next, Nat.add(S.Msg.id(m), RECENT()))

def jmaybe_n(m: Maybe<&2, Nat>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+n}:
      J.JNum{n}

# a cold message's row (bise_proto::core::ColdMsg): [id, thread, from,
# to, reply_to, expect_reply, created_ms, plain, state, settled]; never
# queued nor owing (ships_whole sends those whole)
def cold_json(+r: S.MRec) -> J.Json:
  +m = S.MRec.msg(r)
  J.JArr{[J.JNum{S.Msg.id(m)}, J.JNum{S.Msg.thread(m)}, J.JStr{S.Msg.from(m)}, J.JStr{S.Msg.to(m)}, jmaybe_n(S.Msg.reply_to(m)),
    J.JBool{S.Msg.expect(m)}, J.JNum{S.Msg.created(m)}, J.JBool{S.Msg.plain(m)}, SC.MSt.journal_json(S.MRec.st(r)), J.JBool{S.MRec.settled(r)}]}

# a message, its state, and what the steps' predicates (model.bend) say
# of it: queued, or delivered owing a reply (the hub's Waiting reads them)
def rec_json(+r: S.MRec) -> J.Json:
  +st = S.MRec.st(r)
  V.msg(SC.Msg.journal_json(S.MRec.msg(r)), SC.MSt.journal_json(st), S.MRec.settled(r), M.is_queued(st), M.MRec.owes(r))

# a match, not Bool.pick: pick would encode every message, kept or not
def recs_json.keep(keep: Bool, +r: S.MRec, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}:
      rec_json(r) <> rest
    case False{}:
      rest

# the messages of `ids` (`all`: the whole ones of view_all, ships_whole)
def recs_json.wants(all: Bool, +r: S.MRec, +ids: List<&2, Nat>, +next: Nat) -> Bool:
  match all:
    case True{}:
      ships_whole(r, next)
    case False{}:
      M.nat_in(ids, S.Msg.id(S.MRec.msg(r)))

def recs_json(xs: List<&2, S.MRec>, +all: Bool, +ids: List<&2, Nat>, +next: Nat) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      recs_json.keep(recs_json.wants(all, r, ids, next), r, recs_json(t, all, ids, next))

def colds_json.keep(whole: Bool, +r: S.MRec, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match whole:
    case True{}:
      rest
    case False{}:
      cold_json(r) <> rest

# view_all's cold messages, in send order (the others of recs_json)
def colds_json(xs: List<&2, S.MRec>, +next: Nat) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      colds_json.keep(ships_whole(r, next), r, colds_json(t, next))

# the cold rows: view_all's only (a step's view sends none, absent)
def colds_when(all: Bool, +xs: List<&2, S.MRec>, +next: Nat) -> Maybe<&2, List<&2, J.Json>>:
  match all:
    case True{}:
      Some{colds_json(xs, next)}
    case False{}:
      None{}

# every id of `ids` is a message of `xs`
def ids_in(ids: List<&2, Nat>, +xs: List<&2, S.MRec>) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+i, +t}:
      LZ.and(Maybe.is_some(&2, S.MRec, M.rec_of(xs, i)), u => ids_in(t, xs))

# the lowest id of `ids` (0 when none)
def ids_lo.go(ids: List<&2, Nat>, +acc: Nat) -> Nat:
  match ids:
    case Nil{}:
      acc
    case Con{+i, +t}:
      ids_lo.go(t, Nat.min(acc, i))

def ids_lo(ids: List<&2, Nat>) -> Nat:
  match ids:
    case Nil{}:
      0n
    case Con{+i, +t}:
      ids_lo.go(t, i)

# the head of `xs` has an id below `lo`
def id_below(xs: List<&2, S.MRec>, +lo: Nat) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, _}:
      Nat.is_lt(M.MRec.id(h), lo)

# the cold messages of `ids`, newest first: St.old is newest first, so the
# walk stops at the first id below the lowest one asked (`below`)
def old_of(xs: List<&2, S.MRec>, below: Bool, +ids: List<&2, Nat>, +lo: Nat) -> List<&2, S.MRec>:
  match xs below:
    case Nil{} _:
      Nil{}
    case Con{_, _} True{}:
      Nil{}
    case Con{+r, +t} False{}:
      +rest = old_of(t, id_below(t, lo), ids, lo)
      Bool.pick(List<&2, S.MRec>, M.nat_in(ids, M.MRec.id(r)), r <> rest, rest)

# where the view finds the step's messages (issue 21): the hot ones when
# they are all there (a message sent and still queued, or owing a reply);
# else the hot ones with the cold ones of `ids` (a message the step just
# turned cold), in send order; all of them (view_all): M.every
def view_recs.some(here: Bool, +s: S.St, +ids: List<&2, Nat>) -> List<&2, S.MRec>:
  match here:
    case True{}:
      S.St.hot(s)
    case False{}:
      +lo = ids_lo(ids)
      List.reverse(&2, S.MRec, M.ins_all(S.St.hot(s), old_of(S.St.old(s), id_below(S.St.old(s), lo), ids, lo)))

def view_recs.go(every: Bool, +s: S.St, +ids: List<&2, Nat>) -> List<&2, S.MRec>:
  match every:
    case True{}:
      M.every(s)
    case False{}:
      view_recs.some(ids_in(ids, S.St.hot(s)), s, ids)

def view_recs(+s: S.St, +all: Bool, +ids: List<&2, Nat>) -> List<&2, S.MRec>:
  view_recs.go(all, s, ids)

# the messages of the view: no walk of every message when the step
# touched none (an idle tick)
def msgs_json(walk: Bool, +xs: List<&2, S.MRec>, +all: Bool, +ids: List<&2, Nat>, +next: Nat) -> List<&2, J.Json>:
  match walk:
    case True{}:
      recs_json(xs, all, ids, next)
    case False{}:
      Nil{}

def cards_json(xs: List<&2, S.Card>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      SC.Card.journal_json(c) <> cards_json(t)

def opt_list(m: Maybe<&2, Nat>) -> List<&2, Nat>:
  match m:
    case None{}:
      Nil{}
    case Some{+r}:
      [r]

# the message ids an event touches (a reply settles its question)
def ev_ids(e: M.Ev) -> List<&2, Nat>:
  match e:
    case M.EMsgSent{+m}:
      S.Msg.id(m) <> opt_list(S.Msg.reply_to(m))
    case M.EMsgState{+id, _}:
      [id]
    case M.ESettled{+id}:
      [id]
    case _:
      Nil{}

def touched(evs: List<&2, M.Ev>) -> List<&2, Nat>:
  match evs:
    case Nil{}:
      Nil{}
    case Con{+e, +t}:
      List.append(&2, Nat, ev_ids(e), touched(t))

# name-free: msgs_moved names no id; its rows are the hot ones to `to`
# after the step (a few more rows than moved, never fewer)
def to_ids(xs: List<&2, S.MRec>, +n: String) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      +rest = to_ids(t, n)
      Bool.pick(List<&2, Nat>, String.eq(S.Msg.to(S.MRec.msg(r)), n), M.MRec.id(r) <> rest, rest)

def moved_ids(evs: List<&2, M.Ev>, +hot: List<&2, S.MRec>) -> List<&2, Nat>:
  match evs:
    case Nil{}:
      Nil{}
    case Con{M.EMsgsMoved{_, +to}, +t}:
      List.append(&2, Nat, to_ids(hot, to), moved_ids(t, hot))
    case Con{_, +t}:
      moved_ids(t, hot)

# the agents an event changes (a rename: both names)
def ev_names(e: M.Ev) -> List<&2, String>:
  match e:
    case M.ETaskCreated{+name, _, _, _, _}:
      [name]
    case M.ELifecycle{+name, _, _}:
      [name]
    case M.EWorkspace{+name, _}:
      [name]
    case M.ESnapshot{+name, _}:
      [name]
    case M.EPlaced{+name, _, _}:
      [name]
    case M.EDeclared{+name, _, _}:
      [name]
    case M.EReported{+name, _}:
      [name]
    case M.EFileTouched{+name, _}:
      [name]
    case M.EClaimDropped{+name, _}:
      [name]
    case M.ERenamed{+name, +nn}:
      [name, nn]
    # S10: following and a job's end change the agent's row
    case M.EFollow{+name, _, _}:
      [name]
    case M.EJobEnd{+name, _}:
      [name]
    case _:
      Nil{}

# name-free: a take of an old name strips it from its holder (its row's
# old names change); the step's `retarget` out names that holder
def fx_name.of(is_rt: Bool, +x: J.Json) -> List<&2, String>:
  match is_rt:
    case True{}:
      [J.get_str_or(x, "to", "")]
    case False{}:
      Nil{}

def fx_names(xs: List<&2, X.Fx>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{X.FO{+x}, +t}:
      List.append(&2, String, fx_name.of(String.eq(J.get_str_or(x, "fx", ""), "retarget"), x), fx_names(t))
    case Con{_, +t}:
      fx_names(t)

def names_of(evs: List<&2, M.Ev>) -> List<&2, String>:
  match evs:
    case Nil{}:
      Nil{}
    case Con{+e, +t}:
      List.append(&2, String, ev_names(e), names_of(t))

# a runtime at rest: no REPL, no wait, no turn, no turn ended (what
# rt_find answers for an agent it does not know)
def rt_rest(turn: Maybe<&2, Nat>, down: Bool, waiting: Bool, turns: Nat) -> Bool:
  match turn:
    case None{}:
      match turns:
        case 0n:
          down && Bool.not(waiting)
        case 1n+_:
          False{}
    case Some{_}:
      False{}

# a mark of what an agent's view reads outside the durable state: a
# runtime not at rest (run, waiting, turn) or a wait (`sb wait`). A step
# changes that part of an agent's view only if it adds, removes or
# changes one of its marks (a runtime at rest is one value, the one
# rt_find answers for an agent it does not know).
type Mark is Data:
  Mark{name: String, key: String}

def Mark.name(x: Mark) -> String:
  match x:
    case Mark{+name, _}:
      name

def Mark.key(x: Mark) -> String:
  match x:
    case Mark{_, +key}:
      key

def mark_eq(+a: Mark, +b: Mark) -> Bool:
  String.eq(Mark.name(a), Mark.name(b)) && String.eq(Mark.key(a), Mark.key(b))

def turn_key(m: Maybe<&2, Nat>) -> String:
  match m:
    case None{}:
      "-"
    case Some{+t}:
      Nat.show(t)

def rt_key(+x: X.Rt) -> String:
  H.cat([H.run_str(X.Rt.run(x)), " ", Bool.pick(String, X.Rt.waiting(x), "w", "-"), " ", turn_key(X.Rt.turn(x)), " ", Nat.show(X.Rt.turns(x))])

# a runtime's mark: none at rest, else its key
def rt_mark.of(rest: Bool, +x: X.Rt) -> Maybe<&2, String>:
  match rest:
    case True{}:
      None{}
    case False{}:
      Some{rt_key(x)}

def rt_mark(+x: X.Rt) -> Maybe<&2, String>:
  rt_mark.of(rt_rest(X.Rt.turn(x), H.is_down(X.Rt.run(x)), X.Rt.waiting(x), X.Rt.turns(x)), x)

def mark_same(a: Maybe<&2, String>, +b: Maybe<&2, String>) -> Bool:
  match a b:
    case None{} None{}:
      True{}
    case Some{+x} Some{+y}:
      String.eq(x, y)
    case _ _:
      False{}

# the agents of the step's `rt` outs (core.bend set_rt, the one writer of
# a runtime but a rename, whose names its event gives)
def rt_fx.of(is_rt: Bool, +x: J.Json) -> List<&2, String>:
  match is_rt:
    case True{}:
      [J.get_str_or(x, "agent", "")]
    case False{}:
      Nil{}

def rt_names(xs: List<&2, X.Fx>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{X.FO{+x}, +t}:
      List.append(&2, String, rt_fx.of(String.eq(J.get_str_or(x, "fx", ""), "rt"), x), rt_names(t))
    case Con{_, +t}:
      rt_names(t)

def rt_moved.keep(same: Bool, +n: String, +rest: List<&2, String>) -> List<&2, String>:
  match same:
    case True{}:
      rest
    case False{}:
      n <> rest

# the agents of `ns` whose runtime mark the step changed (`before`: the
# runtimes before it): the step's own runtimes only, never every agent
# that ever ran (c4b-core-step-flat: his hub keeps ~900 of them). Safe
# because set_rt (or a rename) is the one writer of a runtime
# (tests/state_guard.py) and a put changes only its own name's runtime
# (laws rt_put_finds_it, rt_put_keeps_others)
def rt_moved(ns: List<&2, String>, +before: List<&2, X.Rt>, +after: List<&2, X.Rt>) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      rt_moved.keep(mark_same(rt_mark(H.rt_find(before, n)), rt_mark(H.rt_find(after, n))), n, rt_moved(t, before, after))

def waiter_marks(ws: List<&2, X.Waiter>) -> List<&2, Mark>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      Mark{X.Waiter.agent(w), H.cat(["wait ", Nat.show(X.Waiter.token(w)), " ", Nat.show(X.Waiter.msg(w))])} <> waiter_marks(t)

def mark_in(xs: List<&2, Mark>, +m: Mark) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      LZ.or(mark_eq(h, m), u => mark_in(t, m))

def gone.keep(kept: Bool, +m: Mark, +rest: List<&2, String>) -> List<&2, String>:
  match kept:
    case True{}:
      rest
    case False{}:
      Mark.name(m) <> rest

# the names of the marks of `xs` that are not in `ys`
def gone(xs: List<&2, Mark>, +ys: List<&2, Mark>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+m, +t}:
      gone.keep(mark_in(ys, m), m, gone(t, ys))

# the agents a step may change (hub-lag, idle-cpu): the ones its events
# name (a durable change is an event, model.bend `apply`) and the ones
# whose marks it changed (`before`: the marks before the step). Every
# other agent is as the last view left it: the daemon keeps it. An idle
# tick changes none: its view carries no agent.
def changed.go(+names: List<&2, String>, +before: List<&2, Mark>, +after: List<&2, Mark>) -> List<&2, String>:
  List.append(&2, String, names, List.append(&2, String, gone(before, after), gone(after, before)))

# `rts0`, `ws0`: the runtimes and the wait marks before the step; a
# runtime changes only by the step's `rt` outs (rt_moved) or a rename (its
# event names both names)
def changed(+evs: List<&2, M.Ev>, +fx: List<&2, X.Fx>, +rts0: List<&2, X.Rt>, +ws0: List<&2, Mark>, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>) -> List<&2, String>:
  changed.go(List.append(&2, String, names_of(evs), rt_moved(rt_names(fx), rts0, rts)), ws0, waiter_marks(ws))

# the cards and the notes change only by an event (model.bend `apply`):
# a view with none (`durable` False) leaves them out, the daemon keeps its
# own. The order (every name, archived ones too: ~900 on his hub) only by
# a task_created or a renamed (M.order_ev; law apply_keeps_order): any
# other step leaves it out
# (c4b-core-step-flat, audit-core F7). A match each, not Bool.pick: pick
# would encode them, sent or not.
def order_when(ordered: Bool, +s: S.St) -> Maybe<&2, List<&2, String>>:
  match ordered:
    case True{}:
      Some{S.St.order(s)}
    case False{}:
      None{}

def cards_when(durable: Bool, +s: S.St) -> Maybe<&2, List<&2, J.Json>>:
  match durable:
    case True{}:
      Some{cards_json(S.St.cards(s))}
    case False{}:
      None{}

def notes_when(durable: Bool, +s: S.St) -> Maybe<&2, List<&2, String>>:
  match durable:
    case True{}:
      Some{S.St.notes(s)}
    case False{}:
      None{}

# an event of the timers (`ended`: one that ends a timer)
def timer_ev(+ended: Bool, e: M.Ev) -> Bool:
  match e:
    case M.ETimerSet{_, _}:
      Bool.not(ended)
    case M.ETimerFired{_, _, _, _, _}:
      Bool.not(ended)
    case M.ETimerRun{_, _}:
      Bool.not(ended)
    case M.ETimerStop{_, _, _}:
      True{}
    case M.ETimerName{_, _, _, _}:
      Bool.not(ended)
    case _:
      False{}

def timer_evs(+ended: Bool, evs: List<&2, M.Ev>) -> Bool:
  match evs:
    case Nil{}:
      False{}
    case Con{+e, +t}:
      LZ.or(timer_ev(ended, e), u => timer_evs(ended, t))

# the timers (`sb every`) only when a step changed them: an idle tick
# carries none (idle-cpu), a fire no ended ones
def timers_live(go: Bool, +s: S.St) -> Maybe<&2, List<&2, J.Json>>:
  match go:
    case True{}:
      Some{TM.live_json(S.Tms.live(S.St.tm(s)))}
    case False{}:
      None{}

def timers_ended(go: Bool, +s: S.St) -> Maybe<&2, List<&2, J.Json>>:
  match go:
    case True{}:
      Some{TM.ended_list_json(S.Tms.ended(S.St.tm(s)))}
    case False{}:
      None{}

# an event of the step changes the order (M.order_ev)
def order_evs(evs: List<&2, M.Ev>) -> Bool:
  match evs:
    case Nil{}:
      False{}
    case Con{+e, +t}:
      LZ.or(M.order_ev(e), u => order_evs(t))

def wake_evs(evs: List<&2, M.Ev>) -> Bool:
  match evs:
    case Nil{}:
      False{}
    case Con{M.EWake{_}, _}:
      True{}
    case Con{_, +t}:
      wake_evs(t)

# the watches (`sb wake`, wakes.bend) only when a step changed them, or
# all: an idle tick carries none (idle-cpu)
def wakes_live(go: Bool, +s: S.St) -> Maybe<&2, List<&2, J.Json>>:
  match go:
    case True{}:
      Some{WK.live_json(S.Wakes.live(S.Tms.wk(S.St.tm(s))))}
    case False{}:
      None{}

def wakes_ended(go: Bool, +s: S.St) -> Maybe<&2, List<&2, J.Json>>:
  match go:
    case True{}:
      Some{WK.ended_list_json(S.Wakes.ended(S.Tms.wk(S.St.tm(s))))}
    case False{}:
      None{}

# the view: the agents in `names` (or all), the messages in `ids` (or
# all); the order, cards and notes when an event of `evs` (the step's
# journal) changed the durable state (or all); the timers and the
# watches when an event changed them (or all). cost-pause: `paused`
# (null: it runs) in every view, a few bytes.
def agent_or(m: Maybe<&2, S.Agent>, +rest: List<&2, S.Agent>) -> List<&2, S.Agent>:
  match m:
    case None{}:
      rest
    case Some{+a}:
      a <> rest

# the agents split (agents.bend): every agent, in creation order (St.order,
# each looked up: St.agents, then arch), the ones in `names` or all
def every_agents(ord: List<&2, String>, +s: S.St, +all: Bool, +names: List<&2, String>) -> List<&2, S.Agent>:
  match ord:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      LZ.pick(List<&2, S.Agent>, all || M.str_in(names, n),
        u => agent_or(M.get_agent(s, n), every_agents(t, s, all, names)), u => every_agents(t, s, all, names))

# a name of `names` is an archived agent's
def any_arch(ns: List<&2, String>, +s: S.St) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case Con{+n, +t}:
      LZ.or(AG.has(S.St.arch(s), n), u => any_arch(t, s))

# a step's agents: St.agents' changed ones when no archived agent changed
# (a tick, a say: the archived ones are never walked), else every agent
# in creation order (view_all, a step that archived or restored one)
def view_agents(wide: Bool, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +all: Bool, +names: List<&2, String>) -> List<&2, J.Json>:
  match wide:
    case True{}:
      agents_json(every_agents(S.St.order(s), s, all, names), rts, ws, s, True{}, Nil{})
    case False{}:
      agents_json(S.St.agents(s), rts, ws, s, False{}, names)

def view.go(+s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +all: Bool, +ids: List<&2, Nat>, +names: List<&2, String>, +durable: Bool, +ordered: Bool, +timers: Bool, +ended: Bool, +wakes: Bool) -> J.Json:
  +xs = view_recs(s, all, ids)
  V.view(view_agents(all || any_arch(names, s), s, rts, ws, all, names), all,
    msgs_json(all || Bool.not(List.is_empty(&2, Nat, ids)), xs, all, ids, S.St.next_msg(s)), all,
    colds_when(all, xs, S.St.next_msg(s)),
    S.St.next_msg(s), S.St.next_card(s), S.St.paused(s),
    order_when(ordered, s), cards_when(durable, s), notes_when(durable, s),
    timers_live(timers, s), timers_ended(ended, s), wakes_live(wakes, s), wakes_ended(wakes, s))

def view(+s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +all: Bool, +ids: List<&2, Nat>, +names: List<&2, String>, +evs: List<&2, M.Ev>) -> J.Json:
  view.go(s, rts, ws, all, ids, names, all || Bool.not(List.is_empty(&2, M.Ev, evs)), all || order_evs(evs),
    all || timer_evs(False{}, evs), all || timer_evs(True{}, evs), all || wake_evs(evs))
