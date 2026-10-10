# hub/codec.bend — the journal events and messages as JSON, in the exact
# serde format of rust/switchboard/src/model.rs (the Rust mirror and the
# journal file read them back).
import Base

import ../vendor/json.bend as J

import ./model.bend as M

import ./timers.bend as TM

import ./xhub-model.bend as XH

def jmaybe_nat(m: Maybe<&2, Nat>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+n}:
      J.JNum{n}

def jmaybe_str(m: Maybe<&2, String>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+s}:
      J.JStr{s}

def life_str(l: M.Life) -> String:
  match l:
    case M.LActive{}:
      "active"
    case M.LStopped{}:
      "stopped"
    case M.LArchived{}:
      "archived"
    case M.LFailed{}:
      "failed"

def decl_str(d: M.Decl) -> String:
  match d:
    case M.DWorking{}:
      "working"
    case M.DDone{}:
      "done"
    case M.DBlocked{}:
      "blocked"

def jmaybe_decl(m: Maybe<&2, M.Decl>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+d}:
      J.JStr{decl_str(d)}

def via_kv(m: Maybe<&2, String>) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match m:
    case None{}:
      Nil{}
    case Some{+v}:
      [J.kv("via", J.JStr{v})]

def msg_json(+m: M.Msg) -> J.Json:
  J.obj(List.append(&2, Sigma<&2, &2, String, _ => J.Json>, [J.kv("id", J.JNum{M.Msg.id(m)}), J.kv("thread", J.JNum{M.Msg.thread(m)}),
    J.kv("from", J.JStr{M.Msg.from(m)}), J.kv("to", J.JStr{M.Msg.to(m)}),
    J.kv("reply_to", jmaybe_nat(M.Msg.reply_to(m))),
    J.kv("expect_reply", J.JBool{M.Msg.expect(m)}), J.kv("auto", J.JBool{M.Msg.auto(m)}),
    J.kv("text", J.JStr{M.Msg.text(m)}), J.kv("created_ms", J.JNum{M.Msg.created(m)}),
    J.kv("plain", J.JBool{M.Msg.plain(m)}), J.kv("queued", J.JBool{M.Msg.queued(m)})], via_kv(M.Msg.via(m))))

def card_json(+c: M.Card) -> J.Json:
  J.obj([J.kv("id", J.JNum{M.Card.id(c)}), J.kv("kind", J.JStr{M.Card.kind(c)}),
    J.kv("agent", J.JStr{M.Card.agent(c)}), J.kv("text", J.JStr{M.Card.text(c)}),
    J.kv("for_msg", jmaybe_nat(M.Card.for_msg(c))), J.kv("created_ms", J.JNum{M.Card.created(c)}),
    J.kv("place", jmaybe_str(M.Card.place(c))), J.kv("pr", jmaybe_nat(M.Card.pr(c)))])

def mst_kvs(s: M.MSt) -> List<&2, Sigma<&2, &2, String, _ => J.Json>>:
  match s:
    case M.Queued{+r}:
      [J.kv("state", J.JStr{"queued"}), J.kv("reason", J.JStr{r})]
    case M.Delivered{}:
      [J.kv("state", J.JStr{"delivered"})]
    case M.Rejected{+e}:
      [J.kv("state", J.JStr{"rejected"}), J.kv("error", J.JStr{e})]
    case M.Cancelled{}:
      [J.kv("state", J.JStr{"cancelled"})]

def ty(+t: String) -> Sigma<&2, &2, String, _ => J.Json>:
  J.kv("type", J.JStr{t})

def op_type(op: XH.OutOp) -> String:
  match op:
    case XH.OpAcked{}:
      "x_acked"
    case XH.OpTried{}:
      "x_failed"
    case XH.OpSettled{}:
      "x_settled"

def route_op_json(+rid: Nat, op: XH.RouteOp) -> J.Json:
  match op:
    case XH.RTo{+to, +name}:
      J.obj([ty("route_set"), J.kv("rid", J.JNum{rid}), J.kv("to", J.JStr{to}), J.kv("name", J.JStr{name})])
    case XH.REnd{+s}:
      J.obj([ty(String.append("route_", XH.rst_str(s))), J.kv("rid", J.JNum{rid})])

