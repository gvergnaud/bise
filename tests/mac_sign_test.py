#!/usr/bin/env python3
"""The one macOS signing core (scripts/mac_sign.py) and the terminal release's signing
(scripts/dist_sign.py, scripts/notary.py): pure parts, and an ad hoc signing of a fake app root.

  - what is code (thin, fat, Java class, text), the order (deepest first, the top last), the
    codesign argv (hardened, a timestamp only with a certificate);
  - the entitlements: one folder (packaging/entitlements/), bend-jsrt's file the same in both
    pipelines (the terminal's and the desktop's), bise.app's on the CLI bundle and its
    executable, never on a library;
  - the designated requirement: a Developer ID one names the id and the team, never a cdhash;
  - the bundle builder: bise.app's tree, Info.plist and the bise link;
  - an official release never signs ad hoc: --required without an identity or notary credentials
    fails, naming what is missing;
  - a fake app root signed ad hoc by dist_sign, verified strict.

No network, no keychain, no notarization.

  python3 -B tests/mac_sign_test.py
"""
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
import desktop_sign as S  # noqa: E402
import dist_sign as D  # noqa: E402
import mac_sign as M  # noqa: E402
import notary as N  # noqa: E402


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


def test_what_is_code():
    check(M.macho_kind(thin(2)) == "exec", "MH_EXECUTE is an executable")
    check(M.macho_kind(thin(6)) == "lib", "MH_DYLIB is a library")
    check(M.macho_kind(thin(8)) == "lib", "MH_BUNDLE (a .node module) is signed like a library")
    check(M.macho_kind(b"#!/bin/sh\necho hi\n" + b"\0" * 50) is None, "a script is not code")
    fat = b"\xca\xfe\xba\xbe" + struct.pack(">I", 2) + struct.pack(">IIIII", 0x0100000C, 0, 4096, 100, 14) + b"\0" * 40
    check(M.fat_slice(fat) == 4096, "a universal binary's first slice")
    java = b"\xca\xfe\xba\xbe" + struct.pack(">I", 52) + b"\0" * 56
    check(M.fat_slice(java) is None, "a Java class is not a universal binary")


def test_the_order_and_argv():
    items = [("Contents/MacOS/bise", "exec"), ("Contents/Frameworks/Electron Framework.framework", "bundle"),
             ("Contents/Frameworks/Electron Framework.framework/Versions/A/Electron Framework", "lib"),
             ("Contents/Frameworks/bise Helper.app", "bundle"),
             ("Contents/Frameworks/bise Helper.app/Contents/MacOS/bise Helper", "exec"),
             ("Contents/Resources/bin/bise.app", "bundle"),
             ("Contents/Resources/bin/bise.app/Contents/MacOS/bise", "exec")]
    order = [r for r, _ in M.plan(items)]
    check(order[-1] == ".", "the app is signed last")
    for inner, outer in (("Contents/Frameworks/Electron Framework.framework/Versions/A/Electron Framework",
                          "Contents/Frameworks/Electron Framework.framework"),
                         ("Contents/Frameworks/bise Helper.app/Contents/MacOS/bise Helper", "Contents/Frameworks/bise Helper.app"),
                         ("Contents/Resources/bin/bise.app/Contents/MacOS/bise", "Contents/Resources/bin/bise.app"),
                         ("Contents/Resources/bin/bise.app", ".")):
        check(order.index(inner) < order.index(outer), "%s before the bundle around it" % inner)
    check(M.plan(list(reversed(items))) == M.plan(items), "the order doesn't depend on the walk")
    check(M.plan(items, outer=False)[-1][0] != ".", "an app root (not a bundle) is not signed itself")
    a = M.codesign_argv("/x/bin/sb-core", "exec", "ABC", "/e.plist", "/k", "bin/sb-core")
    check("--options" in a and a[a.index("--options") + 1] == "runtime", "hardened runtime")
    check("--timestamp" in a and "--timestamp=none" not in a, "a certificate gets a secure timestamp")
    check(a[a.index("--identifier") + 1] == "dev.bise.sb-core", "a bare executable is dev.bise.<name>")
    check(a[a.index("--entitlements") + 1] == "/e.plist" and a[a.index("--keychain") + 1] == "/k", "entitlements and keychain")
    check("--entitlements" not in M.codesign_argv("/x", "exec", "ABC", None), "no entitlements: none passed")
    adhoc = M.codesign_argv("/x", "bundle", "-", None, None, ".")
    check("--timestamp=none" in adhoc and "--timestamp" not in adhoc, "ad hoc: no timestamp (Apple stamps certificates only)")
    check(M.ident("Contents/Frameworks/bise Helper (GPU)") == "dev.bise.bise-Helper-GPU", M.ident("bise Helper (GPU)"))


