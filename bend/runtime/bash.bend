# runtime/bash.bend — the bash tool.
#
# A command runs inside a generated /bin/sh wrapper: a sync window, then
# a background handoff with a file contract (slot dir, out, rc, stdin
# fifo). The contract text is pure and pinned by LAWS.bend; bash_exec
# is the one effect.
import Base
import ../core/text.bend as Tx
import ./tools.bend as X
import ./tools-pure.bend as Xp
import ./persist.bend as P
import ./mcp.bend as Mcp
import ./proc.bend as Proc
import ./settings.bend as St
import ./bash-pure.bend as Shp
import ./provider-pure.bend as Pvp
def bg_after.env(+envv: String, +dflt: U32) -> U32:
  Shp.bg_after.read(U32.read(envv), dflt)

# env > config > default; the config file only feeds LIVE sessions
# (BEND_REPL_PORT is the live marker - the scripted suites stay
# hermetic against the user's config)
def bg_after.pick(port_empty: Bool, +envv: String) -> IO(U32):
  match port_empty:
    case True{}:
      IO.pure(U32, Shp.bg_after.clamp(bg_after.env(envv, 30)))
    case False{}:
      do IO<U32>:
        +cfgt : String <- St.read_cfg()
        IO.pure(U32, Shp.bg_after.clamp(St.env_cfg_u32(envv, cfgt, "bg_after", 30)))

def bg_after() -> IO(U32):
  do IO<U32>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_BG_AFTER")
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    bg_after.pick(String.is_empty(P.env_or(pr, "")), P.env_or(r, ""))

# BEND_BG_DIR (the hub: the agent's tmp/bg), else <BEND_BG_ROOT or
# $TMPDIR or /tmp>/bend-bg-<port> (Shp.bg_dir_pick)
def bg_dir() -> IO(String):
  do IO<String>:
    dr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_BG_DIR")
    root : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_BG_ROOT")
    tr : Result<&1, &1, U32 & String, String> <- IO.get_env("TMPDIR")
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    IO.pure(String, Shp.bg_dir_pick(P.env_or(dr, ""), P.env_or(root, ""),
      P.env_or(tr, ""), P.env_or(pr, "0")))

# flags: the gate's allow flags ("sandbox", "sandbox net", or "" for a
# plain run, Shp.sandbox_profile); the profiles sit in the side dir
@unsafe
def bash_exec(command: String) -> IO(Xp.ExecRes):
  bash_exec_as(command, "")

@unsafe
def bash_exec_as(command: String, +flags: String) -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    +secs : U32 <- bg_after()
    +bg : String <- bg_dir()
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +port : String <- IO.pure(String, P.env_or(pr, "0"))
    # NO flatten: real newlines stay — the heredoc and the runner
    # subshell both accept multi-line commands (flatten silently
    # restructured them)
    wr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WORKDIR")
    +side : String <- P.side_dir()
    # the interrupt flag: a stop cuts the sync command (Shp.bg_stop_check)
    +script : String <- IO.pure(String,
      Shp.bg_wrap(Shp.with_workdir(P.env_or(wr, ""), String.trim(command)),
        bg, secs, Pvp.interrupt_path_of(side, port)))
    +path : String <- IO.pure(String, Shp.bash_script_path(side, port, script))
    w : Unit <- P.write_file(path, script)
    out : String <- Proc.run(Shp.sh_argv(Shp.sandbox_profile(side, flags), path))
    IO.pure(Xp.ExecRes, Shp.exec_of(out))

# boot cleanup: the wrappers of a previous process on this port cannot be
# running (the parent waits for the old REPL to die before respawning), so
# stale <side dir>/bend-sh-<port>-*.sh files are safe to remove - the
# wrapper leak (one file per bash call) would otherwise grow without bound.
def bash_cleanup(+port: String) -> IO(Unit):
  do IO<Unit>:
    +side : String <- P.side_dir()
    out : String <- Proc.run(Con{"/bin/sh", Con{"-c",
      Con{Shp.bash_cleanup_cmd(side, port), Nil{}}}})
    IO.pure(Unit, Unit{})

def exec_mr(m: Mcp.MR) -> Xp.ExecRes:
  match m:
    case Mcp.MR{+ok, +out}:
      Xp.Ex{ok, out}

def exec_search(+args: String, +live: Bool) -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    r : Mcp.MR <- Mcp.merged_search(args, live)
    IO.pure(Xp.ExecRes, exec_mr(r))

def exec_mcp.key(+name: String, +args: String, +key: String)
    -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    r : Mcp.MR <- Mcp.mcp_call(name, args, key)
    IO.pure(Xp.ExecRes, exec_mr(r))

def exec_mcp.scripted(+name: String) -> Xp.ExecRes:
  Xp.Ex{False{},
    String.append("unknown tool: ", String.append(name,
      " — remote MCP tools need a live session"))}

def exec_mcp.live(+name: String, +args: String) -> IO(Xp.ExecRes):
  do IO<Xp.ExecRes>:
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("MISTRAL_API_KEY")
    +key : String <- IO.pure(String, P.env_or(kr, ""))
    exec_mcp.key(name, args, key)

def exec_mcp(+name: String, +args: String, live: Bool) -> IO(Xp.ExecRes):
  match live:
    case True{}:
      exec_mcp.live(name, args)
    case False{}:
      IO.pure(Xp.ExecRes, exec_mcp.scripted(name))
