#!/usr/bin/env python3
"""Give disk space back: delete the agents' idle cargo targets (mac-budget,
2026-10-21). heavy-run calls it; nobody else needs to.

  python3 scripts/disk_reclaim.py [--want <GB>] [--idle-h <h>] [--min-idle-min <m>] [--dry-run]
  python3 scripts/disk_reclaim.py --self-test

Why: each agent worktree holds a rust/target of 6-12 GB that nothing deletes;
on 2026-10-20 they and a cold rebuild took the data volume to 1.3 GB free,
every heavy job waited 12 h and main cleaned by hand. A target is a cache:
cargo rebuilds it (the seed clone and sccache make that cheaper).

What it deletes: a cargo target dir (a dir named `target` with cargo's
CACHEDIR.TAG) at most 4 levels under a worktree of ~/.bise/worktrees/<hub>/
<agent>/ (HEAVY_RECLAIM_ROOTS: a ':' list of roots instead, tests). Never the
user's own clones, never anything else.
  - every target unused for --idle-h hours (6) goes, whatever the free space;
  - while the data volume has under --want GB free, the least recently used
    targets unused for --min-idle-min minutes (30) go, oldest first, one at a
    time, until it has --want GB again.
Used = the newest mtime of the target and its profile dirs (debug, release,
their deps/.fingerprint/incremental/build): each build writes there.
Never a target in use (the hub's rule: no process uses it): one whose cargo
lock (<profile>/.cargo-lock) another process holds, one a running process
executes a file from (a test binary), or one in a worktree where a heavy-run
job runs (its cwd, ~/.bise/run/reserve). Never the gate seed
(~/.bise/cache/gate-seed) nor the user's clones: only worktree targets. The cargo locks are held while the dir
is renamed away, so a cargo that starts then waits and gets a fresh target.
The owner is told: the new empty target holds RECLAIMED.txt (what, when, why),
and heavy-run prints it at the owner's next job in that worktree (its build
is cold, that's why). Each deletion is a line in ~/.bise/run/reclaim.log:
<epoch> <path> <idle min> <GB freed> <reason>.
Never ~/.bise/dev/build (its target holds the try's own bise, which 'bise-ambient --real'
swaps in; keep-current m_30630 found it gone, removed by hand), whatever the roots say:
protected(), checked on each target's real path.
A target: a dir named target with cargo's CACHEDIR.TAG or a .rustc_info.json (a clone of the
gate seed had no tag: on 2026-11-08, 11 of 12 task targets were never seen). The roots
include the tip check's tree (~/.bise/run/tip-check) and .land-check's (a dot project).
The trim (disk-hygiene, architect m_55867), before any deletion: a target whose green gate
wrote <target>/.idle-since and that is idle --min-idle-min since is measured once: its own
bytes are the files newer than its .seeded (gate.sh's mark of a seed clone; without it, its
incremental dirs only). Over HEAVY_TRIM_GB (4): its incremental dirs go when that brings it
under, else it is replaced by a fresh clone of the seed (its worktree's `gate.sh seed-into`),
under its cargo locks; a reclaim.log line `trim-incremental|trim-reclone`. While the disk is
under --want, the idle targets' incremental dirs go first (`low-incremental`), whole targets
after. Once an hour each agent tmp's size goes to ~/.bise/run/tmp-sizes.log, and a tmp over
HEAVY_TMP_CAP_GB (2) is printed once (~/.bise/run/tmp-warned): measured a week before more.
One reclaim at a time (~/.bise/run/reclaim.lock): a second one returns at once.
HEAVY_FREE_KB_FILE (tests): the free KB are read from that file, not statvfs.
"""
import argparse
import fcntl
import os
import shutil
import subprocess
import sys
import time

GB = 1 << 30
PROFILE_SUBDIRS = ("deps", ".fingerprint", "incremental", "build")


# ---- pure part ------------------------------------------------------------

