#!/usr/bin/env python3
"""scripts/desktop_dmg.py (desktop-dmg.sh), its pure parts and one DMG of a fake bundle, on fake data.

The runtime's id (desktop-<sha12>, --rev for a second build of a commit: architect m_23936, one id =
one build), its share VERSION (no repo=, built= from the commit time, target, channel=share), the
repack as 'bise' / dev.bise.desktop from the candidate's own bundle (never a build), and the DMG
script run on a stand-in bundle under a temp dir: bise.app and an Applications link inside, a
.sha256 next to it; mounted read-only, never browsed, never opened. No app is launched.

  python3 -B tests/desktop_dmg_test.py
"""
import os
import plistlib
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), "scripts"))
import desktop_dmg as D  # noqa: E402
from desktop_try_env import TryError  # noqa: E402

SHA = "0123456789abcdef0123456789abcdef01234567"


def check(ok, what):
    if not ok:
        raise AssertionError(what)


def test_ids():
    check(D.share_id(SHA) == "desktop-0123456789ab", D.share_id(SHA))
    check(D.share_id(SHA, "2") == "desktop-0123456789ab-2", "a second build of a commit takes a new id")
    for bad in ("1", "x", ""):
        try:
            D.share_id(SHA, bad)
            check(False, "--rev %r accepted" % bad)
        except TryError:
            pass
    s = D.share_paths("/c/0123456789ab", SHA)
    check(s["root"] == "/c/0123456789ab/share/desktop-0123456789ab", s["root"])
    check(s["dmg"] == "/c/0123456789ab/share/bise-desktop-01234567-arm64.dmg", s["dmg"])
    check(s["app"].endswith("/share/dist/mac-arm64/bise.app"), s["app"])
    check(D.share_paths("/c/x", SHA, "3")["dmg"].endswith("-01234567-3-arm64.dmg"), "the rev names the DMG")


def test_version():
    cand = "id=root-0123456789ab\ncommit=%s\nrepo=/Users/g/src/bise\n" % SHA
    v = D.share_version(cand, "desktop-0123456789ab", SHA, "feat: x\ny", D.utc_z(1760450000))
    lines = v.splitlines()
    check(lines[0] == "id=desktop-0123456789ab", v)
    check(not any(l.startswith("repo=") for l in lines), "no dev path: " + v)
    check(sum(l.startswith("id=") for l in lines) == 1 and sum(l.startswith("commit=") for l in lines) == 1, v)
    check("built=2025-10-14T13:53:20Z" in lines and "target=darwin-arm64" in lines and "channel=share" in lines, v)
    check("subject=feat: x y" in lines, v)
    # the same commit gives the same text: a second DMG of it makes the same root
    check(v == D.share_version(cand, "desktop-0123456789ab", SHA, "feat: x\ny", D.utc_z(1760450000)), "deterministic")


def test_repack():
    c = {"src": "/c/src", "app": "/c/dist/mac-arm64/bise cand.app"}
    s = D.share_paths("/c", SHA)
    sc = D.repack_script(c, s, "/b/sign")
    check("BISE_PACK_NAME=bise BISE_PACK_APP_ID=dev.bise.desktop " in sc, sc)
    check("BISE_PACK_ROOT=/c/share/desktop-0123456789ab" in sc and "BISE_PACK_SIGN_DIR=/b/sign" in sc, sc)
    check("'/c/dist/mac-arm64/bise cand.app/Contents/Resources/bin/bise'" in sc, "the candidate's own bise: " + sc)
    # fast-try p1: a candidate's worktree has no node_modules (it built in the work checkout): the
    # repack installs it from the candidate's lockfile when missing, and builds nothing
    check("pnpm build" not in sc and "cargo" not in sc and sc.count("pnpm install") == 1
          and "if [ ! -d node_modules ]; then pnpm install --frozen-lockfile; fi" in sc, "a repack, never a build: " + sc)
    check("BISE_PACK_VERSION" not in sc, "a dev build: never offered an app update")
    # electron-builder.mjs reads tray-badge.node next to BISE_PACK_HELPER: the candidate's helper
    # (MacOS/) and badge (Resources/) are copied into one dir, the helper given from there
    check("'/c/dist/mac-arm64/bise cand.app/Contents/Resources/tray-badge.node'" in sc
          and "BISE_PACK_HELPER=/c/share/pack-helper/bise-mac-helper " in sc, "the helper and its badge side by side: " + sc)


