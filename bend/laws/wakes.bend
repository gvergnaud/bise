# bend/laws/wakes.bend: a watch of `sb wake` is a pause or an alarm
# (hub/wakes.bend `resumes`), and a reply still pending after an ask's
# hold is a pause until it comes (ask-wait, laws ask_*). Its other laws (once, gone, drop, max,
# replay) are in bend/laws/hub.bend. Closed by bend/proofs/wakes.bend; the
# gate checks every bend/proofs/*.bend: a law here with no proof there
# fails it.
import Base
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/wakes.bend as HWK
import ../hub/reach.bend as HR
import ../hub/work.bend as HWO

# ---- a pause or an alarm (designer's page agent-waiting-states, pick A) ----
# A watch's `resumes` says whether its agent continues its work at the end
# (a pause, `sb wake --then`) or is free and only woken if it fires (an
# alarm, `--alarm`). Neither said: what the agent started itself (a
# backgrounded bash command, a pid) is a pause; a file, a launchd job or a
# kind this version doesn't know is an alarm. wake_replay_keeps above
# keeps it across a restart (a line from before the field gets its kind's
# default: wakes.bend `resumes.of`).

# LAW: a backgrounded bash command's watch is a pause by default.
law wake_bg_pauses:
  for +s: J.Json
  for e: {HWK.kind_of(s) == HWK.KBg{} : HWK.WatchKind}
  {HWK.resumes_of(s, None{}) == True{} : Bool}

# LAW: a pid's watch (`sb wake --on-exit`) is a pause by default.
law wake_pid_pauses:
  for +s: J.Json
  for e: {HWK.kind_of(s) == HWK.KPid{} : HWK.WatchKind}
  {HWK.resumes_of(s, None{}) == True{} : Bool}

# LAW: a file's watch (`--on-file`) is an alarm by default.
law wake_file_alarms:
  for +s: J.Json
  for e: {HWK.kind_of(s) == HWK.KFile{} : HWK.WatchKind}
  {HWK.resumes_of(s, None{}) == False{} : Bool}

# LAW: a launchd job's watch (`--on-job`) is an alarm by default.
law wake_job_alarms:
  for +s: J.Json
  for e: {HWK.kind_of(s) == HWK.KJob{} : HWK.WatchKind}
  {HWK.resumes_of(s, None{}) == False{} : Bool}

# LAW: a kind this version doesn't know is an alarm by default.
law wake_unknown_alarms:
  for +s: J.Json
  for e: {HWK.kind_of(s) == HWK.KOther{} : HWK.WatchKind}
  {HWK.resumes_of(s, None{}) == False{} : Bool}

# LAW: the agent's word wins over the default (`--then`, `--alarm`).
law wake_then_wins:
  for +s: J.Json
  for +b: Bool
  {HWK.resumes_of(s, Some{b}) == b : Bool}

# LAW: setting a watch, pause or alarm, never changes an agent's run (its
# status: an agent with only alarms stays idle) nor its record.
law wake_set_keeps_run:
  for -o: HS.St
  for ok: Bool
  for +cx: HX.Cx<o>
  for +a: String
  for +agent: String
  for +spec: J.Json
  for +max_text: String
  for +max_ms: Nat
  for +quiet: Bool
  for +then: Maybe<&2, Bool>
  {HC.rt(o, HC.wake_set.agent(o, ok, cx, agent, spec, max_text, max_ms, quiet, then), a) == HC.rt(o, cx, a) : HX.Rt}

law wake_set_keeps_agents:
  for -o: HS.St
  for ok: Bool
  for +cx: HX.Cx<o>
  for +agent: String
  for +spec: J.Json
  for +max_text: String
  for +max_ms: Nat
  for +quiet: Bool
  for +then: Maybe<&2, Bool>
  {HS.St.agents(HC.st(o, HC.wake_set.agent(o, ok, cx, agent, spec, max_text, max_ms, quiet, then))) == HS.St.agents(HC.st(o, cx)) : List<&2, HS.Agent>}

# ---- his stop (the window's wake_stop) ----
# His stop of a watch tells its agent once (`tell`: the words, Rust's
# bise_proto words `watch_stopped`): a pause it meant to continue at, an
# alarm it counted on. The agent's own `sb wake --stop` tells it nothing.

# LAW: the agent's own stop (no words) tells it nothing.
law wake_own_stop_quiet:
  for +a: Bool
  {HWK.stop_tells("", a) == False{} : Bool}

# LAW: his stop (its words) tells an active agent.
law wake_his_stop_tells:
  for +h: Char
  for +t: String
  {HWK.stop_tells(SCon{h, t}, True{}) == True{} : Bool}

# LAW: a stop of a watch that ended (or never was) changes nothing: no
# line, no message.
law wake_stop_gone_quiet:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +why: String
  for +tell: String
  {HC.wake_stop.found(o, None{}, cx, why, tell) == cx : HX.Cx<o>}


# ---- a reply still pending (ask-wait) ----
# `sb ask` / `sb wait` hold the asker WAIT_CAP seconds at most; a hold that
# ends with no reply leaves a `reply` watch (hub/wakes.bend make_reply): a
# pause counted from the ask, until the reply (no wake: the reply is its
# own), the asker moves on (done, failed, its stop, its drop), the asked
# agent is dropped or a day passes (both tell the asker once).

