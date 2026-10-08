"""/restart outside bise's own source tree is a reload (BISE-131), like
VS Code's "Reload Window": in a real terminal (tmux) against the fake
provider, the hub, the agent's REPL and the TUI all restart on the same
version, nothing built, and nothing is lost: the draft stays in the
composer, the agent's session (its history) goes on.

python3 -u tests/tui_reload_tmux.py
"""
import glob
import json
import os
import signal
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, wait_until, pane_rows, MAIN_IDLE  # noqa: E402


def composer(sc):
    return " ".join(r.strip() for r in pane_rows(sc.splitlines()))


def main():
    E = e2e.Env()
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(bise)
    timing = os.path.join(E.tmp, "timing.log")
    E.env["SB_TIMING"] = timing

    def tui_starts():
        try:
            return sum(1 for l in open(timing) if " tui start (connecting)" in l)
        except FileNotFoundError:
            return 0

    def repl_pid():
        f = glob.glob(os.path.join(E.state, "agents", "*", "repl.pid"))
        return open(f[0]).read().strip() if f else ""

    def stop_switcher():
        # the switcher watches the new hub for 2 min: stopped before the
        # test stops the hub, else it rolls back (starts a hub again)
        try:
            os.kill(int(open(os.path.join(E.state, "switch.pid")).read()), signal.SIGTERM)
        except (OSError, ValueError):
            pass

    with tui_session(150, 40, "BISE_HOME=%s" % bise, E=E) as t:
        try:
            t.wait("bise :*")
            t.wait_re(MAIN_IDLE)
            t.typed("the first prompt")
            t.keys("Enter")
            t.wait("the first prompt")
            t.wait_re(MAIN_IDLE)
            pid0 = wait_until(repl_pid, 10, "no repl.pid")
            t.typed("half a thought")
            t.wait("half a thought")
            assert tui_starts() == 1, tui_starts()
            # /restart from the hub's side (the composer keeps the draft)
            c = e2e.Client(os.path.join(E.state, "hub.sock"))
            c.restart()
            # the TUI re-executes itself, the REPL restarts on its session
            wait_until(lambda: tui_starts() >= 2, 30, lambda: "the TUI did not re-exec: %d starts" % tui_starts())
            wait_until(lambda: repl_pid() not in ("", pid0), 30, lambda: "the REPL was not relaunched: pid %s" % repl_pid())
            t.wait("reloading bise")
            sc = t.wait("half a thought")
            assert "half a thought" in composer(sc), sc
            # its history goes on: the next request carries the first prompt
            t.wait_re(MAIN_IDLE)
            n = len(E.fake_requests())
            t.keys("Enter")
            wait_until(lambda: len(E.fake_requests()) > n, 20, "no request after the reload")
            users = json.dumps(E.fake_requests()[-1].get("users"))
            assert "the first prompt" in users and "half a thought" in users, users[:2000]
            assert not glob.glob(os.path.join(bise, "versions", "*")), "nothing built"
            print("OK: /restart outside bise's sources reloads hub, REPL and TUI; draft and history kept")
        finally:
            stop_switcher()


if __name__ == "__main__":
    run(main)
