"""Drive the switchboard TUI in a real terminal (tmux) against the fake
provider, and check the screen: panel, checkout, Esc, preview, cards.

python3 -u tests/tui_tmux.py
"""
import contextlib
import itertools
import os
import re
import shlex
import subprocess
import sys
import traceback

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bise_env  # noqa: E402
import e2e  # noqa: E402
import wait  # noqa: E402

def tmux(*a):
    return subprocess.run(["tmux", *a], capture_output=True, text=True).stdout


def wait_until(fn, timeout, what, poll=wait.SCREEN_POLL):
    """wait.until at the screen's pace (a capture-pane every 0.2 s): the
    screen, the fake provider's log, a file."""
    return wait.until(fn, timeout, what, poll)


def on_screen(cond, sc):
    """`cond` holds on the screen `sc`: a substring, a compiled regex
    (searched) or a function of the screen."""
    if isinstance(cond, str):
        return cond in sc
    if isinstance(cond, re.Pattern):
        return cond.search(sc) is not None
    return bool(cond(sc))


class Tui:
    """One switchboard TUI in its own tmux session, on the throwaway hub
    `E` (e2e.Env). Made by `tui_session`, which also tears it down."""

    def __init__(self, E, name):
        self.E = E
        self.name = name

    def screen(self, colors=False):
        """The pane's text (with colors: its SGR escapes too)."""
        return tmux("capture-pane", "-p", *(["-e"] if colors else []), "-t", self.name)

    def keys(self, *k):
        tmux("send-keys", "-t", self.name, *k)

    def typed(self, text):
        tmux("send-keys", "-t", self.name, "-l", text)

    def wait_any(self, conds, timeout=40, poll=0.2, colors=False):
        """Poll the screen (with its SGR escapes if `colors`) until one of
        `conds` holds; return (its index, the screen). A cond is a
        substring, a compiled regex (searched multiline) or a function of
        the screen. The one poll loop of the tmux tests: a flake fix here
        fixes them all."""
        last = [""]

        def match():
            last[0] = sc = self.screen(colors)
            for i, c in enumerate(conds):
                if on_screen(c, sc):
                    return i, sc
            return None

        def missing():
            print(last[0])
            if not last[0].strip():
                # an empty screen: the session is gone (tmux said why)
                r = subprocess.run(["tmux", "capture-pane", "-p", "-t", self.name],
                                   capture_output=True, text=True)
                print("[empty screen: tmux %r; the TUI's exit is on its screen while it lives]"
                      % r.stderr.strip())
            return "not on screen: %s" % " | ".join(
                "/%s/" % c.pattern if isinstance(c, re.Pattern) else
                repr(c) if isinstance(c, str) else getattr(c, "__doc__", None) or c.__name__
                for c in conds)
        return wait_until(match, timeout, missing, poll)

    def wait(self, needle, timeout=40):
        """Wait for the text `needle` on the screen; return the screen."""
        return self.wait_any([needle], timeout)[1]

    def wait_re(self, pattern, timeout=40):
        """Wait for the regex `pattern` on the screen (multiline)."""
        return self.wait_any([re.compile(pattern, re.M)], timeout)[1]

    def wait_gone(self, needle, timeout=10):
        def gone(sc):
            return needle not in sc
        gone.__doc__ = "gone: %r" % needle
        return self.wait_any([gone], timeout, poll=0.1)[1]

    def sync(self, mark="¤"):
        """A sentinel for "nothing happens" checks: every key and click
        sent before it was handled once a mark typed after them shows in
        the composer, then is erased (the TUI reads its input in order).
        Then check that nothing happened: no fixed wait."""
        self.typed(mark)
        self.wait(mark, 10)
        self.keys("BSpace")
        self.wait_gone(mark)

    def press_until(self, key, cond, sel=None, tries=20, must=True):
        """Press `key` until `cond` (as in wait_any) is on screen; return
        the screen. After each press, wait until the TUI drew it before
        the next one (a blind gap read the screen too early under load and
        the next key overshot the row, BISE-292; two esc too close are
        alt+esc): the match of the regex `sel` (the selected row) changed,
        or the screen did. Not there after `tries`: AssertionError, or
        None when not `must` (the caller waits on)."""
        for _ in range(tries):
            sc = self.screen()
            if on_screen(cond, sc):
                return sc
            was = sel.search(sc).group(0) if sel and sel.search(sc) else None
            self.keys(key)

            def drawn(s, sc=sc, was=was):
                if on_screen(cond, s):
                    return True
                if sel:
                    m = sel.search(s)
                    return m is not None and m.group(0) != was
                return s != sc
            drawn.__doc__ = "the TUI drew %s" % key
            self.wait_any([drawn], 10)
        if not must:
            return None
        print(self.screen())
        raise AssertionError("not on screen after %d %s: %s" % (tries, key, cond))

    def start(self, cols, rows, extra_env=""):
        """(Re)open the TUI: a new tmux session of the same name."""
        tmux("kill-session", "-t", self.name)
        start_tui(self.E, cols, rows, extra_env, self.name)

    def close(self, ok):
        """Kill the session, stop the hub the TUI started, clean up (the
        throwaway dirs are kept, SB_KEEP, when the test failed). A hub of
        another tree fails the test that passed (refuse_other_root)."""
        tmux("kill-session", "-t", self.name)
        if ok and "BISE_APP_ROOT" not in self.E.env:
            refuse_other_root(self.E.state)
        sock = os.path.join(self.E.state, "hub.sock")
        try:
            e2e.stop_hub(sock)
            wait.until(lambda: not os.path.exists(sock), 1, "the hub's socket gone", poll=0.05)
        except Exception:
            pass
        if not ok:
            os.environ["SB_KEEP"] = "1"
        self.E.close()


