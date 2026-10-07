#!/usr/bin/env python3
"""A SKILL.md that never answers does not stop main's first model call
(release-1007: ~/.vibe/skills/grill-me/SKILL.md linked into ~/Documents,
macOS's privacy check made the startup scan's `sed` wait ~40 min, and
main never called the model: e2e and agents_md_e2e failed on 'a model
call of main').

A real hub (e2e.Env), the scripted provider, a HOME of the test's own:
  ~/.vibe/skills/stuck/SKILL.md  a link to a FIFO no one writes (its
                                 open() blocks, as on the privacy check)
  ~/.vibe/skills/slow/SKILL.md   a regular file every reader of which
                                 hangs (a `sed` and a `head` first on
                                 PATH that sleep on any path with /slow/:
                                 the old scan's `sed` waits there)
  ~/.agents/skills/alpha         a good skill
1. main's first model call comes within 30 s of the hub's start;
2. its REPL's stderr (repl.err) names each skipped SKILL.md once;
3. the shared index holds alpha, neither stuck nor slow;
4. no reader is left behind (no `sleep 600` of the shim).
Never the user's ~/Documents: everything is under the test's tmp.
"""
import glob
import os
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import Env, check  # noqa: E402

SKILL = "---\nname: %s\ndescription: The %s skill.\n---\nBody.\n"

SHIM = """#!/bin/sh
case "$*" in *"/slow/"*) exec sleep 600 ;; esac
exec %s "$@"
"""


def main():
    E = Env()
    home = os.path.join(E.tmp, "home")
    for d in (".vibe/skills/stuck", ".vibe/skills/slow", ".agents/skills/alpha", "outside"):
        os.makedirs(os.path.join(home, d))
    os.mkfifo(os.path.join(home, "outside", "fifo"))
    os.symlink(os.path.join(home, "outside", "fifo"), os.path.join(home, ".vibe/skills/stuck/SKILL.md"))
    open(os.path.join(home, ".vibe/skills/slow/SKILL.md"), "w").write(SKILL % ("slow", "slow"))
    open(os.path.join(home, ".agents/skills/alpha/SKILL.md"), "w").write(SKILL % ("alpha", "alpha"))
    shim = os.path.join(E.tmp, "shim")
    os.makedirs(shim)
    for cmd in ("sed", "head"):
        real = shutil.which(cmd)
        p = os.path.join(shim, cmd)
        open(p, "w").write(SHIM % real)
        os.chmod(p, 0o755)
    E.env.update(HOME=home, PATH=shim + os.pathsep + E.env["PATH"])
    index = E.env["BEND_SKILLS_INDEX"]
    ok = False
    try:
        t0 = time.time()
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say("hello")
        c.wait(lambda: E.fake_requests(), 30, "main's first model call")
        took = time.time() - t0
        check(took < 30, "main's first model call %.1f s after the hub's start" % took)
        errs = glob.glob(os.path.join(E.state, "**", "repl.err"), recursive=True)
        err = "".join(open(f).read() for f in errs)
        for name in ("stuck", "slow"):
            path = os.path.join(home, ".vibe/skills/%s/SKILL.md" % name)
            check(err.count("skills scan: skipped %s (" % path) >= 1,
                  "repl.err names the skipped %s: %r" % (name, err[-600:]))
        idx = open(index).read() if os.path.exists(index) else ""
        names = [l.split("\t")[0] for l in idx.splitlines()]
        check(names == ["alpha"], "the shared index holds alpha alone: %r" % idx)
        c.wait(lambda: not subprocess.run(["pgrep", "-f", "%s/.vibe/skills/slow" % home],
                                          capture_output=True).stdout, 10, "no reader of slow left")
        c.wait_idle("main")
        ok = True
    finally:
        E.close()
    print("PASS skills_hang_e2e" if ok else "FAIL skills_hang_e2e")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
