"""keep-state: a real reload (the hub's `version restart`, what `sb
restart`, a version switch and a hub crash restart do to the TUI) keeps
everything, in a real terminal (tmux) against the fake provider:

1. an inbox answer half typed (the item open), and the thread's draft
   that waits behind it;
2. the history scrolled up, and the agent palette open with a query;
3. a reload never lands while keys arrive: the divider says `bise
   restarts when you stop typing`, the TUI stays while the keys go on,
   then reloads once they stop, with what was typed.

python3 -u tests/tui_keep_state_tmux.py
"""
import os
import signal
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, wait_until, pane_rows, MAIN_IDLE  # noqa: E402

COLS, ROWS = 120, 30
CTRL = "\x1b[%d;5u"   # ctrl+digit, the kitty keyboard protocol's form
WAIT_NOTE = "bise restarts when you stop typing"


def composer(sc):
    return " ".join(r.strip() for r in pane_rows(sc.splitlines()))


def card(text):
    return "[[bash: sb card \"$(printf '%s')\"]]" % text


def main():
    E = e2e.Env()
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(bise)
    timing = os.path.join(E.tmp, "timing.log")
    E.env["SB_TIMING"] = timing

    def tui_starts():
        try:
            return sum(1 for l in open(timing) if " tui start (connecting)" in l)
        except FileNotFoundError:
            return 0

    def stop_switcher():
        # the switcher watches the new hub for 2 min: stopped before the
        # test stops the hub, else it rolls back (starts a hub again)
        try:
            os.kill(int(open(os.path.join(E.state, "switch.pid")).read()), signal.SIGTERM)
        except (OSError, ValueError):
            pass

    def reload(t):
        """The hub's restart (a reload): the TUI re-executes itself."""
        n = tui_starts()
        stop_switcher()
        e2e.Client(os.path.join(E.state, "hub.sock")).send({"op": "version", "do": "restart"})
        wait_until(lambda: tui_starts() > n, 40, lambda: "the TUI did not re-exec: %d starts" % tui_starts())
        t.wait_re(MAIN_IDLE)

    env = "BISE_HOME=%s BISE_CTRL_DIGITS=1" % bise
    with tui_session(COLS, ROWS, env, E=E) as t:
        try:
            t.wait("bise :*")
            t.wait_re(MAIN_IDLE)
            # a history taller than the screen
            for i in range(1, 9):
                t.typed("message number %02d" % i)
                t.keys("Enter")
                t.wait("ack: message number %02d" % i)
                t.wait_re(MAIN_IDLE)
            t.typed(card("pick a title\\n1. alpha\\n2. beta"))
            t.keys("Enter")
            t.wait("1 ? main · pick a title")
            t.wait_re(MAIN_IDLE)

            # 1. the thread's draft, then an inbox answer half typed
            t.typed("the thread draft")
            t.wait("the thread draft")
            t.typed(CTRL % ord("1"))
            t.wait("you → ? main · your answer")
            t.typed("half an answer")
            t.wait("half an answer")
            reload(t)
            sc = t.wait("you → ? main", 30)
            sc = t.wait("half an answer")
            assert "half an answer" in composer(sc), sc
            # esc: back to the thread, its draft is there
            t.keys("Escape")
            sc = t.wait("the thread draft")
            assert "the thread draft" in composer(sc), sc
            # the answer is still the item's
            t.typed(CTRL % ord("1"))
            t.wait("half an answer")
            t.keys("Escape")
            t.wait("the thread draft")

            # 2. the history scrolled up, the palette open on a query
            t.press_until("PageUp", "back to the bottom")
            t.press_until("PageUp", "message number 02", tries=10)
            before = [r for r in t.screen().splitlines() if "message number" in r]
            t.keys("C-s")
            t.wait("find an agent")
            t.typed("mai")
            t.wait("mai")
            reload(t)
            sc = t.wait("find an agent", 30)
            assert "mai" in sc, sc
            t.keys("Escape")
            sc = t.wait_gone("find an agent")
            sc = t.wait("back to the bottom")
            after = [r for r in sc.splitlines() if "message number" in r]
            assert after[:1] == before[:1], "the history moved:\n%r\n%r" % (before, after)
            t.keys("End")
            t.wait_gone("back to the bottom")
            sc = t.wait("the thread draft")

            # 3. keys arriving hold the reload; it goes once they stop
            n = tui_starts()
            stop_switcher()
            e2e.Client(os.path.join(E.state, "hub.sock")).send({"op": "version", "do": "restart"})
            t0 = time.time()
            noted = False
            i = 0
            while time.time() - t0 < 12:
                # a key, then the next once it is drawn: a typist's pace
                i += 1
                t.typed(" k")
                sc = t.wait_any([lambda s, i=i: composer(s).count(" k") >= i], 10)[1]
                noted = noted or WAIT_NOTE in sc
            assert tui_starts() == n, "reloaded while keys were arriving"
            assert noted, "never said: %s\n%s" % (WAIT_NOTE, t.screen())
            # the keys stop: the reload goes within a few seconds
            wait_until(lambda: tui_starts() > n, 15, "no reload after the keys stopped")
            t.wait_re(MAIN_IDLE)
            sc = t.wait("the thread draft k k")
            assert composer(sc).count(" k") == i, "keys lost: %d sent\n%s" % (i, sc)
            assert WAIT_NOTE not in sc, sc
            print("OK: inbox answer, thread draft, scroll, palette query kept across reloads; the reload waits for the keys to stop")
        finally:
            stop_switcher()


if __name__ == "__main__":
    run(main)
