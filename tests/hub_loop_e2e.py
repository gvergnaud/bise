#!/usr/bin/env python3
"""The hub's loop holds no agent's `sb` while it spawns, drops and wakes
(hub-loop, docs/issues/32).

On 2026-10-17 his harness hub dropped 48 agents' `sb` commands ("its sb
gave up before the hub took it: not run"), 25 of them during one Tick of
47 s spent in "wakes look", with 5-15 s spells on `sb drop` and `sb
spawn`: the loop ran slow shell work (git worktree add/remove, ps,
launchctl, file looks) inline.

A real hub, the scripted provider, a workspace of FILES files (a
worktree's checkout costs what his does, a little):
  1. 25 tasks, 30 live `sb wake` watches (20 pids, 10 files);
  2. a sampler: main's `sb list` every 200 ms;
  3. meanwhile a spawn and an /archive (each a worktree of FILES files;
     the dropped one with a rust/target of TARGET files: its rm runs off
     the loop, trash.rs), and a burst of every watch's event;
  4. every `sb list` answered in under LIMIT s, every watch fired once,
     and hub.log has no "gave up before the hub took it" and no
     "hub loop busy" (its split, crate::spent, is printed when there is).

Run: python3 -u tests/hub_loop_e2e.py   (HUB_LOOP_FILES=20000 for a heavier repo)
"""
import json
import os
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import Env, check  # noqa: E402

LIMIT = 1.0
TASKS = 25
PIDS = 20
FILES_W = 10
FILES = int(os.environ.get("HUB_LOOP_FILES", "8000"))
# the dropped worktree's rust/target, in files (HUB_LOOP_TARGET)
TARGET = int(os.environ.get("HUB_LOOP_TARGET", "30000"))


def sb(E, agent, *args, ok=True):
    env = {**E.env, "SB_SOCKET": os.path.join(E.state, "agent.sock"), "SB_AGENT": agent}
    r = subprocess.run([e2e.EXE, "sb", *args], env=env, cwd=E.ws, capture_output=True, text=True, timeout=120)
    if ok:
        assert r.returncode == 0, "sb %s: %s %s" % (" ".join(args), r.stdout, r.stderr)
    return r


def hub_log(E):
    try:
        return open(os.path.join(E.state, "hub.log")).read()
    except FileNotFoundError:
        return ""


def journal(E, kind):
    out = []
    for l in open(os.path.join(E.state, "journal.jsonl")):
        if '"%s"' % kind in l:
            out.append(json.loads(l))
    return out


def big_repo(ws, n):
    """`n` small files in 100 folders, one commit."""
    for i in range(n):
        d = os.path.join(ws, "src", "d%02d" % (i % 100))
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "f%05d.txt" % i), "w") as f:
            f.write("file %d\n" % i * 20)
    with open(os.path.join(ws, ".gitignore"), "a") as f:
        f.write("target/\n")
    e2e.sh(ws, "git add -A && git commit -qm files")


def big_target(path, n):
    """A cargo target of `n` files (ignored): what a drop's rm deletes."""
    for i in range(n):
        d = os.path.join(path, "rust", "target", "debug", "deps", "d%02d" % (i % 100))
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "o%05d.rlib" % i), "w") as f:
            f.write("x" * 4096)


def worktree_of(ws, branch):
    """The path of the worktree on `branch`, from git."""
    out = subprocess.run(["git", "worktree", "list", "--porcelain"], cwd=ws, capture_output=True, text=True).stdout
    for block in out.split("\n\n"):
        lines = dict(l.split(" ", 1) for l in block.splitlines() if " " in l)
        if lines.get("branch") == "refs/heads/" + branch:
            return lines["worktree"]
    raise AssertionError("no worktree on %s: %s" % (branch, out))


class Sampler(threading.Thread):
    """main's `sb list`, one every 200 ms (from the end of the last)."""

    def __init__(self, E):
        super().__init__(daemon=True)
        self.E, self.took, self.stop = E, [], threading.Event()

    def run(self):
        while not self.stop.is_set():
            t0 = time.time()
            r = sb(self.E, "main", "list", ok=False)
            self.took.append((t0, time.time() - t0, r.returncode))
            self.stop.wait(0.2)


