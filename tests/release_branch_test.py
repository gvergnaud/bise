#!/usr/bin/env python3
"""scripts/release_branch.py: the pure parts (the open line, what a line lacks, holds, where the
next release comes from, what can be picked), then the commands on a throwaway git repo (no
remote, no push, the quick gate replaced by RELEASE_BRANCH_GATE)."""
import io
import os
import subprocess
import sys
import tempfile
from contextlib import redirect_stdout

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import release_branch as B  # noqa: E402

FAILS = []


def check(ok, what):
    if not ok:
        FAILS.append(what)


def pure():
    check(B.line_name("v2026.11.15") == "release/v2026.11.15", "line name")
    check(B.line_name("desktop-v2026.11.3") is None and B.line_name("v2026.11") is None, "bad tags")
    check(B.open_line(["main", "release/v2026.9.30", "release/v2026.11.2", "release/v2026.11.15",
                       "release/v2026.11.15-2", "release/x"]) == "release/v2026.11.15-2", "newest line by tag")
    check(B.open_line(["main", "sb/x"]) is None, "no line")
    msg = "fix: a\n\nbody\n\n(cherry picked from commit %s)\n" % ("a" * 40)
    check(B.named(msg) == ["a" * 40], "-x names")
    check(B.named("fix: b\n\nBackport-of: %s\n" % ("b" * 12)) == ["b" * 12], "Backport-of names")
    check(B.named("fix: c\n\nsee Backport-of: x\n") == [], "a mention is no name")
    on = {"m1", "m2"}.__contains__
    rows = [("p1", "fix: picked", True, []), ("p2", "fix: redone", False, ["m1"]),
            ("p3", "fix: only here", False, []), ("p4", "fix: names a non-main", False, ["zz"])]
    check([r[0] for r in B.lacking(rows, on)] == ["p3", "p4"], "lacking: patch-equal or named on main is fine")
    holds = [{"sha": "h1", "why": "tasks"}, {"sha": "h2", "why": "other"}]
    check(B.reached(holds, {"h1"}.__contains__) == holds[:1], "reached")
    check(B.cut_base("m", [], None, None) == ("main", "m"), "no hold: main")
    check(B.cut_base("m", [], "release/v1.1.1", "l") == ("main", "m"), "no hold, a line: main (a new line)")
    check(B.cut_base("m", holds[:1], "release/v1.1.1", "l") == ("line", "l"), "held: the line")
    w, why = B.cut_base("m", holds[:1], None, None)
    check(w == "none" and "h1 (tasks)" in why, "held, no line: none %s" % why)
    check("merge" in B.pick_refusal("s" * 40, 2, True, False, False), "a merge is refused")
    check("already" in B.pick_refusal("s" * 40, 1, True, False, True), "already on the line")
    check("not on main" in B.pick_refusal("s" * 40, 1, False, False, False), "not on main")
    check(B.pick_refusal("s" * 40, 1, False, True, False) is None, "a backport is fine")
    check(B.pick_refusal("s" * 40, 1, True, False, False) is None, "a main commit is fine")
    h = B.add_hold([{"sha": "a", "why": "x", "by": "m", "at": 1}], "a", "y", "n", 2)
    check(h == [{"sha": "a", "why": "y", "by": "n", "at": 2}], "a hold again replaces it")


def run(repo, bh, *argv, gate="true"):
    env = dict(os.environ, RELEASE_BRANCH_REPO=repo, BISE_HOME=bh, RELEASE_BRANCH_PUSH="0",
               RELEASE_BRANCH_GATE=gate, SB_AGENT="marketing")
    out = io.StringIO()
    old = dict(os.environ)
    os.environ.clear()
    os.environ.update(env)
    try:
        with redirect_stdout(out):
            rc = B.main(list(argv))
    finally:
        os.environ.clear()
        os.environ.update(old)
    return rc, out.getvalue()


def g(repo, *a):
    return subprocess.run(["git", "-C", repo, *a], capture_output=True, text=True, check=True).stdout.strip()


def commit(repo, path, text, subject):
    with open(os.path.join(repo, path), "w") as f:
        f.write(text)
    g(repo, "add", path)
    g(repo, "commit", "-qm", subject)
    return g(repo, "rev-parse", "HEAD")


