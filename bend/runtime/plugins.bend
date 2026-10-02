# runtime/plugins.bend — agent plugins (Agent Plugins 1.0, the portable
# base). Design: docs/plugins.md.
#
# The REPL cannot hold a stdio MCP server open (Process.run waits for
# the exit), so the Rust side does the work: at live startup this file
# starts `bend-harness plugins serve` in the background, once per REPL.
# The bridge resolves the plugins (~/.agents/plugins and
# <workspace>/.agents/plugins), starts their stdio servers, writes three
# files into the session's plugin dir, then serves the servers over
# loopback HTTP until this REPL is gone:
#   mcp-index.txt    tool lines in the connector index format, the cid
#                    being the loopback URL (runtime/mcp.bend calls it)
#   skills-index.txt skill lines in the skills index format
#   report.txt       the listing /plugins shows
# Scripted runs never start it (hermetic suites).
import Base
import ./persist.bend as P
import ./proc.bend as Proc

# ---- the session's files: $BEND_RUN_DIR/<repl port>/ ----
# The harness exports BEND_RUN_DIR (bise_home: ~/.bise/run, private) and
# creates it 0700; without it (an old harness, a bare REPL), the legacy
# $HOME/.bend-harness/run.

def run_dir.pick(e: Bool, +run: String, +home: String) -> String:
  match e:
    case True{}:
      String.append(home, "/.bend-harness/run")
    case False{}:
      run

def run_dir.of(+run: String, +home: String, +port: String) -> String:
  String.append(run_dir.pick(String.is_empty(run), run, home), String.append("/", port))

def run_dir() -> IO(String):
  do IO<String>:
    rr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_RUN_DIR")
    hr : Result<&1, &1, U32 & String, String> <- IO.get_env("HOME")
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    IO.pure(String, run_dir.of(P.env_or(rr, ""), P.env_or(hr, "/tmp"), P.env_or(pr, "0")))

def dir() -> IO(String):
  do IO<String>:
    +r : String <- run_dir()
    IO.pure(String, String.append(r, "/plugins"))

def mcp_index_path() -> IO(String):
  do IO<String>:
    +d : String <- dir()
    IO.pure(String, String.append(d, "/mcp-index.txt"))

def skills_index_path() -> IO(String):
  do IO<String>:
    +d : String <- dir()
    IO.pure(String, String.append(d, "/skills-index.txt"))

# ---- the start ----
# $1 is the plugin dir. The binary: $BEND_HARNESS_BIN (set by
# bend-harness and by the hub), else the bundle's ./bend-harness, else
# the dev builds. None: no plugins, no error. $PPID is this REPL: the
# wrapper execs the script, so the REPL is its parent. The bridge
# touches ready once its files are written (at most 12 s: a server
# still starting then joins the index later); the wait gives up after
# 15 s.

def start_script() -> String:
  "d=\"$1\"
rm -rf \"$d\"
mkdir -p \"$d\" || exit 0
bin=\"$BEND_HARNESS_BIN\"
[ -n \"$bin\" ] && [ -x \"$bin\" ] || bin=./bend-harness
[ -x \"$bin\" ] || bin=rust/target/release/bend-harness
[ -x \"$bin\" ] || bin=rust/target/debug/bend-harness
[ -x \"$bin\" ] || exit 0
nohup \"$bin\" plugins serve --dir \"$d\" --parent \"$PPID\" > \"$d/bridge.log\" 2>&1 < /dev/null &
i=0
while [ ! -f \"$d/ready\" ] && [ \"$i\" -lt 300 ]; do sleep 0.05; i=$((i+1)); done"

def script_path(+dir: String, +port: String) -> String:
  String.append(dir, String.append("/bend-plugins-start-", String.append(port, ".sh")))

def start() -> IO(Unit):
  do IO<Unit>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +side : String <- P.side_dir()
    +path : String <- IO.pure(String, script_path(side, P.env_or(pr, "0")))
    +d : String <- dir()
    u : Unit <- P.write_file(path, start_script())
    out : String <- Proc.run(Con{"/bin/sh", Con{path, Con{d, Nil{}}}})
    IO.pure(Unit, Unit{})
