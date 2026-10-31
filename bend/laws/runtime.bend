# bend/laws/runtime.bend: the runtime: search, feed annotations, MCP parity, bash handoff, the agent folders, wrapper invariants.
# Open claims, each one closed by bend/proofs/runtime.bend (moved from
# LAWS.bend). The gate checks every bend/proofs/*.bend: a law here
# with no proof there fails it.
import Base
import ../core/types.bend as T
import ../runtime/tools-pure.bend as Xt
import ../runtime/main-pure.bend as Rt
import ../runtime/bash-pure.bend as Sh
import ../runtime/mcp-pure.bend as Mc

# ---- the structured search (runtime/tools.bend) ----
# best_match returns ranked names without declarations, details returns
# declarations and NEVER a silent empty, the flat syntax stays for the
# scripted suite.

# LAW: best_match returns the exact function NAME, not its declaration.
law best_match_names_only:
  {Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"echo thing\"}",
      Con{T.ToolDef{"echo.thing", "a loud description"}, Nil{}})
    == "echo.thing" : String}

# LAW: a two-word match ranks above a one-word match.
law best_match_ranks_words:
  {Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"echo thing\"}",
      Con{T.ToolDef{"other.thing", "d"},
        Con{T.ToolDef{"echo.thing", "d"}, Nil{}}})
    == "echo.thing\nother.thing" : String}

# LAW: best_match caps at 20 names and flags that more exist (the
# a21.zz tool must NOT appear).
law best_match_caps_and_flags:
  {Bool.and(
      String.contains(
        Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"zz\"}",
          Con{T.ToolDef{"a1.zz", "d"},
      Con{T.ToolDef{"a2.zz", "d"},
      Con{T.ToolDef{"a3.zz", "d"},
      Con{T.ToolDef{"a4.zz", "d"},
      Con{T.ToolDef{"a5.zz", "d"},
      Con{T.ToolDef{"a6.zz", "d"},
      Con{T.ToolDef{"a7.zz", "d"},
      Con{T.ToolDef{"a8.zz", "d"},
      Con{T.ToolDef{"a9.zz", "d"},
      Con{T.ToolDef{"a10.zz", "d"},
      Con{T.ToolDef{"a11.zz", "d"},
      Con{T.ToolDef{"a12.zz", "d"},
      Con{T.ToolDef{"a13.zz", "d"},
      Con{T.ToolDef{"a14.zz", "d"},
      Con{T.ToolDef{"a15.zz", "d"},
      Con{T.ToolDef{"a16.zz", "d"},
      Con{T.ToolDef{"a17.zz", "d"},
      Con{T.ToolDef{"a18.zz", "d"},
      Con{T.ToolDef{"a19.zz", "d"},
      Con{T.ToolDef{"a20.zz", "d"},
      Con{T.ToolDef{"a21.zz", "d"},
      Nil{}}}}}}}}}}}}}}}}}}}}}}),
        "moreCandidatesAvailable: true"),
      Bool.not(String.contains(
        Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"zz\"}",
          Con{T.ToolDef{"a1.zz", "d"},
      Con{T.ToolDef{"a2.zz", "d"},
      Con{T.ToolDef{"a3.zz", "d"},
      Con{T.ToolDef{"a4.zz", "d"},
      Con{T.ToolDef{"a5.zz", "d"},
      Con{T.ToolDef{"a6.zz", "d"},
      Con{T.ToolDef{"a7.zz", "d"},
      Con{T.ToolDef{"a8.zz", "d"},
      Con{T.ToolDef{"a9.zz", "d"},
      Con{T.ToolDef{"a10.zz", "d"},
      Con{T.ToolDef{"a11.zz", "d"},
      Con{T.ToolDef{"a12.zz", "d"},
      Con{T.ToolDef{"a13.zz", "d"},
      Con{T.ToolDef{"a14.zz", "d"},
      Con{T.ToolDef{"a15.zz", "d"},
      Con{T.ToolDef{"a16.zz", "d"},
      Con{T.ToolDef{"a17.zz", "d"},
      Con{T.ToolDef{"a18.zz", "d"},
      Con{T.ToolDef{"a19.zz", "d"},
      Con{T.ToolDef{"a20.zz", "d"},
      Con{T.ToolDef{"a21.zz", "d"},
      Nil{}}}}}}}}}}}}}}}}}}}}}}),
        "a21.zz")))
    == True{} : Bool}

