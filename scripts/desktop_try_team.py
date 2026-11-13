#!/usr/bin/env python3
"""desktop-try --candidate <sha> --team-sign: a team-signed copy of a candidate that carries the kb appex.

Gabriel's go (architect m_63930/m_63935, his '1' to m_63585): a test build may be signed with his
Developer ID team (TEAM), so the Finder check of ~/bise (kb-fileprovider's #5) can run: kbenv.ts
gives the File Provider domain only to an app signed by a team whose own application-groups hold
the appex's group (d1b93dcc9: an ad hoc build skips the Finder). The rules:

- only a candidate whose bundle carries Contents/PlugIns/bise-kb.appex; any other is refused;
- the candidate's bundle id (dev.bise.desktop.cand) and its group (<TEAM>.dev.bise.desktop.cand.kb)
  are kept: never his app's;
- the identity is found in the keychain by its team id (security find-identity), never a key
  copied out of it; none there: the run fails, never an ad hoc fallback. The keychain may ask him
  once to let codesign use the key: the log says so;
- it is not notarized and never leaves this Mac;
- the signed bundle is a copy, <cand dir>/team/<arch>/bise cand.app, with its own cand.json
  (team_signed: true): --promote reads the candidate's own cand.json and bundle (team_signed:
  false), never this copy.

Signing is the release's (desktop_sign.sign: inside out, hardened, no --deep), with desktop.plist
plus the group (team_entitlements). The pure parts (identity_of, kb_group, team_entitlements,
team_paths, outer_codesign) and sign_candidate with a fake signer and a fake `security` are tested
in tests/desktop_try_test.py.
"""
import json
import os
import plistlib
import re
import shutil
import subprocess
import time

from desktop_try_env import TryError

TEAM = "9G5B2RTD6U"
APPEX = os.path.join("Contents", "PlugIns", "bise-kb.appex")
GROUPS = "com.apple.security.application-groups"
FIND = ["security", "find-identity", "-v", "-p", "codesigning"]


def identity_of(find_identity, team):
    """The SHA-1 of the first valid 'Developer ID Application: … (<team>)' identity in
    `security find-identity -v -p codesigning`'s output, or None. Pure."""
    for line in find_identity.splitlines():
        m = re.match(r'\s*\d+\)\s+([0-9A-F]{40})\s+"Developer ID Application: .*\((\w+)\)"\s*$', line)
        if m and m.group(2) == team:
            return m.group(1)
    return None


def kb_group(team, app_id):
    """The appex's group, as apps/desktop/native/kb/build.sh writes it: <team>.<app id>.kb. Pure."""
    return "%s.%s.kb" % (team, app_id)


def team_entitlements(base, group):
    """The outer app's entitlements: `base` (desktop.plist's) plus application-groups [group], so
    kbenv.ts finds the appex's group among the app's own. Pure."""
    return dict(base or {}, **{GROUPS: [group]})


def team_paths(c, arch, name):
    """Where the team-signed copy of candidate `c` (desktop_try.cand_paths) lives, apart from what
    --promote reads. Pure."""
    d = os.path.join(c["dir"], "team")
    return {"dir": d, "app": os.path.join(d, arch, name + ".app"), "json": os.path.join(d, "cand.json"),
            "entitlements": os.path.join(d, "entitlements.plist")}


def outer_codesign(app, identity, entitlements):
    """codesign's argv for the outer bundle with this identity (mac_sign's, the release's). Pure."""
    import mac_sign
    return mac_sign.codesign_argv(app, "bundle", identity, entitlements, rel=".")


def find_identity(run=subprocess.run):
    r = run(FIND, capture_output=True, text=True)
    return identity_of((r.stdout or "") + (r.stderr or ""), TEAM)


def default_sign(app, identity, entitlements, say):
    import desktop_sign
    try:
        desktop_sign.sign(app, identity, entitlements=entitlements, say=say)
    except desktop_sign.SignError as e:
        raise TryError("team signing failed: %s" % e)


def default_team(app):
    import mac_sign
    return mac_sign.read_team(app)


def sign_candidate(info, c, arch, name, app_id, say, run=subprocess.run, sign=default_sign, team_of=default_team, base=None):
    """Copies candidate `c`'s built bundle to team_paths and signs the copy with TEAM's Developer ID
    identity; writes and returns the copy's cand.json (info with its app and bise, team_signed:
    true). Raises TryError, signing nothing, when the bundle has no appex or the keychain has no
    identity of TEAM; and when the signed copy does not say TEAM."""
    if not os.path.isdir(os.path.join(c["app"], APPEX)):
        raise TryError("--team-sign is for a candidate that carries bise-kb.appex (a Finder check): %s has no %s"
                       % (c["app"], APPEX))
    identity = find_identity(run)
    if not identity:
        raise TryError("no 'Developer ID Application' identity of team %s in the keychain (%s): nothing signed, "
                       "no ad hoc fallback" % (TEAM, " ".join(FIND)))
    t = team_paths(c, arch, name)
    shutil.rmtree(t["dir"], ignore_errors=True)
    os.makedirs(os.path.dirname(t["app"]))
    shutil.copytree(c["app"], t["app"], symlinks=True)
    if base is None:
        import mac_sign
        with open(mac_sign.DESKTOP_ENT, "rb") as f:
            base = plistlib.load(f)
    group = kb_group(TEAM, app_id)
    with open(t["entitlements"], "wb") as f:
        plistlib.dump(team_entitlements(base, group), f)
    say("signing a copy of candidate %s with the Developer ID of team %s (%s…), not notarized: the keychain may ask "
        "you once to let codesign use that key" % (info["sha"][:8], TEAM, identity[:8]))
    sign(t["app"], identity, t["entitlements"], say)
    got = team_of(t["app"])
    if got != TEAM:
        raise TryError("the signed copy %s says team %s, not %s" % (t["app"], got, TEAM))
    out = dict(info, app=t["app"], bise=os.path.join(t["app"], os.path.relpath(info["bise"], c["app"])), team_signed=True, team=TEAM,
               group=group, identity=identity, signed_ms=int(time.time() * 1000))
    with open(t["json"] + ".tmp", "w") as f:
        json.dump(out, f, indent=1)
    os.replace(t["json"] + ".tmp", t["json"])
    say("team-signed candidate %s: %s (its group %s)" % (info["sha"][:8], t["app"], group))
    return out
