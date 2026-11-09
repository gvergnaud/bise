# tests/mcp_retry.py's probe: one MCP tool call through runtime/mcp.bend's
# mcp_call ($PROBE_TOOL with $PROBE_ARGS, the index where the session's
# plugins bridge writes it: $BEND_RUN_DIR/$BEND_REPL_PORT/plugins/mcp-index.txt),
# then prints "ok" or "err" on the first line and the answer after it.
# $PROBE_KEY: the Mistral key a gateway connector's call sends ("" when unset).
import Base
import ../bend/runtime/mcp.bend as Mcp
import ../bend/runtime/mcp-pure.bend as Mcpp
import ../bend/runtime/main-pure.bend as Rtp
import ../bend/runtime/tools-pure.bend as Xp

def env_of(r: Result<&1, &1, U32 & String, String>) -> String:
  match r:
    case Done{+v}:
      v
    case Fail{_}:
      ""

# a labeled call: the runtime's own sub-call line first (Rtp.sub_ann_x,
# what a call inside run_typescript writes: `subtool <name> ok as <l> : …`)
def labeled(+tool: String, +l: String, m: Mcp.MR) -> Mcp.MR:
  match m:
    case Mcp.MR{+ok, +out}:
      Mcp.MR{ok, Bool.pick(String, String.is_empty(l), out,
        String.concat([Rtp.sub_ann_x(tool, l, Xp.Ex{ok, out}), "\n", out]))}

# a refused account: its line, no call; else the call with the resolved
# account, and its label first (`label <l>`, none when empty)
def call(p: Mcpp.CallPick, +tool: String, +args: String, +key: String) -> IO(Mcp.MR):
  match p:
    case Mcpp.CPRefuse{+why}:
      IO.pure(Mcp.MR, Mcp.MR{False{}, why})
    case Mcpp.CPGo{+n, +l}:
      do IO<Mcp.MR>:
        m : Mcp.MR <- Mcp.mcp_call(tool, Mcp.pick_args(Mcpp.CPGo{n, l}, args), key)
        IO.pure(Mcp.MR, labeled(tool, l, m))

def show(m: Mcp.MR) -> String:
  match m:
    case Mcp.MR{+ok, +out}:
      String.append(Bool.pick(String, ok, "ok", "err"),
        String.append("\n", out))

def main() -> IO(Unit):
  do IO<Unit>:
    tr : Result<&1, &1, U32 & String, String> <- IO.get_env("PROBE_TOOL")
    ar : Result<&1, &1, U32 & String, String> <- IO.get_env("PROBE_ARGS")
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("PROBE_KEY")
    # the account resolved once above the call, as main.bend's gate step
    +tool : String <- IO.pure(String, env_of(tr))
    +args : String <- IO.pure(String, env_of(ar))
    p : Mcpp.CallPick <- Mcp.call_account(tool, args)
    m : Mcp.MR <- call(p, tool, args, env_of(kr))
    IO.print(show(m))
