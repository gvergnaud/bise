# hub/pause.bend - the project's pause (cost-pause: designer's Claude
# Projects benchmark, the user's « une pause globale »): the pure part.
#
# `/pause` (his only: the client socket, never an agent's op) stops every
# busy turn (the interrupt and its 'stopped' line, the turn-stop path)
# and holds everything else: while the state's `paused` is set, sb-core
# delivers nothing (pump, the waiters' hand-off, the end-of-turn nudge),
# fires no timer and starts no turn. Messages, timer wakes and event
# wakes still queue, in their order: nothing is lost. `/resume` clears it
# and pumps every agent in the hub's order. Journal: `paused {at}` and
# `resumed {at}` (gen/journal.bend), replayed like the rest: a restarted hub is
# still paused. Not held: background commands and `sb wake` watches run
# (their wake message waits in the queue); the REPLs stay up, idle.
#
# Here: the decisions over the state and the words. core.bend's pause
# section runs them (it holds pump and the interrupt). Laws in bend/laws/hub.bend:
# paused_delivers_nothing, pause_stops_every_turn, resume_delivers_held,
# pause_idempotent, pause_keeps_order, pause_replays, timer_held_while_paused.
import Base

import ../vendor/json.bend as J

import ./model.bend as M
import ./gen/state.bend as S

import ./cx.bend as X

import ./lazy.bend as LZ

def is_paused(+s: S.St) -> Bool:
  Maybe.is_some(&2, Nat, S.St.paused(s))

def run_busy(r: S.Run) -> Bool:
  match r:
    case S.RBusy{}:
      True{}
    case _:
      False{}

def rt_of(xs: List<&2, X.Rt>, +n: String) -> X.Rt:
  match xs:
    case Nil{}:
      X.Rt{n, S.RDown{}, False{}, None{}, 0n}
    case Con{+h, +t}:
      LZ.pick(X.Rt, String.eq(X.Rt.name(h), n), u => h, u => rt_of(t, n))

# the agents a pause stops: the busy ones, in the hub's order
def busy.go(ns: List<&2, String>, +rts: List<&2, X.Rt>) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case Con{+n, +rest}:
      +more = busy.go(rest, rts)
      Bool.pick(List<&2, String>, run_busy(X.Rt.run(rt_of(rts, n))), n <> more, more)

def busy(+s: S.St, +rts: List<&2, X.Rt>) -> List<&2, String>:
  busy.go(S.St.order(s), rts)

# the messages held: queued for an agent, not delivered yet
def is_queued(s: S.MSt) -> Bool:
  match s:
    case S.Queued{_}:
      True{}
    case _:
      False{}

def held.go(xs: List<&2, S.MRec>) -> Nat:
  match xs:
    case Nil{}:
      0n
    case Con{+r, +t}:
      +rest = held.go(t)
      Bool.pick(Nat, is_queued(S.MRec.st(r)), (1n + rest : Nat), rest)

def held(+s: S.St) -> Nat:
  held.go(S.St.hot(s))

# ---- the words (the hub's notice; the clients draw `paused` typed) ----

# the agents a resume wakes: the distinct recipients of the held messages
def str_in(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      LZ.or(String.eq(h, s), u => str_in(t, s))

def woken.go(xs: List<&2, S.MRec>, +acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case Con{+r, +t}:
      +to = S.Msg.to(S.MRec.msg(r))
      woken.go(t, Bool.pick(List<&2, String>, is_queued(S.MRec.st(r)) && Bool.not(str_in(acc, to)), to <> acc, acc))

def woken(+s: S.St) -> Nat:
  List.length(&2, String, woken.go(S.St.hot(s), Nil{}))

# the held messages that are not bise's own (its go-on notes)
def his.go(xs: List<&2, S.MRec>) -> Nat:
  match xs:
    case Nil{}:
      0n
    case Con{+r, +t}:
      +rest = his.go(t)
      Bool.pick(Nat, is_queued(S.MRec.st(r)) && Bool.not(String.eq(S.Msg.from(S.MRec.msg(r)), M.HUB())), (1n + rest : Nat), rest)

def his(+s: S.St) -> Nat:
  his.go(S.St.hot(s))

# ---- the words (designer m_18789; the clients draw `paused` typed) ----

def count(+n: Nat, +one: String, +many: String) -> String:
  Bool.pick(String, Nat.is_eq(n, 1n), String.append("1 ", one), String.concat([Nat.show(n), " ", many]))

def paused_text(+stopped: Nat) -> String:
  String.concat(["paused: ", count(stopped, "turn", "turns"), " stopped. messages, scheduled tasks and waits are held. /resume goes on."])

# what a cut agent reads at /resume (held like the rest until then)
def GO_ON() -> String:
  "the project was paused. go on where your turn stopped."

# tasks: a work's pause (architect m_64602/m_64603), words to agents.
# The note a busy turn of work `id` gets: it saves and stops by itself;
# with other open works (`open`, their ids) it goes on with them
def work_paused(+id: String, open: List<&2, String>) -> String:
  match open:
    case Nil{}:
      String.concat(["task ", id, " is paused: save your work on your branch, write your resume note (sb report progress \"...\"), then end your turn."])
    case Con{+h, +t}:
      String.concat(["task ", id, " is paused: save its work and write its resume note (sb report progress \"...\"), then go on with ", String.join(Con{h, t}, ", "), "."])

# the note queued at the pause for each agent of work `id`: held while it
# is paused, it steers the work back at resume
def work_resumed(+id: String) -> String:
  String.concat(["task ", id, " resumed: pick it up from your resume note."])

def resumed_text(+agents: Nat, +msgs: Nat) -> String:
  Bool.pick(String, Nat.is_eq(agents, 0n), "resumed: nothing was held.",
    String.concat(["resumed: ", count(agents, "agent goes on", "agents go on"),
      Bool.pick(String, Nat.is_eq(msgs, 0n), "", String.append(", ", count(msgs, "message delivered", "messages delivered"))), "."]))

def ALREADY() -> String:
  "already paused. /resume goes on."

def NOT_PAUSED() -> String:
  "the project isn't paused."
