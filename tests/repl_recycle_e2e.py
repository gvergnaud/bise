#!/usr/bin/env python3
"""repl-cpu-3: the hub recycles an aged REPL at idle, and nobody sees it.

The Bend runtime's allocator ages a long-lived REPL (each model call costs
more CPU than the last, rust/switchboard/src/recycle.rs): the hub counts
the context each REPL read from its usage lines and, past
BISE_RECYCLE_TOKENS, restarts it at its next idle through the switch path
(reload, same binary, port and session). A real hub, real REPLs, the
scripted provider (e2e.Env, 10 input tokens a call); budget 150 tokens:
  1. main runs one turn of 25 model calls (a background job, then 23
     echoes): the hub recycles its REPL exactly once, at idle; repl.pid
     and repl.json name the new process, same port; RSS is lower;
  2. a message sent right at the turn's end (during the recycle) is
     answered exactly once;
  3. the background job started before the recycle is still readable by
     the new process;
  4. the transcript has every step once, no reload line, no history
     replayed; the session log has no reload event;
  5. no second recycle while the new process reads less than the budget.
"""
import json
import re
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import Env, check  # noqa: E402


def rss_kb(pid):
    out = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
    return int(out) if out else 0


def main():
    E = Env()
    E.env.update(BISE_RECYCLE_TOKENS="150", BEND_BG_AFTER="2")
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        adir = os.path.join(E.state, "agents", "main")
        hub_log = os.path.join(E.state, "hub.log")

        def pid():
            try:
                return int(open(os.path.join(adir, "repl.pid")).read().strip())
            except (OSError, ValueError):
                return 0

        def recycles():
            try:
                return [l for l in open(hub_log) if "recycling the REPL of main" in l]
            except FileNotFoundError:
                return []

        pid0 = pid()
        port0 = json.load(open(os.path.join(adir, "repl.json")))["port"]
        # 1. 25 calls: a job past the 2 s window (its slot: tmp/bg/0), 23 echoes
        steps = ["sleep 4; echo bg-job-done"] + ["echo step-%02d" % i for i in range(23)]
        c.say(" ".join("[[bash: %s]]" % s for s in steps))
        c.wait_line("main", "done: tool bash ok: step-22", 120)
        rss0 = rss_kb(pid0)
        # 2. a message at once: it lands while the hub recycles the REPL
        c.say("after the recycle")
        c.wait(lambda: recycles(), 30, "the hub recycles main's REPL")
        c.wait(lambda: pid() not in (0, pid0), 30, "main's new REPL process")
        c.wait_line("main", "ack: after the recycle", 60)
        c.wait_idle("main")
        pid1 = pid()
        info = json.load(open(os.path.join(adir, "repl.json")))
        check(info["pid"] == pid1 and info["port"] == port0,
              "repl.json names the new process on the same port: %r (old pid %d port %d)" % (info, pid0, port0))
        check(subprocess.run(["kill", "-0", str(pid0)], capture_output=True).returncode != 0, "the old process is gone")
        check(len(recycles()) == 1, "one recycle: %r" % recycles())
        rss1 = rss_kb(pid1)
        check(0 < rss1 < rss0, "RSS down after the recycle: %d KB -> %d KB" % (rss0, rss1))
        acks = [l for l in c.lines("main") if "ack: after the recycle" in l]
        check(len(acks) == 1, "the message sent during the recycle is answered once: %r" % acks)
        asked = [r for r in E.fake_requests() if r.get("user") == "after the recycle"]
        check(len(asked) == 1, "one model call for it: %d" % len(asked))
        # 3. the job of the old process, read by the new one
        slot = os.path.join(adir, "tmp", "bg", "0.out")
        c.wait(lambda: os.path.exists(slot) and "bg-job-done" in open(slot).read(), 30, "the job's output in its slot")
        c.say("[[bash: cat $TMPDIR/bg/0.out]]")
        c.wait_line("main", "done: tool bash ok: bg-job-done", 60)
        c.wait_idle("main")
        # 4. the transcript: each step once, no reload, no replayed history
        tr = open(os.path.join(adir, "transcript.log")).read()
        for i in range(23):
            n = len(re.findall(r"tool_result #\d+ ok : step-%02d " % i, tr))
            check(n == 1, "step-%02d's result once in the transcript: %d" % (i, n))
        check("reload" not in tr, "no reload line in the transcript")
        check(tr.count("after the recycle") == 2, "the message and its answer once each: %d" % tr.count("after the recycle"))
        # 5. the new process read 2 calls' worth: no second recycle
        check(len(recycles()) == 1 and pid() == pid1, "no second recycle: %r" % recycles())
        ok = True
        print("repl recycle: ok (rss %d -> %d KB)" % (rss0, rss1))
    finally:
        if not ok:
            os.environ["SB_KEEP"] = "1"
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
