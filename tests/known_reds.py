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

  python3 tests/known_reds.py split <red test>...   the known ones and the NEW ones, one line each
  python3 tests/known_reds.py --self-test            the parser and the split on synthetic data
"""
import os
import sys

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
    with open(FILE) as f:
        parse(f.read())
    print("known_reds: self-test ok")


def main(argv):
    if argv[1:2] == ["--self-test"]:
        self_test()
        return 0
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