def main():
    E = Env()
    sleeps = []
    sampler = None
    try:
        t0 = time.time()
        big_repo(E.ws, FILES)
        print("repo: %d files in %.1f s" % (FILES, time.time() - t0))
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        names = ["t%02d" % i for i in range(1, TASKS + 1)]
        # the last one in a worktree: the /archive below removes it
        for n in names:
            sb(E, "main", "spawn", n, "--objective", "stay around for the hub-loop test", *(["--worktree"] if n == names[-1] else []))
        for n in names:
            c.wait_status(n, "idle", 240)
        print("%d tasks idle after %.1f s" % (TASKS, time.time() - t0))

        # 30 live watches: 20 processes, 10 files, on the tasks in turn
        for i in range(PIDS):
            p = subprocess.Popen(["sleep", "600"])
            sleeps.append(p)
            sb(E, names[i % TASKS], "wake", "--on-exit", str(p.pid), "--note", "PID-%d" % i)
        files = [os.path.join(E.tmp, "rc-%d" % i) for i in range(FILES_W)]
        for i, f in enumerate(files):
            sb(E, names[(PIDS + i) % TASKS], "wake", "--on-file", f, "--note", "FILE-%d" % i)
        check(len(journal(E, "wake_set")) == PIDS + FILES_W, "%d watches set" % (PIDS + FILES_W))
        log0 = len(hub_log(E))

        sampler = Sampler(E)
        sampler.start()
        wait.until(lambda: len(sampler.took) >= 10, 30, "10 sb list before the spawn")
        # a spawn, a drop, then every watch's event at once
        ts = time.time()
        marks = {"spawn": round(ts - t0, 1)}
        sb(E, "main", "spawn", "t26", "--objective", "a late task", "--worktree")
        print("sb spawn answered in %.2f s" % (time.time() - ts))
        # his /archive (main's `sb drop` asks him first)
        dropped = names[-1]
        # its big rust/target (drop-offloop: its rm held his loop 110 s)
        wt = worktree_of(E.ws, "sb/" + dropped)
        tt = time.time()
        big_target(wt, TARGET)
        print("target: %d files in %.1f s" % (TARGET, time.time() - tt))
        ts = time.time()
        marks["archive"] = round(ts - t0, 1)
        c.say("/archive %s" % dropped)
        c.wait(lambda: c.agent(dropped)["status"] == "archived" or any(n["kind"] == "confirm" for n in c.notices()), 60,
               "%s archived or a confirmation" % dropped)
        conf = [n for n in c.notices() if n["kind"] == "confirm"]
        if conf:
            c.confirm(conf[-1]["id"], True)
        c.wait_status(dropped, "archived", 60)
        archive_took = time.time() - ts
        print("/archive done in %.2f s" % archive_took)
        # the removal runs off the loop (bdcd2b7dc): archived comes first
        c.wait(lambda: not os.path.exists(wt), 60, "the dropped worktree's path is free")
        wait.until(lambda: "removed off the loop in" in hub_log(E), 120, "the trash removed its folder")
        print("  " + [l for l in hub_log(E).splitlines() if "removed off the loop in" in l][-1])
        marks["burst"] = round(time.time() - t0, 1)
        for p in sleeps:
            p.kill()
            p.wait()  # a zombie is alive to kill(0)
        for f in files:
            open(f, "w").write("0\n")
        want = PIDS + FILES_W - sum(1 for i in range(PIDS + FILES_W) if names[i % TASKS] == dropped)
        wait.until(lambda: len([j for j in journal(E, "wake_end") if j.get("why") != "gone"]) >= want, 60,
                   "%d watches fired" % want)
        c.wait_status("t26", "idle", 120)
        n = len(sampler.took)
        wait.until(lambda: len(sampler.took) >= n + 10, 30, "10 sb list after the burst")
        sampler.stop.set()
        sampler.join(30)

        took = sorted(t for _, t, _ in sampler.took)
        slow = [(round(a - t0, 1), round(t, 2)) for a, t, _ in sampler.took if t >= LIMIT]
        p = lambda q: took[min(len(took) - 1, int(q * len(took)))]  # noqa: E731
        print("sb list: %d samples, median %.3f s, p95 %.3f s, worst %.3f s, %d over %.0f s %s"
              % (len(took), p(0.5), p(0.95), took[-1], len(slow), LIMIT, slow[:10]))
        print("events at (s):", marks)
        log = hub_log(E)[log0:]
        busy = [l for l in log.splitlines() if "hub loop busy" in l or "hub loop free" in l]
        for l in log.splitlines():
            if "hub loop " in l:
                print("  " + l)
        fired = [j for j in journal(E, "wake_end") if j.get("why") != "gone"]
        check(len({j["id"] for j in fired}) == len(fired), "each watch fired once")
        check(all(r == 0 for _, _, r in sampler.took), "every sb list answered")
        check(not slow, "every sb list under %.0f s (worst %.2f s)" % (LIMIT, took[-1]))
        check("gave up before the hub took it" not in log, "hub.log: no sb given up")
        check(not busy, "hub.log: the loop never held 5 s")
    finally:
        if sampler:
            sampler.stop.set()
        for p in sleeps:
            p.kill()
        E.close()
    print("OK hub_loop_e2e")


if __name__ == "__main__":
    main()
