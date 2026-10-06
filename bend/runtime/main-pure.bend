# runtime/main-pure.bend - the pure half of runtime/main.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base
import ../core/wire.bend as W
import ../core/compaction.bend as C
import ../vendor/json.bend as J
import ./model.bend as M
import ../core/obs.bend as O
import ../core/ev.bend as Ev
import ./remote.bend as Rem
import ../core/session.bend as S
import ../core/types.bend as T
import ../core/text.bend as Tx
import ./tools-pure.bend as Xp
import ../core/edit.bend as Ed
import ./selftools-pure.bend as Sfp

# ---- runtime state and loop vocabulary ----

type Rt is Data:
  Rt{sess: T.Session, script: List<&2, M.Reply>, cmds: List<&2, T.Cmd>,
     live: Bool}

type Job is Data:
  # apply one command, then execute the `after` acts
  JCmd{cmd: T.Cmd, after: List<&2, T.Act>}
  JActs{acts: List<&2, T.Act>}
  JIdle{}
  JDone{}

type Effect is Data:
  EPrints{lines: List<&2, String>}
  # a real provider call: the hub HTTP client, in this process
  EModel{action: U32, request: String, after: List<&2, T.Act>}
  # a real tool execution (bash, the merged search, remote MCP calls).
  # The live flag keeps scripted scenarios hermetic: no MCP index reads.
  EExec{call: T.Call, after: List<&2, T.Act>, live: Bool}
  ENone{}
  # the next job is a queued input popped at the idle boundary: the
  # loop applies it with Rtp.transition_queued (its ev lines say so)
  EDequeue{}

type Trans is Data:
  Tr{effect: Effect, rt: Rt, job: Job}

# ---- observation rendering (pure) ----

def obs_lines(os: List<&2, T.Obs>) -> List<&2, String>:
  match os:
    case Nil{}:
      Nil{}
    case Con{o, rest}:
      Con{O.show_obs(o), obs_lines(rest)}

# ---- the pure transition ----


def rt_live(+rt: Rt) -> Bool:
  match rt:
    case Rt{_, _, _, +live}:
      live

# every tool is a Runtime effect: bash, the merged search (it reads
# the MCP index), remote MCP tools resolved by name, and unknown names
# (which fail cleanly with the name). self.sleep is NOT a model tool
# anymore: it is a programmatic tool, called inside a run_typescript
# program (exec_program.call.sleep below).
def trans.tool(+rt: Rt, +rest: List<&2, T.Act>, +call: T.Call) -> Trans:
  match call:
    case T.Call{_, _, _}:
      Tr{EExec{call, rest, rt_live(rt)}, rt, JIdle{}}

def trans.model.ok(+rt: Rt, +rest: List<&2, T.Act>, +action: U32,
    +text: String, +calls: List<&2, T.Call>, +script2: List<&2, M.Reply>)
    -> Trans:
  match rt:
    case Rt{+sess, _, +cmds, +live}:
      Tr{ENone{}, Rt{sess, script2, cmds, live},
        JCmd{T.CompletionOk{action, text, calls},
          rest}}

def trans.model.fail(+rt: Rt, +rest: List<&2, T.Act>, +action: U32,
    +script2: List<&2, M.Reply>) -> Trans:
  match rt:
    case Rt{+sess, _, +cmds, +live}:
      Tr{ENone{}, Rt{sess, script2, cmds, live},
        JCmd{T.CompletionFail{action, "scripted provider failure"}, rest}}

def trans.model.reply(+rt: Rt, +rest: List<&2, T.Act>, +action: U32,
    +reply: M.Reply, +script2: List<&2, M.Reply>) -> Trans:
  match reply:
    case M.ROk{text, calls}:
      trans.model.ok(rt, rest, action, text, calls, script2)
    case M.RFail{}:
      trans.model.fail(rt, rest, action, script2)

def trans.model.step(+rt: Rt, +rest: List<&2, T.Act>, +action: U32,
    st: M.Stepped) -> Trans:
  match st:
    case M.Stepped{reply, script2}:
      trans.model.reply(rt, rest, action, reply, script2)

def trans.model.live(+sess: T.Session, +rt: Rt, +rest: List<&2, T.Act>,
    +action: U32, +input: List<&2, T.Msg>) -> Trans:
  match sess:
    case T.Sess{+cfg, _, _, _, _, _, _, _}:
      Tr{EModel{action, Rem.build_request(cfg, input), rest}, rt, JIdle{}}

def script_of(+rt: Rt) -> List<&2, M.Reply>:
  match rt:
    case Rt{_, +script, _, _}:
      script