# never deleted, whatever the roots: the dev build dir (~/.bise/dev/build) and everything
# under it. HEAVY_RECLAIM_KEEP (tests): a ':' list instead.
def kept():
    k = os.environ.get("HEAVY_RECLAIM_KEEP")
    ks = [p for p in k.split(":") if p] if k is not None else [os.path.expanduser("~/.bise/dev/build")]
    return [os.path.realpath(p) for p in ks]


def protected(path, keep):
    """Whether `path` (resolved) is one of `keep` or under it. Pure but for realpath."""
    p = os.path.realpath(path)
    return any(p == k or p.startswith(k.rstrip(os.sep) + os.sep) for k in keep)


def plan(targets, free_gb, want_gb, now, idle_h, min_idle_min, strays=frozenset()):
    """The targets to delete, in order, with why.

    targets: [(path, last_used_epoch, busy)]. Returns [(path, reason)]:
    first the strays (a rust/target in a task worktree whose task builds in
    <task>/target) unused >= min_idle_min, then every idle one (unused >= idle_h), then, only while free_gb is
    under want_gb, the least recently used of the rest unused >= min_idle_min.
    The caller deletes them in that order and stops the 'low' ones as soon
    as the disk has want_gb free (it re-reads the free space after each)."""
    free = [(p, t) for p, t, busy in targets if not busy]
    # a second target of a task (strays): unused min_idle_min is enough
    out = [(p, "stray") for t, p in sorted((t, p) for p, t in free if p in strays and now - t >= min_idle_min * 60)]
    idle = sorted([(t, p) for p, t in free if p not in strays and now - t >= idle_h * 3600])
    out += [(p, "idle") for t, p in idle]
    if free_gb < want_gb:
        done = {p for t, p in idle} | {p for p, _ in out}
        lru = sorted([(t, p) for p, t in free if p not in done and now - t >= min_idle_min * 60])
        out += [(p, "low") for t, p in lru]
    return out


def trim_action(own_gb, inc_gb, trim_gb):
    """What the trim does to a target whose own bytes (written since its
    clone) are own_gb, inc_gb of them in incremental/: None under trim_gb,
    'incremental' when dropping those brings it under, else 'reclone' (a
    fresh clone of the seed)."""
    if own_gb <= trim_gb:
        return None
    return "incremental" if own_gb - inc_gb <= trim_gb else "reclone"


def trim_candidates(targets, now, min_idle_min):
    """targets: [(path, last_used, busy, idle_since)], idle_since the mtime of
    the .idle-since a green gate wrote (None: none). The ones to measure: a
    green gate marked them, nothing built since, idle min_idle_min."""
    return [p for p, used, busy, since in targets
            if since is not None and not busy and now - used >= min_idle_min * 60]


def tmp_over(sizes_gb, cap_gb, warned):
    """The agent tmps over cap_gb not warned yet. sizes_gb: {path: GB}."""
    return sorted(p for p, gb in sizes_gb.items() if gb > cap_gb and p not in warned)


