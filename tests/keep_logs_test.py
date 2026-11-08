#!/usr/bin/env python3
"""A red e2e test keeps its world's logs (review-gates m_38613, m_39468: two
reds this week, send_latency_e2e and land_every_e2e, lost their inner logs
with the test's tmp, so their failing line stayed unknown).

- e2e.keep_logs copies the logs of a world (any depth, newest first), each
  file's tail at most KEEP_FILE_MAX, KEEP_TOTAL_MAX in all, never
  node_modules, target or .git.
- A TERM (a gate's sweep, a stopped pool) unwinds like an exit once e2e's
  handler is set: the finally that keeps the logs runs, rc 143.
- run_all passes RUN_ALL_KEEP per test and names the folder on a FAIL line;
  gate.sh's gate_prune keeps it with the red run's logs.
"""
import os
import subprocess
import sys
import tempfile

import e2e
import wait

HERE = os.path.dirname(os.path.abspath(__file__))
fails = []


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what)
    if not ok:
        fails.append(what)


def write(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)


def main():
    tmp = tempfile.mkdtemp(prefix="keeplogs-")
    w = os.path.join(tmp, "world")
    write(os.path.join(w, "st", "hub.log"), b"hub line\n")
    write(os.path.join(w, "ws", "gate", "logs", "land-1-2.log"), b"x" * 100 + b"FAIL clippy\n")
    write(os.path.join(w, "big.out"), b"a" * (e2e.KEEP_FILE_MAX + 10) + b"END")
    write(os.path.join(w, "ws", "node_modules", "dep.log"), b"no")
    write(os.path.join(w, "ws", "target", "debug", "build.log"), b"no")
    write(os.path.join(w, "photo.png"), b"no")
    dest = os.path.join(tmp, "t.keep")
    n = e2e.keep_logs(w, dest)
    kept = sorted(os.path.relpath(os.path.join(d, f), dest) for d, _, fs in os.walk(dest) for f in fs)
    check(kept == ["big.out", "st/hub.log", "ws/gate/logs/land-1-2.log"], "the world's logs are kept, by path: %s" % kept)
    check(n == 3, "it says how many: %d" % n)
    big = open(os.path.join(dest, "big.out"), "rb").read()
    check(len(big) == e2e.KEEP_FILE_MAX and big.endswith(b"END"), "a big file: its last %d bytes" % e2e.KEEP_FILE_MAX)
    check(open(os.path.join(dest, "ws/gate/logs/land-1-2.log")).read().endswith("FAIL clippy\n"),
          "an inner gate's log names its line")
    # the total cap
    w2 = os.path.join(tmp, "w2")
    for i in range(12):
        write(os.path.join(w2, "f%d.log" % i), b"b" * e2e.KEEP_FILE_MAX)
    d2 = os.path.join(tmp, "t2.keep")
    e2e.keep_logs(w2, d2)
    total = sum(os.path.getsize(os.path.join(d2, f)) for f in os.listdir(d2))
    check(total <= e2e.KEEP_TOTAL_MAX, "at most %d bytes a test: %d" % (e2e.KEEP_TOTAL_MAX, total))
    # a TERM: the finally runs, the logs are kept, rc 143
    d3 = os.path.join(tmp, "t3.keep")
    ready = os.path.join(tmp, "ready")
    # the child waits for a signal (signal.pause), then the TERM under test
    prog = ("import os, signal, sys\nsys.path.insert(0, %r)\nimport e2e\n"
            "signal.signal(signal.SIGTERM, e2e._term_is_exit)\n"
            "try:\n    open(%r, 'w').close()\n    signal.pause()\n"
            "finally:\n    e2e.keep_logs(%r, %r)\n") % (HERE, ready, w, d3)
    p = subprocess.Popen([sys.executable, "-c", prog])
    wait.until(lambda: os.path.exists(ready), 20, "the child ready")
    p.terminate()
    rc = p.wait(timeout=20)
    check(rc == 143, "a TERM unwinds as exit 143: %r" % rc)
    check(os.path.exists(os.path.join(d3, "st", "hub.log")), "a TERM'd test still keeps its logs")
    # a second TERM while the finally blocks: the child dies (-15), as before
    ready2, inf = os.path.join(tmp, "ready2"), os.path.join(tmp, "in_finally")
    prog2 = ("import signal, sys\nsys.path.insert(0, %r)\nimport e2e\n"
             "signal.signal(signal.SIGTERM, e2e._term_is_exit)\n"
             "try:\n    open(%r, 'w').close()\n    signal.pause()\n"
             "finally:\n    open(%r, 'w').close()\n    signal.pause()\n") % (HERE, ready2, inf)
    p2 = subprocess.Popen([sys.executable, "-c", prog2])
    wait.until(lambda: os.path.exists(ready2), 20, "the second child ready")
    p2.terminate()
    wait.until(lambda: os.path.exists(inf), 20, "the second child in its finally")
    p2.terminate()
    rc2 = p2.wait(timeout=20)
    check(rc2 == -15, "a second TERM during a blocked finally kills it: %r" % rc2)
    # a flake keeps its red (tests/flaky_keep.py): the pool's log and world, capped, in
    # <gate dir>/flaky-logs/<epoch>-<rev>-<test>/; a folder there older than 7 days goes
    out, gdir = os.path.join(tmp, "out"), os.path.join(tmp, "gate")
    write(os.path.join(out, "amb_e2e.pool.log"), b"c" * (e2e.KEEP_FILE_MAX + 10) + b"FAIL the red line\n")
    write(os.path.join(out, "amb_e2e.log"), b"PASS the rerun\n")
    write(os.path.join(out, "amb_e2e.pool.keep", "st", "hub.log"), b"the red's hub line\n")
    old = os.path.join(gdir, "flaky-logs", "1-abc-old_e2e")
    write(os.path.join(old, "old_e2e.log"), b"old")
    os.utime(old, (1, 1))
    r = subprocess.run([sys.executable, "-B", os.path.join(HERE, "flaky_keep.py"), gdir, "r1", out, "amb_e2e"],
                       capture_output=True, text=True)
    kept = [d for d in os.listdir(os.path.join(gdir, "flaky-logs"))]
    check(r.returncode == 0 and len(kept) == 1 and kept[0].endswith("-r1-amb_e2e") and kept[0] in r.stdout,
          "flaky_keep: one folder <epoch>-<rev>-<test>, named on its line, the 7-day-old one gone: %s %r" % (kept, r.stdout))
    fk = os.path.join(gdir, "flaky-logs", kept[0]) if kept else tmp
    red = open(os.path.join(fk, "amb_e2e.log"), "rb").read() if os.path.exists(os.path.join(fk, "amb_e2e.log")) else b""
    check(len(red) == e2e.KEEP_FILE_MAX and red.endswith(b"FAIL the red line\n"),
          "flaky_keep: the pool's red log (not the green rerun's), its last %d bytes" % e2e.KEEP_FILE_MAX)
    check(os.path.exists(os.path.join(fk, "world", "st", "hub.log")), "flaky_keep: the red's world logs too")
    open(os.path.join(tmp, "f"), "w").close()
    r = subprocess.run([sys.executable, "-B", os.path.join(HERE, "flaky_keep.py"), os.path.join(tmp, "f"), "r1", out, "x"],
                       capture_output=True, text=True)
    check(r.returncode == 0 and "not kept" in r.stdout, "flaky_keep: a gate dir it can't write is one line, rc 0: %r" % r.stdout)
    # the wiring
    run_all = open(os.path.join(HERE, "run_all.sh")).read()
    check('mv "$out/$t.log" "$out/$t.pool.log"' in run_all and 'mv "$out/$t.keep" "$out/$t.pool.keep"' in run_all
          and 'python3 -B tests/flaky_keep.py "$(dirname "$flaky")" "$rev" "$out" $first' in run_all,
          "run_all: the pool's reds set aside before the rerun, a flake's kept by flaky_keep")
    gate = open(os.path.join(HERE, "gate.sh")).read()
    src = open(os.path.join(HERE, "e2e.py")).read()
    check('RUN_ALL_KEEP="$out/$1.keep"' in run_all and "its world's logs: $out/$1.keep" in run_all,
          "run_all: each test gets RUN_ALL_KEEP, a FAIL line names the folder")
    check("! -name '*.keep'" in gate, "gate.sh: a red run's prune keeps the .keep folders")
    check("keep_logs(self.tmp, keep)" in src and "signal.signal(signal.SIGTERM, _term_is_exit)" in src,
          "e2e.Env: a red keeps its world's logs, a TERM unwinds")
    subprocess.run(["rm", "-rf", tmp])
    if fails:
        print("keep_logs_test: FAIL (%d)" % len(fails))
        sys.exit(1)
    print("keep_logs_test: ok (a red e2e test keeps its world's logs)")


if __name__ == "__main__":
    main()
