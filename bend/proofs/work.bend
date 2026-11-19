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
import ../hub/wctx.bend as HWC
import ./lib/hub-2.bend as PL2
import ../laws/work.bend as Laws

def Laws.task_flag_replaces_turn_works(ws, s, from):
  {==}

def Laws.notice_carries_no_works(s):
  {==}

def Laws.his_words_follow_a_single_task(s, to):
  {==}

def Laws.his_words_pick_the_rule_by_their_recipient(s, to):
  {==}

def Laws.his_plain_words_to_main_carry_no_works(s):
  {==}

def Laws.his_words_to_two_tasks_carry_none(a, b):
  {==}

def Laws.his_task_tab_words_carry_its_task(ws, s, to, text, queued, parts):
  {==}

def Laws.task_tab_keeps_its_work(ws, s, to):
  {==}

def Laws.delivered_sets_turn_works(was, now):
  {==}

def Laws.old_steered_line_still_unions(was, now):
  {==}

def Laws.steered_keeps_turn_works(cur, got):
  {==}

def Laws.agent_task_flag_replaces_all(ws, s, from, r):
  {==}

def Laws.reply_carries_its_targets_works(h, t, turn):
  {==}

def Laws.reply_to_no_works_carries_the_turns(turn):
  {==}

def Laws.reply_to_a_gone_message_carries_the_turns(turn):
  {==}

def Laws.a_plain_send_carries_the_turns(s, from):
  {==}

def Laws.a_non_agents_reply_carries_none(s, id):
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

def Laws.a_restart_by_main_never_says_him(id, name):
  {==}

def Laws.a_move_by_main_says_main(t, from):
  {==}

def Laws.a_move_as_him_is_his_gesture_only(from):
  {==}

def Laws.work_restart_changes_no_field(w, by, at):
  {==}

def Laws.restart_note_carries_only_its_work(n, id, text):
  {==}

def Laws.restart_records_whom_it_acted_on(id, by, at, rs, ns):
  {==}

def Laws.a_change_records_no_whom(id, op, by, at):
  {==}

def Laws.restart_restores_a_failed_agent(idle):
  {==}

def Laws.restart_tells_a_failed_agent_once(idle):
  {==}

def Laws.restart_notes_an_idle_agent():
  {==}

def Laws.restart_leaves_a_busy_agent(failed):
  match failed:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.restart_leaves_a_stopped_agent(idle, failed):
  {==}

def Laws.restart_on_a_closed_work_is_refused(f, p, id):
  {==}

def Laws.restart_on_a_paused_work_is_refused(at, id):
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

def Laws.status_is_a_function_of_facts(w, f, s, rts, ws, n, prs, now):
  %Equal.sym(Maybe<&2, HS.Fin>, HS.Work.done(HS.Work.with_done(w, Some{f})), Some{f}, HSL.Work.done.get_with(w, Some{f})) : {HWS.st.done(_, HS.Work.with_done(w, Some{f}), s, rts, ws, n, prs, now) == HWS.WSDone{} : HWS.WSt}
  {==}

def Laws.all_merged_reads_merged_never_done(w, s, ws, now):
  {==}

def Laws.an_open_pr_never_reads_merged(id, k, m, t):
  %Equal.sym(Bool, String.eq(id, id), True{}, PL2.hub_str_eq_refl(id)) : {HLZ.pick(Bool, _, u => Nat.is_eq(1n+k, 0n) && Bool.not(Nat.is_eq(m, 0n)), u => HWS.merged(t, id)) == False{} : Bool}
  {==}

def Laws.pr_states_is_the_whole_set(r, ws, os, ms):
  match r:
    case HX.R{a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, a16, a17}:
      {==}

def Laws.paused_wins_over_the_rest(p, w, s, rts, ws, n, prs, now):
  {==}

def Laws.his_waits_make_needs_you(k, w, s, rts, ws, prs, now):
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

# the R of a context after with_r is that R
def cx_r_with_r(-o: HS.St, cx: HX.Cx<o>, +r: HX.R) -> {HX.Cx.r(o, HX.with_r(o, cx, r)) == r : HX.R}:
  match cx:
    case HX.Cx{c, r0}:
      {==}

# the page waits of R.with_pw(x, v) are v
def r_pw_with(x: HX.R, +v: List<&2, HX.Kn>) -> {HX.R.pw(HX.R.with_pw(x, v)) == v : List<&2, HX.Kn>}:
  match x:
    case HX.R{a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, a16, prs}:
      {==}

def Laws.page_waits_is_the_whole_set(o, cx, ws, ns):
  %Equal.sym(HX.R, HX.Cx.r(o, HX.with_r(o, cx, HX.R.with_pw(HC.rr(o, cx), HC.page_waits(ws, ns)))), HX.R.with_pw(HC.rr(o, cx), HC.page_waits(ws, ns)), cx_r_with_r(o, cx, HX.R.with_pw(HC.rr(o, cx), HC.page_waits(ws, ns)))) : {HX.R.pw(_) == HC.page_waits(ws, ns) : List<&2, HX.Kn>}
  r_pw_with(HC.rr(o, cx), HC.page_waits(ws, ns))

