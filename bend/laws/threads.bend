# bend/laws/threads.bend: stopped turns, a task's thread, message parts, receipts, queued take-back, MCP failures, ended turns, live answers.
# Open claims, each one closed by bend/proofs/threads.bend (moved from
# LAWS.bend). The gate checks every bend/proofs/*.bend: a law here
# with no proof there fails it.
import Base
import ../core/api.bend as Api
import ../core/anth-stream.bend as As
import ../runtime/provider-pure.bend as Pv
import ../runtime/mcp-pure.bend as Mc
import ../runtime/live-pure.bend as Lp
import ../vendor/http/encoding/utf8.bend as U8
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/text.bend as HT
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/picks.bend as HPK
import ../hub/answers.bend as HA
import ../hub/keyed.bend as HKL
import ../hub/gen/feed.bend as HFD
import ../hub/said.bend as HSD

# ---- a stopped turn is a typed fact (desktop reset, architect m_9650 D,
# m_9693): sb-core, where the interrupt is decided, writes one 'stopped'
# line in that agent's feed in the same step as fx interrupt ----

# LAW: the window's or the TUI's interrupt of a running turn emits fx
# interrupt and then exactly one 'stopped' line for that agent.
law interrupt_says_stopped_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for e: {HC.is_busy(HX.Rt.run(HC.rt(o, cx, n))) == True{} : Bool}
  {HC.interrupt.go(o, Some{n}, cx) == HC.feed(o, HC.interrupt.fx(o, cx, n), n, HFD.stopped("stopped")) : HX.Cx<o>}

# LAW: with no turn running, the interrupt is only a notice: no fx
# interrupt, no 'stopped' line.
law interrupt_idle_says_nothing:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for e: {HC.is_busy(HX.Rt.run(HC.rt(o, cx, n))) == False{} : Bool}
  {HC.interrupt.go(o, Some{n}, cx) == HC.notice(o, cx, "no turn in progress to interrupt") : HX.Cx<o>}

# LAW: an agent's `sb interrupt` of a running turn says it the same way
# (one 'stopped' line after fx interrupt), then answers ok.
law req_interrupt_says_stopped_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +token: Nat
  for +agent: String
  for +n: String
  for e: {HC.is_busy(HX.Rt.run(HC.rt(o, cx, n))) == True{} : Bool}
  {HC.req_interrupt(o, Some{n}, cx, token, agent) == HC.reply(o, HC.feed(o, HC.interrupt.fx(o, cx, n), n, HFD.stopped("stopped")), token, HC.ok_body(Nil{})) : HX.Cx<o>}

# ---- what a task sent is a fact of its own thread (desktop bar A.3 and
# S16, architect m_10203): sb-core writes one 'sent' line in the sender's
# own feed for a task's message to another agent; main's feed keeps its
# 'msg' lines, a message to the user stays 'msg-you'. A card main opens
# for a task's message shows in that task's feed too. Views only ----

# LAW: a task's message to another agent writes exactly one 'sent' record
# in the task's feed: to, the message id, whether it waits for a reply,
# text; then its parts when it has some (send-parts, law
# agent_no_parts_no_sent_line: none, nothing more).
law task_send_says_sent_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +m: HS.Msg
  for e: {HC.is_task_send(o, cx, HS.Msg.from(m), HS.Msg.to(m)) == True{} : Bool}
  {HC.feed_sender(o, cx, m) == HC.sent_parts(o, HC.feed(o, cx, HS.Msg.from(m), HFD.sent(HS.Msg.to(m), HS.Msg.id(m), HS.Msg.expect(m), HS.Msg.text(m))), HS.Msg.from(m), m) : HX.Cx<o>}

# LAW: main's messages, the user's, a message to the user, and a message
# from a name that isn't an agent of this hub write no 'sent' line.
law other_send_says_no_sent:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +m: HS.Msg
  for e: {HC.is_task_send(o, cx, HS.Msg.from(m), HS.Msg.to(m)) == False{} : Bool}
  {HC.feed_sender(o, cx, m) == cx : HX.Cx<o>}

# LAW: a sender this hub has no agent of ('@bise', a cross-hub
# delegation, or any unknown name) is never a task's send.
law unknown_sender_is_no_task_send:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +from: String
  for +to: String
  for e: {HC.has_agent(o, cx, from) == False{} : Bool}
  {HC.is_task_send(o, cx, from, to) == False{} : Bool}

# LAW: a card opened for a task's message also writes its 'card' line in
# that task's feed (main's own cards stay in main's feed only).
law task_card_shows_in_its_feed:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +id: Nat
  for +agent: String
  for +text: String
  for e: {HC.is_task_of(o, cx, agent) == True{} : Bool}
  {HC.card_to_asker(o, cx, id, agent, text) == HC.feed(o, cx, agent, HFD.card(id, "question", agent, text)) : HX.Cx<o>}

