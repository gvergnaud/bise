#!/usr/bin/env python3
"""scripts/nightly_desktop.sh on fake data only.

A throwaway repo (one commit) with a copy of the script and tests/known_reds.*, fake
desktop-try.sh, cand.sh, desktop-dmg.sh and pnpm (NIGHTLY_DESKTOP_TRY, _CAND, _DMG, _PNPM) that
write what the real ones write (the candidate's json line, cand.sh's step JSONs, BREACH), a test
clock (NIGHTLY_DESKTOP_NOW), and a throwaway BISE_HOME. Checks: the steps' order and their
green / red / none, the exit (0, 1, 2), a red step of a cand.sh JSON named with its digits as N and
split by known_reds, a gate breach is red, nothing new after --until (none, never green), on
battery nothing runs, each run's line in desktop-runs.tsv (when, sha, verdict, out dir) and what a
tag reads (reds.txt: each red's name, test:land's by name; steps.tsv: each step's state), the
candidate it built kept for a rerun (the newest 2, removed under 20 GB free) and one built
before kept. Never his ~/.bise, no real build or app.

  python3 -B tests/nightly_desktop_test.py
"""
import datetime
import os
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FAILS = []

FAKE_TRY = """#!/usr/bin/env bash
echo "try $*" >> "$FAKE_LOG"
sha=$2; d="$FAKE_CAND_DIR/${sha:0:12}"
[ -n "${FAKE_BUILT_BEFORE:-}" ] && echo "bise try: candidate ${sha:0:8} is built: $d/app"
[ "${FAKE_TRY_RC:-0}" = 0 ] || { echo "cargo: error"; exit 1; }
mkdir -p "$d/app"; echo '{}' > "$d/cand.json"
# the clock moves past the end while it builds
[ -n "${FAKE_TRY_NOW:-}" ] && echo "$FAKE_TRY_NOW" > "$NIGHTLY_DESKTOP_NOW"
echo "{\\"sha\\": \\"$sha\\", \\"app\\": \\"$d/app\\", \\"dir\\": \\"$d\\"}"
"""

FAKE_CAND = """#!/usr/bin/env bash
o=$1; mode=full; [ "${AS_HE_DOES_ONLY:-}" = quick ] && mode=quick
echo "cand $mode $2 first=${AS_HE_DOES_FIRSTRUN:-} voice=${AS_HE_DOES_FAKE_VOICE:-} rows=${AS_HE_DOES_QUICK_ROWS:-}" >> "$FAKE_LOG"
mkdir -p "$o"
echo '{"counts": {"folders": 1}}' > "$o/before.json"
if [ "$mode" = "${FAKE_RED_MODE:-}" ]; then
  echo '{"mode": "x", "steps": [{"name": "fine", "ok": true}, {"name": "an answer to card 12 withdrawn", "ok": false}]}' > "$o/$mode.json"
else
  echo '{"mode": "x", "steps": [{"name": "fine", "ok": true}]}' > "$o/$mode.json"
fi
[ "$mode" = "${FAKE_BREACH_MODE:-}" ] && { echo "library first-open" > "$o/BREACH"; exit 3; }
exit 0
"""

FAKE_DMG = """#!/usr/bin/env bash
echo "dmg $*" >> "$FAKE_LOG"
[ -n "${FAKE_NO_SIGN:-}" ] && { echo "bise dmg: no stable signing identity in /x: a share build is never ad hoc" >&2; exit 1; }
echo "$FAKE_CAND_DIR/share/bise.dmg"
"""

FAKE_HOSTILE = """#!/usr/bin/env bash
echo "hostile" >> "$FAKE_LOG"
[ -n "${FAKE_HOSTILE_RED:-}" ] && { echo "FAIL every probe blocked: ['nested-blob-escape: reached']"; exit 1; }
echo "PASS mcp apps hostile"
"""

FAKE_PNPM = """#!/usr/bin/env bash
echo "pnpm $*" >> "$FAKE_LOG"
[ "$1" = test:land ] && [ -n "${FAKE_LAND_RED:-}" ] && { echo "NEW reds (not in tests/known_reds.txt): keep"; exit 1; }
exit 0
"""


def check(ok, what):
    if not ok:
        FAILS.append(what)