def _self_test():
    now = 1_000_000
    t = [("/a", now - 7 * 3600, False), ("/b", now - 3600, False), ("/c", now - 120, False),
         ("/d", now - 9 * 3600, True), ("/e", now - 2 * 3600, False)]
    assert plan(t, 50, 25, now, 6, 30) == [("/a", "idle")], plan(t, 50, 25, now, 6, 30)
    assert plan(t, 10, 25, now, 6, 30) == [("/a", "idle"), ("/e", "low"), ("/b", "low")]
    assert plan(t, 10, 25, now, 6, 1) == [("/a", "idle"), ("/e", "low"), ("/b", "low"), ("/c", "low")]
    assert plan([], 1, 25, now, 6, 30) == []
    assert all(p != "/d" for p, _ in plan(t, 0, 99, now, 0, 0)), "a busy target is never planned"
    assert plan(t, 50, 25, now, 6, 30, {"/b", "/c", "/d"}) == [("/b", "stray"), ("/a", "idle")], "a stray goes after 30 min"
    assert plan(t, 10, 25, now, 6, 30, {"/b"}) == [("/b", "stray"), ("/a", "idle"), ("/e", "low")], "once"
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        build = os.path.join(d, "state", "build")
        os.makedirs(os.path.join(build, "target"))
        os.symlink(build, os.path.join(d, "build-link"))
        keep = [os.path.realpath(os.path.join(d, "build-link"))]
        assert protected(os.path.join(d, "build-link", "target"), keep), "the dev build target, by its link"
        assert protected(os.path.join(build, "target"), keep) and protected(build, keep)
        assert not protected(os.path.join(d, "state", "build2", "target"), keep), "a sibling with the same prefix"
        assert not protected(os.path.join(d, "wt", "rust", "target"), keep)
        # a target: cargo's tag, or a seed clone's .rustc_info.json (no tag); never another dir named target
        for name, mark in (("a", "CACHEDIR.TAG"), ("b", ".rustc_info.json"), ("c", "README")):
            os.makedirs(os.path.join(d, name, "target"))
            open(os.path.join(d, name, "target", mark), "w").close()
        assert is_target(os.path.join(d, "a", "target")) and is_target(os.path.join(d, "b", "target"))
        assert not is_target(os.path.join(d, "c", "target")) and not is_target(os.path.join(d, "b"))
        # a clone's own bytes: the files newer than its .seeded
        t = os.path.join(d, "b", "target")
        with open(os.path.join(t, "old"), "wb") as fh:
            fh.write(b"x" * 8192)
        os.utime(os.path.join(t, "old"), (now, now))
        with open(os.path.join(t, "new"), "wb") as fh:
            fh.write(b"y" * 8192)
        assert 8192 <= own_bytes(t, now + 1) < 16384 and own_bytes(t, None) >= 16384
    assert trim_action(3.9, 3, 4) is None
    assert trim_action(9, 6, 4) == "incremental", "dropping incremental/ brings it to 3 GB"
    assert trim_action(9, 2, 4) == "reclone"
    assert trim_action(4, 4, 4) is None, "at the cap: kept"
    m = [("/a", now - 3600, False, now - 3600), ("/b", now - 60, False, now - 60), ("/c", now - 3600, True, now - 3600),
         ("/d", now - 3600, False, None)]
    assert trim_candidates(m, now, 30) == ["/a"], "marked, idle 30 min, not busy"
    assert tmp_over({"/x": 7.0, "/y": 1.3, "/z": 2.5}, 2, {"/z"}) == ["/x"], "over the cap, once"
    print("disk_reclaim self-test ok")


# ---- the shell ------------------------------------------------------------

def free_kb():
    f = os.environ.get("HEAVY_FREE_KB_FILE")
    if f:
        try:
            return int(open(f).read().strip() or 0)
        except (OSError, ValueError):
            return 0
    v = os.environ.get("HEAVY_DISK_VOLUME") or "/System/Volumes/Data"
    if not os.path.isdir(v):
        v = os.path.expanduser("~")
    s = os.statvfs(v)
    return s.f_bavail * s.f_frsize // 1024


def roots():
    r = os.environ.get("HEAVY_RECLAIM_ROOTS")
    if r:
        return [p for p in r.split(":") if p]
    base = os.path.expanduser("~/.bise/worktrees")
    # the task folders (.land-check's too: a dot project), and the tip check's tree
    return [os.path.join(base, h, a) for h in _ls(base) for a in _ls(os.path.join(base, h))] \
        + [os.path.expanduser("~/.bise/run/tip-check")]


def stray(t):
    """A rust/target in a task worktree (<project>/<task>/<repo>/rust/target, <task>/owner there,
    not a dot project's): the task builds in <task>/target (the hub's CARGO_TARGET_DIR)."""
    rust = os.path.dirname(t)
    wt = os.path.dirname(rust)
    task = os.path.dirname(wt)
    return (os.path.basename(rust) == "rust" and os.path.isfile(os.path.join(task, "owner"))
            and os.path.exists(os.path.join(wt, ".git")) and not os.path.basename(os.path.dirname(task)).startswith("."))


