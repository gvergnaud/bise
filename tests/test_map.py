#!/usr/bin/env python3
"""Which e2e/tmux tests a change can break: the full gate's test selection
(docs/loop-speed.md §11, the user's yes on q1, 2026-10-12).

  python3 tests/test_map.py select <changed path>...   # "ALL <why>" or the tests
  python3 tests/test_map.py --self-test                # the map is honest

The Rust part of the full gate (build, every Rust test, clippy) always runs;
this picks the e2e/tmux tests of run_all.sh. Every test still runs nightly
(scripts/nightly_gate.sh) and in a release's gate (gate.sh full --all).

The rules, per changed path, the union of all:
  - docs/, site/, nix/, .github/, office/, the notices, README and bise.toml: no test reads them
    at run time; a test that names the very file (a fixture, a path it
    opens) runs.
  - rust/tui/, rust/vendor/crossterm/: the TUI. Every *_tmux test, and every
    other test that names tmux or the TUI crate.
  - bend/laws/, bend/proofs/: PROOF.
  - apps/desktop/, dev/ (the desktop's QA plugin), scripts/desktop*,
    tests/desktop_*, tests/ambient_qa*: the desktop app's own. No terminal
    e2e/tmux test (but one that names the very file); the gate runs the
    desktop's checks instead (`desktop` below).
  - tests/<file>: that test if it is one, and every test whose file names it
    (an import, a path it runs: fake_provider.py), transitively through the
    helpers. tests/fixtures/<x>: the tests that name x.
  - anything else (rust/ but the TUI, bend/, prompts/, plugins/, .agents/,
    scripts/, packaging/, kit/, computer-use/, the gate itself, a path no
    rule knows): ALL. Shared code runs everything.
The self-test makes every top-level path of the repo known here, so a new
one is a decision, not a silent ALL or a silent nothing.

  python3 tests/test_map.py desktop <changed path>...  # "desktop <why>" or "none"

Whether a change runs the desktop app's checks (gate.sh desktop_check:
`pnpm check` in apps/desktop): a desktop path above, the gate itself, or
a seam the desktop reads: the crate of each generator a generated file
under apps/desktop names in its header (`*.gen.ts`, a law of the
self-test), and the core the app runs (rust/tui/src/ambient, rust/harness's
ambient.rs, approot.rs, update.rs). The terminal's tests of a seam run as
for any path; a desktop path never runs them.
"""
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
import desktop_assets  # noqa: E402  what the coverage map can't see (pick-assets)

# top-level paths no e2e/tmux test reads at run time
NONE_PREFIXES = ("docs/", "site/", "nix/", ".github/", "office/")
NONE_FILES = {"LICENSE", "NOTICE", "THIRD_PARTY_NOTICES", "README.md", "flake.nix", "flake.lock",
              "default.nix", ".gitignore", "bise.toml"}
TUI_PREFIXES = ("rust/tui/", "rust/vendor/crossterm/")
PROOF_PREFIXES = ("bend/laws/", "bend/proofs/")
# the gate's own files: a change to them runs everything
GATE_FILES = {"tests/gate.sh", "tests/run_all.sh", "tests/slow_first.py", "tests/test_map.py", "tests/desktop_assets.py"}
# every top-level entry of the repo, and what it is: none, tui, mixed (some
# paths in it have a rule), all
TOP = {
    "docs": "none", "site": "none", "nix": "none", ".github": "none", "LICENSE": "none", "NOTICE": "none",
    "THIRD_PARTY_NOTICES": "none", "README.md": "none", "flake.nix": "none", "flake.lock": "none",
    "default.nix": "none", ".gitignore": "none",
    # the repo's tracked config (issue 28): the land reads it, no test hub does
    "bise.toml": "none",
    # every agent's prompt in this repo (BISE-232): words only, no test reads it
    "AGENTS.md": "none",
    "rust": "mixed", "bend": "mixed", "tests": "mixed",
    "kit": "all", "prompts": "all", "plugins": "all", ".agents": "all", "scripts": "all",
    "packaging": "all", "computer-use": "all", "run.sh": "all", "apps": "mixed", "dev": "desktop",
    # the office SDK: no e2e/tmux test loads it yet; the gate runs
    # tests/office_check.sh on it instead (office-run moves this when bise embeds it)
    "office": "none",
    # example plugins users add by hand (examples/plugins/: MCP Apps views); no e2e/tmux
    # test loads them: tests/fixtures/mcp-apps/check.py checks their shapes and
    # their views' escaping, in gate.sh quick (tests/gate_plan.py's mcpapps)
    "examples": "none",
}
# the desktop app's own paths: its checks, no terminal test
DESKTOP_PREFIXES = ("apps/desktop/", "dev/", "scripts/desktop", "tests/desktop_", "tests/ambient_qa")
# what the desktop's checks read besides its own tree (architect m_19472):
# the generators' crates (pinned by gen_seams_law) and the core the app runs
DESKTOP_SEAMS = ("rust/proto/", "rust/home/", "rust/tui/src/ambient/", "rust/harness/src/ambient.rs",
                 "rust/harness/src/approot.rs", "rust/harness/src/update.rs")
