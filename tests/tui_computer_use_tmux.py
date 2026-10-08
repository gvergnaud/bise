"""Computer use in a real terminal (tmux; docs/computer-use-design.md §8,
designer m_3551, m_3554, m_3896), against a fake `bise computer-use`
(BISE_COMPUTER_USE: it answers setup-check from a file the test writes
and logs every call) and a fake C6 state.json / events.jsonl in a temp
BEND_RUN_DIR. Never the user's browser or apps: the fake gets the
`open` calls too.

- /computer-use: chrome isn't open → the extension's load-unpacked steps
  (⏎ opens chrome://extensions, copies the folder: the flash) → the live
  test runs by itself (∿), then ✓ ready; with the helper, the "for apps"
  rows: ⏎ asks Accessibility, the cursor moves on when it turns ✓,
  Screen Recording's relaunch shows its wait until the reopened helper
  says yes; an agent driving: its line and x stop all.
- the marks: `↖` in main's row, `· ↖ Chrome` in its divider; paused:
  `? you took the wheel · ⏎ give it back`, ⏎ resumes it; /stop main
  stops it; the hub's line in main's feed from events.jsonl.

SB_DUMP=<dir>: every screen checked is written there (designer's review).

python3 -u tests/tui_computer_use_tmux.py
"""
import json
import os
import shutil
import socket
import sys
import tempfile
import threading

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from tui_tmux import tui_session, run  # noqa: E402

COLS, ROWS = 120, 40

FAKE = r'''#!/usr/bin/env python3
import json, os, sys, time
d = os.environ["CU_FAKE_DIR"]
with open(os.path.join(d, "calls.log"), "a") as f:
    f.write(" ".join(sys.argv[1:]) + "\n")
cmd = sys.argv[1] if len(sys.argv) > 1 else ""
if cmd == "setup-check":
    print(open(os.path.join(d, "check.json")).read())
elif cmd == "live-test":
    time.sleep(3)
    print(json.dumps({"ok": True}))
elif cmd == "request":
    print(json.dumps({"what": sys.argv[2], "relaunching": sys.argv[2] == "screen_recording"}))
else:
    print("{}")
'''


def row(id, state, detail="", fix=None):
    return {"id": id, "state": state, "detail": detail, "fix": fix}


NO_HELPER = [row("accessibility", "not_yet", "", "install_helper"), row("screen_recording", "not_yet", "", "install_helper")]


