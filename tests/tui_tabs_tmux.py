"""render-artifacts: a tool output with TABs (`du -sm * | sort -rn`), ANSI
colors and a CR progress line, scrolled up and down, leaves no ghost
cells: the screen after the scroll is the screen a full redraw (a
resize) paints. Before the fix, ratatui put each TAB in one cell while
the terminal jumped to the next tab stop, so the rest of the row landed
to the right and the diff never cleared it (text over the agents panel,
gone on resize).

python3 -u tests/tui_tabs_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wait  # noqa: E402
from tui_tmux import tui_session, run, tmux, MAIN_IDLE  # noqa: E402

COLS, ROWS = 120, 30


def call(n, desc):
    # rows `<size>\t<name>` of every length, a colored one, a CR-rewritten one
    return ("[[bash: for i in $(seq 1 12); do printf '%%d\\t%s-%%s\\n' $((i*i*i*%d)) $(printf 'x%%.0s' $(seq 1 $i)); done; "
            "printf '\\033[31m7\\tred\\033[0m\\n10%%%%\\r100%%%%\\tdone\\n' @@ %s]] " % (desc, n, desc))


def settle(t):
    """The screen once it stops changing (a frame may be in flight)."""
    return wait.stable(t.screen, 10, "the screen")


def main():
    with tui_session(COLS, ROWS) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed(call(3, "electron") + call(7, "bruno-updater") + call(11, "caches"))
        t.keys("Enter")
        t.wait_re(r"\$ caches +✓", 60)
        t.wait("done: ", 30)
        t.keys("C-o")  # every call open: its output in the box
        settle(t)
        # each scroll drawn before the next (the ghost cells come from the
        # frames in between)
        for k in ["PageUp"] * 4 + ["PageDown"] * 2 + ["PageUp"] + ["PageDown"] * 4 + ["PageUp"] * 2:
            t.keys(k)
            wait.stable(t.screen, 10, "the screen after %s" % k, quiet=0.2)
        scrolled = settle(t)
        # a full redraw: a resize repaints every cell
        tmux("resize-window", "-t", t.name, "-x", str(COLS - 1))
        t.wait_any([lambda s, w=scrolled: s != w], 10)   # drawn at the new width
        tmux("resize-window", "-t", t.name, "-x", str(COLS))
        clean = settle(t)
        print("---- scrolled ----\n%s\n---- redrawn ----\n%s" % (scrolled, clean))
        diff = [(i, a, b) for i, (a, b) in enumerate(zip(scrolled.splitlines(), clean.splitlines())) if a != b]
        assert not diff, "ghost cells after the scroll:\n" + "\n".join("%2d %r\n   %r" % d for d in diff)
        # at the bottom, the agent's reply quotes the output: the TAB is a
        # tab stop, the escape sequences and the CR are gone
        t.keys("End")
        end = t.wait_re(r"\b100% +done\b")
        assert "[31m" not in end and "\x1b" not in end, end
        print("PASS tui tabs")


if __name__ == "__main__":
    run(main)
