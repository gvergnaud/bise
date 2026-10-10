# runtime/bash-pure.bend - the pure half of runtime/bash.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base
import ../core/text.bend as Tx
import ./tools-pure.bend as Xp

# ---- the bash tool, fully in Bend (runtime/proc.bend) ----
# Proc.run runs /bin/sh on the wrapper; the answer is the exit status on
# the first line, then stdout and stderr. Commands are flattened first (the wire
# already flattens call args — double safety).

def exec_of.pick.fail(+code: U32, +output: String) -> Xp.ExecRes:
  Xp.Ex{False{}, String.append("exit ",
    String.append(U32.show(code), String.append(": ", output)))}

# the handoff marker: the wrapper prints it as the very first line of
# its output, before the contract. The exit code is no longer the
# signal (a command that really exits 199 must report 199); only the
# wrapper can print this line first — a command's output is only
# cat'd after the marker would have been printed.
def HANDOFF_MARKER() -> String:
  "BEND-BG-HANDOFF-6f2c9e5b"

def exec_of.handoff.rest(sp: Maybe<&2, Nat>, +payload: String) -> Xp.ExecRes:
  match sp:
    case None{}:
      Xp.Ex{True{}, payload}
    case Some{+p}:
      Xp.Ex{True{}, String.drop(payload, Nat.add(p, 1n))}

def exec_of.handoff(+payload: String) -> Xp.ExecRes:
  # strip the marker line; keep the contract
  exec_of.handoff.rest(Tx.find(payload, "\n", 0n), payload)

def exec_of.pick.is_handoff(is_handoff: Bool, +code: U32,
    +output: String) -> Xp.ExecRes:
  match is_handoff:
    case True{}:
      exec_of.handoff(output)
    case False{}:
      exec_of.pick.fail(code, output)

# exit 0 without the marker is a plain success
def exec_of.pick.zero.ok(is_handoff: Bool, +output: String) -> Xp.ExecRes:
  match is_handoff:
    case True{}:
      exec_of.handoff(output)
    case False{}:
      Xp.Ex{True{}, output}

def exec_of.pick.zero(is_zero: Bool, +code: U32, +output: String) -> Xp.ExecRes:
  match is_zero:
    case True{}:
      exec_of.pick.zero.ok(
        String.starts_with(output, HANDOFF_MARKER()), output)
    case False{}:
      exec_of.pick.is_handoff(
        String.starts_with(output, HANDOFF_MARKER()), code, output)

def exec_of.pick(+code: U32, +output: String) -> Xp.ExecRes:
  exec_of.pick.zero(U32.is_eq(code, 0), code, output)

def exec_of.code.read(m: Maybe<&2, U32>, +output: String) -> Xp.ExecRes:
  match m:
    case Some{+c}:
      exec_of.pick(c, output)
    case None{}:
      exec_of.pick(1, output)

def exec_of.code(code_str: String, +output: String) -> Xp.ExecRes:
  exec_of.code.read(U32.read(code_str), output)

def exec_of.split(sp: Maybe<&2, Nat>, +out: String) -> Xp.ExecRes:
  match sp:
    case Some{+p}:
      exec_of.code(String.take(out, p), String.drop(out, Nat.add(p, 1n)))
    case None{}:
      exec_of.code(out, "")

def exec_of(+out: String) -> Xp.ExecRes:
  exec_of.split(Tx.find(out, "\n", 0n), out)

