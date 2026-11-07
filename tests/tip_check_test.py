#!/usr/bin/env python3
"""scripts/tip_check.py: the pure parts (the plan: which range, how long to wait, the spacing
past the CPU bound; the halving that names the first red commit; the verdict lines), then the
runner on a fake repo with a fake gate, heavy-run and power: a green land, close lands in one run,
a red over 3 lands named, the first run's range, one runner at a time, nothing on battery. Never
the real repo, hub or ~/.bise."""
import fcntl
import os
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import tip_check as T  # noqa: E402

FAILS = []
GIT_ENV = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}

# the fake gate: red when the tree has a file BAD; says its base and nice level
FAKE_GATE = """#!/usr/bin/env bash
echo "fake quick $(git rev-parse --short HEAD) base=${GATE_BASE:0:7} nice=$(ps -o nice= -p $$ | tr -d ' ') slot=${FAKE_SLOT:-}"
[ "${1:-}" = seed ] && exit 0
[ -e BAD ] && { echo "FAIL fake_test"; exit 1; }
# a flake: red once, while the file is there
[ -n "${FAKE_FLAKY_FILE:-}" ] && [ -e "$FAKE_FLAKY_FILE" ] && { rm -f "$FAKE_FLAKY_FILE"; echo "FAIL flaky_test"; exit 1; }
exit 0
"""
# the fake heavy-run: --low, then nice 10 as the real one on AC, then the command
FAKE_HEAVY = """#!/usr/bin/env bash
echo "$*" >>"$FAKE_HEAVY_LOG"
[ "$1" = --low ] || { echo "not --low: $*"; exit 9; }
shift
FAKE_SLOT=low exec nice -n 10 "$@"
"""


def check(ok, what):
    if not ok:
        FAILS.append(what)


def pure():
    v = lambda tip, start, cpu=0.0, state="green": {"tip": tip, "base": "b", "state": state, "start": start, "secs": 1,  # noqa: E731
                                                     "cpu": cpu, "commits": 1, "log": "l", "named": ""}
    anc = lambda a, b: a in ("t1", "t2")  # noqa: E731
    now = 1_000_000
    # a tip with a verdict: nothing; the first run: the hook's base (the newest land), no wait
    check(T.plan("t1", [v("t1", now)], "h", now, anc) is None, "a checked tip runs nothing")
    check(T.plan("t3", [], "h", now, anc) == ("h", 0), "first run: the newest land's base, now")
    check(T.plan("t3", [], None, now, anc) == (None, 0), "first run, no hook base: None (tip^)")
    # the base is the newest checked tip main still has; a red or green one, never an infra
    vs = [v("t1", now - 900), v("t2", now - 700), v("gone", now - 650), v("t9", now - 640, state="infra")]
    check(T.plan("t3", vs, "h", now, anc) == ("t2", 0), "base: t2 %s" % (T.plan("t3", vs, "h", now, anc),))
    # at most one run per 10 min from the last start
    check(T.plan("t3", [v("t2", now - 200)], "h", now, anc) == ("t2", 400), "wait 400 s")
    # past 60 CPU-min today, the spacing doubles per bound
    check(T.spacing(0) == 600 and T.spacing(3599) == 600 and T.spacing(3600) == 1200 and T.spacing(7300) == 2400, "spacing")
    busy = [v("t1", now - 3000, cpu=3700), v("t2", now - 200, cpu=10)]
    check(T.plan("t3", busy, "h", now, anc) == ("t2", 1000), "over the bound: 1200 s spacing %s" % (T.plan("t3", busy, "h", now, anc),))
    yesterday = [v("t1", now - 86400 * 2, cpu=99999), v("t2", now - 200)]
    check(T.cpu_today(yesterday, now) == 0 and T.plan("t3", yesterday, "h", now, anc) == ("t2", 400), "yesterday's CPU doesn't count")
    # the first red by halves, the checks it asked for
    asked = []

    def red_from(k):
        def red(c):
            asked.append(c)
            return c >= k
        return red
    check(T.first_red([1, 2, 3, 4, 5, 6, 7, 8], red_from(3)) == 3 and len(asked) == 3, "first red 3 in 3 checks %s" % asked)
    check(T.first_red([1], red_from(1)) == 1 and T.first_red([], red_from(1)) is None, "one commit, none")
    # a verdict line reads back
    line = T.verdict_line(dict(v("t1", now, cpu=12.34), named="n1"))
    back = T.parse_verdicts(line + "short\tline\n")
    check(len(back) == 1 and back[0]["cpu"] == 12.3 and back[0]["named"] == "n1" and back[0]["start"] == now, "verdict %r" % back)


