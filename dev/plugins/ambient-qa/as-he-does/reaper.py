#!/usr/bin/env python3
"""A candidate run's processes: which they are, and their end on every exit of cand.sh (app-quits).

cand.sh opens the candidate with `open -g -n -a`: the app is launchd's child, never cand.sh's, and its
world's hubs are the app's. A kill of cand.sh's tree (heavy-run's, a pass stopped red, a timeout)
never reaches them: three runs of the 11.11 pass left 'bise cand' in his menu bar for hours
(403489d8 04:30, 5f6b3288 04:36, 410cdb89 07:15 on 2026-11-13, each log ending right after 'open').

  reaper.py list app|world|all <app> <world>   the pids of a fresh ps, one per line (stopcand's lists)
  reaper.py watch <pid> <app> <world> <log>    arm, then on <pid>'s end stop what `list all` names

One matcher (`kind`), for stopcand and the reaper: 'app' runs FROM <app>/Contents/ (its command starts
with it: a command that only names it, like the DMG's repack, is not one); 'world' names the world's
<world>/w folder as a path (<world>/w/... or <world>/w alone, so real-shape never takes real-shape-2's).

`watch` detaches (a double fork, its own session: a killpg or a tree kill of cand.sh spares it) and
waits for <pid>'s exit as a kernel event (kqueue NOTE_EXIT, registered while <pid> lives: no poll, no
pid reuse; already gone at registration = reap at once). cand.sh goes on only once it is armed. Then:
one ps, TERM to each match, up to GRACE s, KILL to what is left; one line per process in <log> and on
the run's stdout (pid, what, signal), so the next leak shows in the log. A clean run's stopcand left
nothing: it says so and exits. Tested in tests/cand_reaper_test.py (fake processes under a tmp dir).
"""
import os
import select
import signal
import subprocess
import sys
import time

GRACE = float(os.environ.get("CAND_REAPER_GRACE", "10"))


def kind(cmd, app, world):
    """'app', 'world' or None for one process's command line (pure)."""
    if cmd.startswith(app.rstrip("/") + "/Contents/"):
        return "app"
    w = world.rstrip("/") + "/w"
    i = cmd.find(w)
    while i >= 0:
        after = cmd[i + len(w):i + len(w) + 1]
        if after in ("", "/", " "):
            return "world"
        i = cmd.find(w, i + 1)
    return None


def pick(snapshot, app, world, want="all", me=()):
    """The (pid, kind, command) of a [(pid, command)] snapshot that `want` takes, never a pid in `me`."""
    out = []
    for pid, cmd in snapshot:
        k = kind(cmd, app, world)
        if k and pid not in me and want in ("all", k):
            out.append((pid, k, cmd))
    return out


def snapshot():
    out = subprocess.run(["ps", "-axww", "-o", "pid=,command="], capture_output=True, text=True).stdout
    rows = []
    for line in out.splitlines():
        pid, _, cmd = line.strip().partition(" ")
        if pid.isdigit():
            rows.append((int(pid), cmd.strip()))
    return rows


def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def say(log, line):
    line = "%s reaper: %s\n" % (time.strftime("%H:%M:%S"), line)
    for f in (log, None):
        try:
            if f is None:
                os.write(1, line.encode())
            else:
                with open(f, "a") as h:
                    h.write(line)
        except OSError:
            pass


def reap(app, world, log):
    me = {os.getpid(), os.getppid()}
    doomed = pick(snapshot(), app, world, "all", me)
    if not doomed:
        say(log, "cand.sh ended, nothing of the candidate or its world left")
        return []
    for pid, k, cmd in doomed:
        say(log, "pid %d (%s: %s) SIGTERM" % (pid, k, cmd[:160]))
        try:
            os.kill(pid, signal.SIGTERM)
        except OSError:
            pass
    t0 = time.monotonic()
    while any(alive(p) for p, _, _ in doomed) and time.monotonic() - t0 < GRACE:
        time.sleep(0.2)
    for pid, k, cmd in doomed:
        if alive(pid):
            say(log, "pid %d (%s: %s) SIGKILL after %g s" % (pid, k, cmd[:160], GRACE))
            try:
                os.kill(pid, signal.SIGKILL)
            except OSError:
                pass
    return doomed


def arm(pid):
    """A kqueue that fires on `pid`'s exit, or None when it is already gone."""
    kq = select.kqueue()
    ev = select.kevent(pid, filter=select.KQ_FILTER_PROC, flags=select.KQ_EV_ADD | select.KQ_EV_ONESHOT,
                       fflags=select.KQ_NOTE_EXIT)
    try:
        kq.control([ev], 0, 0)
    except ProcessLookupError:
        kq.close()
        return None
    return kq


def watch(pid, app, world, log):
    """Detach, arm on `pid`, tell the caller (it waits for that line), then reap at its end."""
    r, w = os.pipe()
    if os.fork():  # cand.sh's child: back once the watcher is armed
        os.close(w)
        ok = os.read(r, 16)
        os._exit(0 if ok.startswith(b"armed") else 1)
    os.close(r)
    os.setsid()
    if os.fork():
        os._exit(0)
    for s in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM, signal.SIGPIPE):
        signal.signal(s, signal.SIG_IGN)
    # no fd of cand.sh's but stdout (the run's log): never one of its locks (a lock held here would
    # hold its world past the run); a pipe a caller reads to its end gets EOF once this ends
    os.closerange(3, w)
    os.closerange(w + 1, 4096)
    kq = arm(pid)
    os.write(w, b"armed")
    os.close(w)
    null = os.open(os.devnull, os.O_RDWR)
    os.dup2(null, 0)
    os.dup2(null, 2)
    if kq is not None:
        while not kq.control(None, 1, None):
            pass
    reap(app, world, log)
    os._exit(0)


def main(argv):
    if len(argv) == 5 and argv[1] == "list" and argv[2] in ("app", "world", "all"):
        me = {os.getpid(), os.getppid()}
        for pid, _, _ in pick(snapshot(), argv[3], argv[4], argv[2], me):
            print(pid)
        return 0
    if len(argv) == 6 and argv[1] == "watch" and argv[2].isdigit():
        watch(int(argv[2]), argv[3], argv[4], argv[5])
        return 0
    sys.stderr.write(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