def x_json(e: XH.XEv) -> J.Json:
  match e:
    case XH.XOut{+o}:
      J.obj([ty("x_out"), J.kv("out", XH.out_json(o))])
    case XH.XOp{+xid, +op}:
      J.obj([ty(op_type(op)), J.kv("xid", J.JNum{xid})])
    case XH.XGot{+i}:
      J.obj([ty("x_in"), J.kv("in", XH.xin_json(i))])
    case XH.XReplied{+h, +xid}:
      J.obj([ty("x_replied"), J.kv("hub", J.JStr{h}), J.kv("xid", J.JNum{xid})])
    case XH.XRouteHeld{+r}:
      J.obj([ty("route_held"), J.kv("route", XH.route_json(r))])
    case XH.XRoute{+rid, +op}:
      route_op_json(rid, op)

def act_ev_json(e: TM.ActEv) -> J.Json:
  match e:
    case TM.ActSet{+a}:
      J.obj([ty("act_set"), J.kv("act", TM.act_json(a))])
    case TM.ActDone{+id}:
      J.obj([ty("act_done"), J.kv("id", J.JNum{id})])

def ev_json(e: M.Ev) -> J.Json:
  match e:
    case M.ETaskCreated{+name, +parent, +brief, +ws, +at}:
      J.obj([ty("task_created"), J.kv("name", J.JStr{name}), J.kv("parent", J.JStr{parent}),
        J.kv("brief", brief), J.kv("ws", ws), J.kv("at_ms", J.JNum{at})])
    case M.ELifecycle{+name, +life, +reason}:
      J.obj([ty("lifecycle"), J.kv("name", J.JStr{name}), J.kv("lifecycle", J.JStr{life_str(life)}),
        J.kv("reason", jmaybe_str(reason))])
    case M.EWorkspace{+name, +ws}:
      J.obj([ty("workspace_changed"), J.kv("name", J.JStr{name}), J.kv("ws", ws)])
    case M.ESnapshot{+name, +snap}:
      J.obj([ty("snapshot"), J.kv("name", J.JStr{name}), J.kv("snapshot_ref", jmaybe_str(snap))])
    case M.EDeclared{+name, +status, +note}:
      J.obj([ty("declared"), J.kv("name", J.JStr{name}), J.kv("status", jmaybe_decl(status)),
        J.kv("note", J.JStr{note})])
    case M.EReported{+name, +report}:
      J.obj([ty("reported"), J.kv("name", J.JStr{name}), J.kv("report", report)])
    case M.EFileTouched{+name, +path}:
      J.obj([ty("file_touched"), J.kv("name", J.JStr{name}), J.kv("path", J.JStr{path})])
    case M.ERenamed{+name, +nn}:
      J.obj([ty("renamed"), J.kv("name", J.JStr{name}), J.kv("new_name", J.JStr{nn})])
    case M.EMsgSent{+m}:
      J.obj([ty("message_sent"), J.kv("msg", msg_json(m))])
    case M.EMsgState{+id, +s}:
      J.obj(ty("message_state") <> J.kv("id", J.JNum{id}) <> mst_kvs(s))
    case M.ESettled{+id}:
      J.obj([ty("message_settled"), J.kv("id", J.JNum{id})])
    case M.ECardOpened{+c}:
      J.obj([ty("card_opened"), J.kv("card", card_json(c))])
    case M.ECardClosed{+id, +res}:
      J.obj([ty("card_closed"), J.kv("id", J.JNum{id}), J.kv("resolution", J.JStr{res})])
    case M.EMainNote{+text, +at}:
      J.obj([ty("main_note"), J.kv("text", J.JStr{text}), J.kv("at_ms", J.JNum{at})])
    case M.ENotesFlushed{}:
      J.obj([ty("main_notes_flushed")])
    # the timers' lines, in the shape every.rs wrote them (old hubs read them)
    case M.ETimerSet{+t, +at}:
      J.obj(List.append(&2, Sigma<&2, &2, String, _ => J.Json>,
        [ty("every_set"), J.kv("id", J.JNum{TM.Timer.id(t)}), J.kv("agent", J.JStr{TM.Timer.agent(t)}),
          J.kv("by", J.JStr{TM.Timer.by(t)}), J.kv("text", J.JStr{TM.Timer.text(t)}),
          J.kv("next_ms", J.JNum{TM.Timer.next(t)}), J.kv("at", J.JNum{at}), TM.sched_kv(TM.Timer.sched(t))],
        TM.opt_kvs(t)))
    case M.ETimerFired{+id, +fired, +next, +at, +undelivered}:
      J.obj(List.append(&2, Sigma<&2, &2, String, _ => J.Json>,
        [ty("every_fired"), J.kv("id", J.JNum{id}), J.kv("fired", J.JNum{fired}), J.kv("next_ms", J.JNum{next}),
          J.kv("at", J.JNum{at})],
        Bool.pick(List<&2, Sigma<&2, &2, String, _ => J.Json>>, undelivered, [J.kv("undelivered", J.JBool{True{}})], Nil{})))
    case M.ETimerRun{+id, +at}:
      J.obj([ty("every_run"), J.kv("id", J.JNum{id}), J.kv("at", J.JNum{at})])
    case M.ETimerStop{+id, +why, +at}:
      J.obj([ty("every_stop"), J.kv("id", J.JNum{id}), J.kv("why", J.JStr{why}), J.kv("at", J.JNum{at})])
    # an older sb-core reads every_name as unknown: skipped
    case M.ETimerName{+id, +name, +at}:
      J.obj([ty("every_name"), J.kv("id", J.JNum{id}), J.kv("name", J.JStr{name}), J.kv("at", J.JNum{at})])
    # bise desktop S2 (an older sb-core reads these as unknown: skipped)
    case M.EX{+xe}:
      x_json(xe)
    case M.EAct{+ae}:
      act_ev_json(ae)
    # bise desktop S10 (an older sb-core reads these as unknown: skipped)
    case M.EFollow{+name, +on, +hub}:
      J.obj([ty("follow"), J.kv("name", J.JStr{name}), J.kv("on", J.JBool{on}), J.kv("hub", J.JStr{hub})])
    case M.EJobEnd{+name, +state}:
      J.obj([ty("job_end"), J.kv("name", J.JStr{name}), J.kv("state", J.JStr{state})])