def flow():
    with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as t:
        repo, bh = os.path.join(t, "repo"), os.path.join(t, "home")
        os.makedirs(repo)
        g(repo, "init", "-q", "-b", "main")
        for k, v in (("user.name", "t"), ("user.email", "t@t"), ("commit.gpgsign", "false")):
            g(repo, "config", k, v)
        commit(repo, "a.txt", "a\n", "feat: a")
        pre = commit(repo, "b.txt", "b\n", "fix: b")
        # a feature merged into main: held
        g(repo, "switch", "-qc", "feat")
        commit(repo, "f.txt", "f\n", "feat: big")
        g(repo, "switch", "-q", "main")
        g(repo, "merge", "-q", "--no-ff", "-m", "merge feat", "feat")
        merge = g(repo, "rev-parse", "HEAD")
        rc, out = run(repo, bh, "hold", merge, "feat: he tests it first")
        check(rc == 0 and "held" in out, "hold %s" % out)
        rc, out = run(repo, bh, "status")
        check("nowhere" in out and "feat: he tests it first" in out, "held, no line: status %s" % out)
        rc, out = run(repo, bh, "start", "v2026.11.15")
        check(rc == 1 and "refused" in out and merge[:9] in out, "start on a held tip refused %s" % out)
        rc, out = run(repo, bh, "start", "v2026.11.15", "--from", pre)
        check(rc == 0 and g(repo, "rev-parse", "release/v2026.11.15") == pre, "start before the hold %s" % out)
        rc, out = run(repo, bh, "start", "v2026.11.15", "--from", pre)
        check(rc == 1 and "exists" in out, "a line twice %s" % out)
        # fixes on main after the merge
        fix1 = commit(repo, "a.txt", "a2\n", "fix: a2")
        commit(repo, "c.txt", "c\n", "feat: c")
        fix2 = commit(repo, "f.txt", "f2\n", "fix(feat): f2")  # needs the held feature
        rc, out = run(repo, bh, "candidates")
        check(fix1[:12] in out and fix2[:12] in out and "feat: c" not in out, "candidates %s" % out)
        rc, out = run(repo, bh, "pick", merge)
        check(rc == 1 and "merge" in out, "a merge is refused %s" % out)
        rc, out = run(repo, bh, "pick", fix1, gate="false")
        check(rc == 1 and "quick gate is red" in out and g(repo, "rev-parse", "release/v2026.11.15") == pre,
              "a red gate leaves the line %s" % out)
        rc, out = run(repo, bh, "pick", fix1, gate='test "$GATE_BASE" = %s' % pre)
        tip = g(repo, "rev-parse", "release/v2026.11.15")
        check(rc == 0 and tip != pre and "cherry picked from commit %s" % fix1 in g(repo, "log", "-1", "--format=%B", tip),
              "pick with -x, GATE_BASE the line's tip before %s" % out)
        check(g(repo, "show", "%s:a.txt" % tip) == "a2" and "c.txt" not in g(repo, "ls-tree", "--name-only", tip),
              "only the pick on the line")
        rc, out = run(repo, bh, "pick", fix1)
        check(rc == 1 and "already" in out, "a pick twice %s" % out)
        rc, out = run(repo, bh, "pick", fix2)
        check(rc == 1 and "does not apply" in out and "Backport-of" in out
              and g(repo, "rev-parse", "release/v2026.11.15") == tip, "a pick that conflicts %s" % out)
        rc, out = run(repo, bh, "candidates")
        check(fix1[:12] not in out and fix2[:12] in out, "a picked fix is no candidate %s" % out)
        # the backport: a commit on a branch from the line, naming fix2
        g(repo, "switch", "-qc", "sb/x", "release/v2026.11.15")
        with open(os.path.join(repo, "f.txt"), "w") as f:
            f.write("f2 without feat\n")
        g(repo, "add", "f.txt")
        g(repo, "commit", "-qm", "fix(feat): f2, backport\n\nBackport-of: %s" % fix2)
        back = g(repo, "rev-parse", "HEAD")
        g(repo, "switch", "-q", "main")
        rc, out = run(repo, bh, "pick", back)
        check(rc == 0, "a backport picks %s" % out)
        # a commit only on the line blocks the next line
        g(repo, "switch", "-qc", "sb/y", "release/v2026.11.15")
        only = commit(repo, "o.txt", "o\n", "fix: only on the line")
        g(repo, "switch", "-q", "main")
        g(repo, "update-ref", "refs/heads/release/v2026.11.15", only)
        run(repo, bh, "unhold", merge)
        rc, out = run(repo, bh, "start", "v2026.11.16")
        check(rc == 1 and only[:9] in out and "lacks" in out, "a line with a commit main lacks can't end %s" % out)
        g(repo, "cherry-pick", only)  # landed on main: patch-equal
        rc, out = run(repo, bh, "start", "v2026.11.16")
        check(rc == 0, "the line ends once main has all of it %s" % out)
        rc, out = run(repo, bh, "status")
        check("release/v2026.11.16" in out and "holds: none" in out and "next release from: main" in out, "status %s" % out)
        rc, out = run(repo, bh, "unhold", merge)
        check(rc == 1, "unhold twice %s" % out)


def main():
    pure()
    flow()
    for f in FAILS:
        print("FAIL", f)
    print("release_branch_test: %s" % ("FAIL" if FAILS else "ok"))
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