# LAW: details on a missing name never comes back silently empty — the
# empty message and the missing list (a silent "" once left the model
# retrying blind).
law details_missing_is_loud:
  {Xt.search_reply("{\"mode\": \"details\", \"functions\": [\"no.such\"]}",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "No exact tool function details loaded. Call best_match/all_connector_capabilities first, then request exact returned function names.\n# Missing Exact Names\n- No exact tool function found for: no.such"
    : String}

# LAW: details with a found and a missing name shows both parts.
law details_show_found_and_missing:
  {Xt.search_reply(
      "{\"mode\": \"details\", \"functions\": [\"echo.thing\", \"no.such\"]}",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "echo.thing: desc\n\n# Missing Exact Names\n- No exact tool function found for: no.such"
    : String}

# LAW: the deprecated functionNames alias resolves like functions.
law details_alias_resolves:
  {Xt.search_reply("{\"mode\": \"details\", \"functionNames\": [\"echo.thing\"]}",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "echo.thing: desc\n" : String}

# LAW: the flat scripted syntax still resolves (the suite's hermetic
# path — non-JSON args fall back).
law flat_details_still_works:
  {Xt.search_reply("details: echo.thing",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "echo.thing: desc\n" : String}

# LAW: a connectors filter restricts best_match to that connector.
law best_match_connectors_filter:
  {Xt.search_reply(
      "{\"mode\": \"best_match\", \"query\": \"thing\", \"connectors\": [\"echo\"]}",
      Con{T.ToolDef{"other.thing", "d"},
        Con{T.ToolDef{"echo.thing", "d"}, Nil{}}})
    == "echo.thing" : String}

# ---- the TUI feed annotations (runtime/main.bend) ----
# One line each, single-line, capped — every client parses these.

# LAW: the tool annotation carries the id and the model-facing name.
law tool_ann_format:
  {Rt.call_ann(T.Call{12, "bash", "ls -la"}) == "tool #12 bash : ls -la"
    : String}

# LAW: results annotate with the outcome and a preview.
law tool_res_ann_ok:
  {Rt.res_ann(12, Xt.Ex{True{}, "hello"}) == "tool_result #12 ok : hello"
    : String}

law tool_res_ann_fail:
  {Rt.res_ann(12, Xt.Ex{False{}, "boom"}) == "tool_result #12 fail : boom"
    : String}

# LAW: program sub-calls annotate with name and outcome.
law subtool_ann_format:
  {Rt.sub_ann("github_app.get_me", Xt.Ex{True{}, "{}"})
    == "subtool github_app.get_me ok : {}" : String}

# LAW: annotations stay single-line — one wire line each.
law ann_flat_single_line:
  {Bool.not(String.contains(Rt.ann_flat("a\nb"), "\n")) == True{} : Bool}

# LAW: annotations cap at 200 chars (a 210-char payload trims to 200).
law ann_flat_caps_at_200:
  {String.length(Rt.ann_flat("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")) == 200n : Nat}

# LAW: an engine error payload hands its message back — the model
# reads exit-43 errors and retries.
law node_err_extracts_message:
  {Rt.node_err("{\"error\": \"missing required parameter: owner\"}")
    == "missing required parameter: owner" : String}

# ---- MCP connector parity (vibe connector_registry) ----
# The aliases (normalize, dedupe), the bootstrap URL and the
# image_generation note are Rust's (bend_plugins::connectors, its unit
# tests): the session's bridge writes the index the REPL reads.

# ---- bash background handoff (runtime/bash.bend) ----
# A command that outlives the sync window must never block the turn: it
# detaches under a per-id directory the agent drives with ordinary bash.
# The wrapper is pinned byte for byte — output to a FILE (never a pipe,
# so no orphan can hold the read), the fifo held RDWR (no EOF), the
# reaper writing .rc, the exit-199 contract, the slot cleanup.

# LAW: the wrapper script, byte for byte.
law bg_wrapper_script:
  {Sh.bg_wrap("whoami", "/tmp/bend-bg-0", 30, "/i")
    == "BG=/tmp/bend-bg-0
IF=/i
unset BEND_SESSION_FILE BEND_CONTEXT_FILE BEND_WIRE_LOG BEND_REPL_PORT BEND_EXTRA_PROMPT BEND_CONTINUE BEND_CRASH_NOTE BEND_DEBUG_DIR
mkdir -p $BG
i=0
f=0
while [ -e $BG/$i.out ] || [ -e $BG/$i.rc ] || ! mkdir $BG/$i.slot 2>/dev/null; do
  if [ -e $BG/$i.slot ] || [ -e $BG/$i.out ] || [ -e $BG/$i.rc ]; then i=$((i+1)); else f=$((f+1)); fi
  if [ $f -ge 20 ]; then echo \"background: cannot create a slot in /tmp/bend-bg-0 (disk full or not writable?)\"; rm -f \"$0\"; exit 1; fi
done
mkfifo $BG/$i.in 2>/dev/null
exec 9<>$BG/$i.in
cat > $BG/$i.cmd <<'XQ-3131618372-EOF'
whoami
XQ-3131618372-EOF
if [ $? -ne 0 ]; then echo \"background: could not write /tmp/bend-bg-0/$i.cmd (disk full?)\"; rm -f $BG/$i.in $BG/$i.cmd; rmdir $BG/$i.slot 2>/dev/null; rm -f \"$0\"; exit 1; fi
L=${BEND_MEM_LIMIT_MB:-}; if [ -z \"$L\" ]; then m=$(sysctl -n hw.memsize 2>/dev/null); m=$((${m:-0}/4194304)); if [ $m -eq 0 ] || [ $m -gt 8192 ]; then m=8192; fi; if [ $m -lt 2048 ]; then m=2048; fi; L=$m; fi
O=${BEND_OUT_LIMIT_MB:-2048}
( exec 8>$BG/$i.out; set -m; ( BEND_STDIN=$BG/$i.in /bin/sh $BG/$i.cmd ) > $BG/$i.out 2>&1 </dev/null 9<&- 8<&- & C=$!; set +m; echo $C > $BG/$i.pid; if [ \"$L\" -gt 0 ] 2>/dev/null || [ \"$O\" -gt 0 ] 2>/dev/null; then ( while kill -0 -$C 2>/dev/null; do sleep 2; m=; if [ \"$L\" -gt 0 ] 2>/dev/null; then k=$(ps -axo pgid=,pid=,rss=,ucomm= 2>/dev/null | awk -v g=$C -v l=$((L*1024)) '$1==g && $3>l {print \"pid \" $2 \" (\" $4 \") at \" int($3/1024) \" MB\"; exit}'); if [ -n \"$k\" ]; then m=\"[bash: killed $k of RAM, over the limit of $L MB per process (BEND_MEM_LIMIT_MB, 0 = no limit). A process that keeps growing is a leak or a command that never ends: e.g. its output piped into grep -m1 or head, or a CLI left running in the background]\"; fi; fi; if [ -z \"$m\" ] && [ \"$O\" -gt 0 ] 2>/dev/null; then n=$(wc -c <&8 2>/dev/null); if [ $((${n:-0}+0)) -gt $((O*1048576)) ]; then m=\"[bash: stopped: its output passed $O MB (BEND_OUT_LIMIT_MB, 0 = no limit); a command that prints without end, or reads its own output back]\"; fi; fi; if [ -n \"$m\" ]; then kill -STOP -$C 2>/dev/null; if { [ -d $BG/$i.slot ] && [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; } || { [ ! -d $BG/$i.slot ] && [ -e $BG/$i.rc ]; }; then { [ -z \"$(tail -c 1 $BG/$i.out)\" ] || echo; echo \"$m\"; } >> $BG/$i.out; fi; kill -9 -$C 2>/dev/null; break; fi; done ) >/dev/null 2>&1 & fi; wait $C; R=$?; if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then echo $R > $BG/$i.rc; rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in; rmdir $BG/$i.slot 2>/dev/null; fi; exit $R ) >/dev/null 2>&1 &
r=$!
sleep 30 >/dev/null 2>&1 </dev/null 9<&- & t=$!
trap 'kill $t 2>/dev/null; exit 143' HUP INT TERM
e=0
x=
while kill -0 $r 2>/dev/null; do
  if [ $e -lt 5 ]; then sleep 0.02; else sleep 0.1; fi
  e=$((e+1))
  if [ -s \"$IF\" ] && [ -s $BG/$i.pid ] && grep -q '[^[:space:]]' \"$IF\" 2>/dev/null; then x=$(tr -d '\\n\\r' < \"$IF\"); case \"$x\" in \"\"|1|user) x=\"the user\";; esac; C=$(cat $BG/$i.pid 2>/dev/null); kill -TERM -$C 2>/dev/null; n=0; while kill -0 -$C 2>/dev/null && [ $n -lt 5 ]; do sleep 0.1; n=$((n+1)); done; kill -KILL -$C 2>/dev/null; break; fi
  if ! kill -0 $t 2>/dev/null; then
    printf '%s\\n' \"BEND-BG-HANDOFF-6f2c9e5b\"
    printf '%s\\n' \"background: still running after 30s - id $i (dir /tmp/bend-bg-0)\"
    printf '%s\\n' \"  output: tail -n 50 /tmp/bend-bg-0/$i.out\"
    printf '%s\\n' \"  stdin (only if the command reads the fifo in BEND_STDIN): printf '%s\\n' 'text' > /tmp/bend-bg-0/$i.in  (otherwise its stdin is /dev/null)\"
    printf '%s\\n' \"  status: kill -0 $(cat /tmp/bend-bg-0/$i.pid) && echo RUNNING || cat /tmp/bend-bg-0/$i.rc 2>/dev/null\"
    printf '%s\\n' \"  kill: kill $(cat /tmp/bend-bg-0/$i.pid)\"
    if [ ! -s /tmp/bend-bg-0/$i.pid ]; then echo \"background: could not write /tmp/bend-bg-0/$i.pid (disk full?) - the command runs under the runner pid $r\"; fi
    rm -f \"$0\"
    exit 0
  fi
done
kill $t 2>/dev/null; wait $t 2>/dev/null
wait $r
s=$?
if [ ! -d /tmp/bend-bg-0 ]; then echo \"background: the bg dir vanished before the exit code was recorded: /tmp/bend-bg-0\"; rm -f \"$0\"; exit 1; fi
if [ ! -f /tmp/bend-bg-0/$i.out ]; then echo \"background: could not write /tmp/bend-bg-0/$i.out (disk full?)\"; fi
n=$(wc -c < $BG/$i.out 2>/dev/null); n=$((${n:-0}+0)); if [ $n -gt 1048576 ]; then echo \"[bash: the output is $n bytes; only its first 1048576 follow. The rest is lost: redirect a big output to a file and read it in parts]\"; tail -c 1024 $BG/$i.out | grep -a -E '^\\[bash: (stopped:|killed) '; head -c 1048576 $BG/$i.out; echo; else cat $BG/$i.out 2>/dev/null; fi
if [ ! -s /tmp/bend-bg-0/$i.rc ]; then echo \"background: could not write /tmp/bend-bg-0/$i.rc (disk full?) - the output above may be incomplete\"; fi
if [ -n \"$x\" ]; then echo \"[bash: interrupted by $x: the command was stopped (TERM to its process group, then KILL); its output so far is above. Check what it changed before you run it again]\"; s=130; fi
rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc 2>/dev/null
rmdir $BG/$i.slot 2>/dev/null
rm -f \"$0\"
exit $s"
    : String}

# LAW: the sync window defaults# LAW: the sync window defaults to 30 seconds and clamps into [0, 3600].
law bg_after_defaults:
  {Sh.bg_after.read(None{}, 30) == 30 : U32}

law bg_after_clamps_high:
  {Sh.bg_after.clamp(4000) == 3600 : U32}

law bg_after_keeps_low:
  {Sh.bg_after.clamp(5) == 5 : U32}

# LAW: the handoff maps to a SUCCESSFUL result: the wrapper prints the
# handoff marker as its first line, exec_of strips it and keeps the
# contract — the turn continues and the agent decides.
law exec_of_background_maps_ok:
  {Sh.exec_of(String.append("0\n", String.append(Sh.HANDOFF_MARKER(),
      "\nbackground: still running")))
    == Xt.Ex{True{}, "background: still running"} : Xt.ExecRes}

# LAW: a command that REALLY exits 199 reports as a failure with its
# code — the handoff is detected by the marker, never by the code
# (once: exit 199 was silently swallowed as a handoff).
law exec_of_real_199_fails:
  {Sh.exec_of("199\nreal output")
    == Xt.Ex{False{}, "exit 199: real output"} : Xt.ExecRes}

# LAW: ordinary failures still report their exit code.
law exec_of_exit_still_fails:
  {Sh.exec_of.pick.zero(False{}, 1, "boom")
    == Xt.Ex{False{}, "exit 1: boom"} : Xt.ExecRes}

# ---- the background battery: solidity and agent ergonomics ----

# LAW: the bg dir is per-REPL, keyed by port — parallel terminals
# never share slots, and a /reload (same port) finds its slots again.
law bg_dir_default_root:
  {Sh.bg_dir_of("/tmp", "7702") == "/tmp/bend-bg-7702" : String}

law bg_dir_custom_root:
  {Sh.bg_dir_of("/var/bg", "3") == "/var/bg/bend-bg-3" : String}

# ---- the agent's own folders (approvals-tmp, approvals-design.md §7.1) ----

# LAW: the harness's own files go to the agent's run/ folder when the hub
# names it (BEND_AGENT_RUN), else to $TMPDIR (a headless run), and only
# with neither to /tmp; the trailing "/" of a macOS $TMPDIR is dropped.
law side_dir_is_the_agent_run:
  {Sh.side_dir_of("/h/agents/a/run", "/var/T/") == "/h/agents/a/run" : String}

law side_dir_falls_back_to_tmpdir:
  {Sh.side_dir_of("", "/var/folders/x/T/") == "/var/folders/x/T" : String}

law side_dir_last_resort_tmp:
  {Sh.side_dir_of("", "") == "/tmp" : String}

law dir_trim_keeps_root:
  {Sh.dir_trim("/") == "/" : String}

# LAW: the bg dir is the one the hub names (the agent's tmp/bg, so the
# agent reads its jobs' output in its own temp folder), else per port
# under BEND_BG_ROOT, else under $TMPDIR, else under /tmp.
law bg_dir_is_the_agent_tmp_bg:
  {Sh.bg_dir_pick("/h/agents/a/tmp/bg", "/r", "/var/T/", "7")
    == "/h/agents/a/tmp/bg" : String}

law bg_dir_root_then_tmpdir:
  {Bool.and(String.eq(Sh.bg_dir_pick("", "/r", "/var/T/", "7"), "/r/bend-bg-7"),
     Bool.and(String.eq(Sh.bg_dir_pick("", "", "/var/T/", "7"), "/var/T/bend-bg-7"),
       String.eq(Sh.bg_dir_pick("", "", "", "7"), "/tmp/bend-bg-7")))
    == True{} : Bool}

# LAW: the boot cleanup removes this port's wrappers in the side dir,
# the folder quoted as one shell word.
law bash_cleanup_cmd_quotes_the_dir:
  {Sh.bash_cleanup_cmd("/a b/run", "7") == "rm -f '/a b/run'/bend-sh-7-*.sh"
    : String}

# LAW: one slot = five files in one dir under one id.
law bg_slot_paths:
  {Bool.and(String.eq(Sh.bg_slot("/d", "4", ".out"), "/d/4.out"),
     Bool.and(String.eq(Sh.bg_slot("/d", "4", ".in"), "/d/4.in"),
       Bool.and(String.eq(Sh.bg_slot("/d", "4", ".pid"), "/d/4.pid"),
         Bool.and(String.eq(Sh.bg_slot("/d", "4", ".rc"), "/d/4.rc"),
           String.eq(Sh.bg_slot("/d", "4", ".cmd"), "/d/4.cmd")))))
    == True{} : Bool}

# LAW: the sync window is a clock, never a count of poll turns: a
# `sleep <after>` timer beside the runner, and the poll hands off when
# the timer is gone. (A count of `sleep 0.1` turns ran slow by the
# exec of each sleep: under load a 6 s window lasted 8 s and a 30 s
# one up to 40 s.) The timer holds no pipe and no fifo, is killed on
# a sync end and by the trap when the wrapper is stopped. The poll
# still probes fast first (5 x 20 ms), then 100 ms steps.
law bg_wrapper_window_is_a_clock:
  {Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "r=$!
sleep 5 >/dev/null 2>&1 </dev/null 9<&- & t=$!
trap 'kill $t 2>/dev/null; exit 143' HUP INT TERM
"),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "  if ! kill -0 $t 2>/dev/null; then"),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "done
kill $t 2>/dev/null; wait $t 2>/dev/null
wait $r"),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "  if [ $e -lt 5 ]; then sleep 0.02; else sleep 0.1; fi"),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "if [ $e -ge "))))))
    == True{} : Bool}

# LAW: the headline names the window, the slot id and the dir — the
# agent can ls the dir right away.
law bg_headline_names_window_id_dir:
  {Sh.bg_headline("/tmp/bend-bg-0", "3", 30)
    == "background: still running after 30s - id 3 (dir /tmp/bend-bg-0)"
    : String}

# LAW: each contract line is a COMPLETE command on a CONCRETE path —
# the agent copy-pastes, it never resolves a variable.
law bg_tail_cmd_concrete:
  {Sh.bg_tail_cmd("/tmp/bend-bg-0", "3")
    == "  output: tail -n 50 /tmp/bend-bg-0/3.out" : String}

law bg_stdin_cmd_concrete:
  {Bool.and(
     String.eq(Sh.bg_stdin_cmd("/tmp/bend-bg-0", "3"),
       "  stdin (only if the command reads the fifo in BEND_STDIN): printf '%s\\n' 'text' > /tmp/bend-bg-0/3.in  (otherwise its stdin is /dev/null)"),
     String.contains(Sh.bg_stdin_cmd("/tmp/bend-bg-0", "3"),
       "BEND_STDIN")) == True{} : Bool}

law bg_status_cmd_concrete:
  {Sh.bg_status_cmd("/tmp/bend-bg-0", "3")
    == "  status: kill -0 $(cat /tmp/bend-bg-0/3.pid) && echo RUNNING || cat /tmp/bend-bg-0/3.rc 2>/dev/null"
    : String}

law bg_kill_cmd_concrete:
  {Sh.bg_kill_cmd("/tmp/bend-bg-0", "3")
    == "  kill: kill $(cat /tmp/bend-bg-0/3.pid)" : String}

# ---- wrapper invariants (structure, not bytes) ----

# LAW: the fifo is held RDWR by the wrapper — never EOF for the
# reader, never blocks at open.
# LAW: every wait is total — the wrapper polls its own child, the
# runner ($!), and takes the exit code from `wait` (the runner exits
# with it), never from a file. Once: on a full disk the runner could
# not write the .rc, the wrapper waited for it forever, and a bash call
# hung 19 minutes.
law bg_wrapper_waits_on_runner:
  {Bool.and(
     Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "fi; exit $R ) >/dev/null 2>&1 &
r=$!"),
       String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "while kill -0 $r 2>/dev/null; do")),
     Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "wait $r
s=$?"),
       Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "while [ ! -e $BG/$i.rc ]"))))
    == True{} : Bool}

# LAW: a slot file the runner could not write is named to the model —
# the call returns, and says why a file is missing.
law bg_wrapper_reports_unwritable_files:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_write_note("! -f", "/d", ".out", " (disk full?)")),
     Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "could not write /d/$i.rc (disk full?)"),
       String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_cmd_fail("/d"))))
    == True{} : Bool}

