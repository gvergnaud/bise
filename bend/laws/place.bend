# bend/laws/place.bend: a worktree task is placed off the hub's loop
# (hub-loop, docs/issues/32; hub/place.bend, core.bend's spawn, placed,
# place.ask). Closed by bend/proofs/place.bend; the gate checks every
# bend/proofs/*.bend: a law here with no proof there fails it.
#
# A new input (`placed`), two new effects (`worktree_create`,
# `worktree_drop`) and one new journal kind (`worktree_placed`, its
# replay below; hub_step_replays covers the step as it is). The laws are
# LOCAL, stated on place.bend's decision and core.bend's glue, the only
# path a `placed` takes (dispatch: placed(o, PL.placed_of(st, ..), cx)).
import Base
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/place.bend as HPL
import ../hub/gen/fx.bend as HF
import ../hub/gen/feed.bend as FD

# LAW: no REPL starts for a task whose worktree is still being made:
# spawn (every path: the create, a message to a down task, a restore)
# changes nothing while its workspace is placing.
law no_repl_before_placed:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +r: Bool
  for e: {HPL.waits(HM.get_agent(HC.st(o, cx), n)) == True{} : Bool}
  {HC.spawn(o, cx, n, r) == cx : HX.Cx<o>}

# LAW: a task is placed once: a `placed` for a task no longer placing
# (placed already, or never) never starts it again, whatever it says.
law placed_create_once:
  for +a: HS.Agent
  for +n: String
  for +ws: J.Json
  for +err: String
  for e: {HM.ws_placing(HS.Agent.ws(a)) == False{} : Bool}
  {HPL.is_ok(HPL.placed_of.agent(Some{a}, n, ws, err)) == False{} : Bool}

# LAW: a worktree that could not be made fails its live placing task
# (a new one's, BNew), with git's words, and names its spawner.
law placed_err_fails:
  for +a: HS.Agent
  for +n: String
  for +ws: J.Json
  for +err: String
  for e: {String.is_empty(err) == False{} : Bool}
  {HPL.placed_of.made(HPL.BNew{}, a, n, ws, err) == HPL.PErr{n, err, HPL.parent_of(HS.Agent.parent(a)), HPL.BNew{}} : HPL.Do}

# LAW: a new task's worktree made: the journal's line, main's line, then
# its REPL starts fresh.
law placed_new_starts:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +ws: J.Json
  {HC.placed(o, HPL.POk{n, ws, HPL.BNew{}}, cx)
    == HC.spawn(o, HC.feed(o, HC.emit(o, cx, HM.EPlaced{n, ws, ""}, Unit{}, Unit{}, Unit{}), HM.MAIN(), FD.info(HPL.ready_text(n, ws))), n, False{}) : HX.Cx<o>}

# LAW: that failure is one stop (as failed, with git's words) and one
# word to the spawner (placed.tell: a message, or main's feed for his
# own /new), after the journal's line; nothing else.
law placed_err_tells_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +err: String
  for +parent: String
  {HC.placed(o, HPL.PErr{n, err, parent, HPL.BNew{}}, cx)
    == HC.placed.tell(o, String.eq(parent, HM.USER()),
      HC.stop_noted(o, HC.emit(o, cx, HM.EPlaced{n, J.JNull{}, err}, Unit{}, Unit{}, Unit{}), n, HS.LFailed{}, err),
      parent, HPL.err_text(n, err)) : HX.Cx<o>}

# LAW: a worktree made for a task gone meanwhile (dropped while placing)
# goes again, and that is all the step does.
law placed_orphan_drops_only:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +ws: J.Json
  {HC.placed(o, HPL.POrphan{n, ws}, cx) == HC.out(o, cx, HF.worktree_drop(n, ws, 0n, 0n)) : HX.Cx<o>}

# LAW: the journal's `worktree_placed` replays as the workspace it carries
# (a restart rebuilds the placed task as it was).
law placed_replays_as_ws:
  for +st: HS.St
  for +n: String
  for +ws: J.Json
  {HM.apply(st, HM.EPlaced{n, ws, ""}) == HM.apply(st, HM.EWorkspace{n, ws}) : HS.St}

