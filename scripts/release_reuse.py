#!/usr/bin/env python3
"""release_reuse.py: a recut one fix away from a full run keeps that run's green verdicts
(release-speed plan A step 8, architect m_63312, marketing m_63313).

  scripts/release_reuse.py base <N>          R: the newest FULL tip run (full-runs.tsv, kind
                                              'full', never a by-reuse one: one hop) on an
                                              ancestor of N; nothing printed when none
  scripts/release_reuse.py plan <R> <N>      lane A (the tip gate): 'ALL <why>', or
                                              'REUSE <R> <test>...': the e2e/tmux tests to run
                                              (test_map's pick for R..N, and every test R didn't
                                              pass: a red, a missing row); R's green rows stand.
                                              The Rust part always runs (gate.sh full).
  scripts/release_reuse.py app <R> <N>       lane B (the app side): 'ALL <why>' or 'REUSE <R>':
                                              R's green app verdicts (desktop-runs, checks,
                                              upgrades) stand for N; R's others run again

Reuse needs: R an ancestor of N, R..N within HEAVY_REUSE_FILES (20) files and HEAVY_REUSE_LINES
(400) lines (heavy-run's header lists both), every path mapped (test_map: an unmapped one is
ALL). Lane A: a path of the checks or the gate (CHECK_PATHS) is ALL too. Lane B: no path the app
is built from (APP_PATHS) nor of its checks (CHECK_PATHS: a known_reds or harness change never
rides on R's verdicts): marketing m_63313 (b) asks for the touched checks again, this runs them
all. Only GREEN verdicts of R are kept (marketing m_63313 a).
Files: <BISE_HOME>/run/nightly/full-runs.tsv ('<sha>\\t<end>\\t<rc>\\t<kind>\\t<log>', kind 'full'
or 'by-reuse=<R>', nightly_gate.sh --tip writes it), <BISE_HOME>/gate/timings.tsv (run_all.sh's
rows: '<when>\\t<sha9>\\t<test>\\t<rc>...\\t<kind>'). Tests: tests/release_reuse_test.py.
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
REPO = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(REPO, "tests"))

# what the app is built from: a change there runs the app side again (architect m_63312)
APP_PATHS = ("apps/desktop/src/", "rust/", "bend/", "packaging/", "scripts/desktop")
# the checks and the gate themselves (marketing m_63313 b): never on R's verdicts
CHECK_PATHS = ("apps/desktop/tests/", "apps/desktop/package.json", "apps/desktop/playwright", "apps/desktop/vitest",
               "pnpm-lock.yaml", "tests/known_reds.txt", "tests/run_all.sh", "tests/gate.sh", "tests/test_map.py",
               "scripts/nightly_", "scripts/desktop_tag.py", "scripts/heavy-run", "scripts/release_",
               "scripts/upgrade_paths.py", "scripts/tree_bise.sh", "scripts/tip_", "scripts/known_reds")


def caps(env=None):
    env = os.environ if env is None else env
    return int(env.get("HEAVY_REUSE_FILES") or 20), int(env.get("HEAVY_REUSE_LINES") or 400)


# ---- pure ------------------------------------------------------------------------------------

def full_runs(text):
    """[(sha, kind, rc)] of full-runs.tsv, in order. Pure."""
    out = []
    for l in (text or "").splitlines():
        p = l.split("\t")
        if len(p) >= 4 and p[0].strip():
            out.append((p[0].strip(), p[3].strip(), p[2].strip()))
    return out


def base(runs, is_ancestor):
    """The newest run of kind 'full' whose sha is_ancestor(sha) holds (N itself excluded by the
    caller's test), or None. Pure but for the predicate."""
    for sha, kind, _ in reversed(runs):
        if kind == "full" and is_ancestor(sha):
            return sha
    return None


def too_big(numstat, max_files, max_lines):
    """Why R..N is past the caps, or None. numstat: [(added, deleted, path)] ('-' for binary:
    counted as max_lines, a binary change is never small). Pure."""
    if len(numstat) > max_files:
        return "%d files changed since R (HEAVY_REUSE_FILES %d)" % (len(numstat), max_files)
    lines = sum((max_lines if a == "-" else int(a)) + (max_lines if d == "-" else int(d)) for a, d, _ in numstat)
    if lines > max_lines:
        return "%d lines changed since R (HEAVY_REUSE_LINES %d)" % (lines, max_lines)
    return None


def row_verdicts(timings_text, sha):
    """{test: rc of its LAST row} for that sha's gate-full rows (run_all.sh writes 9-char shas).
    Pure."""
    out = {}
    for l in (timings_text or "").splitlines():
        p = l.split("\t")
        if len(p) >= 8 and p[7] == "gate-full" and p[1] and sha.startswith(p[1]):
            out[p[2]] = p[3]
    return out


def plan_a(changed, tests, rows, select):
    """('ALL', why) or ('REUSE', [tests]). changed: R..N's paths; tests: run_all's list; rows:
    row_verdicts of R; select(changed) -> (set|None, why) (test_map.select). Pure but select."""
    hit = [p for p in changed if p.startswith(CHECK_PATHS)]
    if hit:
        return "ALL", "a change in the checks or the gate (%s): no verdict of R stands" % hit[0]
    picked, why = select(changed)
    if picked is None:
        return "ALL", "test_map: %s" % why
    not_green = [t for t in tests if rows.get(t) != "0"]
    if len(not_green) == len(tests):
        return "ALL", "R has no green row (no gate-full row in timings.tsv)"
    return "REUSE", sorted((set(picked) & set(tests)) | set(not_green)) + sorted(set(picked) - set(tests))


def plan_b(changed):
    """('ALL', why) or ('REUSE', None) for the app side. Pure."""
    for p in changed:
        if p.startswith(APP_PATHS):
            return "ALL", "the app is built from %s" % p
        if p.startswith(CHECK_PATHS):
            return "ALL", "a change in the checks (%s)" % p
    return "REUSE", None


# ---- git and files ---------------------------------------------------------------------------

def git(repo, *args):
    r = subprocess.run(["git", "-C", repo, *args], capture_output=True, text=True)
    return r.returncode, r.stdout


def read(path):
    try:
        return open(path).read()
    except OSError:
        return ""


def bise_home():
    return os.environ.get("BISE_HOME") or os.path.join(os.path.expanduser("~"), ".bise")


def span(repo, r, n):
    """(why not, None) or (None, (changed paths, numstat)) for R..N."""
    rc, _ = git(repo, "merge-base", "--is-ancestor", r, n)
    if rc != 0:
        return "%s is not an ancestor of %s" % (r[:12], n[:12]), None
    rc, out = git(repo, "diff", "--numstat", "--no-renames", r, n)
    if rc != 0:
        return "git diff %s %s failed" % (r[:12], n[:12]), None
    numstat = [tuple(l.split("\t", 2)) for l in out.splitlines() if l.count("\t") >= 2]
    return None, ([p for _, _, p in numstat], numstat)


def cmd_base(repo, n):
    runs = full_runs(read(os.path.join(bise_home(), "run", "nightly", "full-runs.tsv")))
    _, full_n = git(repo, "rev-parse", "--verify", "%s^{commit}" % n)
    full_n = full_n.strip()

    def anc(sha):
        return sha != full_n and git(repo, "merge-base", "--is-ancestor", sha, full_n)[0] == 0
    r = base(runs, anc)
    if r:
        print(r)
    return 0


def checked_span(repo, r, n):
    why, got = span(repo, r, n)
    if why:
        return why, None
    changed, numstat = got
    if not changed:
        return "no change between %s and %s" % (r[:12], n[:12]), None
    why = too_big(numstat, *caps())
    return why, (None if why else changed)


def cmd_plan(repo, r, n):
    why, changed = checked_span(repo, r, n)
    if why:
        print("ALL " + why)
        return 0
    import test_map  # noqa: PLC0415 (lane A's only)
    tests = test_map.run_all_tests(repo)
    txt = test_map.texts(repo)
    rows = row_verdicts(read(os.path.join(bise_home(), "gate", "timings.tsv")), r)
    kind, what = plan_a(changed, tests, rows, lambda c: test_map.select(c, tests, txt))
    print(("ALL " + what) if kind == "ALL" else " ".join(["REUSE", r] + what))
    return 0


def cmd_app(repo, r, n):
    why, changed = checked_span(repo, r, n)
    if why:
        print("ALL " + why)
        return 0
    kind, what = plan_b(changed)
    print(("ALL " + what) if kind == "ALL" else "REUSE " + r)
    return 0


def main(argv):
    repo = os.environ.get("RELEASE_REUSE_REPO") or os.getcwd()
    if argv[:1] == ["base"] and len(argv) == 2:
        return cmd_base(repo, argv[1])
    if argv[:1] == ["plan"] and len(argv) == 3:
        return cmd_plan(repo, argv[1], argv[2])
    if argv[:1] == ["app"] and len(argv) == 3:
        return cmd_app(repo, argv[1], argv[2])
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