law bg_wrapper_fifo_rdrw:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "exec 9<>$BG/$i.in")
    == True{} : Bool}

# LAW (issue #9): the command's stdin is /dev/null, never the fifo: a
# command that reads a non-tty stdin (rg with no path, cat) gets EOF at
# once instead of waiting out the sync window. The fifo is reached by
# name only ($BEND_STDIN), and the command does not inherit fd 9 (nor
# fd 8, the watchdog's hold on its .out).
law bg_wrapper_stdin_devnull:
  {Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "( BEND_STDIN=$BG/$i.in /bin/sh $BG/$i.cmd ) > $BG/$i.out 2>&1 </dev/null 9<&- 8<&- &"),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "<&9")))
    == True{} : Bool}

# LAW: a detached runner is the command's real parent and records the
# TRUE exit code (a sibling cannot wait — that produced 127). The
# record is ownership-checked: the rc lands only if the runner's pid
# still owns the slot.
law bg_wrapper_runner_waits_and_records:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "C=$!; set +m; echo $C > $BG/$i.pid; "),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), " & fi; wait $C; R=$?"),
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "echo $R > $BG/$i.rc")))
    == True{} : Bool}

# LAW: output goes to a FILE, never a pipe — the orphan-holding-pipe
# bug class (minute-long hangs) is structurally gone. The one pipe of
# the wrapper is the memory watchdog's own ps | awk, read by $( ) inside
# the watchdog: no command output goes through it.
law bg_wrapper_output_is_a_file:
  {Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), " /bin/sh $BG/$i.cmd ) > $BG/$i.out 2>&1"),
   Bool.and(String.contains(Sh.bg_watch(), "k=$(ps -axo pgid=,pid=,rss=,ucomm= 2>/dev/null | awk "),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), ".cmd ) |"))))
    == True{} : Bool}

