#!/usr/bin/env python3
"""tip_blame: who to tell when the tip gate (scripts/nightly_gate.sh --tip) is red on main
(review-gates m_36198, main m_36204, Gabriel's « merger et tester après sur main »).

  scripts/tip_blame.py --repo <tree> --tested <sha> --green <green-shas.tsv> --log <gate log>
                       [--journal <journal.jsonl>]... [--out <tip-red>]

The commits last-green..tested (last green: the newest sha of green-shas.tsv that is an ancestor of
the tested one), each with its subject and the agent that landed it: the hub journals'
'✓ <agent> landed <n> commit(s) on main (<sha>)' lines (sb land's own line, in the message that
ends the agent's land), <sha> and its n-1 parents being that agent's. Then the failing tests (the
log's FAIL lines). Out: the tip-red file, written whole (tmp + mv), main forwards it to its 'to:'
line's agents (main m_36219: main's watch on it, sb wake --on-file, deletes it once sent):
  to: <agent> <agent> main
  tip gate red on main <sha12> (<date>)
  commits since the last green <sha12>: ...
No --out: printed. Tests: tests/tip_blame_test.py."""
import argparse
import os
import re
import subprocess
import sys
import time

LANDED = re.compile(r"✓ ([A-Za-z0-9@._-]+) landed (\d+) commits? on main \(([0-9a-f]{7,40})\)")
JOURNAL_TAIL = 64 * 1024 * 1024


