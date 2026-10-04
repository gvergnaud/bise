"""The code blocks of a reply in a real terminal (tmux) against the fake
provider: a ```ts block is a box (`╭─ ts ─…─╮`, its code colored by the
composer's highlighter), the mouse over it shows the copy icon (the word ` copy `) on its
top border, a click on the icon copies the code (BEND_CLIPBOARD_FILE,
never the real clipboard) and the icon says `✓ copied`; ctrl+y copies the
newest block on screen.

python3 -u tests/tui_code_blocks_tmux.py
"""
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, wait_until, MAIN_IDLE  # noqa: E402
from tui_markdown_tmux import fg_of, KEYWORD, NUMBER  # noqa: E402

CODE_TS = "const answer = 42 // the one"
CODE_SH = "ls -la"


def sgr(b, x, y, end="M"):
    """One SGR 1006 mouse report at the 0-based cell (x, y)."""
    return "\x1b[<%d;%d;%d%s" % (b, x + 1, y + 1, end)


def at(sc, text):
    rows = sc.splitlines()
    for y, r in enumerate(rows):
        if text in r:
            return r.find(text), y
    raise AssertionError("no %r on screen:\n%s" % (text, sc))


def read(path):
    try:
        with open(path) as f:
            return f.read()
    except FileNotFoundError:
        return ""


def main():
    clip = os.path.join(tempfile.mkdtemp(prefix="sbclip"), "clip.txt")
    with tui_session(100, 34, "BEND_CLIPBOARD_FILE=%s" % clip) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # the reply: `done: <the tool's output>`, two fenced blocks
        t.typed("[[bash: printf 'see:\\n```ts\\n%s\\n```\\nthen:\\n```bash\\n%s\\n```\\n']]" % (CODE_TS, CODE_SH))
        t.keys("Enter")
        sc = t.wait("╭─ bash ─")
        x, y = at(sc, "╭─ ts ─")
        rows = sc.splitlines()
        assert ("│ " + CODE_TS) in rows[y + 1], sc
        assert "╰─" in rows[y + 2], sc
        assert " copy ─╮" not in sc, sc
        col = t.screen(colors=True)
        # the box's row (your message above holds the same words, plain)
        boxed = "│ " + CODE_TS
        assert fg_of(col, boxed, 2) in KEYWORD, fg_of(col, boxed, 2)
        assert fg_of(col, boxed, 2 + CODE_TS.index("42")) in NUMBER, fg_of(col, boxed, 2 + CODE_TS.index("42"))
        # the mouse over the code: the icon on the block's top border
        t.typed(sgr(35, x + 6, y + 1))
        sc = t.wait(" copy ─╮")
        ix, iy = at(sc, " copy ─╮")
        assert iy == y, sc
        print("---- hover ----\n" + sc)
        # a click on the icon: the code copied, `✓ copied` on the border
        t.typed(sgr(0, ix, iy) + sgr(0, ix, iy, "m"))
        wait_until(lambda: read(clip) == CODE_TS, 10, lambda: "clipboard: %r" % read(clip))
        t.wait("✓ copied ─╮")
        t.wait("copied %d chars" % len(CODE_TS))
        # ctrl+y (the mouse away): the newest block on screen
        # once `✓ copied` is over (codeblock::COPIED_FOR)
        t.wait_gone("✓ copied ─╮")
        t.keys("C-y")
        wait_until(lambda: read(clip) == CODE_SH, 10, lambda: "clipboard: %r" % read(clip))
        print("PASS tui code blocks")


if __name__ == "__main__":
    run(main)
