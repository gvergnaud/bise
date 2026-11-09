# tests/mcp_retry.py's probe: one MCP tool call through runtime/mcp.bend's
# mcp_call ($PROBE_TOOL with $PROBE_ARGS, the index where the session's
# plugins bridge writes it: $BEND_RUN_DIR/$BEND_REPL_PORT/plugins/mcp-index.txt),
# then prints "ok" or "err" on the first line and the answer after it.
# $PROBE_KEY: the Mistral key a gateway connector's call sends ("" when unset).
import Base
import ../bend/runtime/mcp.bend as Mcp

def env_of(r: Result<&1, &1, U32 & String, String>) -> String:
  match r:
    case Done{+v}:
      v
    case Fail{_}:
      ""

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
    m : Mcp.MR <- Mcp.mcp_call(env_of(tr), env_of(ar), env_of(kr))
    IO.print(show(m))
