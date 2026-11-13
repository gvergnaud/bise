#!/usr/bin/env python3
"""scripts/nightly_gate.sh's start path, on fake data only.

A throwaway repo (branch main) with a copy of the script and a fake tests/gate.sh, a fake
heavy-run (NIGHTLY_HEAVY_RUN) that can do what a night does while the run waits for its
slot (remove the tree like the hub's start sweep did on 2026-10-13, land a commit on main,
fail), and a throwaway BISE_HOME. Checks: the tree is made in the slot at main's tip of
that moment (~/.bise/run/nightly/tree/<repo>, its owner file), a removed tree is made
again, a stale tree is checked out and cleaned, `last` and `red` for green, red and a gate
that did not start (an infra failure, not a test red); the desktop steps after the gate (a
check red under coverage is a red night naming it, the coverage map and its audit are owned
tools that never turn it red). Never his ~/.bise, no real gate.

  python3 -B tests/nightly_gate_test.py
"""
import datetime
import os
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FAILS = []

FAKE_GATE = """#!/usr/bin/env bash
echo "fake gate $(git rev-parse --short HEAD) in $PWD GATE_ALL=${GATE_ALL:-} args=$*"
[ -n "${FAKE_GATE_FLAKY:-}" ] && echo "FLAKY e2e $FAKE_GATE_FLAKY (red, then green alone)"
[ "${FAKE_GATE_RC:-0}" = 0 ] && exit 0
echo "FAIL e2e_fake"; exit 1
"""

FAKE_HEAVY = """#!/usr/bin/env bash
# what happens while the run waits for its slot
echo "lockf: $HOME/.bise/run/heavy.lock: already locked"
[ -n "${FAKE_SWEEP:-}" ] && rm -rf "$FAKE_SWEEP"
[ -n "${FAKE_LAND:-}" ] && git -C "$FAKE_LAND" commit -q --allow-empty -m landed
[ -n "${FAKE_HEAVY_FAIL:-}" ] && { echo "heavy-run: $FAKE_HEAVY_FAIL"; exit 2; }
# the slots come at that time (the fake clock)
[ -n "${FAKE_SLOT_AT:-}" ] && echo "$FAKE_SLOT_AT" >"$NIGHTLY_NOW"
[ "${1:-}" = --alone ] && { echo "heavy-run: both slots"; shift; }
[ "${1:-}" = --land ] && { echo "heavy-run: rank 1, a land check"; shift; }
[ "${1:-}" = --prio ] && { echo "heavy-run: rank 0, the release lane${HEAVY_PASS_LANE:+, pass lane $HEAVY_PASS_LANE}"; shift; }
[ -n "${HEAVY_MAX_MIN:-}" ] && echo "heavy-run: max ${HEAVY_MAX_MIN} min"
[ -n "${HEAVY_DEADLINE:-}" ] && echo "heavy-run: deadline $HEAVY_DEADLINE"
# still queued at the deadline: heavy-run's own words, exit 75, no job
[ -n "${FAKE_NOSLOT:-}" ] && [ -n "${HEAVY_DEADLINE:-}" ] && { echo "heavy-run: no slot by 06:00: $FAKE_NOSLOT"; exit 75; }
exec "$@"
"""