def test_entitlements():
    E = M.entitlements_for
    # V8: the same file whatever the pipeline's default (the desktop's, or the terminal's none)
    jsrt_desktop = E("Contents/Resources/bin/bend-jsrt", "exec", S.ENTITLEMENTS)
    jsrt_terminal = E("bend-jsrt", "exec", None)
    check(jsrt_desktop == jsrt_terminal == M.JSRT_ENT, "bend-jsrt: one file in both pipelines (%s, %s)" % (jsrt_desktop, jsrt_terminal))
    check(os.path.dirname(M.JSRT_ENT) == os.path.dirname(S.ENTITLEMENTS) == M.ENT_DIR, "one entitlements folder")
    # the CLI's bundle and its executable: bise.plist, in both
    for default in (None, S.ENTITLEMENTS):
        check(E("bise.app", "bundle", default, M.CLI_ID) == M.CLI_ENT, "bise.app (dev.bise.cli) gets bise.plist")
        check(E("bise.app/Contents/MacOS/bise", "exec", default, None, ["bise.app"]) == M.CLI_ENT, "its executable too")
        check(E("x/l.dylib", "lib", default) is None, "a library gets no entitlements")
        check(E("X.framework", "bundle", default) is None, "a framework gets no entitlements")
    check(E("Contents/Resources/bin/bise.app", "bundle", S.ENTITLEMENTS, M.CLI_ID) == M.CLI_ENT, "nested in the desktop too")
    check(E(".", "bundle", S.ENTITLEMENTS, "dev.bise.desktop") == S.ENTITLEMENTS, "the desktop app keeps its own")
    check(E("Contents/MacOS/bise", "exec", S.ENTITLEMENTS, None, ["Contents/Resources/bin/bise.app"]) == S.ENTITLEMENTS,
          "the desktop's own executable keeps its own")
    check(E("sb-core", "exec", None) is None, "sb-core needs nothing")
    # the agent runtime loads Homebrew's libssl.3 for TLS: one file in both pipelines (-41's bug)
    for exe in ("repl-live", "repl-scripted"):
        check(E(exe, "exec", None) == E("Contents/Resources/bin/" + exe, "exec", S.ENTITLEMENTS) == M.RUNTIME_ENT,
              "%s: runtime.plist in both pipelines" % exe)
    check(E("bise Computer Use.app", "bundle", None, "dev.bise.computer-use") is None, "the helper: none in a release")
    for f in (M.CLI_ENT, M.JSRT_ENT, M.DESKTOP_ENT, M.RUNTIME_ENT):
        with open(f, "rb") as fh:
            e = plistlib.load(fh)
        check(not any("get-task-allow" in k for k in e), "%s: nothing notarization refuses" % f)
        check(not any("calendars" in k or "personal-information" in k for k in e), "%s: no calendar entitlement" % f)
        check(("com.apple.security.cs.disable-library-validation" in e) == (f == M.RUNTIME_ENT),
              "%s: library validation off only where libssl is loaded" % f)
    with open(M.RUNTIME_ENT, "rb") as fh:
        r = plistlib.load(fh)
    check(r == {"com.apple.security.cs.disable-library-validation": True}, "the runtime: library validation off, nothing else")
    with open(M.JSRT_ENT, "rb") as fh:
        j = plistlib.load(fh)
    check(j.get("com.apple.security.cs.allow-jit") is True and j.get("com.apple.security.cs.allow-unsigned-executable-memory") is True, "V8's JIT")
    with open(M.CLI_ENT, "rb") as fh:
        c = plistlib.load(fh)
    check(c.get("com.apple.security.device.audio-input") is True and "com.apple.security.cs.allow-jit" not in c, "bise: the mic, no JIT")


