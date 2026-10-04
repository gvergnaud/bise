# test-bg.bend — the background-command battery, end to end.
#
# Runs REAL commands through Sh.bash_exec (runtime/proc.bend) and
# asserts on the contract the agent actually reads. Complements the
# LAWS: the laws pin the pure contract byte for byte, this suite runs
# it against /bin/sh, real processes, a real fifo, real kills.
#
#   bend test-bg.bend -o /tmp/bg-suite
#   rm -rf /tmp/bgtest
#   BEND_BG_ROOT=/tmp/bgtest BEND_BG_AFTER=2 BEND_SESSION_FILE=/tmp/bgtest-s \
#     BEND_WIRE_LOG=/tmp/bgtest-w /tmp/bg-suite
#
# The rm -rf BEFORE the run matters: the suite asserts on hardcoded
# slot ids, so it must start from an empty bg dir (a command may never
# clean the bg dir itself — s14 proves why). The env pins a 2s sync
# window (so the suite is fast) and a private bg root (so it is
# hermetic). With no BEND_REPL_PORT the dir is /tmp/bgtest/bend-bg-0.
# Every "sleep 1" below is itself a sync command — the suite waits
# through the very machinery it tests. Waits are IO.sleep (pure): a
# background command that ends frees its .slot (pid/cmd/in removed by the
# runner) but its out/rc stay readable, and a slot with an out or rc is
# NEVER recycled (BISE-122: the recycling truncated the job's output, and
# a cat of it fed on itself up to a 100 MiB result that killed the REPL):
# each background job below takes the next id.
#
# Gates: no FAIL line (22 checks, s01-s22).
import Base
import ../runtime/main.bend as Rt
import ../runtime/bash.bend as Sh
import ../runtime/bash-pure.bend as Shp
import ../runtime/tools.bend as X
import ../runtime/tools-pure.bend as Xp
import ../runtime/persist.bend as P

def bg() -> String:
  "/tmp/bgtest/bend-bg-0"

def slot(id: String, ext: String) -> String:
  Shp.bg_slot(bg(), id, ext)

def res_ok(+r: Xp.ExecRes) -> Bool:
  match r:
    case Xp.Ex{+ok, _}:
      ok

def res_out(+r: Xp.ExecRes) -> String:
  match r:
    case Xp.Ex{_, +out}:
      out

def txt_or(m: Maybe<&2, String>) -> String:
  match m:
    case None{}:
      ""
    case Some{+t}:
      t

def readf(path: String) -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- P.read_file(path)
    IO.pure(String, txt_or(m))

def check(cond: Bool, +name: String) -> IO(Unit):
  match cond:
    case True{}:
      do IO<Unit>:
        u : Unit <- IO.print(String.append("PASS ", String.append(name, "\n")))
        IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        u : Unit <- IO.print(String.append("FAIL ", String.append(name, "\n")))
        IO.pure(Unit, Unit{})

# a sync bash delay (UNUSED in the suite: any bash_exec between a
# slot-freeing event and its evidence read recycles the slot and
# destroys the evidence — the suite waits with IO.sleep instead)
def wait() -> IO(Xp.ExecRes):
  Sh.bash_exec("sleep 1")

