# hub/wstatus.bend — tasks: a work's status, computed from the facts
# (page tasks-center's status table; architect m_64423: sb-core owns it,
# a function of facts with a law, never stored, never an agent's word).
# The facts: the work (done, paused, kind, its agents, last), his open
# cards and page waits on it, its agents' runs, waits, timers, watches,
# lives and declared status, and the input's `now`. view.bend encodes the
# row (V.work) for the works a step touched.
import Base

import ../vendor/json.bend as J

import ./model.bend as M
import ./gen/state.bend as S
import ./gen/state-codec.bend as SC
import ./gen/view.bend as V
import ./cx.bend as X
import ./core.bend as H
import ./keyed.bend as KY
import ./work.bend as WO
import ./lazy.bend as LZ

# nothing moves and nothing waits on him for 2 h: Stalled
def STALL_MS() -> Nat:
  7200000n

# a work's status, with what it is about
type WSt is Data:
  WSDone{}
  WSPaused{until: Maybe<&2, Nat>}
  WSNeeds{n: Nat}
  WSWorking{agent: String}
  WSOngoing{}
  WSCrash{agent: String, at: Nat}
  WSReply{agent: String}
  WSTimer{at: Nat}
  WSWatch{agent: String}
  WSReview{}
  WSQuiet{since: Nat, stalled: Bool}

# ---- the facts, per work ----

# his open cards on work `id`
def cards_of(cs: List<&2, S.Card>, +id: String) -> List<&2, Nat>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      LZ.pick(List<&2, Nat>, WO.has(S.Card.works(c), id), u => S.Card.id(c) <> cards_of(t, id), u => cards_of(t, id))

def is_busy(r: S.Run) -> Bool:
  match r:
    case S.RBusy{}:
      True{}
    case _:
      False{}

def turn_serves(m: Maybe<&2, S.Agent>, +id: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+a}:
      WO.has(S.Agent.turn_works(a), id)

# the first of `names` in a turn that serves `id`
def working(ns: List<&2, String>, +s: S.St, +rts: List<&2, X.Rt>, +id: String) -> Maybe<&2, String>:
  match ns:
    case Nil{}:
      None{}
    case Con{+n, +t}:
      LZ.pick(Maybe<&2, String>, is_busy(X.Rt.run(H.rt_find(rts, n))) && turn_serves(M.get_agent(s, n), id),
        u => Some{n}, u => working(t, s, rts, id))

def life_failed(l: S.Life) -> Bool:
  match l:
    case S.LFailed{}:
      True{}
    case _:
      False{}

def is_failed(m: Maybe<&2, S.Agent>) -> Bool:
  match m:
    case Some{+a}:
      life_failed(S.Agent.life(a))
    case None{}:
      False{}

# the first of `names` that crashed for good (failed)
def crashed(ns: List<&2, String>, +s: S.St) -> Maybe<&2, String>:
  match ns:
    case Nil{}:
      None{}
    case Con{+n, +t}:
      LZ.pick(Maybe<&2, String>, is_failed(M.get_agent(s, n)), u => Some{n}, u => crashed(t, s))