_sessions = itertools.count()


@contextlib.contextmanager
def tui_session(cols, rows, env="", E=None):
    """`with tui_session(150, 42) as t:` a TUI on a new throwaway hub (or
    on `E`, which it then owns), torn down at the end, pass or fail."""
    E = E or e2e.Env()
    t = Tui(E, "sbtui%d_%d" % (os.getpid(), next(_sessions)))
    ok = False
    try:
        t.start(cols, rows, env)
        yield t
        ok = True
    finally:
        t.close(ok)


def run(test):
    """A test file's main: `test()` passes or its traceback prints, exit 1."""
    try:
        test()
    except BaseException:
        traceback.print_exc()
        sys.exit(1)
    sys.exit(0)


def panel_row(n, name):
    """The regex of agent `name`'s panel row: its number, a status glyph."""
    return r"\b%d \S+ %s\b" % (n, name)


def in_view(name):
    """The regex of the divider naming the agent in view (book §8 "The
    frame": `├─ you → main ─…─ 42k · 4% ─┤`, BISE-98)."""
    return r"you → %s " % re.escape(name)


# main idle (BISE-303: the divider and the panel say no `idle` at rest):
# its panel row's glyph (from 90 columns), or, in main's view, a divider
# label with no gust after it (`you → main · mistral-small · high · yolo ─`)
MAIN_IDLE = r"\b0 ○ main\b|you → main(?: · [\w.\-]+(?:[ ·][\w.\-]+)*)* ─"


PLACEHOLDER = re.compile(r"^(what's on your mind\?|talk to \S+ directly)$")


def pane_rows(rows):
    """The composer's rows (BISE-98): from the bottom, the first run of
    rows whose text, inside the frame's edges, starts with the bar `│`
    (the key bar and the frame's bottom edge are skipped; a popup may
    hide the divider); each row's text after the bar."""
    out = []
    for r in reversed(rows):
        r = r.rstrip()
        if r.startswith("│"):
            r = r[1:].rstrip()
            if r.endswith("│"):
                r = r[:-1]
        if r.lstrip().startswith("│"):
            text = r.lstrip()[1:].strip()
            # the empty composer's dim placeholder is not text
            out.append("" if PLACEHOLDER.match(text) else text)
        elif out:
            break
    return out[::-1]


XDG_HOMES = ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME")


def pane_env(env):
    """What a pane's command sets from the test's env. A pane starts from
    the tmux server's env, i.e. whoever started the server (the user's
    real HOME): the TUI starts the hub, so a HOME left to the server made
    the hub's REPLs scan his ~/.vibe/skills and ~/.agents/skills (32
    tui_*_tmux tests failed bise_env.refuse_real_skills, m_10559). So the
    HOME, the toolchain homes, PATH, the XDG homes and every bise variable
    (SB_, BEND_, BISE_, MISTRAL_) come from the test, never the server, and
    BISE_APP_ROOT is this tree (docs/issues/14: a pane ran another gate's
    runtime) unless the test names one."""
    keep = ("HOME", "CARGO_HOME", "RUSTUP_HOME", "PATH") + XDG_HOMES
    out = {k: v for k, v in env.items()
           if k in keep or k.startswith(("SB_", "BEND_", "BISE_", "MISTRAL_"))}
    out.setdefault("BISE_APP_ROOT", e2e.ROOT)
    return out


