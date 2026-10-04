"""The ctrl hints (ctrlhint.rs) through the real binary's input parser: the
kitty keyboard protocol's bytes are written to the pane as a terminal with
flags 1+2+8+16 sends them (tmux itself does not speak the protocol, so the
TUI keeps flag 1 alone, but it reads what arrives). Ctrl held alone shows
the ctrl keys in the key bar after ~150 ms, its release takes them away at
once, a ctrl+o combo never shows them, and the associated text types é
(dead key), å (option) and A (caps lock). Option held alone shows the ⌥
keys, an option character (ç) hides them and types; cmd held shows the
cmd keys only once a cmd key came (BISE-277).

TMPDIR=/tmp/ch-run python3 -u tests/tui_ctrl_hints_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wait  # noqa: E402
from tui_tmux import tui_session, run, tmux, MAIN_IDLE  # noqa: E402

CTRL_DOWN = "\x1b[57442;5u"
CTRL_UP = "\x1b[57442;1:3u"
HINT = "ctrl+c quit"
ALT_DOWN = "\x1b[57443;3u"
ALT_UP = "\x1b[57443;1:3u"
ALT_HINT = "\u2325\u23ce newline"
CMD_DOWN = "\x1b[57444;9u"
CMD_UP = "\x1b[57444;1:3u"
CMD_HINT = "cmd+v paste"


def raw(t, s):
    tmux("send-keys", "-t", t.name, "-H", *("%02x" % b for b in s.encode()))


def check(cols, rows):
    with tui_session(cols, rows) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        before = t.screen()
        assert HINT not in before, before
        # held alone: the hints come, the frame's rows stay where they are
        raw(t, CTRL_DOWN)
        sc = t.wait(HINT, timeout=5)
        assert "ctrl+v paste image" in sc, sc
        assert len(sc.splitlines()) == len(before.splitlines()), sc
        print("---- ctrl held at %d columns ----\n%s" % (cols, sc))
        raw(t, CTRL_UP)
        t.wait_gone(HINT, timeout=2)
        # ctrl+o: a combo, no hints even held
        raw(t, CTRL_DOWN + "\x1b[111;5u" + "\x1b[111;5:3u")
        # held well past ctrlhint::DELAY (150 ms): a window, no state shows it
        wait.holds(lambda: (lambda s: HINT not in s and s)(t.screen()), 0.8, "no ctrl hints after ctrl+o",
                   poll=wait.SCREEN_POLL)
        raw(t, CTRL_UP)
        # the typed text: a dead key's é, option's å, caps lock's A, a plain t
        raw(t, "\x1b[101;;233u" + "\x1b[101;1:3u" + "\x1b[97;3;229u" + "\x1b[97;65;65u" + "t")
        sc = t.wait("\u00e9\u00e5At", timeout=5)
        print("---- typed with the flags' bytes ----\n%s" % sc)
        # BISE-277: option held alone shows the ⌥ keys; a character typed
        # with option (c with alt, the text ç) hides them and types ç
        raw(t, ALT_DOWN)
        sc = t.wait(ALT_HINT, timeout=5)
        assert "ctrl+c" not in sc.splitlines()[-2], sc
        print("---- option held ----\n%s" % sc)
        raw(t, "\x1b[99;3;231u")
        t.wait_gone(ALT_HINT, timeout=2)
        raw(t, "\x1b[99;1:3u" + ALT_UP)
        t.wait("\u00e9\u00e5At\u00e7", timeout=5)
        # cmd held before any cmd key: nothing; after cmd+c: the cmd keys
        raw(t, CMD_DOWN)
        wait.holds(lambda: (lambda s: CMD_HINT not in s and s)(t.screen()), 0.8, "no cmd hints before a cmd key",
                   poll=wait.SCREEN_POLL)
        raw(t, CMD_UP + CMD_DOWN + "\x1b[99;9u" + "\x1b[99;9:3u" + CMD_UP)
        raw(t, CMD_DOWN)
        sc = t.wait(CMD_HINT, timeout=5)
        assert "cmd+a select all" in sc, sc
        print("---- cmd held ----\n%s" % sc)
        raw(t, CMD_UP)
        t.wait_gone(CMD_HINT, timeout=2)


def main():
    check(100, 30)
    print("OK: ctrl hints held/released/combo, the associated text (é å A), option and cmd hints (BISE-277)")


if __name__ == "__main__":
    run(main)
