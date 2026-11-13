#!/usr/bin/env python3
"""tip_check: main checked after each land (c5-tip-per-land, audit-tests finding 1; architect
m_55196; Gabriel « du moment que tu fais attention a mon CPU »).

  scripts/tip_check.py <base> <tip> <agent>   bise.toml's [land] after: the hub runs it after a
                                              land that moved main, as main's process (listed,
                                              stoppable, stopped when the hub quits)
  scripts/tip_check.py                        the same for main's tip now (by hand)

The quick gate (tests/gate.sh quick, GATE_BASE = the tip checked last) on main's tip, in its own
tree. One runner on the machine (a lock: a second start exits at once; the runner takes main's
newest tip each round, so close lands share one run over their range). His CPU first:
  - at most one run per SPACING (10 min) from the last run's start; past CPU_BOUND (60) CPU-minutes
    today, the spacing doubles per bound (60: 20 min, 120: 40 min ...);
  - heavy-run --low (rank 3, behind every agent's job) at nice 15 (heavy-run's 10, + 5 here);
  - never on battery: it waits for AC (pmset), the backlog stays one run;
  - the quick gate only: never a test the quick gate doesn't run;
  - red (architect m_63969): only the red steps run again (a cargo test's 'to rerun pass' args, a
    self-test's own command; the quick gate only for a step with none, like desktop_check). The
    range's commits whose own diff reaches a red step (tests/gate_plan.py on that commit's files:
    its crates, the crates using them, its flags, self-tests and desktop): exactly one is
    confirmed on itself, red: tip-red on it; else the red steps by halves over the range. Past
    CPU_BOUND today the halving stops: tip-red with candidates=<shas> (the diff-named commits,
    else the range), main decides. Red, then green on the same commit: flaky-open.
Its tree: ~/.bise/run/tip-check/tree/<repo> (a worktree, never an agent's; rust/target cloned
from the warm seed: tests/gate.sh seed). No checked tip yet: the newest land's range only (its
<base>, else tip^), never main's history.
Out: <BISE_HOME>/gate/tip-verdicts.tsv, one line a run, next to green-shas.tsv:
  <tip> <base> green|red|infra <start epoch> <secs> <cpu secs> <commits> <log> [<named red sha>]
scripts/loop_audit.py sums the CPU per day from it. A red writes ~/.bise/run/nightly/tip-red
(scripts/tip_blame.py: the named commit and its agent), which main forwards, as the tip gate's.
Tests: tests/tip_check_test.py (pure parts, then a fake repo, gate, heavy-run and power)."""
import fcntl
import json
import os
import re
import resource
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
SPACING = 600
CPU_BOUND = 60 * 60  # CPU seconds a day before the spacing doubles
POWER_POLL = 60
POWER_MAX = 12 * 3600
NICE_MORE = 5  # on top of heavy-run's nice 10 on AC: 15


# ---- pure part -------------------------------------------------------------

def parse_verdicts(text):
    """[dict] of tip-verdicts.tsv's lines, oldest first; a short line is skipped"""
    out = []
    for l in text.splitlines():
        f = l.split("\t")
        if len(f) < 8 or not f[3].isdigit():
            continue
        out.append({"tip": f[0], "base": f[1], "state": f[2], "start": int(f[3]), "secs": int(float(f[4] or 0)),
                    "cpu": float(f[5] or 0), "commits": int(f[6] or 0), "log": f[7], "named": f[8] if len(f) > 8 else "",
                    "e2e": f[9] if len(f) > 9 else "", "ahd": f[10] if len(f) > 10 else ""})
    return out


def verdict_line(v):
    return "\t".join(str(x) for x in (v["tip"], v["base"], v["state"], v["start"], v["secs"], round(v["cpu"], 1),
                                       v["commits"], v["log"], v.get("named", ""), v.get("e2e", ""), v.get("ahd", ""))) + "\n"


def day_of(epoch):
    return time.strftime("%Y-%m-%d", time.localtime(epoch))


def cpu_today(verdicts, now):
    """CPU seconds of today's runs (local day)"""
    d = day_of(now)
    return sum(v["cpu"] for v in verdicts if day_of(v["start"]) == d)


