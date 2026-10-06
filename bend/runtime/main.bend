# runtime/main.bend — the Harness Runtime loop.
#
# Executes Core-emitted Actions and feeds normalized results back as
# commands (ADR 0006: the Core is driven only through the Step Protocol).
#
# The loop splits in two:
# - transition: a PURE function from (runtime state, job) to (effect, next
#   state, next job). It decides everything: applies commands, picks model
#   replies, runs pure tools.
# - run: the only IO cycle. It performs the effect (print, provider call,
#   tool exec) and recurses with less fuel, so the Runtime terminates
#   like the Core.
#
# Also here: tool dispatch (exec_tool), run_typescript through bend-jsrt
# (live and scripted alike),
# the --resume history replay, and the steering/interrupt side-channels.
# The tools themselves live in their own modules (bash, patch-tool,
# selftools, mcp, skills), the provider call in provider.bend. Live mode
# calls the real provider; scripted mode pops runtime/model.bend replies.
import Base
import ../core/types.bend as T
import ../core/session.bend as S
import ../core/obs.bend as O
import ../core/wire.bend as W
import ../core/compaction.bend as C
import ../core/text.bend as Tx
import ./model.bend as M
import ./tools.bend as X
import ./tools-pure.bend as Xp
import ./persist.bend as P
import ./remote.bend as Rem
import ./mcp.bend as Mcp
import ./skills.bend as Sk
import ./provider.bend as Pv
import ./bash.bend as Sh
import ./bash-pure.bend as Shp
import ./patch-tool.bend as Ptl
import ./edit-tool.bend as Edt
import ./selftools.bend as Sf
import ./selftools-pure.bend as Sfp
import ./proc.bend as Proc
import ../vendor/json.bend as J
import ./main-pure.bend as Rtp
import ../core/edit.bend as Ed
import ./debug.bend as Dbg
import ./drain.bend as Dr
import ./gate-pure.bend as Gp

# where the loop's output goes (stdout, or a REPL socket): Pv.Out
# what the loop hands back when it stops
type RtEnd is Type:
  REnd{sess: T.Session, out: Pv.Out}

# ---- the IO loop ----

def emit_call_ann(out: Pv.Out, +call: T.Call, live: Bool) -> IO(Pv.Out):
  match live:
    case True{}:
      Dr.emit(out, Con{Rtp.call_ann(call), Nil{}})
    case False{}:
      IO.pure(Pv.Out, out)

def emit_res_ann(out: Pv.Out, +id: U32, +r: Xp.ExecRes, live: Bool) -> IO(Pv.Out):
  match live:
    case True{}:
      Dr.emit(out, Con{Rtp.res_ann(id, r), Nil{}})
    case False{}:
      IO.pure(Pv.Out, out)

# ---- the approvals gate (approvals-design.md §10, approvals.md §3) ----
# In `auto` (the mode file the hub writes in the agent's run/ folder), a
# gated call asks the hub first: `gate <n> <json>`, then the runtime polls
# the gate file for the hub's verdict and the interrupt file (ctrl+c,
# `sb interrupt`: the call does not run; the flag stays for
# drain_interrupt, which ends the turn). No time limit. In `yolo`, or with
# no hub (no mode file), nothing asks: one small file read per call.

def gate.rnd(r: Result<&1, &1, U32 & String, U32>) -> U32:
  match r:
    case Done{+x}:
      x
    case Fail{_}:
      0

def gate.step(m: Maybe<&2, Gp.Verdict>, +k: Nat, next: IO(Gp.Verdict)) -> IO(Gp.Verdict):
  match m:
    case Some{+v}:
      IO.pure(Gp.Verdict, v)
    case None{}:
      do IO<Gp.Verdict>:
        IO.sleep(Gp.poll_ms(k))
        next

# @unsafe: waits as long as the user needs (no fuel: no time limit)
@unsafe
def gate.wait(+k: Nat, +gpath: String, +ipath: String, +n: String, +nonce: String)
    -> IO(Gp.Verdict):
  do IO<Gp.Verdict>:
    g : Maybe<&2, String> <- P.read_file(gpath)
    i : Maybe<&2, String> <- P.read_file(ipath)
    gate.step(Gp.found(g, i, n, nonce), k, gate.wait(1n+k, gpath, ipath, n, nonce))

@unsafe
def gate.ask(out: Pv.Out, +name: String, +args: String, +dir: String)
    -> IO(Pv.Out & Gp.Verdict):
  gate.ask_x(out, name, args, dir, "")