# ---- decoding (the journal replayed at boot) ----

def get_nat(+j: J.Json, +k: String) -> Nat:
  J.get_num_or(j, k, 0n)

def get_s(+j: J.Json, +k: String) -> String:
  J.get_str_or(j, k, "")

def get_b(+j: J.Json, +k: String) -> Bool:
  J.get_bool_or(j, k, False{})

def get_j(+j: J.Json, +k: String) -> J.Json:
  Maybe.default(&2, J.Json, J.get(j, k), J.JNull{})

def opt_nat(m: Maybe<&2, J.Json>) -> Maybe<&2, Nat>:
  match m:
    case Some{J.JNum{+n}}:
      Some{n}
    case _:
      None{}

def opt_str(m: Maybe<&2, J.Json>) -> Maybe<&2, String>:
  match m:
    case Some{J.JStr{+s}}:
      Some{s}
    case _:
      None{}

def life_of(+s: String) -> M.Life:
  Bool.pick(M.Life, String.eq(s, "stopped"), M.LStopped{},
    Bool.pick(M.Life, String.eq(s, "archived"), M.LArchived{},
      Bool.pick(M.Life, String.eq(s, "failed"), M.LFailed{}, M.LActive{})))

def decl_of(m: Maybe<&2, String>) -> Maybe<&2, M.Decl>:
  match m:
    case None{}:
      None{}
    case Some{+s}:
      Some{Bool.pick(M.Decl, String.eq(s, "done"), M.DDone{},
        Bool.pick(M.Decl, String.eq(s, "blocked"), M.DBlocked{}, M.DWorking{}))}

def msg_of(+j: J.Json) -> M.Msg:
  M.Msg{get_nat(j, "id"), get_nat(j, "thread"), get_s(j, "from"), get_s(j, "to"),
    opt_nat(J.get(j, "reply_to")), get_b(j, "expect_reply"), get_b(j, "auto"),
    get_s(j, "text"), get_nat(j, "created_ms"), get_b(j, "plain"), get_b(j, "queued"), opt_str(J.get(j, "via"))}