# LAW: the wrapper NEVER kills the command — ending a background
# command is the agent's decision. One exception, for the machine: the
# memory watchdog kills the command's process group (kill -9 -$C) once
# one of its processes is over the limit; no other kill -9 exists.
law bg_wrapper_never_kills:
  {Bool.and(Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "kill $c")),
     Bool.and(Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "kill -9 $")),
       String.contains(Sh.bg_watch(), "$3>l {print")))
    == True{} : Bool}

# LAW: every command runs in its own process group under the memory
# watchdog (the vercel deploys of 2026-10-01: 32 + 17 GB of node left
# running, the Mac out of memory). The group survives an orphaned child,
# so `( cmd & )` and a pipe whose reader quit are watched too; the watch
# lasts while any process of the group lives; the limit has an override.
# (bg_wrapper_script pins both lines in the wrapper, byte for byte)
law bg_wrapper_watches_memory:
  {Bool.and(String.contains(Sh.bg_watch(), "while kill -0 -$C"),
   Bool.and(String.contains(Sh.bg_watch(), "kill -9 -$C"),
     String.contains(Sh.bg_mem_limit(), "L=${BEND_MEM_LIMIT_MB:-}")))
    == True{} : Bool}

# LAW: a command that finishes inside the window leaves no trace —
# every slot file is removed.
# LAW: a background command that ends frees its slot (pid, cmd and
# fifo are removed by the runner; out and rc stay readable) — the bg
# dir cannot grow without bound and slots are recycled.
law bg_wrapper_runner_frees_slot:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
     "rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in") == True{} : Bool}

