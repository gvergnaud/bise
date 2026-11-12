"""Nothing moves main without the land check (issue 28 part 2), end to end.

A real hub (bise sbd, scripted model) on a clone of this checkout (its HEAD
plus its uncommitted changes, on main, no remote), whose bise.toml the test
sets to run `tests/gate.sh land` (the repo itself has no [land] check since
d3616e24f; the check's code still serves repos that set one).

  ways:  a worktree land whose base moved, one whose base did not, a
         `sb land --here` from main's checkout, `sb feature merge`

A docs-only change lands on each way, before (bise.toml without [land]) and
after; the land wall times are printed (p50, max), before and after.

One red, a UI word as a literal in a tmux test (word_lint), is planted on a
worktree land and on `sb land --here` from the shared folder: the two ways
whose refusal, --add and claim read are different shell code (architect
m_62929). Each is refused with its step and its log, and main's sha does not
move. Which step each kind of red stops at (clippy, proto_ts, desktop) is
the core's table, land_tests.rs's 15 tests; the ways x trunk x moved matrix
is land_check.rs's unit table (architect m_28364). Until release-speed plan
A step 2, 7 refused lands here (4 reds x the worktree way, word_lint on the
3 others) were ~2 min of every release pass.

The cargo target is a clone (cp -c: no bytes) of the caller's warm one;
both check trees (the agent's worktree, the hub's .land-check) build the
workspace crates once at their paths.

Run: python3 -u tests/land_every_e2e.py
"""
import glob
import json
import os
import shutil
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, ROOT, check, out, sh  # noqa: E402

NL = chr(10)
WAYS = ("worktree-moved", "worktree", "here", "merge")
RED_WAYS = ("worktree", "here")
LAND_TIMEOUT = 1500


def clone_checkout(ws):
    """`ws`: this checkout's HEAD and its uncommitted changes, one commit
    on `main`, no remote (the trunk is main), objects shared."""
    shutil.rmtree(ws)
    head = out(ROOT, "git rev-parse HEAD")
    sh(os.path.dirname(ws), "git clone -q --shared --no-checkout %s %s" % (ROOT, ws))
    sh(ws, "git remote remove origin && git checkout -q -B main %s" % head)
    sh(ws, "git config user.email t@t && git config user.name t && git config commit.gpgsign false")
    diff = subprocess.run(["git", "diff", "HEAD", "--binary"], cwd=ROOT, capture_output=True).stdout
    if diff:
        subprocess.run(["git", "apply", "--whitespace=nowarn"], cwd=ws, input=diff, check=True)
    for f in out(ROOT, "git ls-files --others --exclude-standard").splitlines():
        os.makedirs(os.path.dirname(os.path.join(ws, f)) or ws, exist_ok=True)
        shutil.copy2(os.path.join(ROOT, f), os.path.join(ws, f))
    sh(ws, "git add -A && (git diff --cached --quiet || git commit -qm 'this checkout'); git branch -q -D sb/land-every 2>/dev/null; true")


def warm_target(tmp):
    """A clone of the caller's cargo target (APFS: no bytes until written)."""
    src = os.environ.get("CARGO_TARGET_DIR") or os.path.join(ROOT, "rust", "target")
    dst = os.path.join(tmp, "target")
    if os.path.isdir(src):
        subprocess.run(["cp", "-cR", src, dst], check=False)
    os.makedirs(dst, exist_ok=True)
    return dst


def a_word():
    """A catalog word word_lint flags as a literal: no arg, a space in it."""
    for f in sorted(glob.glob(os.path.join(ROOT, "tests", "words", "*.gen.json"))):
        for v in json.load(open(f)).values():
            if isinstance(v, dict) and not v.get("args") and " " in v.get("text", "") and len(v["text"]) > 12 and "'" not in v["text"]:
                return v["text"]
    raise AssertionError("no catalog word found in tests/words")


def append(path, text):
    with open(path, "a") as f:
        f.write(text)


def plant_word(d):
    """The planted red: a UI word as a literal in a tmux test (word_lint)."""
    with open(os.path.join(d, "tests/land_every_red_tmux.py"), "w") as f:
        f.write("WAIT_ON = %r" % a_word() + NL)


class Lands:
    """`sb land` run by an agent's bash, its output, rc and wall time in
    files the test reads (no transcript parsing)."""

    def __init__(self, c, tmp):
        self.c, self.dir, self.n = c, os.path.join(tmp, "lands"), 0
        os.makedirs(self.dir)
        self.times = {}

    def run(self, agent, cmd, kind):
        self.n += 1
        p = os.path.join(self.dir, str(self.n))
        t = "python3 -c 'import time; print(time.time())'"
        script = "s=$(%s); %s > %s.out 2>&1; rc=$?; e=$(%s); echo \"$rc $s $e\" > %s.tmp && mv %s.tmp %s.rc" % (t, cmd, p, t, p, p, p)
        self.c.say("[[bash: %s]]" % script, focus=agent)
        wait.until(lambda: os.path.exists(p + ".rc"), LAND_TIMEOUT, "land %d (%s) by %s" % (self.n, cmd, agent))
        rc, s, e = open(p + ".rc").read().split()
        self.times.setdefault(kind, []).append(float(e) - float(s))
        self.c.wait_idle(agent)
        return int(rc), open(p + ".out").read()


