"""An agent waiting on an event, in the TUI (wake-ui, designer's page
event-wake), through the real binaries: a throwaway hub on the fake
provider, tmux at 150 and 80 columns, dark and light.

  perf's brief backgrounds `cargo test -p storefront --release` (a fake
      cargo that sleeps, then prints 3 lines; BEND_BG_AFTER 2 s) and its
      turn ends: its panel row says `…` with its wait in the time column,
      never `○`; its view's top edge and the line under its thread say
      `… waiting for cargo test · <age>`
  the command ends: the live line goes, its thread says `· cargo test
      ended · rc 0 · after …  ▸ its last 3 lines`, never bise's message
      (`background 0 ended`); ctrl+o opens the lines
  `sb wake --on-file` with a note: `… waiting for the reindex`; a rc file
      with 101: `the reindex appeared · rc 101` with the rc in red
  `sb wake --stop`: `– stopped waiting for the reindex`, the row `○` again
  the same at 80 columns and in the light palette

WAKE_SHOTS=<dir> keeps the captures (.txt and .ansi) for the designer.

python3 -u tests/tui_wake_tmux.py
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, in_view, MAIN_IDLE  # noqa: E402

SHOTS = os.environ.get("WAKE_SHOTS")
SECS = int(os.environ.get("WAKE_TUI_SECS", "25"))
WAITING = re.compile(r"… waiting for cargo test · \d+[sm]")


def shot(t, name):
    if SHOTS:
        os.makedirs(SHOTS, exist_ok=True)
        with open(os.path.join(SHOTS, name + ".ansi"), "w") as f:
            f.write(t.screen(colors=True))
        with open(os.path.join(SHOTS, name + ".txt"), "w") as f:
            f.write(t.screen())


def sb(E, agent, *args):
    env = {**E.env, "SB_SOCKET": os.path.join(E.state, "agent.sock"), "SB_AGENT": agent}
    r = subprocess.run([e2e.EXE, "sb", *args], env=env, cwd=E.ws, capture_output=True, text=True, timeout=60)
    assert r.returncode == 0, "sb %s: %s %s" % (" ".join(args), r.stdout, r.stderr)
    return r.stdout.strip()


def fake_cargo(E):
    """A `cargo` that sleeps, then prints 3 lines (the real one never runs)."""
    d = os.path.join(E.tmp, "bin")
    os.makedirs(d, exist_ok=True)
    p = os.path.join(d, "cargo")
    with open(p, "w") as f:
        f.write("#!/bin/sh\nsleep %d\necho 'running 212 tests'\necho 'test result: ok. 212 passed'\necho 'finished'\n" % SECS)
    os.chmod(p, 0o755)
    return d


def view(t, name):
    t.keys("C-u")
    t.typed("/switch " + name)
    t.keys("Enter")
    t.wait("you → find an agent")
    t.keys("Enter")
    t.wait_re(in_view(name), 20)


def row(sc, name):
    """The panel's row of `name` (its glyph and columns)."""
    return next((l for l in sc.splitlines() if re.search(r"\b\d+ \S+ %s\b" % re.escape(name), l)), "")


def watch_id(E, note):
    out = sb(E, "perf", "wake")
    m = re.search(r"#(\d+) .*%s" % re.escape(note), out)
    assert m, out
    return m.group(1)


def wide(t, E, bin_dir):
    t.wait("bise :*")
    t.wait_re(MAIN_IDLE)
    brief = "the cache fix is in [[bash: PATH=%s:$PATH cargo test -p storefront --release]]" % bin_dir
    sb(E, "main", "spawn", "perf", "--objective", brief)
    t.wait("perf")
    # its turn ended, the command runs: `…`, the wait, never `○`
    sc = t.wait_re(r"\b\d+ … perf\b", 90)
    assert "○ perf" not in sc, sc
    view(t, "perf")
    sc = t.wait_re(WAITING.pattern, 30)
    # the top edge and the line under the thread
    assert len(WAITING.findall(sc)) >= 2, sc
    shot(t, "150-waiting")
    # the command ends: the hub's line, the live line gone
    sc = t.wait("cargo test ended · rc 0 · after", SECS + 60)
    assert "its last 3 lines" in sc, sc
    assert "background 0 ended" not in sc, "bise's wake folds into the line: " + sc
    t.wait_gone("waiting for cargo test", 20)
    shot(t, "150-ended")
    # a watch with a note, then its rc file with 101: red
    rc = os.path.join(E.tmp, "reindex.rc")
    sb(E, "perf", "wake", "--on-file", rc, "--note", "the reindex")
    sc = t.wait("… waiting for the reindex", 30)
    assert "… perf" in row(sc, "perf"), row(sc, "perf")
    shot(t, "150-waiting-note")
    with open(rc, "w") as f:
        f.write("101\n")
    t.wait("the reindex appeared · rc 101", 30)
    ansi = t.screen(colors=True)
    assert re.search(r"\x1b\[[0-9;]*m\s*rc 101", ansi), "the rc in its own color"
    shot(t, "150-failed")
    # stopped by the agent: faint, the row back to ○
    sb(E, "perf", "wake", "--on-file", rc + ".2", "--note", "the reindex")
    t.wait("… waiting for the reindex", 30)
    sb(E, "perf", "wake", "--stop", watch_id(E, "the reindex"))
    sc = t.wait("– stopped waiting for the reindex", 30)
    t.wait_gone("… waiting for the reindex", 20)
    t.wait_re(r"\b\d+ ○ perf\b", 20)
    shot(t, "150-stopped")


def narrow(t, E):
    t.start(80, 30)
    t.wait("bise :*")
    view(t, "perf")
    t.wait("cargo test ended · rc 0")
    sb(E, "perf", "wake", "--on-file", os.path.join(E.tmp, "x.rc"), "--note", "the reindex")
    sc = t.wait("… waiting for the reindex", 30)
    for line in sc.splitlines():
        assert len(line) <= 80, line
    shot(t, "80-waiting")
    t.typed("/theme light")
    t.keys("Enter")
    t.wait("… waiting for the reindex")
    shot(t, "80-waiting-light")
    t.typed("/theme dark")
    t.keys("Enter")


def main():
    E = e2e.Env()
    E.env["BEND_BG_AFTER"] = "2"
    bin_dir = fake_cargo(E)
    with tui_session(150, 40, E=E) as t:
        wide(t, E, bin_dir)
        print("PASS tui wake 150")
        narrow(t, E)
        print("PASS tui wake 80")


if __name__ == "__main__":
    run(main)
