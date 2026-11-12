#!/usr/bin/env python3
"""The simulated fill (mac-budget, 2026-10-21): heavy-run keeps the floor with
15 agents building at once. Not in the gate (~3 min, writes ~20 GB to a
throwaway disk image); run it by hand, under heavy-run --low:

  ~/.bise/run/heavy-run --low python3 scripts/disk_fill_sim.py [--agents 15] [--gb 12]

A real APFS volume of --gb GB (a sparse disk image in $TMPDIR, detached and
deleted at the end) holds 15 fake agent worktrees. Each agent runs 2 heavy
jobs through scripts/heavy-run: a cold build writes 1 GB into its cargo
target (cargo's CACHEDIR.TAG, the build lock held while it writes), then a
rebuild writes 0.5 GB more. A non-heavy writer (like the hub's /tmp
backups) adds 100 MB every 2 s up to 1.5 GB, beyond heavy-run's reach. The
real disk_reclaim.py frees the targets. The scale: floor 3 GB on 12 (15 on
the real 460), stop under 1, each job reserves 2 GB.
Pass: the volume's free space never goes under the floor minus what the
non-heavy writer wrote in one reclaim round (1 GB), every job ends rc=0
(none stopped for disk), and the minimum free space is printed."""
import argparse
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
HEAVY = os.path.join(HERE, "heavy-run")
RECLAIM = os.path.join(HERE, "disk_reclaim.py")
GB = 1 << 30

BUILD = r'''
import fcntl, os, sys, time
t, gb = sys.argv[1], float(sys.argv[2])
os.makedirs(os.path.join(t, "debug", "deps"), exist_ok=True)
open(os.path.join(t, "CACHEDIR.TAG"), "w").write("Signature: 8a477f597d28d172789f06886806bc55\n")
lk = open(os.path.join(t, "debug", ".cargo-lock"), "a")
fcntl.flock(lk, fcntl.LOCK_EX)
chunk = os.urandom(1 << 20) * 64
with open(os.path.join(t, "debug", "deps", "lib%d.rlib" % time.time_ns()), "wb") as fh:
    for _ in range(int(gb * 16)):
        fh.write(chunk); fh.flush(); time.sleep(0.15)
'''


def free_gb(vol):
    s = os.statvfs(vol)
    return s.f_bavail * s.f_frsize / GB


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--agents", type=int, default=15)
    ap.add_argument("--gb", type=int, default=12)
    a = ap.parse_args()
    tmp = tempfile.mkdtemp(prefix="fillsim-", dir=os.environ.get("TMPDIR"))
    img, vol = os.path.join(tmp, "vol.sparseimage"), os.path.join(tmp, "vol")
    subprocess.run(["hdiutil", "create", "-size", "%dg" % a.gb, "-fs", "APFS", "-type", "SPARSE",
                    "-volname", "fillsim", img], check=True, capture_output=True)
    subprocess.run(["hdiutil", "attach", "-nobrowse", "-mountpoint", vol, img], check=True, capture_output=True)
    try:
        return run(a, tmp, vol)
    finally:
        subprocess.run(["hdiutil", "detach", "-force", vol], capture_output=True)
        shutil.rmtree(tmp, ignore_errors=True)


def run(a, tmp, vol):
    home = os.path.join(tmp, "home")
    os.makedirs(os.path.join(home, ".bise", "run"))
    wts = [os.path.join(vol, "wt", "agent%02d" % i) for i in range(a.agents)]
    for w in wts:
        os.makedirs(os.path.join(w, "harness", "rust"))
    floor, stop = 3, 1
    env = {k: v for k, v in os.environ.items() if not k.startswith(("HEAVY_", "SB_AGENT"))}
    env.update(HOME=home, HEAVY_DISK_VOLUME=vol, HEAVY_DISK_WAIT_GB=str(floor), HEAVY_DISK_STOP_GB=str(stop),
               HEAVY_RESERVE_GB="2", HEAVY_DISK_POLL="1", HEAVY_RECLAIM_GAP="3", HEAVY_START_GAP="0",
               HEAVY_LOAD_CEILING="9999", HEAVY_ON_BATTERY="0", HEAVY_NO_LANE="1",
               HEAVY_RECLAIM="%s %s" % (sys.executable, RECLAIM),
               HEAVY_RECLAIM_ROOTS=":".join(wts), HEAVY_MIN_IDLE_MIN="0.1", HEAVY_IDLE_TARGET_H="1")
    start_free = free_gb(vol)
    low = [start_free]
    done = threading.Event()

    def sample():
        while not done.is_set():
            low.append(free_gb(vol))
            time.sleep(0.25)

    def other():
        os.makedirs(os.path.join(vol, "other"))
        for i in range(15):
            if done.is_set():
                return
            with open(os.path.join(vol, "other", "b%d" % i), "wb") as fh:
                fh.write(os.urandom(1 << 20) * 100)
            time.sleep(2)

    rcs = []

    def agent(w):
        t = os.path.join(w, "harness", "rust", "target")
        for gb in (1, 0.5):
            p = subprocess.run([HEAVY, sys.executable, "-c", BUILD, t, str(gb)], env=env, cwd=w,
                               capture_output=True, text=True)
            rcs.append((os.path.basename(w), gb, p.returncode, p.stderr.strip().splitlines()[-1:] if p.returncode else ""))
            time.sleep(1)

    th = [threading.Thread(target=sample), threading.Thread(target=other)] + \
         [threading.Thread(target=agent, args=(w,)) for w in wts]
    t0 = time.time()
    for x in th:
        x.start()
    for x in th[2:]:
        x.join()
    done.set()
    for x in th[:2]:
        x.join()
    log_path = os.path.join(home, ".bise", "run", "reclaim.log")
    log = open(log_path).read().splitlines() if os.path.exists(log_path) else []
    bad = [r for r in rcs if r[2] != 0]
    written = a.agents * 1.5 + 1.5
    print("volume %.1f GB free at start, floor %d GB, %d agents, %.1f GB written (%.1f x the free space)"
          % (start_free, floor, a.agents, written, written / start_free))
    print("min free %.2f GB; %d jobs, %d not rc=0 %s; %d targets reclaimed; %.0f s"
          % (min(low), len(rcs), len(bad), bad[:3], len(log), time.time() - t0))
    ok = min(low) >= floor - 1 and not bad and len(rcs) == 2 * a.agents
    print("PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
