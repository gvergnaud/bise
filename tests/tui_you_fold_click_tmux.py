"""A message of yours over 20 lines folds to 20 rows then `▸ n more
lines`; a click on that row opens it whole (`▾` after its last line),
a click on the `▾` row folds it again, and ctrl+o does the same from
the keyboard. With a few screenshots attached: their sizes wrap to
several rows under the hint, which made the click miss it (the user's
long message with 6 screenshots did not open). Through the real
binaries (tmux, fake provider).

python3 -u tests/tui_you_fold_click_tmux.py
"""
import base64
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, tmux, MAIN_IDLE  # noqa: E402

LINES = 26
SHOTS = 6
# 1x1 PNG
PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==")


def click_at(t, x, y):
    """A left press + release (SGR 1006) at the 0-based (x, y)."""
    t.typed("\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1))


def find(sc, needle):
    """The 0-based (x, y) of the last `needle` on screen, or None."""
    at = None
    for y, row in enumerate(sc.split("\n")):
        x = row.find(needle)
        if x >= 0:
            at = (x, y)
    return at


def click_on(t, needle):
    sc = t.screen()
    at = find(sc, needle)
    assert at, "not on screen: %r\n%s" % (needle, sc)
    click_at(t, at[0], at[1])


def opened(sc):
    # your rows hang behind the accent bar (`│  line 26 ▣ …`, main's
    # `ack: …` echo wraps the same words without it); the `▾` ends the
    # last line's last row (`│  11.35.59.png ▾ ✓✓`)
    return re.search(r"│  line %d\b" % LINES, sc) is not None and re.search(r"│  .*▾ ✓✓", sc) is not None


def paste(t, text):
    """A bracketed paste, as a terminal does for a dropped file."""
    tmux("set-buffer", "-b", "shot", text)
    tmux("paste-buffer", "-p", "-d", "-b", "shot", "-t", t.name)


def main():
    E = e2e.Env()
    shots = []
    for n in range(SHOTS):
        p = os.path.join(E.tmp, "Screenshot 2026-10-02 at 11.3%d.59.png" % n)
        with open(p, "wb") as f:
            f.write(PNG)
        shots.append(p)
    E.env["BEND_IMAGE_DIR"] = os.path.join(E.tmp, "images")
    with tui_session(120, 50, E=E) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        for i in range(1, LINES + 1):
            t.typed("line %d" % i)
            if i < LINES:
                t.keys("C-j")
        # the screenshots dropped at the end of the last line
        for p in shots:
            paste(t, p.replace(" ", "\\ ") + " ")
        t.wait("▣ %d" % SHOTS)
        t.keys("Enter")
        # folded in the history: 20 rows, the hint row, then the sizes
        # of the screenshots on several rows
        t.wait("▸ 6 more lines")
        t.wait_re(MAIN_IDLE)
        sc = t.screen()
        assert re.search(r"│  line 20\b", sc) and not re.search(r"│  line 21\b", sc), sc
        hint = find(sc, "▸ 6 more lines")
        # the rows under the hint up to the blank one: the sizes
        under = sc.split("\n")[hint[1] + 1:]
        sizes = under[:next(i for i, r in enumerate(under) if not re.search(r"│  \S", r))]
        assert "▣ Screenshot" in sizes[0] and "11.35.59.png 1×1" in "".join(sizes), sc
        assert len(sizes) >= 2, "the sizes wrap: %r\n%s" % (sizes, sc)
        # a click on the sizes does nothing
        click_on(t, "11.35.59.png 1×1")
        t.sync()
        assert not opened(t.screen()), t.screen()
        # a click on the hint opens it whole, `▾` after its last line
        click_on(t, "▸ 6 more lines")
        sc = t.wait_any([opened], 10)[1]
        assert "more lines" not in sc, sc
        # a click on the `▾` row folds it again
        click_on(t, "▾")
        sc = t.wait("▸ 6 more lines")
        assert not opened(sc), sc
        # a click on the words of the hint (not its mark) opens it too
        click_on(t, "more lines")
        t.wait_any([opened], 10)
        click_on(t, "▾")
        t.wait("▸ 6 more lines")
        # the keyboard: ctrl+o opens it, again folds it
        t.keys("C-o")
        t.wait_any([opened], 10)
        t.keys("C-o")
        t.wait("▸ 6 more lines")
        print("PASS tui you fold click")


if __name__ == "__main__":
    run(main)
