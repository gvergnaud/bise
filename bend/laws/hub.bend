# bend/laws/hub.bend: the hub (sb-core): agents, messages, timers, watches, pause, queued input, composer, cross-hub, routing, followed jobs.
# Open claims, each one closed by bend/proofs/hub.bend (moved from
# LAWS.bend). The gate checks every bend/proofs/*.bend: a law here
# with no proof there fails it.
import Base
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/inv.bend as HI
import ../hub/gen/state-codec.bend as HSC
import ../hub/timers.bend as HTM
import ../hub/wakes.bend as HWK
import ../hub/xhub-model.bend as HXH
import ../hub/xhub.bend as HXB
import ../hub/gen/kit.bend as HK
import ../hub/gen/feed.bend as HFD
import ../hub/gen/fx.bend as HF
import ../hub/pause.bend as HP
import ../hub/reach.bend as HR
import ../hub/answers.bend as HA
import ../hub/cold.bend as CO

# ---- Switchboard hub (hub/*.bend, the sb-core process) ----
# The laws quantify over EVERY hub state, EVERY input (any JSON line the
# daemon may send) and EVERY sequence of inputs: no test could enumerate
# them. HC.step is the decision of one input, HC.commit what sb-core keeps
# of it, HC.run its loop over a list of inputs.

# LAW: after any input, the hub's state is exactly the replay of the
# journal events it emitted over the state before (the journal is a
# faithful log: a restart that replays it rebuilds the same state).
law hub_step_replays:
  for +h: HC.Hub
  for +i: J.Json
  {HX.Core.d(HC.Hub.d(h), HX.Cx.c(HC.Hub.d(h), HC.step(h, i)))
    == HM.apply_all(HX.journal(HX.Core.fx(HC.Hub.d(h), HX.Cx.c(HC.Hub.d(h), HC.step(h, i)))), HC.Hub.d(h)) : HS.St}

# LAW: over any sequence of inputs (git answers still missing included),
# the final state is the replay of the whole journal over the first one.
law hub_run_replays:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  {HC.Hub.d(HC.RunR.hub(HC.run(ins, h))) == HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)) : HS.St}

# LAW: every message the hub ever sends, over any sequence of inputs,
# takes the next free id of the state it is sent in: no id is reused, so
# no message overwrites (loses) another.
law msg_ids_fresh:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HX.Fresh(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h))

# LAW: no message the hub ever sends is both an automatic reply and a
# question (RFC 0003 §7): automatic replies can never start a chain.
law auto_never_asks:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HX.Quiet(HC.RunR.journal(HC.run(ins, h)))

# LAW: after a tick, every wait still pending ends later than now: a wait
# past its deadline always gets its timeout (with the 25 s cap of `sb
# wait`, no set of agents waiting on each other blocks longer).
law tick_clears_late_waiters:
  for -o: HS.St
  for +cx: HX.Cx<o>
  HC.Later(HC.waiters_of_cx(o, HC.tick(o, cx)), HX.R.now(HX.Cx.r(o, cx)))

# LAW: stopping or dropping a task leaves no wait of it behind (each got
# its `stopped` error), whatever the state.
law stop_leaves_no_waiter:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +life: HS.Life
  for +why: String
  HC.NoneOf(HC.waiters_of_cx(o, HC.stop_task(o, cx, n, life, why)), n)

# ---- the timers of `sb every` (hub/timers.bend, core.bend `timer_fire`) ----
# The timers' state is in HS.St and their lines (every_set, every_fired,
# every_run, every_stop) are events like the others: hub_run_replays,
# msg_ids_fresh and the rest cover them as they are. The two laws on the
# wake are LOCAL: they are stated on core.bend's `timer_fire`, the only
# path a timer wake takes (the tick's due wakes and /scheduled's run now),
# not over any sequence of inputs: the global form would need a fourth
# proof field in HX.Core and an obligation at every emit of the hub.

# LAW: a fire counts only when its message was queued (amb-tools m_5822: a
# one-shot timer was spent on a lost wake). After the wake's send `p`: a
# refused send adds only the retry line (a due wake: every_fired
# undelivered, fired unchanged, next in a minute; a run now: nothing), no
# count and no stop; a queued one adds exactly the fire (every_fired with
# fired + 1, or every_run for a run now) and, for its last time, the
# timer's stop.
law fire_is_a_message:
  for -o: HS.St
  for p: HX.Cx<o> & HC.SendR
  for +t: HS.Timer
  for +due: Bool
  for +next: Nat
  {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.timer_fire.sent(o, p, t, due, next)))) == HI.fire_journal(o, p, t, due, next) : List<&2, HM.Ev>}

# LAW: a timer never stacks wakes: while a message from bise is still
# queued for its agent, a fire (due or run now) changes nothing at all.
law wake_never_stacked:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +t: HS.Timer
  for +text: String
  for +due: Bool
  for +next: Nat
  for q: {HC.bise_queued(HX.d(o, cx), HS.Timer.agent(t)) == True{} : Bool}
  {HC.timer_fire(o, cx, t, text, due, next) == cx : HX.Cx<o>}

# LAW (per event): a counted fire leaves a timer below its times, or it was
# the last one (`spent`: fire_is_a_message shows the same step stops it).
# With the guard (a due wake fires only below its times, HTM.may_fire), a
# timer with times = n never fires more than n times.
law times_bound:
  for +t: HS.Timer
  for +next: Nat
  for +at: Nat
  {HTM.under(HTM.Timer.fire(t, 1n+HS.Timer.fired(t), next, at, False{})) == Bool.not(HTM.spent(t)) : Bool}

# LAW (sched-names, architect m_14532): a timer's name (`every_name`) is
# its name and nothing else: naming it back gives the same timer, so its
# schedule, count, next time, runs and words stay as they were.
law named_keeps_the_rest:
  for +t: HS.Timer
  for +name: String
  {HTM.Timer.named(HTM.Timer.named(t, name), HS.Timer.name(t)) == t : HS.Timer}

# LAW (sched-names): naming a timer gives it that name.
law named_is_the_name:
  for +t: HS.Timer
  for +name: String
  {HS.Timer.name(HTM.Timer.named(t, name)) == name : String}

# LAW (sched-names, architect m_15603): a timer is announced once. The
# every_name step announces it when it is held (core.bend timer_name:
# announce = Timer.held), and its line, replayed or live, leaves it not
# held: a second every_name, after any restart, never announces again.
law named_announces_once:
  for +t: HS.Timer
  for +name: String
  {HS.Timer.held(HTM.Timer.name_line(t, name)) == False{} : Bool}

# LAW: dropping (archiving) a task leaves no live timer of it, in the same
# step (beside stop_leaves_no_waiter). A stop (`sb stop`) keeps them: they
# wait, as before.
law stop_leaves_no_timer:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +why: String
  HI.TimerNoneOf(HS.Tms.live(HS.St.tm(HX.d(o, HC.stop_task(o, cx, n, HS.LArchived{}, why)))), n)

