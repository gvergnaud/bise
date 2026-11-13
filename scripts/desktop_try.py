#!/usr/bin/env python3
"""desktop-try: the user's first try of bise desktop (the new app, apps/desktop), on a bise home of its own.

  bise-ambient --real                                the promoted bundle on YOUR bise home and hubs (no build, no
                                                     try world): stops any open try or instance first, opens it
                                                     with no variable, says the sha, the bundle and the home;
                                                     never BISE_DESKTOP_OFFSCREEN on your home (refused)
  bise-ambient --real --background                   the same behind your windows (open -g, window on screen):
                                                     keep-current's relaunch; already open: left as it is
  bise-ambient --real --stop                         quits it; on a stand-in home (a test's) also its hubs
  bise-ambient --try [repo…] [--fresh] [--replace]   build, set up the try home, open the app on it (internal
                                                     fallback until --real is proven: ambient-lead m_18348)
  bise-ambient --try --stop                          quit the try's app and stop its hubs (by pid)
  scripts/desktop-try.sh --candidate <sha>           a gate candidate in its own dir (<build>/desktop-cand/<sha12>),
                                                     never the try checkout, stamp or staged dir; prints cand.json
                                                     (app, root, bise) for the gate's off-screen launcher
                                                     (or BISE_TRY_CANDIDATE=<sha>). Its own bundle 'bise cand'
                                                     (dev.bise.desktop.cand), so its own userData: never his id
  scripts/desktop-try.sh --candidate <sha> --team-sign   then a copy of it signed with his Developer ID team
                                                     (<cand dir>/team/, team_signed: true), only when it carries
                                                     bise-kb.appex, for a Finder check; never notarized or
                                                     promoted (desktop_try_team.py)
  scripts/desktop-try.sh --promote <sha>             the gate passed: the candidate's build is repacked as 'bise
                                                     try' (dev.bise.desktop.try: electron-builder --dir only, no
                                                     rebuild), the try checkout moves to <sha>, and that bundle
                                                     + the root become the try's build

(~/.local/bin/bise-ambient execs scripts/desktop-try.sh, which runs this.)

The build (under the heavy lock, nice 10): this checkout's bise into his build dir's target (cargo -p
bend-harness), its repl-live, repl-scripted and sb-core (scripts/bins.sh), the native helper
(apps/desktop/native/mac/build.sh), then the new app: pnpm install --frozen-lockfile, pnpm build,
pnpm dist (electron-builder --mac --dir) as its own bundle 'bise try' (dev.bise.desktop.try:
apps/desktop/dist/mac-arm64/bise try.app) with that bise packed inside it, so the app's core and
every hub it starts run the try build. Signed with the stable 'bise ambient dev' identity
(<build>/ambient/sign, or BISE_TRY_SIGN_DIR), else ad hoc. A --candidate builds in the candidates'
one build checkout, <build>/desktop-cand/work (try_build.py, fast-try p1): moved to each sha with git,
so cargo, pnpm and vite rebuild only what changed. Its lock (desktop-cand/work.lock) serializes the
candidates; remade when the gate seed changes; rm -rf work is always safe under the lock.

The try home is ~/.bise/try/desktop (BISE_HOME): its own hubs, sockets, run dirs, journals,
projects.json and prefs, and its own home workspace (BISE_HOME_WORKSPACE=<try>/bise, never his
~/bise). HOME stays his (git config, his tools). Nothing is written under his ~/.bise outside
~/.bise/try/, nothing in his repos:

- his repos (his projects.json, read only, or the ones given) are plain local clones under
  <try>/repos (hardlinked objects, no alternates: a gc in his repo can't break them), their push
  URL set to fail, so nothing pushes into his real repo; re-run reuses them (a fetch from his repo),
  --fresh redoes everything;
- his model login is read, never copied: each API-key entry of his auth.json becomes a
  key_command in the try's config.toml that reads it at each call; a ChatGPT sign-in becomes a
  key_command that refreshes HIS grant under HIS lock (subs-auth m_9175/m_9180); his config.toml
  is copied without [pages] (its mirror would publish to his site);
- no connectors, the computer plugin disabled: his first try runs on his code only;
- the app is opened as a new instance of the try's own bundle (open -n --env BISE_HOME=<try>…), its
  pid kept in <try>/dev/desktop/app.pid; his own bise (app, hub, ~/bise) is never touched, and an
  open try is reopened only with --replace. --stop quits that pid (checked against the try app's
  executable first) and the try home's hubs.

Decisions: ambient-lead m_9146/m_9174/m_11012, architect m_9176 (the four leaks), subs-auth
m_9175/m_9180. The pure parts (env, guard, config, repos, names, build script, app env) are tested
in tests/desktop_try_test.py.
"""
import json
import os
import shlex
import shutil
import signal
import subprocess
import sys
import time

from desktop_try_env import (GATE, SELF_CONTAINED, TryError, clone_names, config_for_try, guard, his_env,  # one module,
                             his_bise, his_repos, holds_bundle, homes, keep_old_root, live_build, not_swapped,
                             self_contained, try_env, under)  # split for its size
import cand_lock  # cand.sh's lock on a candidate, held by --promote
from heavy_slot import HEAVY_RUN, heavy_argv, under_heavy_slot  # noqa: F401  (desktop_dmg, the tests read them here)
from try_procs import hub_socks, stop_hub, try_pids  # split for its size
from try_build import incremental, promote_install, root_script, work_build_paths, work_paths  # noqa: F401  (root_script: desktop_app's)

REPO = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
DESKTOP = os.path.join(REPO, "apps", "desktop")
# the try's own bundle (amb-mac bce89c8e, m_11023): its own userData (~/Library/Application
# Support/bise try) and TCC grants, never his app's; 'open -n -a <path>' opens that exact copy
APP_NAME = "bise try"
APP_ID = "dev.bise.desktop.try"
# a gate candidate's own bundle (ambient-lead m_20670, amb-tools' incident m_20662: a candidate
# with his id made LaunchServices quit his promoted app): its own id, name and so userData
# (~/Library/Application Support/bise cand); --promote repacks it with his (repack_script)
CAND_NAME = "bise cand"
CAND_ID = "dev.bise.desktop.cand"


def say(*a):
    print("bise try:", *a, file=sys.stderr, flush=True)


# ---------------------------------------------------------------- doing

def run(cmd, env, cwd=None, check=True):
    r = subprocess.run(cmd, env=env, cwd=cwd, capture_output=True, text=True)
    if check and r.returncode != 0:
        raise TryError("%s: %s" % (" ".join(cmd), (r.stderr or r.stdout).strip()[-600:]))
    return r


def pid_alive(pid):
    try:
        os.kill(int(pid), 0)
        return True
    except (OSError, ValueError, TypeError):
        return False


def read_pid(path):
    try:
        return int(open(path).read().strip())
    except (OSError, ValueError):
        return None


def build_dir(env):
    """His build dir, as scripts/bins.sh and desktop_app.py pick it: BISE_HOME/dev, else ~/.bise/dev;
    /build. Pure."""
    if env.get("SB_BUILD_DIR"):
        return env["SB_BUILD_DIR"]
    home = env.get("HOME") or os.path.expanduser("~")
    return os.path.join(env.get("BISE_HOME") or os.path.join(home, ".bise"), "dev", "build")


