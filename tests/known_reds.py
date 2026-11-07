#!/usr/bin/env python3
"""The known reds (issue 28): tests/known_reds.txt is the one list of the tests that are red on
main now, each with who fixes it. The full gate (e2e/tmux, run_all.sh's red list) and apps/
desktop's test:land (land.mts, its red checks) split their reds with it: a red in the file is
known (its owner is on it), any other one is NEW, yours until shown otherwise. The rc of the
gate stays what it was: a known red is still red. The land check (gate.sh land) reads nothing
here: its steps are always green.

The file: one red per line, tab-separated, '#' starts a comment line:
  <test>	<since: the sha or date it went red>	<owner: the agent fixing it>	<why>
<test> is the name the runner prints: an e2e/tmux test's (feature_e2e), a desktop check's spec
name without .spec.ts (history-perf).

A line expires (c5-tip-per-land, audit-tests finding 3c): its test ran green on the last 2 shas
that ran it after its since (a sha: the runs on its descendants; a date YYYY-MM-DD: the runs
from that day). It is not red on main now: remove the line, or, still red under load, give it a
fresh since and why (a flake goes to flaky-open, scripts/tip_blame.py). The runs it reads, under
${BISE_HOME:-~/.bise}: gate/timings.tsv's gate-full rows (e2e, tmux, cargo: rc 0 green),
run/nightly/checks-verdicts.tsv (the desktop checks: each run's dir, its specs and reds.txt) and
run/nightly/desktop-runs.tsv (as-he-does/<mode> rows and test:land's checks: steps.tsv and
reds.txt). A test none of them names (a desktop_check vitest file) never expires.
--expire prints each expired line (EXPIRED <test> ...) and exits 1 when one is $SB_AGENT's: red
in its owner's quick gate (tests/gate.sh), a note in everyone else's.

  python3 tests/known_reds.py split <red test>...   the known ones and the NEW ones, one line each
  python3 tests/known_reds.py --expire               the expired lines; rc 1 if one is $SB_AGENT's
  python3 tests/known_reds.py --self-test            the parser, the split and the expiry on synthetic data
"""
import glob
import json
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
FILE = os.path.join(HERE, "known_reds.txt")

# The reds that are never known: a line naming one (or a name under it) is refused, so no one
# lists it to get a desktop tag through (architect m_50511).
NEVER = {
    # the hostile MCP App through the real hub (docs/mcp-apps.md C6, risk 1): its red is a leak
    # out of a view's sandbox, fixed before any tag, never owned and shipped
    "desktop/mcp-apps-hostile": "a view leaking out of its sandbox is never a known red",
}


def parse(text):
    """{test: (since, owner, why)}; a malformed line raises ValueError with its number."""
    reds = {}
    for n, line in enumerate(text.splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) != 4 or not all(p.strip() for p in parts):
            raise ValueError(f"known_reds.txt:{n}: want <test>\\t<since>\\t<owner>\\t<why>, got {line!r}")
        test, since, owner, why = (p.strip() for p in parts)
        for never, reason in NEVER.items():
            if test.startswith(never):
                raise ValueError(f"known_reds.txt:{n}: {test} can't be a known red: {reason}")
        reds[test] = (since, owner, why)
    return reds


def split(reds, failed):
    """The lines that say which reds are known (with their owner) and which are new."""
    failed = list(dict.fromkeys(f.strip() for f in failed if f.strip()))
    known = [f for f in failed if f in reds]
    new = [f for f in failed if f not in reds]
    out = []
    if known:
        out.append("known reds (tests/known_reds.txt): " + ", ".join(f"{f} ({reds[f][1]}, since {reds[f][0]})" for f in known))
    if new:
        out.append("NEW reds (not in tests/known_reds.txt): " + ", ".join(new))
    return out


