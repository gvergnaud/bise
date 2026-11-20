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
import ./wakes.bend as WK

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
  WSMerged{}
  WSQuiet{since: Nat, stalled: Bool}

# ---- the facts, per work ----

# his pick on page tasks-fresh (architect m_66713, m_67541): every PR
# linked to work `id` is merged, at least one, and none open (a draft or
# changes requested count as open; closed-unmerged ones are not counted)
def merged(xs: List<&2, S.Kpr>, +id: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      LZ.pick(Bool, String.eq(S.Kpr.key(h), id),
        u => Nat.is_eq(S.Kpr.open_n(h), 0n) && Bool.not(Nat.is_eq(S.Kpr.merged_n(h), 0n)),
        u => merged(t, id))

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

# a wait counts for work `id` when its own works hold it (stamped from
# its setter's turn), never main's (architect m_67725: main routes; its
# own watch made 22 of his dashboard tasks read waiting on main, and a
# long-running agent's `sb every` every task it was once on)
def counts(+agent: String, ws: List<&2, String>, +id: String) -> Bool:
  LZ.and(Bool.not(String.eq(agent, M.MAIN())), u => WO.has(ws, id))

def msg_works(m: Maybe<&2, S.Msg>) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{+x}:
      S.Msg.works(x)

# who a wait for work `id` waits on (an `sb wait`, `sb ask` or open
# --expect-reply send whose message carries `id`)
def reply_wait(ws: List<&2, X.Waiter>, +s: S.St, +id: String) -> Maybe<&2, String>:
  match ws:
    case Nil{}:
      None{}
    case Con{+w, +t}:
      +m = M.msg_of(s, X.Waiter.msg(w))
      LZ.pick(Maybe<&2, String>, counts(X.Waiter.agent(w), msg_works(m), id), u => to_of(m), u => reply_wait(t, s, id))

# when a timer set for work `id` fires next
def timer_wait(ts: List<&2, S.Timer>, +id: String) -> Maybe<&2, Nat>:
  match ts:
    case Nil{}:
      None{}
    case Con{+x, +t}:
      LZ.pick(Maybe<&2, Nat>, counts(S.Timer.agent(x), S.Timer.works(x), id), u => Some{S.Timer.next(x)}, u => timer_wait(t, id))

# a watch set for work `id` (`sb wake`, or the reply an `sb ask` past its
# timeout still waits for)
def watch_wait(ws: List<&2, S.Watch>, +id: String) -> Maybe<&2, S.Watch>:
  match ws:
    case Nil{}:
      None{}
    case Con{+x, +t}:
      LZ.pick(Maybe<&2, S.Watch>, counts(S.Watch.agent(x), S.Watch.works(x), id), u => Some{x}, u => watch_wait(t, id))

def decl_done(m: Maybe<&2, S.Decl>) -> Bool:
  match m:
    case Some{S.DDone{}}:
      True{}
    case _:
      False{}

def life_archived(l: S.Life) -> Bool:
  match l:
    case S.LArchived{}:
      True{}
    case _:
      False{}

# an agent gone from the hub or archived counts as done
def said_done(m: Maybe<&2, S.Agent>) -> Bool:
  match m:
    case Some{+a}:
      LZ.or(life_archived(S.Agent.life(a)), u => decl_done(S.Agent.declared(a)))
    case None{}:
      True{}

# every agent of `names` said done (an empty list: none did)
def all_done.go(ns: List<&2, String>, +s: S.St) -> Bool:
  match ns:
    case Nil{}:
      True{}
    case Con{+n, +t}:
      LZ.and(said_done(M.get_agent(s, n)), u => all_done.go(t, s))

def all_done.of(ns: List<&2, String>, +s: S.St) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case Con{+n, +t}:
      all_done.go(n <> t, s)

# an agent still on work `id`: gone or archived (it counts as done), or
# its latest turn serves `id`; one that moved on (its latest turn serves
# other works) no longer holds it open (architect m_67725: vb-review,
# steered into 14 of his tasks, held them all)
def current(m: Maybe<&2, S.Agent>, +id: String) -> Bool:
  match m:
    case None{}:
      True{}
    case Some{+a}:
      LZ.or(life_archived(S.Agent.life(a)), u => WO.has(S.Agent.turn_works(a), id))

# its current doers: main routes and never says done (a work main made
# or was told of has main among its agents), so main never holds it out
# of To review (tasks-qa: a live hub never read To review once main was
# in); an agent that moved on is not one
def doers(ns: List<&2, String>, +s: S.St, +id: String) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      +rest = doers(t, s, id)
      LZ.pick(List<&2, String>, LZ.and(Bool.not(String.eq(n, M.MAIN())), u => current(M.get_agent(s, n), id)), u => n <> rest, u => rest)

# every current doer of work `id` said done (none: none did)
def all_done(ns: List<&2, String>, +s: S.St, +id: String) -> Bool:
  all_done.of(doers(ns, s, id), s)

# ---- the status, first fact that holds ----

def quiet(+w: S.Work, +now: Nat) -> WSt:
  WSQuiet{S.Work.last(w), Nat.is_ge(now, Nat.add(S.Work.last(w), STALL_MS()))}

def st.review(r: Bool, +w: S.Work, +now: Nat) -> WSt:
  match r:
    case True{}:
      WSReview{}
    case False{}:
      quiet(w, now)

# a reply watch names who it waits on (tasks-qa: an `sb ask` past its
# timeout read "quiet"), any other watch its agent
def st.watch.of(+x: S.Watch) -> WSt:
  Bool.pick(WSt, WK.is_reply(x), WSReply{WK.reply_to_of(x)}, WSWatch{S.Watch.agent(x)})

def st.watch(m: Maybe<&2, S.Watch>, +w: S.Work, +s: S.St, +now: Nat) -> WSt:
  match m:
    case Some{+x}:
      st.watch.of(x)
    case None{}:
      st.review(all_done(S.Work.agents(w), s, S.Work.id(w)), w, now)

def st.timer(m: Maybe<&2, Nat>, +w: S.Work, +s: S.St, +now: Nat) -> WSt:
  match m:
    case Some{+at}:
      WSTimer{at}
    case None{}:
      st.watch(watch_wait(S.Wakes.live(S.Tms.wk(S.St.tm(s))), S.Work.id(w)), w, s, now)

def st.reply(m: Maybe<&2, String>, +w: S.Work, +s: S.St, +ws: List<&2, X.Waiter>, +now: Nat) -> WSt:
  match m:
    case Some{+a}:
      WSReply{a}
    case None{}:
      st.timer(timer_wait(S.Tms.live(S.St.tm(s)), S.Work.id(w)), w, s, now)

def st.crash(m: Maybe<&2, String>, +w: S.Work, +s: S.St, +ws: List<&2, X.Waiter>, +now: Nat) -> WSt:
  match m:
    case Some{+a}:
      WSCrash{a, S.Work.last(w)}
    case None{}:
      st.reply(reply_wait(ws, s, S.Work.id(w)), w, s, ws, now)

# every linked PR merged: To review, 'merged' (law
# all_merged_reads_merged_never_done: never done, done is his word),
# after his cards and a live turn (architect m_67541)
def st.merged(m: Bool, +w: S.Work, +s: S.St, +ws: List<&2, X.Waiter>, +now: Nat) -> WSt:
  match m:
    case True{}:
      WSMerged{}
    case False{}:
      st.crash(crashed(S.Work.agents(w), s), w, s, ws, now)

def st.kind(k: S.WKind, +w: S.Work, +s: S.St, +ws: List<&2, X.Waiter>, +prs: List<&2, S.Kpr>, +now: Nat) -> WSt:
  match k:
    case S.WOngoing{}:
      WSOngoing{}
    case _:
      st.merged(merged(prs, S.Work.id(w)), w, s, ws, now)

def st.working(m: Maybe<&2, String>, +w: S.Work, +s: S.St, +ws: List<&2, X.Waiter>, +prs: List<&2, S.Kpr>, +now: Nat) -> WSt:
  match m:
    case Some{+a}:
      WSWorking{a}
    case None{}:
      st.kind(S.Work.kind(w), w, s, ws, prs, now)

def st.needs(n: Nat, +w: S.Work, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +prs: List<&2, S.Kpr>, +now: Nat) -> WSt:
  match n:
    case 1n+p:
      WSNeeds{1n+p}
    case 0n:
      st.working(working(S.Work.agents(w), s, rts, S.Work.id(w)), w, s, ws, prs, now)

def st.paused(m: Maybe<&2, Nat>, +w: S.Work, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +n: Nat, +prs: List<&2, S.Kpr>, +now: Nat) -> WSt:
  match m:
    case Some{_}:
      WSPaused{S.Work.until(w)}
    case None{}:
      st.needs(n, w, s, rts, ws, prs, now)

# the status of work `w`, `n` his open cards and page waits on it, `prs`
# the linked PRs' counts (law status_is_a_function_of_facts: these
# arguments and nothing else)
def st.done(m: Maybe<&2, S.Fin>, +w: S.Work, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +n: Nat, +prs: List<&2, S.Kpr>, +now: Nat) -> WSt:
  match m:
    case Some{_}:
      WSDone{}
    case None{}:
      st.paused(S.Work.paused(w), w, s, rts, ws, n, prs, now)

def status(+w: S.Work, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +n: Nat, +prs: List<&2, S.Kpr>, +now: Nat) -> WSt:
  st.done(S.Work.done(w), w, s, rts, ws, n, prs, now)

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
    case WSMerged{}:
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
    case WSMerged{}:
      V.work(w, "to_review", "merged", "", None{}, n, cs)
    case WSReview{}:
      V.work(w, "to_review", "finished", "", None{}, n, cs)
    case other:
      V.work(w, word(other), "", "", None{}, n, cs)

# work `w`'s view row
def row(+w: S.Work, +s: S.St, +rts: List<&2, X.Rt>, +ws: List<&2, X.Waiter>, +pw: List<&2, X.Kn>, +prs: List<&2, S.Kpr>, +now: Nat) -> J.Json:
  +cs = cards_of(S.St.cards(s), S.Work.id(w))
  +n = Nat.add(List.length(&2, Nat, cs), KY.kn_get(pw, S.Work.id(w)))
  row.go(status(w, s, rts, ws, n, prs, now), SC.Work.journal_json(w), n, cs)
