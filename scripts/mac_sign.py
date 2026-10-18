#!/usr/bin/env python3
"""mac-sign: the one signing core of every macOS build bise ships, and the one bundle builder.

  scripts/mac_sign.py bundle <bise.app> <exe> --id <bundle id> --version <v> [--link]
      make <bise.app> (new) around a copy of <exe>: Contents/MacOS/bise and Info.plist
      (info_plist). --link: also <dir>/bise -> bise.app/Contents/MacOS/bise, replacing a file there.

Used by scripts/desktop_sign.py (the desktop app), scripts/dist_sign.py (the terminal release's
app root, packaging/build-dist.sh) and scripts/dev-sign.sh (the dev builds' bise.app, its own
self-signed identity): what is code in a tree (Mach-O scan, deepest first), the codesign argv,
which entitlements a piece gets (one folder for both products: packaging/entitlements/), a bare
executable's identifier, and the designated requirement a Developer ID gives (no cdhash: the same
for every version, what TCC keys a grant on).

The hardened runtime's library validation: a signed piece loads only libraries signed by Apple or
its own team. The agent runtime dlopens Homebrew's libssl.3 for TLS, so it carries
runtime.plist (disable-library-validation); packaging/test-install.sh's TLS turn is the check.

Why a bundle: macOS keys a plain executable's grant (Files and Folders, App Management) on its
path and signature, a bundle's on its id and signature (docs/issues/15-tcc-one-grant.md). Every
version is a new dir, so the hub runs <version>/bise.app/Contents/MacOS/bise, through the link
<version>/bise: one grant for every version. The bundle ids: dev.bise.cli for what ships (the
terminal release and the desktop app's runtime, CLI_ID), dev.bise.bise for the dev builds, so the
two signers never share one id in TCC.

Pure parts tested in tests/mac_sign_test.py (with an ad hoc signing of a fake bundle).
"""
import argparse
import os
import plistlib
import re
import shutil
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
REPO = os.path.dirname(HERE)
ENT_DIR = os.path.join(REPO, "packaging", "entitlements")
# what the desktop gives its own executables and apps (Electron's JIT, the mic, Apple Events)
DESKTOP_ENT = os.path.join(ENT_DIR, "desktop.plist")
# the terminal's bise.app, wherever it is (a release's app root, the desktop's Resources/bin)
CLI_ENT = os.path.join(ENT_DIR, "bise.plist")
# V8: the same file in both products
JSRT_ENT = os.path.join(ENT_DIR, "bend-jsrt.plist")
# the agent runtime, which dlopens Homebrew's libssl.3 for TLS: the same file in both products
RUNTIME_ENT = os.path.join(ENT_DIR, "runtime.plist")
RUNTIME_EXES = ("repl-live", "repl-scripted")
CLI_ID = "dev.bise.cli"
DEV_ID = "dev.bise.bise"
BUNDLES = (".app", ".framework", ".xpc", ".appex", ".bundle")
MH_EXECUTE = 2


class SignError(Exception):
    pass


# ---- what is code --------------------------------------------------------------------------

def macho_kind(head):
    """'exec', 'lib' or None for a file's first bytes (64 are enough for a thin file; a fat one
    needs its first slice's header too: see read_kind). Pure."""
    if len(head) < 16:
        return None
    if head[:4] in (b"\xcf\xfa\xed\xfe", b"\xce\xfa\xed\xfe"):
        return "exec" if struct.unpack("<I", head[12:16])[0] == MH_EXECUTE else "lib"
    if head[:4] in (b"\xfe\xed\xfa\xcf", b"\xfe\xed\xfa\xce"):
        return "exec" if struct.unpack(">I", head[12:16])[0] == MH_EXECUTE else "lib"
    return None


def fat_slice(head):
    """The offset of a universal binary's first slice, or None (not fat; a Java class shares the
    magic but its 'count' is its version, 45 and up). Pure."""
    if len(head) < 28 or head[:4] != b"\xca\xfe\xba\xbe":
        return None
    n = struct.unpack(">I", head[4:8])[0]
    if not 0 < n < 20:
        return None
    return struct.unpack(">I", head[16:20])[0]


def read_kind(path):
    try:
        with open(path, "rb") as f:
            head = f.read(64)
            off = fat_slice(head)
            if off is not None:
                f.seek(off)
                head = f.read(64)
    except OSError:
        return None
    return macho_kind(head)