# the coverage step (apps/desktop/tests/harness/coverage/covmap.mts): checks red or green, a
# tooling that never ran, a map refused (review-gates m_29723)
FAKE_COVMAP = """const [mode] = process.argv.slice(2);
const e = process.env;
import fs from 'node:fs';
if (e.FAKE_ARGS) fs.appendFileSync(e.FAKE_ARGS, 'covmap ' + process.argv.slice(2).join(' ') + '\\n');
const flag = (n) => { const i = process.argv.indexOf(n); return i >= 0 ? process.argv[i + 1] : ''; };
if (mode === 'checks') {
  // the night's end comes during the checks: two specs cut (cut.txt), the clock past the end
  if (e.FAKE_COV_CHECKS === 'cut') {
    const d = flag('--gate-dir') + '/desktop-cov-run';
    fs.mkdirSync(d, { recursive: true });
    fs.writeFileSync(d + '/cut.txt', 'checks/a.spec.ts\\nchecks/b.spec.ts\\n');
    fs.writeFileSync(d + '/report.json', '{}');
    fs.writeFileSync(e.NIGHTLY_NOW, String(Number(flag('--until')) + 300));
    console.log('covmap checks: 1/1 green; 2 not run: cut by the night end'); process.exit(3);
  }
  if (e.FAKE_COV_CHECKS === 'red') { console.log('FAIL keep-red'); process.exit(1); }
  // the hook broke: covmap.mts's checksVerdict reruns the checks without coverage for the verdict
  if (e.FAKE_COV_CHECKS === 'nohook') { console.log('covmap checks: 3/3 green (the coverage hook never took a coverage: this is the run without coverage; no map tonight)'); process.exit(0); }
  if (e.FAKE_COV_CHECKS === 'nohook-red') { console.log('FAIL keep-red'); console.log('covmap checks: 2/3 green (the coverage hook never took a coverage: this is the run without coverage; no map tonight)'); process.exit(1); }
  console.log('covmap checks: 3/3 green'); process.exit(0);
}
if (mode === 'map') {
  if (e.FAKE_COV_MAP === 'refuse') { console.error('covmap: no map written: bundles instead of source files'); process.exit(1); }
  console.log('covmap: map written'); process.exit(0);
}
process.exit(2);
"""
# the desktop app's full end-to-end and the recorded core: their argv, a none run past the end
FAKE_DESKTOP = """echo "desktop-full $*" >>"${FAKE_ARGS:-/dev/null}"
until="${3#@}"
[ -n "$until" ] && [ "$(cat "$NIGHTLY_DESKTOP_NOW")" -ge "$until" ] && { echo "STEP candidate: none (past)"; exit 2; }
exit 0
"""
FAKE_CORE = """echo "core-record" >>"${FAKE_ARGS:-/dev/null}"
exit 0
"""
FAKE_AUDIT = """import os, sys
print("fake audit")
sys.exit(1 if os.environ.get("FAKE_AUDIT_MISS") else 0)
"""


def at(hh, mm, days=0):
    """Today's epoch at hh:mm local, plus days."""
    d = datetime.datetime.now().replace(hour=hh, minute=mm, second=0, microsecond=0) + datetime.timedelta(days=days)
    return str(int(d.timestamp()))


def check(ok, what):
    if not ok:
        FAILS.append(what)


def git(repo, *args):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True, text=True).stdout.strip()


