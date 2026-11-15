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
import ../hub/wakes.bend as HWK
import ../hub/view.bend as HV
import ../hub/keyed.bend as HKY
import ../hub/wctx.bend as HWC
import ../hub/text.bend as HT
import ../hub/gen/words.bend as HCW

# LAW: `--task <id>` gives the message its works, whatever the sender's
# turn serves.
law task_flag_replaces_turn_works:
  for +ws: List<&2, String>
  for +s: HS.St
  for +from: String
  {HC.works_of_send(Some{ws}, s, from) == ws : List<&2, String>}

# LAW: a notice of the hub never inherits works: the hub is never an
# agent, so it has no turn to inherit from. The only hub messages with
# works are a pause's resume note and a Restart's note, and their works
# are explicit (pause_queues_one_resume_note_per_agent,
# restart_note_carries_only_its_work; architect m_64603, m_65555).
law notice_carries_no_works:
  for +s: HS.St
  {HC.works_of_send(None{}, s, HM.HUB()) == Nil{} : List<&2, String>}

# LAW: his message to an agent other than main with no explicit tasks
# (outside a Task tab: SendOpts.works empty) carries the work the
# agent's turn serves when it is exactly one, else none (architect
# m_64507; explicit works replace this rule:
# his_task_tab_words_carry_its_task, m_64887). Read from St at the send:
# the Msg records what it got, so a replay never recomputes it from a
# later turn; a wrong attach is his to move (moved_by_you_sticks).
law his_words_follow_a_single_task:
  for +s: HS.St
  for +to: String
  {HC.works_his.single(False{}, s, to) == HC.single(HC.works_of_agent(HM.get_agent(s, to))) : List<&2, String>}

law his_words_pick_the_rule_by_their_recipient:
  for +s: HS.St
  for +to: String
  {HC.works_his(None{}, s, to) == HC.works_his.single(String.eq(to, HM.MAIN()), s, to) : List<&2, String>}

# LAW: his plain words to main carry no works, whatever main's turn
# serves (architect m_65127): main routes, his plain words to it are
# most often a new ask, and main's birth rule decides (its row: the new
# task's agent spawned with --task <new id>, which replaces).
law his_plain_words_to_main_carry_no_works:
  for +s: HS.St
  {HC.works_his(None{}, s, HM.MAIN()) == Nil{} : List<&2, String>}

# LAW: his words sent with explicit tasks (a Task tab's '+ Agent',
# SendOpts.works, read by task_flag) carry exactly them, whatever the
# recipient's turn holds (architect m_64887): the fact is his act.
law his_task_tab_words_carry_its_task:
  for +ws: List<&2, String>
  for +s: HS.St
  for +to: String
  for +text: String
  for +queued: Bool
  for +parts: String
  {HC.Snd.works(HC.Snd.resolved(s, HC.Snd{HM.USER(), to, text, False{}, None{}, True{}, False{}, queued, None{}, parts, None{}, Some{ws}})) == Some{ws} : Maybe<&2, List<&2, String>>}

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

# LAW: a Restart says honestly who made it: main's `sb task restart`
# (by main) never gets his words (architect m_65555; the Log says "main
# restarted it").
law a_restart_by_main_never_says_him:
  for +id: String
  for +name: String
  {HC.work_restart.text(HM.MAIN(), id, name) == HCW.restart_note_main(id, name) : String}

# LAW: main's `sb task move` is journaled by main, never as his: the
# Log says "main moved…" for it (architect m_65565).
law a_move_by_main_says_main:
  for +t: Nat
  for +from: String
  {HC.move_by(HC.TAReq{t}, from) == from : String}

# LAW: only his gesture moves as him: its line carries no `by` (an older
# line reads the same).
law a_move_as_him_is_his_gesture_only:
  for +from: String
  {HC.move_by(HC.TAHim{}, from) == "" : String}

# ---- a Restart (roma idea 3; architect m_65554, m_65555) ----

# LAW: a Restart changes no field of its work: its effects are its
# agents' (its failed ones restored, its idle ones told), never the task's.
law work_restart_changes_no_field:
  for +w: HS.Work
  for +by: String
  for +at: Nat
  {HWO.change.op(HS.WRestart{}, w, by, at) == w : HS.Work}