def scan(top):
    """Every piece of code under top, as (path relative to it, 'exec' | 'lib' | 'bundle'):
    real files and dirs only (a symlink is the place it points at, signed there)."""
    found = []
    for d, dirs, files in os.walk(top):
        rel = os.path.relpath(d, top)
        for name in sorted(dirs):
            p = os.path.join(d, name)
            if not os.path.islink(p) and name.endswith(BUNDLES):
                found.append((os.path.normpath(os.path.join(rel, name)), "bundle"))
        dirs[:] = [n for n in dirs if not os.path.islink(os.path.join(d, n))]
        for name in sorted(files):
            p = os.path.join(d, name)
            if os.path.islink(p):
                continue
            k = read_kind(p)
            if k:
                found.append((os.path.normpath(os.path.join(rel, name)), k))
    return found


def depth(rel):
    return len([c for c in rel.split(os.sep) if c not in ("", ".")])


def plan(items, outer=True):
    """The signing order: deepest first (what a bundle holds is sealed before the bundle), then
    by path, so two runs sign alike; with outer, the top itself ('.', an app) last. Pure."""
    inner = sorted((i for i in items if i[0] != "."), key=lambda i: (-depth(i[0]), i[0]))
    return inner + ([(".", "bundle")] if outer else [])


def ident(rel):
    """A bare executable's identifier: dev.bise.<name>, its name's spaces and odd chars as '-'. Pure."""
    return "dev.bise." + re.sub(r"[^A-Za-z0-9.-]+", "-", os.path.basename(rel)).strip("-")


def bundle_id(path):
    """A bundle's CFBundleIdentifier (None when it has no readable Info.plist)."""
    for p in (os.path.join(path, "Contents", "Info.plist"), os.path.join(path, "Resources", "Info.plist")):
        try:
            with open(p, "rb") as f:
                return plistlib.load(f).get("CFBundleIdentifier")
        except (OSError, plistlib.InvalidFileException, ValueError):
            continue
    return None


# ---- what each piece gets ------------------------------------------------------------------

def is_app(rel):
    return rel == "." or rel.rstrip("/").endswith(".app")


def in_cli(rel, cli_bundles):
    """The main executable of one of the CLI's bundles (rel under <b>/Contents/MacOS/). Pure."""
    return any(rel.startswith(os.path.join(b, "Contents", "MacOS") + os.sep) for b in cli_bundles if b != ".")


def entitlements_for(rel, kind, default=None, bid=None, cli_bundles=()):
    """The entitlements file of one piece, or None. Pure.
      - bend-jsrt: JSRT_ENT, in both products;
      - the agent runtime (repl-live, repl-scripted): RUNTIME_ENT, in both products (its TLS
        loads Homebrew's libssl.3, which library validation refuses);
      - the CLI's bundle (id CLI_ID or DEV_ID) and its main executable: CLI_ENT;
      - a library or a framework: never;
      - else an executable or an app: the caller's default (the desktop's file; None for a
        terminal release's other binaries: sb-core needs nothing)."""
    if kind == "lib":
        return None
    if kind == "exec" and os.path.basename(rel) == "bend-jsrt":
        return JSRT_ENT
    if kind == "exec" and os.path.basename(rel) in RUNTIME_EXES:
        return RUNTIME_ENT
    if (kind == "bundle" and bid in (CLI_ID, DEV_ID)) or (kind == "exec" and in_cli(rel, cli_bundles)):
        return CLI_ENT
    if kind == "exec" or (kind == "bundle" and is_app(rel)):
        return default
    return None


def codesign_argv(path, kind, identity, entitlements=None, keychain=None, rel=None):
    """codesign's argv for one step, entitlements as decided by entitlements_for. Pure."""
    argv = ["codesign", "--force", "--sign", identity, "--options", "runtime",
            "--timestamp=none" if identity == "-" else "--timestamp"]
    if keychain:
        argv += ["--keychain", keychain]
    if kind == "exec":
        argv += ["--identifier", ident(rel or path)]
    if entitlements:
        argv += ["--entitlements", entitlements]
    return argv + [path]


