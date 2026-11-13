# bend/laws/work.bend: tasks (docs/tasks.md; architect m_64423, m_64425):
# what a message, a turn and a change of a work carry (bend/hub/work.bend,
# core.bend's works_of_send and `sb task`). Closed by bend/proofs/work.bend;
# the gate checks every bend/proofs/*.bend: a law here with no proof there
# fails it.
import Base
import ../hub/gen/state.bend as HS
import ../hub/model.bend as HM
import ../hub/core.bend as HC
import ../hub/work.bend as HWO

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

# LAW: his own words carry no works unless he gave them (a Task tab's).
law his_words_carry_no_works:
  for +s: HS.St
  {HC.works_of_send(None{}, s, HM.USER()) == Nil{} : List<&2, String>}

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

# LAW: an agent other than main never marks a work done: only main (on
# his word) or he does.
law an_agent_never_marks_done:
  for +w: HS.Work
  for +by: String
  for +xs: List<&2, HS.Work>
  {HC.task_refused(w, HS.WDone{}, by, False{}, xs) == Some{"only main changes a task: tick your done-when with sb report --done-when <n>"} : Maybe<&2, String>}