law bg_wrapper_cleans_slot:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
     "rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc")
    == True{} : Bool}

# LAW: the handoff prints the marker as the wrapper's FIRST output
# line and exits 0 — the marker (not an exit code) is the signal, so a
# command that really exits 199 still reports 199.
law bg_wrapper_handoff_marker:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       String.append("printf '%s\\n' \"", Sh.HANDOFF_MARKER())),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "exit 199")))
    == True{} : Bool}

# LAW: the wrapper carries a vanished-dir guard — an agent that cleans
# its bg dir with rm -rf gets a FAST failure with the dir name, never
# a hung tool.
law bg_rc_guard_names_the_dir:
  {String.contains(Sh.bg_rc_guard("/tmp/bend-bg-0"),
     "if [ ! -d /tmp/bend-bg-0 ]; then echo \"background: the bg dir vanished before the exit code was recorded: /tmp/bend-bg-0\"; rm -f \"$0\"; exit 1; fi")
    == True{} : Bool}

# LAW: the wrapper embeds both guards (the waits are all bounded by
# the dir check).
# LAW: slot allocation is an atomic mkdir CAS — two wrappers starting
# at the same instant can never take the same slot (once: concurrent
# calls collided on id 0 and corrupted each other's out/rc). It is
# bounded: a mkdir failing for another reason than an existing slot
# (full disk, read-only dir) ends the call after 20 tries (once: the
# loop retried forever).
law bg_wrapper_atomic_slot_allocation:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "|| ! mkdir $BG/$i.slot 2>/dev/null; do
  if [ -e $BG/$i.slot ] || [ -e $BG/$i.out ] || [ -e $BG/$i.rc ]; then i=$((i+1)); else f=$((f+1)); fi"),
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_slot_fail("/d")))
    == True{} : Bool}