# ---- bash: a sync window, then a background handoff ----
# A command that outlives the sync window never blocks the turn: it keeps
# running detached under a per-id directory that the agent drives with
# ORDINARY bash — tail the .out, write the .in fifo, read the .rc, kill
# the .pid. The command's output is a FILE, never a pipe, so no orphan
# child can ever hold the tool's read open (the bug class that once made
# every command cost the full timeout is structurally gone).
#
# Wrapper shape (bg_wrap below):
#   - id: the first free slot (no .pid) in the session's bg dir
#   - stdin: /dev/null, EOF at once. A command that reads stdin when it
#     is not a tty (rg or grep -r with no path, cat) used to get a fifo
#     that never closed and waited out the whole sync window (issue #9:
#     ~35 s a turn). A command that wants input later reads the fifo
#     by name: `cmd < "$BEND_STDIN"` (the slot's <id>.in). The runner
#     holds the fifo RDWR on fd 9 (never EOF, the open never blocks)
#     and the command does not inherit fd 9; the agent feeds it with
#     printf > <id>.in
#   - a detached RUNNER subshell is the command's real parent (a sibling
#     cannot wait: 127); it records the pid, waits, writes the exit
#     code to <id>.rc and exits with it — the wrapper polls the RUNNER
#     (its own child, $!) and takes the code from `wait`, never from a
#     file: a file the runner could not write (full disk, read-only
#     dir) can never hang the wrapper
#   - within BEND_BG_AFTER seconds (default 30): the tool returns the
#     output and the exit code, and cleans the slot
#   - past the window: the wrapper prints the driving contract and
#     exits 199, which exec_of maps to a SUCCESSFUL result — the agent
#     keeps thinking and interacts through further bash calls

def bg_after.read(m: Maybe<&2, U32>, +dflt: U32) -> U32:
  match m:
    case Some{+secs}:
      secs
    case None{}:
      dflt

def bg_after.clamp2(over: Bool, +secs: U32) -> U32:
  match over:
    case True{}:
      3600
    case False{}:
      secs

def bg_after.clamp(+secs: U32) -> U32:
  bg_after.clamp2(U32.is_lt(3600, secs), secs)

# the session's bg dir, pure: <root>/bend-bg-<port>. One dir per REPL
# process (keyed by port) so parallel terminals never share slots,
# and a /reload respawns on the same port and finds its slots again.
def bg_dir_of(+root: String, +port: String) -> String:
  String.append(root, String.append("/bend-bg-", port))

# ---- the agent's own folders (approvals-tmp, approvals-design.md §7.1) ----
# The harness writes nothing in /tmp. Its own files (the bash wrappers,
# steer, interrupt, the run_typescript files, the plugins and skills
# scripts) go to the "side dir": BEND_AGENT_RUN (the hub sets it to
# ~/.bise/hubs/<hub>/agents/<agent>/run), else $TMPDIR (a headless run:
# tests, bench, a bare repl-live), else /tmp. The names keep the port, so
# two headless runs sharing a TMPDIR never share a file.

# a folder path without its trailing "/" ($TMPDIR ends with one on macOS)
def dir_trim.go(+s: String, e: Bool) -> String:
  match e:
    case True{}:
      String.take(s, Nat.sub(String.length(s), 1n))
    case False{}:
      s

def dir_trim(+s: String) -> String:
  dir_trim.go(s, Bool.and(String.ends_with(s, "/"),
    Bool.not(String.eq(s, "/"))))

def side_dir_of.tmp(e: Bool, +tmp: String) -> String:
  match e:
    case True{}:
      "/tmp"
    case False{}:
      dir_trim(tmp)

def side_dir_of.pick(e: Bool, +run: String, +tmp: String) -> String:
  match e:
    case True{}:
      side_dir_of.tmp(String.is_empty(tmp), tmp)
    case False{}:
      dir_trim(run)

# run: BEND_AGENT_RUN, tmp: TMPDIR ("" when unset)
def side_dir_of(+run: String, +tmp: String) -> String:
  side_dir_of.pick(String.is_empty(run), run, tmp)

# the bg dir: BEND_BG_DIR as is (the hub: the agent's tmp/bg, where the
# agent may read its jobs' output), else <BEND_BG_ROOT or TMPDIR or
# /tmp>/bend-bg-<port>
def bg_dir_pick.go(e: Bool, +dir: String, +root: String, +tmp: String,
    +port: String) -> String:
  match e:
    case True{}:
      bg_dir_of(side_dir_of(root, tmp), port)
    case False{}:
      dir_trim(dir)

def bg_dir_pick(+dir: String, +root: String, +tmp: String, +port: String)
    -> String:
  bg_dir_pick.go(String.is_empty(dir), dir, root, tmp, port)