def main() -> IO(Unit):
  do IO<Unit>:
    # ---- s01 a fast command returns output + exit 0 ----
    +r1 : Xp.ExecRes <- Sh.bash_exec("echo sync-ok")
    # output cleanliness: no shell noise from the ephemeral pid race
    u1 : Unit <- check(Bool.and(res_ok(r1),
        Bool.and(String.contains(res_out(r1), "sync-ok"),
          Bool.not(String.contains(res_out(r1), "No such file")))),
      "s01 sync output ok and clean")

    # ---- s02 exit 0 is a success ----
    +r2 : Xp.ExecRes <- Sh.bash_exec("true")
    u2 : Unit <- check(res_ok(r2), "s02 exit 0 is ok")

    # ---- s03 a failure reports its code ----
    +r3 : Xp.ExecRes <- Sh.bash_exec("false")
    u3 : Unit <- check(Bool.and(Bool.not(res_ok(r3)),
        String.contains(res_out(r3), "exit 1")), "s03 failure carries exit code")

    # ---- s04 stderr is captured too ----
    +r4 : Xp.ExecRes <- Sh.bash_exec("echo err-ok >&2")
    u4 : Unit <- check(String.contains(res_out(r4), "err-ok"),
      "s04 stderr captured")

    # ---- s05 a sync slot is fully cleaned ----
    +t5 : String <- readf(slot("0", ".out"))
    u5 : Unit <- check(String.eq(t5, ""), "s05 sync slot cleaned")

    # ---- s06 a slow command hands off as ok (199 -> contract) ----
    +r6 : Xp.ExecRes <- Sh.bash_exec("sleep 5")
    +o6 : String <- IO.pure(String, res_out(r6))
    u6 : Unit <- check(res_ok(r6), "s06 handoff maps to ok")

    # ---- s07 the contract is concrete: window, id, dir, commands ----
    # The headline/tail/stdin lines match the pinned templates exactly;
    # the status/kill lines go further — the wrapper's $(cat pid) is
    # substituted at print time, so the agent reads the REAL pid.
    +p7 : String <- readf(slot("0", ".pid"))
    u7a : Unit <- check(String.contains(o6, Shp.bg_headline(bg(), "0", 2)),
      "s07a headline names window, id, dir")
    u7b : Unit <- check(String.contains(o6, Shp.bg_tail_cmd(bg(), "0")),
      "s07b output line is a concrete tail command")
    u7c : Unit <- check(String.contains(o6, Shp.bg_stdin_cmd(bg(), "0")),
      "s07c stdin line is a concrete fifo command")
    u7d : Unit <- check(String.contains(o6,
        String.append("  status: kill -0 ", String.trim(p7))),
      "s07d status line probes the real pid")
    u7e : Unit <- check(String.contains(o6,
        String.append("  kill: kill ", String.trim(p7))),
      "s07e kill line uses the real pid")

    # ---- s08 the contract's pid file holds a LIVE pid ----
    +p8 : String <- readf(slot("0", ".pid"))
    +r8 : Xp.ExecRes <- Sh.bash_exec(
      String.append("kill -0 ", String.trim(p8)))
    u8 : Unit <- check(res_ok(r8), "s08 contract pid is live")

    # ---- s09 the kill command works; the runner records 143 ----
    +r9 : Xp.ExecRes <- Sh.bash_exec(
      String.append("kill ", String.trim(p8)))
    +rc9 : String <- readf(slot("0", ".rc"))
    u9 : Unit <- check(String.eq(String.trim(rc9), "143"),
      "s09 kill then rc is 143")
    +u9w : Unit <- IO.sleep(1100)

    # ---- s10 the fifo feeds a reader: printf > .in ----
    +r10 : Xp.ExecRes <- Sh.bash_exec("read x < \"$BEND_STDIN\"; echo got:$x")
    f10 : Xp.ExecRes <- Sh.bash_exec(
      String.append("printf 'fifo-ok\\n' > ", slot("1", ".in")))
    +u10w : Unit <- IO.sleep(1100)
    +o10 : String <- readf(slot("1", ".out"))
    +rc10 : String <- readf(slot("1", ".rc"))
    u10 : Unit <- check(Bool.and(String.contains(o10, "got:fifo-ok"),
        String.eq(String.trim(rc10), "0")), "s10 stdin fifo feeds reader")
    +u10b : Unit <- IO.sleep(1100)

    # ---- s11 tail while running: empty, then filled when done ----
    +r11 : Xp.ExecRes <- Sh.bash_exec("sleep 3; echo late-line")
    +o11a : String <- readf(slot("2", ".out"))
    +u11w : Unit <- IO.sleep(2100)
    +o11b : String <- readf(slot("2", ".out"))
    +rc11 : String <- readf(slot("2", ".rc"))
    u11 : Unit <- check(Bool.and(Bool.not(String.contains(o11a, "late-line")),
        Bool.and(String.contains(o11b, "late-line"),
          String.eq(String.trim(rc11), "0"))), "s11 tail empty then filled")

    # ---- s12 two slow commands run in PARALLEL, distinct slots ----
    +r12a : Xp.ExecRes <- Sh.bash_exec("sleep 8")
    +r12b : Xp.ExecRes <- Sh.bash_exec("sleep 9")
    +p12a : String <- readf(slot("3", ".pid"))
    +p12b : String <- readf(slot("4", ".pid"))
    +l12a : Xp.ExecRes <- Sh.bash_exec(
      String.append("kill -0 ", String.trim(p12a)))
    +l12b : Xp.ExecRes <- Sh.bash_exec(
      String.append("kill -0 ", String.trim(p12b)))
    +k12a : Xp.ExecRes <- Sh.bash_exec(
      String.append("kill ", String.trim(p12a)))
    +rc12a : String <- readf(slot("3", ".rc"))
    +k12b : Xp.ExecRes <- Sh.bash_exec(
      String.append("kill ", String.trim(p12b)))
    +rc12b : String <- readf(slot("4", ".rc"))
    u12 : Unit <- check(Bool.and(Bool.and(res_ok(l12a), res_ok(l12b)),
        Bool.and(String.eq(String.trim(rc12a), "143"),
          String.eq(String.trim(rc12b), "143"))),
      "s12 two parallel slots, both killed")

    # ---- s13 listing the dir is ordinary ls ----
    +r13 : Xp.ExecRes <- Sh.bash_exec(String.append("ls ", bg()))
    u13 : Unit <- check(Bool.and(res_ok(r13),
        Bool.and(String.contains(res_out(r13), "0.rc"),
          String.contains(res_out(r13), "4.rc"))),
      "s13 ls lists ended slots (out/rc kept, pid freed)")

    # ---- s15 multi-line commands stay intact (QA bug 1.1/1.2) ----
    +r15 : Xp.ExecRes <- Sh.bash_exec("echo one\necho two\nif true; then\necho in-block\nfi")
    u15 : Unit <- check(Bool.and(res_ok(r15),
        Bool.and(String.contains(res_out(r15), "one\ntwo"),
          Bool.and(String.contains(res_out(r15), "in-block"),
            Bool.not(String.contains(res_out(r15), "one echo two"))))),
      "s15 multi-line command runs intact")

    # ---- s16 a real exit 199 is a failure (QA bug 1.3) ----
    +r16 : Xp.ExecRes <- Sh.bash_exec("exit 199")
    u16 : Unit <- check(Bool.and(Bool.not(res_ok(r16)),
        String.contains(res_out(r16), "exit 199")),
      "s16 real exit 199 reports as failure")

    # ---- s17 background output is capped at 50MiB (QA bug 3.1) ----
    +r17 : Xp.ExecRes <- Sh.bash_exec(
      "dd if=/dev/zero bs=1048576 count=55 2>/dev/null | cat > /dev/null; echo written")
    u17 : Unit <- check(Bool.and(res_ok(r17),
        String.contains(res_out(r17), "written")),
      "s17 capped output still completes (ulimit)")

    # ---- s18 the wrapper script deletes itself (QA bug 3.6) ----
    +c18a : Xp.ExecRes <- Sh.bash_exec("ls \"${BEND_AGENT_RUN:-${TMPDIR:-/tmp}}\"/bend-sh-*.sh 2>/dev/null | wc -l")
    +r18 : Xp.ExecRes <- Sh.bash_exec("echo self-clean-probe")
    +c18b : Xp.ExecRes <- Sh.bash_exec("ls \"${BEND_AGENT_RUN:-${TMPDIR:-/tmp}}\"/bend-sh-*.sh 2>/dev/null | wc -l")
    u18 : Unit <- check(Bool.and(res_ok(c18a), Bool.and(res_ok(c18b),
        Bool.not(String.eq(String.trim(res_out(c18b)),
          String.append(String.trim(res_out(c18a)), "1"))))),
      "s18 wrapper scripts do not leak per call")

    # ---- s19 a finished job's out survives the next call (BISE-122) ----
    # the job ends, then a command prints something and cats its out:
    # it gets a fresh slot (5), so it reads the job's line, once
    +r19 : Xp.ExecRes <- Sh.bash_exec("sleep 3; echo job-done")
    +u19w : Unit <- IO.sleep(3500)
    +c19 : Xp.ExecRes <- Sh.bash_exec(String.append("echo before; cat ",
      slot("5", ".out")))
    u19 : Unit <- check(Bool.and(res_ok(c19),
        Bool.and(String.contains(res_out(c19), "before
job-done"),
          Nat.is_lt(String.length(res_out(c19)), 100n))),
      "s19 a finished job's out is never recycled")

    # ---- s20 what comes back is bounded at 1 MiB, with the real size ----
    +r20 : Xp.ExecRes <- Sh.bash_exec("head -c 3000000 /dev/zero | tr '\\0' a")
    u20 : Unit <- check(Bool.and(res_ok(r20),
        Bool.and(String.starts_with(res_out(r20),
            "[bash: the output is 3000000 bytes; only its first 1048576 follow."),
          Nat.is_lt(String.length(res_out(r20)), 1049000n))),
      "s20 a 3 MB output comes back cut at 1 MiB")

    # ---- s21 the command does not see the REPL's session vars ----
    # (run the suite with BEND_SESSION_FILE and BEND_WIRE_LOG set)
    +r21 : Xp.ExecRes <- Sh.bash_exec("echo \"env:[${BEND_SESSION_FILE-}][${BEND_WIRE_LOG-}]\"")
    u21 : Unit <- check(String.contains(res_out(r21), "env:[][]"),
      "s21 REPL session vars unset in the command")

    # ---- s22 stdin is /dev/null: readers get EOF, never the window ----
    # (issue #9: the stdin was a fifo that never closed, so rg with no
    # path, cat or read waited out the whole sync window, then handed
    # off). Each one ends in sync, well inside the 2 s window.
    +r22a : Xp.ExecRes <- Sh.bash_exec("read x; echo read-rc:$?")
    +r22b : Xp.ExecRes <- Sh.bash_exec("cat; echo cat-done")
    +r22c : Xp.ExecRes <- Sh.bash_exec("d=$(mktemp -d); cd $d; echo foo > f; if command -v rg >/dev/null; then rg foo; else grep -r foo; fi; echo rg-rc:$?; rm -rf $d")
    u22 : Unit <- check(Bool.and(String.contains(res_out(r22a), "read-rc:1"),
        Bool.and(String.contains(res_out(r22b), "cat-done"),
          Bool.and(String.contains(res_out(r22c), "rg-rc:0"),
            Bool.not(String.contains(String.append(res_out(r22a),
              String.append(res_out(r22b), res_out(r22c))), "background:"))))),
      "s22 stdin readers (read, cat, rg with no path) end in sync")

    # ---- s14 the agent rm -rf's its own bg dir: fast failure, no hang ----
    +r14 : Xp.ExecRes <- Sh.bash_exec(String.append("rm -rf ", bg()))
    u14 : Unit <- check(Bool.and(Bool.not(res_ok(r14)),
        String.contains(res_out(r14), "the bg dir vanished")),
      "s14 bg dir cleanup fails fast, never hangs")

    fin : Unit <- IO.print("---- bg suite done\n")
    IO.pure(Unit, Unit{})
