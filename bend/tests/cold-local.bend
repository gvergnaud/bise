# bend/tests/cold-local.bend — step_is_local / view_is_local, checked on
# real data (issue 36 chunk 5a, architect m_37415: stated, checked by a
# test, not proven). COLD_JOURNAL=<journal.jsonl> is replayed (as a boot:
# replay_fresh, apply); then each input of COLD_INPUTS=<inputs.jsonl> (one
# sb-core input per line, as rust/proto/fixtures/core_in.jsonl), and an
# answer to each open card of the state, is committed twice: on the state,
# and on the state with every cold message dropped but the ones needs_in
# names. It prints `inputs <n> journal <a> fx <b> need <c> miss <d>`:
#   journal: the two journals differ (a step read a dropped record);
#   fx: the two effects differ (a view of a dropped record: expected while
#       the mirror reads cold ones from the store, 5b; the count is printed);
#   need: the two need answers differ;
#   miss: an event the step journaled needs a cold message needs_in did not
#       name (the step would ask a second time: architect m_37415's miss).
# then the first lines of each kind. tests/cold_local_check.py runs it.

import Base
import ../vendor/json.bend as J
import ../hub/model.bend as M
import ../hub/cold.bend as CO
import ../hub/core.bend as H
import ../hub/cx.bend as X
import ../hub/needs.bend as N
import ../hub/inv.bend as HI
import ../hub/gen/state.bend as S
import ../hub/gen/journal.bend as GJ
import ../hub/gen/in.bend as GI
import ../hub/lazy.bend as LZ
import ../runtime/persist.bend as P

# ---- the boot replay ----

def replay.ev(m: Maybe<&2, M.Ev>, +st: S.St) -> S.St:
  match m:
    case None{}:
      st
    case Some{+e}:
      CO.replay_one(CO.replay_fresh(st, e), st, e)


def replay.line(r: Result<&2, &2, String, J.Json>, +st: S.St) -> S.St:
  match r:
    case Done{+j}:
      replay.ev(GJ.of(j), st)
    case Fail{_}:
      st


def replay(ls: List<&2, String>, +st: S.St) -> S.St:
  match ls:
    case Nil{}:
      st
    case Con{+l, +t}:
      replay(t, Bool.pick(S.St, String.is_empty(String.trim(l)), st, replay.line(J.parse(l), st)))

# ---- one input, twice ----

def keep_only(xs: List<&2, S.MRec>, +keep: List<&2, Nat>) -> List<&2, S.MRec>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = keep_only(t, keep)
      Bool.pick(List<&2, S.MRec>, M.nat_in(keep, M.MRec.id(h)), h <> rest, rest)

def evs_strs(es: List<&2, M.Ev>) -> List<&2, String>:
  match es:
    case Nil{}:
      Nil{}
    case Con{+e, +t}:
      J.stringify(GJ.json(e)) <> evs_strs(t)

def evs_str(es: List<&2, M.Ev>) -> String:
  String.join(evs_strs(es), "\n")

def nats_str(ns: List<&2, Nat>) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      Nat.show(n) <> nats_str(t)

def outs(fx: List<&2, X.Fx>) -> List<&2, String>:
  match fx:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      match f:
        case X.FJ{_}:
          outs(t)
        case X.FO{+o}:
          J.stringify(o) <> outs(t)

def need_str(m: Maybe<&2, J.Json>) -> String:
  match m:
    case None{}:
      ""
    case Some{+q}:
      J.stringify(q)

# the cold ids an event list needs that `keep` does not hold
def missed.ids(ns: List<&2, Nat>, +cold: List<&2, Nat>, +keep: List<&2, Nat>) -> List<&2, Nat>:
  match ns:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      +rest = missed.ids(t, cold, keep)
      Bool.pick(List<&2, Nat>, M.nat_in(cold, n) && Bool.not(M.nat_in(keep, n)), n <> rest, rest)


def missed(es: List<&2, M.Ev>, +cold: List<&2, Nat>, +keep: List<&2, Nat>) -> List<&2, Nat>:
  match es:
    case Nil{}:
      Nil{}
    case Con{+e, +t}:
      List.append(&2, Nat, missed.ids(N.Needs.msgs(N.needs(e)), cold, keep), missed(t, cold, keep))


type Acc is Data:
  Acc{n: Nat, journal: Nat, fx: Nat, need: Nat, miss: Nat, notes: List<&2, String>}

def note(bad: Bool, +what: String, +a: Acc) -> Acc:
  match bad a:
    case False{} _:
      a
    case True{} Acc{+n, +jr, +fx, +nd, +ms, +notes}:
      Acc{n, jr, fx, nd, ms, Bool.pick(List<&2, String>, Nat.is_lt(List.length(&2, String, notes), 12n), List.append(&2, String, notes, [what]), notes)}

