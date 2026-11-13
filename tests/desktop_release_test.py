#!/usr/bin/env python3
"""The desktop app's release (scripts/desktop_release.py, desktop_sign.py, desktop_feed.py,
desktop_publish.py): its pure parts, and one inside-out ad hoc signing of a fake bundle, on fake data.

  - signing: what is code (thin, fat, Java class, text), the order (deepest first, the app
    last), the codesign argv (hardened, a timestamp only with a certificate, the entitlements on
    executables and apps, never a library), and a fake .app with a nested framework signed and
    verified deep and strict;
  - the feed: the shape agreed with dist-updater and dist-site (the reader's fields, built = the
    app's VERSION built=, absolute URLs on the immutable desktop-v* release, the DMG under one
    name), the tag scheme, and the channels' rules (never back, stable never ahead of beta);
  - publishing: every gh release create/edit says --latest=false, a build is a draft first,
    prerelease while only beta has it;
  - the runtime: one terminal tag on the commit, the darwin-arm64 archive, the seams;
  - the DMG settings: the designer's layout.json and background (with its @2x) are there.

No network, no gh, no keychain, no notarization.

  python3 -B tests/desktop_release_test.py
"""
import json
import os
import plistlib
import shutil
import struct
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(REPO, "scripts"))
import desktop_feed as F  # noqa: E402
import desktop_publish as P  # noqa: E402
import desktop_release as R  # noqa: E402
import desktop_sign as S  # noqa: E402
import mac_sign as M  # noqa: E402

SHA = "a" * 64
SHB = "b" * 64


def check(ok, what):
    if not ok:
        raise AssertionError(what)


def raises(exc, fn, *a):
    try:
        fn(*a)
    except exc:
        return True
    return False


def thin(filetype):
    return b"\xcf\xfa\xed\xfe" + struct.pack("<III", 0x0100000C, 0, filetype) + b"\0" * 48


def fake_app(root):
    """A fake bise.app: a real executable (a copy of /usr/bin/true, thin arm64 slice not needed:
    codesign signs any Mach-O), a nested framework holding a dylib-shaped copy, a script."""
    app = os.path.join(root, "bise.app")
    for d in ("Contents/MacOS", "Contents/Resources/bin", "Contents/Frameworks/X.framework/Versions/A/Resources"):
        os.makedirs(os.path.join(app, d))
    with open(os.path.join(app, "Contents", "Info.plist"), "w") as f:
        f.write('<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>'
                '<key>CFBundleIdentifier</key><string>dev.bise.test</string><key>CFBundleExecutable</key><string>bise</string>'
                '<key>CFBundlePackageType</key><string>APPL</string></dict></plist>')
    fw = os.path.join(app, "Contents/Frameworks/X.framework")
    with open(os.path.join(fw, "Versions/A/Resources/Info.plist"), "w") as f:
        f.write('<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>'
                '<key>CFBundleIdentifier</key><string>dev.bise.x</string><key>CFBundleExecutable</key><string>X</string>'
                '<key>CFBundlePackageType</key><string>FMWK</string></dict></plist>')
    for dst in ("Contents/MacOS/bise", "Contents/Resources/bin/sb-core", "Contents/Resources/bin/bise",
                "Contents/Resources/bin/bend-jsrt", "Contents/Frameworks/X.framework/Versions/A/X"):
        p = os.path.join(app, dst)
        shutil.copy("/usr/bin/true", p)
        subprocess.run(["codesign", "--remove-signature", p], capture_output=True)
    os.symlink("A", os.path.join(fw, "Versions/Current"))
    os.symlink("Versions/Current/X", os.path.join(fw, "X"))
    os.symlink("Versions/Current/Resources", os.path.join(fw, "Resources"))
    with open(os.path.join(app, "Contents/Resources/bin/run.sh"), "w") as f:
        f.write("#!/bin/sh\n")
    with open(os.path.join(app, "Contents/Resources/bin/VERSION"), "w") as f:
        f.write("id=2026.10.2-38\n")
    return app


def tree(top):
    """A bundle's files and links, relative to it. Pure enough: a walk."""
    out = []
    for d, dirs, files in os.walk(top):
        for n in dirs + files:
            p = os.path.join(d, n)
            if n != "_CodeSignature" and "_CodeSignature" not in os.path.relpath(p, top).split(os.sep):
                out.append((os.path.relpath(p, top), "link" if os.path.islink(p) else "dir" if os.path.isdir(p) else "file"))
    return sorted(out)