def spacing(cpu_s, base=SPACING, bound=CPU_BOUND):
    """seconds between two runs' starts: base, doubled per bound of CPU spent today"""
    return base * 2 ** int(cpu_s // bound) if bound > 0 else base


def plan(tip, verdicts, hook_base, now, is_ancestor, base_spacing=SPACING, bound=CPU_BOUND):
    """(base, wait seconds) for a run on `tip`, or None when it has a verdict. base: the newest
    checked tip that is an ancestor of tip (a run on a sha main left is no base), else the newest
    land's base (the hook's), else None (tip^: the caller's); wait: until the last run's start +
    the spacing (doubled past the CPU bound)."""
    if any(v["tip"] == tip for v in verdicts):
        return None
    base = next((v["tip"] for v in reversed(verdicts) if v["state"] != "infra" and is_ancestor(v["tip"], tip)), None) or hook_base
    last = max((v["start"] for v in verdicts), default=None)
    gap = spacing(cpu_today(verdicts, now), base_spacing, bound)
    return base, 0 if last is None else max(0, last + gap - now)


def first_red(commits, red):
    """the first commit of `commits` (oldest first, the last one known red) that is red, by
    halves: red(c) runs the check on c. Assumes a red stays red after it (one break)."""
    return halve(commits, red, lambda: False)[0]


# the e2e/tmux rows after a green quick (release-speed plan B1, architect m_63540): at most this many
E2E_MAX = 8


def e2e_picks(select_out, tests, cap=E2E_MAX):
    """([tests] to run, None), or ([], 'none:<why>'): test_map.py select's stdout for the range,
    against run_all.sh's tests. ALL or over the cap: none (the nightly and a release run them). Pure."""
    s = (select_out or "").strip()
    if s.startswith("ALL"):
        return [], "none:ALL %s (the nightly and a release run every test)" % s[3:].strip()
    picks = [t for t in s.split() if t in tests]
    if not picks:
        return [], "none:no e2e/tmux test picked"
    if len(picks) > cap:
        return [], "none:%d picked, over the cap %d (the nightly and a release run them)" % (len(picks), cap)
    return picks, None


# a red quick gate (architect m_63969): only its red steps run again, on the named commit to confirm
# it and on the range's commits to halve it, never the whole quick gate unless a red step has no
# command of its own (desktop_check, clippy, PROOF...). A gate.sh step's plan flag (tests/gate_plan.py):
STEP_FLAG = {"proto_ts": "proto_ts", "mcp_apps_plugins": "mcpapps", "office_skills": "office_skills",
             "test_crossterm": "crossterm_changed", "gallery_plugins": "gallery", "office_check": "office",
             "kit": "kit", "computer_use": "cu", "ambient_qa": "aqa", "core_gen_laws": "core_gen"}
# the gate.sh steps a recheck runs alone, gate.sh's own command (tests/tip_check_test.py checks
# each one is still gate.sh's); a self-test's comes from gate_plan's SELFTESTS
STEP_CMD = {"proto_ts": "cd rust && cargo test --offline -q -p bise-proto --features ts --test ts",
            "mcp_apps_plugins": "python3 tests/fixtures/mcp-apps/check.py",
            "office_skills": "python3 tests/office_skills_check.py",
            "test_crossterm": "cd rust && cargo test --offline -q --manifest-path vendor/crossterm/Cargo.toml --lib event::sys::unix::parse"}


def red_steps(text):
    """The last quick run's red steps in a tip-check log: (the FAIL step names, cargo's rerun args
    like '-p switchboard --lib' from its 'to rerun pass' lines). Pure."""
    sec = (text or "").rsplit("\n== ", 1)[-1]
    steps = list(dict.fromkeys(re.split(r" \(|:", l[5:], 1)[0].strip() for l in sec.splitlines() if l.startswith("FAIL ")))
    cargo = list(dict.fromkeys(re.findall(r"to rerun pass `([^`]+)`", sec)))
    return steps, cargo


def is_cargo(step, selfnames):
    """gate.sh's cargo test step: 'test<crate>_<crate>...'. Pure."""
    return step.startswith("test") and len(step) > 4 and step not in ("test-build", "test_crossterm") \
        and step not in selfnames and step not in STEP_FLAG


def step_crates(step, cargo):
    """the crates of a red cargo step: its rerun lines' -p, else its name's. Pure."""
    return [m for a in cargo for m in re.findall(r"-p (\S+)", a)] or [c for c in step[4:].split("_") if c]


def recheck_cmds(steps, cargo, selfcmds):
    """The shell commands (from the repo root) that run only these red steps again, or None when one
    has no command of its own (the caller runs the quick gate). selfcmds: gate_plan's SELFTESTS
    {name: cmd}. Pure."""
    out = []
    for s in steps:
        if s in selfcmds:
            out.append(selfcmds[s])
        elif s in STEP_CMD:
            out.append(STEP_CMD[s])
        elif is_cargo(s, selfcmds):
            out += ["cd rust && cargo test --offline -q " + a for a in cargo] \
                or ["cd rust && cargo test --offline -q " + " ".join("-p " + c for c in step_crates(s, []))]
        else:
            return None
    return list(dict.fromkeys(out)) or None


def reaches(steps, cargo, p, selfnames):
    """Whether a commit's own gate plan p (tests/gate_plan.py's JSON for its diff) runs one of these
    red steps: a red cargo step's crates in its pkgs (gate_plan's pkgs already hold the crates using
    a changed one: a dependency-only commit reaches them), desktop_check with its desktop, a red
    self-test among its selftests, a flag step with its flag; clippy and test-build with any crate.
    A step it can't map (shims_check, a FAIL with no step name) reaches no commit. Pure."""
    p = p or {}
    pkgs = set(p.get("pkgs") or [])
    for s in steps:
        if s == "desktop_check" and p.get("desktop"):
            return True
        if s in selfnames:
            if s in (p.get("selftests") or []):
                return True
        elif s in STEP_FLAG:
            f = STEP_FLAG[s]
            if (p.get("flags") or {}).get(f) or p.get(f):
                return True
        elif s in ("clippy", "test-build"):
            if pkgs:
                return True
        elif is_cargo(s, selfnames) and set(step_crates(s, cargo)) & pkgs:
            return True
    return False


def named_by_diff(steps, cargo, commit_plans, selfnames):
    """The commits, oldest first, whose own diff reaches a red step (architect m_63969).
    commit_plans: [(sha, gate plan or None)]; a commit with no plan is never named. Pure."""
    return [c for c, p in commit_plans if p is not None and reaches(steps, cargo, p, selfnames)]


def halve(commits, red, over):
    """(the first red commit, None) by first_red's halving, or (None, the window still open) when it
    stops before a check once over() (the day's CPU bound). commits oldest first, the last one known
    red."""
    lo, hi = 0, len(commits) - 1
    while lo < hi:
        if over():
            return None, commits[lo:hi + 1]
        mid = (lo + hi) // 2
        if red(commits[mid]):
            hi = mid
        else:
            lo = mid + 1
    return (commits[hi] if commits else None), None


def candidates(red, commit_picks):
    """The commits, oldest first, whose own diff selects a red test (architect m_63540): one is named,
    several are candidates main decides between, none (only ALL picked it) is a flake to own, never a
    guess. commit_picks: [(sha, set of tests, or None for ALL)]. Pure."""
    return [c for c, p in commit_picks if p and set(red) & set(p)]


# ---- the shell --------------------------------------------------------------

def git(repo, *a):
    r = subprocess.run(["git", "-C", repo, *a], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else None


def on_ac():
    forced = os.environ.get("TIP_CHECK_POWER")
    if forced:
        return forced == "ac"
    out = subprocess.run(["pmset", "-g", "batt"], capture_output=True, text=True).stdout
    return "AC Power" in out or "Battery Power" not in out


def tree_for(home, main_tree):
    """the check's own worktree at ~/.bise/run/tip-check/tree/<repo>, made once (its target from
    the warm seed), or None"""
    repo = os.path.basename(main_tree.rstrip("/")) or "repo"
    wt = os.path.join(home, "run", "tip-check", "tree", repo)
    if not os.path.exists(os.path.join(wt, ".git")):
        subprocess.run(["git", "-C", main_tree, "worktree", "prune"], capture_output=True)
        os.makedirs(os.path.dirname(wt), exist_ok=True)
        with open(os.path.join(os.path.dirname(wt), "owner"), "w") as f:
            f.write("tip-check\n")
        if subprocess.run(["git", "-C", main_tree, "worktree", "add", "-q", "--detach", wt, "main"], capture_output=True).returncode:
            return None
        subprocess.run(["bash", "tests/gate.sh", "seed"], cwd=wt, capture_output=True)
    return wt


def quick(home, wt, sha, base, log):
    """(green, secs, cpu secs) of the quick gate on sha with GATE_BASE=base, through heavy-run --low"""
    if subprocess.run(["git", "-C", wt, "checkout", "-q", "-f", "--detach", sha], capture_output=True).returncode:
        return None, 0, 0.0
    subprocess.run(["git", "-C", wt, "clean", "-qfd"], capture_output=True)
    hr = os.environ.get("TIP_CHECK_HEAVY_RUN") or os.path.join(home, "run", "heavy-run")
    env = dict(os.environ, GATE_BASE=base, SB_AGENT="tip-check")
    for k in ("SB_SOCKET", "CARGO_TARGET_DIR", "HEAVY_SLOT"):
        env.pop(k, None)
    t0, c0 = time.time(), resource.getrusage(resource.RUSAGE_CHILDREN)
    with open(log, "a") as out:
        out.write("== %s quick on %s, GATE_BASE=%s\n" % (time.strftime("%F %T"), sha[:12], base[:12]))
        out.flush()
        rc = subprocess.run([hr, "--low", "nice", "-n", str(NICE_MORE), "bash", "tests/gate.sh", "quick"],
                            cwd=wt, env=env, stdout=out, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL).returncode
    c1 = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu = (c1.ru_utime - c0.ru_utime) + (c1.ru_stime - c0.ru_stime)
    return rc == 0, int(time.time() - t0), cpu


def gate_plan_in(wt, paths):
    """tests/gate_plan.py's plan (a dict) in that tree for those paths, {} for none, None when it fails"""
    if not paths:
        return {}
    r = subprocess.run([sys.executable, "-B", "tests/gate_plan.py", "--", *paths], cwd=wt, capture_output=True, text=True)
    try:
        return json.loads(r.stdout) if r.returncode == 0 else None
    except ValueError:
        return None


def self_cmds(wt):
    """gate_plan's SELFTESTS {name: cmd} in that tree's checkout, {} when it can't be read"""
    r = subprocess.run([sys.executable, "-B", "-c", "import json, sys; sys.path.insert(0, 'tests'); import gate_plan; "
                        "print(json.dumps({n: c for n, c, _ in gate_plan.SELFTESTS}))"], cwd=wt, capture_output=True, text=True)
    try:
        return json.loads(r.stdout) if r.returncode == 0 else {}
    except ValueError:
        return {}


def recheck(home, wt, sha, base, log, steps, cargo):
    """(green, secs, cpu secs) of only these red steps on sha (recheck_cmds), through heavy-run --low
    at nice 15 like the quick gate, with the gate's cargo target (the tree's <dir>/target) and the
    tests' sb-core; the quick gate when a step has no command of its own"""
    if subprocess.run(["git", "-C", wt, "checkout", "-q", "-f", "--detach", sha], capture_output=True).returncode:
        return None, 0, 0.0
    subprocess.run(["git", "-C", wt, "clean", "-qfd"], capture_output=True)
    cmds = recheck_cmds(steps, cargo, self_cmds(wt))
    if cmds is None:
        return quick(home, wt, sha, base, log)
    hr = os.environ.get("TIP_CHECK_HEAVY_RUN") or os.path.join(home, "run", "heavy-run")
    env = dict(os.environ, GATE_BASE=base, SB_AGENT="tip-check")
    for k in ("SB_SOCKET", "CARGO_TARGET_DIR", "HEAVY_SLOT"):
        env.pop(k, None)
    tdir = os.path.join(os.path.dirname(wt), "target")
    if os.path.exists(os.path.join(os.path.dirname(wt), "owner")) and os.path.isdir(tdir):
        env["CARGO_TARGET_DIR"] = tdir  # gate.sh's for a tree with an owner file
    env.setdefault("CARGO_BUILD_JOBS", str(((os.cpu_count() or 2) + 1) // 2))
    # one line (heavy.log and heavy-run's reserve file show the command)
    script = 'rc=0; '
    if any("cargo test" in c for c in cmds):
        script += '{ SB_CORE_BIN="$(scripts/bins.sh path sb-core 2>/dev/null)" && export SB_CORE_BIN || unset SB_CORE_BIN; }; '
    script += "".join("(%s) || { echo 'FAIL %s'; rc=1; }; " % (c, c.replace("'", "")) for c in cmds) + "exit $rc"
    t0, c0 = time.time(), resource.getrusage(resource.RUSAGE_CHILDREN)
    with open(log, "a") as out:
        out.write("== %s recheck on %s: %s\n" % (time.strftime("%F %T"), sha[:12], " ".join(steps)))
        out.flush()
        rc = subprocess.run([hr, "--low", "nice", "-n", str(NICE_MORE), "bash", "-c", script],
                            cwd=wt, env=env, stdout=out, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL).returncode
    c1 = resource.getrusage(resource.RUSAGE_CHILDREN)
    return rc == 0, int(time.time() - t0), (c1.ru_utime - c0.ru_utime) + (c1.ru_stime - c0.ru_stime)


def run_all_tests(wt):
    """run_all.sh's e2e/tmux tests in that tree"""
    try:
        s = open(os.path.join(wt, "tests", "run_all.sh")).read()
        return s.split('TESTS="', 1)[1].split('"', 1)[0].split()
    except (OSError, IndexError):
        return []


def select_in(wt, paths):
    """test_map.py select's stdout in that tree for those paths ('ALL <why>' or the tests)"""
    if not paths:
        return ""
    r = subprocess.run([sys.executable, "-B", "tests/test_map.py", "select", *paths], cwd=wt, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else "ALL test_map.py failed"


def e2e(home, wt, tests, log):
    """(green, secs, cpu secs, red tests) of those e2e/tmux tests on the tree's checkout: what run_all
    needs built (as gate.sh full: sb-core and the scripted repls through bins.sh, bise), then
    run_all.sh --only, through heavy-run --low at nice 15 like the quick gate"""
    hr = os.environ.get("TIP_CHECK_HEAVY_RUN") or os.path.join(home, "run", "heavy-run")
    red = tempfile.NamedTemporaryFile("w", suffix=".red", delete=False)
    red.close()
    env = dict(os.environ, SB_AGENT="tip-check", SB_RED_FILE=red.name, SB_RUN_KIND="tip-check-e2e")
    for k in ("SB_SOCKET", "CARGO_TARGET_DIR", "HEAVY_SLOT"):
        env.pop(k, None)
    # TIP_CHECK_E2E_SCRIPT: the tests' fake build and run ($1 the tests)
    script = os.environ.get("TIP_CHECK_E2E_SCRIPT") or (
        'scripts/bins.sh sb-core repl-live repl-scripted && (cd rust && cargo build -q -p bend-harness) '
        '&& tests/run_all.sh --only "$1"')
    t0, c0 = time.time(), resource.getrusage(resource.RUSAGE_CHILDREN)
    with open(log, "a") as out:
        out.write("== %s e2e/tmux on %s: %s\n" % (time.strftime("%F %T"), git(wt, "rev-parse", "--short", "HEAD"), " ".join(tests)))
        out.flush()
        rc = subprocess.run([hr, "--low", "nice", "-n", str(NICE_MORE), "bash", "-c", script, "_", " ".join(tests)],
                            cwd=wt, env=env, stdout=out, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL).returncode
    c1 = resource.getrusage(resource.RUSAGE_CHILDREN)
    try:
        reds = [t for t in open(red.name).read().split() if t in tests]
    finally:
        os.unlink(red.name)
    return rc == 0, int(time.time() - t0), (c1.ru_utime - c0.ru_utime) + (c1.ru_stime - c0.ru_stime), reds


def save_seed(wt, log):
    """main's tip is green here: its target becomes the warm seed of its deps when they have none yet
    (a land changed Cargo.lock or a Cargo.toml), so new task targets clone their deps warm instead of
    an older seed's (disk-hygiene, architect m_55867: deps only, one seed kept; gate.sh seed-save)"""
    for t in (os.path.join(wt, "rust", "target"), os.path.join(os.path.dirname(wt), "target")):
        if os.path.exists(os.path.join(t, ".rustc_info.json")):  # a built target, never one disk_reclaim emptied
            with open(log, "a") as out:
                subprocess.run(["bash", "tests/gate.sh", "seed-save", t], cwd=wt, stdout=out, stderr=subprocess.STDOUT,
                               stdin=subprocess.DEVNULL)
            return


def blame_candidates(home, main_tree, tip, cands, red, log, agent_journals, what="tip check (e2e)",
                     why="each one's own diff picks a red test"):
    """tip-red naming several commits (each one's own diff picks a red e2e test, or the quick gate's
    halving stopped at the CPU bound): one line with a typed candidates=<shas> field, main decides
    (architect m_63540, m_63969)"""
    sys.path.insert(0, HERE)
    import tip_blame
    own = tip_blame.owners(main_tree, tip_blame.landed(tip_blame.journal_texts(agent_journals)))
    agents = []
    rows = []
    for c in cands:
        who = own.get(c)
        rows.append("  %s %s (%s)" % (c[:9], (git(main_tree, "log", "-1", "--format=%s", c) or "")[:100], who or "no land line"))
        if who and who not in agents:
            agents.append(who)
    text = "\n".join([
        "to: %s" % " ".join(agents + ["main"]),
        "%s red on main %s (%s): %s; candidates=%s (%s: main decides)"
        % (what, tip[:9], time.strftime("%Y-%m-%d %H:%M"), " ".join(red), ",".join(c[:9] for c in cands), why),
        *rows,
        "log: %s" % log,
    ]) + "\n"
    out = os.path.join(home, "run", "nightly")
    os.makedirs(out, exist_ok=True)
    dst = os.path.join(out, "tip-red")
    if os.path.exists(dst):
        text = tip_blame.merged(open(dst).read(), text)
    with open(dst + ".tmp", "w") as f:
        f.write(text)
    os.replace(dst + ".tmp", dst)


def blame(home, main_tree, named, log, agent_journals, what="tip check (quick)"):
    """tip-red for the named commit, through tip_blame (its parent as the last green)"""
    sys.path.insert(0, HERE)
    import tip_blame
    out = os.path.join(home, "run", "nightly")
    os.makedirs(out, exist_ok=True)
    parent = git(main_tree, "rev-parse", named + "^") or named
    with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False) as g:
        g.write("%s\t0\t0\ttip-check\t-\n" % parent)
    try:
        _, text = tip_blame.report(main_tree, named, g.name, log, agent_journals, what=what)
    finally:
        os.unlink(g.name)
    dst = os.path.join(out, "tip-red")
    if os.path.exists(dst):
        text = tip_blame.merged(open(dst).read(), text)
    with open(dst + ".tmp", "w") as f:
        f.write(text)
    os.replace(dst + ".tmp", dst)


def flaky_text(sha, log, when):
    """flaky-open's report of a red the re-run didn't confirm (main's: no test named to own it). Pure."""
    return "\n".join([
        "to: main",
        "tip check (quick) flaky on main %s (%s): red, then green on the same sha; nobody named" % (sha[:9], when),
        "log: %s" % log,
        "a flake is a bug (bise-testing rule 8): the log's FAIL lines name the test",
    ]) + "\n"


def flaky(home, sha, log):
    """the unconfirmed red into flaky-open, merged with one main hasn't forwarded yet"""
    sys.path.insert(0, HERE)
    import tip_blame
    out = os.path.join(home, "run", "nightly")
    os.makedirs(out, exist_ok=True)
    text = flaky_text(sha, log, time.strftime("%Y-%m-%d %H:%M"))
    dst = os.path.join(out, "flaky-open")
    if os.path.exists(dst):
        text = tip_blame.merged(open(dst).read(), text)
    with open(dst + ".tmp", "w") as f:
        f.write(text)
    os.replace(dst + ".tmp", dst)


def quick_red(home, main_tree, wt, tip, base, log, v, journals, today_cpu, bound=CPU_BOUND):
    """After a red quick gate on tip (architect m_63969): only its red steps again (recheck). The
    range's commits whose own diff reaches a red step: exactly one is confirmed on itself first
    (red: tip-red on it). Else the red steps by halves over the range, stopped once today's CPU
    passes the bound: tip-red with candidates= (the diff-named commits in the open window, else the
    window), main decides. The commit the halving names, red once more on itself: tip-red on it;
    green: flaky-open (the halving assumes one steady break, architect m_55231). v gets named
    (a sha, or candidates=<shas>), state flaky on a flake, the rechecks' cpu and secs."""
    try:
        steps, cargo = red_steps(open(log).read())
    except OSError:
        steps, cargo = [], []
    commits = (git(main_tree, "rev-list", "--first-parent", "--reverse", base + ".." + tip) or "").split() or [tip]
    if len(commits) > 1:
        selfnames = set(self_cmds(wt))
        cp = [(c, gate_plan_in(wt, (git(main_tree, "diff-tree", "--no-commit-id", "--name-only", "-r", c) or "").split()))
              for c in commits]
        named = named_by_diff(steps, cargo, cp, selfnames)
    else:
        named = list(commits)
    checked = {}

    def red(c):
        if c not in checked:
            g, s, c_ = recheck(home, wt, c, base, log, steps, cargo)
            v["cpu"] += c_
            v["secs"] += s
            checked[c] = g
        return checked[c] is False

    with open(log, "a") as out:
        out.write("== red steps: %s%s; named by their diff: %s\n" % (" ".join(steps) or "none", "".join(" [%s]" % a for a in cargo),
                                                                  " ".join(c[:9] for c in named) or "none"))
    if len(named) == 1 and red(named[0]):
        v["named"] = named[0]
        blame(home, main_tree, named[0], log, journals)
        return
    x, window = halve(commits, red, lambda: today_cpu + v["cpu"] > bound)
    if window is not None:
        cands = [c for c in named if c in window] or window
        v["named"] = "candidates=" + ",".join(c[:9] for c in cands)
        why = "the halving stopped at the day's CPU bound (%d min)" % (bound // 60)
        blame_candidates(home, main_tree, tip, cands, steps, log, journals, what="tip check (quick)",
                         why=why + ("; each one's own diff reaches a red step" if cands is not window else ""))
        return
    if red(x):
        v["named"] = x
        blame(home, main_tree, x, log, journals)
    else:
        v["state"] = "flaky"
        flaky(home, x, log)


def e2e_step(home, main_tree, wt, tip, base, log, v, journals):
    """After a green quick (plan B1, architect m_63540): the e2e/tmux tests test_map picks for
    base..tip, up to E2E_MAX; a red runs alone once more; red again: the commits whose own diff picks
    a red test are named (one: tip-red on it; several: candidates=; none: flaky-open); green the
    second time: flaky-open. v gets e2e=<what>, its cpu and secs, and state red on a confirmed red."""
    changed = (git(main_tree, "diff", "--name-only", base, tip) or "").split()
    picks, why = e2e_picks(select_in(wt, changed), run_all_tests(wt))
    if not picks:
        v["e2e"] = why
        with open(log, "a") as out:
            out.write("== e2e/tmux: %s\n" % why)
        return
    g, s, c, reds = e2e(home, wt, picks, log)
    v["secs"] += s
    v["cpu"] += c
    if g:
        v["e2e"] = "green:" + ",".join(picks)
        return
    reds = reds or picks
    g2, s2, c2, reds2 = e2e(home, wt, reds, log)
    v["secs"] += s2
    v["cpu"] += c2
    if g2:
        v["e2e"] = "flaky:" + ",".join(reds)
        flaky(home, tip, log)
        return
    red = reds2 or reds
    commits = (git(main_tree, "rev-list", "--first-parent", "--reverse", base + ".." + tip) or "").split()
    cp = []
    for c_ in commits:
        got = select_in(wt, (git(main_tree, "diff-tree", "--no-commit-id", "--name-only", "-r", c_) or "").split())
        cp.append((c_, None if got.strip().startswith("ALL") else set(got.split())))
    cands = candidates(red, cp)
    v["e2e"] = "red:" + ",".join(red)
    if len(cands) == 1:
        v["state"], v["named"] = "red", cands[0]
        blame(home, main_tree, cands[0], log, journals, what="tip check (e2e %s)" % " ".join(red))
    elif cands:
        v["state"] = "red"
        v["e2e"] += " candidates=" + ",".join(c_[:9] for c_ in cands)
        blame_candidates(home, main_tree, tip, cands, red, log, journals)
    else:
        # picked only by an ALL rule: no commit to name, a flake to own (never a guess)
        v["e2e"] += " candidates=none"
        flaky(home, tip, log)


# as-he-does quick after a desktop land while he is away (plan B2, architect m_63540)
AWAY_IDLE = 15 * 60   # his idle seconds before it starts
BACK_IDLE = 60        # under this, he is back: the run stops
AWAY_GAP = 2 * 3600   # at most one run per 2 h


def idle_secs():
    """His idle seconds: HIDIdleTime (IOHIDSystem). as-he-does drives the candidate over CDP (no
    CGEvent, no HID) and opens and quits it with open -g and an Apple Event, so our own run never
    moves it (architect m_63540). TIP_CHECK_IDLE_FILE: the tests' fake."""
    f = os.environ.get("TIP_CHECK_IDLE_FILE")
    if f:
        try:
            return float(open(f).read().strip())
        except (OSError, ValueError):
            return 0.0
    r = subprocess.run(["/usr/sbin/ioreg", "-c", "IOHIDSystem"], capture_output=True, text=True)
    for l in r.stdout.splitlines():
        if "HIDIdleTime" in l:
            try:
                return int(l.rsplit("=", 1)[1].strip()) / 1e9
            except (IndexError, ValueError):
                break
    return 0.0


def reaches_desktop(wt, paths):
    """test_map.py desktop: whether those paths reach the desktop app"""
    if not paths:
        return False
    r = subprocess.run([sys.executable, "-B", "tests/test_map.py", "desktop", *paths], cwd=wt, capture_output=True, text=True)
    return r.returncode == 0 and r.stdout.startswith("desktop ")


def away_verdict(steps, reds, known):
    """('green'|'none'|'owned'|'red', [new reds]) of a candidate+quick run: its steps.tsv states and
    reds.txt, split by known_reds at the tip. Pure."""
    q = steps.get("quick", "none")
    if steps.get("candidate") != "green" and q == "none":
        return ("red", reds or ["desktop/candidate"]) if steps.get("candidate") == "red" else ("none", [])
    if q == "green" and not reds:
        return "green", []
    if q == "none":
        return "none", []
    new = [r for r in reds if r not in known]
    return ("red", new) if new else ("owned", [])


def away_step(home, main_tree, wt, tip, base, log, v, journals, now=time.time, sleep=time.sleep):
    """After a green quick on a range that reaches the desktop, while he's away (idle >= 15 min, on
    AC: the runner waits for AC already), at most once per 2 h: nightly_desktop.sh's candidate and
    as-he-does quick on the tip (its as-he-does in a quiet window), on a throwaway BISE_HOME under
    its out dir (never his hubs or ~/bise; the candidate's build dir stays the shared one). He comes
    back (idle < 60 s): its process group stopped, 'none:he came back', never a red. A new red (not
    in known_reds at the tip): tip-red, the commits whose own diff reaches the desktop named (one),
    candidates (several) or flaky-open (none). v gets ahd=<what>."""
    changed = (git(main_tree, "diff", "--name-only", base, tip) or "").split()
    if not reaches_desktop(wt, changed):
        v["ahd"] = "none:no desktop change"
        return
    lastf = os.path.join(home, "run", "tip-check", "away-last")
    try:
        last = float(open(lastf).read().strip())
    except (OSError, ValueError):
        last = 0.0
    if now() - last < float(os.environ.get("TIP_CHECK_AWAY_GAP", AWAY_GAP)):
        v["ahd"] = "none:one ran %d min ago (at most one per 2 h)" % ((now() - last) // 60)
        return
    idle = idle_secs()
    if idle < AWAY_IDLE:
        v["ahd"] = "none:he is here (idle %d s)" % idle
        return
    with open(lastf, "w") as f:
        f.write("%d\n" % now())
    out = os.path.join(home, "run", "tip-check", "away-%s-%s" % (time.strftime("%Y%m%d-%H%M%S"), tip[:9]))
    os.makedirs(os.path.join(out, "home"), exist_ok=True)
    hr = os.environ.get("TIP_CHECK_HEAVY_RUN") or os.path.join(home, "run", "heavy-run")
    nd = os.environ.get("TIP_CHECK_DESKTOP") or "scripts/nightly_desktop.sh"
    env = dict(os.environ, BISE_HOME=os.path.join(out, "home"), NIGHTLY_DESKTOP_STEPS="candidate quick", SB_AGENT="tip-check",
               SB_BUILD_DIR=os.environ.get("SB_BUILD_DIR") or os.path.join(home, "dev", "build"))
    for k in ("SB_SOCKET", "CARGO_TARGET_DIR", "HEAVY_SLOT"):
        env.pop(k, None)
    # node and pnpm for the candidate's build: fnm's default alias, as nightly_gate.sh adds it (the
    # hub starts this runner with no shell setup: on 11-13 03:55 the build died 'pnpm: command not
    # found' and that infra failure was blamed on a land)
    nb = os.path.join(os.environ.get("FNM_DIR") or os.path.join(os.path.expanduser("~"), ".local", "share", "fnm"), "aliases", "default", "bin")
    if os.path.isdir(nb):
        env["PATH"] = nb + os.pathsep + env.get("PATH", "")
    poll = float(os.environ.get("TIP_CHECK_AWAY_POLL", 20))
    with open(log, "a") as o:
        o.write("== %s as-he-does quick on %s (he's away, idle %d s), its out %s\n" % (time.strftime("%F %T"), tip[:12], idle, out))
        o.flush()
        p = subprocess.Popen([hr, "--low", "nice", "-n", str(NICE_MORE), "bash", nd, out, tip], cwd=wt, env=env,
                             stdout=o, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, start_new_session=True)
        while p.poll() is None:
            sleep(poll)
            if p.poll() is None and idle_secs() < BACK_IDLE:
                o.write("== he came back: the run stopped\n")
                for sig, wait in ((15, 10), (9, 5)):
                    try:
                        os.killpg(p.pid, sig)
                    except OSError:
                        break
                    try:
                        p.wait(wait)
                        break
                    except subprocess.TimeoutExpired:
                        continue
                p.wait()
                v["ahd"] = "none:he came back"
                return
    rd = lambda n: open(os.path.join(out, n)).read() if os.path.exists(os.path.join(out, n)) else ""  # noqa: E731
    steps = dict(l.split("\t", 2)[:2] for l in rd("steps.tsv").splitlines() if "\t" in l)
    reds = [l.strip() for l in rd("reds.txt").splitlines() if l.strip()]
    sys.path.insert(0, os.path.join(HERE, "..", "tests"))
    import known_reds
    try:
        known = known_reds.parse(git(main_tree, "show", "%s:tests/known_reds.txt" % tip) or "")
    except ValueError:
        known = {}
    kind, new = away_verdict(steps, reds, known)
    # a tool missing (exit 127, 'command not found') is this machine's, never a land's: no verdict
    infra = [l for l in rd("candidate.log").splitlines()[-20:] if "command not found" in l or "exit 127" in l]
    if kind == "red" and infra:
        v["ahd"] = "none:infra (%s)" % infra[-1].strip()[:120]
        return
    v["ahd"] = {"green": "green", "none": "none:no result (%s)" % out, "owned": "owned:%d" % len(reds)}.get(kind, "red:" + ";".join(new))
    if kind != "red":
        return
    commits = (git(main_tree, "rev-list", "--first-parent", "--reverse", base + ".." + tip) or "").split()
    cands = [c for c in commits if reaches_desktop(wt, (git(main_tree, "diff-tree", "--no-commit-id", "--name-only", "-r", c) or "").split())]
    v["state"] = "red"
    if len(cands) == 1:
        v["named"] = cands[0]
        blame(home, main_tree, cands[0], log, journals, what="tip check (as-he-does quick: %s)" % "; ".join(new))
    elif cands:
        v["ahd"] += " candidates=" + ",".join(c[:9] for c in cands)
        blame_candidates(home, main_tree, tip, cands, new, log, journals)
    else:
        v["state"] = "flaky"
        v["ahd"] += " candidates=none"
        flaky(home, tip, log)


def run(home, main_tree, hook_base, now=time.time, sleep=time.sleep):
    """the runner's rounds: until main's tip has a verdict"""
    vfile = os.path.join(home, "gate", "tip-verdicts.tsv")
    logs = os.path.join(home, "run", "tip-check")
    os.makedirs(os.path.dirname(vfile), exist_ok=True)
    base_spacing = int(os.environ.get("TIP_CHECK_SPACING", SPACING))
    anc = lambda a, b: subprocess.run(["git", "-C", main_tree, "merge-base", "--is-ancestor", a, b], capture_output=True).returncode == 0  # noqa: E731
    journals = [os.path.join(home, "hubs", h, "journal.jsonl") for h in (os.listdir(os.path.join(home, "hubs")) if os.path.isdir(os.path.join(home, "hubs")) else [])]
    waited_power = 0
    while True:
        tip = git(main_tree, "rev-parse", "main")
        if not tip:
            return 0
        verdicts = parse_verdicts(open(vfile).read()) if os.path.exists(vfile) else []
        p = plan(tip, verdicts, hook_base, now(), anc, base_spacing)
        if p is None:
            return 0
        base, wait = p
        if wait > 0:
            sleep(wait)
            continue  # main may have moved: the newest tip, one run
        if not on_ac():
            if waited_power >= POWER_MAX:
                return 0
            sleep(POWER_POLL)
            waited_power += POWER_POLL
            continue
        base = base or git(main_tree, "rev-parse", tip + "^") or tip
        start = int(now())
        log = os.path.join(logs, "tip-check-%s-%s.log" % (time.strftime("%Y%m%d-%H%M%S", time.localtime(start)), tip[:9]))
        wt = tree_for(home, main_tree)
        v = {"tip": tip, "base": base, "start": start, "log": log, "named": "",
             "commits": len((git(main_tree, "rev-list", "--first-parent", base + ".." + tip) or "").split())}
        if wt is None:
            v.update(state="infra", secs=0, cpu=0.0)
        else:
            green, secs, cpu = quick(home, wt, tip, base, log)
            if green:
                save_seed(wt, log)
            v.update(state="infra" if green is None else "green" if green else "red", secs=secs, cpu=cpu)
            if green is False:
                quick_red(home, main_tree, wt, tip, base, log, v, journals, cpu_today(verdicts, now()),
                          int(os.environ.get("TIP_CHECK_CPU_BOUND", CPU_BOUND)))
            if green:
                e2e_step(home, main_tree, wt, tip, base, log, v, journals)
            if green and v["state"] == "green":
                away_step(home, main_tree, wt, tip, base, log, v, journals, now, sleep)
        with open(vfile, "a") as f:
            f.write(verdict_line(v))
        hook_base = None


def main(argv):
    home = os.environ.get("BISE_HOME") or os.path.join(os.path.expanduser("~"), ".bise")
    here = os.path.dirname(HERE)
    main_tree = (git(here, "worktree", "list", "--porcelain") or "").split("\n", 1)[0].replace("worktree ", "", 1) or here
    hook_base = argv[1] if len(argv) > 1 else None
    os.makedirs(os.path.join(home, "run", "tip-check"), exist_ok=True)
    lock = open(os.path.join(home, "run", "tip-check", "lock"), "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        print("tip_check: a runner is on it (it takes main's newest tip next)")
        return 0
    return run(home, main_tree, hook_base)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
