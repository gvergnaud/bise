#!/usr/bin/env python3
"""dist-sign: sign (and notarize) a terminal release's app root on macOS (packaging/build-dist.sh).

  scripts/dist_sign.py <app root> [--identity <sha1|name>] [--keychain <kc>] [--required]
      [--notarize] [--work <dir>]

The app root holds bise.app (dev.bise.cli, made by scripts/mac_sign.py bundle) with bise a link
into it, the bare binaries (repl-live, repl-scripted, sb-core, bend-jsrt), the computer-use
helper 'bise Computer Use.app' and data. Every piece of code is signed on its own, inside out,
never --deep, hardened runtime, with the entitlements of scripts/mac_sign.py entitlements_for
(packaging/entitlements/: bise.plist on bise.app, bend-jsrt.plist on V8, runtime.plist on
repl-live and repl-scripted (their TLS loads Homebrew's libssl.3), none on sb-core and the helper), then checked: codesign --verify --strict on each, and with a real
identity the hardened flag and a Developer ID designated requirement on bise.app and every bare
executable (identifier + team, no cdhash: the same for every version, what TCC keys a grant on).

--notarize: one submission for the whole root (a zip of it: every binary's ticket), then the
bundles are stapled (bise.app, the helper); a bare Mach-O and the tarball can't carry a ticket.
Then Gatekeeper's verdict on bise.app (spctl: 'Notarized Developer ID'). Credentials: scripts/notary.py.

No identity: ad hoc ('-'), the fork / local path. --required (a v* tag, or release.yml's input
sign): no identity, or --notarize without credentials, FAILS with the missing name; an official
release never ships ad hoc binaries quietly. Pure parts tested in tests/mac_sign_test.py.
"""
import argparse
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, HERE)
import mac_sign as M  # noqa: E402
import notary as N  # noqa: E402

LINK = "bise.app/Contents/MacOS/bise"


def say(*a):
    print("dist-sign:", *a, file=sys.stderr)


def choose(identity, required, notarize, env):
    """The identity to sign with ('-' for ad hoc), or a SignError naming what an official release
    lacks. Pure."""
    if required and not identity:
        raise M.SignError("a release must be signed: no BISE_SIGN_ID (a Developer ID Application identity)")
    if notarize and not identity:
        raise M.SignError("--notarize needs an identity (BISE_SIGN_ID), not ad hoc")
    if notarize or required:
        try:
            N.creds(env)
        except N.NotaryError as e:
            if required:
                raise M.SignError(str(e))
    return identity or "-"


def layout_wrong(root):
    """What is wrong with the root's bise layout (bise.app + the bise link), one line each."""
    bad = []
    if M.bundle_id(os.path.join(root, "bise.app")) != M.CLI_ID:
        bad.append("bise.app is missing or not %s" % M.CLI_ID)
    ln = os.path.join(root, "bise")
    if not os.path.islink(ln) or os.readlink(ln) != LINK:
        bad.append("bise is not a link to %s" % LINK)
    return bad


def wanted_ids(steps, bids):
    """The identifier each checked piece's requirement must name: bise.app and every bundle
    directly in the root by their bundle id; every bare executable directly in the root by
    dev.bise.<name>. Pure."""
    want = {}
    for rel, kind, _ in steps:
        if os.sep in rel:
            continue
        if kind == "bundle" and bids.get(rel):
            want[rel] = bids[rel]
        elif kind == "exec":
            want[rel] = M.ident(rel)
    return want


def check(root, steps, identity):
    """The gates after signing, a list of what's wrong."""
    bad = []
    bids = {rel: M.bundle_id(os.path.join(root, rel)) for rel, k, _ in steps if k == "bundle"}
    want = wanted_ids(steps, bids)
    team = M.read_team(os.path.join(root, "bise.app")) if identity != "-" else None
    if identity != "-" and not team:
        bad.append("bise.app has no team: not a Developer ID signature")
    for rel in sorted(want):
        p = os.path.join(root, rel)
        r = subprocess.run(["codesign", "--verify", "--strict", p], capture_output=True, text=True)
        if r.returncode:
            bad.append("%s: %s" % (rel, r.stderr.strip()))
            continue
        if identity == "-":
            continue
        dv = subprocess.run(["codesign", "-dv", p], capture_output=True, text=True).stderr
        if "(runtime)" not in dv:
            bad.append("%s has no hardened runtime" % rel)
        if team and not M.dr_ok(M.read_dr(p), want[rel], team):
            bad.append("%s: not a Developer ID requirement of %s: %s" % (rel, want[rel], M.read_dr(p)))
    return bad


def gatekeeper(app):
    """spctl's verdict on a notarized bundle: '' when accepted as Notarized Developer ID."""
    r = subprocess.run(["spctl", "-a", "-t", "exec", "-vv", app], capture_output=True, text=True)
    out = (r.stderr + r.stdout).strip()
    return "" if r.returncode == 0 and "Notarized Developer ID" in out else out


def sign(root, identity=None, keychain=None, required=False, notarize=False, work=None, env=None):
    env = os.environ if env is None else env
    who = choose(identity, required, notarize, env)
    bad = layout_wrong(root)
    if bad:
        raise M.SignError("; ".join(bad))
    steps = M.sign_tree(root, who, keychain, default=None, outer=False, say=say)
    bad = check(root, steps, who)
    if bad:
        raise M.SignError("the gates: " + "; ".join(bad))
    if who != "-":
        dr = M.read_dr(os.path.join(root, "bise.app"))
        say("bise.app: %s" % dr)
    if notarize:
        try:
            N.submit(root, work or os.path.join(os.path.dirname(os.path.abspath(root)), "notary"), env)
            for rel, kind, _ in steps:
                if kind == "bundle" and os.sep not in rel and rel.endswith(".app"):
                    N.staple(os.path.join(root, rel))
        except N.NotaryError as e:
            raise M.SignError(str(e))
        g = gatekeeper(os.path.join(root, "bise.app"))
        if g:
            raise M.SignError("Gatekeeper refuses bise.app: %s" % g)
        say("notarized and stapled; Gatekeeper: Notarized Developer ID")
    return steps


def main(argv):
    ap = argparse.ArgumentParser(prog="dist_sign.py")
    ap.add_argument("root")
    ap.add_argument("--identity", default=os.environ.get("BISE_SIGN_ID") or None)
    ap.add_argument("--keychain", default=os.environ.get("BISE_SIGN_KEYCHAIN") or None)
    ap.add_argument("--required", action="store_true")
    ap.add_argument("--notarize", action="store_true")
    ap.add_argument("--work")
    a = ap.parse_args(argv)
    try:
        sign(a.root, a.identity, a.keychain, a.required, a.notarize, a.work)
        return 0
    except M.SignError as e:
        say(str(e))
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