# extra: more fields for the gate line's JSON (Gp.ask_line_x)
@unsafe
def gate.ask_x(out: Pv.Out, +name: String, +args: String, +dir: String, +extra: String)
    -> IO(Pv.Out & Gp.Verdict):
  do IO<Pv.Out & Gp.Verdict>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +port : String <- IO.pure(String, P.env_or(pr, "0"))
    +gpath : String <- IO.pure(String, Gp.gate_path(dir, port))
    +ipath : String <- Pv.interrupt_path()
    +now : Nat <- IO.now()
    ra : Result<&1, &1, U32 & String, U32> <- IO.random_u32()
    rb : Result<&1, &1, U32 & String, U32> <- IO.random_u32()
    +nonce : String <- IO.pure(String, Gp.nonce_of(gate.rnd(ra), gate.rnd(rb)))
    +n : String <- IO.pure(String, Nat.show(now))
    u : Unit <- P.write_file(gpath, "")
    out2 : Pv.Out <- Dr.emit(out, [Gp.ask_line_x(n, name, args, nonce, extra)])
    +v : Gp.Verdict <- gate.wait(0n, gpath, ipath, n, nonce)
    out3 : Pv.Out <- Dr.emit(out2, [Gp.done_line(n, Gp.how_of(v))])
    IO.pure(Pv.Out & Gp.Verdict, (out3, v))

@unsafe
def gate.if(asks: Bool, out: Pv.Out, +name: String, +args: String, +dir: String)
    -> IO(Pv.Out & Gp.Verdict):
  match asks:
    case True{}:
      gate.ask(out, name, args, dir)
    case False{}:
      IO.pure(Pv.Out & Gp.Verdict, (out, Gp.VAllow{""}))

# the gate of one call (scripted runs never ask)
@unsafe
def gate(out: Pv.Out, +name: String, +args: String, live: Bool)
    -> IO(Pv.Out & Gp.Verdict):
  match live:
    case False{}:
      IO.pure(Pv.Out & Gp.Verdict, (out, Gp.VAllow{""}))
    case True{}:
      do IO<Pv.Out & Gp.Verdict>:
        +dir : String <- P.side_dir()
        m : Maybe<&2, String> <- P.read_file(Gp.mode_path(dir))
        gate.if(Gp.gated(name) && Gp.mode_asks(m), out, name, args, dir)

# ---- bash under the sandbox (approvals-sandbox, approvals-design.md §6) ----
# An allowed bash call runs under the sandbox its allow names (flags ""
# when none: yolo, the parser path). A run the sandbox stopped asks the
# gate again with its output (the hub: the checker, then a card); an allow
# runs it a second time, without the sandbox; a no keeps the first output
# and adds why.

def bash_gated.after(m: Maybe<&2, String>, out: Pv.Out, +cmd: String,
    +first: String) -> IO(Pv.Out & Xp.ExecRes):
  match m:
    case None{}:
      do IO<Pv.Out & Xp.ExecRes>:
        r : Xp.ExecRes <- Sh.bash_exec_as(cmd, "")
        IO.pure(Pv.Out & Xp.ExecRes, (out, r))
    case Some{+why}:
      IO.pure(Pv.Out & Xp.ExecRes, (out, Xp.Ex{False{}, Gp.stopped_result(first, why)}))

def bash_gated.rerun(pv: Pv.Out & Gp.Verdict, +cmd: String, +first: String)
    -> IO(Pv.Out & Xp.ExecRes):
  match pv:
    case (out, +v):
      bash_gated.after(Gp.denial(v), out, cmd, first)

def bash_gated.out(+r: Xp.ExecRes) -> String:
  match r:
    case Xp.Ex{_, +o}:
      o

@unsafe
def bash_gated.stopped(stop: Bool, out: Pv.Out, +args: String, +cmd: String,
    +r: Xp.ExecRes) -> IO(Pv.Out & Xp.ExecRes):
  match stop:
    case False{}:
      IO.pure(Pv.Out & Xp.ExecRes, (out, r))
    case True{}:
      do IO<Pv.Out & Xp.ExecRes>:
        +dir : String <- P.side_dir()
        pv : Pv.Out & Gp.Verdict <- gate.ask_x(out, "bash", args, dir,
          Gp.denied_extra(bash_gated.out(r)))
        bash_gated.rerun(pv, cmd, bash_gated.out(r))

# args: the call's arguments as the gate line carried them; cmd: the
# command; flags: the allow's (Gp.flags)
@unsafe
def bash_gated(out: Pv.Out, +args: String, +cmd: String, +flags: String)
    -> IO(Pv.Out & Xp.ExecRes):
  do IO<Pv.Out & Xp.ExecRes>:
    +r : Xp.ExecRes <- Sh.bash_exec_as(cmd, flags)
    bash_gated.stopped(Bool.and(Bool.not(String.is_empty(flags)), Shp.sandbox_stopped(r)),
      out, args, cmd, r)

# ---- bend-jsrt: run_typescript in a V8 isolate (the Vibe SDK core pattern) ----
# The model writes real TypeScript/JavaScript; rust/jsrt (deno_core v8 +
# deno_ast) runs it in a FRESH isolate per round with heap limits and a
# watchdog. Tool calls print a request and exit 42; the runtime executes
# the tool through the same exec paths (bash, search, MCP) and RE-RUNS
# the program with the extended results — replay by re-execution, with
# a durable boundary at every tool call. Live and scripted sessions run
# the same engine (BISE-118); the Core only sees the program's one result.

