#!/usr/bin/env python3
"""bins.sh path on a cache miss, in a temp cache (bug-bins-path).

The bug: `bins.sh path <name>` compiled, said "built in N s", then exited 1
with nothing on stdout; the next call (a hit) printed the path. After a
build, the prune loop of the cache ended on `[ -n "" ] && rm` (a file used
in the last hour is kept) and pipefail + set -e ended the script before
`echo "$f"`. It needs 4+ files of that name in the cache, which the shared
cache always has; versions.sh then ran `cp "" ...` and `sb restart` failed.

Checks, with SB_BUILD_DIR in a temp dir (never the shared cache):
- `path sb-core` of a tiny hub tree (bend/hub/main.bend prints a word:
  a miss compiles it in 0.3 s, never a real sb-core: the EDR of a company
  Mac deletes fresh unsigned copies, and each one is a new detection) next
  to 4 recent files of that name: rc 0, stdout = one line, an executable
  in the cache; the recent files are kept; a second call (a hit) prints
  the same path;
- `key` of a source tree without some of the recipe's dirs (an old commit:
  no core/, no rust/home): rc 0 and a 12-hex key;
- `key` on a temp HOME with PATH=/usr/bin:/bin (a Rust test's env) is the
  shell's key (bins-key: bend was looked up in $HOME).

python3 -u tests/bins_path.py
"""
import os, re, shutil, subprocess, sys, tempfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
BINS = os.path.join(ROOT, "scripts", "bins.sh")


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what, flush=True)
    if not ok:
        sys.exit("FAIL " + what)


def env(build):
    e = {k: v for k, v in os.environ.items()
         if not k.startswith("SB_") and k not in ("BEND_SESSION_FILE", "BEND_CONTEXT_FILE",
                                                  "BEND_WIRE_LOG", "BEND_REPL_PORT")}
    e["SB_BUILD_DIR"] = build
    return e


def run(e, *args):
    return subprocess.run([BINS, *args], env=e, capture_output=True, text=True, timeout=600)


def main():
    # $TMPDIR when short (a sandboxed gate cannot write /tmp), else /tmp
    t = tempfile.gettempdir()
    tmp = tempfile.mkdtemp(prefix="sbbp-", dir=t if len(t) <= 40 else "/tmp")
    try:
        build = os.path.join(tmp, "build")
        cache = os.path.join(build, "cache")
        os.makedirs(cache)
        e = env(build)
        # 4 files of the name, used just now: prune keeps them all
        olds = []
        for k in ("aaaaaaaaaaa1", "aaaaaaaaaaa2", "aaaaaaaaaaa3", "aaaaaaaaaaa4"):
            f = os.path.join(cache, "sb-core-" + k)
            with open(f, "w") as fh:
                fh.write("#!/bin/sh\n")
            os.chmod(f, 0o755)
            olds.append(f)

        # the tiny hub tree (bend/runtime/: bins.sh's layout since the root cleanup)
        hub = os.path.join(tmp, "hub")
        os.makedirs(os.path.join(hub, "bend", "hub"))
        os.makedirs(os.path.join(hub, "bend", "runtime"))
        with open(os.path.join(hub, "bend", "hub", "main.bend"), "w") as fh:
            fh.write('import Base\n\ndef main() -> IO(Unit):\n  IO.print("tiny")\n')

        r = run(e, "path", "--src", hub, "sb-core")
        print(r.stderr, end="", flush=True)
        lines = r.stdout.splitlines()
        check(r.returncode == 0, f"path on a miss exits 0 (rc {r.returncode})")
        check("compiling" in r.stderr, "it was a miss (compiled)")
        check(len(lines) == 1, f"stdout is one line: {r.stdout!r}")
        p = lines[0]
        check(os.path.dirname(p) == cache and os.access(p, os.X_OK),
              f"the path is an executable in the temp cache: {p}")
        check(all(os.path.exists(f) for f in olds), "the recent cache files are kept")

        r2 = run(e, "path", "--src", hub, "sb-core")
        check(r2.returncode == 0 and r2.stdout.strip() == p and "compiling" not in r2.stderr,
              "a second call is a hit with the same path")

        # an old source tree: runtime/ and vendor/ only, rust/jsrt only
        src = os.path.join(tmp, "src")
        os.makedirs(os.path.join(src, "runtime"))
        os.makedirs(os.path.join(src, "vendor"))
        os.makedirs(os.path.join(src, "rust", "jsrt"))
        with open(os.path.join(src, "runtime", "repl-live.bend"), "w") as fh:
            fh.write("# x\n")
        with open(os.path.join(src, "rust", "jsrt", "Cargo.toml"), "w") as fh:
            fh.write("[package]\n")
        for name in ("repl-live", "bend-jsrt"):
            r = run(e, "key", "--src", src, name)
            check(r.returncode == 0 and re.fullmatch(r"[0-9a-f]{12}\n", r.stdout) is not None,
                  f"key {name} of a tree without some dirs: rc {r.returncode} {r.stdout!r} {r.stderr[-200:]!r}")
        # and the path is the key of the tree it built
        k = run(e, "key", "--src", hub, "sb-core").stdout.strip()
        check(p.endswith("sb-core-" + k), "path is <name>-<key>")
        # the key of the same sources on another HOME and a bare PATH (a
        # Rust test's temp HOME): the same key (bins-key: it took "bend ?")
        fake = os.path.join(tmp, "home")
        os.makedirs(fake)
        e2 = dict(e, HOME=fake, PATH="/usr/bin:/bin")
        k2 = run(e2, "key", "--src", hub, "sb-core")
        check(k2.returncode == 0 and k2.stdout.strip() == k,
              f"the key on a temp HOME is the shell's: {k2.stdout.strip()!r} vs {k!r} {k2.stderr[-200:]!r}")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    print("bins_path: all ok", flush=True)


if __name__ == "__main__":
    main()