# a line expires after this many shas in a row ran its test green (one green run of a load flake
# says little; two in a row on later shas: it is not red on main now)
GREEN_SHAS = 2
SHA = re.compile(r"^[0-9a-f]{7,40}$")
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def expired(reds, results, after, shas=GREEN_SHAS):
    """[(test, (since, owner, why), [sha, ...])]: the lines whose test ran green on its last `shas`
    shas after since. results: {test: [(when, sha, green)]}; after(since, sha, when) -> bool. A sha
    with a red and a green result of the test is red. Pure."""
    out = []
    for test, line in reds.items():
        # each sha: its newest run and whether every run of it was green; newest first, after()
        # asked only until `shas` of them are found (it runs git)
        newest, green_all = {}, {}
        for when, sha, green in results.get(test, []):
            newest[sha] = max(newest.get(sha, when), when)
            green_all[sha] = green_all.get(sha, True) and green
        last = []
        for sha in sorted(newest, key=lambda s: newest[s], reverse=True):
            if after(line[0], sha, newest[sha]):
                last.append(sha)
                if len(last) == shas or not green_all[sha]:
                    break
        if len(last) == shas and all(green_all[s] for s in last):
            out.append((test, line, last[::-1]))
    return out


def expire_lines(found, agent):
    """(lines to print, rc): rc 1 when one of the expired lines is `agent`'s. Pure."""
    lines = []
    for test, (since, owner, _), last in found:
        mine = " (yours)" if owner == agent else ""
        lines.append("EXPIRED %s%s: owner %s, since %s; green on its last %d shas since (%s): remove the line, or "
                     "still red under load, give it a fresh since and why" % (test, mine, owner, since, len(last), " ".join(s[:9] for s in last)))
    return lines, 1 if any(line[1] == agent for _, line, _ in found) else 0


def _epoch(iso):
    try:
        return time.mktime(time.strptime(iso[:19], "%Y-%m-%dT%H:%M:%S"))
    except ValueError:
        return None


def _tsv(path):
    try:
        with open(path, errors="replace") as f:
            return [l.rstrip("\n").split("\t") for l in f if l.strip()]
    except OSError:
        return []


def _lines(path):
    try:
        with open(path, errors="replace") as f:
            return [l.strip() for l in f if l.strip()]
    except OSError:
        return None


def _checks_ran(d):
    """the desktop checks a run's dir ran: its report*.json specs' files, else its spec dirs
    (checks__thread__send.spec.ts), each as its name (send)"""
    names = set()
    for rep in glob.glob(os.path.join(d, "report*.json")):
        try:
            with open(rep) as f:
                for s in json.load(f).get("suites", []):
                    names.add(os.path.basename(s.get("file", "")).replace(".spec.ts", ""))
        except (OSError, ValueError, AttributeError):
            pass
    for p in glob.glob(os.path.join(d, "checks__*.spec.ts")):
        names.add(os.path.basename(p).split("__")[-1].replace(".spec.ts", ""))
    names.discard("")
    return names


def desktop_checks(root):
    """the desktop checks' names (apps/desktop/tests/checks/**/<name>.spec.ts): what test:land runs"""
    pat = os.path.join(root, "apps", "desktop", "tests", "checks", "**", "*.spec.ts")
    return {os.path.basename(p)[:-len(".spec.ts")] for p in glob.glob(pat, recursive=True)}