# ---- the agent-facing contract, pure ----
# Every driving line below is a COMPLETE shell command on a CONCRETE
# path: the agent copy-pastes it verbatim, no variables to resolve.
# The wrapper prints exactly these (its $i slot variable expanded at
# runtime), so what the laws pin is what the agent reads.

# ---- event-wake: a handoff tells the hub (bash.bend bg_note) ----
# The wrapper's raw output (`<code>\n<marker>\n<headline>...`) of a
# handoff names its slot in the headline (`... - id <i> (dir <bg>)`);
# None for a command that ended in its window.
def handoff_id.close(q: Maybe<&2, Nat>, +rest: String) -> Maybe<&2, String>:
  match q:
    case None{}:
      None{}
    case Some{+n}:
      Some{String.take(rest, n)}

def handoff_id.at(p: Maybe<&2, Nat>, +body: String) -> Maybe<&2, String>:
  match p:
    case None{}:
      None{}
    case Some{+n}:
      +rest = String.drop(body, Nat.add(n, 6n))
      handoff_id.close(Tx.find(rest, " (dir ", 0n), rest)

def handoff_id.body(hand: Bool, +body: String) -> Maybe<&2, String>:
  match hand:
    case False{}:
      None{}
    case True{}:
      handoff_id.at(Tx.find(body, " - id ", 0n), body)

def handoff_id.nl(sp: Maybe<&2, Nat>, +out: String) -> Maybe<&2, String>:
  match sp:
    case None{}:
      None{}
    case Some{+p}:
      +body = String.drop(out, Nat.add(p, 1n))
      handoff_id.body(String.starts_with(body, HANDOFF_MARKER()), body)

