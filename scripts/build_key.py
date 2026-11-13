#!/usr/bin/env python3
"""build_key: one app build per sha (release-speed plan A step 5, architect m_63479).

  scripts/build_key.py <sha>    this tree's key for an app build of <sha>, when one may stand for
                                another (reusable); nothing printed otherwise (dirty, a tool unknown)

desktop_try.py records a candidate's key (and its bise's cargo command) in cand.json;
nightly_desktop.sh's land step at a release and desktop_tag.py's tree_bise take the candidate's
build when their key matches. Its own module (stdlib only): nightly_desktop.sh calls it.
Tests: tests/desktop_try_test.py (build_key_cases).
"""
import os
import subprocess
import sys

# One app build per sha (release-speed plan A step 5, architect m_63479). The env the vite build
# reads (electron.vite.config.ts): its mode, in the key. The ones only electron-builder reads (the
# pack, after vite: bundle id, name, signing, out dir): not in it. desktop_try_test fails when either
# file reads a process.env name outside these two lists, so a sourcemap-like knob can't make two
# different builds share a key unseen.
BUILD_MODE_ENV = ("BISE_DESKTOP_SOURCEMAP",)
PACK_ONLY_ENV = ("BISE_PACK_BISE", "BISE_PACK_HELPER", "BISE_PACK_ROOT", "BISE_PACK_VERSION", "BISE_PACK_APP_ID",
                 "BISE_PACK_IDENTITY", "BISE_PACK_NAME", "BISE_PACK_OUT", "BISE_PACK_SIGN_DIR")
# the cargo command a candidate's packed bise comes from (build_script's), recorded in cand.json:
# scripts/tree_bise.sh takes a candidate's bise only when its own command is this one
BISE_CARGO = "cargo build -p bend-harness"


def build_key(repo, sha, env=None):
    """The key of an app build of `sha` in `repo`: the sha, the node and pnpm it builds with, the
    build's mode (BUILD_MODE_ENV's values). Two builds with the same key are the same app: a release
    pass builds once (the candidate) and its land step and its checks' bise take that build. A tree
    not clean at `sha` says 'dirty': such a key is never reused (reusable() is False). None when the
    sha isn't there."""
    env = dict(os.environ if env is None else env)
    r = subprocess.run(["git", "-C", repo, "rev-parse", "--verify", "-q", "%s^{commit}" % sha], env=env, capture_output=True, text=True)
    if r.returncode != 0:
        return None
    full = r.stdout.strip()
    head = subprocess.run(["git", "-C", repo, "rev-parse", "HEAD"], env=env, capture_output=True, text=True).stdout.strip()
    dirty = head != full or subprocess.run(["git", "-C", repo, "status", "--porcelain", "--untracked-files=no"], env=env,
                                           capture_output=True, text=True).stdout.strip() != ""
    vers = []
    for tool in (["node", "--version"], ["pnpm", "--version"]):
        try:
            vers.append(subprocess.run(tool, env=env, capture_output=True, text=True, timeout=30).stdout.strip().lstrip("v") or "?")
        except (OSError, subprocess.TimeoutExpired):
            vers.append("?")
    mode = ",".join("%s=%s" % (k, env.get(k, "")) for k in BUILD_MODE_ENV)
    return "%s node%s pnpm%s mode[%s]%s" % (full[:12], vers[0], vers[1], mode, " dirty" if dirty else "")


def reusable(key):
    """Whether a build with that key may stand for another of the same key. Pure."""
    return bool(key) and not key.endswith(" dirty") and "?" not in key



def main(argv):
    if len(argv) != 1:
        print(__doc__)
        return 2
    k = build_key(os.getcwd(), argv[0])
    if reusable(k):
        print(k)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
