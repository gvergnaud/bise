#!/usr/bin/env python3
"""scripts/desktop_tag.py: the one way to cut a desktop tag, on fake data only.

Its pure parts (a sha's last run in desktop-runs.tsv, the judge of a run against known_reds, the
mark line, the mark's check), the release body without the mark (desktop_feed), then a throwaway
repo, a throwaway BISE_HOME and a fake nightly (DESKTOP_TAG_NIGHTLY: writes its run line, reds.txt
and steps.tsv like nightly_desktop.sh) run under no heavy slot. The cases (review-gates m_30406):
a green run tags 'green'; a run with only owned reds tags 'owned-reds N (owners: …)' and prints the
open known-reds count; one red outside tests/known_reds.txt refuses, naming it; a step 'none'
refuses; known_reds is read AT THE SHA (a line added later in the working tree doesn't count); a
dirty checkout refuses; `check` refuses a lightweight tag, a tag without the mark and a mark for
another sha, passes both marks and prints the line. Never his ~/.bise, no push.

  python3 -B tests/desktop_tag_test.py
"""
import os
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import desktop_feed as F  # noqa: E402
import desktop_tag as T  # noqa: E402

FAILS = []
# a fake nightly: FAKE_REDS (names, '|'-separated) and FAKE_NONE (a step with no result)
FAKE_NIGHTLY = """#!/usr/bin/env bash
echo "nightly $*" >> "$FAKE_LOG"
out=$1 sha=$2; mkdir -p "$out" "$BISE_HOME/run/nightly"
v=green; : > "$out/reds.txt"
printf 'candidate\\tgreen\\nquick\\tgreen\\nland\\tgreen\\nfull\\tgreen\\ndmg\\t%s\\n' "${FAKE_NONE:-green}" > "$out/steps.tsv"
[ -n "${FAKE_NONE:-}" ] && v=none
if [ -n "${FAKE_REDS:-}" ]; then v=red; echo "$FAKE_REDS" | tr '|' '\\n' > "$out/reds.txt"; fi
printf '%s\\t%s\\t%s\\t%s\\n' "2026-10-18T05:00:00" "$sha" "$v" "$out" >> "$BISE_HOME/run/nightly/desktop-runs.tsv"
[ "$v" = green ] && exit 0 || exit 1
"""
# a fake covmap.mts checks (DESKTOP_TAG_CHECKS): FAKE_CHECK_REDS (names, '|'-separated), FAKE_CHECK_RC
FAKE_CHECKS = """#!/usr/bin/env bash
echo "checks $*" >> "$FAKE_LOG"
d="$3/desktop-cov-run"; mkdir -p "$d"
echo "${FAKE_CHECK_REDS:-}" | tr '|' '\\n' | grep . > "$d/reds.txt"
[ -n "${FAKE_CHECK_REDS:-}" ] && exit 1
exit "${FAKE_CHECK_RC:-0}"
"""
# a fake upgrade_paths.py run (DESKTOP_TAG_UPGRADES, update-safety A): FAKE_UP_RED (a path's tag,
# red), FAKE_UP_NET (a path's tag, not fetched)
FAKE_UPGRADES = """#!/usr/bin/env bash
echo "upgrades $*" >> "$FAKE_LOG"
out=$1; mkdir -p "$out"
printf 'desktop-v2026.10.1\\tgreen\\tswitched\\n' > "$out/verdicts.tsv"
[ -n "${FAKE_UP_RED:-}" ] && { printf '%s\\tred\\tthe switch passed but the hub runs /old, not the candidate\\n' "$FAKE_UP_RED" >> "$out/verdicts.tsv"; exit 1; }
[ -n "${FAKE_UP_NET:-}" ] && { printf '%s\\tnetwork\\tnetwork: HTTP 502\\n' "$FAKE_UP_NET" >> "$out/verdicts.tsv"; exit 3; }
exit 0
"""
# no signing prompt in a throwaway repo (his global config signs)
GIT_ENV = {"GIT_CONFIG_COUNT": "3", "GIT_CONFIG_KEY_0": "commit.gpgsign", "GIT_CONFIG_VALUE_0": "false",
           "GIT_CONFIG_KEY_1": "tag.gpgsign", "GIT_CONFIG_VALUE_1": "false",
           "GIT_CONFIG_KEY_2": "user.email", "GIT_CONFIG_VALUE_2": "t@t", "GIT_AUTHOR_NAME": "t", "GIT_COMMITTER_NAME": "t"}
