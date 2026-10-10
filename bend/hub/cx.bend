# hub/cx.bend — the step context of the hub: the durable state, the
# runtime bookkeeping and the effects of one input. `Core<o>` carries a
# proof that its state is the replay of its journal over `o`, the state
# before the input: every change of the durable state goes through
# `emit`, which extends the proof (law replay_reproduces).
import Base

import ../vendor/json.bend as J

import ./model.bend as M

type Fx is Data:
  FJ{ev: M.Ev}
  FO{o: J.Json}

type Rt is Data:
  Rt{name: String, run: M.Run, waiting: Bool, turn: Maybe<&2, Nat>, turns: Nat}

type Waiter is Data:
  Waiter{token: Nat, agent: String, msg: Nat, deadline: Nat}

# an agent's steer-mode deliveries this turn; `seen`: how many of `ids`
# a receipt (ISteered) already reported, so the next one reports only
# the new ones (seen <= len ids)
type Steer is Data:
  Steer{agent: String, ids: List<&2, Nat>, seen: Nat}

type Cnt is Data:
  Cnt{agent: String, recv: Nat, inj: Nat}

type Kv is Data:
  Kv{key: String, val: String}

type Kn is Data:
  Kn{key: String, n: Nat}

# the runtime part of the hub (never journaled) and the facts of the input
type R is Data:
  R{rts: List<&2, Rt>, waiters: List<&2, Waiter>, steered: List<&2, Steer>,
    counts: List<&2, Cnt>, last: List<&2, Kv>, crashes: List<&2, Kn>,
    workspace: String, dirty: Bool, now: Nat, git: Bool,
    ans: List<&2, J.Json>, need: Maybe<&2, J.Json>}

type Core<-o: M.St> is Data:
  Core{d: M.St, fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : M.St},
    f: Fresh(journal(fx), o), q: Quiet(journal(fx)), v: Deliv(journal(fx), o)}

type Cx<-o: M.St> is Data:
  Cx{c: Core<o>, r: R}

type Both<-A: Data, -B: Data> is Data:
  Both{a: A, b: B}

def Rt.name(x: Rt) -> String:
  match x:
    case Rt{+name, _, _, _, _}:
      name

def Rt.with_name(x: Rt, +v: String) -> Rt:
  match x:
    case Rt{_, +run, +waiting, +turn, +turns}:
      Rt{v, run, waiting, turn, turns}

def Rt.run(x: Rt) -> M.Run:
  match x:
    case Rt{_, +run, _, _, _}:
      run

def Rt.with_run(x: Rt, +v: M.Run) -> Rt:
  match x:
    case Rt{+name, _, +waiting, +turn, +turns}:
      Rt{name, v, waiting, turn, turns}

def Rt.waiting(x: Rt) -> Bool:
  match x:
    case Rt{_, _, +waiting, _, _}:
      waiting

def Rt.with_waiting(x: Rt, +v: Bool) -> Rt:
  match x:
    case Rt{+name, +run, _, +turn, +turns}:
      Rt{name, run, v, turn, turns}

def Rt.turn(x: Rt) -> Maybe<&2, Nat>:
  match x:
    case Rt{_, _, _, +turn, _}:
      turn

def Rt.with_turn(x: Rt, +v: Maybe<&2, Nat>) -> Rt:
  match x:
    case Rt{+name, +run, +waiting, _, +turns}:
      Rt{name, run, waiting, v, turns}

# its turns ended since sb-core started (issue 22): set only by set_rt
# (core.bend), the one writer of a runtime state, +1 each time its run
# leaves RBusy
def Rt.turns(x: Rt) -> Nat:
  match x:
    case Rt{_, _, _, _, +turns}:
      turns

def Rt.with_turns(x: Rt, +v: Nat) -> Rt:
  match x:
    case Rt{+name, +run, +waiting, +turn, _}:
      Rt{name, run, waiting, turn, v}

def Waiter.token(x: Waiter) -> Nat:
  match x:
    case Waiter{+token, _, _, _}:
      token

def Waiter.agent(x: Waiter) -> String:
  match x:
    case Waiter{_, +agent, _, _}:
      agent

def Waiter.with_agent(x: Waiter, +v: String) -> Waiter:
  match x:
    case Waiter{+token, _, +msg, +deadline}:
      Waiter{token, v, msg, deadline}

def Waiter.msg(x: Waiter) -> Nat:
  match x:
    case Waiter{_, _, +msg, _}:
      msg

