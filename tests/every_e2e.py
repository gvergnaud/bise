"""`sb every`, the hub's timers (docs/ambient-roadmap.md B, standing orders).

A real hub (bise sbd, scripted model): main sets a timer that wakes a
task every second (SB_EVERY_MIN_MS lowers the 1-minute minimum for the
test); the task gets the wakes from `bise`; each wake keeps it busy 4 s:
no wake reaches it mid-turn and only one waits for its idle (never
stacked: a few wakes in 12 s, not one per second); `sb every` lists it,
`sb every --stop` ends it; a daily timer (`sb every day 07:30`) and a
short one show in `sb tasks`, come back after a hub restart and the
short one fires again; a dropped task's timers stop with it.

Run: python3 -u tests/every_e2e.py
"""
import json
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, check  # noqa: E402

WAKE = "timer #1 "
# since sched-names (0e5a18bb) a timer's set line and its `sb every` row
# carry its name between the agent and the rhythm:
# '#1 @t1  <name> · every 1s · next 12:16 · by main'
def row(n, agent, rhythm):
    return re.compile(r"#%d @%s  [^·]+ · %s" % (n, re.escape(agent), re.escape(rhythm)))


SET_LINE = row(1, "t1", "every 1s · ")


def wakes(c, agent="t1"):
    """The timer #1 messages `agent` received (its feed's msg-in lines)."""
    return [l for l in c.lines(agent) if l.startswith("sb msg-in") and WAKE in l]


def main_says(c, cmd, needle, timeout=90):
    """main runs one command; the lines of its turn from the first
    tool result on (a result can span several lines), joined."""
    c.wait_idle("main")
    n = len(c.lines("main"))

    def out():
        new = c.lines("main")[n:]
        first = next((i for i, l in enumerate(new) if l.startswith("tool_result")), None)
        return "" if first is None else "\n".join(new[first:])

    c.say("[[bash: %s]]" % cmd)
    c.wait(lambda: needle in out(), timeout, needle)
    c.wait_idle("main")
    return out()


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    E.env["SB_EVERY_MIN_MS"] = "1000"
    ok = True
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say("/new t1: stay around")
        c.wait_status("t1", "idle", 90)

        # a timer needs a duration
        r = main_says(c, 'sb every 0s "x" --to t1', "not a duration")
        check("exit" in r, "0s refused: %r" % r)

        # every 1 s, each wake keeps t1 busy 4 s (the message's own script,
        # written with printf: main's command cannot hold the markers)
        r = main_says(c, "printf '\\133\\133bash: sleep 4\\135\\135 ping the board' | sb every 1s - --to t1", "timer set")
        check(SET_LINE.search(r) is not None and "sb every --stop 1" in r, "the set line: %r" % r)
        c.wait(lambda: len(wakes(c)) >= 1, 30, "a first wake")
        t0 = time.time()
        # three wakes (about 8 s when each keeps t1 busy 4 s), then the
        # same bound on the rate over the time they took: one per second
        # reaches 3 in ~2 s, past the bound
        n = len(wait.until(lambda: (lambda w: len(w) >= 3 and w)(wakes(c)), 20, "3 wakes of timer #1"))
        spent = time.time() - t0 + 5
        check(2 <= n <= spent / 4 + 1, "busy 4 s per wake: %d wakes in %.0f s, never one per second" % (n, spent))
        check(not any("steer" in l and WAKE in l for l in c.lines("t1")), "no wake mid-turn")

        r = main_says(c, "sb every", "@t1")
        check(SET_LINE.search(r) is not None, "sb every lists it: %r" % r)

        # stop: no more wakes
        r = main_says(c, "sb every --stop 1", "stopped")
        check("timer #1 stopped" in r, "stop: %r" % r)
        c.wait_idle("t1")
        n3 = len(wakes(c))
        wait.holds(lambda: len(wakes(c)) == n3, 6, "no wake after the stop (%d before)" % n3)
        r = main_says(c, "sb every --stop 1", "no timer")
        check("no timer #1" in r, "stop twice: %r" % r)

        # a daily timer, a short one; sb tasks shows them
        r = main_says(c, 'sb every day 07:30 "make the morning page"', "timer set")
        check(row(2, "main", "every day 07:30").search(r), "a daily timer: %r" % r)
        r = main_says(c, 'sb every 3s "pong" --to t1', "timer set")
        check(row(3, "t1", "every 3s").search(r), "a third timer: %r" % r)
        r = main_says(c, "sb tasks", "#3 @t1")
        check("## timers (sb every)" in r and "#2 @main" in r, "sb tasks shows them: %r" % r)

        # a restart: the timers come back from the journal and fire again
        E.stop_hub()
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        r = main_says(c, "sb every", "#3")
        check(row(2, "main", "every day 07:30").search(r) and row(3, "t1", "every 3s").search(r), "both back after the restart: %r" % r)
        c.wait(lambda: any(l.startswith("sb msg-in") and "timer #3 " in l for l in c.lines("t1")), 60, "a wake after the restart")

        # a dropped task's timers stop with it (busy or not: --force)
        c.say("/archive t1 --force")
        c.wait_status("t1", ["archived", "stopped"], 60)

        def stopped3():
            journal = [json.loads(l) for l in open(os.path.join(E.state, "journal.jsonl")) if '"every_' in l]
            return any(j.get("type") == "every_stop" and j.get("id") == 3 for j in journal)
        wait.until(stopped3, 30, "the stop of t1's timer #3 in the journal")
        r = main_says(c, "sb every", "#2")
        check("#3 @t1" not in r and "#2 @main" in r, "t1's timer is gone, main's stays: %r" % r)

        # a one-shot day timer to an idle main (amb-tools m_5822): the wake
        # reaches main as a turn, and only then has it run its times
        at = time.strftime("%H:%M", time.localtime(time.time() + 60))
        r = main_says(c, 'sb every day %s "ONE-SHOT brief" --times 1 --until 23:59' % at, "timer set")
        check(row(4, "main", "every day %s" % at).search(r), "the one-shot: %r" % r)
        c.wait(lambda: any(l.startswith("sb msg-in") and "ONE-SHOT brief" in l for l in c.lines("main")), 150, "main's wake")
        c.wait_idle("main")

        # sb-core writes the stop in the step that sent the wake: a client
        # that reads the wake first may be ahead of the file, so wait for
        # both lines, never read once (as core_restart)
        def spent():
            journal = [json.loads(l) for l in open(os.path.join(E.state, "journal.jsonl")) if l.strip()]
            sent = [k for k, j in enumerate(journal) if j.get("type") == "message_sent" and "ONE-SHOT" in j["msg"]["text"]]
            stop = [k for k, j in enumerate(journal) if j.get("type") == "every_stop" and j.get("id") == 4]
            return journal, sent, stop
        wait.until(lambda: spent()[1] and spent()[2], 30, "the one-shot's wake and its stop in the journal")
        journal, sent, stop = spent()
        check(sent and stop and sent[0] < stop[0] and journal[stop[0]]["why"] == "it ran its times",
              "sent, then spent: %r %r" % (sent, stop))
    except AssertionError as e:
        print("FAIL", e)
        if os.environ.get("EVERY_DEBUG"):
            print("\n".join(c.lines("t1")[-60:]))
        ok = False
    finally:
        E.close()
    print("every_e2e:", "ok" if ok else "FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