# LAW: a reply watch is a pause by default.
law wake_reply_pauses:
  for +s: J.Json
  for e: {HWK.kind_of(s) == HWK.KReply{} : HWK.WatchKind}
  {HWK.resumes_of(s, None{}) == True{} : Bool}

# LAW: the reply watch counts from the ask, not from the hold's end.
law ask_timeout_pauses:
  for +ws: HS.Wakes
  for +agent: String
  for +to: String
  for +msg: Nat
  for +asked_at: Nat
  for +now: Nat
  for +works: List<&2, String>
  {HS.Watch.set_at(HWK.make_reply(ws, agent, to, msg, asked_at, now, works)) == asked_at : Nat}

# LAW: the reply watch is a pause: its agent planned to continue on it.
law ask_pause_resumes:
  for +ws: HS.Wakes
  for +agent: String
  for +to: String
  for +msg: Nat
  for +asked_at: Nat
  for +now: Nat
  for +works: List<&2, String>
  {HS.Watch.resumes(HWK.make_reply(ws, agent, to, msg, asked_at, now, works)) == True{} : Bool}

# LAW: a day with no reply tells its active agent once (it is no bash
# slot: not quiet; wake_max_once: ended, never due again).
law ask_max_tells:
  for +ws: HS.Wakes
  for +agent: String
  for +to: String
  for +msg: Nat
  for +asked_at: Nat
  for +now: Nat
  for +works: List<&2, String>
  {HWK.max_says(HWK.make_reply(ws, agent, to, msg, asked_at, now, works), True{}) == True{} : Bool}

# LAW: a second hold that ends on the same message sets no second watch.
law ask_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  for +orig: HS.Msg
  for +w: HS.Watch
  for e: {HWK.reply_for(HS.Wakes.live(HC.wks(o, cx)), agent, HS.Msg.id(orig)) == Some{w} : Maybe<&2, HS.Watch>}
  {HC.reply_pause.orig(o, Some{orig}, cx, agent) == cx : HX.Cx<o>}

# LAW: a reply ends exactly the reply watches it answers, in its step...
law ask_reply_ends:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +from: String
  for +to: String
  for +r: Nat
  {HS.Wakes.live(HC.wks(o, HC.reply_answered(o, cx, from, to, Some{r})))
    == HWK.keep_out(HS.Wakes.live(HC.wks(o, cx)),
      HWK.answered_of(HS.Wakes.live(HC.wks(o, cx)), to, r, HM.names_of(HC.st(o, cx), from), String.eq(from, HM.USER()))) : List<&2, HS.Watch>}

# LAW: ...with no wake: no message from bise (the reply is its own).
law ask_reply_ends_quiet:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +from: String
  for +to: String
  for +r: Maybe<&2, Nat>
  {HM.every(HX.d(o, HC.reply_answered(o, cx, from, to, r))) == HM.every(HX.d(o, cx)) : List<&2, HS.MRec>}

# LAW: the asker moved on (done, failed): its reply watches end, all of
# them...
law ask_done_ends:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  {HS.Wakes.live(HC.wks(o, HC.replies_end(o, cx, agent)))
    == HWK.keep_out(HS.Wakes.live(HC.wks(o, cx)), HWK.replies_of(HS.Wakes.live(HC.wks(o, cx)), HM.names_of(HC.st(o, cx), agent))) : List<&2, HS.Watch>}

# LAW: ...quietly: it said it is done, no message.
law ask_done_quiet:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +agent: String
  {HM.every(HX.d(o, HC.replies_end(o, cx, agent))) == HM.every(HX.d(o, cx)) : List<&2, HS.MRec>}

# LAW: the asked agent gone, a reply watch ends and its active agent hears
# it once, in the same step (ended, it is never an orphan again).
law ask_target_gone_tells:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +w: HS.Watch
  for e: {HR.agent_active(HC.st(o, HC.wake_end(o, cx, HS.Watch.id(w), "gone")), HS.Watch.agent(w)) == True{} : Bool}
  {HC.replies_gone.each(o, [w], cx)
    == HC.wake_say(o, HC.wake_end(o, cx, HS.Watch.id(w), "gone"), HS.Watch.agent(w), HWK.reply_gone_text(w), None{}) : HX.Cx<o>}

# ---- tasks: a watch's works (architect m_64926) ----

# LAW: a watch carries the works it was set with (wake_set and the ask's
# reply watch: its setter's turn's): its hit and its max go out with them,
# so its wake serves that task's turn (delivered_sets_turn_works) and a
# paused task holds it (WO.unheld: paused_work_holds_its_deliveries).
law watch_carries_turn_works:
  for +ws: HS.Wakes
  for +agent: String
  for +spec: J.Json
  for +max_text: String
  for +now: Nat
  for +max_ms: Nat
  for +quiet: Bool
  for +then: Maybe<&2, Bool>
  for +works: List<&2, String>
  {HS.Watch.works(HWK.make(ws, agent, spec, max_text, now, max_ms, quiet, then, works)) == works : List<&2, String>}

# LAW: a watch whose task is done still wakes its agent: only a pause
# holds, and a done task has none (work.bend a_closed_work_holds_nothing),
# so its wake is never held; that is what an agent expects of its own watch.
law a_done_works_watch_is_never_held:
  for +d: HS.Work
  for +t: List<&2, HS.Work>
  for e: {HS.Work.paused(d) == None{} : Maybe<&2, Nat>}
  {HWO.held.by(False{}, [HS.Work.id(d)], d <> t) == False{} : Bool}