def Waiter.deadline(x: Waiter) -> Nat:
  match x:
    case Waiter{_, _, _, +deadline}:
      deadline

def Steer.agent(x: Steer) -> String:
  match x:
    case Steer{+agent, _, _}:
      agent

def Steer.ids(x: Steer) -> List<&2, Nat>:
  match x:
    case Steer{_, +ids, _}:
      ids

def Steer.seen(x: Steer) -> Nat:
  match x:
    case Steer{_, _, +seen}:
      seen

def Cnt.agent(x: Cnt) -> String:
  match x:
    case Cnt{+agent, _, _}:
      agent

def Cnt.recv(x: Cnt) -> Nat:
  match x:
    case Cnt{_, +recv, _}:
      recv

def Cnt.inj(x: Cnt) -> Nat:
  match x:
    case Cnt{_, _, +inj}:
      inj

def Kv.key(x: Kv) -> String:
  match x:
    case Kv{+key, _}:
      key

def Kv.val(x: Kv) -> String:
  match x:
    case Kv{_, +val}:
      val

def Kn.key(x: Kn) -> String:
  match x:
    case Kn{+key, _}:
      key

def Kn.n(x: Kn) -> Nat:
  match x:
    case Kn{_, +n}:
      n

def R.rts(x: R) -> List<&2, Rt>:
  match x:
    case R{+rts, _, _, _, _, _, _, _, _, _, _, _}:
      rts

def R.with_rts(x: R, +v: List<&2, Rt>) -> R:
  match x:
    case R{_, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need}:
      R{v, waiters, steered, counts, last, crashes, workspace, dirty, now, git, ans, need}

def R.waiters(x: R) -> List<&2, Waiter>:
  match x:
    case R{_, +waiters, _, _, _, _, _, _, _, _, _, _}:
      waiters

def R.with_waiters(x: R, +v: List<&2, Waiter>) -> R:
  match x:
    case R{+rts, _, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need}:
      R{rts, v, steered, counts, last, crashes, workspace, dirty, now, git, ans, need}

def R.steered(x: R) -> List<&2, Steer>:
  match x:
    case R{_, _, +steered, _, _, _, _, _, _, _, _, _}:
      steered

def R.with_steered(x: R, +v: List<&2, Steer>) -> R:
  match x:
    case R{+rts, +waiters, _, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need}:
      R{rts, waiters, v, counts, last, crashes, workspace, dirty, now, git, ans, need}

def R.counts(x: R) -> List<&2, Cnt>:
  match x:
    case R{_, _, _, +counts, _, _, _, _, _, _, _, _}:
      counts

def R.with_counts(x: R, +v: List<&2, Cnt>) -> R:
  match x:
    case R{+rts, +waiters, +steered, _, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need}:
      R{rts, waiters, steered, v, last, crashes, workspace, dirty, now, git, ans, need}

def R.last(x: R) -> List<&2, Kv>:
  match x:
    case R{_, _, _, _, +last, _, _, _, _, _, _, _}:
      last

def R.with_last(x: R, +v: List<&2, Kv>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, _, +crashes, +workspace, +dirty, +now, +git, +ans, +need}:
      R{rts, waiters, steered, counts, v, crashes, workspace, dirty, now, git, ans, need}

def R.crashes(x: R) -> List<&2, Kn>:
  match x:
    case R{_, _, _, _, _, +crashes, _, _, _, _, _, _}:
      crashes

def R.with_crashes(x: R, +v: List<&2, Kn>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, _, +workspace, +dirty, +now, +git, +ans, +need}:
      R{rts, waiters, steered, counts, last, v, workspace, dirty, now, git, ans, need}

def R.workspace(x: R) -> String:
  match x:
    case R{_, _, _, _, _, _, +workspace, _, _, _, _, _}:
      workspace

def R.dirty(x: R) -> Bool:
  match x:
    case R{_, _, _, _, _, _, _, +dirty, _, _, _, _}:
      dirty

def R.with_dirty(x: R, +v: Bool) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, _, +now, +git, +ans, +need}:
      R{rts, waiters, steered, counts, last, crashes, workspace, v, now, git, ans, need}

def R.now(x: R) -> Nat:
  match x:
    case R{_, _, _, _, _, _, _, _, +now, _, _, _}:
      now

def R.with_now(x: R, +v: Nat) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, _, +git, +ans, +need}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, v, git, ans, need}

