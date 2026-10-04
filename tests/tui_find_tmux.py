"""BISE-237, BISE-297: ctrl+f finds in the history, in a small box at the
top-right of the history; the composer stays with its draft. Two
messages carry `needle`, a long one between them (opened with ctrl+o)
pushes the first off the screen. ctrl+f: the box opens top-right, the
draft stays in the composer; type `needle`: the newest match is current
(`4/4`); ↑ goes up match by match and the view scrolls to the old
one, never under the box; ↓ comes back; esc closes the box and the keys
go back to the composer (typing adds to the draft). NO_COLOR: the
current match is reversed, the others underlined. Through the real
binaries (tmux, fake provider: `ack: ...`).

python3 -u tests/tui_find_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402

FILLER = " ".join(["filler"] * 350)


def send(t, text, ack):
    t.typed(text)
    t.keys("Enter")
    t.wait(ack, 60)
    t.wait_re(MAIN_IDLE, 60)


def where(sc, text):
    """(row, column) of `text` on the screen."""
    for y, line in enumerate(sc.splitlines()):
        if text in line:
            return y, line.index(text)
    raise AssertionError(f"{text!r} not on the screen:\n{sc}")


def main():
    with tui_session(120, 40, "NO_COLOR=1") as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        send(t, "the needle one", "ack: the needle one")
        send(t, FILLER, "ack: filler filler")
        send(t, "the needle two", "ack: the needle two")
        # a long message of yours folds to 20 rows (BISE-239, BISE-262): opened
        # (ctrl+o), the filler pushes the first needle off the screen
        t.keys("C-o")
        t.wait("filler ▾")
        assert "needle one" not in t.screen(), t.screen()
        t.typed("my draft")
        t.keys("C-f")
        sc = t.wait("find in main")
        # the box: top-right of the history, under the header; the
        # composer keeps the draft, the key bar says the box's keys
        box_y, box_x = where(sc, "⌕")
        assert box_y < 5 and box_x > 40, sc
        assert "╭" in sc.splitlines()[box_y - 1] and "╰" in sc.splitlines()[box_y + 1], sc
        assert "my draft" in sc, sc
        t.wait("⏎ older   shift+⏎ newer   esc close")
        t.typed("needle")
        sc = t.wait("4/4")
        assert "my draft" in sc, sc
        # the current match reversed, the others underlined (NO_COLOR)
        col = t.screen(colors=True)
        assert "\x1b[7mneedle" in col, col[-3000:]
        assert "\x1b[4mneedle" in col, col[-3000:]
        t.keys("Up")
        t.wait("3/4")
        # up again: the ack of the first one, far above; the view goes
        # there (the history's top), the match never under the box
        t.keys("Up")
        sc = t.wait("2/4")
        sc = t.wait("ack: the needle one")
        assert "the needle two" not in sc, sc
        y, x = where(sc, "ack: the needle one")
        assert y > box_y + 1 or x + len("ack: the needle one") < box_x - 3, sc
        t.keys("Up")
        t.wait("1/4")
        # past the oldest: back to the newest
        t.keys("Up")
        t.wait("back to the newest")
        sc = t.wait("ack: the needle two")
        t.keys("Down")
        t.wait("back to the oldest")
        t.wait("the needle one")
        # ↓ newer (shift+⏎ too: the unit tests, tmux sends it as ⏎)
        t.keys("Down")
        t.wait("2/4")
        t.keys("Enter")
        t.wait("1/4")
        # esc: the box closes, the view stays, the keys are the composer's
        t.keys("Escape")
        sc = t.wait_gone("⌕")
        sc = t.screen()
        assert "find in main" not in sc and "ack: the needle one" in sc, sc
        t.typed(" kept")
        t.wait("my draft kept")
        print("PASS tui find")


if __name__ == "__main__":
    run(main)
