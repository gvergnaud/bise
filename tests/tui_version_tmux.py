"""The /version picker in a real terminal (tmux), against the fake
provider: '/version ' opens the list of versions in the composer popup
(tree, the recent commits of this repo, with their marks), the typed text
filters it, the arrows move, Enter asks the hub to build then switch.
An unknown commit gets a clear answer.

python3 -u tests/tui_version_tmux.py
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402



def git(*a):
    return subprocess.run(["git", *a], cwd=e2e.ROOT, capture_output=True, text=True).stdout.strip()


def main():
    E = e2e.Env()
    head = git("log", "-1", "--format=%h")
    second = git("log", "-2", "--format=%h").splitlines()[-1]
    second_subject = git("log", "-1", "--format=%s", second)
    # a bise home of its own: the marks do not depend on the versions
    # built in the real one, and its build dir is a file, so the build
    # Enter starts fails at once (versions.sh: mkdir) and builds nothing
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(os.path.join(bise, "dev"))
    open(os.path.join(bise, "dev", "build"), "w").close()
    with tui_session(160, 42, "BISE_HOME=%s" % bise, E=E) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # the popup: tree and the commits, with the current one marked
        t.typed("/version ")
        sc = t.wait("the working tree")
        assert head in sc, sc
        assert "◉" in sc and "[current]" in sc, sc   # the dev tree runs: tree is current
        # the text filters (on the revision or the subject)
        t.typed(second)
        t.wait("/version %s" % second)     # the keys are in (the subject is on screen before the filter)
        t.wait_gone("the working tree")
        sc = t.wait(second_subject[:40])
        assert "the working tree" not in sc, sc
        assert head not in sc.split("/version")[-1] or head == second, sc
        # clear the filter, move down twice (tree -> head -> second)
        for _ in range(len(second)):
            t.keys("BSpace")
        t.wait("the working tree")
        for _ in range(2):
            # the selected row is a color: each move drawn before the next key
            before = t.screen(colors=True)
            t.keys("Down")
            t.wait_any([lambda s, before=before: s != before], 10, colors=True)
        # Tab fills the composer with the selected entry
        t.keys("Tab")
        sc = t.wait("/version %s" % second)
        # an unknown commit: a clear answer, nothing is built
        for _ in range(len("/version %s" % second)):
            t.keys("BSpace")
        t.typed("/version zzzz999")   # matches no entry: the popup is closed
        t.wait("/version zzzz999")
        t.wait_gone("the working tree")
        t.keys("Enter")
        t.wait("unknown commit zzzz999")
        # Enter on an entry builds, then switches: the build is announced
        # (the working tree: a commit would first check out a worktree)
        t.typed("/version ")
        t.wait("the working tree")   # the popup's first row, selected
        t.keys("Enter")
        t.wait("version tree: building", 20)
        t.wait("build of tree failed", 20)
        print("OK: /version picker (list, filter, arrows, Tab, unknown commit, Enter builds)")


if __name__ == "__main__":
    run(main)