def test_signs_a_fake_bundle():
    if sys.platform != "darwin":
        return
    with tempfile.TemporaryDirectory() as t:
        app = fake_app(t)
        found = dict(M.scan(app))
        check("Contents/Resources/bin/run.sh" not in found, "a script is not signed on its own")
        check("Contents/Frameworks/X.framework/X" not in found, "a symlink is signed where it points")
        check(found.get("Contents/Frameworks/X.framework") == "bundle", "the framework is a bundle")
        # the runtime's bise as the terminal ships it: bin/bise.app (dev.bise.cli) + the link
        R.cli_bundle(app)
        bindir = os.path.join(app, "Contents/Resources/bin")
        check(os.readlink(os.path.join(bindir, "bise")) == "bise.app/Contents/MacOS/bise", "bin/bise is a link into bise.app")
        check(raises(R.ReleaseError, R.cli_bundle, app), "a second time: the pack must give a real bise")
        steps = S.sign(app, "-")
        check(steps[-1] == (".", "bundle"), steps[-1])
        rels = [r for r, _ in steps]
        check(rels.index("Contents/Resources/bin/bise.app") < rels.index("."), "the nested bise.app before the app")
        d = subprocess.run(["codesign", "-dv", os.path.join(app, "Contents/Resources/bin/sb-core")], capture_output=True, text=True).stderr
        check("Identifier=dev.bise.sb-core" in d and R.hardened(d), d)
        e = subprocess.run(["codesign", "-d", "--entitlements", ":-", app], capture_output=True, text=True).stdout
        check("allow-jit" in e, "the app carries its entitlements")
        nested = os.path.join(bindir, "bise.app")
        r = subprocess.run(["codesign", "--verify", "--deep", "--strict", app], capture_output=True, text=True)
        check(r.returncode == 0, "the outer app verifies deep and strict: %s" % r.stderr)
        d = subprocess.run(["codesign", "-dv", nested], capture_output=True, text=True).stderr
        check("Identifier=dev.bise.cli" in d and R.hardened(d), "the nested bundle is dev.bise.cli, hardened: %s" % d)
        e = subprocess.run(["codesign", "-d", "--entitlements", ":-", nested], capture_output=True, text=True).stdout
        check("audio-input" in e and "allow-jit" not in e, "the nested bise.app has the terminal's entitlements: %s" % e)
        e = subprocess.run(["codesign", "-d", "--entitlements", ":-", os.path.join(bindir, "bend-jsrt")], capture_output=True, text=True).stdout
        check("allow-jit" in e and "audio-input" not in e, "bend-jsrt: V8's own file: %s" % e)
        # the same bundle as a terminal release's app root (build-dist.sh: mac_sign.py bundle)
        root = os.path.join(t, "app")
        os.makedirs(root)
        M.bundle(os.path.join(root, "bise.app"), "/usr/bin/true", M.CLI_ID, "2026.10.2-38", link=True)
        check(tree(os.path.join(root, "bise.app")) == tree(nested), "one tree: %s vs %s" % (tree(os.path.join(root, "bise.app")), tree(nested)))
        with open(os.path.join(root, "bise.app/Contents/Info.plist"), "rb") as a, open(os.path.join(nested, "Contents/Info.plist"), "rb") as b:
            check(sorted(plistlib.load(a)) == sorted(plistlib.load(b)), "one Info.plist's keys")
        check(os.readlink(os.path.join(root, "bise")) == os.readlink(os.path.join(bindir, "bise")), "one link")


