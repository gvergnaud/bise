"""idle-exit (docs/idle-exit.md): a hub with no UI stops by itself, and
nothing is lost.

A real hub (bise sbd) on the fake provider, with a grace of 4 s
(BISE_IDLE_EXIT) and a REPL hub watch of 3 s (BEND_HUB_GONE_MS):
- a UI back within the grace: the hub stays;
- the last UI leaves while t1 is mid-turn: the hub waits for the turn,
  then stops, and every REPL with it (checkpointed: "REPLs saved");
- the next start: main and t1 are back with their feeds, and answer;
- a background job of t1 (BEND_BG_AFTER 2 s): the hub waits for it,
  the job ends on its own (its .rc), then the hub stops;
- a hub killed -9 leaves its REPLs: they exit by themselves (no hub for
  the watch's 10 looks), and the next hub brings the agents back.

Run: python3 -u tests/idle_exit_e2e.py
"""
import os
import signal
import socket
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, check  # noqa: E402

GRACE = 4


def repls(state):
    """The live processes started for this hub (their SB_SOCKET)."""
    out = subprocess.run(["ps", "-axww", "-E", "-o", "pid=,command="], capture_output=True, text=True).stdout
    needle = "SB_SOCKET=%s/hub.sock" % state
    got = []
    for l in out.splitlines():
        pid, _, rest = l.strip().partition(" ")
        if pid.isdigit() and needle in rest and not rest.startswith("ps "):
            got.append(int(pid))
    return got


def log(E):
    try:
        return open(os.path.join(E.state, "hub.log")).read()
    except FileNotFoundError:
        return ""


def leave(c):
    """The UI goes (the TUI quits, its terminal closes)."""
    # shutdown: close() alone keeps the fd its reader's makefile holds
    try:
        c.s.shutdown(socket.SHUT_RDWR)
        c.s.close()
    except OSError:
        pass


def stopped(E):
    return E.hub is not None and E.hub.poll() is not None


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    E.env["BISE_IDLE_EXIT"] = str(GRACE)
    E.env["BEND_HUB_GONE_MS"] = "3000"
    E.env["BEND_BG_AFTER"] = "2"
    ok = True
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say("/new t1: {{bash: echo first-turn}}")
        c.wait(lambda: c.agent("t1"), 60, "t1")
        c.wait_idle("t1", "main")
        check("idle exit: after %d s" % GRACE in log(E), "the hub says its grace")

        # a quick relaunch: back within the grace, the hub stays
        n = log(E).count("idle exit: a UI is back")
        leave(c)
        wait.until(lambda: "idle exit: no UI left" in log(E), GRACE, "the hub seeing the UI leave")
        c = e2e.Client(os.path.join(E.state, "hub.sock"))
        wait.until(lambda: log(E).count("idle exit: a UI is back") > n, GRACE, "the hub seeing the UI back")
        # past the grace: still up
        wait.holds(lambda: E.hub.poll() is None, GRACE + 2, "a UI back within the grace: the hub stays")

        # the last UI leaves while t1 is mid-turn: the hub waits for it
        c.say("[[slow: 12]] long turn", focus="t1")
        c.wait_status("t1", "working", 30)
        leave(c)
        wait.until(lambda: "the hub waits for: t1 mid-turn" in log(E), GRACE + 10, "the hub waiting for t1's turn")
        check(E.hub.poll() is None, "no stop mid-turn")
        wait.until(lambda: stopped(E), 60, "the hub stopped after t1's turn")
        L = log(E)
        check("nothing runs: the hub stops" in L, "the stop says why")
        check("REPLs saved and gone: 2 of 2" in L, "both REPLs checkpointed and exited: %s" % L[-600:])
        wait.until(lambda: not repls(E.state), 15, "no process of the hub left")
        E.hub = None

        # the next start: everything is back
        c = E.start_hub()
        c.wait(lambda: c.state is not None, 30, "state")
        check([a["name"] for a in c.state["agents"]] == ["main", "t1"], "main and t1 are back")
        c.wait(lambda: any("first-turn" in l for l in c.lines("t1")), 30, "t1's feed is back")
        c.wait_idle("main", "t1")
        c.say("after the stop", focus="t1")
        c.wait_line("t1", "ack: after the stop", 60)
        check("its turn was interrupted" not in "\n".join(c.lines("t1")[-5:]), "no turn resumed by mistake")

        # a background job holds the hub; it ends on its own, then the stop
        bg = os.path.join(E.state, "agents", "t1", "tmp", "bg")
        c.say("[[bash: sleep 10; echo bg-done]]", focus="t1")
        c.wait(lambda: os.path.isdir(bg) and any(f.endswith(".slot") for f in os.listdir(bg)), 30, "t1's job in the background")
        c.wait_idle("t1")
        leave(c)
        wait.until(lambda: "t1's background job" in log(E), GRACE + 10, "the hub waiting for t1's job")
        wait.until(lambda: stopped(E), 60, "the hub stopped after the job")
        check(any(f.endswith(".rc") for f in os.listdir(bg)), "the job ran to its end (its .rc)")
        wait.until(lambda: not repls(E.state), 15, "no process of the hub left")
        E.hub = None

        # a hub killed -9: its REPLs see no hub and exit by themselves
        E.env["BISE_IDLE_EXIT"] = "off"
        c = E.start_hub()
        c.wait_idle("main", "t1", timeout=60)
        check(len(repls(E.state)) >= 2, "the REPLs run")
        os.kill(E.hub.pid, signal.SIGKILL)
        E.hub.wait()
        E.hub = None
        wait.until(lambda: not repls(E.state), 30, "the orphan REPLs exited")
        # the dead hub's socket file stays (a `bise` finds it refused and
        # starts a hub, which replaces it); start_hub waits for the file
        os.remove(os.path.join(E.state, "hub.sock"))
        c = E.start_hub()
        c.wait(lambda: c.state is not None, 30, "state")
        c.wait_idle("main", "t1", timeout=60)
        c.say("after the crash", focus="t1")
        c.wait_line("t1", "ack: after the crash", 60)
        print("PASS idle_exit_e2e", flush=True)
    except Exception as e:
        ok = False
        print("FAIL idle_exit_e2e: %s" % e, flush=True)
        print(log(E)[-3000:], flush=True)
        os.environ["SB_KEEP"] = "1"
    finally:
        E.close()
        for p in repls(E.state):
            try:
                os.kill(p, 9)
            except OSError:
                pass
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