# compaction calls carry no tool catalog: a tool call cannot even be
# offered, so the summary cannot be malformed by one. Live mode sends the
# bare request to the real provider; scripted mode pops the script like
# every other model call, so the compaction scenarios stay deterministic.
# BISE-268: the first attempt shares the agent calls' prefix (tools
# included, Rem.build_request.compact); the retry goes bare
def compact_first.pd(pd: T.Pending) -> Bool:
  match pd:
    case T.PComp{_, +attempt, _, _, _}:
      Nat.is_le(attempt, 1n)
    case T.PAgent{_}:
      False{}
    case T.PTools{_}:
      False{}

def compact_first(p: Maybe<&2, T.Pending>) -> Bool:
  match p:
    case Some{+pd}:
      compact_first.pd(pd)
    case None{}:
      False{}

def compact_request(first: Bool, +cfg: T.Cfg, +input: List<&2, T.Msg>)
    -> String:
  match first:
    case True{}:
      Rem.build_request.compact(cfg, input)
    case False{}:
      Rem.build_request.bare(cfg, input)

def trans.model.compact.live(+sess: T.Session, +rt: Rt,
    +rest: List<&2, T.Act>, +action: U32, +input: List<&2, T.Msg>) -> Trans:
  match sess:
    case T.Sess{+cfg, _, _, _, _, +pending, _, _}:
      Tr{EModel{action, compact_request(compact_first(pending), cfg, input),
        rest}, rt, JIdle{}}

def trans.model.compact.script(+rt: Rt, +rest: List<&2, T.Act>,
    +action: U32) -> Trans:
  trans.model.step(rt, rest, action, M.model_next(script_of(rt)))

def trans.model.compact.pick(live: Bool, +sess: T.Session, +rt: Rt,
    +rest: List<&2, T.Act>, +action: U32, +input: List<&2, T.Msg>) -> Trans:
  match live:
    case True{}:
      trans.model.compact.live(sess, rt, rest, action, input)
    case False{}:
      trans.model.compact.script(rt, rest, action)

def trans.model.compact(+rt: Rt, +rest: List<&2, T.Act>,
    +action: U32, +input: List<&2, T.Msg>) -> Trans:
  match rt:
    case Rt{+sess, _, _, +live}:
      trans.model.compact.pick(live, sess, rt, rest, action, input)

def trans.model.pick(live: Bool, +sess: T.Session, +rt: Rt,
    +rest: List<&2, T.Act>, +action: U32, +input: List<&2, T.Msg>)
    -> Trans:
  match live:
    case True{}:
      trans.model.live(sess, rt, rest, action, input)
    case False{}:
      trans.model.step(rt, rest, action, M.model_next(script_of(rt)))

def trans.model(+rt: Rt, +rest: List<&2, T.Act>, +action: U32,
    +input: List<&2, T.Msg>) -> Trans:
  match rt:
    case Rt{+sess, _, _, +live}:
      trans.model.pick(live, sess, rt, rest, action, input)

def trans.act(+rt: Rt, +rest: List<&2, T.Act>, +a: T.Act) -> Trans:
  match a:
    case T.AgentCall{action, input}:
      trans.model(rt, rest, action, input)
    case T.CompactionCall{action, input}:
      trans.model.compact(rt, rest, action, input)
    case T.ToolAct{_, call}:
      trans.tool(rt, rest, call)

# the session's own queue drains at the idle boundary: a message the
# Core queued mid-turn (apply.user.busy: Queue never supersedes
# in-flight work) starts the next turn as soon as the current one
# ends — the same contract as a line the client queued between turns
# (rt.cmds takes priority; the tail stays queued for later turns)
def trans.idle.queued(q: List<&2, String>, +sess: T.Session,
    +script: List<&2, M.Reply>, +live: Bool) -> Trans:
  match q:
    case Nil{}:
      Tr{EPrints{Con{"--- idle", Nil{}}}, Rt{sess, script, Nil{}, live}, JDone{}}
    case Con{+text, +rest}:
      Tr{EDequeue{}, Rt{S.set_queued(sess, rest), script, Nil{}, live},
        JCmd{T.UserMsg{T.Queue{}, text}, Nil{}}}

def trans.idle(+rt: Rt) -> Trans:
  match rt:
    case Rt{+sess, +script, cmds, +live}:
      match cmds:
        case Nil{}:
          trans.idle.queued(S.sess_queued(sess), sess, script, live)
        case Con{c, rest}:
          Tr{ENone{}, Rt{sess, script, rest, live}, JCmd{c, Nil{}}}

def trans.acts(+rt: Rt, +acts: List<&2, T.Act>) -> Trans:
  match acts:
    case Nil{}:
      trans.idle(rt)
    case Con{a, rest}:
      trans.act(rt, rest, a)

