# bend/laws/feature.bend: `sb feature retire <name>` (hub/feature.bend,
# core.bend feature_retire), moved from bend/laws/hub.bend. Closed by
# bend/proofs/feature.bend; the gate checks every bend/proofs/*.bend: a
# law here with no proof there fails it.
import Base
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/feature.bend as HFT

# ---- `sb feature retire <name>` (hub/feature.bend, core.bend
# feature_retire; architect m_20561) ----
# A new input (`feature_retire`) with no new journal kind: its events are
# EWorkspace and a message's, which hub_step_replays and the message laws
# cover as they are. The laws below are LOCAL, like the timers': stated on
# feature.bend's decisions and core.bend's feature_retire.one/tell, the
# only path a retire takes (feature_retire.all runs .one over
# HFT.retirees, then one info line to main).

# LAW: a retired workspace no longer lands on the feature, whatever it was
# (every `feature` key goes): its agent's next `sb land` goes to the trunk.
law feature_retire_clears:
  for +ws: J.Json
  for +name: String
  {HFT.lands_on(HFT.unfeature(ws), name) == False{} : Bool}

# LAW: the retire keeps every other field of the workspace (path, branch,
# place, mode...): the agent stays in its worktree.
law feature_retire_keeps_ws:
  for +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>
  for +k: String
  for e: {String.eq(k, "feature") == False{} : Bool}
  {J.get_kv(HFT.unfeature.kvs(kvs), k) == J.get_kv(kvs, k) : Maybe<&2, J.Json>}

# LAW: an archived agent is never retired, and the retire archives no
# one: what it does to an agent is its workspace (an EWorkspace event,
# never an ELifecycle), then its note.
law feature_retire_archives_none:
  for +ws: J.Json
  for +name: String
  {HFT.retiree.life(HS.LArchived{}, ws, name) == False{} : Bool}

law feature_retire_one:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  for +text: String
  {HC.feature_retire.one(o, cx, a, text)
    == HC.feature_retire.tell(o, HS.Agent.life(a), HC.emit(o, cx, HM.EWorkspace{HS.Agent.name(a), HFT.unfeature(HS.Agent.ws(a))}, Unit{}, Unit{}, Unit{}), HS.Agent.name(a), text) : HX.Cx<o>}

# LAW: the retire stops nothing: an agent that is not active (stopped,
# failed) gets its workspace only, never a message that would revive it
# (no spawn); no kill, no interrupt on any path.
law feature_retire_stops_nothing:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +life: HS.Life
  for +n: String
  for +text: String
  for e: {HC.is_active(life) == False{} : Bool}
  {HC.feature_retire.tell(o, life, cx, n, text) == cx : HX.Cx<o>}

# LAW: an active agent gets exactly one message, from the hub, through
# send.thread (never send's revive path), in queued mode: a busy agent
# reads it after its turn (pump keeps a queued-mode message while busy),
# its turn untouched.
law feature_retire_tells_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +text: String
  {HC.feature_retire.tell(o, HS.LActive{}, cx, n, text)
    == HC.fst(o, HC.send.thread(o, None{}, cx, HC.Snd{HM.HUB(), n, text, False{}, None{}, True{}, False{}, True{}, None{}, "", None{}, None{}})) : HX.Cx<o>}
