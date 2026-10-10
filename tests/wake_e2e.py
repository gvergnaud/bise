"""`sb wake` (event-wake): an agent wakes on an event, not a timer.

A real hub (bise sbd, scripted model), a throwaway home:
- t1's bash command goes to the background (BEND_BG_AFTER 2 s), t1 ends
  its turn; when the command ends (WAKE_SECS, 120 by default) bise wakes
  t1 once, with its rc and its last lines, within ~2 s of its end;
- `sb wake --on-exit <pid>`: the end of a process wakes t1 the same way;
- `sb wake --on-file <path>`: the file appearing wakes t1, with its rc,
  and a t1 busy at that moment gets it after its turn, never mid-turn;
- a watch survives a hub restart; one wake per event (the watch ends);
- a dropped task's watches end with it (wake_end `gone` in the journal).

Run: python3 -u tests/wake_e2e.py   (WAKE_SECS=20 for a quick run)
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, check  # noqa: E402

SECS = int(os.environ.get("WAKE_SECS", "120"))
LATE = 2.5  # the wake within ~2 s of the end (a look every second, a tick every 0.5 s)


def wakes(c, agent, needle):
    """The messages from the hub to `agent` whose first line holds `needle`."""
    return [l for l in c.lines(agent) if l.startswith("sb msg-in") and needle in l]


def arrival(c, agent, needle, timeout):
    """When the first wake holding `needle` reached `agent` (this clock)."""
    wait.until(lambda: wakes(c, agent, needle), timeout, "a wake %r for %s" % (needle, agent), poll=0.05)
    return time.time()


def end_time(path):
    return float(open(path).read().strip())


def journal(E, kind):
    return [json.loads(l) for l in open(os.path.join(E.state, "journal.jsonl")) if '"%s"' % kind in l]


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    E.env["BEND_BG_AFTER"] = "2"
    ok = True
    c = None
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say("/new t1: stay around")
        c.wait_status("t1", "idle", 90)
        stamp = "python3 -c 'import time;print(time.time())'"

        # 1. a backgrounded bash command: its end wakes t1, once
        end1 = os.path.join(E.tmp, "end1")
        c.say("[[bash: sleep %d; %s > %s; echo LAST-LINE-OK]]" % (SECS, stamp, end1), focus="t1")
        c.wait(lambda: any(l.startswith("bg_handoff : ") for l in c.lines("t1")), 60, "the handoff's wire line")
        c.wait_idle("t1")
        t = arrival(c, "t1", "background 0 ended", SECS + 60)
        late = t - end_time(end1)
        w = wakes(c, "t1", "background 0 ended")[0]
        check("background 0 ended · rc 0 · after" in w and "· sleep %d" % SECS in w, "the wake says what and its rc: %r" % w)
        k = c.lines("t1").index(w)
        c.wait(lambda: any(l.strip() == "LAST-LINE-OK" for l in c.lines("t1")[k:]) or any("LAST-LINE-OK" in l for l in c.lines("t1")[k:k + 1]),
               10, "its last lines in the wake")
        check(late <= LATE, "woken %.2f s after the end (at most %.1f)" % (late, LATE))
        print("bg: woken %.2f s after the end of a %d s command" % (late, SECS))
        c.wait_idle("t1")
        wait.holds(lambda: len(wakes(c, "t1", "background 0 ended")) == 1, 4, "one wake for one end")

        # 2. --on-exit: a process the agent names
        end2 = os.path.join(E.tmp, "end2")
        c.say("[[bash: (sleep 6; %s > %s) >/dev/null 2>&1 & sb wake --on-exit $! --note PID-NOTE]]" % (stamp, end2), focus="t1")
        c.wait(lambda: any("watch #" in l and "set" in l for l in c.lines("t1")), 60, "the pid watch set")
        c.wait_idle("t1")
        t = arrival(c, "t1", "PID-NOTE", 60)
        late = t - end_time(end2)
        check(late <= LATE, "--on-exit: woken %.2f s after the end" % late)
        print("pid: woken %.2f s after the end" % late)

        # 3. --on-file, the agent busy when it appears: after its turn
        rc = os.path.join(E.tmp, "rc")
        c.wait_idle("t1")
        c.say("[[bash: sb wake --on-file %s --note FILE-NOTE]]" % rc, focus="t1")
        c.wait(lambda: any("watch #" in l and rc in l for l in c.lines("t1")), 60, "the file watch set")
        c.wait_idle("t1")
        c.say("[[bash: sleep 6]]", focus="t1")
        # t1 is mid-turn (its sleep 6) when the file appears
        c.wait_status("t1", "working", 30)
        with open(rc, "w") as f:
            f.write("3\n")
        c.wait_idle("t1", timeout=60)
        arrival(c, "t1", "FILE-NOTE", 30)
        w = wakes(c, "t1", "FILE-NOTE")[0]
        check("rc 3" in w, "the rc file's rc: %r" % w)
        # never steered into the busy turn: the wake comes after that turn's end
        ls = c.lines("t1")
        k_sleep = max(i for i, l in enumerate(ls) if l.startswith("tool #") and l.endswith("bash : sleep 6"))
        k_done = next(i for i in range(k_sleep, len(ls)) if ls[i].strip().startswith("obs: turn_done"))
        check(ls.index(w) > k_done, "the wake after the busy turn's end (wake #%d, turn_done #%d)" % (ls.index(w), k_done))

        # 4. a restart keeps a watch
        rc2 = os.path.join(E.tmp, "rc2")
        c.wait_idle("t1")
        c.say("[[bash: sb wake --on-file %s --note RESTART-NOTE]]" % rc2, focus="t1")
        c.wait(lambda: any("watch #" in l and rc2 in l for l in c.lines("t1")), 60, "the second file watch")
        c.wait_idle("t1")
        E.stop_hub()
        c = E.start_hub()
        c.wait_status("t1", "idle", 90)
        with open(rc2, "w") as f:
            f.write("0\n")
        arrival(c, "t1", "RESTART-NOTE", 30)

        # 5. a dropped task's watches end with it
        c.say("/new t2: stay around")
        c.wait_status("t2", "idle", 90)
        c.say("[[bash: sb wake --on-file %s --note NEVER]]" % os.path.join(E.tmp, "never"), focus="t2")
        c.wait(lambda: any("watch #" in l for l in c.lines("t2")), 60, "t2's watch")
        c.wait_idle("t2")
        live = [j["watch"]["id"] for j in journal(E, "wake_set") if j["watch"]["agent"] == "t2"]
        c.say("/archive t2 --force")
        c.wait_status("t2", ["archived", "stopped"], 60)
        wait.until(lambda: any(j["id"] in live and j["why"] == "gone" for j in journal(E, "wake_end")), 30, "t2's watch ended")
    except AssertionError as e:
        print("FAIL", e)
        if os.environ.get("WAKE_DEBUG") and c:
            print("\n".join(c.lines("t1")[-60:]))
        ok = False
    finally:
        E.close()
    print("wake_e2e:", "ok" if ok else "FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