# LAW: a card whose agent isn't an agent of this hub (or is main) writes
# no extra line.
law other_card_no_extra_line:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +id: Nat
  for +agent: String
  for +text: String
  for e: {HC.is_task_of(o, cx, agent) == False{} : Bool}
  {HC.card_to_asker(o, cx, id, agent, text) == cx : HX.Cx<o>}

# LAW: an agent this hub doesn't have is never a task of it.
law unknown_agent_is_no_task:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for e: {HC.has_agent(o, cx, agent) == False{} : Bool}
  {HC.is_task_of(o, cx, agent) == False{} : Bool}

# LAW: the user answering one of main's own cards does three things, once
# each and in this order: main's note (it still wakes main; its fixed head
# is what the clients match to drop it), the same route line an agent's
# card writes ('you → @main (answer to card #N) : <his answer, whole>',
# the "you answered" row, with his pick, no message: none was sent), and
# the card closed as answered.
law main_card_answer_routes_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  for +text: String
  for +parts: String
  for +via: String
  {HC.answer.q.main(o, cx, c, text, parts, via)
    == HC.close_card_ans(o,
      HA.due_add(o, HC.answer_line(o,
        HC.notify_main(o, cx, HC.cat([HC.MAIN_ANSWER_HEAD(), HC.ns(HS.Card.id(c)), " (\"", HT.one_line(HS.Card.text(c)), "\"): \"", HT.one_line(text), "\""])),
        HM.MAIN(), HM.MAIN(), c, text, parts, None{}), HS.Card.id(c)),
      HS.Card.id(c), "answered", HA.his_ans(c, text, via)) : HX.Cx<o>}

# LAW: his message's feed line is followed, in the same step, by its id
# ('sb you-id : m_N'): the clients key his entry by it (nothing else may
# fall between them); the REPL and the model get the same delivery. A
# message with no parts writes exactly these two lines (an older client's,
# an agent-made one: the feed is what it always was).
law you_line_carries_its_id:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +name: String
  for +m: HS.Msg
  for e: {String.is_empty(HS.Msg.parts(m)) == True{} : Bool}
  {HC.deliver.line(o, False{}, True{}, cx, name, m)
    == HC.feed(o, HC.feed(o, cx, name, HFD.you(HS.Msg.text(m), HS.Msg.via(m))), name, HFD.you_id(HS.Msg.id(m))) : HX.Cx<o>}

# ---- his message's parts (msg-parts) ----
# The clients send his message as typed parts (bise_proto::parts: his
# words, quotes, pastes, images, files) beside the text the model gets.
# sb-core keeps them on the message, opaque (Msg.parts, "" none), and
# writes them at the delivery, so the clients draw his message from them.

# LAW: a message with parts writes them on one 'you-parts' line right
# after its you-id (the clients put them on that entry); the you line
# and its id come first, as always.
law you_parts_follow_the_id:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +name: String
  for +m: HS.Msg
  for e: {String.is_empty(HS.Msg.parts(m)) == False{} : Bool}
  {HC.deliver.line(o, False{}, True{}, cx, name, m)
    == HC.feed(o, HC.feed(o, HC.feed(o, cx, name, HFD.you(HS.Msg.text(m), HS.Msg.via(m))), name, HFD.you_id(HS.Msg.id(m))), name, HFD.you_parts(HC.parts_json(HS.Msg.parts(m)))) : HX.Cx<o>}

# LAW: parts never change what he said: the same message with or without
# parts writes the same lines (his text, its id), the parts only after
# them. The model reads the text; the parts are for the clients.
law parts_never_change_text:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +name: String
  for +m: HS.Msg
  for +p: String
  {HC.deliver.line(o, False{}, True{}, cx, name, HS.Msg.with_parts(m, p))
    == HC.you_parts(o, HC.deliver.line(o, False{}, True{}, cx, name, HS.Msg.with_parts(m, "")), name, p) : HX.Cx<o>}

# LAW: a line with no parts is no line: a route line (an answer to a card,
# a message to another agent) with none is the route line alone.
law no_parts_no_line:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +who: String
  for +card: Maybe<&2, Nat>
  for +said: String
  {HC.route_line(o, cx, who, card, said, "") == HC.feed(o, cx, HM.MAIN(), HFD.route(who, card, said, None{}, None{}, None{})) : HX.Cx<o>}

# LAW: a route line with parts is followed by one 'route-parts' line.
law route_parts_follow_the_route:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +who: String
  for +card: Maybe<&2, Nat>
  for +said: String
  for +parts: String
  for e: {String.is_empty(parts) == False{} : Bool}
  {HC.route_line(o, cx, who, card, said, parts) == HC.feed(o, HC.feed(o, cx, HM.MAIN(), HFD.route(who, card, said, None{}, None{}, None{})), HM.MAIN(), HFD.route_parts(HC.parts_json(parts))) : HX.Cx<o>}

# ---- inbox picks (architect m_22243): his answer's route line ----

