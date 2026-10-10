"""sb-core dies under a live hub (BISE-292): the hub restarts it on the
journal instead of dying with it.

A real hub (bise sbd) with main, a task t1 and a one-shot timer for t1
(`sb every 20s ... --times 1`). sb-core is killed (-9). The hub lives on:
main's feed says sb-core stopped and was restarted, t1 is still there, a
message to main still gets its answer, a new sb-core runs under the hub,
and the timer, sb-core's since core-timers, comes back from the journal
and wakes t1 exactly once.

python3 -u tests/core_restart.py
"""
import json
import os
import signal
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from e2e import EXE, check  # noqa: E402


def cores_of(pid):
    """The sb-core children of the hub `pid`."""
    out = subprocess.run(["ps", "-axww", "-o", "pid=,ppid=,command="], capture_output=True, text=True).stdout
    got = []
    for l in out.splitlines():
        f = l.split(None, 2)
        if len(f) == 3 and f[1] == str(pid) and "sb-core" in f[2]:
            got.append(int(f[0]))
    return got


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    # a timer's period in seconds (a real hub takes a minute at least)
    E.env["SB_EVERY_MIN_MS"] = "1000"
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say('[[bash: sb spawn t1 --objective "stay"]]')
        c.wait(lambda: c.agent("t1") is not None, 60, "t1 exists")
        c.wait_idle("main", "t1")
        # a one-shot timer for t1, due after the restart: sb-core holds it
        # (core-timers), so it must come back from the journal and fire once
        c.say('[[bash: sb every 20s "ONE-SHOT after the restart" --to t1 --times 1]]')
        c.wait(lambda: any("timer set: #1" in l for l in c.lines("main")), 60, "the timer is set")
        c.wait_idle("main")
        old = cores_of(E.hub.pid)
        check(len(old) == 1, "one sb-core under the hub: %s" % old)
        os.kill(old[0], signal.SIGKILL)
        # the next input (a tick at the latest) finds it dead
        c.wait_line("main", "sb-core, the hub's state machine, stopped", 30)
        check(E.hub.poll() is None, "the hub lives on")
        new = cores_of(E.hub.pid)
        check(len(new) == 1 and new != old, "a new sb-core: %s (was %s)" % (new, old))
        check(c.agent("t1") is not None, "t1 is still there (replayed from the journal)")
        n = len(c.lines("main"))
        c.say("hello after the restart")
        c.wait(lambda: any("hello after the restart" in l for l in c.lines("main")[n:]), 30, "main got the message")
        c.wait_idle("main")
        # the timer survived the restart and fires once (then it ran its times)
        def wakes():
            return [l for l in c.lines("t1") if l.startswith("sb msg-in") and "ONE-SHOT after the restart" in l]
        c.wait(lambda: wakes(), 60, "t1's wake after the restart")
        c.wait_idle("main", "t1")

        # the fire's journal lines (sb-core writes them in the step that
        # sent the wake: a client that reads the wake first may be ahead
        # of the file, so wait for them, never read once)
        def journal(kind):
            js = [json.loads(l) for l in open(os.path.join(E.state, "journal.jsonl")) if l.strip()]
            return [j for j in js if j.get("type") == kind and j.get("id") == 1]
        c.wait(lambda: journal("every_fired") and journal("every_stop"), 30, "the fire and its stop in the journal")
        fired, stops = journal("every_fired"), journal("every_stop")
        check(len(wakes()) == 1 and len(fired) == 1, "one wake, one fire: %r %r" % (wakes(), fired))
        check(len(stops) == 1 and stops[0]["why"] == "it ran its times", "then it ran its times: %r" % stops)
        log = open(os.path.join(E.state, "hub.log")).read()
        check("sb-core stopped" in log and "sb-core restarted" in log, "hub.log says so")
        ok = True
        print("PASS core_restart")
    finally:
        if not ok and E.hub:
            print(open(os.path.join(E.tmp, "hub.stderr")).read()[-2000:])
        E.close()


if __name__ == "__main__":
    main()