GEN_SUFFIX = ".gen.ts"
TUI_WORDS = re.compile(r"tmux|rust/tui|bend[-_]tui")


def run_all_tests(root=ROOT):
    """The e2e/tmux tests of run_all.sh, in its order."""
    s = open(os.path.join(root, "tests", "run_all.sh")).read()
    return s.split('TESTS="', 1)[1].split('"', 1)[0].split()


def texts(root=ROOT):
    """{stem: text} of every tests/*.py."""
    out = {}
    d = os.path.join(root, "tests")
    for f in os.listdir(d):
        if f.endswith(".py"):
            with open(os.path.join(d, f), errors="replace") as fh:
                out[f[:-3]] = fh.read()
    return out


_WORD = re.compile(r"[\w-]+")
_INDEX = {}


def _index(txt):
    """{word: stems whose file has it as a whole [\\w-]+ run}, built once per txt (fast-loop: the
    regex per word over every file was 60 s of the quick gate's 100 s self-test at load 29)."""
    k = id(txt)
    if k not in _INDEX or _INDEX[k][0] is not txt:
        idx = {}
        for s, t in txt.items():
            for w in set(_WORD.findall(t)):
                idx.setdefault(w, set()).add(s)
        _INDEX[k] = (txt, idx)
    return _INDEX[k][1]


def naming(word, txt):
    """The stems whose file names `word` (a whole word: e2e is not artifacts_e2e)."""
    if _WORD.fullmatch(word):
        # a maximal [\w-]+ run equal to the word is exactly the regex below's match
        return set(_index(txt).get(word, ()))
    rx = re.compile(r"(?<![\w-])" + re.escape(word) + r"(?![\w-])")
    return {s for s, t in txt.items() if rx.search(t)}


def closure(stems, txt):
    """The stems and every file naming one of them, transitively."""
    seen, todo = set(), list(stems)
    while todo:
        s = todo.pop()
        if s in seen:
            continue
        seen.add(s)
        todo.extend(naming(s, txt) - seen)
    return seen


def select(changed, tests, txt):
    """(None, why) for every test, or (set of tests, whys)."""
    picked, whys = set(), []
    tset = set(tests)
    for p in changed:
        p = p.strip()
        if not p:
            continue
        top = p.split("/", 1)[0]
        if p in GATE_FILES:
            return None, "the gate itself (%s)" % p
        if p.startswith(NONE_PREFIXES) or p in NONE_FILES:
            named = {s for s in naming(p, txt) if s in tset}
            picked |= named
            whys.append("%s: %s" % (p, "named by %s" % " ".join(sorted(named)) if named else "no test"))
        elif p.startswith(DESKTOP_PREFIXES):
            named = {s for s in naming(p, txt) if s in tset}
            picked |= named
            whys.append("%s: the desktop's (its checks)%s" % (p, ", named by %s" % " ".join(sorted(named)) if named else ""))
        elif p.startswith(TUI_PREFIXES):
            tui = {t for t in tests if t.endswith("_tmux") or (t in txt and TUI_WORDS.search(txt[t]))}
            picked |= tui
            whys.append("%s: the TUI's tests" % p)
        elif p.startswith(PROOF_PREFIXES):
            picked.add("PROOF")
            whys.append("%s: PROOF" % p)
        elif top == "tests" and p.endswith(".py") and "/" not in p[len("tests/"):]:
            stem = p[len("tests/"):-3]
            got = {s for s in closure({stem}, txt) if s in tset}
            picked |= got
            whys.append("%s: %d tests name it" % (p, len(got)))
        elif p.startswith("tests/fixtures/"):
            name = p.split("/")[2]
            got = {s for s in closure(naming(name, txt), txt) if s in tset}
            if not got:
                return None, "%s: no test names it (a fixture of a helper?)" % p
            picked |= got
            whys.append("%s: %d tests name it" % (p, len(got)))
        else:
            return None, "%s (shared code or no rule)" % p
    return picked, whys


def desktop(changed):
    """Why the desktop's checks run for this change, or None."""
    for p in changed:
        p = p.strip()
        if p.startswith(DESKTOP_PREFIXES):
            return "%s (the desktop's)" % p
        if p.startswith(DESKTOP_SEAMS):
            return "%s (a seam the desktop reads)" % p
        if p in GATE_FILES:
            return "%s (the gate itself)" % p
    return None


def generators(header):
    """The repo files a generated file's header names as its generator:
    the `rust/....rs` paths in its first lines."""
    return re.findall(r"\b(rust/[\w./-]+\.rs)\b", header)


def gen_seams_law(gen_files):
    """Every generated file under apps/desktop names its generator, and the
    generator's crate (rust/<crate>/) is a desktop seam: a change there
    reruns the checks that read its output. [(path, header)] -> faults."""
    faults = []
    for path, header in gen_files:
        gens = generators(header)
        if not gens:
            faults.append("%s names no generator (rust/<crate>/....rs) in its first lines" % path)
        for g in gens:
            crate = "/".join(g.split("/")[:2]) + "/"
            if not crate.startswith(DESKTOP_SEAMS):
                faults.append("%s: its generator %s (crate %s) is not in DESKTOP_SEAMS" % (path, g, crate))
    return faults