def is_target(p):
    """A cargo target: a dir named target with cargo's CACHEDIR.TAG, or its
    .rustc_info.json (a clone of the gate seed had no tag before gate.sh's
    seed_mark: 11 of 12 task targets on 2026-11-08, never reclaimed)."""
    return os.path.basename(p) == "target" and any(
        os.path.exists(os.path.join(p, f)) for f in ("CACHEDIR.TAG", ".rustc_info.json"))


def _ls(d):
    try:
        return sorted(x for x in os.listdir(d) if os.path.isdir(os.path.join(d, x)))
    except OSError:
        return []


def find_targets(root, depth=4):
    """cargo target dirs under root (is_target), not descending into one."""
    out = []
    if depth < 0:
        return out
    for name in _ls(root):
        if name in ("node_modules", ".git") or name.startswith("target.reclaiming"):
            continue
        p = os.path.join(root, name)
        if os.path.islink(p):
            continue
        if is_target(p):
            out.append(p)
        elif depth > 0:
            out += find_targets(p, depth - 1)
    return out


def profiles(t):
    """The profile dirs of t (with a .cargo-lock), the gate's nested clippy target's too (<t>/clippy/debug)."""
    out = []
    for d in _ls_paths(t):
        if os.path.exists(os.path.join(d, ".cargo-lock")):
            out.append(d)
        else:
            out += [p for p in _ls_paths(d) if os.path.exists(os.path.join(p, ".cargo-lock"))]
    return out


def last_used(t):
    m = 0.0
    for d in [t] + _ls_paths(t):
        for x in [d] + [os.path.join(d, s) for s in PROFILE_SUBDIRS]:
            try:
                m = max(m, os.stat(x).st_mtime)
            except OSError:
                pass
    return m


def _ls_paths(t):
    return [os.path.join(t, d) for d in _ls(t)]


def running_exes():
    """The executables of the running processes (full paths as exec'd)."""
    try:
        out = subprocess.run(["ps", "-Ao", "comm="], capture_output=True, text=True, timeout=20).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    return [line.strip() for line in out.splitlines() if line.strip()]


def lock_all(t):
    """Take every cargo lock of t without waiting: the open fds, or None
    when a lock is held (cargo runs there)."""
    fds = []
    for p in profiles(t):
        try:
            fd = os.open(os.path.join(p, ".cargo-lock"), os.O_RDWR)
        except OSError:
            continue
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            os.close(fd)
            for f in fds:
                os.close(f)
            return None
        fds.append(fd)
    return fds


def job_cwds():
    """The cwds of the running heavy-run jobs (~/.bise/run/reserve/<slot>.<pid>:
    "<GB>\t<cwd>\t<free KB at its start>"), the dead ones' files skipped."""
    d = os.path.expanduser("~/.bise/run/reserve")
    out = []
    for f in _files(d):
        try:
            os.kill(int(f.rsplit(".", 1)[1]), 0)
            out.append(open(os.path.join(d, f)).read().split("\t")[1].strip())
        except (ValueError, IndexError, OSError):
            pass
    return out


def _files(d):
    try:
        return sorted(os.listdir(d))
    except OSError:
        return []


def busy(t, exes, root="", cwds=()):
    if exes is None:
        return True  # can't tell: hands off
    pre = t.rstrip("/") + "/"
    if any(e.startswith(pre) for e in exes):
        return True
    # a heavy job runs in that worktree (a gate, a build): hands off
    if root and any((c.rstrip("/") + "/").startswith(root.rstrip("/") + "/") for c in cwds):
        return True
    fds = lock_all(t)
    if fds is None:
        return True
    for f in fds:
        os.close(f)
    return False


