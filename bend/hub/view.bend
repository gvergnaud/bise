# hub/view.bend — the state the Rust views read (board, snapshot,
# contexts, prompts), as JSON: sb-core is the only source of truth, the
# daemon stores this as is.
import Base

import ../vendor/json.bend as J

import ./model.bend as M

import ./codec.bend as C

import ./cx.bend as X

import ./core.bend as H

def jdecl(d: Maybe<&2, M.Decl>, +note: String) -> J.Json:
  match d:
    case None{}:
      J.JNull{}
    case Some{+x}:
      J.obj([J.kv("status", J.JStr{C.decl_str(x)}), J.kv("note", J.JStr{note})])

# the recipient of a message (null: unknown message)
def waits_to(m: Maybe<&2, M.Msg>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+x}:
      J.JStr{M.Msg.to(x)}

# who `n` waits on: the recipient of the message its first `sb wait` (or
# `sb ask`) waits for the reply to; null when it waits on no one
def waits_on(+ws: List<&2, X.Waiter>, +s: M.St, +n: String) -> J.Json:
  match ws:
    case Nil{}:
      J.JNull{}
    case Con{+w, +t}:
      Bool.pick(J.Json, String.eq(X.Waiter.agent(w), n),
        waits_to(M.msg_of(s, X.Waiter.msg(w))), waits_on(t, s, n))

def agent_json(+a: M.Agent, +x: X.Rt, +wo: J.Json) -> J.Json:
  J.obj([J.kv("name", J.JStr{M.Agent.name(a)}), J.kv("dir", J.JStr{M.Agent.dir(a)}),
    J.kv("is_main", J.JBool{M.Agent.is_main(a)}), J.kv("parent", C.jmaybe_str(M.Agent.parent(a))),
    J.kv("brief", M.Agent.brief(a)), J.kv("created_ms", J.JNum{M.Agent.created(a)}),
    J.kv("ws", M.Agent.ws(a)), J.kv("lifecycle", J.JStr{C.life_str(M.Agent.life(a))}),
    J.kv("failure", C.jmaybe_str(M.Agent.failure(a))),
    J.kv("declared", jdecl(M.Agent.declared(a), M.Agent.note(a))),
    J.kv("last_report", M.Agent.report(a)),
    J.kv("aliases", J.JArr{H.jstrs(M.Agent.aliases(a))}), J.kv("files", J.JArr{H.jstrs(M.Agent.files(a))}),
    J.kv("snapshot_ref", C.jmaybe_str(M.Agent.snap(a))),
    J.kv("run", J.JStr{H.run_str(X.Rt.run(x))}), J.kv("waiting", J.JBool{X.Rt.waiting(x)}),
    J.kv("waiting_on", wo),
    J.kv("turn_ms", H.jturn(X.Rt.turn(x)))])

def is_archived(l: M.Life) -> Bool:
  match l:
    case M.LArchived{}:
      True{}
    case _:
      False{}

# a match, not Bool.pick: pick would encode every agent, kept or not
def agents_json.keep(keep: Bool, +a: M.Agent, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +s: M.St, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}:
      agent_json(a, H.rt_find(rts, M.Agent.name(a)), waits_on(ws, s, M.Agent.name(a))) <> rest
    case False{}:
      rest

# every agent (`all`), else the ones in `names` (the only ones a step
# can change, see `changed`). idle-cpu: a step's view sent every agent
# not archived, changed or not, and the hub ticks every 500 ms: ~127 KB
# per idle tick for 20 agents, re-encoded and re-parsed twice a second.
def agents_json(xs: List<&2, M.Agent>, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +s: M.St, +all: Bool, +names: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +t}:
      agents_json.keep(all || M.str_in(names, M.Agent.name(a)),
        a, rts, ws, s, agents_json(t, rts, ws, s, all, names))

def rec_json(+r: M.MRec) -> J.Json:
  J.obj([J.kv("msg", C.msg_json(M.MRec.msg(r))), J.kv("state", J.obj(C.mst_kvs(M.MRec.st(r)))),
    J.kv("settled", J.JBool{M.MRec.settled(r)})])

# a match, not Bool.pick: pick would encode every message, kept or not
def recs_json.keep(keep: Bool, +r: M.MRec, +rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}:
      rec_json(r) <> rest
    case False{}:
      rest

