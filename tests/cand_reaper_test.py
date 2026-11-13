#!/usr/bin/env python3
"""dev/plugins/ambient-qa/as-he-does/reaper.py (app-quits), on fake data only.

The matcher's rows (pure): a process that runs from the candidate's bundle is the app's, one that only
names it is not; one that names the world's w/ folder is the world's; his real app, a sibling world
(real-shape-22 next to real-shape-2), a file beside w/ and the reaper itself never are.
The watch, on fake processes the test starts under a tmp dir (never his home): a stand-in cand.sh
(a sleep) is killed with SIGKILL; the reaper, armed on it, stops the fake candidate (a copy of sleep
at '<tmp>/bise cand.app/Contents/MacOS/bise cand') and the fake world hub, KILLs the one that ignores
TERM after its grace, logs one line per process, and leaves his app's and the sibling world's
stand-ins running. And the ESRCH row: armed on a pid already gone, it stops them at once.

  python3 -B tests/cand_reaper_test.py
"""
import os
import shutil
import signal
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tests"))
sys.path.insert(0, os.path.join(ROOT, "dev", "plugins", "ambient-qa", "as-he-does"))
import reaper  # noqa: E402
import wait  # noqa: E402

REAPER = os.path.join(ROOT, "dev", "plugins", "ambient-qa", "as-he-does", "reaper.py")
FAILS = []


def check(ok, what):
    if not ok:
        FAILS.append(what)


def rows():
    app = "/x/desktop-cand/abc/dist/mac-arm64/bise cand.app"
    world = "/h/.bise/gate/real-shape-2"
    k = lambda cmd: reaper.kind(cmd, app, world)  # noqa: E731
    check(k(app + "/Contents/MacOS/bise cand --remote-debugging-port=9351") == "app", "the candidate's main is the app's")
    check(k(app + "/Contents/Resources/bin/ambient-core") == "app", "its core is the app's")
    check(k(app + "/") is None, "the bundle folder alone is not a process from it")
    check(k("/bin/bash -c cp '%s/Contents/MacOS/bise-mac-helper' /tmp/x" % app) is None,
          "a command that only names the bundle (the DMG's repack) is not the app's")
    check(k("/Users/g/.bise/dev/try-src/apps/desktop/dist/mac-arm64/bise try.app/Contents/MacOS/bise try") is None,
          "his real app is never matched")
    check(k(world + "/w/h/.local/share/bise/versions/root-1/bise sbd --workspace " + world + "/w/h/bise") == "world",
          "a world hub is the world's")
    check(k("/usr/bin/python3 fake.py " + world + "/w") == "world", "a command ending on <world>/w is the world's")
    check(k("/h/.bise/gate/real-shape-22/w/h/bise sbd") is None, "a sibling world (real-shape-22) is not this one's")
    check(k("/h/.bise/gate/real-shape/w/h/bise sbd") is None, "world 1 is not world 2's")
    check(k("tail -f " + world + "/world-build.log") is None, "a file beside w/ is not the world's")
    check(k("/Users/g/.local/share/bise/versions/root-9/bise sbd --workspace /Users/g/lab/x") is None,
          "his own hubs are never matched")
    snap = [(1, app + "/Contents/MacOS/bise cand"), (2, world + "/w/h/sbd"), (3, "/bin/zsh"), (4, world + "/w/x")]
    check([p for p, _, _ in reaper.pick(snap, app, world)] == [1, 2, 4], "pick takes both kinds")
    check([p for p, _, _ in reaper.pick(snap, app, world, "app")] == [1], "pick app")
    check([p for p, _, _ in reaper.pick(snap, app, world, "world", me={4})] == [2], "pick world, never itself")


def start(*cmd, as_exe=None):
    """A fake process, orphaned at once (launchd reaps it: no zombie of ours reads as alive). `as_exe`:
    the path ps shows as its executable (argv[0]: a copied sleep under a bundle is killed by macOS)."""
    script = '(exec -a "$0" "$@" >/dev/null 2>&1 </dev/null) & echo $!'
    argv0 = as_exe or cmd[0]
    out = subprocess.run(["/bin/bash", "-c", script, argv0] + list(cmd),
                         capture_output=True, text=True, check=True).stdout
    return int(out.strip())


def gone(pids):
    return all(not reaper.alive(p) for p in pids)


def live(tmp, armed_on_dead):
    app = os.path.join(tmp, "build", "bise cand.app")
    his = os.path.join(tmp, "his", "bise try.app")
    world = os.path.join(tmp, "gate", "real-shape-2")
    os.makedirs(os.path.join(world, "w", "h"))
    py = sys.executable
    cand = start("/bin/sleep", "300", as_exe=os.path.join(app, "Contents", "MacOS", "bise cand"))
    hub = start(py, "-c", "import time; time.sleep(300)", os.path.join(world, "w", "h", "sbd"))
    stubborn = start(py, "-c", "import signal, time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(300)",
                     os.path.join(world, "w", "h", "repl-live"))
    keep = [start("/bin/sleep", "300", as_exe=os.path.join(his, "Contents", "MacOS", "bise try")),
            start(py, "-c", "import time; time.sleep(300)", os.path.join(tmp, "gate", "real-shape-22", "w", "h", "sbd"))]
    doomed = [cand, hub, stubborn]
    log = os.path.join(tmp, "reaper.log")
    runner = subprocess.Popen(["/bin/sleep", "300"])
    if armed_on_dead:
        runner.kill()
        runner.wait()
    try:
        env = dict(os.environ, CAND_REAPER_GRACE="0.5")
        r = subprocess.run([py, "-B", REAPER, "watch", str(runner.pid), app, world, log], env=env,
                           stdout=subprocess.DEVNULL, timeout=30)
        check(r.returncode == 0, "watch returns 0 once armed")
        if not armed_on_dead:
            wait.holds(lambda: not gone(doomed), 0.5, "the candidate lives while cand.sh does")
            runner.send_signal(signal.SIGKILL)
            runner.wait()
        tag = "ESRCH" if armed_on_dead else "KILL"
        try:
            wait.until(lambda: gone(doomed), 20, "the candidate and its world stopped (%s)" % tag)
        except AssertionError as e:
            FAILS.append(str(e))
        check(not gone(keep[:1]) and not gone(keep[1:]), "his app and the sibling world left running (%s)" % tag)
        text = wait.until(lambda: open(log).read() if os.path.exists(log) and "SIGKILL" in open(log).read() else "",
                          20, "the reaper's log names the KILL")
        for p in doomed:
            check(("pid %d " % p) in text, "the log names pid %d (%s)" % (p, tag))
        check(("pid %d " % stubborn) in text and "SIGKILL" in text, "the one that ignores TERM gets KILL (%s)" % tag)
        check(all(("pid %d " % p) not in text for p in keep), "the log names none of the kept (%s)" % tag)
    finally:
        if runner.poll() is None:
            runner.kill()
            runner.wait()
        for p in doomed + keep:
            try:
                os.kill(p, signal.SIGKILL)
            except OSError:
                pass


def main():
    rows()
    for dead in (False, True):
        tmp = tempfile.mkdtemp(prefix="cand-reaper-", dir=os.environ.get("TMPDIR"))
        try:
            live(tmp, dead)
        finally:
            shutil.rmtree(tmp, ignore_errors=True)
    for f in FAILS:
        print("FAIL: " + f)
    print("cand_reaper_test: %s" % ("ok" if not FAILS else "%d failed" % len(FAILS)))
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
