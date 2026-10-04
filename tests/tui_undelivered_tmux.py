"""A message to a dropped agent on a throwaway hub (tmux, fake provider):
the hub says `undelivered` (C2 amendment, BISE-86); your line ends with
`✗` and `✗ not delivered: t1 stopped. ⏎ send again · esc drop` follows;
⏎ sends it again (a second `✗`), esc drops the question.

python3 -u tests/tui_undelivered_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, in_view  # noqa: E402

ASK = "✗ not delivered: t1 stopped. ⏎ send again · esc drop"


def main():
    with tui_session(150, 42) as t:
        t.wait("bise :*")
        t.wait_re(in_view("main"))
        # an agent in its own worktree: dropped, nothing revives it
        t.typed("/new -w t1: idle")
        t.keys("Enter")
        t.wait("@t1", 30)
        # t1 started and idle in the hub before the archive
        e2e.Client(os.path.join(t.E.state, "hub.sock")).wait_idle("t1", timeout=60)
        t.typed("/archive t1")
        t.keys("Enter")
        if t.wait_any(["answer y", "@t1 archived"], 20)[0] == 0:
            t.typed("y")
            t.keys("Enter")
        t.wait("@t1 archived", 20)
        t.typed("@t1 hello-undelivered")
        t.keys("Enter")
        t.wait_re(r"hello-undelivered ✗", 20)
        sc = t.wait(ASK, 20)
        print(sc)
        # ⏎ sends it again: a second ✗, the first question is answered
        t.keys("Enter")
        def sent_again(sc):
            """sent again: two 'hello-undelivered ✗'"""
            return sc.count("hello-undelivered ✗") >= 2
        t.wait_any([sent_again], 20)
        assert t.screen().count(ASK) == 1, t.screen()
        # esc drops the question
        t.keys("Escape")
        t.wait_gone(ASK)
        assert "✗ not delivered: t1 stopped." in t.screen()
        print("PASS tui undelivered")


if __name__ == "__main__":
    run(main)
