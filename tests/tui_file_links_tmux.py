"""File links in the feed (BISE-264), in a real terminal (tmux) against
the fake provider: a path to a file of the workspace in the reply is a
link (OSC 8 `file://…`, tmux keeps it: capture-pane -e), and a plain
click opens it in $EDITOR at its line. The editors are fakes (scripts
named like the real ones: bise picks the line syntax by the name):

- `code` (a GUI editor) runs detached as `code -g <file>:<line>`;
- `vim` (a terminal editor) runs in the terminal panel as
  `vim +<line> <file>`; when it exits the panel goes back as it was.

python3 -u tests/tui_file_links_tmux.py
"""
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from tui_tmux import tui_session, run, wait_until, MAIN_IDLE  # noqa: E402

COLS, ROWS = 150, 42
MSG = "look at notes/plan.md:3 and missing/nope.md:2"


def fake(tmp, name, body):
    p = os.path.join(tmp, name)
    with open(p, "w") as f:
        f.write("#!/bin/sh\n" + body)
    os.chmod(p, 0o755)
    return p


def click(t, x, y):
    """A plain click (press + release, SGR 1006) at the 0-based cell."""
    t.typed("\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1))


def ask(t):
    """Send MSG; the reply's row and the path's column in it."""
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    t.typed(MSG)
    t.keys("Enter")
    t.wait("ack: " + MSG)
    rows = t.screen().splitlines()
    y = max(i for i, r in enumerate(rows) if "ack: look at" in r)
    return rows, y, rows[y].find("notes/plan.md")


def args_of(log):
    return open(log).read().splitlines() if os.path.exists(log) else []


def main():
    tmp = tempfile.mkdtemp(prefix="sbfilelinks")
    log = os.path.join(tmp, "args")
    blank = "VISUAL= BISE_EDITOR= "    # the host's own editor never runs

    # a GUI editor: detached, `-g file:line`
    code = fake(tmp, "code", 'for a in "$@"; do echo "$a"; done >> %s\n' % log)
    E = e2e.Env()
    os.makedirs(os.path.join(E.ws, "notes"))
    with open(os.path.join(E.ws, "notes", "plan.md"), "w") as f:
        f.write("one\ntwo\nthree\n")
    with tui_session(COLS, ROWS, blank + "EDITOR=" + code, E=E) as t:
        rows, y, x = ask(t)
        # the existing path is a file:// hyperlink; the missing one is not
        sc = t.screen(colors=True)
        assert "notes/plan.md\x1b\\" in sc and ";file://" in sc, sc[-3000:]
        assert "missing/nope.md\x1b\\" not in sc, sc[-3000:]
        click(t, x + 3, y)
        wait_until(lambda: len(args_of(log)) >= 2, 10, lambda: "the fake code never ran: %r" % args_of(log))
        got = args_of(log)
        assert got[0] == "-g", got
        path, line = got[1].rsplit(":", 1)
        assert line == "3" and os.path.realpath(path) == os.path.realpath(os.path.join(E.ws, "notes", "plan.md")), got
        t.wait("opening plan.md:3 in code")
        # the missing path is plain text: a click opens nothing
        click(t, rows[y].find("missing/nope.md") + 3, y)
        # the opener is a process of its own: its log line may come late
        wait.holds(lambda: len(args_of(log)) == 2, 0.5, lambda: "no open on plain text: %r" % args_of(log))
    os.remove(log)

    # a terminal editor: in the terminal panel, `+line file`; it exits on
    # Enter and the panel hides again
    vim = fake(tmp, "vim", 'for a in "$@"; do echo "$a"; done >> %s\necho "FAKEVIM $*"\nread x\n' % log)
    E = e2e.Env()
    os.makedirs(os.path.join(E.ws, "notes"))
    with open(os.path.join(E.ws, "notes", "plan.md"), "w") as f:
        f.write("one\ntwo\nthree\n")
    with tui_session(COLS, ROWS, blank + "EDITOR=" + vim, E=E) as t:
        rows, y, x = ask(t)
        click(t, x + 3, y)
        t.wait("terminal · vim · ctrl+` hide")
        t.wait("FAKEVIM +3 ")
        got = args_of(log)
        assert got[0] == "+3" and os.path.realpath(got[1]) == os.path.realpath(os.path.join(E.ws, "notes", "plan.md")), got
        # the keys go to the editor: Enter ends it, the panel goes
        t.keys("Enter")
        t.wait_gone("terminal · vim", 20)
    print("PASS tui file links")


if __name__ == "__main__":
    run(main)