def try_paths(env, desktop=DESKTOP):
    """Where the try build goes: the bise (cargo -p bend-harness into <build>/target, the old
    try target), the native helper (one stable path: the EDR deletes fresh unsigned binaries) and
    the new app, packaged by `pnpm dist` in apps/desktop/dist (`desktop`: that checkout's apps/desktop,
    the try checkout's for --promote); candidates under <build>/desktop-cand."""
    build = build_dir(env)
    arch = "mac-arm64" if os.uname().machine == "arm64" else "mac"
    staged = os.path.join(build, "desktop-try", "staged")
    return {"target": os.path.join(build, "target"), "bise": os.path.join(build, "target", "debug", "bise"),
            "helper": os.path.join(build, "desktop-try", "bise-mac-helper"),
            "sign": os.path.join(build, "ambient", "sign"),
            "cand": os.path.join(build, "desktop-cand"), "arch": arch,
            "app": os.path.join(desktop, "dist", arch, APP_NAME + ".app"),
            # while his try runs from "app", a build is packed here instead (never over a running bundle)
            "staged_dir": staged, "staged": os.path.join(staged, arch, APP_NAME + ".app")}


def build_script(paths, sign_dir=None, out=None, root=None, repo=REPO, name=APP_NAME, app_id=APP_ID, inc=False):
    """The one build, run under the heavy lock: bise and its helpers for the try home (repl-live,
    repl-scripted, sb-core next to the checkout `repo`), the snapshot app root `root` (root_script), the
    native helper, then the new app (apps/desktop: pnpm install --frozen-lockfile, build, dist =
    electron-builder --mac --dir, the try's bise packed inside it), into apps/desktop/dist, or into
    `out` (BISE_PACK_OUT: the staging dir, a candidate's dir), as the bundle `name` with id `app_id`
    (his try's, or a candidate's CAND_NAME/CAND_ID). `inc` (a candidate's sha): built in the
    candidates' one build checkout, only what changed rebuilds (try_build.incremental). Pure."""
    q = shlex.quote
    desktop = os.path.join(repo, "apps", "desktop")
    lines = ["set -euo pipefail",
             "export CARGO_TARGET_DIR=%s CARGO_INCREMENTAL=0" % q(paths["target"]),
             "cd %s/rust && %s" % (q(repo), BISE_CARGO),
             "cd %s && scripts/bins.sh repl-live repl-scripted sb-core" % q(repo),
             "export MACOSX_DEPLOYMENT_TARGET=\"$(scripts/bins.sh macos-target)\"",
             "mkdir -p %s" % q(os.path.dirname(paths["helper"])),
             # ad hoc here; afterPack re-signs the whole app deep with the stable identity
             "apps/desktop/native/mac/build.sh %s" % q(paths["helper"]),
             "cd %s" % q(desktop),
             "pnpm install --frozen-lockfile",
             "pnpm build",
             # BISE_PACK_ROOT: the snapshot goes inside the bundle (Contents/Resources/bin), so the
             # app opens with no env (architect m_18354); its core installs it as a version dir
             "BISE_PACK_NAME=%s BISE_PACK_APP_ID=%s BISE_PACK_BISE=%s BISE_PACK_HELPER=%s%s%s%s pnpm dist"
             % (q(name), q(app_id), q(paths["bise"]), q(paths["helper"]), " BISE_PACK_ROOT=%s" % q(root) if root else "",
                " BISE_PACK_SIGN_DIR=%s" % q(sign_dir) if sign_dir else "", " BISE_PACK_OUT=%s" % q(out) if out else "")]
    if out:
        # a fresh staging dir: a half-written bundle from a failed build never gets swapped in
        lines.insert(lines.index("cd %s" % q(desktop)), "rm -rf %s" % q(out))
    if root:
        lines[lines.index("mkdir -p %s" % q(os.path.dirname(paths["helper"]))):0] = root_script(root)
    return "\n".join(incremental(lines, repo, paths, inc) if inc else lines) + "\n"


def root_path(paths, stamp):
    """Where the snapshot of that build goes: <build>/desktop-try/root-<head8>[-<changes8>]-<recipe6>,
    one per source and recipe, so a staged build never writes the root a running try reads."""
    name = "root-%s%s-%s" % (stamp["head"][:8], "-" + stamp["changes"][:8] if stamp["changes"] else "", stamp["recipe"][:6])
    return os.path.join(os.path.dirname(paths["helper"]), name)


def stamp_path(paths):
    """The try build's stamp, next to its native helper in his build dir."""
    return os.path.join(os.path.dirname(paths["helper"]), "try.stamp")


# one app build per sha (release-speed plan A step 5): the key, its mode env and the bise's cargo
# command, in their own module (nightly_desktop.sh calls it alone)
from build_key import BISE_CARGO, BUILD_MODE_ENV, PACK_ONLY_ENV, build_key, reusable  # noqa: E402,F401


def build_stamp(env, paths, sign, repo=REPO):
    """What the try build was made from: this checkout's HEAD, a hash of its uncommitted changes
    (git status --porcelain, every untracked file listed, and git diff HEAD, so a second edit of a
    changed file counts) and a hash of the build recipe itself (build_script with these paths and
    this signing). Reads git only, never takes the lock (ambient-lead m_11206)."""
    import hashlib
    head = run(["git", "-C", repo, "rev-parse", "HEAD"], env).stdout.strip()
    status = run(["git", "-C", repo, "status", "--porcelain", "--untracked-files=all"], env).stdout
    diff = subprocess.run(["git", "-C", repo, "diff", "HEAD", "--binary"], env=env, capture_output=True).stdout
    changes = hashlib.sha256(status.encode() + b"\0" + diff).hexdigest() if status.strip() else ""
    recipe = hashlib.sha256(build_script(paths, sign, repo=repo).encode()).hexdigest()
    return {"head": head, "changes": changes, "recipe": recipe}


def packed_bise(app):
    return os.path.join(app, "Contents", "Resources", "bin", "bise")


def missing_outputs(paths, app, root):
    """The outputs of a try build packed at `app` with its snapshot app root `root` that are not
    there, each by its path (the bundle, its bise inside, the try bise, the native helper, the
    root's repl-live); 'the snapshot root (a stamp from before it)' when there is no root. The
    refusals name them (keep-current m_30630: the try bise was gone and the swap's word blamed
    running processes)."""
    if not root:
        return ["the snapshot root (a stamp from before it: it builds once)"]
    return [p for p in (app, packed_bise(app), paths["bise"], paths["helper"], os.path.join(root, "repl-live"))
            if not os.path.exists(p)]


def outputs_there(paths, app, root):
    """Every output of a try build packed at `app` is there (missing_outputs is empty)."""
    return not missing_outputs(paths, app, root)


def same_source(stamp, saved):
    """The saved stamp was made from this source and recipe (wherever it was packed)."""
    return bool(saved) and {k: v for k, v in saved.items() if k in ("head", "changes", "recipe")} == stamp


def up_to_date(stamp, saved, paths):
    """The build can be skipped: the saved stamp is this one, packed live, and every output is
    there (the packed 'bise try.app' with its bise inside, the try bise, the native helper)."""
    return same_source(stamp, saved) and saved.get("where", "live") == "live" and outputs_there(paths, paths["app"], saved.get("root"))


def read_stamp(paths):
    try:
        return json.load(open(stamp_path(paths)))
    except (OSError, ValueError):
        return None


def write_stamp(paths, stamp):
    os.makedirs(os.path.dirname(stamp_path(paths)), exist_ok=True)
    with open(stamp_path(paths) + ".tmp", "w") as f:
        json.dump(stamp, f)
    os.replace(stamp_path(paths) + ".tmp", stamp_path(paths))


