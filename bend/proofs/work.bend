# bend/proofs/work.bend: the proofs of bend/laws/work.bend. Each holds by
# unfolding: works_of_send picks the flag or the sender's turn (the hub
# and he are never agents), turn_works picks set or union, a rename opens
# the work's record, and an agent's done is refused before any check.
import Base
import ../hub/gen/state.bend as HS
import ../hub/gen/state-laws.bend as HSL
import ../hub/model.bend as HM
import ../hub/core.bend as HC
import ../hub/work.bend as HWO
import ../hub/cx.bend as HX
import ../hub/wstatus.bend as HWS
import ../laws/work.bend as Laws

def Laws.task_flag_replaces_turn_works(ws, s, from):
  {==}

def Laws.notice_carries_no_works(s):
  {==}

def Laws.his_words_follow_a_single_task(s, to):
  {==}

def Laws.his_words_to_two_tasks_carry_none(a, b):
  {==}

def Laws.task_tab_keeps_its_work(ws, s, to):
  {==}

def Laws.delivered_sets_turn_works(was, now):
  {==}

def Laws.steered_adds_turn_works(was, now):
  {==}

def Laws.named_by_you_sticks(w, name, by, at):
  HSL.Work.named_by_you.get_with(HS.Work.with_name(w, name), HS.Work.named_by_you(w) || String.eq(by, "user"))

def Laws.an_agent_never_marks_done(w, by, xs):
  {==}

def Laws.old_agent_reads_no_turn_works():
  {==}

def Laws.status_is_a_function_of_facts(w, f, s, rts, ws, n, now):
  %Equal.sym(Maybe<&2, HS.Fin>, HS.Work.done(HS.Work.with_done(w, Some{f})), Some{f}, HSL.Work.done.get_with(w, Some{f})) : {HWS.st.done(_, HS.Work.with_done(w, Some{f}), s, rts, ws, n, now) == HWS.WSDone{} : HWS.WSt}
  {==}

def Laws.paused_wins_over_the_rest(p, w, s, rts, ws, n, now):
  {==}

def Laws.his_waits_make_needs_you(k, w, s, rts, ws, now):
  {==}