type NR is Data:
  NRDone{out: String}
  NRReq{name: String, args: String}
  NRErr{why: String}

# the recorded results travel as a JSON array of strings
def res_val.go(results: List<&2, String>, +acc: List<&2, J.Json>)
    -> List<&2, J.Json>:
  match results:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case Con{+r, +rest}:
      res_val.go(rest, Con{J.JStr{r}, acc})

def res_val(results: List<&2, String>) -> J.Json:
  J.JArr{res_val.go(results, Nil{})}

def results_json(results: List<&2, String>) -> String:
  J.stringify(res_val(results))

# the request's args render as raw tool-arg text
def args_raw.str(+v: J.Json) -> String:
  match v:
    case J.JStr{+s}:
      s
    case _:
      J.stringify(v)

def args_raw(m: Maybe<&2, J.Json>) -> String:
  match m:
    case None{}:
      "{}"
    case Some{+v}:
      args_raw.str(v)

def node_req.ok(+v: J.Json) -> NR:
  NRReq{Rtp.node_req.tool(J.get(v, "tool")), args_raw(J.get(v, "args"))}

def node_req.res(r: Maybe<&2, J.Json>) -> NR:
  match r:
    case Some{+v}:
      node_req.ok(v)
    case None{}:
      NRErr{"bad tool request from the program"}

def node_req(+body: String) -> NR:
  node_req.res(Rtp.json_m(J.parse(String.trim(body))))

def node_split.zero(z: Bool, +code: U32, +body: String) -> NR:
  match z:
    case True{}:
      NRDone{String.trim(body)}
    case False{}:
      NRErr{String.append("program error (exit ",
        String.append(U32.show(code), ")"))}

def node_split.pick43(is43: Bool, +code: U32, +body: String) -> NR:
  match is43:
    case True{}:
      NRErr{Rtp.node_err(body)}
    case False{}:
      node_split.zero(U32.is_eq(code, 0), code, body)

def node_split.pick42(is42: Bool, +code: U32, +body: String) -> NR:
  match is42:
    case True{}:
      node_req(body)
    case False{}:
      node_split.pick43(U32.is_eq(code, 43), code, body)

def node_split.pick(+code: U32, +body: String) -> NR:
  node_split.pick42(U32.is_eq(code, 42), code, body)

def node_split.code(c: U32, +body: String) -> NR:
  node_split.pick(c, body)

def node_split.read(m: Maybe<&2, U32>, +code: String, +body: String) -> NR:
  match m:
    case Some{+c}:
      node_split.code(c, body)
    case None{}:
      NRErr{"unreadable exit status"}

def node_split.code_str(+code: String, +body: String) -> NR:
  node_split.read(U32.read(code), code, body)

def node_split.some(+p: Nat, +out: String) -> NR:
  node_split.code_str(String.take(out, p),
    String.drop(out, Nat.add(p, 1n)))

def node_split.none(+out: String) -> NR:
  node_split.code_str(out, "")

def node_split.sp(sp: Maybe<&2, Nat>, +out: String) -> NR:
  match sp:
    case Some{+p}:
      node_split.some(p, out)
    case None{}:
      node_split.none(out)

def node_split(sp: Maybe<&2, Nat>, +out: String) -> NR:
  node_split.sp(sp, out)

# the engine: BEND_JSRT_BIN (the harness sets it from its app root), else
# the dev tree's debug build, relative to the app root (the cwd)
def node_cmd(+path: String, +res_path: String, +err_path: String) -> String:
  String.append("\"${BEND_JSRT_BIN:-rust/jsrt/target/debug/bend-jsrt}\" ",
    String.append(path,
      String.append(" ", String.append(res_path,
        String.append(" 2>", err_path)))))

def node_run.go(+prog: String, +results: List<&2, String>, +path: String,
    +res_path: String, +err_path: String) -> IO(NR):
  do IO<NR>:
    u : Unit <- P.write_file(path, prog)
    u2 : Unit <- P.write_file(res_path, results_json(results))
    +out : String <- Proc.run(Con{"/bin/sh", Con{"-c",
      Con{node_cmd(path, res_path, err_path), Nil{}}}})
    IO.pure(NR, node_split(Tx.find(out, "\n", 0n), out))

def node_run(+prog: String, +results: List<&2, String>, +port: String)
    -> IO(NR):
  do IO<NR>:
    +d : String <- P.side_dir()
    node_run.go(prog, results,
      String.append(d, String.append("/bend-prog-", String.append(port, ".ts"))),
      String.append(d, String.append("/bend-res-", String.append(port, ".json"))),
      String.append(d, String.append("/bend-prog-", String.append(port, ".err"))))