def bundle_procs(app):
    """Every process that holds that bundle (the app, its Electron helpers, its packed bise and
    the hubs it started; never the browsers' native host: holds_bundle), by the bundle's path, any
    try home: (pid, command), read only, never killed here."""
    out = subprocess.run(["ps", "-axww", "-o", "pid=,command="], capture_output=True, text=True).stdout
    procs = []
    for l in out.splitlines():
        pid, _, cmd = l.strip().partition(" ")
        if pid.isdigit() and holds_bundle(cmd, app) and int(pid) != os.getpid():
            procs.append((int(pid), cmd))
    return procs


def bundle_pids(app):
    return [p for p, _ in bundle_procs(app)]


def build(env, paths):
    """The try build under the heavy lock (/usr/bin/lockf -k ~/.bise/run/heavy.lock nice -n 10),
    only when the stamp says something changed: an up-to-date build opens at once, without queueing
    behind other heavy jobs. While a 'bise try' runs from the live bundle (ambient-lead m_11607),
    the build is packed into the staging dir instead and the stamp says where: 'staged';
    swap_staged puts it live once nothing runs from it. Returns True when it built."""
    # the stable 'bise ambient dev' identity (amb-mac m_11023): the try's grants survive rebuilds; else ad hoc
    sign = os.environ.get("BISE_TRY_SIGN_DIR") or (paths["sign"] if os.path.isdir(paths["sign"]) else None)
    stamp = build_stamp(env, paths, sign)
    saved = read_stamp(paths)
    short = stamp["head"][:8] + (" + uncommitted changes" if stamp["changes"] else "")
    if up_to_date(stamp, saved, paths):
        say("up to date (%s), opening" % short)
        return False
    if same_source(stamp, saved) and saved.get("where") == "staged" and outputs_there(paths, paths["staged"], saved.get("root")):
        say("up to date (%s), staged: your try is open: quit it to get the new build" % short)
        return False
    busy = bundle_pids(paths["app"])
    lock = os.path.expanduser("~/.bise/run/heavy.lock")
    os.makedirs(os.path.dirname(lock), exist_ok=True)
    if busy:
        say("your try is open (pid %d): building %s into staging, never over it (waits for other heavy jobs first)…" % (busy[0], short))
    else:
        say("building %s for the try (waits for other heavy jobs first)…" % short)
    # the root the live bundle runs on, kept until a swap retires it with the bundle
    live_root = (saved or {}).get("root") if (saved or {}).get("where", "live") == "live" else (saved or {}).get("live_root")
    root = root_path(paths, stamp)
    if busy and live_root and os.path.realpath(root) == os.path.realpath(live_root):
        # the same source rebuilt under a running try (its outputs went missing): never its root
        root = "%s-%d" % (root, int(time.time()))
    script = build_script(paths, sign, out=paths["staged_dir"] if busy else None, root=root)
    r = subprocess.run(heavy_argv(["/bin/bash", "-c", script], lock), env=env)
    if r.returncode != 0:
        raise TryError("the build failed (exit %d)" % r.returncode)
    app = paths["staged"] if busy else paths["app"]
    for k, p in (("bise", paths["bise"]), ("app", app), ("app root", os.path.join(root, "repl-live"))):
        if not os.path.exists(p):
            raise TryError("no %s at %s after the build" % (k, p))
    new = dict(stamp, where="staged" if busy else "live", root=root, **keep_old_root(saved))
    if busy and live_root and live_root != root:
        new["live_root"] = live_root
    write_stamp(paths, new)
    if busy:
        say("staged: your try is open: quit it to get the new build (the next bise-ambient --try swaps it in)")
    else:
        if os.path.isdir(paths["staged_dir"]):
            shutil.rmtree(paths["staged_dir"], ignore_errors=True)   # older than what is live now
        prune_roots(paths, keep={root})
    return True


def prune_roots(paths, keep):
    """Removes the try's snapshot roots other than `keep`, only when nothing runs from the live
    bundle (a running try's hubs read theirs)."""
    if bundle_pids(paths["app"]):
        return
    d = os.path.dirname(paths["helper"])
    for n in os.listdir(d) if os.path.isdir(d) else []:
        p = os.path.join(d, n)
        if n.startswith("root-") and p not in keep and os.path.isdir(p):
            shutil.rmtree(p, ignore_errors=True)


def try_root(paths):
    """The app root the try app is launched with: its build's snapshot (the stamp's root), never
    this checkout (ambient-lead m_11624). None when the build has none yet."""
    saved = read_stamp(paths) or {}
    root = saved.get("root") if saved.get("where", "live") == "live" else saved.get("live_root")
    return root if root and os.path.exists(os.path.join(root, "repl-live")) else None


def prev_bundle(paths):
    """Where the previous bundle waits after a swap: <build>/desktop-try/prev/<arch>/bise try.app,
    next to the staging dir. Never '<app>.old': the folder next to the bundle belongs to the app's
    own updater, which reads a '<app>.old' it didn't make as a failed update (ambient-lead m_18341,
    architect m_18352)."""
    staged_dir = paths["staged_dir"]
    return os.path.join(os.path.dirname(staged_dir), "prev", os.path.relpath(paths["staged"], staged_dir))


def retire_legacy_old(paths):
    """A launcher before prev/ left its previous bundle at '<app>.old': it goes to prev/ (or away
    when prev/ holds one already), so the app's updater never sees it. Returns the moved path."""
    legacy, prev = paths["app"] + ".old", prev_bundle(paths)
    if not os.path.isdir(legacy):
        return None
    if os.path.exists(prev):
        shutil.rmtree(legacy, ignore_errors=True)
        return None
    os.makedirs(os.path.dirname(prev), exist_ok=True)
    try:
        os.rename(legacy, prev)
    except OSError:
        shutil.rmtree(legacy, ignore_errors=True)
        return None
    return prev


def swap_staged(paths):
    """A staged build goes live when nothing runs from the live bundle: live -> prev/ (prev_bundle),
    staged -> live (renames), the stamp then says 'live'. Returns the prev/ path to remove once the
    new app's core answers (finish_swap), or None when there is nothing to swap or the try is open
    (then untouched: 'your try is open: quit it to get the new build')."""
    saved = read_stamp(paths)
    if not saved or saved.get("where") != "staged":
        return None
    if not outputs_there(paths, paths["staged"], saved.get("root")):
        gone = missing_outputs(paths, paths["staged"], saved.get("root"))
        say("the staged build is incomplete, nothing to swap to: missing %s" % (", ".join(gone) or "?"))
        return None
    if bundle_pids(paths["app"]):
        say("your try is open: quit it to get the new build")
        return None
    retire_legacy_old(paths)
    app, old = paths["app"], prev_bundle(paths)
    if os.path.exists(old):
        shutil.rmtree(old)
    os.makedirs(os.path.dirname(old), exist_ok=True)
    os.makedirs(os.path.dirname(app), exist_ok=True)
    if os.path.exists(app):
        os.rename(app, old)
    try:
        os.rename(paths["staged"], app)
    except OSError:
        # another volume: copy next to it, then one rename
        shutil.rmtree(app + ".new", ignore_errors=True)
        shutil.copytree(paths["staged"], app + ".new", symlinks=True)
        os.rename(app + ".new", app)
        shutil.rmtree(paths["staged_dir"], ignore_errors=True)
    # the staged build's snapshot root goes live with it; the old one is retired with prev/
    live = {k: v for k, v in saved.items() if k not in ("live_root", "old_root")}
    live["where"] = "live"
    if saved.get("live_root") and saved["live_root"] != saved.get("root"):
        live["old_root"] = saved["live_root"]
    # the one before goes now, with the prev/ bundle it served (finish_swap keeps both)
    if saved.get("old_root") and saved["old_root"] not in (saved.get("root"), saved.get("live_root")):
        shutil.rmtree(saved["old_root"], ignore_errors=True)
    write_stamp(paths, live)
    say("swapped in the staged build (%s)" % saved.get("head", "")[:8])
    return old if os.path.exists(old) else None