def R.git(x: R) -> Bool:
  match x:
    case R{_, _, _, _, _, _, _, _, _, +git, _, _}:
      git

def R.with_git(x: R, +v: Bool) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, _, +ans, +need}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, v, ans, need}

def R.ans(x: R) -> List<&2, J.Json>:
  match x:
    case R{_, _, _, _, _, _, _, _, _, _, +ans, _}:
      ans

def R.with_ans(x: R, +v: List<&2, J.Json>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, _, +need}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, git, v, need}

def R.need(x: R) -> Maybe<&2, J.Json>:
  match x:
    case R{_, _, _, _, _, _, _, _, _, _, _, +need}:
      need

def R.with_need(x: R, +v: Maybe<&2, J.Json>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, _}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, git, ans, v}

# the journal events among the effects, in order
def journal(fx: List<&2, Fx>) -> List<&2, M.Ev>:
  match fx:
    case Nil{}:
      Nil{}
    case Con{f, rest}:
      match f:
        case FJ{+e}:
          e <> journal(rest)
        case FO{_}:
          journal(rest)

# ---- what every journal satisfies (laws msg_ids_fresh, auto_never_asks) ----

# a new message (a new card) takes the next free id of the state it is
# created in: no id is ever reused, so nothing overwrites another; a new
# task or a new name takes a name nobody has (law L4: names stay apart)
def FreshEv(+s: M.St, e: M.Ev) -> Data:
  match e:
    case M.EMsgSent{+m}:
      {M.Msg.id(m) == M.St.next_msg(s) : Nat}
    case M.ECardOpened{+c}:
      {M.Card.id(c) == M.St.next_card(s) : Nat}
    case M.ETaskCreated{+name, _, _, _, _}:
      {M.name_taken(s, name) == False{} : Bool}
    case M.ERenamed{_, +new}:
      {M.name_taken(s, new) == False{} : Bool}
    case _:
      Unit

def Fresh(evs: List<&2, M.Ev>, +s: M.St) -> Data:
  match evs:
    case Nil{}:
      Unit
    case Con{+e, +rest}:
      Both<FreshEv(s, e), Fresh(rest, M.apply(s, e))>

# an automatic reply never asks for a reply (RFC 0003 §7): no chain
def QuietEv(e: M.Ev) -> Data:
  match e:
    case M.EMsgSent{+m}:
      {Bool.and(M.Msg.auto(m), M.Msg.expect(m)) == False{} : Bool}
    case _:
      Unit

def Quiet(evs: List<&2, M.Ev>) -> Data:
  match evs:
    case Nil{}:
      Unit
    case Con{+e, +rest}:
      Both<QuietEv(e), Quiet(rest)>

def fresh_snoc(+evs: List<&2, M.Ev>, -s: M.St, -e: M.Ev, g: Fresh(evs, s), k: FreshEv(M.apply_all(evs, s), e))
    -> Fresh(M.snoc(M.Ev, evs, e), s):
  match evs:
    case Nil{}:
      Both{k, Unit{}}
    case Con{h, rest}:
      match g:
        case Both{g1, g2}:
          Both{g1, fresh_snoc(rest, M.apply(s, h), e, g2, k)}

def quiet_snoc(+evs: List<&2, M.Ev>, -e: M.Ev, g: Quiet(evs), k: QuietEv(e)) -> Quiet(M.snoc(M.Ev, evs, e)):
  match evs:
    case Nil{}:
      Both{k, Unit{}}
    case Con{h, rest}:
      match g:
        case Both{g1, g2}:
          Both{g1, quiet_snoc(rest, e, g2, k)}

def emit_f.k(-o: M.St, -d: M.St, -fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : M.St}, -e: M.Ev,
    k: FreshEv(d, e)) -> FreshEv(M.apply_all(journal(fx), o), e):
  %p : FreshEv(_, e)
  k

def journal_snoc_ev(+fx: List<&2, Fx>, -e: M.Ev)
    -> {M.snoc(M.Ev, journal(fx), e) == journal(M.snoc(Fx, fx, FJ{e})) : List<&2, M.Ev>}:
  match fx:
    case Nil{}:
      {==}
    case Con{f, rest}:
      match f:
        case FJ{h}:
          %journal_snoc_ev(rest, e) : {h <> M.snoc(M.Ev, journal(rest), e) == h <> _ : List<&2, M.Ev>}
          {==}
        case FO{_}:
          journal_snoc_ev(rest, e)

