# core/session.bend — the Harness Core: the Step Protocol state machine.
#
# apply(session, command) -> Rejected{reason} | Accepted{obs, acts, next}
#
# - a Rejected result carries no session: a rejected command cannot
#   mutate state or consume an input id (true by type)
# - every Accepted result increments inputs by exactly one
# - the turn, steering, notifications, tool-call correlation, the null
#   iteration limit (ADR 0015), and ADR 0011 preflight compaction all
#   live here. The Core performs no IO.
import Base
import ./types.bend as T
import ./estimate.bend as E
import ./compaction.bend as C
import ./wire.bend as W
import ./history.bend as H

# ---- dispatch result (internal) ----
type DRes is Data:
  DR{obs: List<&2, T.Obs>, acts: List<&2, T.Act>,
     pending: Maybe<&2, T.Pending>, turn: Maybe<&2, T.Turn>,
     notifs: List<&2, String>, history: List<&2, T.Msg>}

# ---- small pure helpers ----

def inject_users(texts: List<&2, String>) -> List<&2, T.Msg>:
  match texts:
    case Nil{}:
      Nil{}
    case Con{h, rest}:
      Con{T.injected_user(h), inject_users(rest)}

def steer_obs(texts: List<&2, String>) -> List<&2, T.Obs>:
  match texts:
    case Nil{}:
      Nil{}
    case Con{h, rest}:
      Con{T.Steered{h}, steer_obs(rest)}

def notif_obs(texts: List<&2, String>) -> List<&2, T.Obs>:
  match texts:
    case Nil{}:
      Nil{}
    case Con{h, rest}:
      Con{T.NotifDelivered{h}, notif_obs(rest)}

# the role of the first message of a projected input
def head_role(input: List<&2, T.Msg>) -> Maybe<&2, T.Role>:
  match input:
    case Nil{}:
      None{}
    case Con{m, _}:
      match m:
        case T.Msg{role, _, _, _}:
          Some{role}

# the projected model input: system message + canonical history, every
# call answered before the next message (H.pair_calls, BISE-242: a
# history cut by a restart never reaches the provider unpaired)
def model_input(+cfg: T.Cfg, +history: List<&2, T.Msg>) -> List<&2, T.Msg>:
  match cfg:
    case T.Cfg{_, _, _, system, _}:
      Con{T.sys(system), H.pair_calls(history)}

def calls_count(calls: List<&2, T.Call>) -> Nat:
  match calls:
    case Nil{}:
      0n
    case Con{_, rest}:
      Nat.add(1n, calls_count(rest))

def tool_acts(+calls: List<&2, T.Call>, +base: U32) -> List<&2, T.Act>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{c, rest}:
      Con{T.ToolAct{base, c}, tool_acts(rest, U32.add(base, 1))}

# provider replies number calls per completion (1, 2, ...), so sequential
# turns would repeat ids on the wire; the Core renumbers calls to their
# action ids — ToolAct, PTools and history all agree, and the wire pairs
# each tool result with exactly one call
def renumber(+calls: List<&2, T.Call>, +base: U32) -> List<&2, T.Call>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{+c, rest}:
      match c:
        case T.Call{_, +name, +args}:
          Con{T.Call{base, name, args}, renumber(rest, U32.add(base, 1))}

def tool_start_obs(+calls: List<&2, T.Call>) -> List<&2, T.Obs>:
  match calls:
    case Nil{}:
      Nil{}
    case Con{c, rest}:
      match c:
        case T.Call{cid, _, _}:
          Con{T.ToolStarted{cid}, tool_start_obs(rest)}

def tool_result_msg(+name: String, +out: String, ok: Bool) -> T.Msg:
  match ok:
    case True{}:
      T.tool_res(String.append("tool ", String.append(name,
        String.append(" ok: ", W.escape_nl(out)))))
    case False{}:
      T.tool_res(String.append("tool ", String.append(name,
        String.append(" failed: ", W.escape_nl(out)))))

# one tool call taken out of the waiting list, with the remainder
type Taken is Data:
  Got{c: T.Call, rest: List<&2, T.Call>}

def take_call.put(hit: Bool, +c: T.Call, +rest: List<&2, T.Call>,
    tail: Maybe<&2, Taken>) -> Maybe<&2, Taken>:
  match hit:
    case True{}:
      Some{Got{c, rest}}
    case False{}:
      tail

def take_call(waiting: List<&2, T.Call>, +id: U32) -> Maybe<&2, Taken>:
  match waiting:
    case Nil{}:
      None{}
    case +c <> +rest:
      match c:
        case T.Call{cid, _, _}:
          take_call.put(U32.is_eq(cid, id), c, rest, take_call(rest, id))

