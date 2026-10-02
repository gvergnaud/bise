"""/update in bise's dev build, in a real terminal (tmux), dev-update: in
bise's source tree (a fake one: scripts/versions.sh + rust/switchboard)
/update builds the workspace's HEAD with scripts/versions.sh (a fake build
here: a few seconds, then a compile error), the header says
`building <sha> · 0s` while it runs, a second /update says it is already
building, and the failed build leaves `▲ couldn't build <sha>, you're
still on …: <last line>` in main's feed with the build's tail folded
under it; nothing switches. Not a real 5-minute build.

python3 -u tests/tui_update_tmux.py
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, wait_until, MAIN_IDLE  # noqa: E402

FAKE_BUILD = r"""#!/bin/sh
echo "$*" >> "$(dirname "$0")/../build-args"
sleep 4
i=1
while [ $i -le 24 ]; do echo "   Compiling crate$i v0.1.0" >&2; i=$((i+1)); done
echo "error: could not compile \`bise\` (bin \"bise\") due to 1 previous error" >&2
exit 1
"""


def main():
    E = e2e.Env()
    ws = E.ws
    e2e.sh(ws, "mkdir -p scripts rust/switchboard && touch rust/switchboard/Cargo.toml")
    script = os.path.join(ws, "scripts", "versions.sh")
    open(script, "w").write(FAKE_BUILD)
    os.chmod(script, 0o755)
    e2e.sh(ws, "git add -A && git commit -qm 'the dev tree'")
    head = os.popen("git -C %s rev-parse --short HEAD" % ws).read().strip()
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(bise)
    with tui_session(160, 42, "BISE_HOME=%s" % bise, E=E) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed("/update")
        time.sleep(0.3)
        t.keys("Enter")
        t.wait("building the latest commit, %s, then restarting on it. your agents keep running." % head)
        t.wait("building %s · " % head)              # the header's item
        t.typed("/update")
        time.sleep(0.3)
        t.keys("Enter")
        t.wait("already building %s · " % head)
        sc = t.wait("couldn't build %s, you're still on " % head, 30)
        assert "▲ couldn't build" in sc, sc
        assert "could not compile `bise`" in sc, sc
        t.wait("▸ 20 more lines")
        # the header's item is gone (the answer `already building …` stays in the feed)
        header = lambda: [r for r in t.screen().split("\n") if "bise :*" in r or "bise " in r[:12]][:1]
        wait_until(lambda: header() and ("building %s · " % head) not in header()[0], 10,
                   lambda: "header still building: %r" % header())
        args = open(os.path.join(ws, "build-args")).read().split("\n")
        assert [a for a in args if a] == ["build %s" % head], args   # built once, HEAD, no pull
        status = os.popen("git -C %s status --porcelain --untracked-files=no" % ws).read()
        assert status == "", status                                  # the working tree untouched
        print("OK: /update in the dev build: builds HEAD, header item, already building, failure folded, nothing switched")


if __name__ == "__main__":
    run(main)
