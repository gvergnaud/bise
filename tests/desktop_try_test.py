#!/usr/bin/env python3
"""scripts/desktop_try.py ('bise-ambient --try'), on fake data only.

The pure parts: the try's environment (no bise override or internal variable of the caller
survives, BISE_HOME and BISE_HOME_WORKSPACE are the try's: architect m_9176 (1)-(2)), the guard
(everything under ~/.bise/try or a test's ~/.bise/gate), his config without [pages] and with a
key_command per auth entry (never the key itself), his projects.json read, the clone names.

Then one run of the script on a throwaway HOME-like tree under ~/.bise/gate (a fake 'real' bise
home with config.toml, auth.json and projects.json naming two fake repos), with the tree's bise
and --no-build --no-open: the clones (no alternates, push URL disabled, his repos untouched),
the try's projects.json, its config.toml (0600, no key), a re-run that reuses the clones,
--fresh, and --stop stopping a hub started on the try home (by its socket, then its pid).
Never his ~/.bise, never an app (an app on the try home is the off-screen harness's job).

  python3 -B tests/desktop_try_test.py        (the integration part needs rust/target/debug/bise)
"""
import json
import os
import plistlib
import shlex
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import desktop_try as T  # noqa: E402
import desktop_try_env as E  # noqa: E402  (its pure where and what, split out for size)

sys.path.insert(0, HERE)
import wait  # noqa: E402

FAILS = []


def check(ok, what):
    if not ok:
        FAILS.append(what)
        print("FAIL", what)


def pure():
    # one heavy slot (main m_13151, ambient-lead m_13161): heavy-run when there, the old lock
    # without it, nice alone when already in a slot (never both slots, never nested)
    cmd, lock = ["/bin/bash", "-c", "x"], "/l/heavy.lock"
    hr = os.path.join(tempfile.mkdtemp(prefix="dt-hr-"), "heavy-run")
    with open(hr, "w") as f:
        f.write("#!/bin/sh\n")
    os.chmod(hr, 0o755)
    check(T.heavy_argv(cmd, lock, under=False, heavy_run=hr) == [hr] + cmd, "through heavy-run")
    check(T.heavy_argv(cmd, lock, under=False, heavy_run=hr, prio=True) == [hr, "--prio"] + cmd, "a release: --prio, the release lane")
    check(T.heavy_argv(cmd, lock, under=False, heavy_run=hr + "-none") == ["/usr/bin/lockf", "-k", lock, "nice", "-n", "10"] + cmd, "the old lock")
    check(T.heavy_argv(cmd, lock, under=True, heavy_run=hr) == ["nice", "-n", "10"] + cmd, "in a slot already: no second one")
    check(T.under_heavy_slot(env={}) is False, "this test runs in no slot")
    # heavy-run's 4 slots and its HEAVY_SLOT job (gate-reds, review-gates m_33477): a job in slot 3
    # or 4 runs in place (heavy-run refuses a nested call); the python and the node detectors agree
    import heavy_slot as HS
    check(HS.under_heavy_slot(env={"HEAVY_SLOT": "heavy3.lock %d" % os.getpid()}), "HEAVY_SLOT's job is this process: in a slot")
    check(not HS.under_heavy_slot(env={"HEAVY_SLOT": "heavy3.lock 1"}) and not HS.under_heavy_slot(env={"HEAVY_SLOT": "x"}),
          "a HEAVY_SLOT of no ancestor (a daemon that outlived its job), or no pid: not in a slot")
    run3 = os.path.join(tempfile.mkdtemp(prefix="dt-slot-"), ".bise", "run")
    os.makedirs(run3)
    heavy_ts = os.path.join(ROOT, "apps", "desktop", "tests", "harness", "heavy.mts")
    probe = {"py": [sys.executable, "-c", "import sys; sys.path.insert(0, %r); import heavy_slot; print(heavy_slot.under_heavy_slot())"
                    % os.path.join(ROOT, "scripts")],
             "node": ["node", "--experimental-strip-types", "--disable-warning=ExperimentalWarning", "--input-type=module", "-e",
                      "import(%r).then((m) => console.log(m.underHeavySlot() ? 'True' : 'False'))" % heavy_ts]}
    env0 = {k: v for k, v in os.environ.items() if k != "HEAVY_SLOT"}
    for n in ("heavy.lock", "heavy3.lock", "heavy4.lock"):
        for lang, argv in probe.items():
            r = subprocess.run(["/usr/bin/lockf", "-k", os.path.join(run3, n)] + argv, capture_output=True, text=True, env=env0)
            check(r.stdout.strip() == "True", "%s: under a lockf of %s: in a slot (%r %r)" % (lang, n, r.stdout, r.stderr[-200:]))
    for lang, argv in probe.items():
        r = subprocess.run(argv, capture_output=True, text=True, env=env0)
        check(r.stdout.strip() == "False", "%s: no lockf, no HEAVY_SLOT: no slot (%r)" % (lang, r.stdout))
        r = subprocess.run(["/bin/bash", "-c", 'export HEAVY_SLOT="heavy4.lock $$"; "$@"', "_"] + argv, capture_output=True, text=True, env=env0)
        check(r.stdout.strip() == "True", "%s: HEAVY_SLOT's job is an ancestor: in a slot (%r)" % (lang, r.stdout))
    real = os.path.expanduser("~/.bise")
    tryh = os.path.join(real, "try", "desktop")
    base = {"HOME": "/Users/x", "PATH": "/usr/bin", "BEND_RUN_DIR": real + "/run", "SB_SOCKET": "/s", "BISE_ROLE": "task",
            "BISE_EXPORTS_FOR": "x", "BISE_HOME": real, "BISE_HOME_WORKSPACE": "/Users/x/bise", "SB_BUILD_DIR": "/b", "ANTHROPIC_FOUNDRY_API_KEY": "k"}
    e = T.try_env(base, real, tryh)
    check(e["BISE_HOME"] == tryh and e["BISE_HOME_WORKSPACE"] == tryh + "/bise", "the try's homes: %s" % e)
    check(not [k for k in ("BEND_RUN_DIR", "SB_SOCKET", "BISE_ROLE", "BISE_EXPORTS_FOR", "SB_BUILD_DIR") if k in e], "no override or internal var survives: %s" % e)
    check(e["HOME"] == "/Users/x" and e["ANTHROPIC_FOUNDRY_API_KEY"] == "k", "HOME and his provider env stay")
    e2 = T.try_env(dict(base, PATH="%s/hubs/h-1/bin:/usr/bin:%s/run/x:/bin" % (real, real)), real, tryh)
    check(e2["PATH"] == "/usr/bin:/bin", "an agent shell's hub bin (his sb) leaves the try's PATH: %s" % e2["PATH"])
    other = "/Users/x/.bise"
    check(E.clean_path("%s/hubs/h/bin:/usr/bin:%s/run/r:%s/try/bin" % (other, other, other), real, homes=(other,)) == "/usr/bin:%s/try/bin" % other,
          "the default ~/.bise's hubs/ and run/ go too when the real home is a stand-in")
    h = T.his_env(base)
    check("BISE_HOME" not in h and "SB_SOCKET" not in h and h["HOME"] == "/Users/x", "his env: no BISE_HOME, no hub var")

    T.guard(real, tryh, [tryh + "/repos/a"])
    for bad_home in (real, os.path.join(real, "hubs"), "/tmp/try"):
        try:
            T.guard(real, bad_home)
            check(False, "guard refuses the try home %s" % bad_home)
        except T.TryError:
            pass
    try:
        T.guard(real, tryh, [real + "/config.toml"])
        check(False, "guard refuses a write into his ~/.bise")
    except T.TryError:
        pass

    cfg = ('model = "opus-5.5"\napprovals = "yolo"\n\n[providers.foundry]\nbase_url = "https://p/v1"\n\n'
           '[providers.openrouter]\nkey_command = "old"\nname = "x"\n\n[pages]\nmirror = "~/.bise/pages-mirror/deploy.sh"\n\n[voice]\nsounds = true\n')
    out = T.config_for_try(cfg, {"openrouter": {"type": "api", "key": "sk-or-SECRET"}, "chatgpt": {"type": "oauth", "refresh": "rt-SECRET"},
                                 "weird": {"type": "oauth"}}, real, "/b/bise")
    check("SECRET" not in out, "no key or token in the try's config")
    check("[pages]" not in out and "mirror" not in out, "no [pages]: its mirror would publish to his site")
    check('model = "opus-5.5"' in out and "[voice]" in out and 'base_url = "https://p/v1"' in out, "the rest of his config stays")
    check(out.count("[providers.openrouter]") == 1 and "old" not in out
          and "key_command = \"jq -r .openrouter.key '%s/auth.json'\"" % real in out, "openrouter: one table, a jq key_command:\n" + out)
    check("[providers.chatgpt]\nkey_command = \"BISE_HOME='%s' '/b/bise' auth token chatgpt\"" % real in out, "chatgpt: his grant through his home")
    check("[providers.weird]" not in out, "an unknown sign-in gets nothing")
    import tomllib
    tomllib.loads(out)

    check(T.his_repos(json.dumps({"v": 1, "projects": [{"path": "/b", "order": 1}, {"path": "/a", "order": 0}, {"name": "x"}]})) == ["/a", "/b"], "projects.json by order")
    check(T.his_repos("not json") == [], "a bad projects.json: none")
    check(T.clone_names(["/x/app", "/y/app", "/z/my repo"]) == ["app", "app-2", "my-repo"], "clone names")
    # the NEW app (lead m_11012): its own bundle, built and packed with the try's bise, opened on the try home
    p = T.try_paths({"HOME": "/h", "BISE_HOME": "/h/.bise"})
    check(p["bise"] == "/h/.bise/dev/build/target/debug/bise" and p["app"].endswith("/apps/desktop/dist/mac-arm64/bise try.app")
          and p["helper"] == "/h/.bise/dev/build/desktop-try/bise-mac-helper", "try paths: %s" % p)
    check(T.app_exe(p["app"]).endswith("bise try.app/Contents/MacOS/bise try"), "the app's executable")
    # what holds the bundle against a swap: the app, its helpers, its core and hubs; never the
    # browsers' native host, which Chrome starts again within 2 s (chrome-host-swap)
    app, bin_ = "/h/dist/bise try.app", "/h/dist/bise try.app/Contents/Resources/bin/bise"
    held = {c: T.holds_bundle(c, app) for c in (
        app + "/Contents/MacOS/bise try", app + "/Contents/Frameworks/bise try Helper.app/Contents/MacOS/bise try Helper --type=gpu",
        bin_ + " core --desktop", bin_ + " daemon --repo /r", bin_ + " computer-use chrome-host chrome-extension://bogffepmbkbmbfejcadaipgphgkocgob/",
        bin_ + " computer-use chrome-host", "/h/other.app/Contents/Resources/bin/bise computer-use chrome-host chrome-extension://x/",
        "/h/.local/share/bise/versions/root-x/bise computer-use broker", "/h/dist/bise try.app.old/Contents/MacOS/bise try",
        # the broker from the bundle, chrome-host's child, respawned within 1 s (keep-current m_61383): never holds it
        bin_ + " computer-use broker", bin_ + " computer-use broker --idle-exit 600", bin_ + " computer-use brokerx")}
    check([c for c, h in held.items() if h] == list(held)[:4] + [bin_ + " computer-use chrome-host", bin_ + " computer-use brokerx"],
          "only the app's own processes hold it; the browsers' host and the broker (their argv) never: %s" % held)
    sc = T.build_script(p, "/h/sign")
    order = [sc.index(x) for x in ("cargo build -p bend-harness", "bins.sh repl-live repl-scripted sb-core", "native/mac/build.sh",
                                   "pnpm install --frozen-lockfile", "pnpm build", "pnpm dist")]
    check(order == sorted(order), "build: bise, helpers, native helper, then install, build, dist: %s" % sc)
    check("BISE_PACK_NAME='bise try' BISE_PACK_APP_ID=dev.bise.desktop.try BISE_PACK_BISE=/h/.bise/dev/build/target/debug/bise" in sc
          and "BISE_PACK_SIGN_DIR=/h/sign pnpm dist" in sc and "ambient.sh" not in sc and "apps/ambient" not in sc,
          "dist packs the try's bise into its own bundle, signed stable; no old app: %s" % sc)
    check("BISE_PACK_SIGN_DIR" not in T.build_script(p, None), "no sign dir: ad hoc")
    ae = T.app_env("/h/.bise/try/desktop", {"PATH": "/usr/bin"}, "/h/build/desktop-try/root-abc")
    check(not any(e.startswith("BISE_APP_ROOT=") for e in ae),
          "never an app root in the app's env: the snapshot is inside the bundle (architect m_18354): %s" % ae)
    sr = T.build_script(p, "/h/sign", root="/h/build/desktop-try/root-abc")
    check("BISE_PACK_ROOT=/h/build/desktop-try/root-abc " in sr and "BISE_PACK_ROOT" not in sc,
          "the snapshot is packed into the bundle when there is one: %s" % sr)
    check(ae[:2] == ["BISE_HOME=/h/.bise/try/desktop", "BISE_HOME_WORKSPACE=/h/.bise/try/desktop/bise"] and "PATH=/usr/bin" in ae
          and not any(e.split("=")[0] in ("BISE_DESKTOP_TEST", "BISE_TEST_HOME", "BISE_DESKTOP_USER_DATA", "BISE_DESKTOP_HELPER", "BISE_DESKTOP_BISE")
                      for e in ae), "the app's env: the try home, never a test variable: %s" % ae)