# ---- a join while placing (`sb spawn B --place A`, A placing; rows233B) ----

# LAW: a join on a placing agent never makes a worktree of its own: its
# ws carries A's place, and place.ask emits no worktree_create for it.
law place_join_no_create:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +ws: J.Json
  for +p: String
  for e: {J.get_str(ws, "place") == Some{p} : Maybe<&2, String>}
  {HC.place.ask(o, HPL.makes(ws), cx, n, ws) == cx : HX.Cx<o>}

# LAW: no REPL for the joiner before its place is made: its ws is placing,
# so spawn changes nothing (no_repl_before_placed, on the joiner).
law place_join_waits:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +b: String
  for +r: Bool
  for e: {HPL.waits(HM.get_agent(HC.st(o, cx), b)) == True{} : Bool}
  {HC.spawn(o, cx, b, r) == cx : HX.Cx<o>}

# LAW: A's worktree made, a live placing joiner gets that same ws (path,
# branch, place), then its REPL (POk).
law place_join_shares:
  for +a: HS.Agent
  for +b: String
  for +n: String
  for +ws: J.Json
  for +bk: HPL.Back
  for +err: String
  {HPL.placed_of.made(HPL.BNew{}, a, b, HPL.join.ws(HPL.POk{n, ws, bk}), HPL.join.err(HPL.POk{n, ws, bk}, err)) == HPL.POk{b, ws, HPL.BNew{}} : HPL.Do}

# LAW: A's worktree not made, a live placing joiner fails too, with the
# same words, and its spawner is told (PErr, then placed_err_tells_once).
law place_join_err_fails_both:
  for +a: HS.Agent
  for +b: String
  for +n: String
  for +e: String
  for +p: String
  for +bk: HPL.Back
  for +err: String
  for h: {String.is_empty(e) == False{} : Bool}
  {HPL.placed_of.made(HPL.BNew{}, a, b, HPL.join.ws(HPL.PErr{n, e, p, bk}), HPL.join.err(HPL.PErr{n, e, p, bk}, err))
    == HPL.PErr{b, e, HPL.parent_of(HS.Agent.parent(a)), HPL.BNew{}} : HPL.Do}

# ---- a drop off the loop (issue 32 3b: `worktree_drop`, `unplaced`) ----

# LAW: a drop commits the archive first (the task stopped, its ws
# `dropping` with what to save), then asks the shell for the removal: the
# step never waits on git.
law drop_fx_after_archive:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  for +d: Nat
  for +u: Nat
  {HC.drop.off(o, cx, a, d, u)
    == HC.out(o, HC.emit(o, HC.stop_noted(o, cx, HS.Agent.name(a), HS.LArchived{}, "drop"),
      HM.EWorkspace{HS.Agent.name(a), HPL.dropping_ws(HS.Agent.ws(a), d, u)}, Unit{}, Unit{}, Unit{}),
      HF.worktree_drop(HS.Agent.name(a), HS.Agent.ws(a), d, u)) : HX.Cx<o>}

# LAW: no restore while the shell still removes the worktree: the answer
# says so, and nothing else moves.
law no_restore_while_dropping:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  {HC.restore.dropping(o, True{}, cx, a) == (cx, HC.TErr{HPL.dropping_text(HS.Agent.name(a))}) : HX.Cx<o> & HC.TaskR}

# LAW: an `unplaced` for a task not dropping (an orphan's worktree, a
# second answer) changes nothing.
law unplaced_not_dropping_inert:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  for +snap: Maybe<&2, String>
  for +err: String
  {HC.unplaced.go(o, False{}, cx, a, snap, err) == cx : HX.Cx<o>}

# LAW: a removal that failed keeps the worktree (no longer dropping, not
# dropped) and the saved work the task had: a /restore reuses it.
law undropped_keeps_worktree:
  for +a: HS.Agent
  for +sn: Maybe<&2, String>
  {HM.agent_op(a, HM.ODrop{False{}, sn}) == HM.agent_op(a, HM.OWs{J.set(HS.Agent.ws(a), "state", J.JStr{""})}) : HS.Agent}