def Laws.no_page_waits_no_needs(ns, id):
  {==}

def Laws.moved_by_you_sticks(c, ws):
  %Equal.sym(Bool, Nat.is_eq(HS.Card.id(c), HS.Card.id(c)), True{}, eq_refl(HS.Card.id(c))) : {HLZ.pick(List<&2, HS.Card>, _, u => HS.Card.with_moved(HS.Card.with_works(c, ws), True{}) <> Nil{}, u => c <> HM.card_moved(Nil{}, HS.Card.id(c), ws)) == [HS.Card.with_moved(HS.Card.with_works(c, ws), True{})] : List<&2, HS.Card>}
  {==}

# ---- a blocked report is a card ----

def Laws.blocked_report_opens_one_card(o, cx, agent, text, i, t, e):
  %Equal.sym(List<&2, Nat>, HC.card_ids(HS.St.cards(HC.st(o, cx)), agent, Nil{}), i <> t, e)
    : {HC.blocked_card.go(o, List.is_empty(&2, Nat, _), cx, agent, text) == cx : HX.Cx<o>}
  {==}

def Laws.blocked_card_ends_when_it_moves_on(c, t, a, r, e):
  %Equal.sym(Bool, HS.Card.from_report(c), True{}, r)
    : {Bool.pick(List<&2, Nat>, _ && String.eq(HS.Card.agent(c), a), HS.Card.id(c) <> HC.report_card_ids(t, a), HC.report_card_ids(t, a)) == HS.Card.id(c) <> HC.report_card_ids(t, a) : List<&2, Nat>}
  %Equal.sym(Bool, String.eq(HS.Card.agent(c), a), True{}, e)
    : {Bool.pick(List<&2, Nat>, True{} && _, HS.Card.id(c) <> HC.report_card_ids(t, a), HC.report_card_ids(t, a)) == HS.Card.id(c) <> HC.report_card_ids(t, a) : List<&2, Nat>}
  {==}

def Laws.a_notify_blocked_card_never_ends_on_moved_on(c, t, a, r):
  %Equal.sym(Bool, HS.Card.from_report(c), False{}, r)
    : {Bool.pick(List<&2, Nat>, _ && String.eq(HS.Card.agent(c), a), HS.Card.id(c) <> HC.report_card_ids(t, a), HC.report_card_ids(t, a)) == HC.report_card_ids(t, a) : List<&2, Nat>}
  {==}

def Laws.his_answer_on_a_blocked_card_reaches_its_agent(o, cx, c, id, text, parts, via, r):
  %Equal.sym(Bool, HS.Card.from_report(c), True{}, r)
    : {HLZ.pick(HX.Cx<o>, _, u => HC.answer.blocked(o, cx, c, text, parts, via), u => HC.answer.kind(o, HC.card_kind(HS.Card.kind(c)), cx, c, text, parts, via)) == HC.answer.blocked(o, cx, c, text, parts, via) : HX.Cx<o>}
  {==}

def Laws.blocked_answer_carries_its_cards_works(c, text, parts):
  {==}

# ---- a task's birth ----

def Laws.work_birth_never_takes_a_taken_id(o, cx, a, by, w, e):
  %Equal.sym(Bool, HWO.taken(HC.st(o, cx), HS.Work.id(w)), True{}, e)
    : {HLZ.pick(HX.Cx<o>, _, u => HC.task_err(o, a, cx, HC.cat(["task id ", HS.Work.id(w), " is taken: try another name"])), u => HC.req_task.born.go(o, cx, a, by, w))
      == HC.task_err(o, a, cx, HC.cat(["task id ", HS.Work.id(w), " is taken: try another name"])) : HX.Cx<o>}
  {==}

def Laws.work_birth_of_a_free_id(o, cx, a, by, w, e):
  %Equal.sym(Bool, HWO.taken(HC.st(o, cx), HS.Work.id(w)), False{}, e)
    : {HLZ.pick(HX.Cx<o>, _, u => HC.task_err(o, a, cx, HC.cat(["task id ", HS.Work.id(w), " is taken: try another name"])), u => HC.req_task.born.go(o, cx, a, by, w))
      == HC.req_task.born.go(o, cx, a, by, w) : HX.Cx<o>}
  {==}

def Laws.work_id_unique_next(st, slug, n, f, e):
  %Equal.sym(Bool, HWO.taken(st, HWO.free.id(slug, n)), False{}, e)
    : {HWO.taken(st, HLZ.pick(String, _, u => HWO.free.go(f, st, slug, 1n+n), u => HWO.free.id(slug, n))) == False{} : Bool}
  e

def Laws.work_birth_takes_turn(o, cx, token, by, id):
  {==}

def Laws.his_work_birth_takes_no_turn(o, cx, by, id):
  {==}

def Laws.main_never_holds_a_work_out_of_review(ns, s):
  {==}