def results(home, tests, checks):
    """{test: [(when, sha, green)]} for the named tests, from the gate's and the nightly's records
    (the module doc says which); checks: the desktop checks' names"""
    tests = set(tests)
    res = {t: [] for t in tests}
    for r in _tsv(os.path.join(home, "gate", "timings.tsv")):
        if len(r) >= 8 and r[7] == "gate-full" and r[2] in tests and r[0].isdigit():
            res[r[2]].append((int(r[0]), r[1], r[3] == "0"))
    for r in _tsv(os.path.join(home, "run", "nightly", "checks-verdicts.tsv")):
        if len(r) < 4 or r[2] not in ("green", "red") or _epoch(r[0]) is None:
            continue
        red = _lines(os.path.join(r[3], "reds.txt"))
        if red is None and r[2] == "red":
            continue
        ran = _checks_ran(r[3]) | set(red or [])
        for t in tests & ran:
            res[t].append((_epoch(r[0]), r[1], t not in (red or [])))
    for r in _tsv(os.path.join(home, "run", "nightly", "desktop-runs.tsv")):
        if len(r) < 4 or r[2] not in ("green", "red") or _epoch(r[0]) is None:
            continue
        red = _lines(os.path.join(r[3], "reds.txt"))
        steps = dict(s[:2] for s in _tsv(os.path.join(r[3], "steps.tsv")) if len(s) >= 2)
        if red is None:
            continue
        for t in tests:
            m = re.match(r"^as-he-does/([a-z-]+): ", t)
            if m:
                ran = steps.get(m.group(1)) in ("green", "red")
            else:
                # a desktop check test:land ran: its step green, or red with its reds named
                ran = t in checks and (t in red or steps.get("land") == "green"
                                       or (steps.get("land") == "red" and "desktop/test:land" not in red))
            if ran:
                res[t].append((_epoch(r[0]), r[1], t not in red))
    return res


def git_after(repo):
    """after(since, sha, when): a sha since is the run's sha or its ancestor (git, cached); a date
    since is that day or later; anything else never"""
    memo = {}

    def after(since, sha, when):
        if DATE.match(since):
            return time.strftime("%Y-%m-%d", time.localtime(when)) >= since
        if not SHA.match(since) or not SHA.match(sha):
            return False
        k = (since, sha)
        if k not in memo:
            memo[k] = subprocess.run(["git", "-C", repo, "merge-base", "--is-ancestor", since, sha],
                                     capture_output=True).returncode == 0
        return memo[k]
    return after