# LAW: the journal's `worktree_dropped` replays as the dropped workspace
# and its saved work's ref (a restart rebuilds the archived task as it was).
law dropped_replays_as_ws:
  for +a: HS.Agent
  for +sn: Maybe<&2, String>
  {HM.agent_op(a, HM.ODrop{True{}, sn})
    == HM.agent_op(HM.agent_op(a, HM.OWs{J.set(J.set(HS.Agent.ws(a), "state", J.JStr{""}), "dropped", J.JBool{True{}})}), HM.OSnap{sn}) : HS.Agent}

# LAW: a hub stopped mid-drop asks for the removal again at its boot, with
# what to save (it rode with the ws); a task not dropping gets nothing.
law boot_reemits_stuck_drop:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  {HC.boot.drop(o, True{}, cx, a)
    == HC.out(o, cx, HF.worktree_drop(HS.Agent.name(a), HS.Agent.ws(a), J.get_num_or(HS.Agent.ws(a), "drop_dirty", 0n), J.get_num_or(HS.Agent.ws(a), "drop_unpushed", 0n))) : HX.Cx<o>}

law boot_skips_not_dropping:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  {HC.boot.drop(o, False{}, cx, a) == cx : HX.Cx<o>}

# ---- a restore off the loop (issue 32 3b: `worktree_restore`, `placed`) ----

# LAW: a /restore of a task whose worktree is gone makes the task active
# at once, its ws placing (no REPL: no_repl_before_placed), then asks the
# shell to make the dropped ws again with its saved work.
law restore_off_asks_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  {HC.restore.off(o, cx, a)
    == HC.restore.go.as(o, HC.out(o, HC.emit(o, cx, HM.EWorkspace{HS.Agent.name(a), HPL.restoring_ws(HS.Agent.ws(a))}, Unit{}, Unit{}, Unit{}),
      HF.worktree_restore(HS.Agent.name(a), HS.Agent.ws(a), HS.Agent.snap(a))), a, HPL.restoring_note()) : HX.Cx<o> & HC.TaskR}

# LAW: a restore's worktree made: POk with its Back (its REPL resumes);
# not made: PErr with its Back (back to the dropped ws), git's words.
law placed_restored:
  for +a: HS.Agent
  for +n: String
  for +ws: J.Json
  for +from: J.Json
  {HPL.placed_of.made(HPL.BRestore{from}, a, n, ws, "") == HPL.POk{n, ws, HPL.BRestore{from}} : HPL.Do}

law placed_unrestored:
  for +a: HS.Agent
  for +n: String
  for +ws: J.Json
  for +from: J.Json
  for +err: String
  for e: {String.is_empty(err) == False{} : Bool}
  {HPL.placed_of.made(HPL.BRestore{from}, a, n, ws, err) == HPL.PErr{n, err, HPL.parent_of(HS.Agent.parent(a)), HPL.BRestore{from}} : HPL.Do}

# LAW: a restore made again: its ws, its saved work used (no snapshot
# left), then its REPL resumes.
law restore_ok_clears_snapshot:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +ws: J.Json
  for +from: J.Json
  {HC.placed(o, HPL.POk{n, ws, HPL.BRestore{from}}, cx)
    == HC.spawn(o, HC.feed(o, HC.emit(o, HC.emit(o, cx, HM.EPlaced{n, ws, ""}, Unit{}, Unit{}, Unit{}), HM.ESnapshot{n, None{}}, Unit{}, Unit{}, Unit{}),
      HM.MAIN(), FD.info(HPL.ready_text(n, ws))), n, True{}) : HX.Cx<o>}

