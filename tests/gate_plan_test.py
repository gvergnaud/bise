#!/usr/bin/env python3
"""tests/gate_plan.py's plan on fixtures: changed paths in, the gate's steps out (no process, < 1 s).

Each case of tests/fixtures/gate-plan/cases.json gives paths (and what the tree has: the files
still there, the compile-time includes, whether apps/desktop exists) and the plan fields it
wants; a field it does not name is not checked.

  python3 -B tests/gate_plan_test.py
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gate_plan  # noqa: E402

CASES = os.path.join(HERE, "fixtures", "gate-plan", "cases.json")


def diff(want, got, at=""):
    """The fields of want that got does not match (a dict: only its keys)."""
    if isinstance(want, dict) and isinstance(got, dict):
        return [d for k, v in want.items() for d in diff(v, got.get(k), at + "." + k)]
    return [] if want == got else ["%s: want %r, got %r" % (at or "plan", want, got)]


def main():
    with open(CASES) as fh:
        cases = json.load(fh)
    fails = []
    for c in cases:
        tree = gate_plan.Tree(includes=c.get("includes"), exists=c.get("exists"), desktop=c.get("desktop", True))
        got = gate_plan.plan(c["paths"], tree)
        bad = diff(c["want"], got)
        print(("ok   " if not bad else "FAIL ") + c["name"])
        for b in bad:
            print("     " + b)
        fails += bad
    # the shell lines gate.sh evals: every field, quoted
    sh = gate_plan.to_sh(gate_plan.plan(["apps/desktop/src/a b.ts"], gate_plan.Tree(exists=["apps/desktop/src/a b.ts"])))
    want = "drel='src/a b.ts'"
    if want not in sh.splitlines():
        fails.append("to_sh: no line %s in %r" % (want, sh))
    if fails:
        sys.exit("gate_plan_test: %d failed" % len(fails))
    print("gate_plan_test: ok (%d cases)" % len(cases))


if __name__ == "__main__":
    main()
