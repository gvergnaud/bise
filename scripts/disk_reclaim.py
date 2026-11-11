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
protected(), checked on each target's real path; but for the desktop candidates' build target
(desktop-cand/work/target, allowed(): a seed clone the launched .app never points into), busy
while a candidate build holds desktop-cand/work.lock.
A target: a dir named target with cargo's CACHEDIR.TAG or a .rustc_info.json (a clone of the
gate seed had no tag: on 2026-11-08, 11 of 12 task targets were never seen). The roots
include the tip check's tree (~/.bise/run/tip-check), .land-check's (a dot project), the
nightly/tip gate's tree (~/.bise/run/nightly/tree) and ~/.bise/dev/build/desktop-cand.
The trim (disk-hygiene, architect m_55867), before any deletion: a target whose green gate
wrote <target>/.idle-since and that is idle --min-idle-min since is measured once: its own
bytes are the files newer than its .seeded (gate.sh's mark of a seed clone; without it, all
its files: it shares nothing with the seed). Over HEAVY_TRIM_GB (4): its incremental dirs go
when that brings it under, else it is replaced by a fresh clone of the seed: its worktree's
`gate.sh seed-into` builds the clone beside it, then, still not busy, under its cargo locks,
the old one is swapped out and deleted; a failed clone leaves it as it was. A reclaim.log
line `trim-incremental|trim-reclone` (`...-failed`, `...-skipped-busy`).
The own-cap (--own <cwd>, fast-loop, architect m_59130; main m_59126: three active task
targets reached 36+8+6 GB between trims, never idle 30 min nor green-marked, and every gate
waited hours for disk): heavy-run runs it at each job's admission, before the job waits or
builds. The targets of the job's agent dir, not busy, measured at most once per
HEAVY_OWN_GAP_MIN (10) each (<target>.own-checked, written after the measure), over
HEAVY_OWN_CAP_GB (10) of their own: the same trim, logged `trim-own-<act>`. One measure at
a time per target (<target>.own-lock, a locked one is skipped); every error is a line, never
a failed job. Each measure: a line in ~/.bise/run/own-checks.log
<epoch> <target> <own GB> <seconds> <action or ->.
While the disk is
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


def allowed():
    """Under a kept dir but a normal target all the same: the desktop candidates' build target
    (try_build.py: a gate-seed clone; the .app he launches copies or hard-links its binaries out of
    bins.sh's cache, never pointing into it; architect m_59204). HEAVY_RECLAIM_ALLOW (tests)."""
    a = os.environ.get("HEAVY_RECLAIM_ALLOW")
    xs = [p for p in a.split(":") if p] if a is not None else [os.path.expanduser("~/.bise/dev/build/desktop-cand/work/target")]
    return [os.path.realpath(p) for p in xs]


def _under(p, ks):
    return any(p == k or p.startswith(k.rstrip(os.sep) + os.sep) for k in ks)


def protected(path, keep, allow=()):
    """Whether `path` (resolved) is one of `keep` or under it, and not one of `allow` or under it.
    Pure but for realpath."""
    p = os.path.realpath(path)
    return _under(p, keep) and not _under(p, allow)


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