# LAW: the restart note carries exactly its work, explicitly (never
# inherited from a turn).
law restart_note_carries_only_its_work:
  for +n: String
  for +id: String
  for +text: String
  {HC.Snd.works(HC.work_restart_note(n, id, text)) == Some{[id]} : Maybe<&2, List<&2, String>>}

# LAW: a closed task (done or merged) is never restarted: reopen it first.
law restart_on_a_closed_work_is_refused:
  for +f: HS.Fin
  for +p: Maybe<&2, Nat>
  for +id: String
  {Maybe.is_some(&2, String, HC.task_refused.restart(Some{f}, p, id)) == True{} : Bool}

# LAW: a paused task is never restarted: Resume is the gesture.
law restart_on_a_paused_work_is_refused:
  for +at: Nat
  for +id: String
  {Maybe.is_some(&2, String, HC.task_refused.restart(None{}, Some{at}, id)) == True{} : Bool}

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

# LAW: a reply watch (an `sb ask` past its timeout) reads as waiting on
# that reply, naming who it waits on; any other watch names its agent
# (tasks-qa: the task read "quiet").
law a_reply_watch_names_who_it_waits_on:
  for +to: String
  for +msg: Nat
  {HWS.st.watch.of(HS.Watch.with_spec(HS.Watch.empty(), HWK.reply_spec(to, msg))) == HWS.WSReply{to} : HWS.WSt}

# LAW: main never holds a work out of To review: it routes and never
# says done, so its agents' done is read without it (tasks-qa).
law main_never_holds_a_work_out_of_review:
  for +ns: List<&2, String>
  for +s: HS.St
  {HWS.all_done(HM.MAIN() <> ns, s) == HWS.all_done(ns, s) : Bool}

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

# ---- a blocked report is a card (spec tasks-center row 63; architect
# m_64926/m_64931) ----

# LAW: an agent's blocked report opens a card for him only while that
# agent has none open: a second blocked report, or one after its question
# card, opens nothing more (what waits on him is already a card). Main's
# own `sb notify` about the same blocked is no typed link to this card:
# its role prompt says not to open another (prompts.rs), no law can.
law blocked_report_opens_one_card:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for +text: String
  for +i: Nat
  for +t: List<&2, Nat>
  for e: {HC.card_ids(HS.St.cards(HC.st(o, cx)), agent, Nil{}) == i <> t : List<&2, Nat>}
  {HC.blocked_card(o, cx, agent, text) == cx : HX.Cx<o>}

# LAW: its agent moved on (reports progress or done, says working or
# done): the card its blocked report opened is one the moved-on rule
# closes (report_cards_end); a drop or an archive closes every card of it.
law blocked_card_ends_when_it_moves_on:
  for +c: HS.Card
  for +t: List<&2, HS.Card>
  for +a: String
  for r: {HS.Card.from_report(c) == True{} : Bool}
  for e: {String.eq(HS.Card.agent(c), a) == True{} : Bool}
  {HC.report_card_ids(c <> t, a) == HS.Card.id(c) <> HC.report_card_ids(t, a) : List<&2, Nat>}

# LAW: the fact decides, never the kind: a card of kind "blocked" made any
# other way (`sb notify`, from_report False) is never closed by the
# moved-on rule.
law a_notify_blocked_card_never_ends_on_moved_on:
  for +c: HS.Card
  for +t: List<&2, HS.Card>
  for +a: String
  for r: {HS.Card.from_report(c) == False{} : Bool}
  {HC.report_card_ids(c <> t, a) == HC.report_card_ids(t, a) : List<&2, Nat>}

# LAW: his answer on a blocked report's card goes to its agent
# (answer.blocked: his message to it, the card closed answered), whatever
# its kind reads.
law his_answer_on_a_blocked_card_reaches_its_agent:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  for +id: Nat
  for +text: String
  for +parts: String
  for +via: String
  for r: {HS.Card.from_report(c) == True{} : Bool}
  {HC.answer.card(o, Some{c}, cx, id, text, parts, via) == HC.answer.blocked(o, cx, c, text, parts, via) : HX.Cx<o>}