# arrival order (vibe_sdk pushes to a Vec): the list is oldest-first,
# so delivery injects the steering in the order it arrived
def add_steer(turn: Maybe<&2, T.Turn>, +text: String) -> Maybe<&2, T.Turn>:
  match turn:
    case None{}:
      None{}
    case Some{t}:
      match t:
        case T.Turn{nulls, steer, notifs}:
          Some{T.Turn{nulls,
            List.append(&2, String, steer, Con{text, Nil{}}), notifs}}

def add_notif(turn: Maybe<&2, T.Turn>, +text: String) -> Maybe<&2, T.Turn>:
  match turn:
    case None{}:
      None{}
    case Some{t}:
      match t:
        case T.Turn{nulls, steer, notifs}:
          Some{T.Turn{nulls, steer,
            List.append(&2, String, notifs, Con{text, Nil{}})}}

# ---- session field updates ----

def set_turn(+s: T.Session, turn: Maybe<&2, T.Turn>) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, _, +pending, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, turn, pending, queued, notifs}

def set_pending(+s: T.Session, p: Maybe<&2, T.Pending>) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, turn, p, queued, notifs}

def bump_inputs(+s: T.Session) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued, +notifs}:
      T.Sess{cfg, history, Nat.add(inputs, 1n), actions, turn, pending, queued, notifs}

def queue_msg(+s: T.Session, +text: String) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, turn, pending,
        List.append(&2, String, queued, Con{text, Nil{}}), notifs}

def steer_session(+s: T.Session, +text: String) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, add_steer(turn, text), pending,
        queued, notifs}

def notif_turn(+s: T.Session, +text: String) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, add_notif(turn, text), pending,
        queued, notifs}

def hold_notif(+s: T.Session, +text: String) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued, +notifs}:
      T.Sess{cfg, history, inputs, actions, turn, pending, queued,
        Con{text, notifs}}

def start_turn(+text: String, +s: T.Session) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, _, _, +queued, +notifs}:
      T.Sess{cfg, List.append(&2, T.Msg, history, Con{T.user(text), Nil{}}),
        Nat.add(inputs, 1n), actions,
        Some{T.Turn{0n, Nil{}, Nil{}}}, None{}, queued, notifs}

# ---- the between-turns gate (/reload) ----
# Checkpoints are only coherent at turn boundaries: turn and pending
# are empty there (ADR 0002 — they are not serialized). The queue and
# the held notifications ARE serialized (QUEUE/NOTIF lines, v2), so
# they no longer block a reload: the restarted process pops the
# queue into the turn it owes, by itself. A reload may still only run
# with no turn and no pending work.

def no_turn(turn: Maybe<&2, T.Turn>) -> Bool:
  match turn:
    case None{}:
      True{}
    case Some{_}:
      False{}

def no_pending(pending: Maybe<&2, T.Pending>) -> Bool:
  match pending:
    case None{}:
      True{}
    case Some{_}:
      False{}

# between two turns in the strict sense: everything the checkpoint does
# NOT serialize is empty, so a reload cannot lose session state
def reload_allowed(+s: T.Session) -> Bool:
  match s:
    case T.Sess{_, _, _, _, +turn, +pending, _, _}:
      Bool.and(no_turn(turn), no_pending(pending))

# ---- law helpers: what the durable history holds ----

def sess_history(+s: T.Session) -> List<&2, T.Msg>:
  match s:
    case T.Sess{_, +history, _, _, _, _, _, _}:
      history

# the queued texts held for the next turn (queue_msg appends; the
# runtime's idle boundary pops them one turn at a time)
def sess_queued(+s: T.Session) -> List<&2, String>:
  match s:
    case T.Sess{_, _, _, _, _, _, +queued, _}:
      queued

def set_queued(+s: T.Session, +q: List<&2, String>) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, _, +notifs}:
      T.Sess{cfg, history, inputs, actions, turn, pending, q, notifs}

# does the history hold this injected user message?
def history_has_user(msgs: List<&2, T.Msg>, +text: String) -> Bool:
  match msgs:
    case Nil{}:
      False{}
    case Con{+m, +rest}:
      match m:
        case T.Msg{T.RUser{}, +t, _, True{}}:
          Bool.or(String.eq(t, text), history_has_user(rest, text))
        case _:
          history_has_user(rest, text)

# the injected user texts, in history (arrival) order
def injected_user_texts(msgs: List<&2, T.Msg>) -> List<&2, String>:
  match msgs:
    case Nil{}:
      Nil{}
    case Con{+m, +rest}:
      match m:
        case T.Msg{T.RUser{}, +t, _, True{}}:
          Con{t, injected_user_texts(rest)}
        case _:
          injected_user_texts(rest)

# the next session of an apply result (the fallback is only for
# totality; the law flows take the Accepted path)
def res_next(r: T.Res, +fallback: T.Session) -> T.Session:
  match r:
    case T.Accepted{_, _, +next}:
      next
    case T.Rejected{_}:
      fallback