def refuse_other_root(state):
    """The hub the TUI started runs this tree's runtime: its hub.root is
    e2e.ROOT (docs/issues/14), else the test tested other code."""
    try:
        root = open(os.path.join(state, "hub.root")).read().strip()
    except FileNotFoundError:
        return
    if os.path.realpath(root) != os.path.realpath(e2e.ROOT):
        raise AssertionError("the TUI's hub ran another tree: hub.root %s, this tree %s" % (root, e2e.ROOT))


def pane_unset(env):
    """What a pane's command unsets before it sets pane_env (env(1) applies
    the -u first): every internal and test variable, the caller's path
    overrides and the XDG homes, whatever the server's env holds."""
    return list(bise_env.NOT_INHERITED + bise_env.OWN_PATHS + XDG_HOMES)


def start_tui(E, cols, rows, extra_env, session):
    """Open the switchboard TUI of the throwaway hub E in the tmux session
    `session`, with E's SB_/BEND_/MISTRAL_ env and its BISE_APPROVALS
    (+ extra_env, "K=V ...")."""
    # shlex.quote, not list2cmdline: the line runs in `sh -c`/`zsh -c`, where
    # double quotes still run backticks and $(…) (an agent's BEND_TOOLS_NOTE
    # holds `node`: the pane ran a node REPL and bise never started)
    bise_env.refuse_real_home(E.env)
    envs = " ".join("%s=%s" % (k, shlex.quote(v)) for k, v in pane_env(E.env).items())
    unset = " ".join("-u " + k for k in pane_unset(E.env))   # tmux's server env may carry them
    # a TUI that exits early leaves its last screen and its exit code
    # until close() kills the session (the timeout print shows them)
    # the test's extra_env last: it wins over E.env (tui_onboarding_tmux
    # gives its own HOME and BISE_HOME there)
    cmd = "cd %s && env %s %s%s %s switchboard --workspace %s; echo \"[switchboard exited: $?]\"; sleep 600" % (
        e2e.ROOT, unset, envs, " " + extra_env if extra_env else "", e2e.EXE, E.ws)
    tmux("new-session", "-d", "-s", session, "-x", str(cols), "-y", str(rows), cmd)


def main():
    with tui_session(150, 42) as t:
        sc = t.wait("bise :*")
        t.wait_re(in_view("main"))
        assert "@ file   $ skills   / commands" in sc, sc
        t.wait_re(MAIN_IDLE)
        t.typed('crée [[bash: sb spawn t1 --objective "écris {{bash: echo hi-t1}}"]]')
        t.keys("Enter")
        sc = t.wait_re(panel_row(1, "t1"))
        t.wait("new agent @t1")
        t.wait_re(r"t1 +(→ \S+|m_\d)", 60)           # the automatic reply in main's feed (level 3: names in columns)
        # select the task with ⌥↓ (next: main, then t1), enter it
        t.keys("M-Down")
        t.keys("M-Down")
        sc = t.wait("⏎ enter   space preview")
        t.keys("Enter")
        sc = t.wait_re(in_view("t1"))
        assert "you're talking to t1 directly. main isn't in the loop. esc back to main." in sc, sc
        # its brief, folded (BISE-12)
        assert "brief" in sc, sc
        t.wait("done: tool bash ok: hi-t1")
        # talk to it directly
        t.typed("salut t1")
        t.keys("Enter")
        t.wait("ack: salut t1")
        # Esc goes back to main, which learns about it
        t.keys("Escape")
        sc = t.wait_re(in_view("main"))
        t.wait("You talked to @t1 (1 message)")
        # Alt+1 checks out task 1 again; Esc back
        t.keys("M-1")
        t.wait_re(in_view("t1"))
        t.keys("Escape")
        t.wait_re(in_view("main"))
        # preview: select, Space; the status says it; Esc closes
        t.keys("M-Down")
        t.keys("M-Down")
        t.keys("Space")
        t.wait("preview of t1")
        t.keys("Escape")
        # a slash command and its notice
        t.typed("/agents")
        t.keys("Enter")
        t.wait("écris {{bash: echo hi-t1}}")
        # archive from the panel with D: it asks first (BISE-43), y archives
        t.keys("M-Down")
        t.keys("M-Down")
        t.typed("D")
        t.wait("archive t1? /restore brings it back. y / n")
        t.typed("y")
        t.wait("@t1 archived", 20)
        t.wait("1 archived")
        print(t.screen())
    print("PASS tui")


if __name__ == "__main__":
    run(main)
