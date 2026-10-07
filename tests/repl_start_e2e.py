#!/usr/bin/env python3
"""REPL starts after a switch (crate::repl_start): a few at a time, judged on
progress, a slow start is not a death.

A switch to 7fc455f7 reloaded 22 REPLs in one tick at load 16; their starts
took 22-38 s and a flat 20 s limit failed the switch. A real hub, 8 agents
(main + 7 tasks), the scripted provider, `SB_SLOW_SPAWN` (a file: the ms
each start waits before its spawn, with no progress):
  1. a reload (`/restart` outside bise's tree) with 8 s per start: at most 4
     REPLs switch at once (hub.log's switching/switched lines), every one
     switches, the probation passes (no rollback);
  2. a reload whose REPLs never start (a delay far past the 45 s without
     progress): the stalls report to probation and the switch rolls back.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import Client, Env, check  # noqa: E402

TASKS = ["t%d" % i for i in range(1, 8)]


def log_lines(E):
    try:
        return open(os.path.join(E.state, "hub.log")).read().splitlines()
    except FileNotFoundError:
        return []


def most_at_once(lines):
    """The most REPLs between 'switching the REPL of X' and its
    'switched the REPL of X' (or its failed start) at one time."""
    open_, most = set(), 0
    for l in lines:
        m = re.search(r"switching the REPL of (\S+) to", l)
        if m:
            open_.add(m.group(1))
            most = max(most, len(open_))
        m = re.search(r"switched the REPL of (\S+)$", l) or re.search(r"repl (\S+) not started", l)
        if m:
            open_.discard(m.group(1))
    return most


def hub_pid(E):
    try:
        return open(os.path.join(E.state, "hub.pid")).read().strip()
    except FileNotFoundError:
        return ""


def follow(E, c, old_pid):
    """The client of the hub that replaced `old_pid`, at once: a UI stays
    on it, as the TUI does (no UI for 120 s: the hub idle-exits)."""
    sock = os.path.join(E.state, "hub.sock")
    box = []

    def up():
        if hub_pid(E) in ("", old_pid):
            return False
        try:
            box.append(Client(sock))
            return True
        except OSError:
            return False
    c.wait(up, 120, "the new hub's socket")
    return box[-1]


def main():
    E = Env()
    slow = os.path.join(E.tmp, "slow-spawn-ms")
    open(slow, "w").write("0")
    E.env["SB_SLOW_SPAWN"] = slow
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        # one spawn per message: a task's reply cuts main's turn, and the
        # scripted provider drops the rest of a cut turn's calls
        # (a reply landing at the same moment still cuts it: asked again)
        for t in TASKS:
            for _ in range(4):
                c.wait_idle("main", timeout=120)
                c.say('[[bash: sb spawn %s --objective "{{bash: echo %s-ok}}"]]' % (t, t))
                try:
                    c.wait(lambda t=t: c.agent(t) is not None, 30, "%s exists" % t)
                    break
                except AssertionError:
                    continue
            check(c.agent(t) is not None, "%s spawned" % t)
        c.wait_idle("main", *TASKS, timeout=180)
        n0 = len(log_lines(E))

        # 1. a slow but healthy reload
        open(slow, "w").write("8000")
        pid0 = hub_pid(E)
        c.send({"op": "version", "do": "restart"})
        c = follow(E, c, pid0)
        c.wait(lambda: any(re.search(r"switch: \S+ (good|failed)", l) for l in log_lines(E)[n0:]), 300,
               "the reload's probation ends")
        after = log_lines(E)[n0:]
        check(not any(" failed" in l and "switch:" in l for l in after), "no rollback: %s" % [l for l in after if "switch:" in l])
        switched = {m.group(1) for l in after for m in [re.search(r"switched the REPL of (\S+)$", l)] if m}
        check(switched >= set(TASKS) | {"main"}, "every REPL switched: %s" % sorted(switched))
        most = most_at_once(after)
        check(0 < most <= 4, "at most 4 starts at once: %d" % most)

        # 2. a version whose REPLs never start rolls back
        c.wait_idle("main", *TASKS, timeout=120)
        n1 = len(log_lines(E))
        open(slow, "w").write("100000000")
        pid1 = hub_pid(E)
        c.send({"op": "version", "do": "restart"})
        c = follow(E, c, pid1)
        c.wait(lambda: any(re.search(r"switch: \S+ failed: .*no progress", l) for l in log_lines(E)[n1:]), 240,
               "the stalled reload rolls back")
        # the rollback hub's REPLs start again
        open(slow, "w").write("0")
        after = log_lines(E)[n1:]
        check(any("rollback to" in l for l in after), "a rollback: %s" % [l for l in after if "switch:" in l])
        print("repl starts: ok (%d at once at most; a never-starting version rolled back)" % most)
    finally:
        E.close()


if __name__ == "__main__":
    main()
