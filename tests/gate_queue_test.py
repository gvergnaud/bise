#!/usr/bin/env python3
"""Every gate of an agent goes through heavy-run's queue, one gate a tree
(main m_48101: at 21:55 his Mac sat at load 93/12 on battery; side-resize's
and esc-sheet's quick gates ran tests/gate.sh by hand, outside heavy-run,
and esc-sheet's tree ran two gates at once).

On gate.sh itself, with a fake heavy-run (GATE_HEAVY_RUN) that records what
it got and ends, so no real gate runs:
- a quick from an agent's shell (no HEAVY_SLOT) re-runs itself through
  heavy-run, by its absolute path (heavy-run's light lane still matches it)
- a second gate in a tree whose gate runs refuses at once, naming the first,
  and never reaches heavy-run; in a slot too (HEAVY_SLOT set)
- a land check (the hub's land queue) and GATE_NO_HEAVY=1 take neither
"""
import os
import subprocess
import sys
import tempfile

import wait

HERE = os.path.dirname(os.path.abspath(__file__))
GATE = os.path.join(HERE, "gate.sh")
TREE = os.path.realpath(os.path.join(HERE, ".."))
fails = []


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what)
    if not ok:
        fails.append(what)


def seed_carries_deps_only(tmp, senv):
    """A seed built in tree A, cloned into tree B whose sources are older than
    the seed's outputs (the nightly tree: git rewrites only the changed files),
    must not hand B A's build of the repo's own crates: cargo would reuse it
    with A's CARGO_MANIFEST_DIR baked in, and once A is gone a test that reads
    a fixture through it fails (bise-peer's agent_key fixture, the
    desktop-v2026.10.21 tip gate, marketing m_48661). A real cargo build of a
    one-crate workspace, no deps."""
    a, b = os.path.join(tmp, "tree-a"), os.path.join(tmp, "tree-b")
    home = os.path.join(tmp, "cargohome-seed")
    seed_dir = os.path.join(home, "cache", "gate-seed", "k2")
    crate = os.path.join(a, "rust", "wsx")
    os.makedirs(os.path.join(crate, "src"))
    open(os.path.join(a, "rust", "Cargo.toml"), "w").write('[workspace]\nmembers = ["wsx"]\nresolver = "2"\n')
    open(os.path.join(crate, "Cargo.toml"), "w").write('[package]\nname = "wsx"\nversion = "0.1.0"\nedition = "2021"\n')
    open(os.path.join(crate, "fix.txt"), "w").write("here")
    open(os.path.join(crate, "src", "lib.rs"), "w").write(
        '#[test]\nfn reads_its_fixture() {\n'
        '    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fix.txt");\n'
        '    assert_eq!(std::fs::read_to_string(&p).expect("fixture"), "here");\n}\n')
    cenv = dict(senv, CARGO_TARGET_DIR=seed_dir)
    r = subprocess.run(["cargo", "test", "--offline", "-q", "--no-run"], cwd=os.path.join(a, "rust"), env=cenv,
                       capture_output=True, text=True, timeout=300)
    if r.returncode != 0:
        check(False, "seed fixture: cargo test --no-run in tree A: %r" % r.stderr[-300:])
        return
    subprocess.run("git init -q && git add -A && git -c user.email=t@t -c user.name=t -c commit.gpgsign=false commit -qm i",
                   shell=True, cwd=a, check=True)
    subprocess.run(["cp", "-Rp", a, b], check=True)  # B's sources keep A's mtimes, older than the seed
    subprocess.run(["rm", "-rf", a], check=True)
    r = subprocess.run(["bash", GATE, "seed"], cwd=b, env=dict(senv, BISE_HOME=home), capture_output=True, text=True, timeout=60)
    check(r.returncode == 0 and os.path.isdir(os.path.join(b, "rust", "target", "debug")),
          "seed fixture: tree B's rust/target is a clone of A's seed: %r" % r.stdout)
    r = subprocess.run(["cargo", "test", "--offline", "-q"], cwd=os.path.join(b, "rust"), env=senv,
                       capture_output=True, text=True, timeout=300)
    check(r.returncode == 0, "a seeded tree rebuilds the repo's own crates: its test reads B's fixture, not deleted A's: %r"
          % (r.stdout + r.stderr)[-400:])
    s = os.listdir(os.path.join(seed_dir, "debug", ".fingerprint"))
    check(any(f.startswith("wsx-") for f in s), "the seed itself is left as it is (its own tree may still use it)")


