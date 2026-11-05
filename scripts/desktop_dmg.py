#!/usr/bin/env python3
"""desktop-dmg: a DMG of bise desktop to hand to someone (a colleague's try), from one commit.

  scripts/desktop-dmg.sh <sha> [--rev <n>]     build it; the .dmg path on stdout, its .sha256 next to it

No new pipeline (ambient-lead m_23935): the gate's candidate of <sha> (desktop_try.candidate: its own
worktree and dir, 'bise cand', one heavy slot), then a repack of what it built as the shipping bundle
'bise' (dev.bise.desktop) with the stable 'bise ambient dev' identity (his TCC grants stay across the
next DMG; ad hoc would lose them), electron-builder --dir only, then `hdiutil create` of a folder with
bise.app and an Applications link. arm64 only (x86_64 would need x86 builds of bise, repl-live,
sb-core and the V8 engine). No BISE_PACK_VERSION: a dev build, never offered an app update.

The runtime inside is a copy of the candidate's root with a share VERSION (architect m_23936), as
packaging/build-dist.sh writes one: no repo= (a dev path means nothing on his Mac), built= (the
commit's time, so a second DMG of the same commit makes the same root), target=, channel=share (free
text: no code reads it), and its own id desktop-<sha12> (never the gate's root-<sha12>). On his Mac
the core places it as <prefix>/versions/<id>, an install root of his prefix (current untouched), so
his older terminal bise never moves a hub down and his next `bise update` moves it up
(switch::should_follow_install). One id = one build (update.rs place()): a DMG whose root differs
from the one made before for that id needs --rev <n> (desktop-<sha12>-<n>), never the same id.

Never here: launching the bundle (dev.bise.desktop must not be registered on the build Mac: the
launch proof runs on the 'bise cand' twin), /Applications, an upload. Its folder:
<build>/desktop-cand/<sha12>/share. The pure parts are tested in tests/desktop_dmg_test.py.
"""
import datetime
import os
import shlex
import subprocess
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, HERE)
from desktop_try import (HEAVY_RUN, REPO, bundle_id, cand_paths, candidate, heavy_argv, his_env, pack_helper, packed_bise,  # noqa: E402
                         promote_install, resolve_sha, try_paths)
from desktop_try_env import TryError  # noqa: E402

NAME = "bise"
APP_ID = "dev.bise.desktop"
TARGET = "darwin-arm64"


def say(*a):
    print("bise dmg:", *a, file=sys.stderr, flush=True)


def share_id(sha, rev=None):
    """The runtime's VERSION id: desktop-<sha12>, -<rev> for a second build of that commit. Pure."""
    if rev is not None and (not str(rev).isdigit() or int(rev) < 2):
        raise TryError("--rev must be a number >= 2 (got %r)" % (rev,))
    return "desktop-%s%s" % (sha[:12], "" if rev is None else "-%s" % rev)


def share_paths(cand_dir, sha, rev=None, arch="mac-arm64"):
    """Where the share build goes, under the candidate's dir. Pure."""
    d = os.path.join(cand_dir, "share")
    rid = share_id(sha, rev)
    dmg = os.path.join(d, "bise-desktop-%s%s-arm64.dmg" % (sha[:8], "" if rev is None else "-%s" % rev))
    return {"dir": d, "id": rid, "root": os.path.join(d, rid), "dist": os.path.join(d, "dist"),
            "app": os.path.join(d, "dist", arch, NAME + ".app"), "stage": os.path.join(d, "dmg"),
            "dmg": dmg, "sha256": dmg + ".sha256", "mnt": os.path.join(d, "mnt")}


def share_version(text, rid, commit, subject, built):
    """The share runtime's VERSION from the candidate root's: no repo= and no old id, then its own
    id, commit, subject, built (UTC, a Z), target and channel. Pure."""
    keep = [l for l in text.splitlines() if l.strip() and l.split("=", 1)[0] not in ("id", "repo", "commit", "subject", "built", "target", "channel")]
    return "\n".join(["id=%s" % rid, "commit=%s" % commit, "subject=%s" % subject.replace("\n", " "), "built=%s" % built,
                      *keep, "target=%s" % TARGET, "channel=share"]) + "\n"


