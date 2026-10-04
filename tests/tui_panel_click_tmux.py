"""A click on an agent in the right panel focuses it, like Alt+N, in a
real terminal (tmux) against the fake provider: the SGR mouse reports
of a left press on each task row, then on main.

python3 -u tests/tui_panel_click_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, panel_row, in_view, MAIN_IDLE  # noqa: E402

COLS, ROWS = 150, 42


def click_on(t, label):
    """A left press + release (SGR 1006) on the panel row showing
    `label` (the panel is the right quarter of the screen)."""
    panel_x = COLS - max(28, min(40, COLS // 4))
    for y, row in enumerate(t.screen().splitlines()):
        x = row.find(label, panel_x)
        if x >= 0:
            # SGR coordinates are 1-based
            seq = "\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1)
            t.typed(seq)
            return
    print(t.screen())
    raise AssertionError("not in the panel: %r" % label)


def main():
    with tui_session(COLS, ROWS) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed('[[bash: sb spawn t1 --objective "first"]] [[bash: sb spawn t2 --objective "second"]]')
        t.keys("Enter")
        t.wait_re(panel_row(1, "t1"))
        t.wait_re(panel_row(2, "t2"))
        click_on(t, " t1")
        t.wait_re(in_view("t1"))
        click_on(t, " t2")
        t.wait_re(in_view("t2"))
        # the selected agent shows its objective under its row: that
        # row belongs to the agent too
        t.keys("M-Down")
        t.keys("M-Down")
        t.wait("first")
        click_on(t, "first")
        t.wait_re(in_view("t1"))
        click_on(t, " main")
        t.wait_re(in_view("main"))
        # the feed still takes clicks: the composer keeps its text
        t.typed("still here")
        t.wait("still here")
        print("PASS tui panel click")


if __name__ == "__main__":
    run(main)