def proof_run_reaps_its_own(tmp):
    """tests/proof_run.py (the PROOF step) reaps its bends with os.wait4(-1)
    for their CPU time; a Popen it dropped while its bend ran was reaped by
    subprocess behind its back (Popen.__del__, then the next Popen's cleanup),
    so wait4 met no child: 'ChildProcessError: no child processes' under load
    (mcp-apps-bridge, m_50209). A fake bend that ends at once makes that race
    the rule: 200 proofs files, 4 at a time (40 met it 1 run in 3)."""
    root, out, fbin = (os.path.join(tmp, "proof-" + d) for d in ("root", "out", "bin"))
    os.makedirs(os.path.join(root, "bend", "proofs"))
    os.makedirs(fbin)
    for i in range(200):
        open(os.path.join(root, "bend", "proofs", "p%02d.bend" % i), "w").write("x" * (i + 1))
    with open(os.path.join(fbin, "bend"), "w") as f:
        f.write("#!/bin/sh\necho ALL PROOFS CHECK\n")
    os.chmod(os.path.join(fbin, "bend"), 0o755)
    e = dict(os.environ, PATH=fbin + os.pathsep + os.environ["PATH"])
    r = subprocess.run([sys.executable, os.path.join(HERE, "proof_run.py"), root, "4", out], env=e,
                       capture_output=True, text=True, timeout=120)
    check(r.returncode == 0 and "ok   PROOF" in r.stdout and r.stdout.count("ok   bend/proofs/") == 200,
          "proof_run.py reaps each of its 200 bends itself, none taken by subprocess: %r"
          % (r.stdout[-200:] + r.stderr[-300:]))


def desktop_check_quick_only():
    """A quick's whole-app desktop_check runs check:quick: check with test:quick
    (the unit project) for test and no build (quick builds nothing,
    c3-quick-90s: the full gate, the tip and a release build), the same plain
    lint; every other gate runs check. The build's views:check (kit/views,
    committed, fresh) runs in both of quick's desktop paths: a stale web.css
    passed a quick at 371fd9510 once the build was out of it."""
    import json
    s = json.load(open(os.path.join(HERE, "..", "apps", "desktop", "package.json")))["scripts"]
    want = s["check"].replace("pnpm test ", "pnpm test:quick ").replace("pnpm typecheck && ", "pnpm typecheck && pnpm views:check && ")
    check(want.endswith(" && pnpm build") and s.get("check:quick") == want[:-len(" && pnpm build")]
          and s.get("test:quick") == s["test"] + " --project unit",
          "check:quick is check with views:check, test:quick (unit, no bims-laws) for test, without its build: %r"
          % s.get("check:quick"))
    check(s.get("build", "").endswith(" && pnpm views:check") and s.get("views:check", "").endswith("scripts/views-web.mts --check"),
          "the build ends with views:check (scripts/views-web.mts --check): %r" % s.get("build"))
    src = open(GATE).read()
    check("pnpm -s typecheck && pnpm -s views:check" in src,
          "gate.sh: quick's by-path desktop_check runs views:check after typecheck")
    # a change that reaches both: desktop_check next to the Rust tests, not after them (main's
    # 11-10 audit: a Rust + desktop quick was their sum, p50 347 s)
    start = src.find('{ desktop_check >"$out/desktop.res" 2>&1; echo $? >"$out/desktop.rc"; } &')
    tests = src.find('step "test$(echo "$pkgs"')
    check(0 < start < tests and 'if [ -n "$dwhy" ] && [ -n "$pkgs" ]; then' in src
          and 'wait "$desk_pid"; cat "$out/desktop.res"' in src,
          "gate.sh: with Rust and desktop changes, desktop_check starts before the Rust tests, its result read after")
    # and a red Rust step leaves none of it: on_exit lists the run's tree before it kills the jobs
    # (killed first, the background job's pnpm and eslint were orphans no longer under $$)
    d, j = src.find('doomed="$(gate_procs "$(gate_mark "$run")" $$ $wt_bins)"'), src.find("  kill $(jobs -p) 2>/dev/null")
    check(0 < d < j and 'gate_kill "$doomed"' in src,
          "gate.sh on_exit: the run's process tree listed before kill $(jobs -p), then killed")
    uses = [l.strip() for l in src.splitlines() if "check:quick" in l and not l.strip().startswith("#")]
    check(len(uses) == 1 and '[ "$mode" = quick ] && [ "${1:-}" != strict ] && echo check:quick || echo check' in uses[0],
          "gate.sh: check:quick only in a quick's non-strict desktop_check, `check` everywhere else: %r" % uses)