def own_due(busy, checked_at, now, gap_min):
    """Whether the own-cap measures a target now: not busy, and not measured
    in the last gap_min minutes (checked_at: its stamp's mtime, None: never)."""
    return not busy and (checked_at is None or now - checked_at >= gap_min * 60)


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
        # the candidates' build target is let out of the kept dir; the dev build's own target is not
        cand = os.path.join(build, "desktop-cand", "work", "target")
        allow = [os.path.realpath(cand)]
        assert not protected(cand, keep, allow) and not protected(os.path.join(cand, "debug"), keep, allow)
        assert protected(os.path.join(build, "target"), keep, allow), "the try's own bise stays kept"
        assert protected(os.path.join(build, "desktop-cand", "work", "src"), keep, allow)
        # a candidate build holds work.lock (lockf) through every step: its target is busy then
        os.makedirs(os.path.join(cand, "debug"))
        open(os.path.join(cand, "debug", ".cargo-lock"), "w").close()
        assert not busy(cand, [])
        wl = os.open(os.path.join(build, "desktop-cand", "work.lock"), os.O_RDWR | os.O_CREAT)
        fcntl.flock(wl, fcntl.LOCK_EX)
        assert busy(cand, []), "a candidate builds: its work.lock is held"
        os.close(wl)
        assert not busy(cand, [])
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
        # a target with no .seeded shares nothing with the seed: all of it is its own, deps too
        u = os.path.join(d, "u", "target")
        for sub in ("debug/deps", "debug/incremental"):
            os.makedirs(os.path.join(u, sub))
            with open(os.path.join(u, sub, "f"), "wb") as fh:
                fh.write(b"z" * 8192)
        open(os.path.join(u, "debug", ".cargo-lock"), "w").close()  # a profile dir: cargo's lock
        own, inc = measure(u)
        assert own * GB >= 16384 and 8192 <= inc * GB < 16384, (own, inc)
        # a failed reclone leaves the target byte for byte as it was, with no sibling left
        before = {p: open(p, "rb").read() for p in _all_files(u)}
        log = os.path.join(d, "log")
        r = trim_one(u, 0, log, seed=lambda wt, p: False, wt=d)
        assert r is None and {p: open(p, "rb").read() for p in _all_files(u)} == before
        assert sorted(os.listdir(os.path.dirname(u))) == ["target"], os.listdir(os.path.dirname(u))
        assert open(log).read().split("\t")[-1].strip() == "trim-reclone-failed"
        # busy when the clone is ready (something built there meanwhile): skipped, nothing moved
        def seeded_copy(wt, p):
            os.makedirs(p)
            return True
        r = trim_one(u, 0, log, seed=seeded_copy, wt=d, is_busy=lambda: True)
        assert r is None and {p: open(p, "rb").read() for p in _all_files(u)} == before
        assert sorted(os.listdir(os.path.dirname(u))) == ["target"]
        assert open(log).read().splitlines()[-1].split("\t")[-1] == "trim-reclone-skipped-busy"
        # not busy: the clone is swapped in, the old one gone
        r = trim_one(u, 0, log, seed=seeded_copy, wt=d, is_busy=lambda: False, tag="trim-own")
        assert r is not None and r[0] == "reclone" and _all_files(u) == [], r
        assert sorted(os.listdir(os.path.dirname(u))) == ["target"]
        assert open(log).read().splitlines()[-1].split("\t")[-1] == "trim-own-reclone"
    assert own_due(False, None, now, 10), "never measured: due"
    assert not own_due(True, None, now, 10), "busy: skipped"
    assert not own_due(False, now - 9 * 60, now, 10) and own_due(False, now - 10 * 60, now, 10), "the gap"
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
    """The task folders (.land-check's too: a dot project), the tip check's tree, the nightly/tip
    gate's tree and the desktop candidates' build checkout (architect m_59204: 41 and 24 GB of their
    own sat outside the roots on 2026-11-11). One own-cap for all: with CARGO_INCREMENTAL=0
    (nightly_gate.sh), a tip run from a fresh seed clone leaves 2.7 GB of its own, far under it.
    HEAVY_RECLAIM_ROOTS (tests): a ':' list instead."""
    r = os.environ.get("HEAVY_RECLAIM_ROOTS")
    if r:
        return [p for p in r.split(":") if p]
    base = os.path.expanduser("~/.bise/worktrees")
    return [os.path.join(base, h, a) for h in _ls(base) for a in _ls(os.path.join(base, h))] \
        + [os.path.expanduser(p) for p in ("~/.bise/run/tip-check", "~/.bise/run/nightly/tree", "~/.bise/dev/build/desktop-cand")]


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
    # the candidates' build checkout: a build holds <..>/desktop-cand/work.lock (lockf) through
    # every step, cargo's and pnpm's alike (try_build.py)
    work = os.path.dirname(t.rstrip("/"))
    if os.path.basename(work) == "work" and os.path.exists(os.path.join(os.path.dirname(work), "work.lock")):
        try:
            fd = os.open(os.path.join(os.path.dirname(work), "work.lock"), os.O_RDWR)
        except OSError:
            return True
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            return True
        finally:
            os.close(fd)
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