def test_a_release_refuses_an_appex_without_its_own_entitlements():
    """architect m_64086: PRESERVE on an appex built without entitlements would ship a Finder
    extension fileproviderd refuses; the release's signing checks every signed appex."""
    ax = "Contents/PlugIns/bise-kb.appex"
    steps = [(ax + "/Contents/MacOS/bise-kb", "exec", M.PRESERVE), (ax, "bundle", M.PRESERVE), ("Contents/MacOS/bise", "exec", None),
             (".", "bundle", None)]
    own = {M.SANDBOX: True, M.GROUPS: ["9G5B2RTD6U.dev.bise.desktop.kb"]}
    asked = []
    ok = lambda p: (asked.append(p), own)[1]  # noqa: E731
    check(S.appex_refusals("/a.app", steps, ok) == [] and asked == ["/a.app/" + ax], "its own (sandbox + group): fine, only the appex read: %s" % asked)
    for bad in ({}, {M.GROUPS: own[M.GROUPS]}, {M.SANDBOX: True}, dict(own, **{"com.apple.security.cs.allow-jit": True})):
        r = S.appex_refusals("/a.app", steps, lambda p: bad)
        check(len(r) == 1 and r[0].startswith(ax + ":"), "refused: %s -> %s" % (bad, r))
    if sys.platform != "darwin":
        return
    # the release's sign() itself: an appex built with its own entitlements keeps them (ad hoc, real
    # codesign), one built without them is refused
    with tempfile.TemporaryDirectory() as t:
        app = fake_app(t)
        appex = os.path.join(app, ax)
        os.makedirs(os.path.join(appex, "Contents", "MacOS"))
        with open(os.path.join(appex, "Contents", "Info.plist"), "wb") as f:
            plistlib.dump({"CFBundleIdentifier": "dev.bise.test.kb", "CFBundleExecutable": "bise-kb", "CFBundlePackageType": "XPC!"}, f)
        shutil.copy("/usr/bin/true", os.path.join(appex, "Contents", "MacOS", "bise-kb"))
        subprocess.run(["codesign", "--remove-signature", os.path.join(appex, "Contents", "MacOS", "bise-kb")], capture_output=True)
        subprocess.run(["codesign", "--force", "--sign", "-", appex], capture_output=True)
        check(raises(S.SignError, S.sign, app, "-"), "an appex built without its entitlements: the release refuses it")
        kb = os.path.join(t, "kb.plist")
        with open(kb, "wb") as f:
            plistlib.dump(own, f)
        subprocess.run(["codesign", "--force", "--sign", "-", "--entitlements", kb, appex], capture_output=True)
        S.sign(app, "-")
        check(M.read_entitlements(appex) == own, "the appex keeps its build's entitlements: %s" % M.read_entitlements(appex))
        check("allow-jit" in str(M.read_entitlements(app)), "the app keeps its own")


def wire(states, version=1):
    """Electron Framework-shaped bytes: some code, the sentinel, a fuse wire, more code."""
    return b"\xcf\xfa\xed\xfe junk" + R.FUSE_SENTINEL + bytes([version, len(states)]) + states + b"\x00 more"


def test_the_fuse_wire():
    default = b"1011001r"  # Electron 44's defaults, read in the audit's bise.app
    locked = b"0000111r"
    w = R.fuse_wires(wire(default))
    check(len(w) == 1 and w[0]["RunAsNode"] is True and w[0]["EnableCookieEncryption"] is False, w)
    check(w[0]["GrantFileProtocolExtraPrivileges"] is None, "'r' is a removed fuse: %s" % w)
    bad = R.fuses_wrong(w)
    check(len(bad) == 5 and any("RunAsNode is True" in b for b in bad), "every one of the 5 is wrong by default: %s" % bad)
    check(R.fuses_wrong(R.fuse_wires(wire(locked))) == [], "the locked wire passes")
    check(R.fuses_wrong(R.fuse_wires(wire(locked) + wire(default)))[0].startswith("fuse "), "a universal binary: both slices")
    check(R.fuses_wrong(R.fuse_wires(b"no sentinel")) == ["no fuse wire in Electron Framework"], "no wire is refused")
    check(len(R.fuses_wrong(R.fuse_wires(wire(b"000")))) == 3, "a short wire lacks --inspect's and the asar fuses")
    check(raises(R.ReleaseError, R.fuse_wires, wire(locked, version=2)), "an unknown wire version stops")
    if shutil.which("node"):
        # electron-builder.mjs flips what the gates read back, nothing more, nothing less
        with tempfile.TemporaryDirectory() as t:
            for f in ("bise", "helper", "tray-badge.node", "VERSION", "repl-live", "sb-core"):
                open(os.path.join(t, f), "w").close()
            js = "import(process.argv[1]).then(m => console.log(JSON.stringify(m.FUSES)))"
            r = subprocess.run(["node", "-e", js, "file://" + os.path.join(R.DESKTOP, "electron-builder.mjs")],
                               capture_output=True, text=True,
                               env=dict(R.pack_env({"PATH": os.environ.get("PATH", "")}, R.paths(t), t),
                                        BISE_PACK_BISE=os.path.join(t, "bise"), BISE_PACK_HELPER=os.path.join(t, "helper"),
                                        BISE_PACK_VERSION=os.path.join(t, "VERSION")))
            check(r.returncode == 0, r.stderr)
            check(json.loads(r.stdout) == R.LOCKED, "electron-builder.mjs FUSES == desktop_release LOCKED: %s" % r.stdout)


