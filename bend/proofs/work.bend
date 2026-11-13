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
import ../hub/view.bend as HV
import ../hub/keyed.bend as HKY
import ../hub/lazy.bend as HLZ
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

def Laws.an_agent_never_acts_as_him(t, from):
  {==}

def Laws.only_main_changes_any_work(t, from):
  {==}

def Laws.paused_work_holds_its_deliveries(m, rest):
  {==}

def Laws.a_message_with_no_work_is_never_held(xs):
  {==}

def Laws.pause_never_stops_an_idle_agent(ids, xs, now):
  {==}

def Laws.pause_never_stops_an_agent_with_an_open_work(now):
  {==}

def Laws.pause_never_stops_a_turn_with_no_work(xs, now):
  {==}

def Laws.pause_interrupts_only_after_grace(at, now):
  {==}

def Laws.pause_queues_one_resume_note_per_agent(n, id):
  {==}

def Laws.stale_until_never_resumes(u, now):
  {==}

def Laws.until_follows_the_last_pause(p, at, now):
  {==}

def Laws.pause_without_end_never_resumes(p, now):
  {==}

def Laws.a_closed_work_holds_nothing(w, by, at):
  HSL.Work.paused.get_with(HS.Work.with_until(HS.Work.with_until_timer(HS.Work.with_done(w, Some{HWO.fin(at, by)}), None{}), None{}), None{})

def Laws.a_merged_work_holds_nothing(into, w, by, at):
  HSL.Work.paused.get_with(HS.Work.with_until(HS.Work.with_until_timer(HS.Work.with_done(HS.Work.with_merged_into(w, Some{into}), Some{HWO.fin(at, by)}), None{}), None{}), None{})

def Laws.his_words_are_never_held(ids, xs):
  {==}

def Laws.a_done_work_folds_after_seven_days(f, now):
  {==}

def Laws.an_open_work_never_folds(now):
  {==}

def Laws.status_is_a_function_of_facts(w, f, s, rts, ws, n, now):
  %Equal.sym(Maybe<&2, HS.Fin>, HS.Work.done(HS.Work.with_done(w, Some{f})), Some{f}, HSL.Work.done.get_with(w, Some{f})) : {HWS.st.done(_, HS.Work.with_done(w, Some{f}), s, rts, ws, n, now) == HWS.WSDone{} : HWS.WSt}
  {==}

def Laws.paused_wins_over_the_rest(p, w, s, rts, ws, n, now):
  {==}

def Laws.his_waits_make_needs_you(k, w, s, rts, ws, now):
  {==}

def Laws.a_card_event_touches_only_its_works(c, cs0):
  {==}

# n equals itself
def eq_refl(n: Nat) -> {Nat.is_eq(n, n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      eq_refl(p)

def Laws.same_page_waits_leave_its_row(pw, id):
  %Equal.sym(Bool, Nat.is_eq(HKY.kn_get(pw, id), HKY.kn_get(pw, id)), True{}, eq_refl(HKY.kn_get(pw, id))) : {Bool.not(_) == False{} : Bool}
  {==}

def Laws.moved_by_you_sticks(c, ws):
  %Equal.sym(Bool, Nat.is_eq(HS.Card.id(c), HS.Card.id(c)), True{}, eq_refl(HS.Card.id(c))) : {HLZ.pick(List<&2, HS.Card>, _, u => HS.Card.with_moved(HS.Card.with_works(c, ws), True{}) <> Nil{}, u => c <> HM.card_moved(Nil{}, HS.Card.id(c), ws)) == [HS.Card.with_moved(HS.Card.with_works(c, ws), True{})] : List<&2, HS.Card>}
  {==}
