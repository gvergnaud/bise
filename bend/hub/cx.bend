# hub/cx.bend — the step context of the hub: the durable state, the
# runtime bookkeeping and the effects of one input. `Core<o>` carries a
# proof that its state is the replay of its journal over `o`, the state
# before the input: every change of the durable state goes through
# `emit`, which extends the proof (law replay_reproduces).
import Base

import ../vendor/json.bend as J

import ./model.bend as M
import ./gen/state.bend as S

import ./picks.bend as PK

import ./lazy.bend as LZ

type Fx is Data:
  FJ{ev: M.Ev}
  FO{o: J.Json}

type Rt is Data:
  Rt{name: String, run: S.Run, waiting: Bool, turn: Maybe<&2, Nat>, turns: Nat}

type Waiter is Data:
  Waiter{token: Nat, agent: String, msg: Nat, deadline: Nat}

# an agent's steer-mode deliveries this turn; `seen`: how many of `ids`
# a receipt (ISteered) already reported, so the next one reports only
# the new ones (seen <= len ids); `rx_seen`: the same for the received
# receipt (ISteerRx), a second cursor of its own (no order between the
# two is required)
type Steer is Data:
  Steer{agent: String, ids: List<&2, Nat>, seen: Nat, rx_seen: Nat}

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
    ans: List<&2, J.Json>, need: Maybe<&2, J.Json>, answered: Maybe<&2, PK.Answer>,
    due: List<&2, Nat>, ctx: List<&2, S.Vctx>}
# due (inbox-record): the cards closed with his answer whose route line's
# position the shell has not given yet (answer_pos); runtime only, so a
# position lost to a restart stays unknown

type Core<-o: S.St> is Data:
  Core{d: S.St, fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : S.St},
    f: Fresh(journal(fx), o), q: Quiet(journal(fx)), v: Deliv(journal(fx), o)}

type Cx<-o: S.St> is Data:
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

def Rt.run(x: Rt) -> S.Run:
  match x:
    case Rt{_, +run, _, _, _}:
      run

def Rt.with_run(x: Rt, +v: S.Run) -> Rt:
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
    case Steer{+agent, _, _, _}:
      agent

def Steer.ids(x: Steer) -> List<&2, Nat>:
  match x:
    case Steer{_, +ids, _, _}:
      ids

def Steer.seen(x: Steer) -> Nat:
  match x:
    case Steer{_, _, +seen, _}:
      seen

