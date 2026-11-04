#!/usr/bin/env python3
"""desktop-release: build bise desktop for a release, signed, notarized, stapled, as a DMG and a zip.

  scripts/desktop-release.sh runtime --out <dir> [--tag <v...>] [--commit <sha>] [--dry-run]
      the runtime the app carries: a published terminal release's darwin-arm64 archive (its
      sha256 checked), unpacked; prints its app root. Default: the terminal tag on the commit.
      --dry-run: a seam skew (below) is a warning, as a dry run ships nothing.
  scripts/desktop-release.sh build --out <dir> --runtime-root <root> [--tag desktop-vYYYY.M.N]
      [--identity <sha1|->] [--keychain <kc>] [--notarize] [--built <UTC Z>] [--repo owner/name]
      [--helper <a built helper>]
      the native helper (built, or --helper: CI's cache of it), the app (pnpm build,
      electron-builder --dir, unsigned), signed inside out (desktop_sign.py), the DMG (the
      designer's window: apps/desktop/resources/dmg/), with --notarize one notarization, the
      DMG's (Apple notarizes the app inside it too), stapled on the DMG and on the app, then
      the zip (the updater's) of the stapled app, the gates, and
      <out>/release/: bise-mac-arm64.{dmg,zip} + .sha256, VERSION, desktop.json (with --tag).
  scripts/desktop-release.sh dmg <bise.app> <out.dmg> [--identity <sha1|->]
      the DMG alone, from a built app (a look check).

No --tag: a dry run (id desktop-dry-<sha12>, no feed): what CI's dry_run and a local check make.
Notarizing reads APPLE_API_KEY_PATH (the .p8 file's path), APPLE_API_KEY_ID and APPLE_API_ISSUER.

The runtime: a desktop release carries a published terminal release (desktop-dist, his go), so
the app's hubs run a bise every terminal also runs. Its commit must be in the app's history and
the seams between the two (rust/proto, rust/home, the core rust/tui/src/ambient) unchanged since:
an app that needs a newer core is refused, never shipped with an older one.

Never here: a publish, a tag, a push; secrets in argv (the key is a file path; the certificate
lives in the workflow's throwaway keychain). Pure parts: tests/desktop_release_test.py.
"""
import argparse
import datetime
import glob
import hashlib
import importlib.util
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tarfile

HERE = os.path.dirname(os.path.realpath(__file__))
REPO = os.path.dirname(HERE)
sys.path.insert(0, HERE)
import desktop_feed as F  # noqa: E402
import desktop_sign as S  # noqa: E402
import mac_sign as M  # noqa: E402
import notary as N  # noqa: E402

DESKTOP = os.path.join(REPO, "apps", "desktop")
DMG_DIR = os.path.join(DESKTOP, "resources", "dmg")
NAME = "bise"
APP_ID = "dev.bise.desktop"
# the native helper's floor (apps/desktop/native/mac/build.sh: ScreenCaptureKit)
MACOS_TARGET = "14.0"
DMGBUILD = "dmgbuild==1.6.7"
# what a desktop app needs from its runtime's core: unchanged between the two commits
SEAMS = ("rust/proto", "rust/home", "rust/tui/src/ambient")


class ReleaseError(Exception):
    pass


def say(*a):
    print("desktop-release:", *a, file=sys.stderr, flush=True)


def run(argv, cwd=None, env=None, quiet=False):
    say("$ " + " ".join(shlex.quote(x) for x in argv))
    r = subprocess.run(argv, cwd=cwd, env=env, capture_output=quiet, text=True)
    if r.returncode != 0:
        raise ReleaseError("%s failed (rc %d)%s" % (argv[0], r.returncode, (": " + (r.stderr or "").strip()[-800:]) if quiet else ""))
    return r


