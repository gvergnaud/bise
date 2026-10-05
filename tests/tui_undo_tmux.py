"""The composer's undo and redo through a real terminal (tmux): type,
delete a word, ctrl+z twice, redo once; the text and the cursor match
each step. tmux sends ctrl+shift+z as plain ctrl+z (no kitty keyboard
protocol), so redo is ctrl+y here, the fallback /help names. Then ctrl+z
with nothing left to undo says there is no undo of what was sent.
Through the real binaries (throwaway hub, fake provider).

python3 -u tests/tui_undo_tmux.py
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, tmux, wait_until, MAIN_IDLE  # noqa: E402


def composer(t):
    """The composer's row: (its text, the cursor's index in it). The TUI
    draws its own cursor, a reversed cell (NO_COLOR); the terminal's
    cursor is elsewhere."""
    for line in t.screen(colors=True).splitlines():
        if "fix" not in line or "\x1b[7m" not in line:
            continue
        head, tail = line.split("\x1b[7m", 1)
        plain = lambda x: re.sub(r"\x1b\[[0-9;]*m", "", x)
        head = plain(head)
        start = head.find("fix")
        if start < 0:
            continue
        text = (head[start:] + plain(tail)).split("│")[0].rstrip()
        return text, len(head) - start
    return None, None


def step(t, want_text, want_cursor):
    return wait_until(lambda: composer(t) == (want_text, want_cursor), 10,
                      f"composer {want_text!r} cursor at {want_cursor}: {composer(t)}")


def main():
    with tui_session(100, 30, "NO_COLOR=1") as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed("fix the login bug")
        step(t, "fix the login bug", 17)
        # cursor before "bug", then ctrl+w deletes "login "
        t.keys("M-b")
        step(t, "fix the login bug", 14)
        t.keys("C-w")
        step(t, "fix the bug", 8)
        # ctrl+z: the word comes back, the cursor where it was
        t.keys("C-z")
        step(t, "fix the login bug", 14)
        # ctrl+z: the last typed word goes ("bug")
        t.keys("C-z")
        step(t, "fix the login", 14)
        # redo once (ctrl+y: tmux cannot tell ctrl+shift+z apart)
        t.keys("C-y")
        step(t, "fix the login bug", 14)
        assert "no undo" not in t.screen(), t.screen()
        for _ in range(5):
            t.keys("C-z")
        t.wait("no undo: an agent may already have acted")
        print("PASS tui undo")


if __name__ == "__main__":
    run(main)