def to_of(m: Maybe<&2, S.Msg>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{+x}:
      Some{S.Msg.to(x)}

# who an agent of `names` waits on (its `sb wait`, `sb ask` or open
# --expect-reply send)
def reply_wait(ws: List<&2, X.Waiter>, +ns: List<&2, String>, +s: S.St) -> Maybe<&2, String>:
  match ws:
    case Nil{}:
      None{}
    case Con{+w, +t}:
      LZ.pick(Maybe<&2, String>, WO.has(ns, X.Waiter.agent(w)), u => to_of(M.msg_of(s, X.Waiter.msg(w))), u => reply_wait(t, ns, s))

# when an agent of `names` is woken next by its timer
def timer_wait(ts: List<&2, S.Timer>, +ns: List<&2, String>) -> Maybe<&2, Nat>:
  match ts:
    case Nil{}:
      None{}
    case Con{+x, +t}:
      LZ.pick(Maybe<&2, Nat>, WO.has(ns, S.Timer.agent(x)), u => Some{S.Timer.next(x)}, u => timer_wait(t, ns))

# an agent of `names` waits on a watch (`sb wake`)
def watch_wait(ws: List<&2, S.Watch>, +ns: List<&2, String>) -> Maybe<&2, String>:
  match ws:
    case Nil{}:
      None{}
    case Con{+x, +t}:
      LZ.pick(Maybe<&2, String>, WO.has(ns, S.Watch.agent(x)), u => Some{S.Watch.agent(x)}, u => watch_wait(t, ns))

def decl_done(m: Maybe<&2, S.Decl>) -> Bool:
  match m:
    case Some{S.DDone{}}:
      True{}
    case _:
      False{}

# an agent gone from the hub counts as done
def said_done(m: Maybe<&2, S.Agent>) -> Bool:
  match m:
    case Some{+a}:
      decl_done(S.Agent.declared(a))
    case None{}:
      True{}

# every agent of `names` said done (an empty list: none did)
def all_done.go(ns: List<&2, String>, +s: S.St) -> Bool:
  match ns:
    case Nil{}:
      True{}
    case Con{+n, +t}:
      LZ.and(said_done(M.get_agent(s, n)), u => all_done.go(t, s))

def all_done(ns: List<&2, String>, +s: S.St) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case Con{+n, +t}:
      all_done.go(n <> t, s)

# ---- the status, first fact that holds ----

def quiet(+w: S.Work, +now: Nat) -> WSt:
  WSQuiet{S.Work.last(w), Nat.is_ge(now, Nat.add(S.Work.last(w), STALL_MS()))}

def st.review(r: Bool, +w: S.Work, +now: Nat) -> WSt:
  match r:
    case True{}:
      WSReview{}
    case False{}:
      quiet(w, now)

def st.watch(m: Maybe<&2, String>, +w: S.Work, +s: S.St, +now: Nat) -> WSt:
  match m:
    case Some{+a}:
      WSWatch{a}
    case None{}:
      st.review(all_done(S.Work.agents(w), s), w, now)

def st.timer(m: Maybe<&2, Nat>, +w: S.Work, +s: S.St, +now: Nat) -> WSt:
  match m:
    case Some{+at}:
      WSTimer{at}
    case None{}:
      st.watch(watch_wait(S.Wakes.live(S.Tms.wk(S.St.tm(s))), S.Work.agents(w)), w, s, now)

def st.reply(m: Maybe<&2, String>, +w: S.Work, +s: S.St, +ws: List<&2, X.Waiter>, +now: Nat) -> WSt:
  match m:
    case Some{+a}:
      WSReply{a}
    case None{}:
      st.timer(timer_wait(S.Tms.live(S.St.tm(s)), S.Work.agents(w)), w, s, now)

def st.crash(m: Maybe<&2, String>, +w: S.Work, +s: S.St, +ws: List<&2, X.Waiter>, +now: Nat) -> WSt:
  match m:
    case Some{+a}:
      WSCrash{a, S.Work.last(w)}
    case None{}:
      st.reply(reply_wait(ws, S.Work.agents(w), s), w, s, ws, now)

def st.kind(k: S.WKind, +w: S.Work, +s: S.St, +ws: List<&2, X.Waiter>, +now: Nat) -> WSt:
  match k:
    case S.WOngoing{}:
      WSOngoing{}
    case _:
      st.crash(crashed(S.Work.agents(w), s), w, s, ws, now)

def st.working(m: Maybe<&2, String>, +w: S.Work, +s: S.St, +ws: List<&2, X.Waiter>, +now: Nat) -> WSt:
  match m:
    case Some{+a}:
      WSWorking{a}
    case None{}:
      st.kind(S.Work.kind(w), w, s, ws, now)

def st.needs(n: Nat, +w: S.Work, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +now: Nat) -> WSt:
  match n:
    case 1n+p:
      WSNeeds{1n+p}
    case 0n:
      st.working(working(S.Work.agents(w), s, rts, S.Work.id(w)), w, s, ws, now)

def st.paused(m: Maybe<&2, Nat>, +w: S.Work, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +n: Nat, +now: Nat) -> WSt:
  match m:
    case Some{_}:
      WSPaused{S.Work.until(w)}
    case None{}:
      st.needs(n, w, s, rts, ws, now)

# the status of work `w`, `n` his open cards and page waits on it (law
# status_is_a_function_of_facts: these arguments and nothing else)
def st.done(m: Maybe<&2, S.Fin>, +w: S.Work, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +n: Nat, +now: Nat) -> WSt:
  match m:
    case Some{_}:
      WSDone{}
    case None{}:
      st.paused(S.Work.paused(w), w, s, rts, ws, n, now)

def status(+w: S.Work, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +n: Nat, +now: Nat) -> WSt:
  st.done(S.Work.done(w), w, s, rts, ws, n, now)

# ---- the row ----

def word(x: WSt) -> String:
  match x:
    case WSDone{}:
      "done"
    case WSPaused{_}:
      "paused"
    case WSNeeds{_}:
      "needs_you"
    case WSWorking{_}:
      "working"
    case WSOngoing{}:
      "ongoing"
    case WSCrash{_, _}:
      "stalled"
    case WSReply{_}:
      "waiting"
    case WSTimer{_}:
      "waiting"
    case WSWatch{_}:
      "waiting"
    case WSReview{}:
      "to_review"
    case WSQuiet{_, +stalled}:
      Bool.pick(String, stalled, "stalled", "waiting")

def until_why(m: Maybe<&2, Nat>) -> String:
  match m:
    case Some{_}:
      "until"
    case None{}:
      ""

def row.go(x: WSt, +w: J.Json, +n: Nat, +cs: List<&2, Nat>) -> J.Json:
  match x:
    case WSPaused{+until}:
      V.work(w, "paused", until_why(until), "", until, n, cs)
    case WSWorking{+a}:
      V.work(w, "working", "agent", a, None{}, n, cs)
    case WSCrash{+a, +at}:
      V.work(w, "stalled", "crash", a, Some{at}, n, cs)
    case WSReply{+a}:
      V.work(w, "waiting", "reply", a, None{}, n, cs)
    case WSTimer{+at}:
      V.work(w, "waiting", "timer", "", Some{at}, n, cs)
    case WSWatch{+a}:
      V.work(w, "waiting", "watch", a, None{}, n, cs)
    case WSQuiet{+since, +stalled}:
      V.work(w, Bool.pick(String, stalled, "stalled", "waiting"), "quiet", "", Some{since}, n, cs)
    case other:
      V.work(w, word(other), "", "", None{}, n, cs)

# work `w`'s view row
def row(+w: S.Work, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +pw: List<&2, X.Kn>, +now: Nat) -> J.Json:
  +cs = cards_of(S.St.cards(s), S.Work.id(w))
  +n = Nat.add(List.length(&2, Nat, cs), KY.kn_get(pw, S.Work.id(w)))
  row.go(status(w, s, rts, ws, n, now), SC.Work.journal_json(w), n, cs)