def gen_files(root=ROOT):
    """(path, first lines) of every *.gen.ts git knows under apps/desktop."""
    out = subprocess.run(["git", "ls-files", "apps/desktop"], cwd=root, capture_output=True, text=True).stdout.split()
    got = []
    for p in out:
        if p.endswith(GEN_SUFFIX):
            with open(os.path.join(root, p), errors="replace") as fh:
                got.append((p, "".join(fh.readline() for _ in range(4))))
    return got


# ---- desktop-pick: which harness checks (apps/desktop/tests/checks) a change can break (fast-try
# p2; architect m_29198 m_29411, review-gates m_29412). The map is the nightly's coverage
# (apps/desktop/tests/harness/coverage/covmap.mts writes <gate dir>/desktop-cov.json, never
# committed): per check, the source files V8 saw run in it and the files its spec imports. This is
# its one reader. Anything it can't vouch for is ALL.
COV_FILE = "desktop-cov.json"
COV_SCHEMA = 1
COV_MAX_AGE_H = 60
APP = "apps/desktop/"
APP_SRC = APP + "src/"
APP_CHECKS = APP + "tests/checks/"
# shared code: what the main process or the preload runs, and the generated proto. The map also
# lists every source of the main and preload bundles (its "shared")
APP_SHARED = (APP_SRC + "main/", APP_SRC + "proto/")
VITEST_FILE = re.compile(r"^apps/desktop/src/(.*\.test\.tsx?|test-setup\.ts)$")


def cov_problem(cov, now, nightly_days):
    """Why this map can't be trusted (ALL), or None. cov: the parsed JSON; nightly_days: the
    dates (YYYY-MM-DD) of the nightly runs, newest last."""
    if not isinstance(cov, dict) or cov.get("schema") != COV_SCHEMA:
        return "the coverage map is not schema %d" % COV_SCHEMA
    for k, t in (("sha", str), ("day", str), ("epoch", int), ("files", list), ("shared", list), ("checks", dict)):
        if not isinstance(cov.get(k), t):
            return "the coverage map has no %s" % k
    age = (now - cov["epoch"]) / 3600.0
    if age > COV_MAX_AGE_H:
        return "the coverage map is %d h old (more than %d h)" % (age, COV_MAX_AGE_H)
    if cov["day"] not in nightly_days[-2:]:
        return "the coverage map (%s) is not from one of the last 2 nightlies (%s)" % (
            cov["day"], ", ".join(nightly_days[-2:]) or "none ran")
    bad = [f for f in cov["files"] + cov["shared"] if not f.startswith(APP_SRC)]
    if bad or not cov["files"]:
        return "the coverage map holds bundles, not source files (%s)" % (bad[0] if bad else "no file")
    for c, e in cov["checks"].items():
        if not (isinstance(e, dict) and isinstance(e.get("covered"), list) and isinstance(e.get("imports"), list)):
            return "the coverage map's check %s is malformed" % c
        if any(not isinstance(i, int) or not 0 <= i < len(cov["files"]) for i in e["covered"]):
            return "the coverage map's check %s names a file it doesn't list" % c
    return None


def desktop_pick(changed, cov, all_list, specs, tree=None):
    """(None, why) for every check, or (checks, whys). changed: the repo paths of the change (the
    map's sha to the new tree, the try's from to the new tree: review-gates m_29412 #1); cov: a
    trusted map (cov_problem None); all_list: the files the audit sent to ALL until the next map;
    specs: the checks of the new tree (apps/desktop/tests/checks/**.spec.ts, repo paths); tree: the
    new tree's import graph (desktop_assets.Tree, or a function that makes it) that places what the map can't see (a .css, a
    type-only or vitest-only module, the hero build), None: those are ALL."""
    files = cov["files"]
    known = set(files)
    shared = set(cov["shared"])
    checks = {APP + "tests/" + c: e for c, e in cov["checks"].items()}
    covers = {}
    for c, e in checks.items():
        for i in e["covered"]:
            covers.setdefault(files[i], set()).add(c)
        for f in e["imports"]:
            covers.setdefault(f, set()).add(c)
    picked, whys = set(), []
    built = []

    def new_tree():
        # the graph costs seconds: read only when a path needs it (tree may be a thunk)
        if not built:
            built.append(tree() if callable(tree) else tree)
        return built[0]
    for p in sorted(set(x.strip() for x in changed if x.strip())):
        if desktop([p]) is None:
            continue
        if p in GATE_FILES:
            return None, "%s: the gate itself" % p
        if p in all_list:
            return None, "%s: a file the nightly audit missed (ALL until the next map)" % p
        if not p.startswith(APP):
            return None, "%s: outside the app, it reaches the desktop" % p
        if p.startswith(APP_SHARED) or p in shared:
            return None, "%s: shared code (the main process, the preload or the proto)" % p
        if p.startswith(APP_SRC):
            if p in known:
                got = covers.get(p, set())
                picked |= got
                whys.append("%s: %d checks ran it" % (p, len(got)))
            elif VITEST_FILE.match(p):
                whys.append("%s: a vitest file, no check" % p)
            else:
                got, why = desktop_assets.place(p, new_tree(), cov, shared)
                if got is None:
                    return None, why
                picked |= got
                whys.append(why)
        elif p.startswith(APP_CHECKS) and p.endswith(".spec.ts"):
            picked.add(p)
            whys.append("%s: the check itself" % p)
        elif p.startswith(APP_CHECKS) and p in covers:
            got = covers[p]
            picked |= got
            whys.append("%s: %d checks import it" % (p, len(got)))
        elif p == desktop_assets.HERO_CONFIG and tree is not None:
            got, why = desktop_assets.place(p, new_tree(), cov, shared)
            if got is None:
                return None, why
            picked |= got
            whys.append(why)
        else:
            return None, "%s: the map doesn't know it (the harness, a config, a fixture read by path)" % p
    # a check the map can't vouch for runs every time: new, or its coverage incomplete
    blind = {s for s in specs if s not in checks or not checks[s].get("complete")}
    if blind:
        picked |= blind
        whys.append("%d checks the map has no full coverage of (new, or the app not profiled): always" % len(blind))
    return {s for s in picked if s in set(specs)}, whys


