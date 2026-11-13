#!/usr/bin/env python3
"""scripts/release_reuse.py (release-speed plan A step 8): its pure decisions, then base/plan/app
on a throwaway repo with a fake full-runs.tsv and timings.tsv."""
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "scripts"))
import release_reuse as RR  # noqa: E402

FAILS = []


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what)
    if not ok:
        FAILS.append(what)


def pure():
    runs = RR.full_runs("a\t1\t0\tfull\tl\nb\t2\t1\tfull\tl\nc\t3\t0\tby-reuse=b\tl\nd\t4\t0\tpartial\tl\n")
    check([s for s, _, _ in runs] == ["a", "b", "c", "d"], "full_runs reads every line")
    # one hop: a by-reuse run is never R; a red full run is (its green rows stand)
    check(RR.base(runs, lambda s: True) == "b", "base: the newest FULL run, red or green, never a by-reuse or partial one")
    check(RR.base(runs, lambda s: s == "a") == "a", "base: only an ancestor of N")
    check(RR.base(runs, lambda s: False) is None, "base: none")
    check(RR.too_big([("1", "1", "x")] * 21, 20, 400) is not None, "caps: 21 files is too many")
    check(RR.too_big([("300", "101", "x")], 20, 400) is not None, "caps: 401 lines is too many")
    check(RR.too_big([("-", "-", "x.png")], 20, 400) is not None, "caps: a binary change is never small")
    check(RR.too_big([("200", "200", "x")] , 20, 400) is None, "caps: 400 lines in 1 file is within")
    rows = RR.row_verdicts("1\tabc123456\tt1\t0\t1\t0\t5\tgate-full\n1\tabc123456\tt2\t1\t1\t0\t5\tgate-full\n"
                           "1\tabc123456\tt2\t0\t1\t0\t5\tgate-quick\n1\tabc123456\tt3\t1\t1\t0\t5\tgate-full\n"
                           "2\tabc123456\tt3\t0\t1\t0\t5\tgate-full\n1\tfff000000\tt4\t0\t1\t0\t5\tgate-full\n", "abc123456789")
    check(rows == {"t1": "0", "t2": "1", "t3": "0"}, "row_verdicts: R's gate-full rows, the last one per test: %r" % rows)
    tests = ["t1", "t2", "t3", "t4"]
    pick = lambda c: ({"t1"}, "named")  # noqa: E731
    k, got = RR.plan_a(["tests/t1.py"], tests, rows, pick)
    # t1 picked; t2 red on R and t4 without a row on R run again; t3 green on R stands
    check(k == "REUSE" and got == ["t1", "t2", "t4"], "plan_a: the pick and what R didn't pass: %r %r" % (k, got))
    k, why = RR.plan_a(["x/shared.rs"], tests, rows, lambda c: (None, "x/shared.rs (shared code or no rule)"))
    check(k == "ALL" and "shared code" in why, "plan_a: an unmapped path is ALL")
    for p in ("tests/known_reds.txt", "tests/run_all.sh", "scripts/heavy-run", "scripts/nightly_gate.sh", "apps/desktop/tests/checks/a.ts"):
        k, why = RR.plan_a([p], tests, rows, pick)
        check(k == "ALL" and p in why, "plan_a: a change in the checks or the gate (%s) is ALL" % p)
    k, why = RR.plan_a(["tests/t1.py"], tests, {}, pick)
    check(k == "ALL", "plan_a: R with no row at all is ALL")
    for p in ("apps/desktop/src/App.tsx", "rust/tui/src/x.rs", "bend/hub/a.bend", "packaging/x.sh", "scripts/desktop-try.sh",
              "apps/desktop/tests/harness/driver.ts", "tests/known_reds.txt", "scripts/desktop_tag.py"):
        k, why = RR.plan_b([p])
        check(k == "ALL", "plan_b: %s runs the app side again (%s)" % (p, why))
    check(RR.plan_b(["tests/t1.py", "docs/x.md"]) == ("REUSE", None), "plan_b: a test fix keeps R's app verdicts")


def repo_cases():
    with tempfile.TemporaryDirectory() as d:
        repo, bh = os.path.join(d, "repo"), os.path.join(d, "home")
        os.makedirs(os.path.join(repo, "tests"))
        os.makedirs(os.path.join(bh, "run", "nightly"))
        os.makedirs(os.path.join(bh, "gate"))
        g = lambda *a: subprocess.run(["git", "-C", repo, *a], capture_output=True, text=True, check=True).stdout.strip()  # noqa: E731
        g("init", "-q", "-b", "main")
        g("config", "user.email", "t@t")
        g("config", "user.name", "t")
        g("config", "commit.gpgsign", "false")
        open(os.path.join(repo, "tests", "run_all.sh"), "w").write('TESTS="alpha_e2e beta_e2e"\n')
        open(os.path.join(repo, "tests", "alpha_e2e.py"), "w").write("# alpha\n")
        open(os.path.join(repo, "tests", "beta_e2e.py"), "w").write("# beta\n")
        g("add", "-A")
        g("commit", "-q", "-m", "r")
        r = g("rev-parse", "HEAD")
        open(os.path.join(repo, "tests", "alpha_e2e.py"), "a").write("# the fix\n")
        g("commit", "-q", "-am", "fix")
        n = g("rev-parse", "HEAD")
        env = dict(os.environ, BISE_HOME=bh, RELEASE_REUSE_REPO=repo)
        run = lambda *a: subprocess.run([sys.executable, os.path.join(HERE, "..", "scripts", "release_reuse.py"), *a],  # noqa: E731
                                        env=env, capture_output=True, text=True).stdout.strip()
        check(run("base", n) == "", "base: no full-runs.tsv, no R")
        open(os.path.join(bh, "run", "nightly", "full-runs.tsv"), "w").write("%s\t1\t1\tfull\tlog\n%s\t2\t0\tby-reuse=%s\tlog\n" % (r, n, r))
        check(run("base", n) == r, "base: R is the full run before N (N's own by-reuse line never)")
        open(os.path.join(bh, "gate", "timings.tsv"), "w").write("1\t%s\talpha_e2e\t1\t1\t0\t5\tgate-full\n1\t%s\tbeta_e2e\t0\t1\t0\t5\tgate-full\n" % (r[:9], r[:9]))
        out = run("plan", r, n)
        check(out == "REUSE %s alpha_e2e" % r, "plan: the fixed test runs, beta green on R stands: %r" % out)
        check(run("app", r, n) == "REUSE " + r, "app: a test fix keeps R's app verdicts")
        out = run("plan", n, r)
        check(out.startswith("ALL ") and "not an ancestor" in out, "plan: R after N is ALL: %r" % out)
        env["HEAVY_REUSE_LINES"] = "0"
        out = run("plan", r, n)
        check(out.startswith("ALL ") and "HEAVY_REUSE_LINES" in out, "plan: past the line cap is ALL: %r" % out)


pure()
repo_cases()
print("release_reuse_test: %s" % ("ok" if not FAILS else "%d FAILED" % len(FAILS)))
sys.exit(1 if FAILS else 0)
