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
import threading  # noqa: E402
sys.path.insert(0, os.path.join(ROOT, "tests"))
import wait  # noqa: E402


def wait_for(pred, secs=30, what="the state"):
    try:
        return bool(wait.until(pred, secs, what))
    except Exception:  # noqa: BLE001 (a timeout is a False here)
        return False

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
    # plan B1 (architect m_63540): the e2e/tmux picks after a green quick, typed in the 10th field
    line = T.verdict_line(dict(v("t1", now), e2e="green:a_e2e,b_tmux"))
    check(T.parse_verdicts(line)[0]["e2e"] == "green:a_e2e,b_tmux" and T.parse_verdicts(T.verdict_line(v("t1", now)))[0]["e2e"] == "",
          "the e2e field reads back, empty on an old line")
    tests = ["a_e2e", "b_e2e", "c_tmux"] + ["t%d_e2e" % i for i in range(10)]
    check(T.e2e_picks("a_e2e c_tmux PROOFX\n", tests) == (["a_e2e", "c_tmux"], None), "picks: test_map's, run_all's only")
    picks, why = T.e2e_picks("ALL rust/x.rs (shared code or no rule)\n", tests)
    check(picks == [] and why.startswith("none:ALL rust/x.rs"), "ALL: none, said %r" % why)
    picks, why = T.e2e_picks(" ".join("t%d_e2e" % i for i in range(9)), tests)
    check(picks == [] and "9 picked, over the cap 8" in why, "over the cap: none %r" % why)
    check(T.e2e_picks("", tests) == ([], "none:no e2e/tmux test picked"), "nothing picked")
    cp = [("c1", {"a_e2e"}), ("c2", None), ("c3", {"b_e2e", "a_e2e"}), ("c4", set())]
    check(T.candidates(["a_e2e"], cp) == ["c1", "c3"], "candidates: each commit whose own diff picks a red test")
    check(T.candidates(["b_e2e"], cp) == ["c3"], "one candidate: named")
    check(T.candidates(["c_tmux"], cp) == [], "picked only by ALL: no candidate (a flake to own)")
    # plan B2: the away run's verdict from nightly_desktop.sh's steps and reds, split by known_reds
    known = {"as-he-does/quick: old": ("x", "o", "w")}
    check(T.away_verdict({"candidate": "green", "quick": "green"}, [], known) == ("green", []), "away: green")
    check(T.away_verdict({"candidate": "green", "quick": "red"}, ["as-he-does/quick: old"], known) == ("owned", []), "away: a known red is owned")
    check(T.away_verdict({"candidate": "green", "quick": "red"}, ["as-he-does/quick: old", "as-he-does/quick: new"], known)
          == ("red", ["as-he-does/quick: new"]), "away: a new red")
    check(T.away_verdict({"candidate": "red", "quick": "none"}, ["desktop/candidate"], known) == ("red", ["desktop/candidate"]), "away: the candidate red")
    check(T.away_verdict({"candidate": "green", "quick": "none"}, [], known) == ("none", []), "away: no quick result, never red")
    check(T.parse_verdicts(T.verdict_line(dict(v("t1", now), e2e="none:x", ahd="green")))[0]["ahd"] == "green", "the ahd field reads back")


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
    # the away run splits its reds by known_reds (plan B2)
    shutil.copy(os.path.join(ROOT, "tests", "known_reds.py"), os.path.join(repo, "tests", "known_reds.py"))
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
        # nice 15 over the caller's own (this test inside a gate already runs at heavy-run's 10)
        want = "nice=%d" % min(20, os.nice(0) + 15)
        check("base=%s" % base0[:7] in log and want in log and "slot=low" in log, "quick at %s in heavy-run --low: %r" % (want, log))
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
        # 7. plan B1 (architect m_63540): after a green quick, the e2e/tmux tests test_map picks for
        # the range (a fake test_map: a changed tests/<name>.py picks <name>; a fake build and run:
        # a test with a file E2EBAD_<name> in the tree is red)
        with open(os.path.join(repo, "tests", "test_map.py"), "w") as f:
            f.write("import sys\nprint(' '.join(p[6:-3] for p in sys.argv[2:] if p.startswith('tests/') and p.endswith('_e2e.py')))\n")
        with open(os.path.join(repo, "tests", "run_all.sh"), "w") as f:
            f.write('TESTS="one_e2e two_e2e"\n')
        g("add", "tests")
        g("-c", "commit.gpgsign=false", "commit", "-q", "-m", "the fake map")
        fake = 'for t in $1; do echo "ran $t"; if [ -e "E2EBAD_$t" ]; then echo "$t" >>"$SB_RED_FILE"; f=1; fi; done; exit ${f:-0}'
        e2e_env = dict(TIP_CHECK_E2E_SCRIPT=fake)
        n0 = len(verdicts())

        def land_file(name, agent, path, bad=False):
            open(os.path.join(repo, path), "w").write("%s\n" % name)
            if bad:
                open(os.path.join(repo, "E2EBAD_" + os.path.basename(path)[:-3]), "w").close()
            g("add", "-A")
            g("-c", "commit.gpgsign=false", "commit", "-q", "-m", "feat: %s" % name)
            sha = g("rev-parse", "HEAD")
            with open(os.path.join(home, "hubs", "h1", "journal.jsonl"), "a") as f:
                f.write('{"msg":{"text":"✓ %s landed 1 commit on main (%s)"}}\n' % (agent, sha[:9]))
            return sha
        f1 = land_file("f1", "iota", "tests/one_e2e.py")
        r = tip_check(f1 + "^", f1, "iota", **e2e_env)
        vs = verdicts()
        check(len(vs) == n0 + 1 and vs[-1]["state"] == "green" and vs[-1]["e2e"] == "green:one_e2e", "e2e green after quick %s %s" % (vs[-1:], r.stderr[-300:]))
        if os.path.exists(tr):
            os.remove(tr)
        # red twice, one commit whose own diff picks it: named, tip-red on its lander
        f2 = land_file("f2", "kappa", "tests/two_e2e.py", bad=True)
        r = tip_check(f2 + "^", f2, "kappa", **e2e_env)
        vs = verdicts()
        text = open(tr).read() if os.path.exists(tr) else ""
        check(vs[-1]["state"] == "red" and vs[-1]["named"] == f2 and vs[-1]["e2e"] == "red:two_e2e" and text.startswith("to: kappa main"),
              "e2e red twice: its one commit named %s %r" % (vs[-1:], text))
        # two commits in the range both pick the red test: candidates=, main decides
        os.remove(tr)
        f3 = land_file("f3", "lambda", "tests/two_e2e.py", bad=True)
        open(os.path.join(repo, "tests", "two_e2e.py"), "w").write("y\n")
        g("-c", "commit.gpgsign=false", "commit", "-q", "-am", "feat: f4")
        f4 = g("rev-parse", "HEAD")
        r = tip_check(f2, f4, "lambda", **e2e_env)
        vs = verdicts()
        text = open(tr).read() if os.path.exists(tr) else ""
        check(vs[-1]["state"] == "red" and vs[-1]["named"] == "" and "candidates=%s,%s" % (f3[:9], f4[:9]) in vs[-1]["e2e"]
              and "candidates=%s,%s" % (f3[:9], f4[:9]) in text, "two candidates, one tip-red line %s %r" % (vs[-1:], text))
        # over the cap or ALL: none, said
        check(all(v["e2e"] for v in vs[n0:]), "every new verdict has its e2e field")
        # (step 7's red test fixed: its marker gone)
        os.remove(os.path.join(repo, "E2EBAD_two_e2e"))
        g("add", "-A")
        g("-c", "commit.gpgsign=false", "commit", "-q", "-m", "fix: two_e2e")
        if os.path.exists(tr):
            os.remove(tr)
        # 8. plan B2 (architect m_63540): a desktop land while he's away: the candidate and as-he-does
        # quick (a fake nightly_desktop.sh) on a throwaway BISE_HOME under its out dir; he comes back:
        # stopped, 'none:he came back', never red. The fake test_map: apps/desktop/ reaches the desktop.
        with open(os.path.join(repo, "tests", "test_map.py"), "w") as f:
            f.write("import sys\nif sys.argv[1] == 'desktop':\n"
                    "    print('desktop x' if any(p.startswith('apps/desktop/') for p in sys.argv[2:]) else 'none')\n"
                    "else:\n    print(' '.join(p[6:-3] for p in sys.argv[2:] if p.startswith('tests/') and p.endswith('_e2e.py')))\n")
        g("-c", "commit.gpgsign=false", "commit", "-q", "-am", "the fake desktop map")
        os.makedirs(os.path.join(repo, "apps", "desktop"), exist_ok=True)
        idlef, nd, ndlog = os.path.join(t, "idle"), os.path.join(t, "fake_nd.sh"), os.path.join(t, "nd.log")
        with open(nd, "w") as f:
            f.write('echo "steps=$NIGHTLY_DESKTOP_STEPS home=$BISE_HOME out=$1 sha=$2" >>"%s"\n'
                    'i=0; while [ -e "%s.slow" ] && [ $i -lt 100 ]; do i=$((i+1)); [ "$(cat %s)" = 0 ] && break; '
                    'perl -e "select(undef,undef,undef,0.1)"; done\n'
                    'printf "candidate\\tgreen\\nquick\\t%%s\\n" "${FAKE_Q:-green}" >"$1/steps.tsv"\n'
                    '[ -n "${FAKE_RED:-}" ] && echo "$FAKE_RED" >"$1/reds.txt"; exit 0\n' % (ndlog, idlef, idlef))
        away_env = dict(e2e_env, TIP_CHECK_IDLE_FILE=idlef, TIP_CHECK_DESKTOP=nd, TIP_CHECK_AWAY_POLL="0.2", TIP_CHECK_AWAY_GAP="0")
        open(idlef, "w").write("3600")
        h1 = land_file("h1", "mu", "apps/desktop/a.ts")
        r = tip_check(h1 + "^", h1, "mu", **away_env)
        vs = verdicts()
        nl = open(ndlog).read().splitlines() if os.path.exists(ndlog) else []
        check(vs[-1]["ahd"] == "green" and nl and "steps=candidate quick" in nl[-1], "away, a desktop land: candidate + quick, green %s %s %s" % (vs[-1:], nl, r.stderr[-300:]))
        home_of = nl[-1].split("home=")[1].split()[0] if nl else ""
        out_of = nl[-1].split("out=")[1].split()[0] if nl else "?"
        check(home_of.startswith(out_of + "/") and not home_of.startswith(os.path.join(home, ".bise")) and home_of != home,
              "its BISE_HOME is a throwaway under its out dir, never his: %r (out %r)" % (home_of, out_of))
        # here (idle 10 s): nothing runs
        open(idlef, "w").write("10")
        h2 = land_file("h2", "nu", "apps/desktop/b.ts")
        n_before = len(nl)
        tip_check(h2 + "^", h2, "nu", **away_env)
        check(verdicts()[-1]["ahd"].startswith("none:he is here") and len(open(ndlog).read().splitlines()) == n_before, "he's here: no run")
        # away, then back mid-run (the fake idle file drops; our run doesn't move it): stopped, none
        open(idlef, "w").write("3600")
        open(idlef + ".slow", "w").close()
        h3 = land_file("h3", "xi", "apps/desktop/c.ts")
        n_before = len(open(ndlog).read().splitlines())

        def comes_back():
            # once the fake run started: he touches the Mac
            if wait_for(lambda: len(open(ndlog).read().splitlines()) > n_before, 30, "the away run's start"):
                open(idlef, "w").write("0")
        th = threading.Thread(target=comes_back)
        th.start()
        r = tip_check(h3 + "^", h3, "xi", **away_env)
        th.join()
        os.remove(idlef + ".slow")
        vs = verdicts()
        check(vs[-1]["ahd"] == "none:he came back" and vs[-1]["state"] == "green", "he came back mid-run: stopped, none, never red %s %s" % (vs[-1:], r.stderr[-300:]))
        # a new red away: its one desktop commit named (tip-red)
        if os.path.exists(tr):
            os.remove(tr)
        open(idlef, "w").write("3600")
        h4 = land_file("h4", "omicron", "apps/desktop/d.ts")
        r = tip_check(h4 + "^", h4, "omicron", FAKE_Q="red", FAKE_RED="as-he-does/quick: a new red", **away_env)
        vs = verdicts()
        text = open(tr).read() if os.path.exists(tr) else ""
        check(vs[-1]["state"] == "red" and vs[-1]["named"] == h4 and vs[-1]["ahd"] == "red:as-he-does/quick: a new red"
              and text.startswith("to: omicron main"), "away, a new red: its commit named %s %r" % (vs[-1:], text))
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