# LAW: an option picked by its number gets that option's words, the
# number with them.
law an_option_number_gets_its_words:
  {HPK.option_pick("which one?\n1. at every step\n2. only at checkout", " 2 ") == Some{HPK.Pick{2n, "only at checkout"}} : Maybe<&2, HPK.Pick>}

# LAW: words get no pick (his words are the row's words).
law words_get_no_pick:
  {HPK.option_pick("which one?\n1. at every step\n2. only at checkout", "only at checkout") == None{} : Maybe<&2, HPK.Pick>}

# LAW: a number that isn't one of the card's options gets no pick.
law a_number_off_the_list_gets_no_pick:
  {HPK.option_pick("which one?\n1. at every step\n2. only at checkout", "3") == None{} : Maybe<&2, HPK.Pick>}

# LAW: a pick's number and its words are both on the line or both absent.
law pick_both_or_neither:
  for p: Maybe<&2, HPK.Pick>
  {Maybe.is_some(&2, Nat, HPK.pick_n(p)) == Maybe.is_some(&2, String, HPK.pick_words(p)) : Bool}

# LAW: his answer's route line says what he said, unchanged (the pick
# rides beside it; said == render(parts) still holds).
law the_answer_line_keeps_said:
  for +who: String
  for +c: HS.Card
  for +said: String
  for +msg: Maybe<&2, Nat>
  {HC.answer_rec(HPK.ans(HS.Card.id(c), HS.Card.text(c), who, said), msg) == HFD.route(who, Some{HS.Card.id(c)}, said, msg, HPK.pick_n(HPK.option_pick(HS.Card.text(c), said)), HPK.pick_words(HPK.option_pick(HS.Card.text(c), said))) : J.Json}

# LAW: the step's answer (inbox picks (3), link.rs AStep ans_*): the card,
# who asked, what he said and the question are all there or all absent...
law answer_who_with_its_card:
  for m: Maybe<&2, HPK.Answer>
  {Maybe.is_some(&2, Nat, HPK.card_of(m)) == Maybe.is_some(&2, String, HPK.who_of(m)) : Bool}

law answer_said_with_its_card:
  for m: Maybe<&2, HPK.Answer>
  {Maybe.is_some(&2, Nat, HPK.card_of(m)) == Maybe.is_some(&2, String, HPK.said_of(m)) : Bool}

law answer_ask_with_its_card:
  for m: Maybe<&2, HPK.Answer>
  {Maybe.is_some(&2, Nat, HPK.card_of(m)) == Maybe.is_some(&2, String, HPK.ask_of(m)) : Bool}

# LAW: ...a pick only when they are there...
law answer_pick_needs_the_card:
  for m: Maybe<&2, HPK.Answer>
  {Bool.and(Maybe.is_some(&2, String, HPK.pick_of_ans(m)), Bool.not(Maybe.is_some(&2, Nat, HPK.card_of(m)))) == False{} : Bool}

# LAW: ...and the step's pick is the route line's pick: both come from the
# one answer that answer_line notes (X.answered) and writes (answer_rec)
law the_steps_pick_is_the_route_lines:
  for +c: HS.Card
  for +who: String
  for +said: String
  {HPK.pick_of_ans(Some{HPK.ans(HS.Card.id(c), HS.Card.text(c), who, said)}) == HPK.pick_words(HPK.option_pick(HS.Card.text(c), said)) : Maybe<&2, String>}

# LAW: the answer noted in a step is the one the step's answer reads
law the_noted_answer_is_read_back:
  for -o: HS.St
  for cx: HX.Cx<o>
  for +a: HPK.Answer
  {HX.R.answered(HX.Cx.r(o, HX.answered(o, cx, a))) == Some{a} : Maybe<&2, HPK.Answer>}

# LAW: an agent's card: his answer's route line goes in the asker's feed
# too (inbox picks (2); its fold makes the asker's msg-in that row).
law an_agents_answer_is_in_the_askers_feed:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +asker: String
  for +c: HS.Card
  for +said: String
  for +parts: String
  for +msg: Maybe<&2, Nat>
  for e: {String.eq(asker, HM.MAIN()) == False{} : Bool}
  {HC.asker_line(o, cx, asker, c, said, parts, msg) == HC.answer_line(o, cx, asker, asker, c, said, parts, msg) : HX.Cx<o>}

# LAW: main's own card: the one line in main's feed, no second.
law mains_own_answer_is_one_line:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  for +said: String
  for +parts: String
  for +msg: Maybe<&2, Nat>
  {HC.asker_line(o, cx, HM.MAIN(), c, said, parts, msg) == cx : HX.Cx<o>}

# LAW: his answer to a question card reaches the asker with his parts on
# the message (the asker's thread draws it like any message of his).
law answer_keeps_its_parts:
  for +to: String
  for +text: String
  for +r: Maybe<&2, Nat>
  for +parts: String
  {HC.Snd.parts(HC.answer.snd(to, text, r, parts)) == parts : String}