def write(path, text, mode=0o755):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)
    os.chmod(path, mode)


def at(hh, mm):
    """Today's epoch at hh:mm local."""
    d = datetime.datetime.now().replace(hour=hh, minute=mm, second=0, microsecond=0)
    return str(int(d.timestamp()))


def main():
    t = os.path.realpath(tempfile.mkdtemp(prefix="nightly-desktop-test-", dir=os.environ.get("TMPDIR")))
    try:
        repo, home = os.path.join(t, "harness"), os.path.join(t, "home")
        for d in (("apps", "desktop", "node_modules", "@playwright", "test"), ("apps", "desktop", "node_modules", "electron", "dist", "Electron.app"), ("scripts",), ("tests",)):
            os.makedirs(os.path.join(repo, *d))
        for f in ("nightly_desktop.sh", "cand_lock.py"):
            shutil.copy(os.path.join(ROOT, "scripts", f), os.path.join(repo, "scripts"))
        for f in ("known_reds.py", "known_reds.txt"):
            shutil.copy(os.path.join(ROOT, "tests", f), os.path.join(repo, "tests"))
        with open(os.path.join(repo, "tests", "known_reds.txt"), "a") as f:
            f.write("as-he-does/quick: an answer to card N withdrawn\tabc123\tgate-nightly\tthe fake's red\n")
        subprocess.run(["git", "init", "-q", "-b", "main", repo], check=True)
        subprocess.run(["git", "-C", repo, "-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "one"], check=True)
        sha = subprocess.run(["git", "-C", repo, "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
        fakes = os.path.join(t, "fakes")
        for n, body in (("try", FAKE_TRY), ("cand", FAKE_CAND), ("dmg", FAKE_DMG), ("pnpm", FAKE_PNPM), ("hostile", FAKE_HOSTILE)):
            write(os.path.join(fakes, n), body)
        clock = os.path.join(t, "now")

        def run(*args, **extra):
            log = os.path.join(t, "log")
            open(log, "w").close()
            with open(clock, "w") as f:
                f.write(extra.pop("NOW", at(4, 30)))
            out = os.path.join(t, "out")
            shutil.rmtree(out, ignore_errors=True)
            env = dict(os.environ, BISE_HOME=os.path.join(home, ".bise"), HOME=home, FAKE_LOG=log,
                       FAKE_CAND_DIR=os.path.join(t, "cand"), NIGHTLY_DESKTOP_NOW=clock, NIGHTLY_DESKTOP_BATTERY="0",
                       NIGHTLY_DESKTOP_TRY=os.path.join(fakes, "try"), NIGHTLY_DESKTOP_CAND=os.path.join(fakes, "cand"),
                       NIGHTLY_DESKTOP_DMG=os.path.join(fakes, "dmg"), NIGHTLY_DESKTOP_PNPM=os.path.join(fakes, "pnpm"),
                       NIGHTLY_DESKTOP_HOSTILE=os.path.join(fakes, "hostile"))
            env.update(extra)
            env.pop("BISE_SOCKET", None)
            r = subprocess.run(["bash", os.path.join(repo, "scripts", "nightly_desktop.sh"), out, *args], env=env,
                               capture_output=True, text=True, timeout=60)
            calls = open(log).read().splitlines()
            return r.returncode, r.stdout + r.stderr, calls

        runs_tsv = os.path.join(home, ".bise", "run", "nightly", "desktop-runs.tsv")
        out = os.path.join(t, "out")

        def last_run():
            """the last desktop-runs.tsv line's 4 columns: when, sha, verdict, out dir"""
            lines = open(runs_tsv).read().splitlines() if os.path.exists(runs_tsv) else []
            return lines[-1].split("\t") if lines else []

        def facts():
            """what a tag reads: reds.txt's names, steps.tsv's {step: state}"""
            reds = open(os.path.join(out, "reds.txt")).read().splitlines()
            steps = dict(l.split("\t", 1) for l in open(os.path.join(out, "steps.tsv")).read().splitlines())
            return reds, steps

        # all green: the steps by value, every quick row (no QUICK_ROWS) with the first run and the voice
        rc, o, calls = run()
        check(rc == 0, "green: rc %s\n%s" % (rc, o))
        kinds = [c.split()[0] + ("" if not c.startswith("cand") else " " + c.split()[1]) for c in calls]
        check(kinds == ["try", "cand quick", "pnpm", "pnpm", "hostile", "cand full", "dmg"], "green: the order %s" % kinds)
        check(any(c.startswith("cand quick") and "first=1 voice=1 rows=" in c and c.endswith("rows=") for c in calls), "green: quick args %s" % calls)
        lr = last_run()
        check(len(lr) == 4 and lr[1:] == [sha, "green", out], "green: desktop-runs.tsv's line %s" % lr)
        check(facts() == ([], {"candidate": "green", "quick": "green", "land": "green", "mcp-apps": "green", "full": "green", "dmg": "green"}), "green: reds.txt, steps.tsv %s" % (facts(),))
        check(not os.path.exists(os.path.join(home, ".bise", "run", "nightly", "desktop-green.tsv")), "green: no desktop-green.tsv any more")
        check(os.path.isdir(os.path.join(t, "cand", sha[:12])) and "kept the night's candidate" in o, "green: the candidate it built is kept for a rerun")
        check("STEP dmg: green" in o and "nightly desktop: green" in o, "green: summary\n" + o)

        # a red step in quick's JSON: digits as N, a known red with its owner; still red (exit 1)
        rc, o, _ = run(FAKE_RED_MODE="quick")
        check(rc == 1 and "FAIL quick" in o, "red row: rc %s\n%s" % (rc, o))
        check("known reds (tests/known_reds.txt): as-he-does/quick: an answer to card N withdrawn (gate-nightly" in o, "red row: known split\n" + o)
        check(last_run()[1:3] == [sha, "red"], "red row: its run line %s" % last_run())
        reds, steps = facts()
        check(reds == ["as-he-does/quick: an answer to card N withdrawn"] and steps.get("quick") == "red", "red row: reds.txt, steps.tsv %s" % (facts(),))
        rc, o, _ = run(FAKE_RED_MODE="full")
        check(rc == 1 and "NEW reds (not in tests/known_reds.txt): as-he-does/full: an answer to card N withdrawn" in o, "red full: NEW\n" + o)

        # a gate breach is red, the next steps still run
        rc, o, calls = run(FAKE_BREACH_MODE="quick")
        check(rc == 1 and "STEP quick: red (gate breach: library first-open)" in o and any(c.startswith("cand full") for c in calls), "breach\n" + o)

        # test:land red; the DMG with no signing identity is none, not red
        rc, o, _ = run(FAKE_LAND_RED="1")
        check(facts()[0] == ["keep"], "land red: its reds by name, not 'desktop/test:land' %s" % (facts(),))
        check(rc == 1 and "FAIL land" in o and "NEW reds" in o, "land red\n" + o)
        rc, o, _ = run(FAKE_NO_SIGN="1")
        check(rc == 2 and "STEP dmg: none (no signing identity" in o, "no sign: none\n" + o)

        # no candidate: red, and the app's steps are none, test:land still runs
        rc, o, calls = run(FAKE_TRY_RC="1")
        check(rc == 1 and "FAIL candidate" in o and "STEP quick: none (no candidate)" in o and "STEP full: none (no candidate)" in o, "no cand\n" + o)
        check([c.split()[0] for c in calls] == ["try", "pnpm", "pnpm", "hostile"], "no cand: land and the hostile view still run %s" % calls)

        # the hostile view leaking: its own red, never known (known_reds.py refuses to list it)
        rc, o, _ = run(FAKE_HOSTILE_RED="1")
        check(rc == 1 and "FAIL mcp-apps" in o and "nested-blob-escape" in o, "hostile red: " + o)
        check(facts()[0] == ["desktop/mcp-apps-hostile"] and facts()[1].get("mcp-apps") == "red", "hostile red: reds.txt, steps.tsv %s" % (facts(),))
        check("NEW reds (not in tests/known_reds.txt): desktop/mcp-apps-hostile" in o, "hostile red, NEW: " + o)

        # the hard end: the build ends past 08:00, nothing new starts: none, never green
        rc, o, calls = run("--until", "08:00", NOW=at(7, 50), FAKE_TRY_NOW=at(8, 1))
        check(rc == 2 and "STEP quick: none (past 08:00)" in o and "STEP dmg: none (past 08:00)" in o, "until\n" + o)
        check([c.split()[0] for c in calls] == ["try"], "until: nothing after the build %s" % calls)
        check(last_run()[2] == "none" and facts()[1].get("quick") == "none", "until: a none run %s %s" % (last_run(), facts()))
        # a start after the hour ends at tomorrow's: everything runs
        rc, o, _ = run("--until", "08:00", NOW=at(9, 0))
        check(rc == 0, "until tomorrow\n" + o)

        # on battery nothing runs; a second run in a row with no result is red, naming the last green
        rc, o, calls = run(NIGHTLY_DESKTOP_BATTERY="1")
        check(rc == 2 and calls == [] and "STEP candidate: none (on battery)" in o, "battery\n" + o)
        rc, o, calls = run(NIGHTLY_DESKTOP_BATTERY="1")
        check(rc == 1 and "FAIL desktop-runs" in o and "hasn't run since " in o and "on %s" % sha in o, "battery twice: red\n" + o)
        check("last green run: " in o and "none yet" not in o, "battery twice: the last green named\n" + o)
        rc, o, _ = run()
        check(rc == 0, "a green run after: green\n" + o)
        rc, o, _ = run(NIGHTLY_DESKTOP_BATTERY="1")
        check(rc == 2, "one none after a green: no result, not red\n" + o)

        # the newest 2 the nightly built stay (cands.txt), an older one goes; another agent's candidate
        # (not in cands.txt) and one a process runs from always stay; under 20 GB free the night's goes
        cands_txt = os.path.join(home, ".bise", "run", "nightly", "cands.txt")
        cd = lambda n: os.path.join(t, "cand", n)
        for n in ("aaaaaaaaaaaa", "bbbbbbbbbbbb", "cccccccccccc", "oooooooooooo"):
            os.makedirs(cd(n), exist_ok=True)
            open(os.path.join(cd(n), "cand.json"), "w").write("{}")
        # cccc: a process runs from it (sleep through a symlink under its dir: a copy of a system
        # binary is killed at its first exec here)
        os.symlink("/bin/sleep", os.path.join(cd("cccccccccccc"), "sleep"))
        busy = subprocess.Popen([os.path.join(cd("cccccccccccc"), "sleep"), "60"])
        try:
            with open(cands_txt, "w") as f:
                f.write("".join(cd(n) + "\n" for n in ("cccccccccccc", "aaaaaaaaaaaa", "bbbbbbbbbbbb")))
            shutil.rmtree(cd(sha[:12]), ignore_errors=True)
            rc, o, _ = run()
        finally:
            busy.kill()
            busy.wait()
        left = sorted(os.listdir(os.path.join(t, "cand")))
        want = sorted(["bbbbbbbbbbbb", "cccccccccccc", "oooooooooooo", sha[:12]])
        check(rc == 0 and left == want and "removed the older candidate %s" % cd("aaaaaaaaaaaa") in o, "keep 2: %s\n%s" % (left, o))
        check("kept the older candidate %s: pid %d runs from it" % (cd("cccccccccccc"), busy.pid) in o, "keep 2: the busy one kept\n" + o)
        check(open(cands_txt).read().split() == [cd("cccccccccccc"), cd("bbbbbbbbbbbb"), cd(sha[:12])], "keep 2: cands.txt %s" % open(cands_txt).read())
        for n in ("bbbbbbbbbbbb", "cccccccccccc", "oooooooooooo"):
            shutil.rmtree(cd(n), ignore_errors=True)
        shutil.rmtree(cd(sha[:12]), ignore_errors=True)
        rc, o, _ = run(NIGHTLY_DESKTOP_FREE_GB="12")
        check(rc == 0 and not os.path.exists(cd(sha[:12])) and "12 GB free" in o, "low disk: removed\n" + o)

        # a candidate built before (another agent's) is kept
        rc, o, _ = run(FAKE_BUILT_BEFORE="1")
        check(rc == 0 and os.path.isdir(os.path.join(t, "cand", sha[:12])), "built before: kept\n" + o)
    finally:
        shutil.rmtree(t, ignore_errors=True)
    for f in FAILS:
        print("FAIL", f)
    print("nightly_desktop_test: %s" % ("ok" if not FAILS else "%d failed" % len(FAILS)))
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