# the obs lines of an apply, then its session-log facts (core/ev.bend)
def trans.res.accepted(+rt: Rt, +after: List<&2, T.Act>, +obs: List<&2, T.Obs>,
    +acts: List<&2, T.Act>, +next: T.Session, +evs: List<&2, String>) -> Trans:
  match rt:
    case Rt{_, +script, +cmds, +live}:
      Tr{EPrints{List.append(&2, String, obs_lines(obs), evs)},
        Rt{next, script, cmds, live},
        JActs{List.append(&2, T.Act, acts, after)}}

def trans.res(+rt: Rt, +after: List<&2, T.Act>, res: T.Res,
    +evs: List<&2, String>) -> Trans:
  match res:
    case T.Rejected{reason}:
      Tr{EPrints{Con{String.append("core rejected: ", reason), Nil{}}},
        rt, JActs{after}}
    case T.Accepted{obs, acts, next}:
      trans.res.accepted(rt, after, obs, acts, next, evs)

# `fromq`: the command is a queued input popped at the idle boundary
def trans.cmd.src(+fromq: Bool, +rt: Rt, +after: List<&2, T.Act>,
    +cmd: T.Cmd) -> Trans:
  match rt:
    case Rt{+sess, _, _, _}:
      +res = S.apply(sess, cmd)
      trans.res(rt, after, res, Ev.evs(fromq, sess, cmd, res))

def trans.cmd(+rt: Rt, +after: List<&2, T.Act>, +cmd: T.Cmd) -> Trans:
  trans.cmd.src(False{}, rt, after, cmd)

def merge_acts(+acts: List<&2, T.Act>, +cont: Job) -> Job:
  match cont:
    case JCmd{cmd, after}:
      JCmd{cmd, List.append(&2, T.Act, acts, after)}
    case JActs{as0}:
      JActs{List.append(&2, T.Act, acts, as0)}
    case JIdle{}:
      JActs{acts}
    case JDone{}:
      JActs{acts}

def trans.drain.ok(+rt: Rt, +cont: Job, +rest_cmds: List<&2, T.Cmd>,
    +obs: List<&2, T.Obs>, +acts: List<&2, T.Act>, +next: T.Session,
    +evs: List<&2, String>) -> Trans:
  match rt:
    case Rt{_, +script, _, +live}:
      Tr{EPrints{List.append(&2, String, obs_lines(obs), evs)},
        Rt{next, script, rest_cmds, live},
        merge_acts(acts, cont)}

def trans.drain.rejected(+rt: Rt, +cont: Job, +rest_cmds: List<&2, T.Cmd>,
    +reason: String) -> Trans:
  match rt:
    case Rt{+sess, +script, _, +live}:
      Tr{EPrints{Con{String.append("core rejected: ", reason), Nil{}}},
        Rt{sess, script, rest_cmds, live}, cont}

def trans.drain.res(+rt: Rt, +cont: Job, +rest_cmds: List<&2, T.Cmd>,
    res: T.Res, +evs: List<&2, String>) -> Trans:
  match res:
    case T.Rejected{reason}:
      # consumed with a rejection line; the pending work continues
      trans.drain.rejected(rt, cont, rest_cmds, reason)
    case T.Accepted{obs, acts, next}:
      trans.drain.ok(rt, cont, rest_cmds, obs, acts, next, evs)

def trans.drain.cmd(+rt: Rt, +cont: Job, +rest_cmds: List<&2, T.Cmd>,
    +sess: T.Session, +c: T.Cmd) -> Trans:
  +res = S.apply(sess, c)
  trans.drain.res(rt, cont, rest_cmds, res, Ev.evs(False{}, sess, c, res))

def trans.job(+rt: Rt, +cont: Job) -> Trans:
  match cont:
    case JCmd{cmd, after}:
      trans.cmd(rt, after, cmd)
    case JActs{acts}:
      trans.acts(rt, acts)
    case JIdle{}:
      trans.idle(rt)
    case JDone{}:
      Tr{ENone{}, rt, JDone{}}

def trans.drain(+rt: Rt, +cont: Job) -> Trans:
  match rt:
    case Rt{+sess, +script, cmds, _}:
      match cmds:
        case Nil{}:
          trans.job(rt, cont)
        case Con{+c, rest}:
          # only steering, notifications, and interrupts interleave with
          # in-flight work; queued messages and compaction wait for an
          # idle boundary (the Core would sort them out anyway, but the
          # client contract sends them between turns)
          match c:
            case T.UserMsg{T.Steer{}, _}:
              trans.drain.cmd(rt, cont, rest, sess, c)
            case T.Notify{_}:
              trans.drain.cmd(rt, cont, rest, sess, c)
            case T.Interrupt{}:
              trans.drain.cmd(rt, cont, rest, sess, c)
            case _:
              trans.job(rt, cont)