def Steer.rx_seen(x: Steer) -> Nat:
  match x:
    case Steer{_, _, _, +rx}:
      rx

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
    case R{+rts, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      rts

def R.with_rts(x: R, +v: List<&2, Rt>) -> R:
  match x:
    case R{_, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need, +answered, +due, +ctx}:
      R{v, waiters, steered, counts, last, crashes, workspace, dirty, now, git, ans, need, answered, due, ctx}

def R.waiters(x: R) -> List<&2, Waiter>:
  match x:
    case R{_, +waiters, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      waiters

def R.with_waiters(x: R, +v: List<&2, Waiter>) -> R:
  match x:
    case R{+rts, _, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need, +answered, +due, +ctx}:
      R{rts, v, steered, counts, last, crashes, workspace, dirty, now, git, ans, need, answered, due, ctx}

def R.steered(x: R) -> List<&2, Steer>:
  match x:
    case R{_, _, +steered, _, _, _, _, _, _, _, _, _, _, _, _}:
      steered

def R.with_steered(x: R, +v: List<&2, Steer>) -> R:
  match x:
    case R{+rts, +waiters, _, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need, +answered, +due, +ctx}:
      R{rts, waiters, v, counts, last, crashes, workspace, dirty, now, git, ans, need, answered, due, ctx}

def R.counts(x: R) -> List<&2, Cnt>:
  match x:
    case R{_, _, _, +counts, _, _, _, _, _, _, _, _, _, _, _}:
      counts

def R.with_counts(x: R, +v: List<&2, Cnt>) -> R:
  match x:
    case R{+rts, +waiters, +steered, _, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need, +answered, +due, +ctx}:
      R{rts, waiters, steered, v, last, crashes, workspace, dirty, now, git, ans, need, answered, due, ctx}

def R.last(x: R) -> List<&2, Kv>:
  match x:
    case R{_, _, _, _, +last, _, _, _, _, _, _, _, _, _, _}:
      last

def R.with_last(x: R, +v: List<&2, Kv>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, _, +crashes, +workspace, +dirty, +now, +git, +ans, +need, +answered, +due, +ctx}:
      R{rts, waiters, steered, counts, v, crashes, workspace, dirty, now, git, ans, need, answered, due, ctx}

def R.crashes(x: R) -> List<&2, Kn>:
  match x:
    case R{_, _, _, _, _, +crashes, _, _, _, _, _, _, _, _, _}:
      crashes

def R.with_crashes(x: R, +v: List<&2, Kn>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, _, +workspace, +dirty, +now, +git, +ans, +need, +answered, +due, +ctx}:
      R{rts, waiters, steered, counts, last, v, workspace, dirty, now, git, ans, need, answered, due, ctx}

def R.workspace(x: R) -> String:
  match x:
    case R{_, _, _, _, _, _, +workspace, _, _, _, _, _, _, _, _}:
      workspace

def R.dirty(x: R) -> Bool:
  match x:
    case R{_, _, _, _, _, _, _, +dirty, _, _, _, _, _, _, _}:
      dirty

def R.with_dirty(x: R, +v: Bool) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, _, +now, +git, +ans, +need, +answered, +due, +ctx}:
      R{rts, waiters, steered, counts, last, crashes, workspace, v, now, git, ans, need, answered, due, ctx}

def R.now(x: R) -> Nat:
  match x:
    case R{_, _, _, _, _, _, _, _, +now, _, _, _, _, _, _}:
      now

def R.with_now(x: R, +v: Nat) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, _, +git, +ans, +need, +answered, +due, +ctx}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, v, git, ans, need, answered, due, ctx}

def R.git(x: R) -> Bool:
  match x:
    case R{_, _, _, _, _, _, _, _, _, +git, _, _, _, _, _}:
      git

def R.with_git(x: R, +v: Bool) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, _, +ans, +need, +answered, +due, +ctx}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, v, ans, need, answered, due, ctx}

def R.ans(x: R) -> List<&2, J.Json>:
  match x:
    case R{_, _, _, _, _, _, _, _, _, _, +ans, _, _, _, _}:
      ans

def R.with_ans(x: R, +v: List<&2, J.Json>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, _, +need, +answered, +due, +ctx}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, git, v, need, answered, due, ctx}

def R.need(x: R) -> Maybe<&2, J.Json>:
  match x:
    case R{_, _, _, _, _, _, _, _, _, _, _, +need, _, _, _}:
      need

def R.with_need(x: R, +v: Maybe<&2, J.Json>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, _, +answered, +due, +ctx}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, git, ans, v, answered, due, ctx}

# his answer to a card in this step, for the step's answer (inbox picks
# (3), architect m_22636): set where its route line is written
def R.answered(x: R) -> Maybe<&2, PK.Answer>:
  match x:
    case R{_, _, _, _, _, _, _, _, _, _, _, _, +answered, _, _}:
      answered

def R.with_answered(x: R, +v: Maybe<&2, PK.Answer>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need, _, +due, +ctx}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, git, ans, need, v, due, ctx}

def R.due(x: R) -> List<&2, Nat>:
  match x:
    case R{_, _, _, _, _, _, _, _, _, _, _, _, _, +due, _}:
      due

def R.with_due(x: R, +v: List<&2, Nat>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need, +answered, _, +ctx}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, git, ans, need, answered, v, ctx}

def R.ctx(x: R) -> List<&2, S.Vctx>:
  match x:
    case R{_, _, _, _, _, _, _, _, _, _, _, _, _, _, +ctx}:
      ctx

