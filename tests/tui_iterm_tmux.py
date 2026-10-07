"""iTerm2's two bugs (Gauthier), through the real binary in tmux on a
throwaway hub and the fake provider:

- ⌥0-9 where Option types characters (iTerm2's default Option key):
  the bytes iTerm2 sends are written to the pane. Without the kitty
  protocol ⌥1 is `¡`, ⌥0 `º`; with it (iTerm2 3.5+, flags 1+2+8+16)
  `CSI 49;3;161u`. Both go to agent 1 and back to main
  (BISE_OPTION_DIGITS=1: the U.S. layout, whatever this machine's). The
  Esc+ setting's ESC 1 still works; `€` (not a digit's) types.
- the artifact chips in the thread: the OSC 8 the terminal gets (its
  own cmd+click) is the page's https url or the doc's file://, never
  `artifact:` (macOS can't route it); a plain click opens it through
  bise (BISE_OPEN: a fake that logs).

python3 -u tests/tui_iterm_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from tui_tmux import tui_session, run, tmux, in_view, panel_row, wait_until, MAIN_IDLE  # noqa: E402
from tui_artifacts_tmux import sb, fakes, click, at, nbsp_as_spaces  # noqa: E402
from tui_tmux import Tui  # noqa: E402


def raw(t, s):
    tmux("send-keys", "-t", t.name, "-H", *("%02x" % b for b in s.encode()))


def keys(t):
    t.typed('crée [[bash: sb spawn t1 --objective "écris {{bash: echo hi-t1}}"]]')
    t.keys("Enter")
    t.wait_re(panel_row(1, "t1"))
    t.wait_re(MAIN_IDLE, 60)
    # legacy bytes: ⌥1 is `¡`, ⌥0 `º`
    for there, back, how in (("¡", "º", "the characters"), ("\x1b[49;3;161u", "\x1b[48;3;186u", "CSI u"),
                             ("\x1b1", "\x1b0", "Esc+")):
        raw(t, there)
        t.wait_re(in_view("t1"))
        raw(t, back)
        t.wait_re(in_view("main"))
        print("ok: ⌥1 / ⌥0 as %s" % how)
    # an option character that is not a digit's types
    raw(t, "€")
    t.wait("€")
    sc = t.screen()
    assert "¡" not in sc and "º" not in sc, sc
    t.keys("BSpace")


def chips(t, E, log):
    os.makedirs(os.path.join(E.ws, "docs"))
    plan = os.path.join(E.ws, "docs", "q3-plan.md")
    open(plan, "w").write("# q3\n\nthree plans\n")
    sb(E, "main", E.ws, "artifact", "add", "docs/q3-plan.md", "--title", "q3 plan")
    sb(E, "main", E.ws, "artifact", "add", "https://bise.dev/m/artifacts", "--title", "artifacts mock")
    t.wait("artifacts mock")
    sc = t.wait("↗ q3 plan")
    col = t.screen(colors=True)
    assert "\x1b]8;id=" in col, "no OSC 8 at all: %r" % col[-2000:]
    assert ";artifact:" not in col, "an artifact: url reached the terminal: %r" % col[-3000:]
    assert ";https://bise.dev/m/artifacts\x1b\\" in col, col[-3000:]
    assert ";file://" + os.path.realpath(plan) in col or ";file://" + plan in col, col[-3000:]
    print("ok: the chips' OSC 8 are https:// and file://")
    # a plain click on the page's chip: bise opens its url
    x, y = at(sc, "↗ artifacts mock")
    click(t, x + 4, y)
    wait_until(lambda: os.path.exists(log) and "https://bise.dev/m/artifacts" in open(log).read(), 10,
               lambda: "the click opens the page: %r" % (open(log).read() if os.path.exists(log) else ""))
    print("ok: a plain click opens the page")


def main():
    nbsp_as_spaces(Tui)
    E = e2e.Env()
    env, log = fakes(E)
    with tui_session(150, 42, env + " BISE_OPTION_DIGITS=1", E=E) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        keys(t)
        chips(t, E, log)
    print("PASS tui iterm: ⌥0-9 as iTerm2 sends them, artifact chips' links")


if __name__ == "__main__":
    run(main)
