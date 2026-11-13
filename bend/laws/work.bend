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
import ../hub/view.bend as HV
import ../hub/keyed.bend as HKY

# LAW: `--task <id>` gives the message its works, whatever the sender's
# turn serves.
law task_flag_replaces_turn_works:
  for +ws: List<&2, String>
  for +s: HS.St
  for +from: String
  {HC.works_of_send(Some{ws}, s, from) == ws : List<&2, String>}

# LAW: a notice of the hub never inherits works: the hub is never an
# agent, so it has no turn to inherit from. The only hub message with
# works is a pause's resume note, and they are explicit
# (pause_queues_one_resume_note_per_agent; architect m_64603).
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

# LAW: an agent never acts as him: its request's change is by the agent
# itself, whatever it writes (his gestures come only from a client's
# socket, In work_act; architect m_64425).
law an_agent_never_acts_as_him:
  for +t: Nat
  for +from: String
  {HC.task_by(HC.TAReq{t}, from) == from : String}

# LAW: only main among the agents changes any work: the right is read
# from the asker, never from a name it carries.
law only_main_changes_any_work:
  for +t: Nat
  for +from: String
  {HC.task_all(HC.TAReq{t}, from) == String.eq(from, HM.MAIN()) : Bool}

# ---- a work's pause (architect m_64602/m_64603) ----

# LAW: a message held by its works' pause is not delivered: it stays
# queued, the ones after it keep their order.
law paused_work_holds_its_deliveries:
  for +m: HS.Msg
  for +rest: List<&2, HS.Msg>
  {HWO.unheld.pick(True{}, m, rest) == rest : List<&2, HS.Msg>}

# LAW: a message with no work (a notice, a word to no task) is never
# held by a work's pause.
law a_message_with_no_work_is_never_held:
  for +xs: List<&2, HS.Work>
  {HWO.all_paused(Nil{}, xs) == False{} : Bool}

# LAW: an agent that ended its turn itself (what the note asks) is never
# interrupted.
law pause_never_stops_an_idle_agent:
  for +ids: List<&2, String>
  for +xs: List<&2, HS.Work>
  for +now: Nat
  {HWO.stops(False{}, ids, xs, now) == False{} : Bool}

# LAW: a turn's open work (no pause time) never lets the stop through:
# every work of the turn must pass the grace (grace.go ands them), and a
# turn that serves no work is never stopped.
law pause_never_stops_an_agent_with_an_open_work:
  for +now: Nat
  {HWO.grace.at(None{}, now) == False{} : Bool}

law pause_never_stops_a_turn_with_no_work:
  for +xs: List<&2, HS.Work>
  for +now: Nat
  {HWO.stops.works(Nil{}, xs, now) == False{} : Bool}

# LAW: a paused work stops a turn only once PAUSE_GRACE passed since its
# pause: the note goes first, the stop after.
law pause_interrupts_only_after_grace:
  for +at: Nat
  for +now: Nat
  {HWO.grace.at(Some{at}, now) == Nat.is_ge(now, Nat.add(at, HWO.PAUSE_GRACE_MS())) : Bool}

# LAW: the resume note carries exactly its work, explicitly; one per
# agent of the work, in the hub's order (work_pause.agents filters
# St.order, which names each agent once).
law pause_queues_one_resume_note_per_agent:
  for +n: String
  for +id: String
  {HC.Snd.works(HC.work_resume_note(n, id)) == Some{[id]} : Maybe<&2, List<&2, String>>}

# LAW: an until act of a work no longer paused does nothing (resumed by
# hand before its end).
law stale_until_never_resumes:
  for +u: Maybe<&2, Nat>
  for +now: Nat
  {HWO.until_due.go(None{}, u, now) == False{} : Bool}

# LAW: an until act of a work paused again with no end does nothing; with
# a new end, it resumes only once that end came.
law until_follows_the_last_pause:
  for +p: Nat
  for +at: Nat
  for +now: Nat
  {HWO.until_due.go(Some{p}, Some{at}, now) == Nat.is_ge(now, at) : Bool}