def main():
    d = tempfile.mkdtemp(prefix="cu-", dir=e2e.short_tmp())
    run_dir = os.path.join(d, "run")
    cu = os.path.join(run_dir, "computer-use")
    os.makedirs(cu)
    fake = os.path.join(d, "fake-cu")
    with open(fake, "w") as f:
        f.write(FAKE)
    os.chmod(fake, 0o755)
    ext = os.path.join(d, "dev", "try", "computer-use", "extension")
    clip = os.path.join(d, "clipboard")

    def check(rows, browser="Chrome"):
        v = {"browser": browser, "min_major": 116, "extension": {"id": "bogffepmbkbmbfejcadaipgphgkocgob", "dir": ext},
             "browsers": [{"key": "chrome", "name": "Chrome", "app": "/Applications/Google Chrome.app"}], "rows": rows}
        tmp = os.path.join(d, "check.json.tmp")
        with open(tmp, "w") as f:
            json.dump(v, f)
        os.replace(tmp, os.path.join(d, "check.json"))

    def state(agents):
        tmp = os.path.join(cu, "state.json.tmp")
        with open(tmp, "w") as f:
            json.dump({"agents": agents, "browsers": [], "apps": {}}, f)
        os.replace(tmp, os.path.join(cu, "state.json"))

    def calls():
        try:
            return open(os.path.join(d, "calls.log")).read()
        except FileNotFoundError:
            return ""

    def called(what, timeout=10):
        wait.until(lambda: what in calls(), timeout, lambda: "a call %r in\n%s" % (what, calls()))

    dump = os.environ.get("SB_DUMP")
    n = [0]

    def shot(t, name, sc):
        if dump:
            os.makedirs(dump, exist_ok=True)
            n[0] += 1
            with open(os.path.join(dump, "%02d-%s.txt" % (n[0], name)), "w") as f:
                f.write(sc)
            with open(os.path.join(dump, "%02d-%s.ansi" % (n[0], name)), "w") as f:
                f.write(t.screen(colors=True))

    check([row("browser", "waits", "Chrome isn't open", "open_browser"), row("extension", "not_yet"), row("live_test", "not_yet")] + NO_HELPER)
    state({})
    # computer use's commands socket (docs/issues/18): /stop stops by the
    # agent's key there (bise_computer_use::cli::stop_agent), not through
    # the fake binary; a fake broker logs each command as 'ctl <op> <key>'
    ctl = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    ctl.bind(os.path.join(run_dir, "computer-use-ctl.sock"))
    ctl.listen(8)

    def ctl_serve(conn):
        f = conn.makefile("rwb")
        for line in f:
            try:
                m = json.loads(line)
            except ValueError:
                continue
            if "id" not in m:
                continue
            agent = (m.get("args") or {}).get("agent", "")
            with open(os.path.join(d, "calls.log"), "a") as log:
                log.write("ctl %s %s\n" % (m.get("op"), agent))
            res = {"agents": {}} if m.get("op") == "status" else {"stopped": [agent]}
            f.write((json.dumps({"id": m["id"], "ok": True, "result": res}) + "\n").encode())
            f.flush()

    def ctl_accept():
        while True:
            try:
                c, _ = ctl.accept()
            except OSError:
                return
            threading.Thread(target=ctl_serve, args=(c,), daemon=True).start()

    threading.Thread(target=ctl_accept, daemon=True).start()
    # BISE_EXPORTS_FOR empty: an inherited stamp (the agent's shell, tmux's
    # server) would make bise_home drop BEND_RUN_DIR as stale
    env = "BISE_EXPORTS_FOR= BISE_COMPUTER_USE=%s CU_FAKE_DIR=%s BEND_CLIPBOARD_FILE=%s" % (fake, d, clip)
    # the TUI and the hub: this run dir, never the user's (E.env's
    # BEND_ vars come after extra_env on the TUI's command line)
    E = e2e.Env()
    E.env["BEND_RUN_DIR"] = run_dir
    try:
        with tui_session(COLS, ROWS, env=env, E=E) as t:
            t.wait("bise :*")
            # ---- the setup screen ----
            t.typed("/computer-use")
            t.keys("Enter")
            sc = t.wait("Chrome isn't open. open it, i'll wait")
            assert "computer use · agents can drive Chrome" in sc, sc
            assert "? Chrome" in sc and "· Chrome extension    after Chrome" in sc and "· live test           last" in sc, sc
            assert "⏎ open Chrome   ↑↓ step   esc later" in sc, sc
            assert "for apps" not in sc and "accessibility" not in sc, sc
            assert "in yolo, agents act without asking, purchases included. ⇧⇥ for auto." in sc, sc
            assert "what the agents see goes only to their model:" in sc, sc
            shot(t, "chrome-not-open", sc)
            t.keys("Enter")
            called("open -a /Applications/Google Chrome.app")
            # Chrome open: the extension's steps
            check([row("browser", "done", "Chrome 154"), row("extension", "waits", "add bise to Chrome", "add_extension"), row("live_test", "not_yet")] + NO_HELPER)
            sc = t.wait("3  i'll see it here, no key needed")
            assert "✓ Chrome              Chrome 154" in sc and "? Chrome extension    add bise to Chrome" in sc, sc
            assert "1  ⏎ opens chrome://extensions · turn on Developer mode, top right" in sc, sc
            assert "⏎ open chrome://extensions" in sc, sc
            shot(t, "extension-steps", sc)
            t.keys("Enter")
            sc = t.wait("opened · path copied")
            called("open -a /Applications/Google Chrome.app chrome://extensions")
            assert open(clip).read() == ext
            shot(t, "extension-flash", sc)
            # the extension connects: the live test runs by itself
            check([row("browser", "done", "Chrome 154"), row("extension", "done", "v0.1.0"), row("live_test", "waits", "last", "run_live_test")] + NO_HELPER)
            sc = t.wait("∿ live test           opening a tab in the background…")
            called("live-test --json")
            # the extension is in: the step-2 flash is gone, the key bar back
            sc = t.wait("↑↓ step   esc later")
            assert "path copied" not in sc, sc
            shot(t, "live-test-running", sc)
            check([row("browser", "done", "Chrome 154"), row("extension", "done", "v0.1.0"), row("live_test", "done", "ready")] + NO_HELPER)
            sc = t.wait("✓ Chrome is ready. ask any agent to use it.", timeout=15)
            assert "✓ live test           opened a tab, clicked a button. all set" in sc, sc
            assert "checks it again" not in sc, sc
            shot(t, "ready", sc)
            # the live test failed: try again (contractions)
            check([row("browser", "done", "Chrome 154"), row("extension", "done", "v0.1.0"), row("live_test", "failed", "the click did not land", "run_live_test")] + NO_HELPER)
            sc = t.wait("✗ live test           the click didn't land")
            assert "⏎ try again" in sc, sc
            shot(t, "live-test-failed", sc)
            # the helper installed: the apps rows after the live test
            web = [row("browser", "done", "Chrome 154"), row("extension", "done", "v0.1.0"), row("live_test", "done", "ready")]
            check(web + [row("accessibility", "waits", "", "request_accessibility"), row("screen_recording", "waits", "", "request_screen_recording")])
            sc = t.wait("for apps")
            assert "agents can drive your apps and Chrome" in sc, sc
            t.wait("turn on bise Computer Use in the list. i'll see it")
            sc = t.wait("⏎ open System Settings")
            assert "? accessibility       bise can click and type in apps" in sc, sc
            shot(t, "apps-accessibility", sc)
            t.keys("Enter")
            called("request accessibility")
            check(web + [row("accessibility", "done"), row("screen_recording", "waits", "", "request_screen_recording")])
            sc = t.wait("macOS reopens it, i'll wait")
            assert "✓ accessibility       bise can click and type in apps" in sc, sc
            shot(t, "apps-screen-recording", sc)
            t.keys("Enter")
            called("request screen_recording")
            check(web + [row("accessibility", "done"), row("screen_recording", "checking")])
            sc = t.wait("∿ screen recording    bise Computer Use reopens…")
            shot(t, "apps-reopening", sc)
            check(web + [row("accessibility", "done"), row("screen_recording", "done")])
            sc = t.wait("✓ screen recording    bise can see app windows")
            sc = t.wait("✓ ready. ask any agent to use Chrome or an app.")
            shot(t, "apps-done", sc)
            # an agent drives: its line, x stops them all
            state({"main": {"driving": "Chrome", "where": "amazon.fr", "since_ms": 1, "paused": False, "stopped": False}})
            sc = t.wait("↖ main drives Chrome")
            assert "x stop all" in sc, sc
            shot(t, "stop-all", sc)
            t.typed("x")
            called("stop --all")
            t.keys("Escape")
            t.wait_gone("computer use · agents can drive")
            # ---- the marks ----
            sc = t.wait(" · ↖ Chrome")
            div = [l for l in sc.split("\n") if "you → main" in l][-1]
            assert div.rstrip().endswith("↖ Chrome ─────────────────────────────────────────────────────────┤") or "↖ Chrome" in div, div
            mrow = [l for l in sc.split("\n") if " main " in l and ":*" in l and "│" in l]
            assert any(l.rstrip(" │").endswith("↖") for l in mrow), mrow
            shot(t, "driving-marks", sc)
            # paused: the divider asks, ⏎ gives it back
            state({"main": {"driving": "Chrome", "where": "amazon.fr", "since_ms": 1, "paused": True, "stopped": False}})
            sc = t.wait("? you took the wheel · ⏎ give it back")
            shot(t, "paused", sc)
            t.keys("Enter")
            called("resume main")
            # /stop main: stop, then the hub's line in main's feed
            state({"main": {"driving": "Chrome", "where": "amazon.fr", "since_ms": 1, "paused": False, "stopped": False}})
            t.wait_gone("you took the wheel")
            t.typed("/stop main")
            t.keys("Enter")
            # by main's key (<this hub's tag id>.main) on the commands socket
            called("ctl stop ")
            assert any(l.startswith("ctl stop ") and l.endswith(".main") and len(l.split(" ")[2].split(".")[0]) == 16 for l in calls().splitlines()), calls()
            with open(os.path.join(cu, "events.jsonl"), "a") as f:
                f.write(json.dumps({"t": 1, "agent": "main", "event": "stopped", "by": "you", "driving": "Chrome"}) + "\n")
            state({"main": {"driving": None, "where": None, "paused": False, "stopped": True}})
            sc = t.wait("↖ main stopped driving Chrome · you stopped it", timeout=15)
            assert " · ↖ Chrome" not in sc, sc
            # one line only (m_3904): no echo of /stop, no "no turn to interrupt"
            assert "stopped main:" not in sc and "no turn in progress" not in sc, sc
            shot(t, "stopped-feed-line", sc)
    finally:
        ctl.close()
        shutil.rmtree(d, ignore_errors=True)


if __name__ == "__main__":
    run(main)
