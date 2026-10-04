# hub/timers.bend — the timers of `sb every` (docs/ambient-roadmap.md B,
# standing orders) in sb-core's durable state: their records, how each of
# their journal lines changes them (`every_set`, `every_fired`,
# `every_run`, `every_stop`, applied by model.bend `apply`), the pure
# decisions over a record (is it due, why it ends, may it fire, its next
# time) and the view's JSON of them.
#
# Owns: the live timers (by id), the ended ones (at most ENDED_KEPT, for
# memory: the views show a week of them), the last id. `page` is an
# opaque pass-through (ambient-app's `sb every --page`).
# Not here: the steps that send, emit and ask (core.bend's `---- timers
# ----` section: they call send, pump and ask, which are core's, and
# Bend has no import cycles: this module is imported by model.bend, so
# it imports neither model nor core); the clock (Rust: the tick's `now`,
# every::next_daily for a daily timer); the words (Rust every.rs: labels,
# the list, the wake texts). The laws: LAWS.bend fire_is_a_message,
# wake_never_stacked, times_bound, stop_leaves_no_timer.
import Base

import ../vendor/json.bend as J

# when a timer fires: every `ms`, or every day at that minute (local time,
# the hub's next_daily)
type Sched is Data:
  Every{ms: Nat}
  Daily{min: Nat}

# `next`: when it is due; `last`: when it last woke its agent (0: never);
# `runs`: its last runs (at most RUNS_KEPT, oldest first)
type Timer is Data:
  Timer{id: Nat, agent: String, by: String, text: String, sched: Sched,
    next: Nat, until: Maybe<&2, Nat>, times: Maybe<&2, Nat>, fired: Nat,
    page: Maybe<&2, String>, last: Nat, runs: List<&2, Nat>}

# a timer that ended: when (0: an old line without it) and why
type Ended is Data:
  Ended{t: Timer, at: Nat, why: String}

type Tms is Data:
  Tms{live: List<&2, Timer>, ended: List<&2, Ended>, last_id: Nat}

def RUNS_KEPT() -> Nat:
  5n

def ENDED_KEPT() -> Nat:
  200n

def MIN_MS() -> Nat:
  60000n

def empty() -> Tms:
  Tms{Nil{}, Nil{}, 0n}

# ---- fields ----

def Timer.id(x: Timer) -> Nat:
  match x:
    case Timer{+id, _, _, _, _, _, _, _, _, _, _, _}:
      id

def Timer.agent(x: Timer) -> String:
  match x:
    case Timer{_, +agent, _, _, _, _, _, _, _, _, _, _}:
      agent

def Timer.by(x: Timer) -> String:
  match x:
    case Timer{_, _, +by, _, _, _, _, _, _, _, _, _}:
      by

def Timer.text(x: Timer) -> String:
  match x:
    case Timer{_, _, _, +text, _, _, _, _, _, _, _, _}:
      text

def Timer.sched(x: Timer) -> Sched:
  match x:
    case Timer{_, _, _, _, +sched, _, _, _, _, _, _, _}:
      sched

def Timer.next(x: Timer) -> Nat:
  match x:
    case Timer{_, _, _, _, _, +next, _, _, _, _, _, _}:
      next

def Timer.until(x: Timer) -> Maybe<&2, Nat>:
  match x:
    case Timer{_, _, _, _, _, _, +until, _, _, _, _, _}:
      until

def Timer.times(x: Timer) -> Maybe<&2, Nat>:
  match x:
    case Timer{_, _, _, _, _, _, _, +times, _, _, _, _}:
      times

def Timer.fired(x: Timer) -> Nat:
  match x:
    case Timer{_, _, _, _, _, _, _, _, +fired, _, _, _}:
      fired

def Timer.page(x: Timer) -> Maybe<&2, String>:
  match x:
    case Timer{_, _, _, _, _, _, _, _, _, +page, _, _}:
      page

def Timer.last(x: Timer) -> Nat:
  match x:
    case Timer{_, _, _, _, _, _, _, _, _, _, +last, _}:
      last

