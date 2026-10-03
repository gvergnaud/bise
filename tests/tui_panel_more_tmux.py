"""Every agent is reachable in a panel taller than the screen
(sidebar-more), in a real terminal (tmux) against the fake provider:
~40 agents, 6 of them in a feature group. The list ends in `↓ n more`;
a click on it scrolls a page, `↑ n more` scrolls back, the wheel
scrolls 3 rows, and a click reaches the last agent; Alt+↑ (the
selection from the bottom) reaches it from the keyboard.

python3 -u tests/tui_panel_more_tmux.py
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, in_view, MAIN_IDLE  # noqa: E402

COLS, ROWS = 150, 42
PANEL_X = COLS - max(28, min(40, COLS // 4))
SOLO = 33
GROUP = ["s%d" % i for i in range(1, 7)]
LAST = GROUP[-1]


def panel(sc):
    """The panel's rows (the right part of each screen row)."""
    return [r[PANEL_X:] for r in sc.splitlines()]


def row_of(sc, pred):
    for y, r in enumerate(panel(sc)):
        if pred(r):
            return y
    return None


def press(t, x, y, button=0):
    """An SGR 1006 press (+ release for a click) at 0-based (x, y)."""
    seq = "\x1b[<%d;%d;%dM" % (button, x + 1, y + 1)
    if button == 0:
        seq += "\x1b[<0;%d;%dm" % (x + 1, y + 1)
    t.typed(seq)


def more_row(sc, arrow):
    return row_of(sc, lambda r: re.search(r"│\s+%s \d+ more" % arrow, r) is not None)


def x_of(sc, y, text):
    """The screen column of `text` in the panel's part of row `y`."""
    x = sc.splitlines()[y].find(text, PANEL_X)
    assert x >= 0, (text, sc)
    return x


def main():
    with tui_session(COLS, ROWS) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # one bash call: past 30 s the tool goes on in the background
        # and the fake main's turn ends, the spawns go on
        t.typed("[[bash: sb feature new sculpt && for n in %s; do sb spawn $n --feature sculpt --objective x; done; "
                "for i in $(seq 10 %d); do sb spawn a$i --objective x; done]]"
                % (" ".join(GROUP), 9 + SOLO))
        t.keys("Enter")
        # the last spawn is in main's feed, main is idle
        t.wait("new agent @a%d" % (9 + SOLO), 400)
        t.wait_re(MAIN_IDLE, 240)
        sc = t.wait_re(r"↓ \d+ more", 30)
        assert "✗" not in "".join(panel(sc)), "an agent failed: " + sc

        # the mouse: `↓ n more` a page at a time, down to the last agent
        for _ in range(12):
            sc = t.screen()
            if LAST in "".join(panel(sc)):
                break
            y = more_row(sc, "↓")
            assert y is not None, sc
            press(t, x_of(sc, y, "↓"), y)
            # the panel moved, with a `↑` row over it
            t.wait(lambda s: panel(s) != panel(sc) and more_row(s, "↑") is not None, 10)
        sc = t.wait(lambda s: LAST in "".join(panel(s)), 10)
        assert more_row(sc, "↑") is not None, sc
        assert "sculpt" in "".join(panel(sc)), "the feature group's title: " + sc
        y = row_of(sc, lambda r: re.search(r"\b%s\b" % LAST, r) is not None)
        press(t, x_of(sc, y, LAST), y)
        t.wait_re(in_view(LAST))
        print("mouse: reached", LAST)

        # `↑ n more` back to the top, then the wheel down 3 rows
        for _ in range(12):
            sc = t.screen()
            y = more_row(sc, "↑")
            if y is None:
                break
            press(t, x_of(sc, y, "↑"), y)
            t.wait(lambda s: panel(s) != panel(sc), 10)
        sc = t.wait_re(r"\b0 \S+ main\b")
        assert more_row(sc, "↑") is None, sc
        y = ROWS // 2
        press(t, x_of(sc, y, "○"), y, button=65)
        sc = t.wait_re(r"↑ 3 more")
        assert not re.search(r"\b0 \S+ main\b", "\n".join(panel(sc))), sc

        # the keyboard: Esc to main, Alt+↑ selects the last agent from
        # the bottom (the panel follows), Enter enters it
        t.keys("Escape")
        t.wait_re(in_view("main"))
        t.keys("M-Up")
        sc = t.wait(lambda s: LAST in "".join(panel(s)), 10)
        t.keys("Enter")
        t.wait_re(in_view(LAST))
        # numbers 0-9 still work
        t.keys("M-0")
        t.wait_re(in_view("main"))
        print("PASS tui panel more")


if __name__ == "__main__":
    run(main)
