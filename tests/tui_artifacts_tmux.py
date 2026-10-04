"""Artifacts and diffs in the TUI (site/m/artifacts: B with C's doors, D,
E), through the real binaries: a throwaway hub on the fake provider, the
real `sb artifact add`, tmux at 150 and 80 columns.

  the empty /artifacts says how things get in
  main adds a doc and a page link: the ↗ lines in main's thread, the
      header's `↗ 2 new`
  /artifacts: the list as drawn, the key bar; `/` and `plan` narrow it,
      ⏎ gives the keys back (`1 of 2`); esc clears, esc closes; the
      header's count went
  the same file added again changed: v2, `v` opens its versions
  @ finds the doc by name: a `↗ q3 plan` chip in the composer, the key
      bar says how it goes; sent, the reply names it as a chip
  t1 in its own worktree commits 2 files: `/diff sb/t1` opens the panel
      on the right (150) with the keys, the composer dim and saying so;
      ⏎ on `all 2 files ▸` the file list; `fix this` typed lands in the
      composer (its f opens nothing), the bar ends `ctrl+g close`; a
      click in the panel takes the keys, esc closes it; ctrl+g opens and
      closes; at 80 the diff takes the screen, esc closes it

ART_SHOTS=<dir> keeps the captures (.txt and .ansi) for the designer.

python3 -u tests/tui_artifacts_tmux.py
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import Tui, tui_session, run, MAIN_IDLE  # noqa: E402

SHOTS = os.environ.get("ART_SHOTS")


def nbsp_as_spaces(Tui):
    """An artifact's chip keeps its words on one row with non-breaking
    spaces: the screen as read here has spaces."""
    screen = Tui.screen

    def plain(self, colors=False):
        return screen(self, colors).replace("\u00a0", " ")
    Tui.screen = plain


def shot(t, name):
    if SHOTS:
        os.makedirs(SHOTS, exist_ok=True)
        with open(os.path.join(SHOTS, name + ".ansi"), "w") as f:
            f.write(t.screen(colors=True))
        with open(os.path.join(SHOTS, name + ".txt"), "w") as f:
            f.write(t.screen())


def sb(E, agent, cwd, *args):
    env = {**E.env, "SB_SOCKET": os.path.join(E.state, "hub.sock"), "SB_AGENT": agent}
    r = subprocess.run([e2e.EXE, "sb", *args], env=env, cwd=cwd, capture_output=True, text=True, timeout=60)
    assert r.returncode == 0, "sb %s: %s %s" % (" ".join(args), r.stdout, r.stderr)
    return r.stdout.strip()


def command(t, line, needle):
    """Type `line` and ⏎ once: the popup runs it (a second ⏎ would land
    in the screen it opened)."""
    t.typed(line)
    t.wait(line)
    t.keys("Enter")
    return t.wait(needle)


def fakes(E):
    """The editor and the opener are fakes: a launch writes its words to
    `<tmp>/opened`; nothing of the host's ever runs."""
    bin_dir = os.path.join(E.tmp, "bin")
    os.makedirs(bin_dir, exist_ok=True)
    log = os.path.join(E.tmp, "opened")
    for name in ("zed", "open-fake"):
        path = os.path.join(bin_dir, name)
        with open(path, "w") as f:
            f.write("#!/bin/sh\necho %s \"$@\" >> %s\n" % (name, log))
        os.chmod(path, 0o755)
    env = "VISUAL= EDITOR= BISE_EDITOR=%s BISE_OPEN=%s" % (os.path.join(bin_dir, "zed"), os.path.join(bin_dir, "open-fake"))
    return env, log