def transition(rt: Rt, job: Job) -> Trans:
  match job:
    case JCmd{cmd, after}:
      trans.cmd(rt, after, cmd)
    case JActs{acts}:
      trans.drain(rt, JActs{acts})
    case JIdle{}:
      trans.idle(rt)
    case JDone{}:
      Tr{ENone{}, rt, JDone{}}

# the transition after an EDequeue: the popped input is applied as a
# queued one (turn_started cause "queue", from_queue)
def transition_queued(rt: Rt, job: Job) -> Trans:
  match job:
    case JCmd{cmd, after}:
      trans.cmd.src(True{}, rt, after, cmd)
    case _:
      transition(rt, job)

# ---- one edit toolset per request, after a provider switch ----
# The catalog and the prompt line are built at start for the provider
# of that start (repl-live.bend live_cfg). The model is re-resolved
# before each call (/model), so a request whose provider wants the
# other toolset is re-tooled here: its tool lines (the whole catalog,
# bash's description names the edit tool too) and the system prompt's
# edit line are swapped. The session keeps its history: past
# apply_patch, edit or write_file calls replay as they are.
def rt_tools(+rt: Rt) -> List<&2, T.ToolDef>:
  match rt:
    case Rt{+sess, _, _, _}:
      match sess:
        case T.Sess{+cfg, _, _, _, _, _, _, _}:
          match cfg:
            case T.Cfg{_, _, _, _, +tools}:
              tools

def retool(+request: String, +old: List<&2, T.ToolDef>,
    +new: List<&2, T.ToolDef>, +old_line: String, +new_line: String) -> String:
  Ed.replace(Ed.replace(request, Rem.ser_tools(old), Rem.ser_tools(new), False{}),
    old_line, new_line, False{})

# Vibe's edit tools (core/edit.bend)
def is_edit_tool(+name: String) -> Bool:
  Bool.or(String.eq(name, "edit"), String.eq(name, "write_file"))

# ---- feed annotations for the TUI (live only) ----
# The Core's obs carry ids and outcomes; the runtime adds what the
# engineer actually needs: the tool name, the args, and a result
# preview. One line each, flattened and capped:
#   tool #<id> <name> : <args>
#   tool_result #<id> <ok|fail> : <out>
#   subtool <name> <ok|fail> : <out>          (inside run_typescript)

def ann_flat(+s: String) -> String:
  String.take(Tx.flatten(s), 200n)

def ok_label(ok: Bool) -> String:
  match ok:
    case True{}:
      "ok"
    case False{}:
      "fail"

# a bash call shows its command, not the JSON of its description; an
# edit or write_file call its path first (the hub's activity line)
def call_ann.edit(is_edit: Bool, +name: String, +args: String) -> String:
  match is_edit:
    case True{}:
      Ed.ann_args(name, args)
    case False{}:
      args

def call_ann.pick(is_bash: Bool, +name: String, +args: String) -> String:
  match is_bash:
    case True{}:
      W.flat_cmd(args)
    case False{}:
      call_ann.edit(is_edit_tool(name), name, args)

def call_ann.args(+name: String, +args: String) -> String:
  call_ann.pick(String.eq(name, "bash"), name, args)

def call_ann(+call: T.Call) -> String:
  match call:
    case T.Call{+id, +name, +args}:
      String.append("tool #", String.append(U32.show(id),
        String.append(" ", String.append(name,
          String.append(" : ", ann_flat(call_ann.args(name, args)))))))

def res_ann(+id: U32, +r: Xp.ExecRes) -> String:
  match r:
    case Xp.Ex{+ok, +out}:
      String.append("tool_result #", String.append(U32.show(id),
        String.append(" ", String.append(ok_label(ok),
          String.append(" : ", ann_flat(out))))))

# a JSON parse as a Maybe (malformed text is no value)
def json_m(r: Result<&2, &2, String, J.Json>) -> Maybe<&2, J.Json>:
  match r:
    case Done{+j}:
      Some{j}
    case Fail{_}:
      None{}

# parse the Proc.run output: exit-code line, then stdout
def node_req.tool_str(v: J.Json) -> String:
  match v:
    case J.JStr{+s}:
      s
    case _:
      "?"

def node_req.tool(m: Maybe<&2, J.Json>) -> String:
  match m:
    case None{}:
      "?"
    case Some{+v}:
      node_req.tool_str(v)