def git(repo, *args):
    r = subprocess.run(["git", "-C", repo, *args], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else None


def landed(texts):
    """[(agent, n, sha)] from the journals' text, in order (a later line wins for a sha)"""
    out = []
    for t in texts:
        for m in LANDED.finditer(t):
            out.append((m.group(1), int(m.group(2)), m.group(3)))
    return out


def owners(repo, lands):
    """{full sha: agent}: each land's sha and its n-1 first parents"""
    own = {}
    for agent, n, sha in lands:
        shas = git(repo, "rev-list", "--first-parent", "-n", str(max(n, 1)), sha)
        for s in (shas or "").split():
            own[s] = agent
    return own


def last_green(repo, green_tsv, tested):
    """the newest sha of green-shas.tsv that is an ancestor of `tested` (itself included), or None"""
    try:
        shas = [l.split("\t", 1)[0] for l in open(green_tsv).read().splitlines() if l.strip()]
    except OSError:
        return None
    for s in reversed(shas):
        if subprocess.run(["git", "-C", repo, "merge-base", "--is-ancestor", s, tested], capture_output=True).returncode == 0:
            return s
    return None


def fails(log_text):
    """the failing tests and steps: the log's FAIL lines, their names"""
    names = []
    for l in log_text.splitlines():
        if l.startswith("FAIL "):
            n = l[5:].split(":", 1)[0].strip()
            if n and n not in names:
                names.append(n)
    return names


def report(repo, tested, green_tsv, log, journals, now=None, what="tip gate"):
    """(agents, text): the tip-red file's content"""
    green = last_green(repo, green_tsv, tested)
    rng = "%s..%s" % (green, tested) if green else tested
    lines = git(repo, "log", "--first-parent", "--format=%H %s", rng, *([] if green else ["-n", "20"])) or ""
    own = owners(repo, landed(journal_texts(journals)))
    commits, agents = [], []
    for l in lines.splitlines():
        sha, _, subject = l.partition(" ")
        who = own.get(sha)
        commits.append("  %s %s (%s)" % (sha[:9], subject[:100], who or "no land line: main's or a hand commit"))
        if who and who not in agents:
            agents.append(who)
    try:
        log_text = open(log, errors="replace").read()
    except OSError:
        log_text = ""
    red = fails(log_text)
    when = time.strftime("%Y-%m-%d %H:%M", time.localtime(now or time.time()))
    text = "\n".join([
        "to: %s" % " ".join(agents + ["main"]),
        "%s red on main %s (%s): %s" % (what, tested[:9], when, " ".join(red) or "no FAIL line, see the log"),
        "commits since the last green %s:" % (green[:9] if green else "(none known: the last 20)"),
        *commits,
        "log: %s" % log,
        "your land in this range? revert it first if the red is yours, then fix (main m_36091)",
    ]) + "\n"
    return agents, text


FLAKY = re.compile(r"^FLAKY (\S+) (\S+) ")
FLAKE_DAYS = 14


def flakes(log_text):
    """[(kind, test)]: the log's 'FLAKY <kind> <test> (red, then green alone)' lines (run_all.sh's
    rerun), in order, once each"""
    out = []
    for l in log_text.splitlines():
        m = FLAKY.match(l)
        if m and (m.group(1), m.group(2)) not in out:
            out.append((m.group(1), m.group(2)))
    return out


def flake_rates(timings, flaky, now, days=FLAKE_DAYS):
    """{test: (flaky, runs)} over the last `days` days: runs, its rows in timings.tsv's gate-full
    rows ([epoch, rev, test, rc, ..., kind]); flaky, its rows in flaky.tsv ([epoch, rev, kind,
    test]: run_all.sh's red, then green alone). Pure; scripts/loop_audit.py reads it too."""
    since = now - days * 86400
    out = {}
    for r in timings:
        if len(r) >= 8 and r[7] == "gate-full" and r[0].isdigit() and int(r[0]) >= since:
            f, n = out.get(r[2], (0, 0))
            out[r[2]] = (f, n + 1)
    for r in flaky:
        if len(r) >= 4 and r[0].isdigit() and int(r[0]) >= since:
            f, n = out.get(r[3], (0, 0))
            out[r[3]] = (f + 1, max(n, f + 1))
    return out


def tsv_rows(path):
    try:
        with open(path, errors="replace") as f:
            return [l.rstrip("\n").split("\t") for l in f if l.strip()]
    except OSError:
        return []


def test_file(repo, kind, test):
    """the file of a flaky test: tests/<test>.py for an e2e/tmux one; a cargo test's module path
    names none"""
    if kind == "e2e":
        for f in ("tests/%s.py" % test, "tests/%s.sh" % test):
            if os.path.exists(os.path.join(repo, f)):
                return f
    return None


def flake_owner(repo, kind, test, own):
    """who owns a flake: the agent that last landed a change to its test's file (own: {sha:
    agent}), else main"""
    f = test_file(repo, kind, test)
    for sha in ((git(repo, "log", "--format=%H", "-n", "20", "main", "--", f) or "").split() if f else []):
        if sha in own:
            return own[sha]
    return "main"


def flaky_report(repo, tested, log, journals, timings, now=None, what="tip gate"):
    """(agents, text) of the flaky-open file, or (None, None) when the log has no FLAKY line: each
    flake with its rate over 14 days and its owner"""
    try:
        log_text = open(log, errors="replace").read()
    except OSError:
        log_text = ""
    fl = flakes(log_text)
    if not fl:
        return None, None
    now = now or time.time()
    own = owners(repo, landed(journal_texts(journals)))
    rates = flake_rates(tsv_rows(timings), tsv_rows(os.path.join(os.path.dirname(timings), "flaky.tsv")), now)
    agents, body = [], []
    for kind, test in fl:
        who = flake_owner(repo, kind, test, own)
        n, runs = rates.get(test, (0, 0))
        body.append("  %s (%s): flaky in %d of %d full gates in %d days, owner %s" % (test, kind, n, runs, FLAKE_DAYS, who))
        if who != "main" and who not in agents:
            agents.append(who)
    when = time.strftime("%Y-%m-%d %H:%M", time.localtime(now))
    text = "\n".join([
        "to: %s" % " ".join(agents + ["main"]),
        "%s on main %s (%s): %d flaky (red, then green alone)" % (what, tested[:9], when, len(fl)),
        *body,
        "log: %s" % log,
        "a flake is a bug (bise-testing rule 8): find the race and make it an input, or say why in tests/known_reds.txt",
    ]) + "\n"
    return agents, text


def journal_texts(journals):
    """the last JOURNAL_TAIL bytes of each journal"""
    texts = []
    for j in journals:
        try:
            with open(j, "rb") as f:
                f.seek(max(0, os.path.getsize(j) - JOURNAL_TAIL))
                texts.append(f.read().decode("utf-8", "replace"))
        except OSError:
            pass
    return texts


def merged(old, new):
    """a report main has not forwarded yet (the file still there) and a new one: one 'to:' line, every
    agent of both (main last), then both bodies"""
    def split(t):
        lines = t.splitlines()
        to = lines[0][3:].split() if lines and lines[0].startswith("to:") else []
        return [w for w in to if w != "main"], lines[1:] if to or (lines and lines[0].startswith("to:")) else lines
    a, ab = split(old)
    b, bb = split(new)
    to = a + [w for w in b if w not in a] + ["main"]
    return "\n".join(["to: " + " ".join(to), *ab, "", *bb]) + "\n"


def main(argv):
    a = argparse.ArgumentParser()
    a.add_argument("--repo", required=True)
    a.add_argument("--tested", required=True)
    a.add_argument("--green", required=True)
    a.add_argument("--log", required=True)
    a.add_argument("--journal", action="append", default=[])
    a.add_argument("--out")
    a.add_argument("--what", default="tip gate")
    a.add_argument("--flaky", metavar="TIMINGS", help="write the flakes (flaky-open) instead of the reds; timings.tsv for their rate")
    o = a.parse_args(argv)
    tested = git(o.repo, "rev-parse", o.tested) or o.tested
    if o.flaky:
        _, text = flaky_report(o.repo, tested, o.log, o.journal, o.flaky, what=o.what)
        if text is None:
            return 0
    else:
        _, text = report(o.repo, tested, o.green, o.log, o.journal, what=o.what)
    if o.out:
        text = merged(open(o.out).read(), text) if os.path.exists(o.out) else text
        with open(o.out + ".tmp", "w") as f:
            f.write(text)
        os.replace(o.out + ".tmp", o.out)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