def wide(t, E):
    if True:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        # the empty list
        command(t, "/artifacts", "artifacts · what your agents made")
        sc = t.wait("nothing yet. when an agent makes a page, a doc or a file for you, it lands here.")
        assert "agents add what they make with sb artifact add. bise pages come in by themselves." in sc, sc
        shot(t, "150-empty")
        t.keys("Escape")
        t.wait_gone("what your agents made")

        # main makes two
        os.makedirs(os.path.join(E.ws, "docs"))
        plan = os.path.join(E.ws, "docs", "q3-plan.md")
        open(plan, "w").write("# q3\n\nthree plans\n")
        out = sb(E, "main", E.ws, "artifact", "add", "docs/q3-plan.md", "--title", "q3 plan")
        assert "link it as [q3 plan](artifact:q3-plan)" in out, out
        sb(E, "main", E.ws, "artifact", "add", "https://bise.dev/m/artifacts", "--title", "artifacts mock")
        sc = t.wait_re(r"↗ \d new")
        sc = t.wait("artifacts mock")
        assert "↗ q3 plan" in sc and "doc · main" in sc, sc
        shot(t, "150-thread")

        # the list, its search, its keys
        command(t, "/artifacts", "artifacts · what your agents made")
        sc = t.wait("all agents · 2   tab this agent")
        assert "/ find: a title, an agent, a kind" in sc, sc
        assert "today" in sc and "› artifacts mock" in sc and "q3 plan" in sc, sc
        assert "⏎ open   space quick look   v versions   c copy   @ put it in a message   esc close" in sc, sc
        shot(t, "150-open")
        t.typed("/")
        t.wait("⏎ done   ↑↓ choose   esc clear the search")
        t.typed("plan")
        sc = t.wait("/ plan▏   1 of 2")
        assert "artifacts mock" not in sc.split("1 of 2")[1].split("q3 plan")[0], sc
        shot(t, "150-search")
        t.keys("Enter")
        sc = t.wait("/ plan   1 of 2")
        assert "esc clear the search" in sc and "v versions" in sc, sc
        shot(t, "150-search-done")
        t.keys("Escape")
        t.wait("/ find: a title, an agent, a kind")
        t.keys("Escape")
        sc = t.wait_gone("what your agents made")
        assert not re.search(r"↗ \d new", sc), "the header's count goes once you looked:\n" + sc

        # a second version
        open(plan, "a").write("and a fourth\n")
        os.utime(plan, (1, 2_000_000_000))
        out = sb(E, "main", E.ws, "artifact", "add", "docs/q3-plan.md")
        assert "v2" in out, out
        command(t, "/artifacts", "artifacts · what your agents made")
        # the newest first: q3 plan, now at v2, is selected
        sc = t.wait_re(r"› q3 plan .* v2")
        t.keys("v")
        sc = t.wait("q3 plan · 2 versions")
        assert "⏎ open this version   ↑↓ choose   esc back to the list" in sc, sc
        shot(t, "150-versions")
        t.keys("Escape")
        t.wait_gone("2 versions")
        t.keys("Escape")
        t.wait_gone("what your agents made")

        # @ puts the chip in your message
        t.typed("@q3")
        t.wait("q3 plan")
        t.keys("Enter")
        sc = t.wait("↗ q3 plan")
        t.typed("is it right?")
        sc = t.wait("the ↗ chips go as the artifact's link and title, never as @name")
        shot(t, "150-chip-composer")
        t.keys("Enter")
        sc = t.wait("ack:")
        t.wait_re(MAIN_IDLE, 60)
        log = open(E.fake_log).read()
        assert "[q3 plan](artifact:q3-plan)" in log, "the model gets the title and the link"
        shot(t, "150-chip-reply")

        # an agent in its own worktree, 2 files committed: its diff
        out = sb(E, "main", E.ws, "spawn", "t1", "--place", "new", "--objective", "make the pricing page")
        wt = out.split(" in worktree ")[1].split(" (branch ")[0]
        branch = out.split(" (branch ")[1].split(")")[0]
        os.makedirs(os.path.join(wt, "src"))
        open(os.path.join(wt, "src", "pricing.tsx"), "w").write("".join("line %d\n" % i for i in range(40)))
        with open(os.path.join(wt, "README"), "a") as f:
            f.write("pricing\n")
        e2e.sh(wt, "git add -A && git commit -qm pricing")
        diff_focus(t, branch, "150")
        light(t, "150", branch)
        return branch, wt


def click(t, x, y):
    """A left press + release (SGR 1006) at 0-based (x, y)."""
    t.typed("\x1b[<0;%d;%dM\x1b[<0;%d;%dm" % (x + 1, y + 1, x + 1, y + 1))


def at(sc, needle):
    for y, row in enumerate(sc.splitlines()):
        x = row.find(needle)
        if x >= 0:
            return x, y
    raise AssertionError("not on screen: %r\n%s" % (needle, sc))


def diff_focus(t, branch, width):
    """The diff panel on the right and the composer (designer m_7291): a
    letter is never lost. Opened by a key the panel has the keys and the
    composer says so; typed letters land in the composer (an `f` never
    opens the file list); esc closes; a click door leaves the keys to the
    composer; `ctrl+g close` always on the title row."""
    command(t, "/diff " + branch, "files")
    sc = t.wait("src/pricing.tsx")
    assert "vs main · 2 files" in sc and "all 2 files ▸" in sc, sc
    title = [r for r in sc.splitlines() if "vs main · 2 files" in r][0]
    assert "ctrl+g close" in title, title
    sc = t.wait("the diff has the keys · type to write here")
    assert "↑↓ scroll   tab next file   ⏎ open in your editor   esc close   type to write" in sc, sc
    shot(t, width + "-diff")
    # ⏎ on `all 2 files ▸` (the cursor's first row): the list
    t.keys("Enter")
    sc = t.wait("type to filter the files")
    assert "M changed   A added   D deleted" in sc, sc
    shot(t, width + "-diff-files")
    t.keys("Escape")
    t.wait_gone("type to filter the files")
    # typed with the panel focused: in the composer, the panel stays
    t.typed("fix this")
    sc = t.wait("fix this")
    assert "vs main · 2 files" in sc and "type to filter the files" not in sc, sc
    assert "the diff has the keys" not in sc, sc
    sc = t.wait("ctrl+g close", 5)
    assert sc.splitlines()[-2].rstrip(" │").endswith("ctrl+g close"), sc.splitlines()[-2]
    shot(t, width + "-diff-typed")
    # the composer has the keys: esc is its own, the panel stays
    t.keys("C-u")
    t.keys("Escape")
    sc = t.wait("vs main · 2 files")
    # a click in the panel takes the keys back; esc closes it
    x, y = at(sc, "src/pricing.tsx")
    click(t, x, y)
    t.wait("the diff has the keys · type to write here")
    t.keys("Escape")
    t.wait_gone("vs main · 2 files")
    # ctrl+g opens it with the keys (main's own changes), ctrl+g closes
    t.keys("C-g")
    sc = t.wait("the diff has the keys · type to write here")
    shot(t, width + "-diff-ctrl-g")
    t.keys("C-g")
    t.wait_gone("the diff has the keys")
    t.wait("agents")