# ---------------------------------------------------------------- candidates (ambient-lead m_14320)
# A gate candidate is built for one sha in its own dir, <build>/desktop-cand/<sha12>: a detached
# worktree of that sha (src/), its packed app (dist/<arch>/bise cand.app, bise inside, its own id
# dev.bise.desktop.cand and so its own userData: LaunchServices never mixes it with his app,
# ambient-lead m_20670), its snapshot app root (root/) and cand.json. Building one never touches the
# try checkout (try-src), the try stamp or the staged dir, so his next --try still opens what the
# gate passed. qa-native's gate launcher opens cand.json's app off-screen with BISE_APP_ROOT=its
# root. Once the gate passes, --promote <sha> repacks that build as his 'bise try' (repack_script:
# electron-builder --dir on the candidate's own out/, bise, helper and root; no rebuild), moves the
# try checkout to that sha and the bundle and root into the try's staged place: they go live at
# once when no try is open, else at his next --try; under cand.sh's lock on it (cand_lock.py).


def try_checkout(env):
    """The checkout his bise-ambient --try builds from (~/.local/bin/bise-ambient's TRY_CHECKOUT)."""
    return env.get("BISE_TRY_CHECKOUT") or os.path.join(env.get("HOME") or os.path.expanduser("~"), ".bise", "dev", "try-src")


def resolve_sha(sha, env, repo=REPO):
    r = subprocess.run(["git", "-C", repo, "rev-parse", "--verify", "-q", "%s^{commit}" % sha], env=env, capture_output=True, text=True)
    if r.returncode != 0 or not r.stdout.strip():
        raise TryError("no commit %s in %s" % (sha, repo))
    return r.stdout.strip()


def cand_paths(paths, sha):
    d = os.path.join(paths["cand"], sha[:12])
    return {"dir": d, "src": os.path.join(d, "src"), "dist": os.path.join(d, "dist"), "root": os.path.join(d, "root-" + sha[:12]),
            "json": os.path.join(d, "cand.json"), "app": os.path.join(d, "dist", paths["arch"], CAND_NAME + ".app"),
            # --promote's repack with his try's name and id, before it moves into the try's staged place
            "promoted_dir": os.path.join(d, "promoted"), "promoted": os.path.join(d, "promoted", paths["arch"], APP_NAME + ".app")}


def bundle_id(app):
    """The bundle's CFBundleIdentifier (Contents/Info.plist), or None."""
    import plistlib
    try:
        with open(os.path.join(app, "Contents", "Info.plist"), "rb") as f:
            return plistlib.load(f).get("CFBundleIdentifier")
    except (OSError, ValueError, plistlib.InvalidFileException):
        return None


def cand_built(c):
    """The candidate's bundle (with its bise inside, and the candidate's own id: one built before
    m_20670 had his and builds again) and its root are there."""
    return (all(os.path.exists(p) for p in (c["app"], packed_bise(c["app"]), os.path.join(c["root"], "repl-live")))
            and bundle_id(c["app"]) == CAND_ID)


def pack_helper(app, hd):
    """A repack's native helper from a packed bundle `app`: electron-builder.mjs reads the tray's
    badge addon next to BISE_PACK_HELPER (native/mac/build.sh builds both in one dir), and a bundle
    has them apart (MacOS/, Resources/), so both are copied into `hd`; without it the config's load
    throws, which electron-builder says as 'Cannot access default before initialization'
    (cf6922d30 added the badge). Returns (its shell lines, the helper's path in `hd`). Pure."""
    q = shlex.quote
    return (["rm -rf %s" % q(hd), "mkdir -p %s" % q(hd),
             "cp -p %s %s %s/" % (q(os.path.join(app, "Contents", "MacOS", "bise-mac-helper")),
                                  q(os.path.join(app, "Contents", "Resources", "tray-badge.node")), q(hd))],
            os.path.join(hd, "bise-mac-helper"))


def repack_script(c, sign_dir=None):
    """--promote's repack (ambient-lead m_20670): the candidate's gated build packed again as his
    try's bundle (APP_NAME, APP_ID), from what the candidate built and nothing else: its worktree's
    apps/desktop out/ and node_modules (no install, no build), the bise, native helper and tray badge
    inside its bundle (pack_helper), its root; electron-builder --dir only, into c["promoted_dir"]. Pure."""
    q = shlex.quote
    app = c["app"]
    copy, helper = pack_helper(app, os.path.join(c["dir"], "pack-helper"))
    return "\n".join([
        "set -euo pipefail",
        "cd %s" % q(os.path.join(c["src"], "apps", "desktop")),
        promote_install(),
        "rm -rf %s" % q(c["promoted_dir"]),
        *copy,
        "BISE_PACK_NAME=%s BISE_PACK_APP_ID=%s BISE_PACK_BISE=%s BISE_PACK_HELPER=%s BISE_PACK_ROOT=%s%s BISE_PACK_OUT=%s pnpm dist"
        % (q(APP_NAME), q(APP_ID), q(packed_bise(app)), q(helper), q(c["root"]),
           " BISE_PACK_SIGN_DIR=%s" % q(sign_dir) if sign_dir else "", q(c["promoted_dir"]))]) + "\n"


def candidate(env, paths, sha, repo=REPO, sign=None):
    """Builds the packed app for `sha` into its own candidate dir (one heavy slot), unless already
    built; never the try checkout, the try stamp or the staged dir. Returns cand.json's dict."""
    sha = resolve_sha(sha, env, repo)
    c = cand_paths(paths, sha)
    if os.path.isdir(c["src"]):
        head = run(["git", "-C", c["src"], "rev-parse", "HEAD"], env).stdout.strip()
        if head != sha:
            raise TryError("%s holds %s, not %s: remove that candidate dir first" % (c["src"], head[:8], sha[:8]))
    else:
        os.makedirs(c["dir"], exist_ok=True)
        run(["git", "-C", repo, "worktree", "add", "-q", "--detach", c["src"], sha], env)
    info = {"sha": sha, "app": c["app"], "root": c["root"], "bise": packed_bise(c["app"]), "dir": c["dir"]}
    if cand_built(c) and os.path.exists(c["json"]):
        say("candidate %s is built: %s" % (sha[:8], c["app"]))
        return dict(json.load(open(c["json"])), **info)
    say("building candidate %s into %s (waits for a heavy slot)…" % (sha[:8], c["dir"]))
    # fast-try p1: built in the candidates' one build checkout, under its lock (try_build.py)
    script = build_script(work_build_paths(paths), sign, out=c["dist"], root=c["root"], repo=c["src"], name=CAND_NAME, app_id=CAND_ID, inc=sha)
    lock = os.path.expanduser("~/.bise/run/heavy.lock")
    # the work lock first, then the heavy slot: a candidate waiting for the other never holds a slot (but
    # when a caller wrapped us in heavy-run already)
    r = subprocess.run(["/usr/bin/lockf", "-k", work_paths(paths["cand"])["lock"]] + heavy_argv(["/bin/bash", "-c", script], lock), env=env)
    if r.returncode != 0:
        raise TryError("the candidate build failed (exit %d)" % r.returncode)
    if not cand_built(c):
        raise TryError("no packed app with id %s, or no root, in %s after the build (id: %s)" % (CAND_ID, c["dir"], bundle_id(c["app"])))
    info["built_ms"] = int(time.time() * 1000)
    # what it was built from (build_key): a release's land step and checks take this build when theirs
    # matches; and the cargo command of its packed bise (tree_bise.sh takes it only for its own)
    info["build_key"] = build_key(c["src"], sha, env)
    info["bise_cargo"] = BISE_CARGO
    # this bundle is the build's own signing (stable or ad hoc); a --team-sign copy is apart (desktop_try_team)
    info["team_signed"] = False
    with open(c["json"] + ".tmp", "w") as f:
        json.dump(info, f, indent=1)
    os.replace(c["json"] + ".tmp", c["json"])
    say("candidate %s built: %s (its runtime %s is inside it: open it with no env)" % (sha[:8], c["app"], c["root"]))
    return info