# ---- the project's pause (cost-pause, hub/pause.bend) ----
# Its lines (paused, resumed) are events like the others: the replay laws
# (hub_run_replays, replay_reproduces) cover them, so a restarted hub is
# still paused; gen/journal-laws.bend's journal_paused_round and
# journal_resumed_round pin the lines themselves. LOCAL laws, on the
# steps of core.bend's pause section and its gates.

# LAW: a paused project delivers nothing: pump (every message's way to an
# agent: his, an agent's, bise's timer and event wakes, a card's answer,
# another hub's) leaves the step as it was; the message stays queued.
law paused_delivers_nothing:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for q: {HP.is_paused(HX.d(o, cx)) == True{} : Bool}
  {HC.pump(o, cx, n) == cx : HX.Cx<o>}

# LAW: a paused project fires no timer: a due one waits for /resume.
law timer_held_while_paused:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +t: HS.Timer
  for +due: Bool
  for q: {HP.is_paused(HX.d(o, cx)) == True{} : Bool}
  {HC.timer_may(o, cx, t, due) == False{} : Bool}

# LAW: pause is idempotent: a second /pause only says so (no line, no
# interrupt); /resume of a running project likewise.
law pause_idempotent:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for q: {HP.is_paused(HX.d(o, cx)) == True{} : Bool}
  {HC.pause(o, cx) == HC.notice(o, cx, HP.ALREADY()) : HX.Cx<o>}

law resume_idempotent:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for q: {HP.is_paused(HX.d(o, cx)) == False{} : Bool}
  {HC.resume(o, cx) == HC.notice(o, cx, HP.NOT_PAUSED()) : HX.Cx<o>}

# LAW: the paused line pauses, the resumed line resumes, and neither
# touches a message: the queue's order is the same before and after.
law paused_line_pauses:
  for +s: HS.St
  for +at: Nat
  {HS.St.paused(HM.apply(s, HM.EPaused{at})) == Some{at} : Maybe<&2, Nat>}

law resumed_line_resumes:
  for +s: HS.St
  for +at: Nat
  {HS.St.paused(HM.apply(s, HM.EResumed{at})) == None{} : Maybe<&2, Nat>}

law pause_keeps_order:
  for +s: HS.St
  for +at: Nat
  {HM.every(HM.apply(s, HM.EPaused{at})) == HM.every(s) : List<&2, HS.MRec>}

law resume_keeps_order:
  for +s: HS.St
  for +at: Nat
  {HM.every(HM.apply(s, HM.EResumed{at})) == HM.every(s) : List<&2, HS.MRec>}

# ---- the watches of `sb wake` (event-wake, hub/wakes.bend) ----
# Their lines (wake_set, wake_end) are events like the others: the replay
# laws (hub_run_replays, replay_reproduces) cover them as they are, so a
# watch survives a restart and an ended one stays ended (wake_replay_keeps
# below pins the line itself). Like the timers' laws, the wake laws are
# LOCAL, on the steps of core.bend's watches section.

# LAW: a watch wakes its agent once. Its hit ends it (wake_end, in the
# same step as its message): after the end, no hit finds it.
law wake_hit_once:
  for +ws: HS.Wakes
  for +id: Nat
  for +why: String
  for +at: Nat
  {HWK.hit(HWK.end(ws, id, why, at), id) == None{} : Maybe<&2, HS.Watch>}

# LAW: a hit that finds no live watch (ended: hit, max, stop, or its
# agent dropped) changes nothing at all: no message, no line.
law wake_gone_never_hits:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +text: String
  {HC.wake_hit.found(o, None{}, cx, text) == cx : HX.Cx<o>}

# LAW: dropping (archiving) a task leaves no live watch of it, in the
# same step (beside stop_leaves_no_timer). A stop keeps them: they wait.
law wake_drop_ends:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  HI.WatchNoneOf(HS.Wakes.live(HC.wks(o, HC.wakes_gone(o, HS.LArchived{}, cx, n))), n)

# LAW: a watch past its max wakes once: the tick ends it, and an ended
# watch is never due again.
law wake_max_once:
  for +xs: List<&2, HS.Watch>
  for +id: Nat
  for +now: Nat
  {HWK.find(HWK.due(HWK.drop(xs, id), now), id) == None{} : Maybe<&2, HS.Watch>}

# LAW: a watch's line read back is the same watch (the journal keeps it
# across a restart: its spec, words, times and agent).
law wake_replay_keeps:
  for +x: HS.Watch
  {HSC.Watch.journal_of(HSC.Watch.journal_json(x)) == x : HS.Watch}

# ---- who a wake reaches, and when (wake-deliver, hub/reach.bend) ----
# His rule: his message, his card answer, a watch's wake and a timer's
# fire reach an active agent NOW. Idle: a turn starts. Busy: it is steered
# into the turn (pump's steer, as `sb send`'s default --mode steer).
# Starting or down: it waits in the queue. Paused: nothing is delivered
# or steered (paused_delivers_nothing, timer_held_while_paused,
# paused_steers_nothing). wake_never_stacked is unchanged.
# CHANGED (wake-deliver): a watch's wake was queued mode (held until the
# busy turn's end) and a due timer fired only for an idle agent (a busy
# one at its next idle); now both reach a busy agent in its turn.

# LAW: a watch's wake (its hit, its max) is a message from bise that is
# never queued mode: an idle agent starts a turn, a busy one gets it in
# its turn (busy_steers_now).
law wake_steers_busy:
  for +a: String
  for +text: String
  {HC.wake_snd(a, text) == HC.Snd{HM.HUB(), a, text, False{}, None{}, False{}, False{}, False{}, None{}, "", None{}} : HC.Snd}

# LAW: a busy agent's pump steers every queued message that is not queued
# mode (a wake, a timer's fire, his message, an agent's): in order, now.
law busy_steers_now:
  for +m: HS.Msg
  for +rest: List<&2, HS.Msg>
  for q: {HS.Msg.queued(m) == False{} : Bool}
  {HC.not_queued_mode(m <> rest) == m <> HC.not_queued_mode(rest) : List<&2, HS.Msg>}

# LAW: a timer reaches an active agent that is busy (before: idle only).
law timer_reaches_busy:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  for p: {HC.is_active(HS.Agent.life(a)) == True{} : Bool}
  for q: {HX.Rt.run(HC.rt(o, cx, HS.Agent.name(a))) == HS.RBusy{} : HS.Run}
  {HC.ready.go(o, Some{a}, cx) == True{} : Bool}