law pause_without_end_never_resumes:
  for +p: Nat
  for +now: Nat
  {HWO.until_due.go(Some{p}, None{}, now) == False{} : Bool}

# LAW: his move of a card gives it his works and marks it moved: no
# stamp changes a card's works after its open, so his move stays
# (architect m_64425; an artifact's is its own Meta in the shell's store,
# set through fx art_works: artifacts_works.rs, m_64740).
law moved_by_you_sticks:
  for +c: HS.Card
  for +ws: List<&2, String>
  {HM.card_moved([c], HS.Card.id(c), ws) == [HS.Card.with_moved(HS.Card.with_works(c, ws), True{})] : List<&2, HS.Card>}

# LAW: a work closed by done has no pause left: held() needs every work
# of a message paused, so nothing waits on it any more (architect
# m_64704; core.bend pumps what it held in the same step).
law a_closed_work_holds_nothing:
  for +w: HS.Work
  for +by: String
  for +at: Nat
  {HS.Work.paused(HWO.change.op(HS.WDone{}, w, by, at)) == None{} : Maybe<&2, Nat>}

# LAW: the same for a merge: the held messages go, to its target
# (retarget at the delivery: merge_redirects).
law a_merged_work_holds_nothing:
  for +into: String
  for +w: HS.Work
  for +by: String
  for +at: Nat
  {HS.Work.paused(HWO.change.op(HS.WMerge{into}, w, by, at)) == None{} : Maybe<&2, Nat>}

# LAW: his own words are never held by a pause: they reach the agent
# (tasks-ux m_64703: his word beats the pause).
law his_words_are_never_held:
  for +ids: List<&2, String>
  for +xs: List<&2, HS.Work>
  {HWO.held.by(True{}, ids, xs) == False{} : Bool}

# ---- the fold (architect m_64423 Q3) ----

# LAW: a done work folds to its tomb once DONE_KEEP_MS (7 days) passed
# since its done, not before.
law a_done_work_folds_after_seven_days:
  for +f: HS.Fin
  for +now: Nat
  {HWO.fold_due.go(Some{f}, now) == Nat.is_ge(now, Nat.add(HS.Fin.at(f), HWO.DONE_KEEP_MS())) : Bool}

# LAW: an open work never folds.
law an_open_work_never_folds:
  for +now: Nat
  {HWO.fold_due.go(None{}, now) == False{} : Bool}

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

# LAW: a card event touches only its card's works: the rows of the other
# works stay as they are (architect m_64541; cards are frequent).
law a_card_event_touches_only_its_works:
  for +c: HS.Card
  for +cs0: List<&2, HS.Card>
  {HV.card_works(HM.ECardOpened{c}, cs0) == HS.Card.works(c) : List<&2, String>}

# LAW: page_waits is the whole set: the input replaces his page waits,
# never merges into the last ones, so a work missing from it has none
# (architect m_64740: the shell keeps no last-sent copy).
law page_waits_is_the_whole_set:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +ws: List<&2, String>
  for +ns: List<&2, Nat>
  {HX.R.pw(HX.Cx.r(o, HC.page_waits.in(o, True{}, cx, ws, ns))) == HC.page_waits(ws, ns) : List<&2, HX.Kn>}

# LAW: nothing waits on him: every work's count is 0.
law no_page_waits_no_needs:
  for +ns: List<&2, Nat>
  for +id: String
  {HKY.kn_get(HC.page_waits(Nil{}, ns), id) == 0n : Nat}

# LAW: page waits the same on a work leave its row as it is: page_waits
# recomputes only the works whose count changed.
law same_page_waits_leave_its_row:
  for +pw: List<&2, HX.Kn>
  for +id: String
  {HV.pw_moved(pw, pw, id) == False{} : Bool}