def test_designated_requirement():
    dev_id = ('identifier "dev.bise.cli" and anchor apple generic and certificate 1[field.1.2.840.113635.100.6.2.6] '
              '/* exists */ and certificate leaf[field.1.2.840.113635.100.6.1.13] /* exists */ and '
              'certificate leaf[subject.OU] = "9G5B2RTD6U"')
    check(M.dr_ok(dev_id, "dev.bise.cli", "9G5B2RTD6U"), "a Developer ID requirement")
    check(M.dr_ok(dev_id.replace('"9G5B2RTD6U"', "AB12CD34EF"), "dev.bise.cli", "AB12CD34EF"), "an unquoted team")
    check(not M.dr_ok(dev_id, "dev.bise.bise", "9G5B2RTD6U"), "another identifier")
    check(not M.dr_ok(dev_id, "dev.bise.cli", "XXXXXXXXXX"), "another team")
    check(not M.dr_ok('cdhash H"e1a412f012c09a9262868641cf2cd73ca9143ab9"', "dev.bise.cli", "9G5B2RTD6U"), "ad hoc: a cdhash")
    check(not M.dr_ok('identifier "dev.bise.cli" and certificate leaf = H"0a1b"', "dev.bise.cli", "0a1b"), "self-signed: a leaf hash")
    check(M.designated("Executable=/x\n# designated => %s\n" % dev_id) == dev_id, "read from codesign -d -r-")
    check(M.team_of("Identifier=x\nTeamIdentifier=9G5B2RTD6U\n") == "9G5B2RTD6U", "the team")
    check(M.team_of("TeamIdentifier=not set\n") is None, "ad hoc has no team")


def test_the_bundle():
    info = M.info_plist(M.CLI_ID, "2026.10.2-38")
    check(info["CFBundleIdentifier"] == "dev.bise.cli" and info["CFBundleExecutable"] == "bise", info)
    check(info["LSUIElement"] is True and "NSMicrophoneUsageDescription" in info and "NSAppleEventsUsageDescription" in info, info)
    with tempfile.TemporaryDirectory() as t:
        exe = os.path.join(t, "exe")
        shutil.copy("/bin/echo" if os.path.exists("/bin/echo") else sys.executable, exe)
        open(os.path.join(t, "bise"), "w").close()  # a plain bise there: replaced by the link
        M.bundle(os.path.join(t, "bise.app"), exe, M.CLI_ID, "v1", link=True)
        check(os.readlink(os.path.join(t, "bise")) == D.LINK, "bise -> %s" % D.LINK)
        check(os.path.isfile(os.path.join(t, "bise")), "the link resolves")
        check(M.bundle_id(os.path.join(t, "bise.app")) == M.CLI_ID, "its id")
        check(D.layout_wrong(t) == [], D.layout_wrong(t))
        check(raises(M.SignError, M.bundle, os.path.join(t, "bise.app"), exe, M.CLI_ID, "v1"), "never over a bundle")
        check(raises(M.SignError, M.bundle, os.path.join(t, "other.app"), exe, M.CLI_ID, "v1"), "always named bise.app")
        os.remove(os.path.join(t, "bise"))
        check(any("link" in b for b in D.layout_wrong(t)), "a root without the link is refused")


def test_a_release_is_never_ad_hoc():
    key = {"APPLE_API_KEY_PATH": "/k.p8", "APPLE_API_KEY_ID": "K", "APPLE_API_ISSUER": "I"}
    check(D.choose(None, False, False, {}) == "-", "a fork / local build: ad hoc")
    check(D.choose("ABC", False, False, {}) == "ABC", "an identity signs")
    check(raises(M.SignError, D.choose, None, True, False, key), "required, no identity: fails")
    check(raises(M.SignError, D.choose, "ABC", True, True, {}), "required, no notary credentials: fails")
    check(raises(M.SignError, D.choose, None, False, True, key), "notarize ad hoc: fails")
    check(D.choose("ABC", True, True, key) == "ABC", "required with everything")
    try:
        D.choose(None, True, False, key)
    except M.SignError as e:
        check("BISE_SIGN_ID" in str(e), "the error names what's missing: %s" % e)
    try:
        D.choose("ABC", True, True, {"APPLE_API_KEY_PATH": "/k"})
    except M.SignError as e:
        check("APPLE_API_KEY_ID" in str(e) and "APPLE_API_ISSUER" in str(e), "names the missing keys: %s" % e)


def test_notary_parts():
    key = {"APPLE_API_KEY_PATH": "/k.p8", "APPLE_API_KEY_ID": "K", "APPLE_API_ISSUER": "I", "APPLE_KEYCHAIN_PROFILE": "p"}
    check(N.creds(key)[:2] == ["--key", "/k.p8"], "the API key first")
    check(N.creds({"APPLE_KEYCHAIN_PROFILE": "bise-notary"}) == ["--keychain-profile", "bise-notary"], "else a profile")
    check(raises(N.NotaryError, N.creds, {}), "neither: an error")
    a = N.notary_argv("/x.zip", ["--keychain-profile", "p"])
    check(a[:4] == ["xcrun", "notarytool", "submit", "/x.zip"] and "--wait" in a, a)
    check(N.verdict('{"status": "Accepted", "id": "1"}')["status"] == "Accepted" and N.verdict("oops") == {}, "the verdict")