# LAW: a due wake of a timer whose agent it reaches (idle or busy:
# timer_reaches_busy), in a project not paused, with no wake from bise
# still queued for it, fires on the timer's own terms only: due and under
# its times.
law timer_fires_busy:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +t: HS.Timer
  for p: {HP.is_paused(HX.d(o, cx)) == False{} : Bool}
  for q: {HC.bise_queued(HX.d(o, cx), HS.Timer.agent(t)) == False{} : Bool}
  for r: {HC.timer_ready(o, cx, HS.Timer.agent(t)) == True{} : Bool}
  {HC.timer_may(o, cx, t, True{}) == HTM.may_fire.go(True{}, t, True{}, HC.now(o, cx)) : Bool}

# LAW: a paused project steers nothing into a busy agent's turn either:
# pump (the only step that steers: pump.send.go's "steer") leaves the
# step as it was; the wake stays queued for /resume.
law paused_steers_nothing:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for q: {HP.is_paused(HX.d(o, cx)) == True{} : Bool}
  for r: {HX.Rt.run(HC.rt(o, cx, n)) == HS.RBusy{} : HS.Run}
  {HC.pump(o, cx, n) == cx : HX.Cx<o>}

# LAW: a page question answered on its page: its card closes, then the
# card's asker is told or not (page_answer_tells_asker below).
law page_answer_closes:
  for -o: HS.St
  for +c: HS.Card
  for +cx: HX.Cx<o>
  for +to: String
  for +page: String
  for +text: String
  for +block: String
  {HC.page_answered(o, Some{c}, cx, to, page, text, block) == HC.page_answered.tell(o, HR.tells_asker(HM.resolve(HX.d(o, HC.close_card_ans(o, cx, HS.Card.id(c), "answered", HA.page_ans(text, page, block))), HS.Card.agent(c)), HM.resolve(HX.d(o, HC.close_card_ans(o, cx, HS.Card.id(c), "answered", HA.page_ans(text, page, block))), to)), HC.close_card_ans(o, cx, HS.Card.id(c), "answered", HA.page_ans(text, page, block)), c, to, page, text) : HX.Cx<o>}

# LAW: the asker told: exactly one message from bise to the card's asker,
# with his answer; not told: nothing more than the close.
law page_answer_tells_asker:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  for +to: String
  for +page: String
  for +text: String
  {HC.page_answered.tell(o, True{}, cx, c, to, page, text) == HC.fst(o, HC.send(o, cx, HC.msg(HM.HUB(), HS.Card.agent(c), HR.answer_words(HS.Card.id(c), page, to, text), False{}, None{}, False{}, False{}))) : HX.Cx<o>}

law page_answer_quiet:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  for +to: String
  for +page: String
  for +text: String
  {HC.page_answered.tell(o, False{}, cx, c, to, page, text) == cx : HX.Cx<o>}

# LAW (plan-ok, architect m_26549, m_26571): a page's own card is asked
# by the first live agent of its chain (a plan's agent, then the page's),
# else main. Live: registered and active (not stopped, failed or
# archived).
law page_card_asker_main:
  for +s: HS.St
  {HR.page_asker(s, []) == HM.MAIN() : String}

law page_card_asker_first:
  for +s: HS.St
  for +h: String
  for +t: List<&2, String>
  for q: {HR.agent_active(s, h) == True{} : Bool}
  {HR.page_asker(s, h <> t) == h : String}

law page_card_asker_next:
  for +s: HS.St
  for +h: String
  for +t: List<&2, String>
  for q: {HR.agent_active(s, h) == False{} : Bool}
  {HR.page_asker(s, h <> t) == HR.page_asker(s, t) : String}

# LAW: a plan card opens one card, asked by that chain's agent; a kind
# that is no page card's opens nothing.
law page_card_opens:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agents: List<&2, String>
  for +text: String
  {HC.page_card(o, cx, agents, "plan", text) == HC.page_card.open(o, cx, "plan", HR.page_asker(HX.d(o, cx), agents), text) : HX.Cx<o>}

# LAW (notif-artifact, architect m_26569): a page's question or drawn
# pick is a page card too: one card, asked by the page's agent (main when
# it is gone), never main's by default.
law page_card_opens_question:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agents: List<&2, String>
  for +text: String
  {HC.page_card(o, cx, agents, "question", text) == HC.page_card.open(o, cx, "question", HR.page_asker(HX.d(o, cx), agents), text) : HX.Cx<o>}

law page_card_other_kind:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agents: List<&2, String>
  for +text: String
  {HC.page_card(o, cx, agents, "merge", text) == HC.notice(o, cx, "not a page card: merge") : HX.Cx<o>}

# LAW: who is told: the asker when it is another agent than the one the
# pick went to; never the same agent twice, never an unknown asker.
law page_asker_other:
  for +x: String
  for +y: String
  for q: {String.eq(x, y) == False{} : Bool}
  {HR.tells_asker(Some{x}, Some{y}) == True{} : Bool}

law page_asker_same:
  for +x: String
  for +y: String
  for q: {String.eq(x, y) == True{} : Bool}
  {HR.tells_asker(Some{x}, Some{y}) == False{} : Bool}

law page_asker_unknown:
  for to: Maybe<&2, String>
  {HR.tells_asker(None{}, to) == False{} : Bool}

# LAW (L2, no message lost or doubled): a new hub keeps each message as
# ONE record — its text and its single delivery state; no state exists
# without its message (the state is a field of the record).
law init_unique:
  for +w: String
  HI.Unique(HM.init(w))