def measure(t):
    """t's own GB and the part of them in incremental/: the files newer than
    its .seeded; without one, all of them (it shares nothing with a seed)."""
    seeded = mtime(os.path.join(t, ".seeded"))
    return own_bytes(t, seeded) / GB, sum(own_bytes(i, seeded) for i in incrementals(t)) / GB


def _all_files(t):
    return sorted(os.path.join(dp, f) for dp, _, fs in os.walk(t) for f in fs)


def seed_into(wt, path):
    """`wt`'s gate.sh seed-into `path` (absent): a clone of the seed there; whether it worked."""
    env = dict(os.environ, PATH=os.path.expanduser("~/.cargo/bin") + os.pathsep + os.environ.get("PATH", ""))
    try:
        return subprocess.run(["/bin/bash", os.path.join(wt, "tests", "gate.sh"), "seed-into", path], cwd=wt, env=env,
                              capture_output=True, timeout=900).returncode == 0 and os.path.isdir(path)
    except (OSError, subprocess.SubprocessError):
        return False


def _log(log, t, gb, reason):
    with open(log, "a") as fh:
        fh.write("%d\t%s\t%d\t%.1f\t%s%s" % (time.time(), t, 0, gb, reason, chr(10)))


def trim_one(t, trim_gb, log, measured=None, tag="trim", seed=seed_into, wt=None, is_busy=None):
    """Trim t: its own bytes (measured: (own GB, incremental GB), else measured
    here), then nothing, its incremental dirs dropped, or the whole target
    replaced by a fresh clone of the seed. The clone is built beside t
    (<t>.reclone-<pid>, gate.sh seed-into); then, t still not busy
    (is_busy(), checked again: anything may have started there meanwhile),
    under its cargo locks, t is swapped out for it and deleted. A failed
    clone, or t busy by then: the clone goes, t stays as it was. Returns
    (action, GB freed) or None (busy, under trim_gb, or failed)."""
    own, inc = measured if measured is not None else measure(t)
    try:
        os.remove(os.path.join(t, ".idle-since"))  # measured once per green gate
    except OSError:
        pass
    act = trim_action(own, inc, trim_gb)
    if act == "reclone" and wt is None:
        wt = seed_into_from(t)
    if act == "reclone" and wt is None:
        act = "incremental"  # no seed to clone from here: the incremental part only
    if act is None:
        return None
    before = free_kb()
    fresh = None
    if act == "reclone":
        fresh = "%s.reclone-%d" % (t, os.getpid())
        shutil.rmtree(fresh, ignore_errors=True)
        if not seed(wt, fresh):
            shutil.rmtree(fresh, ignore_errors=True)
            _log(log, t, 0, "%s-reclone-failed" % tag)
            return None
    if (is_busy or (lambda: busy(t, running_exes())))():
        if fresh:
            shutil.rmtree(fresh, ignore_errors=True)
        _log(log, t, 0, "%s-%s-skipped-busy" % (tag, act))
        return None
    fds = lock_all(t)
    if fds is None:
        if fresh:
            shutil.rmtree(fresh, ignore_errors=True)
        _log(log, t, 0, "%s-%s-skipped-busy" % (tag, act))
        return None
    away = []
    try:
        for d in (incrementals(t) if act == "incremental" else [t]):
            a = gone_dir(d)
            os.rename(d, a)
            away.append(a)
        if fresh:
            os.rename(fresh, t)
            fresh = None
    except OSError:
        if act == "reclone" and away and not os.path.exists(t):
            os.rename(away.pop(), t)  # the swap failed half-way: the old one back
    finally:
        for f in fds:
            os.close(f)
    if fresh:
        shutil.rmtree(fresh, ignore_errors=True)
    for a in away:
        shutil.rmtree(a, ignore_errors=True)
    gb = max(0, free_kb() - before) / (GB / 1024)
    _log(log, t, gb, "%s-%s" % (tag, act))
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