def gone_dir(t):
    return "%s.reclaiming-%d-%d" % (t, int(time.time()), os.getpid())


def reclaim_one(t, reason, idle_min, log):
    fds = lock_all(t)
    if fds is None:
        return None
    before = free_kb()
    away = gone_dir(t)
    try:
        os.rename(t, away)
    except OSError:
        for f in fds:
            os.close(f)
        return None
    for f in fds:
        os.close(f)
    os.makedirs(t, exist_ok=True)
    shutil.rmtree(away, ignore_errors=True)
    gb = max(0, free_kb() - before) / (GB / 1024)
    stamp = time.strftime("%Y-%m-%d %H:%M")
    why = ("unused %.1f h" % (idle_min / 60.0)) + {"low": ", the disk was low",
                                                   "stray": ", a second target: the task builds in <task dir>/target"}.get(reason, "")
    with open(os.path.join(t, "RECLAIMED.txt"), "w") as fh:
        fh.write("heavy-run: this cargo target was deleted at %s by scripts/disk_reclaim.py (%s, %.1f GB freed): "
                 "its next build is cold.\n" % (stamp, why, gb))
    with open(log, "a") as fh:
        fh.write("%d\t%s\t%d\t%.1f\t%s\n" % (time.time(), t, idle_min, gb, reason))
    return gb


def incrementals(t):
    """cargo's incremental dirs of t: <profile>/incremental, the nested clippy target's too."""
    return [os.path.join(p, "incremental") for p in profiles(t) if os.path.isdir(os.path.join(p, "incremental"))]


def own_bytes(d, since):
    """The bytes of the files under d written after `since` (an epoch; None:
    all of them), symlinks skipped: a clone's own blocks, the ones it shares
    with the seed being older than its .seeded."""
    n = 0
    for dirpath, _, files in os.walk(d):
        for f in files:
            try:
                s = os.lstat(os.path.join(dirpath, f))
            except OSError:
                continue
            if not os.path.islink(os.path.join(dirpath, f)) and (since is None or s.st_mtime > since):
                n += s.st_blocks * 512
    return n


def mtime(p):
    try:
        return os.stat(p).st_mtime
    except OSError:
        return None


def seed_into_from(t):
    """The worktree whose tests/gate.sh clones the seed into t: <wt>/rust/target's
    <wt>, a task folder's <task>/target's <task>/<repo>; None when none."""
    parent = os.path.dirname(t)
    cands = [os.path.dirname(parent)] if os.path.basename(parent) == "rust" else _ls_paths(parent)
    for c in cands:
        if os.path.isfile(os.path.join(c, "tests", "gate.sh")) and os.path.exists(os.path.join(c, ".git")):
            return c
    return None