def card_of(+j: J.Json) -> M.Card:
  M.Card{get_nat(j, "id"), get_s(j, "kind"), get_s(j, "agent"), get_s(j, "text"),
    opt_nat(J.get(j, "for_msg")), get_nat(j, "created_ms"), opt_str(J.get(j, "place")),
    opt_nat(J.get(j, "pr"))}

def mst_of(+j: J.Json) -> M.MSt:
  +s = get_s(j, "state")
  Bool.pick(M.MSt, String.eq(s, "delivered"), M.Delivered{},
    Bool.pick(M.MSt, String.eq(s, "rejected"), M.Rejected{get_s(j, "error")},
      Bool.pick(M.MSt, String.eq(s, "cancelled"), M.Cancelled{}, M.Queued{get_s(j, "reason")})))

type EvK is Data:
  KTaskCreated{}
  KLifecycle{}
  KWorkspace{}
  KSnapshot{}
  KDeclared{}
  KReported{}
  KFileTouched{}
  KRenamed{}
  KMsgSent{}
  KMsgState{}
  KSettled{}
  KCardOpened{}
  KCardClosed{}
  KMainNote{}
  KNotesFlushed{}
  KEverySet{}
  KEveryFired{}
  KEveryRun{}
  KEveryStop{}
  KEveryName{}
  KXOut{}
  KXAcked{}
  KXFailed{}
  KXSettled{}
  KXIn{}
  KXReplied{}
  KActSet{}
  KActDone{}
  KRouteHeld{}
  KRouteSet{}
  KRouteSent{}
  KRouteCancelled{}
  KFollow{}
  KJobEnd{}
  KUnknown{}

def ev_kind.x(+t: String) -> EvK:
  Bool.pick(EvK, String.eq(t, "x_out"), KXOut{},
  Bool.pick(EvK, String.eq(t, "x_acked"), KXAcked{},
  Bool.pick(EvK, String.eq(t, "x_failed"), KXFailed{},
  Bool.pick(EvK, String.eq(t, "x_settled"), KXSettled{},
  Bool.pick(EvK, String.eq(t, "x_in"), KXIn{},
  Bool.pick(EvK, String.eq(t, "x_replied"), KXReplied{},
  Bool.pick(EvK, String.eq(t, "act_set"), KActSet{},
  Bool.pick(EvK, String.eq(t, "act_done"), KActDone{},
  Bool.pick(EvK, String.eq(t, "route_held"), KRouteHeld{},
  Bool.pick(EvK, String.eq(t, "route_set"), KRouteSet{},
  Bool.pick(EvK, String.eq(t, "route_sent"), KRouteSent{},
  Bool.pick(EvK, String.eq(t, "route_cancelled"), KRouteCancelled{},
  Bool.pick(EvK, String.eq(t, "follow"), KFollow{},
  Bool.pick(EvK, String.eq(t, "job_end"), KJobEnd{},
  KUnknown{}))))))))))))))

def ev_kind(+t: String) -> EvK:
  Bool.pick(EvK, String.eq(t, "task_created"), KTaskCreated{},
  Bool.pick(EvK, String.eq(t, "lifecycle"), KLifecycle{},
  Bool.pick(EvK, String.eq(t, "workspace_changed"), KWorkspace{},
  Bool.pick(EvK, String.eq(t, "snapshot"), KSnapshot{},
  Bool.pick(EvK, String.eq(t, "declared"), KDeclared{},
  Bool.pick(EvK, String.eq(t, "reported"), KReported{},
  Bool.pick(EvK, String.eq(t, "file_touched"), KFileTouched{},
  Bool.pick(EvK, String.eq(t, "renamed"), KRenamed{},
  Bool.pick(EvK, String.eq(t, "message_sent"), KMsgSent{},
  Bool.pick(EvK, String.eq(t, "message_state"), KMsgState{},
  Bool.pick(EvK, String.eq(t, "message_settled"), KSettled{},
  Bool.pick(EvK, String.eq(t, "card_opened"), KCardOpened{},
  Bool.pick(EvK, String.eq(t, "card_closed"), KCardClosed{},
  Bool.pick(EvK, String.eq(t, "main_note"), KMainNote{},
  Bool.pick(EvK, String.eq(t, "main_notes_flushed"), KNotesFlushed{},
  Bool.pick(EvK, String.eq(t, "every_set"), KEverySet{},
  Bool.pick(EvK, String.eq(t, "every_fired"), KEveryFired{},
  Bool.pick(EvK, String.eq(t, "every_run"), KEveryRun{},
  Bool.pick(EvK, String.eq(t, "every_stop"), KEveryStop{},
  Bool.pick(EvK, String.eq(t, "every_name"), KEveryName{},
  ev_kind.x(t)))))))))))))))))))))