def Timer.runs(x: Timer) -> List<&2, Nat>:
  match x:
    case Timer{_, _, _, _, _, _, _, _, _, _, _, +runs}:
      runs

def Tms.live(x: Tms) -> List<&2, Timer>:
  match x:
    case Tms{+live, _, _}:
      live

def Tms.ended(x: Tms) -> List<&2, Ended>:
  match x:
    case Tms{_, +ended, _}:
      ended

def Tms.last_id(x: Tms) -> Nat:
  match x:
    case Tms{_, _, +last_id}:
      last_id

# ---- a timer's runs ----

def last_is(m: Maybe<&2, Nat>, +n: Nat) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+x}:
      Nat.is_eq(x, n)

# the last RUNS_KEPT of `xs`
def keep_last(+xs: List<&2, Nat>) -> List<&2, Nat>:
  List.drop(&2, Nat, xs, Nat.sub(List.length(&2, Nat, xs), RUNS_KEPT()))

# a run at `at` (0, or the same as the last one: nothing)
def runs_add(+rs: List<&2, Nat>, +at: Nat) -> List<&2, Nat>:
  Bool.pick(List<&2, Nat>, Nat.is_eq(at, 0n) || last_is(List.last(&2, Nat, rs), at), rs,
    keep_last(List.append(&2, Nat, rs, [at])))

# an old `undelivered` line: its lost wake was the last run, not a run
def runs_unrun(+rs: List<&2, Nat>, +last: Nat) -> List<&2, Nat>:
  Bool.pick(List<&2, Nat>, last_is(List.last(&2, Nat, rs), last),
    List.take(&2, Nat, rs, Nat.sub(List.length(&2, Nat, rs), 1n)), rs)

# ---- one record, one line ----

# `every_fired`: its count, its next time, when it ran
def Timer.fire(x: Timer, +fired: Nat, +next: Nat, +at: Nat, +undelivered: Bool) -> Timer:
  match x:
    case Timer{+id, +agent, +by, +text, +sched, _, +until, +times, _, +page, _, +runs}:
      Timer{id, agent, by, text, sched, next, until, times, fired, page, at,
        Bool.pick(List<&2, Nat>, undelivered, runs_unrun(runs, at), runs_add(runs, at))}

# `every_run`: a run now, outside the count, the next wake unchanged
def Timer.run(x: Timer, +at: Nat) -> Timer:
  match x:
    case Timer{+id, +agent, +by, +text, +sched, +next, +until, +times, +fired, +page, _, +runs}:
      Timer{id, agent, by, text, sched, next, until, times, fired, page, at, runs_add(runs, at)}

# ---- the live list (sorted by id, like `sb every` lists them) ----

def find(xs: List<&2, Timer>, +id: Nat) -> Maybe<&2, Timer>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      Bool.pick(Maybe<&2, Timer>, Nat.is_eq(Timer.id(h), id), Some{h}, find(t, id))

# replace the timer of the same id, else insert it in id order
def put(xs: List<&2, Timer>, +x: Timer) -> List<&2, Timer>:
  match xs:
    case Nil{}:
      [x]
    case Con{+h, +t}:
      Bool.pick(List<&2, Timer>, Nat.is_eq(Timer.id(h), Timer.id(x)), x <> t,
        Bool.pick(List<&2, Timer>, Nat.is_lt(Timer.id(x), Timer.id(h)), x <> h <> t, h <> put(t, x)))

# every timer but those of id `id`
def drop(xs: List<&2, Timer>, +id: Nat) -> List<&2, Timer>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, Timer>, Nat.is_eq(Timer.id(h), id), drop(t, id), h <> drop(t, id))

def fire_in(xs: List<&2, Timer>, +id: Nat, +fired: Nat, +next: Nat, +at: Nat, +undelivered: Bool) -> List<&2, Timer>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(Timer, Nat.is_eq(Timer.id(h), id), Timer.fire(h, fired, next, at, undelivered), h)
        <> fire_in(t, id, fired, next, at, undelivered)