def trim_one(t, trim_gb, log):
    """Trim t (green-marked, idle): its own bytes measured, then nothing, its
    incremental dirs dropped, or the whole target replaced by a fresh clone
    of the seed (gate.sh seed-into), under its cargo locks. Without .seeded
    (a target from before seed_mark), only incremental/ counts as its own.
    Returns (action, GB freed) or None (busy, or under trim_gb)."""
    seeded = mtime(os.path.join(t, ".seeded"))
    incs = incrementals(t)
    inc = sum(own_bytes(i, seeded) for i in incs) / GB
    own = own_bytes(t, seeded) / GB if seeded is not None else inc
    try:
        os.remove(os.path.join(t, ".idle-since"))  # measured once per green gate
    except OSError:
        pass
    act = trim_action(own, inc, trim_gb)
    wt = seed_into_from(t) if act == "reclone" else None
    if act == "reclone" and wt is None:
        act = "incremental"  # no seed to clone from here: the incremental part only
    if act is None:
        return None
    fds = lock_all(t)
    if fds is None:
        return None
    before = free_kb()
    away = []
    try:
        for d in (incs if act == "incremental" else [t]):
            a = gone_dir(d)
            os.rename(d, a)
            away.append(a)
    except OSError:
        pass
    finally:
        for f in fds:
            os.close(f)
    if act == "reclone" and away:
        env = dict(os.environ, PATH=os.path.expanduser("~/.cargo/bin") + os.pathsep + os.environ.get("PATH", ""))
        try:
            ok = subprocess.run(["/bin/bash", os.path.join(wt, "tests", "gate.sh"), "seed-into", t], cwd=wt, env=env,
                                capture_output=True, timeout=900).returncode == 0
        except (OSError, subprocess.SubprocessError):
            ok = False
        if not ok and not os.path.exists(t):
            os.makedirs(t, exist_ok=True)
            with open(os.path.join(t, "RECLAIMED.txt"), "w") as fh:
                fh.write("heavy-run: this cargo target held %.1f GB of its own at %s and no seed clone could replace it "
                         "(scripts/disk_reclaim.py): its next build is cold.\n" % (own, time.strftime("%Y-%m-%d %H:%M")))
    for a in away:
        shutil.rmtree(a, ignore_errors=True)
    gb = max(0, free_kb() - before) / (GB / 1024)
    with open(log, "a") as fh:
        fh.write("%d\t%s\t%d\t%.1f\ttrim-%s\n" % (time.time(), t, 0, gb, act))
    return act, gb


def drop_incremental(t, log):
    """The disk is low: t's incremental dirs go (its deps and outputs stay warm), under its cargo locks."""
    incs = incrementals(t)
    fds = lock_all(t) if incs else None
    if fds is None:
        return None
    before = free_kb()
    away = []
    try:
        for d in incs:
            a = gone_dir(d)
            os.rename(d, a)
            away.append(a)
    except OSError:
        pass
    finally:
        for f in fds:
            os.close(f)
    for a in away:
        shutil.rmtree(a, ignore_errors=True)
    gb = max(0, free_kb() - before) / (GB / 1024)
    with open(log, "a") as fh:
        fh.write("%d\t%s\t%d\t%.1f\tlow-incremental\n" % (time.time(), t, 0, gb))
    return gb


def agent_tmps():
    """The agents' tmp dirs (<home>/hubs/<hub>/agents/<agent>/tmp); HEAVY_TMP_ROOTS (tests): a ':' list instead."""
    r = os.environ.get("HEAVY_TMP_ROOTS")
    if r is not None:
        return [p for p in r.split(":") if p]
    hubs = os.path.expanduser("~/.bise/hubs")
    return [os.path.join(hubs, h, "agents", a, "tmp") for h in _ls(hubs)
            for a in _ls(os.path.join(hubs, h, "agents")) if os.path.isdir(os.path.join(hubs, h, "agents", a, "tmp"))]


