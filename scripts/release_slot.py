#!/usr/bin/env python3
"""release_slot: continuous releases (page release-cadence; Gabriel 2026-10-28: « une release dès
qu'il y a des changements [...] l'écart minimum entre deux releases, ça peut être genre deux
heures »). A timer (sb every, never launchd) wakes the release manager; this script decides
whether a cut starts now. It never cuts: the steps of a cut and who does each are in
docs/release-cadence.md. Every release goes out on its own: no word of his is asked (Gabriel
2026-10-28).

  scripts/release_slot.py [--min-gap-h 2] [--repo <tree>] [--now <epoch>]
      prints 'slot: cut <sha12> on <main|release/vX>' or 'slot: skip: <why>', then what it read: the
      release's tip (main's, or the open release line's while main holds a held commit:
      scripts/release_branch.py), the holds, the last
      published terminal release (its commit, its age), a terminal draft still out, a cut under
      way, the tip gate's verdict on the tip, the next terminal and desktop tags.
      Exit 0 cut, 1 skip, 2 an error (git, gh).
  scripts/release_slot.py start <sha>    a cut starts on <sha> (the marker below)
  scripts/release_slot.py end            the cut ended (published, or not this time, or skipped)

Skip, in this order: a cut is under way (the marker, younger than STALE_H; an older one is named
and ignored); main holds a held commit and no release line is open; a terminal draft is out (a cut
not finished, or one that stopped: one release at a time); nothing new on main (or the line) since the last published terminal release; nothing shipped changed (only docs/,
tests/, private/...: NOT_SHIPPED); the last terminal release was published less than --min-gap-h
ago (says when the next can start); the tip gate ran on the tip and found it red; the Mac is
busy (on battery, load >= MAX_LOAD, a quick gate running: the app's end-to-end steps go red
then with no real cause). A tip the tip
gate has not run on yet is a cut: the cut runs it (scripts/nightly_gate.sh --tip) first.

The terminal and the app go out together, on the same sha, terminal first: the app carries a
published terminal release, the v* tag on its own commit (desktop-release.yml).

Reads: git (main, release/*, tags), <BISE_HOME>/run/release/holds.json, gh (the releases of gvergnaud/bise), <BISE_HOME>/gate/green-shas.tsv,
<BISE_HOME>/run/nightly/tip-last, pmset, the load, ps. Its one state, which it owns: <BISE_HOME>/run/release/cut
('<sha> <epoch>', written by start, removed by end). Pure parts: tests/release_slot_test.py.
"""
import datetime
import json
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, HERE)
import release_branch  # noqa: E402
import tip_green  # noqa: E402

REPO_GH = "gvergnaud/bise"
STALE_H = 8
MAX_LOAD = 12
NOT_SHIPPED = ("docs/", "tests/", "private/", ".agents/", ".github/ISSUE_TEMPLATE/")
DESK_RE = re.compile(r"^desktop-v([0-9]{4})\.([0-9]{1,2})\.([0-9]{1,4})$")


def shipped(paths):
    """the changed paths a user can get: not under NOT_SHIPPED, not a top-level .md"""
    return [p for p in paths if p and not p.startswith(NOT_SHIPPED) and not ("/" not in p and p.endswith(".md"))]


def tip_verdict(sha, green, tip_last):
    """'green' | 'red' | 'not run': green-shas.tsv first, then tip-last's line for this sha"""
    if sha in green:
        return "green"
    f = tip_last.split()
    if len(f) >= 2 and f[0] == sha and f[1] == "red":
        return "red"
    return "not run"


def machine_busy(ac, load, quick):
    """why the Mac can't take a cut now, or None: on battery nightly_desktop.sh runs nothing; under
    load or beside a quick gate its end-to-end steps go red with no real cause (17ec33f84)"""
    if not ac:
        return "on battery"
    if load >= MAX_LOAD:
        return "load %.1f (a cut waits under %d)" % (load, MAX_LOAD)
    if quick:
        return "a quick gate is running"
    return None