def gate_dir():
    """gate.sh's gate dir: <BISE_HOME>/gate (~/.bise/gate)."""
    return os.path.join(os.environ.get("BISE_HOME") or os.path.expanduser("~/.bise"), "gate")


def nightly_days(home=None):
    """The dates the nightly ran (scripts/nightly_gate.sh's <date>.log), oldest first."""
    h = home or os.environ.get("BISE_HOME") or os.path.expanduser("~/.bise")
    try:
        names = os.listdir(os.path.join(h, "run", "nightly"))
    except OSError:
        return []
    return sorted(n[:-4] for n in names if re.match(r"^\d{4}-\d\d-\d\d\.log$", n))


def git(root, *args):
    r = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True)
    return r.returncode, r.stdout


def changed_since(root, base, to):
    """The paths <base> to <to> touches (a rename: both), or None when git can't say; to None: the
    working tree, untracked files included."""
    rc, out = git(root, "diff", "--name-only", "--no-renames", base, *([to] if to else []))
    if rc:
        return None
    got = set(out.split())
    if not to:
        rc, out = git(root, "ls-files", "--others", "--exclude-standard")
        if rc:
            return None
        got |= set(out.split())
    return got


def desktop_pick_cli(root, frm, to, gdir, now, days, log=None):
    """(None, why) or (checks, whys), the changed set, the map's sha; logged to <gdir>/
    desktop-pick.jsonl with `log` (try, land): the nightly audit replays it."""
    path = os.path.join(gdir, COV_FILE)
    sel, why, changed, msha = None, None, set(), None
    rc, _ = git(root, "rev-parse", "--verify", "-q", frm + "^{commit}")
    got = changed_since(root, frm, to) if rc == 0 else None
    if got is None:
        why = "git can't diff %s to %s" % (frm, to or "the working tree")
    else:
        changed = got
        try:
            with open(path) as fh:
                cov = json.load(fh)
        except FileNotFoundError:
            cov, why = None, "no coverage map (%s; the nightly writes it)" % path
        except (OSError, ValueError) as e:
            cov, why = None, "the coverage map is unreadable (%s)" % e
        if cov is not None:
            why = cov_problem(cov, now, days)
        if why is None:
            msha = cov["sha"]
            if git(root, "merge-base", "--is-ancestor", msha, frm)[0]:
                why = "the coverage map's sha %s is not an ancestor of %s" % (msha[:9], frm[:9])
            else:
                since = changed_since(root, msha, to)
                if since is None:
                    why = "git can't diff the map's sha %s" % msha[:9]
                else:
                    changed |= since
        if why is None:
            try:
                with open(os.path.join(gdir, "desktop-pick-all.txt")) as fh:
                    all_list = {line.strip() for line in fh if line.strip() and not line.startswith("#")}
            except FileNotFoundError:
                all_list = set()
            rc, out = git(root, "ls-files", "--", APP_CHECKS)
            specs = [s for s in out.split() if s.endswith(".spec.ts") and os.path.exists(os.path.join(root, s))]
            specs += [s for s in changed if s.startswith(APP_CHECKS) and s.endswith(".spec.ts")
                      and s not in specs and os.path.exists(os.path.join(root, s))]
            sel, why = desktop_pick(changed, cov, all_list, specs, lambda: new_tree(root, to, changed, cov, specs))
    if log:
        line = {"t": int(now), "kind": log, "from": frm, "to": to or "worktree", "map": msha,
                "all": why if sel is None else None, "picked": sorted(sel or []), "changed": sorted(changed)}
        try:
            with open(os.path.join(gdir, "desktop-pick.jsonl"), "a") as fh:
                fh.write(json.dumps(line) + "\n")
        except OSError:
            pass
    return sel, why, changed, msha