# LAW: a slot that still holds a finished job's results (.out, .rc) is
# never handed to the next command: recycling it truncated the job's
# output, and a next command that cat'd that .out fed its own output
# back into itself up to the old 100 MiB file size limit (a 100 MiB result: "bend: memory
# fault" in three agents).
law bg_wrapper_keeps_finished_results:
  {String.starts_with(Sh.bg_slot_loop(),
     "while [ -e $BG/$i.out ] || [ -e $BG/$i.rc ] || ")
    == True{} : Bool}

# LAW: what the REPL reads back is bounded at 1 MiB, with a line that
# gives the real size (the REPL died on a 100 MiB String).
law bg_wrapper_output_back_bounded:
  {Bool.and(String.eq(Sh.bg_out_max(), "1048576"),
     Bool.and(String.contains(Sh.bg_cat_out("/d"), "head -c 1048576 $BG/$i.out"),
       String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_cat_out("/d"))))
    == True{} : Bool}

# LAW: the command never inherits the REPL's private environment (its
# session file, wire log, context file, port): a REPL started from an
# agent's shell wrote its turns into the live agent's wire log.
law bg_wrapper_unsets_repl_env:
  {Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "
unset BEND_SESSION_FILE BEND_CONTEXT_FILE BEND_WIRE_LOG BEND_REPL_PORT "),
     Bool.not(String.contains(Sh.bg_unset_private(), "SB_SOCKET")))
    == True{} : Bool}

