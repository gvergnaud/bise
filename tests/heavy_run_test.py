#!/usr/bin/env python3
"""scripts/heavy-run's queue (flow-time): jobs get a slot by rank (--prio, a
land check, the rest, --low), then first come first served; a dead waiter's
ticket blocks nobody; heavy.log gets a start and an end line; --install puts
the script in ~/.bise/run. Runs in a temp HOME with one slot (HEAVY_SLOTS=1),
~15 s."""
import json
import os
import shlex
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wait  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
HEAVY = os.path.join(HERE, "..", "scripts", "heavy-run")
fails = []


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what)
    if not ok:
        fails.append(what)


def wait_for(pred, secs=30, what="the state"):
    try:
        return bool(wait.until(pred, secs, what))
    except AssertionError as e:
        print("     " + str(e)[:300])
        return False


def stays(pred, secs, what):
    try:
        return bool(wait.holds(pred, secs, what))
    except AssertionError as e:
        print("     " + str(e)[:300])
        return False


def job_ends(p, secs=30):
    """The process's rc once it ends, waiting as wait.until does (scaled by
    the machine's load: a fixed 30 s timed out at load 120); None if not."""
    wait_for(lambda: p.poll() is not None, secs, "the job ends")
    return p.poll()


def held(slot):
    """The slot is taken (a probe with lockf -t 0 fails)."""
    return subprocess.run(["/usr/bin/lockf", "-k", "-t", "0", slot, "true"], capture_output=True).returncode != 0