# LAW (L2): from a state whose messages are split right (HI.Split: every
# state the hub reaches, by split_at_init and split_kept_by_run), any
# sequence of inputs keeps each message one record: no two records ever
# share an id, every id is below the next free one.
law msgs_unique:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.Split(HC.Hub.d(h))
  HI.Unique(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# LAW (L2): over any sequence of inputs, the messages of the first state
# start, in order, the messages of the last: a message once sent is never
# removed (its record, and so its state, stays). Every message is
# HM.every, the two sides put together; the hypothesis HI.Split holds in
# every state the hub reaches: split_at_init and split_kept_by_run prove
# it, for every input kind, and split_after_replay for the boot replay.
law msgs_kept:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.Split(HC.Hub.d(h))
  HI.Kept(HC.Hub.d(h), HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# LAW (L3, delivered at most once, RFC 0003 §6.1): over any sequence of
# inputs, a message becomes Delivered only while it is Queued, and it goes
# back to Queued only as an explicit steer_leftover (steering the runtime
# never read). So a message is delivered once, and again only after an
# explicit steer_leftover.
law delivered_once:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HX.Deliv(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h))

# LAW (L12): stopping or dropping a task, in any state whose messages are
# split right (every state the hub reaches: split_kept_by_run), leaves no
# message queued for it among all its messages (each got
# recipient_unavailable). queued_for reads the same (hot_queued_for).
law stop_leaves_no_queue:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +life: HS.Life
  for +why: String
  for u: HI.Split(HX.d(o, cx))
  {HM.queued_in(HM.every(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.names_of(HX.d(o, HC.stop_task(o, cx, n, life, why)), n)) == Nil{} : List<&2, HS.Msg>}

# ---- queued user input (the window's "send queued", bise desktop) ----
# The user's `input {queued: true}` is a queued-mode message (Msg.queued),
# the path `sb send --mode queued` already takes. An older sb-core ignores
# the field and delivers at once (a safe degrade, no journal change).

# LAW: a queued input never answers a question card, at send: it is a
# message for the next turn. A card still open at the turn's end stays
# open; the input arrives as a message (says.card with no card).
law user_queued_never_answers_a_card:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +to: String
  for +text: String
  for +parts: String
  for +from: String
  {HC.user_says(o, cx, to, text, True{}, parts, from) == HC.says.card(o, None{}, cx, to, text, True{}, parts, from) : HX.Cx<o>}
# (his words typed in another thread: bend/laws/say_from.bend)

def any_queued(xs: List<&2, HS.Msg>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      HS.Msg.queued(h) || any_queued(t)

# LAW: what a busy agent is handed (steered) holds no queued-mode message:
# a queued input waits for the turn's end. So a plain input sent while
# queued ones wait goes first (steered); the queued ones follow at idle,
# in id order (queued_for), in one say batch (user_queued_said_at_idle),
# each once (delivered_once).
law user_queued_waits_for_turn_end:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +name: String
  for +q: List<&2, HS.Msg>
  for +m: Bool
  {HC.pump.run(o, HS.RBusy{}, cx, name, q, m) == HC.pump.batch(o, HC.not_queued_mode(q), cx, name, True{}, m) : HX.Cx<o>}

law busy_batch_has_no_queued:
  for +xs: List<&2, HS.Msg>
  {any_queued(HC.not_queued_mode(xs)) == False{} : Bool}

# LAW: at idle, everything queued for the agent goes in one say batch, in
# its order (queued and plain alike).
law user_queued_said_at_idle:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +name: String
  for +q: List<&2, HS.Msg>
  for +m: Bool
  {HC.pump.run(o, HS.RIdle{}, cx, name, q, m) == HC.pump.batch(o, q, cx, name, False{}, m) : HX.Cx<o>}

# ---- the composer rule (lead m_8777, architect m_8801) ----
# An approval card takes one of its options, never typed words: the TUI's
# /answer and the window share it. The words leave the card open and wake
# nobody (the window sends them to the agent as a message).

# LAW: typed words on a confirm card (not 1, 2, 3 nor the TUI's explicit
# `deny: <why>`) are refused: no verdict, the card stays open.
law card_confirm_words_refused:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  for +text: String
  for +parts: String
  for +via: String
  for e: {HA.confirm_ok(text) == False{} : Bool}
  {HC.answer.kind(o, HC.KConfirm{}, cx, c, text, parts, via) == HC.words_refused(o, cx, c, HA.confirm_opts()) : HX.Cx<o>}

# LAW: `no` and `no: <why>` (any case, lead m_10900) are deny aliases: the
# card gets the same verdict path as its option 3, with his words as the
# note (the gate reads them as no, never allow: gate::answer_of).
law card_confirm_no_alias:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  for +text: String
  for +parts: String
  for +via: String
  for e: {HA.is_no(String.to_lower(String.trim(text))) == True{} : Bool}
  {HC.answer.kind(o, HC.KConfirm{}, cx, c, text, parts, via) == HC.answer.confirm(o, True{}, cx, c, text, via) : HX.Cx<o>}

# LAW: only `no` itself or its `no:` prefix: words that start like it stay
# words.
law card_confirm_nope_is_words:
  {HA.confirm_ok("nope") == False{} : Bool}

law card_confirm_now_is_words:
  {HA.confirm_ok("now") == False{} : Bool}

law card_confirm_no_thanks_is_words:
  {HA.confirm_ok("No thanks") == False{} : Bool}

# LAW: a hub item (merge, feature_*, update, signin) takes only one of its
# options' numbers: other words are refused, the item stays open, main is
# not woken.
law card_choice_words_refused:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  for +text: String
  for +parts: String
  for +via: String
  for e: {HC.is_option(HS.Card.text(c), text) == False{} : Bool}
  {HC.answer.kind(o, HC.KChoice{}, cx, c, text, parts, via) == HC.words_refused(o, cx, c, "one of its numbers") : HX.Cx<o>}

# ---- bise's cross-hub messages (desktop S2, hub/xhub.bend) ----
# Local laws on xhub.bend's decisions and xhub-model.bend's state: the
# core's `cross-hub` section only wires them (it decides nothing).

# LAW: only main forwards the user's words to a project.
law xhub_only_main_forwards:
  for +s: HS.St
  for +from: String
  for +id: Nat
  for e: {String.eq(from, HM.MAIN()) == False{} : Bool}
  {HXB.forward(s, from, id) == HXB.FwdErr{"only main forwards to the projects"} : HXB.Fwd}

# LAW: a message of his is forwarded once: with an outbox entry for it,
# main's forward is refused.
law xhub_forward_once:
  for +s: HS.St
  for +id: Nat
  for +ob: HS.Out
  for e: {HXH.out_by_msg(HS.Xs.outbox(HS.St.x(s)), id) == Some{ob} : Maybe<&2, HS.Out>}
  {HXB.forward(s, HM.MAIN(), id) == HXB.FwdErr{String.append(String.append(HXB.mid(id), " was already forwarded to "), HS.Out.project(ob))} : HXB.Fwd}

# LAW: only his own words go by reference (architect's B): a message not
# from the user is refused, whatever it says (bise's own question goes by
# `sb project ask`, from @bise, with no authority).
law xhub_forward_needs_his_message:
  for +m: HS.Msg
  for +id: Nat
  for e: {String.eq(HS.Msg.from(m), HM.USER()) == False{} : Bool}
  {HXB.his.go(Some{m}, id) == HXB.FwdErr{String.append(HXB.mid(id), " is not a message the user sent you: forward his own words only (your own question: sb project ask)")} : HXB.Fwd}

# LAW: bise's own question needs words: a blank one is refused, any other
# may go (architect m_8864: the core's section validates nothing itself).
law xhub_ask_needs_words:
  for +text: String
  for b: Bool
  for e: {HM.trim_is_empty(text) == b : Bool}
  {HXB.ask_ok(text) == Bool.pick(Maybe<&2, String>, b, Some{"empty question"}, None{}) : Maybe<&2, String>}

# LAW: a send settles once: after its settle, no answer opens it again
# (a second x_reply for it changes nothing: core's xanswer on None).
law xhub_outbox_settles_once:
  for +x: HS.Xs
  for +xid: Nat
  {HXB.open(HXH.x_op(x, xid, HXH.OpSettled{}), xid) == None{} : Maybe<&2, HS.Out>}

# LAW: a delivery is said once: once received, the same origin (hub, xid)
# is seen (core's xin acks it and says nothing).
law xhub_xin_delivered_once:
  for +x: HS.Xs
  for +i: HS.XIn
  {HXB.seen(HXH.x_in(x, i), HS.XIn.hub(i), HS.XIn.xid(i)) == True{} : Bool}

# LAW: an answered record is never due again (core's xreplied journals
# x_replied with each answer it sends back).
law xhub_xin_replied_once:
  for +i: HS.XIn
  {HXB.reply.of(Some{HS.XIn.with_replied(i, True{})}) == None{} : Maybe<&2, HS.XIn>}

# ---- the routing hold (desktop S2 step 2, xhub.bend + core's `routing
# hold` section) ----

# LAW: a route has one outcome: once it ended (sent or cancelled), it is
# never held again, so no fire, correction or cancel acts on it twice.
law route_one_outcome:
  for +r: HS.Route
  for s: HS.RSt
  for e: {HXB.is_held(s) == False{} : Bool}
  {HXB.held.of(Some{HXH.route_op(HXH.REnd{s}, r)}) == None{} : Maybe<&2, HS.Route>}

# LAW: a correction acts only on a held route: one that went or was
# cancelled gets a notice, nothing else.
law route_correct_only_held:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +rid: Nat
  for +to: String
  for +name: String
  {HC.route_correct.go(o, None{}, cx, rid, to, name) == HC.route_gone(o, cx, rid) : HX.Cx<o>}

# LAW: a correction changes the project only: its due time stays (his 2 s
# don't start again).
law route_correct_keeps_due:
  for +r: HS.Route
  for +to: String
  for +name: String
  {HS.Route.due(HXH.route_op(HXH.RTo{to, name}, r)) == HS.Route.due(r) : Nat}

# LAW: a held route's act is due HOLD_MS (2 s) after the hold, so with
# act_due_only it fires at that tick or later, never before.
law route_fires_at_due_only:
  for +x: HS.Xs
  for +tm: HS.Tms
  for +d: HS.Route
  for +now: Nat
  {HS.Act.due(HXB.route_act(HXB.route_of_hold(x, tm, d, now))) == Nat.add(now, 2000n) : Nat}

# LAW (architect m_8883): a fire makes no turn: his message is journaled
# (an id for the history and `--input`) by route_said, never by a send
# (which pumps main), then delivered, settled and forwarded; main reads
# it in a note at its next turn.
law route_fire_makes_no_turn:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +r: HS.Route
  {HC.route_fire(o, Some{r}, cx) == HC.route_fire.sent(o, HC.route_said(o, cx, r), r, HS.St.next_msg(HC.st(o, cx))) : HX.Cx<o>}

# ---- a held route with no target (an older hub's route model left it
# in its journal; S2 step 5 is gone, only his `@<project>` routes now) ----

# LAW: such a route whose act fires goes plain to bise's main (the cancel
# path), never to a project's outbox: his words are never lost.
law route_unclear_goes_plain:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +r: HS.Route
  for e: {HXB.is_unclear(r) == True{} : Bool}
  {HC.route_go(o, Some{r}, cx) == HC.route_plain(o, cx, r) : HX.Cx<o>}

# LAW (route-name, architect m_24939): a fire says where his words went in
# bise's main thread, by his message's id: his line, its id and parts,
# then the `routed` record naming the project, the out and that id (the
# window and the TUI fold it on his entry by the id, never by position).
law route_fire_says_where:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +r: HS.Route
  for +id: Nat
  for +xid: Nat
  {HC.route_fire.lines(o, cx, r, id, xid) == HC.feed(o, HC.you_parts(o, HC.feed(o, HC.feed(o, cx, HM.MAIN(), HFD.you(HS.Route.text(r), HXB.via_of(HS.Route.via(r)))), HM.MAIN(), HFD.you_id(id)), HM.MAIN(), HS.Route.parts(r)), HM.MAIN(), HFD.routed(HS.Route.to(r), HS.Route.name(r), xid, id)) : HX.Cx<o>}

# ---- his parts go with his routed words (R41 (6), architect m_18164) ----

# LAW: a route's correction or pick keeps his parts (only its target
# changes), so what fires carries what he sent.
law route_to_keeps_parts:
  for +r: HS.Route
  for +to: String
  for +name: String
  {HS.Route.parts(HXH.route_op(HXH.RTo{to, name}, r)) == HS.Route.parts(r) : String}

# LAW: the outbox entry a fired route makes carries its parts to the
# project (route_fire.sent's new_out).
law route_out_carries_parts:
  for +x: HS.Xs
  for +r: HS.Route
  for +id: Nat
  {HS.Out.parts(HXB.out_of_route(x, r, id)) == HS.Route.parts(r) : String}

# LAW: no parts, no key: an out or a route without parts journals and is
# delivered byte for byte as before (an older core replays it unchanged).
law route_no_parts_no_key:
  {HK.omit_s("parts", "") == Nil{} : List<&2, Sigma<&2, &2, String, _ => J.Json>>}

# LAW: on the project, his forwarded message keeps its parts (the
# you-parts path of a direct input); bise's own question has none.
law xin_input_keeps_parts:
  for +text: String
  for +parts: String
  {HC.xin.snd(HS.KInput{}, text, parts) == HC.Snd{HM.USER(), HM.MAIN(), text, True{}, None{}, False{}, False{}, False{}, Some{HXB.VIA_BISE()}, parts, None{}} : HC.Snd}

law xin_ask_has_no_parts:
  for +text: String
  for +parts: String
  {HC.xin.snd(HS.KAsk{}, text, parts) == HC.Snd{HXB.FROM_BISE(), HM.MAIN(), text, True{}, None{}, False{}, False{}, False{}, None{}, "", None{}} : HC.Snd}

# ---- followed jobs (desktop S10, core.bend's `followed jobs`) ----

# LAW: before its end a followed task's progress makes no job_end: a
# progress report changes nothing but the report itself.
law follow_quiet_before_end:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +from: String
  for +summary: String
  {HC.req_report.kind(o, cx, from, "progress", summary) == cx : HX.Cx<o>}

# LAW: its done ends the job in the same step (job_end when followed).
law follow_done_at_its_report:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +from: String
  for +summary: String
  {HC.req_report.kind(o, cx, from, "done", summary) == HC.job_end(o, HC.report_done(o, cx, from), from, "done", summary) : HX.Cx<o>}

# LAW: a failure ends it at once, in the same step as the failure.
law follow_failed_at_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +from: String
  for +summary: String
  {HC.req_report.kind(o, cx, from, "failed", summary) == HC.job_end(o, HC.report_failed(o, cx, from, summary), from, "failed", summary) : HX.Cx<o>}

# LAW: an agent not followed has no job_end (so a second end of the same
# job, after the first turned follow off, says nothing).
law follow_done_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +name: String
  for +state: String
  for +summary: String
  for +label: String
  for +hub: String
  {HC.job_end.go(o, False{}, cx, name, state, summary, label, hub) == cx : HX.Cx<o>}

# LAW (land-claims, architect m_49443): a dropped claim is gone: after
# EClaimDropped{name, p} (IClaimsClean: p's file had no change left) the
# agent claims p no more, however many times it was listed.
law claim_dropped_unclaims:
  for +a: HS.Agent
  for +p: String
  {HM.str_in(HS.Agent.files(HM.agent_op(a, HM.OUnclaim{p})), p) == False{} : Bool}

# LAW (land-claims): a later edit claims the file again: after
# EFileTouched{name, p} the agent claims p, a dropped claim too.
law file_touched_claims:
  for +a: HS.Agent
  for +p: String
  {HM.str_in(HS.Agent.files(HM.agent_op(a, HM.OFile{p})), p) == True{} : Bool}

# LAW (architect m_9154): one job per follow: job_end turns follow off.
law follow_one_shot:
  for +a: HS.Agent
  {HM.follows(HM.agent_op(a, HM.OFollow{None{}})) == False{} : Bool}

# LAW (J, architect m_10223): the hub that asked for a follow is kept
# with it (journaled by EFollow), so its end can go back there.
law follow_hub_journaled:
  for +a: HS.Agent
  for +hub: String
  {HM.follow_hub(HM.agent_op(a, HM.OFollow{Some{hub}})) == hub : String}

# LAW (J): the job's end clears it with the follow: a later end of the
# same agent goes back nowhere.
law follow_hub_cleared:
  for +a: HS.Agent
  {HM.follow_hub(HM.agent_op(a, HM.OFollow{None{}})) == "" : String}

# LAW (J): an end goes back only to a hub that asked: a follow of this
# hub's own main or window ("") puts nothing in the outbox.
law follow_end_back_only_to_who_asked:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +hub: String
  for +j: J.Json
  {HC.job_end.back(o, True{}, cx, hub, j) == cx : HX.Cx<o>}

# LAW (one clock): an act fires once: after its act_done, it is gone.
law act_fires_once:
  for +tm: HS.Tms
  for +id: Nat
  {HTM.act_find(HS.Tms.acts(HTM.act_done(tm, id)), id) == None{} : Maybe<&2, HS.Act>}

def all_due(xs: List<&2, HS.Act>, +now: Nat) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Nat.is_le(HS.Act.due(h), now) && all_due(t, now)

# LAW (one clock): a tick fires only acts due by its now.
law act_due_only:
  for +xs: List<&2, HS.Act>
  for +now: Nat
  {all_due(HTM.acts_due(xs, now), now) == True{} : Bool}

# LAW (L12): stopping or dropping a task, in any state where no two open
# cards share an id (every state the hub reaches: cards_unique), leaves no
# open card of it.
law stop_leaves_no_card:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +n: String
  for +life: HS.Life
  for +why: String
  for u: HI.UniqueCards(HX.d(o, cx))
  {HC.card_ids(HS.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), n, Nil{}) == Nil{} : List<&2, Nat>}

# LAW (L12 support): a new hub has no two open cards with one id...
law init_cards_unique:
  for +w: String
  HI.UniqueCards(HM.init(w))

# ...and any sequence of inputs keeps it so (each card takes the next
# free id).
law cards_unique:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.UniqueCards(HC.Hub.d(h))
  HI.UniqueCards(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# LAW (L4 support): a new hub keeps the agents' names apart...
law init_names_apart:
  for +w: String
  HI.NamesApart(HM.init(w))

# ...and any sequence of inputs keeps them apart: no name or old name of an
# agent is ever a name or old name of another one (a message sent to an
# agent can only be for it).
law names_apart:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.NamesApart(HC.Hub.d(h))
  HI.NamesApart(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# ---- the split messages (issue 21 steps 3-4): St.hot and St.old ----
# The messages are two lists: St.hot, the hot ones (HM.MRec.is_hot:
# queued, or owing a reply), ids ascending, what the walks read; and
# St.old, the cold ones, ids descending (newest first), which a step
# reads only by id. HM.every puts them back together, in send order: what
# the laws msgs_unique, msgs_kept and view_all read. HI.Split says the
# two sides are right; these laws say it always holds, and that each
# message event does on the split what it did on one list.

# LAW: a new hub's split is right...
law split_at_init:
  for +w: String
  HI.Split(HM.init(w))

# ...any sequence of inputs keeps it right (every input kind: each event
# it journals is fresh, msg_ids_fresh)...
law split_kept_by_run:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.Split(HC.Hub.d(h))
  HI.Split(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# ...and so does a boot replay's batch (it skips a message_sent whose id is
# below next_msg: CO.replay_fresh).
law split_after_replay:
  for +ms: List<&2, Maybe<&2, HM.Ev>>
  for +st: HS.St
  for +i: Nat
  for +sk: List<&2, Nat>
  for u: HI.Split(st)
  HI.Split(Pair.fst(HS.St, List<&2, Nat>, CO.replay_batch(ms, st, i, sk)))

# LAW: every message of a split state is in send order: ids ascending.
law ids_ascending:
  for +s: HS.St
  for u: HI.Split(s)
  HI.Asc(HM.every(s))

# LAW: the cold ones are inert: on a split state, any event with a fresh
# id does to every message what it did to one list of them (HI.apply1:
# the new message last and its reply's settle, a state, a settle).
law cold_is_inert:
  for +s: HS.St
  for +e: HM.Ev
  for u: HI.Split(s)
  for k: HX.FreshEv(s, e)
  {HM.every(HM.apply(s, e)) == HI.apply1(HM.every(s), e) : List<&2, HS.MRec>}

# LAW (name-free, architect m_31494): a take of `n`, an old name of a live
# agent `h`, moves its queued messages to `n` to `h` (msgs_moved): queued
# still, same id, now to `h`...
law take_moves_live_queued:
  for +r: HS.MRec
  for +n: String
  for +h: String
  for q: {HM.is_queued(HS.MRec.st(r)) == True{} : Bool}
  for e: {String.eq(HS.Msg.to(HS.MRec.msg(r)), n) == True{} : Bool}
  HX.Both<{HM.moved([r], n, h) == [HM.MRec.to(r, h)] : List<&2, HS.MRec>}, HX.Both<{HM.MRec.id(HM.MRec.to(r, h)) == HM.MRec.id(r) : Nat},
    HX.Both<{HM.is_queued(HS.MRec.st(HM.MRec.to(r, h))) == True{} : Bool}, {HS.Msg.to(HS.MRec.msg(HM.MRec.to(r, h))) == h : String}>>>

# ...and the replies owed to `n`: still owed, same id, now by `h`...
law take_moves_live_owed:
  for +r: HS.MRec
  for +n: String
  for +h: String
  for o: {HM.MRec.owes(r) == True{} : Bool}
  for e: {String.eq(HS.Msg.to(HS.MRec.msg(r)), n) == True{} : Bool}
  HX.Both<{HM.moved([r], n, h) == [HM.MRec.to(r, h)] : List<&2, HS.MRec>}, HX.Both<{HM.MRec.id(HM.MRec.to(r, h)) == HM.MRec.id(r) : Nat},
    HX.Both<{HM.MRec.owes(HM.MRec.to(r, h)) == True{} : Bool}, {HS.Msg.to(HS.MRec.msg(HM.MRec.to(r, h))) == h : String}>>>

# ...and msgs_moved replays to the same hot side, on every message too.
law msgs_moved_replays:
  for +s: HS.St
  for +n: String
  for +h: String
  for u: HI.Split(s)
  HX.Both<{HS.St.hot(HM.apply(s, HM.EMsgsMoved{n, h})) == HM.moved(HS.St.hot(s), n, h) : List<&2, HS.MRec>},
    {HM.every(HM.apply(s, HM.EMsgsMoved{n, h})) == HM.moved(HM.every(s), n, h) : List<&2, HS.MRec>}>

# LAW: on a split state, each walk over the hot side reads what it
# would read over every message: the cold ones are inert. The queues...
law hot_queued_for:
  for +s: HS.St
  for +n: String
  for h: HI.Split(s)
  {HM.queued_for(s, n) == HM.queued_in(HM.every(s), HM.names_of(s, n)) : List<&2, HS.Msg>}

# ...the replies owed...
law hot_unanswered_for:
  for +s: HS.St
  for +n: String
  for h: HI.Split(s)
  {HM.unanswered_for(s, n) == HM.unanswered_in(HM.every(s), HM.names_of(s, n)) : List<&2, HS.Msg>}

# ...and the pause's counts: the messages held, the agents a resume
# wakes, the held messages that are not bise's own.
law hot_held:
  for +s: HS.St
  for h: HI.Split(s)
  {HP.held(s) == HP.held.go(HM.every(s)) : Nat}

law hot_woken:
  for +s: HS.St
  for h: HI.Split(s)
  {HP.woken(s) == List.length(&2, String, HP.woken.go(HM.every(s), Nil{})) : Nat}

law hot_his:
  for +s: HS.St
  for h: HI.Split(s)
  {HP.his(s) == HP.his.go(HM.every(s)) : Nat}

# ---- MCP App views (docs/mcp-apps.md, C5; architect m_47, m_50085) ----
# A view's call that needs his ok opens a `confirm` card outside the
# agent's turn; a view's message is one message from the fixed sender
# `view`, its plugin typed.

# LAW: a view's card opens without touching any agent's runtime (never
# the agent's `waiting` slot, its run or its turn).
law view_card_keeps_runtime:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for +call: String
  for +text: String
  for +ask: String
  {HX.R.rts(HX.Cx.r(o, HC.open_view_confirm(o, cx, agent, call, text, ask))) == HX.R.rts(HX.Cx.r(o, cx)) : List<&2, HX.Rt>}

# LAW: a view card's due finds it gone (answered or closed before):
# nothing happens, so it is closed once, by his answer or by its due.
law view_card_due_after_close:
  for -o: HS.St
  for +cx: HX.Cx<o>
  {HC.card_due(o, None{}, cx) == cx : HX.Cx<o>}

# LAW: closing a card leaves every other card as it was (an answer
# touches only its card).
law card_close_keeps_other:
  for +xs: List<&2, HS.Card>
  for +a: Nat
  for +b: Nat
  for e: {Nat.is_eq(a, b) == False{} : Bool}
  {HM.card_of(HM.drop_card(xs, a), b) == HM.card_of(xs, b) : Maybe<&2, HS.Card>}

# LAW: two cards closed in either order leave the same cards (a view's
# card and the agent's own, answered in any order).
law card_close_commutes:
  for +xs: List<&2, HS.Card>
  for +a: Nat
  for +b: Nat
  {HM.drop_card(HM.drop_card(xs, a), b) == HM.drop_card(HM.drop_card(xs, b), a) : List<&2, HS.Card>}

# LAW: a view's message is from `view`, never from the user...
law view_msg_from_view:
  for +agent: String
  for +plugin: String
  for +text: String
  for +id: Nat
  for +now: Nat
  {HS.Msg.from(HC.view_msg_of(agent, plugin, text, id, now)) == HM.VIEW() : String}

# ...a name no agent can take, so no agent's message looks like a view's
# and `sb send view` reaches no agent...
law view_name_taken:
  for +st: HS.St
  {HM.name_taken(st, HM.VIEW()) == True{} : Bool}

# ...to that view's agent only...
law view_msg_to_its_agent:
  for +agent: String
  for +plugin: String
  for +text: String
  for +id: Nat
  for +now: Nat
  {HS.Msg.to(HC.view_msg_of(agent, plugin, text, id, now)) == agent : String}

# ...queued (never steered into a turn)...
law view_msg_queued:
  for +agent: String
  for +plugin: String
  for +text: String
  for +id: Nat
  for +now: Nat
  {HS.Msg.queued(HC.view_msg_of(agent, plugin, text, id, now)) == True{} : Bool}

# ...its plugin typed, as the input named it...
law view_msg_plugin:
  for +agent: String
  for +plugin: String
  for +text: String
  for +id: Nat
  for +now: Nat
  {HS.Msg.view(HC.view_msg_of(agent, plugin, text, id, now)) == Some{plugin} : Maybe<&2, String>}

# ...and only to an active agent: one stopped, archived or gone gets no
# message (nothing is journaled, no EMsgSent).
law view_msg_needs_active:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for +plugin: String
  for +text: String
  for e: {HC.live_agent(HM.agent_named(HS.St.agents(HX.d(o, cx)), agent)) == False{} : Bool}
  {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.view_message(o, cx, agent, plugin, text))))
    == HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))) : List<&2, HM.Ev>}

