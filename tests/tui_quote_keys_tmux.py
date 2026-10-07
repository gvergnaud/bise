"""Quoting a selection without typing, in a real terminal (tmux) against
the fake provider, with text already in the composer and the cursor
inside it: space quotes at the cursor (no space of its own), tab (the
popup's `tab quote`) quotes, and a click on the popup's ask words
quotes, each chip at the cursor, and the popup goes away each time.

python3 -u tests/tui_quote_keys_tmux.py
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, wait_until, MAIN_IDLE  # noqa: E402

COLS, ROWS = 120, 36
HINT = " type to ask about it · tab quote · cmd+c copy "


def chips(n):
    """The composer's row with `n` quote chips between `why` and `slow`
    (a chip is drawn padded: `why  ❝ 1   slow`), no space typed."""
    inner = r"\s+".join("❝ %d" % i for i in range(1, n + 1))
    return re.compile(r"│\s+why\s+" + inner + r"\s+slow\s+│")


def main():
    with tui_session(COLS, ROWS) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed("the login breaks on safari")
        t.keys("Enter")
        t.wait("ack: the login breaks on safari")

        def drag():
            # the quote strip above the composer moves the history up:
            # find the reply again each time
            rows = t.screen().splitlines()
            y = max(i for i, r in enumerate(rows) if "ack: the login" in r)
            x = rows[y].find("login")
            # press, drag (SGR 1006: button 32 is the left one moving), release
            t.typed("\x1b[<0;%d;%dM\x1b[<32;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 7, y + 1, x + 7, y + 1))

            def shown():
                r = t.screen().splitlines()
                return HINT in r[y - 1] and "ack: the login" in r[y]

            wait_until(shown, 10, lambda: "the popup over the selection:\n" + t.screen())
            return y

        def gone(why):
            wait_until(lambda: HINT not in t.screen(), 10, lambda: why + " puts the popup away:\n" + t.screen())

        # text in the composer, the cursor between its two words
        t.typed("why slow")
        t.wait("why slow")
        t.keys("Left", "Left", "Left", "Left", "Left")

        # space: the quote at the cursor, no space of its own
        drag()
        t.typed(" ")
        t.wait(chips(1))
        gone("space")

        # tab: the next quote at the cursor, right after the first chip
        drag()
        t.keys("Tab")
        t.wait(chips(2))
        gone("tab")

        # a click on the popup's ask words: the third, at the cursor
        y = drag()
        r = t.screen().splitlines()
        cx = r[y - 1].find("ask about it")
        t.typed("\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (cx + 1, y, cx + 1, y))
        t.wait(chips(3))
        gone("a click on it")
        print("PASS tui quote keys")


if __name__ == "__main__":
    run(main)
