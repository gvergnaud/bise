#!/usr/bin/env python3
"""A flaky test keeps its red (main m_56513): run_all's pool red, then green
alone, used to lose the red's log with run_all's tmp (ambient_projects_e2e's
five flakes of 10-27..29 left no line to read).

    flaky_keep.py <gate dir> <rev> <run_all out> <test>...

For each test: `<out>/<test>.pool.log` (its last KEEP_FILE_MAX bytes) and the
world's logs of `<out>/<test>.pool.keep` (e2e.keep_logs: KEEP_FILE_MAX a file,
KEEP_TOTAL_MAX in all) go to `<gate dir>/flaky-logs/<epoch>-<rev>-<test>/`; its
path is printed. A folder there older than KEEP_DAYS goes, as the gate's logs
(gate.sh: <gate dir>/logs, 7 days). Never fails the run: a copy that can't be
made is one line.
"""
import os
import shutil
import sys
import time

import e2e

KEEP_DAYS = 7


def prune(root, now, days=KEEP_DAYS):
    """Remove root's folders older than `days` (by mtime). Pure but for the rm."""
    for name in os.listdir(root):
        p = os.path.join(root, name)
        try:
            if now - os.path.getmtime(p) > days * 86400:
                shutil.rmtree(p, ignore_errors=True) if os.path.isdir(p) else os.unlink(p)
        except OSError:
            pass


def keep(gate, rev, out, test, now):
    """Copy one test's pool red; its folder."""
    dest = os.path.join(gate, "flaky-logs", "%d-%s-%s" % (now, rev, test))
    os.makedirs(dest, exist_ok=True)
    log = os.path.join(out, test + ".pool.log")
    if os.path.isfile(log):
        size = os.path.getsize(log)
        take = min(size, e2e.KEEP_FILE_MAX)
        with open(log, "rb") as r, open(os.path.join(dest, test + ".log"), "wb") as w:
            r.seek(size - take)
            w.write(r.read(take))
    world = os.path.join(out, test + ".pool.keep")
    if os.path.isdir(world):
        e2e.keep_logs(world, os.path.join(dest, "world"))
    return dest


def main(argv):
    if len(argv) < 4:
        print("usage: flaky_keep.py <gate dir> <rev> <run_all out> <test>...", file=sys.stderr)
        return 2
    gate, rev, out, tests = argv[0], argv[1], argv[2], argv[3:]
    now = int(time.time())
    try:
        os.makedirs(os.path.join(gate, "flaky-logs"), exist_ok=True)
        prune(os.path.join(gate, "flaky-logs"), now)
        for t in tests:
            print("FLAKY %s: its red kept in %s" % (t, keep(gate, rev, out, t, now)))
    except OSError as e:
        print("flaky_keep: the red's log not kept: %s" % e)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
