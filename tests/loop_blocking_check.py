"""Nothing slow on the hub's loop (chantier c4-loop-never-waits): in the
loop's modules (rust/switchboard/src/daemon.rs, daemon/**, core*.rs), a
process run to its end (`.output()`, `.status()` of a Command), a
`thread::sleep` or a whole read of a wire.log is refused outside a
closure given to a `spawn(...)` and the functions of
tests/loop_blocking.txt. A slow thing goes to a worker that sends a Msg
back (daemon/place_work.rs's model).

  python3 tests/loop_blocking_check.py              check (run_all.sh, gate.sh quick)
  python3 tests/loop_blocking_check.py --self-test  it refuses planted cases

tests/loop_blocking.txt: `<file>::<fn> | <n> | off|loop | <why>`, one per
function, n its hits. `off`: it never runs on the loop (a worker's
thread, a start before the loop, a stop after it, a test knob). `loop`:
it still holds the loop, the debt this chantier pays; its why names the
land that takes it off. A function with more hits than its n is refused,
with fewer too (lower n, or delete the line: the debt is paid).

What it cannot see: a blocking call in a library module (worktree.rs,
land.rs, ...) that the loop calls; the measure (scripts/hub_loop_held.py
on hub.log) does.
"""
import glob
import os
import re
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SRC = os.path.join("rust", "switchboard", "src")
SLOW = re.compile(r"\.output\(\)|\.status\(\)|\bthread::sleep\(")
# a `.status()` / `.output()` is a process when its statement builds a Command
PROCESS = re.compile(r"Command::new|_command\(|\.args?\(|\.std(?:in|out|err)\(|\.current_dir\(|\.envs?\(|\.env_clear\(|\bcmd\b|\bcommand\b")
READ = re.compile(r"\bfs::read(?:_to_string)?\(")
HELP = ("a slow call on the hub's loop: run it in a worker that sends a Msg back "
        "(daemon/place_work.rs), or, when it never runs on the loop, add its "
        "function to tests/loop_blocking.txt with why")


def loop_files(root):
    """The loop's modules, tests left out."""
    src = os.path.join(root, SRC)
    fs = [os.path.join(src, "daemon.rs")]
    fs += glob.glob(os.path.join(src, "daemon", "**", "*.rs"), recursive=True)
    fs += glob.glob(os.path.join(src, "core*.rs"))
    return sorted(f for f in fs if os.path.isfile(f) and "tests" not in os.path.basename(f)
                  and os.sep + "tests" + os.sep not in f)


def blank(src):
    """src with comments and string/char literals blanked (same offsets)."""
    out, i, n = [], 0, len(src)
    keep = lambda t: re.sub(r"[^\n]", " ", t)  # noqa: E731
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
        elif src.startswith("/*", i):
            j = src.find("*/", i)
            j = n if j < 0 else j + 2
        elif c == "r" and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")) and re.match(r'r#*"', src[i:i + 8]):
            m = re.match(r'r(#*)"', src[i:i + 8])
            j = src.find('"' + m.group(1), i + len(m.group(0)))
            j = n if j < 0 else j + 1 + len(m.group(1))
        elif c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append('"' + keep(src[i + 1:j]) + '"')
            i = j + 1
            continue
        elif c == "'" and re.match(r"'(\\.|[^\\'])'", src[i:i + 4]):
            j = i + len(re.match(r"'(\\.|[^\\'])'", src[i:i + 4]).group(0))
        else:
            out.append(c)
            i += 1
            continue
        out.append(keep(src[i:j]))
        i = j
    return "".join(out)


def close(s, i, a, b):
    """The index of the bracket b that closes the a at s[i]."""
    d = 0
    for k in range(i, len(s)):
        d += (s[k] == a) - (s[k] == b)
        if d == 0:
            return k
    return len(s)