# LAW: a steered receipt names the steer-mode deliveries since the last
# one, on one feed line at this point of the turn, and marks them seen.
law steered_receipt_names_the_new_ones:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for +h: Nat
  for +t: List<&2, Nat>
  for +total: Nat
  {HC.steer_receipt.go(o, h <> t, cx, agent, total)
    == HC.feed(o, HC.set_r(o, cx, HX.R.with_steered(HC.rr(o, cx), HC.steer_mark(HX.R.steered(HC.rr(o, cx)), agent, total))),
      agent, HFD.steered(h <> t)) : HX.Cx<o>}

# LAW: a receipt once every steered id was seen (or with none) marks
# nothing and writes nothing (a second receipt with no new steer).
law steered_receipt_after_all_seen_is_nothing:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for +ids: List<&2, Nat>
  for +total: Nat
  {HC.steer_receipt.go(o, List.drop(&2, Nat, ids, List.length(&2, Nat, ids)), cx, agent, total) == cx : HX.Cx<o>}

# LAW: a new steer keeps the cursor: the ids already reported stay
# reported, the new ones come after them.
law steer_add_keeps_seen:
  for +xs: List<&2, HX.Steer>
  for +n: String
  for +ids: List<&2, Nat>
  {HKL.steer_seen(HKL.steer_add(xs, n, ids), n) == HKL.steer_seen(xs, n) : Nat}

# ---- the received receipt (ISteerRx, the clients' ✓) ----
# A second cursor, rx_seen, apart from steered's `seen`: the runtime
# receives a steer before it reads it, but no order between the two
# cursors is required (rx_seen <= seen is NOT a law: an older runtime may
# send ISteered alone, and each receipt only names what it did not name).

# LAW: a received receipt names the steer-mode deliveries since the last
# one ('sb steer-rx : m_3 m_4') and moves rx_seen only (seen kept).
law steer_rx_receipt_names_the_new_ones:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for +h: Nat
  for +t: List<&2, Nat>
  for +total: Nat
  {HC.steer_rx_receipt.go(o, h <> t, cx, agent, total)
    == HC.feed(o, HC.set_r(o, cx, HX.R.with_steered(HC.rr(o, cx), HC.steer_rx_mark(HX.R.steered(HC.rr(o, cx)), agent, total))),
      agent, HFD.steer_rx(h <> t)) : HX.Cx<o>}

# LAW: a received receipt with nothing new marks nothing and writes
# nothing.
law steer_rx_receipt_after_all_seen_is_nothing:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for +ids: List<&2, Nat>
  for +total: Nat
  {HC.steer_rx_receipt.go(o, List.drop(&2, Nat, ids, List.length(&2, Nat, ids)), cx, agent, total) == cx : HX.Cx<o>}

# LAW: a new steer keeps the received cursor too (steer_rename keeps it by
# construction, steer_del drops it with the agent's entry).
law steer_add_keeps_rx_seen:
  for +xs: List<&2, HX.Steer>
  for +n: String
  for +ids: List<&2, Nat>
  {HKL.steer_rx_seen(HKL.steer_add(xs, n, ids), n) == HKL.steer_rx_seen(xs, n) : Nat}

# ---- his queued message taken back (HubCmd queued_take, issue 16) ----
# No new state: a taken-back message is Rejected{"taken_back"}, the state
# reject_ids already writes, so an older sb-core replays it as rejected and
# never delivers it (no new journal kind, rollback-safe).

# LAW: a queued message of his is taken back: one EMsgState, Rejected.
law unqueue_takes_back_a_queued_one:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +q: String
  for +id: Nat
  for +to: String
  {HC.unqueue.take(o, HS.Queued{q}, cx, id, to) == HC.emit(o, cx, HM.EMsgState{id, HS.Rejected{"taken_back"}}, Unit{}, Unit{}, Unit{}) : HX.Cx<o>}

# LAW: a message already delivered is left as it is (a notice, the state
# unchanged); so are one already rejected, one cancelled, an unknown id, and
# a message that isn't his.
law unqueue_leaves_a_delivered_one:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +id: Nat
  for +to: String
  {HX.d(o, HC.unqueue.take(o, HS.Delivered{}, cx, id, to)) == HX.d(o, cx) : HS.St}

law unqueue_leaves_a_rejected_one:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +r: String
  for +id: Nat
  for +to: String
  {HX.d(o, HC.unqueue.take(o, HS.Rejected{r}, cx, id, to)) == HX.d(o, cx) : HS.St}

law unqueue_leaves_an_unknown_id:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +id: Nat
  {HX.d(o, HC.unqueue.go(o, None{}, cx, id)) == HX.d(o, cx) : HS.St}

law unqueue_never_takes_anothers:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +r: HS.MRec
  for +id: Nat
  for e: {String.eq(HS.Msg.from(HS.MRec.msg(r)), HM.USER()) == False{} : Bool}
  {HX.d(o, HC.unqueue.go(o, Some{r}, cx, id)) == HX.d(o, cx) : HS.St}