# ---- turn death: commit what the turn was still holding ----
# vibe_sdk finish_turn: steering accepted for the turn is a real user
# message the client already shows. A turn that ends before its next
# model call -- an interruption, a failure, or the iteration cap --
# would otherwise drop it with the turn. Committing it here keeps
# model context consistent with the history the user sees, so the next
# turn's model call carries it. No Steered observation: the model
# never received it.

def finish_history(turn: Maybe<&2, T.Turn>, +history: List<&2, T.Msg>)
    -> List<&2, T.Msg>:
  match turn:
    case None{}:
      history
    case Some{t}:
      match t:
        case T.Turn{_, +steer, +tnotifs}:
          List.append(&2, T.Msg,
            List.append(&2, T.Msg, history, inject_users(steer)),
            inject_users(tnotifs))

def finish_turn(+s: T.Session) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued,
        +notifs}:
      T.Sess{cfg, finish_history(turn, history), inputs, actions,
        None{}, None{}, queued, notifs}

# a null-iteration retry keeps the active turn (vibe_sdk): the pending
# steering and notifications survive into the retried turn
def retry_turn(+s: T.Session, +nulls: Nat) -> Maybe<&2, T.Turn>:
  match s:
    case T.Sess{_, _, _, _, +turn, _, _, _}:
      match turn:
        case None{}:
          Some{T.Turn{nulls, Nil{}, Nil{}}}
        case Some{t}:
          match t:
            case T.Turn{_, +steer, +tnotifs}:
              Some{T.Turn{nulls, steer, tnotifs}}

# ---- compaction start (shared by automatic preflight and manual command) ----

def start_compaction.too_large(auto: Bool, turn: Maybe<&2, T.Turn>,
    +steer: List<&2, String>, +notifs: List<&2, String>,
    +history: List<&2, T.Msg>) -> DRes:
  match auto:
    case True{}:
      # in-turn: the active turn fails; what it was still holding is
      # committed (finish_turn), not dropped with the turn
      DR{Con{T.CompactionFailed{"compaction_request_too_large"},
          Con{T.TurnDone{"failed: compaction_request_too_large"},
            List.append(&2, T.Obs, steer_obs(steer), notif_obs(notifs))}},
        Nil{}, None{}, None{}, Nil{}, finish_history(turn, history)}
    case False{}:
      # manual between-turn compaction returns idle
      DR{Con{T.CompactionFailed{"compaction_request_too_large"},
          List.append(&2, T.Obs, steer_obs(steer), notif_obs(notifs))},
        Nil{}, None{}, turn, Nil{}, history}

def start_compaction.project(+auto: Bool, +extra: String, +actions: U32,
    turn: Maybe<&2, T.Turn>, +steer: List<&2, String>,
    +notifs: List<&2, String>, p: C.PRes,
    +history: List<&2, T.Msg>) -> DRes:
  match p:
    case C.POk{+input}:
      # ADR 0011: the first request uses the compaction id as its action id.
      # The turn rides along: steering it holds is NOT consumed by the
      # compaction (vibe_sdk keeps it pending) -- it is committed at the
      # first agent call after the compaction completes
      DR{Con{T.CompactionStarted{actions, auto},
          List.append(&2, T.Obs, steer_obs(steer), notif_obs(notifs))},
        Con{T.CompactionCall{actions, input}, Nil{}},
        Some{T.PComp{actions, 1n, actions, extra, auto}},
        turn, Nil{}, history}
    case C.PBig{}:
      start_compaction.too_large(auto, turn, steer, notifs, history)

def start_compaction(auto: Bool, +extra: String, +cfg: T.Cfg,
    +history: List<&2, T.Msg>, +actions: U32, turn: Maybe<&2, T.Turn>,
    +steer: List<&2, String>, +notifs: List<&2, String>) -> DRes:
  start_compaction.project(auto, extra, actions, turn, steer, notifs,
    C.project(cfg, history, extra), history)

# ---- dispatch: the seam before every agent completion ----
# Steering and notifications enter model context here (model-safe
# boundary), then ADR 0011 preflight budgets the complete input.

def dispatch.pick(fits: Bool, +cfg: T.Cfg, +history: List<&2, T.Msg>,
    +history2: List<&2, T.Msg>, +actions: U32, +nulls: Nat,
    +steer: List<&2, String>, +notifs: List<&2, String>) -> DRes:
  match fits:
    case True{}:
      # the injected users are COMMITTED into the durable history
      # (vibe_sdk: state.context.extend(steering)) -- they survive
      # into every later model call and the checkpoint
      DR{List.append(&2, T.Obs, steer_obs(steer), notif_obs(notifs)),
        Con{T.AgentCall{actions, model_input(cfg, history2)}, Nil{}},
        Some{T.PAgent{actions}},
        Some{T.Turn{nulls, Nil{}, Nil{}}}, Nil{}, history2}
    case False{}:
      # oversized input starts compaction before the agent Action. The
      # steering is NOT consumed (vibe_sdk keeps it pending): the
      # compaction request projects the un-injected history, and the
      # turn carries the steering for the first agent call after it
      start_compaction(True{}, "", cfg, history, actions,
        Some{T.Turn{nulls, steer, notifs}}, Nil{}, Nil{})

