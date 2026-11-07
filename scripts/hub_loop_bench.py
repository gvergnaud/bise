#!/usr/bin/env python3
"""The hub's loop on a fake hub of 25 agents (chantier c4-loop-never-waits):
how much of the time it is held and the state push's median, measured
before and after each land. A measure, not a test: run it through
`scripts/heavy-run --low`.

  scripts/heavy-run --low python3 -u scripts/hub_loop_bench.py [--tasks 25] [--archived 25] [--secs 120]

A throwaway hub (tests/e2e.py: the scripted provider, its own HOME), TASKS
live tasks and ARCHIVED archived ones (his hub keeps every archived agent
in its state), then SECS of work: main's `sb list` every 200 ms and a
message from main to a task every 2 s (its answer: feed lines, state
pushes). The hub logs its loop stats every 30 s (SB_LOOP_STATS_MS);
scripts/hub_loop_held.py reads its hub.log at the end.
"""
import argparse
import os
import subprocess
import sys
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tests"))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import e2e  # noqa: E402
import hub_loop_held  # noqa: E402

STATS_MS = 30_000


def sb(E, agent, *args):
    env = {**E.env, "SB_SOCKET": os.path.join(E.state, "agent.sock"), "SB_AGENT": agent}
    return subprocess.run([e2e.EXE, "sb", *args], env=env, cwd=E.ws, capture_output=True, text=True, timeout=180)


def lister(E, stop, took):
    """main's `sb list` every 200 ms: what an agent waits for an answer."""
    while not stop.is_set():
        t0 = time.time()
        sb(E, "main", "list")
        took.append(time.time() - t0)
        stop.wait(0.2)


def main(argv):
    p = argparse.ArgumentParser()
    p.add_argument("--tasks", type=int, default=25)
    p.add_argument("--archived", type=int, default=25)
    p.add_argument("--secs", type=int, default=120)
    p.add_argument("--sample", default="", help="a file: macOS `sample` of the hub during the load")
    a = p.parse_args(argv)
    E = e2e.Env()
    E.env["SB_LOOP_STATS_MS"] = str(STATS_MS)
    # the REPLs of this tree (scripts/bins.sh repl-live repl-scripted sb-core first)
    E.env["BISE_APP_ROOT"] = e2e.ROOT
    print("fake hub in %s (SB_KEEP=1 keeps it)" % E.tmp)
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        t0 = time.time()
        gone = ["a%04d" % i for i in range(a.archived)]
        live = ["t%02d" % i for i in range(1, a.tasks + 1)]

        def spawn(names):
            for n in names:
                r = sb(E, "main", "spawn", n, "--objective", "stay around for the hub-loop bench")
                assert r.returncode == 0, r.stdout + r.stderr
            for n in names:
                c.wait_status(n, "idle", 300)
        # the archived ones by 25: never more REPLs at once than that
        for k in range(0, len(gone), 25):
            spawn(gone[k:k + 25])
            for n in gone[k:k + 25]:
                r = sb(E, "main", "drop", n)
                assert r.returncode == 0, r.stdout + r.stderr
        spawn(live)
        print("%d live, %d archived after %.0f s" % (len(live), len(gone), time.time() - t0))
        start = time.time()
        stop, took = threading.Event(), []
        th = threading.Thread(target=lister, args=(E, stop, took), daemon=True)
        th.start()
        if a.sample:
            # where the hub's CPU goes meanwhile (macOS `sample`, 20 s, 1 ms)
            subprocess.Popen(["sample", str(E.hub.pid), "20", "1", "-file", a.sample],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        i = 0
        while time.time() - start < a.secs:
            sb(E, "main", "send", live[i % len(live)], "bench message %d" % i)
            i += 1
            stop.wait(2.0)
        # the last window's stats line
        stop.wait(STATS_MS / 1000 + 2)
        stop.set()
        th.join(10)
        took.sort()
        med = took[len(took) // 2] if took else 0
        print("sb list: %d answers, median %.3f s, worst %.3f s" % (len(took), med, took[-1] if took else 0))
        with open(os.path.join(E.state, "hub.log")) as f:
            lines = f.read().splitlines()
        for l in lines:
            if " hub loop " in l:
                print("  " + l.split(" ", 1)[1][:300])
        rs = hub_loop_held.runs(lines)
        s = hub_loop_held.summary(rs, since_ms=int(start * 1000))
        print("RESULT busy %.2f %% (every message), held >= 1 s %.2f %%, pushes %d p50 %s ms, blocks over 5 s %d"
              % (s["busy_pct"] or 0, s["held_pct"], s["pushes"], s["push_p50_ms"], s["blocks_over_5s"]))
    finally:
        E.close()


if __name__ == "__main__":
    main(sys.argv[1:])