# LAW: the queue after a take-back is the queue before minus that message:
# its own record becomes Rejected{"taken_back"} (no longer queued, so the
# pump never delivers it: delivered_once), and every other record is the
# same. Read on the records (the taking back is map_msg with OSt).
law unqueue_marks_its_own_record:
  for +xs: List<&2, HS.MRec>
  for +id: Nat
  for +x: HS.MSt
  {HM.state_of.go(HM.rec_of(HM.map_msg(xs, id, HM.OSt{x}), id)) == HC.st_after(HM.rec_of(xs, id), x) : Maybe<&2, HS.MSt>}

law unqueue_keeps_every_other_record:
  for +xs: List<&2, HS.MRec>
  for +id: Nat
  for +x: HS.MSt
  for +y: Nat
  for e: {Nat.is_eq(id, y) == False{} : Bool}
  {HM.rec_of(HM.map_msg(xs, id, HM.OSt{x}), y) == HM.rec_of(xs, y) : Maybe<&2, HS.MRec>}

# LAW (the race, unqueue vs the turn-end pump, both orders): sb-core steps
# one input at a time. Taken back first: the record is Rejected
# (unqueue_takes_back_a_queued_one, unqueue_marks_its_own_record), not
# queued, so the pump never delivers it (queued_in, delivered_once).
# Delivered first: the take-back leaves it delivered
# (unqueue_leaves_a_delivered_one). So exactly one of delivered or
# taken_back.

# ---- his queued message said now (HubCmd queued_steer, architect m_25139) ----
# No new state and no new journal kind: a steered queued message is the
# pump's own delivery of that one message (deliver_each: mark_delivered,
# its feed line with its id and parts), so it keeps its id, is delivered
# once (delivered_once: only while queued) and is never both queued and
# steered (Delivered is not Queued, queued_in leaves it).

# LAW: a queued message of his to a busy agent goes into its running turn:
# the pump's busy delivery of that message alone (steered, its id kept).
law steer_queued_steers_into_the_turn:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +m: HS.Msg
  for +name: String
  for +im: Bool
  {HC.steer_queued.run(o, HS.RBusy{}, cx, m, name, im) == HC.pump.batch(o, [m], cx, name, True{}, im) : HX.Cx<o>}

# LAW: to an idle agent, it is a turn of its own: the pump's idle delivery.
law steer_queued_starts_a_turn_when_idle:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +m: HS.Msg
  for +name: String
  for +im: Bool
  {HC.steer_queued.run(o, HS.RIdle{}, cx, m, name, im) == HC.pump.batch(o, [m], cx, name, False{}, im) : HX.Cx<o>}

# LAW: an agent that can't take it (down, starting) leaves it queued: a
# notice, the state unchanged; so does an agent that is gone.
law steer_queued_waits_for_a_down_agent:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +m: HS.Msg
  for +name: String
  for +im: Bool
  {HX.d(o, HC.steer_queued.run(o, HS.RDown{}, cx, m, name, im)) == HX.d(o, cx) : HS.St}

law steer_queued_waits_for_a_starting_agent:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +m: HS.Msg
  for +name: String
  for +im: Bool
  {HX.d(o, HC.steer_queued.run(o, HS.RStarting{}, cx, m, name, im)) == HX.d(o, cx) : HS.St}

law steer_queued_leaves_it_for_a_gone_agent:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +m: HS.Msg
  {HX.d(o, HC.steer_queued.agent(o, None{}, cx, m)) == HX.d(o, cx) : HS.St}

# LAW: a message no longer queued (delivered, taken back, rejected,
# cancelled) is left as it is, as are an unknown id and a message that
# isn't his.
law steer_queued_leaves_a_delivered_one:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +m: HS.Msg
  {HX.d(o, HC.steer_queued.take(o, HS.Delivered{}, cx, m)) == HX.d(o, cx) : HS.St}

law steer_queued_leaves_a_rejected_one:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +m: HS.Msg
  for +r: String
  {HX.d(o, HC.steer_queued.take(o, HS.Rejected{r}, cx, m)) == HX.d(o, cx) : HS.St}

law steer_queued_leaves_an_unknown_id:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +id: Nat
  {HX.d(o, HC.steer_queued.go(o, None{}, cx, id)) == HX.d(o, cx) : HS.St}

law steer_queued_never_steers_anothers:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +r: HS.MRec
  for +id: Nat
  for e: {String.eq(HS.Msg.from(HS.MRec.msg(r)), HM.USER()) == False{} : Bool}
  {HX.d(o, HC.steer_queued.go(o, Some{r}, cx, id)) == HX.d(o, cx) : HS.St}

# LAW (the races, sb-core steps one input at a time): steered first, the
# record is Delivered, so the turn's end never says it again
# (delivered_once) and a take-back after it is a notice
# (unqueue_leaves_a_delivered_one). Taken back first, it is Rejected and a
# steer after it is a notice (steer_queued_leaves_a_rejected_one). The
# turn's end first, it is Delivered and a steer after it is a notice
# (steer_queued_leaves_a_delivered_one). So exactly one outcome each time.

