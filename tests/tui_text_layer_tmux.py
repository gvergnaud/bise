"""The text layer (BISE-290) in a real terminal (tmux) against the fake
provider: every text bise draws selects, copies and opens its links the
same way. On an inbox card (its strip row, then the card view), the
`/` popup, the help and a tool's box opened with ctrl+o: a drag copies
(BEND_CLIPBOARD_FILE, never the real clipboard: `copied N chars`), a
plain click on a url opens it (BISE_OPEN: a script that logs the url, in
place of `open`), and a click on a card's words still opens the card.

python3 -u tests/tui_text_layer_tmux.py
"""
import os
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, wait_until, MAIN_IDLE  # noqa: E402

COLS, ROWS = 150, 42
# two presses on one cell within 400 ms are a double click
# (app::MouseState::press): the gap is the input, not a wait
DOUBLE_CLICK_GAP = 0.5
CARD_URL = "https://perf.example/report"
BOX_URL = "https://box.example/y"


def read(path):
    try:
        with open(path) as f:
            return f.read()
    except FileNotFoundError:
        return ""


def sgr(b, x, y, end="M"):
    """One SGR 1006 mouse report at the 0-based cell (x, y)."""
    return "\x1b[<%d;%d;%d%s" % (b, x + 1, y + 1, end)


def at(sc, text, last=False):
    """The 0-based cell where `text` starts on the screen (its last
    occurrence with `last`)."""
    rows = sc.splitlines()
    ys = [y for y, r in enumerate(rows) if text in r]
    if not ys:
        raise AssertionError("no %r on screen:\n%s" % (text, sc))
    y = ys[-1] if last else ys[0]
    return rows[y].find(text), y


def click(t, x, y):
    t.typed(sgr(0, x, y) + sgr(0, x, y, "m"))
    # never the next press's double click
    time.sleep(DOUBLE_CLICK_GAP)


def drag_copies(t, clip, text, last=False):
    """A drag over `text`: the clipboard gets it, the note says so."""
    if os.path.exists(clip):
        os.remove(clip)
    x, y = at(t.screen(), text, last)
    end = x + len(text) - 1
    t.typed(sgr(0, x, y) + sgr(32, x + 2, y) + sgr(32, end, y) + sgr(0, end, y, "m"))
    wait_until(lambda: read(clip) == text, 10, lambda: "the drag copies %r: %r" % (text, read(clip)))
    t.wait("copied %d chars" % len(text))
    time.sleep(DOUBLE_CLICK_GAP)


def opens(t, log, url, text, last=False):
    """A plain click on `text` opens `url`."""
    n = len(read(log).split())
    x, y = at(t.screen(), text, last)
    click(t, x + 2, y)
    wait_until(lambda: len(read(log).split()) > n, 10, lambda: "the click on %r at %r opens %s: %r\n%s" % (text, (x, y), url, read(log), t.screen()))
    assert read(log).split()[-1] == url, read(log)
    t.wait("opening " + url)


def main():
    tmp = tempfile.mkdtemp(prefix="sbtext")
    log = os.path.join(tmp, "opened")
    clip = os.path.join(tmp, "clipboard")
    opener = os.path.join(tmp, "open.sh")
    with open(opener, "w") as f:
        f.write('#!/bin/sh\necho "$1" >> %s\n' % log)
    os.chmod(opener, 0o755)
    with tui_session(COLS, ROWS, "BISE_OPEN=%s BEND_CLIPBOARD_FILE=%s" % (opener, clip)) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # a card with a url, and a tool whose output has one
        t.typed("[[bash: sb card \"$(printf 'the bundle grew, see %s. split it?\\n1. yes\\n2. no')\"]] "
                "[[bash: echo location: %s]]" % (CARD_URL, BOX_URL))
        t.keys("Enter")
        t.wait("inbox · 1 waiting for you")
        t.wait("? main · the bundle grew")
        # the strip row: a drag copies its words, the card stays closed
        drag_copies(t, clip, "the bundle grew", last=True)
        assert "your answer" not in t.screen(), t.screen()
        # a click on its url opens it, not the card
        opens(t, log, CARD_URL, "perf.example", last=True)
        # a click on its words opens the card, as before
        x, y = at(t.screen(), "the bundle grew", last=True)
        click(t, x + 1, y)
        if "1 yes" not in t.screen():
            click(t, x + 1, y)
        t.wait("1 yes")
        # the card view: the same
        drag_copies(t, clip, "split it?", last=True)
        opens(t, log, CARD_URL, "perf.example/report", last=True)
        t.keys("Escape")
        t.wait_gone("1 yes")
        # the tool's box, opened with ctrl+o: its output copies, its url opens
        t.keys("C-o")
        t.wait("location: " + BOX_URL)
        drag_copies(t, clip, "location:", last=True)
        opens(t, log, BOX_URL, "box.example/y", last=True)
        # the `/` popup: a drag copies a command's words, picks nothing
        t.typed("/he")
        t.wait("/help")
        drag_copies(t, clip, "/help", last=True)
        # the help
        t.keys("BSpace", "BSpace", "BSpace")
        t.typed("/help")
        t.keys("Enter")
        t.wait("/shortcuts")
        # the help opens on the commands; its sections come after them, so
        # page down to "talk to agents" (each new command pushes it lower)
        t.press_until("NPage", "talk to agents", tries=6, must=False)
        t.wait("talk to agents")
        drag_copies(t, clip, "talk to agents")
        assert "talk to agents" in t.screen(), "the help stays open"
        t.keys("Escape")
    print("PASS tui text layer")


if __name__ == "__main__":
    run(main)
