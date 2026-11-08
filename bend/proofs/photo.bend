# bend/proofs/photo.bend: the proofs of bend/laws/photo.bend. The photo
# reads back what it wrote (gen/state-codec.bend's St.photo_round); a
# batch's state does not depend on its counters (the index and the
# skipped list only feed the answer), so a replay of p ++ t is the
# replay of t from the state after p.
import Base
import ../vendor/json.bend as J
import ../hub/photo.bend as P
import ../hub/cold.bend as CO
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/gen/state-codec.bend as HSC
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../laws/photo.bend as Laws

# a batch's state is the same whatever its counters start at
def counters(+ms: List<&2, Maybe<&2, HM.Ev>>, +st: HS.St, +i: Nat, +sk: List<&2, Nat>, +j: Nat, +sk2: List<&2, Nat>) -> {P.st_of(CO.replay_batch(ms, st, i, sk)) == P.st_of(CO.replay_batch(ms, st, j, sk2)) : HS.St}:
  match ms:
    case Nil{}:
      {==}
    case Con{None{}, rest}:
      counters(rest, st, 1n+i, i <> sk, 1n+j, j <> sk2)
    case Con{Some{e}, rest}:
      counters(rest, CO.replay_one(CO.replay_fresh(st, e), st, e), 1n+i, Bool.pick(List<&2, Nat>, CO.replay_fresh(st, e), sk, i <> sk), 1n+j, Bool.pick(List<&2, Nat>, CO.replay_fresh(st, e), sk2, j <> sk2))

# a batch of p ++ t: the batch of t from the state after p
def append_split(+p: List<&2, Maybe<&2, HM.Ev>>, +t: List<&2, Maybe<&2, HM.Ev>>, +st: HS.St, +i: Nat, +sk: List<&2, Nat>) -> {P.st_of(CO.replay_batch(List.append(&2, Maybe<&2, HM.Ev>, p, t), st, i, sk)) == P.st_of(CO.replay_batch(t, P.st_of(CO.replay_batch(p, st, i, sk)), 0n, Nil{})) : HS.St}:
  match p:
    case Nil{}:
      counters(t, st, i, sk, 0n, Nil{})
    case Con{None{}, rest}:
      append_split(rest, t, st, 1n+i, i <> sk)
    case Con{Some{e}, rest}:
      append_split(rest, t, CO.replay_one(CO.replay_fresh(st, e), st, e), 1n+i, Bool.pick(List<&2, Nat>, CO.replay_fresh(st, e), sk, i <> sk))

def Laws.photo_is_the_state(d, r, f, q):
  %Equal.sym(Maybe<&2, J.Json>, HX.R.need(r), None{}, q) : {P.photo.go(_, d) == Some{d} : Maybe<&2, HS.St>}
  {==}

def Laws.restore_keeps_the_state(d, x, r):
  %HSC.St.photo_round(d) : {P.restore(HC.Hub{x, r, HC.Fresh{}}, HSC.St.photo_json(d)) == Some{HC.Hub{_, P.fresh(HX.R.workspace(r)), HC.Restored{}}} : Maybe<&2, HC.Hub>}
  {==}

def Laws.restore_only_fresh(x, r, j):
  {==}

def Laws.restore_once(x, r, j):
  {==}

def Laws.restore_more_prepends(a, x, r):
  %HSC.St.photo_round(a) : {P.restore_more(HC.Hub{x, r, HC.Restored{}}, HSC.St.photo_json(a)) == Some{HC.Hub{HSC.St.slice_join(_, x), r, HC.Restored{}}} : Maybe<&2, HC.Hub>}
  {==}

def Laws.restore_more_not_fresh(x, r, j):
  {==}

def Laws.restore_more_not_live(x, r, j):
  {==}

def Laws.restore_in_slices(k, st, r):
  %HSC.St.slice_round(k, st) : {P.restore_more(HC.Hub{HSC.St.slice_drop(k, st), r, HC.Restored{}}, HSC.St.photo_json(HSC.St.slice_take(k, st))) == Some{HC.Hub{_, r, HC.Restored{}}} : Maybe<&2, HC.Hub>}
  %HSC.St.photo_round(HSC.St.slice_take(k, st)) : {P.restore_more(HC.Hub{HSC.St.slice_drop(k, st), r, HC.Restored{}}, HSC.St.photo_json(HSC.St.slice_take(k, st))) == Some{HC.Hub{HSC.St.slice_join(_, HSC.St.slice_drop(k, st)), r, HC.Restored{}}} : Maybe<&2, HC.Hub>}
  {==}

def Laws.replay_from_photo(p, t, st):
  %Equal.sym(HS.St, HSC.St.photo_of(HSC.St.photo_json(P.replayed(p, st))), P.replayed(p, st), HSC.St.photo_round(P.replayed(p, st))) : {P.replayed(List.append(&2, Maybe<&2, HM.Ev>, p, t), st) == P.replayed(t, _) : HS.St}
  append_split(p, t, st, 0n, Nil{})