def R.with_ctx(x: R, +v: List<&2, S.Vctx>) -> R:
  match x:
    case R{+rts, +waiters, +steered, +counts, +last, +crashes, +workspace, +dirty, +now, +git, +ans, +need, +answered, +due, _}:
      R{rts, waiters, steered, counts, last, crashes, workspace, dirty, now, git, ans, need, answered, due, v}

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
# task or a new name takes a name no agent has (law L4: names stay apart;
# an old name may be taken: apply strips it, name-free)
def FreshEv(+s: S.St, e: M.Ev) -> Data:
  match e:
    case M.EMsgSent{+m}:
      {S.Msg.id(m) == S.St.next_msg(s) : Nat}
    case M.ECardOpened{+c}:
      {S.Card.id(c) == S.St.next_card(s) : Nat}
    case M.ETaskCreated{+name, _, _, _, _}:
      {M.name_held(s, name) == False{} : Bool}
    case M.ERenamed{_, +new}:
      {M.name_held(s, new) == False{} : Bool}
    case _:
      Unit

def Fresh(evs: List<&2, M.Ev>, +s: S.St) -> Data:
  match evs:
    case Nil{}:
      Unit
    case Con{+e, +rest}:
      Both<FreshEv(s, e), Fresh(rest, M.apply(s, e))>

# an automatic reply never asks for a reply (RFC 0003 §7): no chain
def QuietEv(e: M.Ev) -> Data:
  match e:
    case M.EMsgSent{+m}:
      {Bool.and(S.Msg.auto(m), S.Msg.expect(m)) == False{} : Bool}
    case _:
      Unit

def Quiet(evs: List<&2, M.Ev>) -> Data:
  match evs:
    case Nil{}:
      Unit
    case Con{+e, +rest}:
      Both<QuietEv(e), Quiet(rest)>

def fresh_snoc(+evs: List<&2, M.Ev>, -s: S.St, -e: M.Ev, g: Fresh(evs, s), k: FreshEv(M.apply_all(evs, s), e))
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

def emit_f.k(-o: S.St, -d: S.St, -fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : S.St}, -e: M.Ev,
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

def emit_f(-o: S.St, -d: S.St, +fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : S.St},
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

def out_f(-o: S.St, +fx: List<&2, Fx>, f: Fresh(journal(fx), o), -x: J.Json) -> Fresh(journal(M.snoc(Fx, fx, FO{x})), o):
  %journal_snoc_out(fx, x) : Fresh(_, o)
  f

def out_q(+fx: List<&2, Fx>, q: Quiet(journal(fx)), -x: J.Json) -> Quiet(journal(M.snoc(Fx, fx, FO{x}))):
  %journal_snoc_out(fx, x) : Quiet(_)
  q

# a message is delivered only while it is queued, and a message goes back
# to the queue only as a steer_leftover (law delivered_once)
def queued_at.go(m: Maybe<&2, S.MSt>) -> Bool:
  match m:
    case Some{S.Queued{_}}:
      True{}
    case _:
      False{}

def queued_at(+s: S.St, +id: Nat) -> Bool:
  queued_at.go(M.state_of(s, id))

def DelivSt(+s: S.St, +id: Nat, x: S.MSt) -> Data:
  match x:
    case S.Delivered{}:
      {queued_at(s, id) == True{} : Bool}
    case S.Queued{+r}:
      {String.eq(r, "steer_leftover") == True{} : Bool}
    case _:
      Unit

def DelivEv(+s: S.St, e: M.Ev) -> Data:
  match e:
    case M.EMsgState{+id, +x}:
      DelivSt(s, id, x)
    case _:
      Unit

def Deliv(evs: List<&2, M.Ev>, +s: S.St) -> Data:
  match evs:
    case Nil{}:
      Unit
    case Con{+e, +rest}:
      Both<DelivEv(s, e), Deliv(rest, M.apply(s, e))>

def deliv_snoc(+evs: List<&2, M.Ev>, -s: S.St, -e: M.Ev, g: Deliv(evs, s), k: DelivEv(M.apply_all(evs, s), e))
    -> Deliv(M.snoc(M.Ev, evs, e), s):
  match evs:
    case Nil{}:
      Both{k, Unit{}}
    case Con{h, rest}:
      match g:
        case Both{g1, g2}:
          Both{g1, deliv_snoc(rest, M.apply(s, h), e, g2, k)}

def emit_v.k(-o: S.St, -d: S.St, -fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : S.St}, -e: M.Ev,
    k: DelivEv(d, e)) -> DelivEv(M.apply_all(journal(fx), o), e):
  %p : DelivEv(_, e)
  k