def handoff_id(+out: String) -> Maybe<&2, String>:
  handoff_id.nl(Tx.find(out, "
", 0n), out)

# the command's first line, for the hub's words (at most 200 chars)
def first_line.at(sp: Maybe<&2, Nat>, +s: String) -> String:
  match sp:
    case None{}:
      String.take(s, 200n)
    case Some{+p}:
      String.take(String.take(s, p), 200n)

def first_line(+s: String) -> String:
  first_line.at(Tx.find(s, "
", 0n), String.trim(s))

# the line under the contract, when a hub watches the slot `id`
# (designer m_16533; `sb wake --stop bg/<id>` finds the slot's watch)
def handoff_note(+id: String) -> String:
  String.append("  its end wakes you with its rc and its last 20 lines, no timer needed. sb wake lists your watches; sb wake --stop bg/",
    String.append(id, " cancels this one."))

# one slot's file: <dir>/<id><ext>
def bg_slot(+bg: String, +id: String, +ext: String) -> String:
  String.append(bg, String.append("/", String.append(id, ext)))

def bg_headline(+bg: String, +id: String, +secs: U32) -> String:
  String.append("background: still running after ",
    String.append(U32.show(secs),
      String.append("s - id ",
        String.append(id, String.append(" (dir ", String.append(bg, ")"))))))

def bg_tail_cmd(+bg: String, +id: String) -> String:
  String.append("  output: tail -n 50 ", bg_slot(bg, id, ".out"))

def bg_stdin_cmd(+bg: String, +id: String) -> String:
  String.append("  stdin (only if the command reads the fifo in BEND_STDIN): printf '%s\\n' 'text' > ",
    String.append(bg_slot(bg, id, ".in"),
      "  (otherwise its stdin is /dev/null)"))

def bg_status_cmd(+bg: String, +id: String) -> String:
  String.append("  status: kill -0 $(cat ",
    String.append(bg_slot(bg, id, ".pid"),
      String.append(") && echo RUNNING || cat ",
        String.append(bg_slot(bg, id, ".rc"), " 2>/dev/null"))))

def bg_kill_cmd(+bg: String, +id: String) -> String:
  String.append("  kill: kill $(cat ",
    String.append(bg_slot(bg, id, ".pid"), ")"))

# the vanished-dir guard: if the agent deletes the bg dir under a
# running wrapper (a legitimate cleanup), the tool fails with a clear
# message instead of cat'ing files that are gone
def bg_rc_guard(+bg: String) -> String:
  String.append("if [ ! -d ", String.append(bg,
    String.append(" ]; then echo \"background: the bg dir vanished before the exit code was recorded: ",
      String.append(bg, "\"; rm -f \"$0\"; exit 1; fi"))))

# slot allocation is bounded: mkdir failing on an EXISTING slot moves to
# the next id; failing for any other reason (full disk, read-only or
# uncreatable bg dir) 20 times ends the call with this message (once:
# the loop retried forever)
def bg_slot_fail(+bg: String) -> String:
  String.append("  if [ $f -ge 20 ]; then echo \"background: cannot create a slot in ",
    String.append(bg, " (disk full or not writable?)\"; rm -f \"$0\"; exit 1; fi"))

# the .cmd write can fail too (full disk): never run a truncated command
def bg_cmd_fail(+bg: String) -> String:
  String.append("if [ $? -ne 0 ]; then echo \"background: could not write ",
    String.append(bg_slot(bg, "$i", ".cmd"),
      " (disk full?)\"; rm -f $BG/$i.in $BG/$i.cmd; rmdir $BG/$i.slot 2>/dev/null; rm -f \"$0\"; exit 1; fi"))

# a slot file the runner could not write: the call still returns, and
# says why a file is missing (the output may be cut short)
def bg_write_note(+test: String, +bg: String, +ext: String, +why: String)
    -> String:
  String.append("if [ ", String.append(test, String.append(" ",
    String.append(bg_slot(bg, "$i", ext),
      String.append(" ]; then echo \"background: could not write ",
        String.append(bg_slot(bg, "$i", ext),
          String.append(why, "\"; fi")))))))

# The sync window is a CLOCK: a `sleep <after>` timer the wrapper starts
# beside the runner; the window ends when the timer does. (It was a
# count of poll turns, after * 10 + 5, each one a `sleep 0.1` plus the
# exec of /bin/sleep: under load, nice 10 and the EDR's exec cost a
# turn took 120-140 ms, a 6 s window lasted 7.6-8.4 s and a 30 s one
# 34-40 s, so an 8 s job came back whole in a 6 s window.) The poll
# itself stays: 5 turns at 20 ms then 100 ms steps, so a trivial
# command costs ~30 ms. The timer is the wrapper's child, outside the
# command's process group (set -m is in the runner): the memory
# watchdog never sees it, and killing it never touches the command.
# It never outlives the wrapper: killed on a sync end, and by the trap
# when the wrapper is stopped (HUP, INT, TERM); at the handoff it has
# already ended. Its output goes nowhere (it holds no pipe of the
# tool's), and it drops the fifo's fd 9.
def bg_timer(+after: U32) -> String:
  String.append("sleep ", String.append(U32.show(after),
    " >/dev/null 2>&1 </dev/null 9<&- & t=$!"))

def bg_timer_trap() -> String:
  "trap 'kill $t 2>/dev/null; exit 143' HUP INT TERM"

# Every wait is TOTAL: the wrapper polls its own child, the runner
# ($!), and reads the exit code from `wait`, so it never waits for a
# file. (Once: on a full disk the runner could not write the .rc, the
# wrapper waited for it forever, and a bash call hung 19 minutes.) The
# .pid is still written for the agent's status/kill lines.
# The wrapper goes to a file (the node_run pattern) and sh runs the
# file: the command never travels as an argument.
#
# The file is named by a hash of its OWN content: two different
# commands never share a wrapper file, so an orphaned wrapper (a
# process that died mid-command, still sleeping in its poll loop)
# can never re-read ANOTHER command's bytes when it wakes up. Re-running
# the same command rewrites identical bytes, which is safe: the orphan
# just continues its own script.
# FNV-1a over the code points, in U32: the arithmetic wraps mod 2^32
# by construction, so no script length can overflow. (The former
# positional sum c * (pos^2 + 1) was a Nat growing as length^3: it hit
# the runtime's 2^48-1 ceiling - "bend: a Nat past the largest
# immediate" - on a ~20k-char ASCII script, far sooner with accented
# text, and every long heredoc the model wrote killed the REPL.) The
# hash stays a U32 end to end: a large Nat is also slow to normalize in
# the laws. Fuel-first (the codebase idiom).
def bg_script_hash.go(fuel: Nat, +s: String, +h: U32) -> U32:
  match fuel:
    case 0n:
      h
    case 1n+k:
      match s:
        case SNil{}:
          h
        case SCon{Chr{+c}, t}:
          bg_script_hash.go(k, t, U32.mul(U32.xor(h, c), 16777619))

def bg_script_hash(+s: String) -> U32:
  bg_script_hash.go(Nat.add(String.length(s), 1n), s, 2166136261)

# the .cmd heredoc sentinel derives from the command's own hash: a
# command containing the sentinel would have to contain its own hash
def bg_sentinel(+command: String) -> String:
  String.append("XQ-", String.append(U32.show(bg_script_hash(command)),
    "-EOF"))

def bash_script_path(+dir: String, +port: String, +script: String)
    -> String:
  String.append(dir, String.append("/bend-sh-", String.append(port,
    String.append("-", String.append(U32.show(bg_script_hash(script)), ".sh")))))

# the boot cleanup of a previous process's wrappers on this port (the
# folder quoted: it may hold a space)
def bash_cleanup_cmd(+dir: String, +port: String) -> String:
  String.append("rm -f ", String.append(Tx.sh_quote(dir),
    String.append("/bend-sh-", String.append(port, "-*.sh"))))

# ---- BEND_WORKDIR: where the command runs ----
# A switchboard agent runs its REPL from the app root (the runtime's
# relative paths live there) but works in its own directory: the
# workspace, or a task's git worktree. The command then starts with a
# cd into it; a missing directory fails the command instead of running
# it somewhere else. Empty: the REPL's own directory, unchanged.
def with_workdir.pick(empty: Bool, +dir: String, +command: String)
    -> String:
  match empty:
    case True{}:
      command
    case False{}:
      String.append("cd ", String.append(Tx.sh_quote(dir),
        String.append(" || exit 1\n", command)))

def with_workdir(+dir: String, +command: String) -> String:
  with_workdir.pick(String.is_empty(dir), dir, command)

# ---- the REPL's private environment stays in the REPL ----
# The hub (and the harness) hand the REPL its session file, wire log,
# context file, port...: they belong to THIS process. A command that
# inherits them and starts another REPL (a scripted test run from an
# agent's shell) makes that REPL write its turns into the live agent's
# wire log and session (once: the scripted plan's "Program complete."
# and "scripted provider failure" landed in a live agent's transcript).
# The command runs without them; SB_SOCKET/SB_AGENT stay (the sb CLI
# needs them), and a harness drops those itself for its child.
def bg_unset_private() -> String:
  "unset BEND_SESSION_FILE BEND_CONTEXT_FILE BEND_WIRE_LOG BEND_REPL_PORT BEND_EXTRA_PROMPT BEND_CONTINUE BEND_CRASH_NOTE BEND_DEBUG_DIR"

# ---- slot allocation never recycles a slot that still holds results ----
# A background command that ends frees its .slot but KEEPS its .out and
# .rc for the agent to read. A slot is free only once those are gone:
# recycling it earlier truncated the job's output (the next command's
# own > <id>.out), and a next command that cat'd <id>.out after printing
# anything read its own output file back into itself until ulimit -f
# stopped it: a 100 MiB tool result that killed the REPL ("bend: memory
# fault", three agents in one hour, each cat'ing the .out of a finished
# background cp).
def bg_slot_loop() -> String:
  "while [ -e $BG/$i.out ] || [ -e $BG/$i.rc ] || ! mkdir $BG/$i.slot 2>/dev/null; do"

def bg_slot_skip() -> String:
  "  if [ -e $BG/$i.slot ] || [ -e $BG/$i.out ] || [ -e $BG/$i.rc ]; then i=$((i+1)); else f=$((f+1)); fi"

# ---- what the REPL reads back is bounded ----
# The REPL turns the whole output into a String before it caps what the
# model sees (Xp.cap_out, 20000 chars); a 100 MiB output crashes it
# (measured: 75 MB passes, 100 MiB is "bend: memory fault"). The
# wrapper hands back at most bg_out_max() bytes, after a line that says
# how big the output was.
def bg_out_max() -> String:
  "1048576"

def bg_cat_out(+bg: String) -> String:
  String.append("n=$(wc -c < $BG/$i.out 2>/dev/null); n=$((${n:-0}+0)); if [ $n -gt ",
    String.append(bg_out_max(),
      String.append(" ]; then echo \"[bash: the output is $n bytes; only its first ",
        String.append(bg_out_max(),
          String.append(" follow. The rest is lost: redirect a big output to a file and read it in parts]\"; head -c ",
            String.append(bg_out_max(), " $BG/$i.out; echo; else cat $BG/$i.out 2>/dev/null; fi"))))))

# ---- no command can eat the machine's memory ----
# 2026-10-01: two `vercel deploy` runs an agent piped into `grep -m1`
# printed their URL, then never ended and grew ~100 MB/s: 32 and 17 GB
# of node in 6 minutes, the Mac out of memory, its disk full of swap,
# a reboot. The command runs in a process group of its own (set -m: the
# group outlives an orphaned child, `( cmd & )` included), and a
# watchdog beside it reads the group's resident memory every 2 s while
# any of it lives. A process over the limit gets the whole group killed
# and the .out says which one, how big, and why: the command's own .out
# only (the runner still owns the slot, or the slot ended with its rc;
# an orphan of a finished sync command dies silently, never into the
# next command's output).
# The limit, per process: BEND_MEM_LIMIT_MB (0 = none), else a quarter
# of the RAM between 2 and 8 GB (8 GB when the RAM is unknown).
def bg_mem_limit() -> String:
  "L=${BEND_MEM_LIMIT_MB:-}; if [ -z \"$L\" ]; then m=$(sysctl -n hw.memsize 2>/dev/null); m=$((${m:-0}/4194304)); if [ $m -eq 0 ] || [ $m -gt 8192 ]; then m=8192; fi; if [ $m -lt 2048 ]; then m=2048; fi; L=$m; fi"

# started in the runner right after the command ($C leads its group)
def bg_mem_watch() -> String:
  "if [ \"$L\" -gt 0 ] 2>/dev/null; then ( while kill -0 -$C 2>/dev/null; do sleep 2; k=$(ps -axo pgid=,pid=,rss=,ucomm= 2>/dev/null | awk -v g=$C -v l=$((L*1024)) '$1==g && $3>l {print \"pid \" $2 \" (\" $4 \") at \" int($3/1024) \" MB\"; exit}'); if [ -n \"$k\" ]; then kill -9 -$C 2>/dev/null; if { [ -d $BG/$i.slot ] && [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; } || { [ ! -d $BG/$i.slot ] && [ -e $BG/$i.rc ]; }; then echo \"[bash: killed $k of RAM, over the limit of $L MB per process (BEND_MEM_LIMIT_MB, 0 = no limit). A process that keeps growing is a leak or a command that never ends: e.g. its output piped into grep -m1 or head, or a CLI left running in the background]\" >> $BG/$i.out; fi; break; fi; done ) >/dev/null 2>&1 & fi"

# ---- the wrapper: one line per generated shell statement ----
def bg_wrap.seg(+a: String, +b: String) -> String:
  String.append(a, String.append("\n", b))

# the contract lines carry the LITERAL dir (not $BG): at runtime the
# agent reads a fully concrete path, with only the slot id resolved.
# printf, not echo: printf's %s argument is printed verbatim.
def bg_wrap.echo(+line: String, +rest: String) -> String:
  bg_wrap.seg(String.append("    printf '%s\\n' \"", String.append(line, "\"")), rest)

# ---- a stop cuts the sync command (issue 13) ----
# The poll loop reads the REPL's interrupt flag ($IF: the hub writes who
# asked; a cleared flag is an empty file, so the test is a builtin until
# a stop comes). A set flag stops the SYNC command only: TERM to its own
# process group (-$C, the pid the runner wrote), up to 0.5 s for it to
# go, then KILL; the loop ends and the result is the output so far, then
# a line naming who stopped it, exit 130. A background handoff is never
# touched: its wrapper has exited, nothing polls the flag for it. The
# flag stays for the turn's boundary drain (runtime/drain.bend), which
# ends the turn.
def bg_stop_check() -> String:
  "  if [ -s \"$IF\" ] && [ -s $BG/$i.pid ] && grep -q '[^[:space:]]' \"$IF\" 2>/dev/null; then x=$(tr -d '\\n\\r' < \"$IF\"); case \"$x\" in \"\"|1|user) x=\"the user\";; esac; C=$(cat $BG/$i.pid 2>/dev/null); kill -TERM -$C 2>/dev/null; n=0; while kill -0 -$C 2>/dev/null && [ $n -lt 5 ]; do sleep 0.1; n=$((n+1)); done; kill -KILL -$C 2>/dev/null; break; fi"

def bg_stop_note() -> String:
  "if [ -n \"$x\" ]; then echo \"[bash: interrupted by $x: the command was stopped (TERM to its process group, then KILL); its output so far is above. Check what it changed before you run it again]\"; s=130; fi"

# ipath: the REPL's interrupt flag (provider-pure interrupt_path_of)
def bg_wrap(+command: String, +bg: String, +after: U32, +ipath: String) -> String:
  +sent = bg_sentinel(command)
  bg_wrap.seg(String.append("BG=", bg),
  bg_wrap.seg(String.append("IF=", ipath),
  bg_wrap.seg(bg_unset_private(),
  bg_wrap.seg("mkdir -p $BG",
  bg_wrap.seg("i=0",
  bg_wrap.seg("f=0",
  bg_wrap.seg(bg_slot_loop(),
  bg_wrap.seg(bg_slot_skip(),
  bg_wrap.seg(bg_slot_fail(bg),
  bg_wrap.seg("done",
  bg_wrap.seg("mkfifo $BG/$i.in 2>/dev/null",
  bg_wrap.seg("exec 9<>$BG/$i.in",
  bg_wrap.seg(String.append("cat > $BG/$i.cmd <<'", String.append(sent, "'")),
  bg_wrap.seg(command,
  bg_wrap.seg(sent,
  bg_wrap.seg(bg_cmd_fail(bg),
    # the command runs FROM THE .cmd FILE, never inline: a heredoc in an
  # inline command would swallow the wrapper own following lines
  # (polling, handoff, result print) as its body - the wrapper then
  # exits before printing anything and the tool returns empty.
  bg_wrap.seg(bg_mem_limit(),
  bg_wrap.seg(String.append("( ulimit -f 102400; set -m; ( BEND_STDIN=$BG/$i.in /bin/sh $BG/$i.cmd ) > $BG/$i.out 2>&1 </dev/null 9<&- & C=$!; set +m; echo $C > $BG/$i.pid; ",
      String.append(bg_mem_watch(),
      "; wait $C; R=$?; if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then echo $R > $BG/$i.rc; rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in; rmdir $BG/$i.slot 2>/dev/null; fi; exit $R ) >/dev/null 2>&1 &")),
  bg_wrap.seg("r=$!",
  bg_wrap.seg(bg_timer(after),
  bg_wrap.seg(bg_timer_trap(),
  bg_wrap.seg("e=0",
  bg_wrap.seg("x=",
  bg_wrap.seg("while kill -0 $r 2>/dev/null; do",
  bg_wrap.seg("  if [ $e -lt 5 ]; then sleep 0.02; else sleep 0.1; fi",
  bg_wrap.seg("  e=$((e+1))",
  bg_wrap.seg(bg_stop_check(),
  bg_wrap.seg("  if ! kill -0 $t 2>/dev/null; then",
  bg_wrap.seg(String.append("    printf '%s\\n' \"",
    String.append(HANDOFF_MARKER(), "\"")),
  bg_wrap.echo(bg_headline(bg, "$i", after),
  bg_wrap.echo(bg_tail_cmd(bg, "$i"),
  bg_wrap.echo(bg_stdin_cmd(bg, "$i"),
  bg_wrap.echo(bg_status_cmd(bg, "$i"),
  bg_wrap.echo(bg_kill_cmd(bg, "$i"),
  bg_wrap.seg(String.append("    ", bg_write_note("! -s", bg, ".pid",
    " (disk full?) - the command runs under the runner pid $r")),
  bg_wrap.seg("    rm -f \"$0\"",
  bg_wrap.seg("    exit 0",
  bg_wrap.seg("  fi",
  bg_wrap.seg("done",
  bg_wrap.seg("kill $t 2>/dev/null; wait $t 2>/dev/null",
  bg_wrap.seg("wait $r",
  bg_wrap.seg("s=$?",
  bg_wrap.seg(bg_rc_guard(bg),
  bg_wrap.seg(bg_write_note("! -f", bg, ".out", " (disk full?)"),
  bg_wrap.seg(bg_cat_out(bg),
  bg_wrap.seg(bg_write_note("! -s", bg, ".rc",
    " (disk full?) - the output above may be incomplete"),
  bg_wrap.seg(bg_stop_note(),
  bg_wrap.seg("rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc 2>/dev/null",
  bg_wrap.seg("rmdir $BG/$i.slot 2>/dev/null",
  bg_wrap.seg("rm -f \"$0\"",
  "exit $s"))))))))))))))))))))))))))))))))))))))))))))))))))

# ---- the sandbox (approvals-sandbox, approvals-design.md §6) ----
# In auto on macOS the hub's allow line carries flags: "sandbox" runs the
# wrapper under <side>/sandbox.sb (writes only in the roots, network on
# loopback and unix sockets), "sandbox net" under <side>/sandbox-net.sb
# (the same writes, the network open: a part names a network program).
# No flag: plain /bin/sh (yolo, the parser path, an allowed rerun). The
# hub writes both profiles next to the gate file; a missing one makes
# sandbox-exec fail, so the command does not run (fails closed).

def SANDBOX_EXEC() -> String:
  "/usr/bin/sandbox-exec"

def sandbox_profile.net(net: Bool, +side: String) -> String:
  match net:
    case True{}:
      String.append(side, "/sandbox-net.sb")
    case False{}:
      String.append(side, "/sandbox.sb")

def sandbox_profile.on(on: Bool, +side: String, +flags: String) -> String:
  match on:
    case True{}:
      sandbox_profile.net(String.contains(flags, "net"), side)
    case False{}:
      ""

# side: the agent's run/ folder (side_dir_of), flags: the allow line's
# words after "allow" ("" for none); "" = no sandbox
def sandbox_profile(+side: String, +flags: String) -> String:
  sandbox_profile.on(String.contains(flags, "sandbox"), side, flags)

def sh_argv.pick(plain: Bool, +profile: String, +path: String)
    -> List<&2, String>:
  match plain:
    case True{}:
      Con{"/bin/sh", Con{path, Nil{}}}
    case False{}:
      Con{SANDBOX_EXEC(), Con{"-f", Con{profile,
        Con{"/bin/sh", Con{path, Nil{}}}}}}

# the wrapper's argv: /bin/sh <path>, under sandbox-exec with a profile
def sh_argv(+profile: String, +path: String) -> List<&2, String>:
  sh_argv.pick(String.is_empty(profile), profile, path)

# Codex's heuristic (sandboxing/src/denial.rs): no sure way to tell, so a
# failed run whose output shows a refused write or a failed network call
# was stopped by the sandbox. The hub reads the path from the output.
def sandbox_stopped.text(+low: String) -> Bool:
  Bool.or(String.contains(low, "operation not permitted"),
  Bool.or(String.contains(low, "could not resolve host"),
  Bool.or(String.contains(low, "couldn't connect to server"),
  Bool.or(String.contains(low, "nodename nor servname provided"),
  Bool.or(String.contains(low, "temporary failure in name resolution"),
  Bool.or(String.contains(low, "failed to lookup address"),
    String.contains(low, "network is unreachable")))))))

def sandbox_stopped.go(ok: Bool, +out: String) -> Bool:
  match ok:
    case True{}:
      False{}
    case False{}:
      sandbox_stopped.text(String.to_lower(out))

# a sandboxed run's result: stopped by the sandbox? (a success, a
# background handoff included, never is)
def sandbox_stopped(+r: Xp.ExecRes) -> Bool:
  match r:
    case Xp.Ex{+ok, +out}:
      sandbox_stopped.go(ok, out)