# ---- his send answered with its message's id (A1, architect m_21450) ----
# sb-core makes his message (m_N) when his input arrives, queued too, and
# answers its id with the step (AStep.msg = HSD.said_id(HSD.said(journal))):
# the window's turn/send result carries it, and the window matches his
# pending row to the 'you' entry whose you-id line carries the same id.

# m is a message of his that an event of the journal names (HSD.his_msg:
# its EMsgSent, sent by the user)
def HisIn(evs: List<&2, HM.Ev>, +m: HS.Msg) -> Data:
  match evs:
    case Nil{}:
      Empty
    case Con{+e, +t}:
      Either<&2, &2, {HSD.his_msg(e) == Some{m} : Maybe<&2, HS.Msg>}, HisIn(t, m)>

def SaidIn(evs: List<&2, HM.Ev>, s: Maybe<&2, HS.Msg>) -> Data:
  match s:
    case None{}:
      Unit
    case Some{+m}:
      HisIn(evs, m)

# LAW: the message a step answers (HSD.said) is one of his that the step's
# journal sent: never another agent's, never one the step did not make.
law said_is_his:
  for +evs: List<&2, HM.Ev>
  SaidIn(evs, HSD.said(evs))

# LAW: his delivered message's you-id line carries that message's own id
# (HS.Msg.id), the id HSD.said_id answered for it: the answered id is the
# id the you-id line carries later.
law said_you_id:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +name: String
  for +m: HS.Msg
  {HC.deliver.line(o, False{}, True{}, cx, name, m)
    == HC.you_parts(o, HC.feed(o, HC.feed(o, cx, name, HFD.you(HS.Msg.text(m), HS.Msg.via(m))), name, HFD.you_id(HS.Msg.id(m))), name, HS.Msg.parts(m)) : HX.Cx<o>}

law said_id_is_the_msg_id:
  for +m: HS.Msg
  {HSD.said_id(Some{m}) == Some{HS.Msg.id(m)} : Maybe<&2, Nat>}

# ---- MCP calls that fail (runtime/mcp-pure.bend) ----

# LAW: a call is tried again (fresh initialize) only when it never
# reached the tool and a new try may work: initialize got no answer or a
# 5xx, the call could not connect, or the server lost the session. Once
# tools/call may have run (no answer read, a timeout, a 5xx, an error it
# answered) it is never sent again; an auth refusal is not retried.
law mcp_retry_only_unsent:
  {Bool.and(Mc.retry(Mc.SInit{}, Mc.WNet{Pv.NConnect{61, "refused"}}),
    Bool.and(Mc.retry(Mc.SInit{}, Mc.WStatus{503, ""}),
    Bool.and(Bool.not(Mc.retry(Mc.SInit{}, Mc.WStatus{401, ""})),
    Bool.and(Mc.retry(Mc.SCall{}, Mc.WStatus{404, "unknown session"}),
    Bool.and(Mc.retry(Mc.SCall{}, Mc.WNet{Pv.NConnect{61, "refused"}}),
    Bool.and(Bool.not(Mc.retry(Mc.SCall{}, Mc.WNet{Pv.NRead{54, "reset"}})),
    Bool.and(Bool.not(Mc.retry(Mc.SCall{}, Mc.WNet{Pv.NTimeout{}})),
    Bool.and(Bool.not(Mc.retry(Mc.SCall{}, Mc.WStatus{502, ""})),
      Bool.not(Mc.retry(Mc.SCall{}, Mc.WRpc{"-32602", "no such tool"})))))))))) == True{} : Bool}

# LAW: the model reads the kind of failure, the server, the phase, the
# cause, what bise tried, and whether calling again is safe - never just
# "initialize failed".
law mcp_fail_text_says_why_and_safety:
  {Bool.and(String.eq(Mc.fail_text("web_search.web_search", "api.mistral.ai",
      Mc.SInit{}, Mc.WNet{Pv.NDns{"the system resolver did not answer (temporary failure in name resolution)", "your network or VPN"}}, 2n),
      "mcp call failed (network): web_search.web_search (api.mistral.ai), initialize: cannot resolve api.mistral.ai: the system resolver did not answer (temporary failure in name resolution) — check your network or VPN. bise reconnected and retried 2 times. the call never reached the server: safe to call again."),
    String.eq(Mc.fail_text("x.y", "h", Mc.SCall{}, Mc.WStatus{502, "{\"message\":\"bad gateway\"}"}, 0n),
      "mcp call failed (server error): x.y (h), tools/call: h answered HTTP 502: bad gateway. not retried: the call may have run on the server, check before calling it again (safe if the tool only reads).")) == True{} : Bool}

# ---- an agent's ended turns, counted where its run changes (issue 22,
# architect m_16521): the agents row carries working and turns from one
# source, sb-core's runtime state, so no row has one half without the
# other. set_rt is the one writer of a runtime state ----

