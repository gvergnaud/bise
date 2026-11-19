# hub/work.bend — tasks (page tasks-center; architect m_64423, m_64425):
# a "task" in the UI and the sb verbs is a Work here (a "task" in the
# hub's code is an agent: task_created, create_task). The pure part of
# the works sb-core keeps in St.works: find one, put one, apply one
# change (`work_changed`'s WOp), an agent's turn's works (work_turn),
# a done work folded to a tomb. model.bend's apply calls these, nothing
# here decides who is woken or told: core.bend's work section does.
import Base

import ./lazy.bend as LZ
import ./gen/state.bend as S

# ---- lists of names ----

def has(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      LZ.or(String.eq(h, s), u => has(t, s))

# `s` at the end of `xs`, unless it is there already
def add(xs: List<&2, String>, +s: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      [s]
    case Con{+h, +t}:
      LZ.pick(List<&2, String>, String.eq(h, s), u => Con{h, t}, u => h <> add(t, s))

# `xs` without `s` (his "Not on this task", WRemoveAgent)
def drop(xs: List<&2, String>, +s: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = drop(t, s)
      Bool.pick(List<&2, String>, String.eq(h, s), rest, h <> rest)

# every name of `ys` added to `xs`, in their order, once each
def union.go(ys: List<&2, String>, xs: List<&2, String>) -> List<&2, String>:
  match ys:
    case Nil{}:
      xs
    case Con{+h, +t}:
      union.go(t, add(xs, h))

def union(xs: List<&2, String>, ys: List<&2, String>) -> List<&2, String>:
  union.go(ys, xs)

# every name of `xs` is in `ys`
def within(xs: List<&2, String>, +ys: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      LZ.and(has(ys, h), u => within(t, ys))

# the same names, whatever their order
def same(+xs: List<&2, String>, +ys: List<&2, String>) -> Bool:
  LZ.and(within(xs, ys), u => within(ys, xs))

# ---- the works ----

def find(xs: List<&2, S.Work>, +id: String) -> Maybe<&2, S.Work>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      LZ.pick(Maybe<&2, S.Work>, String.eq(S.Work.id(h), id), u => Some{h}, u => find(t, id))

# `w` in the place of the work with its id, else at the end
def put(xs: List<&2, S.Work>, +w: S.Work) -> List<&2, S.Work>:
  match xs:
    case Nil{}:
      [w]
    case Con{+h, +t}:
      LZ.pick(List<&2, S.Work>, String.eq(S.Work.id(h), S.Work.id(w)), u => w <> t, u => h <> put(t, w))

def del(xs: List<&2, S.Work>, +id: String) -> List<&2, S.Work>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      LZ.pick(List<&2, S.Work>, String.eq(S.Work.id(h), id), u => t, u => h <> del(t, id))

# ---- one change ----

def goal(+text: String) -> S.Goal:
  S.Goal.with_text(S.Goal.empty(), text)

# done-when item `n` (1 the first; 0 none) ticked or not, by `by`
def tick.set(+g: S.Goal, +done: Bool, +by: String) -> S.Goal:
  S.Goal.with_by(S.Goal.with_done(g, done), Bool.pick(String, done, by, ""))

def tick(n: Nat, gs: List<&2, S.Goal>, +done: Bool, +by: String) -> List<&2, S.Goal>:
  match n gs:
    case _ Nil{}:
      Nil{}
    case 0n Con{+g, +t}:
      g <> t
    case 1n+0n Con{+g, +t}:
      tick.set(g, done, by) <> t
    case 1n+1n+p Con{+g, +t}:
      g <> tick(1n+p, t, done, by)

def fin(+at: Nat, +by: String) -> S.Fin:
  S.Fin.with_by(S.Fin.with_at(S.Fin.empty(), at), by)

def opt(+n: Nat) -> Maybe<&2, Nat>:
  match n:
    case 0n:
      None{}
    case 1n+p:
      Some{1n+p}

# a closed work holds nothing: done or merged, its pause ends with it,
# so the messages it held go (law a_closed_work_holds_nothing; architect
# m_64704)
def unpaused(+w: S.Work) -> S.Work:
  S.Work.with_paused(S.Work.with_until(S.Work.with_until_timer(w, None{}), None{}), None{})

# `w` after the change `op` made by `by` at `at` (a rename by him names
# it for good; a pause keeps when it began and its end; a done or a
# reopen; a merge marks it done into its target)
def change.op(op: S.WOp, +w2: S.Work, +by: String, +at: Nat) -> S.Work:
  match op:
    case S.WRename{+name}:
      S.Work.with_named_by_you(S.Work.with_name(w2, name), S.Work.named_by_you(w2) || String.eq(by, "user"))
    case S.WWhy{+why}:
      S.Work.with_why(w2, why)
    case S.WGoal{+text}:
      S.Work.over_goals(w2, gs => List.append(&2, S.Goal, gs, [goal(text)]))
    case S.WTick{+n}:
      S.Work.over_goals(w2, gs => tick(n, gs, True{}, by))
    case S.WUntick{+n}:
      S.Work.over_goals(w2, gs => tick(n, gs, False{}, by))
    case S.WMakeOngoing{}:
      S.Work.with_kind(w2, S.WOngoing{})
    case S.WPause{+until}:
      S.Work.with_until(S.Work.with_paused(w2, Some{at}), opt(until))
    case S.WResume{}:
      S.Work.with_until_timer(S.Work.with_until(S.Work.with_paused(w2, None{}), None{}), None{})
    case S.WDone{}:
      unpaused(S.Work.with_done(w2, Some{fin(at, by)}))
    case S.WReopen{}:
      S.Work.with_done(w2, None{})
    case S.WMerge{+into}:
      unpaused(S.Work.with_done(S.Work.with_merged_into(w2, Some{into}), Some{fin(at, by)}))
    case S.WAddAgent{+agent}:
      S.Work.over_agents(w2, xs => add(xs, agent))
    case S.WRemoveAgent{+agent}:
      S.Work.over_agents(w2, xs => drop(xs, agent))
    case S.WUntilTimer{+timer}:
      S.Work.with_until_timer(w2, opt(timer))
    # a Restart changes no field: its effects are its agents' (core.bend
    # req_task.after; law work_restart_changes_no_field)
    case S.WRestart{}:
      w2
    case S.WNoop{}:
      w2

def change(+w: S.Work, op: S.WOp, +by: String, +at: Nat) -> S.Work:
  change.op(op, S.Work.with_last(w, Nat.max(S.Work.last(w), at)), by, at)

# work `m` (the one found) changed; none: the list as it is
def changed.at(m: Maybe<&2, S.Work>, +xs: List<&2, S.Work>, +op: S.WOp, +by: String, +at: Nat) -> List<&2, S.Work>:
  match m:
    case None{}:
      xs
    case Some{+w}:
      put(xs, change(w, op, by, at))

def changed(+xs: List<&2, S.Work>, +id: String, +op: S.WOp, +by: String, +at: Nat) -> List<&2, S.Work>:
  changed.at(find(xs, id), xs, op, by, at)

# ---- an agent's turn ----

# every work of `ids` served by `agent` at `at`: it joins its agents, and
# the work's last fact is then
def served.go(ids: List<&2, String>, +xs: List<&2, S.Work>, +agent: String, +at: Nat) -> List<&2, S.Work>:
  match ids:
    case Nil{}:
      xs
    case Con{+id, +t}:
      served.go(t, changed(xs, id, S.WAddAgent{agent}, agent, at), agent, at)

def served(+xs: List<&2, S.Work>, ids: List<&2, String>, +agent: String, +at: Nat) -> List<&2, S.Work>:
  served.go(ids, xs, agent, at)

# the work_turn line of `agent`'s turn at `at` (`add`: steered in)
def wturn(+agent: String, +ws: List<&2, String>, +add: Bool, +at: Nat) -> S.WTurn:
  S.WTurn.with_at(S.WTurn.with_add(S.WTurn.with_works(S.WTurn.with_agent(S.WTurn.empty(), agent), ws), add), at)

# an agent's turn's works after a work_turn: a steered delivery adds, a
# new turn sets
def turn_works(+was: List<&2, String>, +now: List<&2, String>, add: Bool) -> List<&2, String>:
  match add:
    case True{}:
      union(was, now)
    case False{}:
      now

# the works a delivered batch carries: every message's, once each
def batch_works(ms: List<&2, S.Msg>) -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case Con{+m, +t}:
      union(S.Msg.works(m), batch_works(t))

# a delivery to an agent whose turn carries `cur`, of a batch carrying
# `got`, `busy` its turn running (steered) or not (a new turn: set): the
# work_turn line to write, none when its turn's works stay the same. A
# steer keeps the turn's works (architect m_66621): a coordinator's turn
# never collects every task steered into it; the answer to a steer
# carries its own task by its reply_to. Old journals' steered lines
# (add) still union on replay: turn_works is unchanged.
def turn_change.go(kept: Bool, +got: List<&2, String>) -> Maybe<&2, List<&2, String>>:
  match kept:
    case True{}:
      None{}
    case False{}:
      Some{got}

# `agent` is on work `m` already (no such work: nothing to join)
def joined(m: Maybe<&2, S.Work>, +agent: String) -> Bool:
  match m:
    case None{}:
      True{}
    case Some{+w}:
      has(S.Work.agents(w), agent)

# the works of `ids` that `agent` is not on yet: a steered delivery still
# makes it their agent (an explicit add_agent line each), its turn's
# works unchanged (until joining by act, architect m_66646 point 3)
def not_joined(ids: List<&2, String>, +xs: List<&2, S.Work>, +agent: String) -> List<&2, String>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = not_joined(t, xs, agent)
      Bool.pick(List<&2, String>, joined(find(xs, h), agent), rest, h <> rest)

def turn_change(+cur: List<&2, String>, +got: List<&2, String>, busy: Bool) -> Maybe<&2, List<&2, String>>:
  match busy:
    case True{}:
      None{}
    case False{}:
      turn_change.go(same(got, cur), got)

# ---- a birth (`sb task new`) ----

def goals(xs: List<&2, String>) -> List<&2, S.Goal>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      goal(h) <> goals(t)

def tomb_has(ts: List<&2, S.Tomb>, +id: String) -> Bool:
  match ts:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      LZ.or(String.eq(S.Tomb.id(h), id), u => tomb_has(t, id))

def work_has(xs: List<&2, S.Work>, +id: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      LZ.or(String.eq(S.Work.id(h), id), u => work_has(t, id))

# an id a work or a tomb holds (a folded work's id stays reserved: law
# tomb_keeps_id_reserved)
def taken(+st: S.St, +id: String) -> Bool:
  LZ.or(work_has(S.St.works(st), id), u => tomb_has(S.St.tombs(st), id))

def free.id(+slug: String, +n: Nat) -> String:
  String.concat([slug, "-", Nat.show(n)])

# `slug-n` for the first n from `n` on that no work or tomb holds (`fuel`
# one more than their count: one of them is free)
def free.go(fuel: Nat, +st: S.St, +slug: String, +n: Nat) -> String:
  match fuel:
    case 0n:
      free.id(slug, n)
    case 1n+f:
      LZ.pick(String, taken(st, free.id(slug, n)), u => free.go(f, st, slug, 1n+n), u => free.id(slug, n))

# a new work's id: its slug, else the first free `slug-2`, `slug-3`...
# Its fuel (one more than the works and tombs) leaves one `slug-n` free
# among the tries: why the birth's guard (core.bend req_task.born, law
# work_birth_never_takes_a_taken_id) never refuses; the guard, not this
# count, is what keeps ids unique.
def free(+st: S.St, +slug: String) -> String:
  +fuel = Nat.add(1n, Nat.add(List.length(&2, S.Work, S.St.works(st)), List.length(&2, S.Tomb, S.St.tombs(st))))
  LZ.pick(String, taken(st, slug), u => free.go(fuel, st, slug, 2n), u => slug)

# the newest message of his to `agent`: where a work it opens comes from
def origin.newer(+best: Maybe<&2, Nat>, +id: Nat) -> Maybe<&2, Nat>:
  Some{Nat.max(Maybe.default(&2, Nat, best, 0n), id)}

def origin.go(rs: List<&2, S.MRec>, +agent: String, +best: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match rs:
    case Nil{}:
      best
    case Con{+r, +t}:
      +m = S.MRec.msg(r)
      +his = String.eq(S.Msg.from(m), "user") && String.eq(S.Msg.to(m), agent)
      origin.go(t, agent, LZ.pick(Maybe<&2, Nat>, his, u => origin.newer(best, S.Msg.id(m)), u => best))

def origin(+st: S.St, +agent: String) -> Maybe<&2, Nat>:
  origin.go(S.St.old(st), agent, origin.go(S.St.hot(st), agent, None{}))

# the work `sb task new` makes: `by` serves it from now
def born(+id: String, +name: String, +why: String, +gs: List<&2, String>, +origin: Maybe<&2, Nat>, +by: String, +at: Nat) -> S.Work:
  +w = S.Work.with_goals(S.Work.with_why(S.Work.with_name(S.Work.with_id(S.Work.empty(), id), name), why), goals(gs))
  S.Work.with_last(S.Work.with_agents(S.Work.with_created(S.Work.with_by(S.Work.with_origin(w, origin), by), at), [by]), at)

# his words that started it (roma idea 5, architect m_65544): the shell's
# `said` of message `msg` on `sb task new` (it reads the text, sb-core
# keeps none), stamped once at birth, only when `msg` is the work's own
# origin; else none
def with_said.is(m: Maybe<&2, Nat>, +n: Nat) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+x}:
      Nat.is_eq(x, n)

def with_said(+w: S.Work, +msg: Nat, +said: String) -> S.Work:
  +ok = with_said.is(S.Work.origin(w), msg)
  S.Work.with_said(w, Bool.pick(String, ok, String.trim(said), ""))

# ---- a merged id reads its target (law merge_redirects) ----

def target.of(m: Maybe<&2, S.Work>, +id: String) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{+w}:
      Some{Maybe.default(&2, String, S.Work.merged_into(w), id)}

# the live work an id names: itself, or the work it was merged into;
# none: no work (a tomb's id included)
def target(+xs: List<&2, S.Work>, +id: String) -> Maybe<&2, String>:
  target.of(find(xs, id), id)

# ---- a work's pause (architect m_64425 Q-c) ----

# 2 min: a busy turn whose works are all paused gets the note first; it
# is stopped only after this, if it still runs
def PAUSE_GRACE_MS() -> Nat:
  120000n

# when work `m` (the one found) was paused; none: not paused, or no work
def paused_at(m: Maybe<&2, S.Work>) -> Maybe<&2, Nat>:
  match m:
    case None{}:
      None{}
    case Some{+w}:
      S.Work.paused(w)

def is_paused(+xs: List<&2, S.Work>, +id: String) -> Bool:
  Maybe.is_some(&2, Nat, paused_at(find(xs, id)))

def every_paused(ids: List<&2, String>, +xs: List<&2, S.Work>) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      LZ.and(is_paused(xs, h), u => every_paused(t, xs))

# works `ids` all paused, and at least one: a message, a turn with no
# work is never a paused work's
def all_paused(ids: List<&2, String>, +xs: List<&2, S.Work>) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      every_paused(Con{h, t}, xs)

# a message held by its works' pause: it waits in the queue, in its
# order, until one of them resumes (law paused_work_holds_its_deliveries)
def held.by(him: Bool, ids: List<&2, String>, +xs: List<&2, S.Work>) -> Bool:
  match him:
    case True{}:
      False{}
    case False{}:
      all_paused(ids, xs)

# his own words are never held: they reach the agent, paused or not (his
# word beats the pause; law his_words_are_never_held, tasks-ux m_64703)
def held(+m: S.Msg, +xs: List<&2, S.Work>) -> Bool:
  held.by(String.eq(S.Msg.from(m), "user"), S.Msg.works(m), xs)

# a delivery's works read as the live ones they name: a merged work's
# held message serves its target (law merge_redirects)
def retarget.one(m: Maybe<&2, String>, +id: String) -> String:
  match m:
    case None{}:
      id
    case Some{+t}:
      t

def retarget.go(ids: List<&2, String>, +xs: List<&2, S.Work>, +acc: List<&2, String>) -> List<&2, String>:
  match ids:
    case Nil{}:
      acc
    case Con{+h, +t}:
      retarget.go(t, xs, add(acc, retarget.one(target(xs, h), h)))

# in their order, once each
def retarget(ids: List<&2, String>, +xs: List<&2, S.Work>) -> List<&2, String>:
  retarget.go(ids, xs, Nil{})

def unheld.pick(h: Bool, +m: S.Msg, +rest: List<&2, S.Msg>) -> List<&2, S.Msg>:
  match h:
    case True{}:
      rest
    case False{}:
      m <> rest

# the messages a delivery may take: the held ones stay queued
def unheld(ms: List<&2, S.Msg>, +xs: List<&2, S.Work>) -> List<&2, S.Msg>:
  match ms:
    case Nil{}:
      Nil{}
    case Con{+m, +t}:
      unheld.pick(held(m, xs), m, unheld(t, xs))

def grace.at(m: Maybe<&2, Nat>, +now: Nat) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+at}:
      Nat.is_ge(now, Nat.add(at, PAUSE_GRACE_MS()))

def grace.go(ids: List<&2, String>, +xs: List<&2, S.Work>, +now: Nat) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      LZ.and(grace.at(paused_at(find(xs, h)), now), u => grace.go(t, xs, now))

def stops.works(ids: List<&2, String>, +xs: List<&2, S.Work>, +now: Nat) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      grace.go(Con{h, t}, xs, now)

# a busy turn serving `ids` is stopped: every one of them paused for the
# grace at least. An agent that ended its turn itself, a turn with one
# open work or with none: never (laws
# pause_never_stops_an_agent_with_an_open_work,
# pause_never_stops_an_idle_agent, pause_interrupts_only_after_grace)
def stops(busy: Bool, ids: List<&2, String>, +xs: List<&2, S.Work>, +now: Nat) -> Bool:
  match busy:
    case False{}:
      False{}
    case True{}:
      stops.works(ids, xs, now)

# the works of `ids` not paused: what the turn goes on with
def open_of(ids: List<&2, String>, +xs: List<&2, S.Work>) -> List<&2, String>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = open_of(t, xs)
      Bool.pick(List<&2, String>, is_paused(xs, h), rest, h <> rest)

def any_paused(xs: List<&2, S.Work>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+w, +t}:
      LZ.or(Maybe.is_some(&2, Nat, S.Work.paused(w)), u => any_paused(t))

# a pause's until act resumes the work only while it is still paused
# with an end that came; a pause changed meanwhile leaves the old act
# without effect (law stale_until_never_resumes)
def until_due.go(p: Maybe<&2, Nat>, u: Maybe<&2, Nat>, +now: Nat) -> Bool:
  match p u:
    case Some{_} Some{+at}:
      Nat.is_ge(now, at)
    case _ _:
      False{}

def until_due(+w: S.Work, +now: Nat) -> Bool:
  until_due.go(S.Work.paused(w), S.Work.until(w), now)

# ---- the fold ----

# 7 days: a done work stays in the lists, then folds
def DONE_KEEP_MS() -> Nat:
  604800000n

def done_at(m: Maybe<&2, S.Fin>) -> Nat:
  match m:
    case None{}:
      0n
    case Some{+f}:
      S.Fin.at(f)

# a work folds DONE_KEEP_MS after its done; an open one never (law
# a_done_work_folds_after_seven_days)
def fold_due.go(m: Maybe<&2, S.Fin>, +now: Nat) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+f}:
      Nat.is_ge(now, Nat.add(S.Fin.at(f), DONE_KEEP_MS()))

# the ids of the works due to fold at `now`
def folds(xs: List<&2, S.Work>, +now: Nat) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      +rest = folds(t, now)
      Bool.pick(List<&2, String>, fold_due.go(S.Work.done(w), now), S.Work.id(w) <> rest, rest)

def tomb(+w: S.Work) -> S.Tomb:
  S.Tomb.with_merged_into(S.Tomb.with_done(S.Tomb.with_name(S.Tomb.with_id(S.Tomb.empty(), S.Work.id(w)), S.Work.name(w)), done_at(S.Work.done(w))), S.Work.merged_into(w))

# the tombs after work `m` (the one found) folded: its tomb first
def fold.tombs(m: Maybe<&2, S.Work>, +ts: List<&2, S.Tomb>) -> List<&2, S.Tomb>:
  match m:
    case None{}:
      ts
    case Some{+w}:
      tomb(w) <> ts

# work `id` folded: out of the works, its tomb first in the tombs (the
# state's shape stays plain: a getter of any other field computes)
def folded(+st: S.St, +id: String) -> S.St:
  S.St.with_tombs(S.St.with_works(st, del(S.St.works(st), id)), fold.tombs(find(S.St.works(st), id), S.St.tombs(st)))