# A view's call of an agent that is not active opens no card in his
# inbox (nothing is journaled); the view's call gets a no.
law view_card_needs_active:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for +call: String
  for +text: String
  for +ask: String
  for e: {HC.live_agent(HM.agent_named(HS.St.agents(HX.d(o, cx)), agent)) == False{} : Bool}
  {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.open_view_confirm(o, cx, agent, call, text, ask))))
    == HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))) : List<&2, HM.Ev>}

# A view's card is said to the shell by its id, typed (fx view_card),
# never found by its place's text (d4, mcp-apps review). The last
# effect of a step.
def last_fx.or(m: Maybe<&2, HX.Fx>, +f: HX.Fx) -> Maybe<&2, HX.Fx>:
  match m:
    case None{}:
      Some{f}
    case Some{+g}:
      Some{g}

def last_fx(fx: List<&2, HX.Fx>) -> Maybe<&2, HX.Fx>:
  match fx:
    case Nil{}:
      None{}
    case Con{+f, +rest}:
      last_fx.or(last_fx(rest), f)

# LAW: a view's new card ends its step with fx view_card saying its id
# (the id the card opened with: the next one)...
law view_card_says_its_id:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +v: HS.ViewAsk
  for +text: String
  {last_fx(HX.Core.fx(o, HX.Cx.c(o, HC.open_view_confirm.new(o, cx, v, text))))
    == Some{HX.FO{J.obj(HF.view_card(HS.ViewAsk.agent(v), HS.ViewAsk.call(v), Some{HS.St.next_card(HX.d(o, cx))}))}} : Maybe<&2, HX.Fx>}