# LAW: a run that leaves RBusy (to idle, down or starting) counts one
# more ended turn.
law hub_turns_counts_every_busy_exit:
  for now: HS.Run
  for +n: Nat
  for e: {HC.is_busy(now) == False{} : Bool}
  {HC.turns_after(HS.RBusy{}, now, n) == 1n+n : Nat}

# LAW: any other run change (busy to busy, or from a run that was not
# busy) leaves the count as it was.
law hub_turns_never_moves_otherwise:
  for was: HS.Run
  for now: HS.Run
  for +n: Nat
  for e: {Bool.and(HC.is_busy(was), Bool.not(HC.is_busy(now))) == False{} : Bool}
  {HC.turns_after(was, now, n) == n : Nat}

# LAW: set_rt, every runtime change, stores the agent's ended turns as
# its runtime before them moved by turns_after (whatever count the new
# state carried): a step that ends a turn and starts the next one (the
# idle's pump) goes through it twice, Busy -> Idle (+1), Idle -> Busy.
law hub_set_rt_counts_turns:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +x: HX.Rt
  {HX.Rt.turns(HC.rt(o, HC.set_rt(o, cx, x), HX.Rt.name(x)))
    == HC.turns_after(HX.Rt.run(HC.rt(o, cx, HX.Rt.name(x))), HX.Rt.run(x), HX.Rt.turns(HC.rt(o, cx, HX.Rt.name(x)))) : Nat}

# ---- the answer as it streams in (runtime/live-pure.bend) ----