def root_ships_what_a_version_dir_has():
    """The app's runtime snapshot carries what scripts/versions.sh puts in a version dir for the
    agents: the built-in plugins and the office SDK's bundles (without office/dist the app's
    agents loaded the office skills and `bise office run` had no office.js). Pure."""
    s = "\n".join(T.root_script("/r/root-abc"))
    for what in ("cp -R plugins /r/root-abc.tmp/plugins", "cp -R office/dist /r/root-abc.tmp/office/dist",
                 "cp -R kit /r/root-abc.tmp/kit", "cp -R prompts /r/root-abc.tmp/prompts"):
        check(what in s, "root_script: %s" % what)
    # the xlsx grid (never committed): from bins.sh into Sheets' view folder, as versions.sh does
    # (xlsx-fix: the app's roots 29c2d5aa6 and 824c1ca54 had no office-sheet.js, the viewer's
    # import of it a 404), and a failed build stops the root
    check("scripts/bins.sh path --src \"$PWD\" office-sheet)\" || { " in s
          and "cp \"$g\" /r/root-abc.tmp/plugins/sheets/ai.mistral.vibe/views/grid/office-sheet.js" in s, "root_script: the xlsx grid: %s" % s)
    check("scripts/runtime_check.sh /r/root-abc.tmp\nrm -rf /r/root-abc && mv" in s, "root_script: checked whole before the rename: %s" % s)


def root_of(script):
    """The snapshot app root a build script makes (root_script's last line: rm -rf R && mv R.tmp R)."""
    for line in script.splitlines():
        if line.startswith("rm -rf ") and " && mv " in line:
            return shlex.split(line)[-1]
    return None