KNOWN = "# known\nas-he-does/quick: row one\tabc\tnight-reds\tits why\nhistory-perf\tdef\tland-reds\tflaky\n"


def check(ok, what):
    if not ok:
        FAILS.append(what)


def raises(fn, words):
    try:
        fn()
        return "no error"
    except T.TagError as e:
        return None if words in str(e) else str(e)


def pure():
    tsv = "w1\taaa\tred\t/o1\nw2\tbbb\tgreen\t/o2\nw3\taaa\tgreen\t/o3\nbad line\n"
    check(T.last_run(tsv, "aaa") == {"when": "w3", "verdict": "green", "out": "/o3", "reuse": ""}, "last_run: the last line of that sha")
    # a recut's reused line (plan A step 8): read as the sha's run, never by the full check after the tag
    rtsv = tsv + "w4\tccc\tgreen\t/o3\tby-reuse=aaa\n"
    check(T.last_run(rtsv, "ccc") == {"when": "w4", "verdict": "green", "out": "/o3", "reuse": "aaa"}, "last_run: a by-reuse line names R")
    check(T.last_run(rtsv, "ccc", fresh=True) is None, "last_run fresh: a by-reuse line doesn't count")
    with tempfile.TemporaryDirectory() as d:
        nd = os.path.join(d, "run", "nightly")
        os.makedirs(nd)
        open(os.path.join(nd, "desktop-runs.tsv"), "w").write("w1\taaa\tgreen\t/oa\n")
        open(os.path.join(nd, "checks-verdicts.tsv"), "w").write("w1\taaa\tred\t/oc\n")
        got = T.reuse_verdicts(d, "ccc", "aaa", "w9")
        check(got == ["desktop-runs"], "reuse_verdicts: only R's GREEN verdicts stand (red checks, no upgrades run again): %r" % got)
        check(T.last_run(open(os.path.join(nd, "desktop-runs.tsv")).read(), "ccc") == {"when": "w9", "verdict": "green", "out": "/oa", "reuse": "aaa"},
              "reuse_verdicts: N's line points at R's run, typed by-reuse")
        check(T.reuse_verdicts(d, "ccc", "aaa", "w10") == [], "reuse_verdicts: N's own line already there: nothing added")
    check(T.last_run(tsv, "aa") is None and T.last_run("", "aaa") is None, "last_run: exact sha only")
    known = {"r1": ("s", "night-reds", "w"), "r2": ("s", "land-reds", "w")}
    ok = {"candidate": "green", "quick": "red", "dmg": "green"}
    check(T.judge("green", ok, [], known) == ("green", []), "judge: green")
    check(T.judge("red", ok, ["r1", "r2"], known) == ("owned-reds", [("r1", "night-reds"), ("r2", "land-reds")]), "judge: owned")
    check(raises(lambda: T.judge("red", ok, ["r1", "r9"], known), "outside tests/known_reds.txt at that sha: r9") is None, "judge: outside")
    check(raises(lambda: T.judge("red", dict(ok, dmg="none"), ["r1"], known), "no result: dmg") is None, "judge: a none step")
    check(raises(lambda: T.judge("none", ok, [], known), "didn't run every step") is None, "judge: verdict none")
    check(raises(lambda: T.judge("red", ok, [], known), "no red named") is None, "judge: red with no name")
    g = T.mark_line("green", [], "abc", "w")
    o = T.mark_line("owned-reds", [("r1", "night-reds"), ("r2", "land-reds"), ("r3", "night-reds")], "abc", "w")
    check(g == "nightly-desktop: green abc w", g)
    check(o == "nightly-desktop: owned-reds 3 abc w (owners: land-reds, night-reds)", o)
    m = T.message("- one\n- two\n", o)
    check(m == "- one\n- two\n\n" + o + "\n", "message: %r" % m)
    check(T.marked(m, "abc") == o and T.marked(T.message("", g), "abc") == g, "marked: both kinds")
    check(T.marked(m, "ab") is None and T.marked("- one\n", "abc") is None, "marked: exact sha")
    check(T.marked("say nightly-desktop: green abc w", "abc") is None, "marked: a line of its own")
    check(F.notes_from_tag_message(m, "t") == "- one\n- two\n", "the release body drops the owned-reds mark")
    check(F.notes_from_tag_message(T.message("- one", g), "t") == "- one\n", "the release body drops the green mark")
    check(F.notes_from_tag_message(g + "\n", "desktop-v2026.10.1") == "bise for Mac, desktop-v2026.10.1.\n", "the mark alone: the stand-in body")


