"""The processes an agent starts die with it (BISE-243).

A real hub (bise sbd). Task t1 starts, through its bash tool, an orphan
sleep, a hub of its own (bise sbd, with its main REPL)
and a tmux server; task t3 a sleep; t2 a sleep; the test itself (the
user) a sleep. /drop t1: all of t1's go, the others stay. sb stop t3:
its sleep goes. The hub quits for good: t2's goes, the user's stays. At
the next start, a process tagged for an agent the hub does not know goes.

Run: python3 -u tests/proc_cleanup.py
"""
import os
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bise_env  # noqa: E402
import e2e  # noqa: E402
from e2e import EXE, check  # noqa: E402


def table():
    """pid -> command line plus environment (ps -E), this user's."""
    out = subprocess.run(["ps", "-axww", "-E", "-o", "pid=,command="], capture_output=True, text=True).stdout
    rows = {}
    for l in out.splitlines():
        pid, _, rest = l.strip().partition(" ")
        if pid.isdigit():
            rows[int(pid)] = rest
    return rows


def alive(needle):
    """The live processes whose command or environment has `needle`."""
    me = os.getpid()
    return [p for p, t in table().items() if needle in t and p != me and not t.startswith("ps ")]


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    # the hubs' sockets must keep their natural place (<state>/hub.sock):
    # the REPLs are found by SB_SOCKET=<child-st>/agent.sock and the ghost by
    # the hash of that path. Under an agent's deep $TMPDIR the path does
    # not fit a unix socket, the hub reaches it through /tmp/bise-<uid>/...
    # and nothing matched: green in the gate (a short TMPDIR), a timeout
    # alone ("t1's hub and its REPL")
    tempfile.tempdir = e2e.short_tmp()
    E = e2e.Env()
    tag = str(os.getpid())  # sleep 901<pid>: a number, and ours
    tsock = "sbpc-%d" % os.getpid()
    child_ws = os.path.join(E.tmp, "child-ws")
    child_st = os.path.join(E.tmp, "child-st")
    os.makedirs(child_ws)
    start = os.path.join(E.tmp, "start.sh")
    open(start, "w").write(
        "set -e\n"
        # a macOS binary (its environment hidden), reparented to pid 1
        # once the shell ends: found by the REPL's session
        "(nohup sleep 901%s </dev/null >/dev/null 2>&1 &)\n"
        # a hub of its own, as a test does (the agent's SB_ variables out)
        "env -u SB_SOCKET -u SB_AGENT -u SB_TASK SB_STATE_DIR=%s nohup %s sbd --workspace %s"
        " </dev/null >/dev/null 2>&1 &\n"
        "tmux -L %s new-session -d -s x 'sleep 902%s'\n"
        # where its tmux socket is (TMUX_TMPDIR: its temp folder, when it fits)
        "echo \"${TMUX_TMPDIR:-}\" > %s\n"
        "echo started\n" % (tag, child_st, EXE, child_ws, tsock, tag, os.path.join(E.tmp, "t1-tmux")))
    user = subprocess.Popen(["sleep", "909" + tag], env=bise_env.clean_env())
    ok = True
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say("/new t1: {{bash: sh %s}}" % start)
        c.say("/new t2: {{bash: nohup sleep 903%s </dev/null >/dev/null 2>&1 & echo ok}}" % tag)
        c.say("/new t3: {{bash: nohup sleep 904%s </dev/null >/dev/null 2>&1 & echo ok}}" % tag)
        c.wait(lambda: c.agent("t1") and c.agent("t2") and c.agent("t3"), 60, "the tasks")
        c.wait_idle("t1", "t2", "t3", "main")
        c.wait(lambda: alive("SB_SOCKET=%s/agent.sock" % child_st) and alive("sbd --workspace " + child_ws),
               60, "t1's hub and its REPL")
        for n in ("901", "902", "903", "904"):
            c.wait(lambda: alive("sleep " + n + tag), 20, "sleep " + n)
        # t1's tmux socket is in its temp folder when it fits (TMUX_TMPDIR,
        # approvals-design.md §7.1), else in /tmp
        t1_dir = open(os.path.join(E.tmp, "t1-tmux")).read().strip()
        t1_tmux = {k: v for k, v in bise_env.clean_env().items() if k != "TMUX_TMPDIR"}
        if t1_dir:
            t1_tmux["TMUX_TMPDIR"] = t1_dir
        check(subprocess.run(["tmux", "-L", tsock, "has-session"], env=t1_tmux).returncode == 0, "t1's tmux")

        c.say("/archive t1")
        c.wait_status("t1", "archived", 30)
        c.wait(lambda: not alive("sleep 901" + tag), 15, "t1's sleep killed")
        c.wait(lambda: not alive("sbd --workspace " + child_ws), 15, "t1's hub killed")
        c.wait(lambda: not alive("SB_SOCKET=%s/agent.sock" % child_st), 15, "t1's hub's REPL killed")
        c.wait(lambda: not alive("sleep 902" + tag), 15, "t1's tmux pane killed")
        check(subprocess.run(["tmux", "-L", tsock, "has-session"], stderr=subprocess.DEVNULL, env=t1_tmux).returncode != 0,
              "t1's tmux server gone")
        check(alive("sleep 903" + tag) and alive("sleep 904" + tag), "t2's and t3's sleeps survive")
        check(user.poll() is None, "the user's sleep survives")

        c.say("[[bash: sb stop t3 enough]]")
        c.wait_status("t3", "stopped", 30)
        c.wait(lambda: not alive("sleep 904" + tag), 15, "t3's sleep killed at its stop")
        check(alive("sleep 903" + tag), "t2's sleep survives t3's stop")

        E.stop_hub()
        check(not alive("sleep 903" + tag), "the hub quit for good: t2's sleep killed")
        check(user.poll() is None, "the user's sleep survives the hub")

        # a process an earlier hub left for an agent it does not know
        sock = os.path.join(E.state, "hub.sock")
        # (a tmux server: macOS hides the environment of its own binaries)
        gsock = tsock + "-ghost"
        subprocess.run(["tmux", "-L", gsock, "new-session", "-d", "-s", "g", "sleep 905" + tag], check=True,
                       env={**bise_env.clean_env(), "BISE_OWNERS": "%s.ghost.1" % e2e.hub_tag(sock)})
        c = E.start_hub()
        c.wait(lambda: not alive("sleep 905" + tag), 20, "the ghost's tmux killed at the start")
        check(user.poll() is None, "the user's sleep survives the start")
        print("PASS proc_cleanup", flush=True)
    except Exception as e:
        ok = False
        print("FAIL proc_cleanup: %s" % e, flush=True)
        os.environ["SB_KEEP"] = "1"
    finally:
        E.close()
        user.kill()
        for t in (tsock, tsock + "-ghost"):
            subprocess.run(["tmux", "-L", t, "kill-server"], stderr=subprocess.DEVNULL)
        for p in alive(tag) + alive(child_st):
            try:
                os.kill(p, 9)
            except OSError:
                pass
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
