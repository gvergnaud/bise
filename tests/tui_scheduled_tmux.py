"""Scheduled tasks in the TUI (site/m/timers), through the real binaries:
a throwaway hub on the fake provider, the real `sb every`, tmux at 150
and 80 columns, dark and light.

  t1 schedules its own (every 2m, 6 times), main schedules one for t2
      (every day 07:30): main's thread has main's ◷ line only; the
      agents panel shows ◷ and the next run on both rows, its legend
  t1's thread: its ◷ scheduled line, never `switchboard`
  /scheduled: the list soonest first, its key bar; ⏎ opens one; r runs
      it now (a ◷ run line in t1's thread, outside the count); x asks on
      the row, n keeps, x y stops: tab shows it ended, t1's thread says
      `ended · stopped by you`, and the note of the stop never shows
  the same screens at 80 columns and in the light palette

SCHED_SHOTS=<dir> keeps the captures (.txt and .ansi) for the designer.

python3 -u tests/tui_scheduled_tmux.py
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402

SHOTS = os.environ.get("SCHED_SHOTS")


def shot(t, name):
    if SHOTS:
        os.makedirs(SHOTS, exist_ok=True)
        with open(os.path.join(SHOTS, name + ".ansi"), "w") as f:
            f.write(t.screen(colors=True))
        with open(os.path.join(SHOTS, name + ".txt"), "w") as f:
            f.write(t.screen())


def sb(E, agent, cwd, *args):
    env = {**E.env, "SB_SOCKET": os.path.join(E.state, "hub.sock"), "SB_AGENT": agent}
    r = subprocess.run([e2e.EXE, "sb", *args], env=env, cwd=cwd, capture_output=True, text=True, timeout=60)
    assert r.returncode == 0, "sb %s: %s %s" % (" ".join(args), r.stdout, r.stderr)
    return r.stdout.strip()


def command(t, line, needle):
    t.typed(line)
    t.wait(line)
    t.keys("Enter")
    return t.wait(needle)


def no_switchboard(sc):
    assert "switchboard" not in sc, "the hub's old name reached the user:\n" + sc


def setup(t, E):
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    for name, what in (("t1", "fix the build"), ("t2", "ship the release")):
        sb(E, "main", E.ws, "spawn", name, "--objective", what)
        t.wait(name)
    out = sb(E, "t1", E.ws, "every", "2m", "check the build and tell me what failed", "--times", "6")
    assert "timer set: #1 @t1 every 2m" in out, out
    out = sb(E, "main", E.ws, "every", "day", "07:30", "ship a release if main landed something", "--to", "t2")
    assert "timer set: #2 @t2 every day 07:30" in out, out


def wide(t, E):
    setup(t, E)
    # main's thread: main's own, never t1's; the panel's ◷ and its legend
    sc = t.wait("main scheduled #2 for t2 · every day 07:30")
    assert "t1 scheduled #1" not in sc, sc
    sc = t.wait("= its next run · /scheduled")
    assert "◷ 2m" in sc and "◷ 07:30" in sc, sc
    no_switchboard(sc)
    shot(t, "150-main-thread")
    # /scheduled: the list, soonest first
    command(t, "/scheduled", "scheduled · what wakes your agents, and when")
    sc = t.wait("2 active   tab ended too")
    assert "/ find: an agent, the words" in sc, sc
    rows = [l for l in sc.splitlines() if "◷ t1" in l or "◷ t2" in l]
    assert len(rows) == 2 and "#1" in rows[0] and "› " in rows[0], sc
    assert "every 2m" in rows[0] and "0 of 6" in rows[0] and "by t1" in rows[0], rows[0]
    assert "every day 07:30" in rows[1] and "by main" in rows[1], rows[1]
    assert "⏎ open   r run now   x stop   / find   tab ended too   esc close" in sc, sc
    shot(t, "150-list")
    # one opened
    t.keys("Enter")
    sc = t.wait("scheduled task #1 · t1")
    for want in ("wakes       t1", "set by      t1", "when        every 2m · 6 times", "ends        after its 6th run",
                 "the words it sends", "check the build and tell me what failed", "r run now   x stop   esc back to the list"):
        assert want in sc, want + "\n" + sc
    shot(t, "150-opened")
    # run now: outside the count
    t.keys("r")
    sc = t.wait("ran now: t1 is on it. the next run stays at")
    shot(t, "150-run-now")
    t.keys("Escape")
    t.wait("2 active   tab ended too")
    # x asks on the row: n keeps
    t.keys("x")
    sc = t.wait("stop this scheduled task? it won't wake t1 again.   y stop   n or esc keep")
    shot(t, "150-stop-ask")
    t.keys("n")
    t.wait_gone("stop this scheduled task?")
    t.keys("x")
    t.wait("stop this scheduled task?")
    t.keys("y")
    sc = t.wait("1 active   tab ended too")
    t.keys("Tab")
    sc = t.wait("1 active · 1 ended   tab active only")
    assert "stopped by you" in sc, sc
    shot(t, "150-ended")
    t.keys("Escape")
    t.wait_gone("what wakes your agents")
    # t1's thread: set, the run now, ended; never `switchboard`, never the stop's note
    t.keys("M-1")
    sc = t.wait("scheduled #1 ended · stopped by you")
    assert "t1 scheduled #1 · every 2m · 6 times · next" in sc, sc
    assert "scheduled #1 · run now by you · check the build" in sc, sc
    assert "the user stopped timer" not in sc and "(stop it:" not in sc, sc
    no_switchboard(sc)
    shot(t, "150-agent-thread")
    t.keys("M-0")
    light(t, "150")


def light(t, width):
    command(t, "/theme light", "light")
    t.wait_re(MAIN_IDLE)
    shot(t, width + "-main-thread-light")
    command(t, "/scheduled", "1 active")
    shot(t, width + "-list-light")
    t.keys("Enter")
    t.wait("scheduled task #2 · t2")
    shot(t, width + "-opened-light")
    t.keys("Escape")
    t.keys("Escape")
    t.wait_gone("tab ended too")
    command(t, "/theme dark", "dark")


def narrow(t):
    t.start(80, 30)
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    command(t, "/scheduled", "scheduled")
    sc = t.wait("1 active")
    assert "what wakes your agents" not in sc, sc
    assert "⏎ open   r run now   x stop   esc close" in sc, sc
    assert "daily 07:30" in sc, sc
    for line in sc.splitlines():
        assert len(line) <= 80, line
    shot(t, "80-list")
    t.keys("Enter")
    t.wait("scheduled task #2 · t2")
    shot(t, "80-opened")
    t.keys("Escape")
    t.keys("Escape")
    t.wait_gone("1 active")
    light(t, "80")


def main():
    E = e2e.Env()
    with tui_session(150, 40, E=E) as t:
        wide(t, E)
        print("PASS tui scheduled 150")
        narrow(t)
        print("PASS tui scheduled 80")


if __name__ == "__main__":
    run(main)