def out(argv, cwd=None):
    return subprocess.run(argv, cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def write_sha(path):
    s = sha256(path)
    with open(path + ".sha256", "w") as f:
        f.write("%s  %s\n" % (s, os.path.basename(path)))
    return s


def now_z():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


# ---- the runtime -------------------------------------------------------------------------------

def runtime_tag(points_at):
    """The terminal release on the commit (`git tag --points-at`'s lines): exactly one v* tag. Pure."""
    tags = sorted(t for t in points_at.split() if t.startswith("v") and not t.startswith("desktop-"))
    if len(tags) != 1:
        raise ReleaseError("the commit carries %s terminal release tags (%s): cut a terminal release on it first, or give --tag"
                           % (len(tags) or "no", ", ".join(tags) or "none"))
    return tags[0]


def archive_name(names):
    """The darwin-arm64 terminal archive among a release's asset names. Pure."""
    m = [n for n in names if n.startswith("bise-") and n.endswith("-darwin-arm64.tar.gz")]
    if len(m) != 1:
        raise ReleaseError("the terminal release has %d darwin-arm64 archives: %s" % (len(m), m))
    return m[0]


def skew(changed):
    """The seam files changed between the runtime's commit and the app's (`git diff --name-only`). Pure."""
    return [p for p in changed.splitlines() if p.strip() and any(p == s or p.startswith(s + "/") for s in SEAMS)]


def runtime(out_dir, tag=None, commit="HEAD", repo=None, dry=False):
    commit = out(["git", "rev-parse", commit + "^{commit}"], cwd=REPO)
    tag = tag or runtime_tag(out(["git", "tag", "--points-at", commit], cwd=REPO))
    d = os.path.join(out_dir, "runtime", tag)
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    gh = ["gh", "release"] + (["-R", repo] if repo else [])
    names = json.loads(out(gh + ["view", tag, "--json", "assets"]))["assets"]
    arch = archive_name([a["name"] for a in names])
    run(gh + ["download", tag, "-D", d, "-p", arch, "-p", arch + ".sha256"])
    want = open(os.path.join(d, arch + ".sha256")).read().split()[0]
    if sha256(os.path.join(d, arch)) != want:
        raise ReleaseError("%s's sha256 is not its .sha256's" % arch)
    with tarfile.open(os.path.join(d, arch)) as t:
        t.extractall(d, filter="tar") if hasattr(tarfile, "data_filter") else t.extractall(d)
    roots = glob.glob(os.path.join(d, "bise-*", "app"))
    if len(roots) != 1 or not os.path.isfile(os.path.join(roots[0], "VERSION")):
        raise ReleaseError("%s holds no app/VERSION" % arch)
    root = roots[0]
    v = F.read_version(open(os.path.join(root, "VERSION")).read())
    rc = v.get("commit", "")
    if subprocess.run(["git", "merge-base", "--is-ancestor", rc, commit], cwd=REPO).returncode != 0:
        raise ReleaseError("the runtime %s (commit %s) is not in %s's history" % (tag, rc[:12], commit[:12]))
    moved = skew(out(["git", "diff", "--name-only", rc, commit, "--", *SEAMS], cwd=REPO))
    if moved and dry:
        say("dry run: the seams changed since %s (%s); a release would refuse this runtime" % (tag, ", ".join(moved[:5])))
    elif moved:
        raise ReleaseError("the app's seams with its core changed since %s (%s): cut a terminal release on %s first"
                           % (tag, ", ".join(moved[:5]), commit[:12]))
    say("runtime %s (id %s, commit %s)" % (tag, v.get("id"), rc[:12]))
    return root


# ---- the build ---------------------------------------------------------------------------------

def paths(out_dir):
    o = os.path.abspath(out_dir)
    rel = os.path.join(o, "release")
    return {"out": o, "helper": os.path.join(o, "helper", "bise-mac-helper"), "version": os.path.join(o, "VERSION"),
            "dist": os.path.join(o, "dist"), "app": os.path.join(o, "dist", "mac-arm64", NAME + ".app"),
            "release": rel, "zip": os.path.join(rel, F.ZIP), "dmg": os.path.join(rel, F.DMG),
            "feed": os.path.join(rel, F.FEED), "notary": os.path.join(o, "notary")}


def helper_argv(out_path):
    """The native helper's build (apps/desktop/native/mac/build.sh). Pure."""
    return [os.path.join(DESKTOP, "native", "mac", "build.sh"), out_path]


def pack_env(base, p, runtime_root):
    """electron-builder's inputs (electron-builder.mjs): the shipping bundle 'bise', the runtime's
    bise, the helper, the runtime, the app's VERSION; unsigned here (BISE_PACK_IDENTITY none: its
    deep ad hoc signing cost 16 s and desktop_sign signs every piece after anyway), notarized
    after by notarize: no CSC_* (its signing) and no APPLE_* (its notarization, which
    electron-builder 26 starts on its own when it sees them; its config also says notarize: false)
    reach it. Pure."""
    env = {k: v for k, v in base.items() if not k.startswith(("BISE_PACK_", "CSC_", "APPLE_"))}
    env.update({"BISE_PACK_NAME": NAME, "BISE_PACK_APP_ID": APP_ID, "BISE_PACK_BISE": os.path.realpath(os.path.join(runtime_root, "bise")),
                "BISE_PACK_HELPER": p["helper"], "BISE_PACK_ROOT": runtime_root, "BISE_PACK_VERSION": p["version"],
                "BISE_PACK_IDENTITY": "none", "BISE_PACK_OUT": p["dist"], "CSC_IDENTITY_AUTO_DISCOVERY": "false"})
    return env


def notarize(p):
    """One notarization for both: the signed DMG, which holds the signed app, is submitted (Apple
    notarizes the code nested in it, the app's cdhashes too); the DMG is stapled, then the app
    in dist from the same ticket (the updater's zip is made from it after). Apple refusing the
    app's ticket from the DMG's submission: the app goes on its own (the old, second wait).
    Submit, wait, read the verdict (its log on a refusal), staple, validate: scripts/notary.py."""
    try:
        N.submit(p["dmg"], p["notary"])
        N.staple(p["dmg"])
        try:
            N.staple(p["app"])
        except N.NotaryError as e:
            say("no ticket for the app from the DMG's submission (%s): notarizing the app alone" % str(e)[-160:])
            N.submit(p["app"], p["notary"])
            N.staple(p["app"])
    except N.NotaryError as e:
        raise ReleaseError(str(e))


def cli_bundle(app):
    """The runtime's bise as the terminal release ships it (mac_sign.bundle, the one builder):
    Contents/Resources/bin/bise.app (dev.bise.cli) around the packed bin/bise, and bin/bise a link
    into it. The core places bin/ as <prefix>/versions/<id> and runs hubs from there: one bundle id
    and one Developer ID, so his folder grants follow the hub across the terminal and the app."""
    bindir = os.path.join(app, "Contents", "Resources", "bin")
    exe = os.path.join(bindir, "bise")
    if os.path.islink(exe):
        raise ReleaseError("%s is a link: the pack must copy the runtime's real bise" % exe)
    v = F.read_version(open(os.path.join(bindir, "VERSION")).read())
    try:
        M.bundle(os.path.join(bindir, "bise.app"), exe, M.CLI_ID, v.get("id") or "0", link=True)
    except M.SignError as e:
        raise ReleaseError(str(e))


def layout(dmg_dir=DMG_DIR):
    with open(os.path.join(dmg_dir, "layout.json")) as f:
        return json.load(f)


def dmg_settings(app, lay, dmg_dir=DMG_DIR):
    """dmgbuild's settings for the designer's window (layout.json): the app and an Applications
    link at their places, the background (its @2x next to it makes one HiDPI tiff), the volume's
    icon the app's own, no toolbar or sidebar, compressed (ULMO, macOS 10.15+). Pure."""
    w = lay["window"]
    return {
        "volume_name": lay.get("volume_name", NAME), "format": lay.get("format", "ULMO"), "filesystem": "HFS+",
        "files": [app], "symlinks": {"Applications": "/Applications"},
        "icon": os.path.join(app, "Contents", "Resources", "icon.icns"),
        "background": os.path.join(dmg_dir, lay["background"]),
        "window_rect": ((w["x"], w["y"]), (w["width"], w["height"])),
        "icon_size": float(lay.get("icon_size", 128)), "text_size": float(lay.get("text_size", 13)),
        "icon_locations": {NAME + ".app": tuple(lay["icons"]["app"]), "Applications": tuple(lay["icons"]["applications"])},
        "show_status_bar": False, "show_tab_view": False, "show_toolbar": False, "show_pathbar": False,
        "show_sidebar": False, "default_view": "icon-view", "show_icon_preview": False, "hide_extensions": [NAME + ".app"],
    }


def dmgbuild_argv(settings_file, volume, dmg):
    inner = ["-m", "dmgbuild", "-s", settings_file, volume, dmg]
    if importlib.util.find_spec("dmgbuild"):
        return [sys.executable] + inner
    return ["uv", "run", "--no-project", "--python", ">=3.10", "--with", DMGBUILD, "python"] + inner


def make_dmg(app, dmg, identity="-", keychain=None):
    lay = layout()
    s = dmg_settings(os.path.abspath(app), lay)
    os.makedirs(os.path.dirname(os.path.abspath(dmg)), exist_ok=True)
    sf = os.path.abspath(dmg) + ".settings.py"
    with open(sf, "w") as f:
        f.write("".join("%s = %r\n" % kv for kv in s.items()))
    if os.path.exists(dmg):
        os.remove(dmg)
    try:
        run(dmgbuild_argv(sf, s["volume_name"], dmg))
    finally:
        os.remove(sf)
    argv = ["codesign", "--force", "--sign", identity, "--timestamp=none" if identity == "-" else "--timestamp"]
    run(argv + (["--keychain", keychain] if keychain else []) + [dmg])
    return dmg


def hardened(codesign_dv):
    """Whether `codesign -dv`'s output says the hardened runtime (flags=0x10000(runtime), or
    0x10002(adhoc,runtime)). Pure."""
    return re.search(r"flags=0x[0-9a-f]+\([^)]*\bruntime\b", codesign_dv) is not None


# Electron's fuse wire (@electron/fuses): this sentinel in Electron Framework, then the wire's
# version (1), its length, and one byte per fuse in this order: '0' off, '1' on, 'r' removed.
FUSE_SENTINEL = b"dL7pKGdnNz796PbbjQWNKmHXBZaB9tsX"
FUSE_NAMES = ("RunAsNode", "EnableCookieEncryption", "EnableNodeOptionsEnvironmentVariable",
              "EnableNodeCliInspectArguments", "EnableEmbeddedAsarIntegrityValidation", "OnlyLoadAppFromAsar",
              "LoadBrowserProcessSpecificV8Snapshot", "GrantFileProtocolExtraPrivileges")
# what electron-builder.mjs's FUSES flips (apple-audit item 2; tests/desktop_release_test.py keeps
# the two the same): the gates refuse an app whose wire says otherwise
LOCKED = {"RunAsNode": False, "EnableNodeOptionsEnvironmentVariable": False, "EnableNodeCliInspectArguments": False,
          "EnableEmbeddedAsarIntegrityValidation": True, "OnlyLoadAppFromAsar": True}


def framework_binary(app):
    return os.path.join(app, "Contents", "Frameworks", "Electron Framework.framework", "Electron Framework")


def fuse_wires(data):
    """Every fuse wire in an Electron Framework's bytes (two in a universal binary), each a dict
    name → True/False/None (None: removed, or a byte this reader doesn't know). Pure."""
    wires = []
    i = data.find(FUSE_SENTINEL)
    while i != -1:
        at = i + len(FUSE_SENTINEL)
        if data[at] != 1:
            raise ReleaseError("fuse wire version %d, this reader knows 1" % data[at])
        raw = data[at + 2:at + 2 + data[at + 1]]
        wires.append({n: {ord("0"): False, ord("1"): True}.get(b) for n, b in zip(FUSE_NAMES, raw)})
        i = data.find(FUSE_SENTINEL, at)
    return wires


def fuses_wrong(wires, want=None):
    """What differs from the locked fuses (LOCKED), one line each; no wire is wrong too. Pure."""
    want = LOCKED if want is None else want
    if not wires:
        return ["no fuse wire in Electron Framework"]
    return ["fuse %s is %s, want %s" % (n, w.get(n), on) for w in wires for n, on in sorted(want.items()) if w.get(n) is not on]


def check(p, signed):
    """The gates: the app's signature deep and strict, hardened, with its entitlements; mounted, the
    DMG holds bise.app and the Applications link; signed with a real identity, Gatekeeper's own
    verdicts (spctl on the app and the DMG, syspolicy_check distribution). A list of what's wrong."""
    bad = []
    app = p["app"]
    r = subprocess.run(["codesign", "--verify", "--deep", "--strict", app], capture_output=True, text=True)
    if r.returncode:
        bad.append("codesign --verify: " + r.stderr.strip())
    d = subprocess.run(["codesign", "-dv", "--entitlements", ":-", app], capture_output=True, text=True)
    if not hardened(d.stderr):
        bad.append("bise.app has no hardened runtime")
    if "com.apple.security.cs.allow-jit" not in d.stdout:
        bad.append("bise.app has no JIT entitlement")
    with open(framework_binary(app), "rb") as f:
        bad += fuses_wrong(fuse_wires(f.read()))
    for b in ("bise", "sb-core", "repl-live", "bend-jsrt"):
        f = os.path.join(app, "Contents", "Resources", "bin", b)
        if os.path.exists(f) and not hardened(subprocess.run(["codesign", "-dv", f], capture_output=True, text=True).stderr):
            bad.append("%s is not hardened" % b)
    mnt = os.path.join(p["out"], "mnt")
    os.makedirs(mnt, exist_ok=True)
    r = subprocess.run(["hdiutil", "attach", "-nobrowse", "-noautoopen", "-readonly", "-mountpoint", mnt, p["dmg"]], capture_output=True, text=True)
    if r.returncode:
        bad.append("hdiutil attach: " + r.stderr.strip())
    else:
        try:
            if not os.path.isdir(os.path.join(mnt, NAME + ".app")):
                bad.append("the DMG holds no bise.app")
            if os.path.realpath(os.path.join(mnt, "Applications")) != "/Applications":
                bad.append("the DMG has no Applications link")
            if not os.path.exists(os.path.join(mnt, ".background.tiff")) and not glob.glob(os.path.join(mnt, ".background.*")):
                bad.append("the DMG has no background")
        finally:
            subprocess.run(["hdiutil", "detach", "-quiet", mnt])
    if signed:
        for argv in (["spctl", "--assess", "--type", "execute", "-vv", app],
                     ["spctl", "--assess", "--type", "open", "--context", "context:primary-signature", "-vv", p["dmg"]],
                     ["xcrun", "stapler", "validate", app], ["xcrun", "stapler", "validate", p["dmg"]]):
            r = subprocess.run(argv, capture_output=True, text=True)
            if r.returncode:
                bad.append("%s: %s" % (" ".join(argv[:3]), (r.stderr or r.stdout).strip()))
        if shutil.which("syspolicy_check"):
            r = subprocess.run(["syspolicy_check", "distribution", app], capture_output=True, text=True)
            if r.returncode:
                bad.append("syspolicy_check distribution: " + (r.stdout + r.stderr).strip()[-600:])
    return bad


def build(a):
    p = paths(a.out)
    commit = out(["git", "rev-parse", "HEAD"], cwd=REPO)
    built = F.utc_z(a.built or now_z())
    root = os.path.abspath(a.runtime_root)
    rv = F.read_version(open(os.path.join(root, "VERSION")).read())
    rid = F.app_id(a.tag) if a.tag else F.dry_id(commit)
    if a.notarize and a.identity == "-":
        raise ReleaseError("--notarize needs a Developer ID identity, not ad hoc")
    for k in ("helper", "release"):
        shutil.rmtree(os.path.dirname(p[k]) if k == "helper" else p[k], ignore_errors=True)
    shutil.rmtree(p["dist"], ignore_errors=True)
    os.makedirs(os.path.dirname(p["helper"]))
    os.makedirs(p["release"])
    with open(p["version"], "w") as f:
        f.write(F.version_text(rid, built, commit, rv.get("id", "")))
    if a.helper:
        # CI's cache of the helper, keyed on its sources (desktop-release.yml): signed again below
        shutil.copy2(a.helper, p["helper"])
        # and the tray's badge addon, built next to it by the same build.sh
        badge = os.path.join(os.path.dirname(a.helper), "tray-badge.node")
        if not os.path.isfile(badge):
            raise ReleaseError("no tray-badge.node next to --helper %s: native/mac/build.sh builds both" % a.helper)
        shutil.copy2(badge, os.path.join(os.path.dirname(p["helper"]), "tray-badge.node"))
        say("helper %s (built before from the same sources)" % a.helper)
    else:
        env = dict(os.environ, MACOSX_DEPLOYMENT_TARGET=MACOS_TARGET)
        env.pop("HELPER_SIGN_ID", None)
        run(helper_argv(p["helper"]), env=env)
    run(["pnpm", "install", "--frozen-lockfile"], cwd=DESKTOP)
    run(["pnpm", "build"], cwd=DESKTOP)
    run(["pnpm", "dist"], cwd=DESKTOP, env=pack_env(os.environ, p, root))
    if not os.path.isdir(p["app"]):
        raise ReleaseError("electron-builder made no %s" % p["app"])
    cli_bundle(p["app"])
    S.sign(p["app"], a.identity, a.keychain, say=say)
    make_dmg(p["app"], p["dmg"], a.identity, a.keychain)
    if a.notarize:
        notarize(p)
    run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", p["app"], p["zip"]])
    bad = check(p, signed=a.notarize)
    if bad:
        raise ReleaseError("the gates: " + "; ".join(bad))
    zs, ds = write_sha(p["zip"]), write_sha(p["dmg"])
    shutil.copy(p["version"], os.path.join(p["release"], "VERSION"))
    if a.tag:
        text = open(p["version"]).read()
        f = F.feed(a.repo, a.tag, "beta", text, zs, os.path.getsize(p["zip"]), ds, os.path.getsize(p["dmg"]), built)
        with open(p["feed"], "w") as fh:
            fh.write(F.dumps(f))
    say("built %s (%s, runtime %s, %s)" % (p["dmg"], rid, rv.get("id"), "notarized" if a.notarize else "not notarized"))
    print(p["release"])


def main(argv):
    ap = argparse.ArgumentParser(prog="desktop-release.sh")
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("runtime")
    r.add_argument("--out", required=True)
    r.add_argument("--tag")
    r.add_argument("--commit", default="HEAD")
    r.add_argument("--repo")
    r.add_argument("--dry-run", action="store_true", help="a seam skew is a warning (a dry run ships nothing)")
    b = sub.add_parser("build")
    b.add_argument("--out", required=True)
    b.add_argument("--runtime-root", required=True)
    b.add_argument("--tag")
    b.add_argument("--identity", default="-")
    b.add_argument("--keychain")
    b.add_argument("--helper", help="a native helper built before from the same sources (CI's cache)")
    b.add_argument("--notarize", action="store_true")
    b.add_argument("--built")
    b.add_argument("--repo", default="gvergnaud/bise")
    d = sub.add_parser("dmg")
    d.add_argument("app")
    d.add_argument("dmg")
    d.add_argument("--identity", default="-")
    d.add_argument("--keychain")
    a = ap.parse_args(argv)
    try:
        if a.cmd == "runtime":
            print(runtime(a.out, a.tag, a.commit, a.repo, a.dry_run))
        elif a.cmd == "build":
            build(a)
        else:
            print(make_dmg(a.app, a.dmg, a.identity, a.keychain))
        return 0
    except (ReleaseError, F.FeedError, S.SignError) as e:
        say(str(e))
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
