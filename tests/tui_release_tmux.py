"""/release-bise in a real terminal (tmux), BISE-235: only in bise's dev
build. A hub whose workspace is not bise's source tree refuses it; in a
dev workspace (a fake one: scripts/versions.sh + rust/switchboard) the
popup offers it, the preview shows the tag, the commit and what is new
since the last tag, `y` runs it, the steps and the result land in main's
feed and the header says it runs. The script is a fake one
(BISE_RELEASE_SCRIPT): nothing is tagged, pushed or published.

python3 -u tests/tui_release_tmux.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, wait_until, MAIN_IDLE  # noqa: E402

FAKE = r"""#!/bin/sh
echo "$*" >> "$(dirname "$0")/release-args"
case "$*" in *--next-tag*) echo v2026.10.3; exit 0 ;; esac
echo "publish-release: dry run: would tag ${3%%${3#???????}} $1, git push origin $1" >&2
sleep 3
echo "publish-release: watching run 4312: gh run view 4312" >&2
sleep 3
echo "publish-release: the draft:" >&2
"""


def refused_elsewhere():
    """Not bise's source tree: the hub refuses, whoever asks."""
    E = e2e.Env()
    try:
        c = E.start_hub()
        c.send({"op": "release", "do": "plan", "dry": True})
        wait_until(lambda: any("runs only in bise's dev build" in n.get("text", "") for n in c.notices()), 10,
                   lambda: "no refusal: %r" % c.notices())
    finally:
        E.close()


def main():
    refused_elsewhere()
    E = e2e.Env()
    ws = E.ws
    e2e.sh(ws, "mkdir -p scripts rust/switchboard && touch scripts/versions.sh rust/switchboard/Cargo.toml"
               " && git add -A && git commit -qm 'the dev tree' && git -c tag.gpgSign=false tag v2026.10.1"
               " && git commit -q --allow-empty -m 'more room around the text'"
               " && git commit -q --allow-empty -m 'fix the voice chip'")
    fake = os.path.join(E.tmp, "fake-release.sh")
    open(fake, "w").write(FAKE)
    os.chmod(fake, 0o755)
    E.env["SB_RELEASE_TEST"] = "1"
    with tui_session(160, 42, "BISE_RELEASE_SCRIPT=%s" % fake, E=E) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed("/release-b")
        t.wait("tag HEAD, CI builds it")
        t.typed("ise dry-run")
        t.wait("/release-bise dry-run")   # the whole line in the composer
        t.keys("Enter")
        sc = t.wait("dry run of v2026.10.3: nothing is pushed or published. go?")
        assert "release v2026.10.3 · " in sc and "fix the voice chip · 2 commits since v2026.10.1 · dry run" in sc, sc
        assert "more room around the text" in sc, sc
        t.typed("y")
        t.keys("Enter")
        sc = t.wait("releasing v2026.10.3 · ")          # the header's item
        t.wait("would tag")
        t.wait("CI building · 0s")
        t.wait("✓ CI run 4312 started")
        sc = t.wait("dry run of v2026.10.3 · nothing pushed, nothing published", 30)
        assert "CI building" not in sc, sc               # the running row gave its place
        t.wait_gone("releasing v2026.10.3 · ")
        args = open(os.path.join(E.tmp, "release-args")).read()
        assert "--next-tag" in args and "v2026.10.3 --rev " in args and "--publish --dry-run" in args, args
        print("OK: /release-bise refused elsewhere; dev: popup, preview, y, steps, header item, result")


if __name__ == "__main__":
    run(main)