def version(tag="desktop-v2026.10.1", built="2026-10-12T08:40:00Z"):
    return F.version_text(F.app_id(tag), built, "c" * 40, "8c13fd8")


def test_tags_and_version():
    check(F.app_id("desktop-v2026.10.1") == "desktop-2026.10.1", "a tag's app id")
    for bad in ("v2026.10.2-34", "desktop-v2026.10", "desktop-latest", "desktop-v2026.10.1-beta", ""):
        check(raises(F.FeedError, F.app_id, bad), "%r is no desktop release tag" % bad)
    check(F.dry_id("0123456789abcdef") == "desktop-dry-0123456789ab", "a dry run's id")
    v = F.read_version(version())
    check(v["id"] == "desktop-2026.10.1" and v["built"] == "2026-10-12T08:40:00Z" and v["runtime_id"] == "8c13fd8", v)
    for bad in ("2026-10-12T08:40:00+00:00", "2026-10-12", "2026-10-12T08:40:00.5Z"):
        check(raises(F.FeedError, F.version_text, "x", bad, "c", "r"), "built %r is refused" % bad)


def feed(tag="desktop-v2026.10.1", built="2026-10-12T08:40:00Z", channel="beta"):
    return F.feed("gvergnaud/bise", tag, channel, version(tag, built), SHA, 1000, SHB, 2000, built)


def test_the_feed_shape():
    f = feed()
    check(f["kind"] == "desktop" and f["channel"] == "beta" and f["version"] == "desktop-v2026.10.1", f)
    check(f["paused"] is False and f["runtime_id"] == "8c13fd8", f)
    t = f["targets"]["darwin-arm64"]
    check(t["id"] == "desktop-2026.10.1" and t["built"] == "2026-10-12T08:40:00Z", "built is the VERSION's, byte for byte")
    check(t["url"] == "https://github.com/gvergnaud/bise/releases/download/desktop-v2026.10.1/bise-mac-arm64.zip", t["url"])
    check(t["dmg"]["url"].endswith("/desktop-v2026.10.1/bise-mac-arm64.dmg") and t["dmg"]["size"] == 2000, t["dmg"])
    check(t["sha256"] == SHA and t["size"] == 1000 and t["min_macos"] == R.MACOS_TARGET, t)
    check(f["notes_url"] == "https://github.com/gvergnaud/bise/releases/tag/desktop-v2026.10.1", f["notes_url"])
    check(F.parse(F.dumps(f)) == f, "the feed round-trips")
    check(raises(F.FeedError, F.feed, "gvergnaud/bise", "desktop-v2026.10.2", "beta", version(), SHA, 1, SHB, 1,
                 "2026-10-12T08:40:00Z"), "a VERSION from another tag is refused")
    check(raises(F.FeedError, F.parse, json.dumps({"version": "x", "targets": {}})), "a terminal latest.json is no feed")
    s = F.on_channel(dict(f, paused=True), "stable", "2026-10-13T00:00:00Z")
    check(s["channel"] == "stable" and s["paused"] is False and s["targets"] == f["targets"], "promote keeps the build")
    check(F.with_paused(f, True)["paused"] is True, "pause")


def test_the_feed_reader_reads_it():
    """The Rust reader's fields, as parse_desktop wants them (kind, targets.<t>.{id,url,sha256,built})."""
    t = feed()["targets"][F.TARGET]
    for k in ("id", "url", "sha256", "built"):
        check(isinstance(t.get(k), str) and t[k], "the reader needs %s" % k)