def test_staple_retries():
    class R:
        def __init__(self, rc):
            self.returncode, self.stdout, self.stderr = rc, "", "Error 68" if rc else ""
    real, calls, waits = N.subprocess.run, [], []
    try:
        answers = [1, 1, 0, 0]  # staple fails twice (Apple's ticket not served yet), then works; validate works
        N.subprocess.run = lambda argv, **kw: (calls.append(argv[2]), R(answers.pop(0)))[1]
        N.staple("/a.app", sleep=waits.append)
        check(calls == ["staple", "staple", "staple", "validate"] and waits == [3, 5], "staple is tried again after Error 68, short first")
        check(N.STAPLE_WAITS == (3, 5, 10, 20, 30), "the backoff: 3, 5, 10, 20, 30 s")
        calls.clear()
        waits.clear()
        N.subprocess.run = lambda argv, **kw: (calls.append(argv[2]), R(1))[1]
        check(raises(N.NotaryError, N.staple, "/a.app", (1, 2), waits.append) and calls == ["staple"] * 3 and waits == [1, 2],
              "it fails after its tries")
    finally:
        N.subprocess.run = real


def test_wanted_ids():
    steps = [("bise.app/Contents/MacOS/bise", "exec", None), ("bise Computer Use.app", "bundle", None),
             ("bise.app", "bundle", None), ("sb-core", "exec", None), ("plugins/x/bin/tool", "exec", None)]
    w = D.wanted_ids(steps, {"bise.app": M.CLI_ID, "bise Computer Use.app": "dev.bise.computer-use"})
    check(w == {"bise.app": M.CLI_ID, "bise Computer Use.app": "dev.bise.computer-use", "sb-core": "dev.bise.sb-core"}, w)


def fake_root(t):
    """A fake terminal app root: bise.app + its link, two bare binaries, a helper app, data."""
    root = os.path.join(t, "app")
    os.makedirs(root)
    exe = os.path.join(t, "exe")
    shutil.copy("/usr/bin/true", exe)
    subprocess.run(["codesign", "--remove-signature", exe], capture_output=True)
    M.bundle(os.path.join(root, "bise.app"), exe, M.CLI_ID, "v1", link=True)
    os.symlink("bise", os.path.join(root, "sb"))
    for b in ("sb-core", "bend-jsrt", "repl-live"):
        shutil.copy(exe, os.path.join(root, b))
    h = os.path.join(root, "bise Computer Use.app", "Contents")
    os.makedirs(os.path.join(h, "MacOS"))
    shutil.copy(exe, os.path.join(h, "MacOS", "bise-computer-use"))
    with open(os.path.join(h, "Info.plist"), "wb") as f:
        plistlib.dump({"CFBundleIdentifier": "dev.bise.computer-use", "CFBundleExecutable": "bise-computer-use",
                       "CFBundlePackageType": "APPL"}, f)
    with open(os.path.join(root, "VERSION"), "w") as f:
        f.write("id=v1\n")
    return root


def test_signs_a_fake_root():
    if sys.platform != "darwin":
        return
    with tempfile.TemporaryDirectory() as t:
        root = fake_root(t)
        steps = D.sign(root, None)
        rels = [r for r, _, _ in steps]
        check(rels.index("bise.app/Contents/MacOS/bise") < rels.index("bise.app"), "inside out")
        check("bise" not in rels and "sb" not in rels, "links are never signed")
        ents = {r: e for r, _, e in steps}
        check(ents["bise.app"] == M.CLI_ENT and ents["bend-jsrt"] == M.JSRT_ENT and ents["sb-core"] is None
              and ents["repl-live"] == M.RUNTIME_ENT, ents)
        dv = subprocess.run(["codesign", "-dv", os.path.join(root, "bise.app")], capture_output=True, text=True).stderr
        check("Identifier=dev.bise.cli" in dv, dv)
        e = subprocess.run(["codesign", "-d", "--entitlements", ":-", os.path.join(root, "bend-jsrt")], capture_output=True, text=True).stdout
        check("allow-jit" in e, "bend-jsrt carries V8's entitlements")
        e = subprocess.run(["codesign", "-d", "--entitlements", ":-", os.path.join(root, "repl-live")], capture_output=True, text=True).stdout
        check("disable-library-validation" in e, "repl-live may load Homebrew's libssl")
        r = subprocess.run([os.path.join(root, "bise")], capture_output=True)
        check(r.returncode == 0, "the signed bise runs through its link")


def main():
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    for t in tests:
        t()
    print("mac sign: %d tests ok" % len(tests))


if __name__ == "__main__":
    main()