def repo_flow():
    t = os.path.realpath(tempfile.mkdtemp(prefix="desktop-tag-test-", dir=os.environ.get("TMPDIR")))
    saved = {k: os.environ.get(k) for k in GIT_ENV}
    os.environ.update(GIT_ENV)
    saved_heavy = T.heavy_argv
    try:
        repo, bh, log = os.path.join(t, "repo"), os.path.join(t, "home", ".bise"), os.path.join(t, "log")
        subprocess.run(["git", "init", "-q", "-b", "main", repo], check=True)
        g = lambda *a: subprocess.run(["git", "-C", repo, *a], check=True, capture_output=True, text=True, stdin=subprocess.DEVNULL).stdout.strip()  # noqa: E731
        os.makedirs(os.path.join(repo, "tests"))
        with open(os.path.join(repo, "tests", "known_reds.txt"), "w") as f:
            f.write(KNOWN)
        g("add", "tests/known_reds.txt")
        g("commit", "-q", "-m", "one")
        sha = g("rev-parse", "HEAD")
        fake = os.path.join(t, "nightly")
        with open(fake, "w") as f:
            f.write(FAKE_NIGHTLY)
        fake_checks = os.path.join(t, "checks")
        with open(fake_checks, "w") as f:
            f.write(FAKE_CHECKS)
        fake_up = os.path.join(t, "upgrades")
        with open(fake_up, "w") as f:
            f.write(FAKE_UPGRADES)
        env = dict(os.environ, BISE_HOME=bh, DESKTOP_TAG_NIGHTLY=fake, DESKTOP_TAG_CHECKS=fake_checks, DESKTOP_TAG_UPGRADES=fake_up, FAKE_LOG=log)
        # no heavy slot in the test: the argv as it is
        prios = []  # a release's run asks heavy-run's release lane (--prio)
        T.heavy_argv = lambda cmd, lock, prio=False: (prios.append(prio), [c for c in cmd if c not in ("/usr/bin/caffeinate", "-is")])[1]
        runs = os.path.join(bh, "run", "nightly", "desktop-runs.tsv")
        cruns = os.path.join(bh, "run", "nightly", "checks-verdicts.tsv")
        uruns = os.path.join(bh, "run", "nightly", "upgrade-verdicts.tsv")
        calls = lambda: open(log).read().splitlines() if os.path.exists(log) else []  # noqa: E731
        nightlies = lambda: [c for c in calls() if c.startswith("nightly ")]  # noqa: E731

        def cut(tag, **extra):
            for f in (runs, cruns, uruns):
                if os.path.exists(f):
                    os.remove(f)
            try:
                return T.cut(tag, None, "- the notes\n", repo=repo, env=dict(env, **extra))
            except T.TagError as e:
                return "ERR " + str(e)

        check("desktop-vYYYY.M.N" in cut("v2026.10.1"), "a bad tag name")
        # tip-green first (main m_36204): a sha the tip gate never passed is refused before any run,
        # unless main names it (--not-tip-green "<why>": said, the release log keeps it)
        r = cut("desktop-v2026.10.1")
        check(r.startswith("ERR refused: %s is not tip-green" % sha[:12]) and calls() == [], "not tip-green: refused, no run: %s %s" % (r, calls()))
        try:
            T.cut("desktop-v2026.10.1", None, "", repo=repo, env=dict(env, FAKE_REDS="as-he-does/full: row nine"), why="main: a hotfix")
        except T.TagError as e:
            r = str(e)
        check("1 red(s) outside" in r, "--not-tip-green: past the tip check to the run: %s" % r)
        # --checks-only (a release pass's lane B, plan A step 7): not tip-green yet (lane A runs
        # beside), every verdict run and judged, no tag; a red is refused the same
        for f in (runs, cruns, uruns):
            if os.path.exists(f):
                os.remove(f)
        r = T.cut("desktop-v2026.10.1", None, "", repo=repo, env=dict(env), checks_only=True)
        check(r.startswith("nightly-desktop: green %s" % sha) and not g("tag", "-l"), "--checks-only: judged green, no tag: %s" % r)
        check(os.path.exists(cruns), "--checks-only: the checks' verdict is written for the tag's rerun")
        try:
            r = T.cut("desktop-v2026.10.1", None, "", repo=repo, env=dict(env, FAKE_CHECK_REDS="app-start"), checks_only=True)
        except T.TagError as e:
            r = "ERR " + str(e)
        check(r.startswith("nightly-desktop: green") or "app-start" in r or r.startswith("ERR"), "--checks-only reads the run's verdicts: %s" % r)
        check(not g("tag", "-l"), "--checks-only: still no tag")
        os.makedirs(os.path.join(bh, "gate"), exist_ok=True)
        with open(os.path.join(bh, "gate", "green-shas.tsv"), "w") as f:
            f.write("%s\t1\t1\ttip\tlog\n" % sha)
        # one red outside the file: refused, naming it; no tag
        r = cut("desktop-v2026.10.1", FAKE_REDS="as-he-does/quick: row one|as-he-does/full: row nine")
        check(r.startswith("ERR") and "1 red(s) outside" in r and "as-he-does/full: row nine" in r, "outside: %s" % r)
        check(not g("tag", "-l"), "outside: no tag")
        # a step with no result: refused
        r = cut("desktop-v2026.10.1", FAKE_NONE="none")
        check(r.startswith("ERR") and "no result: dmg" in r, "none: %s" % r)
        # a line added in the working tree only: the checkout is dirty, refused before any run
        with open(os.path.join(repo, "tests", "known_reds.txt"), "a") as f:
            f.write("as-he-does/full: row nine\tx\tnight-reds\tlater\n")
        open(log, "w").close()
        r = cut("desktop-v2026.10.1", FAKE_REDS="as-he-does/full: row nine")
        check(r.startswith("ERR") and "clean" in r and calls() == [], "dirty: %s %s" % (r, calls()))
        # known_reds read AT THE SHA: the line committed in a later sha, that sha's run is owned;
        # back on the first sha, the same red is outside
        g("commit", "-q", "-am", "two")
        sha2 = g("rev-parse", "HEAD")
        with open(os.path.join(bh, "gate", "green-shas.tsv"), "a") as f:
            f.write("%s\t2\t1\ttip\tlog\n" % sha2)
        r = cut("desktop-v2026.10.9", FAKE_REDS="as-he-does/full: row nine")
        check(r == "nightly-desktop: owned-reds 1 %s 2026-10-18T05:00:00 (owners: night-reds)" % sha2, "at sha2: %s" % r)
        g("tag", "-d", "desktop-v2026.10.9")
        g("checkout", "-q", sha)
        r = cut("desktop-v2026.10.1", FAKE_REDS="as-he-does/full: row nine")
        check(r.startswith("ERR") and "outside" in r, "at sha1, the later line doesn't count: %s" % r)
        # only owned reds: tagged 'owned-reds 2 (owners: …)', the count printed
        open(log, "w").close()
        r = cut("desktop-v2026.10.1", FAKE_REDS="as-he-does/quick: row one|history-perf")
        check(r == "nightly-desktop: owned-reds 2 %s 2026-10-18T05:00:00 (owners: land-reds, night-reds)" % sha, "owned: %s" % r)
        check(len(nightlies()) == 1, "owned: the nightly ran once %s" % calls())
        msg = g("tag", "-l", "--format=%(contents)", "desktop-v2026.10.1")
        check(msg.startswith("- the notes") and msg.endswith(r), "owned: the message %r" % msg)
        # a run already there for the sha: taken as it is, no new run; a green one tags 'green'
        os.makedirs(os.path.join(t, "o-green"))
        open(os.path.join(t, "o-green", "reds.txt"), "w").close()
        with open(os.path.join(t, "o-green", "steps.tsv"), "w") as f:
            f.write("candidate\tgreen\nquick\tgreen\nland\tgreen\nfull\tgreen\ndmg\tgreen\n")
        os.makedirs(os.path.dirname(runs), exist_ok=True)
        open(log, "w").close()
        try:
            with open(runs, "w") as f:
                f.write("2026-10-18T05:00:00\t%s\tgreen\t%s\n" % (sha, os.path.join(t, "o-green")))
            with open(cruns, "w") as f:
                f.write("2026-10-18T04:00:00\t%s\tgreen\t%s\n" % (sha, os.path.join(t, "o-green")))
            r = T.cut("desktop-v2026.10.2", None, "", repo=repo, env=env)
        except T.TagError as e:
            r = "ERR " + str(e)
        check(r == "nightly-desktop: green %s 2026-10-18T05:00:00" % sha and calls() == [], "green run taken: %s %s" % (r, calls()))
        check("exists" in cut("desktop-v2026.10.2"), "an existing tag")
        # the desktop checks on that exact sha (review-gates m_37177): a night's run there is taken;
        # none there, they run now (release lane) and their line is kept; a red outside known_reds,
        # a cut run (rc 3) and a red with no name refuse; an owned red goes in the owned count
        r = cut("desktop-v2026.10.6", FAKE_CHECK_REDS="app-start")
        check(r.startswith("ERR the desktop checks") and "outside tests/known_reds.txt at that sha: app-start" in r and not g("tag", "-l", "desktop-v2026.10.6"), "checks: a red outside refuses: %s" % r)
        check(any(c.startswith("checks checks --gate-dir ") for c in calls()), "checks: run when the sha has none: %s" % calls())
        check(open(cruns).read().split("\t")[1:3] == [sha, "red"], "checks: its line kept: %r" % open(cruns).read())
        r = cut("desktop-v2026.10.6", FAKE_CHECK_RC="3")
        check(r.startswith("ERR the desktop checks") and "didn't run every step" in r, "checks: a cut run refuses: %s" % r)
        r = cut("desktop-v2026.10.6", FAKE_CHECK_RC="1")
        check(r.startswith("ERR the desktop checks") and "no red named" in r, "checks: red with no name refuses: %s" % r)
        r = cut("desktop-v2026.10.6", FAKE_CHECK_REDS="history-perf")
        check(r == "nightly-desktop: owned-reds 1 %s 2026-10-18T05:00:00 (owners: land-reds)" % sha, "checks: an owned red: %s" % r)
        # the upgrade paths (update-safety A, marketing m_58864): beside the checks, in the release
        # lane; a broken path refuses the tag with its name; a release not fetched says network
        r = cut("desktop-v2026.10.7", FAKE_UP_RED="desktop-v2026.10.0")
        check(r.startswith("ERR the upgrade paths") and "desktop-v2026.10.0: the switch passed but the hub runs /old" in r and not g("tag", "-l", "desktop-v2026.10.7"), "upgrades: a red path refuses: %s" % r)
        check(any(c.startswith("upgrades ") for c in calls()) and open(uruns).read().split("\t")[1:3] == [sha, "red"], "upgrades: run, its line kept: %s" % calls())
        r = cut("desktop-v2026.10.7", FAKE_UP_NET="desktop-v2026.10.0")
        check(r.startswith("ERR the upgrade paths") and "(network)" in r and "HTTP 502" in r, "upgrades: network says so: %s" % r)
        os.makedirs(os.path.join(t, "o-checks-red"))
        with open(os.path.join(t, "o-checks-red", "reds.txt"), "w") as f:
            f.write("app-start\n")
        open(log, "w").close()
        try:
            with open(runs, "w") as f:
                f.write("2026-10-18T05:00:00\t%s\tgreen\t%s\n" % (sha, os.path.join(t, "o-green")))
            with open(cruns, "w") as f:
                f.write("2026-10-18T04:00:00\t%s\tred\t%s\n" % (sha, os.path.join(t, "o-checks-red")))
            with open(uruns, "w") as f:
                f.write("2026-10-18T04:00:00\t%s\tgreen\t%s\n" % (sha, os.path.join(t, "o-green")))
            r = T.cut("desktop-v2026.10.7", None, "", repo=repo, env=env)
        except T.TagError as e:
            r = "ERR " + str(e)
        check(r.startswith("ERR the desktop checks (2026-10-18T04:00:00)") and "app-start" in r and calls() == [], "checks: a night's red run is taken, not rerun: %s %s" % (r, calls()))
        # check: both cut tags pass and print their line; lightweight, no mark, another sha fail
        for tag in ("desktop-v2026.10.1", "desktop-v2026.10.2"):
            try:
                T.check(tag, repo=repo)
            except T.TagError as e:
                check(False, "check of %s: %s" % (tag, e))
        g("tag", "desktop-v2026.10.3")
        g("tag", "-a", "desktop-v2026.10.4", "-m", "- notes")
        g("tag", "-a", "desktop-v2026.10.5", "-m", "nightly-desktop: green %s w" % ("0" * 40))
        for tag, why in (("desktop-v2026.10.3", "lightweight"), ("desktop-v2026.10.4", "carries no"), ("desktop-v2026.10.5", "carries no")):
            check(raises(lambda: T.check(tag, repo=repo), why) is None, "check %s" % tag)
        check(prios and all(prios), "a release's run asks the release lane (--prio): %r" % prios)
    finally:
        T.heavy_argv = saved_heavy
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        shutil.rmtree(t, ignore_errors=True)