def try_module(checkout):
    """The try checkout's own desktop_try.py: its stamp is the one his next --try computes."""
    import importlib.util
    spec = importlib.util.spec_from_file_location("desktop_try_at_try_src", os.path.join(checkout, "scripts", "desktop_try.py"))
    m = importlib.util.module_from_spec(spec)
    # a module it imports that this run hasn't loaded comes from ITS scripts/ (try-all m_63887:
    # --promote from an fd7e3a42d checkout failed 'No module named build_key', added in a442912b7)
    sd = os.path.join(checkout, "scripts")
    sys.path.insert(0, sd)
    try:
        spec.loader.exec_module(m)
    finally:
        sys.path.remove(sd)
    return m


def promote(env, sha, checkout=None, repo=REPO):
    """The gate passed: the try checkout moves to `sha` and the candidate's bundle and root become
    the try's staged build, stamped as the try checkout's own script computes it, so his next --try
    reuses it (no rebuild); live at once when no try is open. Returns the new stamp."""
    sha = resolve_sha(sha, env, repo)
    checkout = checkout or try_checkout(env)
    probe = cand_paths(try_paths(env), sha)
    if not cand_built(probe) or not os.path.exists(probe["json"]):
        raise TryError("no built candidate for %s: scripts/desktop-try.sh --candidate %s first" % (sha[:8], sha[:8]))
    if not os.path.isdir(os.path.join(checkout, ".git")) and not os.path.isfile(os.path.join(checkout, ".git")):
        raise TryError("no try checkout at %s" % checkout)
    dirty = run(["git", "-C", checkout, "status", "--porcelain", "--untracked-files=no"], env).stdout.strip()
    if dirty:
        raise TryError("the try checkout %s has changes: never moved over them" % checkout)
    # his try's identity back on the gated build (m_20670): repacked before anything moves, so a
    # failed repack leaves the try checkout, stamp and staged dir as they were
    sign_dir = try_paths(env)["sign"]
    sign = os.environ.get("BISE_TRY_SIGN_DIR") or (sign_dir if os.path.isdir(sign_dir) else None)
    say("repacking candidate %s as %s (%s; waits for a heavy slot)…" % (sha[:8], APP_NAME, APP_ID))
    r = subprocess.run(heavy_argv(["/bin/bash", "-c", repack_script(probe, sign)], os.path.expanduser("~/.bise/run/heavy.lock")), env=env)
    if r.returncode != 0:
        raise TryError("the repack as %s failed (exit %d)" % (APP_ID, r.returncode))
    if bundle_id(probe["promoted"]) != APP_ID or not os.path.exists(packed_bise(probe["promoted"])):
        raise TryError("the repack in %s is not %s with its bise inside (id: %s)" % (probe["promoted_dir"], APP_ID, bundle_id(probe["promoted"])))
    # the moves hold cand.sh's lock on the candidate, taken once the heavy slot is let go (no deadlock with
    # a pass waiting in a slot); a pass runs: wait up to CAND_LOCK_WAIT s (7200), then refuse, nothing moved
    try:
        with cand_lock.held(env.get("HOME") or os.path.expanduser("~"), probe["app"], "desktop-try --promote %s" % sha[:12],
                            float(env.get("CAND_LOCK_WAIT") or 7200), say):
            return promote_locked(env, sha, checkout, repo, probe, sign)
    except cand_lock.Busy as e:
        raise TryError("promote %s refused, nothing moved: %s" % (sha[:8], e))


def promote_locked(env, sha, checkout, repo, probe, sign):  # promote's moves, under the candidate's lock
    if not cand_built(probe):
        raise TryError("the candidate %s went while promote waited for its lock" % sha[:8])
    run(["git", "-C", checkout, "checkout", "-q", "--detach", sha], env)
    m = try_module(checkout)
    tp = m.try_paths(env)
    stamp = m.build_stamp(env, tp, sign)
    if stamp["head"] != sha or stamp["changes"]:
        raise TryError("the try checkout is not clean at %s after the move" % sha[:8])
    saved = read_stamp(tp) or {}
    live_root = saved.get("root") if saved.get("where", "live") == "live" else saved.get("live_root")
    root = root_path(tp, stamp)
    if live_root and os.path.realpath(root) == os.path.realpath(live_root):
        root = "%s-%d" % (root, int(time.time()))
    for src, dst in ((probe["root"], root), (probe["promoted"], tp["staged"])):
        if os.path.exists(dst):
            shutil.rmtree(dst)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        try:
            os.rename(src, dst)
        except OSError:
            shutil.copytree(src, dst, symlinks=True)
    new = dict(stamp, where="staged", root=root, promoted=sha, **keep_old_root(saved))  # gated: desktop_try_swap
    if live_root and live_root != root:
        new["live_root"] = live_root
    write_stamp(tp, new)
    # the candidate is used up: its worktree and dir go, unless a process still runs from it
    if busy := cand_lock.users(probe["dir"]):
        say("kept %s: %d process(es) still run from it (pid %s); remove it once they end" % (probe["dir"], len(busy), busy[0][0]))
    else:
        subprocess.run(["git", "-C", repo, "worktree", "remove", "--force", probe["src"]], env=env, capture_output=True)
        shutil.rmtree(probe["dir"], ignore_errors=True)
    if m.swap_staged(tp):
        say("promoted %s: live now (the previous bundle waits in %s until the new core answers)" % (sha[:8], prev_bundle(tp)))
    else:
        say("promoted %s: staged; it goes live at his next bise-ambient --try" % sha[:8])
    return read_stamp(tp)


def core_up(app):
    """The new app's core runs: its packed bise has a process (the app starts it at launch)."""
    return any(True for _ in [p for p in bundle_pids(app)
                              if subprocess.run(["ps", "-o", "command=", "-p", str(p)], capture_output=True, text=True)
                              .stdout.startswith(packed_bise(app))])


def finish_swap(old, app, timeout=60, paths=None):
    """Waits for the swapped-in app's core. The previous bundle (prev/, prev_bundle) and its
    snapshot root (the stamp's old_root) stay either way, for a rollback, until the next swap
    replaces them (ambient-lead m_21844: a core that answers can still fail his first talk)."""
    t0 = time.time()
    while time.time() - t0 < timeout:
        if core_up(app):
            return True
        time.sleep(0.5)
    say("the new try's core didn't start within %d s: the previous build stays at %s" % (timeout, old))
    return False