def dispatch.thresh(+threshold: Nat, +cfg: T.Cfg, +history: List<&2, T.Msg>,
    +actions: U32, turn: Maybe<&2, T.Turn>, +sess_notifs: List<&2, String>)
    -> DRes:
  match turn:
    case None{}:
      DR{Nil{}, Nil{}, None{}, None{}, Nil{}, history}
    case Some{t}:
      match t:
        case T.Turn{+nulls, +steer, +tnotifs}:
          +all_notifs = List.append(&2, String, sess_notifs, tnotifs)
          +history2 = List.append(&2, T.Msg,
            List.append(&2, T.Msg, history, inject_users(steer)),
            inject_users(all_notifs))
          dispatch.pick(Nat.is_lt(E.input_tokens(cfg, history2), threshold),
            cfg, history, history2, actions, nulls, steer, all_notifs)

def dispatch.turn(+cfg: T.Cfg, +history: List<&2, T.Msg>, +actions: U32,
    turn: Maybe<&2, T.Turn>, +sess_notifs: List<&2, String>) -> DRes:
  match cfg:
    case T.Cfg{+threshold, _, _, _, _}:
      dispatch.thresh(threshold, cfg, history, actions, turn, sess_notifs)

def dispatch(+s: T.Session) -> DRes:
  match s:
    case T.Sess{+cfg, +history, _, +actions, +turn, _, _, +notifs}:
      dispatch.turn(cfg, history, actions, turn, notifs)

# wrap a dispatch result as an Accepted apply result; the emitted action id
# is consumed, so the next action id moves on by one
def dispatch_result(+d: DRes, +pre: List<&2, T.Obs>, +s: T.Session) -> T.Res:
  match d:
    case DR{obs, acts, pending, turn, notifs, history}:
      match s:
        case T.Sess{+cfg, +hist, +inputs, +actions, _, _, +queued, _}:
          # the DR's history replaces the session's: the dispatch may
          # have committed injected steering and notifications
          T.Accepted{List.append(&2, T.Obs, pre, obs), acts,
            T.Sess{cfg, history, inputs, U32.add(actions, 1), turn, pending,
              queued, notifs}}

# ---- completion result handlers ----

def apply.completion.too_large(auto: Bool, +code: String, +s: T.Session) -> T.Res:
  match auto:
    case True{}:
      # the turn dies: commit what it was still holding
      T.Accepted{Con{T.CompactionFailed{code},
          Con{T.TurnDone{String.append("failed: ", code)}, Nil{}}},
        Nil{}, bump_inputs(finish_turn(s))}
    case False{}:
      T.Accepted{Con{T.CompactionFailed{code}, Nil{}}, Nil{},
        bump_inputs(set_pending(s, None{}))}

def apply.completion.resume(auto: Bool, +s: T.Session, +summary: String) -> T.Res:
  match auto:
    case True{}:
      # the turn continues on the replacement context; the first agent
      # Action after compaction uses the full new input
      d = dispatch(s)
      dispatch_result(d, Con{T.CompactionDone{summary}, Nil{}}, bump_inputs(s))
    case False{}:
      T.Accepted{Con{T.CompactionDone{summary}, Nil{}}, Nil{},
        bump_inputs(set_pending(s, None{}))}

def apply.completion.accepted(r: C.RRes, +auto: Bool, +summary: String,
    +s: T.Session) -> T.Res:
  match r:
    case C.ROk{+h2}:
      match s:
        case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
          apply.completion.resume(auto,
            T.Sess{cfg, h2, inputs, actions, turn, None{}, queued, notifs},
            summary)
    case C.RBig{}:
      apply.completion.too_large(auto, "compaction_replacement_too_large", s)

def apply.completion.malformed.project(p: C.PRes, +action: U32,
    +compaction: U32, +extra: String, +auto: Bool, +s: T.Session) -> T.Res:
  match p:
    case C.POk{+input}:
      match s:
        case T.Sess{+cfg, +history, +inputs, _, +turn, _, +queued, +notifs}:
          # one format retry: new action id derived from the stable
          # compaction id, attempt 2, same fitted projection
          T.Accepted{Con{T.CandidateDiscarded{"invalid_compaction_summary"},
              Nil{}},
            Con{T.CompactionCall{action, input}, Nil{}},
            T.Sess{cfg, history, Nat.add(inputs, 1n), U32.add(action, 1),
              turn, Some{T.PComp{compaction, 2n, action, extra, auto}},
              queued, notifs}}
    case C.PBig{}:
      apply.completion.too_large(auto, "compaction_request_too_large", s)

