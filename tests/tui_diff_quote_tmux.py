"""Select + type to quote in the diff panel (diffquote.rs, designer
m_7568), through the real binaries: a throwaway hub on the fake
provider, tmux at 150 and 80 columns.

  t1 in its own worktree commits a 40-line file; in t1's view ctrl+g
      opens its diff on the right; a drag over 3 lines tints them and
      puts ` type to ask t1 about it · tab quote · cmd+c copy ` right above them;
      typing puts `❝ 1` in t1's composer (the strip says
      `src/pricing.tsx:6-8 · 3 lines`), ⏎ sends: t1's model gets the
      `<selection from="t1 vs main" file="src/pricing.tsx" new="6-8">`
      tag with the lines and the words, its thread a `❝` line
  only removed lines: `… · 2 removed lines`, `old="…"` only
  from main's view, `/diff <t1's branch>`: main gets it
  at 80 the diff takes the screen: shift+↓ selects, a letter quotes and
      the panel closes

DQ_SHOTS=<dir> keeps the captures (.txt and .ansi) for the designer;
DQ_LIGHT=1 runs it in the light palette.

python3 -u tests/tui_diff_quote_tmux.py
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, wait_until, in_view, MAIN_IDLE  # noqa: E402
from tui_artifacts_tmux import sb, command  # noqa: E402

SHOTS = os.environ.get("DQ_SHOTS")
LIGHT = os.environ.get("DQ_LIGHT") == "1"


def shot(t, name):
    if SHOTS:
        os.makedirs(SHOTS, exist_ok=True)
        name += "-light" if LIGHT else "-dark"
        with open(os.path.join(SHOTS, name + ".ansi"), "w") as f:
            f.write(t.screen(colors=True))
        with open(os.path.join(SHOTS, name + ".txt"), "w") as f:
            f.write(t.screen())


def mouse(t, kind, x, y):
    """SGR 1006 at 0-based (x, y): `press`, `drag` (button 32), `release`."""
    code, end = {"press": (0, "M"), "drag": (32, "M"), "release": (0, "m")}[kind]
    t.typed("\x1b[<%d;%d;%d%s" % (code, x + 1, y + 1, end))


def row_of(sc, needle, x0=0):
    for y, row in enumerate(sc.splitlines()):
        x = row.find(needle, x0)
        if x >= 0:
            return x, y
    raise AssertionError("not on screen: %r\n%s" % (needle, sc))


def drag_rows(t, first, last, x0):
    """A drag in the panel from the row showing `first` to the one
    showing `last`."""
    sc = t.screen()
    x, y0 = row_of(sc, first, x0)
    _, y1 = row_of(sc, last, x0)
    mouse(t, "press", x, y0)
    mouse(t, "drag", x + 2, (y0 + y1) // 2)
    mouse(t, "drag", x + 2, y1)
    mouse(t, "release", x + 2, y1)
    return x, y0


def sent_to(E, agent, needle, timeout=40):
    def got():
        for r in E.fake_requests():
            if r.get("agent") == agent and needle in json.dumps(r):
                return r
        return None
    return wait_until(got, timeout, lambda: "%s never got %r: %s" % (agent, needle, E.fake_requests()[-3:]))


def worktree(E):
    out = sb(E, "main", E.ws, "spawn", "t1", "--place", "new", "--objective", "make the pricing page")
    wt = out.split(" in worktree ")[1].split(" (branch ")[0]
    branch = out.split(" (branch ")[1].split(")")[0]
    os.makedirs(os.path.join(wt, "src"))
    open(os.path.join(wt, "src", "pricing.tsx"), "w").write("".join("line %d\n" % i for i in range(40)))
    with open(os.path.join(wt, "README"), "a") as f:
        f.write("pricing\n")
    e2e.sh(wt, "git add -A && git commit -qm pricing")
    return wt, branch


def wide(t):
    E = t.E
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    if LIGHT:
        command(t, "/theme light", "light")
    wt, branch = worktree(E)
    # t1's view (a click on its row in the panel), then its diff
    sc = t.wait_re(r"\bt1\b")
    x, y = row_of(sc, "t1", 150 - 40)
    mouse(t, "press", x, y)
    mouse(t, "release", x, y)
    t.wait_re(in_view("t1"), 30)
    t.keys("C-g")
    sc = t.wait("line 5")
    panel_x = sc.splitlines()[[i for i, r in enumerate(sc.splitlines()) if "line 5" in r][0]].find("line 5") - 12
    x, y0 = drag_rows(t, "line 5", "line 7", panel_x)
    hint = " type to ask t1 about it · tab quote · cmd+c copy "
    sc = t.wait(hint)
    rows = sc.splitlines()
    assert hint in rows[y0 - 1], "the popup right above the selection:\n" + sc
    shot(t, "150-selected")
    t.typed("why these three?")
    sc = t.wait("❝ 1")
    t.wait_gone(hint)
    sc = t.wait("src/pricing.tsx:6-8 · 3 lines")
    shot(t, "150-composer")
    t.keys("Enter")
    r = sent_to(E, "t1", 'new=\\"6-8\\"')
    blob = json.dumps(r)
    assert 'from=\\"t1 vs main\\" file=\\"src/pricing.tsx\\" new=\\"6-8\\"' in blob, blob
    assert "+line 5\\n+line 6\\n+line 7" in blob and "why these three?" in blob, blob
    sc = t.wait_re(r"❝ line 5 line 6 line 7 · src/pricing.tsx:6-8 · 3 lines")
    shot(t, "150-thread")

    # t1's turn runs meanwhile: give the close the time of a busy frame
    t.keys("C-g")
    t.wait_gone("t1 vs main", 30)

    # from main's view, the same branch: the popup names main
    x, y = row_of(t.screen(), "main", 150 - 40)
    mouse(t, "press", x, y)
    mouse(t, "release", x, y)
    t.wait_re(in_view("main"), 30)
    command(t, "/diff " + branch, "line 5")
    sc = t.screen()
    panel_x = sc.splitlines()[[i for i, r in enumerate(sc.splitlines()) if "line 2" in r][0]].find("line 2") - 12
    drag_rows(t, "line 2", "line 3", panel_x)
    sc = t.wait(" type to ask main about it · tab quote · cmd+c copy ")
    shot(t, "150-from-main")
    t.keys("Escape")
    t.keys("C-g")
    t.wait_gone(" vs main")


def removed(t):
    """A branch that only removes lines: the strip says so."""
    E = t.E
    e2e.sh(E.ws, "printf 'a\\nb\\nc\\nd\\n' > gone.txt && git add gone.txt && git commit -qm gone")
    e2e.sh(E.ws, "git checkout -qb trim && sed -i '' -e '2,3d' gone.txt && git commit -qam trim && git checkout -q -")
    command(t, "/diff trim", "gone.txt")
    sc = t.wait("− b")
    px = row_of(sc, "− b")[0] - 10
    drag_rows(t, "− b", "− c", px)
    t.wait(" type to ask main about it")
    t.typed("x")
    t.wait("gone.txt:2-3 · 2 removed lines")
    shot(t, "150-removed")
    t.keys("C-u")


def narrow(t):
    t.start(80, 30)
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    if LIGHT:
        command(t, "/theme light", "light")
    command(t, "/diff trim", "gone.txt")
    sc = t.wait("− b")
    shot(t, "80-diff")
    # the cursor down to `− b`, then shift+↓
    for _ in range(40):
        sc = t.screen()
        if "open gone.txt:2 in your editor" in sc:
            break
        t.keys("Down")
    else:
        raise AssertionError("the cursor never reached −b:\n" + sc)
    t.keys("S-Down")
    sc = t.wait(" type to ask main about it")
    shot(t, "80-selected")
    t.typed("fix")
    sc = t.wait("❝ 1")
    assert "gone.txt:2-3 · 2 removed lines" in sc, sc
    shot(t, "80-composer")


def main():
    with tui_session(150, 42) as t:
        wide(t)
        removed(t)
        narrow(t)
        print("PASS tui diff quote")


if __name__ == "__main__":
    run(main)