def setup(real, tryh, repos, env, bise):
    """The try home: its config, his repos' clones, registered as its projects; the computer
    plugin off. Re-run: the same clones, fetched."""
    os.makedirs(os.path.join(tryh, "bise"), exist_ok=True)
    os.makedirs(os.path.join(tryh, "repos"), exist_ok=True)
    cfg = os.path.join(tryh, "config.toml")
    his_cfg = open(os.path.join(real, "config.toml")).read() if os.path.exists(os.path.join(real, "config.toml")) else ""
    try:
        his_auth = json.load(open(os.path.join(real, "auth.json")))
    except (OSError, ValueError):
        his_auth = {}
    if his_auth and not shutil.which("jq", path=env.get("PATH")):
        raise TryError("jq is needed to read your API key where it is (brew install jq)")
    guard(real, tryh, [cfg])
    with open(cfg + ".tmp", "w") as f:
        f.write(config_for_try(his_cfg, his_auth, real, bise))
    os.chmod(cfg + ".tmp", 0o600)
    os.replace(cfg + ".tmp", cfg)
    names = clone_names(repos)
    listed = []
    try:
        listed = [p.get("path") for p in json.load(open(os.path.join(tryh, "projects.json"))).get("projects", [])]
    except (OSError, ValueError):
        pass
    out = []
    for src, name in zip(repos, names):
        dest = os.path.join(tryh, "repos", name)
        guard(real, tryh, [dest])
        if os.path.isdir(os.path.join(dest, ".git")):
            run(["git", "-C", dest, "fetch", "-q", "origin"], env, check=False)
            state = "reused"
        else:
            # a plain local clone: hardlinked objects, no alternates (architect m_9176 (4))
            run(["git", "clone", "-q", src, dest], env)
            state = "cloned"
        # origin is his real repo: a push from the try must fail, never write into it
        run(["git", "-C", dest, "remote", "set-url", "--push", "origin", "DISABLED-in-try"], env)
        if os.path.realpath(dest) not in [os.path.realpath(p) for p in listed if p]:
            run([bise, "project", "add", dest], env)
        out.append({"repo": src, "clone": dest, "state": state})
    run([bise, "plugins", "disable", "computer"], env, check=False)
    return out


def app_env(tryh, env, root):
    """What the new app is opened with (open --env): the try home, its own home workspace, the
    try's PATH. Never an app root: the build's snapshot is inside the bundle (BISE_PACK_ROOT), and
    main never passes BISE_APP_ROOT to its core (an internal name). `root` is unused. Pure."""
    return ["BISE_HOME=%s" % tryh, "BISE_HOME_WORKSPACE=%s" % os.path.join(tryh, "bise"),
            "PATH=%s" % env.get("PATH", "/usr/bin:/bin")]


def app_pid_file(tryh):
    return os.path.join(tryh, "dev", "desktop", "app.pid")


def app_exe(app):
    return os.path.join(app, "Contents", "MacOS", os.path.basename(app)[:-len(".app")])


def find_app_pid(app, tryh):
    """The try's app among the running processes: its executable is the try app's and its own
    environment says BISE_HOME=<try> (ps -E; nothing else of it is read). Never by name."""
    out = subprocess.run(["ps", "-axwwE", "-o", "pid=,command="], capture_output=True, text=True).stdout
    exe = app_exe(app)
    for l in out.splitlines():
        pid, _, rest = l.strip().partition(" ")
        if pid.isdigit() and rest.startswith(exe) and (" BISE_HOME=%s " % tryh) in rest + " ":
            return int(pid)
    return None