def test_dmg_of_a_stand_in():
    t = tempfile.mkdtemp(prefix="bise-dmg-test-")
    try:
        s = D.share_paths(t, SHA)
        info = os.path.join(s["app"], "Contents")
        os.makedirs(os.path.join(info, "Resources", "bin"))
        with open(os.path.join(info, "Info.plist"), "wb") as f:
            plistlib.dump({"CFBundleIdentifier": D.APP_ID, "CFBundleExecutable": "bise", "CFBundlePackageType": "APPL"}, f)
        with open(os.path.join(info, "Resources", "bin", "VERSION"), "w") as f:
            f.write("id=%s\ncommit=%s\n" % (s["id"], SHA))
        os.makedirs(os.path.join(info, "MacOS"))
        shutil.copy("/usr/bin/true", os.path.join(info, "MacOS", "bise"))
        r = subprocess.run(["codesign", "--force", "--deep", "--sign", "-", s["app"]], capture_output=True, text=True)
        check(r.returncode == 0, r.stderr)
        check(D.check_app(s["app"], s["id"]) == [], D.check_app(s["app"], s["id"]))
        check(D.check_app(s["app"], "desktop-other") != [], "another id is caught")
        r = subprocess.run(["/bin/bash", "-c", D.dmg_script(s)], capture_output=True, text=True)
        check(r.returncode == 0, r.stderr)
        check(os.path.exists(s["dmg"]) and not os.path.exists(s["stage"]), "the DMG, the stage gone")
        sha = open(s["sha256"]).read().split()
        check(sha[1] == os.path.basename(s["dmg"]) and len(sha[0]) == 64, sha)
        check(D.check_dmg(s) == [], D.check_dmg(s))
        check(not os.path.ismount(s["mnt"]), "detached after the check")
    finally:
        subprocess.run(["hdiutil", "detach", "-quiet", "-force", os.path.join(t, "share", "mnt")], capture_output=True)
        shutil.rmtree(t, ignore_errors=True)


def test_heavy_argv():
    # the repack's slot (review-gates m_29770): heavy-run --low outside a slot, nice 10 inside one
    # (the nightly holds both: never a wait on its own ancestor), the old lock without heavy-run
    t = tempfile.mkdtemp(dir=os.environ.get("TMPDIR"))
    try:
        hr = os.path.join(t, "heavy-run")
        with open(hr, "w") as f:
            f.write("#!/bin/sh\n")
        os.chmod(hr, 0o755)
        out = D.heavy_dmg_argv("x", under=False, heavy_run=hr, lock="/l")
        check(out == [hr, "--low", "/bin/bash", "-c", "x"], "outside a slot: %s" % out)
        out = D.heavy_dmg_argv("x", under=True, heavy_run=hr, lock="/l")
        check(out == ["nice", "-n", "10", "/bin/bash", "-c", "x"], "inside a slot: %s" % out)
        out = D.heavy_dmg_argv("x", under=False, heavy_run=os.path.join(t, "none"), lock="/l")
        check(out == ["/usr/bin/lockf", "-k", "/l", "nice", "-n", "10", "/bin/bash", "-c", "x"], "no heavy-run: %s" % out)
        check(D.LOCK.endswith("/.bise/run/heavy.lock"), "a real lock: %s" % D.LOCK)
    finally:
        shutil.rmtree(t, ignore_errors=True)


def test_repack_failure():
    # desktop-tag-37dd2ac2: a bare 'failed' while its pnpm install ran on; a signal is named, from outside
    app = "/c/dist/mac-arm64/bise cand.app"
    for rc in (-9, 137):
        w = D.repack_failure(rc, app)
        check("killed by SIGKILL" in w and app in w, "a kill: %s" % w)
    check("SIGTERM" in D.repack_failure(-15, app), "a TERM")
    check("disk" in D.repack_failure(28, app) and "time limit" in D.repack_failure(124, app), "heavy-run's stops")
    w = D.repack_failure(1, app)
    check(w.startswith("exit 1:") and "killed" not in w, "a plain failure: %s" % w)


if __name__ == "__main__":
    for name, fn in sorted((n, f) for n, f in globals().items() if n.startswith("test_")):
        fn()
        print("ok", name)