def tree_bise_env():
    """checks_run's world runs the tagged tree's own bise (tree_bise.sh), as the nightly: its env
    carries QA_BISE and AMBIENT_QA_APP_ROOT; a preset QA_BISE or a fake checks run is left alone."""
    t = os.path.realpath(tempfile.mkdtemp(prefix="desktop-tag-bise-", dir=os.environ.get("TMPDIR")))
    saved = T.heavy_argv
    try:
        T.heavy_argv = lambda cmd, lock, **kw: cmd
        repo, bh = os.path.join(t, "repo"), os.path.join(t, "home", ".bise")
        os.makedirs(os.path.join(repo, "scripts"))
        with open(os.path.join(repo, "scripts", "tree_bise.sh"), "w") as f:
            f.write('echo building >&2; mkdir -p "$CARGO_TARGET_DIR/debug"; touch "$CARGO_TARGET_DIR/debug/bise"; echo "$CARGO_TARGET_DIR/debug/bise"\n')
        env = {"PATH": os.environ["PATH"]}
        T.tree_bise(bh, repo, env)
        want = os.path.join(t, "target", "debug", "bise")
        check(env.get("QA_BISE") == want, "QA_BISE is the tree's bise, built beside the tree: %r" % env.get("QA_BISE"))
        check(env.get("AMBIENT_QA_APP_ROOT") == repo, "AMBIENT_QA_APP_ROOT is the tagged tree")
        env = {"PATH": os.environ["PATH"], "QA_BISE": "/given/bise"}
        T.tree_bise(bh, repo, env)
        check(env["QA_BISE"] == "/given/bise" and "AMBIENT_QA_APP_ROOT" not in env, "a preset QA_BISE is kept")
        env = {"PATH": os.environ["PATH"], "DESKTOP_TAG_CHECKS": "fake"}
        T.tree_bise(bh, repo, env)
        check("QA_BISE" not in env, "a fake checks run builds no bise")
        # a build that fails: no QA_BISE, the why said; checks_run runs no checks, its line is 'none'
        with open(os.path.join(repo, "scripts", "tree_bise.sh"), "w") as f:
            f.write('echo "cargo failed" >&2; exit 1\n')
        env = {"PATH": os.environ["PATH"]}
        why = T.tree_bise(bh, repo, env)
        check(why == "tree_bise.sh exit 1" and "QA_BISE" not in env, "a failed build sets no QA_BISE and says why: %r" % why)
        raises(lambda: T.checks_run(bh, "a" * 40, repo, {"PATH": os.environ["PATH"]}), "tree_bise.sh exit 1")
        tsv = open(os.path.join(bh, "run", "nightly", "checks-verdicts.tsv")).read()
        check("\t%s\tnone\t" % ("a" * 40) in tsv, "the failed build's run is 'none' in checks-verdicts.tsv: %r" % tsv)
    finally:
        T.heavy_argv = saved
        shutil.rmtree(t, ignore_errors=True)


def main():
    pure()
    tree_bise_env()
    repo_flow()
    for f in FAILS:
        print("FAIL", f)
    print("desktop_tag_test: %s" % ("ok" if not FAILS else "%d failed" % len(FAILS)))
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