# LAW: only the command's own output is capped: no `ulimit -f` (it
# killed rustc writing a 100 MiB rlib with SIGXFSZ), and the watchdog
# stops a command whose .out passes BEND_OUT_LIMIT_MB (2048 by default),
# read through the fd it holds on that .out (an orphan's unlinked .out
# too). Once an output-looping command filled the disk (a 6.4 GB .out).
law bg_wrapper_caps_only_its_output:
  {Bool.and(Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "ulimit -f")),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       String.append("
", String.append(Sh.bg_out_limit(), "
( exec 8>$BG/$i.out; set -m; ( "))),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), " 2>&1 </dev/null 9<&- 8<&- & C=$!;"),
   Bool.and(String.eq(Sh.bg_out_limit(), "O=${BEND_OUT_LIMIT_MB:-2048}"),
   Bool.and(String.contains(Sh.bg_watch(),
       "n=$(wc -c <&8 2>/dev/null); if [ $((${n:-0}+0)) -gt $((O*1048576)) ]; then m=\"[bash: stopped: its output passed $O MB (BEND_OUT_LIMIT_MB, 0 = no limit)"),
     Bool.not(String.contains(Sh.bg_watch(), "8<")))))))
    == True{} : Bool}

# LAW: the runner checks it still OWNS the slot before writing the rc
# and freeing the files — a recycled slot cannot be corrupted by the
# previous command's dying runner.
law bg_wrapper_runner_ownership_check:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
     "if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then") == True{}
    : Bool}

# LAW: both exit paths delete the wrapper script ($0) — no more
# +1 leaked script per bash call (the byte law pins both occurrences).
law bg_wrapper_cleans_its_script:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "rm -f \"$0\"") == True{}
    : Bool}

# LAW: multi-line commands stay intact — the heredoc and the runner
# subshell carry them verbatim (flatten once silently joined lines
# with spaces, restructuring scripts).
law bg_wrapper_multiline_command_intact:
  {Bool.and(
     String.contains(Sh.bg_wrap("echo a\necho b", "/d", 5, "/i"), "echo a\necho b"),
     Bool.not(String.contains(Sh.bg_wrap("echo a\necho b", "/d", 5, "/i"),
       "echo a echo b"))) == True{} : Bool}

# LAW: the .cmd heredoc sentinel derives from the command's own hash —
# a command cannot contain its own sentinel without containing its
# own hash.
law bg_sentinel_derives_from_command:
  {Bool.and(
     String.eq(Sh.bg_sentinel("whoami"), Sh.bg_sentinel("whoami")),
     Bool.not(String.eq(Sh.bg_sentinel("whoami"),
       Sh.bg_sentinel("whoamiX")))) == True{} : Bool}

law bg_wrapper_has_the_guard:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_rc_guard("/d"))
    == True{} : Bool}

# LAW: the wrapper filename derives from the script's own content:
# re-running the same command rewrites IDENTICAL bytes (an orphaned
# wrapper just continues its own script), and the hash is pinned so
# the naming cannot silently degrade.
law bg_script_hash_deterministic:
  {Sh.bg_script_hash("ab") == 1294271946 : U32}

# LAW: the hash is a U32 - code points past 2^20 included (the former
# Nat sum overflowed the runtime's 2^48-1 and killed the REPL).
law bg_script_hash_high_code_points:
  {Sh.bg_script_hash("é—􏿿") == 1694011805 : U32}