# LAW: that message is his, to the card's agent, and carries the card's
# works (its task's turn, like any card's answer).
law blocked_answer_carries_its_cards_works:
  for +c: HS.Card
  for +text: String
  for +parts: String
  {HC.answer.blocked.snd(c, text, parts) == HC.Snd{HM.USER(), HS.Card.agent(c), text, False{}, None{}, False{}, False{}, False{}, None{}, parts, None{}, Some{HS.Card.works(c)}} : HC.Snd}

# ---- a task's birth (architect, owed before tasks merges) ----

# LAW: a birth never takes a taken id (architect m_64989): an id a work or a
# tomb holds is refused, the work never emitted. Births are the only step
# that adds to works (a rename keeps the id, a merge or a fold makes a tomb,
# tombs keep their ids), so works and tombs stay unique by construction.
# WO.free's fuel is why the refusal never fires (liveness), not why ids are unique.
law work_birth_never_takes_a_taken_id:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HC.TAsk
  for +by: String
  for +w: HS.Work
  for e: {HWO.taken(HC.st(o, cx), HS.Work.id(w)) == True{} : Bool}
  {HC.req_task.born(o, cx, a, by, w) == HC.task_err(o, a, cx, HC.cat(["task id ", HS.Work.id(w), " is taken: try another name"])) : HX.Cx<o>}

# LAW: the other case: a free id is born (EWorkCreated, then its turn).
law work_birth_of_a_free_id:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HC.TAsk
  for +by: String
  for +w: HS.Work
  for e: {HWO.taken(HC.st(o, cx), HS.Work.id(w)) == False{} : Bool}
  {HC.req_task.born(o, cx, a, by, w) == HC.req_task.born.go(o, cx, a, by, w) : HX.Cx<o>}

# LAW (free's own row, not the uniqueness law): the walk past a taken slug
# keeps the first `slug-n` it finds free.
law work_id_unique_next:
  for +st: HS.St
  for +slug: String
  for +n: Nat
  for +f: Nat
  for +e: {HWO.taken(st, HWO.free.id(slug, n)) == False{} : Bool}
  {HWO.taken(st, HWO.free.go(1n+f, st, slug, n)) == False{} : Bool}

# LAW: an agent's new task (sb task new) takes its turn in the same step:
# its turn serves it from then (a work_turn line, its agent, that id).
law work_birth_takes_turn:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +token: Nat
  for +by: String
  for +id: String
  {HC.req_task.turn(o, HC.TAReq{token}, cx, by, id) == HX.emit(o, cx, HM.EWorkTurn{HWO.wturn(by, [id], True{}, HC.now(o, cx))}, Unit{}, Unit{}, Unit{}) : HX.Cx<o>}

# LAW: his own new task (the Board's, the Task tab's) takes no turn: he has none.
law his_work_birth_takes_no_turn:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +by: String
  for +id: String
  {HC.req_task.turn(o, HC.TAHim{}, cx, by, id) == cx : HX.Cx<o>}

# ---- a joining agent's context (roma idea 5; architect m_65317, m_65509) ----

# LAW: a delivery for a work its agent already serves (the agent is in its
# agents) gives it no context: the context is only for the works it joins.
law work_context_only_for_new_works:
  for +w: HS.Work
  for +agent: String
  for e: {HWO.has(HS.Work.agents(w), agent) == True{} : Bool}
  {HWC.fresh([HS.Work.id(w)], [w], agent) == Nil{} : List<&2, String>}

# LAW: a work its agent does not serve yet is one it joins: its context
# comes with the delivery.
law work_context_for_a_joined_work:
  for +w: HS.Work
  for +agent: String
  for e: {HWO.has(HS.Work.agents(w), agent) == False{} : Bool}
  {HWC.fresh([HS.Work.id(w)], [w], agent) == [HS.Work.id(w)] : List<&2, String>}

# LAW: the context is its blocks clipped at CTX_MAX chars, a function of
# the state and the works alone (never a message's words): what an agent
# reads instead of main's thread, bounded.
law work_context_is_capped:
  for ids: List<&2, String>
  for +st: HS.St
  for +pw: List<&2, HX.Kn>
  {HWC.ctx(ids, st, pw) == HT.clip(HWC.raw(ids, st, pw), HWC.CTX_MAX()) : String}