def self_test():
    reds = parse("# test\tsince\towner\twhy\n\nhistory-perf\t12c3d88dc\tland-reds\tthe match moves\n")
    assert reds == {"history-perf": ("12c3d88dc", "land-reds", "the match moves")}, reds
    try:
        parse("a\tb\tc\n")
        raise AssertionError("3 fields parsed")
    except ValueError as e:
        assert "known_reds.txt:1" in str(e), e
    for leak in ("desktop/mcp-apps-hostile", "desktop/mcp-apps-hostile: nested-blob-escape"):
        try:
            parse(f"{leak}\tabc\tsomeone\tlater")
            raise AssertionError(f"{leak} parsed as a known red")
        except ValueError as e:
            assert "never a known red" in str(e), e
    assert split(reds, ["history-perf", "feature_e2e", "feature_e2e", ""]) == [
        "known reds (tests/known_reds.txt): history-perf (land-reds, since 12c3d88dc)",
        "NEW reds (not in tests/known_reds.txt): feature_e2e",
    ]
    assert split(reds, []) == []
    # the expiry: green on the last 2 shas after since; a red after a green keeps it; a sha with
    # a red and a green is red; runs before since don't count
    line = ("s0", "own", "why")
    order = {"s0": 0, "old": -1, "a": 1, "b": 2, "c": 3}
    after = lambda since, sha, when: order[sha] > order[since]  # noqa: E731
    R = lambda *rs: [(n, sha, g) for n, (sha, g) in enumerate(rs)]  # noqa: E731
    cases = {
        "fixed": (R(("a", False), ("b", True), ("c", True)), True),
        "red again": (R(("a", True), ("b", True), ("c", False)), False),
        "one green sha": (R(("a", True), ("a", True)), False),
        "mixed sha": (R(("b", True), ("c", True), ("c", False)), False),
        "before since": (R(("old", True), ("a", True)), False),
        "no record": ([], False),
    }
    found = expired({t: line for t in cases}, {t: c[0] for t, c in cases.items()}, after)
    got = {t for t, _, _ in found}
    assert got == {t for t, c in cases.items() if c[1]}, got
    assert found[0][2] == ["b", "c"], found
    lines, rc = expire_lines(found, "own")
    assert rc == 1 and lines[0].startswith("EXPIRED fixed (yours): owner own, since s0; green on its last 2 shas"), lines
    assert expire_lines(found, "other") == ([lines[0].replace(" (yours)", "")], 0)
    assert expire_lines([], "own") == ([], 0)
    # the records: a fake home with each kind of run (never the real ~/.bise)
    import tempfile
    with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as h:
        os.makedirs(os.path.join(h, "gate"))
        nd = os.path.join(h, "run", "nightly")
        cv, dr = os.path.join(nd, "cv1"), os.path.join(nd, "dr1")
        os.makedirs(cv)
        os.makedirs(dr)
        with open(os.path.join(h, "gate", "timings.tsv"), "w") as f:
            f.write("100\tabc1234\tfeature_e2e\t0\t1\t0\t3.0\tgate-full\n100\tabc1234\tother_e2e\t1\t1\t0\t3\tgate-full\n"
                    "101\tabc1234\tfeature_e2e\t1\t1\t0\t3\tadhoc\n")
        with open(os.path.join(nd, "checks-verdicts.tsv"), "w") as f:
            f.write("2026-11-07T15:58:50\tdef5678\tred\t%s\n2026-11-07T16:00:00\tdef5678\tnone\t%s\n" % (cv, cv))
        with open(os.path.join(cv, "reds.txt"), "w") as f:
            f.write("badge\n")
        with open(os.path.join(cv, "report.json"), "w") as f:
            json.dump({"suites": [{"file": "checks/native/badge.spec.ts"}, {"file": "checks/thread/send.spec.ts"}]}, f)
        with open(os.path.join(nd, "desktop-runs.tsv"), "w") as f:
            f.write("2026-11-07T15:46:19\tdef5678\tred\t%s\n" % dr)
        with open(os.path.join(dr, "reds.txt"), "w") as f:
            f.write("frame-csp\nas-he-does/full: a step\n")
        with open(os.path.join(dr, "steps.tsv"), "w") as f:
            f.write("quick\tgreen\nfull\tred\nland\tred\n")
        t0 = _epoch("2026-11-07T15:58:50")
        res = results(h, ["feature_e2e", "other_e2e", "badge", "send", "move-apps", "frame-csp",
                          "as-he-does/quick: x", "as-he-does/full: a step", "desktop_check: src/a.test.ts"],
                      {"badge", "send", "move-apps", "frame-csp"})
        assert res["feature_e2e"] == [(100, "abc1234", True)], res
        assert res["other_e2e"] == [(100, "abc1234", False)], res
        t1 = _epoch("2026-11-07T15:46:19")
        # a desktop check: from the checks run (its report) and test:land's (red, its reds named)
        assert sorted(res["badge"]) == [(t1, "def5678", True), (t0, "def5678", False)], res
        assert sorted(res["send"]) == [(t1, "def5678", True), (t0, "def5678", True)], res
        assert res["move-apps"] == [(t1, "def5678", True)], res
        assert [g for _, _, g in res["frame-csp"]] == [False], res
        assert [g for _, _, g in res["as-he-does/quick: x"]] == [True], res
        assert [g for _, _, g in res["as-he-does/full: a step"]] == [False], res
        assert res["desktop_check: src/a.test.ts"] == [], res
    with open(FILE) as f:
        parse(f.read())
    print("known_reds: self-test ok")


def main(argv):
    if argv[1:2] == ["--self-test"]:
        self_test()
        return 0
    if argv[1:2] == ["--expire"]:
        with open(FILE) as f:
            reds = parse(f.read())
        home = os.environ.get("BISE_HOME") or os.path.join(os.path.expanduser("~"), ".bise")
        root = os.path.dirname(HERE)
        found = expired(reds, results(home, reds, desktop_checks(root)), git_after(root))
        lines, rc = expire_lines(found, os.environ.get("SB_AGENT", ""))
        for line in lines:
            print(line)
        return rc
    if argv[1:2] != ["split"]:
        print(__doc__.strip().splitlines()[-2], file=sys.stderr)
        return 2
    with open(FILE) as f:
        reds = parse(f.read())
    for line in split(reds, argv[2:]):
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