def run_in(xs: List<&2, Timer>, +id: Nat, +at: Nat) -> List<&2, Timer>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(Timer, Nat.is_eq(Timer.id(h), id), Timer.run(h, at), h) <> run_in(t, id, at)

def str_in(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      String.eq(h, s) || str_in(t, s)

# the ids of the timers whose agent is one of `ns` (an agent's names: its
# own and its old ones)
def ids_in(xs: List<&2, Timer>, +ns: List<&2, String>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, Nat>, str_in(ns, Timer.agent(h)), Timer.id(h) <> ids_in(t, ns), ids_in(t, ns))

# ---- for the law stop_leaves_no_timer (PROOF.bend) ----

def nat_in(xs: List<&2, Nat>, +n: Nat) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Nat.is_eq(n, h) || nat_in(t, n)

# the timers whose id is not in `ids`
def keep_out(xs: List<&2, Timer>, +ids: List<&2, Nat>) -> List<&2, Timer>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, Timer>, nat_in(ids, Timer.id(h)), keep_out(t, ids), h <> keep_out(t, ids))

# every id of `xs` is in `ys`
def all_in(xs: List<&2, Nat>, +ys: List<&2, Nat>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      nat_in(ys, h) && all_in(t, ys)

# ---- the state's changes, one per journal line ----

def set(+tm: Tms, +x: Timer) -> Tms:
  match tm:
    case Tms{+live, +ended, +last}:
      Tms{put(live, x), ended, Nat.max(last, Timer.id(x))}

def fired(+tm: Tms, +id: Nat, +fired: Nat, +next: Nat, +at: Nat, +undelivered: Bool) -> Tms:
  match tm:
    case Tms{+live, +ended, +last}:
      Tms{fire_in(live, id, fired, next, at, undelivered), ended, last}

def run(+tm: Tms, +id: Nat, +at: Nat) -> Tms:
  match tm:
    case Tms{+live, +ended, +last}:
      Tms{run_in(live, id, at), ended, last}

def cap(+xs: List<&2, Ended>) -> List<&2, Ended>:
  List.drop(&2, Ended, xs, Nat.sub(List.length(&2, Ended, xs), ENDED_KEPT()))

def ended_add(m: Maybe<&2, Timer>, +xs: List<&2, Ended>, +at: Nat, +why: String) -> List<&2, Ended>:
  match m:
    case None{}:
      xs
    case Some{+x}:
      cap(List.append(&2, Ended, xs, [Ended{x, at, why}]))

# a stop: the timer leaves the live list (whatever was there), and joins
# the ended ones when it was live
def stop(+tm: Tms, +id: Nat, +why: String, +at: Nat) -> Tms:
  match tm:
    case Tms{+live, +ended, +last}:
      Tms{drop(live, id), ended_add(find(live, id), ended, at, why), last}

# ---- the pure decisions (core.bend's timers section wires them) ----

def opt_le(m: Maybe<&2, Nat>, +now: Nat) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+u}:
      Nat.is_le(u, now)

# its end time passed
def past_until(+x: Timer, +now: Nat) -> Bool:
  opt_le(Timer.until(x), now)

def is_due(+x: Timer, +now: Nat) -> Bool:
  Nat.is_le(Timer.next(x), now)

# something to look at in a tick: a timer due or past its end. A tick
# where none is (the common case) does nothing at all (idle-cpu, 51051c52)
def any_work(xs: List<&2, Timer>, +now: Nat) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      is_due(h, now) || past_until(h, now) || any_work(t, now)

# why a timer ends at a tick (None: it does not): its agent is gone
# (unknown or archived), or its end time passed
def end_why(+x: Timer, +gone: Bool, +now: Nat) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, gone, Some{String.concat(["@", Timer.agent(x), " is gone"])},
    Bool.pick(Maybe<&2, String>, past_until(x, now), Some{"its end time passed"}, None{}))

def under.go(m: Maybe<&2, Nat>, +fired: Nat) -> Bool:
  match m:
    case None{}:
      True{}
    case Some{+n}:
      Nat.is_lt(fired, n)