# run the program to completion: request -> execute -> record -> re-run
# (one machine, bottom-up; pending decisions ride action constructors)
type EpNext is Data:
  EpDone{out: String}
  EpFail{why: String}
  EpRe{results: List<&2, String>}

type EpAct is Data:
  EpRun{}
  EpNext{nx: EpNext}

def exec_program.decide(r: Xp.ExecRes, +results: List<&2, String>)
    -> EpNext:
  match r:
    case Xp.Ex{+ok, +out}:
      match ok:
        case True{}:
          EpRe{List.append(&2, String, results, Con{out, Nil{}})}
        case False{}:
          EpFail{String.append("program call failed: ", out)}

def exec_program.call.node(is_node: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_node:
    case True{}:
      IO.pure(Xp.ExecRes, Xp.Ex{False{},
        "run_typescript inside a program is not supported"})
    case False{}:
      Sh.exec_mcp(name, args, live)

def exec_program.call.sleep.valid.pick(valid: Bool, +secs: U32)
    -> IO(Xp.ExecRes):
  match valid:
    case True{}:
      do IO<Xp.ExecRes>:
        u : Unit <- IO.sleep(U32.mul(secs, 1000))
        IO.pure(Xp.ExecRes, Xp.Ex{True{},
          String.append("slept ", String.append(U32.show(secs), "s"))})
    case False{}:
      IO.pure(Xp.ExecRes, Xp.Ex{False{}, "sleep requires 0 < seconds < 20"})

def exec_program.call.sleep.valid(+secs: U32) -> IO(Xp.ExecRes):
  exec_program.call.sleep.valid.pick(Rtp.prog_sleep.bounds(secs), secs)

def exec_program.call.sleep(is_sleep: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_sleep:
    case True{}:
      do IO<Xp.ExecRes>:
        +pr : Maybe<&2, J.Json> <- IO.pure(Maybe<&2, J.Json>,
          Rtp.json_m(J.parse(String.trim(args))))
        +secs : U32 <- IO.pure(U32, Rtp.prog_sleep.secs(pr, args))
        exec_program.call.sleep.valid(secs)
    case False{}:
      exec_program.call.node(String.eq(name, "run_typescript"), name, args,
        live)

def exec_program.call.self(is_self: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_self:
    case True{}:
      IO.pure(Xp.ExecRes, Sfp.self_exec(name))
    case False{}:
      exec_program.call.sleep(String.eq(name, "self.sleep"), name, args,
        live)

def exec_program.call.skill(is_skill: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_skill:
    case True{}:
      do IO<Xp.ExecRes>:
        r : Mcp.MR <- Sk.exec_skill(args, live)
        IO.pure(Xp.ExecRes, Sh.exec_mr(r))
    case False{}:
      exec_program.call.self(Sfp.is_self_tool(name), name, args, live)

def exec_program.call.search(is_search: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_search:
    case True{}:
      Sh.exec_search(args, live)
    case False{}:
      exec_program.call.skill(String.eq(name, "skill"), name, args, live)

def exec_program.call.write(is_write: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_write:
    case True{}:
      Edt.exec_write(args, live)
    case False{}:
      exec_program.call.search(String.eq(name, "search_tool_functions"),
        name, args, live)

def exec_program.call.edit(is_edit: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_edit:
    case True{}:
      Edt.exec_edit(args, live)
    case False{}:
      exec_program.call.write(String.eq(name, "write_file"), name, args, live)

def exec_program.call.patch(is_patch: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_patch:
    case True{}:
      Ptl.exec_patch(args, live)
    case False{}:
      exec_program.call.edit(String.eq(name, "edit"), name, args, live)

def exec_program.call.bash(is_bash: Bool, +name: String, +args: String,
    +live: Bool) -> IO(Xp.ExecRes):
  match is_bash:
    case True{}:
      Sh.bash_exec(args)
    case False{}:
      exec_program.call.patch(String.eq(name, "apply_patch"),
        name, args, live)

def emit_sub(out: Pv.Out, +name: String, +r: Xp.ExecRes, live: Bool) -> IO(Pv.Out):
  match live:
    case True{}:
      Dr.emit(out, Con{Rtp.sub_ann(name, r), Nil{}})
    case False{}:
      IO.pure(Pv.Out, out)

# a self.* request filed from inside the program joins the program's
# request set (it will ride the program's result)
def exec_program.call.self_req(is_self: Bool, +name: String, +d: Sfp.Def)
    -> Sfp.Def:
  match is_self:
    case True{}:
      Sfp.def_merge(d, String.eq(name, "self.compact"),
        String.eq(name, "self.reload"))
    case False{}:
      d

# a program's call past its gate: run (bash under the sandbox its allow
# names), or its denial as the result
def exec_program.call.run(is_bash: Bool, out: Pv.Out, +name: String, +args: String,
    +flags: String, +live: Bool) -> IO(Pv.Out & Xp.ExecRes):
  match is_bash:
    case True{}:
      bash_gated(out, args, args, flags)
    case False{}:
      do IO<Pv.Out & Xp.ExecRes>:
        r : Xp.ExecRes <- exec_program.call.bash(False{}, name, args, live)
        IO.pure(Pv.Out & Xp.ExecRes, (out, r))

def exec_program.call.after(m: Maybe<&2, String>, out: Pv.Out, +name: String,
    +args: String, +flags: String, +live: Bool) -> IO(Pv.Out & Xp.ExecRes):
  match m:
    case None{}:
      exec_program.call.run(String.eq(name, "bash"), out, name, args, flags, live)
    case Some{+why}:
      IO.pure(Pv.Out & Xp.ExecRes, (out, Xp.Ex{False{}, why}))

def exec_program.call.gated.emit(ro: Pv.Out & Xp.ExecRes, +name: String, +live: Bool,
    +d: Sfp.Def) -> IO(Pv.Out & (Sfp.Def & Xp.ExecRes)):
  match ro:
    case (out, +r):
      do IO<Pv.Out & (Sfp.Def & Xp.ExecRes)>:
        +d2 : Sfp.Def <- IO.pure(Sfp.Def,
          exec_program.call.self_req(Sfp.is_self_tool(name), name, d))
        out2 : Pv.Out <- emit_sub(out, name, r, live)
        IO.pure(Pv.Out & (Sfp.Def & Xp.ExecRes), (out2, (d2, r)))

def exec_program.call.gated(pv: Pv.Out & Gp.Verdict, +name: String, +args: String,
    +live: Bool, +d: Sfp.Def) -> IO(Pv.Out & (Sfp.Def & Xp.ExecRes)):
  match pv:
    case (out0, +v):
      do IO<Pv.Out & (Sfp.Def & Xp.ExecRes)>:
        ro : Pv.Out & Xp.ExecRes <- exec_program.call.after(Gp.denial(v), out0, name, args,
          Gp.flags(v), live)
        exec_program.call.gated.emit(ro, name, live, d)

@unsafe
def exec_program.call(out: Pv.Out, +name: String, +args: String, +live: Bool,
    +d: Sfp.Def) -> IO(Pv.Out & (Sfp.Def & Xp.ExecRes)):
  do IO<Pv.Out & (Sfp.Def & Xp.ExecRes)>:
    pv : Pv.Out & Gp.Verdict <- gate(out, name, args, live)
    exec_program.call.gated(pv, name, args, live, d)

def exec_program.call.ungated(out: Pv.Out, +name: String, +args: String, +live: Bool,
    +d: Sfp.Def) -> IO(Pv.Out & (Sfp.Def & Xp.ExecRes)):
  do IO<Pv.Out & (Sfp.Def & Xp.ExecRes)>:
    +r : Xp.ExecRes <- exec_program.call.bash(String.eq(name, "bash"),
      name, args, live)
    +d2 : Sfp.Def <- IO.pure(Sfp.Def,
      exec_program.call.self_req(Sfp.is_self_tool(name), name, d))
    out2 : Pv.Out <- emit_sub(out, name, r, live)
    IO.pure(Pv.Out & (Sfp.Def & Xp.ExecRes), (out2, (d2, r)))

# consume the call pair once: the handle rides beside the next step
def exec_program.nr.done(pr: Pv.Out & (Sfp.Def & Xp.ExecRes),
    +results: List<&2, String>) -> Pv.Out & (Sfp.Def & EpNext):
  match pr:
    case (out2, (d2, r)):
      (out2, (d2, exec_program.decide(r, results)))

def exec_program.nr(out: Pv.Out, nr: NR, +prog: String,
    +results: List<&2, String>, +live: Bool, +d: Sfp.Def)
    -> IO(Pv.Out & (Sfp.Def & EpNext)):
  match nr:
    case NRDone{+o}:
      IO.pure(Pv.Out & (Sfp.Def & EpNext), (out, (d, EpDone{o})))
    case NRErr{+why}:
      IO.pure(Pv.Out & (Sfp.Def & EpNext), (out, (d, EpFail{why})))
    case NRReq{+name, +args}:
      do IO<Pv.Out & (Sfp.Def & EpNext)>:
        pr : Pv.Out & (Sfp.Def & Xp.ExecRes) <- exec_program.call(out, name, args,
          live, d)
        IO.pure(Pv.Out & (Sfp.Def & EpNext), exec_program.nr.done(pr, results))

def exec_program.step(pr: Pv.Out & (Sfp.Def & EpNext), k: Nat, +prog: String,
    +results: List<&2, String>, +live: Bool,
    next: Nat -> Pv.Out -> Sfp.Def -> EpNext -> IO(Pv.Out & Xp.ExecRes))
    -> IO(Pv.Out & Xp.ExecRes):
  match pr:
    case (out2, (d2, nx)):
      next(k)(out2)(d2)(nx)

# the program's final value carries the requests it filed: the model
# reads its own acks back in the tool result, and the turn-boundary
# scan finds the markers in the history
def exec_program.done(d: Sfp.Def, +o: String) -> Xp.ExecRes:
  Xp.Ex{True{}, String.append(o, Sfp.def_acks.suffix(d))}

def exec_program.fail(d: Sfp.Def, +why: String) -> Xp.ExecRes:
  Xp.Ex{False{}, String.append(why, Sfp.def_acks.suffix(d))}

@unsafe
def exec_program.go(fuel: Nat, out: Pv.Out, d: Sfp.Def, act: EpAct, +prog: String,
    +results: List<&2, String>, +live: Bool) -> IO(Pv.Out & Xp.ExecRes):
  match fuel:
    case 0n:
      IO.pure(Pv.Out & Xp.ExecRes,
        (out, exec_program.fail(d, "program step budget exhausted")))
    case 1n+k:
      match act:
        case EpRun{}:
          do IO<Pv.Out & Xp.ExecRes>:
            pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
            +port : String <- IO.pure(String, P.env_or(pr, "0"))
            nr : NR <- node_run(prog, results, port)
            por : Pv.Out & (Sfp.Def & EpNext) <- exec_program.nr(out, nr, prog,
              results, live, d)
            exec_program.step(por, k, prog, results, live,
              k2 => o2 => d2 => nx => exec_program.go(k2, o2, d2,
                EpNext{nx}, prog, results, live))
        case EpNext{+nx}:
          match nx:
            case EpDone{+o}:
              IO.pure(Pv.Out & Xp.ExecRes, (out, exec_program.done(d, o)))
            case EpFail{+why}:
              IO.pure(Pv.Out & Xp.ExecRes, (out, exec_program.fail(d, why)))
            case EpRe{+rs}:
              exec_program.go(k, out, d, EpRun{}, prog, rs, live)

# @unsafe: the loop recurses through the step continuation closure
@unsafe
def exec_program(fuel: Nat, pr0: Pv.Out & (Sfp.Def & EpAct), +prog: String,
    +results: List<&2, String>, +live: Bool) -> IO(Pv.Out & Xp.ExecRes):
  match pr0:
    case (out, (d, act)):
      exec_program.go(fuel, out, d, act, prog, results, live)

def exec_tool.with(out: Pv.Out, act: IO(Xp.ExecRes)) -> IO(Pv.Out & Xp.ExecRes):
  do IO<Pv.Out & Xp.ExecRes>:
    r : Xp.ExecRes <- act
    IO.pure(Pv.Out & Xp.ExecRes, (out, r))

# the skill loader: the vibe_sdk exact-name contract, executed by
# reading the SKILL.md the catalog points at
def exec_tool.skill(+args: String, +live: Bool) -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    r : Mcp.MR <- Sk.exec_skill(args, live)
    IO.pure(Xp.ExecRes, Sh.exec_mr(r))

def exec_tool.run(t: Rtp.Tool, out: Pv.Out, +name: String, +args: String,
    +live: Bool) -> IO(Pv.Out & Xp.ExecRes):
  match t:
    case Rtp.TNode{}:
      exec_program(1000n, (out, (Sfp.def_none(), EpRun{})),
        W.call_code(args), Nil{}, live)
    case Rtp.TBash{}:
      exec_tool.with(out, Sh.bash_exec(W.flat_cmd(args)))
    case Rtp.TSearch{}:
      exec_tool.with(out, Sh.exec_search(args, live))
    case Rtp.TSkill{}:
      exec_tool.with(out, exec_tool.skill(args, live))
    case Rtp.TPatch{}:
      exec_tool.with(out, Ptl.exec_patch(args, live))
    case Rtp.TEdit{}:
      exec_tool.with(out, Edt.exec_edit(args, live))
    case Rtp.TWrite{}:
      exec_tool.with(out, Edt.exec_write(args, live))
    case Rtp.TMcp{}:
      exec_tool.with(out, Sh.exec_mcp(name, args, live))

# a call past its gate: run (bash under the sandbox its allow names), or
# its denial as a failed result
def exec_tool.go(is_bash: Bool, out: Pv.Out, +name: String, +args: String,
    +flags: String, +live: Bool) -> IO(Pv.Out & Xp.ExecRes):
  match is_bash:
    case True{}:
      bash_gated(out, args, W.flat_cmd(args), flags)
    case False{}:
      exec_tool.run(Rtp.tool_of(name), out, name, args, live)

def exec_tool.after(m: Maybe<&2, String>, out: Pv.Out, +name: String, +args: String,
    +flags: String, +live: Bool) -> IO(Pv.Out & Xp.ExecRes):
  match m:
    case None{}:
      exec_tool.go(String.eq(name, "bash"), out, name, args, flags, live)
    case Some{+why}:
      IO.pure(Pv.Out & Xp.ExecRes, (out, Xp.Ex{False{}, why}))

def exec_tool.gated(pv: Pv.Out & Gp.Verdict, +name: String, +args: String,
    +live: Bool) -> IO(Pv.Out & Xp.ExecRes):
  match pv:
    case (out, +v):
      exec_tool.after(Gp.denial(v), out, name, args, Gp.flags(v), live)

@unsafe
def exec_tool(out: Pv.Out, +call: T.Call, +live: Bool) -> IO(Pv.Out & Xp.ExecRes):
  match call:
    case T.Call{_, +name, +args}:
      do IO<Pv.Out & Xp.ExecRes>:
        pv : Pv.Out & Gp.Verdict <- gate(out, name, args, live)
        exec_tool.gated(pv, name, args, live)

def emit_code_ann.pick(is_code: Bool, out: Pv.Out, +call: T.Call) -> IO(Pv.Out):
  match is_code:
    case True{}:
      Dr.emit(out, Con{Rtp.code_ann(call), Rtp.intent_anns(call)})
    case False{}:
      IO.pure(Pv.Out, out)

def emit_code_ann(out: Pv.Out, +call: T.Call, live: Bool) -> IO(Pv.Out):
  match live:
    case True{}:
      emit_code_ann.pick(Rtp.is_code_call(Rtp.call_name(call)), out, call)
    case False{}:
      IO.pure(Pv.Out, out)

def harness_info(+model: String, +threshold: String) -> IO(String):
  do IO<String>:
    +sp : String <- Dr.steer_path()
    +ip : String <- Pv.interrupt_path()
    IO.pure(String, Rtp.info_line(model, threshold, sp, ip))

# consumes the exec pair once and hands (handle, cmd) to the loop's
# continuation; the closure passed by run captures its +params and
# self-references run, so no mutual recursion is needed
def exec_done.drained(pd: Pv.Out & Rtp.Rt, k: Nat, +cmd: T.Cmd,
    next: Nat -> Pv.Out -> Rtp.Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match pd:
    case (out5, +rt3):
      next(k)(out5)(rt3)(cmd)

def exec_done(pr: Pv.Out & Xp.ExecRes, k: Nat, +call: T.Call,
    +after: List<&2, T.Act>, +rt2: Rtp.Rt, +live: Bool,
    next: Nat -> Pv.Out -> Rtp.Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match pr:
    case (out3, +r):
      do IO<RtEnd>:
        out4 : Pv.Out <- emit_res_ann(out3, Rtp.call_id(call), r, live)
        +cmd : T.Cmd <- IO.pure(T.Cmd, X.exec_cmd(r, Rtp.call_id(call)))
        pd : Pv.Out & Rtp.Rt <- Dr.drain_steer(out4, rt2)
        exec_done.drained(pd, k, cmd, next)

# the same boundary for a model result: the completion is applied on the
# drained state, so steering that arrived during the call is already in
# the turn and lands in the next model input
def model_done(pd: Pv.Out & Rtp.Rt, k: Nat, +res: Rem.RemoteRes, +action: U32,
    +after: List<&2, T.Act>,
    next: Nat -> Pv.Out -> Rtp.Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match pd:
    case (out3, +rt3):
      do IO<RtEnd>:
        +cmd : T.Cmd <- IO.pure(T.Cmd, Rem.remote_cmd(res, action))
        next(k)(out3)(rt3)(cmd)

# the checkpoint before every provider call: the history is coherent
# there (every tool result is in, the completion is not asked yet), so
# a process that dies during the call - or anywhere later in the hop -
# restarts on everything the turn had done so far (the checkpoint text
# carries no turn and no pending: the restored session is idle)
def save_rt(+rt: Rtp.Rt) -> IO(Unit):
  match rt:
    case Rtp.Rt{+sess, _, _, _}:
      P.save(sess)

def model_back(nr: Pv.Out & Rem.RemoteRes, +rt2: Rtp.Rt, k: Nat, +action: U32,
    +after: List<&2, T.Act>,
    next: Nat -> Pv.Out -> Rtp.Rt -> T.Cmd -> IO(RtEnd)) -> IO(RtEnd):
  match nr:
    case (n, +res):
      do IO<RtEnd>:
        pd : Pv.Out & Rtp.Rt <- Dr.drain_steer(n, rt2)
        model_done(pd, k, res, action, after, next)

# one edit toolset per request (main-pure.bend retool): the provider
# this call goes to picks it; a catalog with no edit tool (scripted
# scenarios) is left alone
def retool_now.want(same: Bool, +was: Bool, +tools: List<&2, T.ToolDef>,
    +request: String) -> IO(String):
  match same:
    case True{}:
      IO.pure(String, request)
    case False{}:
      do IO<String>:
        +cat : List<&2, T.ToolDef> <- X.catalog_for(Bool.not(was))
        IO.pure(String, Rtp.retool(request, tools, cat, Ed.prompt_line(was),
          Ed.prompt_line(Bool.not(was))))

def retool_now.set(m: Maybe<&2, Bool>, +tools: List<&2, T.ToolDef>,
    +request: String) -> IO(String):
  match m:
    case None{}:
      IO.pure(String, request)
    case Some{+was}:
      do IO<String>:
        +model : String <- Pv.resolved_model()
        retool_now.want(Bool.not(Bool.xor(was, Ed.is_patch_provider(Ed.provider_of(model)))),
          was, tools, request)

def retool_now(+tools: List<&2, T.ToolDef>, +request: String) -> IO(String):
  retool_now.set(Xp.toolset(tools), tools, request)

# The IO loop. The pure transition is computed as an argument and matched
# as a parameter, so `run` is a single self-recursive def; every hop burns
# one unit of fuel, so the Runtime terminates like the Core. The loop
# returns the final session and output handle, so callers (the scenarios,
# the REPL) can continue from where it stopped.
# @unsafe: the EExec branch recurses through the exec_done continuation,
# and the EModel branch through model_done (both carry the drained state)
@unsafe
def run(fuel: Nat, out: Pv.Out, +rt: Rtp.Rt, +job: Rtp.Job, tr: Rtp.Trans) -> IO(RtEnd):
  match fuel:
    case 0n:
      match rt:
        case Rtp.Rt{+sess, _, _, _}:
          IO.pure(RtEnd, REnd{sess, out})
    case 1n+k:
      match rt:
        case Rtp.Rt{+sess, _, _, _}:
          match job:
            case Rtp.JDone{}:
              IO.pure(RtEnd, REnd{sess, out})
            case _:
              match tr:
                case Rtp.Tr{effect, +rt2, +job2}:
                  match effect:
                    case Rtp.ENone{}:
                      run(k, out, rt2, job2, Rtp.transition(rt2, job2))
                    case Rtp.EDequeue{}:
                      run(k, out, rt2, job2, Rtp.transition_queued(rt2, job2))
                    case Rtp.EPrints{lines}:
                      do IO<RtEnd>:
                        out2 : Pv.Out <- Dr.emit(out, lines)
                        run(k, out2, rt2, job2, Rtp.transition(rt2, job2))
                    case Rtp.EModel{+action, +request, +after}:
                      # the real provider call (the hub HTTP client);
                      # the side channel is drained before the result is
                      # applied, so steering sent during the call enters
                      # the turn at this safe boundary
                      do IO<RtEnd>:
                        sv : Unit <- save_rt(rt2)
                        +ctx : String <- context_now()
                        +req2 : String <- retool_now(Rtp.rt_tools(rt2), request)
                        nr : Pv.Out & Rem.RemoteRes <- Pv.model_call.req(
                          out, Rem.with_context(ctx, req2), action)
                        model_back(nr, rt2, k, action, after,
                          k2 => o3 => +r3 => +c =>
                            run(k2, o3, r3, Rtp.JCmd{c, after},
                              Rtp.transition(r3, Rtp.JCmd{c, after})))
                    case Rtp.EExec{+call, +after, +live}:
                      # the real tool: bash, the merged search, or a
                      # remote MCP call resolved by name. The annotations
                      # carry name/args/result to the TUI (live only);
                      # program sub-calls annotate inside exec_program.
                      do IO<RtEnd>:
                        out1 : Pv.Out <- emit_call_ann(out, call, live)
                        out2 : Pv.Out <- emit_code_ann(out1, call, live)
                        pr : Pv.Out & Xp.ExecRes <- exec_tool(out2, call, live)
                        exec_done(pr, k, call, after, rt2, live,
                          k2 => o4 => +r3 => +c =>
                            run(k2, o4, r3, Rtp.JCmd{c, after},
                              Rtp.transition(r3, Rtp.JCmd{c, after})))

# BEND_CONTEXT_FILE: the ephemeral context, re-read before every model
# call (runtime/remote.bend with_context); unset or missing: ""
def context_now.read(empty: Bool, +path: String) -> IO(String):
  match empty:
    case True{}:
      IO.pure(String, "")
    case False{}:
      P.read_or(path, "")

def context_now() -> IO(String):
  do IO<String>:
    cr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_CONTEXT_FILE")
    +path : String <- IO.pure(String, P.env_or(cr, ""))
    context_now.read(String.is_empty(path), path)

# ---- scenarios (ADR 0006: the Core tested through the Step Protocol) ----

def sys_prompt() -> String:
  "You are a precise assistant running on the Bend Unified Harness."

def cfg(threshold: Nat, max_nulls: Nat) -> T.Cfg:
  T.Cfg{threshold, 40n, max_nulls, sys_prompt(), X.catalog_short()}