def stamp_skips_build():
    """An up-to-date try build opens with no build call (ambient-lead m_11206): the stamp (HEAD,
    the uncommitted changes, the recipe) matches and every output exists -> no lockf, no cargo, no
    pnpm, 'up to date (<sha>), opening'. A missing stamp, another recipe or a missing output builds
    (under the lock) and writes the stamp. The build itself is faked: never a real cargo or pnpm."""
    w = tempfile.mkdtemp(prefix="dtry-stamp-")
    build = os.path.join(w, "build")
    paths = {"target": os.path.join(build, "target"), "bise": os.path.join(build, "target", "debug", "bise"),
             "helper": os.path.join(build, "desktop-try", "bise-mac-helper"), "sign": os.path.join(build, "ambient", "sign"),
             "app": os.path.join(w, "dist", "bise try.app"),
             "staged_dir": os.path.join(build, "desktop-try", "staged"),
             "staged": os.path.join(build, "desktop-try", "staged", "mac-arm64", "bise try.app")}
    calls, said = [], []
    real_run, real_say, old_sign = T.subprocess.run, T.say, os.environ.pop("BISE_TRY_SIGN_DIR", None)

    def outputs(script):
        for p in (paths["bise"], paths["helper"], T.packed_bise(paths["app"]), os.path.join(root_of(script), "repl-live")):
            os.makedirs(os.path.dirname(p), exist_ok=True)
            open(p, "w").close()

    def fake_run(cmd, *a, **k):
        # the build, whatever runs it (heavy-run, the old lockf, or nice alone in a slot): never for real
        if cmd and cmd[-3:-1] == ["/bin/bash", "-c"]:
            calls.append(cmd)
            outputs(cmd[-1])
            return subprocess.CompletedProcess(cmd, 0)
        return real_run(cmd, *a, **k)
    T.subprocess.run, T.say = fake_run, (lambda *a: said.append(" ".join(str(x) for x in a)))
    try:
        env = dict(os.environ)
        check(T.build(env, paths) is True and len(calls) == 1 and os.path.exists(T.stamp_path(paths)),
              "no stamp: builds under the lock and writes the stamp: %s" % said)
        del calls[:], said[:]
        check(T.build(env, paths) is False and not calls, "a stamped tree: no build call (%d)" % len(calls))
        head = real_run(["git", "-C", ROOT, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
        check(any(s.startswith("up to date (%s" % head[:8]) and s.endswith("), opening") for s in said), "says up to date: %s" % said)
        os.remove(T.packed_bise(paths["app"]))
        del calls[:]
        check(T.build(env, paths) is True and len(calls) == 1, "a missing output (the packed bise) builds again")
        stamp = json.load(open(T.stamp_path(paths)))
        with open(T.stamp_path(paths), "w") as f:
            json.dump(dict(stamp, recipe="older"), f)
        del calls[:]
        check(T.build(env, paths) is True and len(calls) == 1, "another recipe builds again")
        with open(T.stamp_path(paths), "w") as f:
            json.dump(dict(stamp, head="0" * 40), f)
        del calls[:]
        check(T.build(env, paths) is True and len(calls) == 1, "another HEAD builds again")
        s = T.build_stamp(env, paths, None)
        r = T.read_stamp(paths)["root"]
        check(T.up_to_date(s, dict(s, root=r), paths) and not T.up_to_date(s, dict(s, root=r, changes="x"), paths) and not T.up_to_date(s, None, paths)
              and not T.up_to_date(s, dict(s), paths),
              "up_to_date: same stamp only")
    finally:
        T.subprocess.run, T.say = real_run, real_say
        if old_sign is not None:
            os.environ["BISE_TRY_SIGN_DIR"] = old_sign
        shutil.rmtree(w, ignore_errors=True)


def staged_never_over_a_running_try():
    """The law of a running try (ambient-lead m_11607), with faked processes and bundles, never a
    real build: while anything runs from the live 'bise try.app' the build is packed into the
    staging dir (BISE_PACK_OUT, a fresh dir) and the live bundle isn't touched; the stamp says
    'staged'; a swap is refused while it runs ('your try is open: quit it to get the new build');
    once nothing runs: live -> .old, staged -> live, stamp 'live', the next build call is up to
    date; .old goes once the new core answers; a stamp saying 'staged' with no staged bundle swaps
    nothing."""
    w = tempfile.mkdtemp(prefix="dtry-staged-")
    build = os.path.join(w, "build")
    staged_dir = os.path.join(build, "desktop-try", "staged")
    paths = {"target": os.path.join(build, "target"), "bise": os.path.join(build, "target", "debug", "bise"),
             "helper": os.path.join(build, "desktop-try", "bise-mac-helper"), "sign": os.path.join(build, "ambient", "sign"),
             "app": os.path.join(w, "dist", "mac-arm64", "bise try.app"),
             "staged_dir": staged_dir, "staged": os.path.join(staged_dir, "mac-arm64", "bise try.app")}
    scripts, said, running = [], [], []
    real_run, real_say, real_pids, real_core = T.subprocess.run, T.say, T.bundle_pids, T.core_up
    old_sign = os.environ.pop("BISE_TRY_SIGN_DIR", None)

    def mark(app):
        p = os.path.join(app, "Contents", "build.txt")
        return open(p).read() if os.path.exists(p) else None

    def pack(app, n, root):
        for p in (paths["bise"], paths["helper"], T.packed_bise(app), os.path.join(root, "repl-live")):
            os.makedirs(os.path.dirname(p), exist_ok=True)
            open(p, "w").close()
        with open(os.path.join(app, "Contents", "build.txt"), "w") as f:
            f.write("build %d" % n)

    def fake_run(cmd, *a, **k):
        if cmd and cmd[-3:-1] == ["/bin/bash", "-c"]:
            script = cmd[-1]
            scripts.append(script)
            pack(paths["staged"] if "BISE_PACK_OUT=" in script else paths["app"], len(scripts), root_of(script))
            return subprocess.CompletedProcess(cmd, 0)
        return real_run(cmd, *a, **k)
    T.subprocess.run, T.say = fake_run, (lambda *a: said.append(" ".join(str(x) for x in a)))
    T.bundle_pids = lambda app: list(running) if app == paths["app"] else []
    try:
        env = dict(os.environ)
        check(T.build(env, paths) is True and mark(paths["app"]) == "build 1" and T.read_stamp(paths)["where"] == "live",
              "nothing runs: the build is packed live")
        root1 = T.try_root(paths)
        check(root1 and root1.startswith(os.path.join(build, "desktop-try", "root-")) and not root1.startswith(ROOT)
              and "cp -R prompts %s.tmp/prompts" % root1 in scripts[0] and "bins.sh path --src" in scripts[0],
              "the app root is a snapshot in the build dir, never the checkout: %s" % root1)
        # his try opens from the live bundle; the source changes (a new HEAD in the stamp)
        running.append(77328)
        T.write_stamp(paths, dict(T.read_stamp(paths), head="0" * 40))
        del said[:]
        check(T.build(env, paths) is True and len(scripts) == 2, "a change while it runs builds")
        check("BISE_PACK_OUT=%s" % staged_dir in scripts[1] and "rm -rf %s" % staged_dir in scripts[1],
              "into a fresh staging dir: %s" % scripts[1][-400:])
        check(mark(paths["app"]) == "build 1" and mark(paths["staged"]) == "build 2", "the running bundle is never written over")
        check(T.read_stamp(paths)["where"] == "staged", "the stamp says staged")
        root2 = T.read_stamp(paths)["root"]
        check(root2 != root1 and os.path.isdir(root1) and T.try_root(paths) == root1,
              "a staged build gets its own root; the running try keeps reading its own: %s %s" % (root1, root2))
        check(any("quit it to get the new build" in s for s in said), "it says so: %s" % said)
        check(T.build(env, paths) is False and len(scripts) == 2, "staged and up to date: no second build")
        del said[:]
        check(T.swap_staged(paths) is None and mark(paths["app"]) == "build 1",
              "no swap while it runs: the live bundle stays")
        check(said == ["your try is open: quit it to get the new build"], "it says: %s" % said)
        # he quits: the next --try swaps it in, with no build step
        running.clear()
        old = T.swap_staged(paths)
        check(old == os.path.join(build, "desktop-try", "prev", "mac-arm64", "bise try.app") and mark(paths["app"]) == "build 2"
              and mark(old) == "build 1" and not os.path.exists(paths["staged"]),
              "swapped: live is the staged build, the old one in prev/: %s" % old)
        check(not os.path.exists(paths["app"] + ".old") and not any(n.endswith(".old") for n in os.listdir(os.path.dirname(paths["app"]))),
              "nothing next to the bundle: '<app>.old' is the app updater's alone (ambient-lead m_18341)")
        check(T.read_stamp(paths)["where"] == "live", "the stamp says live")
        check(T.try_root(paths) == root2, "the swap moves the root with the bundle: the app opens on %s" % T.try_root(paths))
        check(T.build(env, paths) is False and len(scripts) == 2, "the swapped build is up to date: no build call")
        T.core_up = lambda app: app == paths["app"]
        check(T.finish_swap(old, paths["app"], timeout=5, paths=paths) and os.path.isdir(old) and os.path.isdir(root1)
              and os.path.isdir(root2) and T.read_stamp(paths).get("old_root") == root1,
              "prev/ and its root stay after the core answers, for a rollback (ambient-lead m_21844)")
        # the next swap retires them: a fake staged build over the live one
        os.makedirs(os.path.join(paths["staged"], "Contents"))
        T.write_stamp(paths, dict(T.read_stamp(paths), where="staged", live_root=root2))
        real_out, T.outputs_there = T.outputs_there, lambda *a: True
        try:
            check(T.swap_staged(paths) == old and not os.path.exists(root1) and mark(old) == "build 2"
                  and T.read_stamp(paths).get("old_root") is None,
                  "the next swap removes the root before (prev/ now holds the build it replaced)")
        finally:
            T.outputs_there = real_out
        # put build 2 back live for the rest
        shutil.rmtree(paths["app"])
        os.rename(old, paths["app"])
        T.write_stamp(paths, dict(T.read_stamp(paths), where="live"))
        # a stamp that says staged with no staged bundle: nothing to swap
        T.write_stamp(paths, dict(T.read_stamp(paths), where="staged"))
        check(T.swap_staged(paths) is None and mark(paths["app"]) == "build 2", "no staged bundle: no swap")
        # the recipe in the stamp doesn't depend on where it's packed
        check(T.build_script(paths, None, out=staged_dir).replace(" BISE_PACK_OUT=%s" % staged_dir, "")
              .replace("rm -rf %s\n" % staged_dir, "") == T.build_script(paths, None), "out only adds the staging lines")
    finally:
        T.subprocess.run, T.say, T.bundle_pids, T.core_up = real_run, real_say, real_pids, real_core
        if old_sign is not None:
            os.environ["BISE_TRY_SIGN_DIR"] = old_sign
        shutil.rmtree(w, ignore_errors=True)


def legacy_old_retired():
    """A launcher before prev/ left 'bise try.app.old' next to the bundle, which the app's updater
    read as a failed update ('the new bise didn't start right', ambient-lead m_18341): the next
    --try or swap moves it to prev/ (or drops it when prev/ holds one), never touching the bundle."""
    w = tempfile.mkdtemp(prefix="dtry-legacy-")
    try:
        staged_dir = os.path.join(w, "build", "desktop-try", "staged")
        paths = {"app": os.path.join(w, "dist", "mac-arm64", "bise try.app"), "staged_dir": staged_dir,
                 "staged": os.path.join(staged_dir, "mac-arm64", "bise try.app")}
        prev = T.prev_bundle(paths)
        check(prev == os.path.join(w, "build", "desktop-try", "prev", "mac-arm64", "bise try.app"), "prev/ is under desktop-try: %s" % prev)
        check(T.retire_legacy_old(paths) is None, "no legacy .old: nothing to do")
        for d, mark in ((paths["app"], "live"), (paths["app"] + ".old", "old")):
            os.makedirs(os.path.join(d, "Contents"))
            open(os.path.join(d, "Contents", "m"), "w").write(mark)
        check(T.retire_legacy_old(paths) == prev and not os.path.exists(paths["app"] + ".old")
              and open(os.path.join(prev, "Contents", "m")).read() == "old"
              and open(os.path.join(paths["app"], "Contents", "m")).read() == "live", "the legacy .old moves to prev/, the bundle stays")
        os.makedirs(paths["app"] + ".old")
        check(T.retire_legacy_old(paths) is None and not os.path.exists(paths["app"] + ".old") and os.path.isdir(prev),
              "prev/ holds one already: the legacy .old goes")
    finally:
        shutil.rmtree(w, ignore_errors=True)


def real_opens_with_nothing():
    """bise-ambient --real (ambient-lead m_18348): his own home gets no variable at all; a stand-in
    home (amb-tools' real-shaped world, under ~/.bise/gate, BISE_TRY_REAL=<home>/.bise) gets HOME
    and BISE_HOME, plus BISE_DESKTOP_OFFSCREEN only when the caller set it; anything else is
    refused. A bundle without its runtime inside (amb-core m_18365) is refused, never patched
    with BISE_APP_ROOT. Then one run through the open hook on a stand-in: the legacy .old is
    retired and the app gets exactly HOME and BISE_HOME."""
    his = os.path.expanduser("~/.bise")
    check(T.real_app_env({"BISE_DESKTOP_OFFSCREEN": "1", "BISE_APP_ROOT": "/x", "BISE_DESKTOP_TEST": "1"}, his, his) == [],
          "his own home: no variable, not even the off-screen knob or a test one")
    stand = os.path.join(T.GATE, "real-shape", "w", "h", ".bise")
    check(T.real_app_env({}, stand, his) == ["HOME=%s" % os.path.dirname(stand), "BISE_HOME=%s" % stand],
          "a stand-in home: HOME and BISE_HOME, the projects scan stays in it")
    check(T.real_app_env({"BISE_DESKTOP_OFFSCREEN": "1", "BISE_DESKTOP_TEST": "1"}, stand, his)[-1] == "BISE_DESKTOP_OFFSCREEN=1"
          and len(T.real_app_env({"BISE_DESKTOP_OFFSCREEN": "1"}, stand, his)) == 3, "the off-screen run adds that knob only")
    for bad in ("/tmp/h/.bise", os.path.join(T.GATE, "w", "h")):
        try:
            T.real_app_env({}, bad, his)
            check(False, "%s refused" % bad)
        except T.TryError:
            check(True, "%s refused" % bad)
    w = tempfile.mkdtemp(prefix="dtry-real-", dir=T.GATE if os.path.isdir(T.GATE) else None)
    try:
        app = os.path.join(w, "dist", "mac-arm64", "bise try.app")
        os.makedirs(os.path.join(app, "Contents", "Resources", "bin"))
        check(T.self_contained(app) == ["bise", "VERSION", "repl-live", "sb-core"], "an empty bundle lacks its runtime")
        for n in T.SELF_CONTAINED:
            open(os.path.join(app, "Contents", "Resources", "bin", n), "w").close()
        check(T.self_contained(app) == [], "the runtime inside: self-contained")
        if not os.path.isdir(T.GATE):
            return
        real = os.path.join(w, "h", ".bise")
        os.makedirs(real)
        os.makedirs(app + ".old")
        staged_dir = os.path.join(w, "build", "desktop-try", "staged")
        paths = {"app": app, "staged_dir": staged_dir, "staged": os.path.join(staged_dir, "mac-arm64", "bise try.app"),
                 "helper": os.path.join(w, "build", "desktop-try", "bise-mac-helper"), "bise": os.path.join(w, "build", "bise")}
        got = os.path.join(w, "got")
        hook = os.path.join(w, "hook.sh")
        with open(hook, "w") as f:
            f.write("#!/bin/sh\nprintf '%%s\\n' \"$@\" > %s\n" % shlex.quote(got))
        os.chmod(hook, 0o755)
        # the bundle carries 1c5659b8; the try checkout's stamp says a newer build waits, staged (m_21726)
        with open(os.path.join(app, "Contents", "Resources", "bin", "VERSION"), "w") as f:
            f.write("id=root-1c5659b8-abcdef\ncommit=1c5659b8aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n")
        T.write_stamp(paths, {"head": "b51422c0" + "b" * 32, "changes": "", "recipe": "r", "where": "staged", "root": os.path.join(w, "nowhere")})
        said, real_say, old_hook = [], T.say, os.environ.get("BISE_TRY_OPEN")
        T.say = lambda *a: said.append(" ".join(str(x) for x in a))
        os.environ["BISE_TRY_OPEN"] = hook
        try:
            T.open_real(real, os.path.join(real, "try", "desktop"), paths, environ={"BISE_DESKTOP_TEST": "1", "PATH": "/usr/bin:/bin"}, timeout=0.5)
        finally:
            T.say = real_say
            if old_hook is None:
                os.environ.pop("BISE_TRY_OPEN", None)
            else:
                os.environ["BISE_TRY_OPEN"] = old_hook
        check(open(got).read().split("\n")[:3] == [app, "HOME=%s" % os.path.join(w, "h"), "BISE_HOME=%s" % real],
              "the app gets HOME and BISE_HOME only: %r" % open(got).read())
        check(not os.path.exists(app + ".old"), "the legacy .old is out of the updater's path")
        check(any(("your home %s" % real) in s and app in s for s in said), "it says the home and the bundle: %s" % said)
        check(any(("your home %s" % real) in s and "(1c5659b8" in s for s in said), "the open line names the bundle's own build: %s" % said)
        loud = [s for s in said if s.startswith("NOT UPDATED")]
        check(len(loud) == 1 and "OLD build 1c5659b8" in loud[0] and "staged build b51422c0" in loud[0],
              "a staged build that did not go live is said loudly, old and staged named: %s" % said)
    finally:
        shutil.rmtree(w, ignore_errors=True)


def real_open_fronts_a_running_app():
    """--real on an app already open with nothing staged brings it to the front, never quits it
    (app-quits: his launcher click restarted the same build); a staged build or an open try restarts."""
    check(T.real_plan([4242], False, False) == "front", "open, nothing staged: to the front")
    check(T.real_plan([4242], True, False) == "restart", "a staged build waits: restart to swap it in")
    check(T.real_plan([4242], False, True) == "restart", "the try is open: it stops, then the real one opens")
    check(T.real_plan([], False, False) == "restart", "not running: it opens")
    # `running` is bundle_apps(<the applet's bundle path>): an older bundle at another path running
    # on his home counts as none here, so the restart path, never 'front' on the wrong version
    other = os.path.join(tempfile.mkdtemp(dir=os.environ.get("TMPDIR")), "bise try.app")
    check(T.real_plan(T.bundle_apps(other), False, False) == "restart",
          "an older bundle path running elsewhere: restart, never 'front' on it")
    check(T.real_front_cmd("/A/bise try.app") == ["open", "-a", "/A/bise try.app"], "the front: no -n, no second instance")


def real_open_in_background_offscreen():
    """--real with BISE_DESKTOP_OFFSCREEN=1 opens with 'open -g' (never in front, amb-tools m_18713);
    his own --real opens as before."""
    app = "/A/bise try.app"
    check(T.real_open_cmd(app, [], {}) == ["open", "-n", "-a", app], "his --real: a plain open")
    off = T.real_open_cmd(app, ["HOME=/g/h", "BISE_HOME=/g/h/.bise", "BISE_DESKTOP_OFFSCREEN=1"], {"BISE_DESKTOP_OFFSCREEN": "1"})
    check(off[:5] == ["open", "-g", "-n", "-a", app], "the knob: open -g: %r" % off)
    check(off[5:] == ["--env", "HOME=/g/h", "--env", "BISE_HOME=/g/h/.bise", "--env", "BISE_DESKTOP_OFFSCREEN=1"], "its env passes")
    check(T.real_open_cmd(app, [], {"BISE_DESKTOP_OFFSCREEN": "0"})[1] == "-n", "'0' is not the knob")


def real_stop_never_his_hubs():
    """bise-ambient --real --stop (ambient-lead m_18642): the bundle quits; on his own home his hubs
    are never touched; a stand-in outside ~/.bise/gate is refused; on a stand-in under it, its hubs
    are stopped (here: none running, nothing killed). quit_bundle is faked: no app is touched."""
    real_quit = T.quit_bundle
    T.quit_bundle = lambda app: []
    try:
        his = os.path.expanduser("~/.bise")
        r = T.stop_real(his, "/nowhere/bise try.app", default_real=his)
        check(r == {"app": [], "hubs": [], "killed": []}, "his home: only the app, never his hubs: %r" % r)
        try:
            T.stop_real("/tmp/x/.bise", "/nowhere/bise try.app", default_real=his)
            check(False, "a stand-in outside the gate is refused")
        except T.TryError:
            check(True, "a stand-in outside the gate is refused")
        if os.path.isdir(T.GATE):
            w = tempfile.mkdtemp(prefix="dtry-realstop-", dir=T.GATE)
            try:
                r = T.stop_real(os.path.join(w, ".bise"), "/nowhere/bise try.app", default_real=his)
                check(r == {"app": [], "hubs": [], "killed": []}, "a stand-in with no hub: nothing to stop: %r" % r)
            finally:
                shutil.rmtree(w, ignore_errors=True)
    finally:
        T.quit_bundle = real_quit


def quit_only_this_bundle():
    """quit_bundle and quit_app never quit by bundle id (ambient-lead m_22006): a stand-in or a copy
    of his try has his id, and 'tell application id … to quit' quit his app too. Only the pids whose
    executable is inside THIS bundle's path get a signal; another bundle's app of the same name and
    id, this bundle's helpers and its core don't; no pid: nothing at all. ps and kill are faked."""
    src = open(os.path.join(ROOT, "scripts", "desktop_try.py")).read()
    check("osascript" not in src and "tell application" not in src, "the launcher never sends an Apple Event")
    app, his = "/g/build/desktop-try/mac-arm64/bise try.app", "/Users/x/.bise/dev/build/desktop-try/mac-arm64/bise try.app"
    ps = "".join("%d %s%s" % (p, c, chr(10)) for p, c in (
        (11, his + "/Contents/MacOS/bise try"), (12, app + "/Contents/MacOS/bise try --flag"),
        (13, app + "/Contents/Frameworks/bise try Helper.app/Contents/MacOS/bise try Helper"),
        (14, app + "/Contents/Resources/bin/bise ambient-core --workspace /w")))
    calls, alive = [], {12}
    real = (T.subprocess.run, T.os.kill, T.pid_alive)

    def run(argv, **kw):
        calls.append(argv)
        return subprocess.CompletedProcess(argv, 0, stdout=ps if argv[0] == "ps" else "", stderr="")

    def kill(pid, sig):
        calls.append(("kill", pid, sig))
        alive.discard(pid)
    T.subprocess.run, T.os.kill, T.pid_alive = run, kill, lambda p: p in alive
    try:
        check(T.quit_bundle(app, timeout=1) == [12], "only this bundle's app")
        kills = [c for c in calls if c[0] == "kill"]
        check(kills == [("kill", 12, T.signal.SIGTERM)], "one SIGTERM to its pid, nothing else: %r" % kills)
        check([c for c in calls if c[0] != "ps" and c[0] != "kill"] == [], "no other command: %r" % calls)
        del calls[:]
        ps = ""
        check(T.quit_bundle(app, timeout=1) == [] and [c for c in calls if c[0] != "ps"] == [], "no app of this bundle: nothing sent")
    finally:
        T.subprocess.run, T.os.kill, T.pid_alive = real


def hook_keeps_pid():
    """BISE_TRY_OPEN (the gate's open): the app the hook starts is found and its pid kept in
    <try>/dev/desktop/app.pid, so --stop finds it (qa-native m_11066). The 'app' is a throwaway
    bundle whose executable is a link to this python (not /bin/sleep: ps -E hides a platform
    binary's env), started by the hook with the env it gets, sleeping."""
    gate = os.path.realpath(os.path.expanduser("~/.bise/gate"))
    os.makedirs(gate, exist_ok=True)
    w = tempfile.mkdtemp(prefix="dtry-hook-", dir=gate)
    tryh, app = os.path.join(w, "try"), os.path.join(w, "bise try.app")
    started = []
    old = os.environ.get("BISE_TRY_OPEN")
    try:
        os.makedirs(os.path.join(app, "Contents", "MacOS"))
        os.symlink(os.path.realpath(sys.executable), T.app_exe(app))
        hook = os.path.join(w, "open.sh")
        with open(hook, "w") as f:
            f.write('#!/bin/sh\napp="$1"; shift\nenv "$@" "$app/Contents/MacOS/bise try" -c "import signal; signal.pause()" </dev/null >/dev/null 2>&1 &\necho $! > "%s/started"\n' % w)
        os.chmod(hook, 0o755)
        os.environ["BISE_TRY_OPEN"] = hook
        pid = T.open_app(app, tryh, {"PATH": "/usr/bin:/bin"}, os.path.join(w, "root"))
        started.append(int(open(os.path.join(w, "started")).read()))
        check(pid == started[0], "the hook's app is found: %s vs %s" % (pid, started))
        check(T.read_pid(T.app_pid_file(tryh)) == started[0], "its pid is kept in app.pid")
        check(T.app_running(app, tryh) == started[0], "app_running (what --stop reads) sees it")
    finally:
        if old is None:
            os.environ.pop("BISE_TRY_OPEN", None)
        else:
            os.environ["BISE_TRY_OPEN"] = old
        for p in started:
            try:
                os.kill(p, 15)
            except OSError:
                pass
        shutil.rmtree(w, ignore_errors=True)


def git(*a, cwd=None):
    return subprocess.run(["git"] + list(a), cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()


def integration():
    bise = os.path.join(ROOT, "rust", "target", "debug", "bise")
    if not os.access(bise, os.X_OK):
        print("skip the integration part: no %s" % bise)
        return
    # the try hub's app root is this tree (try_env's BISE_APP_ROOT): with no ./repl-live it exits at
    # once ('BISE_APP_ROOT=<tree> has no repl-live'), and the socket wait below ran out at 116-120 s
    # (nightly-reds, promote-lock m_35629). The test places it itself (scripts/bins.sh: a cache hit
    # ~0.3 s, a miss builds it); under a gate (SB_RUN_KIND) a REPL it can't place is red, by hand a
    # printed skip (review-gates m_36050: never a silent green where nobody placed it)
    repl = os.path.join(ROOT, "repl-live")
    placed = subprocess.run([os.path.join(ROOT, "scripts", "bins.sh"), "repl-live"], cwd=ROOT,
                            capture_output=True, text=True)
    if placed.returncode != 0 or not os.access(repl, os.X_OK):
        why = "no %s: scripts/bins.sh repl-live: %s" % (repl, (placed.stderr or placed.stdout)[-400:])
        if os.environ.get("SB_RUN_KIND"):
            check(False, "the try hub's REPL: " + why)
            return
        print("skip the integration part (by hand, outside a gate): " + why)
        return
    gate = os.path.realpath(os.path.expanduser("~/.bise/gate"))
    os.makedirs(gate, exist_ok=True)
    w = tempfile.mkdtemp(prefix="dtry-", dir=gate)
    real, tryh = os.path.join(w, "real"), os.path.join(w, "real", "try", "desktop")
    try:
        os.makedirs(real)
        repos = []
        for n in ("alpha", "beta"):
            r = os.path.join(w, "code", n)
            os.makedirs(r)
            git("init", "-q", "-b", "main", cwd=r)
            with open(os.path.join(r, "README.md"), "w") as f:
                f.write("# %s\n" % n)
            git("-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", "add", "-A", cwd=r)
            git("-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "init", cwd=r)
            repos.append(r)
        with open(os.path.join(real, "projects.json"), "w") as f:
            json.dump({"v": 1, "projects": [{"path": p, "name": os.path.basename(p), "order": i} for i, p in enumerate(repos)]}, f)
        with open(os.path.join(real, "config.toml"), "w") as f:
            f.write('model = "mistral-small-latest"\n\n[pages]\nmirror = "/nowhere/deploy.sh"\n')
        with open(os.path.join(real, "auth.json"), "w") as f:
            json.dump({"openrouter": {"type": "api", "key": "sk-or-FAKE"}}, f)
        before = {r: git("rev-parse", "HEAD", cwd=r) + git("status", "--porcelain", cwd=r) for r in repos}
        env = dict(T.his_env(os.environ), BISE_TRY_REAL=real, BISE_TRY_BISE=bise)

        def go(*args):
            return subprocess.run([sys.executable, "-B", os.path.join(ROOT, "scripts", "desktop_try.py"), "--try"] + list(args),
                                  env=env, capture_output=True, text=True, timeout=180)
        r = go("--no-build", "--no-open")
        check(r.returncode == 0, "the first run: %s" % r.stderr[-800:])
        for n in ("alpha", "beta"):
            c = os.path.join(tryh, "repos", n)
            check(os.path.isdir(os.path.join(c, ".git")), "clone %s" % n)
            check(not os.path.exists(os.path.join(c, ".git", "objects", "info", "alternates")), "no alternates in %s" % n)
            check(git("remote", "get-url", "--push", "origin", cwd=c) == "DISABLED-in-try", "push disabled in %s" % n)
        listed = [p["path"] for p in json.load(open(os.path.join(tryh, "projects.json")))["projects"]]
        check(sorted(os.path.basename(p) for p in listed) == ["alpha", "beta"] and all(p.startswith(tryh) for p in listed),
              "the try's projects are the clones: %s" % listed)
        cfg = os.path.join(tryh, "config.toml")
        text = open(cfg).read()
        check(oct(os.stat(cfg).st_mode & 0o777) == "0o600" and "FAKE" not in text and "jq -r .openrouter.key" in text and "[pages]" not in text,
              "the try's config: 0600, a key_command, no key, no [pages]:\n" + text)
        check(not os.path.exists(os.path.join(real, "hubs")) and json.load(open(os.path.join(real, "projects.json")))["projects"][0]["path"] == repos[0],
              "his home untouched")
        check({r: git("rev-parse", "HEAD", cwd=r) + git("status", "--porcelain", cwd=r) for r in repos} == before, "his repos untouched")

        r = go("--no-build", "--no-open")
        check(r.returncode == 0 and "reused" in r.stderr and "cloned" not in r.stderr, "a re-run reuses the clones: %s" % r.stderr[-400:])
        check(len(json.load(open(os.path.join(tryh, "projects.json")))["projects"]) == 2, "no project added twice")

        # a hub on the try home, then --stop
        henv = T.try_env(T.his_env(os.environ), real, tryh)
        henv["BEND_PROVIDER_URL"] = "http://127.0.0.1:9/v1/chat/completions"
        ws = os.path.join(tryh, "repos", "alpha")
        herr = os.path.join(w, "hub.stderr")
        with open(herr, "w") as err:
            hub = subprocess.Popen([bise, "sbd", "--workspace", ws], env=henv, stdin=subprocess.DEVNULL,
                                   stdout=err, stderr=err, start_new_session=True)

        # a hub that exits never makes its socket: say why at once, never after the whole wait
        def sock_or_exit():
            if hub.poll() is not None:
                raise AssertionError("the try hub exited (rc %s) before its socket: %s" % (hub.returncode, open(herr).read()[-800:]))
            return T.hub_socks(tryh)
        wait.until(sock_or_exit, 30, "the try hub's socket under %s/hubs" % tryh, poll=0.2)
        check(T.hub_socks(tryh) and T.hub_socks(tryh)[0].startswith(tryh + "/hubs/"), "the hub's state is in the try home: %s" % T.hub_socks(tryh))
        r = go("--stop")
        check(r.returncode == 0 and "your own bise wasn't touched" in r.stderr, "--stop: %s" % r.stderr[-400:])
        try:
            hub.wait(timeout=20)
        except subprocess.TimeoutExpired:
            check(False, "--stop left the hub running")
            hub.kill()
        check(not T.try_pids(tryh), "nothing of the try left: %s" % T.try_pids(tryh))

        r = go("--no-build", "--no-open", "--fresh")
        check(r.returncode == 0 and r.stderr.count("cloned") == 2, "--fresh clones again: %s" % r.stderr[-400:])
    finally:
        for pid in T.try_pids(tryh):
            try:
                os.kill(pid, 15)
            except OSError:
                pass
        shutil.rmtree(w, ignore_errors=True)


def candidate_then_promote():
    """ambient-lead m_14320: --candidate <sha> builds into its own dir and never touches the try
    checkout, the try stamp or the staged dir; built twice, it builds once; --promote <sha> moves the
    try checkout to it and the built bundle + root in (no build), stamped as the try checkout's own
    script computes it (his next --try is up to date), and the candidate dir goes. The build is faked."""
    import re
    w = tempfile.mkdtemp(prefix="dt-cand-")
    repo, trysrc = os.path.join(w, "repo"), os.path.join(w, "try-src")
    genv = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t")
    git = lambda *a, cwd=repo: subprocess.run(["git"] + list(a), cwd=cwd, env=genv, check=True, capture_output=True, text=True).stdout.strip()  # noqa: E731
    os.makedirs(os.path.join(repo, "scripts"))
    os.makedirs(os.path.join(repo, "tests"))
    for f in ("desktop_try.py", "desktop_try_env.py", "heavy_slot.py"):
        shutil.copy(os.path.join(ROOT, "scripts", f), os.path.join(repo, "scripts", f))
    shutil.copy(os.path.join(ROOT, "tests", "bise_env.py"), os.path.join(repo, "tests", "bise_env.py"))
    with open(os.path.join(repo, ".gitignore"), "w") as f:
        f.write("dist/\n__pycache__/\n")      # as the real repo's .gitignore: the packed app is no change
    git("init", "-q", "-b", "main")
    git("add", "-A")
    git("-c", "commit.gpgsign=false", "commit", "-q", "-m", "a")
    a = git("rev-parse", "HEAD")
    with open(os.path.join(repo, "NEW.md"), "w") as f:
        f.write("the gate's candidate\n")
    git("add", "-A")
    git("-c", "commit.gpgsign=false", "commit", "-q", "-m", "b")
    b = git("rev-parse", "HEAD")
    git("worktree", "add", "-q", "--detach", trysrc, a)
    env = dict(genv, SB_BUILD_DIR=os.path.join(w, "build"), BISE_TRY_CHECKOUT=trysrc)
    env.pop("BISE_TRY_SIGN_DIR", None)
    paths = T.try_paths(env)
    # his current try: live at a, with its root
    old_root = os.path.join(os.path.dirname(paths["helper"]), "root-old")
    os.makedirs(old_root)
    open(os.path.join(old_root, "repl-live"), "w").close()
    T.write_stamp(paths, {"head": a, "changes": "", "recipe": "r", "where": "live", "root": old_root})
    before = open(T.stamp_path(paths)).read()
    builds, cmds, fail_next = [], [], []
    real_run, real_say = T.subprocess.run, T.say

    def fake_run(cmd, *k, **kw):
        if cmd and cmd[-3:-1] == ["/bin/bash", "-c"]:
            script = cmd[-1]
            builds.append(script)
            cmds.append(cmd)
            if fail_next:
                fail_next.pop()
                return subprocess.CompletedProcess(cmd, 1)
            out = shlex.split(re.search(r"BISE_PACK_OUT=(\S+)", script).group(1))[0]
            name = shlex.split(re.search(r"BISE_PACK_NAME=('[^']*'|\S+)", script).group(1))[0]
            app_id = re.search(r"BISE_PACK_APP_ID=(\S+)", script).group(1)
            app = os.path.join(out, paths["arch"], name + ".app")
            made = [T.packed_bise(app)]
            roots = re.findall(r"rm -rf \S+ && mv \S+ (\S+)", script)
            if roots:   # a full build (the candidate's): its root, the try bise and helper too
                made += [os.path.join(shlex.split(roots[-1])[0], "repl-live"), paths["bise"], paths["helper"]]
            for p in made:
                os.makedirs(os.path.dirname(p), exist_ok=True)
                open(p, "w").close()
            with open(os.path.join(app, "Contents", "Info.plist"), "wb") as f:
                plistlib.dump({"CFBundleIdentifier": app_id, "CFBundleName": name}, f)
            with open(os.path.join(app, "Contents", "build.txt"), "w") as f:
                f.write("build %d %s" % (len(builds), app_id))
            return subprocess.CompletedProcess(cmd, 0)
        return real_run(cmd, *k, **kw)
    T.subprocess.run, T.say = fake_run, (lambda *x: None)
    try:
        info = T.candidate(env, paths, b[:8], repo=repo)
        c = T.cand_paths(paths, b)
        check(len(builds) == 1 and info["sha"] == b and info["app"] == c["app"] and T.cand_built(c), "the candidate is built: %s" % info)
        # fast-try p1: built in the candidates' one build checkout moved to b, its root in its own worktree
        wk = T.work_paths(paths["cand"])
        check("checkout -q --force --detach %s" % b in builds[0] and "cd %s/rust && cargo build" % shlex.quote(wk["src"]) in builds[0]
              and "BISE_PACK_BISE=%s " % shlex.quote(os.path.join(wk["target"], "debug", "bise")) in builds[0]
              and "cd %s && scripts/bins.sh" % shlex.quote(c["src"]) in builds[0] and "BISE_PACK_OUT=%s" % shlex.quote(c["dist"]) in builds[0]
              and "cp -cR out %s" % shlex.quote(os.path.join(c["src"], "apps", "desktop", "out")) in builds[0],
              "built in the work checkout at b, its root and out/ in its own worktree, into its own dir: %s" % builds[0])
        check(cmds[0][:3] == ["/usr/bin/lockf", "-k", wk["lock"]], "the whole build holds the work lock, taken before the heavy slot: %s" % cmds[0])
        check(git("rev-parse", "HEAD", cwd=c["src"]) == b, "the candidate's worktree is at b")
        check(git("rev-parse", "HEAD", cwd=trysrc) == a, "the try checkout never moved")
        check(open(T.stamp_path(paths)).read() == before and not os.path.exists(paths["staged_dir"]), "the try stamp and staged dir untouched")
        check(json.load(open(c["json"]))["root"] == c["root"], "cand.json says where the root is")
        check(json.load(open(c["json"]))["team_signed"] is False and info["team_signed"] is False,
              "a candidate without --team-sign is the build's own signing: team_signed false")
        T.candidate(env, paths, b, repo=repo)
        check(len(builds) == 1, "built once: %d builds" % len(builds))
        # ambient-lead m_20670: a candidate is its own bundle, never his id (LaunchServices mixed them)
        check(c["app"].endswith("/bise cand.app") and T.bundle_id(c["app"]) == T.CAND_ID == "dev.bise.desktop.cand",
              "the candidate's Info.plist has the candidate's id: %s" % T.bundle_id(c["app"]))
        check("BISE_PACK_NAME='bise cand' BISE_PACK_APP_ID=dev.bise.desktop.cand " in builds[0] and "desktop.try" not in builds[0],
              "the candidate is packed as 'bise cand' (its own userData): %s" % builds[0])
        check(T.CAND_ID != T.APP_ID and T.CAND_NAME != T.APP_NAME, "candidate and try never share an id or a name")
        plist = os.path.join(c["app"], "Contents", "Info.plist")
        good = open(plist, "rb").read()
        with open(plist, "wb") as f:
            plistlib.dump({"CFBundleIdentifier": T.APP_ID}, f)
        check(not T.cand_built(c), "a candidate with his id (built before m_20670) is not built: it builds again")
        with open(plist, "wb") as f:
            f.write(good)

        # a failed repack moves nothing
        fail_next.append(1)
        try:
            T.promote(env, b, repo=repo)
            check(False, "a failed repack promotes")
        except T.TryError:
            pass
        check(git("rev-parse", "HEAD", cwd=trysrc) == a and open(T.stamp_path(paths)).read() == before and T.cand_built(c),
              "a failed repack leaves the try checkout, stamp and candidate as they were")
        builds.pop()

        # promote-lock: a pass on the candidate holds cand.sh's lock (a lockf, as cand.sh takes it,
        # on this test's HOME): the promote refuses after its wait with nothing moved, the candidate
        # dir intact; a pass that ends within the wait lets it through
        import threading
        import cand_lock as L
        env["HOME"] = os.path.join(w, "home")
        base = L.lock_base(env["HOME"], c["app"])
        os.makedirs(os.path.dirname(base))
        with open(base + ".holder", "w") as f:
            f.write("pid 4242 since now, out q, app %s" % c["app"])
        hold = subprocess.Popen(["/usr/bin/lockf", base + ".lock", "sleep", "60"])
        check(wait.until(lambda: subprocess.run(["/usr/bin/lockf", "-t", "0", base + ".lock", "true"], capture_output=True).returncode == 75,
                         10, "the pass holds the lock"), "the stand-in pass holds the candidate lock")
        try:
            T.promote(dict(env, CAND_LOCK_WAIT="1"), b, repo=repo)
            check(False, "a promote while a pass holds the candidate lock")
        except T.TryError as e:
            check("refused" in str(e) and "pid 4242" in str(e), "the refusal names the pass: %s" % e)
        check(git("rev-parse", "HEAD", cwd=trysrc) == a and open(T.stamp_path(paths)).read() == before and T.cand_built(c)
              and os.path.isdir(c["src"]), "a refused promote moves nothing and the candidate dir survives")
        builds.pop()   # its repack: the counts below are the promote that goes through
        said = []
        T.say = said.append
        threading.Thread(target=lambda: wait.until(lambda: any("held by" in s for s in said), 30, "promote waits") and hold.kill(),
                         daemon=True).start()

        stamp = T.promote(dict(env, CAND_LOCK_WAIT="60"), b, repo=repo)
        hold.wait()
        T.say = lambda *x: None
        check(any("held by pid 4242" in s and "waiting up to 60 s" in s for s in said), "the promote said it waits for the pass: %s" % said)
        check(not os.path.exists(base + ".lock") or "desktop-try --promote" in open(base + ".holder").read(),
              "the promote held the lock as its holder")
        env["HOME"] = genv["HOME"]
        m = T.try_module(trysrc)
        tp = m.try_paths(env)
        check(len(builds) == 2, "promote repacks once: %d" % len(builds))
        rp = builds[1]
        # fast-try p1: its worktree has no node_modules (built in the work checkout): installed when missing
        check(all(x not in rp for x in ("cargo build", "pnpm build", "bins.sh")) and "pnpm dist" in rp and T.promote_install() in rp
              and rp.count("pnpm install") == 1
              and "cd %s" % shlex.quote(os.path.join(c["src"], "apps", "desktop")) in rp,
              "the repack is electron-builder only, in the candidate's worktree: %s" % rp)
        check("BISE_PACK_NAME='bise try' BISE_PACK_APP_ID=dev.bise.desktop.try " in rp
              and "BISE_PACK_BISE=%s" % shlex.quote(T.packed_bise(c["app"])) in rp
              and "BISE_PACK_HELPER=%s " % shlex.quote(os.path.join(c["dir"], "pack-helper", "bise-mac-helper")) in rp
              and shlex.quote(os.path.join(c["app"], "Contents", "Resources", "tray-badge.node")) in rp
              and "BISE_PACK_ROOT=%s" % shlex.quote(c["root"]) in rp,
              "repacked as his try from the candidate's own bise, helper (with its badge) and root: %s" % rp)
        check(git("rev-parse", "HEAD", cwd=trysrc) == b, "the try checkout moved to b")
        check(stamp["head"] == b and stamp["where"] == "live" and stamp.get("old_root") == old_root, "live now (no try open): %s" % stamp)
        check(open(os.path.join(tp["app"], "Contents", "build.txt")).read() == "build 2 dev.bise.desktop.try"
              and T.bundle_id(tp["app"]) == T.APP_ID == "dev.bise.desktop.try",
              "the promoted bundle is the repack, with his try's id: %s" % T.bundle_id(tp["app"]))
        check(m.up_to_date(m.build_stamp(env, tp, None), m.read_stamp(tp), tp), "his next --try is up to date: no rebuild")
        check(not os.path.exists(c["dir"]) and c["src"] not in git("worktree", "list"), "the candidate dir and worktree are gone")
        try:
            T.promote(env, b, repo=repo)
            check(False, "a second promote without a candidate")
        except T.TryError:
            pass
    finally:
        T.subprocess.run, T.say = real_run, real_say
        shutil.rmtree(w, ignore_errors=True)


def repack_reads_its_config():
    """try-repack-fix: a packed bundle has its helper in MacOS/ and its tray badge in Resources/;
    pack_helper's lines put them side by side, and electron-builder.mjs loads with that helper
    (it throws without the badge next to it, which electron-builder says as 'Cannot access
    default before initialization': --promote's repack failed so from cf6922d30 to this fix)."""
    if not shutil.which("node"):
        return
    with tempfile.TemporaryDirectory() as t:
        app = os.path.join(t, "cand", "bise cand.app")
        for f in ("Contents/MacOS/bise-mac-helper", "Contents/Resources/tray-badge.node",
                  "bise", "root/VERSION", "root/repl-live", "root/sb-core"):
            p = os.path.join(app if f.startswith("Contents") else t, f)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            open(p, "w").close()
        copy, helper = T.pack_helper(app, os.path.join(t, "pack-helper"))
        r = subprocess.run(["bash", "-euo", "pipefail", "-c", "\n".join(copy * 2)], capture_output=True, text=True)
        check(r.returncode == 0, "pack_helper's lines run (twice: a re-run starts clean): %s" % r.stderr)
        js = "import(process.argv[1]).then(m => console.log(m.default.appId))"
        env = {"PATH": os.environ.get("PATH", ""), "BISE_PACK_APP_ID": T.APP_ID, "BISE_PACK_BISE": os.path.join(t, "bise"),
               "BISE_PACK_ROOT": os.path.join(t, "root"), "BISE_PACK_HELPER": helper}
        load = lambda e: subprocess.run(["node", "-e", js, "file://" + os.path.join(ROOT, "apps", "desktop", "electron-builder.mjs")],
                                        capture_output=True, text=True, env=e)
        r = load(env)
        check(r.returncode == 0 and r.stdout.strip() == T.APP_ID, "electron-builder.mjs loads with pack_helper's helper: %s" % r.stderr)
        r = load(dict(env, BISE_PACK_HELPER=os.path.join(app, "Contents", "MacOS", "bise-mac-helper")))
        check(r.returncode != 0 and "tray-badge.node" in r.stderr, "the bundle's own helper path, its badge apart, still fails: %s" % r.stderr)


def build_key_cases():
    """One app build per sha (release-speed plan A step 5, architect m_63479): build_key is the sha,
    node, pnpm and the build's mode; a dirty tree's never stands; the build config reads no env the
    key doesn't cover."""
    import re
    with tempfile.TemporaryDirectory() as d:
        g = lambda *a: subprocess.run(["git", "-C", d, *a], capture_output=True, text=True, check=True).stdout.strip()  # noqa: E731
        g("init", "-q")
        g("config", "user.email", "t@t")
        g("config", "user.name", "t")
        g("config", "commit.gpgsign", "false")
        open(os.path.join(d, "f"), "w").write("1\n")
        g("add", "f")
        g("commit", "-q", "-m", "one")
        sha = g("rev-parse", "HEAD")
        k = T.build_key(d, sha, env=dict(os.environ))
        check(k is not None and k.startswith(sha[:12] + " node") and "mode[BISE_DESKTOP_SOURCEMAP=]" in k and T.reusable(k),
              "build_key: the sha, node, pnpm and the mode: %r" % k)
        check(T.build_key(d, sha, env=dict(os.environ)) == k, "build_key: stable for the same sha and tools")
        k2 = T.build_key(d, sha, env=dict(os.environ, BISE_DESKTOP_SOURCEMAP="1"))
        check(k2 != k and "BISE_DESKTOP_SOURCEMAP=1" in k2, "build_key: another mode (a sourcemap build) is another key")
        open(os.path.join(d, "f"), "w").write("2\n")
        kd = T.build_key(d, sha, env=dict(os.environ))
        check(kd.endswith(" dirty") and not T.reusable(kd), "build_key: a dirty tree says so, never reused: %r" % kd)
        check(T.build_key(d, "0" * 40, env=dict(os.environ)) is None, "build_key: no such sha, None")
    # the guard: every process.env name the vite config or electron-builder reads is in the key's mode
    # (the vite config's must be) or a pack-only one, so a new knob can't share a key unseen
    desk = os.path.join(HERE, "..", "apps", "desktop")
    pat = re.compile(r"process\.env\[['\"]([A-Z0-9_]+)['\"]\]|process\.env\.([A-Z0-9_]+)|need\(['\"]([A-Z0-9_]+)['\"]\)")
    for f, allowed in (("electron.vite.config.ts", T.BUILD_MODE_ENV), ("electron-builder.mjs", T.BUILD_MODE_ENV + T.PACK_ONLY_ENV)):
        text = open(os.path.join(desk, f)).read()
        names = {a or b or c for a, b, c in pat.findall(text)}
        check(names and not (names - set(allowed)), "%s reads only env build_key covers: %s" % (f, sorted(names - set(allowed)) or "ok"))
        check("process.env[" not in re.sub(r"process\.env\[['\"][A-Z0-9_]+['\"]\]|process\.env\[name\]", "", text),
              "%s reads no env by a computed name but need()'s" % f)
    check(T.BISE_CARGO in T.build_script(T.try_paths({"HOME": "/h"}), None, repo="/r"), "the candidate's bise is built by BISE_CARGO")


def cand_users():
    """promote-lock: cand_lock.users(dir) finds a process running from under a candidate dir (a copy
    of /bin/sleep there), by the given or the resolved path, and nothing once it ended."""
    import cand_lock as L
    w = tempfile.mkdtemp(prefix="dt-users-")
    p = None
    try:
        # /bin/sleep named as if it ran from the dir (ps shows argv; a copied platform binary is killed)
        os.makedirs(os.path.join(w, "cand", "dist"))
        p = subprocess.Popen([os.path.join(os.path.realpath(w), "cand", "dist", "sleep"), "60"], executable="/bin/sleep")
        d = os.path.join(w, "cand")
        check(wait.until(lambda: p.pid in [u for u, _ in L.users(d)], 10, "the user shows"), "users() finds a process under the dir")
        check(L.users(os.path.join(w, "other")) == [], "nothing under another dir")
        p.kill()
        p.wait()
        check(wait.until(lambda: L.users(d) == [], 10, "no user"), "no user once it ended")
    finally:
        if p and p.poll() is None:
            p.kill()
        shutil.rmtree(w, ignore_errors=True)


def work_checkout_never_stale():
    """fast-try p1 (scripts/try_build.py, review-gates m_29380, architect m_29409): candidates build in
    one checkout moved to each sha. Real git and real cargo on a tiny workspace whose crate
    include_str!s a file outside its package (cargo keeps that path ABSOLUTE in its dep-info, as
    switchboard's prompts): A, then B = A + only that file changed, A's own worktree kept, older
    than A's build: B's binary says B's words (the shape of 11c753ff, one target for worktrees at
    different paths, shipped A's). The first build seeds the target from gate.sh seed-path; out/ never
    survives into the next build, node_modules does; a seed change remakes the checkout."""
    import try_build as B
    if not shutil.which("cargo"):
        print("skip work_checkout_never_stale: no cargo")
        return
    w = tempfile.mkdtemp(prefix="dt-work-")
    try:
        repo, tools, cand, seed = (os.path.join(w, n) for n in ("repo", "tools", "desktop-cand", "seed-k1"))
        for d in ("rust/p/src", "prompts", "apps/desktop"):
            os.makedirs(os.path.join(repo, d))
        files = {"rust/Cargo.toml": '[workspace]\nmembers = ["p"]\nresolver = "2"\n',
                 "rust/p/Cargo.toml": '[package]\nname = "p"\nversion = "0.1.0"\nedition = "2021"\n',
                 "rust/p/src/main.rs": 'fn main() { print!("{}", include_str!("../../../prompts/tone.txt")); }\n',
                 "prompts/tone.txt": "words of A", "apps/desktop/.gitignore": "out/\nnode_modules/\n"}
        for p, s in files.items():
            open(os.path.join(repo, p), "w").write(s)
        git("init", "-q", cwd=repo)
        git("add", "-A", cwd=repo)
        git("-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", "commit", "-qm", "A", cwd=repo)
        a = git("rev-parse", "HEAD", cwd=repo)
        open(os.path.join(repo, "prompts/tone.txt"), "w").write("words of B")
        git("-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", "commit", "-qam", "B", cwd=repo)
        b = git("rev-parse", "HEAD", cwd=repo)
        os.makedirs(os.path.join(tools, "tests"))
        os.makedirs(seed)
        open(os.path.join(seed, "from-seed"), "w").write("warm")
        gate = os.path.join(tools, "tests", "gate.sh")
        open(gate, "w").write("#!/bin/sh\ncat %s\n" % shlex.quote(os.path.join(w, "seed-now")))
        os.chmod(gate, 0o755)
        open(os.path.join(w, "seed-now"), "w").write(seed)
        wk = B.work_paths(cand)
        binary = os.path.join(wk["target"], "debug", "p")

        def build(sha):
            # each candidate has its own worktree (kept, as an unpromoted one is), the build runs in work/
            own = os.path.join(cand, sha[:12], "src")
            if not os.path.isdir(own):
                git("worktree", "add", "-q", "--detach", own, sha, cwd=repo)
            sh = "\n".join(["set -euo pipefail"] + B.work_lines(wk, own, sha, tools=tools)
                           + ["export CARGO_TARGET_DIR=%s" % shlex.quote(wk["target"]), "cd %s/rust && cargo build -q --offline" % shlex.quote(wk["src"])])
            r = subprocess.run(["/usr/bin/lockf", "-k", wk["lock"], "/bin/bash", "-c", sh], capture_output=True, text=True)
            return r, own
        r, own_a = build(a)
        check(r.returncode == 0 and subprocess.run([binary], capture_output=True, text=True).stdout == "words of A"
              and open(os.path.join(wk["target"], "from-seed")).read() == "warm" and "a new build checkout" in r.stdout,
              "A: the work checkout made from the seed, A's words: %s" % (r.stdout + r.stderr))
        # A's own worktree stays, its file older than A's build (what made 11c753ff's cargo find B fresh)
        os.utime(os.path.join(own_a, "prompts", "tone.txt"), (1, 1))
        os.makedirs(os.path.join(wk["src"], "apps", "desktop", "out"))
        open(os.path.join(wk["src"], "apps", "desktop", "out", "stale.js"), "w").write("A's bundle")
        os.makedirs(os.path.join(wk["src"], "apps", "desktop", "node_modules"))
        r, _ = build(b)
        check(r.returncode == 0 and subprocess.run([binary], capture_output=True, text=True).stdout == "words of B",
              "B, only a prompt changed, A's worktree kept: B's words, never A's: %s" % (r.stdout + r.stderr))
        check("a new build checkout" not in r.stdout and os.path.exists(os.path.join(wk["target"], "from-seed")), "the work checkout is kept between candidates")
        check(not os.path.exists(os.path.join(wk["src"], "apps", "desktop", "out")) and os.path.isdir(os.path.join(wk["src"], "apps", "desktop", "node_modules")),
              "nothing the last build made survives but node_modules (architect m_29409)")
        # a killed build's index.lock (review-gates m_29435): the checkout is made again once, the build goes on
        open(os.path.join(git("rev-parse", "--absolute-git-dir", cwd=wk["src"]), "index.lock"), "w").close()
        r, _ = build(a)
        check(r.returncode == 0 and "can't move" in r.stdout and subprocess.run([binary], capture_output=True, text=True).stdout == "words of A",
              "a wedged checkout is made again and the candidate still builds: %s" % (r.stdout + r.stderr))
        open(os.path.join(w, "seed-now"), "w").write("")
        r, _ = build(a)
        check(r.returncode == 0 and "a new build checkout" in r.stdout and "none, a cold target" in r.stdout
              and not os.path.exists(os.path.join(wk["target"], "from-seed"))
              and subprocess.run([binary], capture_output=True, text=True).stdout == "words of A", "another seed (none here): the work checkout is made again")
        line = B.promote_install()
        check("pnpm install --frozen-lockfile" in line and "[ ! -d node_modules ]" in line, "a repack installs the candidate's node_modules when missing")
    finally:
        shutil.rmtree(w, ignore_errors=True)


def team_sign_cases():
    """--candidate <sha> --team-sign (architect m_63935, Gabriel's go m_63930): a copy of a candidate
    that carries the kb appex, signed with his team's identity found by team id; a fake `security`
    and a fake signer, never his keychain. No appex, no identity or the wrong team: it fails, nothing
    signed, no ad hoc fallback; the candidate's own bundle and cand.json stay as built."""
    import desktop_try_team as K
    sha = "ab" * 20
    other = "1) %s \"Developer ID Application: Someone Else (ZZZZZZZZZZ)\"" % ("C" * 40)
    dev = "2) %s \"Apple Development: Gabriel (9G5B2RTD6U)\"" % ("D" * 40)
    ours = "3) %s \"Developer ID Application: Gabriel Vergnaud (9G5B2RTD6U)\"" % ("E" * 40)
    found = "  %s\n  %s\n  %s\n     3 valid identities found\n" % (other, dev, ours)
    check(K.identity_of(found, K.TEAM) == "E" * 40, "the identity is the Developer ID Application one of his team: %s" % K.identity_of(found, K.TEAM))
    check(K.identity_of("  %s\n  %s\n     2 valid identities found\n" % (other, dev), K.TEAM) is None, "no Developer ID of his team: none")
    # the group is the built appex's own (its Info.plist, what kbenv.ts reads; a fresh one when build.sh's
    # KB_GROUP names it, kb-fileprovider m_64056), never recomputed; another team's or none: refused
    group = "9G5B2RTD6U.dev.bise.desktop.cand.kb2"
    fp = lambda g: {"NSExtension": {"NSExtensionFileProviderDocumentGroup": g}}  # noqa: E731
    check(K.kb_group(fp(group), K.TEAM) == group, "the appex's own group: %s" % K.kb_group(fp(group), K.TEAM))
    check(K.kb_group(fp("ZZZZZZZZZZ.dev.bise.desktop.cand.kb"), K.TEAM) is None and K.kb_group(fp("9G5B2RTD6U."), K.TEAM) is None
          and K.kb_group({}, K.TEAM) is None, "another team's group, an empty one or none: refused")
    ent = K.team_entitlements({"com.apple.security.cs.allow-jit": True}, group)
    check(ent == {"com.apple.security.cs.allow-jit": True, K.GROUPS: [group]}, "desktop.plist's keys plus the group: %s" % ent)
    team_argv, adhoc = K.outer_codesign("/c/x.app", "E" * 40, "/c/e.plist"), K.outer_codesign("/c/x.app", "-", "/c/e.plist")
    check(team_argv[team_argv.index("--sign") + 1] == "E" * 40 and "--timestamp" in team_argv and "--options" in team_argv,
          "--team-sign's codesign: his team's identity, hardened, timestamped: %s" % team_argv)
    check(adhoc[adhoc.index("--sign") + 1] == "-" and "--timestamp=none" in adhoc, "without it, the ad hoc command: %s" % adhoc)

    w = tempfile.mkdtemp(prefix="dt-team-")
    try:
        c = T.cand_paths({"cand": w, "arch": "mac-arm64"}, sha)
        os.makedirs(os.path.join(c["app"], "Contents", "Resources", "bin"))
        with open(os.path.join(c["app"], "Contents", "Info.plist"), "wb") as f:
            plistlib.dump({"CFBundleIdentifier": T.CAND_ID}, f)
        open(T.packed_bise(c["app"]), "w").close()
        info = {"sha": sha, "app": c["app"], "root": c["root"], "bise": T.packed_bise(c["app"]), "dir": c["dir"], "team_signed": False}
        with open(c["json"], "w") as f:
            json.dump(info, f)
        own = open(c["json"]).read()
        signed, asked = [], []

        def security(cmd, **kw):
            asked.append(cmd)
            return subprocess.CompletedProcess(cmd, 0, found, "")

        import mac_sign as M
        steps, last = [], {}  # the real inside-out signing (mac_sign.sign_tree), codesign faked: (rel, entitlements)

        def codesign(argv, **kw):
            path = argv[-1]
            ent = argv[argv.index("--entitlements") + 1] if "--entitlements" in argv else None
            with open(ent, "rb") if ent else open(os.devnull, "rb") as f:
                d = plistlib.load(f) if ent else ("preserve" if "--preserve-metadata=entitlements" in argv else None)
            steps.append((os.path.relpath(path, signed[-1][0]), d))
            last[os.path.normpath(path)] = d
            return subprocess.CompletedProcess(argv, 0, "", "")

        def sign(app, identity, entitlements, own, say):
            with open(entitlements, "rb") as f:
                signed.append((app, identity, plistlib.load(f)))
            real = M.subprocess.run
            M.subprocess.run = codesign
            try:
                M.sign_tree(app, identity, None, entitlements, outer=True, own=own)
            finally:
                M.subprocess.run = real
        quiet = lambda *x: None  # noqa: E731
        go = lambda run=security, team="9G5B2RTD6U": K.sign_candidate(info, c, "mac-arm64", T.CAND_NAME, quiet,  # noqa: E731
                                                                     run=run, sign=sign, team_of=lambda a: team, base={"jit": True},
                                                                     entitlements_of=lambda p: last.get(os.path.normpath(p)) if isinstance(last.get(os.path.normpath(p)), dict) else {})
        try:
            go()
            check(False, "a candidate with no appex is refused")
        except T.TryError as e:
            check("bise-kb.appex" in str(e) and not signed and not asked, "no appex: refused before the keychain, nothing signed: %s" % e)
        mach_exec = b"\xcf\xfa\xed\xfe" + b"\0" * 8 + b"\x02\0\0\0" + b"\0" * 48  # a thin MH_EXECUTE header
        for exe in (os.path.join(c["app"], "Contents", "MacOS", T.CAND_NAME), os.path.join(c["app"], K.APPEX, "Contents", "MacOS", "bise-kb")):
            os.makedirs(os.path.dirname(exe), exist_ok=True)
            with open(exe, "wb") as f:
                f.write(mach_exec)
        with open(os.path.join(c["app"], K.APPEX, "Contents", "Info.plist"), "wb") as f:
            plistlib.dump(dict(fp("ZZZZZZZZZZ.dev.bise.desktop.cand.kb"), CFBundleIdentifier=T.CAND_ID + ".kb"), f)
        os.makedirs(os.path.dirname(os.path.join(c["src"], K.KB_ENT)))
        with open(os.path.join(c["src"], K.KB_ENT), "wb") as f:
            plistlib.dump({K.SANDBOX: True, K.GROUPS: ["@GROUP@"]}, f)
        try:
            go()
            check(False, "an appex whose group is not his team's is refused")
        except T.TryError as e:
            check("names no file provider group of team" in str(e) and not signed and not asked, "a group of another team: refused: %s" % e)
        with open(os.path.join(c["app"], K.APPEX, "Contents", "Info.plist"), "wb") as f:
            plistlib.dump(dict(fp(group), CFBundleIdentifier=T.CAND_ID + ".kb"), f)
        os.remove(os.path.join(c["src"], K.KB_ENT))
        try:
            go()
            check(False, "a candidate whose worktree has no kb.entitlements is refused")
        except T.TryError as e:
            check(K.KB_ENT in str(e) and not signed and not asked, "no kb.entitlements: refused before the keychain: %s" % e)
        with open(os.path.join(c["src"], K.KB_ENT), "wb") as f:
            plistlib.dump({K.SANDBOX: True, K.GROUPS: ["@GROUP@"]}, f)
        try:
            go(run=lambda cmd, **kw: subprocess.CompletedProcess(cmd, 0, "     0 valid identities found\n", ""))
            check(False, "no identity of his team: refused")
        except T.TryError as e:
            check("no ad hoc fallback" in str(e) and not signed, "no identity: fails loudly, nothing signed: %s" % e)
        out = go()
        t = K.team_paths(c, "mac-arm64", T.CAND_NAME)
        check(asked == [K.FIND], "the identity is looked up by team in the keychain: %s" % asked)
        check(len(signed) == 1 and signed[0][0] == t["app"] and signed[0][1] == "E" * 40 and signed[0][2] == {"jit": True, K.GROUPS: [group]},
              "the copy is signed with his team's identity and the group: %s" % signed)
        # kb-fileprovider's #5 (dfbcd55b7): fileproviderd refused an appex signed with the app's entitlements.
        # Inside out: the appex (its executable, then the bundle) with kb.entitlements (sandbox + the group,
        # @GROUP@ rewritten), then the app with its own; never the app's file on the appex, never --deep
        appex_ent, app_ent = {K.SANDBOX: True, K.GROUPS: [group]}, {"jit": True, K.GROUPS: [group]}
        order = [rel for rel, _ in steps]
        want = [os.path.join(K.APPEX, "Contents", "MacOS", "bise-kb"), K.APPEX, os.path.join("Contents", "MacOS", T.CAND_NAME), "."]
        check(sorted(order) == sorted(want) and order.index(want[0]) < order.index(want[1]) < order.index(".") == len(order) - 1,
              "inside out: the appex's executable, its bundle, the app last: %s" % order)
        check(dict(steps) == {want[0]: appex_ent, want[1]: appex_ent, want[2]: app_ent, ".": app_ent},
              "the appex gets its own entitlements (sandbox + group), the app its own: %s" % steps)
        try:
            K.sign_candidate(
                info, c, "mac-arm64", T.CAND_NAME, quiet, run=security, sign=sign, team_of=lambda a: "9G5B2RTD6U",
                base={"jit": True}, entitlements_of=lambda p: app_ent)
            check(False, "a signed appex without the sandbox is refused")
        except T.TryError as e:
            check("not sandboxed" in str(e), "the signed appex must say the sandbox and the group: %s" % e)
        check(K.appex_ok(appex_ent, group) and not K.appex_ok(dict(appex_ent, **{"com.apple.security.cs.allow-jit": True}), group)
              and not K.appex_ok({K.SANDBOX: True, K.GROUPS: ["x"]}, group), "appex_ok: sandbox, the group, none of the app's keys")
        del steps[:]
        out = go()
        check(t["app"] != c["app"] and t["app"].startswith(c["dir"] + "/team/") and T.bundle_id(t["app"]) == T.CAND_ID
              and os.path.isdir(os.path.join(t["app"], K.APPEX)), "a copy apart, with the candidate's own id and its appex: %s" % t["app"])
        saved = json.load(open(t["json"]))
        check(saved == out and out["team_signed"] is True and out["app"] == t["app"] and out["bise"] == T.packed_bise(t["app"])
              and out["group"] == group and out["sha"] == sha, "the copy's cand.json: team_signed true, its app and bise: %s" % out)
        check(open(c["json"]).read() == own, "the candidate's own cand.json (what --promote reads) is untouched")
        try:
            go(team=None)
            check(False, "a copy that does not say his team is refused")
        except T.TryError as e:
            check("not 9G5B2RTD6U" in str(e), "the signed copy must say his team: %s" % e)
    finally:
        shutil.rmtree(w, ignore_errors=True)
    for argv in (["--team-sign"], ["--promote", sha, "--team-sign"], ["--try", "--team-sign"], ["--candidate", sha, "--team-sign", "--real"]):
        try:
            T.main(argv)
            check(False, "%s is refused" % argv)
        except T.TryError as e:
            check("--team-sign goes with --candidate" in str(e), "%s: --team-sign with --candidate only: %s" % (argv, e))


if __name__ == "__main__":
    pure()
    team_sign_cases()
    work_checkout_never_stale()
    root_ships_what_a_version_dir_has()
    stamp_skips_build()
    staged_never_over_a_running_try()
    legacy_old_retired()
    real_opens_with_nothing()
    real_stop_never_his_hubs()
    quit_only_this_bundle()
    real_open_fronts_a_running_app()
    real_open_in_background_offscreen()
    candidate_then_promote()
    repack_reads_its_config()
    cand_users()
    build_key_cases()
    if "--pure" not in sys.argv:
        hook_keeps_pid()
        integration()
    print("desktop-try: %s" % ("all pass" if not FAILS else "%d FAIL" % len(FAILS)))
    sys.exit(1 if FAILS else 0)