def emit_f(-o: M.St, -d: M.St, +fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : M.St},
    f: Fresh(journal(fx), o), -e: M.Ev, k: FreshEv(d, e)) -> Fresh(journal(M.snoc(Fx, fx, FJ{e})), o):
  %journal_snoc_ev(fx, e) : Fresh(_, o)
  fresh_snoc(journal(fx), o, e, f, emit_f.k(o, d, fx, p, e, k))

def emit_q(+fx: List<&2, Fx>, q: Quiet(journal(fx)), -e: M.Ev, k: QuietEv(e)) -> Quiet(journal(M.snoc(Fx, fx, FJ{e}))):
  %journal_snoc_ev(fx, e) : Quiet(_)
  quiet_snoc(journal(fx), e, q, k)

def journal_snoc_out(+fx: List<&2, Fx>, -x: J.Json)
    -> {journal(fx) == journal(M.snoc(Fx, fx, FO{x})) : List<&2, M.Ev>}:
  match fx:
    case Nil{}:
      {==}
    case Con{f, rest}:
      match f:
        case FJ{h}:
          %journal_snoc_out(rest, x) : {h <> journal(rest) == h <> _ : List<&2, M.Ev>}
          {==}
        case FO{_}:
          journal_snoc_out(rest, x)

def out_f(-o: M.St, +fx: List<&2, Fx>, f: Fresh(journal(fx), o), -x: J.Json) -> Fresh(journal(M.snoc(Fx, fx, FO{x})), o):
  %journal_snoc_out(fx, x) : Fresh(_, o)
  f

def out_q(+fx: List<&2, Fx>, q: Quiet(journal(fx)), -x: J.Json) -> Quiet(journal(M.snoc(Fx, fx, FO{x}))):
  %journal_snoc_out(fx, x) : Quiet(_)
  q

# a message is delivered only while it is queued, and a message goes back
# to the queue only as a steer_leftover (law delivered_once)
def queued_at.go(m: Maybe<&2, M.MSt>) -> Bool:
  match m:
    case Some{M.Queued{_}}:
      True{}
    case _:
      False{}

def queued_at(+s: M.St, +id: Nat) -> Bool:
  queued_at.go(M.state_of(s, id))

def DelivSt(+s: M.St, +id: Nat, x: M.MSt) -> Data:
  match x:
    case M.Delivered{}:
      {queued_at(s, id) == True{} : Bool}
    case M.Queued{+r}:
      {String.eq(r, "steer_leftover") == True{} : Bool}
    case _:
      Unit

def DelivEv(+s: M.St, e: M.Ev) -> Data:
  match e:
    case M.EMsgState{+id, +x}:
      DelivSt(s, id, x)
    case _:
      Unit

def Deliv(evs: List<&2, M.Ev>, +s: M.St) -> Data:
  match evs:
    case Nil{}:
      Unit
    case Con{+e, +rest}:
      Both<DelivEv(s, e), Deliv(rest, M.apply(s, e))>

def deliv_snoc(+evs: List<&2, M.Ev>, -s: M.St, -e: M.Ev, g: Deliv(evs, s), k: DelivEv(M.apply_all(evs, s), e))
    -> Deliv(M.snoc(M.Ev, evs, e), s):
  match evs:
    case Nil{}:
      Both{k, Unit{}}
    case Con{h, rest}:
      match g:
        case Both{g1, g2}:
          Both{g1, deliv_snoc(rest, M.apply(s, h), e, g2, k)}

def emit_v.k(-o: M.St, -d: M.St, -fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : M.St}, -e: M.Ev,
    k: DelivEv(d, e)) -> DelivEv(M.apply_all(journal(fx), o), e):
  %p : DelivEv(_, e)
  k

def emit_v(-o: M.St, -d: M.St, +fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : M.St},
    v: Deliv(journal(fx), o), -e: M.Ev, k: DelivEv(d, e)) -> Deliv(journal(M.snoc(Fx, fx, FJ{e})), o):
  %journal_snoc_ev(fx, e) : Deliv(_, o)
  deliv_snoc(journal(fx), o, e, v, emit_v.k(o, d, fx, p, e, k))

def out_v(-o: M.St, +fx: List<&2, Fx>, v: Deliv(journal(fx), o), -x: J.Json) -> Deliv(journal(M.snoc(Fx, fx, FO{x})), o):
  %journal_snoc_out(fx, x) : Deliv(_, o)
  v

# ---- the replay lemmas ----