def apply.completion.malformed(retry: Bool, +compaction: U32, +extra: String,
    +auto: Bool, +s: T.Session) -> T.Res:
  match retry:
    case True{}:
      match s:
        case T.Sess{+cfg, +history, _, +actions, _, _, _, _}:
          apply.completion.malformed.project(
            C.project(cfg, history, extra), U32.add(actions, 1),
            compaction, extra, auto, s)
    case False{}:
      apply.completion.too_large(auto, "invalid_compaction_summary", s)

def apply.completion.reply(v: Maybe<&2, String>, +auto: Bool,
    +compaction: U32, +attempt: Nat, +extra: String, +s: T.Session) -> T.Res:
  match v:
    case Some{+summary}:
      match s:
        case T.Sess{+cfg, +history, _, _, _, _, _, _}:
          apply.completion.accepted(C.replace(cfg, history, summary), auto,
            summary, s)
    case None{}:
      apply.completion.malformed(Nat.is_lt(attempt, 3n), compaction, extra,
        auto, s)

def apply.completion.comp(matched: Bool, +text: String,
    +calls: List<&2, T.Call>, +compaction: U32, +attempt: Nat,
    +extra: String, +auto: Bool, +s: T.Session) -> T.Res:
  match matched:
    case False{}:
      T.Rejected{"stale completion action"}
    case True{}:
      apply.completion.reply(C.valid_summary(text, calls), auto, compaction,
        attempt, extra, s)

def cfg_max_nulls(+cfg: T.Cfg) -> Nat:
  match cfg:
    case T.Cfg{_, _, max_nulls, _, _}:
      max_nulls

def apply.completion.tools(+text: String, +calls: List<&2, T.Call>,
    +s: T.Session) -> T.Res:
  # commit the assistant message, emit one Action per tool call, and wait
  # for every call id before the next dispatch
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
      # the started obs carries the same action ids as the ToolActs (and
      # as the finished obs), so every consumer can pair by id
      T.Accepted{Con{T.Assistant{text},
          tool_start_obs(renumber(calls, U32.add(actions, 1)))},
        tool_acts(renumber(calls, U32.add(actions, 1)), U32.add(actions, 1)),
        T.Sess{cfg, List.append(&2, T.Msg, history,
            Con{T.asst(text, renumber(calls, U32.add(actions, 1))), Nil{}}),
          Nat.add(inputs, 1n),
          U32.add(actions, U32.from_nat(Nat.add(calls_count(calls), 1n))),
          turn, Some{T.PTools{renumber(calls, U32.add(actions, 1))}}, queued, notifs}}

def apply.completion.null2(over: Bool, +nulls: Nat, +s: T.Session) -> T.Res:
  match over:
    case True{}:
      # the iteration cap kills the turn: commit what it still held
      T.Accepted{Con{T.NullIteration{},
          Con{T.TurnDone{"failed: null iteration limit"}, Nil{}}},
        Nil{}, bump_inputs(finish_turn(s))}
    case False{}:
      # retry within the limit: the active turn survives with its
      # pending steering and notifications (vibe_sdk keeps the turn)
      +s1 = bump_inputs(set_turn(s, retry_turn(s, Nat.add(nulls, 1n))))
      d = dispatch(s1)
      dispatch_result(d, Con{T.NullIteration{}, Nil{}}, s1)

def apply.completion.empty(+s: T.Session) -> T.Res:
  match s:
    case T.Sess{+cfg, _, _, _, +turn, _, _, _}:
      match turn:
        case None{}:
          T.Rejected{"no active turn"}
        case Some{t}:
          match t:
            case T.Turn{+nulls, _, _}:
              apply.completion.null2(
                Bool.not(Nat.is_le(Nat.add(nulls, 1n), cfg_max_nulls(cfg))),
                nulls, s)

def apply.completion.plain2.finish(+s2: T.Session, +text: String) -> T.Res:
  # finish_turn committed any steering that arrived while the call was
  # in flight (vibe_sdk finish_turn); the assistant message lands on top
  match s2:
    case T.Sess{+cfg, +hist2, +inputs, +actions, _, _, +queued, +notifs}:
      T.Accepted{Con{T.Assistant{text},
          Con{T.TurnDone{"completed"}, Nil{}}},
        Nil{},
        T.Sess{cfg, List.append(&2, T.Msg, hist2,
          Con{T.asst(text, Nil{}), Nil{}}),
          Nat.add(inputs, 1n), actions, None{}, None{}, queued, notifs}}

def apply.completion.plain2(empty: Bool, +text: String, +s: T.Session) -> T.Res:
  match empty:
    case False{}:
      # a final completion: commit and end the turn
      apply.completion.plain2.finish(finish_turn(s), text)
    case True{}:
      # ADR 0015: an empty candidate commits nothing and emits no action
      apply.completion.empty(s)