def recs_json(xs: List<&2, M.MRec>, +all: Bool, +ids: List<&2, Nat>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      recs_json.keep(all || M.nat_in(ids, M.Msg.id(M.MRec.msg(r))), r, recs_json(t, all, ids))

# the messages of the view: no walk of every message when the step
# touched none (an idle tick)
def msgs_json(walk: Bool, +xs: List<&2, M.MRec>, +all: Bool, +ids: List<&2, Nat>) -> List<&2, J.Json>:
  match walk:
    case True{}:
      recs_json(xs, all, ids)
    case False{}:
      Nil{}

def cards_json(xs: List<&2, M.Card>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      C.card_json(c) <> cards_json(t)

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
      M.Msg.id(m) <> opt_list(M.Msg.reply_to(m))
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
    case M.EDeclared{+name, _, _}:
      [name]
    case M.EReported{+name, _}:
      [name]
    case M.EFileTouched{+name, _}:
      [name]
    case M.ERenamed{+name, +nn}:
      [name, nn]
    case _:
      Nil{}

def names_of(evs: List<&2, M.Ev>) -> List<&2, String>:
  match evs:
    case Nil{}:
      Nil{}
    case Con{+e, +t}:
      List.append(&2, String, ev_names(e), names_of(t))

# a runtime at rest: no REPL, no wait, no turn (what rt_find answers
# for an agent it does not know)
def rt_rest(turn: Maybe<&2, Nat>, down: Bool, waiting: Bool) -> Bool:
  match turn:
    case None{}:
      down && Bool.not(waiting)
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
  H.cat([H.run_str(X.Rt.run(x)), " ", Bool.pick(String, X.Rt.waiting(x), "w", "-"), " ", turn_key(X.Rt.turn(x))])

def rt_marks.keep(rest: Bool, +x: X.Rt, +acc: List<&2, Mark>) -> List<&2, Mark>:
  match rest:
    case True{}:
      acc
    case False{}:
      Mark{X.Rt.name(x), rt_key(x)} <> acc

def rt_marks(rts: List<&2, X.Rt>) -> List<&2, Mark>:
  match rts:
    case Nil{}:
      Nil{}
    case Con{+x, +t}:
      rt_marks.keep(rt_rest(X.Rt.turn(x), H.is_down(X.Rt.run(x)), X.Rt.waiting(x)), x, rt_marks(t))

def waiter_marks(ws: List<&2, X.Waiter>) -> List<&2, Mark>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      Mark{X.Waiter.agent(w), H.cat(["wait ", Nat.show(X.Waiter.token(w)), " ", Nat.show(X.Waiter.msg(w))])} <> waiter_marks(t)

# the marks of every runtime not at rest and every wait (main.bend takes
# them before a step, `changed` after it)
def marks(+rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>) -> List<&2, Mark>:
  List.append(&2, Mark, rt_marks(rts), waiter_marks(ws))

def mark_in(xs: List<&2, Mark>, +m: Mark) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      mark_eq(h, m) || mark_in(t, m)

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

def changed(+evs: List<&2, M.Ev>, +before: List<&2, Mark>, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>) -> List<&2, String>:
  changed.go(names_of(evs), before, marks(rts, ws))

# the order, the cards and the notes change only by an event (model.bend
# `apply`): a view with none (`durable` False) leaves them out, the
# daemon keeps its own
def durable_kvs(durable: Bool, +s: M.St) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match durable:
    case True{}:
      [J.kv("order", J.JArr{H.jstrs(M.St.order(s))}),
        J.kv("cards", J.JArr{cards_json(M.St.cards(s))}),
        J.kv("notes", J.JArr{H.jstrs(M.St.notes(s))})]
    case False{}:
      Nil{}

# the view: the agents in `names` (or all), the messages in `ids` (or
# all); the order, cards and notes when `durable` (or all)
def view(+s: M.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +all: Bool, +ids: List<&2, Nat>, +names: List<&2, String>, +durable: Bool) -> J.Json:
  J.obj(List.append(&2, Sigma<&2, &2, String, _ => J.Json>,
    [J.kv("agents", J.JArr{agents_json(M.St.agents(s), rts, ws, s, all, names)}),
      J.kv("all_agents", J.JBool{all}),
      J.kv("msgs", J.JArr{msgs_json(all || Bool.not(List.is_empty(&2, Nat, ids)), M.St.msgs(s), all, ids)}),
      J.kv("all_msgs", J.JBool{all}),
      J.kv("next_msg", J.JNum{M.St.next_msg(s)}), J.kv("next_card", J.JNum{M.St.next_card(s)})],
    durable_kvs(all || durable, s)))