def light(t, width, branch):
    """The same screens in the light palette (/theme light), then dark."""
    command(t, "/theme light", "light")
    t.wait_re(MAIN_IDLE)
    command(t, "/artifacts", "all agents · 2")
    shot(t, width + "-open-light")
    t.keys("Escape")
    t.wait_gone("all agents · 2")
    command(t, "/diff " + branch, "src/pricing.tsx")
    shot(t, width + "-diff-light")
    t.keys("C-g")
    t.wait_gone("vs main · 2 files")
    command(t, "/theme dark", "dark")


def narrow(t, branch):
    if True:
        t.start(80, 30, fakes(t.E)[0])
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        command(t, "/artifacts", "artifacts")
        sc = t.wait("all agents · 2")
        assert "what your agents made" not in sc and "tab this agent" not in sc, sc
        assert "⏎ open   space look   v versions   esc close" in sc, sc
        for line in sc.splitlines():
            assert len(line) <= 80, line
        shot(t, "80-open")
        t.typed("/")
        t.typed("plan")
        t.wait("/ plan▏   1 of 2")
        shot(t, "80-search")
        t.keys("Enter")
        t.wait("/ plan   1 of 2")
        shot(t, "80-search-done")
        t.keys("v")
        t.wait("2 versions")
        shot(t, "80-versions")
        t.keys("Escape")
        t.keys("Escape")
        t.keys("Escape")
        t.wait_gone("all agents · 2")
        command(t, "/diff " + branch, "src/pricing.tsx")
        sc = t.wait("esc close")
        assert "bise :* ── diff" in sc, sc
        assert "↑↓ scroll   tab next file   f files   ⏎ editor   esc close" in sc, sc
        assert "ctrl+g close" not in sc and "the diff has the keys" not in sc, sc
        shot(t, "80-diff")
        t.keys("Escape")
        t.wait_gone("bise :* ── diff")
        light(t, "80", branch)


def landed(t, E, env, branch, wt):
    """t1 lands: the `± 2 files` under its landed line opens that land
    (`t1 landed on main · <sha> · 2 files`, its files), never t1's branch
    vs today's main, empty once landed; `/diff <its branch>` says its
    work is all on main already and ⏎ shows what it landed last."""
    t.start(150, 40, env)
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    out = sb(E, "t1", wt, "land", "make the pricing page")
    sha = re.search(r"\(([0-9a-f]{7,})\)", out).group(1)[:7]
    sc = t.wait("± 2 files")
    shot(t, "150-landed-line")
    x, y = at(sc, "± 2 files")
    click(t, x + 2, y)
    sc = t.wait("t1 landed on main · %s · 2 files" % sha)
    sc = t.wait("src/pricing.tsx")
    assert "no changes against main" not in sc and "still working" not in sc, sc
    assert "README" in sc, sc
    # a click door: the composer keeps the keys
    assert "the diff has the keys" not in sc and "ctrl+g close" in sc, sc
    shot(t, "150-landed-diff")
    t.keys("C-g")
    t.wait_gone("t1 landed on main")
    # the branch, now on main: says so, ⏎ shows its last land
    command(t, "/diff " + branch, "work is all on main already")
    sc = t.wait("t1's work is all on main already · show what it landed last")
    assert "⏎ show what it landed last" in sc, sc
    shot(t, "150-landed-empty")
    t.keys("Enter")
    sc = t.wait("t1 landed on main · %s · 2 files" % sha)
    t.keys("Escape")
    t.wait_gone("t1 landed on main")


def main():
    nbsp_as_spaces(Tui)
    E = e2e.Env()
    env, _ = fakes(E)
    with tui_session(150, 40, env, E=E) as t:
        branch, wt = wide(t, E)
        print("PASS tui artifacts 150")
        narrow(t, branch)
        print("PASS tui artifacts 80")
        landed(t, E, env, branch, wt)
        print("PASS tui artifacts landed")


if __name__ == "__main__":
    run(main)