def apply.completion.agent(matched: Bool, +text: String,
    +calls: List<&2, T.Call>, +s: T.Session) -> T.Res:
  match matched:
    case False{}:
      T.Rejected{"stale completion action"}
    case True{}:
      match calls:
        case Nil{}:
          apply.completion.plain2(String.is_empty(String.trim(text)), text, s)
        case Con{_, _}:
          # run_typescript is an ordinary tool: the runtime runs it in
          # V8 (bend-jsrt) and answers one tool result
          apply.completion.tools(text, calls, s)

def apply.completion.pending(action: U32, +text: String,
    +calls: List<&2, T.Call>, +s: T.Session, +p: T.Pending) -> T.Res:
  match p:
    case T.PAgent{a}:
      apply.completion.agent(U32.is_eq(a, action), text, calls, s)
    case T.PTools{_}:
      T.Rejected{"completion while tools pending"}
    case T.PComp{+compaction, +attempt, a, +extra, +auto}:
      apply.completion.comp(U32.is_eq(a, action), text, calls, compaction,
        attempt, extra, auto, s)

def apply.completion(action: U32, +text: String, +calls: List<&2, T.Call>,
    +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, _, +pending, _, _}:
      match pending:
        case None{}:
          T.Rejected{"no pending completion"}
        case Some{p}:
          apply.completion.pending(action, text, calls, s, p)

# ---- completion failure ----

# any assistant message: a history compaction can still shrink (a
# replacement holds the summary and user messages only, so a call
# refused for its size right after a compaction is not compacted again)
def has_asst(hs: List<&2, T.Msg>) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case Con{+m, rest}:
      match m:
        case T.Msg{+role, _, _, _}:
          match role:
            case T.RAsst{}:
              True{}
            case _:
              has_asst(rest)

# the turn a compaction carries: the notifications that arrived during
# the refused call go with it (dispatch delivers them after)
def turn_with_notifs(turn: Maybe<&2, T.Turn>, +notifs: List<&2, String>)
    -> Maybe<&2, T.Turn>:
  match turn:
    case None{}:
      Some{T.Turn{0n, Nil{}, notifs}}
    case Some{t}:
      match t:
        case T.Turn{+nulls, +steer, +tnotifs}:
          Some{T.Turn{nulls, steer, List.append(&2, String, notifs, tnotifs)}}

# big-request (2026-10-04): a call the provider refused for its size,
# after the images were already cut (runtime/provider.bend), starts an
# automatic compaction instead of failing the turn: the turn goes on
# from the replacement, like a compaction past the token threshold.
# Any other failure fails the active turn; the candidate is discarded
# without entering model context.
def apply.completion_fail.big(big: Bool, +error: String, +s: T.Session)
    -> T.Res:
  match big:
    case True{}:
      match s:
        case T.Sess{+cfg, +history, _, +actions, +turn, _, _, +notifs}:
          dispatch_result(start_compaction(True{}, "", cfg, history, actions,
              turn_with_notifs(turn, notifs), Nil{}, Nil{}),
            Con{T.CandidateDiscarded{error}, Nil{}}, bump_inputs(s))
    case False{}:
      T.Accepted{Con{T.CandidateDiscarded{error},
          Con{T.TurnDone{String.append("failed: ", error)}, Nil{}}},
        Nil{}, bump_inputs(finish_turn(s))}

def apply.completion_fail.agent(matched: Bool, +error: String,
    +s: T.Session) -> T.Res:
  match matched:
    case False{}:
      T.Rejected{"stale completion action"}
    case True{}:
      apply.completion_fail.big(Bool.and(W.too_large(error),
          has_asst(sess_history(s))), error, s)

def apply.completion_fail.comp(matched: Bool, +error: String, +auto: Bool,
    +s: T.Session) -> T.Res:
  match matched:
    case False{}:
      T.Rejected{"stale completion action"}
    case True{}:
      apply.completion.too_large(auto, String.append("provider failure: ", error), s)

def apply.completion_fail.pending(action: U32, +error: String,
    +s: T.Session, +p: T.Pending) -> T.Res:
  match p:
    case T.PAgent{a}:
      apply.completion_fail.agent(U32.is_eq(a, action), error, s)
    case T.PTools{_}:
      T.Rejected{"completion while tools pending"}
    case T.PComp{+compaction, +attempt, a, +extra, +auto}:
      apply.completion_fail.comp(U32.is_eq(a, action), error, auto, s)

def apply.completion_fail(action: U32, +error: String, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, _, +pending, _, _}:
      match pending:
        case None{}:
          T.Rejected{"no pending completion"}
        case Some{p}:
          apply.completion_fail.pending(action, error, s, p)

# ---- tool results ----

def apply.tool.done(+rest: List<&2, T.Call>, +call: U32, ok: Bool,
    +s1: T.Session) -> T.Res:
  match rest:
    case Nil{}:
      # every call answered: the next dispatch decides what happens next
      d = dispatch(s1)
      dispatch_result(d,
        Con{T.ToolFinished{call, ok}, Con{T.ToolCommitted{call}, Nil{}}},
        bump_inputs(s1))
    case Con{_, _}:
      T.Accepted{Con{T.ToolFinished{call, ok}, Con{T.ToolCommitted{call}, Nil{}}},
        Nil{}, bump_inputs(s1)}