def new_tree(root, to, changed, cov, specs):
    """The new tree's import graph for desktop_assets (its code files under apps/desktop, the
    commit's or the working tree's), None when git can't read it: what the map can't see is ALL."""
    if to:
        rc, out = git(root, "ls-tree", "-r", "--name-only", to, "--", "apps/desktop")
        if rc:
            return None
        paths = desktop_assets.tree_paths(out.split("\n"))
        r = subprocess.run(["git", "cat-file", "--batch"], cwd=root, input="".join("%s:%s\n" % (to, p) for p in paths).encode(),
                           capture_output=True)
        if r.returncode:
            return None
        texts, buf = {}, r.stdout
        for p in paths:
            head, _, buf = buf.partition(b"\n")
            parts = head.split()
            if len(parts) != 3:
                return None
            n = int(parts[2])
            texts[p] = buf[:n].decode("utf8", "replace")
            buf = buf[n + 1:]
    else:
        rc, out = git(root, "ls-files", "-co", "--exclude-standard", "--", "apps/desktop")
        if rc:
            return None
        texts = {}
        for p in desktop_assets.tree_paths(out.split("\n")):
            try:
                with open(os.path.join(root, p), encoding="utf8", errors="replace") as fh:
                    texts[p] = fh.read()
            except OSError:
                continue
    known = set(cov["files"]) | set(cov["shared"])
    ts = {p: texts[p] for p in changed if p in texts and p.startswith(APP_SRC) and p.endswith(desktop_assets.EXTS)
          and p not in known and not VITEST_FILE.match(p)}
    return desktop_assets.Tree(texts, desktop_assets.emits_js(root, ts), specs, set(cov["files"]), cov["shared"])


def git_tops(root=ROOT):
    # one path a line: a fixture may hold spaces (Chrome's "Local Extension Settings")
    out = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True).stdout.splitlines()
    return {p.split("/", 1)[0] for p in out}


def self_test():
    tests, txt = run_all_tests(), texts()
    bad = [t for t in tests if t != "PROOF" and t not in txt]
    assert not bad, "run_all.sh names tests with no tests/<t>.py: %s" % bad
    unknown = sorted(git_tops() - set(TOP))
    assert not unknown, ("top-level paths with no rule in tests/test_map.py TOP: %s "
                         "(say what they are: none, all, or a rule)" % unknown)
    # the TUI: every tmux test, and the tests that drive the TUI otherwise
    sel, _ = select(["rust/tui/src/feed.rs"], tests, txt)
    tmux = {t for t in tests if t.endswith("_tmux")}
    assert sel is not None and tmux <= sel and "e2e" in sel and "artifacts_e2e" not in sel, sorted(sel or [])
    # docs: nothing, unless a test names the file
    assert select(["docs/loop-speed.md"], tests, txt)[0] == set()
    # a helper runs every test naming it, transitively (e2e.py: nearly all)
    sel, _ = select(["tests/wait.py"], tests, txt)
    assert "tui_tmux" in sel and "e2e" in sel, sorted(sel)
    sel, _ = select(["tests/fake_provider.py"], tests, txt)
    assert "e2e" in sel and "core_restart" in sel, sorted(sel)
    # a test alone: itself (and the tests naming it)
    sel, _ = select(["tests/wake_e2e.py"], tests, txt)
    assert "wake_e2e" in sel and "tui_tmux" not in sel, sorted(sel)
    # the laws: PROOF
    assert select(["bend/laws/hub.bend"], tests, txt)[0] == {"PROOF"}
    assert select(["bend/proofs/lib/hub-1.bend"], tests, txt)[0] == {"PROOF"}
    # shared code and the unknown: everything
    for p in ("bend/hub/core.bend", "rust/switchboard/src/core.rs", "prompts/prompt-bise.txt",
              "tests/run_all.sh", "newdir/x"):
        assert select([p], tests, txt)[0] is None, p
    # the union
    sel, _ = select(["docs/a.md", "bend/proofs/hub.bend", "rust/tui/src/feed.rs"], tests, txt)
    assert "PROOF" in sel and "tui_tmux" in sel
    # the desktop's paths: no terminal test, its checks; a seam: both
    for p in ("apps/desktop/src/thread/rows.tsx", "dev/plugins/ambient-qa/plugin.json", "scripts/desktop_try.py",
              "tests/desktop_try_test.py", "tests/ambient_qa_test.py"):
        assert select([p], tests, txt)[0] == set(), p
        assert desktop([p]), p
    # a desktop script's own test, by the path it names (trim-2: in run_all.sh, picked on its source)
    assert select(["scripts/desktop_app.py"], tests, txt)[0] == {"desktop_app_test"}
    assert select(["dev/plugins/ambient-qa/world.py"], tests, txt)[0] == {"ambient_qa_world_test"}
    for p in ("rust/proto/src/draft.rs", "rust/home/src/env.rs", "rust/harness/src/update.rs"):
        assert select([p], tests, txt)[0] is None and desktop([p]), p
    sel, _ = select(["rust/tui/src/ambient/core/hubs.rs"], tests, txt)
    assert "tui_tmux" in sel and desktop(["rust/tui/src/ambient/core/hubs.rs"])
    for p in ("rust/tui/src/feed.rs", "rust/switchboard/src/core.rs", "bend/hub/core.bend", "docs/a.md",
              "scripts/versions.sh", "apps/other/x"):
        assert desktop([p]) is None, p
    assert select(["apps/other/x"], tests, txt)[0] is None
    # every generated file under apps/desktop names a generator in a seam
    assert gen_seams_law([("apps/desktop/src/x.gen.ts", "// generated by bise-proto (rust/proto/tests/ts.rs)\n")]) == []
    assert gen_seams_law([("a.gen.ts", "// generated from VARS (rust/switchboard/src/x.rs)\n")])
    assert gen_seams_law([("a.gen.ts", "// hand-written\n")])
    faults = gen_seams_law(gen_files())
    assert not faults, "\n".join(faults)
    desktop_pick_self_test()
    desktop_assets.self_test()
    print("test_map self-test: ok (%d tests, %d in the TUI group)" % (len(tests), len(select(["rust/tui/x"], tests, txt)[0])))