def count(+a: Acc, +dj: Bool, +df: Bool, +dn: Bool, +dm: Bool) -> Acc:
  match a:
    case Acc{+n, +jr, +fx, +nd, +ms, +notes}:
      Acc{1n+n, Bool.pick(Nat, dj, 1n+jr, jr), Bool.pick(Nat, df, 1n+fx, fx), Bool.pick(Nat, dn, 1n+nd, nd), Bool.pick(Nat, dm, 1n+ms, ms), notes}

def one(+st: S.St, +r: X.R, +i: J.Json, +a: Acc) -> Acc:
  +tag = J.stringify(i)
  +keep = N.Needs.msgs(N.needs_in(st, GI.of(i)))
  +cold = HI.ids(S.St.old(st))
  +t1 = H.commit(H.Hub{st, r, H.Live{}}, i)
  +t2 = H.commit(H.Hub{S.St.with_old(st, keep_only(S.St.old(st), keep)), r, H.Live{}}, i)
  +es = X.journal(H.Turn.fx(t1))
  +dj = Bool.not(String.eq(evs_str(es), evs_str(X.journal(H.Turn.fx(t2)))))
  +df = Bool.not(String.eq(String.join(outs(H.Turn.fx(t1)), "\n"), String.join(outs(H.Turn.fx(t2)), "\n")))
  +dn = Bool.not(String.eq(need_str(H.Turn.need(t1)), need_str(H.Turn.need(t2))))
  +ms = missed(es, cold, keep)
  +dm = Bool.not(List.is_empty(&2, Nat, ms))
  +a1 = note(dj, String.concat(["journal differs: ", tag]), count(a, dj, df, dn, dm))
  +a2 = note(dn, String.concat(["need differs: ", tag]), a1)
  note(dm, String.concat(["miss ", String.join(nats_str(ms), ","), ": ", tag]), a2)

def inputs.one(empty: Bool, p: Result<&2, &2, String, J.Json>, +st: S.St, +r: X.R, +a: Acc) -> Acc:
  match empty p:
    case True{} _:
      a
    case False{} Done{+i}:
      one(st, r, i, a)
    case False{} Fail{_}:
      a

# an answer to each open card (its for_msg is what needs_in names)

def inputs(ls: List<&2, String>, +st: S.St, +r: X.R, +a: Acc) -> Acc:
  match ls:
    case Nil{}:
      a
    case Con{+l, +t}:
      inputs(t, st, r, inputs.one(String.is_empty(String.trim(l)), J.parse(l), st, r, a))


def answers(cs: List<&2, S.Card>, +st: S.St, +r: X.R, +a: Acc) -> Acc:
  match cs:
    case Nil{}:
      a
    case Con{+c, +t}:
      +i = J.obj([J.kv("t", J.JStr{"answer"}), J.kv("card", J.JNum{S.Card.id(c)}), J.kv("text", J.JStr{"1"}), J.kv("ans", J.JArr{Nil{}}), J.kv("git", J.JBool{True{}}), J.kv("now", J.JNum{1000000n})])
      answers(t, st, r, one(st, r, i, a))

def report(a: Acc) -> String:
  match a:
    case Acc{+n, +jr, +fx, +nd, +ms, +notes}:
      String.concat(["inputs ", Nat.show(n), " journal ", Nat.show(jr), " fx ", Nat.show(fx), " need ", Nat.show(nd), " miss ", Nat.show(ms), "\n", String.join(notes, "\n")])

def go(mj: Maybe<&2, String>, mi: Maybe<&2, String>) -> IO(Unit):
  match mj mi:
    case Some{+j} Some{+ins}:
      +st = replay(String.split(j, '\n'), M.init("/w"))
      +r = H.Hub.r(H.hub_new("/w"))
      +a0 = {Acc{0n, 0n, 0n, 0n, 0n, Nil{}} : Acc}
      IO.print(report(answers(S.St.cards(st), st, r, inputs(String.split(ins, '\n'), st, r, a0))))
    case _ _:
      IO.print("UNREADABLE")

def main() -> IO(Unit):
  do IO<Unit>:
    rj : Result<&1, &1, U32 & String, String> <- IO.get_env("COLD_JOURNAL")
    ri : Result<&1, &1, U32 & String, String> <- IO.get_env("COLD_INPUTS")
    +pj : String <- IO.pure(String, P.env_or(rj, ""))
    +pi : String <- IO.pure(String, P.env_or(ri, ""))
    mj : Maybe<&2, String> <- P.read_file(pj)
    mi : Maybe<&2, String> <- P.read_file(pi)
    go(mj, mi)