# bise desktop S2's kinds (an older sb-core has none: KUnknown, skipped)
def out_st_of(+s: String) -> XH.OutSt:
  Bool.pick(XH.OutSt, String.eq(s, "settled"), XH.OSettled{}, Bool.pick(XH.OutSt, String.eq(s, "acked"), XH.OAcked{}, XH.OPending{}))

def out_of(+j: J.Json) -> XH.Out:
  XH.Out{get_nat(j, "xid"), get_s(j, "project"), XH.kind_of(get_s(j, "kind")), get_nat(j, "msg"), get_s(j, "text"),
    out_st_of(get_s(j, "state")), get_nat(j, "tries"), get_s(j, "context")}

def xin_of(+j: J.Json) -> XH.XIn:
  XH.XIn{get_s(j, "hub"), get_nat(j, "xid"), get_nat(j, "msg"), XH.kind_of(get_s(j, "kind")), get_b(j, "replied")}

def route_of(+j: J.Json) -> XH.Route:
  XH.Route{get_nat(j, "rid"), get_s(j, "text"), get_s(j, "via"), get_s(j, "to"), get_s(j, "name"), get_nat(j, "due_ms"),
    get_nat(j, "act"), get_s(j, "why"), XH.rst_of(get_s(j, "state")), get_s(j, "context")}

def what_of(+j: J.Json) -> TM.What:
  Bool.pick(TM.What, String.eq(get_s(j, "kind"), "retry"), TM.Retry{get_nat(j, "xid")}, TM.RouteSend{get_nat(j, "rid")})

def act_of(+j: J.Json) -> TM.Act:
  TM.Act{get_nat(j, "id"), get_nat(j, "due"), what_of(get_j(j, "what"))}

# an every_set's schedule, as every.rs read it: every_ms (at least 1 s),
# else daily_min (mod a day), else not a timer (the line is skipped)
def sched_of(e: Maybe<&2, Nat>, d: Maybe<&2, Nat>) -> Maybe<&2, TM.Sched>:
  match e:
    case Some{+p}:
      Some{TM.Every{Nat.max(p, 1000n)}}
    case None{}:
      Maybe.map(&2, Nat, TM.Sched, m => TM.Daily{Nat.mod(m, 1440n)}, d)

def timer_set_of(s: Maybe<&2, TM.Sched>, +j: J.Json) -> Maybe<&2, M.Ev>:
  match s:
    case None{}:
      None{}
    case Some{+sc}:
      Some{M.ETimerSet{TM.Timer{get_nat(j, "id"), get_s(j, "agent"), get_s(j, "by"), get_s(j, "text"), sc,
        get_nat(j, "next_ms"), opt_nat(J.get(j, "until_ms")), opt_nat(J.get(j, "times")), 0n,
        opt_str(J.get(j, "page")), 0n, Nil{}, get_s(j, "name")}, get_nat(j, "at")}}