def apply.tool.commit(+call: U32, +out: String, +ok: Bool, +s: T.Session,
    +c: T.Call, +rest: List<&2, T.Call>) -> T.Res:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
      match c:
        case T.Call{_, +name, _}:
          apply.tool.done(rest, call, ok,
            T.Sess{cfg, List.append(&2, T.Msg, history,
              Con{tool_result_msg(name, out, ok), Nil{}}), inputs,
              actions, turn, Some{T.PTools{rest}}, queued, notifs})

def apply.tool.found(+call: U32, +out: String, +ok: Bool, +s: T.Session,
    +found: Taken) -> T.Res:
  match found:
    case Got{c, rest}:
      apply.tool.commit(call, out, ok, s, c, rest)

def apply.tool.take(+call: U32, +out: String, +ok: Bool, +s: T.Session,
    +waiting: List<&2, T.Call>, taken: Maybe<&2, Taken>) -> T.Res:
  match taken:
    case None{}:
      # a late result cannot complete the current work
      T.Rejected{"unknown tool call id"}
    case Some{found}:
      apply.tool.found(call, out, ok, s, found)

def apply.tool.pending(+call: U32, +out: String, +ok: Bool, +s: T.Session,
    +p: T.Pending) -> T.Res:
  match p:
    case T.PTools{+waiting}:
      apply.tool.take(call, out, ok, s, waiting, take_call(waiting, call))
    case T.PAgent{_}:
      T.Rejected{"completion pending, not tools"}
    case T.PComp{_, _, _, _, _}:
      T.Rejected{"compaction pending, not tools"}

def apply.tool(+call: U32, +out: String, +ok: Bool, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, _, +pending, _, _}:
      match pending:
        case None{}:
          T.Rejected{"no pending tool result"}
        case Some{p}:
          apply.tool.pending(call, out, ok, s, p)

# ---- user messages ----

def apply.user.start(+text: String, +s: T.Session) -> T.Res:
  +s1 = start_turn(text, s)
  d = dispatch(s1)
  dispatch_result(d, Con{T.TurnStarted{}, Nil{}}, s1)

def apply.user.busy(mode: T.Mode, +text: String, +s: T.Session) -> T.Res:
  match mode:
    case T.Queue{}:
      # a queued message does not supersede in-flight work; it starts the
      # next turn
      T.Accepted{Nil{}, Nil{}, bump_inputs(queue_msg(s, text))}
    case T.Steer{}:
      # ADR 0005: steering is pending input, inserted at the next
      # model-safe boundary
      T.Accepted{Con{T.SteerReceived{text}, Nil{}}, Nil{},
        bump_inputs(steer_session(s, text))}

def apply.user.idle(mode: T.Mode, +text: String, +s: T.Session,
    turn: Maybe<&2, T.Turn>) -> T.Res:
  match turn:
    case None{}:
      # idle: a user message starts a turn
      apply.user.start(text, s)
    case Some{t}:
      apply.user.busy(mode, text, s)

def apply.user.state(mode: T.Mode, +text: String, +s: T.Session,
    turn: Maybe<&2, T.Turn>, pending: Maybe<&2, T.Pending>) -> T.Res:
  match pending:
    case None{}:
      apply.user.idle(mode, text, s, turn)
    case Some{p}:
      apply.user.busy(mode, text, s)

def apply.user(mode: T.Mode, +text: String, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, +turn, +pending, _, _}:
      apply.user.state(mode, text, s, turn, pending)

# ---- notifications ----

def apply.notify.idle(pending: Maybe<&2, T.Pending>, +text: String,
    +s: T.Session) -> T.Res:
  match pending:
    case None{}:
      # idle: the notification enters model context immediately
      match s:
        case T.Sess{+cfg, +history, +inputs, +actions, +turn, _, +queued, +notifs}:
          T.Accepted{Con{T.NotifReceived{text},
              Con{T.NotifDelivered{text}, Nil{}}},
            Nil{},
            T.Sess{cfg,
              List.append(&2, T.Msg, history,
                Con{T.injected_user(
                  String.append("[notification] ", text)), Nil{}}),
              Nat.add(inputs, 1n), actions, turn, None{}, queued, notifs}}
    case Some{p}:
      # held while a between-turn compaction is pending; delivered at the
      # next dispatch
      T.Accepted{Con{T.NotifReceived{text}, Nil{}}, Nil{},
        bump_inputs(hold_notif(s, text))}

def apply.notify(+text: String, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, +turn, +pending, _, _}:
      match turn:
        case Some{t}:
          # received during a turn: enters model context at the next
          # model-safe boundary
          T.Accepted{Con{T.NotifReceived{text}, Nil{}}, Nil{},
            bump_inputs(notif_turn(s, text))}
        case None{}:
          apply.notify.idle(pending, text, s)