# ...and the card it journals carries its ask typed (Card.view), the one
# thing stop, due, boot and the join read...
law view_card_typed:
  for +id: Nat
  for +text: String
  for +v: HS.ViewAsk
  for +now: Nat
  {HS.Card.view(HC.view_card_new(id, text, v, now)) == Some{v} : Maybe<&2, HS.ViewAsk>}

law view_card_id:
  for +id: Nat
  for +text: String
  for +v: HS.ViewAsk
  for +now: Nat
  {HS.Card.id(HC.view_card_new(id, text, v, now)) == id : Nat}

# LAW: the same ask while its card is open joins it: nothing journaled
# (no second card) and fx view_card says the open card's id (d3)...
law view_card_joins_same_ask:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  for +v: HS.ViewAsk
  for +text: String
  {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.open_view_confirm.at(o, Some{c}, cx, v, text))))
    == HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))) : List<&2, HM.Ev>}

law view_card_joined_says_its_id:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  for +v: HS.ViewAsk
  for +text: String
  {last_fx(HX.Core.fx(o, HX.Cx.c(o, HC.open_view_confirm.at(o, Some{c}, cx, v, text))))
    == Some{HX.FO{J.obj(HF.view_card(HS.ViewAsk.agent(v), HS.ViewAsk.call(v), Some{HS.Card.id(c)}))}} : Maybe<&2, HX.Fx>}