class target_lock:
    """<t>.own-lock, without waiting: `with target_lock(t) as mine`, mine False
    when another reclaim measures or trims t now (then skip it)."""

    def __init__(self, t):
        self.p = t.rstrip("/") + ".own-lock"
        self.fd = None

    def __enter__(self):
        try:
            self.fd = os.open(self.p, os.O_RDWR | os.O_CREAT, 0o644)
            fcntl.flock(self.fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return True
        except OSError:
            if self.fd is not None:
                os.close(self.fd)
                self.fd = None
            return False

    def __exit__(self, *_):
        if self.fd is not None:
            os.close(self.fd)


def own_cap(cwd):
    """The own-cap of a job about to start in cwd: its agent dir's targets,
    each measured when own_due, trimmed over HEAVY_OWN_CAP_GB. What it did is
    printed (heavy-run shows it to the job) and logged."""
    cwd = os.path.realpath(cwd)
    root = next((r for r in roots() if (cwd + "/").startswith(os.path.realpath(r).rstrip("/") + "/")), None)
    if root is None:
        return
    run = os.path.expanduser("~/.bise/run")
    os.makedirs(run, exist_ok=True)
    cap = float(os.environ.get("HEAVY_OWN_CAP_GB", 10))
    gap = float(os.environ.get("HEAVY_OWN_GAP_MIN", 10))
    keep, allow = kept(), allowed()
    for t in find_targets(root):
        if protected(t, keep, allow):
            continue
        with target_lock(t) as mine:
            if not mine:
                continue
            stamp = t.rstrip("/") + ".own-checked"
            is_busy = lambda: busy(t, running_exes(), root, job_cwds())  # noqa: E731
            if not own_due(is_busy(), mtime(stamp), time.time(), gap):
                continue
            s = time.time()
            own, inc = measure(t)
            secs = time.time() - s
            open(stamp, "w").close()  # after the measure: a failed walk retries next time
            r = None
            if own > cap:
                r = trim_one(t, cap, os.path.join(run, "reclaim.log"), measured=(own, inc), tag="trim-own", is_busy=is_busy)
            with open(os.path.join(run, "own-checks.log"), "a") as fh:
                fh.write("%d\t%s\t%.1f\t%.2f\t%s%s" % (time.time(), t, own, secs, r[0] if r else "-", chr(10)))
            if r:
                print("your cargo target %s held %.1f GB of its own (over HEAVY_OWN_CAP_GB %g): %s, %.1f GB freed"
                      % (t, own, cap, "a fresh clone of the seed" if r[0] == "reclone" else "its incremental dirs dropped", r[1]))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--want", type=float, default=0, help="GB free to reach (0: idle ones only)")
    ap.add_argument("--idle-h", type=float, default=float(os.environ.get("HEAVY_IDLE_TARGET_H", 6)))
    ap.add_argument("--min-idle-min", type=float, default=float(os.environ.get("HEAVY_MIN_IDLE_MIN", 30)))
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--own", metavar="CWD", help="the own-cap of the job that starts in CWD (heavy-run)")
    a = ap.parse_args()
    if a.self_test:
        _self_test()
        return 0
    if a.own:
        try:
            own_cap(a.own)
        except Exception as e:  # never a failed job: a line, and the job goes on
            print("disk_reclaim --own: %s: %s" % (type(e).__name__, e))
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
    allow = allowed()
    root_of = {t: r for r in roots() for t in find_targets(r) if not protected(t, keep, allow)}
    ts = [(t, last_used(t), busy(t, exes, r, cwds)) for t, r in root_of.items()]
    used = {t: u for t, u, _ in ts}
    log = os.path.join(run, "reclaim.log")
    freed, n, trimmed = 0.0, 0, set()
    # the trim: a target a green gate marked, idle since, over HEAVY_TRIM_GB of its own
    marked = [(t, u, b, mtime(os.path.join(t, ".idle-since"))) for t, u, b in ts]
    for t in trim_candidates(marked, now, a.min_idle_min):
        if a.dry_run:
            print("would measure %s for a trim" % t)
            continue
        with target_lock(t) as mine:
            r = trim_one(t, float(os.environ.get("HEAVY_TRIM_GB", 4)), log,
                         is_busy=lambda: busy(t, running_exes(), root_of[t], job_cwds())) if mine else None
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