# below its times (no times: always)
def under(+x: Timer) -> Bool:
  under.go(Timer.times(x), Timer.fired(x))

def spent.go(m: Maybe<&2, Nat>, +fired: Nat) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+n}:
      Nat.is_le(n, 1n+fired)

# the fire about to be counted is its last one (it ran its times)
def spent(+x: Timer) -> Bool:
  spent.go(Timer.times(x), Timer.fired(x))

def may_fire.go(due: Bool, +x: Timer, +ready: Bool, +now: Nat) -> Bool:
  match due:
    case False{}:
      True{}
    case True{}:
      ready && is_due(x, now) && under(x)

# the guard of every wake (law wake_never_stacked): its agent has no wake
# from bise still queued; a due wake also needs the timer due and under
# its times, and its agent active and idle (a busy one: at its next idle)
def may_fire(+bise_queued: Bool, +due: Bool, +x: Timer, +ready: Bool, +now: Nat) -> Bool:
  Bool.not(bise_queued) && may_fire.go(due, x, ready, now)

# the next time of an every-N timer: a period after the due time, a
# missed period skipped (never caught up). A daily one: the hub's answer
# (every::next_daily, local time)
def next_every(+p: Nat, +from: Nat, +now: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(Nat.add(from, p), now), Nat.add(now, p), Nat.add(from, p))

def next_of(s: Sched, +from: Nat, +now: Nat, +daily: Nat) -> Nat:
  match s:
    case Every{+p}:
      next_every(p, from, now)
    case Daily{_}:
      daily

def is_daily(s: Sched) -> Bool:
  match s:
    case Every{_}:
      False{}
    case Daily{_}:
      True{}

# ---- JSON (the journal lines' fields, the view) ----

def opt_kv(+k: String, m: Maybe<&2, Nat>) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match m:
    case None{}:
      Nil{}
    case Some{+n}:
      [J.kv(k, J.JNum{n})]

def opt_kv_s(+k: String, m: Maybe<&2, String>) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match m:
    case None{}:
      Nil{}
    case Some{+s}:
      [J.kv(k, J.JStr{s})]

def sched_kv(s: Sched) -> Sigma<&2, &2, String, _ => J.Json>:
  match s:
    case Every{+p}:
      J.kv("every_ms", J.JNum{p})
    case Daily{+m}:
      J.kv("daily_min", J.JNum{m})

# the optional fields of a timer (`every_set` and the view)
def opt_kvs(+x: Timer) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  List.concat(&2, Sigma<&2, &2, String, _ => J.Json>,
    [opt_kv("until_ms", Timer.until(x)), opt_kv("times", Timer.times(x)), opt_kv_s("page", Timer.page(x))])

def nums(xs: List<&2, Nat>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.JNum{h} <> nums(t)

def timer_kvs(+x: Timer) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  List.append(&2, Sigma<&2, &2, String, _ => J.Json>,
    [J.kv("id", J.JNum{Timer.id(x)}), J.kv("agent", J.JStr{Timer.agent(x)}), J.kv("by", J.JStr{Timer.by(x)}),
      J.kv("text", J.JStr{Timer.text(x)}), sched_kv(Timer.sched(x)), J.kv("next_ms", J.JNum{Timer.next(x)}),
      J.kv("fired", J.JNum{Timer.fired(x)}), J.kv("last_ms", J.JNum{Timer.last(x)}),
      J.kv("runs", J.JArr{nums(Timer.runs(x))})],
    opt_kvs(x))

def timer_json(+x: Timer) -> J.Json:
  J.obj(timer_kvs(x))

def ended_json(e: Ended) -> J.Json:
  match e:
    case Ended{+x, +at, +why}:
      J.obj(List.append(&2, Sigma<&2, &2, String, _ => J.Json>, timer_kvs(x),
        [J.kv("ended_ms", J.JNum{at}), J.kv("why", J.JStr{why})]))

def live_json(xs: List<&2, Timer>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      timer_json(h) <> live_json(t)

def ended_list_json(xs: List<&2, Ended>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      ended_json(h) <> ended_list_json(t)