def app_running(app, tryh):
    """The try app's pid when it runs: its pid file, checked against what that pid runs now."""
    pid = read_pid(app_pid_file(tryh))
    if pid and pid_alive(pid):
        cmd = subprocess.run(["ps", "-o", "command=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
        if cmd.startswith(app_exe(app)):
            return pid
    return None


def open_app(app, tryh, env, root):
    """The new app on the try home, as a normal app (his run, not a test): a new instance (-n) of the
    try's own bundle, so his own app, if any runs, is left alone. BISE_TRY_OPEN (tests, gate only):
    that command gets the app and its env instead; the app it starts is found and kept the same way,
    so --stop finds it (qa-native m_11066)."""
    os.makedirs(os.path.dirname(app_pid_file(tryh)), exist_ok=True)
    hook = os.environ.get("BISE_TRY_OPEN")
    if hook:
        run([hook, app] + app_env(tryh, env, root), env)
    else:
        cmd = ["open", "-n", "-a", app]
        for e in app_env(tryh, env, root):
            cmd += ["--env", e]
        run(cmd, env)
    return keep_app_pid(app, tryh)


def bundle_apps(app):
    """The app processes (the main executable, not its helpers or core) running from that bundle,
    on any home: the try's and the real one share it."""
    exe = app_exe(app)
    out = subprocess.run(["ps", "-axww", "-o", "pid=,command="], capture_output=True, text=True).stdout
    pids = []
    for l in out.splitlines():
        pid, _, cmd = l.strip().partition(" ")
        if pid.isdigit() and (cmd == exe or cmd.startswith(exe + " ")) and int(pid) != os.getpid():
            pids.append(int(pid))
    return pids


def quit_bundle(app, timeout=10):
    """Every instance of THIS bundle quits (one app holds fn and the menu bar): SIGTERM (Electron
    quits as on ⌘Q), then SIGKILL, to each pid whose executable is inside this bundle's path
    (bundle_apps), else nothing. Never by bundle id: a stand-in or copy of his try shares his id,
    and an Apple Event by id would quit his app too (ambient-lead m_22006); never by name."""
    pids = bundle_apps(app)
    for sig in (signal.SIGTERM, signal.SIGKILL):
        for pid in [p for p in pids if pid_alive(p)]:
            try:
                os.kill(pid, sig)
            except OSError:
                pass
        t0 = time.time()
        while any(pid_alive(p) for p in pids) and time.time() - t0 < timeout:
            time.sleep(0.2)
        if not any(pid_alive(p) for p in pids):
            break
    return pids


def real_app_env(environ, real, default_real):
    """What --real opens the app with: nothing, the app reads his own ~/.bise as on a normal start.
    A proof on a stand-in home (BISE_TRY_REAL=<home>/.bise, under ~/.bise/gate only: amb-tools'
    real-shaped world) adds HOME=<home> and BISE_HOME=<home>/.bise (BISE_HOME alone would let the
    projects scan walk his real folders: amb-tools m_18370), and BISE_DESKTOP_OFFSCREEN=1 when the
    caller set it (the off-screen run); never another variable. On his own home the knob never
    reaches the app: 'open' hands it the caller's environment, which his_env scrubs of every test
    variable (BISE_DESKTOP_* among them: real-onscreen, keep-current's relaunch with
    BISE_DESKTOP_OFFSCREEN=1 put his window on no display); 'behind his windows' is --background
    (open -g). Pure."""
    if os.path.realpath(real) == os.path.realpath(default_real):
        return []
    if not under(real, GATE) or os.path.basename(real) != ".bise":
        raise TryError("--real on a stand-in home: BISE_TRY_REAL=<home>/.bise under ~/.bise/gate only (got %s)" % real)
    out = ["HOME=%s" % os.path.dirname(real), "BISE_HOME=%s" % real]
    if environ.get("BISE_DESKTOP_OFFSCREEN") == "1":
        out.append("BISE_DESKTOP_OFFSCREEN=1")
    return out


def real_open_cmd(app, extra, environ, background=False):
    """The 'open' of --real (pure). -g (LaunchServices never brings the app to the front) for
    --background (keep-current's relaunch at idle: his window on screen, behind his own) and with
    BISE_DESKTOP_OFFSCREEN=1 (amb-tools' as-he-does runs on a stand-in: amb-tools m_18713, 'bise try'
    took his focus for ~4 s); his own --real opens as any app does."""
    g = background or environ.get("BISE_DESKTOP_OFFSCREEN") == "1"
    cmd = ["open"] + (["-g"] if g else []) + ["-n", "-a", app]
    for e in extra:
        cmd += ["--env", e]
    return cmd


def open_bundle(real, app, extra, environ, background=False):
    """--real's open of `app` on his home `real` (or a stand-in's), `extra` its variables
    (real_app_env): BISE_TRY_OPEN's hook in tests on a stand-in, else real_open_cmd. The app's
    failed swap reopens the bundle it left the same way (desktop_try_swap.reopen)."""
    hook = os.environ.get("BISE_TRY_OPEN")
    if hook:
        if not under(real, GATE):
            raise TryError("BISE_TRY_OPEN: for tests on a stand-in home under ~/.bise/gate only")
        run([hook, app] + extra, his_env(environ))
    else:
        run(real_open_cmd(app, extra, environ, background), his_env(environ))


def real_plan(running, staged, try_open, offscreen=False):
    """What --real does (pure): 'front' when the bundle already runs on his home with no staged build
    to swap in, no try open, and its window placed on a display: it is brought to the front, never
    quit (app-quits: his '~/Applications/bise (try)' click at 08:28 on 2026-11-13 quit and reopened
    the same dd93c388 for nothing); else 'restart' (quit, swap the staged build in, open). `offscreen`:
    a running instance carries BISE_DESKTOP_OFFSCREEN=1 (offscreen_apps): its window is on no display,
    so the front shows him nothing (real-onscreen: 12:50, pid 30078) and it restarts."""
    return "front" if running and not staged and not try_open and not offscreen else "restart"


def ps_env_offscreen(line):
    """One 'ps -E' line (pid, command, then its environment) carries BISE_DESKTOP_OFFSCREEN=1. Pure."""
    return (" BISE_DESKTOP_OFFSCREEN=1 " in " %s " % line.strip())


def offscreen_apps(pids):
    """The pids among `pids` (bundle_apps) whose own environment says BISE_DESKTOP_OFFSCREEN=1 (ps -E)."""
    if not pids:
        return []
    out = subprocess.run(["ps", "-wwE", "-o", "pid=,command=", "-p", ",".join(str(p) for p in pids)],
                         capture_output=True, text=True).stdout
    return [int(l.split()[0]) for l in out.splitlines() if l.split() and l.split()[0].isdigit() and ps_env_offscreen(l)]


def real_front_cmd(app):
    """The 'open' that brings the running bundle to the front (pure): no -n, so LaunchServices
    activates the instance of this path instead of starting a second one."""
    return ["open", "-a", app]


def open_real(real, tryh, paths, environ=None, timeout=20, background=False):
    """bise-ambient --real (ambient-lead m_18348): the promoted bundle on his real bise home, his
    running hubs; no build, no try world. Already open with nothing staged and on a display: brought
    to the front (real_plan), or with `background` (--background) left as it is. Else any open try is
    stopped and any instance of the bundle quits first; the bundle is opened with no variable
    (real_app_env), behind his windows with `background` (open -g). Says what it opened."""
    environ = os.environ if environ is None else environ
    app = paths["app"]
    extra = real_app_env(environ, real, his_bise())
    try_open = os.path.isdir(tryh) and (app_running(app, tryh) or any(os.path.exists(s) for s in hub_socks(tryh)))
    running = bundle_apps(app)
    staged = (read_stamp(paths) or {}).get("where") == "staged"
    if not extra and real_plan(running, staged, try_open, bool(offscreen_apps(running))) == "front":
        if not background:
            run(real_front_cmd(app), his_env(environ))
        say("bise desktop was already open on your home %s: %s (%s, pid %d): %s, not restarted"
            % (real, app, live_build(app).get("commit", "")[:8] or "unknown build", running[0],
               "left as it is" if background else "brought to the front"))
        return running[0]
    if try_open:
        r = stop(real, tryh, app)
        say("stopped the try (app %s, %d hubs)" % (r["app"] or "not running", len(r["hubs"])))
    quit_bundle(app)
    swap_staged(paths)
    saved, live = read_stamp(paths) or {}, live_build(app).get("commit", "")[:8]
    if saved.get("where") == "staged":
        say(not_swapped(saved.get("head", "")[:8], live, bundle_procs(app)))
    retire_legacy_old(paths)
    if not os.environ.get("BISE_TRY_OPEN") and not os.path.isdir(app):
        raise TryError("no promoted bundle at %s: scripts/desktop-try.sh --promote <sha> first" % app)
    missing = [] if os.environ.get("BISE_TRY_OPEN") else self_contained(app)
    if missing:
        raise TryError("the promoted bundle %s needs its runtime inside (Contents/Resources/bin lacks %s): "
                       "promote a candidate built with amb-core's self-contained pack" % (app, ", ".join(missing)))
    open_bundle(real, app, extra, environ, background)
    t0, pids = time.time(), []
    while not pids and time.time() - t0 < timeout:
        pids = bundle_apps(app)
        if not pids:
            time.sleep(0.3)
    if pids and os.path.exists(prev_bundle(paths)):
        # the previous bundle and its snapshot root, once the new core answers
        finish_swap(prev_bundle(paths), app, paths=paths)
    say("bise desktop is open on your home %s: %s (%s%s). the try is stopped; its home %s stays on disk"
        % (real, app, live_build(app).get("commit", "")[:8] or "unknown build", ", pid %d" % pids[0] if pids else ", not seen yet", tryh))
    return pids[0] if pids else None


def stop_real(real, app, default_real=None):
    """bise-ambient --real --stop: every instance of the bundle quits. On a stand-in home (under
    ~/.bise/gate, amb-tools' world) its hubs stop too, then every process left whose args name that
    home (repl-live outlived the app: ambient-lead m_18642), by pid; on his own home, never his hubs."""
    default_real = default_real or his_bise()
    out = {"app": quit_bundle(app), "hubs": [], "killed": []}
    if os.path.realpath(real) == os.path.realpath(default_real):
        return out
    if not under(real, GATE):
        raise TryError("--real --stop on a stand-in home %s: under ~/.bise/gate only" % real)
    # its processes name the stand-in HOME (the hubs' --workspace <home>/bise is beside .bise, not in
    # it: the law's run left an sbd), so match the home that holds this .bise when it is under the gate
    home = os.path.dirname(os.path.normpath(real)) if os.path.basename(os.path.normpath(real)) == ".bise" else real
    if not under(home, GATE):
        home = real
    for sock in hub_socks(real):
        if os.path.exists(sock) and stop_hub(sock):
            out["hubs"].append(sock)
    t0 = time.time()
    while try_pids(home) and time.time() - t0 < 15:
        time.sleep(0.3)
    for pid in try_pids(home):
        try:
            os.kill(pid, signal.SIGTERM)
            out["killed"].append(pid)
        except OSError:
            pass
    return out


def keep_app_pid(app, tryh, timeout=20):
    """Waits for the try's app to show up (find_app_pid) and keeps its pid in <try>/dev/desktop/app.pid,
    which app_running and --stop read. None when it never shows up."""
    t0, pid = time.time(), None
    while pid is None and time.time() - t0 < timeout:
        pid = find_app_pid(app, tryh)
        if pid is None:
            time.sleep(0.3)
    if pid:
        os.makedirs(os.path.dirname(app_pid_file(tryh)), exist_ok=True)
        with open(app_pid_file(tryh), "w") as f:
            f.write("%d\n" % pid)
    return pid


def quit_app(app, tryh, timeout=10):
    """The try's app quits (SIGTERM: Electron quits as on ⌘Q), by its checked pid. Returns it."""
    pid = app_running(app, tryh)
    if not pid:
        return None
    # SIGTERM to the checked pid (Electron's quit: will-quit stops its core and shells, the helper
    # quits), never an Apple Event by id (his try shares it: quit_bundle), never pkill by name
    os.kill(pid, signal.SIGTERM)
    t0 = time.time()
    while pid_alive(pid) and time.time() - t0 < timeout:
        time.sleep(0.2)
    if pid_alive(pid):
        os.kill(pid, signal.SIGKILL)
    return pid


def stop(real, tryh, app=None):
    """The try's app (its checked pid), then each hub (stop_hub on its socket, then SIGTERM by pid)."""
    guard(real, tryh)
    out = {"app": None, "hubs": [], "killed": []}
    if app:
        out["app"] = quit_app(app, tryh)
    for sock in hub_socks(tryh):
        if os.path.exists(sock) and stop_hub(sock):
            out["hubs"].append(sock)
    t0 = time.time()
    while try_pids(tryh) and time.time() - t0 < 15:
        time.sleep(0.3)
    for pid in try_pids(tryh):
        try:
            os.kill(pid, signal.SIGTERM)
            out["killed"].append(pid)
        except OSError:
            pass
    return out


def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    flags = {a for a in argv if a.startswith("--")}
    bad = flags - {"--try", "--real", "--stop", "--background", "--fresh", "--replace", "--no-build", "--no-open", "--candidate",
                   "--promote", "--team-sign"}
    if bad:
        raise TryError("unknown option %s (--real [--background], --try [repo…] [--fresh|--replace], --try --stop, --candidate <sha> [--team-sign], "
                       "--promote <sha>)" % ", ".join(sorted(bad)))
    cand = os.environ.get("BISE_TRY_CANDIDATE") if "--promote" not in flags else None
    if "--team-sign" in flags and ("--candidate" not in flags or flags - {"--candidate", "--team-sign"}):
        raise TryError("--team-sign goes with --candidate <sha> only")
    if "--candidate" in flags or cand:
        # a gate candidate: its own dir, never the try checkout, stamp or staged dir (ambient-lead m_14320)
        sha = args[0] if args else cand
        if not sha:
            raise TryError("--candidate <sha>")
        mine = his_env(os.environ)
        paths = try_paths(mine)
        sign = os.environ.get("BISE_TRY_SIGN_DIR") or (paths["sign"] if os.path.isdir(paths["sign"]) else None)
        info = candidate(mine, paths, sha, sign=sign)
        if "--team-sign" in flags:
            import desktop_try_team  # Gabriel's go for his team (architect m_63935): a signed copy, apart
            info = desktop_try_team.sign_candidate(info, cand_paths(paths, info["sha"]), paths["arch"], CAND_NAME, say)
        print(json.dumps(info))
        return 0
    if "--promote" in flags:
        if not args:
            raise TryError("--promote <sha>")
        print(json.dumps(promote(his_env(os.environ), args[0])))
        return 0
    if "--background" in flags and ("--real" not in flags or "--stop" in flags):
        raise TryError("--background goes with --real only (it opens the bundle behind your windows)")
    if "--real" in flags:
        if flags - {"--real", "--stop", "--background"} or args:
            raise TryError("--real [--background|--stop] takes nothing else: it opens (or stops) the promoted bundle on your home")
        real, tryh = homes()
        paths = try_paths(his_env(os.environ))
        if "--stop" in flags:
            r = stop_real(real, paths["app"])
            say("stopped bise desktop (%d instances, %d hubs, %d processes left killed)%s"
                % (len(r["app"]), len(r["hubs"]), len(r["killed"]), "" if r["hubs"] or r["killed"] else "; your hubs weren't touched"))
            return 0
        open_real(real, tryh, paths, background="--background" in flags)
        return 0
    real, tryh = homes()
    guard(real, tryh)
    mine = his_env(os.environ)
    paths = try_paths(mine)
    if "--stop" in flags:
        r = stop(real, tryh, paths["app"])
        say("stopped the try (app %s, %d hubs). your own bise wasn't touched" % (r["app"] or "not running", len(r["hubs"])))
        return 0
    env = try_env(os.environ, real, tryh)
    # tests only (a try home under ~/.bise/gate): BISE_TRY_BISE names the bise (no build), BISE_TRY_OPEN
    # a command that gets the app and its env instead of 'open' (never on his screen)
    hooks = [k for k in ("BISE_TRY_BISE", "BISE_TRY_OPEN") if os.environ.get(k)]
    if hooks and not under(tryh, GATE):
        raise TryError("%s: for tests on a try home under ~/.bise/gate only" % ", ".join(hooks))
    if os.environ.get("BISE_TRY_BISE"):
        if "--no-open" not in flags and not os.environ.get("BISE_TRY_OPEN"):
            raise TryError("BISE_TRY_BISE is for tests: with --no-open or BISE_TRY_OPEN only")
        paths = dict(paths, bise=os.environ["BISE_TRY_BISE"])
    # one try app at a time: an open one is reopened only with --replace; his own app is never touched
    running = app_running(paths["app"], tryh)
    if running and "--replace" not in flags and "--no-open" not in flags:
        raise TryError("your try is open (pid %d): quit it to get the new build. bise-ambient --try --replace reopens it, "
                       "--stop closes it" % running)
    # a build staged while the try was open goes live now if nothing runs from the live bundle
    # (never over a running one: swap_staged checks every process of the bundle, not just the pid file)
    swap_staged(paths)
    if "--no-build" not in flags:
        build(his_env(os.environ), paths)
    bise = paths["bise"]
    if not os.access(bise, os.X_OK):
        raise TryError("no bise at %s: build first" % bise)
    # the app root of everything the try starts: the build's snapshot, never this checkout
    # (BISE_TRY_BISE, a gate test with no build: this checkout, test only)
    root = try_root(paths) or (REPO if os.environ.get("BISE_TRY_BISE") else None)
    if not root:
        raise TryError("no snapshot app root for the try build: build first (without --no-build)")
    env["BISE_APP_ROOT"] = root
    if "--fresh" in flags and os.path.isdir(tryh):
        stop(real, tryh, paths["app"])
        guard(real, tryh, [tryh])
        shutil.rmtree(tryh)
    repos = [os.path.realpath(r) for r in args] or his_repos(open(os.path.join(real, "projects.json")).read()
                                                            if os.path.exists(os.path.join(real, "projects.json")) else "")
    for r in repos:
        if not os.path.isdir(os.path.join(r, ".git")) and not os.path.isfile(os.path.join(r, ".git")):
            raise TryError("%s is not a git repo" % r)
    done = setup(real, tryh, repos, env, bise)
    for d in done:
        say("%s %s -> %s" % (d["state"], d["repo"], d["clone"]))
    if "--no-open" in flags:
        say("set up (not opened): %s" % tryh)
        return 0
    if running:
        quit_app(paths["app"], tryh)
        if swap_staged(paths):      # --replace: the build staged while it ran, with its own root
            root = try_root(paths) or root
            env["BISE_APP_ROOT"] = root
    if not os.environ.get("BISE_TRY_OPEN") and not os.path.isdir(paths["app"]):
        raise TryError("no app at %s: build first (without --no-build)" % paths["app"])
    pid = open_app(paths["app"], tryh, env, root)
    retire_legacy_old(paths)
    if os.path.exists(prev_bundle(paths)):
        # the previous bundle and its snapshot root, once the new core answers
        finish_swap(prev_bundle(paths), paths["app"], paths=paths)
    say("bise desktop is open on the try home %s (%d projects%s). stop it: bise-ambient --try --stop"
        % (tryh, len(done), ", pid %d" % pid if pid else ""))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except TryError as e:
        say(str(e))
        sys.exit(1)