with tempfile.TemporaryDirectory() as home:
    # plenty of faked free space: the real disk's never makes a job wait here
    open(os.path.join(home, "plenty_kb"), "w").write(str(500 * 1048576))
    # on AC as far as heavy-run knows (HEAVY_ON_BATTERY=0): the --alone cases run on battery too
    env = dict(os.environ, HOME=home, HEAVY_SLOTS="1", HEAVY_FREE_KB_FILE=os.path.join(home, "plenty_kb"), HEAVY_ON_BATTERY="0",
               # the machine's real load never holds a job here (the load cases fake it)
               HEAVY_LOAD_MAX="9999", HEAVY_START_GAP="0", HEAVY_RECLAIM_GAP="0", HEAVY_RECLAIM="true")
    env.pop("SB_AGENT", None)
    env.pop("HEAVY_SLOT", None)
    env.pop("HEAVY_SUBMIT", None)
    run = os.path.join(home, ".bise", "run")
    log = os.path.join(run, "heavy.log")
    order = os.path.join(home, "order")
    go = os.path.join(home, "go")
    q = os.path.join(run, "queue")

    def lines():
        return open(log).read().splitlines() if os.path.exists(log) else []

    holder = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go], env=env)
    check(wait_for(lambda: len(lines()) == 1), "the first job gets the free slot at once")
    os.makedirs(q, exist_ok=True)
    # a ticket of a waiter that died: ahead of everyone, it must block nobody
    open(os.path.join(q, "0-1-99999999"), "w").close()

    def job(name, submit, *flags):
        e = dict(env, HEAVY_SUBMIT=str(submit))
        cmd = [HEAVY, *flags, "sh", "-c", 'echo "$0" >> "$1"', name, order]
        return subprocess.Popen(cmd, env=e, stderr=subprocess.DEVNULL)

    jobs = [job("low", 999, "--low"), job("rest-a", 1000), job("land", 1002, "--land"),
            job("prio", 1003, "--prio"), job("rest-b", 1001)]
    check(wait_for(lambda: len([f for f in os.listdir(q) if not f.startswith("0-1-")]) == 5),
          "each waiting job holds a ticket")
    open(go, "w").close()
    rcs = [j.wait(timeout=60) for j in jobs]
    holder.wait(timeout=10)
    got = open(order).read().split()
    check(rcs == [0] * 5, "every job ran: %r" % rcs)
    check(got == ["prio", "land", "rest-a", "rest-b", "low"], "by rank, then by submit time: %r" % got)
    check(os.listdir(q) == [], "no ticket left behind: %r" % os.listdir(q))
    ends = [l for l in lines() if l.split("\t")[2] != "-"]
    starts = [l for l in lines() if l.split("\t")[2] == "-"]
    check(len(starts) == 6 and len(ends) == 6, "a start and an end line per job: %d/%d" % (len(starts), len(ends)))
    check(all(l.endswith("rc=0") and l.split("\t")[2].isdigit() for l in ends), "the end line has its end time and rc")

    # a SIGKILLed waiter's ticket and a reused pid's ticket (a process
    # younger than the ticket) block nobody
    go3 = os.path.join(home, "go3")
    holder = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go3], env=env)
    check(wait_for(lambda: len([l for l in lines() if l.split("\t")[2] == "-"]) == 7), "a holder takes the slot")
    # (no release lane here: a --prio job would run in it at once, no waiter)
    dead = subprocess.Popen([HEAVY, "--prio", "true"], env=dict(env, HEAVY_SUBMIT="1", HEAVY_NO_LANE="1"), stderr=subprocess.DEVNULL)
    check(wait_for(lambda: any(f.startswith("0-1-") for f in os.listdir(q))), "a --prio waiter's ticket")
    dead.kill()
    dead.wait()
    young = subprocess.Popen(["sleep", "60"])
    with open(os.path.join(q, "0-2-%d" % young.pid), "w") as fh:
        fh.write("%d" % (time.time() - 1000))
    after = subprocess.Popen([HEAVY, "sh", "-c", 'echo "$HEAVY_SLOT" > "$0"', os.path.join(home, "slotenv")], env=env,
                             stderr=subprocess.DEVNULL)
    wait_for(lambda: any(f.startswith("2-") for f in os.listdir(q)), what="the next job's ticket")
    open(go3, "w").close()
    check(after.wait(timeout=30) == 0, "the next job runs past a killed waiter's and a reused pid's tickets")
    young.kill()
    holder.wait(timeout=10)
    slot_env = open(os.path.join(home, "slotenv")).read().split()
    check(len(slot_env) == 2 and slot_env[0] == "heavy.lock" and slot_env[1].isdigit(),
          "the job has HEAVY_SLOT=<slot> <pid>: %r" % slot_env)
    check(os.listdir(q) == [], "the dead tickets are gone: %r" % os.listdir(q))

    # the job's rc comes back; a land check is ranked without --land
    r = subprocess.run([HEAVY, "sh", "-c", "exit 3", "tests/gate.sh", "full"], env=env, capture_output=True)
    check(r.returncode == 3, "the job's rc comes back: %d" % r.returncode)
    # nested: refused
    r = subprocess.run([HEAVY, "sh", "-c", '"$0" true', HEAVY], env=env, capture_output=True, text=True)
    check(r.returncode == 2 and "refused" in r.stderr, "a heavy-run inside a heavy job is refused")
    # the release lane (heavy5.lock, fast-loop): every agent slot held, a --prio
    # job runs at once in the lane; a plain job never takes it
    go5 = os.path.join(home, "go5")
    hold = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go5], env=env)
    check(wait_for(lambda: held(os.path.join(run, "heavy.lock"))), "a job holds the one agent slot")
    r = subprocess.run([HEAVY, "--prio", "sh", "-c", 'echo "$HEAVY_SLOT"'], env=env, capture_output=True, text=True, timeout=30)
    check(r.returncode == 0 and r.stdout.split()[:1] == ["heavy5.lock"], "a --prio job runs in the release lane: %r" % r.stdout)
    plain = subprocess.Popen([HEAVY, "true"], env=env, stderr=subprocess.DEVNULL)
    check(stays(lambda: plain.poll() is None, 2, "the plain job waits"), "a plain job never takes the release lane")
    open(go5, "w").close()
    check(plain.wait(timeout=30) == 0, "the plain job runs once the agent slot frees")
    hold.wait(timeout=10)
    # a slot held without heavy-run (an old caller's lockf) is busy; an --alone
    # waiter (the nightly) drains the queue: a job after it never takes the
    # free slot 2 meanwhile
    on_ac = True  # HEAVY_ON_BATTERY=0 in env
    if on_ac:
        os.remove(order)
        go2 = os.path.join(home, "go2")
        env2 = dict(env, HEAVY_SLOTS="2", HEAVY_ALONE_DRAIN_MIN="0")  # past its drain time at once
        hold = subprocess.Popen(["/usr/bin/lockf", "-k", os.path.join(run, "heavy.lock"), "sh", "-c",
                                 'while [ ! -e "$0" ]; do sleep 0.1; done', go2])
        wait_for(lambda: held(os.path.join(run, "heavy.lock")), what="the plain lockf holds slot 1")
        alone = subprocess.Popen([HEAVY, "--alone", "sh", "-c", 'echo alone >> "$0"', order], env=env2)
        check(wait_for(lambda: any(open(os.path.join(q, f)).read().split()[-1:] == ["alone"] for f in os.listdir(q))),
              "an --alone waiter holds an 'alone' ticket")
        rest = subprocess.Popen([HEAVY, "sh", "-c", 'echo rest >> "$0"', order], env=env2, stderr=subprocess.DEVNULL)
        check(stays(lambda: not os.path.exists(order), 3, "nothing runs"),
              "slot 2 free, slot 1 held by a plain lockf: nothing ran past the --alone waiter")
        open(go2, "w").close()
        alone.wait(timeout=30)
        rest.wait(timeout=30)
        hold.wait(timeout=10)
        got = open(order).read().split() if os.path.exists(order) else []
        check(got == ["alone", "rest"], "the --alone job first, then the rest: %r" % got)
        # a young --alone waiter (main m_37006) holds no slot and nobody back: a job after it
        # runs in the free slot 2 while slot 1 is busy; the --alone job runs once all are free
        os.remove(order)
        go7 = os.path.join(home, "go7")
        envy = dict(env, HEAVY_SLOTS="2", HEAVY_ALONE_DRAIN_MIN="10")
        hold = subprocess.Popen(["/usr/bin/lockf", "-k", os.path.join(run, "heavy.lock"), "sh", "-c",
                                 'while [ ! -e "$0" ]; do sleep 0.1; done', go7])
        wait_for(lambda: held(os.path.join(run, "heavy.lock")), what="the plain lockf holds slot 1")
        alone = subprocess.Popen([HEAVY, "--alone", "sh", "-c", 'echo alone >> "$0"', order], env=envy)
        check(wait_for(lambda: any(open(os.path.join(q, f)).read().split()[-1:] == ["alone"] for f in os.listdir(q))),
              "the young --alone waiter has its ticket")
        rest = subprocess.Popen([HEAVY, "sh", "-c", 'echo rest >> "$0"', order], env=envy, stderr=subprocess.DEVNULL)
        check(job_ends(rest, 30) == 0 and open(order).read().split() == ["rest"],
              "a job after a young --alone waiter runs in the free slot: %r" % (open(order).read() if os.path.exists(order) else None))
        check(alone.poll() is None and not held(os.path.join(run, "heavy2.lock")),
              "the --alone waiter holds no slot while one is busy")
        open(go7, "w").close()
        job_ends(hold, 10)
        check(job_ends(alone, 30) == 0 and open(order).read().split() == ["rest", "alone"], "the --alone job runs once all 4 are free")
        # an --alone measure keeps its class: after a land check submitted before it
        os.remove(order)
        go4 = os.path.join(home, "go4")
        hold = subprocess.Popen(["/usr/bin/lockf", "-k", os.path.join(run, "heavy.lock"), "sh", "-c",
                                 'while [ ! -e "$0" ]; do sleep 0.1; done', go4])
        wait_for(lambda: held(os.path.join(run, "heavy.lock")), what="the plain lockf holds slot 1")
        env1 = dict(env, HEAVY_SLOTS="1")
        land = subprocess.Popen([HEAVY, "--land", "sh", "-c", 'echo land >> "$0"', order], env=dict(env1, HEAVY_SUBMIT="3000"),
                                stderr=subprocess.DEVNULL)
        meas = subprocess.Popen([HEAVY, "--alone", "sh", "-c", 'echo alone >> "$0"', order], env=dict(env1, HEAVY_SUBMIT="3001"))
        check(wait_for(lambda: len(os.listdir(q)) == 2), "both wait with a ticket")
        check(stays(lambda: not os.path.exists(order), 2, "nothing runs"),
              "the --alone measure takes no slot while a land check is ahead")
        open(go4, "w").close()
        job_ends(land)
        job_ends(meas)
        job_ends(hold)
        got = open(order).read().split() if os.path.exists(order) else []
        check(got == ["land", "alone"], "a land check before an --alone measure: %r" % got)
        # an --alone job that ran while a release-lane job ran: its end line says
        # 'lane=busy' (the lane runs beside it; a timing red may come from it)
        go6 = os.path.join(home, "go6")
        lane = subprocess.Popen([HEAVY, "--prio", "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go6], env=env1,
                                stderr=subprocess.DEVNULL)
        # read from heavy.log, not by probing the lock: a probe holds the lock for
        # a moment, and at load 100+ the --prio job's own try met it and took slot 1
        check(wait_for(lambda: any(l.split("\t")[2:4] == ["-", "heavy5.lock"] and "go6" in l for l in lines())),
              "a --prio job holds the release lane")
        a = subprocess.run([HEAVY, "--alone", "sh", "-c", "echo marked"], env=env1, capture_output=True, text=True, timeout=30)
        open(go6, "w").close()
        lane.wait(timeout=10)
        end = [l for l in lines() if l.split("\t")[2] != "-" and "echo marked" in l]
        check(a.returncode == 0 and len(end) == 1 and end[0].endswith("rc=0\tlane=busy"), "an --alone job beside the lane is marked: %r" % end)
    else:
        print("skip --alone on battery (it waits for AC power)")
    # the disk (disk-guard): faked free space; under 15 GB a job waits and
    # says why (stderr and the agent's status, a fake sb); a running job is
    # stopped under 5 GB with rc=28 and a clear line
    GB = 1048576
    freef = os.path.join(home, "free_kb")
    said = os.path.join(home, "sb_said")
    fbin = os.path.join(home, "bin")
    os.makedirs(fbin, exist_ok=True)
    with open(os.path.join(fbin, "sb"), "w") as fh:
        fh.write('#!/bin/sh\necho "$*" >> "%s"\n' % said)
    os.chmod(os.path.join(fbin, "sb"), 0o755)
    envd = dict(env, HEAVY_FREE_KB_FILE=freef, HEAVY_DISK_POLL="0.2", SB_AGENT="fake",
                PATH=fbin + os.pathsep + env["PATH"])
    # the disk cases count flat reserves: the test's own jobs so far each left
    # a disk.tsv line ('other', ~0 written), and 5 of them make a learned 1 GB
    fresh_measures = lambda: os.path.exists(os.path.join(run, "disk.tsv")) and os.remove(os.path.join(run, "disk.tsv"))  # noqa: E731
    fresh_measures()
    open(freef, "w").write(str(11 * GB))
    ran = os.path.join(home, "disk_ran")
    errf = open(os.path.join(home, "disk_err"), "w+")
    waiter = subprocess.Popen([HEAVY, "sh", "-c", 'touch "$0"', ran], env=envd, stderr=errf)
    check(stays(lambda: not os.path.exists(ran), 2, "the job waits"), "11 GB free: the job waits for disk (10 kept free + 2)")
    sb_lines = lambda: open(said).read() if os.path.exists(said) else ""
    wait_for(lambda: "waiting for disk room" in sb_lines(), 30, "the disk-wait status")
    check("waiting for disk room: 11 GB free" in sb_lines() and sb_lines().startswith("status working --note"),
          "the waiting agent's status says why: %r" % sb_lines())
    # the disk stays short for 2.5 s from the status, which heavy-run writes
    # once it waits on the disk: its own clock then counts >= 2 s whatever
    # the load (the desktop-v2026.10.21 tip gate: a slow start left 1 s of
    # the test's fixed 2 s, 'waited=disk:1')
    check(stays(lambda: not os.path.exists(ran), 2.5, "the job waits"), "it still waits 2.5 s after its status")
    open(freef, "w").write(str(20 * GB))
    check(waiter.wait(timeout=30) == 0 and os.path.exists(ran), "20 GB free: the job runs")
    # its start line says what it waited on (review-gates m_37641): the disk, some seconds
    st = [l.split("\t") for l in lines() if ran in l and l.split("\t")[2] == "-"]
    wt = dict(kv.split(":") for kv in st[-1][6][len("waited="):].split()) if st and st[-1][6:7] and st[-1][6].startswith("waited=") else {}
    # (its last round, the slot probe once the disk was there, counts a second or two as slot)
    check(int(wt.get("disk", 0)) >= 2 and int(wt.get("slot", 9)) <= 2 and wt.get("load") == "0",
          "its start line: waited=disk:>=2 slot:<=2 load:0: %r" % (st[-1][6:] if st else lines()[-2:]))
    errf.seek(0)
    err = errf.read()
    check("heavy-run: waiting for disk room: 11 GB free" in err and "disk room again (20 GB free)" in err,
          "stderr names the wait and its end: %r" % err)
    r = subprocess.Popen([HEAVY, "sh", "-c", 'sleep 60 & wait', "x"], env=envd, stderr=subprocess.PIPE, text=True)
    wait_for(lambda: len([l for l in lines() if l.split("\t")[2] == "-"]) > len(
        [l for l in lines() if l.split("\t")[2] != "-"]), what="the long job runs")
    t0 = time.time()
    open(freef, "w").write(str(3 * GB))
    try:
        _, err = r.communicate(timeout=20)
    except subprocess.TimeoutExpired:
        r.kill()
        _, err = r.communicate()
    check(r.returncode == 28 and time.time() - t0 < 15, "3 GB free: the running job is stopped, rc=28: %r" % r.returncode)
    check("heavy-run: stopped" in err and "3 GB free" in err, "the stop line says why: %r" % err)
    check("heavy-run: stopped" in sb_lines(), "the stop goes to the agent's status")
    check(lines()[-1].endswith("rc=28"), "heavy.log ends the job with rc=28: %r" % lines()[-1])
    check(not [f for f in os.listdir(run) if f.startswith("disk-stop.")], "no disk-stop marker left")
    # the reserves (mac-budget): a job needs 15 GB + its reserve + what the
    # running jobs reserved; while it waits, the reclaim runs (a fake here)
    reclaimed = os.path.join(home, "reclaim_args")
    fake_reclaim = os.path.join(fbin, "reclaim")
    with open(fake_reclaim, "w") as fh:
        fh.write('#!/bin/sh\necho "$*" >> "%s"\n' % reclaimed)
    os.chmod(fake_reclaim, 0o755)
    envr = dict(envd, HEAVY_SLOTS="2", HEAVY_RECLAIM=fake_reclaim)
    open(freef, "w").write(str(21 * GB))
    go_r = os.path.join(home, "go_reserve")
    big = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go_r],
                           env=dict(envr, HEAVY_RESERVE_GB="10"), stderr=subprocess.DEVNULL)
    check(wait_for(lambda: [f for f in os.listdir(os.path.join(run, "reserve")) if f.startswith("heavy")]
                   if os.path.isdir(os.path.join(run, "reserve")) else False),
          "a running job leaves its reserve in ~/.bise/run/reserve")
    ran2 = os.path.join(home, "ran2")
    fresh_measures()
    e2 = open(os.path.join(home, "err2"), "w+")
    second = subprocess.Popen([HEAVY, "sh", "-c", 'touch "$0"', ran2], env=envr, stderr=e2)
    check(stays(lambda: not os.path.exists(ran2), 2, "the second job waits"),
          "21 GB free, 10 reserved by a running job that named it: a 2 GB job waits (needs 22) %r" % lines()[-3:])
    check(wait_for(lambda: os.path.exists(reclaimed) and "--want 22" in open(reclaimed).read(), 10),
          "the waiting job asks the reclaim for 22 GB")
    e2.seek(0)
    check("starts at 22 GB (10 kept free + 2 it may write + 10 the running jobs may still write)" in e2.read(), "the wait says the sum")
    open(freef, "w").write(str(22 * GB))
    check(job_ends(second, 30) == 0 and os.path.exists(ran2), "22 GB free: it runs")
    # what the disk lost since the running job started counts as written by
    # it: started at 30 GB free, now 22 (8 written): 2 left, a 2 GB job
    # needs 19 and runs (the whole sum would want 27: main m_36317)
    open(go_r, "w").close()
    job_ends(big, 30)
    open(freef, "w").write(str(30 * GB))
    go_w = os.path.join(home, "go_written")
    big = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go_w],
                           env=dict(envr, HEAVY_RESERVE_GB="10"), stderr=subprocess.DEVNULL)
    check(wait_for(lambda: [f for f in os.listdir(os.path.join(run, "reserve")) if f.startswith("heavy")]),
          "the 10 GB job runs")
    open(freef, "w").write(str(20 * GB))
    ran5 = os.path.join(home, "ran5")
    r = subprocess.run([HEAVY, "sh", "-c", 'touch "$0"', ran5], env=envr, capture_output=True, text=True, timeout=30)
    check(r.returncode == 0 and os.path.exists(ran5), "10 GB written since it started: a 2 GB job runs at 20 GB free (the whole sum would want 22): %r" % r.stderr[-300:])
    open(go_w, "w").close()
    job_ends(big, 30)
    check(not [f for f in os.listdir(os.path.join(run, "reserve"))], "the reserves are gone with their jobs")
    # the reserve per kind (main m_37136): each job's disk.tsv line says what
    # it wrote (free KB at its start minus the lowest the watcher saw)
    dtsv = os.path.join(run, "disk.tsv")
    if os.path.exists(dtsv):
        os.remove(dtsv)
    open(freef, "w").write(str(40 * GB))
    go_d = os.path.join(home, "go_disk")
    big = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go_d], env=envr, stderr=subprocess.DEVNULL)
    check(wait_for(lambda: [f for f in os.listdir(os.path.join(run, "reserve")) if f.startswith("heavy")]), "the disk job runs")
    open(freef, "w").write(str(37 * GB))
    # the watcher has seen 37 GB: its disk-min file says so (no sleep: the event)
    check(wait_for(lambda: [f for f in os.listdir(run) if f.startswith("disk-min.")
                            and open(os.path.join(run, f)).read().strip() == str(37 * GB)]), "the watcher saw 37 GB")
    open(freef, "w").write(str(39 * GB))
    open(go_d, "w").close()
    job_ends(big, 30)
    d = open(dtsv).read().splitlines()[-1].split("\t") if os.path.exists(dtsv) else []
    check(len(d) == 7 and d[1] == "other" and d[2] == str(3 * GB) and d[3] == str(40 * GB) and d[4] == str(37 * GB),
          "disk.tsv: kind other, wrote 3 GB (40 at start, 37 at the lowest, 39 at the end): %r" % d)
    check(not [f for f in os.listdir(run) if f.startswith("disk-min.")], "no disk-min file left")
    # a reclaim during the job (review-gates m_37481): what it freed (its
    # reclaim.log line) is added back. 40 GB at the start, a reclaim frees 3
    # (43), the job writes 4: 39 at the end, the lowest seen 39. It wrote
    # 40 - 39 + 3 = 4 GB, not the 1 the lowest alone says
    rlog = os.path.join(run, "reclaim.log")
    go_m = os.path.join(home, "go_mark")
    open(freef, "w").write(str(40 * GB))
    big = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go_m], env=envr, stderr=subprocess.DEVNULL)
    check(wait_for(lambda: [f for f in os.listdir(os.path.join(run, "reserve")) if f.startswith("heavy")]), "the reclaimed job runs")
    open(freef, "w").write(str(43 * GB))
    with open(rlog, "a") as fh:
        fh.write("%d\t/x/rust/target\t30\t3.0\tlow\n" % time.time())
        fh.write("%d\t/y/rust/target\t30\t5.0\tlow\n" % (time.time() - 3600))
    open(freef, "w").write(str(39 * GB))
    check(wait_for(lambda: [f for f in os.listdir(run) if f.startswith("disk-min.")
                            and open(os.path.join(run, f)).read().strip() == str(39 * GB)]), "the watcher saw 39 GB")
    open(go_m, "w").close()
    job_ends(big, 30)
    d = open(dtsv).read().splitlines()[-1].split("\t")
    check(d[1] == "other" and d[2] == str(4 * GB), "a reclaim freed 3 GB mid-job: it wrote 4 GB (the reclaim an hour before not counted): %r" % d[:5])
    # a reclaim still running at its end has not logged what it freed: '-'
    go_m2 = os.path.join(home, "go_mark2")
    big = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go_m2], env=envr, stderr=subprocess.DEVNULL)
    check(wait_for(lambda: [f for f in os.listdir(os.path.join(run, "reserve")) if f.startswith("heavy")]), "the next job runs")
    open(os.path.join(run, "reclaim.running"), "w").close()
    open(go_m2, "w").close()
    job_ends(big, 30)
    os.remove(os.path.join(run, "reclaim.running"))
    os.remove(rlog)
    d = open(dtsv).read().splitlines()[-1].split("\t")
    check(d[1] == "other" and d[2] == "-", "a reclaim still running at its end: written '-': %r" % d[:3])

    def reserve(kind, rows, src="\tsrc=target"):
        with open(dtsv, "w") as fh:
            fh.writelines(("1\t%s\t%s\t0\t0\t0\tx" + src + "\n") % (kind, kb) for kb in rows)
        fn = 'eval "$(sed -n -e "/^reserve_gb()/,/^}/p" -e "/^p90_gb()/,/^}/p" "$0")"; reserve_gb "$1"'
        return subprocess.run(["bash", "-c", fn, HEAVY, kind], env=dict(env, HOME=home), capture_output=True, text=True).stdout.strip()
    check(reserve("cargo-cold", [1 * GB] * 4) == "10" and reserve("cargo-cold", []) == "10",
          "4 jobs of a kind, or none: the flat guess, 10")
    check(reserve("cargo-cold", [GB // 2] * 4 + [int(2.5 * GB)]) == "3", "5 jobs: the p90, 2.5 GB rounded up to 3")
    check(reserve("cargo-cold", [GB // 2] * 17 + [int(2.5 * GB)] * 3) == "3", "20 jobs: the p90, 2.5 GB rounded up to 3")
    check(reserve("cargo-warm", [9 * GB] * 20) == "3", "never over the flat guess (cargo-warm 3)")
    check(reserve("full", [0] * 20) == "1", "at least 1 GB")
    check(reserve("full", [9 * GB] * 20 + [0] * 20) == "1", "the last 20 of that kind only")
    check(reserve("other", [9 * GB] * 20) == "2" and reserve("cargo-cold", [GB] * 4 + ["bad"] * 3) == "10",
          "other 2; a bad line is skipped (4 left: flat)")
    check(reserve("cargo-cold", [GB] * 4 + ["-"]) == "10", "a '-' line: skipped, 4 left: flat")
    # a cargo kind's rows count only when they measured its own targets' growth (src=target,
    # architect m_60754): its old free-drop rows charged every other writer to it, and stay unread
    check(reserve("cargo-cold", [GB // 2] * 20, src="") == "10", "cargo-cold: 20 free-drop rows (no src=target): unread, flat 10")
    # a cargo job's end: its row is what disk_reclaim.py --grown says (here a fake: 4096 KB), src=target
    os.remove(dtsv)
    fake_grown = os.path.join(home, "fake_grown")
    with open(fake_grown, "w") as fh:
        fh.write('#!/bin/sh\n[ "$1" = --grown ] && echo 4096\nexit 0\n')
    os.chmod(fake_grown, 0o755)
    r = subprocess.run([HEAVY, "sh", "-c", "true", "cargo"], env=dict(envr, HEAVY_RECLAIM=fake_grown), capture_output=True, timeout=60)
    rows = lambda: open(dtsv).read().splitlines() if os.path.exists(dtsv) else []  # noqa: E731
    check(r.returncode == 0 and wait_for(lambda: rows()), "a cargo job ends at once, its row comes after")
    d = rows()[-1].split("\t") if rows() else []
    check(len(d) == 8 and d[1].startswith("cargo-") and d[2] == "4096" and d[7] == "src=own",
          "a cargo job's disk.tsv row: its targets' growth (--grown), src=own: %r" % d)
    # each kind's dirs from one table (kind_dirs, architect m_60879): --grown gets them
    kd = lambda k: subprocess.run(["bash", "-c", 'eval "$(sed -n -e "/^kind_dirs()/,/^}/p" "$0")"; kind_dirs "$1"', HEAVY, k],  # noqa: E731
                                  env=dict(env, CARGO_TARGET_DIR=""), capture_output=True, text=True).stdout.split()
    check(kd("cargo-warm") == ["rust/target", "target", "../target"] and "apps/desktop/out" in kd("desktop") and kd("other") == [],
          "kind_dirs: cargo -> its targets, desktop -> its out/ and caches, other -> none (the free-drop row)")
    # ~/.bise/dev/build's idle piles: disk_reclaim.reclaimable(), pure (architect m_61530)
    sys.path.insert(0, os.path.join(HERE, "..", "scripts"))
    import disk_reclaim as dr
    old = {"exec_known": True, "exec_live": False, "lock_held": False, "newest": False, "nightly_kept": False,
           "age_h": 30, "keep_h": 24}
    rows = [(("cand", "a" * 12), old, "delete"),
            (("cand", "a" * 12), dict(old, newest=True), "keep"),
            (("cand", "a" * 12), dict(old, nightly_kept=True), "keep"),
            (("cand", "a" * 12), dict(old, lock_held=True), "keep"),
            (("cand", "a" * 12), dict(old, exec_live=True), "keep"),
            (("cand", "a" * 12), dict(old, exec_known=False), "keep"),
            (("cand", "a" * 12), dict(old, age_h=10), "keep")]
    cache = {"exec_known": True, "exec_live": False, "lock_held": False, "cargo_held": False,
             "idle_min": 40, "idle_h": 6, "min_idle_min": 30, "low": False}
    rows += [(("cache", "target-jsrt"), cache, "keep"),
             (("cache", "target-jsrt"), dict(cache, low=True), "delete"),
             (("cache", "target-jsrt"), dict(cache, low=True, lock_held=True), "keep"),
             (("cache", "target-jsrt"), dict(cache, low=True, cargo_held=True), "keep"),
             (("cache", "target-jsrt"), dict(cache, idle_min=400), "delete"),
             (("cache", "target-jsrt"), dict(cache, low=True, idle_min=20), "keep")]
    rows += [(("other", n), dict(old, exec_known=True), "keep") for n in ("desktop-try", "ambient", "desktop", "cache")]
    bad = [(e, f, want, dr.reclaimable(e, f)) for e, f, want in rows if dr.reclaimable(e, f)[0] != want]
    check(not bad, "reclaimable: an old candidate goes; the newest, a nightly-kept one, a held lock, a live process, "
          "an unread process table, a young one stay; a cache goes idle 6 h or low + 30 min, never locked; "
          "desktop-try/ambient/desktop/cache always stay: %r" % bad)
    # 'desktop' (a quick gate whose heavy steps are desktop/office only, architect m_60481): its own
    # reserve the flat 2; what a RUNNING one may still write, flat 2 under 10 measured runs, then
    # their p90 (measured_gb), like the other kinds
    def measured(kind, rows, src="\tsrc=target"):
        with open(dtsv, "w") as fh:
            fh.writelines(("1\t%s\t%s\t0\t0\t0\tx" + src + "\n") % (kind, kb) for kb in rows)
        fn = 'eval "$(sed -n -e "/^measured_gb()/,/^}/p" -e "/^p90_gb()/,/^}/p" "$0")"; measured_gb "$1"'
        return subprocess.run(["bash", "-c", fn, HEAVY, kind], env=dict(env, HOME=home), capture_output=True, text=True).stdout.strip()
    check(reserve("desktop", []) == "2" and reserve("desktop", [9 * GB] * 20) == "2", "desktop's own reserve: flat 2, never over")
    check(measured("desktop", [5 * GB] * 9) == "2", "a running desktop job, 9 measured: the flat 2")
    check(measured("desktop", [5 * GB] * 10) == "5", "a running desktop job, 10 measured: their p90, 5")
    check(measured("cargo-warm", [19 * GB] * 20, src="") == "10", "a running cargo-warm job: 20 free-drop rows at 19 GB: unread, flat 10")
    check(measured("cargo-warm", [GB] * 9) == "10" and measured("cargo-warm", [5 * GB] * 10) == "5",
          "a running cargo-warm job: 9 own rows, its flat 10 (a guess errs high, architect m_61208); 10 at 5 GB, their p90 5")
    # the cargo ceiling (architect m_61208): max(1, cpus / 6), HEAVY_CARGO_MAX wins; the cargo
    # jobs running are the live reserve files of a cargo kind
    cm = lambda **e: subprocess.run(["bash", "-c", 'eval "$(sed -n -e "/^cargo_max()/p" "$0")"; cargo_max', HEAVY],  # noqa: E731
                                    env=dict(env, **e), capture_output=True, text=True).stdout.strip()
    ncpu = int(subprocess.run(["/usr/sbin/sysctl", "-n", "hw.ncpu"], capture_output=True, text=True).stdout)
    check(cm() == str(max(1, ncpu // 6)) and cm(HEAVY_CARGO_MAX="3") == "3", "cargo_max: max(1, cpus / 6), HEAVY_CARGO_MAX wins")
    rdir = os.path.join(home, "cr", ".bise", "run", "reserve")
    os.makedirs(rdir, exist_ok=True)
    me = os.getpid()
    for name, kind in (("heavy.lock.%d" % me, "cargo-warm"), ("heavy2.lock.%d" % me, "desktop"),
                       ("heavy3.lock.%d" % me, "full"), ("heavy4.lock.999999", "cargo-cold")):
        with open(os.path.join(rdir, name), "w") as fh:
            fh.write("3\t/x\t0\tcmd\t%s\n" % kind)
    cr = subprocess.run(["bash", "-c", 'eval "$(sed -n -e "/^cargo_running()/,/^}/p" -e "/^rfield()/p" "$0")"; cargo_running', HEAVY],
                        env=dict(env, HOME=os.path.join(home, "cr")), capture_output=True, text=True).stdout.strip()
    check(cr == "2", "cargo_running: the live cargo-warm and full jobs, not the desktop one nor a dead pid's: %r" % cr)
    check(measured("other", [3 * GB] * 10, src="") == "3", "the other kinds keep their free-drop rows")
    # any kind: its free-drop rows count until 10 own rows exist (src=own), then only the own ones
    def mixed(kind, free_rows, own_rows):
        with open(dtsv, "w") as fh:
            fh.writelines("1\t%s\t%d\t0\t0\t0\tx\n" % (kind, kb) for kb in free_rows)
            fh.writelines("1\t%s\t%d\t0\t0\t0\tx\tsrc=own\n" % (kind, kb) for kb in own_rows)
        fn = 'eval "$(sed -n -e "/^measured_gb()/,/^}/p" -e "/^p90_gb()/,/^}/p" "$0")"; measured_gb "$1"'
        return subprocess.run(["bash", "-c", fn, HEAVY, kind], env=dict(env, HOME=home), capture_output=True, text=True).stdout.strip()
    check(mixed("desktop", [21 * GB] * 20, [0] * 9) == "21", "a running desktop job: 9 own rows, its free-drop rows still count (21)")
    check(mixed("desktop", [21 * GB] * 20, [GB] * 10) == "1", "a running desktop job: 10 own rows at 1 GB: only they count (1)")
    # its kind from the tree's own plan (tests/gate_plan.py's JSON, as data): a fake tree
    gk = os.path.join(home, "gk", "tests")
    os.makedirs(gk, exist_ok=True)
    open(os.path.join(gk, "gate.sh"), "w").close()

    def kind_of(plan, cmd=None):
        p = dict({"pkgs": [], "hub_changed": 0, "bend_changed": 0, "core_gen": 0, "desktop": None}, **plan)
        p["flags"] = dict({"proto_ts": 0, "office": 0, "office_skills": 0, "crossterm_changed": 0}, **plan.get("flags", {}))
        with open(os.path.join(gk, "gate_plan.py"), "w") as fh:
            fh.write("print(%r)\n" % json.dumps(p))
        fn = 'eval "$(sed -n -e "/^gate_kind()/,/^}/p" -e "/^job_kind()/,/^}/p" "$0")"; job_kind "${@:1}"'
        return subprocess.run(["bash", "-c", fn, HEAVY] + (cmd or ["bash", os.path.join(gk, "gate.sh"), "quick"]),
                              cwd=home, env=dict(env, HOME=home), capture_output=True, text=True).stdout.strip()
    dsk = {"desktop": {"why": "x"}}
    check(kind_of(dsk) == "desktop" and kind_of({"flags": {"office": 1}}) == "desktop",
          "a quick gate with a desktop or office step and no cargo one: kind desktop")
    check(kind_of(dict(dsk, pkgs=["switchboard"])) == "cargo-cold" and kind_of(dict(dsk, bend_changed=1)) == "cargo-cold"
          and kind_of(dict(dsk, flags={"proto_ts": 1})) == "cargo-cold", "a rust, PROOF or proto step too: a cargo kind")
    check(kind_of({}) == "cargo-cold", "no heavy step (a light gate that queued anyway): a cargo kind, as before")
    check(kind_of(dsk, ["bash", os.path.join(gk, "gate.sh"), "full"]) == "full", "a full gate stays full")
    with open(os.path.join(gk, "gate_plan.py"), "w") as fh:
        fh.write("raise SystemExit(1)\n")
    check(kind_of.__call__ and subprocess.run(["bash", "-c", 'eval "$(sed -n -e "/^gate_kind()/,/^}/p" -e "/^job_kind()/,/^}/p" "$0")"; job_kind bash "$1" quick',
                                              HEAVY, os.path.join(gk, "gate.sh")], cwd=home, env=dict(env, HOME=home),
                                             capture_output=True, text=True).stdout.strip() == "cargo-cold",
          "a plan that fails: a cargo kind, as before")
    # a job that outgrows its learned reserve (1 GB here) is still stopped by
    # the floor: under HEAVY_DISK_STOP_GB (5) free, rc=28
    reserve("other", [0] * 20)
    open(freef, "w").write(str(40 * GB))
    r = subprocess.Popen([HEAVY, "sh", "-c", 'sleep 60 & wait', "x"], env=envr, stderr=subprocess.PIPE, text=True)
    check(wait_for(lambda: [f for f in os.listdir(os.path.join(run, "reserve")) if f.startswith("heavy")]), "the 1 GB job runs")
    check(open(os.path.join(run, "reserve", [f for f in os.listdir(os.path.join(run, "reserve")) if f.startswith("heavy")][0])).read().split("\t")[0] == "1",
          "its reserve is the learned 1 GB")
    open(freef, "w").write(str(3 * GB))
    try:
        _, err = r.communicate(timeout=20)
    except subprocess.TimeoutExpired:
        r.kill()
        _, err = r.communicate()
    check(r.returncode == 28 and "3 GB free" in err, "it wrote past its reserve: stopped at 3 GB free, rc=28: %r %r" % (r.returncode, err[-200:]))
    # its heavy.log line for the audit (review-gates m_37481): the free GB and each running job's counted share
    ds = [l.split("\t") for l in lines() if "\tdisk-stop\t" in l]
    check(ds and ds[-1][2] == "heavy.lock" and ds[-1][3] == "3" and ds[-1][4].startswith("heavy.lock.") and " other 0" in ds[-1][4],
          "rc 28: a disk-stop line in heavy.log (slot, 3 GB free, its share 'other 0'): %r" % (ds[-1:] or lines()[-2:]))
    os.remove(dtsv)
    # the total reserved is capped (HEAVY_RESERVE_TOTAL_MAX, 20): with 4 here,
    # a 10 GB job running, a 2 GB job needs 15 + 2 + 4 = 21 and runs at 22
    open(freef, "w").write(str(30 * GB))
    go_c = os.path.join(home, "go_cap")
    envc = dict(envr, HEAVY_RESERVE_TOTAL_MAX="4")
    big = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go_c],
                           env=dict(envc, HEAVY_RESERVE_GB="10"), stderr=subprocess.DEVNULL)
    check(wait_for(lambda: [f for f in os.listdir(os.path.join(run, "reserve")) if f.startswith("heavy")]), "the 10 GB job runs (cap)")
    # 18 GB free now, and its start mark set to 18 too: no drop counts as
    # written by it, its whole 10 GB stays reserved (uncapped: 22 needed)
    ran6 = os.path.join(home, "ran6")
    open(freef, "w").write(str(18 * GB))
    for f in os.listdir(os.path.join(run, "reserve")):
        p = os.path.join(run, "reserve", f)
        parts = open(p).read().split("\t")
        parts[2] = str(18 * GB)
        open(p, "w").write("\t".join(parts))
    r = subprocess.run([HEAVY, "sh", "-c", 'touch "$0"', ran6], env=envc, capture_output=True, text=True, timeout=30)
    check(r.returncode == 0 and os.path.exists(ran6), "the reserves capped at 4: a 2 GB job runs at 18 GB free (needs 16): %r" % r.stderr[-300:])
    open(go_c, "w").close()
    job_ends(big, 30)
    # the running jobs count what their kind measured (main m_37463), from 10
    # measured runs on (review-gates m_37660), else their flat guess: an
    # 'other' job (flat 2) beside a 2 GB job at 13 GB free (10 floor + 2):
    # 10 runs at 0 count 0 (it runs); none, or 9 runs at 0, count the flat 2
    # (it waits, 14); 10 runs at 3 GB count 3 (it waits, 15); 9 small runs and
    # one 6.5 GB script: the p90 is the second largest, 1, not 7 (it runs, 13)
    for measured, runs_now, what in (([0] * 10, True, "10 runs at 0: 0"), ([], False, "nothing measured: flat 2"),
                                     ([0] * 9, False, "9 runs at 0: under 10, flat 2"), ([3 * GB] * 10, False, "10 runs at 3 GB: 3"),
                                     ([GB // 10] * 9 + [int(6.5 * GB)], True, "9 small runs + one 6.5 GB: 1, not 7")):
        if os.path.exists(dtsv):
            os.remove(dtsv)
        if measured:
            with open(dtsv, "w") as fh:
                fh.writelines("1\tother\t%d\t0\t0\t0\tx\n" % m for m in measured)
        open(freef, "w").write(str(30 * GB))
        go_k = os.path.join(home, "go_kind")
        if os.path.exists(go_k):
            os.remove(go_k)
        big = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go_k], env=envr, stderr=subprocess.DEVNULL)
        check(wait_for(lambda: [f for f in os.listdir(os.path.join(run, "reserve")) if f.startswith("heavy")]), "the 'other' job runs")
        rf = [f for f in os.listdir(os.path.join(run, "reserve")) if f.startswith("heavy")][0]
        parts = open(os.path.join(run, "reserve", rf)).read().rstrip("\n").split("\t")
        check(parts[4] == "other", "its reserve file: kind other: %r" % parts)
        parts[2] = str(13 * GB)
        open(os.path.join(run, "reserve", rf), "w").write("\t".join(parts) + "\n")
        open(freef, "w").write(str(13 * GB))
        ran7 = os.path.join(home, "ran7")
        if os.path.exists(ran7):
            os.remove(ran7)
        # its own reserve named (2): the case is what the running job counts
        small = subprocess.Popen([HEAVY, "sh", "-c", 'touch "$0"', ran7], env=dict(envr, HEAVY_RESERVE_GB="2"), stderr=subprocess.DEVNULL)
        if runs_now:
            check(job_ends(small, 30) == 0 and os.path.exists(ran7), "a running 'other' job, %s: a 2 GB job runs at 13 GB free" % what)
        else:
            check(stays(lambda: not os.path.exists(ran7), 2, "it waits"), "a running 'other' job, %s: a 2 GB job waits at 13 GB free" % what)
            small.terminate()
            small.wait()
        open(go_k, "w").close()
        job_ends(big, 30)
    if os.path.exists(dtsv):
        os.remove(dtsv)
    # plenty again for the cases below (their waits are the load's, not the disk's)
    open(freef, "w").write(str(40 * GB))
    # the load (mac-budget): with a heavy job running, a new one waits while
    # the load is over HEAVY_LOAD_MAX; a lone job never waits for it
    loadf = os.path.join(home, "load")
    open(loadf, "w").write("50")
    envl = dict(envr, HEAVY_LOAD_FILE=loadf, HEAVY_LOAD_MAX="24")
    ran3 = os.path.join(home, "ran3")
    lone = subprocess.run([HEAVY, "sh", "-c", 'touch "$0"', ran3], env=envl, capture_output=True, timeout=30)
    check(lone.returncode == 0 and os.path.exists(ran3), "load 50, no heavy job: a lone job runs")
    go_l = os.path.join(home, "go_load")
    hold = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go_l], env=envl,
                            stderr=subprocess.DEVNULL)
    # its start line, not a probe of the lock (a probe can meet a waiter's own try)
    wait_for(lambda: any(l.split("\t")[2] == "-" and "go_load" in l for l in lines()), what="slot 1 held")
    ran4 = os.path.join(home, "ran4")
    e4 = open(os.path.join(home, "err4"), "w+")
    w4 = subprocess.Popen([HEAVY, "sh", "-c", 'touch "$0"', ran4], env=envl, stderr=e4)
    check(stays(lambda: not os.path.exists(ran4), 3, "it waits"), "load 50, a heavy job runs: the next one waits %r" % lines()[-3:])
    e4.seek(0)
    check("the machine is busy (load 50, the limit 24" in e4.read(), "the wait says the load")
    # the load gate keeps the queue's order (rank, then submit time) and
    # never holds the --prio lane (review-gates m_36096)
    lorder = os.path.join(home, "load_order")
    rest_a = subprocess.Popen([HEAVY, "sh", "-c", 'echo rest-a >> "$0"', lorder], env=dict(envl, HEAVY_SUBMIT="5000"),
                              stderr=subprocess.DEVNULL)
    rest_b = subprocess.Popen([HEAVY, "sh", "-c", 'echo rest-b >> "$0"', lorder], env=dict(envl, HEAVY_SUBMIT="5001"),
                              stderr=subprocess.DEVNULL)
    land2 = subprocess.Popen([HEAVY, "--land", "sh", "-c", 'echo land >> "$0"', lorder], env=dict(envl, HEAVY_SUBMIT="5002"),
                             stderr=subprocess.DEVNULL)
    r = subprocess.run([HEAVY, "--prio", "sh", "-c", 'echo "$HEAVY_SLOT"'], env=dict(envl, HEAVY_NO_LANE=""), capture_output=True,
                       text=True, timeout=30)
    check(r.returncode == 0 and r.stdout.startswith("heavy5.lock"), "load 50: a --prio job still runs in the lane: %r" % r.stdout)
    check(wait_for(lambda: len(os.listdir(q)) >= 4), "the held jobs wait with their tickets")
    open(loadf, "w").write("10")
    check(job_ends(w4, 30) == 0 and os.path.exists(ran4), "load 10: it runs")
    for j in (rest_a, rest_b, land2):
        job_ends(j, 60)
    # the load never starves a job: past HEAVY_LOAD_WAIT_MAX it goes
    open(loadf, "w").write("50")
    ran6 = os.path.join(home, "ran6")
    r = subprocess.run([HEAVY, "sh", "-c", 'touch "$0"', ran6], env=dict(envl, HEAVY_LOAD_WAIT_MAX="3"), capture_output=True,
                       text=True, timeout=60)
    check(r.returncode == 0 and os.path.exists(ran6) and "3 s at most" in r.stderr,
          "load 50 for good: the job goes after HEAVY_LOAD_WAIT_MAX: %r" % r.stderr[-300:])
    got = open(lorder).read().split() if os.path.exists(lorder) else []
    check(got == ["land", "rest-a", "rest-b"], "after the load wait, by rank then submit time: %r" % got)
    open(go_l, "w").close()
    job_ends(hold, 30)
    # the time limit: a job over HEAVY_MAX_MIN is stopped, rc=124
    r = subprocess.run([HEAVY, "sh", "-c", "sleep 60 & wait"], env=dict(envr, HEAVY_MAX_MIN="1", HEAVY_MAX_SEC="2"),
                       capture_output=True, text=True, timeout=40)
    check(r.returncode == 124 and "over its limit (HEAVY_MAX_MIN=1)" in r.stderr,
          "a job over its time limit is stopped, rc=124: %r %r" % (r.returncode, r.stderr[-200:]))
    check(lines()[-1].endswith("rc=124"), "heavy.log ends it with rc=124")
    # an --alone job that uses no CPU (main m_36953: a hung caffeinate held every slot 3 h at 0 %):
    # stopped past HEAVY_IDLE_MIN, rc=124, said; one that burns CPU runs on (0.1 s a window here: the
    # test machine may be loaded); a plain job never
    # (plenty of disk: envr carries the disk cases' low free space, an --alone job reserves 10 GB)
    roomy = dict(envr, HEAVY_FREE_KB_FILE=os.path.join(home, "plenty_kb"))
    idle = dict(roomy, HEAVY_IDLE_MIN="1", HEAVY_IDLE_SEC="4", HEAVY_IDLE_POLL="1", HEAVY_IDLE_CPU_CS="10", HEAVY_DISK_POLL="1")
    try:
        r = subprocess.run([HEAVY, "--alone", "sh", "-c", "sleep 60 & wait"], env=idle, capture_output=True, text=True, timeout=60)
    except subprocess.TimeoutExpired as e:
        r = subprocess.CompletedProcess(e.cmd, -1, "", (e.stderr or b"").decode() if isinstance(e.stderr, bytes) else (e.stderr or ""))
    check(r.returncode == 124 and "an --alone job (it holds every slot) used no CPU for 1 min" in r.stderr,
          "an idle --alone job is stopped, rc=124: %r %r" % (r.returncode, r.stderr[-300:]))
    burn = 'import time\nt = time.time()\nwhile time.time() - t < 8: pass'
    r = subprocess.run([HEAVY, "--alone", sys.executable, "-c", burn], env=idle, capture_output=True, text=True, timeout=60)
    check(r.returncode == 0, "a busy --alone job runs past HEAVY_IDLE_SEC: %r %r" % (r.returncode, r.stderr[-300:]))
    r = subprocess.run([HEAVY, "sh", "-c", "sleep 6 & wait"], env=idle, capture_output=True, text=True, timeout=60)
    check(r.returncode == 0, "a plain job is never stopped for idling: %r %r" % (r.returncode, r.stderr[-300:]))
    # the waiting line names the jobs holding the slots (slot, folder, minutes, command)
    go_h = os.path.join(home, "go_h")
    hw = os.path.join(home, "holder-wt")
    os.makedirs(hw)
    hold2 = [subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.2; done', go_h], env=roomy, cwd=hw) for _ in range(2)]
    wait_for(lambda: len([f for f in os.listdir(os.path.join(run, "reserve")) if not f.startswith(".")]) >= 2, 30, "two jobs hold both slots")
    eh = open(os.path.join(home, "errh"), "w+")
    wh = subprocess.Popen([HEAVY, "true"], env=roomy, stderr=eh)
    wait_for(lambda: "busy or promised" in open(eh.name).read(), 30, "the waiter's line")
    eh.seek(0)
    said = eh.read()
    check("held by: heavy" in said and " holder-wt 0 min: sh -c while" in said, "the waiting line names the holders: %r" % said)
    # HEAVY_DEADLINE (review-gates m_38056): still queued then, the job never starts; rc 75 and who
    # held the slots
    rand = os.path.join(home, "ran_deadline")
    r = subprocess.run([HEAVY, "sh", "-c", 'touch "$0"', rand], env=dict(roomy, HEAVY_DEADLINE=str(int(time.time()) + 2)),
                       capture_output=True, text=True, timeout=60)
    check(r.returncode == 75 and not os.path.exists(rand) and "heavy-run: no slot by " in r.stderr and " holder-wt " in r.stderr,
          "the slots held past HEAVY_DEADLINE: rc 75, no job, the holders named: %r %r" % (r.returncode, r.stderr[-300:]))
    r = subprocess.run([HEAVY, "sh", "-c", 'touch "$0"', rand], env=dict(roomy, HEAVY_DEADLINE="06:00"), capture_output=True, text=True, timeout=30)
    check(r.returncode == 2 and not os.path.exists(rand) and "HEAVY_DEADLINE wants an epoch" in r.stderr,
          "HEAVY_DEADLINE not an epoch: refused, rc 2: %r %r" % (r.returncode, r.stderr[-200:]))
    open(go_h, "w").close()
    for j in hold2 + [wh]:
        job_ends(j, 30)
    # an --alone job's time limit is the rest's (120 min); the nightly's stays 360
    check("*\" --alone \"*) echo 120" in open(HEAVY).read() and "*nightly_gate.sh*) echo 360" in open(HEAVY).read(),
          "max_min: --alone 120, the nightly 360")
    # the battery: on battery under 30 %, no job starts; plugged in, it runs
    ranb = os.path.join(home, "ranb")
    eb = open(os.path.join(home, "errb"), "w+")
    wb = subprocess.Popen([HEAVY, "sh", "-c", 'touch "$0"', ranb], env=dict(envr, HEAVY_ON_BATTERY="1", HEAVY_BATTERY_PCT="12"),
                          stderr=eb)
    check(stays(lambda: not os.path.exists(ranb), 2, "it waits"), "on battery at 12 %: the job waits")
    wb.kill()
    wb.wait()
    eb.seek(0)
    check("on battery at 12 %, under 30 %: plug the Mac in" in eb.read(), "the wait says plug it in")
    r = subprocess.run([HEAVY, "sh", "-c", 'touch "$0"', ranb], env=dict(envr, HEAVY_ON_BATTERY="1", HEAVY_BATTERY_PCT="80"),
                       capture_output=True, timeout=30)
    check(r.returncode == 0 and os.path.exists(ranb), "on battery at 80 %: it runs")
    # the per-slot threads, and an --alone job keeps the defaults
    r = subprocess.run([HEAVY, "sh", "-c", 'echo "$RUST_TEST_THREADS/$SB_TEST_JOBS/$CARGO_BUILD_JOBS"'],
                       env={k: v for k, v in envr.items() if k not in ("RUST_TEST_THREADS", "SB_TEST_JOBS", "CARGO_BUILD_JOBS")},
                       capture_output=True, text=True, timeout=30)
    check(r.stdout.strip() == "4/2/4", "a slot gets 4 test threads, 2 e2e jobs, 4 cargo jobs: %r" % r.stdout)
    # a reclaimed target: the owner's next job there says so, once
    wt = os.path.join(home, "wt")
    os.makedirs(os.path.join(wt, "rust", "target"))
    open(os.path.join(wt, "rust", "target", "RECLAIMED.txt"), "w").write("heavy-run: this cargo target was deleted at X\n")
    r = subprocess.run([HEAVY, "true"], env=envr, cwd=wt, capture_output=True, text=True, timeout=30)
    check("this cargo target was deleted at X" in r.stderr and not os.path.exists(os.path.join(wt, "rust", "target", "RECLAIMED.txt")),
          "the next job in that worktree prints RECLAIMED.txt and removes it: %r" % r.stderr[-300:])
    # the light lane: a quick gate whose tree runs no heavy step ('gate.sh light' exits 0) runs at
    # once with both slots held, its heavy.log lines on slot 'light'; a heavy one, or a quick gate
    # with more in its command, queues (a stand-in gate.sh: 'light' answers $LIGHT_RC)
    lw = os.path.join(home, "lw")
    os.makedirs(os.path.join(lw, "tests"))
    with open(os.path.join(lw, "tests", "gate.sh"), "w") as f:
        f.write('[ "$1" = light ] && exit "${LIGHT_RC:-0}"\necho "quick ran in $HEAVY_SLOT"\n')
    os.chmod(os.path.join(lw, "tests", "gate.sh"), 0o755)
    go_l2 = os.path.join(home, "go_l2")
    holds = [subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.2; done', go_l2], env=roomy) for _ in range(2)]
    wait_for(lambda: len([f for f in os.listdir(os.path.join(run, "reserve")) if not f.startswith(".")]) >= 2, 30, "both slots held")
    r = subprocess.run([HEAVY, "--land", "bash", "-c", "tests/gate.sh quick 2>&1 | tail -3"], env=roomy, cwd=lw,
                       capture_output=True, text=True, timeout=30)
    check(r.returncode == 0 and "quick ran in light " in r.stdout and "a light quick gate" in r.stderr,
          "a light quick gate runs with every slot held, HEAVY_SLOT='light <pid>': %r %r" % (r.stdout, r.stderr[-200:]))
    check([l.split("\t")[3] for l in lines() if "gate.sh quick" in l][-2:] == ["light", "light"] and lines()[-1].endswith("rc=0"),
          "heavy.log: its start and end on slot 'light'")
    for cmd, env_l, what in ((["tests/gate.sh", "quick"], dict(roomy, LIGHT_RC="1"), "a heavy tree"),
                             (["bash", "-c", "tests/gate.sh quick; cargo test"], roomy, "more than the quick gate"),
                             (["tests/gate.sh", "quick"], dict(roomy, HEAVY_NO_LIGHT="1"), "HEAVY_NO_LIGHT")):
        p = subprocess.Popen([HEAVY] + (["bash"] if cmd[0].endswith("gate.sh") else []) + cmd, env=env_l, cwd=lw,
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        check(stays(lambda: p.poll() is None, 2, "it waits"), "%s: the quick gate waits for a slot" % what)
        p.kill()
        p.wait()
    # the cap (review-gates m_37126): HEAVY_LIGHT_MAX light runs at once; the next waits, then runs
    go_l3 = os.path.join(home, "go_l3")
    lh = subprocess.Popen(["/usr/bin/lockf", "-k", os.path.join(run, "light1.lock"), "sh", "-c",
                           'while [ ! -e "$0" ]; do sleep 0.1; done', go_l3])
    wait_for(lambda: held(os.path.join(run, "light1.lock")), what="light1.lock held")
    el = open(os.path.join(home, "errl"), "w+")
    lp = subprocess.Popen([HEAVY, "bash", "tests/gate.sh", "quick"], env=dict(roomy, HEAVY_LIGHT_MAX="1"), cwd=lw,
                          stdout=subprocess.PIPE, stderr=el, text=True)
    check(stays(lambda: lp.poll() is None, 2, "it waits"), "the light cap held: the next light run waits")
    check("1 light quick gates run already: waiting for one to end" in open(el.name).read(), "the wait says so")
    open(go_l3, "w").close()
    check(job_ends(lp, 30) == 0 and "quick ran in light " in lp.stdout.read(), "the cap frees: it runs")
    job_ends(lh, 10)
    open(go_l2, "w").close()
    for j in holds:
        job_ends(j, 30)
    # a tree that turned heavy after the light check: gate.sh quick in a light run (HEAVY_SLOT 'light')
    # refuses every heavy step and runs none (the real gate.sh, in a throwaway worktree of this repo)
    repo = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    lt = os.path.join(home, "lt")
    if subprocess.run(["git", "-C", repo, "worktree", "add", "-q", "--detach", lt, "HEAD"], capture_output=True).returncode == 0:
        try:
            g = os.path.join(repo, "tests", "gate.sh")
            open(os.path.join(lt, "tests", "gate.sh"), "w").write(open(g).read())  # this tree's gate.sh, uncommitted or not
            with open(os.path.join(lt, "rust", "home", "src", "lib.rs"), "a") as f:
                f.write("\n// a change after the light check\n")
            envg = {k: v for k, v in roomy.items() if not k.startswith(("SB_", "GATE_", "CARGO_"))}
            r = subprocess.run(["bash", "tests/gate.sh", "quick"], cwd=lt, env=dict(envg, HEAVY_SLOT="light 1"),
                               capture_output=True, text=True, timeout=300)
            check(r.returncode == 1 and "FAIL heavy step in a light run: rust(" in r.stdout and "clippy" not in r.stdout
                  and "test-build" not in r.stdout, "a tree turned heavy: the light run refuses, no cargo: %r" % r.stdout[-400:])
        finally:
            subprocess.run(["git", "-C", repo, "worktree", "remove", "--force", lt], capture_output=True)
    # a job's command with newlines (bims-clay's node run.mts '<JSON>',
    # release-53 m_41368): its reserve file stays one line, and an older
    # heavy-run's multi-line file no longer fails every new job
    rdir = os.path.join(run, "reserve")
    os.makedirs(rdir, exist_ok=True)
    live = subprocess.Popen(["sleep", "60"])
    try:
        bad = os.path.join(rdir, "heavy9.lock.%d" % live.pid)
        open(bad, "w").write('2\t/x\t100\tnode run.mts [{"fn":\n"a"},\n{"fn":"b"}]\tother\n')
        r = subprocess.run([HEAVY, "sh", "-c", "true"], env=env, capture_output=True, text=True, timeout=60)
        check(r.returncode == 0 and "syntax error" not in r.stderr and "operand expected" not in r.stderr,
              "an old multi-line reserve file: a new job still runs: %r %r" % (r.returncode, r.stderr[-300:]))
        os.remove(bad)
        gon = os.path.join(home, "go_nl")
        p = subprocess.Popen([HEAVY, "sh", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', gon, '[{"fn":\n"a"}]'],
                             env=env, stderr=subprocess.DEVNULL)
        check(wait_for(lambda: [f for f in os.listdir(rdir) if f.startswith("heavy")], what="its reserve file"),
              "the job with newlines runs")
        mine = [f for f in os.listdir(rdir) if f.startswith("heavy")]
        text = open(os.path.join(rdir, mine[0])).read()
        check(text.count("\n") == 1 and '[{"fn": "a"}]' in text, "a command with newlines: its reserve file is one line: %r" % text)
        open(gon, "w").close()
        job_ends(p, 30)
        last = open(log).read().splitlines()[-1]
        check(last.count("\t") >= 6 and "rc=" in last, "its heavy.log end line is whole: %r" % last[-120:])
    finally:
        live.kill()
    # --install
    r = subprocess.run([HEAVY, "--install"], env=env, capture_output=True, text=True)
    dst = os.path.join(run, "heavy-run")
    check(r.returncode == 0 and os.access(dst, os.X_OK) and open(dst).read() == open(HEAVY).read(),
          "--install copies the script to ~/.bise/run/heavy-run")
    check(open(os.path.join(run, "disk_reclaim.py")).read() == open(os.path.join(HERE, "..", "scripts", "disk_reclaim.py")).read(),
          "--install puts disk_reclaim.py next to it")

sys.exit(1 if fails else 0)
