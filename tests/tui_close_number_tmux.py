"""`/close N` with a card that does not exist, in a real terminal (tmux)
on a throwaway hub: the TUI sends the number typed (never a setup card's
own id, 2^50 + n, whose digits hold "999"), the hub answers `no open
card #999`, and huge or garbage numbers get one line naming them while
the hub stays up (it panicked on 'json: number too large').

python3 -u tests/tui_close_number_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402


def close(t, arg, want):
    t.typed("/close %s" % arg)
    t.keys("Enter")
    return t.wait(want)


def main():
    with tui_session(150, 42) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        sc = close(t, "999", "no open card #999 ")
        assert "1125899906842" not in sc, sc
        close(t, "99999999999999999999999", "no open card #99999999999999999999999")
        close(t, "281474976710656", "no open card #281474976710656")
        close(t, "abc", "not a card number: abc")
        # the hub is still up: a message reaches main
        t.typed("still there?")
        t.keys("Enter")
        t.wait("still there?")
        t.sync()
        assert "hub disconnected" not in t.screen(), t.screen()


run(main)