# ---- manual compaction ----

def apply.compact.idle(pending: Maybe<&2, T.Pending>, +extra: String,
    +s: T.Session) -> T.Res:
  match pending:
    case Some{p}:
      T.Rejected{"compaction already pending"}
    case None{}:
      match s:
        case T.Sess{+cfg, +history, _, +actions, _, _, _, +notifs}:
          +d = start_compaction(False{}, extra, cfg, history, actions, None{},
            Nil{}, notifs)
          dispatch_result(d, Nil{}, bump_inputs(s))

def apply.compact(+extra: String, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, +turn, +pending, _, _}:
      match turn:
        case Some{t}:
          T.Rejected{"session busy: compact between turns"}
        case None{}:
          apply.compact.idle(pending, extra, s)

# ---- interrupt and fail_turn ----

# BR-001: an interrupt must not orphan the calls the turn holds. The
# completion carrying them is already in history; every waiting call
# gets a failed "interrupted" tool result BEFORE the turn dies, so
# the wire contract (every call answered by the next message) survives
# the interrupt. The synthesized answers ride the history BEFORE
# finish_turn commits the steering, so each answer stays adjacent to
# its assistant call.

def interrupt_closes.calls(waiting: List<&2, T.Call>,
    +acc: List<&2, T.Msg>) -> List<&2, T.Msg>:
  match waiting:
    case Nil{}:
      acc
    case Con{+c, rest}:
      match c:
        case T.Call{_, +name, _}:
          interrupt_closes.calls(rest,
            List.append(&2, T.Msg, acc,
              Con{tool_result_msg(name, "interrupted", False{}), Nil{}}))

def interrupt_closes.kind(pp: T.Pending) -> List<&2, T.Msg>:
  match pp:
    case T.PTools{+waiting}:
      interrupt_closes.calls(waiting, Nil{})
    case T.PAgent{_}:
      # the completion is not in history yet: nothing is orphaned
      Nil{}
    case T.PComp{_, _, _, _, _}:
      # a compaction holds no model-visible call
      Nil{}

def interrupt_closes(p: Maybe<&2, T.Pending>) -> List<&2, T.Msg>:
  match p:
    case None{}:
      Nil{}
    case Some{+pp}:
      interrupt_closes.kind(pp)

def with_msgs(+s: T.Session, +extra: List<&2, T.Msg>) -> T.Session:
  match s:
    case T.Sess{+cfg, +history, +inputs, +actions, +turn, +pending, +queued,
        +notifs}:
      T.Sess{cfg, List.append(&2, T.Msg, history, extra), inputs, actions,
        turn, pending, queued, notifs}

def apply.interrupt(+s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, Some{_}, +pending, _, _}:
      # the turn dies: what it was still holding is committed
      T.Accepted{Con{T.CandidateDiscarded{"interrupt"},
          Con{T.TurnDone{"interrupted"}, Nil{}}},
        Nil{},
        bump_inputs(finish_turn(with_msgs(s, interrupt_closes(pending))))}
    case T.Sess{_, _, _, _, None{}, _, _, _}:
      # interrupting an inactive session is an accepted idempotent no-op
      T.Accepted{Nil{}, Nil{}, bump_inputs(s)}

def apply.fail(+error: String, +s: T.Session) -> T.Res:
  match s:
    case T.Sess{_, _, _, _, +turn, +pending, _, _}:
      match turn:
        case Some{t}:
          # the turn dies: what it was still holding is committed, and
          # the calls it waited for are answered first (BR-001, BISE-242)
          T.Accepted{Con{T.TurnDone{String.append("failed: ", error)}, Nil{}},
            Nil{}, bump_inputs(finish_turn(with_msgs(s, interrupt_closes(pending))))}
        case None{}:
          T.Rejected{"no active turn"}

# ---- the step protocol entry point ----

def apply.cmd(cmd: T.Cmd, +s: T.Session) -> T.Res:
  match cmd:
    case T.UserMsg{mode, text}:
      apply.user(mode, text, s)
    case T.Notify{text}:
      apply.notify(text, s)
    case T.Compact{extra}:
      apply.compact(extra, s)
    case T.CompletionOk{action, text, calls}:
      apply.completion(action, text, calls, s)
    case T.CompletionFail{action, error}:
      apply.completion_fail(action, error, s)
    case T.ToolOk{call, out}:
      apply.tool(call, out, True{}, s)
    case T.ToolFail{call, error}:
      apply.tool(call, error, False{}, s)
    case T.Interrupt{}:
      apply.interrupt(s)
    case T.FailTurn{error}:
      apply.fail(error, s)

def apply(s: T.Session, cmd: T.Cmd) -> T.Res:
  apply.cmd(cmd, s)
