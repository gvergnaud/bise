# bend/laws/work.bend: tasks (docs/tasks.md; architect m_64423, m_64425):
# what a message, a turn and a change of a work carry (bend/hub/work.bend,
# core.bend's works_of_send and `sb task`). Closed by bend/proofs/work.bend;
# the gate checks every bend/proofs/*.bend: a law here with no proof there
# fails it.
import Base
import ../hub/gen/state.bend as HS
import ../hub/gen/state-codec.bend as HSC
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/core.bend as HC
import ../hub/work.bend as HWO
import ../hub/cx.bend as HX
import ../hub/wstatus.bend as HWS

# LAW: `--task <id>` gives the message its works, whatever the sender's
# turn serves.
law task_flag_replaces_turn_works:
  for +ws: List<&2, String>
  for +s: HS.St
  for +from: String
  {HC.works_of_send(Some{ws}, s, from) == ws : List<&2, String>}

# LAW: a notice of the hub carries no works: the hub is never an agent,
# so it has no turn to inherit from.
law notice_carries_no_works:
  for +s: HS.St
  {HC.works_of_send(None{}, s, HM.HUB()) == Nil{} : List<&2, String>}

# LAW: his message to an agent (outside a Task tab) carries the work the
# agent's turn serves when it is exactly one, else none (architect
# m_64507). Read from St at the send: the Msg records what it got, so a
# replay never recomputes it from a later turn; a wrong attach is his to
# move (moved_by_you_sticks).
law his_words_follow_a_single_task:
  for +s: HS.St
  for +to: String
  {HC.works_his(None{}, s, to) == HC.single(HC.works_of_agent(HM.get_agent(s, to))) : List<&2, String>}

# LAW: the edge: the recipient serves two works, his message carries none.
law his_words_to_two_tasks_carry_none:
  for +a: String
  for +b: String
  {HC.single([a, b]) == Nil{} : List<&2, String>}

# LAW: a Task tab's message keeps its own work, whatever the recipient
# serves.
law task_tab_keeps_its_work:
  for +ws: List<&2, String>
  for +s: HS.St
  for +to: String
  {HC.works_his(Some{ws}, s, to) == ws : List<&2, String>}

# LAW: a delivery that starts a turn sets the turn's works to the
# batch's.
law delivered_sets_turn_works:
  for +was: List<&2, String>
  for +now: List<&2, String>
  {HWO.turn_works(was, now, False{}) == now : List<&2, String>}

# LAW: a delivery steered into a running turn adds its works to the
# turn's.
law steered_adds_turn_works:
  for +was: List<&2, String>
  for +now: List<&2, String>
  {HWO.turn_works(was, now, True{}) == HWO.union(was, now) : List<&2, String>}

# LAW: a name he gave stays his: a rename by him marks it, a rename by an
# agent never clears the mark.
law named_by_you_sticks:
  for +w: HS.Work
  for +name: String
  for +by: String
  for +at: Nat
  {HS.Work.named_by_you(HWO.change.op(HS.WRename{name}, w, by, at)) == (HS.Work.named_by_you(w) || String.eq(by, "user")) : Bool}

# LAW: an agent's line from before tasks (no `turn_works` key) reads a
# turn that serves no work (architect m_64502; Msg's and Card's `works`
# are pinned by fixtures/core_state_defaults.jsonl's lines).
law old_agent_reads_no_turn_works:
  {HS.Agent.turn_works(HSC.Agent.photo_of(J.obj([J.kv("name", J.JStr{"docs"}), J.kv("dir", J.JStr{"docs"}), J.kv("life", J.JStr{"active"})]))) == Nil{} : List<&2, String>}

# LAW: an agent other than main never marks a work done: only main (on
# his word) or he does.
law an_agent_never_marks_done:
  for +w: HS.Work
  for +by: String
  for +xs: List<&2, HS.Work>
  {HC.task_refused(w, HS.WDone{}, by, False{}, xs) == Some{"only main changes a task: tick your done-when with sb report --done-when <n>"} : Maybe<&2, String>}

# ---- status (bend/hub/wstatus.bend; architect m_64423: a function of
# facts, never stored, never an agent's word) ----

# LAW: the status is a function of facts, the first that holds in the
# page's order: a done work is Done whatever its agents, cards and clock.
law status_is_a_function_of_facts:
  for +w: HS.Work
  for +f: HS.Fin
  for +s: HS.St
  for +rts: List<&2, HX.Rt>
  for +ws: List<&2, HX.Waiter>
  for +n: Nat
  for +now: Nat
  {HWS.status(HS.Work.with_done(w, Some{f}), s, rts, ws, n, now) == HWS.WSDone{} : HWS.WSt}

# LAW: a paused work reads Paused (with its until) before anything waits
# on him or runs.
law paused_wins_over_the_rest:
  for +p: Nat
  for +w: HS.Work
  for +s: HS.St
  for +rts: List<&2, HX.Rt>
  for +ws: List<&2, HX.Waiter>
  for +n: Nat
  for +now: Nat
  {HWS.st.paused(Some{p}, w, s, rts, ws, n, now) == HWS.WSPaused{HS.Work.until(w)} : HWS.WSt}

# LAW: an open card or page wait of his on a work makes it Needs you,
# whatever its agents do.
law his_waits_make_needs_you:
  for +k: Nat
  for +w: HS.Work
  for +s: HS.St
  for +rts: List<&2, HX.Rt>
  for +ws: List<&2, HX.Waiter>
  for +now: Nat
  {HWS.st.needs(1n+k, w, s, rts, ws, now) == HWS.WSNeeds{1n+k} : HWS.WSt}