# LAW: pieces cut anywhere (an event split across pieces, the next
# event's start inside a piece) give the same partial as one piece: the
# text since the last one, at offset 0 of its block.
law live_pieces_cut_anywhere_same_line:
  {Con{Lp.emit_line(Lp.take(Lp.feeds(Con{"event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}
event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}
", Nil{}}, Lp.lv0(True{}, "17.0", Api.Anthropic{})), 0n)),
    Con{Lp.emit_line(Lp.take(Lp.feeds(Con{"event: content_block_delta
dat", Con{"a: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}
event", Con{": content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}
", Nil{}}}}, Lp.lv0(True{}, "17.0", Api.Anthropic{})), 0n)), Nil{}}}
    == Con{"partial #17.0 +0 : Hello", Con{"partial #17.0 +0 : Hello", Nil{}}} : List<&2, String>}

# LAW: a call that is not the agent's answer (compaction, one-shot)
# streams nothing: no text waits.
law live_off_streams_nothing:
  {Lp.has(Lp.feeds(Con{"event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}
event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}
", Nil{}}, Lp.lv0(False{}, "17.0", Api.Anthropic{}))) == False{} : Bool}

# LAW: thinking never streams, only the answer's text.
law live_thinking_never_streams:
  {Lp.emit_line(Lp.take(Lp.feeds(Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hmm\"}}
", Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}
", Nil{}}}, Lp.lv0(True{}, "17.0", Api.Anthropic{})), 0n))
    == "partial #17.0 +0 : ok" : String}

# LAW: the offset counts what the block already sent (escaped); a new
# block starts again at 0 (the hub replaces the text).
law live_offset_per_block:
  {Con{Lp.emit_line(Lp.take(Lp.feed(Lp.emit_lv(Lp.take(Lp.feeds(Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ab\"}}
", Nil{}}, Lp.lv0(True{}, "17.0", Api.Anthropic{})), 0n)), "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"c\"}}
"), 300n)),
    Con{Lp.emit_line(Lp.take(Lp.feed(Lp.emit_lv(Lp.take(Lp.feeds(Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ab\"}}
", Nil{}}, Lp.lv0(True{}, "17.0", Api.Anthropic{})), 0n)), "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"c\"}}
"), 300n)), Nil{}}}
    == Con{"partial #17.0 +2 : c", Con{"partial #17.0 +0 : c", Nil{}}} : List<&2, String>}

# LAW: at most one partial per every_ms(): due only with text waiting
# and the last one 400 ms old (designer m_17504).
law live_throttle:
  {Con{Lp.due(Lp.feeds(Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"a\"}}
", Nil{}}, Lp.lv0(True{}, "17.0", Api.Anthropic{})), 399n),
    Con{Lp.due(Lp.feeds(Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"a\"}}
", Nil{}}, Lp.lv0(True{}, "17.0", Api.Anthropic{})), 400n),
    Con{Lp.due(Lp.lv0(True{}, "17.0", Api.Anthropic{}), 900n), Nil{}}}}
    == Con{False{}, Con{True{}, Con{False{}, Nil{}}}} : List<&2, Bool>}

# the Messages API's text delta, up to its text's opening quote
def live_pre() -> String:
  "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\""

# raw UTF-8 in the literal: an é (2 bytes) and an emoji (4 bytes)
def live_l1() -> String:
  String.concat([live_pre(), U8.utf8.encode("café 😀"), "\"}}
"])

# JSON escapes: a newline, quotes, \u00e9, a surrogate pair
def live_l2() -> String:
  String.concat([live_pre(), "a\\nb \\\"q\\\" \\u00e9 \\ud83d\\ude00\"}}
"])

def live_agrees(ls: List<&2, String>) -> List<&2, Bool>:
  match ls:
    case Nil{}:
      Nil{}
    case Con{+l, rest}:
      Con{Lp.agree(Api.Anthropic{}, l), live_agrees(rest)}

def live_fires(ls: List<&2, String>) -> List<&2, Bool>:
  match ls:
    case Nil{}:
      Nil{}
    case Con{+l, rest}:
      Con{Lp.fires(Api.Anthropic{}, l), live_fires(rest)}

# LAW: the fast path (a text delta read without J.parse) reads what
# J.parse reads, on every line it takes, and takes only the exact shape:
# a plain delta, escapes and a surrogate pair, raw UTF-8, index 12, an
# empty delta, trailing blanks; never a leading zero, a space, an extra
# field, a bad escape, junk after the object, a thinking delta, an
# unterminated literal or a lone surrogate (those go to J.parse).
law live_fast_reads_as_slow:
  {Con{live_agrees(
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}",
      Con{String.trim(live_l2()), Con{String.trim(live_l1()),
      Con{"data: {\"type\":\"content_block_delta\",\"index\":12,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}} \r",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":01,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\": 0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\",\"y\":1}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"\\q\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}x",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"x\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"\\ud83d\"}}",
      Nil{}}}}}}}}}}}}}}}),
    Con{live_fires(
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}",
      Con{String.trim(live_l2()), Con{String.trim(live_l1()),
      Con{"data: {\"type\":\"content_block_delta\",\"index\":12,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}} \r",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":01,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\": 0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\",\"y\":1}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"\\q\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}x",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"x\"}}",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x",
      Con{"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"\\ud83d\"}}",
      Nil{}}}}}}}}}}}}}}}), Nil{}}}
    == Con{[True{}, True{}, True{}, True{}, True{}, True{}, True{}, True{},
        True{}, True{}, True{}, True{}, True{}, True{}],
      Con{[True{}, True{}, True{}, True{}, True{}, True{}, False{}, False{},
        False{}, False{}, False{}, False{}, False{}, False{}], Nil{}}}
      : List<&2, List<&2, Bool>>}

# LAW: streaming on agrees with streaming off. The same stream, its
# pieces cut inside an emoji's bytes, a take after each group (a flush
# with several parts waiting), gives partials whose text, joined, is the
# final reply's text byte for byte (As.reply, the stream decoded whole);
# fed as one piece it gives that text in one partial.
law live_streamed_text_is_the_reply:
  {Con{As.reply(U8.utf8.decode(String.concat([
      "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}
", live_l1(), live_l2(), live_pre(), "\"}}
", live_pre(), "!\"}}
", "data: {\"type\":\"content_block_stop\",\"index\":0}
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}
"]))),
    Con{String.join(Lp.runs([["event: content_block_delta
", String.take(live_l1(), 91n)],
      [String.drop(live_l1(), 91n), "event: content_block_delta
", live_l2()],
      [String.concat([live_pre(), "\"}}
"]), String.concat([live_pre(), "!\"}}
"])]], Lp.lv0(True{}, "17.0", Api.Anthropic{})), "|"),
    Con{String.join(Lp.runs([[String.concat([live_l1(), live_l2(), live_pre(),
      "\"}}
", live_pre(), "!\"}}
"])]], Lp.lv0(True{}, "17.0", Api.Anthropic{})), "|"), Nil{}}}}
    == Con{"OK café 😀a\\nb \"q\" é 😀!
END
",
      Con{"partial #17.0 +0 : café 😀a\\nb \"q\" é 😀|partial #17.0 +18 : !",
      Con{"partial #17.0 +0 : café 😀a\\nb \"q\" é 😀!", Nil{}}}} : List<&2, String>}

# ---- c4b-core-step-flat: the runtimes are read by name only, so their
# order is never observable (rt_put puts the runtime it sets at the
# head), and a put changes only its own name's runtime: with set_rt the
# one writer of a runtime but a rename (tests/state_guard.py), a step's
# view diffs only the runtimes its rt outs name (view.bend rt_moved) ----

# LAW: a runtime put is the one found under its name.
law rt_put_finds_it:
  for +xs: List<&2, HX.Rt>
  for +x: HX.Rt
  {HC.rt_find(HC.rt_put(xs, x), HX.Rt.name(x)) == x : HX.Rt}

# LAW: a runtime put leaves every other name's runtime as it was.
law rt_put_keeps_others:
  for +xs: List<&2, HX.Rt>
  for +x: HX.Rt
  for +n: String
  for +e: {String.eq(HX.Rt.name(x), n) == False{} : Bool}
  {HC.rt_find(HC.rt_put(xs, x), n) == HC.rt_find(xs, n) : HX.Rt}
