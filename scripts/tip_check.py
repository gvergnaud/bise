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
  - red: the quick gate again on the range's commits, by halves, to name the first red one.
Its tree: ~/.bise/run/tip-check/tree/<repo> (a worktree, never an agent's; rust/target cloned
from the warm seed: tests/gate.sh seed). No checked tip yet: the newest land's range only (its
<base>, else tip^), never main's history.
Out: <BISE_HOME>/gate/tip-verdicts.tsv, one line a run, next to green-shas.tsv:
  <tip> <base> green|red|infra <start epoch> <secs> <cpu secs> <commits> <log> [<named red sha>]
scripts/loop_audit.py sums the CPU per day from it. A red writes ~/.bise/run/nightly/tip-red
(scripts/tip_blame.py: the named commit and its agent), which main forwards, as the tip gate's.
Tests: tests/tip_check_test.py (pure parts, then a fake repo, gate, heavy-run and power)."""
import fcntl
import os
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
                    "cpu": float(f[5] or 0), "commits": int(f[6] or 0), "log": f[7], "named": f[8] if len(f) > 8 else ""})
    return out


def verdict_line(v):
    return "\t".join(str(x) for x in (v["tip"], v["base"], v["state"], v["start"], v["secs"], round(v["cpu"], 1),
                                       v["commits"], v["log"], v.get("named", ""))) + "\n"


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
    lo, hi = 0, len(commits) - 1
    while lo < hi:
        mid = (lo + hi) // 2
        if red(commits[mid]):
            hi = mid
        else:
            lo = mid + 1
    return commits[hi] if commits else None


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


def blame(home, main_tree, named, log, agent_journals):
    """tip-red for the named commit, through tip_blame (its parent as the last green)"""
    sys.path.insert(0, HERE)
    import tip_blame
    out = os.path.join(home, "run", "nightly")
    os.makedirs(out, exist_ok=True)
    parent = git(main_tree, "rev-parse", named + "^") or named
    with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False) as g:
        g.write("%s\t0\t0\ttip-check\t-\n" % parent)
    try:
        _, text = tip_blame.report(main_tree, named, g.name, log, agent_journals, what="tip check (quick)")
    finally:
        os.unlink(g.name)
    dst = os.path.join(out, "tip-red")
    if os.path.exists(dst):
        text = tip_blame.merged(open(dst).read(), text)
    with open(dst + ".tmp", "w") as f:
        f.write(text)
    os.replace(dst + ".tmp", dst)


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
            v.update(state="infra" if green is None else "green" if green else "red", secs=secs, cpu=cpu)
            if green is False and v["commits"] > 1:
                # one red over several lands: the quick gate on their commits, by halves
                commits = (git(main_tree, "rev-list", "--first-parent", "--reverse", base + ".." + tip) or "").split()
                spent = [0.0]

                def red(c):
                    g, s, c_ = quick(home, wt, c, base, log)
                    spent[0] += c_
                    return g is False
                v["named"] = first_red(commits, red)
                v["cpu"] += spent[0]
            elif green is False:
                v["named"] = tip
            if v["named"]:
                blame(home, main_tree, v["named"], log, journals)
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