# exit 43: the engine printed {"error": "..."} — hand the message back
# to the model so it can read the error and retry
def node_err.res(r: Maybe<&2, J.Json>) -> String:
  match r:
    case Some{+v}:
      node_req.tool(J.get(v, "error"))
    case None{}:
      "program failed"

def node_err(+body: String) -> String:
  node_err.res(json_m(J.parse(String.trim(body))))

# ---- self.sleep: a programmatic tool (called inside a program) ----
# await self.sleep(20) inside a run_typescript program. The seconds
# come as a bare number (self.sleep(20)) or as {seconds: 20}; the HOST
# sleeps between the replay rounds (the V8 watchdog only bounds the
# program own CPU), and the recorded result replays by position, so
# the wait happens exactly once.
def prog_sleep.field(f: J.Json) -> U32:
  match f:
    case J.JNum{+n}:
      U32.from_nat(n)
    case _:
      0

def prog_sleep.parsed.get(m: Maybe<&2, J.Json>, +v: J.Json) -> U32:
  match m:
    case Some{+f}:
      prog_sleep.field(f)
    case None{}:
      prog_sleep.field(v)

def prog_sleep.parsed(+v: J.Json) -> U32:
  prog_sleep.parsed.get(J.get(v, "seconds"), v)

def prog_sleep.secs.none(+args: String) -> U32:
  Maybe.default(&2, U32, U32.read(args), 0)

def prog_sleep.secs(m: Maybe<&2, J.Json>, +args: String) -> U32:
  match m:
    case Some{+v}:
      prog_sleep.parsed(v)
    case None{}:
      prog_sleep.secs.none(args)

def prog_sleep.bounds(+secs: U32) -> Bool:
  Bool.and(U32.is_lt(0, secs), U32.is_lt(secs, 20))

# the sub-call line renders in the TUI feed (live only): one line per
# tool the program called, with the outcome. Out is Type-kind: every
# emit writes the same connection, so the emitter param is shared and
# the returned handle is discarded here.
def sub_ann(+name: String, +r: Xp.ExecRes) -> String:
  match r:
    case Xp.Ex{+ok, +out}:
      String.append("subtool ", String.append(name,
        String.append(" ", String.append(ok_label(ok),
          String.append(" : ", ann_flat(out))))))

# tool dispatch: which executor a call's name selects (checked in this
# order; anything unknown is an MCP connector tool). self.compact and
# self.reload are no top-level tools (BISE-293): only a run_typescript
# program files them (runtime/main.bend exec_program.call.self). run_typescript runs
# in V8 (bend-jsrt), live and scripted alike (BISE-118). The comparisons
# are a handful of short String.eq: evaluating them all is cheap.
type Tool is Data:
  TNode{}
  TBash{}
  TSearch{}
  TSkill{}
  TPatch{}
  TEdit{}
  TWrite{}
  TMcp{}

def tool_of(+name: String) -> Tool:
  Bool.pick(Tool, String.eq(name, "run_typescript"), TNode{},
  Bool.pick(Tool, String.eq(name, "bash"), TBash{},
  Bool.pick(Tool, String.eq(name, "search_tool_functions"), TSearch{},
  Bool.pick(Tool, String.eq(name, "skill"), TSkill{},
  Bool.pick(Tool, String.eq(name, "apply_patch"), TPatch{},
  Bool.pick(Tool, String.eq(name, "edit"), TEdit{},
  Bool.pick(Tool, String.eq(name, "write_file"), TWrite{}, TMcp{})))))))

def call_name(c: T.Call) -> String:
  match c:
    case T.Call{_, +name, _}:
      name

def call_id(c: T.Call) -> U32:
  match c:
    case T.Call{id, _, _}:
      id

def call_args(c: T.Call) -> String:
  match c:
    case T.Call{_, _, args}:
      args

# the source line of a code tool (run_typescript, bash, apply_patch -
# the TUI renders the patch as a diff): the FULL args,
# wire-encoded (newline as \N, backslash doubled - the same reversible
# encoding the provider wire uses), never flattened or capped, so the
# TUI renders the whole program or script; the tool # preview line
# stays capped at 200
# (a bash call with a description shows its command: W.flat_cmd)
# edit and write_file show as the V4A patch they amount to (the TUI
# draws it as a diff, like apply_patch's)
def code_src.write(is_write: Bool, +args: String) -> String:
  match is_write:
    case True{}:
      Ed.write_view(args)
    case False{}:
      W.flat_cmd(args)

def code_src.edit(is_edit: Bool, +name: String, +args: String) -> String:
  match is_edit:
    case True{}:
      Ed.edit_view(args)
    case False{}:
      code_src.write(String.eq(name, "write_file"), args)