def Laws.a_reply_watch_names_who_it_waits_on(to, msg):
  {==}

# ---- a joining agent's context: fresh finds the work by its own id, then
# reads whether the agent is in its agents ----

def Laws.work_context_only_for_new_works(w, agent, e):
  %Equal.sym(Bool, String.eq(HS.Work.id(w), HS.Work.id(w)), True{}, PL2.hub_str_eq_refl(HS.Work.id(w)))
    : {Bool.pick(List<&2, String>, HWC.fresh.one(HLZ.pick(Maybe<&2, HS.Work>, _, u => Some{w}, u => HWO.find(Nil{}, HS.Work.id(w))), agent), [HS.Work.id(w)], Nil{}) == Nil{} : List<&2, String>}
  %Equal.sym(Bool, HWO.has(HS.Work.agents(w), agent), True{}, e)
    : {Bool.pick(List<&2, String>, Bool.not(_), [HS.Work.id(w)], Nil{}) == Nil{} : List<&2, String>}
  {==}

def Laws.work_context_for_a_joined_work(w, agent, e):
  %Equal.sym(Bool, String.eq(HS.Work.id(w), HS.Work.id(w)), True{}, PL2.hub_str_eq_refl(HS.Work.id(w)))
    : {Bool.pick(List<&2, String>, HWC.fresh.one(HLZ.pick(Maybe<&2, HS.Work>, _, u => Some{w}, u => HWO.find(Nil{}, HS.Work.id(w))), agent), [HS.Work.id(w)], Nil{}) == [HS.Work.id(w)] : List<&2, String>}
  %Equal.sym(Bool, HWO.has(HS.Work.agents(w), agent), False{}, e)
    : {Bool.pick(List<&2, String>, Bool.not(_), [HS.Work.id(w)], Nil{}) == [HS.Work.id(w)] : List<&2, String>}
  {==}

def Laws.work_context_is_capped(ids, st, pw):
  {==}

# ---- joining a task ----

def Laws.a_steer_never_joins(cur, got):
  {==}

def Laws.turn_start_joins_its_batch(cur, got, e):
  %Equal.sym(Bool, HWO.same(got, cur), False{}, e) : {HWO.turn_change.go(_, got) == Some{got} : Maybe<&2, List<&2, String>>}
  {==}

def Laws.acting_joins_its_works(w, agent, e):
  %Equal.sym(Bool, String.eq(HS.Work.id(w), HS.Work.id(w)), True{}, PL2.hub_str_eq_refl(HS.Work.id(w)))
    : {Bool.pick(List<&2, String>, HWO.joined(HLZ.pick(Maybe<&2, HS.Work>, _, u => Some{w}, u => HWO.find(Nil{}, HS.Work.id(w))), agent), Nil{}, [HS.Work.id(w)]) == [HS.Work.id(w)] : List<&2, String>}
  %Equal.sym(Bool, HWO.has(HS.Work.agents(w), agent), False{}, e)
    : {Bool.pick(List<&2, String>, _, Nil{}, [HS.Work.id(w)]) == [HS.Work.id(w)] : List<&2, String>}
  {==}

def Laws.acting_on_its_work_joins_nothing(w, agent, e):
  %Equal.sym(Bool, String.eq(HS.Work.id(w), HS.Work.id(w)), True{}, PL2.hub_str_eq_refl(HS.Work.id(w)))
    : {Bool.pick(List<&2, String>, HWO.joined(HLZ.pick(Maybe<&2, HS.Work>, _, u => Some{w}, u => HWO.find(Nil{}, HS.Work.id(w))), agent), Nil{}, [HS.Work.id(w)]) == Nil{} : List<&2, String>}
  %Equal.sym(Bool, HWO.has(HS.Work.agents(w), agent), True{}, e)
    : {Bool.pick(List<&2, String>, _, Nil{}, [HS.Work.id(w)]) == Nil{} : List<&2, String>}
  {==}

def Laws.work_context_once_per_agent_without_membership(id):
  %Equal.sym(Bool, String.eq(id, id), True{}, PL2.hub_str_eq_refl(id))
    : {HWC.unseen.pick(HLZ.or(_, u => HWO.has(Nil{}, id)), id, Nil{}) == Nil{} : List<&2, String>}
  {==}

# ---- cards ----

def Laws.mains_card_carries_only_its_task(s):
  {==}

def Laws.mains_card_keeps_its_task(ws, s):
  {==}

def Laws.an_archived_askers_card_closes():
  {==}

# ---- "Not on this task" ----

def Laws.only_he_or_main_unlinks_an_agent(w, a, by, xs):
  {==}

def Laws.only_he_or_main_links_an_agent(w, a, by, xs):
  {==}

def Laws.unlink_drops_the_agent(a):
  %Equal.sym(Bool, String.eq(a, a), True{}, PL2.hub_str_eq_refl(a)) : {Bool.pick(List<&2, String>, _, Nil{}, [a]) == Nil{} : List<&2, String>}
  {==}