def sign_tree(top, identity, keychain=None, default=None, outer=True, say=None):
    """Sign every piece under top, inside out (plan(scan(top))), each with entitlements_for.
    Returns the steps as (rel, kind, entitlements)."""
    steps = plan(scan(top), outer)
    bids = {rel: bundle_id(os.path.normpath(os.path.join(top, rel))) for rel, k in steps if k == "bundle"}
    cli = [rel for rel, b in bids.items() if b in (CLI_ID, DEV_ID)]
    done = []
    for rel, kind in steps:
        path = os.path.normpath(os.path.join(top, rel))
        ent = entitlements_for(rel, kind, default, bids.get(rel), cli)
        if ent and not os.path.isfile(ent):
            raise SignError("no entitlements file at %s" % ent)
        r = subprocess.run(codesign_argv(path, kind, identity, ent, keychain, rel), capture_output=True, text=True)
        if r.returncode != 0:
            raise SignError("codesign %s: %s" % (rel, (r.stderr or r.stdout).strip()))
        done.append((rel, kind, ent))
    if say:
        say("signed %d pieces of code in %s, inside out" % (len(done), os.path.basename(os.path.abspath(top))))
    return done


# ---- the designated requirement -----------------------------------------------------------

def designated(output):
    """The designated requirement from `codesign -d -r-`'s output, or ''. Pure."""
    m = re.search(r"designated => (.*)", output)
    return m.group(1).strip() if m else ""


def team_of(codesign_dv):
    """The TeamIdentifier from `codesign -dv`'s output, or None (ad hoc: 'not set'). Pure."""
    m = re.search(r"^TeamIdentifier=(.+)$", codesign_dv, re.M)
    t = m.group(1).strip() if m else ""
    return t if t and t != "not set" else None


def dr_ok(req, want_id, team):
    """Whether a designated requirement is a Developer ID one that names want_id and team, with
    no cdhash in it (a cdhash changes with every build: TCC would ask again). Pure."""
    if not req or "cdhash" in req or "certificate leaf = H" in req:
        return False
    if 'identifier "%s"' % want_id not in req or "anchor apple generic" not in req:
        return False
    return re.search(r'certificate leaf\[subject\.OU\] = "?%s"?' % re.escape(team), req) is not None


def read_dr(path):
    r = subprocess.run(["codesign", "-d", "-r-", path], capture_output=True, text=True)
    return designated(r.stdout + r.stderr)


def read_team(path):
    r = subprocess.run(["codesign", "-dv", path], capture_output=True, text=True)
    return team_of(r.stderr + r.stdout)


# ---- the bundle ----------------------------------------------------------------------------

def info_plist(bid, version):
    """The Info.plist of the terminal's bise.app: one executable, no Dock icon, and the two usage
    strings TCC shows (voice; osascript under the hub). Pure."""
    return {
        "CFBundleIdentifier": bid,
        "CFBundleName": "bise",
        "CFBundleExecutable": "bise",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": version,
        "LSUIElement": True,
        "NSMicrophoneUsageDescription": "bise listens when you talk to it (voice).",
        "NSAppleEventsUsageDescription": "bise and its agents run AppleScript you allow (osascript).",
    }


def bundle(app, exe, bid, version, link=False):
    """Make <app> (a new path, named bise.app) around a copy of <exe>; with link, <dir>/bise ->
    bise.app/Contents/MacOS/bise (a file or link there is replaced). Unsigned: the caller signs."""
    if os.path.basename(app.rstrip("/")) != "bise.app":
        raise SignError("the bundle must be named bise.app: %s" % app)
    if os.path.lexists(app):
        raise SignError("%s exists" % app)
    if not os.path.isfile(exe):
        raise SignError("no executable at %s" % exe)
    macos = os.path.join(app, "Contents", "MacOS")
    os.makedirs(macos)
    shutil.copy(exe, os.path.join(macos, "bise"))
    with open(os.path.join(app, "Contents", "Info.plist"), "wb") as f:
        plistlib.dump(info_plist(bid, version), f)
    if link:
        ln = os.path.join(os.path.dirname(app.rstrip("/")), "bise")
        if os.path.lexists(ln):
            os.remove(ln)
        os.symlink("bise.app/Contents/MacOS/bise", ln)
    return app


def main(argv):
    ap = argparse.ArgumentParser(prog="mac_sign.py")
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("bundle")
    b.add_argument("app")
    b.add_argument("exe")
    b.add_argument("--id", required=True)
    b.add_argument("--version", required=True)
    b.add_argument("--link", action="store_true")
    a = ap.parse_args(argv)
    try:
        if a.cmd == "bundle":
            bundle(a.app, a.exe, a.id, a.version, a.link)
        return 0
    except (SignError, OSError) as e:
        print("mac-sign: %s" % e, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