def emit_v(-o: S.St, -d: S.St, +fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : S.St},
    v: Deliv(journal(fx), o), -e: M.Ev, k: DelivEv(d, e)) -> Deliv(journal(M.snoc(Fx, fx, FJ{e})), o):
  %journal_snoc_ev(fx, e) : Deliv(_, o)
  deliv_snoc(journal(fx), o, e, v, emit_v.k(o, d, fx, p, e, k))

def out_v(-o: S.St, +fx: List<&2, Fx>, v: Deliv(journal(fx), o), -x: J.Json) -> Deliv(journal(M.snoc(Fx, fx, FO{x})), o):
  %journal_snoc_out(fx, x) : Deliv(_, o)
  v

# ---- the replay lemmas ----

def apply_all_snoc(+evs: List<&2, M.Ev>, -s: S.St, -e: M.Ev)
    -> {M.apply(M.apply_all(evs, s), e) == M.apply_all(M.snoc(M.Ev, evs, e), s) : S.St}:
  match evs:
    case Nil{}:
      {==}
    case Con{h, rest}:
      apply_all_snoc(rest, M.apply(s, h), e)

def emit_ok(-o: S.St, +d: S.St, +fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : S.St}, -e: M.Ev)
    -> {M.apply(d, e) == M.apply_all(journal(M.snoc(Fx, fx, FJ{e})), o) : S.St}:
  %journal_snoc_ev(fx, e) : {M.apply(d, e) == M.apply_all(_, o) : S.St}
  %apply_all_snoc(journal(fx), o, e) : {M.apply(d, e) == _ : S.St}
  %p : {M.apply(d, e) == M.apply(_, e) : S.St}
  {==}

def out_ok(-o: S.St, +d: S.St, +fx: List<&2, Fx>, p: {d == M.apply_all(journal(fx), o) : S.St}, -x: J.Json)
    -> {d == M.apply_all(journal(M.snoc(Fx, fx, FO{x})), o) : S.St}:
  %journal_snoc_out(fx, x) : {d == M.apply_all(_, o) : S.St}
  p

# ---- the core primitives ----

def Core.d(-o: S.St, c: Core<o>) -> S.St:
  match c:
    case Core{+d, _, _, _, _, _}:
      d

# the only way the state changes: an event, applied and journaled. The
# caller proves the event keeps the journal's invariants (for anything
# but a new message both hold by definition: `{==}`)
def core_emit(-o: S.St, c: Core<o>, +e: M.Ev, k: FreshEv(Core.d(o, c), e), kq: QuietEv(e), kv: DelivEv(Core.d(o, c), e)) -> Core<o>:
  match c:
    case Core{+d, +fx, +p, f, q, v}:
      Core{M.apply(d, e), M.snoc(Fx, fx, FJ{e}), emit_ok(o, d, fx, p, e), emit_f(o, d, fx, p, f, e, k), emit_q(fx, q, e, kq), emit_v(o, d, fx, p, v, e, kv)}

def core_out(-o: S.St, c: Core<o>, +x: J.Json) -> Core<o>:
  match c:
    case Core{+d, +fx, p, f, q, v}:
      Core{d, M.snoc(Fx, fx, FO{x}), out_ok(o, d, fx, p, x), out_f(o, fx, f, x), out_q(fx, q, x), out_v(o, fx, v, x)}

def core_start(+d: S.St) -> Core<d>:
  Core{d, Nil{}, {==}, Unit{}, Unit{}, Unit{}}

def Core.fx(-o: S.St, c: Core<o>) -> List<&2, Fx>:
  match c:
    case Core{_, +fx, _, _, _, _}:
      fx

# ---- the context ----

def Cx.c(-o: S.St, cx: Cx<o>) -> Core<o>:
  match cx:
    case Cx{+c, _}:
      c

def Cx.r(-o: S.St, cx: Cx<o>) -> R:
  match cx:
    case Cx{_, +r}:
      r

def d(-o: S.St, +cx: Cx<o>) -> S.St:
  Core.d(o, Cx.c(o, cx))

