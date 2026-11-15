#!/usr/bin/env python3
"""scripts/release_slot.py: the decision of one wake of the continuous releases (cut or skip and
why), what counts as shipped, the next desktop tag. Pure parts only: no git, no gh."""
import datetime
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import release_slot as R  # noqa: E402

FAILS = []


def check(ok, what):
    if not ok:
        FAILS.append(what)


def state(**kw):
    s = {"tip": "t" * 40, "last_sha": "l" * 40, "last_at": 0, "since": 3, "shipped": ["rust/tui/src/a.rs"],
         "draft": None, "cut": None, "verdict": "not run", "gap_h": 2,
         "ac": True, "load": 3.0, "quick": False}
    s.update(kw)
    return s


def main():
    H = 3600
    now = 10 * H
    check(R.decide(now, state()) == ("cut", "3 commits since the last release (1 shipped files); tip gate: not run"),
          "cut %s" % (R.decide(now, state()),))
    w, why = R.decide(now, state(cut=("c" * 40, now - H)))
    check(w == "skip" and "under way" in why, "cut under way %s" % why)
    check(R.decide(now, state(cut=("c" * 40, now - R.STALE_H * H)))[0] == "cut", "a stale marker is ignored")
    w, why = R.decide(now, state(draft="v2026.10.28"))
    check(w == "skip" and "v2026.10.28" in why, "draft out %s" % why)
    check(R.decide(now, state(tip="l" * 40))[0] == "skip", "tip is the last release")
    w, why = R.decide(now, state(nowhere="main holds h1 (tasks) and no line is open"))
    check(w == "skip" and "no line is open" in why, "held, no line: skip %s" % why)
    w, why = R.decide(now, state(since=0, **{"from": "release/v2026.11.15"}))
    check(w == "skip" and "release/v2026.11.15" in why, "nothing new on the line %s" % why)
    check(R.decide(now, state(since=0))[0] == "skip", "no commit since")
    w, why = R.decide(now, state(shipped=[]))
    check(w == "skip" and "none in what users get" in why, "nothing shipped %s" % why)
    w, why = R.decide(now, state(last_at=now - H))
    check(w == "skip" and "next can start at %s" % R.hm(now + H) in why, "gap %s" % why)
    check(R.decide(now, state(last_at=now - 2 * H))[0] == "cut", "exactly the gap is a cut")
    w, why = R.decide(now, state(verdict="red"))
    check(w == "skip" and "red" in why, "red tip %s" % why)
    check(R.decide(now, state(verdict="green"))[0] == "cut", "green tip")
    for kw, word in (({"ac": False}, "on battery"), ({"load": 12.0}, "load 12.0"), ({"quick": True}, "quick gate")):
        w, why = R.decide(now, state(**kw))
        check(w == "skip" and word in why, "busy %s %s" % (kw, why))
    check(R.decide(now, state(load=11.9))[0] == "cut", "load under the cap")

    check(R.shipped(["docs/a.md", "tests/x.py", "private/p", "README.md", "apps/desktop/src/a.ts", "scripts/x.py"])
          == ["apps/desktop/src/a.ts", "scripts/x.py"], "shipped %s" % R.shipped(["docs/a.md", "apps/desktop/src/a.ts"]))
    a = "a" * 40
    check(R.tip_verdict(a, [a], "") == "green", "green-shas")
    check(R.tip_verdict(a, [], "%s red 900 /log" % a) == "red", "tip-last red")
    check(R.tip_verdict(a, [], "%s red 900 /log" % ("b" * 40)) == "not run", "another sha's red")
    d = datetime.date(2026, 10, 28)
    check(R.next_desktop_tag(["desktop-v2026.10.21", "desktop-v2026.10.3", "desktop-v2026.9.40", "v2026.10.28"], d)
          == "desktop-v2026.10.22", "next desktop tag")
    check(R.next_desktop_tag(["desktop-v2026.10.21"], datetime.date(2026, 11, 2)) == "desktop-v2026.11.1", "new month")
    if FAILS:
        print("FAIL release_slot_test:\n  " + "\n  ".join(FAILS))
        return 1
    print("ok release_slot_test")
    return 0


if __name__ == "__main__":
    sys.exit(main())
