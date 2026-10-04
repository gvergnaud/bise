"""No blind sleep in the tests (docs/issues/10-tests-wait.md): a
`time.sleep(` (or `from time import sleep`) in tests/*.py is refused
outside tests/wait.py and the reviewed lines of tests/sleep_exceptions.txt.
A test waits for a state with wait.until / wait.holds / wait.stable.

python3 tests/sleep_check.py              check tests/ (run_all.sh, gate.sh quick)
python3 tests/sleep_check.py --self-test  it refuses a planted time.sleep(2),
                                          passes wait.py and an excepted line
"""
import glob
import os
import re
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SLEEP = re.compile(r"\btime\.sleep\(|\bfrom\s+time\s+import\b[^#\n]*\bsleep\b")
HELP = ("a blind sleep: wait for the state with wait.until(fn, timeout, what), "
        "wait.holds for 'nothing happens' or wait.stable (tests/wait.py); a sleep "
        "that is the behavior under test goes in tests/sleep_exceptions.txt, with why")


def exceptions(path):
    """{(file name, the line's text stripped)}: `<file> | <line> | <why>`
    per line; # comments and blank lines skipped; a line without its why
    is refused."""
    out = set()
    for n, l in enumerate(open(path), 1):
        if not l.strip() or l.lstrip().startswith("#"):
            continue
        parts = [p.strip() for p in l.rstrip("\n").split(" | ")]
        if len(parts) != 3 or not all(parts):
            raise SystemExit("%s:%d: want `<file> | <line> | <why>`: %r" % (path, n, l))
        out.add((parts[0], parts[1]))
    return out


def blind(folder, allowed):
    """The `<file>:<n>: <line>` of every refused sleep in folder/*.py."""
    bad = []
    for f in sorted(glob.glob(os.path.join(folder, "*.py"))):
        name = os.path.basename(f)
        if name in ("wait.py", "sleep_check.py"):   # the waits; this check's own words
            continue
        for n, l in enumerate(open(f, encoding="utf-8"), 1):
            if SLEEP.search(l) and (name, l.strip()) not in allowed:
                bad.append("tests/%s:%d: %s" % (name, n, l.strip()))
    return bad


def self_test():
    with tempfile.TemporaryDirectory() as d:
        def put(name, text):
            open(os.path.join(d, name), "w").write(text)
        put("wait.py", open(os.path.join(HERE, "wait.py")).read())
        put("t_ok.py", "import wait\nwait.until(lambda: 1, 5, 'one')\n"
                       "proc.sleep(2)  # not time's\nprint('sleep 2 in a shell line')\n")
        put("t_fake.py", "import time\n    time.sleep(float(slow))\n")
        allowed = {("t_fake.py", "time.sleep(float(slow))")}
        assert blind(d, allowed) == [], blind(d, allowed)
        put("t_blind.py", "import time\nt.keys('Enter')\ntime.sleep(2)\nassert done()\n")
        put("t_import.py", "from time import monotonic, sleep\n")
        got = blind(d, allowed)
        assert got == ["tests/t_blind.py:3: time.sleep(2)",
                       "tests/t_import.py:1: from time import monotonic, sleep"], got
        # the excepted line, moved to another file: refused there
        put("t_other.py", "time.sleep(float(slow))\n")
        assert blind(d, allowed)[-1] == "tests/t_other.py:1: time.sleep(float(slow))", blind(d, allowed)
    print("sleep_check self-test: a planted time.sleep(2) is refused, wait.py and an excepted line pass")


def main():
    if "--self-test" in sys.argv:
        self_test()
        return
    bad = blind(HERE, exceptions(os.path.join(HERE, "sleep_exceptions.txt")))
    if bad:
        print("\n".join(bad))
        sys.exit("FAILED sleep check: %d line(s): %s" % (len(bad), HELP))
    print("sleep check: no blind time.sleep in tests/")


if __name__ == "__main__":
    main()