# LAW: different scripts hash apart — two commands never share a
# wrapper file, so an orphan can never re-read another command's
# bytes when it wakes from a poll sleep.
law bg_script_hash_distinguishes:
  {Bool.not(String.eq(
      U32.show(Sh.bg_script_hash("ab")),
      U32.show(Sh.bg_script_hash("cd"))))
    == True{} : Bool}

law bash_script_path_per_command:
  {Bool.and(String.eq(Sh.bash_script_path("/r", "7702", "a b"),
      Sh.bash_script_path("/r", "7702", "a b")),
     Bool.not(String.eq(Sh.bash_script_path("/r", "7702", "a b"),
       Sh.bash_script_path("/r", "7702", "a c")))) == True{} : Bool}

# LAW: the wrapper scripts live in the side dir.
law bash_script_path_in_side_dir:
  {String.starts_with(Sh.bash_script_path("/h/agents/a/run", "7", "ls"),
     "/h/agents/a/run/bend-sh-7-") == True{} : Bool}

# ---- MCP Apps views (runtime/mcp-pure.bend, docs/mcp-apps.md C1-C3) ----

# LAW: an index line's description part splits off its last field
# ` | ui: <uri>`: the model reads the rest, the call gets the uri.
law ui_field_splits_off:
  {Mc.desc_ui("#Show a counter | input: {\"type\":\"object\"} | ui: ui://fake-app/counter")
    == ("#Show a counter | input: {\"type\":\"object\"}", "ui://fake-app/counter") : String & String}

# LAW: no ui field: the description as it is, no uri.
law ui_field_absent:
  {Mc.desc_ui("#Say a word | input: {}") == ("#Say a word | input: {}", "") : String & String}

# LAW: the field is the LAST marker (one inside the schema's words is
# not it).
law ui_field_is_the_last:
  {Mc.desc_ui("#a | input: {\"description\":\"x | ui: y\"} | ui: ui://p/v")
    == ("#a | input: {\"description\":\"x | ui: y\"}", "ui://p/v") : String & String}

# LAW: text with a space after the last marker is no uri: nothing split.
law ui_field_has_no_space:
  {Mc.desc_ui("#a | ui: not a uri") == ("#a | ui: not a uri", "") : String & String}

# LAW: a tool with no view whose schema text ends with ` | ui: <word>`
# keeps its whole description: the field is a `ui://` uri (MCP Apps'
# only scheme), never any word after the marker.
law ui_field_needs_the_ui_scheme:
  {(Mc.desc_ui("#a | input: {\"description\":\"x | ui: y\"}"),
    Mc.desc_ui("#a | ui: https://p/v"))
    == (("#a | input: {\"description\":\"x | ui: y\"}", ""),
        ("#a | ui: https://p/v", "")) : (String & String) & (String & String)}

# LAW: the model's tool description never carries the ui field.
law index_tool_hides_the_ui_field:
  {Mc.index_of("http://127.0.0.1:9/t/p/s p show : #Show | input: {} | ui: ui://p/v")
    == Con{T.ToolDef{"p.show", "#Show | input: {}"}, Nil{}} : List<&2, T.ToolDef>}

# LAW: only a plugin tool (its loopback bridge) with a view asks the
# bridge to keep its full result (x-bise-call); a connector never does.
law asks_view_only_local_with_ui:
  {(Mc.asks_view("http://127.0.0.1:9/t/p/s", "ui://p/v"),
    (Mc.asks_view("http://127.0.0.1:9/t/p/s", ""),
     Mc.asks_view("conn-id-42", "ui://p/v")))
    == (True{}, (False{}, False{})) : Bool & (Bool & Bool)}

# LAW: a kept ref is one `<tool id> <ref>` line; no ref or no id: nothing.
law view_entry_line:
  {(Mc.view_entry("7", "{\"call\":\"1-0\"}"), (Mc.view_entry("7", ""), Mc.view_entry("", "{}")))
    == ("7 {\"call\":\"1-0\"}\n", ("", "")) : String & (String & String)}

# LAW: the views file becomes one `tool_view #<id> : <ref>` line per
# ref, each under its OWN id, in order (two views of one run_typescript;
# a late ref of an earlier tool); a bad line is dropped.
law view_lines_each_under_its_id:
  {Mc.view_lines("7 {\"call\":\"1-0\"}\n7 {\"call\":\"1-1\"}\n\nbad\n3 {\"call\":\"0-0\"}\n")
    == Con{"tool_view #7 : {\"call\":\"1-0\"}",
         Con{"tool_view #7 : {\"call\":\"1-1\"}",
           Con{"tool_view #3 : {\"call\":\"0-0\"}", Nil{}}}} : List<&2, String>}

# LAW: the runtime's files of views sit in its side folder, per REPL.
law views_files_in_side_dir:
  {(Mc.tool_now_path("/h/agents/a/run", "7"), Mc.views_path("/h/agents/a/run", "7"))
    == ("/h/agents/a/run/bend-tool-7.txt", "/h/agents/a/run/bend-views-7.txt") : String & String}