def test_the_channels():
    n1, n2 = feed(built="2026-10-12T08:40:00Z"), feed("desktop-v2026.10.2", "2026-10-14T08:40:00Z")
    check(F.may_publish(n1, None, "beta") is None, "the first beta")
    check(F.may_publish(n1, n1, "beta") is None, "a re-run")
    check("never goes back" in F.may_publish(n1, n2, "beta"), "beta never goes back")
    check("beta first" in F.may_publish(n1, None, "stable", None), "stable needs a beta")
    check("beta first" in F.may_publish(n2, None, "stable", n1), "stable never ahead of beta")
    check(F.may_publish(n1, None, "stable", n2) is None, "an older build beta went through")
    check(F.may_publish(n2, n1, "stable", n2) is None, "the promote")
    check("never goes back" in F.may_publish(n1, n2, "stable", n2), "stable never goes back")
    check(F.may_publish(n1, None, "nightly") is not None, "two channels only")
    check(F.CHANNELS == {"beta": "desktop-beta", "stable": "desktop-latest"}, F.CHANNELS)
    check(F.notes_from_tag_message("  - one\n- two\n", "t") == "- one\n- two\n", "the tag's notes")
    check(F.notes_from_tag_message("", "desktop-v2026.10.1") == "bise for Mac, desktop-v2026.10.1.\n", "a stand-in")


def test_publish_argv():
    argvs = [P.draft_argv("desktop-v2026.10.1", "n.md", ["a"]), P.publish_build_argv("desktop-v2026.10.1", "beta"),
             P.publish_build_argv("desktop-v2026.10.1", "stable"), P.rolling_create_argv("desktop-beta", "t", "n", "main"),
             P.rolling_edit_argv("desktop-latest", "t", "n"), P.upload_argv("desktop-beta", ["x"])]
    check(P.every_gh_write(argvs), "every gh release create/edit says --latest=false")
    check(not P.every_gh_write([["gh", "release", "edit", "x", "--draft=false"]]), "the pin catches a missing one")
    check("--draft" in argvs[0] and "--prerelease" in argvs[0], "a build is a prerelease draft first")
    check("--prerelease=true" in argvs[1] and "--prerelease=false" in argvs[2], "prerelease until stable has it")
    for n in ("latest.json", "install.sh") + tuple(x for x in P.ASSETS if x.endswith(".tar.gz")):
        check(n not in P.ASSETS, "the desktop never uploads %s" % n)
    check(F.DMG in P.ASSETS and F.FEED in P.ASSETS and F.ZIP in P.ASSETS, P.ASSETS)
    src = open(os.path.join(REPO, "scripts", "desktop_publish.py")).read()
    check("--latest=true" not in src and "make_latest" not in src, "never latest")


def test_runtime_rules():
    check(R.runtime_tag("v2026.10.2-35\ndesktop-v2026.10.1\n") == "v2026.10.2-35", "the terminal tag on the commit")
    check(raises(R.ReleaseError, R.runtime_tag, "desktop-v2026.10.1"), "no terminal release on the commit")
    check(raises(R.ReleaseError, R.runtime_tag, "v1 v2"), "two terminal tags: ambiguous")
    names = ["bise-8c13fd8-darwin-arm64.tar.gz", "bise-8c13fd8-darwin-arm64.tar.gz.sha256", "bise-8c13fd8-darwin-x86_64.tar.gz",
             "latest.json"]
    check(R.archive_name(names) == "bise-8c13fd8-darwin-arm64.tar.gz", "the arm64 archive")
    check(R.skew("rust/proto/x.rs\nrust/home/src/a.rs\napps/desktop/x.ts\nrust/protocol/y\n") == ["rust/proto/x.rs", "rust/home/src/a.rs"],
          "the seams")


