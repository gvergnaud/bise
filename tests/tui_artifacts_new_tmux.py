"""The `new` mark in /artifacts (user, 2026-10-04: clicking `↗ 24 new`
opens the list and he can't tell which ones are new), through the real
binaries: a throwaway hub on the fake provider, tmux at 150 and 80.

  one artifact, looked at: not new
  3 added while the list is closed: the header says `↗ 3 new`
  /artifacts: exactly those 3 rows say `new`, the old one does not;
      the header's count went (the hub has them seen), the marks stay
      while the list is open
  closed and opened again: no row says `new`
  a new version of the old one: its row and, in the versions box, v2
      say `new`, v1 does not
  at 80 the same, every line in 80 columns

ART_SHOTS=<dir> keeps the captures (.txt and .ansi) for the designer;
BISE_THEME=light for the light ones.

python3 -u tests/tui_artifacts_new_tmux.py
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import Tui, tui_session, run, MAIN_IDLE  # noqa: E402
from tui_artifacts_tmux import nbsp_as_spaces, shot, sb, command, fakes  # noqa: E402

NEW_ROW = re.compile(r"^[│\s]*›?\s*(.+?)\s{1,2}new\s{2,}")


def new_rows(sc):
    """The titles of the rows that say `new` in the list."""
    out = []
    for row in sc.splitlines():
        m = NEW_ROW.match(row)
        if m:
            out.append(m.group(1).strip())
    return out


def write(E, name, words):
    path = os.path.join(E.ws, name)
    with open(path, "w") as f:
        f.write(words)
    return path


def visit(t, E, width, tag=""):
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    # one artifact, looked at once
    write(E, "old.md", "# old\n")
    sb(E, "main", E.ws, "artifact", "add", "old.md", "--title", "old notes")
    t.wait("↗ main · old notes")
    command(t, "/artifacts", "artifacts")
    t.wait("old notes")
    t.keys("Escape")
    t.wait_gone("all agents ·")
    # 3 while the list is closed
    for name, title in (("a.md", "alpha plan"), ("b.md", "beta sheet"), ("c.md", "gamma deck")):
        write(E, name, "# %s\n" % title)
        sb(E, "main", E.ws, "artifact", "add", name, "--title", title)
    t.wait("↗ 3 new")
    shot(t, "%s-header%s" % (width, tag))
    command(t, "/artifacts", "all agents · 4")
    sc = t.wait("gamma deck")
    assert sorted(new_rows(sc)) == ["alpha plan", "beta sheet", "gamma deck"], sc
    shot(t, "%s-new%s" % (width, tag))
    # the header's count went; the marks stay while it is open
    t.keys("Down")
    sc = t.wait_re(r"› beta sheet")
    assert sorted(new_rows(sc)) == ["alpha plan", "beta sheet", "gamma deck"], sc
    for line in sc.splitlines():
        assert len(line) <= width, "too wide: %r" % line
    t.keys("Escape")
    sc = t.wait_gone("all agents ·")
    assert not re.search(r"↗ \d+ new", sc), "the header's count goes once you looked:\n" + sc
    # opened again: nothing new
    command(t, "/artifacts", "all agents · 4")
    sc = t.wait("gamma deck")
    assert new_rows(sc) == [], sc
    t.keys("Escape")
    t.wait_gone("all agents ·")


def versions(t, E):
    """A new version of an old one: the row and its v2 say `new`."""
    path = os.path.join(E.ws, "old.md")
    with open(path, "a") as f:
        f.write("and more\n")
    os.utime(path, (1, 2_000_000_000))
    out = sb(E, "main", E.ws, "artifact", "add", "old.md")
    assert "v2" in out, out
    t.wait("↗ main · old notes")
    command(t, "/artifacts", "all agents · 4")
    sc = t.wait_re(r"› old notes")
    assert new_rows(sc) == ["old notes"], sc
    t.keys("v")
    sc = t.wait("old notes · 2 versions")
    v2 = [r for r in sc.splitlines() if re.search(r"│ .{2}v2 ", r)]
    v1 = [r for r in sc.splitlines() if re.search(r"│ .{2}v1 ", r)]
    assert v2 and " new" in v2[0], sc
    assert v1 and " new" not in v1[0], sc
    shot(t, "150-versions-new")
    t.keys("Escape")
    t.wait_gone("2 versions")
    t.keys("Escape")
    t.wait_gone("all agents ·")


def main():
    nbsp_as_spaces(Tui)
    E = e2e.Env()
    env, _ = fakes(E)
    with tui_session(150, 40, env, E=E) as t:
        visit(t, E, 150)
        versions(t, E)
        print("PASS tui artifacts new 150")
    E = e2e.Env()
    env, _ = fakes(E)
    with tui_session(80, 30, env, E=E) as t:
        visit(t, E, 80)
        print("PASS tui artifacts new 80")
    # the light palette, for the designer's captures (150 and 80)
    for cols, rows in ((150, 40), (80, 30)):
        E = e2e.Env()
        env, _ = fakes(E)
        with tui_session(cols, rows, env, E=E) as t:
            t.wait("bise :*")
            t.wait_re(MAIN_IDLE)
            command(t, "/theme light", "light")
            visit(t, E, cols, "-light")
            print("PASS tui artifacts new %d light" % cols)


if __name__ == "__main__":
    run(main)