# ...and the card it joins is one of that very ask (same view, same tool
# and arguments): another tool or another view opens its own card
def found_is(m: Maybe<&2, HS.Card>, +v: HS.ViewAsk) -> Bool:
  match m:
    case None{}:
      True{}
    case Some{+c}:
      HC.view_is(HS.Card.view(c), v)

law view_card_of_its_ask:
  for +cs: List<&2, HS.Card>
  for +v: HS.ViewAsk
  {found_is(HC.view_card_of(cs, v), v) == True{} : Bool}

# ...and an agent not active: no card, and fx view_card says none (the
# view's call gets its no at once).
law view_card_says_no_card:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for +call: String
  for +text: String
  for +ask: String
  for e: {HC.live_agent(HM.agent_named(HS.St.agents(HX.d(o, cx)), agent)) == False{} : Bool}
  {last_fx(HX.Core.fx(o, HX.Cx.c(o, HC.open_view_confirm(o, cx, agent, call, text, ask))))
    == Some{HX.FO{J.obj(HF.view_card(agent, call, None{}))}} : Maybe<&2, HX.Fx>}

# A view's card gets one answer: his, its due's, its agent's stop or the
# hub's boot. No state law says it alone: each of them answers only a card
# still open and closes it (view_card_due_after_close, view_card_due_closes,
# view_card_stop_answers with stop_leaves_no_card, boot_answers_view_cards
# and boot_closes_view_cards), and core_mcp_view_tests runs Boot twice (the
# second answers nothing).
# The `confirm` effects among a step's effects (each one answers a card).
def confirm_n(+x: J.Json) -> Nat:
  Bool.pick(Nat, String.eq(J.get_str_or(x, "fx", ""), "confirm"), 1n, 0n)

