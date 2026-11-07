# bend/proofs/place.bend: the proofs of bend/laws/place.bend (hub-loop).
# Each holds by unfolding the definitions involved, after one rewrite.
import Base
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/place.bend as HPL
import ../hub/gen/fx.bend as HF
import ../laws/place.bend as Laws

def Laws.no_repl_before_placed(o, cx, n, r, e):
  %Equal.sym(Bool, HPL.waits(HM.get_agent(HC.st(o, cx), n)), True{}, e) : {HC.spawn.if(o, Bool.not(_) && HC.has_agent(o, cx, n) && HC.is_down(HX.Rt.run(HC.rt(o, cx, n))), cx, n, r) == cx : HX.Cx<o>}
  {==}

# whatever the error says, a task not placing is never started
def place_pick_not_ok(c: Bool, +n: String, +ws: J.Json) -> {HPL.is_ok(Bool.pick(HPL.Do, c, HPL.POrphan{n, ws}, HPL.PNone{})) == False{} : Bool}:
  match c:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.placed_create_once(a, n, ws, err, e):
  %Equal.sym(Bool, HM.ws_placing(HS.Agent.ws(a)), False{}, e) : {HPL.is_ok(HPL.placed_of.live(_ && Bool.not(HPL.is_archived(HS.Agent.life(a))), a, n, ws, err)) == False{} : Bool}
  place_pick_not_ok(String.is_empty(err), n, ws)

def Laws.placed_err_fails(a, n, ws, err, e):
  %Equal.sym(Bool, String.is_empty(err), False{}, e) : {Bool.pick(HPL.Do, _, HPL.POk{n, ws}, HPL.PErr{n, err, HPL.parent_of(HS.Agent.parent(a))}) == HPL.PErr{n, err, HPL.parent_of(HS.Agent.parent(a))} : HPL.Do}
  {==}

def Laws.placed_err_tells_once(o, cx, n, err, parent):
  {==}

def Laws.placed_orphan_drops_only(o, cx, n, ws):
  {==}

def Laws.placed_replays_as_ws(st, n, ws):
  {==}

def Laws.place_join_no_create(o, cx, n, ws, p, e):
  %Equal.sym(Maybe<&2, String>, J.get_str(ws, "place"), Some{p}, e) : {HC.place.ask(o, HPL.no_place(_) && HM.ws_placing(ws), cx, n, ws) == cx : HX.Cx<o>}
  {==}

def Laws.place_join_waits(o, cx, b, r, e):
  Laws.no_repl_before_placed(o, cx, b, r, e)

def Laws.place_join_shares(a, b, n, ws, err):
  {==}

def Laws.place_join_err_fails_both(a, b, n, e, p, err, h):
  Laws.placed_err_fails(a, b, J.JNull{}, e, h)

def Laws.drop_fx_after_archive(o, cx, a, d, u):
  {==}

def Laws.no_restore_while_dropping(o, cx, a):
  {==}

def Laws.unplaced_not_dropping_inert(o, cx, a, snap, err):
  {==}

def Laws.undropped_keeps_worktree(a, sn):
  {==}

def Laws.dropped_replays_as_ws(a, sn):
  {==}

def Laws.boot_reemits_stuck_drop(o, cx, a):
  {==}

def Laws.boot_skips_not_dropping(o, cx, a):
  {==}