def mark(+r: R) -> R:
  match r:
    case R{+a, +b, +c, +e, +f, +g, +w, _, +n, +gi, +an, +ne, +ad, +due, +ctx}:
      R{a, b, c, e, f, g, w, True{}, n, gi, an, ne, ad, due, ctx}

def emit(-o: S.St, cx: Cx<o>, +e: M.Ev, k: FreshEv(d(o, cx), e), kq: QuietEv(e), kv: DelivEv(d(o, cx), e)) -> Cx<o>:
  match cx:
    case Cx{+c, +r}:
      Cx{core_emit(o, c, e, k, kq, kv), mark(r)}

def out(-o: S.St, cx: Cx<o>, +x: J.Json) -> Cx<o>:
  match cx:
    case Cx{+c, +r}:
      Cx{core_out(o, c, x), r}

def with_r(-o: S.St, cx: Cx<o>, +r: R) -> Cx<o>:
  match cx:
    case Cx{+c, _}:
      Cx{c, r}

# his answer `a` noted for the step's answer (R.answered)
def answered(-o: S.St, cx: Cx<o>, +a: PK.Answer) -> Cx<o>:
  match cx:
    case Cx{+c, +r}:
      Cx{c, R.with_answered(r, Some{a})}

def dirty(-o: S.St, +cx: Cx<o>) -> Cx<o>:
  with_r(o, cx, mark(Cx.r(o, cx)))

def now(-o: S.St, +cx: Cx<o>) -> Nat:
  R.now(Cx.r(o, cx))

# ---- the runtimes' and the waits' lists (moved from core.bend) ----

def rt_rename(xs: List<&2, Rt>, +old: String, +new: String) -> List<&2, Rt>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(Rt, String.eq(Rt.name(h), old), Rt.with_name(h, new), h) <> rt_rename(t, old, new)

def any_waiter(xs: List<&2, Waiter>, +n: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      LZ.or(String.eq(Waiter.agent(h), n), u => any_waiter(t, n))

def waiters_of(xs: List<&2, Waiter>, +n: String) -> List<&2, Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_of(t, n)
      Bool.pick(List<&2, Waiter>, String.eq(Waiter.agent(h), n), h <> rest, rest)

def waiters_not(xs: List<&2, Waiter>, +n: String) -> List<&2, Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_not(t, n)
      Bool.pick(List<&2, Waiter>, String.eq(Waiter.agent(h), n), rest, h <> rest)

def waiters_late(xs: List<&2, Waiter>, +now: Nat) -> List<&2, Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_late(t, now)
      Bool.pick(List<&2, Waiter>, Nat.is_le(Waiter.deadline(h), now), h <> rest, rest)

def waiters_keep(xs: List<&2, Waiter>, +now: Nat) -> List<&2, Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = waiters_keep(t, now)
      Bool.pick(List<&2, Waiter>, Nat.is_le(Waiter.deadline(h), now), rest, h <> rest)

def waiters_rename(xs: List<&2, Waiter>, +old: String, +new: String) -> List<&2, Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(Waiter, String.eq(Waiter.agent(h), old), Waiter.with_agent(h, new), h) <> waiters_rename(t, old, new)

# the first waiter a new message releases (RFC 0003 §6.2): the agent's
# wait for this reply, or any wait when the message asks a question
def waiter_hit(+w: Waiter, +to: String, +reply_to: Maybe<&2, Nat>, +asks: Bool) -> Bool:
  String.eq(Waiter.agent(w), to) && (M.maybe_nat_is(reply_to, Waiter.msg(w)) || asks)

def waiter_pick(xs: List<&2, Waiter>, +to: String, +reply_to: Maybe<&2, Nat>, +asks: Bool) -> Maybe<&2, Waiter>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      LZ.pick(Maybe<&2, Waiter>, waiter_hit(h, to, reply_to, asks), u => Some{h}, u => waiter_pick(t, to, reply_to, asks))

def waiter_drop(xs: List<&2, Waiter>, +tok: Nat) -> List<&2, Waiter>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      LZ.pick(List<&2, Waiter>, Nat.is_eq(Waiter.token(h), tok), u => t, u => h <> waiter_drop(t, tok))