def log_tmps(run, now):
    """Once an hour: each agent tmp's GB in <run>/tmp-sizes.log (<epoch> <path> <GB>), and one line per tmp
    over HEAVY_TMP_CAP_GB (2) on stdout, once (<run>/tmp-warned): a week of sizes decides more (architect m_55867)."""
    stamp = os.path.join(run, "tmp-sizes.stamp")
    last = mtime(stamp)
    if last is not None and now - last < 3600:
        return
    open(stamp, "w").close()
    sizes = {t: own_bytes(t, None) / GB for t in agent_tmps()}
    with open(os.path.join(run, "tmp-sizes.log"), "a") as fh:
        for t, gb in sorted(sizes.items()):
            fh.write("%d\t%s\t%.2f\n" % (now, t, gb))
    wf = os.path.join(run, "tmp-warned")
    try:
        warned = set(open(wf).read().split("\n"))
    except OSError:
        warned = set()
    over = tmp_over(sizes, float(os.environ.get("HEAVY_TMP_CAP_GB", 2)), warned)
    for t in over:
        print("disk_reclaim: %s holds %.1f GB (over %s GB)" % (t, sizes[t], os.environ.get("HEAVY_TMP_CAP_GB", 2)))
    if over:
        with open(wf, "a") as fh:
            fh.write("".join(t + "\n" for t in over))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--want", type=float, default=0, help="GB free to reach (0: idle ones only)")
    ap.add_argument("--idle-h", type=float, default=float(os.environ.get("HEAVY_IDLE_TARGET_H", 6)))
    ap.add_argument("--min-idle-min", type=float, default=float(os.environ.get("HEAVY_MIN_IDLE_MIN", 30)))
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        _self_test()
        return 0
    run = os.path.expanduser("~/.bise/run")
    os.makedirs(run, exist_ok=True)
    lk = open(os.path.join(run, "reclaim.lock"), "w")
    try:
        fcntl.flock(lk, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        print("disk_reclaim: another reclaim runs")
        return 0
    # leftovers of a reclaim that died mid-rm
    for r in roots():
        for dirpath, dirs, _ in os.walk(r):
            for d in list(dirs):
                if d.startswith("target.reclaiming-"):
                    shutil.rmtree(os.path.join(dirpath, d), ignore_errors=True)
                    dirs.remove(d)
                elif d in ("node_modules", ".git", "target"):
                    dirs.remove(d)
            if dirpath.count(os.sep) - r.count(os.sep) >= 4:
                dirs[:] = []
    exes = running_exes()
    now = time.time()
    cwds = job_cwds()
    keep = kept()
    ts = [(t, last_used(t), busy(t, exes, r, cwds)) for r in roots() for t in find_targets(r) if not protected(t, keep)]
    used = {t: u for t, u, _ in ts}
    log = os.path.join(run, "reclaim.log")
    freed, n, trimmed = 0.0, 0, set()
    # the trim: a target a green gate marked, idle since, over HEAVY_TRIM_GB of its own
    marked = [(t, u, b, mtime(os.path.join(t, ".idle-since"))) for t, u, b in ts]
    for t in trim_candidates(marked, now, a.min_idle_min):
        if a.dry_run:
            print("would measure %s for a trim" % t)
            continue
        r = trim_one(t, float(os.environ.get("HEAVY_TRIM_GB", 4)), log)
        if r is not None:
            trimmed.add(t)
            freed += r[1]
            n += 1
            print("disk_reclaim: trimmed %s (%s, %.1f GB)" % (t, r[0], r[1]))
    # the disk is low: first the incremental dirs of the idle targets, then whole targets
    if free_kb() / (GB / 1024) < a.want:
        for t, u, b in sorted(ts, key=lambda x: x[1]):
            if b or now - u < a.min_idle_min * 60 or free_kb() / (GB / 1024) >= a.want:
                continue
            if a.dry_run:
                print("would drop the incremental dirs of %s" % t)
                continue
            gb = drop_incremental(t, log)
            if gb is not None:
                freed += gb
                print("disk_reclaim: dropped %s's incremental dirs (%.1f GB)" % (t, gb))
    if not a.dry_run:
        log_tmps(run, now)
    todo = plan(ts, free_kb() / (GB / 1024), a.want, now, a.idle_h, a.min_idle_min, {t for t, _, _ in ts if stray(t)})
    for t, reason in todo:
        if t in trimmed or not os.path.isdir(t):
            continue  # trimmed in this pass: its use dates are stale
        if reason == "low" and free_kb() / (GB / 1024) >= a.want:
            break
        idle_min = int((now - used[t]) / 60)
        if a.dry_run:
            print("would delete %s (%s, unused %d min)" % (t, reason, idle_min))
            continue
        gb = reclaim_one(t, reason, idle_min, log)
        if gb is not None:
            freed += gb
            n += 1
            print("disk_reclaim: deleted %s (%s, unused %d min, %.1f GB)" % (t, reason, idle_min, gb))
    if n:
        print("disk_reclaim: %d target(s), %.1f GB freed, %.1f GB free now" % (n, freed, free_kb() / (GB / 1024)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