def stats(xs):
    return "p50 %.2f s, max %.2f s (%d lands)" % (statistics.median(xs), max(xs), len(xs)) if xs else "none"


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    clone_checkout(E.ws)
    E.env["CARGO_TARGET_DIR"] = warm_target(E.tmp)
    ws = E.ws
    main_sha = lambda: out(ws, "git rev-parse main")  # noqa: E731
    # the repo dropped its own [land] check (d3616e24f); the test plants the
    # check it measures, so the land check's code stays proven for repos with one
    toml = '[land]' + NL + 'check = "tests/gate.sh land"' + NL
    # before: main's tip without [land] (lands as before issue 28 part 2)
    with open(os.path.join(ws, "bise.toml"), "w") as f:
        f.write("# no land check" + NL)
    sh(ws, "git commit -qam 'no land check'")
    ok = True
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say('[[bash: sb spawn wk --place new --objective "{{bash: true}}"]]')
        c.wait(lambda: c.agent("wk") is not None, 60, "wk")
        c.wait_idle("wk", "main")
        wt = out(ws, "git worktree list --porcelain | sed -n 's/^worktree //p' | grep '/wk/'")
        check(os.path.isdir(wt), "wk's worktree: %r" % wt)
        L = Lands(c, E.tmp)
        nfeat = [0]

        def land(way, plant, msg, kind):
            """Plant (a function of a checkout) and land it the `way`; (rc, out)."""
            if way in ("worktree", "worktree-moved"):
                sh(wt, "git reset -q --hard main && git clean -qfd")
                plant(wt)
                sh(wt, "git add -A && git commit -qm %s" % json.dumps(msg))
                if way == "worktree-moved":
                    sh(ws, "mkdir -p docs && echo %d > docs/land-every-moved-%d.md && git add docs && git commit -qm moved" % (L.n, L.n))
                return L.run("wk", 'sb land %s' % json.dumps(msg), kind)
            if way == "here":
                before = set(out(ws, "git status --porcelain --untracked-files=all").splitlines())
                plant(ws)
                now = out(ws, "git status --porcelain --untracked-files=all").splitlines()
                files = [l[3:] for l in now if l not in before]
                add = " ".join("--add %s" % f for f in files)
                r = L.run("main", "sb land --here %s %s" % (add, json.dumps(msg)), kind)
                if r[0] != 0:
                    sh(ws, "git checkout -q -- . && rm -f %s" % " ".join(files))
                return r
            nfeat[0] += 1
            name = "le%d" % nfeat[0]
            rc, o = L.run("main", "sb feature new %s" % name, "setup")
            check(rc == 0, o)
            tmpwt = os.path.join(E.tmp, "fw-%s" % name)
            sh(ws, "git worktree add -q %s %s" % (tmpwt, name))
            plant(tmpwt)
            sh(tmpwt, "git add -A && git commit -qm %s" % json.dumps(msg))
            sh(ws, "git worktree remove --force %s" % tmpwt)
            return L.run("main", "sb feature merge %s" % name, kind)

        def docs(d):
            n = L.n
            os.makedirs(os.path.join(d, "docs"), exist_ok=True)
            with open(os.path.join(d, "docs", "land-every-%d.md" % n), "w") as f:
                f.write("a docs-only change %d" % n + NL)

        # before: each way lands a docs-only change, no check
        for way in WAYS:
            rc, o = land(way, docs, "docs before", "docs-before")
            check(rc == 0, "%s: a docs-only land lands without a check: %s" % (way, o))
        # after: the land check back at main's tip
        with open(os.path.join(ws, "bise.toml"), "w") as f:
            f.write(toml)
        sh(ws, "git commit -qam 'the land check'")
        for way in WAYS:
            m = main_sha()
            rc, o = land(way, docs, "docs after", "docs-after")
            check(rc == 0, "%s: a docs-only land lands (no step): %s" % (way, o))
            check(main_sha() != m, "%s: main moved" % way)
            print("ok   %s: a docs-only change lands" % way)
        # one red per shell path (architect m_62929): a UI word as a literal, on a worktree land
        # and on `sb land --here` from the shared folder, the two ways whose refusal, --add and
        # claim read are different shell code
        for way in RED_WAYS:
            m = main_sha()
            rc, o = land(way, plant_word, "red word_lint", "red")
            check(rc != 0, "word_lint / %s: refused: %s" % (way, o))
            check("the land check is red at word_lint" in o, "word_lint / %s: the step's words: %s" % (way, o))
            check("(log: " in o, "word_lint / %s: its log path: %s" % (way, o))
            check(main_sha() == m, "word_lint / %s: main did not move" % way)
            print("ok   word_lint / %s: refused, main unmoved" % way)
        print("land wall time, docs-only, before: %s" % stats(L.times.get("docs-before")))
        print("land wall time, docs-only, after:  %s" % stats(L.times.get("docs-after")))
        print("land wall time, refused reds:      %s" % stats(L.times.get("red")))
    except Exception as e:  # noqa: BLE001
        ok = False
        print("FAIL: %s" % e)
        for f in sorted(glob.glob(os.path.join(E.tmp, "lands", "*.out")))[-3:]:
            print("--- %s" % f)
            print(open(f).read()[-2000:])
    finally:
        E.close()
    print("PASS land_every_e2e" if ok else "FAIL land_every_e2e")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