def test_pack_and_dmg_settings():
    p = R.paths("/o")
    apple = {"APPLE_API_KEY_PATH": "/k.p8", "APPLE_API_KEY_ID": "K", "APPLE_API_ISSUER": "I", "APPLE_ID": "a@b",
             "APPLE_APP_SPECIFIC_PASSWORD": "p", "APPLE_TEAM_ID": "T", "APPLE_KEYCHAIN_PROFILE": "bise-notary"}
    env = R.pack_env({"PATH": "/bin", "BISE_PACK_SIGN_DIR": "/s", "CSC_LINK": "x", "CSC_KEY_PASSWORD": "y", **apple}, p, "/rt")
    check("BISE_PACK_SIGN_DIR" not in env and "CSC_LINK" not in env and "CSC_KEY_PASSWORD" not in env,
          "no other signing reaches electron-builder")
    check(not [k for k in env if k.startswith("APPLE_")], "no notary key reaches electron-builder (it would notarize)")
    check(env["PATH"] == "/bin" and env["CSC_IDENTITY_AUTO_DISCOVERY"] == "false" and env["BISE_PACK_IDENTITY"] == "none", env)
    check(env["BISE_PACK_APP_ID"] == "dev.bise.desktop" and env["BISE_PACK_NAME"] == "bise", env)
    check(env["BISE_PACK_BISE"] == "/rt/bise" and env["BISE_PACK_ROOT"] == "/rt" and env["BISE_PACK_VERSION"] == "/o/VERSION", env)
    check(p["dmg"].endswith("/release/bise-mac-arm64.dmg") and p["zip"].endswith("/release/bise-mac-arm64.zip"), p)
    lay = R.layout()
    s = R.dmg_settings("/o/bise.app", lay)
    check(s["files"] == ["/o/bise.app"] and s["symlinks"] == {"Applications": "/Applications"}, s)
    check(set(s["icon_locations"]) == {"bise.app", "Applications"}, s["icon_locations"])
    check(s["window_rect"][1] == (lay["window"]["width"], lay["window"]["height"]), s["window_rect"])
    check(os.path.isfile(s["background"]), "the background %s" % s["background"])
    base, ext = os.path.splitext(s["background"])
    check(os.path.isfile(base + "@2x" + ext), "its @2x next to it (one HiDPI tiff)")
    check(not s["show_toolbar"] and not s["show_sidebar"], "a bare window")
    if shutil.which("node"):
        # electron-builder.mjs itself, with the notary keys in the env: it never notarizes
        with tempfile.TemporaryDirectory() as t:
            for f in ("bise", "helper", "tray-badge.node", "VERSION", "repl-live", "sb-core"):
                open(os.path.join(t, f), "w").close()
            js = "import(process.argv[1]).then(m => console.log(JSON.stringify(m.default.mac)))"
            r = subprocess.run(["node", "-e", js, "file://" + os.path.join(R.DESKTOP, "electron-builder.mjs")],
                               capture_output=True, text=True,
                               env=dict(R.pack_env({"PATH": os.environ.get("PATH", ""), **apple}, R.paths(t), t),
                                        BISE_PACK_BISE=os.path.join(t, "bise"), BISE_PACK_HELPER=os.path.join(t, "helper"),
                                        BISE_PACK_VERSION=os.path.join(t, "VERSION"), **apple))
            check(r.returncode == 0, r.stderr)
            mac = json.loads(r.stdout)
            check(mac.get("notarize") is False, "electron-builder's config says notarize: false: %s" % mac)
            check(mac.get("identity") is None, "a release's pack signs nothing (desktop_sign signs after): %s" % mac.get("identity"))
            # the calendar is never read (Apple audit #554): no calendar usage string in Info.plist
            info = mac.get("extendInfo", {})
            check(not [k for k in info if "Calendar" in k], "no calendar usage string: %s" % list(info))
            # an empty VERSION (no id) leaves Info.plist's versions to electron-builder
            check("bundleShortVersion" not in mac and "bundleVersion" not in mac, "no id, no plist version: %s" % mac)
            # a release's VERSION names Info.plist's versions, never package.json's 0.0.0 (pr24), and
            # its short one as About and the settings say it (shortVersion): '2026.10.1', from the
            # app id (desktop-2026.10.1) or a tag-like version= (desktop-v2026.10.3)
            for text, short in ((version(), "2026.10.1"), (version() + "version=desktop-v2026.10.3\n", "2026.10.3")):
                with open(os.path.join(t, "VERSION"), "w") as f:
                    f.write(text)
                r = subprocess.run(["node", "-e", js, "file://" + os.path.join(R.DESKTOP, "electron-builder.mjs")],
                                   capture_output=True, text=True,
                                   env=dict(R.pack_env({"PATH": os.environ.get("PATH", "")}, R.paths(t), t),
                                            BISE_PACK_BISE=os.path.join(t, "bise"), BISE_PACK_HELPER=os.path.join(t, "helper"),
                                            BISE_PACK_VERSION=os.path.join(t, "VERSION")))
                check(r.returncode == 0, r.stderr)
                mac = json.loads(r.stdout)
                check(mac.get("bundleShortVersion") == short and mac.get("bundleVersion") == "c" * 12,
                      "Info.plist's versions from VERSION: %s" % {k: mac.get(k) for k in ("bundleShortVersion", "bundleVersion")})
    check(R.dmgbuild_argv("s.py", "bise", "o.dmg")[-4:] == ["-s", "s.py", "bise", "o.dmg"], "dmgbuild's argv")


def main():
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    for t in tests:
        t()
    print("desktop release: %d tests ok" % len(tests))


if __name__ == "__main__":
    main()