def fake_cov(**kw):
    """A small trusted map: a.tsx ran in keep, b.tsx in keep and menus, c.ts in none; menus imports
    the world helper; shared/api.ts is in the main bundle and the renderer's, and keep ran it."""
    cov = {"schema": COV_SCHEMA, "sha": "0" * 40, "day": "2026-10-17", "epoch": 1000000,
           # shared/api.ts: in the renderer and the main bundle both (shared wins)
           "files": [APP_SRC + "app/a.tsx", APP_SRC + "app/b.tsx", APP_SRC + "app/c.ts", APP_SRC + "shared/api.ts"],
           "shared": [APP_SRC + "shared/api.ts"],
           "checks": {"checks/app/keep.spec.ts": {"covered": [0, 1, 3], "imports": [], "complete": True},
                      "checks/app/menus.spec.ts": {"covered": [1], "imports": [APP_CHECKS + "world.ts"], "complete": True}}}
    cov.update(kw)
    return cov


def desktop_pick_self_test():
    """Every ALL rule of desktop-pick has its own case here (review-gates m_29412)."""
    keep, menus = APP_CHECKS + "app/keep.spec.ts", APP_CHECKS + "app/menus.spec.ts"
    specs = [keep, menus]
    days, now = ["2026-10-16", "2026-10-17"], 1000000 + 3600
    cov = fake_cov()
    assert cov_problem(cov, now, days) is None
    # the map's trust: schema, age, the last 2 nightlies, source files only, its indexes
    assert "schema" in cov_problem(fake_cov(schema=0), now, days)
    assert "schema" in cov_problem(None, now, days)
    assert "no epoch" in cov_problem(fake_cov(epoch="x"), now, days)
    assert "h old" in cov_problem(cov, 1000000 + 61 * 3600, days)
    assert cov_problem(cov, 1000000 + 59 * 3600, days) is None
    assert "last 2 nightlies" in cov_problem(cov, now, ["2026-10-17", "2026-10-18", "2026-10-19"])
    assert "last 2 nightlies" in cov_problem(cov, now, [])
    assert "bundles" in cov_problem(fake_cov(files=["apps/desktop/out/renderer/assets/index.js"]), now, days)
    assert "bundles" in cov_problem(fake_cov(files=[]), now, days)
    assert "bundles" in cov_problem(fake_cov(shared=["apps/desktop/out/main/index.js"]), now, days)
    assert "doesn't list" in cov_problem(fake_cov(checks={"checks/x.spec.ts": {"covered": [9], "imports": []}}), now, days)
    assert "malformed" in cov_problem(fake_cov(checks={"checks/x.spec.ts": {"covered": [0]}}), now, days)

    def pick(*changed, all_list=(), specs=specs, cov=cov):
        return desktop_pick(list(changed), cov, set(all_list), specs)
    # a file V8 saw run: the checks that ran it; none ran it: no check
    assert pick(APP_SRC + "app/a.tsx")[0] == {keep}
    assert pick(APP_SRC + "app/b.tsx")[0] == {keep, menus}
    assert pick(APP_SRC + "app/c.ts")[0] == set()
    # a check's own spec, a helper it imports, a vitest file, a path the desktop doesn't read
    assert pick(menus)[0] == {menus}
    assert pick(APP_CHECKS + "world.ts")[0] == {menus}
    assert pick(APP_SRC + "app/a.test.tsx")[0] == set()
    assert pick("docs/x.md", "rust/tui/src/feed.rs")[0] == set()
    # ALL, each rule
    for p, why in ((APP_SRC + "app/new.tsx", "doesn't know it (new"),                 # new, renamed, type-only
                   (APP_SRC + "app/look.css", "doesn't know it (new"),                # an asset V8 can't see
                   (APP_SRC + "main/index.ts", ": shared code"),                     # the main process
                   (APP_SRC + "main/preload.ts", ": shared code"),                   # the preload
                   (APP_SRC + "proto/hub.gen.ts", ": shared code"),                  # the proto
                   (APP_SRC + "shared/api.ts", ": shared code"),                     # in the main bundle
                   (APP + "tests/harness/launch.ts", "doesn't know it (the harness"),          # the harness
                   (APP + "package.json", "doesn't know it (the harness"),                    # a config
                   (APP + "pnpm-lock.yaml", "doesn't know it (the harness"),
                   (APP + "electron.vite.config.ts", "doesn't know it (the harness"),
                   (APP_CHECKS + "fixtures/data.json", "doesn't know it (the harness"),       # read by path
                   ("rust/proto/src/draft.rs", ": outside the app"),            # a seam
                   ("dev/plugins/ambient-qa/fake_gh.py", ": outside the app"),
                   ("tests/gate.sh", ": the gate itself")):
        sel, w = pick(APP_SRC + "app/a.tsx", p)
        assert sel is None and why in w, (p, w)
    sel, w = pick(APP_SRC + "app/c.ts", all_list=[APP_SRC + "app/c.ts"])
    assert sel is None and "audit" in w, w
    # the new tree places a .css by the checks that loaded its importer (desktop_assets' own self-test
    # has every rule); without the tree, or without the import, it is ALL
    texts = {APP_SRC + "app/a.tsx": "import './look.css';\n", APP_SRC + "app/look.css": ".l {}\n", keep: "", menus: "",
             APP_SRC + "renderer/index.html": '<script type="module" src="../app/a.tsx"></script>',
             APP_SRC + "main/index.ts": "import '../app/b';\nimport '../app/c';\nimport '../shared/api';\n",
             APP_SRC + "app/b.tsx": "", APP_SRC + "app/c.ts": "", APP_SRC + "shared/api.ts": ""}
    lcov = fake_cov(loads=True, checks={"checks/app/keep.spec.ts": {"covered": [0], "imports": [], "loads": [0, 1], "complete": True},
                                        "checks/app/menus.spec.ts": {"covered": [1], "imports": [], "loads": [1], "complete": True}})
    tree = desktop_assets.Tree(texts, {}, specs, set(lcov["files"]))
    assert desktop_pick([APP_SRC + "app/look.css"], lcov, set(), specs, tree)[0] == {keep}
    tree = desktop_assets.Tree({**texts, APP_SRC + "app/a.tsx": ""}, {}, specs, set(lcov["files"]))
    assert desktop_pick([APP_SRC + "app/look.css"], lcov, set(), specs, tree)[0] is None
    tree = desktop_assets.Tree(texts, {}, specs, set(lcov["files"]))
    assert desktop_pick([APP + "hero.vite.config.ts"], lcov, set(), specs, tree)[0] == set()
    assert desktop_pick([APP + "hero.vite.config.ts"], lcov, set(), specs)[0] is None
    # a check the map can't vouch for runs every time: a new one, one without full coverage
    new = APP_CHECKS + "app/new.spec.ts"
    assert pick(APP_SRC + "app/c.ts", specs=specs + [new])[0] == {new}
    blind = fake_cov(checks={"checks/app/keep.spec.ts": {"covered": [], "imports": [], "complete": False},
                             "checks/app/menus.spec.ts": {"covered": [1], "imports": [], "complete": True}})
    assert pick(APP_SRC + "app/c.ts", cov=blind)[0] == {keep}
    # a check gone from the tree is not picked
    assert pick(APP_SRC + "app/b.tsx", specs=[keep])[0] == {keep}
    desktop_pick_git_self_test()


