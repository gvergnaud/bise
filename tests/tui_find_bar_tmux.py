"""The find bar (user QA 2026-10-04): in the history pane's top-right
corner, flush against the frame's top edge and the panel's rule; it
searches the whole thread (main's transcript holds 2500 lines, the TUI
gets the last 1000: a match on line 5 is found, its pages come in); the
field has the composer's keys (shift+arrows select, alt+arrows jump
words, ctrl+a / ctrl+e the ends, alt+backspace deletes a word); the
chevrons and the × click. Through the real binaries (tmux, fake
provider), on a throwaway hub whose transcript is written before it
starts (the hub reads it back, like after a restart).

python3 -u tests/tui_find_bar_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, tmux  # noqa: E402

LINES = 2500
OLD = 5


def write_transcript(E):
    """Start the hub once (main's folder), stop it, write main's long
    transcript: `  obs: assistant: filler <n>`, the zebra on line OLD."""
    E.start_hub()
    E.stop_hub()
    path = os.path.join(E.state, "agents", "main", "transcript.log")
    ts = int(time.time() * 1000) - LINES * 1000
    with open(path, "w") as f:
        for n in range(1, LINES + 1):
            words = "the ancient zebra bug" if n == OLD else "filler %d" % n
            f.write("%d\t  obs: assistant: %s\n" % (ts + n * 1000, words))


def where(sc, text):
    for y, line in enumerate(sc.splitlines()):
        if text in line:
            return y, line.index(text)
    raise AssertionError(f"{text!r} not on the screen:\n{sc}")


def click(t, x, y):
    """A left click at screen cell (x, y), 0-based (SGR mouse)."""
    seq = "\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1)
    tmux("send-keys", "-t", t.name, "-l", seq)


def main():
    E = e2e.Env()
    write_transcript(E)
    with tui_session(150, 42, "NO_COLOR=1", E=E) as t:
        t.wait("bise :*")
        t.wait("filler %d" % LINES)
        t.keys("C-f")
        sc = t.wait("find in main")
        lines = sc.splitlines()
        # the corner: the box's top border on the row under the frame's
        # top edge, its right border against the panel's rule
        y, x = where(sc, "⌕")
        assert y == 2, sc
        top = lines[1]
        corner = top.index("╮")
        assert top[corner + 1] == "│", top
        assert "↑  ↓  ×" in lines[2], lines[2]
        # the whole thread: the zebra on line 5, 2495 lines above the
        # loaded part, comes in and is the current match
        t.typed("zebra bug")
        sc = t.wait("1/1", 60)
        t.wait("the ancient zebra bug", 30)
        # the composer's keys: alt+← a word left, shift+alt+→ selects it,
        # typing replaces it; ctrl+a / ctrl+e the ends; alt+backspace
        t.keys("M-b")                 # Option+← in Ghostty (ESC b)
        t.keys("C-a")
        t.keys("M-F")                 # shift+alt+→: select "zebra"
        t.typed("ancient zebra")      # typing replaces the selection
        t.wait_re(r"⌕ ancient zebra bug\s+1/1", 10)
        t.keys("C-e")
        t.keys("M-BSpace")            # alt+backspace: "bug" goes
        t.wait_re(r"⌕ ancient zebra\s+1/1", 10)
        # the chevrons and the ×, by mouse
        t.keys("C-u")
        t.typed("filler 249")         # filler 249, 2490..2499: 11 matches
        sc = t.wait("/11", 60)
        line = [l for l in sc.splitlines() if "⌕" in l][0]
        y = sc.splitlines().index(line)
        before = line.split("⌕")[1].split()[2]
        click(t, line.index("↑"), y)
        sc = t.wait_re(r"\d+/11", 10)
        after = [l for l in sc.splitlines() if "⌕" in l][0].split("⌕")[1].split()[2]
        assert after != before, (before, after)
        click(t, line.index("↓"), y)
        t.wait(before, 10)
        click(t, line.index("×"), y)
        t.wait_gone("⌕")
        print("PASS tui find bar")


if __name__ == "__main__":
    run(main)