# LAW: a restore not made again: the task goes back archived in the
# dropped ws it came from (its snapshot untouched), git's words in main's
# feed; no REPL.
law restore_err_keeps_archived:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +e: String
  for +p: String
  for +from: J.Json
  {HC.placed(o, HPL.PErr{n, e, p, HPL.BRestore{from}}, cx)
    == HC.feed(o, HC.stop_noted(o, HC.emit(o, HC.emit(o, cx, HM.EPlaced{n, J.JNull{}, e}, Unit{}, Unit{}, Unit{}), HM.EWorkspace{n, from}, Unit{}, Unit{}, Unit{}),
      n, HS.LArchived{}, e), HM.MAIN(), FD.warn(HPL.unrestored_text(n, e))) : HX.Cx<o>}

# LAW: a hub stopped mid-restore asks for it again at its boot (a mate
# that joined it waits for the same: boot_back skips it).
law boot_reemits_stuck_restore:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  for +from: J.Json
  {HC.boot.place(o, HPL.BRestore{from}, cx, a) == HC.out(o, cx, HF.worktree_restore(HS.Agent.name(a), from, HS.Agent.snap(a))) : HX.Cx<o>}

law boot_skips_restore_joiner:
  for +ws: J.Json
  {HPL.boot_back.go(True{}, ws) == HPL.BNew{} : HPL.Back}

# ---- an isolate and a feature off the loop (issue 32 3c) ----

# LAW: /isolate stops the REPL, makes its ws placing (no REPL until
# placed: no_repl_before_placed), then asks the shell for the worktree;
# its answer says so, nothing waits on git.
law isolate_off_asks_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  {HC.isolate.off(o, cx, a)
    == (HC.feed(o, HC.out(o, HC.emit(o, HC.upd_rt(o, HC.out(o, cx, HF.kill(HS.Agent.name(a))), HX.Rt.with_run(HC.rt(o, HC.out(o, cx, HF.kill(HS.Agent.name(a))), HS.Agent.name(a)), HS.RDown{})),
        HM.EWorkspace{HS.Agent.name(a), HPL.isolating_ws(HS.Agent.ws(a))}, Unit{}, Unit{}, Unit{}), HF.worktree_create(HS.Agent.name(a), False{}, "")),
      HM.MAIN(), FD.info(HPL.isolating_text(HS.Agent.name(a)))), HC.TOk{HS.Agent.name(a)}) : HX.Cx<o> & HC.TaskR}

# LAW: an isolate's worktree made: its REPL resumes there, then is told
# its new folder.
law isolate_ok_resumes_there:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +ws: J.Json
  for +from: J.Json
  {HC.placed(o, HPL.POk{n, ws, HPL.BIsolate{from}}, cx)
    == HC.fst(o, HC.send(o, HC.spawn(o, HC.feed(o, HC.emit(o, cx, HM.EPlaced{n, ws, ""}, Unit{}, Unit{}, Unit{}), HM.MAIN(), FD.info(HPL.isolated_text(n, ws))), n, True{}),
      HC.msg(HM.USER(), n, HPL.isolated_note(ws), False{}, None{}, True{}, False{}))) : HX.Cx<o>}

# LAW: an isolate's worktree not made: back in the shared ws it had, its
# REPL resumed there, git's words in main's feed.
law isolate_err_back_shared:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +e: String
  for +p: String
  for +from: J.Json
  {HC.placed(o, HPL.PErr{n, e, p, HPL.BIsolate{from}}, cx)
    == HC.spawn(o, HC.feed(o, HC.emit(o, HC.emit(o, cx, HM.EPlaced{n, J.JNull{}, e}, Unit{}, Unit{}, Unit{}), HM.EWorkspace{n, from}, Unit{}, Unit{}, Unit{}),
      HM.MAIN(), FD.warn(HPL.unisolated_text(n, e))), n, True{}) : HX.Cx<o>}

# LAW: a placing ws's worktree_create carries its feature (`sb spawn
# --feature`: from the feature's tip; "": main's).
law feature_rides_the_fx:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +ws: J.Json
  {HC.place.ask(o, True{}, cx, n, ws) == HC.out(o, cx, HF.worktree_create(n, J.get_bool_or(ws, "with_changes", False{}), HPL.ws_feature(ws))) : HX.Cx<o>}