def utc_z(epoch):
    """A commit time as VERSION's built= (bise_home::release's one form). Pure."""
    return datetime.datetime.fromtimestamp(int(epoch), datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def root_script(src_root, s, version_text):
    """The share runtime: a clone of the candidate root (no rust/: an old path's second jsrt) with
    the share VERSION, built in <root>.tmp, then one rename. Pure."""
    q = shlex.quote
    t = s["root"] + ".tmp"
    return ["rm -rf %s" % q(t), "mkdir -p %s" % q(s["dir"]),
            "cp -Rc %s %s 2>/dev/null || cp -R %s %s" % (q(src_root), q(t), q(src_root), q(t)),
            "rm -rf %s/rust" % q(t),
            "printf '%%s' %s > %s/VERSION" % (q(version_text), q(t)),
            "mv %s %s" % (q(t), q(s["root"]))]


def repack_script(c, s, sign_dir):
    """The candidate's build packed again as 'bise' (dev.bise.desktop) with the share runtime, from
    what the candidate built only (its worktree's out/ and node_modules, the bise and helper in its
    bundle): electron-builder --dir, signed deep by afterPack. Pure."""
    q = shlex.quote
    app = c["app"]
    sign = " BISE_PACK_SIGN_DIR=%s" % q(sign_dir) if sign_dir else ""
    # the helper and its tray badge side by side, as electron-builder.mjs reads them (pack_helper;
    # desktop-tag-88207c55's dmg red)
    copy, helper = pack_helper(app, os.path.join(s["dir"], "pack-helper"))
    return "\n".join([
        "set -euo pipefail",
        "cd %s" % q(os.path.join(c["src"], "apps", "desktop")),
        promote_install(),
        "rm -rf %s" % q(s["dist"]),
        *copy,
        "BISE_PACK_NAME=%s BISE_PACK_APP_ID=%s BISE_PACK_BISE=%s BISE_PACK_HELPER=%s BISE_PACK_ROOT=%s%s BISE_PACK_OUT=%s pnpm dist"
        % (q(NAME), q(APP_ID), q(packed_bise(app)), q(helper), q(s["root"]), sign,
           q(s["dist"]))]) + "\n"


def dmg_script(s):
    """The DMG: bise.app (ditto keeps the framework links and the signature) and an Applications
    link, compressed read-only (UDZO), and its sha256. Pure."""
    q = shlex.quote
    return "\n".join([
        "set -euo pipefail",
        "rm -rf %s %s %s" % (q(s["stage"]), q(s["dmg"]), q(s["sha256"])),
        "mkdir -p %s" % q(s["stage"]),
        "ditto %s %s/%s.app" % (q(s["app"]), q(s["stage"]), NAME),
        "ln -s /Applications %s/Applications" % q(s["stage"]),
        "hdiutil create -quiet -volname %s -srcfolder %s -fs HFS+ -format UDZO -ov %s" % (q(NAME), q(s["stage"]), q(s["dmg"])),
        "(cd %s && shasum -a 256 %s > %s)" % (q(s["dir"]), q(os.path.basename(s["dmg"])), q(os.path.basename(s["sha256"]))),
        "rm -rf %s" % q(s["stage"])]) + "\n"


def read(path):
    try:
        return open(path).read()
    except OSError:
        return None


def check_app(app, rid):
    """The share bundle is what it says: dev.bise.desktop, its runtime's VERSION the share one (no
    repo=), its signature valid deep and strict. A list of what's wrong ([]: fine)."""
    bad = []
    if bundle_id(app) != APP_ID:
        bad.append("bundle id %s, not %s" % (bundle_id(app), APP_ID))
    v = read(os.path.join(app, "Contents", "Resources", "bin", "VERSION")) or ""
    if "id=%s\n" % rid not in v or "\nrepo=" in "\n" + v:
        bad.append("its runtime's VERSION is not the share one: %r" % v[:200])
    r = subprocess.run(["codesign", "--verify", "--deep", "--strict", app], capture_output=True, text=True)
    if r.returncode != 0:
        bad.append("codesign --verify --deep --strict: %s" % (r.stderr.strip() or r.returncode))
    return bad


def check_dmg(s):
    """The DMG mounted (read-only, not browsed, never opened): bise.app passes check_app and the
    Applications link is there; then detached."""
    os.makedirs(s["mnt"], exist_ok=True)
    r = subprocess.run(["hdiutil", "attach", "-nobrowse", "-noautoopen", "-readonly", "-mountpoint", s["mnt"], s["dmg"]],
                       capture_output=True, text=True)
    if r.returncode != 0:
        return ["hdiutil attach: %s" % r.stderr.strip()]
    try:
        bad = check_app(os.path.join(s["mnt"], NAME + ".app"), s["id"])
        if os.readlink(os.path.join(s["mnt"], "Applications")) != "/Applications":
            bad.append("no Applications link")
        return bad
    finally:
        subprocess.run(["hdiutil", "detach", "-quiet", s["mnt"]])


LOCK = os.path.expanduser("~/.bise/run/heavy.lock")


def heavy_dmg_argv(script, under=None, heavy_run=HEAVY_RUN, lock=LOCK):
    """The repack's argv: one heavy job at low priority (ambient-lead m_23935: heavy-run --low).
    Already in a heavy slot (the nightly's scripts/nightly_desktop.sh holds both) heavy_argv's
    rule: there, at nice 10, never waiting on a slot its own ancestor holds; no heavy-run on this
    Mac: the old single lock. Pure (given `under`)."""
    argv = heavy_argv(["/bin/bash", "-c", script], lock, under=under, heavy_run=heavy_run)
    return [heavy_run, "--low"] + argv[1:] if argv[:1] == [heavy_run] else argv


def heavy(script, env):
    return subprocess.run(heavy_dmg_argv(script), env=env).returncode


def repack_failure(rc, app):
    """Why the repack failed, from its exit code, in one line (desktop-tag-37dd2ac2: a bare 'failed'
    while its pnpm install ran on alone; a cand.sh pass on the candidate had read the repack's
    bash -c, which names the bundle's bise-mac-helper, as a helper breach and killed it). A signal
    (subprocess's -n, or a shell's 128+n through nice or heavy-run) came from outside. Pure."""
    import signal
    n = -rc if rc < 0 else rc - 128 if 128 < rc < 160 else None
    if n is not None:
        try:
            name = signal.Signals(n).name
        except ValueError:
            name = "signal %d" % n
        return ("killed by %s (exit %d): something outside stopped it, the repack never signals itself; "
                "look for a process that kills by a path in %s (a pass on that candidate) or heavy-run's stop line" % (name, rc, app))
    if rc == 28:
        return "exit 28: heavy-run stopped it, the disk was nearly full"
    if rc == 124:
        return "exit 124: heavy-run stopped it, over its time limit"
    return "exit %d: the pnpm install or electron-builder lines above say why" % rc


def build(env, sha, rev=None, repo=REPO):
    sha = resolve_sha(sha, env, repo)
    paths = try_paths(env)
    sign = os.environ.get("BISE_TRY_SIGN_DIR") or (paths["sign"] if os.path.isdir(paths["sign"]) else None)
    if not sign:
        raise TryError("no stable signing identity in %s: a share build is never ad hoc (his grants would not survive)" % paths["sign"])
    info = candidate(env, paths, sha, repo=repo, sign=sign)
    c = cand_paths(paths, sha)
    s = share_paths(c["dir"], sha, rev, paths["arch"])
    git = lambda *a: subprocess.run(["git", "-C", repo, *a], capture_output=True, text=True, check=True).stdout.strip()  # noqa: E731
    text = share_version(read(os.path.join(info["root"], "VERSION")) or "", s["id"], sha, git("log", "-1", "--format=%s", sha),
                         utc_z(git("log", "-1", "--format=%ct", sha)))
    old = read(os.path.join(s["root"], "VERSION"))
    if old is None:
        r = subprocess.run(["/bin/bash", "-c", "set -euo pipefail\n" + "\n".join(root_script(info["root"], s, text))])
        if r.returncode != 0:
            raise TryError("the share runtime %s failed" % s["root"])
    elif old != text:
        raise TryError("%s holds another VERSION than this build's: one id = one build, use --rev <n>" % s["root"])
    say("repacking candidate %s as %s (%s, waits for a heavy slot)…" % (sha[:8], NAME, APP_ID))
    rc = heavy(repack_script(c, s, sign), env)
    if rc != 0:
        raise TryError("the repack as %s failed, %s" % (APP_ID, repack_failure(rc, c["app"])))
    bad = check_app(s["app"], s["id"])
    if bad:
        raise TryError("the packed app: " + "; ".join(bad))
    if subprocess.run(["/bin/bash", "-c", dmg_script(s)]).returncode != 0:
        raise TryError("hdiutil create failed")
    bad = check_dmg(s)
    if bad:
        raise TryError("the DMG: " + "; ".join(bad))
    say("built %s (runtime %s; its candidate twin for the launch proof: %s)" % (s["dmg"], s["id"], info["app"]))
    return s["dmg"]


def main(argv):
    args = list(argv)
    rev = None
    if "--rev" in args:
        i = args.index("--rev")
        rev = args[i + 1] if i + 1 < len(args) else ""
        del args[i:i + 2]
    try:
        if len(args) != 1 or args[0].startswith("-"):
            raise TryError("usage: desktop-dmg.sh <sha> [--rev <n>]")
        print(build(his_env(os.environ), args[0], rev))
        return 0
    except TryError as e:
        say(str(e))
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
