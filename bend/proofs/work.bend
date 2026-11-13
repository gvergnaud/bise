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
import ../laws/work.bend as Laws

def Laws.task_flag_replaces_turn_works(ws, s, from):
  {==}

def Laws.notice_carries_no_works(s):
  {==}

def Laws.his_words_carry_no_works(s):
  {==}

def Laws.delivered_sets_turn_works(was, now):
  {==}

def Laws.steered_adds_turn_works(was, now):
  {==}

def Laws.named_by_you_sticks(w, name, by, at):
  HSL.Work.named_by_you.get_with(HS.Work.with_name(w, name), HS.Work.named_by_you(w) || String.eq(by, "user"))

def Laws.an_agent_never_marks_done(w, by, xs):
  {==}
