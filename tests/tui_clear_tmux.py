"""/clear and Ctrl+L in a real terminal (tmux) on a throwaway hub with the
fake provider: the feed in focus is emptied (like a terminal clear), and
scrolling up pages the cleared lines back from the hub, in their order;
the lines that come after the clear show below the notice.

python3 -u tests/tui_clear_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402


def scroll_up_until(t, needle, tries=30):
    return t.press_until("PageUp", needle, tries=tries)


def main():
    with tui_session(120, 30) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed("first-marker")
        t.keys("Enter")
        t.wait("ack: first-marker")
        t.typed("second-marker")
        t.keys("Enter")
        t.wait("ack: second-marker")
        # /clear: the feed is empty but for the notice
        t.typed("/clear")
        t.keys("Enter")
        sc = t.wait("display cleared")
        assert "first-marker" not in sc and "second-marker" not in sc, sc
        print("---- after /clear ----\n" + sc)
        # a line after the clear shows below the notice
        t.typed("third-marker")
        t.keys("Enter")
        sc = t.wait("ack: third-marker")
        assert "first-marker" not in sc, sc
        assert sc.index("display cleared") < sc.index("third-marker"), sc
        # scrolling up brings the cleared lines back, in order
        sc = scroll_up_until(t, "ack: first-marker")
        t.keys("End")
        t.sync()   # the End handled (it may not move the view)
        sc = t.screen()
        print("---- scrolled back, then End ----\n" + sc)
        for a, b in [("first-marker", "second-marker"), ("ack: second-marker", "display cleared"),
                     ("display cleared", "third-marker")]:
            assert a in sc and b in sc and sc.index(a) < sc.index(b), (a, b, sc)
        # Ctrl+L: the same clear, without the notice
        t.keys("C-l")
        t.wait_gone("third-marker")
        sc = scroll_up_until(t, "ack: third-marker")
        print("---- Ctrl+L, then scrolled back ----\n" + sc)


if __name__ == "__main__":
    run(main)