def fake_world(t):
    repo = os.path.join(t, "repo")
    env = dict(os.environ, **GIT_ENV)
    g = lambda *a: subprocess.run(["git", "-C", repo, *a], env=env, check=True, capture_output=True, text=True).stdout.strip()  # noqa: E731
    subprocess.run(["git", "init", "-q", "-b", "main", repo], check=True)
    os.makedirs(os.path.join(repo, "tests"))
    os.makedirs(os.path.join(repo, "scripts"))
    with open(os.path.join(repo, "tests", "gate.sh"), "w") as f:
        f.write(FAKE_GATE)
    for s in ("tip_check.py", "tip_blame.py"):
        shutil.copy(os.path.join(ROOT, "scripts", s), os.path.join(repo, "scripts", s))
    g("add", ".")
    g("-c", "commit.gpgsign=false", "commit", "-q", "-m", "base")
    heavy = os.path.join(t, "heavy-run")
    with open(heavy, "w") as f:
        f.write(FAKE_HEAVY)
    os.chmod(heavy, 0o755)
    home = os.path.join(t, "home")
    os.makedirs(os.path.join(home, "hubs", "h1"))
    run_env = dict(os.environ, BISE_HOME=home, TIP_CHECK_HEAVY_RUN=heavy, TIP_CHECK_POWER="ac", TIP_CHECK_SPACING="0",
                   FAKE_HEAVY_LOG=os.path.join(t, "heavy.log"))
    for k in ("SB_AGENT", "SB_SOCKET", "HEAVY_SLOT"):
        run_env.pop(k, None)

    def land(name, agent, bad=None):
        if bad is True:
            open(os.path.join(repo, "BAD"), "w").close()
            g("add", "BAD")
        g("-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "feat: %s" % name)
        sha = g("rev-parse", "HEAD")
        with open(os.path.join(home, "hubs", "h1", "journal.jsonl"), "a") as f:
            f.write('{"msg":{"text":"✓ %s landed 1 commit on main (%s)"}}\n' % (agent, sha[:9]))
        return sha

    def tip_check(*args, **env):
        return subprocess.run([sys.executable, os.path.join(repo, "scripts", "tip_check.py"), *args],
                              env=dict(run_env, **env), capture_output=True, text=True, timeout=60)

    def verdicts():
        p = os.path.join(home, "gate", "tip-verdicts.tsv")
        return T.parse_verdicts(open(p).read()) if os.path.exists(p) else []
    return repo, home, g, land, tip_check, verdicts


def world():
    t = os.path.realpath(tempfile.mkdtemp(prefix="tip-check-test-", dir=os.environ.get("TMPDIR")))
    try:
        repo, home, g, land, tip_check, verdicts = fake_world(t)
        base0 = g("rev-parse", "HEAD")
        # 1. the first run: only the newest land's range, green, in its own tree at nice 15, --low
        a1 = land("a1", "alpha")
        r = tip_check(base0, a1, "alpha")
        vs = verdicts()
        check(r.returncode == 0 and len(vs) == 1 and vs[0]["tip"] == a1 and vs[0]["base"] == base0 and vs[0]["state"] == "green"
              and vs[0]["commits"] == 1, "first land green %s %s" % (vs, r.stderr))
        log = open(vs[0]["log"]).read() if vs else ""
        check("base=%s" % base0[:7] in log and "nice=15" in log and "slot=low" in log, "quick at nice 15 in heavy-run --low: %r" % log)
        wt = os.path.join(home, "run", "tip-check", "tree", "repo")
        check(os.path.exists(os.path.join(wt, ".git")), "its own tree")
        # 2. the same tip again: nothing runs
        r = tip_check(base0, a1, "alpha")
        check(len(verdicts()) == 1, "a checked tip: no run")
        # 3. close lands: one run over their range
        b1, b2 = land("b1", "beta"), land("b2", "gamma")
        r = tip_check(b1 + "^", b2, "gamma")
        vs = verdicts()
        check(len(vs) == 2 and vs[1]["tip"] == b2 and vs[1]["base"] == a1 and vs[1]["commits"] == 2, "two lands, one run %s" % vs)
        # 4. a red over 3 lands: the first red one named, tip-red to its lander
        c1, c2, c3 = land("c1", "delta"), land("c2", "epsilon", bad=True), land("c3", "zeta")
        r = tip_check(c2, c3, "zeta")
        vs = verdicts()
        check(len(vs) == 3 and vs[2]["state"] == "red" and vs[2]["named"] == c2 and vs[2]["base"] == b2, "red named c2 %s %s" % (vs[-1:], r.stderr))
        tr = os.path.join(home, "run", "nightly", "tip-red")
        text = open(tr).read() if os.path.exists(tr) else ""
        check(text.startswith("to: epsilon main\n") and "tip check (quick) red on main %s" % c2[:9] in text and "feat: c2 (epsilon)" in text
              and "feat: c1" not in text and "fake_test" in text, "tip-red names c2's lander only\n" + text)
        # 4b. a red the re-run doesn't confirm (architect m_55231): flaky, into flaky-open; no
        # tip-red on the innocent lander
        os.remove(tr)
        os.remove(os.path.join(repo, "BAD"))
        g("rm", "-q", "--cached", "BAD")
        g("-c", "commit.gpgsign=false", "commit", "-q", "-m", "fix: c2")
        e1 = land("e1", "theta")
        once = os.path.join(t, "flaky-once")
        open(once, "w").close()
        r = tip_check(c3, e1, "theta", FAKE_FLAKY_FILE=once)
        vs = verdicts()
        fo = os.path.join(home, "run", "nightly", "flaky-open")
        ftext = open(fo).read() if os.path.exists(fo) else ""
        check(len(vs) == 4 and vs[3]["state"] == "flaky" and vs[3]["named"] == "" and not os.path.exists(tr)
              and ftext.startswith("to: main\n") and "flaky on main %s" % e1[:9] in ftext,
              "an unconfirmed red: flaky %s %r %s" % (vs[-1:], ftext, r.stderr))
        # 5. on battery: nothing runs (it waits for AC; the test's POWER_MAX makes it give up at once)
        land("d1", "eta")
        r = subprocess.run([sys.executable, "-c", "import sys; sys.path.insert(0, %r); import tip_check as T; T.POWER_MAX = 0; "
                            "sys.exit(T.main(['x']))" % os.path.join(repo, "scripts")],
                           env=dict(os.environ, BISE_HOME=home, TIP_CHECK_POWER="battery", TIP_CHECK_SPACING="0"),
                           capture_output=True, text=True, timeout=60)
        check(len(verdicts()) == 4, "on battery: no run %s" % r.stderr)
        # 6. one runner on the machine: a second start while the lock is held exits at once
        lock = open(os.path.join(home, "run", "tip-check", "lock"), "w")
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        r = tip_check()
        lock.close()
        check(r.returncode == 0 and "a runner is on it" in r.stdout and len(verdicts()) == 4, "a second runner: %r" % r.stdout)
        heavy = open(os.path.join(t, "heavy.log")).read().splitlines()
        check(all(l.startswith("--low nice -n 5 bash tests/gate.sh quick") for l in heavy), "every run --low, +5: %r" % heavy)
    finally:
        shutil.rmtree(t, ignore_errors=True)


def main():
    pure()
    world()
    for f in FAILS:
        print("FAIL", f)
    print("tip_check_test: %s" % ("ok" if not FAILS else "%d failed" % len(FAILS)))
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
