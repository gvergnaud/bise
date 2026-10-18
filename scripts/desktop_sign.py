#!/usr/bin/env python3
"""desktop-sign: sign bise desktop's bundle from the inside out, hardened, with one identity.

  scripts/desktop_sign.py <bise.app> --identity <sha1|name|-> [--keychain <kc>] [--entitlements <plist>]

The release's signing (desktop-release.yml, scripts/desktop_release.py): never `codesign --deep`.
Every piece of code in the bundle is signed on its own, deepest first, so each one's seal is
made before the bundle around it seals it: the Mach-O files (the runtime's repl-live, sb-core,
bend-jsrt, the native helper, Electron's dylibs and its .node modules), then the nested bundles
(the runtime's bise.app, Electron's frameworks, the 'bise Helper' apps), then bise.app.

The signing core is scripts/mac_sign.py, the terminal release's too (scripts/dist_sign.py):
  - hardened runtime (--options runtime) on everything, a secure timestamp with a real identity
    (--timestamp; none with '-', the dry run's ad hoc identity: Apple's timestamp server only
    stamps certificates);
  - the entitlements (mac_sign.entitlements_for): the desktop's own
    (packaging/entitlements/desktop.plist: V8's JIT for Electron, the microphone, Apple Events)
    on its executables and apps, never on a library; the runtime's bise.app (dev.bise.cli),
    bend-jsrt and repl-live get the terminal release's files, so the hub has one identity, V8
    one set of entitlements and the runtime's TLS loads Homebrew's libssl.3 in both products;
  - a bare executable is named dev.bise.<name>; a bundle keeps its Info.plist's id.

What a symlink points at is signed where it lives, never twice. The pure parts are tested in
tests/mac_sign_test.py, the desktop's use of them in tests/desktop_release_test.py.
"""
import argparse
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, HERE)
import mac_sign as M  # noqa: E402

ENTITLEMENTS = M.DESKTOP_ENT
SignError = M.SignError


def sign(app, identity, keychain=None, entitlements=ENTITLEMENTS, say=None):
    """Sign every piece of the app inside out (mac_sign.sign_tree), then verify the whole: deep
    and strict. With a real identity, the runtime's bise.app must carry a Developer ID
    requirement naming dev.bise.cli (mac_sign.dr_ok)."""
    if not os.path.isdir(app) or not app.rstrip("/").endswith(".app"):
        raise SignError("not an app bundle: %s" % app)
    if entitlements and not os.path.isfile(entitlements):
        raise SignError("no entitlements file at %s" % entitlements)
    steps = M.sign_tree(app, identity, keychain, entitlements, outer=True, say=say)
    r = subprocess.run(["codesign", "--verify", "--deep", "--strict", "--verbose=2", app], capture_output=True, text=True)
    if r.returncode != 0:
        raise SignError("codesign --verify --deep --strict: %s" % r.stderr.strip())
    if identity != "-":
        team = M.read_team(app)
        for rel, kind, _ in steps:
            p = os.path.join(app, rel)
            if kind == "bundle" and M.bundle_id(p) == M.CLI_ID and not M.dr_ok(M.read_dr(p), M.CLI_ID, team or "?"):
                raise SignError("%s: not a Developer ID requirement of %s: %s" % (rel, M.CLI_ID, M.read_dr(p)))
    return [(rel, kind) for rel, kind, _ in steps]


def main(argv):
    ap = argparse.ArgumentParser(prog="desktop_sign.py")
    ap.add_argument("app")
    ap.add_argument("--identity", required=True, help="a codesigning identity (its SHA-1 or name), '-' for ad hoc")
    ap.add_argument("--keychain")
    ap.add_argument("--entitlements", default=ENTITLEMENTS)
    a = ap.parse_args(argv)
    try:
        sign(a.app, a.identity, a.keychain, a.entitlements, say=lambda m: print("desktop-sign:", m, file=sys.stderr))
        return 0
    except SignError as e:
        print("desktop-sign: %s" % e, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