def code_src(+name: String, +args: String) -> String:
  code_src.edit(String.eq(name, "edit"), name, args)

def code_ann(+call: T.Call) -> String:
  String.append("tool_code #", String.append(U32.show(call_id(call)),
    String.append(" : ", W.wire_encode(code_src(call_name(call), call_args(call))))))

# the intent line of a bash or run_typescript call (BISE-223): the
# model's one-line description of the call, for the TUI to show in
# place of the code; no line when the call has none
def is_intent_call(+name: String) -> Bool:
  Bool.or(String.eq(name, "run_typescript"), String.eq(name, "bash"))

def intent_ann(+call: T.Call) -> String:
  String.append("tool_intent #", String.append(U32.show(call_id(call)),
    String.append(" : ", W.call_intent(call_args(call)))))

def intent_anns.pick(has: Bool, +call: T.Call) -> List<&2, String>:
  match has:
    case True{}:
      Con{intent_ann(call), Nil{}}
    case False{}:
      Nil{}

def intent_anns(+call: T.Call) -> List<&2, String>:
  intent_anns.pick(Bool.and(is_intent_call(call_name(call)),
    Bool.not(String.is_empty(W.call_intent(call_args(call))))), call)

def is_code_call(+name: String) -> Bool:
  Bool.or(String.eq(name, "run_typescript"),
    Bool.or(String.eq(name, "bash"),
      Bool.or(String.eq(name, "apply_patch"), is_edit_tool(name))))

# ---- --resume / --continue: replay the restored history to the client ----
# A client's feed starts empty: a fresh TUI after --resume, and after a
# /reload respawn too. The REPL replays the restored history as the SAME
# wire lines a live turn prints, each prefixed "history ", so the client
# renders it with its live code path (tool blocks included). Two lines
# have no live form: "you : <text>" (a user message, typed client-side
# in a live turn) and "injected : <text>" (steering / notifications).
# Tool results carry no call id: they follow their assistant message in
# call order, so the walk keeps the calls still waiting (pend). A call
# that never got its result (interrupt, crash) replays as failed.
# One self-recursive def, fuel first, the state in Data; the output
# accumulates reversed.

type Rp is Data:
  Rp{out: List<&2, String>, ms: List<&2, T.Msg>, pend: List<&2, T.Call>}

def hist(+l: String) -> String:
  String.append("history ", l)

