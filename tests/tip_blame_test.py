#!/usr/bin/env python3
"""scripts/tip_blame.py on a fake repo: the last green is the newest green-shas.tsv sha that is an
ancestor of the tested one; each commit since it gets the agent of its '✓ <agent> landed <n>
commits on main (<sha>)' line (the land's sha and its n-1 parents), a hand commit none; the
failing tests from the log's FAIL lines; tip-red written whole with its 'to:' line (the agents,
then main). Never the real journal or repo."""
import os
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import tip_blame as B  # noqa: E402

FAILS = []
GIT_ENV = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}


def check(ok, what):
    if not ok:
        FAILS.append(what)


def main():
    t = os.path.realpath(tempfile.mkdtemp(prefix="tip-blame-test-", dir=os.environ.get("TMPDIR")))
    try:
        repo = os.path.join(t, "repo")
        env = dict(os.environ, **GIT_ENV)
        g = lambda *a: subprocess.run(["git", "-C", repo, *a], env=env, check=True, capture_output=True, text=True).stdout.strip()  # noqa: E731
        subprocess.run(["git", "init", "-q", "-b", "main", repo], check=True)
        sha = {}
        for n in ("base", "a1", "a2", "hand", "b1"):
            g("-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "feat: %s" % n)
            sha[n] = g("rev-parse", "HEAD")
        green = os.path.join(t, "green-shas.tsv")
        with open(green, "w") as f:
            f.write("%s\t1\t1\tx\tlog\n" % sha["base"])
            f.write("%s\t2\t1\tside\tlog\n" % ("f" * 40))  # not an ancestor: skipped
        journal = os.path.join(t, "journal.jsonl")
        with open(journal, "w") as f:
            f.write('{"msg":{"text":"rc=0\\n✓ alpha landed 2 commits on main (%s) · pushed"}}\n' % sha["a2"][:9])
            f.write('{"msg":{"text":"✓ beta landed 1 commit on main (%s)"}}\n' % sha["b1"][:9])
        log = os.path.join(t, "tip.log")
        with open(log, "w") as f:
            f.write("START x\nFAIL partial_e2e: timed out\nFAIL desktop-checks\nFAIL partial_e2e\n")

        check(B.last_green(repo, green, sha["b1"]) == sha["base"], "last green: base")
        check(B.fails(open(log).read()) == ["partial_e2e", "desktop-checks"], "fails %s" % B.fails(open(log).read()))
        agents, text = B.report(repo, sha["b1"], green, log, [journal], now=0)
        check(agents == ["beta", "alpha"], "agents newest first %s" % agents)
        lines = text.splitlines()
        check(lines[0] == "to: beta alpha main", "to line %r" % lines[0])
        check("partial_e2e desktop-checks" in lines[1] and sha["b1"][:9] in lines[1], "head %r" % lines[1])
        body = "\n".join(lines)
        check("%s feat: a1 (alpha)" % sha["a1"][:9] in body and "%s feat: a2 (alpha)" % sha["a2"][:9] in body,
              "a land of 2 commits: both alpha's\n" + body)
        check("%s feat: hand (no land line" % sha["hand"][:9] in body, "a hand commit: no agent\n" + body)
        check(sha["base"][:9] + " feat: base" not in body, "the last green itself is out\n" + body)

        out = os.path.join(t, "tip-red")
        r = subprocess.run([sys.executable, os.path.join(ROOT, "scripts", "tip_blame.py"), "--repo", repo, "--tested", "main",
                            "--green", green, "--log", log, "--journal", journal, "--out", out], capture_output=True, text=True)
        check(r.returncode == 0 and open(out).read().startswith("to: beta alpha main\n") and not os.path.exists(out + ".tmp"),
              "the file: rc %s %s" % (r.returncode, r.stderr))
        # a second red before main forwarded the first: one file, one 'to:' line, both reports
        r = subprocess.run([sys.executable, os.path.join(ROOT, "scripts", "tip_blame.py"), "--repo", repo, "--tested", sha["a2"],
                            "--green", green, "--log", log, "--journal", journal, "--out", out, "--what", "nightly"], capture_output=True, text=True)
        both = open(out).read()
        check(r.returncode == 0 and both.startswith("to: beta alpha main\n") and both.count("to:") == 1
              and "tip gate red on main %s" % sha["b1"][:9] in both and "nightly red on main %s" % sha["a2"][:9] in both,
              "merged\n" + both)
        # flaky-open: the log's FLAKY lines, each with its rate (gate-full rows of 14 days) and its
        # owner (who last landed a change to its test file, else main)
        with open(os.path.join(repo, "tests_flaky.py"), "w") as f:
            f.write("x\n")
        os.makedirs(os.path.join(repo, "tests"))
        os.rename(os.path.join(repo, "tests_flaky.py"), os.path.join(repo, "tests", "partial_e2e.py"))
        g("add", "tests/partial_e2e.py")
        g("-c", "commit.gpgsign=false", "commit", "-q", "-m", "test: partial")
        sha["c1"] = g("rev-parse", "HEAD")
        with open(journal, "a") as f:
            f.write('{"msg":{"text":"✓ gamma landed 1 commit on main (%s)"}}\n' % sha["c1"][:9])
        flog = os.path.join(t, "flaky.log")
        with open(flog, "w") as f:
            f.write("FLAKY e2e partial_e2e (red, then green alone)\nFLAKY cargo sb::a::b (red, then green alone)\n"
                    "FLAKY e2e partial_e2e (red, then green alone)\n")
        now = 100 * 86400
        timings = os.path.join(t, "timings.tsv")
        with open(timings, "w") as f:
            for i, rc in enumerate((1, 0, 0, 1)):
                f.write("%d\tabc\tpartial_e2e\t%d\t1\t1\t3\tgate-full\n" % (now - i * 3600, rc))
            f.write("%d\tabc\tpartial_e2e\t1\t1\t1\t3\tgate-full\n" % (now - 15 * 86400))  # older than 14 days
            f.write("%d\tabc\tpartial_e2e\t1\t1\t1\t3\tadhoc\n" % now)  # an agent's own run
        check(B.flakes(open(flog).read()) == [("e2e", "partial_e2e"), ("cargo", "sb::a::b")], "flakes")
        with open(os.path.join(t, "flaky.tsv"), "w") as f:  # next to timings.tsv, run_all.sh's
            f.write("%d\tabc\te2e\tpartial_e2e\n%d\tabc\te2e\tpartial_e2e\n" % (now - 3600, now))
            f.write("%d\tabc\te2e\tpartial_e2e\n" % (now - 15 * 86400))  # older than 14 days
            f.write("%d\tabc\tcargo\tsb::a::b\n" % now)
        rates = B.flake_rates(B.tsv_rows(timings), B.tsv_rows(os.path.join(t, "flaky.tsv")), now)
        check(rates.get("partial_e2e") == (2, 4) and rates.get("sb::a::b") == (1, 1), "rates %s" % rates)
        agents, text = B.flaky_report(repo, sha["c1"], flog, [journal], timings, now=now)
        lines = text.splitlines()
        check(agents == ["gamma"] and lines[0] == "to: gamma main", "flaky to %r" % text)
        check("partial_e2e (e2e): flaky in 2 of 4 full gates in 14 days, owner gamma" in text, "flaky line\n" + text)
        check("sb::a::b (cargo): flaky in 1 of 1 full gates in 14 days, owner main" in text, "a cargo flake: main\n" + text)
        check(B.flaky_report(repo, sha["c1"], log, [journal], timings, now=now) == (None, None), "no FLAKY line: no file")
        # its lander archived: main owns it (architect m_55171); the live ones from the hubs' view.json
        hv = os.path.join(t, "home", "hubs", "h1")
        os.makedirs(hv)
        with open(os.path.join(hv, "view.json"), "w") as f:
            f.write('{"agents": [{"name": "main", "archived": false}, {"name": "gamma", "archived": true}]}')
        live = B.live_agents(os.path.join(t, "home"))
        check(live == {"main"} and B.live_agents(os.path.join(t, "nohome")) is None, "live %s" % live)
        agents, text = B.flaky_report(repo, sha["c1"], flog, [journal], timings, now=now, live=live)
        check(agents == [] and "partial_e2e (e2e): flaky in 2 of 4 full gates in 14 days, owner main" in text,
              "an archived lander: main\n" + text)
        _, text = B.flaky_report(repo, sha["c1"], flog, [journal], timings, now=now, live={"gamma", "main"})
        check("owner gamma" in text, "a live lander keeps it\n" + text)
        fo = os.path.join(t, "flaky-open")
        fenv = dict(os.environ, BISE_HOME=os.path.join(t, "nohome"))  # never the real hubs' view.json
        r = subprocess.run([sys.executable, os.path.join(ROOT, "scripts", "tip_blame.py"), "--flaky", timings, "--repo", repo,
                            "--tested", "main", "--green", green, "--log", log, "--journal", journal, "--out", fo],
                           capture_output=True, text=True, env=fenv)
        check(r.returncode == 0 and not os.path.exists(fo), "no flake: no flaky-open %s" % r.stderr)
        r = subprocess.run([sys.executable, os.path.join(ROOT, "scripts", "tip_blame.py"), "--flaky", timings, "--repo", repo,
                            "--tested", "main", "--green", green, "--log", flog, "--journal", journal, "--out", fo],
                           capture_output=True, text=True, env=fenv)
        check(r.returncode == 0 and open(fo).read().startswith("to: gamma main\n") and "1 flaky" not in open(fo).read(),
              "flaky-open written %s" % r.stderr)
        # no green known: the last 20 commits, said so
        _, text = B.report(repo, sha["b1"], os.path.join(t, "none.tsv"), log, [journal], now=0)
        check("(none known: the last 20)" in text and "feat: base" in text, "no green\n" + text)
    finally:
        shutil.rmtree(t, ignore_errors=True)
    for f in FAILS:
        print("FAIL", f)
    print("tip_blame_test: %s" % ("ok" if not FAILS else "%d failed" % len(FAILS)))
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