def desktop_pick_git_self_test():
    """The changed set and the map's sha, on a throwaway repo (review-gates m_29412 #1): a map at M,
    then a commit makes a.tsx call c.ts (from), then a change touches c.ts only: keep ran a.tsx at M,
    so keep is picked through M..to. And the map's sha must be an ancestor of from."""
    import shutil
    import tempfile
    d = tempfile.mkdtemp(prefix="test-map-")
    try:
        def g(*a):
            # never the user's signing key or hooks: a throwaway repo's commits
            subprocess.run(["git", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", *a], cwd=d,
                           check=True, capture_output=True,
                           env={**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t",
                                "GIT_COMMITTER_EMAIL": "t@t"})
            return subprocess.run(["git", "rev-parse", "HEAD"], cwd=d, capture_output=True, text=True).stdout.strip()

        def put(p, s):
            os.makedirs(os.path.dirname(os.path.join(d, p)), exist_ok=True)
            with open(os.path.join(d, p), "w") as fh:
                fh.write(s)
        g("init", "-q", "-b", "main")
        keep = APP_CHECKS + "app/keep.spec.ts"
        for p in (APP_SRC + "app/a.tsx", APP_SRC + "app/b.tsx", APP_SRC + "app/c.ts", keep, APP_CHECKS + "app/menus.spec.ts"):
            put(p, "x\n")
        g("add", "-A")
        m = g("commit", "-qm", "M")
        put(APP_SRC + "app/a.tsx", "import c\n")
        g("commit", "-qam", "a calls c")
        frm = subprocess.run(["git", "rev-parse", "HEAD"], cwd=d, capture_output=True, text=True).stdout.strip()
        put(APP_SRC + "app/c.ts", "y\n")
        to = g("commit", "-qam", "c changes")
        gd = os.path.join(d, "gate")
        os.makedirs(gd)
        days, now = ["2026-10-17"], 1000000 + 3600

        def write(cov):
            with open(os.path.join(gd, COV_FILE), "w") as fh:
                json.dump(cov, fh)
        write(fake_cov(sha=m))
        sel, why, changed, _ = desktop_pick_cli(d, frm, to, gd, now, days, log="try")
        assert sel is not None and keep in sel and APP_SRC + "app/a.tsx" in changed, (sel, why, changed)
        with open(os.path.join(gd, "desktop-pick.jsonl")) as fh:
            line = json.loads(fh.read().splitlines()[-1])
        assert line["kind"] == "try" and keep in line["picked"] and line["map"] == m, line
        # the working tree: an uncommitted and an untracked change count
        put(APP_SRC + "app/b.tsx", "z\n")
        sel, why, changed, _ = desktop_pick_cli(d, to, None, gd, now, days)
        assert APP_SRC + "app/b.tsx" in changed and sel is not None, (sel, why)
        put(APP_SRC + "app/untracked.tsx", "u\n")
        sel, why, _, _ = desktop_pick_cli(d, to, None, gd, now, days)
        assert sel is None and "untracked.tsx" in why, why
        # the map's sha not an ancestor of from (a map from a later commit, or another branch)
        write(fake_cov(sha=to))
        sel, why, _, _ = desktop_pick_cli(d, frm, to, gd, now, days)
        assert sel is None and "not an ancestor" in why, why
        # no map, an unreadable one, an unknown from
        os.remove(os.path.join(gd, COV_FILE))
        sel, why, _, _ = desktop_pick_cli(d, frm, to, gd, now, days)
        assert sel is None and "no coverage map" in why, why
        with open(os.path.join(gd, COV_FILE), "w") as fh:
            fh.write("{not json")
        sel, why, _, _ = desktop_pick_cli(d, frm, to, gd, now, days)
        assert sel is None and "unreadable" in why, why
        sel, why, _, _ = desktop_pick_cli(d, "f" * 40, to, gd, now, days)
        assert sel is None and "can't diff" in why, why
    finally:
        shutil.rmtree(d, ignore_errors=True)


def desktop_pick_main(args):
    """desktop-pick --from <sha> [--to <sha>] [--log try|land]: the checks on stdout ("ALL <why>",
    or "PICK <n>" then one check per line), the whys on stderr. Shadow mode (no <gate dir>/
    desktop-pick.on): the caller still runs everything; this only says and logs."""
    opts, i = {}, 0
    while i < len(args):
        if args[i] in ("--from", "--to", "--log", "--gate-dir") and i + 1 < len(args):
            opts[args[i][2:]] = args[i + 1]
            i += 2
        else:
            sys.exit("desktop-pick: unknown argument %s" % args[i])
    if "from" not in opts:
        sys.exit("usage: test_map.py desktop-pick --from <sha> [--to <sha>] [--log try|land]")
    gdir = opts.get("gate-dir") or gate_dir()
    import time
    sel, why, changed, _ = desktop_pick_cli(ROOT, opts["from"], opts.get("to"), gdir, time.time(), nightly_days(),
                                            opts.get("log"))
    mode = "on" if os.path.exists(os.path.join(gdir, "desktop-pick.on")) else "shadow (everything still runs)"
    print("# desktop-pick: %d paths changed; mode %s" % (len(changed), mode), file=sys.stderr)
    if sel is None:
        print("ALL " + why)
        return None
    print("PICK %d" % len(sel))
    for s in sorted(sel):
        print(s)
    for w in why:
        print("# " + w, file=sys.stderr)
    return None


def main(argv):
    if argv[1:2] == ["--self-test"]:
        return self_test()
    if argv[1:2] == ["desktop"]:
        why = desktop(argv[2:])
        print("desktop " + why if why else "none")
        return None
    if argv[1:2] == ["desktop-pick"]:
        return desktop_pick_main(argv[2:])
    if argv[1:2] != ["select"]:
        sys.exit("usage: test_map.py select|desktop <changed path>... | desktop-pick --from <sha> [--to <sha>] "
                 "[--log try|land] | --self-test")
    tests, txt = run_all_tests(), texts()
    sel, why = select(argv[2:], tests, txt)
    if sel is None:
        print("ALL " + why)
    else:
        # run_all.sh's order (the slowest-first sort comes after)
        print(" ".join(t for t in tests if t in sel))
        for w in why:
            print("# " + w, file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv)
