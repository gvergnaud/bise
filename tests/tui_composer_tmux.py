"""The composer in a real terminal (tmux, legacy key encodings), against
the fake provider: Up recalls the history and Down past the newest entry
brings the draft back; Option+←/→ (ESC b / ESC f) jump words, Cmd+←/→
(Ctrl+A / Ctrl+E in Ghostty) jump to the line ends, Option+Backspace and
Cmd+Backspace (Ctrl+U) delete a word / to the line start, Ctrl+/ (0x1F)
undoes, Option+` then e (ESC ` e) types è. The mouse (SGR reports written to the pane): a drag in the feed
selects and copies on release ("copied N chars"), a drag in the composer
too; the copies go to BEND_CLIPBOARD_FILE, never the real clipboard.

python3 -u tests/tui_composer_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, pane_rows, wait_until, MAIN_IDLE  # noqa: E402



def composer(t, sc=None):
    """The composer's text row(s) (book §8 "The frame"): the rows with the
    bar `│` under the divider, the blank bar rows left out, each row's
    text after the bar. Of `sc`, or of the screen now."""
    rows = (t.screen() if sc is None else sc).rstrip("\n").splitlines()
    return "\n".join(x for x in pane_rows(rows) if x)


def wait_composer(t, text, timeout=5):
    def shows(sc):
        return composer(t, sc) == text
    shows.__doc__ = "composer %r" % text
    t.wait_any([shows], timeout, poll=0.1)


def mouse(t, kind, x, y):
    """One SGR mouse report at the 0-based cell (x, y): press, drag, release."""
    code, end = {"press": (0, "M"), "drag": (32, "M"), "release": (0, "m")}[kind]
    t.typed("\x1b[<%d;%d;%d%s" % (code, x + 1, y + 1, end))


def find(t, text):
    """The 0-based (column, row) of `text` on the screen."""
    for y, r in enumerate(t.screen().splitlines()):
        if text in r:
            return r.index(text), y
    raise AssertionError("not on screen: %r" % text)


def wait_clip(path, text, timeout=5):
    def read():
        return open(path).read() if os.path.exists(path) else None
    wait_until(lambda: read() == text, timeout,
               lambda: "clipboard %r, expected %r" % (read(), text), poll=0.1)


def main():
    E = e2e.Env()
    clip = os.path.join(E.tmp, "clipboard.txt")
    E.env["BEND_CLIPBOARD_FILE"] = clip
    with tui_session(150, 42, E=E) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # one entry in the history
        t.typed("first message")
        t.keys("Enter")
        t.wait("first message")
        wait_composer(t, "")   # sent: the history has it
        # a draft; Up shows the history, Down brings the draft back
        t.typed("my draft words")
        wait_composer(t, "my draft words")
        t.keys("Up")
        wait_composer(t, "first message")
        t.keys("Down")
        wait_composer(t, "my draft words")
        # word left (ESC b), then type: inserted before "words"
        t.keys("M-b")
        t.typed("X")
        wait_composer(t, "my draft Xwords")
        # line start (Ctrl+A = Cmd+←) and line end (Ctrl+E = Cmd+→)
        t.keys("C-a")
        t.typed("Y")
        t.keys("C-e")
        t.typed("Z")
        wait_composer(t, "Ymy draft XwordsZ")
        # word right from the start (ESC f): after "Ymy"
        t.keys("C-a")
        t.keys("M-f")
        t.typed("!")
        wait_composer(t, "Ymy! draft XwordsZ")
        # Option+Backspace deletes the word before the cursor
        t.keys("C-e")
        t.keys("M-BSpace")
        wait_composer(t, "Ymy! draft")
        # Cmd+Backspace (Ctrl+U) deletes to the line start; Ctrl+/ undoes
        t.keys("C-u")
        wait_composer(t, "")   # the empty composer: the prompt and the cursor only
        t.keys("C-_")
        wait_composer(t, "Ymy! draft")
        # the feed: drag over "first message" in the fake reply, release copies
        t.wait("ack: first message")   # the reply may come late under load
        x, y = find(t, "ack: first message")
        mouse(t, "press", x + 5, y)
        mouse(t, "drag", x + 10, y)
        mouse(t, "drag", x + 17, y)
        mouse(t, "release", x + 17, y)
        t.wait("copied 13 chars")
        wait_clip(clip, "first message")
        # a plain click in the feed selects nothing (no copy)
        os.remove(clip)
        mouse(t, "press", x + 2, y)
        mouse(t, "release", x + 2, y)
        t.sync()
        assert not os.path.exists(clip)
        # the composer: a double click selects the word, the release copies
        x, y = find(t, "Ymy! draft")
        mouse(t, "press", x + 6, y)
        mouse(t, "release", x + 6, y)
        mouse(t, "press", x + 6, y)
        mouse(t, "release", x + 6, y)
        wait_clip(clip, "draft")
        # typing replaces the selection
        t.typed("text")
        wait_composer(t, "Ymy! text")
        # macOS accents with Option as Alt (Ghostty on U.S. layouts):
        # Option+` e = ESC ` e -> è; Option+e e -> é; Option+c -> ç
        t.keys("M-`")
        t.typed("e")
        t.keys("M-e")
        t.typed("e")
        t.keys("M-c")
        wait_composer(t, "Ymy! textèéç")
        print("OK: composer (Up/Down keep the draft, word and line jumps, deletes, undo, Option accents, mouse selection + copy in the feed and the composer)")


if __name__ == "__main__":
    run(main)