def hits(path, text):
    """[(fn, line)] of the slow calls of one file outside a spawn's closure."""
    s = blank(text)
    m = re.search(r"#\[cfg\(test\)\]\s*mod\s+\w+\s*\{", s)
    if m:
        s = s[:m.start()]
    spawned = [(m.end(), close(s, m.end() - 1, "(", ")")) for m in re.finditer(r"\bspawn\s*\(", s)]
    fns = []
    for m in re.finditer(r"\bfn\s+(\w+)", s):
        j = m.end()
        while j < len(s) and s[j] not in "{;":
            j += 1
        if j < len(s) and s[j] == "{":
            fns.append((j, close(s, j, "{", "}"), m.group(1)))
    found = []
    for m in SLOW.finditer(s):
        if m.group(0) != "thread::sleep(":
            stmt = s[max(s.rfind(";", 0, m.start()), 0):m.start()]
            if not PROCESS.search(stmt):
                continue
        found.append(m.start())
    for m in READ.finditer(s):
        arg = text[m.end():close(s, m.end() - 1, "(", ")")]
        if re.search(r"wire(?!\.offset)", arg):
            found.append(m.start())
    out = []
    for p in sorted(found):
        if any(a <= p < b for a, b in spawned):
            continue
        inner = [n for a, b, n in fns if a <= p < b]
        out.append((inner[-1] if inner else "?", s.count("\n", 0, p) + 1))
    return out


def listed(path):
    """{"<file>::<fn>": (n, kind)}."""
    out = {}
    for k, l in enumerate(open(path, encoding="utf-8"), 1):
        if not l.strip() or l.lstrip().startswith("#"):
            continue
        parts = [p.strip() for p in l.rstrip("\n").split(" | ")]
        if len(parts) != 4 or not all(parts) or parts[2] not in ("off", "loop") or not parts[1].isdigit():
            raise SystemExit("%s:%d: want `<file>::<fn> | <n> | off|loop | <why>`: %r" % (path, k, l))
        out[parts[0]] = (int(parts[1]), parts[2])
    return out


def check(root, allowed):
    """The refused lines, one per problem."""
    got = {}
    for f in loop_files(root):
        rel = os.path.relpath(f, os.path.join(root, SRC))
        for fn, line in hits(f, open(f, encoding="utf-8").read()):
            got.setdefault("%s::%s" % (rel, fn), []).append(line)
    bad = []
    for key, lines in sorted(got.items()):
        n = allowed.get(key, (0, ""))[0]
        if len(lines) > n:
            bad.append("%s: %d slow call(s) (lines %s), %d listed" % (key, len(lines), ", ".join(map(str, lines)), n))
    for key, (n, kind) in sorted(allowed.items()):
        have = len(got.get(key, []))
        if have < n:
            bad.append("%s: %d listed, %d left: lower its n or delete its line (%s)" % (key, n, have, kind))
    return bad


def self_test():
    with tempfile.TemporaryDirectory() as d:
        src = os.path.join(d, SRC)
        os.makedirs(os.path.join(src, "daemon"))

        def put(name, text):
            open(os.path.join(src, name), "w").write(text)
        put("daemon.rs", "fn tick(&mut self) {\n    let o = Command::new(\"git\").arg(\"x\").output();\n"
                         "    let st = a.status();\n    // thread::sleep(x) in a comment\n"
                         "    let s = \"thread::sleep(\";\n    std::thread::spawn(move || {\n"
                         "        std::thread::sleep(d);\n        let b = std::fs::read(&wire);\n    });\n}\n")
        put("daemon/repl.rs", "fn busy_at(wire: &Path) -> bool {\n    let Ok(b) = std::fs::read(wire) else { return false };\n"
                              "    let off = std::fs::read_to_string(adir.join(\"wire.offset\"));\n}\n"
                              "#[cfg(test)]\nmod tests {\n    fn t() { std::thread::sleep(d); }\n}\n")
        put("core_x_tests.rs", "fn t() { std::thread::sleep(d); }\n")
        got = check(d, {})
        assert got == ["daemon.rs::tick: 1 slow call(s) (lines 2), 0 listed",
                       "daemon/repl.rs::busy_at: 1 slow call(s) (lines 2), 0 listed"], got
        assert check(d, {"daemon.rs::tick": (1, "loop"), "daemon/repl.rs::busy_at": (1, "loop")}) == []
        got = check(d, {"daemon.rs::tick": (2, "loop"), "daemon/repl.rs::busy_at": (1, "loop")})
        assert got == ["daemon.rs::tick: 2 listed, 1 left: lower its n or delete its line (loop)"], got
    print("loop_blocking_check self-test: a planted git and a whole wire.log read refused; "
          "a spawn's closure, comments, strings, tests and agent.status() pass")


def main():
    if "--self-test" in sys.argv:
        self_test()
        return
    bad = check(ROOT, listed(os.path.join(HERE, "loop_blocking.txt")))
    if bad:
        print("\n".join(bad))
        sys.exit("FAILED loop blocking check: %d function(s): %s" % (len(bad), HELP))
    print("loop blocking check: nothing slow on the hub's loop past tests/loop_blocking.txt")


if __name__ == "__main__":
    main()
