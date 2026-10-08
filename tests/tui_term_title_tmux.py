"""The terminal's tab title (title-bise, designer's pick m_13176) in a
real terminal (tmux, its `#{pane_title}`) on a throwaway hub and the fake
provider: `?N` the inbox's cards, `↻N` the agents at work, then `bise`
and the repo's folder, each count left out at 0.

  the pane's title before bise: `before-bise`
  the TUI starts: `bise · ws`
  a card arrives: `?1 bise · ws`
  main adds an artifact: still `?1 bise · ws` (the title does not count them)
  t1 starts (its brief is slow): `?1 ↻1 bise · ws`; it stops: `?1 bise · ws`
  /quit: the title bise found is back (`before-bise`, CSI 23 t)
  BISE_TERM_TITLE=0: the title is never touched

python3 -u tests/tui_term_title_tmux.py
"""
import os
import shlex
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bise_env  # noqa: E402
import e2e  # noqa: E402
import wait  # noqa: E402
from tui_tmux import Tui, tmux, wait_until, run, MAIN_IDLE, pane_env, pane_unset  # noqa: E402

BEFORE = "before-bise"


def start(t, extra_env=""):
    """The TUI of t.E in tmux session t.name, after a shell set the
    pane's title to BEFORE (tui_tmux.start_tui with that one step first)."""
    E = t.E
    tmux("kill-session", "-t", t.name)
    envs = " ".join("%s=%s" % (k, shlex.quote(v)) for k, v in pane_env(E.env).items())
    unset = " ".join("-u " + k for k in pane_unset(E.env))
    cmd = ("printf '\\033]2;%s\\007'; sleep 0.3; cd %s && env %s %s%s %s switchboard --workspace %s; "
           "echo \"[switchboard exited: $?]\"; sleep 600") % (
        BEFORE, e2e.ROOT, unset, envs, " " + extra_env if extra_env else "", e2e.EXE, E.ws)
    tmux("new-session", "-d", "-s", t.name, "-x", "150", "-y", "40", cmd)


def title(t):
    return tmux("display-message", "-p", "-t", t.name, "#{pane_title}").rstrip("\n")


def title_is(t, want, timeout=40):
    wait_until(lambda: title(t) == want, timeout,
               lambda: "pane_title %r, wanted %r; screen:\n%s" % (title(t), want, t.screen()))


def sb(E, agent, *args):
    env = {**E.env, "SB_SOCKET": os.path.join(E.state, "agent.sock"), "SB_AGENT": agent}
    r = subprocess.run([e2e.EXE, "sb", *args], env=env, cwd=E.ws, capture_output=True, text=True, timeout=60)
    assert r.returncode == 0, "sb %s: %s %s" % (" ".join(args), r.stdout, r.stderr)
    return r.stdout.strip()


def quit_tui(t):
    t.typed("/quit")
    t.wait("/quit")
    t.keys("Enter")
    t.wait("[switchboard exited: 0]")


def live(t):
    E = t.E
    folder = os.path.basename(E.ws)
    start(t)
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    title_is(t, "bise · " + folder)
    print("ok: bise and the folder (%r)" % title(t))
    # a card arrives
    t.typed("[[bash: sb card \"$(printf 'pick one\\n1. alpha\\n2. beta')\"]]")
    t.keys("Enter")
    title_is(t, "?1 bise · " + folder)
    print("ok: a card: %r" % title(t))
    # main adds an artifact: the title does not count them
    sb(E, "main", "artifact", "add", "https://example.com/plan", "--title", "plan")
    t.wait("↗")   # the header says it
    wait.holds(lambda: title(t) == "?1 bise · " + folder, 1.0,
               lambda: "an artifact changed the title: %r" % title(t))
    print("ok: an artifact leaves it: %r" % title(t))
    # an agent starts (its brief keeps it at work 6 s), then stops
    sb(E, "main", "spawn", "t1", "--objective", "[[slow: 6]] say hi")
    title_is(t, "?1 ↻1 bise · " + folder)
    print("ok: t1 at work: %r" % title(t))
    title_is(t, "?1 bise · " + folder, timeout=60)
    print("ok: t1 stopped: %r" % title(t))
    quit_tui(t)
    title_is(t, BEFORE, timeout=10)
    print("ok: restored on exit: %r" % title(t))


def off(t):
    """BISE_TERM_TITLE=0: bise never writes a title."""
    start(t, "BISE_TERM_TITLE=0")
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    t.wait("inbox · 1 waiting for you")   # the card: a title would say it
    # a window: a title written by a later tick would show in it
    wait.holds(lambda: title(t) == BEFORE, 1.5, lambda: "no title written: %r" % title(t))
    quit_tui(t)
    assert title(t) == BEFORE, title(t)
    print("ok: BISE_TERM_TITLE=0 leaves %r" % title(t))


def main():
    E = e2e.Env()
    t = Tui(E, "sbtitle%d" % os.getpid())
    ok = False
    try:
        live(t)
        off(t)
        ok = True
    finally:
        t.close(ok)
    print("PASS tui term title")


if __name__ == "__main__":
    run(main)