def main():
    t = tempfile.mkdtemp(prefix="nightly-test-", dir=os.environ.get("TMPDIR"))
    t = os.path.realpath(t)
    try:
        repo, home = os.path.join(t, "harness"), os.path.join(t, "home")
        os.makedirs(os.path.join(repo, "scripts"))
        os.makedirs(os.path.join(repo, "tests"))
        for f in ("nightly_gate.sh", "tip_blame.py"):
            shutil.copy(os.path.join(ROOT, "scripts", f), os.path.join(repo, "scripts"))
        with open(os.path.join(repo, "tests", "gate.sh"), "w") as f:
            f.write(FAKE_GATE)
        cov = os.path.join(repo, "apps", "desktop", "tests", "harness", "coverage")
        os.makedirs(cov)
        with open(os.path.join(cov, "covmap.mts"), "w") as f:
            f.write(FAKE_COVMAP)
        with open(os.path.join(repo, "tests", "desktop_pick_audit.py"), "w") as f:
            f.write(FAKE_AUDIT)
        with open(os.path.join(repo, "scripts", "nightly_desktop.sh"), "w") as f:
            f.write(FAKE_DESKTOP)
        with open(os.path.join(repo, "scripts", "core_record.sh"), "w") as f:
            f.write(FAKE_CORE)
        heavy = os.path.join(t, "heavy-run")
        with open(heavy, "w") as f:
            f.write(FAKE_HEAVY)
        os.chmod(heavy, 0o755)
        subprocess.run(["git", "init", "-q", "-b", "main", repo], check=True)
        git(repo, "config", "user.email", "t@example.invalid")
        git(repo, "config", "user.name", "t")
        git(repo, "config", "commit.gpgsign", "false")
        git(repo, "config", "core.hooksPath", "/dev/null")
        git(repo, "add", "-A")
        git(repo, "commit", "-q", "-m", "init")

        out = os.path.join(home, "run", "nightly")
        tree = os.path.join(out, "tree")
        wt = os.path.join(tree, "harness")
        # never the caller's night (NIGHTLY_END, NIGHTLY_NOW...): the nightly's gate runs this test
        # (TIP_RUN too: the tip gate's slot run carries it, and its gate runs this test)
        env0 = {k: v for k, v in os.environ.items() if not k.startswith(("GIT_", "FAKE_", "NIGHTLY_", "TIP_"))}
        clock, args_log = os.path.join(t, "now"), os.path.join(t, "args")
        env0.update(BISE_HOME=home, NIGHTLY_HEAVY_RUN=heavy, NIGHTLY_NOW=clock, FAKE_ARGS=args_log,
                    HEAVY_LOG=os.path.join(t, "heavy.log"))

        def night(NOW=None, **fake):
            # the fake clock: 01:30 today, when the launchd job queues the night
            with open(clock, "w") as f:
                f.write(NOW or at(1, 30))
            open(args_log, "w").close()
            env = dict(env0, **fake)
            p = subprocess.run(["bash", os.path.join(repo, "scripts", "nightly_gate.sh")], env=env,
                               capture_output=True, text=True, timeout=60)
            logs = sorted(f for f in os.listdir(out) if f.endswith(".log"))
            log = open(os.path.join(out, logs[-1])).read() if logs else ""
            last = open(os.path.join(out, "last")).read().strip() if os.path.exists(os.path.join(out, "last")) else ""
            red = open(os.path.join(out, "red")).read() if os.path.exists(os.path.join(out, "red")) else None
            return p.returncode, log, last, red

        def short():
            return git(repo, "rev-parse", "--short", "main")

        # 1. the first night: the tree made in the slot, the gate run in it with --all
        rc, log, last, red = night()
        check(rc == 0, "first night green: rc %s, log %r" % (rc, log))
        check(os.path.exists(os.path.join(wt, ".git")) and os.path.exists(os.path.join(tree, "owner")),
              "first night: the tree %s and its owner file" % wt)
        check("fake gate %s in %s GATE_ALL=1 args=full --all" % (short(), wt) in log, "first night: the gate ran in the tree: %r" % log)
        check(log.startswith("QUEUED ") and "\nSTART " in log, "first night: QUEUED then START: %r" % log)
        check("heavy-run: both slots" in log, "first night: heavy-run --alone, never beside an agent's job: %r" % log)
        check(last.split()[1:3] == [short(), "green"], "first night: last %r" % last)
        check(red is None, "first night: no red")
        check("LANE " not in log, "first night: no release job in the lane, no LANE line: %r" % log)
        # a release job in heavy-run's lane (heavy5.lock) during the night: the log says so
        with open(os.path.join(t, "heavy.log"), "w") as f:
            f.write("1\t%d\t-\theavy5.lock\t/x\tpython3 scripts/desktop_tag.py \n" % (int(time.time()) - 60))
        rc, log, last, red = night()
        check("LANE a release job ran in the lane during this night" in log, "a lane night: the LANE line: %r" % log)
        os.remove(os.path.join(t, "heavy.log"))

        # 2. the tree removed while the run waits (the hub's sweep), and a land on main
        rc, log, last, red = night(FAKE_SWEEP=tree, FAKE_LAND=repo)
        tip = short()
        check(rc == 0, "swept night green: rc %s, log %r" % (rc, log))
        check("fake gate %s in %s" % (tip, wt) in log, "swept night: the gate ran on main's new tip %s: %r" % (tip, log))
        check(last.split()[1:3] == [tip, "green"], "swept night: last %r" % last)
        check(wt in git(repo, "worktree", "list"), "swept night: the tree is a worktree again")

        # 3. a red night names the failing tests
        rc, log, last, red = night(FAKE_GATE_RC="1")
        check(rc != 0 and last.split()[2] == "red", "red night: rc %s, last %r" % (rc, last))
        check(red is not None and "e2e_fake" in red and "did not start" not in red, "red night: red %r" % red)
        # ... and who landed since the last green night: the tip-red file main forwards (review-gates m_36509)
        trf = os.path.join(out, "tip-red")
        tr = open(trf).read() if os.path.exists(trf) else ""
        check(tr.startswith("to: main\n") and "nightly red on main" in tr and "e2e_fake" in tr, "red night: tip-red %r" % tr)
        if os.path.exists(trf):
            os.remove(trf)

        # 4. a stale tree (an old commit, a stray file) is checked out at the tip and cleaned
        git(wt, "checkout", "-q", "--detach", "main~1")
        open(os.path.join(wt, "stray"), "w").write("x")
        rc, log, last, red = night()
        check(rc == 0 and last.split()[1:3] == [short(), "green"], "stale tree: rc %s, last %r" % (rc, last))
        check(not os.path.exists(os.path.join(wt, "stray")), "stale tree: the stray file is cleaned")
        check(red is None, "green night after red: red removed")

        # 5. heavy-run fails before the gate: an infra red, not a test red
        rc, log, last, red = night(FAKE_HEAVY_FAIL="boom")
        check(rc != 0 and last.split()[1:3] == ["?", "infra"] and "heavy-run: boom" in last, "heavy fails: rc %s, last %r" % (rc, last))
        check(red is not None and "did not start" in red and "infra" in red and "heavy-run: boom" in red, "heavy fails: red %r" % red)

        # the desktop steps after the gate (review-gates m_29723): a check red under coverage is a
        # product red named in red; the map and its audit are try-select's tools, never a red night
        rc, log, last, red = night()
        check(rc == 0 and "STEP desktop-checks: green" in log and "STEP desktop-map: green" in log
              and "STEP desktop-pick-audit: green" in log, "desktop steps green: rc %s, log %r" % (rc, log))
        rc, log, last, red = night(FAKE_COV_CHECKS="red")
        check(rc != 0 and last.split()[2] == "red" and red is not None and "keep-red" in red,
              "a check red under coverage: a red night naming it: rc %s, last %r, red %r" % (rc, last, red))
        check("STEP desktop-map: green" in log, "a red check: the map is still made: %r" % log)
        rc, log, last, red = night(FAKE_COV_MAP="refuse")
        check(rc == 0 and last.split()[2] == "green" and red is None
              and "STEP desktop-map: red (owner try-select; not a product red)" in log
              and "desktop-pick-audit" not in log.split("STEP desktop-map: red")[1],
              "covmap refuses: the gate's verdict, owned, no audit: rc %s, last %r, log %r" % (rc, last, log))
        rc, log, last, red = night(FAKE_AUDIT_MISS="1")
        check(rc == 0 and red is None and "STEP desktop-pick-audit: red (owner try-select" in log,
              "an audit miss: owned, the night green: rc %s, log %r" % (rc, log))
        rc, log, last, red = night(FAKE_COV_CHECKS="nohook")
        check(rc == 0 and red is None and "STEP desktop-checks: green" in log,
              "the hook broke, the checks green without it: a green night: rc %s, log %r" % (rc, log))
        rc, log, last, red = night(FAKE_COV_CHECKS="nohook-red", FAKE_COV_MAP="refuse")
        check(rc != 0 and red is not None and "keep-red" in red and "STEP desktop-map: red (owner try-select" in log,
              "the hook broke and a check is red: a red night naming it, the map lost (owned): rc %s, red %r" % (rc, red))
        rc, log, last, red = night(FAKE_GATE_RC="1", FAKE_COV_MAP="refuse")
        check(rc != 0 and red is not None and "e2e_fake" in red, "a red gate stays red whatever the map: %r" % red)

        # the night's end (review-gates m_32494): nothing new starts at or after 06:00
        end = at(6, 0)
        def calls():
            return open(args_log).read().splitlines()
        # a night that finishes: every step, by value (checks, map + audit, desktop-full,
        # core-record), each told the end
        rc, log, last, red = night()
        steps = [l.split(":")[0][5:] for l in log.splitlines() if l.startswith("STEP ") and ": start " in l]
        check(rc == 0 and red is None and len(last.split()) == 4 and "cut:" not in last,
              "a night that finishes: rc %s, last %r" % (rc, last))
        check(steps == ["desktop-checks", "desktop-map", "desktop-pick-audit", "desktop-full", "core-record"],
              "the order by value: %r" % steps)
        c = calls()
        check(any(x.startswith("covmap checks ") and "--until %s" % end in x for x in c)
              and "desktop-full %s/desktop-%s --until @%s" % (out, datetime.date.today().isoformat(), end) in c,
              "the end told to the checks and desktop-full: %r" % c)
        check("nothing new starts after %s 06:00" % datetime.date.today().isoformat() in log, "QUEUED names the end: %r" % log)
        # the end comes during the checks: the specs cut are none, nothing starts after them, the
        # slots are free; a first cut night is not red
        rc, log, last, red = night(FAKE_COV_CHECKS="cut")
        c = calls()
        check(rc == 0 and red is None and "STEP desktop-checks: none (cut by the night's end 06:00)" in log,
              "a night cut at 06:00: none, not red: rc %s, red %r, log %r" % (rc, red, log))
        check("STEP desktop-map: start" in log and any(x.startswith("covmap map ") for x in c)
              and "STEP desktop-pick-audit: none (past the night's end 06:00, not started)" in log,
              "cut: the map of the specs that ran still made, past the end; the audit not: %r %r" % (c, log))
        check("core-record" not in c and "STEP core-record: none (past the night's end 06:00, not started)" in log,
              "cut: core-record never starts: %r %r" % (c, log))
        check("STEP desktop-full: none (cut by the night's end 06:00)" in log
              and [l for l in log.split("STEP desktop-checks: none")[1].splitlines() if l.endswith(": green")] == ["STEP desktop-map: green"],
              "cut: desktop-full none, nothing green after the end: %r" % log)
        check(last.split()[2] == "green" and last.split()[4] == "cut:core-record,desktop-checks,desktop-full,desktop-pick-audit",
              "cut: last names what the end cut: %r" % last)
        hist = os.path.join(out, "checks-runs.tsv")
        check(open(hist).read().splitlines()[-1].split("\t")[2:] == ["cut", "2"], "cut: checks-runs.tsv %r" % open(hist).read())
        # the checks' verdict per full sha, for a release's desktop_tag.py (review-gates m_37177): cut is none
        cv = open(os.path.join(out, "checks-verdicts.tsv")).read().splitlines()
        check(len(cv[-1].split("\t")[1]) == 40 and cv[-1].split("\t")[2] == "none"
              and any(l.split("\t")[2] == "green" for l in cv), "checks-verdicts.tsv: a green night, then a cut one none: %r" % cv)
        # a second cut night in a row: red, desktop-checks-cut
        rc, log, last, red = night(FAKE_COV_CHECKS="cut")
        check(rc != 0 and last.split()[2] == "red" and red is not None and "desktop-checks-cut" in red,
              "two cut nights: red: rc %s, last %r, red %r" % (rc, last, red))
        # a night that finishes in between: the next cut is a first one again
        rc, log, last, red = night()
        check(rc == 0 and red is None, "a whole night after: green, red removed: rc %s" % rc)
        rc, log, last, red = night(FAKE_COV_CHECKS="cut")
        check(rc == 0 and red is None, "a cut night after a whole one: not red: rc %s, red %r" % (rc, red))
        rc, log, last, red = night()
        # still queued at the night's end (review-gates m_38056: 2026-10-23 queued 01:43, waiting at
        # 09:31): heavy-run gets the end as HEAVY_DEADLINE and gives up with who held the slots; the
        # night is a skip with that why, nothing ran, not red the first time
        rc, log, last, red = night(FAKE_NOSLOT="heavy5 release-49 300 min: desktop_tag d39283a8")
        check(rc == 3 and "fake gate" not in log and "START " not in log and "heavy-run: deadline " in log
              and "SKIP no slot by 06:00: heavy5 release-49 300 min: desktop_tag d39283a8" in log
              and last.split()[1:3] == ["?", "skip"] and "no slot by 06:00: heavy5 release-49" in last and red is None,
              "no slot by the end: a skip naming the holders: rc %s, last %r, red %r, log %r" % (rc, last, red, log))
        rc, log, last, red = night(FAKE_NOSLOT="heavy5 x")
        check(rc == 1 and red is not None and "two nights in a row" in red and "no slot by 06:00" in red,
              "a second no-slot night in a row: red: rc %s, red %r" % (rc, red))
        rc, log, last, red = night()
        check(rc == 0 and red is None, "a whole night after: green again: rc %s" % rc)
        # the slots come past the end (06:10): nothing runs, not even the gate; a none night, the
        # second in a row red
        rc, log, last, red = night(FAKE_SLOT_AT=at(6, 10))
        check(rc == 3 and "fake gate" not in log and "START " not in log and last.split()[1:3] == ["?", "none"]
              and "past the night's end 06:00" in last and red is None,
              "slots past the end: nothing runs, none: rc %s, last %r, red %r, log %r" % (rc, last, red, log))
        # a Mac asleep at 01:30 runs the job at its wake (07:00): that night's end is past
        # (its SKIP says why, from pmset's log: review-gates m_37144)
        pm = os.path.join(t, "pmset.log")
        with open(pm, "w") as f:
            f.write("2026-10-21 10:20:06 +0200 Wake                \tWake from Hibernate [CDNVA] : due to trackpadkeyboard Using BATT (Charge: 1%)\n")
        rc, log, last, red = night(NOW=at(7, 0), NIGHTLY_PMSET_LOG=pm)
        check(rc == 1 and "fake gate" not in log and red is not None and "two nights in a row" in red,
              "a wake at 07:00, the second none night: red: rc %s, red %r, log %r" % (rc, red, log))
        check(log.startswith("SKIP started at 07:00, past the night's end 06:00: the Mac was asleep") and "QUEUED" not in log
              and "Wake from Hibernate" in log and last.split()[1:3] == ["?", "skip"] and "Charge: 1%" in last,
              "a late start: a SKIP with its reason, no slot taken: log %r, last %r" % (log, last))
        # queued at 23:00: the end is tomorrow's 06:00
        rc, log, last, red = night(NOW=at(23, 0))
        check(rc == 0 and "covmap checks --gate-dir %s --until %s" % (os.path.join(home, "gate"), at(6, 0, 1)) in calls(),
              "queued at 23:00: tomorrow's 06:00: rc %s, %r" % (rc, calls()))
        # the DST night (2026-10-25 in Paris, a 25 h day): queued at 23:00 the 24th, the end is 06:00
        # CET by the calendar, not 05:00 (+ 86400: the tip red on ca58673f6, run on the 24th)
        rc, log, last, red = night(NOW="1792875600", TZ="Europe/Paris")
        check(rc == 0 and "covmap checks --gate-dir %s --until 1792904400" % os.path.join(home, "gate") in calls(),
              "the DST night: queued 2026-10-24 23:00 CEST, the end 2026-10-25 06:00 CET (1792904400): rc %s, %r" % (rc, calls()))
        # NIGHTLY_UNTIL empty (a run by hand in the day): no end
        rc, log, last, red = night(NOW=at(14, 0), NIGHTLY_UNTIL="")
        check(rc == 0 and red is None and not any("--until" in x for x in calls()), "no end: %r" % calls())
        # ...even under a caller's NIGHTLY_END (2026-10-20: this test, run by the night's own gate,
        # inherited that night's 06:00; its no-end night was past it and started nothing)
        rc, log, last, red = night(NOW=at(14, 0), NIGHTLY_UNTIL="", NIGHTLY_END=at(6, 0))
        check(rc == 0 and red is None and "fake gate" in log and not any("--until" in x for x in calls()),
              "no end, a caller's NIGHTLY_END ignored: rc %s, %r" % (rc, calls()))

        # launchd's PATH has no node (2026-10-18: desktop-checks and desktop-map 'node: No such
        # file', exit 127): the script puts fnm's default alias dir (FNM_DIR) first, so an
        # in-slot step finds node there
        bare = "/usr/bin:/bin:/usr/sbin:/sbin"
        fnm = os.path.join(t, "fnm")
        os.makedirs(os.path.join(fnm, "aliases", "default", "bin"))
        fake_node = os.path.join(fnm, "aliases", "default", "bin", "node")
        with open(fake_node, "w") as f:
            f.write("#!/bin/bash\necho \"fnm's node $*\"\n")
        os.chmod(fake_node, 0o755)
        rc, log, last, red = night(PATH=bare, FNM_DIR=fnm)
        check(rc == 0 and red is None and "fnm's node " in log and "STEP desktop-checks: green" in log,
              "no node on launchd's PATH: the steps find fnm's: rc %s, log %r" % (rc, log))
        # no node at all: the product step that cannot run is a red night naming it, the owned
        # tool's is no result (review-gates m_30808: 'no result' hid the harness checks)
        rc, log, last, red = night(PATH=bare, FNM_DIR=os.path.join(t, "no-fnm"))
        check(rc != 0 and last.split()[2] == "red" and "FAIL desktop-checks: cannot run (exit 127)" in log
              and red is not None and "desktop-checks " in red,
              "no node anywhere: desktop-checks red, cannot run: rc %s, last %r, red %r, log %r" % (rc, last, red, log))
        check("STEP desktop-map: no result (exit 127: cannot run)" in log and "FAIL desktop-map" not in log,
              "no node anywhere: the owned map is no result: %r" % log)
        rc, log, last, red = night()
        check(rc == 0 and red is None, "green again after: rc %s" % rc)

        # --tip: main's tip as a land check (heavy-run --land), once per sha, never on a green one,
        # whatever the load or another full gate (land first, test after: main m_36204)
        def tip_run(**fake):
            env = dict(env0, **fake)
            before = set(f for f in os.listdir(out) if f.startswith("tip-") and f.endswith(".log"))
            p = subprocess.run(["bash", os.path.join(repo, "scripts", "nightly_gate.sh"), "--tip"], env=env,
                               capture_output=True, text=True, timeout=60)
            new = sorted(set(f for f in os.listdir(out) if f.startswith("tip-") and f.endswith(".log")) - before)
            log = open(os.path.join(out, new[-1])).read() if new else ""
            lastf = os.path.join(out, "tip-last")
            return p.returncode, log, open(lastf).read().split() if os.path.exists(lastf) else []

        full = git(repo, "rev-parse", "main")
        gate_dir = os.path.join(home, "gate")
        os.makedirs(gate_dir, exist_ok=True)
        green = os.path.join(gate_dir, "green-shas.tsv")
        # 7. the tip already green: nothing runs
        with open(green, "w") as f:
            f.write("%s\t1\t600\tx\tlog\n" % full)
        rc, log, last = tip_run()
        check(rc == 0 and log == "", "tip green: no run (rc %s, log %r)" % (rc, log))
        os.remove(green)
        # the red nights above left their tip-red (main deletes it once forwarded)
        if os.path.exists(os.path.join(out, "tip-red")):
            os.remove(os.path.join(out, "tip-red"))
        # 8-10. another full gate holds its lock, the load is high: the tip gate runs all the same, in
        # one slot as a land check, in the tree with --all; tip-last says green
        lock = os.path.join(gate_dir, "full-%d.lock" % os.getuid())
        os.makedirs(lock)
        with open(os.path.join(lock, "owner"), "w") as f:
            f.write("%d x agent root\n" % os.getpid())
        rc, log, last = tip_run(TIP_GATE_MAX_LOAD="0")
        shutil.rmtree(lock)
        check(rc == 0 and "fake gate %s in %s GATE_ALL=1 args=full --all" % (short(), wt) in log, "tip, busy: the gate ran: rc %s, %r" % (rc, log))
        check("heavy-run: rank 1, a land check" in log and "heavy-run: both slots" not in log, "tip: --land, one slot: %r" % log)
        check(last[:2] == [full, "green"], "tip: tip-last %r" % last)
        check(not os.path.exists(os.path.join(out, "tip-red")), "tip green: no tip-red")
        check("desktop-full" not in log or ("STEP desktop-full: none (a tip run" in log and "desktop-full start" not in log),
              "tip: no desktop-full end-to-end (the nightly and a release run it): %r" % log)
        check("STEP desktop-checks: none (a tip run" in log and "STEP core-record: none (a tip run" in log and "covmap checks" not in log,
              "tip: no desktop checks nor core-record (the nightly runs them): %r" % log)
        check("heavy-run: max 120 min" in log, "tip: a wall cap of 120 min (TIP_MAX_MIN): %r" % log)
        # --tip --sha <sha> (a release pass's lane A, plan A step 7): that sha, pinned, though main
        # moved on; in the release lane; tried already (tip-last): it runs again
        git(repo, "commit", "-q", "--allow-empty", "-m", "after the pin")
        before = set(f for f in os.listdir(out) if f.startswith("tip-") and f.endswith(".log"))
        p = subprocess.run(["bash", os.path.join(repo, "scripts", "nightly_gate.sh"), "--tip", "--sha", full],
                           env=dict(env0, HEAVY_PASS_LANE="A"), capture_output=True, text=True, timeout=60)
        new = sorted(set(f for f in os.listdir(out) if f.startswith("tip-") and f.endswith(".log")) - before)
        log = open(os.path.join(out, new[-1])).read() if new else ""
        started = [l.split()[-1] for l in log.splitlines() if l.startswith("START ")]
        check(p.returncode == 0 and started and full.startswith(started[0]) and short() != started[0],
              "--tip --sha: the pinned sha ran, not main's tip: rc %s, %r %r" % (p.returncode, log[-400:], p.stderr[-200:]))
        check("heavy-run: rank 0, the release lane, pass lane A" in log, "--tip --sha: in the release lane, its pass lane: %r" % log[-400:])
        check(open(os.path.join(out, "tip-last")).read().split()[:2] == [full, "green"], "--tip --sha: tip-last names the pinned sha")
        # each tip run typed in full-runs.tsv (release_reuse.py's R, plan A step 8): the fake gate
        # names no e2e/tmux set, so 'partial', never an R
        fr = open(os.path.join(out, "full-runs.tsv")).read().splitlines()[-1].split("\t")
        check(fr[0] == full and fr[2] == "0" and fr[3] == "partial", "--tip --sha: its full-runs.tsv line: %r" % fr)
        # TIP_FULL (the full check after a by-reuse tag): a by-reuse green line doesn't count
        with open(green, "w") as f:
            f.write("%s\t1\t600\tx\tlog\tby-reuse=abc\n" % full)
        p = subprocess.run(["bash", os.path.join(repo, "scripts", "nightly_gate.sh"), "--tip", "--sha", full],
                           env=env0, capture_output=True, text=True, timeout=60)
        check(p.returncode == 0 and "green already: " in p.stdout, "--tip --sha: green by reuse is green for a cut: %r" % p.stdout)
        p = subprocess.run(["bash", os.path.join(repo, "scripts", "nightly_gate.sh"), "--tip", "--sha", full],
                           env=dict(env0, TIP_FULL="1"), capture_output=True, text=True, timeout=60)
        check(p.returncode == 0 and "green already" not in p.stdout, "TIP_FULL: a by-reuse line doesn't count, it runs: %r" % p.stdout)
        os.remove(green)
        p = subprocess.run(["bash", os.path.join(repo, "scripts", "nightly_gate.sh"), "--tip", "--sha", "no-such-sha"],
                           env=env0, capture_output=True, text=True, timeout=60)
        check(p.returncode == 2 and "no commit 'no-such-sha'" in p.stderr, "--tip --sha: a bad sha, rc 2: %r" % p.stderr)
        # a night that never started (review-gates m_37144: the job disabled at 01:00): from 07:00 the
        # tip gate finds no log for the day and says why, once, in last and red
        today = os.path.join(out, datetime.date.today().isoformat() + ".log")
        if os.path.exists(today):
            os.remove(today)
        for f in ("red", "nostart-" + datetime.date.today().isoformat()):
            if os.path.exists(os.path.join(out, f)):
                os.remove(os.path.join(out, f))
        with open(clock, "w") as f:
            f.write(at(8, 0))
        tip_run(NIGHTLY_JOB_LOADED="0")
        lastn = open(os.path.join(out, "last")).read()
        redn = open(os.path.join(out, "red")).read() if os.path.exists(os.path.join(out, "red")) else ""
        check(lastn.split()[1:3] == ["?", "nostart"] and "is not loaded" in lastn and "did not start" in redn,
              "no nightly log at 08:00, the job not loaded: last %r, red %r" % (lastn, redn))
        os.remove(os.path.join(out, "red"))
        tip_run(NIGHTLY_JOB_LOADED="0")
        check(not os.path.exists(os.path.join(out, "red")), "the no-start is said once a day")
        # 11. the same sha again (tried): nothing runs; a red one is not retried either
        rc, log, last = tip_run()
        check(rc == 0 and log == "", "tip tried: no second run (rc %s, log %r)" % (rc, log))
        git(repo, "commit", "-q", "--allow-empty", "-m", "moved")
        jour = os.path.join(t, "journal.jsonl")
        with open(jour, "w") as f:
            f.write('{"msg":{"text":"✓ lander landed 1 commit on main (%s)"}}\n' % git(repo, "rev-parse", "--short", "main"))
        rc, log, last = tip_run(FAKE_GATE_RC="1", TIP_JOURNALS=jour)
        check(rc != 0 and last[:2] == [git(repo, "rev-parse", "main"), "red"], "tip red: rc %s, tip-last %r" % (rc, last))
        tr = open(os.path.join(out, "tip-red")).read() if os.path.exists(os.path.join(out, "tip-red")) else ""
        check(tr.startswith("to: lander main\n") and " moved (lander)" in tr,
              "tip red: tip-red names the lander and its commit: %r" % tr)
        check(not os.path.exists(os.path.join(out, "flaky-open")), "no FLAKY line: no flaky-open")
        rc, log, last = tip_run()
        check(log == "", "tip red: not retried on the same sha (%r)" % log)
        # a green tip with a flake: flaky-open, its test and its rate, to main (c5-tip-per-land)
        git(repo, "commit", "-q", "--allow-empty", "-m", "moved again")
        rc, log, last = tip_run(FAKE_GATE_FLAKY="flake_e2e", TIP_JOURNALS=jour)
        fo = open(os.path.join(out, "flaky-open")).read() if os.path.exists(os.path.join(out, "flaky-open")) else ""
        check(rc == 0 and last[1:2] == ["green"] and fo.startswith("to: main\n") and "flake_e2e (e2e): flaky in 0 of 0" in fo,
              "tip green with a flake: flaky-open %r (rc %s, last %r)" % (fo, rc, last))
        os.remove(os.path.join(out, "flaky-open"))

        # 6. main's tip has no tests/gate.sh: NOSTART, an infra red
        git(repo, "rm", "-q", "tests/gate.sh")
        git(repo, "commit", "-q", "-m", "no gate")
        rc, log, last, red = night()
        check(rc != 0 and last.split()[1:3] == ["?", "infra"] and "no tests/gate.sh" in last, "no gate: rc %s, last %r" % (rc, last))
        check(red is not None and "did not start" in red, "no gate: red %r" % red)
    finally:
        subprocess.run(["git", "-C", os.path.join(t, "harness"), "worktree", "prune"], capture_output=True)
        shutil.rmtree(t, ignore_errors=True)
    if FAILS:
        print("nightly_gate_test: FAIL\n  " + "\n  ".join(FAILS))
        return 1
    print("nightly_gate_test: ok (6 nights, the desktop steps, the night's end, a late start, a night not started, 5 tip runs)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