def main():
    tmp = tempfile.mkdtemp(prefix="gate-queue-")
    rec = os.path.join(tmp, "heavy-run.args")
    fake = os.path.join(tmp, "heavy-run")
    with open(fake, "w") as f:
        f.write('#!/bin/bash\necho "slot=${HEAVY_SLOT:-} locked=${GATE_TREE_LOCKED:-} $*" >> %s\n' % rec)
    os.chmod(fake, 0o755)
    env = {k: v for k, v in os.environ.items()
           if k not in ("HEAVY_SLOT", "GATE_NO_HEAVY", "GATE_TREE_LOCKED", "BISE_GATE_RUN", "BISE_GATE_START")}
    env.update(BISE_HOME=os.path.join(tmp, "home"), GATE_HEAVY_RUN=fake)

    def gate(*args, **extra):
        e = dict(env, **extra)
        return subprocess.run(["bash", GATE] + list(args), cwd=TREE, env=e, capture_output=True, text=True, timeout=120)

    def calls():
        return open(rec).read().splitlines() if os.path.exists(rec) else []

    # 1. an agent's quick: through heavy-run, by its absolute path, under the tree lock
    r = gate("quick")
    c = calls()
    check(r.returncode == 0 and len(c) == 1 and c[0].endswith("%s quick" % GATE) and c[0].startswith("slot= locked=%s " % TREE),
          "a quick from an agent's shell goes through heavy-run, absolute path, tree locked: %r %r" % (c, r.stdout[-200:]))
    check("goes through heavy-run's queue" in r.stdout, "it says so: %r" % r.stdout[-200:])
    # 2. a gate already runs in this tree: the next one refuses at once
    locks = os.path.join(tmp, "home", "gate", "locks")
    tl = [os.path.join(locks, f) for f in os.listdir(locks) if f.startswith("tree-") and f.endswith(".lock")]
    check(len(tl) == 1, "one lock file for this tree: %r" % tl)
    go = os.path.join(tmp, "go")
    holder = subprocess.Popen(["/usr/bin/lockf", "-k", tl[0], "bash", "-c", 'while [ ! -e "$0" ]; do sleep 0.1; done', go])
    try:
        wait.until(lambda: subprocess.run(["/usr/bin/lockf", "-k", "-t", "0", tl[0], "true"], capture_output=True).returncode != 0,
                   20, "the holder takes the tree lock")
        open(tl[0] + ".holder", "w").write("pid %d quick since now\n" % holder.pid)
        n = len(calls())
        r = gate("quick")
        check(r.returncode == 1 and "a gate already runs in this tree (pid %d quick since now)" % holder.pid in r.stdout
              and len(calls()) == n, "a second gate in the tree refuses at once, naming the first: %r" % r.stdout[-300:])
        r = gate("quick", HEAVY_SLOT="heavy 1")
        check(r.returncode == 1 and "one gate a tree" in r.stdout, "in a slot too: %r" % r.stdout[-200:])
        # 3. a land check and GATE_NO_HEAVY take neither the lock nor the queue
        r = gate("land")
        check(r.returncode == 2 and "usage" in r.stderr and len(calls()) == n, "a land check: neither (its usage, rc 2): %r" % r.stderr[-200:])
    finally:
        open(go, "w").close()
        holder.wait(timeout=20)
    # the lock is free again once the first gate ends (-k keeps the file, the
    # release frees it): the next gate goes through (architect m_48124)
    n = len(calls())
    r = gate("quick")
    check(r.returncode == 0 and len(calls()) == n + 1 and os.path.exists(tl[0]),
          "after the first gate ends, the next one goes through, the lock file kept: %r" % r.stdout[-200:])
    check(not [f for f in os.listdir(locks) if ".started." in f], "no start mark left behind")
    # 5. gate.sh seed (bise.toml [worktree] setup): a new worktree's rust/target
    # starts as a clone of the warm seed; never fails the spawn (architect m_48124)
    repo, home = os.path.join(tmp, "repo"), os.path.join(tmp, "seedhome")
    os.makedirs(os.path.join(repo, "rust"))
    os.makedirs(os.path.join(home, "cache", "gate-seed", "k1", "debug"))
    open(os.path.join(home, "cache", "gate-seed", "k1", "debug", "marker"), "w").write("warm")
    open(os.path.join(repo, "rust", "Cargo.lock"), "w").write("")
    subprocess.run("git init -q && git add -A && git -c user.email=t@t -c user.name=t -c commit.gpgsign=false commit -qm i",
                   shell=True, cwd=repo, check=True)
    senv = {k: v for k, v in env.items() if k != "CARGO_TARGET_DIR"}

    def seed(**extra):
        e = dict(senv, BISE_HOME=home)
        e.update(extra)
        return subprocess.run(["bash", GATE, "seed"], cwd=repo, env=e,
                              capture_output=True, text=True, timeout=60)
    r = seed()
    check(r.returncode == 0 and open(os.path.join(repo, "rust", "target", "debug", "marker")).read() == "warm",
          "seed: rust/target is a clone of the warm seed: %r" % r.stdout)
    r = seed()
    check(r.returncode == 0 and "nothing to do" in r.stdout, "seed again: the target is kept, rc 0: %r" % r.stdout)
    subprocess.run(["rm", "-rf", os.path.join(repo, "rust", "target")])
    r = seed(BISE_HOME=os.path.join(tmp, "none"))
    check(r.returncode == 0 and "builds cold" in r.stdout and not os.path.exists(os.path.join(repo, "rust", "target")),
          "no seed: a line, rc 0, no target: %r" % r.stdout)
    r = seed(CARGO_TARGET_DIR=os.path.join(tmp, "elsewhere"))
    check(r.returncode == 0 and not os.path.exists(os.path.join(repo, "rust", "target")), "CARGO_TARGET_DIR set: nothing, rc 0")
    toml = open(os.path.join(HERE, "..", "bise.toml")).read()
    check('setup = "tests/gate.sh seed"' in toml, "bise.toml: [worktree] setup runs gate.sh seed")
    seed_carries_deps_only(tmp, senv)
    proof_run_reaps_its_own(tmp)
    desktop_check_quick_only()
    # 4. the wiring a test can't run whole (a real gate): the nested gate and GATE_NO_HEAVY skip both
    src = open(GATE).read()
    check('if [ "$mode" != land ] && [ -z "$gate_outer" ] && [ -z "${GATE_NO_HEAVY:-}" ]; then' in src,
          "gate.sh: a gate under a live gate, a land check and GATE_NO_HEAVY skip the queue and the lock")
    # 5. no bare `wait`: bash 5.2's waits on the log's `exec > >(tee ...)` too,
    # which end only with the gate, so it never returns (stop-not-crash's quick
    # on a .bend change: every step done, then 18 min in the PROOF wait). Each
    # wait names its pid.
    import re
    bare = [n for n, line in enumerate(src.splitlines(), 1)
            if re.search(r"(^\s*|[;&|{(]\s*|\bthen\s+|\bdo\s+)wait\s*($|;|&&|\|\||\))", line.split("#")[0])]
    check(not bare, "gate.sh: no bare wait (it waits on the log's tees and hangs): lines %r" % bare)
    subprocess.run(["rm", "-rf", tmp])
    if fails:
        print("gate_queue_test: FAIL (%d)" % len(fails))
        sys.exit(1)
    print("gate_queue_test: ok (an agent's gate takes heavy-run's queue, one gate a tree)")


if __name__ == "__main__":
    main()