def fx_confirms(f: HX.Fx) -> Nat:
  match f:
    case HX.FJ{_}:
      0n
    case HX.FO{+x}:
      confirm_n(x)

def confirms(fx: List<&2, HX.Fx>) -> Nat:
  match fx:
    case Nil{}:
      0n
    case Con{+f, +rest}:
      Nat.add(fx_confirms(f), confirms(rest))

# LAW: a view card still open at its due gets one `confirm` (a no)...
law view_card_due_answers:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  {confirms(HX.Core.fx(o, HX.Cx.c(o, HC.card_due(o, Some{c}, cx))))
    == Nat.add(confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), 1n) : Nat}

# ...and the one event journaled is its close as timed out (so its card
# is gone: a second due finds nothing, view_card_due_after_close).
law view_card_due_closes:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.card_due(o, Some{c}, cx))))
    == HM.snoc(HM.Ev, HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))), HM.ECardClosed{HS.Card.id(c), "timed out", HS.Ans.empty()}) : List<&2, HM.Ev>}

# the view cards of `agent` in `cs`
def view_cards(cs: List<&2, HS.Card>, +agent: String) -> Nat:
  match cs:
    case Nil{}:
      0n
    case Con{+c, +rest}:
      +n = view_cards(rest, agent)
      Bool.pick(Nat, String.eq(HS.Card.agent(c), agent) && HC.is_view(HS.Card.view(c)), 1n+n, n)

# LAW: an agent's stop answers each of its view cards once (one `confirm`,
# a no, per card placed view:<agent>/...) and no other card; the stop
# closes them all (stop_leaves_no_card), so their due finds them gone
# (view_card_due_after_close): no second answer.
law view_card_stop_answers:
  for -o: HS.St
  for +cs: List<&2, HS.Card>
  for +cx: HX.Cx<o>
  for +agent: String
  {confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_nos(o, cs, cx, agent))))
    == Nat.add(confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), view_cards(cs, agent)) : Nat}

# the view cards in `cs`, whoever's
def view_cards_all(cs: List<&2, HS.Card>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case Con{+c, +rest}:
      +n = view_cards_all(rest)
      Bool.pick(Nat, HC.is_view(HS.Card.view(c)), 1n+n, n)

# LAW: at boot, sb-core answers each view card left open once (one
# `confirm`, a no: its call ended with the hub that ran it) and no other
# card (d4: never the shell, never by the place's text)...
law boot_answers_view_cards:
  for -o: HS.St
  for +cs: List<&2, HS.Card>
  for +cx: HX.Cx<o>
  {confirms(HX.Core.fx(o, HX.Cx.c(o, HC.view_ends(o, cs, cx))))
    == Nat.add(confirms(HX.Core.fx(o, HX.Cx.c(o, cx))), view_cards_all(cs)) : Nat}

# ...and closes it (so no due, stop or boot after finds it: one answer,
# view_card_due_after_close)
law boot_closes_view_cards:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +c: HS.Card
  {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.view_ends.one(o, True{}, cx, c))))
    == HM.snoc(HM.Ev, HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))), HM.ECardClosed{HS.Card.id(c), "ended with the hub", HS.Ans.empty()}) : List<&2, HM.Ev>}