# a match on the kind, not a Bool.pick chain: the chain decoded the event
# as every kind, then kept one
def ev_of.kind(k: EvK, +j: J.Json, +n: String) -> Maybe<&2, M.Ev>:
  match k:
    case KTaskCreated{}:
      Some{M.ETaskCreated{n, get_s(j, "parent"), get_j(j, "brief"), get_j(j, "ws"), get_nat(j, "at_ms")}}
    case KLifecycle{}:
      Some{M.ELifecycle{n, life_of(get_s(j, "lifecycle")), opt_str(J.get(j, "reason"))}}
    case KWorkspace{}:
      Some{M.EWorkspace{n, get_j(j, "ws")}}
    case KSnapshot{}:
      Some{M.ESnapshot{n, opt_str(J.get(j, "snapshot_ref"))}}
    case KDeclared{}:
      Some{M.EDeclared{n, decl_of(opt_str(J.get(j, "status"))), get_s(j, "note")}}
    case KReported{}:
      Some{M.EReported{n, get_j(j, "report")}}
    case KFileTouched{}:
      Some{M.EFileTouched{n, get_s(j, "path")}}
    case KRenamed{}:
      Some{M.ERenamed{n, get_s(j, "new_name")}}
    case KMsgSent{}:
      Some{M.EMsgSent{msg_of(get_j(j, "msg"))}}
    case KMsgState{}:
      Some{M.EMsgState{get_nat(j, "id"), mst_of(j)}}
    case KSettled{}:
      Some{M.ESettled{get_nat(j, "id")}}
    case KCardOpened{}:
      Some{M.ECardOpened{card_of(get_j(j, "card"))}}
    case KCardClosed{}:
      Some{M.ECardClosed{get_nat(j, "id"), get_s(j, "resolution")}}
    case KMainNote{}:
      Some{M.EMainNote{get_s(j, "text"), get_nat(j, "at_ms")}}
    case KNotesFlushed{}:
      Some{M.ENotesFlushed{}}
    case KEverySet{}:
      timer_set_of(sched_of(opt_nat(J.get(j, "every_ms")), opt_nat(J.get(j, "daily_min"))), j)
    case KEveryFired{}:
      Some{M.ETimerFired{get_nat(j, "id"), get_nat(j, "fired"), get_nat(j, "next_ms"), get_nat(j, "at"), get_b(j, "undelivered")}}
    case KEveryRun{}:
      Some{M.ETimerRun{get_nat(j, "id"), get_nat(j, "at")}}
    case KEveryStop{}:
      Some{M.ETimerStop{get_nat(j, "id"), get_s(j, "why"), get_nat(j, "at")}}
    case KEveryName{}:
      Some{M.ETimerName{get_nat(j, "id"), get_s(j, "name"), get_nat(j, "at")}}
    case KXOut{}:
      Some{M.EX{XH.XOut{out_of(get_j(j, "out"))}}}
    case KXAcked{}:
      Some{M.EX{XH.XOp{get_nat(j, "xid"), XH.OpAcked{}}}}
    case KXFailed{}:
      Some{M.EX{XH.XOp{get_nat(j, "xid"), XH.OpTried{}}}}
    case KXSettled{}:
      Some{M.EX{XH.XOp{get_nat(j, "xid"), XH.OpSettled{}}}}
    case KXIn{}:
      Some{M.EX{XH.XGot{xin_of(get_j(j, "in"))}}}
    case KXReplied{}:
      Some{M.EX{XH.XReplied{get_s(j, "hub"), get_nat(j, "xid")}}}
    case KActSet{}:
      Some{M.EAct{TM.ActSet{act_of(get_j(j, "act"))}}}
    case KActDone{}:
      Some{M.EAct{TM.ActDone{get_nat(j, "id")}}}
    case KFollow{}:
      # J: an older journal has no hub ("": this hub's own follow)
      Some{M.EFollow{get_s(j, "name"), get_b(j, "on"), get_s(j, "hub")}}
    case KJobEnd{}:
      Some{M.EJobEnd{get_s(j, "name"), get_s(j, "state")}}
    case KRouteHeld{}:
      Some{M.EX{XH.XRouteHeld{route_of(get_j(j, "route"))}}}
    case KRouteSet{}:
      Some{M.EX{XH.XRoute{get_nat(j, "rid"), XH.RTo{get_s(j, "to"), get_s(j, "name")}}}}
    case KRouteSent{}:
      Some{M.EX{XH.XRoute{get_nat(j, "rid"), XH.REnd{XH.RSent{}}}}}
    case KRouteCancelled{}:
      Some{M.EX{XH.XRoute{get_nat(j, "rid"), XH.REnd{XH.RCancelled{}}}}}
    case KUnknown{}:
      None{}

def ev_of(+j: J.Json) -> Maybe<&2, M.Ev>:
  ev_of.kind(ev_kind(get_s(j, "type")), j, get_s(j, "name"))