def rev_onto(xs: List<&2, String>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case Con{h, t}:
      rev_onto(t, Con{h, acc})

def fin_label(ok: Bool) -> String:
  match ok:
    case True{}:
      " ok"
    case False{}:
      " failed"

def replay.fin(+id: U32, ok: Bool) -> String:
  hist(String.append("  obs: tool_finished #",
    String.append(U32.show(id), fin_label(ok))))

def replay.code(is_code: Bool, +c: T.Call) -> List<&2, String>:
  match is_code:
    case True{}:
      Con{hist(code_ann(c)), Nil{}}
    case False{}:
      Nil{}

def replay.intent(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case Con{+l, t}:
      Con{hist(l), replay.intent(t)}

def replay.call(+c: T.Call) -> List<&2, String>:
  Con{hist(String.append("  obs: tool_started #", U32.show(call_id(c)))),
    Con{hist(call_ann(c)),
      List.append(&2, String, replay.code(is_code_call(call_name(c)), c),
        replay.intent(intent_anns(c)))}}

def replay.calls(cs: List<&2, T.Call>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, t}:
      List.append(&2, String, replay.call(c), replay.calls(t))

def replay.abandon(cs: List<&2, T.Call>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, t}:
      Con{replay.fin(call_id(c), False{}), replay.abandon(t)}

# a committed result: "tool <name> ok: <out>" / "tool <name> failed: <out>"
# (session.tool_result_msg), the out newline-escaped
def replay.res.lines(+id: U32, +ok: Bool, +out: String) -> List<&2, String>:
  Con{hist(res_ann(id, Xp.Ex{ok, W.unescape_nl(out)})),
    Con{replay.fin(id, ok), Nil{}}}

def replay.res.fail(is_fail: Bool, +id: U32, +text: String, +pfail: String)
    -> List<&2, String>:
  match is_fail:
    case True{}:
      replay.res.lines(id, False{}, String.drop(text, String.length(pfail)))
    case False{}:
      replay.res.lines(id, True{}, text)

def replay.res.ok(is_ok: Bool, +id: U32, +text: String, +pok: String,
    +pfail: String) -> List<&2, String>:
  match is_ok:
    case True{}:
      replay.res.lines(id, True{}, String.drop(text, String.length(pok)))
    case False{}:
      replay.res.fail(String.starts_with(text, pfail), id, text, pfail)

def replay.res(+c: T.Call, +text: String) -> List<&2, String>:
  match c:
    case T.Call{+id, +name, _}:
      +base = String.append("tool ", name)
      replay.res.ok(String.starts_with(text, String.append(base, " ok: ")),
        id, text, String.append(base, " ok: "),
        String.append(base, " failed: "))

def replay.injected.other(is_preamble: Bool, +text: String)
    -> List<&2, String>:
  match is_preamble:
    case True{}:
      Nil{}
    case False{}:
      Con{hist(String.append("injected : ", W.escape_nl(text))), Nil{}}

def replay.injected.summary(is_summary: Bool, +text: String)
    -> List<&2, String>:
  match is_summary:
    case True{}:
      Con{hist(O.show_obs(T.CompactionDone{
        Maybe.default(&2, String, C.parse_summary(text), text)})), Nil{}}
    case False{}:
      replay.injected.other(
        String.starts_with(text, "The earlier conversation was compacted."),
        text)

def replay.user(inj: Bool, +text: String, +rest: List<&2, T.Msg>,
    +pend: List<&2, T.Call>, +out: List<&2, String>) -> Rp:
  match inj:
    case False{}:
      Rp{Con{hist(String.append("you : ", W.escape_nl(text))), out},
        rest, pend}
    case True{}:
      Rp{rev_onto(replay.injected.summary(
          String.starts_with(text, "Summary of the earlier conversation:"),
          text), out), rest, pend}

def replay.asst(empty: Bool, +text: String) -> List<&2, String>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      # the restored text may carry real newlines: escaped, the line stays
      # one wire line (live replies arrive escaped already)
      Con{hist(O.show_obs(T.Assistant{W.escape_nl(text)})), Nil{}}

def replay.tool(pend: List<&2, T.Call>, +text: String,
    +rest: List<&2, T.Msg>, +out: List<&2, String>) -> Rp:
  match pend:
    case Nil{}:
      Rp{out, rest, Nil{}}
    case Con{+c, +t}:
      Rp{rev_onto(replay.res(c, text), out), rest, t}

def replay.msg(+m: T.Msg, +rest: List<&2, T.Msg>, +pend: List<&2, T.Call>,
    +out: List<&2, String>) -> Rp:
  match m:
    case T.Msg{+role, +text, +calls, +inj}:
      match role:
        case T.RUser{}:
          replay.user(inj, text, rest, pend, out)
        case T.RAsst{}:
          # a new assistant message: calls still waiting never got a result
          Rp{rev_onto(List.append(&2, String, replay.abandon(pend),
              List.append(&2, String, replay.asst(String.is_empty(text), text),
                replay.calls(calls))), out), rest, calls}
        case T.RTool{}:
          replay.tool(pend, text, rest, out)
        case T.RSys{}:
          Rp{out, rest, pend}

def replay.step(st: Rp) -> Rp:
  match st:
    case Rp{+out, +ms, +pend}:
      match ms:
        case Nil{}:
          Rp{rev_onto(replay.abandon(pend), out), Nil{}, Nil{}}
        case Con{+m, +rest}:
          replay.msg(m, rest, pend, out)

def replay.go(fuel: Nat, st: Rp) -> Rp:
  match fuel:
    case 0n:
      st
    case 1n+k:
      replay.go(k, replay.step(st))

def replay.out(st: Rp) -> List<&2, String>:
  match st:
    case Rp{+out, _, _}:
      List.reverse(&2, String, out)

# one step per message, one more to flush the calls left waiting
def replay(+history: List<&2, T.Msg>) -> List<&2, String>:
  replay.out(replay.go(Nat.add(List.length(&2, T.Msg, history), 1n),
    Rp{Nil{}, history, Nil{}}))


# ---- mid-turn steering side-channel (ADR 0005, live) ----
# The REPL is single-threaded: the socket is only read BETWEEN turns, so
# a "steer" line sent during a turn waits in the kernel buffer until the
# turn ends — the Core's mid-turn steering was never exercised live (an
# idle steer even starts a NEW turn, the exact opposite of ADR 0005).
# Codex-style fix: the client appends steering lines to a per-process
# file, and the Runtime drains it at the two safe boundaries — right
# before a model result or a tool result is applied. There the Core is
# mid-turn, so apply routes Steer to steer_session; the completion is
# then applied on a session that already holds the steering, and
# dispatch injects it into the next model input.

def steer_path_of(+dir: String, +port: String) -> String:
  String.append(dir, String.append("/bend-steer-", String.append(port, ".txt")))

# ---- the harness-info line: what every client must know ----
# The REPL is the single source of truth. Both clients (the TUI via
# the bend-harness parent, and bend_client.py) read this line from the
# REPL's output instead of recomputing anything: the resolved model,
# the compaction threshold, and the two side-channel paths. One line,
# space-separated key=value pairs, printed BEFORE the "REPL on" banner
# (the parent waits for the banner, so the info is always there).
def info_line(+model: String, +threshold: String, +steer: String,
    +interrupt: String) -> String:
  String.append("harness-info model=",
    String.append(model,
      String.append(" threshold=",
        String.append(threshold,
          String.append(" steer=",
            String.append(steer,
              String.append(" interrupt=", interrupt)))))))

# the feed lines for one drained steering line's apply result; mid-turn
# Steer is always Accepted (apply.user.busy), so the Rejected arm is
# the defensive one
def drain_steer.emit_of(res: T.Res) -> List<&2, String>:
  match res:
    case T.Rejected{+reason}:
      Con{String.append("  obs: steering_rejected: ", reason), Nil{}}
    case T.Accepted{+obs, _, _}:
      obs_lines(obs)

def drain_steer.next_of(+sess: T.Session, res: T.Res) -> T.Session:
  match res:
    case T.Rejected{_}:
      sess
    case T.Accepted{_, _, +next}:
      next

# the result of a call a stop left behind (main.bend exec_tool.raced):
# who stopped it, and that it may still finish
def abandoned(+who: String, +what: String) -> String:
  String.concat(["interrupted by ", who, ": the turn stopped without waiting for ",
    what, ", which cannot be cancelled from here; it may still finish (on its server, for a remote call)"])

# ---- a tool that a stop cut (issue 13) ----
# The tool came back with the interrupt flag set (the bash wrapper
# stopped its command, or the call ended meanwhile): its own result is
# applied BEFORE the boundary drain applies the interrupt, so the
# history keeps what ran and was cut ("interrupted by <who>" and the
# output so far) instead of the Core's bare "interrupted". The call then
# has its one result: interrupt_closes answers only the calls still
# waiting. The acts the result asked for (the next model call) wait in
# First until the drain: a turn the interrupt ended drops them and goes
# idle (the batch's next tools never start); a turn still on runs them.
type First is Data:
  First{lines: List<&2, String>, rt: Rt, acts: List<&2, T.Act>}

def tool_first.res(+rt: Rt, +sess: T.Session, +cmd: T.Cmd, res: T.Res)
    -> First:
  match rt:
    case Rt{_, +script, +cmds, +live}:
      match res:
        case T.Rejected{+reason}:
          First{Con{String.append("core rejected: ", reason), Nil{}}, rt, Nil{}}
        case T.Accepted{+obs, +acts, +next}:
          First{List.append(&2, String, obs_lines(obs),
              Ev.evs(False{}, sess, cmd, res)),
            Rt{next, script, cmds, live}, acts}

def tool_first(+rt: Rt, +cmd: T.Cmd) -> First:
  match rt:
    case Rt{+sess, _, _, _}:
      tool_first.res(rt, sess, cmd, S.apply(sess, cmd))

def turn_on(s: T.Session) -> Bool:
  match s:
    case T.Sess{_, _, _, _, +turn, _, _, _}:
      match turn:
        case None{}:
          False{}
        case Some{_}:
          True{}

def after_first.go(on: Bool, +acts: List<&2, T.Act>, +after: List<&2, T.Act>)
    -> Job:
  match on:
    case True{}:
      JActs{List.append(&2, T.Act, acts, after)}
    case False{}:
      JIdle{}

# the job after the drain: rt is the drained state
def after_first(+rt: Rt, +acts: List<&2, T.Act>, +after: List<&2, T.Act>)
    -> Job:
  match rt:
    case Rt{+sess, _, _, _}:
      after_first.go(turn_on(sess), acts, after)

# one drained steering line: its escaped newlines come back (the same
# wire convention as a socket "steer"), so a multi-line message fits the
# one-line-per-steer file format
def steer_cmd(+l: String) -> T.Cmd:
  T.UserMsg{T.Steer{}, W.unescape_nl(l)}

# the drained content becomes steering lines; blank lines never steer
def drain_steer.push(empty: Bool, +l: String, +rest: List<&2, String>)
    -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      Con{l, rest}

def drain_steer.skip_go(lines: List<&2, String>) -> List<&2, String>:
  match lines:
    case Nil{}:
      Nil{}
    case Con{+l, rest}:
      drain_steer.push(String.is_empty(l), l, drain_steer.skip_go(rest))