def hm(epoch):
    return datetime.datetime.fromtimestamp(epoch).strftime("%H:%M")


def decide(now, s):
    """('cut'|'skip', why) for one wake. s: tip, last_sha, last_at (epoch), since (commits),
    shipped (paths), draft (tag or None), cut ((sha, epoch) or None), verdict, gap_h"""
    cut = s["cut"]
    if s.get("nowhere"):
        return "skip", s["nowhere"]
    if cut and now - cut[1] < STALE_H * 3600:
        return "skip", "a cut is under way on %s since %s" % (cut[0][:12], hm(cut[1]))
    if s["draft"]:
        return "skip", "the terminal draft %s is out (a cut not finished, or stopped): one release at a time" % s["draft"]
    if s["tip"] == s["last_sha"] or s["since"] == 0:
        return "skip", "nothing new on %s since the last release" % s.get("from", "main")
    if not s["shipped"]:
        return "skip", "%d commits since the last release, none in what users get (docs, tests only)" % s["since"]
    ready = s["last_at"] + s["gap_h"] * 3600
    if now < ready:
        return "skip", "the last release went out at %s: the next can start at %s" % (hm(s["last_at"]), hm(ready))
    if s["verdict"] == "red":
        return "skip", "%s's tip %s is red at the tip gate" % (s.get("from", "main"), s["tip"][:12])
    busy = machine_busy(s["ac"], s["load"], s["quick"])
    if busy:
        return "skip", "the Mac is not ready for a cut: %s" % busy
    return "cut", "%d commits since the last release (%d shipped files); tip gate: %s" % (
        s["since"], len(s["shipped"]), s["verdict"])


def next_desktop_tag(tags, today):
    """desktop-vYYYY.M.N: N one past the highest of this month, else 1"""
    ns = [int(m.group(3)) for m in map(DESK_RE.match, tags)
          if m and int(m.group(1)) == today.year and int(m.group(2)) == today.month]
    return "desktop-v%d.%d.%d" % (today.year, today.month, max(ns, default=0) + 1)


def gh_time(s):
    return datetime.datetime.strptime(s, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc).timestamp()


