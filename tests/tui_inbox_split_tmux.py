"""The user's inbox holds only what needs the user (BISE-299), in a real
terminal (tmux) against the fake provider: a task asks main and reports
blocked, a second one asks main too; none of it shows in the inbox strip
above the divider (it is main's traffic, in main's feed). Main escalates
with `sb card`: one row in the strip; the user answers it and it leaves.

python3 -u tests/tui_inbox_split_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, in_view, MAIN_IDLE  # noqa: E402

COLS, ROWS = 150, 42


def inbox_box(sc):
    """The rows of the inbox box: from its `╭─ inbox` top to its `╰` bottom."""
    lines = sc.splitlines()
    top = next((i for i, l in enumerate(lines) if "╭─ inbox" in l), None)
    assert top is not None, sc
    col = lines[top].index("╭─ inbox")
    end = next((i for i in range(top + 1, len(lines)) if lines[i][col:col + 1] == "╰"), len(lines) - 1)
    return "\n".join(lines[top:end + 1])


def main():
    with tui_session(COLS, ROWS, env="BISE_CTRL_DIGITS=1") as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # two agents message main: questions, a blocked report
        t.typed('/new ia: {{bash: sb send main --expect-reply "ia asks main" && sb report blocked "ia is stuck on a key"}}')
        t.keys("Enter")
        t.wait("ia is stuck on a key", 60)
        t.typed('/new ib: {{bash: sb send main --expect-reply "ib asks main"}}')
        t.keys("Enter")
        sc = t.wait("ib asks main", 60)
        t.wait_re(in_view("main"))
        # a few frames later, still nothing for the user: no strip, no
        # "needs you" in the header
        time.sleep(1.5)
        sc = t.screen()
        for gone in ("waiting for you", "ctrl+1 open", "needs you", "ia needs you"):
            assert gone not in sc, (gone, sc)
        # main escalates: one row in the user's inbox
        t.typed('[[bash: sb card "ship the export on friday?"]]')
        t.keys("Enter")
        t.wait("waiting for you", 60)
        sc = t.wait("? main · ship the export on friday?")
        # the inbox box only: main's thread may quote the task's question
        # ('ia asked: ia asks main · i answered: …')
        box = inbox_box(sc)
        assert "? main · ship the export on friday?" in box, sc
        assert "ia asks main" not in box and "? ia ·" not in box, box
        # only the user answers it: open it, type, ⏎; it leaves the strip
        t.typed("\x1b[49;5u")             # ctrl+1, the kitty form (BISE-302)
        t.wait("your answer")
        t.typed("yes friday")
        t.wait("⏎ sends your answer")
        t.keys("Enter")
        t.wait_gone("waiting for you", 20)
        t.wait("✓ you answered main: yes friday")
        time.sleep(0.3)
        print("PASS tui inbox split")


if __name__ == "__main__":
    run(main)