def apply_all_snoc(+evs: List<&2, M.Ev>, -s: M.St, -e: M.Ev)
    -> {M.apply(M.apply_all(evs, s), e) == M.apply_all(M.snoc(M.Ev, evs, e), s) : M.St}:
  match evs:
    case Nil{}:
      {==}
    case Con{h, rest}:
      apply_all_snoc(rest, M.apply(s, h), e)

def emit_ok(-o: M.St, +d: M.St, +fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : M.St}, -e: M.Ev)
    -> {M.apply(d, e) == M.apply_all(journal(M.snoc(Fx, fx, FJ{e})), o) : M.St}:
  %journal_snoc_ev(fx, e) : {M.apply(d, e) == M.apply_all(_, o) : M.St}
  %apply_all_snoc(journal(fx), o, e) : {M.apply(d, e) == _ : M.St}
  %p : {M.apply(d, e) == M.apply(_, e) : M.St}
  {==}

def out_ok(-o: M.St, +d: M.St, +fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : M.St}, -x: J.Json)
    -> {d == M.apply_all(journal(M.snoc(Fx, fx, FO{x})), o) : M.St}:
  %journal_snoc_out(fx, x) : {d == M.apply_all(_, o) : M.St}
  p

# ---- the core primitives ----

def Core.d(-o: M.St, c: Core<o>) -> M.St:
  match c:
    case Core{+d, _, _, _, _, _}:
      d

# the only way the state changes: an event, applied and journaled. The
# caller proves the event keeps the journal's invariants (for anything
# but a new message both hold by definition: `{==}`)
def core_emit(-o: M.St, c: Core<o>, +e: M.Ev, k: FreshEv(Core.d(o, c), e), kq: QuietEv(e), kv: DelivEv(Core.d(o, c), e)) -> Core<o>:
  match c:
    case Core{+d, +fx, +p, f, q, v}:
      Core{M.apply(d, e), M.snoc(Fx, fx, FJ{e}), emit_ok(o, d, fx, p, e), emit_f(o, d, fx, p, f, e, k), emit_q(fx, q, e, kq), emit_v(o, d, fx, p, v, e, kv)}

def core_out(-o: M.St, c: Core<o>, +x: J.Json) -> Core<o>:
  match c:
    case Core{+d, +fx, p, f, q, v}:
      Core{d, M.snoc(Fx, fx, FO{x}), out_ok(o, d, fx, p, x), out_f(o, fx, f, x), out_q(fx, q, x), out_v(o, fx, v, x)}

def core_start(+d: M.St) -> Core<d>:
  Core{d, Nil{}, {==}, Unit{}, Unit{}, Unit{}}

def Core.fx(-o: M.St, c: Core<o>) -> List<&2, Fx>:
  match c:
    case Core{_, +fx, _, _, _, _}:
      fx

# ---- the context ----

def Cx.c(-o: M.St, cx: Cx<o>) -> Core<o>:
  match cx:
    case Cx{+c, _}:
      c

def Cx.r(-o: M.St, cx: Cx<o>) -> R:
  match cx:
    case Cx{_, +r}:
      r

def d(-o: M.St, +cx: Cx<o>) -> M.St:
  Core.d(o, Cx.c(o, cx))

def mark(+r: R) -> R:
  match r:
    case R{+a, +b, +c, +e, +f, +g, +w, _, +n, +gi, +an, +ne}:
      R{a, b, c, e, f, g, w, True{}, n, gi, an, ne}

def emit(-o: M.St, cx: Cx<o>, +e: M.Ev, k: FreshEv(d(o, cx), e), kq: QuietEv(e), kv: DelivEv(d(o, cx), e)) -> Cx<o>:
  match cx:
    case Cx{+c, +r}:
      Cx{core_emit(o, c, e, k, kq, kv), mark(r)}

def out(-o: M.St, cx: Cx<o>, +x: J.Json) -> Cx<o>:
  match cx:
    case Cx{+c, +r}:
      Cx{core_out(o, c, x), r}

def with_r(-o: M.St, cx: Cx<o>, +r: R) -> Cx<o>:
  match cx:
    case Cx{+c, _}:
      Cx{c, r}

def dirty(-o: M.St, +cx: Cx<o>) -> Cx<o>:
  with_r(o, cx, mark(Cx.r(o, cx)))

def now(-o: M.St, +cx: Cx<o>) -> Nat:
  R.now(Cx.r(o, cx))