def sh(argv, cwd):
    r = subprocess.run(argv, cwd=cwd, capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError("%s: %s" % (" ".join(argv[:3]), (r.stderr or r.stdout).strip()[:300]))
    return r.stdout.strip()


def read(path):
    try:
        return open(path).read().strip()
    except OSError:
        return ""


def marker(bh):
    return os.path.join(bh, "run", "release", "cut")


def read_cut(path):
    f = read(path).split()
    return (f[0], int(f[1])) if len(f) == 2 and f[1].isdigit() else None


def gather(repo, bh, gap_h):
    sh(["git", "fetch", "-q", "--tags", "origin"], repo)
    # the release's sha: main's tip, or the open release line's when main holds a held commit
    # (scripts/release_branch.py: holds.json, the release/* refs)
    line = release_branch.Repo(repo)
    if not line.main:
        raise RuntimeError("no branch main in %s" % repo)
    held = release_branch.reached(release_branch.read_holds(bh), lambda h: release_branch.is_ancestor(repo, h, line.main))
    where, tip = release_branch.cut_base(line.main, held, line.line, line.line_tip)
    nowhere = tip if where == "none" else None
    if where != "line":
        tip = line.main
    rels = json.loads(sh(["gh", "release", "list", "-R", REPO_GH, "--limit", "40",
                          "--json", "tagName,isDraft,publishedAt"], repo))
    term = [r for r in rels if r["tagName"].startswith("v")]
    last = next((r for r in term if not r["isDraft"]), None)
    if last is None:
        raise RuntimeError("no published terminal release on %s" % REPO_GH)
    last_sha = sh(["git", "rev-list", "-n1", last["tagName"]], repo)
    files = sh(["git", "diff", "--name-only", last_sha, tip], repo).splitlines()
    return {
        "tip": tip, "from": line.line if where == "line" else "main", "nowhere": nowhere,
        "holds": ["%s (%s)" % (h["sha"][:9], h["why"]) for h in held], "line": line.line,
        "last": last["tagName"], "last_sha": last_sha, "last_at": gh_time(last["publishedAt"]),
        "since": int(sh(["git", "rev-list", "--count", "%s..%s" % (last_sha, tip)], repo)),
        "shipped": shipped(files),
        "draft": next((r["tagName"] for r in term if r["isDraft"]), None),
        "cut": read_cut(marker(bh)),
        "verdict": tip_verdict(tip, tip_green.green_shas(os.path.join(bh, "gate", "green-shas.tsv")),
                               read(os.path.join(bh, "run", "nightly", "tip-last"))),
        "gap_h": gap_h,
        "ac": "AC Power" in sh(["pmset", "-g", "batt"], repo),
        "load": os.getloadavg()[0],
        "quick": bool(re.search(r"gate\.sh( quick)?\s*$", sh(["ps", "-axo", "command"], repo), re.M)),
        "subject": sh(["git", "log", "-1", "--format=%s", tip], repo),
        "next_term": sh([os.path.join(repo, "packaging", "publish-release.sh"), "--next-tag"], repo).splitlines()[-1],
        "desk_tags": sh(["git", "tag", "-l", "desktop-v*"], repo).split(),
    }


def main(argv):
    args = list(argv)
    bh = os.environ.get("BISE_HOME") or os.path.join(os.path.expanduser("~"), ".bise")
    if args[:1] == ["start"] and len(args) == 2:
        os.makedirs(os.path.dirname(marker(bh)), exist_ok=True)
        with open(marker(bh), "w") as f:
            f.write("%s %d\n" % (args[1], int(time.time())))
        print("cut started on %s" % args[1][:12])
        return 0
    if args == ["end"]:
        if os.path.exists(marker(bh)):
            os.remove(marker(bh))
        print("cut ended")
        return 0
    opts = {"--min-gap-h": "2", "--repo": os.path.dirname(HERE), "--now": None}
    while args:
        k = args.pop(0)
        if k not in opts or not args:
            print(__doc__)
            return 2
        opts[k] = args.pop(0)
    now = float(opts["--now"]) if opts["--now"] else time.time()
    try:
        s = gather(opts["--repo"], bh, float(opts["--min-gap-h"]))
    except (RuntimeError, ValueError, OSError) as e:
        print("release_slot: %s" % e)
        return 2
    what, why = decide(now, s)
    print("slot: cut %s on %s" % (s["tip"][:12], s["from"]) if what == "cut" else "slot: skip: %s" % why)
    if what == "cut":
        print("why: %s" % why)
    print("%s: %s %s" % (s["from"], s["tip"][:12], s["subject"]))
    print("holds on main: %s" % (", ".join(s["holds"]) or "none"))
    print("open release line: %s%s" % (s["line"] or "none", "" if s["from"] != "main" or not s["line"] else
                                      " (no hold reaches main: a cut starts a new line on main's tip)"))
    print("last terminal release: %s on %s, published %s, %d commits since (%d shipped files)" % (
        s["last"], s["last_sha"][:12], datetime.datetime.fromtimestamp(s["last_at"]).strftime("%Y-%m-%d %H:%M"),
        s["since"], len(s["shipped"])))
    print("terminal draft out: %s" % (s["draft"] or "none"))
    cut = s["cut"]
    print("cut under way: %s" % ("%s since %s%s" % (cut[0][:12], hm(cut[1]),
                                 " (stale: ignored)" if now - cut[1] >= STALE_H * 3600 else "") if cut else "none"))
    print("tip gate on %s: %s" % (s["from"], s["verdict"]))
    print("Mac: %s" % (machine_busy(s["ac"], s["load"], s["quick"]) or "ready (AC, load %.1f, no quick gate)" % s["load"]))
    print("next tags: %s, then %s (terminal first, same sha)" % (
        s["next_term"], next_desktop_tag(s["desk_tags"], datetime.date.fromtimestamp(now))))
    return 0 if what == "cut" else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
